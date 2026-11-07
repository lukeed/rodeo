use crate::agents::{self, State};
use crate::model::{AgentRow, MachineRow, Snapshot, SpaceRow, TabRow};
use crate::view::Mode;

use super::App;

impl App {
    pub fn agent_state(&self, address: &str) -> Option<State> {
        let terminal = self.terminals.get(address)?;
        agents::state(terminal, self.looked(address))
    }

    fn rollup(&self, addresses: &[String]) -> Option<State> {
        agents::rollup(addresses.iter().filter_map(|a| self.agent_state(a)))
    }

    pub fn snapshot(&self) -> Snapshot {
        let home = self.home();
        let active_space = self.view.space.clone();
        let active_tab = self.view.active_tab(&self.machines);
        let focused_pane = self.view.focused_pane(&self.machines);
        let active_machine = self.active_machine();
        let machines = self
            .machines
            .iter()
            .map(|m| MachineRow {
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
                active: m.name == active_machine,
            })
            .collect();
        let selected = match self.view.mode {
            Mode::Navigate => self.view.nav.clone(),
            _ => None,
        }
        .or_else(|| self.view.highlight.as_ref().map(|(k, _)| k.clone()));
        let mut spaces = Vec::new();
        let mut agents_rows = Vec::new();
        for (index, space) in self.machines.spaces().into_iter().enumerate() {
            let key = space.key();
            let addresses: Vec<String> = space.tabs().flat_map(|t| t.addresses()).collect();
            let git = self.git.get(&key);
            let name = self.space_name(&key, &home);
            spaces.push(SpaceRow {
                key: key.clone(),
                machine: space.machine.map(str::to_owned),
                machine_label: space
                    .machine
                    .unwrap_or(crate::machines::LOCAL_LABEL)
                    .to_owned(),
                number: index + 1,
                name: name.clone(),
                custom: !space.space.name.is_empty(),
                folder: space.space.folder.clone(),
                state: self.rollup(&addresses),
                focused: active_space.as_deref() == Some(key.as_str()),
                selected: selected.as_deref() == Some(key.as_str()),
                branch: git.and_then(|g| g.branch.clone()),
                ahead: git.map_or(0, |g| g.ahead),
                behind: git.map_or(0, |g| g.behind),
                tabs: space.space.tabs.len(),
            });
            let multi = space.space.tabs.len() > 1;
            for tab in space.tabs() {
                let tab_label = (multi || !tab.tab.name.is_empty()).then(|| tab.name());
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
                    agents_rows.push(AgentRow {
                        pane: pane.key(),
                        address: address.clone(),
                        space: key.clone(),
                        tab: tab.key(),
                        machine: space
                            .machine
                            .unwrap_or(crate::machines::LOCAL_LABEL)
                            .to_owned(),
                        workspace: name.clone(),
                        tab_label: tab_label.clone(),
                        pane_label: status_title.or_else(|| pane.node.title.clone()),
                        agent: agents::label(terminal).unwrap_or_default(),
                        state,
                        terminal_title_stripped: agents::strip_activity(&title).to_owned(),
                        terminal_title: title,
                        changed_ms: terminal.status_ms.unwrap_or(0),
                        focused: focused_pane.as_deref() == Some(pane.key().as_str()),
                    });
                }
            }
        }
        let tabs = active_space
            .as_deref()
            .and_then(|s| self.machines.space(s))
            .map(|space| {
                space
                    .tabs()
                    .map(|tab| {
                        let key = tab.key();
                        TabRow {
                            label: tab.name(),
                            custom: !tab.tab.name.is_empty(),
                            zoomed: self.view.is_zoomed(&key),
                            focused: active_tab.as_deref() == Some(key.as_str()),
                            state: self.rollup(&tab.addresses()),
                            key,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Snapshot {
            machines,
            spaces,
            tabs,
            agents: agents_rows,
            active_space,
            active_tab,
            focused_pane,
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
}
