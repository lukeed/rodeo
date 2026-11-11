#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Help,
    Settings,
    Machines,
    NewWorkspace,
    NewWorktree,
    OpenWorktree,
    RemoveWorktree,
    RenameWorkspace,
    CloseWorkspace,
    WorkspacePicker,
    Goto,
    Detach,
    ReloadConfig,
    OpenNotificationTarget,
    PreviousWorkspace,
    NextWorkspace,
    PreviousAgent,
    NextAgent,
    FocusAgent(usize),
    NewTab,
    RenameTab,
    PreviousTab,
    NextTab,
    MoveTabPrevious,
    MoveTabNext,
    SwitchTab(usize),
    SwitchWorkspace(usize),
    CloseTab,
    RenamePane,
    EditScrollback,
    ClearPane,
    CopyMode,
    FocusPaneLeft,
    FocusPaneDown,
    FocusPaneUp,
    FocusPaneRight,
    SwapPaneLeft,
    SwapPaneDown,
    SwapPaneUp,
    SwapPaneRight,
    CyclePaneNext,
    CyclePanePrevious,
    LastPane,
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    Zoom,
    ResizeMode,
    ResizePaneLeft,
    ResizePaneDown,
    ResizePaneUp,
    ResizePaneRight,
    ToggleSidebar,
    NavigateWorkspaceUp,
    NavigateWorkspaceDown,
    Command(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Indexed {
    Tab,
    Workspace,
    Agent,
}

impl Indexed {
    pub fn action(self, index: usize) -> Action {
        match self {
            Self::Tab => Action::SwitchTab(index),
            Self::Workspace => Action::SwitchWorkspace(index),
            Self::Agent => Action::FocusAgent(index),
        }
    }

    pub(crate) fn slot(self) -> usize {
        match self {
            Self::Tab => 0,
            Self::Workspace => 1,
            Self::Agent => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Action(Action),
    Indexed(Indexed),
}

pub(crate) const NAVIGATE_FIELDS: &[(&str, &str, Action)] = &[
    ("navigate_workspace_up", "up", Action::NavigateWorkspaceUp),
    (
        "navigate_workspace_down",
        "down",
        Action::NavigateWorkspaceDown,
    ),
    ("navigate_pane_left", "h", Action::FocusPaneLeft),
    ("navigate_pane_down", "j", Action::FocusPaneDown),
    ("navigate_pane_up", "k", Action::FocusPaneUp),
    ("navigate_pane_right", "l", Action::FocusPaneRight),
];

pub(crate) const ACTION_FIELDS: &[(&str, &str, Slot)] = &[
    ("help", "prefix+?", Slot::Action(Action::Help)),
    ("settings", "prefix+s", Slot::Action(Action::Settings)),
    ("machines", "prefix+m", Slot::Action(Action::Machines)),
    (
        "new_workspace",
        "prefix+shift+n",
        Slot::Action(Action::NewWorkspace),
    ),
    (
        "new_worktree",
        "prefix+shift+g",
        Slot::Action(Action::NewWorktree),
    ),
    ("open_worktree", "", Slot::Action(Action::OpenWorktree)),
    ("remove_worktree", "", Slot::Action(Action::RemoveWorktree)),
    (
        "rename_workspace",
        "prefix+shift+w",
        Slot::Action(Action::RenameWorkspace),
    ),
    (
        "close_workspace",
        "prefix+shift+d",
        Slot::Action(Action::CloseWorkspace),
    ),
    (
        "workspace_picker",
        "prefix+w",
        Slot::Action(Action::WorkspacePicker),
    ),
    ("goto", "prefix+g", Slot::Action(Action::Goto)),
    ("detach", "prefix+q", Slot::Action(Action::Detach)),
    (
        "reload_config",
        "prefix+shift+r",
        Slot::Action(Action::ReloadConfig),
    ),
    (
        "open_notification_target",
        "prefix+o",
        Slot::Action(Action::OpenNotificationTarget),
    ),
    (
        "previous_workspace",
        "",
        Slot::Action(Action::PreviousWorkspace),
    ),
    ("next_workspace", "", Slot::Action(Action::NextWorkspace)),
    ("previous_agent", "", Slot::Action(Action::PreviousAgent)),
    ("next_agent", "", Slot::Action(Action::NextAgent)),
    ("focus_agent", "", Slot::Indexed(Indexed::Agent)),
    ("new_tab", "prefix+c", Slot::Action(Action::NewTab)),
    (
        "rename_tab",
        "prefix+shift+t",
        Slot::Action(Action::RenameTab),
    ),
    (
        "previous_tab",
        "prefix+p",
        Slot::Action(Action::PreviousTab),
    ),
    ("next_tab", "prefix+n", Slot::Action(Action::NextTab)),
    (
        "move_tab_previous",
        "",
        Slot::Action(Action::MoveTabPrevious),
    ),
    ("move_tab_next", "", Slot::Action(Action::MoveTabNext)),
    ("switch_tab", "prefix+1..9", Slot::Indexed(Indexed::Tab)),
    ("switch_workspace", "", Slot::Indexed(Indexed::Workspace)),
    (
        "close_tab",
        "prefix+shift+x",
        Slot::Action(Action::CloseTab),
    ),
    (
        "rename_pane",
        "prefix+shift+p",
        Slot::Action(Action::RenamePane),
    ),
    (
        "edit_scrollback",
        "prefix+e",
        Slot::Action(Action::EditScrollback),
    ),
    ("clear_pane", "", Slot::Action(Action::ClearPane)),
    ("copy_mode", "prefix+[", Slot::Action(Action::CopyMode)),
    (
        "focus_pane_left",
        "prefix+h",
        Slot::Action(Action::FocusPaneLeft),
    ),
    (
        "focus_pane_down",
        "prefix+j",
        Slot::Action(Action::FocusPaneDown),
    ),
    (
        "focus_pane_up",
        "prefix+k",
        Slot::Action(Action::FocusPaneUp),
    ),
    (
        "focus_pane_right",
        "prefix+l",
        Slot::Action(Action::FocusPaneRight),
    ),
    (
        "swap_pane_left",
        "prefix+shift+h",
        Slot::Action(Action::SwapPaneLeft),
    ),
    (
        "swap_pane_down",
        "prefix+shift+j",
        Slot::Action(Action::SwapPaneDown),
    ),
    (
        "swap_pane_up",
        "prefix+shift+k",
        Slot::Action(Action::SwapPaneUp),
    ),
    (
        "swap_pane_right",
        "prefix+shift+l",
        Slot::Action(Action::SwapPaneRight),
    ),
    ("last_pane", "", Slot::Action(Action::LastPane)),
    (
        "cycle_pane_next",
        "prefix+tab",
        Slot::Action(Action::CyclePaneNext),
    ),
    (
        "cycle_pane_previous",
        "prefix+shift+tab",
        Slot::Action(Action::CyclePanePrevious),
    ),
    (
        "split_vertical",
        "prefix+v",
        Slot::Action(Action::SplitVertical),
    ),
    (
        "split_horizontal",
        "prefix+minus",
        Slot::Action(Action::SplitHorizontal),
    ),
    ("close_pane", "prefix+x", Slot::Action(Action::ClosePane)),
    ("zoom", "prefix+z", Slot::Action(Action::Zoom)),
    ("resize_mode", "prefix+r", Slot::Action(Action::ResizeMode)),
    ("resize_pane_left", "", Slot::Action(Action::ResizePaneLeft)),
    ("resize_pane_down", "", Slot::Action(Action::ResizePaneDown)),
    ("resize_pane_up", "", Slot::Action(Action::ResizePaneUp)),
    (
        "resize_pane_right",
        "",
        Slot::Action(Action::ResizePaneRight),
    ),
    (
        "toggle_sidebar",
        "prefix+b",
        Slot::Action(Action::ToggleSidebar),
    ),
];

pub(crate) const RESOLVE_ORDER: &[Action] = &[
    Action::Help,
    Action::Settings,
    Action::Machines,
    Action::WorkspacePicker,
    Action::NewWorkspace,
    Action::NewWorktree,
    Action::OpenWorktree,
    Action::RemoveWorktree,
    Action::RenameWorkspace,
    Action::CloseWorkspace,
    Action::PreviousWorkspace,
    Action::NextWorkspace,
    Action::PreviousAgent,
    Action::NextAgent,
    Action::NewTab,
    Action::RenameTab,
    Action::PreviousTab,
    Action::NextTab,
    Action::MoveTabPrevious,
    Action::MoveTabNext,
    Action::CloseTab,
    Action::RenamePane,
    Action::EditScrollback,
    Action::ClearPane,
    Action::CopyMode,
    Action::FocusPaneLeft,
    Action::FocusPaneDown,
    Action::FocusPaneUp,
    Action::FocusPaneRight,
    Action::SwapPaneLeft,
    Action::SwapPaneDown,
    Action::SwapPaneUp,
    Action::SwapPaneRight,
    Action::LastPane,
    Action::CyclePaneNext,
    Action::CyclePanePrevious,
    Action::SplitVertical,
    Action::SplitHorizontal,
    Action::ClosePane,
    Action::Zoom,
    Action::ResizeMode,
    Action::ResizePaneLeft,
    Action::ResizePaneDown,
    Action::ResizePaneUp,
    Action::ResizePaneRight,
    Action::ToggleSidebar,
    Action::ReloadConfig,
    Action::OpenNotificationTarget,
    Action::Detach,
    Action::Goto,
];

pub fn default_binding(field: &str) -> Option<&'static str> {
    NAVIGATE_FIELDS
        .iter()
        .map(|(name, default, _)| (*name, *default))
        .chain(
            ACTION_FIELDS
                .iter()
                .map(|(name, default, _)| (*name, *default)),
        )
        .find_map(|(name, default)| (name == field).then_some(default))
}

pub fn binding_fields() -> impl Iterator<Item = &'static str> {
    NAVIGATE_FIELDS
        .iter()
        .map(|(name, ..)| *name)
        .chain(ACTION_FIELDS.iter().map(|(name, ..)| *name))
}
