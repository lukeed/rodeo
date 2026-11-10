use crate::agents::{self, State};
use crate::machines::{LOCAL_LABEL, Machine, SpaceRef, TabRef};
use crate::model::{
    AgentRow, MachineRow, MachineStatus, PaneRow, Snapshot, SpaceRow, TabRow, Worktree,
};
use crate::sync::State as SlotState;
use crate::view::{Mode, Nav};

use super::{App, ConnState};

impl App {
    pub fn agent_state(&self, address: &str) -> Option<State> {
        let terminal = self.terminals.get(address)?;
        agents::state(terminal, self.looked(address))
    }

    fn rollup(&self, addresses: &[String]) -> Option<State> {
        agents::rollup(addresses.iter().filter_map(|a| self.agent_state(a)))
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut spaces = Vec::new();
        let mut panes = Vec::new();
        let mut agents = Vec::new();
        for (index, space) in self.machines.spaces().into_iter().enumerate() {
            let row = self.space_row(index, space, false);
            for tab in space.tabs() {
                self.pane_rows(tab, &mut panes);
                self.agent_rows(&row, tab, &mut agents);
            }
            spaces.push(row);
        }
        let mut stale_spaces = Vec::new();
        let mut stale_agents = Vec::new();
        for machine in self.machines.iter().filter(|m| is_stale(m)) {
            for (index, space) in machine.slot.doc().spaces.iter().enumerate() {
                let space = SpaceRef {
                    machine: machine.key(),
                    space,
                };
                let row = self.space_row(index, space, true);
                for tab in space.tabs() {
                    self.agent_rows(&row, tab, &mut stale_agents);
                }
                stale_spaces.push(row);
            }
        }
        Snapshot {
            machines: self.machine_rows(),
            spaces,
            tabs: self.tab_rows(),
            panes,
            agents,
            stale_spaces,
            stale_agents,
            active_space: self.view.space.clone(),
            active_tab: self.view.active_tab(&self.machines),
            focused_pane: self.view.focused_pane(&self.machines),
            mode: self.view.mode,
            collapsed: self
                .prefs
                .sidebar_collapsed
                .unwrap_or(self.config.ui.sidebar_start_collapsed),
            agent_sort: self
                .prefs
                .agent_panel_sort
                .unwrap_or(self.config.ui.agent_panel_sort),
            menu_attention: false,
            online: self.conn.is_online(),
        }
    }

    pub fn active_machine_row(&self) -> Option<MachineRow> {
        self.machine_rows().into_iter().find(|m| m.active)
    }

    fn machine_rows(&self) -> Vec<MachineRow> {
        let active = self.active_machine();
        let selected = match self.selected() {
            Some(Nav::Machine(name)) => Some(name),
            _ => None,
        };
        self.machines
            .iter()
            .map(|m| {
                let (status, auth, why) = self.machine_status(m);
                MachineRow {
                    name: m.name.clone(),
                    label: m.label().to_owned(),
                    state: if m.is_local() {
                        self.conn.label().to_owned()
                    } else {
                        m.state_word().to_owned()
                    },
                    connected: if m.is_local() {
                        self.conn.is_online()
                    } else {
                        m.is_connected()
                    },
                    active: m.name == active,
                    selected: selected == Some(&m.name),
                    status,
                    auth,
                    why,
                }
            })
            .collect()
    }

    fn machine_status(&self, machine: &Machine) -> (MachineStatus, bool, Option<String>) {
        if machine.is_local() {
            let status = match self.conn.state {
                ConnState::Connecting => MachineStatus::Connecting,
                ConnState::Online => MachineStatus::Online,
                ConnState::Down => MachineStatus::Reconnecting,
            };
            return (status, false, self.conn.error.clone());
        }
        let why = machine.info.as_ref().and_then(|i| i.why.clone());
        match machine.state_word() {
            "connected" => match machine.slot.state() {
                SlotState::Undecodable(error) | SlotState::Unreadable(error) => {
                    (MachineStatus::Attention, false, Some(error.clone()))
                }
                SlotState::Loading => (MachineStatus::Connecting, false, None),
                SlotState::Ready => (MachineStatus::Online, false, None),
            },
            "connecting" => (MachineStatus::Connecting, false, why),
            "unreachable" => (MachineStatus::Reconnecting, false, why),
            "needs_login" => (MachineStatus::Attention, true, why),
            "needs_update" => (MachineStatus::Attention, false, why),
            _ => (MachineStatus::Disabled, false, why),
        }
    }

    fn selected(&self) -> Option<&Nav> {
        self.view
            .nav
            .as_ref()
            .filter(|_| self.view.mode == Mode::Navigate)
    }

