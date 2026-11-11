use std::collections::BTreeMap;
use std::time::Duration;

use config::{CustomCommand, PopupSize};
use crossterm::event::{MouseEvent, MouseEventKind};
use hc::{AttachOptions, ExecParams, Output, RunParams, Size};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, Widget};
use vt::{Point, Selection, SelectionKind};

use crate::app::{App, Msg};
use crate::clipboard::shell_quote;
use crate::ids::{local, qualify};
use crate::panes::{Link, Pane};
use crate::render::Cursor;
use crate::view::Mode;

pub const LABEL: &str = "rodeo.popup";
pub const OWNER: &str = "rodeo.owner";
const QUIT_WAIT: Duration = Duration::from_secs(2);
const MAKE_TEMP: &str = r#"umask 077
f=$(mktemp "${TMPDIR:-/tmp}/rodeo-scrollback.XXXXXX") || exit 1
cat > "$f" || exit 1
printf %s "$f""#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub outer: Rect,
    pub inner: Rect,
}

pub fn geometry(
    width: Option<PopupSize>,
    height: Option<PopupSize>,
    area: Rect,
) -> Option<Geometry> {
    let outer_w = width
        .map_or(area.width / 2, |w| w.resolve(area.width))
        .max(6)
        .min(area.width);
    let outer_h = height
        .map_or(area.height / 2, |h| h.resolve(area.height))
        .max(4)
        .min(area.height);
    if outer_w < 6 || outer_h < 4 {
        return None;
    }
    let outer = Rect::new(
        area.x + (area.width - outer_w) / 2,
        area.y + (area.height - outer_h) / 2,
        outer_w,
        outer_h,
    );
    let inner_w = outer_w - 2;
    let cols = if inner_w <= 4 { inner_w } else { inner_w - 1 };
    Some(Geometry {
        outer,
        inner: Rect::new(outer.x + 1, outer.y + 1, cols, outer_h - 2),
    })
}

