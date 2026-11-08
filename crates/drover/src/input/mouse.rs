use std::time::{Duration, Instant};

use config::Action;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use vt::{Point, Selection, SelectionKind};

use crate::app::App;
use crate::hits::{PaneHit, inside};
use crate::overlay::MenuTarget;
use crate::view::{Click, ClickTarget, Drag, Mode, MouseSelection, Press};

const DOUBLE_CLICK: Duration = Duration::from_millis(350);
const WORD_HOLD: Duration = Duration::from_millis(500);

fn clamp_in(rect: Rect, x: u16, y: u16) -> (u16, u16) {
    (
        x.clamp(rect.x, rect.right().saturating_sub(1)) - rect.x,
        y.clamp(rect.y, rect.bottom().saturating_sub(1)) - rect.y,
    )
}

impl App {
    pub(super) fn mouse(&mut self, event: MouseEvent) {
        if let Some(mut overlay) = self.view.overlay.take() {
            let snapshot = self.snapshot();
            let ctx = crate::overlay::Ctx {
                palette: &self.palette,
                keymap: &self.keymap,
                config: &self.config,
                snapshot: &snapshot,
            };
            let outcome = overlay.mouse(&event, &ctx);
            self.view.overlay = Some(overlay);
            self.overlay_outcome(outcome);
            return;
        }
        if self.view.drag.is_some()
            && matches!(
                event.kind,
                MouseEventKind::Drag(_) | MouseEventKind::Up(_) | MouseEventKind::Moved
            )
        {
            self.drag_event(event);
            return;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => self.left_down(event),
            MouseEventKind::Down(MouseButton::Right) => self.right_down(event),
            MouseEventKind::Down(MouseButton::Middle) => {
                if let Some(hit) = self.hits.pane_at(event.column, event.row).cloned()
                    && self.forward(&hit, event)
                {
                    self.view.drag = hit.address.map(|address| Drag::Program { address });
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => self.wheel(event),
            MouseEventKind::Moved => {
                if let Some(hit) = self.hits.pane_at(event.column, event.row).cloned()
                    && self.focused_address() == hit.address
                {
                    self.forward(&hit, event);
                }
            }
            _ => {}
        }
    }

    fn program_wants_mouse(&self, hit: &PaneHit) -> bool {
        hit.address
            .as_ref()
            .and_then(|a| self.panes.get(a))
            .is_some_and(|p| p.term.wants_mouse())
    }

    fn forward(&mut self, hit: &PaneHit, event: MouseEvent) -> bool {
        let Some(address) = &hit.address else {
            return false;
        };
        let (col, row) = clamp_in(hit.content, event.column, event.row);
        let Some(pane) = self.panes.get_mut(address) else {
            return false;
        };
        match pane.term.mouse(&event, col, row) {
            Some(bytes) => {
                if pane.term.wants_mouse() {
                    pane.term.scroll_to_bottom();
                }
                pane.input(&bytes);
                true
            }
            None => false,
        }
    }

    fn left_down(&mut self, event: MouseEvent) {
        let (x, y) = (event.column, event.row);
        if let Some(toast) = self.hits.toast
            && inside(toast, x, y)
        {
            if let Some(target) = self.notice.take().and_then(|n| n.target) {
                self.focus_pane(&target);
            }
            return;
        }
        if self.sidebar_down(x, y) {
            return;
        }
        if self.tab_bar_down(x, y) {
            return;
        }
        if let Some(split) = self.hits.split_at(x, y).cloned() {
            let pointer = match split.axis {
                doc::Axis::Row => x,
                doc::Axis::Column => y,
            };
            self.view.drag = Some(Drag::Split {
                machine: split.machine,
                tab: split.tab,
                container: split.container,
                index: split.index,
                axis: split.axis,
                grab: i32::from(split.pos) - i32::from(pointer),
                origin: split.origin,
                preview: None,
            });
            return;
        }
        if let Some(hit) = self.hits.pane_at(x, y).cloned() {
            self.pane_down(&hit, event);
        }
    }

    fn pane_down(&mut self, hit: &PaneHit, event: MouseEvent) {
        let (x, y) = (event.column, event.row);
        self.view.selection = None;
        self.focus_pane(&hit.key);
        if self.view.mode == Mode::Navigate {
            self.view.mode = Mode::Terminal;
            self.view.nav = None;
        }
        let Some(address) = hit.address.clone() else {
            return;
        };
        if let Some(gutter) = hit.gutter
            && inside(gutter, x, y)
        {
            if self.view.mode != Mode::Copy {
                self.view.mode = Mode::Terminal;
            }
            self.scroll_to_pointer(&address, gutter, y);
            self.view.drag = Some(Drag::Scrollbar { address, grab: 0 });
            return;
        }
        if !inside(hit.content, x, y) {
            return;
        }
        if self.program_wants_mouse(hit) && !event.modifiers.contains(KeyModifiers::SHIFT) {
            if self.forward(hit, event) {
                self.view.drag = Some(Drag::Program { address });
            }
            return;
        }
        let now = Instant::now();
        let double = self.view.click.is_some_and(|c| {
            c.target == ClickTarget::Pane
                && now.duration_since(c.when) <= DOUBLE_CLICK
                && c.at.0.abs_diff(x) <= 1
                && c.at.1.abs_diff(y) <= 1
        }) && event.modifiers.is_empty();
        self.view.click = Some(Click {
            at: (x, y),
            when: now,
            target: ClickTarget::Pane,
        });
        let Some(pane) = self.panes.get(&address) else {
            return;
        };
        let (col, row) = clamp_in(hit.content, x, y);
        let point = pane.term.viewport_point(col, row);
        if double {
            self.view.click = None;
            if let Some((start, end)) = pane.term.word_bounds(point) {
                self.view.selection = Some(MouseSelection {
                    address: address.clone(),
                    selection: Selection::new(start, end, SelectionKind::Char),
                    dragging: false,
                    visible: true,
                    clear_at: None,
                });
                self.view.drag = Some(Drag::Select {
                    address,
                    anchor: start,
                    word: true,
                });
            }
            return;
        }
        self.view.drag = Some(Drag::Select {
            address,
            anchor: point,
            word: false,
        });
    }

    fn sidebar_down(&mut self, x: u16, y: u16) -> bool {
        let hits = self.hits.sidebar.clone();
        if hits.toggle.is_some_and(|r| inside(r, x, y)) {
            self.toggle_sidebar();
            self.relayout();
            return true;
        }
        if hits.divider.is_some_and(|r| inside(r, x, y)) {
            let now = Instant::now();
            let double = self.view.click.is_some_and(|c| {
                c.target == ClickTarget::SidebarEdge && now.duration_since(c.when) <= DOUBLE_CLICK
            });
            self.view.click = Some(Click {
                at: (x, y),
                when: now,
                target: ClickTarget::SidebarEdge,
            });
            if double {
                self.prefs.sidebar_width = None;
                self.save_prefs();
                self.relayout();
            } else {
                self.view.drag = Some(Drag::SidebarEdge);
            }
            return true;
        }
        if hits.section_divider.is_some_and(|r| inside(r, x, y)) {
            self.view.drag = Some(Drag::SectionDivider);
            return true;
        }
        if hits.new_workspace.is_some_and(|r| inside(r, x, y)) {
            self.run_action(Action::NewWorkspace);
            return true;
        }
        if hits.menu.is_some_and(|r| inside(r, x, y)) {
            self.global_menu();
            return true;
        }
        if hits.agent_sort.is_some_and(|r| inside(r, x, y)) {
            let current = self
                .prefs
                .agent_panel_sort
                .unwrap_or(self.config.ui.agent_panel_sort);
            self.prefs.agent_panel_sort = Some(match current {
                config::AgentPanelSort::Spaces => config::AgentPanelSort::Priority,
                config::AgentPanelSort::Priority => config::AgentPanelSort::Spaces,
            });
            self.view.agent_scroll = 0;
            self.save_prefs();
            return true;
        }
        if let Some((_, pane)) = hits.agents.iter().find(|(r, _)| inside(*r, x, y)) {
            self.focus_pane(pane);
            return true;
        }
        if let Some((_, space)) = hits.spaces.iter().find(|(r, _)| inside(*r, x, y)) {
            self.view.drag = Some(Drag::Press {
                target: Press::Space(space.clone()),
                at: (x, y),
                now: (x, y),
            });
            return true;
        }
        hits.area.is_some_and(|r| inside(r, x, y))
    }

    fn tab_bar_down(&mut self, x: u16, y: u16) -> bool {
        let tabs = self.hits.tabs.clone();
        if tabs.new_tab.is_some_and(|r| inside(r, x, y)) {
            self.run_action(Action::NewTab);
            return true;
        }
        if tabs.scroll_left.is_some_and(|r| inside(r, x, y)) {
            self.view.tab_scroll = self.view.tab_scroll.saturating_sub(1);
            self.view.reveal_tab = false;
            return true;
        }
        if tabs.scroll_right.is_some_and(|r| inside(r, x, y)) {
            let count = self.snapshot().tabs.len();
            self.view.tab_scroll = (self.view.tab_scroll + 1).min(count.saturating_sub(1));
            self.view.reveal_tab = false;
            return true;
        }
        if let Some(tab) = self.hits.tab_at(x, y).map(str::to_owned) {
            self.view.drag = Some(Drag::Press {
                target: Press::Tab(tab),
                at: (x, y),
                now: (x, y),
            });
            return true;
        }
        tabs.row.is_some_and(|r| inside(r, x, y))
    }

    fn right_down(&mut self, event: MouseEvent) {
        let (x, y) = (event.column, event.row);
        if let Some((_, space)) = self
            .hits
            .sidebar
            .spaces
            .iter()
            .find(|(r, _)| inside(*r, x, y))
            .cloned()
        {
            self.context_menu(MenuTarget::Space(space), (x, y));
            return;
        }
        if let Some(tab) = self.hits.tab_at(x, y).map(str::to_owned) {
            self.context_menu(MenuTarget::Tab(tab), (x, y));
            return;
        }
        let Some(hit) = self.hits.pane_at(x, y).cloned() else {
            return;
        };
        if inside(hit.content, x, y) && self.program_wants_mouse(&hit) {
            let modifier = self.config.ui.right_click_passthrough_modifier.0;
            let label = hit
                .address
                .as_ref()
                .and_then(|a| self.terminals.get(a))
                .and_then(|t| t.label("herdr.right-click"))
                == Some("program");
            let held = modifier.is_some_and(|m| !m.is_empty() && event.modifiers.contains(m));
            if label && event.modifiers.is_empty() || held {
                let mut event = event;
                if let Some(m) = modifier {
                    event.modifiers.remove(m);
                }
                self.focus_pane(&hit.key);
                if self.forward(&hit, event) {
                    self.view.drag = hit.address.map(|address| Drag::Program { address });
                }
                return;
            }
        }
        self.context_menu(MenuTarget::Pane(hit.key), (x, y));
    }

    fn wheel(&mut self, event: MouseEvent) {
        let (x, y) = (event.column, event.row);
        let up = event.kind == MouseEventKind::ScrollUp;
        if self.hits.tabs.row.is_some_and(|r| inside(r, x, y)) {
            self.run_action(if up {
                Action::PreviousTab
            } else {
                Action::NextTab
            });
            return;
        }
        if self
            .hits
            .sidebar
            .spaces_body
            .is_some_and(|r| inside(r, x, y))
        {
            self.view.sidebar_scroll = if up {
                self.view.sidebar_scroll.saturating_sub(1)
            } else {
                self.view.sidebar_scroll + 1
            };
            return;
        }
        if self
            .hits
            .sidebar
            .agents_body
            .is_some_and(|r| inside(r, x, y))
        {
            self.view.agent_scroll = if up {
                self.view.agent_scroll.saturating_sub(1)
            } else {
                self.view.agent_scroll + 1
            };
            return;
        }
        let Some(hit) = self.hits.pane_at(x, y).cloned() else {
            return;
        };
        if self.view.focused_pane(&self.machines).as_deref() != Some(hit.key.as_str()) {
            self.focus_pane(&hit.key);
        }
        if self.forward(&hit, event) {
            return;
        }
        let Some(address) = hit.address else {
            return;
        };
        let lines = isize::try_from(self.config.ui.mouse_scroll_lines()).unwrap_or(3);
        if let Some(pane) = self.panes.get_mut(&address) {
            pane.term.scroll(if up { -lines } else { lines });
        }
    }

    fn scroll_to_pointer(&mut self, address: &str, gutter: Rect, y: u16) {
        let Some(pane) = self.panes.get_mut(address) else {
            return;
        };
        let bar = pane.term.scrollbar();
        let max = bar.max_offset();
        if max == 0 || gutter.height == 0 {
            return;
        }
        let row = u64::from(y.saturating_sub(gutter.y).min(gutter.height - 1));
        let span = u64::from(gutter.height.saturating_sub(1).max(1));
        let offset = (row * max + span / 2) / span;
        let top = pane.term.first_row() + offset.min(max);
        pane.term.scroll_to_row(top);
    }

    fn drag_event(&mut self, event: MouseEvent) {
        let (x, y) = (event.column, event.row);
        let up = matches!(event.kind, MouseEventKind::Up(_));
        let Some(drag) = self.view.drag.clone() else {
            return;
        };
        match drag {
            Drag::Split { .. } => self.drag_split(&drag, x, y, up),
            Drag::SidebarEdge => {
                let (min, max) = self.config.ui.sidebar_bounds_or_default();
                self.prefs.sidebar_width = Some((x + 1).clamp(min, max));
                if up {
                    self.view.drag = None;
                    self.save_prefs();
                }
                self.relayout();
            }
            Drag::SectionDivider => {
                let area = self.regions.sidebar;
                if area.height > 0 {
                    let ratio = f32::from(y.saturating_sub(area.y)) / f32::from(area.height);
                    self.prefs.sidebar_section_split = Some(ratio.clamp(0.1, 0.9));
                }
                if up {
                    self.view.drag = None;
                    self.save_prefs();
                }
            }
            Drag::Select {
                address,
                anchor,
                word,
            } => self.drag_select(&address, anchor, word, x, y, up),
            Drag::Program { address } => {
                if let Some(hit) = self
                    .hits
                    .panes
                    .iter()
                    .find(|p| p.address.as_deref() == Some(address.as_str()))
                    .cloned()
                {
                    self.forward(&hit, event);
                }
                if up {
                    self.view.drag = None;
                }
            }
            Drag::Scrollbar { address, .. } => {
                if let Some(gutter) = self
                    .hits
                    .panes
                    .iter()
                    .find(|p| p.address.as_deref() == Some(address.as_str()))
                    .and_then(|p| p.gutter)
                {
                    self.scroll_to_pointer(&address, gutter, y);
                }
                if up {
                    self.view.drag = None;
                }
            }
            Drag::Press { target, at, .. } => {
                if let Some(Drag::Press { now, .. }) = self.view.drag.as_mut() {
                    *now = (x, y);
                }
                if up {
                    self.view.drag = None;
                    self.release_press(target, at, x, y);
                }
            }
        }
    }

    fn release_press(&mut self, target: Press, at: (u16, u16), x: u16, y: u16) {
        let moved = at.0.abs_diff(x).max(at.1.abs_diff(y)) >= 1;
        match target {
            Press::Space(space) => {
                let over = self
                    .hits
                    .sidebar
                    .spaces
                    .iter()
                    .any(|(r, k)| *k == space && inside(*r, x, y));
                if !moved || over {
                    self.focus_space(&space);
                    if self.view.mode == Mode::Navigate {
                        self.view.mode = Mode::Terminal;
                        self.view.nav = None;
                    }
                }
            }
            Press::Tab(tab) => match self.hits.tab_at(x, y).map(str::to_owned) {
                Some(other) if moved && other != tab => {
                    let index = self.snapshot().tabs.iter().position(|t| t.key == other);
                    if let Some(index) = index {
                        self.move_tab_to(&tab, index);
                    }
                }
                _ if !moved => self.focus_tab(&tab),
                _ => {}
            },
        }
    }

    fn drag_split(&mut self, drag: &Drag, x: u16, y: u16, up: bool) {
        let Drag::Split {
            machine,
            tab,
            container,
            index,
            axis,
            grab,
            origin,
            ..
        } = drag
        else {
            return;
        };
        let pointer = match axis {
            doc::Axis::Row => x,
            doc::Axis::Column => y,
        };
        let cells = (i32::from(pointer) + grab - i32::from(*origin)).max(0);
        let new_a = f32::from(u16::try_from(cells).unwrap_or(u16::MAX));
        if up {
            self.view.drag = None;
            self.drag_divider(machine.as_deref(), tab, container, *index, new_a);
        } else if let Some(Drag::Split { preview, .. }) = self.view.drag.as_mut() {
            *preview = Some(new_a);
        }
        self.relayout();
    }

    fn drag_select(&mut self, address: &str, anchor: Point, word: bool, x: u16, y: u16, up: bool) {
        let Some(hit) = self
            .hits
            .panes
            .iter()
            .find(|p| p.address.as_deref() == Some(address))
            .cloned()
        else {
            self.view.drag = None;
            return;
        };
        let content = hit.content;
        let Some(pane) = self.panes.get_mut(address) else {
            self.view.drag = None;
            return;
        };
        if y < content.y {
            let lines =
                isize::try_from((content.y - y).saturating_mul(3).clamp(3, 15)).unwrap_or(3);
            pane.term.scroll(-lines);
        } else if y >= content.bottom() {
            let lines = isize::try_from((y + 1 - content.bottom()).saturating_mul(3).clamp(3, 15))
                .unwrap_or(3);
            pane.term.scroll(lines);
        }
        let (col, row) = clamp_in(content, x, y);
        let mut head = pane.term.viewport_point(col, row);
        let mut start = anchor;
        if word {
            if let Some((ws, we)) = pane.term.word_bounds(head) {
                head = if head < anchor { ws } else { we };
            }
            if let Some(existing) = &self.view.selection
                && head < anchor
            {
                start = existing.selection.head.max(anchor);
            }
        }
        let moved = head != anchor;
        let dragging = moved || self.view.selection.as_ref().is_some_and(|s| s.dragging);
        if dragging || word {
            let selection = if word {
                let (a, b) = self
                    .view
                    .selection
                    .as_ref()
                    .map_or((anchor, head), |s| s.selection.ordered());
                let (lo, hi) = (a.min(head).min(start), b.max(head));
                Selection::new(lo, hi, SelectionKind::Char)
            } else {
                Selection::new(anchor, head, SelectionKind::Char)
            };
            self.view.selection = Some(MouseSelection {
                address: address.to_owned(),
                selection,
                dragging: moved || self.view.selection.as_ref().is_some_and(|s| s.dragging),
                visible: true,
                clear_at: None,
            });
        }
        if !up {
            return;
        }
        self.view.drag = None;
        let Some(selection) = self.view.selection.clone() else {
            return;
        };
        if !selection.dragging && !word {
            self.view.selection = None;
            return;
        }
        let text = self
            .panes
            .get(address)
            .map(|p| p.term.selection_text(&selection.selection))
            .unwrap_or_default();
        if self.config.ui.copy_on_select {
            self.copied(&text);
            if selection.dragging {
                self.view.selection = None;
            } else if let Some(s) = self.view.selection.as_mut() {
                s.clear_at = Some(Instant::now() + WORD_HOLD);
            }
        }
    }
}