    fn space_row(&self, index: usize, space: SpaceRef<'_>, stale: bool) -> SpaceRow {
        let key = space.key();
        let addresses: Vec<String> = space.tabs().flat_map(|t| t.addresses()).collect();
        let git = self.git.get(&key);
        let selected = self.selected().and_then(Nav::space);
        let worktree = git.and_then(|g| {
            let common = g.common.as_deref()?;
            Some(Worktree {
                key: space
                    .machine
                    .map_or_else(|| common.to_owned(), |m| format!("{m}:{common}")),
                linked: g.linked,
            })
        });
        let name = if stale {
            space
                .space
                .display_name(git.and_then(|g| g.toplevel.as_deref()), &self.home())
        } else {
            self.space_name(&key, &self.home())
        };
        SpaceRow {
            name,
            machine: space.machine.map(str::to_owned),
            machine_label: space.machine.unwrap_or(LOCAL_LABEL).to_owned(),
            number: index + 1,
            custom: !space.space.name.is_empty(),
            folder: space.space.folder.clone(),
            state: self.rollup(&addresses),
            focused: !stale && self.view.space.as_deref() == Some(key.as_str()),
            selected: !stale && selected == Some(key.as_str()),
            branch: git.and_then(|g| g.branch.clone()),
            ahead: git.map_or(0, |g| g.ahead),
            behind: git.map_or(0, |g| g.behind),
            tabs: space.space.tabs.len(),
            worktree,
            stale,
            key,
        }
    }

    fn agent_rows(&self, space: &SpaceRow, tab: TabRef<'_>, out: &mut Vec<AgentRow>) {
        let focused = self.view.focused_pane(&self.machines);
        let tab_label = (space.tabs > 1 || !tab.tab.name.is_empty()).then(|| tab.name());
        for pane in tab.panes() {
            let Some(address) = pane.address() else {
                continue;
            };
            let Some(terminal) = self.terminals.get(&address) else {
                continue;
            };
            let Some(state) = agents::state(terminal, self.looked(&address)) else {
                continue;
            };
            let title = terminal.title.clone();
            let status_title = terminal
                .status
                .as_ref()
                .map(|s| s.title.clone())
                .filter(|t| !t.is_empty());
            out.push(AgentRow {
                pane: pane.key(),
                space: space.key.clone(),
                tab: tab.key(),
                machine: space.machine_label.clone(),
                workspace: space.name.clone(),
                tab_label: tab_label.clone(),
                pane_label: status_title.or_else(|| pane.node.title.clone()),
                agent: agents::label(terminal).unwrap_or_default(),
                state,
                terminal_title_stripped: agents::strip_activity(&title).to_owned(),
                terminal_title: title,
                changed_ms: terminal.status_ms.unwrap_or(0),
                focused: !space.stale && focused.as_deref() == Some(pane.key().as_str()),
                stale: space.stale,
                labels: terminal.labels.clone(),
                address,
            });
        }
    }

    fn pane_rows(&self, tab: TabRef<'_>, out: &mut Vec<PaneRow>) {
        let focused = self.view.focused_pane(&self.machines);
        let count = tab.panes().count();
        for pane in tab.panes() {
            let key = pane.key();
            let address = pane.address();
            let terminal = address.as_deref().and_then(|a| self.terminals.get(a));
            let status = terminal.and_then(|t| t.status.as_ref());
            out.push(PaneRow {
                tab_label: tab.name(),
                tab_custom: !tab.tab.name.is_empty(),
                tab_panes: count,
                cwd: terminal
                    .map(|t| {
                        t.foreground
                            .as_ref()
                            .map(|f| f.cwd.clone())
                            .filter(|cwd| !cwd.is_empty())
                            .unwrap_or_else(|| t.cwd.clone())
                    })
                    .unwrap_or_default(),
                agent: terminal.and_then(agents::label).filter(|a| !a.is_empty()),
                agent_title: status
                    .map(|s| s.title.clone())
                    .filter(|t| !t.is_empty())
                    .or_else(|| {
                        terminal
                            .map(|t| agents::strip_activity(&t.title).to_owned())
                            .filter(|t| !t.is_empty())
                    }),
                right_click: terminal.and_then(|t| t.label("herdr.right-click")) == Some("program"),
                address: address.clone(),
                space: pane.space_key(),
                tab: tab.key(),
                title: pane.node.title.clone(),
                terminal_title: terminal.map(|t| t.title.clone()).unwrap_or_default(),
                module: match pane.node.content() {
                    Some(doc::Content::Mod { name, .. }) => Some(name.clone()),
                    _ => None,
                },
                state: address.as_deref().and_then(|a| self.agent_state(a)),
                focused: focused.as_deref() == Some(key.as_str()),
                key,
            });
        }
    }

    fn tab_rows(&self) -> Vec<TabRow> {
        let active = self.view.active_tab(&self.machines);
        let Some(space) = self
            .view
            .space
            .as_deref()
            .and_then(|s| self.machines.space(s))
        else {
            return Vec::new();
        };
        space
            .tabs()
            .map(|tab| {
                let key = tab.key();
                TabRow {
                    label: tab.name(),
                    custom: !tab.tab.name.is_empty(),
                    zoomed: self.view.is_zoomed(&key),
                    focused: active.as_deref() == Some(key.as_str()),
                    state: self.rollup(&tab.addresses()),
                    key,
                }
            })
            .collect()
    }
}

fn is_stale(machine: &Machine) -> bool {
    !machine.is_local()
        && machine.slot.version() > 0
        && !(machine.is_connected() && machine.slot.is_ready())
}
