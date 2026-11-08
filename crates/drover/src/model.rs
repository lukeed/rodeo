use config::AgentPanelSort;

use crate::agents::State;
use crate::view::Mode;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineRow {
    pub name: Option<String>,
    pub label: String,
    pub state: String,
    pub connected: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpaceRow {
    pub key: String,
    pub machine: Option<String>,
    pub machine_label: String,
    pub number: usize,
    pub name: String,
    pub custom: bool,
    pub folder: String,
    pub state: Option<State>,
    pub focused: bool,
    pub selected: bool,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub tabs: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabRow {
    pub key: String,
    pub label: String,
    pub custom: bool,
    pub zoomed: bool,
    pub focused: bool,
    pub state: Option<State>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneRow {
    pub key: String,
    pub address: Option<String>,
    pub space: String,
    pub tab: String,
    pub title: Option<String>,
    pub terminal_title: String,
    pub module: Option<String>,
    pub state: Option<State>,
    pub focused: bool,
    pub tab_label: String,
    pub tab_custom: bool,
    pub tab_panes: usize,
    pub cwd: String,
    pub agent: Option<String>,
    pub agent_title: Option<String>,
    pub right_click: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRow {
    pub pane: String,
    pub address: String,
    pub space: String,
    pub tab: String,
    pub machine: String,
    pub workspace: String,
    pub tab_label: Option<String>,
    pub pane_label: Option<String>,
    pub agent: String,
    pub state: State,
    pub terminal_title: String,
    pub terminal_title_stripped: String,
    pub changed_ms: u64,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub machines: Vec<MachineRow>,
    pub spaces: Vec<SpaceRow>,
    pub tabs: Vec<TabRow>,
    pub panes: Vec<PaneRow>,
    pub agents: Vec<AgentRow>,
    pub active_space: Option<String>,
    pub active_tab: Option<String>,
    pub focused_pane: Option<String>,
    pub mode: Mode,
    pub collapsed: bool,
    pub agent_sort: AgentPanelSort,
    pub menu_attention: bool,
    pub online: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            machines: Vec::new(),
            spaces: Vec::new(),
            tabs: Vec::new(),
            panes: Vec::new(),
            agents: Vec::new(),
            active_space: None,
            active_tab: None,
            focused_pane: None,
            mode: Mode::Terminal,
            collapsed: false,
            agent_sort: AgentPanelSort::Spaces,
            menu_attention: false,
            online: true,
        }
    }
}

impl Snapshot {
    pub fn space(&self, key: &str) -> Option<&SpaceRow> {
        self.spaces.iter().find(|s| s.key == key)
    }

    pub fn active(&self) -> Option<&SpaceRow> {
        self.space(self.active_space.as_deref()?)
    }

    pub fn ordered_agents(&self) -> Vec<&AgentRow> {
        let mut agents: Vec<&AgentRow> = self.agents.iter().collect();
        if self.agent_sort == AgentPanelSort::Priority {
            agents.sort_by(|a, b| {
                b.state
                    .priority()
                    .cmp(&a.state.priority())
                    .then(b.changed_ms.cmp(&a.changed_ms))
            });
        }
        agents
    }
}