pub fn scrollback_text(term: &vt::Terminal) -> String {
    let (cols, _) = term.size();
    let (first, end) = (term.first_row(), term.end_row());
    if end <= first || cols == 0 {
        return String::new();
    }
    let selection = Selection::new(
        Point::new(first, 0),
        Point::new(end - 1, cols - 1),
        SelectionKind::Char,
    );
    let mut text = term.selection_text(&selection);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

pub fn editor_command(path: &str) -> String {
    format!(
        r#"scrollback_file={}; eval "${{EDITOR:-vi}} \"\$scrollback_file\""; status=$?; rm -f "$scrollback_file"; exit $status"#,
        shell_quote(path)
    )
}

pub type After = Box<dyn FnOnce(&mut App, Option<i32>)>;

pub struct Spec {
    pub argv: Vec<String>,
    pub machine: Option<String>,
    pub cwd: Option<String>,
    pub env: BTreeMap<String, String>,
    pub title: String,
    pub width: Option<PopupSize>,
    pub height: Option<PopupSize>,
    pub after: Option<After>,
}

pub struct Popup {
    pub title: String,
    pub width: Option<PopupSize>,
    pub height: Option<PopupSize>,
    pub pane: Option<Pane>,
    pub geometry: Option<Geometry>,
    seq: u64,
    exit: Option<i32>,
    after: Option<After>,
}

impl Popup {
    pub fn address(&self) -> Option<String> {
        self.pane.as_ref().map(|p| p.address.clone())
    }
}

impl App {
    pub fn active_env(&self) -> (BTreeMap<String, String>, Option<String>) {
        let machine = self.active_machine();
        let mut env = BTreeMap::new();
        if let Some(space) = &self.view.space {
            env.insert(
                "HERDR_ACTIVE_WORKSPACE_ID".to_owned(),
                local(space, machine.as_deref()).to_owned(),
            );
        }
        if let Some(tab) = self.view.active_tab(&self.machines) {
            env.insert(
                "HERDR_ACTIVE_TAB_ID".to_owned(),
                local(&tab, machine.as_deref()).to_owned(),
            );
        }
        let pane = self.view.focused_pane(&self.machines);
        if let Some(pane) = &pane {
            env.insert(
                "HERDR_ACTIVE_PANE_ID".to_owned(),
                local(pane, machine.as_deref()).to_owned(),
            );
        }
        let cwd = pane.as_deref().and_then(|p| self.live_cwd(p));
        if let Some(cwd) = &cwd {
            env.insert("HERDR_ACTIVE_PANE_CWD".to_owned(), cwd.clone());
        }
        (env, cwd)
    }

    fn popup_cwd(&self, live: Option<String>) -> Option<String> {
        let machine = self.active_machine();
        live.or_else(|| {
            self.view
                .space
                .as_deref()
                .and_then(|s| self.machines.space(s))
                .map(|s| s.space.folder.clone())
                .filter(|f| !f.is_empty())
        })
        .or_else(|| machine.is_none().then(|| self.home()))
    }

    pub fn open_popup(&mut self, command: &CustomCommand) {
        if self.view.space.is_none() {
            self.show_error("no active workspace");
            return;
        }
        let (env, live) = self.active_env();
        let cwd = self.popup_cwd(live);
        let spec = Spec {
            argv: vec!["/bin/sh".into(), "-c".into(), command.command.clone()],
            machine: self.active_machine(),
            cwd,
            env,
            title: "popup".to_owned(),
            width: command.width,
            height: command.height,
            after: None,
        };
        if let Err(error) = self.start_popup(spec) {
            self.show_error(error);
        }
    }

    pub fn edit_scrollback(&mut self) {
        let Some(pane_key) = self.view.focused_pane(&self.machines) else {
            return;
        };
        let Some(address) = self.focused_address() else {
            return;
        };
        let Some(text) = self.panes.get(&address).map(|p| scrollback_text(&p.term)) else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let machine = self
            .machines
            .pane(&pane_key)
            .and_then(|p| p.machine.map(str::to_owned));
        let (mut env, live) = self.active_env();
        let cwd = self.popup_cwd(live);
        if let Some(editor) = self.env.get("EDITOR").filter(|e| !e.is_empty()) {
            env.insert("EDITOR".to_owned(), editor.to_string_lossy().into_owned());
        }
        let params = ExecParams {
            argv: vec!["sh".into(), "-c".into(), MAKE_TEMP.into()],
            on: machine.clone(),
            input: Some(text.into_bytes()),
            timeout: Some(Duration::from_secs(30)),
            ..ExecParams::default()
        };
        self.spawn(async move {
            let result = client.exec(&params).await;
            move |app: &mut App| {
                let path = match result {
                    Ok(exec) if exec.success() => {
                        String::from_utf8_lossy(&exec.stdout).into_owned()
                    }
                    Ok(exec) => {
                        let why = String::from_utf8_lossy(&exec.stderr).trim().to_owned();
                        app.show_error(format!("cannot save the scrollback: {why}"));
                        return;
                    }
                    Err(error) => {
                        app.show_error(format!("cannot save the scrollback: {error}"));
                        return;
                    }
                };
                let started = app.start_popup(Spec {
                    argv: vec!["/bin/sh".into(), "-c".into(), editor_command(&path)],
                    machine,
                    cwd,
                    env,
                    title: String::new(),
                    width: Some(PopupSize::Percent(100)),
                    height: Some(PopupSize::Percent(100)),
                    after: None,
                });
                if let Err(error) = started {
                    app.show_error(error);
                }
            }
        });
    }

    pub fn start_popup(&mut self, spec: Spec) -> Result<(), &'static str> {
        if self.popup.is_some() {
            return Err("popup already open");
        }
        let Some(client) = self.client() else {
            return Err("hc is not connected");
        };
        let Some(geometry) = geometry(spec.width, spec.height, self.regions.surface) else {
            return Err("terminal area too small for popup");
        };
        let size = (geometry.inner.width, geometry.inner.height);
        self.popup_seq += 1;
        let seq = self.popup_seq;
        self.popup = Some(Popup {
            title: spec.title,
            width: spec.width,
            height: spec.height,
            pane: None,
            geometry: Some(geometry),
            seq,
            exit: None,
            after: spec.after,
        });
        self.view.overlay = None;
        self.view.mode = Mode::Terminal;
        let params = RunParams {
            argv: spec.argv,
            size: Some(Size {
                cols: size.0.max(2),
                rows: size.1.max(1),
            }),
            display: Some(self.display()).filter(|d| !d.is_empty()),
            cwd: spec.cwd,
            on: spec.machine.clone(),
            env: spec.env,
            labels: BTreeMap::from([
                (LABEL.to_owned(), "1".to_owned()),
                (OWNER.to_owned(), self.popup_owner()),
            ]),
            rm: true,
            ..RunParams::default()
        };
        let machine = spec.machine;
        self.spawn(async move {
            let result = client.run(&params).await;
            move |app: &mut App| match result {
                Ok(terminal) => {
                    let id = local(&terminal.address, machine.as_deref());
                    app.popup_started(seq, qualify(id, machine.as_deref()), size);
                }
                Err(error) => {
                    app.show_error(format!("cannot start the popup: {error}"));
                    app.close_popup(seq);
                }
            }
        });
        Ok(())
    }

    fn popup_owner(&self) -> String {
        format!("{}:{}", self.hostname, std::process::id())
    }

    pub fn sweep_popups(&mut self) {
        let strays: Vec<String> = self
            .terminals
            .iter()
            .filter(|t| t.is_live() && t.label(LABEL).is_some())
            .filter(|t| {
                t.label(OWNER)
                    .and_then(|owner| owner.rsplit_once(':'))
                    .is_some_and(|(host, pid)| {
                        host == self.hostname && pid.parse().is_ok_and(|pid| !running(pid))
                    })
            })
            .map(|t| t.address.clone())
            .collect();
        for address in strays {
            tracing::info!(%address, "ending a popup left by a gone rodeo");
            self.kill(address);
        }
    }

    pub async fn end_popup(&mut self) {
        let (Some(address), Some(client)) =
            (self.popup.as_ref().and_then(Popup::address), self.client())
        else {
            return;
        };
        if let Ok(Err(error)) = tokio::time::timeout(QUIT_WAIT, client.kill(&address)).await
            && !error.is_not_found()
            && !error.is_finished()
        {
            tracing::warn!(%address, %error, "cannot end the popup");
        }
    }

    fn popup_started(&mut self, seq: u64, address: String, size: (u16, u16)) {
        let options = self.vt_options();
        let cell_px = self.host.cell_px;
        let Some(popup) = self.popup.as_mut().filter(|p| p.seq == seq) else {
            self.kill(address);
            return;
        };
        let mut pane = match Pane::new(address.clone(), size, options) {
            Ok(pane) => pane,
            Err(error) => {
                self.show_error(format!("cannot start the popup: {error}"));
                self.close_popup(seq);
                self.kill(address);
                return;
            }
        };
        let _ = pane.term.resize(size.0, size.1, cell_px);
        pane.shown = true;
        pane.link = Link::Attaching;
        popup.pane = Some(pane);
        self.attach_popup(seq, address, size);
    }

    fn attach_popup(&mut self, seq: u64, address: String, size: (u16, u16)) {
        let Some(client) = self.client() else {
            return;
        };
        let request = AttachOptions {
            display: self.display(),
            caller: self.caller.clone(),
            ..AttachOptions::new(address, size.0, size.1)
        };
        let tx = self.tx.clone();
        tokio::task::spawn_local(async move {
            let send = |apply: Box<dyn FnOnce(&mut App)>| {
                tx.send(Msg::Apply {
                    generation: None,
                    apply,
                })
                .is_ok()
            };
            match client.attach(&request).await {
                Ok(attached) => {
                    let (mut reader, writer) = attached.split();
                    if !send(Box::new(move |app: &mut App| {
                        app.popup_attached(seq, writer);
                    })) {
                        return;
                    }
                    loop {
                        let output = reader.next().await;
                        let end = matches!(output, None | Some(Output::Closed(_)));
                        if !send(Box::new(move |app: &mut App| app.popup_output(seq, output)))
                            || end
                        {
                            return;
                        }
                    }
                }
                Err(error) => {
                    send(Box::new(move |app: &mut App| {
                        app.close_popup(seq);
                        app.show_error(format!("cannot show the popup: {error}"));
                    }));
                }
            }
        });
    }

    fn popup_pane(&mut self, seq: u64) -> Option<&mut Pane> {
        self.popup
            .as_mut()
            .filter(|p| p.seq == seq)
            .and_then(|p| p.pane.as_mut())
    }

    fn popup_attached(&mut self, seq: u64, writer: hc::AttachWriter) {
        let Some(pane) = self.popup_pane(seq) else {
            let _ = writer.close();
            return;
        };
        pane.writer = Some(writer);
        pane.link = Link::Attached;
        pane.led = Some(pane.size);
        pane.reply();
        pane.focus(true);
    }

    fn popup_output(&mut self, seq: u64, output: Option<Output>) {
        let Some(pane) = self.popup_pane(seq) else {
            return;
        };
        match output {
            Some(Output::Bytes(bytes)) => {
                pane.feed(&bytes);
                for event in pane.term.take_events() {
                    match event {
                        vt::Event::Bell => self.out.push(0x07),
                        vt::Event::Clipboard(text) => self.copy_to_host(&text),
                        _ => {}
                    }
                }
            }
            Some(Output::Resize { .. }) => {}
            Some(Output::Closed(ending)) => {
                if let Some(popup) = self.popup.as_mut() {
                    popup.exit = ending.exit_code;
                }
                self.close_popup(seq);
            }
            None => self.close_popup(seq),
        }
    }

    pub fn close_popup(&mut self, seq: u64) {
        let Some(mut popup) = self.popup.take() else {
            return;
        };
        if popup.seq != seq {
            self.popup = Some(popup);
            return;
        }
        if let Some(pane) = popup.pane.as_mut() {
            pane.detach();
        }
        self.view.overlay = None;
        self.view.mode = if self.view.space.is_some() {
            Mode::Terminal
        } else {
            Mode::Navigate
        };
        self.relayout();
        if let Some(after) = popup.after.take() {
            after(self, popup.exit);
        }
    }

    pub fn fit_popup(&mut self) {
        let surface = self.regions.surface;
        let cell_px = self.host.cell_px;
        let Some(popup) = self.popup.as_mut() else {
            return;
        };
        popup.geometry = geometry(popup.width, popup.height, surface);
        let Some((geometry, pane)) = popup.geometry.zip(popup.pane.as_mut()) else {
            return;
        };
        pane.resize((geometry.inner.width, geometry.inner.height), cell_px);
        pane.lead();
    }

    pub fn popup_key(&mut self, key: &crossterm::event::KeyEvent) -> bool {
        let Some(popup) = self.popup.as_mut() else {
            return false;
        };
        let Some(pane) = popup.pane.as_mut() else {
            return true;
        };
        if self.keymap.is_image_paste(*key) {
            self.image_paste(Some(*key));
            return true;
        }
        let bytes = pane.term.key(key);
        if !bytes.is_empty() {
            pane.term.scroll_to_bottom();
            pane.input(&bytes);
        }
        true
    }

    pub fn popup_paste(&mut self, text: &str) -> bool {
        let Some(popup) = self.popup.as_mut() else {
            return false;
        };
        let Some(pane) = popup.pane.as_mut() else {
            return true;
        };
        if text.is_empty() {
            self.image_paste(None);
            return true;
        }
        let bytes = pane.term.paste(text);
        pane.term.scroll_to_bottom();
        pane.input(&bytes);
        true
    }

    pub fn popup_mouse(&mut self, event: MouseEvent) -> bool {
        let lines = isize::try_from(self.config.ui.mouse_scroll_lines()).unwrap_or(3);
        let Some(popup) = self.popup.as_mut() else {
            return false;
        };
        let (Some(geometry), Some(pane)) = (popup.geometry, popup.pane.as_mut()) else {
            return true;
        };
        let inner = geometry.inner;
        if !inner.contains(Position::new(event.column, event.row)) {
            return true;
        }
        let (col, row) = (event.column - inner.x, event.row - inner.y);
        if let Some(bytes) = pane.term.mouse(&event, col, row) {
            pane.input(&bytes);
            return true;
        }
        match event.kind {
            MouseEventKind::ScrollUp => pane.term.scroll(-lines),
            MouseEventKind::ScrollDown => pane.term.scroll(lines),
            _ => {}
        }
        true
    }
}

