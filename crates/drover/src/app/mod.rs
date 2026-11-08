mod attach;
mod conn;
mod docs;
mod extras;
mod notify;
mod reconcile;
mod resume;
mod snapshot;
mod spawn;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use config::{Config, Keymap, Palette, Preferences};
use crossterm::event::Event;
use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedSender;

use crate::hits::Hits;
use crate::layout::{Regions, TabGeom};
use crate::machines::Machines;
use crate::panes::Panes;
use crate::view::View;

pub use conn::{Conn, State as ConnState};
pub use extras::{Files, Git, Listing, strip_controls};
pub use spawn::Spawn;

pub type Apply = Box<dyn FnOnce(&mut App)>;

pub enum Msg {
    Host(Event),
    Output {
        address: String,
        attach: u64,
        output: Option<hc::Output>,
    },
    Notice {
        generation: u64,
        notice: hc::Notice,
    },
    Apply {
        generation: Option<u64>,
        apply: Apply,
    },
    Tick,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostInfo {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub cursor: Option<Rgb>,
    pub cell_px: (u32, u32),
}

pub struct Options {
    pub env: hc::Env,
    pub config_path: PathBuf,
    pub prefs_path: PathBuf,
    pub loaded: config::Loaded,
    pub host: HostInfo,
    pub caller: Option<String>,
    pub notes: Vec<String>,
    pub hostname: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub text: String,
    pub until: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub dot: NoticeDot,
    pub until: Instant,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeDot {
    Red,
    Yellow,
    Blue,
    Accent,
}

pub struct App {
    pub env: hc::Env,
    pub config_path: PathBuf,
    pub prefs_path: PathBuf,
    pub config: Config,
    pub keymap: Keymap,
    pub palette: Palette,
    pub diagnostics: Vec<String>,
    pub prefs: Preferences,
    pub host: HostInfo,
    pub caller: Option<String>,
    pub hostname: String,
    pub conn: Conn,
    pub terminals: hc::Terminals,
    pub terminals_ready: bool,
    pub machines: Machines,
    pub panes: Panes,
    pub view: View,
    pub screen: Rect,
    pub regions: Regions,
    pub geoms: HashMap<String, TabGeom>,
    pub hits: Hits,
    pub out: Vec<u8>,
    pub feedback: Option<Feedback>,
    pub notice: Option<Notice>,
    pub error: Option<(String, Instant)>,
    pub git: HashMap<String, Git>,
    pub files: Files,
    pub status: extras::Status,
    pub tx: UnboundedSender<Msg>,
    pub quit: bool,
    pub dirty: bool,
    pub ticks: u64,
    pub full_redraw: bool,
    born: HashMap<String, Instant>,
    launching: HashSet<(Option<String>, String, String)>,
    adopting: HashSet<String>,
    checking: HashSet<String>,
    killing: HashSet<String>,
    looked: HashMap<String, u64>,
    written_where: HashMap<String, String>,
    first_space: bool,
    title: Option<String>,
    cursor_shape: Option<vt::CursorShape>,
    mouse_captured: Option<bool>,
    resume: Option<resume::Resume>,
}

const FEEDBACK: Duration = Duration::from_secs(2);
const ERROR: Duration = Duration::from_secs(5);

impl App {
    pub fn new(options: Options, tx: UnboundedSender<Msg>) -> Self {
        let config = options.loaded.config;
        let build = config.keymap();
        let prefs = Preferences::load(&options.prefs_path);
        let resume = resume::Resume::load(&resume::Resume::path(&options.prefs_path));
        let palette = config.palette(None);
        let mut app = Self {
            env: options.env,
            config_path: options.config_path,
            prefs_path: options.prefs_path,
            keymap: build.keymap,
            palette,
            diagnostics: options.loaded.diagnostics,
            prefs,
            host: options.host,
            caller: options.caller,
            hostname: options.hostname,
            conn: Conn::default(),
            terminals: hc::Terminals::default(),
            terminals_ready: false,
            machines: Machines::default(),
            panes: Panes::default(),
            view: View::default(),
            screen: Rect::default(),
            regions: Regions::default(),
            geoms: HashMap::new(),
            hits: Hits::default(),
            out: Vec::new(),
            feedback: None,
            notice: None,
            error: None,
            git: HashMap::new(),
            files: Files::default(),
            status: extras::Status::default(),
            tx,
            quit: false,
            dirty: true,
            ticks: 0,
            full_redraw: false,
            born: HashMap::new(),
            launching: HashSet::new(),
            adopting: HashSet::new(),
            checking: HashSet::new(),
            killing: HashSet::new(),
            looked: HashMap::new(),
            written_where: HashMap::new(),
            first_space: false,
            title: None,
            cursor_shape: None,
            mouse_captured: None,
            resume,
            config,
        };
        for note in options.notes {
            app.toast_feedback(note);
        }
        app
    }

