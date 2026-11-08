use std::rc::Rc;
use std::time::Duration;

use config::{Action, AgentPanelSort};
use doc::Document;

use crate::app::{App, NoticeDot};
use crate::hits::inside;
use crate::ids::local;
use crate::model::{MachineStatus, SpaceRow};
use crate::sidebar::{self, Drag as SidebarDrag, scroll};
use crate::view::{Drag, Mode, Press};

const NOTICE: Duration = Duration::from_secs(8);
const DIAGNOSTIC: Duration = Duration::from_secs(15);

impl App {
    pub(super) fn sidebar_badge(&mut self, x: u16, y: u16) -> bool {
        let Some(name) = self
            .hits
            .sidebar
            .machines
            .iter()
            .find(|h| inside(h.badge, x, y))
            .map(|h| h.name.clone())
        else {
            return false;
        };
        let snapshot = self.snapshot();
        let Some(machine) = snapshot
            .machines
            .iter()
            .find(|m| m.name == name && sidebar::has_diagnostic(m))
        else {
            return false;
        };
        let command = match (&machine.name, machine.auth) {
            (Some(name), true) => format!("hc machine login {name}"),
            _ => "hc machine ls".to_owned(),
        };
        let title = format!("{}: {command}", machine.label);
        let body = machine.why.clone().unwrap_or_default();
        self.show_notice(&title, &body, NoticeDot::Yellow, None, DIAGNOSTIC);
        true
    }

    pub(super) fn sidebar_hover(&mut self, x: u16, y: u16) {
        let hover = self
            .hits
            .sidebar
            .machines
            .iter()
            .find(|h| inside(h.badge, x, y))
            .map(|h| h.name.clone());
        self.view.sidebar.hover = hover;
    }

    fn not_ready(&mut self, machine: Option<&str>) {
        let label = machine.unwrap_or(crate::machines::LOCAL_LABEL);
        self.show_notice(
            "Endpoint unavailable",
            &format!("{label} is not ready"),
            NoticeDot::Yellow,
            None,
            NOTICE,
        );
    }

