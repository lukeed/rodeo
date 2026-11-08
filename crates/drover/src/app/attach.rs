use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use base64::Engine;
use crossterm::Command;
use doc::{Cell, Document};
use hc::{AttachOptions, Output};

use super::{App, Msg};
use crate::ids::{is_pending, local, split};
use crate::layout::{self, Chrome, SidebarWidth};
use crate::panes::{Link, Pane};
use crate::sync::Edit;
use crate::view::Drag;

const HISTORY: usize = 2000;
const EXIT_REASONS: [&str; 3] = ["exited", "killed", "finished"];

impl App {
    pub fn vt_options(&self) -> vt::Options {
        vt::Options {
            scrollback_bytes: self.config.advanced.scrollback_limit_bytes,
            image_bytes: if self.config.kitty_graphics() {
                vt::Options::default().image_bytes
            } else {
                0
            },
        }
    }

    pub fn cell(&self) -> Cell {
        let (w, h) = self.host.cell_px;
        Cell {
            width: u16::try_from(w).unwrap_or(8).max(1),
            height: u16::try_from(h).unwrap_or(16).max(1),
        }
    }

    pub fn display(&self) -> hc::Display {
        let (w, h) = self.host.cell_px;
        hc::Display {
            fg: self.host.fg.map(super::Rgb::hex),
            bg: self.host.bg.map(super::Rgb::hex),
            cursor: self.host.cursor.map(super::Rgb::hex),
            cell: (w > 0 && h > 0).then_some([w, h]),
        }
    }

