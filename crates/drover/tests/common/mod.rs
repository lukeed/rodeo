#![allow(
    dead_code,
    reason = "each test binary uses a different part of the fixture"
)]

use std::cell::RefCell;
use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use doc::Document;
use drover::app::{App, HostInfo, Msg, Options};
use drover::runtime::Runner;
use hc::TestCore;
use ratatui::backend::TestBackend;

pub const WITHIN: Duration = Duration::from_secs(20);

pub fn run<F: Future<Output = ()>>(test: impl FnOnce() -> F) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, test());
}

#[derive(Clone, Default)]
pub struct Sink(Rc<RefCell<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Harness {
    pub core: TestCore,
    pub runner: Runner<TestBackend>,
    pub host: Sink,
    pub outside: hc::Client,
    pub config_path: PathBuf,
}

impl Harness {
    pub async fn start() -> Self {
        Self::with_config("").await
    }

    pub async fn with_config(text: &str) -> Self {
        Self::sized(120, 40, text).await
    }

    pub async fn sized(cols: u16, rows: u16, text: &str) -> Self {
        Self::build(cols, rows, text, None).await
    }

    pub async fn launched_in(folder: &str) -> Self {
        Self::build(120, 40, "", Some(folder)).await
    }

    async fn build(cols: u16, rows: u16, text: &str, folder: Option<&str>) -> Self {
        let core = TestCore::start().await;
        let launch = folder.map_or_else(|| core.root().to_path_buf(), |f| core.root().join(f));
        std::fs::create_dir_all(&launch).expect("a launch folder");
        let dir = core.root().join("config").join("drover");
        std::fs::create_dir_all(&dir).expect("config dir");
        let config_path = dir.join("config.toml");
        std::fs::write(&config_path, text).expect("config file");
        let loaded = config::Config::parse(text, &dir);
        let options = Options {
            env: core.env().clone(),
            config_path: config_path.clone(),
            prefs_path: core.root().join("state").join("drover").join("prefs.json"),
            loaded,
            host: HostInfo {
                cell_px: (8, 16),
                ..HostInfo::default()
            },
            caller: None,
            notes: Vec::new(),
            hostname: "testhost".into(),
            launch_dir: Some(launch.display().to_string()),
        };
        let host = Sink::default();
        let runner = Runner::new(
            options,
            TestBackend::new(cols, rows),
            Box::new(host.clone()),
        )
        .expect("a runner");
        let outside = core.client().await;
        Self {
            core,
            runner,
            host,
            outside,
            config_path,
        }
    }

    pub fn second(&self) -> Runner<TestBackend> {
        let options = Options {
            env: self.core.env().clone(),
            config_path: self.config_path.clone(),
            prefs_path: self
                .core
                .root()
                .join("state")
                .join("drover")
                .join("second.json"),
            loaded: config::Config::load(&self.config_path),
            host: HostInfo {
                cell_px: (8, 16),
                ..HostInfo::default()
            },
            caller: None,
            notes: Vec::new(),
            hostname: "testhost".into(),
            launch_dir: Some(self.core.root().display().to_string()),
        };
        Runner::new(
            options,
            TestBackend::new(100, 30),
            Box::new(Sink::default()),
        )
        .expect("a second runner")
    }

    pub async fn both(&mut self, other: &mut Runner<TestBackend>) {
        let _ = tokio::time::timeout(Duration::from_millis(50), self.runner.step()).await;
        let _ = tokio::time::timeout(Duration::from_millis(50), other.step()).await;
    }

    pub fn app(&self) -> &App {
        &self.runner.app
    }

    pub fn app_mut(&mut self) -> &mut App {
        &mut self.runner.app
    }

    pub fn screen(&mut self) -> String {
        self.runner.draw();
        let buffer = self.runner.terminal.backend().buffer();
        let area = buffer.area;
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    pub fn host_output(&self) -> String {
        String::from_utf8_lossy(&self.host.0.borrow()).into_owned()
    }

    pub async fn until(&mut self, what: &str, done: impl Fn(&mut Self) -> bool) {
        let started = Instant::now();
        loop {
            if done(self) {
                return;
            }
            if started.elapsed() > WITHIN {
                let screen = self.screen();
                panic!("timed out waiting for {what}\n{screen}");
            }
            let _ = tokio::time::timeout(Duration::from_millis(100), self.runner.step()).await;
        }
    }

    pub async fn settle(&mut self) {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(300) {
            let _ = tokio::time::timeout(Duration::from_millis(50), self.runner.step()).await;
        }
    }

    pub fn send(&self, event: Event) {
        let _ = self.runner.sender().send(Msg::Host(event));
    }

    pub async fn press(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.send(Event::Key(KeyEvent::new(code, modifiers)));
        self.pump().await;
    }

    pub async fn pump(&mut self) {
        let _ = tokio::time::timeout(Duration::from_millis(200), self.runner.step()).await;
    }

    pub async fn prefix(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.press(KeyCode::Char('b'), KeyModifiers::CONTROL).await;
        self.press(code, modifiers).await;
    }

    pub async fn prefix_char(&mut self, ch: char) {
        let modifiers = if ch.is_ascii_uppercase() {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        };
        self.prefix(KeyCode::Char(ch), modifiers).await;
    }

    pub async fn typed(&mut self, text: &str) {
        for ch in text.chars() {
            let event = match ch {
                '\r' | '\n' => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                ch if ch.is_ascii_uppercase() => {
                    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::SHIFT)
                }
                ch => KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            };
            self.send(Event::Key(event));
        }
        self.pump().await;
    }

    pub async fn mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
        self.send(Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }));
        self.pump().await;
    }

    pub async fn click(&mut self, column: u16, row: u16) {
        self.mouse(MouseEventKind::Down(MouseButton::Left), column, row)
            .await;
        self.mouse(MouseEventKind::Up(MouseButton::Left), column, row)
            .await;
    }

    pub fn doc(&self) -> Document {
        self.app().machines.local().slot.doc().clone()
    }

    pub async fn stored(&self) -> (Document, u64) {
        let doc = self
            .outside
            .doc_get(doc::NAME, None)
            .await
            .expect("doc.get");
        (
            Document::parse(&doc.value).expect("a valid doc"),
            doc.version,
        )
    }

    pub async fn until_stored(&mut self, what: &str, done: impl Fn(&Document) -> bool) -> Document {
        let started = Instant::now();
        loop {
            let (doc, _) = self.stored().await;
            if done(&doc) && self.app().machines.local().slot.is_settled() {
                return doc;
            }
            if started.elapsed() > WITHIN {
                let screen = self.screen();
                panic!("timed out waiting for the stored doc: {what}\n{doc:#?}\n{screen}");
            }
            let _ = tokio::time::timeout(Duration::from_millis(100), self.runner.step()).await;
        }
    }

    pub async fn ready(&mut self) {
        self.until("a first shell prompt", |h| {
            let panes = h.doc().addresses();
            panes.len() == 1
                && h.app()
                    .panes
                    .iter()
                    .any(|p| p.link == drover::panes::Link::Attached)
                && h.screen().contains('$')
        })
        .await;
    }

    pub fn focused(&self) -> Option<String> {
        self.app().view.focused_pane(&self.app().machines)
    }

    pub fn focused_address(&self) -> Option<String> {
        self.app().focused_address()
    }

    pub fn tab_panes(&self) -> Vec<String> {
        let app = self.app();
        let Some(tab) = app.view.active_tab(&app.machines) else {
            return Vec::new();
        };
        app.machines
            .tab(&tab)
            .map(|t| t.panes().map(|p| p.key()).collect())
            .unwrap_or_default()
    }

    pub fn pane_text(&self, address: &str) -> String {
        let Some(pane) = self.app().panes.get(address) else {
            return String::new();
        };
        let (_, rows) = pane.term.size();
        let top = pane.term.viewport_top();
        (0..u64::from(rows))
            .filter_map(|r| pane.term.row_text(top + r))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