    pub(super) fn sidebar_down(&mut self, x: u16, y: u16) -> bool {
        let hits = self.hits.sidebar.clone();
        if !hits.area.is_some_and(|r| inside(r, x, y)) {
            return false;
        }
        if hits.toggle.is_some_and(|r| inside(r, x, y)) {
            self.toggle_sidebar();
            self.relayout();
            return true;
        }
        if hits.divider.is_some_and(|r| inside(r, x, y)) {
            self.sidebar_edge_down(x, y);
            return true;
        }
        if hits.section_divider.is_some_and(|r| inside(r, x, y)) {
            self.view.drag = Some(Drag::SectionDivider);
            return true;
        }
        if let Some(track) = hits.spaces_bar.filter(|r| inside(*r, x, y)) {
            let m = hits.spaces_metrics;
            match scroll::grab(m, track, y) {
                Some(grab) => self.view.drag = Some(Drag::Sidebar(SidebarDrag::Spaces { grab })),
                None => self.view.sidebar_scroll = scroll::start_at_row(m, track, y),
            }
            return true;
        }
        if let Some(track) = hits.agents_bar.filter(|r| inside(*r, x, y)) {
            let m = hits.agents_metrics;
            match scroll::grab(m, track, y) {
                Some(grab) => self.view.drag = Some(Drag::Sidebar(SidebarDrag::Agents { grab })),
                None => self.view.agent_scroll = scroll::start_at_row(m, track, y),
            }
            return true;
        }
        if hits.agent_sort.is_some_and(|r| inside(r, x, y)) {
            let current = self
                .prefs
                .agent_panel_sort
                .unwrap_or(self.config.ui.agent_panel_sort);
            self.prefs.agent_panel_sort = Some(match current {
                AgentPanelSort::Spaces => AgentPanelSort::Priority,
                AgentPanelSort::Priority => AgentPanelSort::Spaces,
            });
            self.view.agent_scroll = 0;
            self.save_prefs();
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
        if let Some(machine) = hits.machine_at(x, y).cloned() {
            self.machine_click(&machine, inside(machine.toggle, x, y));
            return true;
        }
        if let Some(space) = hits.space_at(x, y) {
            if let Some((_, key)) = space.group.as_ref().filter(|(r, _)| inside(*r, x, y)) {
                self.toggle_group(key);
                return true;
            }
            if space.stale {
                self.not_ready(space.machine.as_deref());
                return true;
            }
            self.view.drag = Some(Drag::Press {
                target: Press::Space(space.key.clone()),
                at: (x, y),
                now: (x, y),
            });
            return true;
        }
        if let Some(agent) = hits.agent_at(x, y).cloned() {
            if agent.stale {
                self.not_ready(agent.machine.as_deref());
            } else {
                self.focus_pane(&agent.pane);
            }
            return true;
        }
        true
    }

    fn toggle_group(&mut self, key: &str) {
        let groups = &mut self.prefs.collapsed_groups;
        if let Some(at) = groups.iter().position(|g| g == key) {
            groups.remove(at);
        } else {
            groups.push(key.to_owned());
        }
        self.save_prefs();
    }

    fn machine_click(&mut self, hit: &sidebar::MachineHit, on_toggle: bool) {
        let snapshot = self.snapshot();
        let Some(machine) = snapshot.machines.iter().find(|m| m.name == hit.name) else {
            return;
        };
        if on_toggle || machine.active {
            let folded = &mut self.view.sidebar.collapsed_machines;
            if !folded.remove(&machine.name) {
                folded.insert(machine.name.clone());
            }
            return;
        }
        if machine.status != MachineStatus::Online {
            self.not_ready(machine.name.as_deref());
            return;
        }
        let remembered = self
            .view
            .sidebar
            .last
            .get(&machine.name)
            .filter(|key| snapshot.space(key).is_some())
            .cloned();
        let first = snapshot
            .spaces
            .iter()
            .find(|s| s.machine == machine.name)
            .map(|s| s.key.clone());
        if let Some(key) = remembered.or(first) {
            self.focus_space(&key);
        }
    }

    pub(super) fn sidebar_wheel(&mut self, x: u16, y: u16, up: bool) -> bool {
        let hits = &self.hits.sidebar;
        let step = |at: usize, max: usize| {
            if up {
                at.saturating_sub(1)
            } else {
                (at + 1).min(max)
            }
        };
        if hits.spaces_body.is_some_and(|r| inside(r, x, y)) {
            self.view.sidebar_scroll = step(self.view.sidebar_scroll, hits.spaces_metrics.max);
            return true;
        }
        if hits.agents_body.is_some_and(|r| inside(r, x, y)) {
            self.view.agent_scroll = step(self.view.agent_scroll, hits.agents_metrics.max);
            return true;
        }
        false
    }

    pub(super) fn sidebar_drag(&mut self, drag: SidebarDrag, y: u16, up: bool) {
        let hits = &self.hits.sidebar;
        match drag {
            SidebarDrag::Spaces { grab } => {
                if let Some(track) = hits.spaces_bar {
                    self.view.sidebar_scroll =
                        scroll::start_dragged(hits.spaces_metrics, track, y, grab);
                }
            }
            SidebarDrag::Agents { grab } => {
                if let Some(track) = hits.agents_bar {
                    self.view.agent_scroll =
                        scroll::start_dragged(hits.agents_metrics, track, y, grab);
                }
            }
        }
        if up {
            self.view.drag = None;
        }
    }

    pub(super) fn release_space(&mut self, space: &str, at: (u16, u16), x: u16, y: u16) {
        let moved = at.0.abs_diff(x).max(at.1.abs_diff(y)) >= 1;
        if moved && self.drop_space(space, x, y) {
            return;
        }
        let over = self
            .hits
            .sidebar
            .space_at(x, y)
            .is_some_and(|h| h.key == space);
        if !moved || over {
            self.focus_space(space);
            if self.view.mode == Mode::Navigate {
                self.view.mode = Mode::Terminal;
                self.view.nav = None;
            }
        }
    }

    fn drop_space(&mut self, space: &str, x: u16, y: u16) -> bool {
        let hits = &self.hits.sidebar;
        let Some(before) = hits.drop_target(x, y).map(|slot| slot.before.clone()) else {
            return false;
        };
        let active = hits.active.clone();
        let snapshot = self.snapshot();
        let rows: Vec<&SpaceRow> = snapshot
            .spaces
            .iter()
            .filter(|s| s.machine == active)
            .collect();
        let Some(index) = rows.iter().position(|s| s.key == space) else {
            return false;
        };
        if !sidebar::draggable(rows[index], active.as_ref()) {
            return false;
        }
        let block = sidebar::block(&rows, index);
        if before.as_ref().is_some_and(|b| block.contains(b)) {
            return true;
        }
        let machine = active.as_deref();
        let ids: Vec<String> = block.iter().map(|k| local(k, machine).to_owned()).collect();
        let before = before.map(|b| local(&b, machine).to_owned());
        self.edit(
            machine,
            Rc::new(move |doc: &mut Document| doc.move_spaces(&ids, before.as_deref())),
        );
        true
    }
}