fn running(pid: i32) -> bool {
    rustix::process::Pid::from_raw(pid).is_some_and(|pid| {
        matches!(
            rustix::process::test_kill_process(pid),
            Ok(()) | Err(rustix::io::Errno::PERM)
        )
    })
}

pub fn render(app: &mut App, buf: &mut Buffer) -> Option<Cursor> {
    app.fit_popup();
    let popup = app.popup.as_ref()?;
    let geometry = popup.geometry?;
    let p = &app.palette;
    Clear.render(geometry.outer, buf);
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(p.accent))
        .title(popup.title.as_str())
        .style(Style::new().bg(p.panel_bg))
        .render(geometry.outer, buf);
    let pane = popup.pane.as_ref()?;
    let rendered = pane
        .term
        .render(geometry.inner, buf, &vt::RenderOptions::default());
    rendered
        .cursor
        .filter(|_| app.view.overlay.is_none())
        .map(|c| Cursor {
            x: c.x,
            y: c.y,
            shape: c.shape,
            blinking: c.blinking,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_popup_is_half_the_surface() {
        let g = geometry(None, None, Rect::new(10, 2, 100, 40)).expect("a popup");
        assert_eq!(g.outer, Rect::new(35, 12, 50, 20));
        assert_eq!(g.inner, Rect::new(36, 13, 47, 18));
    }

    #[test]
    fn sizes_clamp_to_the_surface() {
        let g = geometry(
            Some(PopupSize::Percent(100)),
            Some(PopupSize::Cells(500)),
            Rect::new(0, 0, 80, 24),
        )
        .expect("a popup");
        assert_eq!(g.outer, Rect::new(0, 0, 80, 24));
        let tiny =
            geometry(Some(PopupSize::Cells(1)), None, Rect::new(0, 0, 8, 6)).expect("a popup");
        assert_eq!((tiny.outer.width, tiny.inner.width), (6, 4));
        assert!(geometry(None, None, Rect::new(0, 0, 5, 3)).is_none());
    }

    #[test]
    fn editor_command_quotes_the_path_and_falls_back_to_vi() {
        let command = editor_command("/tmp/a b");
        assert!(command.starts_with("scrollback_file='/tmp/a b';"));
        assert!(command.contains("${EDITOR:-vi}"));
        assert!(command.contains(r#"rm -f "$scrollback_file""#));
    }
}