    pub fn sync_panes(&mut self) {
        let wanted: Vec<String> = self.machines.panes().filter_map(|p| p.address()).collect();
        let keep: HashSet<&String> = wanted.iter().collect();
        for address in self.panes.addresses() {
            if !keep.contains(&address)
                && let Some(mut pane) = self.panes.remove(&address)
            {
                pane.detach();
                if self
                    .view
                    .copy
                    .as_ref()
                    .is_some_and(|c| c.address == address)
                {
                    self.view.copy = None;
                    self.view.mode = crate::view::Mode::Terminal;
                }
                if self
                    .view
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.address == address)
                {
                    self.view.selection = None;
                }
            }
        }
        for address in wanted {
            if self.panes.contains(&address) {
                continue;
            }
            let size = self
                .terminals
                .get(&address)
                .filter(|t| t.cols > 0 && t.rows > 0)
                .map_or((80, 24), |t| (t.cols, t.rows));
            match Pane::new(address.clone(), size, self.vt_options()) {
                Ok(pane) => self.panes.insert(pane),
                Err(error) => tracing::error!(%address, %error, "cannot make a terminal"),
            }
        }
        self.relayout();
        self.attach_idle();
    }

    fn attach_idle(&mut self) {
        if self.client().is_none() || !self.terminals_ready {
            return;
        }
        let idle: Vec<String> = self
            .panes
            .iter()
            .filter(|p| p.link == Link::Idle && !is_pending(&p.address))
            .map(|p| p.address.clone())
            .collect();
        for address in idle {
            match self.terminals.get(&address) {
                Some(t) if t.is_finished() => {
                    let reason = t.reason.clone().unwrap_or_else(|| "finished".into());
                    if let Some(pane) = self.panes.get_mut(&address) {
                        pane.end(Link::Ended(reason));
                    }
                }
                Some(t) if t.is_lost() => {
                    let reason = t.reason.clone().unwrap_or_default();
                    if let Some(pane) = self.panes.get_mut(&address) {
                        pane.end(Link::Lost(reason));
                    }
                }
                _ => self.attach(&address),
            }
        }
    }

    pub(super) fn retry_attaches(&mut self) {
        let failed: Vec<String> = self
            .panes
            .iter()
            .filter(|p| matches!(p.link, Link::Failed(_)))
            .map(|p| p.address.clone())
            .collect();
        for address in failed {
            if let Some(pane) = self.panes.get_mut(&address) {
                pane.link = Link::Idle;
            }
        }
        self.attach_idle();
    }

    pub fn reattach(&mut self, address: &str) {
        if let Some(pane) = self.panes.get_mut(address)
            && !matches!(pane.link, Link::Attached | Link::Attaching)
        {
            pane.detach();
            pane.link = Link::Idle;
            self.attach(address);
        }
    }

    fn attach(&mut self, address: &str) {
        let Some(client) = self.client() else {
            return;
        };
        let options = self.vt_options();
        let display = self.display();
        let caller = self.caller.clone();
        let Some(pane) = self.panes.get_mut(address) else {
            return;
        };
        if let Ok(term) = vt::Terminal::new(pane.size.0, pane.size.1, options) {
            pane.term = term;
            let _ = pane
                .term
                .resize(pane.size.0, pane.size.1, self.host.cell_px);
        }
        pane.attach += 1;
        pane.link = Link::Attaching;
        pane.led = None;
        pane.focus_sent = None;
        let attach = pane.attach;
        let request = AttachOptions {
            history: HISTORY,
            resumed: !pane.shown,
            display,
            caller,
            ..AttachOptions::new(address, pane.size.0, pane.size.1)
        };
        let shown_size = pane.shown.then_some(pane.size);
        let tx = self.tx.clone();
        let generation = Some(self.conn.generation);
        let address = address.to_owned();
        tokio::task::spawn_local(async move {
            let apply = |apply: Box<dyn FnOnce(&mut App)>| Msg::Apply { generation, apply };
            match client.attach(&request).await {
                Ok(attached) => {
                    let (mut reader, writer) = attached.split();
                    let target = address.clone();
                    let _ = tx.send(apply(Box::new(move |app: &mut App| {
                        app.attached(&target, attach, Ok(writer), shown_size);
                    })));
                    loop {
                        let output = reader.next().await;
                        let end = matches!(output, None | Some(Output::Closed(_)));
                        if tx
                            .send(Msg::Output {
                                address: address.clone(),
                                attach,
                                output,
                            })
                            .is_err()
                            || end
                        {
                            return;
                        }
                    }
                }
                Err(error) => {
                    let _ = tx.send(apply(Box::new(move |app: &mut App| {
                        app.attached(&address, attach, Err(error), shown_size);
                    })));
                }
            }
        });
    }

    fn attached(
        &mut self,
        address: &str,
        attach: u64,
        result: Result<hc::AttachWriter, hc::HcError>,
        shown_size: Option<(u16, u16)>,
    ) {
        let Some(pane) = self.panes.get_mut(address) else {
            if let Ok(writer) = result {
                let _ = writer.close();
            }
            return;
        };
        if pane.attach != attach {
            if let Ok(writer) = result {
                let _ = writer.close();
            }
            return;
        }
        match result {
            Ok(writer) => {
                pane.connect(writer);
                pane.led = shown_size;
                pane.reply();
                pane.lead();
                self.relayout();
            }
            Err(error) => {
                tracing::debug!(%address, %error, "attach failed");
                pane.end(if error.is_not_found() || error.is_finished() {
                    Link::Ended(error.kind().unwrap_or("finished").to_owned())
                } else if error.is_unavailable() {
                    Link::Lost(error.to_string())
                } else {
                    Link::Failed(error.to_string())
                });
                self.reconcile();
            }
        }
    }

    pub(super) fn output(&mut self, address: &str, attach: u64, output: Option<Output>) {
        let Some(pane) = self.panes.get_mut(address) else {
            return;
        };
        if pane.attach != attach {
            return;
        }
        match output {
            Some(Output::Bytes(bytes)) => {
                pane.term.feed(&bytes);
                pane.reply();
                let events = pane.term.take_events();
                self.vt_events(address, events);
            }
            Some(Output::Resize { .. }) => {}
            Some(Output::Closed(ending)) => {
                pane.writer = None;
                pane.led = None;
                pane.focus_sent = None;
                if ending.is_detach() {
                    pane.link = Link::Idle;
                    return;
                }
                let reason = ending.reason.clone().unwrap_or_default();
                if EXIT_REASONS.contains(&reason.as_str()) {
                    pane.end(Link::Ended(reason));
                    self.remove_terminal(address);
                } else {
                    pane.end(Link::Lost(reason));
                }
            }
            None => {
                pane.writer = None;
                pane.link = Link::Idle;
            }
        }
    }

    fn vt_events(&mut self, address: &str, events: Vec<vt::Event>) {
        for event in events {
            match event {
                vt::Event::Bell => self.out.push(0x07),
                vt::Event::Title(title) => {
                    if let Some(pane) = self.panes.get_mut(address) {
                        pane.title = title;
                    }
                }
                vt::Event::Clipboard(text) => self.copy_to_host(&text),
                vt::Event::Notify { title, body } => {
                    if let Some(pane) = self.panes.get_mut(address) {
                        pane.notifications.push((title, body));
                        if pane.notifications.len() > 32 {
                            pane.notifications.remove(0);
                        }
                    }
                }
                vt::Event::Cwd(_) => {}
            }
        }
    }

    pub fn copy_to_host(&mut self, text: &str) {
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        self.out
            .extend_from_slice(format!("\x1b]52;c;{encoded}\x07").as_bytes());
    }

    pub fn remove_terminal(&mut self, address: &str) {
        let owner = self.owner(address);
        let gone = local(address, owner.as_deref()).to_owned();
        let edit: Edit = Rc::new(move |doc: &mut Document| {
            doc.remove_gone(|a| a == gone);
            Ok(())
        });
        self.edit_quiet(owner.as_deref(), edit);
    }

    pub fn owner(&self, address: &str) -> Option<String> {
        let (machine, _) = split(address);
        machine
            .filter(|m| self.machines.get(Some(m)).is_some())
            .map(str::to_owned)
    }

    pub fn sidebar_width(&self) -> SidebarWidth {
        let collapsed = self
            .prefs
            .sidebar_collapsed
            .unwrap_or(self.config.ui.sidebar_start_collapsed);
        if collapsed {
            match self.config.ui.sidebar_collapsed_mode {
                config::SidebarCollapsedMode::Compact => SidebarWidth::Compact,
                config::SidebarCollapsedMode::Hidden => SidebarWidth::Hidden,
            }
        } else {
            SidebarWidth::Expanded(layout::expanded_width(
                &self.config.ui,
                self.prefs.sidebar_width,
            ))
        }
    }

    pub fn relayout(&mut self) {
        let screen = self.screen;
        let sidebar = self.sidebar_width();
        let chrome = Chrome::from_ui(&self.config.ui);
        let cell = self.cell();
        let mut geoms = HashMap::new();
        let mut sizes: HashMap<String, (u16, u16)> = HashMap::new();
        let mut regions = layout::regions(screen, &self.config.ui, sidebar, 0);
        let active_tab = self.view.active_tab(&self.machines);
        for space in self.machines.spaces() {
            let space_regions =
                layout::regions(screen, &self.config.ui, sidebar, space.space.tabs.len());
            if self.view.space.as_deref() == Some(space.key().as_str()) {
                regions = space_regions;
            }
            for tab in space.tabs() {
                let key = tab.key();
                let focused = self.view.focused_in(&self.machines, &key);
                let zoom = focused
                    .as_deref()
                    .filter(|_| self.view.is_zoomed(&key))
                    .map(|p| local(p, space.machine).to_owned());
                let gutter = |id: &str| {
                    tab.tab
                        .root
                        .find(id)
                        .and_then(|n| n.address())
                        .map(|a| crate::ids::qualify(a, space.machine))
                        .and_then(|a| self.panes.get(&a))
                        .is_none_or(|p| !p.term.alternate_screen())
                };
                let previewed = self.preview(&key, tab.tab);
                let geom = layout::tab(
                    previewed.as_ref().unwrap_or(tab.tab),
                    space_regions.surface,
                    chrome,
                    zoom.as_deref(),
                    cell,
                    &gutter,
                );
                for pane in &geom.panes {
                    if let Some(address) = tab
                        .tab
                        .root
                        .find(&pane.id)
                        .and_then(|n| n.address())
                        .map(|a| crate::ids::qualify(a, space.machine))
                    {
                        let size = (pane.content.width, pane.content.height);
                        if active_tab.as_deref() == Some(key.as_str()) {
                            sizes.insert(address, size);
                        } else {
                            sizes.entry(address).or_insert(size);
                        }
                    }
                }
                geoms.insert(key, geom);
            }
        }
        let shown: HashSet<String> = active_tab
            .as_ref()
            .and_then(|tab| {
                let geom = geoms.get(tab)?;
                let found = self.machines.tab(tab)?;
                Some(
                    geom.panes
                        .iter()
                        .filter_map(|p| found.tab.root.find(&p.id)?.address())
                        .map(|a| crate::ids::qualify(a, found.machine))
                        .collect(),
                )
            })
            .unwrap_or_default();
        let online = self.conn.is_online();
        let focused = self
            .view
            .focused_pane(&self.machines)
            .and_then(|p| self.machines.pane(&p).and_then(|p| p.address()))
            .filter(|_| self.view.host_focused && online);
        let cell_px = self.host.cell_px;
        for pane in self.panes.iter_mut() {
            if let Some(size) = sizes.get(&pane.address) {
                pane.resize(*size, cell_px);
            }
            pane.shown = online && shown.contains(&pane.address);
            if pane.shown {
                pane.lead();
            } else {
                pane.led = None;
            }
            pane.focus(focused.as_deref() == Some(pane.address.as_str()));
        }
        self.geoms = geoms;
        self.regions = regions;
        self.update_mouse_capture();
    }

    fn preview(&self, key: &str, tab: &doc::Tab) -> Option<doc::Tab> {
        let Some(Drag::Split {
            tab: dragged,
            container,
            index,
            preview: Some(new_a),
            ..
        }) = &self.view.drag
        else {
            return None;
        };
        if dragged != key {
            return None;
        }
        let geom = self.geoms.get(key)?;
        let mut scratch = Document {
            spaces: vec![doc::Space {
                tabs: vec![tab.clone()],
                ..doc::Space::default()
            }],
        };
        scratch
            .drag_divider(
                container,
                *index,
                *new_a,
                |id| geom.rect_of(id),
                self.cell(),
            )
            .ok()?;
        scratch.spaces.pop()?.tabs.pop()
    }

    fn update_mouse_capture(&mut self) {
        let wants = self
            .view
            .focused_pane(&self.machines)
            .and_then(|p| self.machines.pane(&p).and_then(|p| p.address()))
            .and_then(|a| self.panes.get(&a))
            .is_some_and(|p| p.term.wants_mouse());
        let capture = self.config.ui.mouse_capture || wants;
        if self.mouse_captured == Some(capture) {
            return;
        }
        self.mouse_captured = Some(capture);
        let mut ansi = String::new();
        let written = if capture {
            crossterm::event::EnableMouseCapture.write_ansi(&mut ansi)
        } else {
            crossterm::event::DisableMouseCapture.write_ansi(&mut ansi)
        };
        if written.is_ok() {
            self.out.extend_from_slice(ansi.as_bytes());
        }
    }
}