    pub fn start(&mut self, screen: Rect) {
        self.screen = screen;
        self.connect();
    }

    pub fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Host(event) => self.host_event(event),
            Msg::Output {
                address,
                attach,
                output,
            } => self.output(&address, attach, output),
            Msg::Notice { generation, notice } => {
                if generation == self.conn.generation {
                    self.notice_event(notice);
                }
            }
            Msg::Apply { generation, apply } => {
                if generation.is_none_or(|g| g == self.conn.generation) {
                    apply(self);
                }
            }
            Msg::Tick => self.tick(),
            Msg::Quit => self.quit = true,
        }
        self.dirty = true;
    }

    pub fn spawn<F, R>(&self, future: F)
    where
        F: Future<Output = R> + 'static,
        R: FnOnce(&mut App) + 'static,
    {
        let tx = self.tx.clone();
        let generation = Some(self.conn.generation);
        tokio::task::spawn_local(async move {
            let apply = future.await;
            let _ = tx.send(Msg::Apply {
                generation,
                apply: Box::new(apply),
            });
        });
    }

    pub fn later(&self, after: Duration, apply: impl FnOnce(&mut App) + 'static) {
        let tx = self.tx.clone();
        tokio::task::spawn_local(async move {
            tokio::time::sleep(after).await;
            let _ = tx.send(Msg::Apply {
                generation: None,
                apply: Box::new(apply),
            });
        });
    }

    pub fn client(&self) -> Option<hc::Client> {
        self.conn.client.clone()
    }

    pub fn home(&self) -> String {
        self.env.home().to_string_lossy().into_owned()
    }

    pub fn toast_feedback(&mut self, text: impl Into<String>) {
        self.feedback = Some(Feedback {
            text: text.into(),
            until: Instant::now() + FEEDBACK,
        });
    }

    pub fn show_error(&mut self, text: impl Into<String>) {
        let text = text.into();
        tracing::warn!(error = %text, "shown to the user");
        self.error = Some((text, Instant::now() + ERROR));
    }

    pub fn show_notice(
        &mut self,
        title: &str,
        body: &str,
        dot: NoticeDot,
        target: Option<String>,
        ttl: Duration,
    ) {
        self.notice = Some(Notice {
            title: title.to_owned(),
            body: body.to_owned(),
            dot,
            until: Instant::now() + ttl,
            target,
        });
    }

    fn tick(&mut self) {
        self.ticks += 1;
        let now = Instant::now();
        if self.feedback.as_ref().is_some_and(|f| f.until <= now) {
            self.feedback = None;
        }
        if self.notice.as_ref().is_some_and(|n| n.until <= now) {
            self.notice = None;
        }
        if self.error.as_ref().is_some_and(|(_, until)| *until <= now) {
            self.error = None;
        }
        if let Some(selection) = &self.view.selection
            && selection.clear_at.is_some_and(|at| at <= now)
        {
            self.view.selection = None;
        }
        self.born
            .retain(|_, at| now.duration_since(*at) < Duration::from_secs(30));
        if self.ticks.is_multiple_of(10) {
            self.poll_machines();
            self.retry_docs();
        }
        if self.ticks.is_multiple_of(4) {
            self.reconcile();
            self.refresh_extras();
        }
        self.write_seen();
        self.conn_tick();
    }

    pub fn save_prefs(&mut self) {
        if let Err(error) = self.prefs.save(&self.prefs_path) {
            tracing::warn!(%error, path = %self.prefs_path.display(), "cannot save preferences");
        }
    }

    pub fn reload_config(&mut self) {
        let reload = self.config.reload(&self.config_path);
        let status = reload.status;
        let keys_kept = reload
            .diagnostics
            .iter()
            .any(|d| d.contains("kept current keybinds"));
        self.config = reload.config;
        let build = self.config.keymap();
        if build.prefix_error.is_none() && !keys_kept {
            self.keymap = build.keymap;
        }
        self.palette = self.config.palette(None);
        self.diagnostics = reload.diagnostics;
        self.mouse_captured = None;
        let detail = match status {
            config::ReloadStatus::Applied => format!(
                "using {}",
                self.config_path
                    .file_name()
                    .map_or_else(|| "config.toml".into(), |n| n.to_string_lossy())
            ),
            config::ReloadStatus::Partial | config::ReloadStatus::Failed => {
                "with warnings".to_owned()
            }
        };
        if status == config::ReloadStatus::Failed {
            self.show_error("config reload failed; keeping current config");
        } else {
            self.toast_feedback(format!("reloaded config · {detail}"));
        }
        self.relayout();
    }

    pub fn banner(&self) -> Option<String> {
        config::summary(&self.diagnostics, &self.config_path)
    }

    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out)
    }

    pub fn set_cursor_shape(&mut self, shape: vt::CursorShape, blinking: bool) {
        if self.cursor_shape == Some(shape) {
            return;
        }
        self.cursor_shape = Some(shape);
        let code = match shape {
            vt::CursorShape::Block | vt::CursorShape::BlockHollow => 1,
            vt::CursorShape::Underline => 3,
            vt::CursorShape::Bar => 5,
        } + u8::from(!blinking);
        self.out
            .extend_from_slice(format!("\x1b[{code} q").as_bytes());
    }

    pub fn update_title(&mut self) {
        let Some(template) = self.config.window_title() else {
            return;
        };
        let home = self.home();
        let workspace = self
            .view
            .space
            .clone()
            .map(|s| self.space_name(&s, &home))
            .unwrap_or_default();
        let tab = self
            .view
            .active_tab(&self.machines)
            .and_then(|t| self.machines.tab(&t).map(|t| t.name()))
            .unwrap_or_default();
        let focused = self.view.focused_pane(&self.machines);
        let pane = focused
            .as_deref()
            .and_then(|p| self.machines.pane(p))
            .and_then(|p| p.node.title.clone())
            .unwrap_or_default();
        let terminal_title = self
            .focused_address()
            .and_then(|a| self.terminals.get(&a).map(|t| t.title.clone()))
            .map(|t| crate::agents::strip_activity(&t).to_owned())
            .unwrap_or_default();
        let hostname = self
            .active_machine()
            .and_then(|m| {
                self.machines
                    .get(Some(&m))
                    .and_then(|m| m.info.as_ref().map(|i| i.host.clone()))
            })
            .unwrap_or_else(|| self.hostname.clone());
        let title = template.render(&config::TitleValues {
            hostname: &hostname,
            workspace: &workspace,
            tab: &tab,
            pane: &pane,
            terminal_title: &terminal_title,
        });
        let title = title.unwrap_or_else(|| "drover".to_owned());
        if self.title.as_deref() == Some(title.as_str()) {
            return;
        }
        self.out
            .extend_from_slice(format!("\x1b]0;{title}\x07").as_bytes());
        self.title = Some(title);
    }

    pub fn shutdown(&mut self) {
        self.save_view();
        for pane in self.panes.iter_mut() {
            pane.detach();
        }
    }
}
