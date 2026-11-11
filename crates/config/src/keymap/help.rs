use super::{Action, Indexed, Keymap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpEntry {
    pub keys: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpGroup {
    pub title: &'static str,
    pub entries: Vec<HelpEntry>,
}

const UNSET: &str = "unset";

const WORKSPACES_TABS: &[(Row, &str)] = &[
    (Row::Action(Action::WorkspacePicker), "workspace navigation"),
    (Row::Action(Action::Goto), "session navigator"),
    (Row::Action(Action::NewWorkspace), "new workspace"),
    (Row::Action(Action::NewWorktree), "new worktree"),
    (Row::Action(Action::OpenWorktree), "open worktree"),
    (
        Row::Action(Action::RemoveWorktree),
        "delete worktree checkout",
    ),
    (Row::Action(Action::RenameWorkspace), "rename workspace"),
    (Row::Action(Action::CloseWorkspace), "close workspace"),
    (Row::Action(Action::PreviousWorkspace), "previous workspace"),
    (Row::Action(Action::NextWorkspace), "next workspace"),
    (Row::Indexed(Indexed::Workspace), "switch workspace 1-9"),
    (Row::Action(Action::PreviousAgent), "previous agent"),
    (Row::Action(Action::NextAgent), "next agent"),
    (Row::Indexed(Indexed::Agent), "focus agent 1-9"),
    (Row::Action(Action::NewTab), "new tab"),
    (Row::Action(Action::RenameTab), "rename tab"),
    (Row::Action(Action::PreviousTab), "previous tab"),
    (Row::Action(Action::NextTab), "next tab"),
    (Row::Action(Action::MoveTabPrevious), "move tab left"),
    (Row::Action(Action::MoveTabNext), "move tab right"),
    (Row::Indexed(Indexed::Tab), "switch tab 1-9"),
    (Row::Action(Action::CloseTab), "close tab"),
];

const PANES: &[(Row, &str)] = &[
    (Row::Action(Action::SplitVertical), "split vertical"),
    (Row::Action(Action::SplitHorizontal), "split horizontal"),
    (Row::Action(Action::ClosePane), "close pane"),
    (Row::Action(Action::RenamePane), "rename pane"),
    (Row::Action(Action::EditScrollback), "edit scrollback"),
    (Row::Action(Action::ClearPane), "clear pane"),
    (Row::Action(Action::CopyMode), "copy mode"),
    (Row::Action(Action::Zoom), "zoom pane"),
    (Row::Action(Action::ResizeMode), "resize mode"),
    (Row::Action(Action::ResizePaneLeft), "resize pane left"),
    (Row::Action(Action::ResizePaneDown), "resize pane down"),
    (Row::Action(Action::ResizePaneUp), "resize pane up"),
    (Row::Action(Action::ResizePaneRight), "resize pane right"),
    (Row::Action(Action::ToggleSidebar), "toggle sidebar"),
    (Row::Action(Action::FocusPaneLeft), "focus pane left"),
    (Row::Action(Action::FocusPaneDown), "focus pane down"),
    (Row::Action(Action::FocusPaneUp), "focus pane up"),
    (Row::Action(Action::FocusPaneRight), "focus pane right"),
    (Row::Action(Action::CyclePaneNext), "cycle pane next"),
    (
        Row::Action(Action::CyclePanePrevious),
        "cycle pane previous",
    ),
    (Row::Action(Action::LastPane), "last pane"),
];

#[derive(Clone, Copy)]
enum Row {
    Action(Action),
    Indexed(Indexed),
}

fn entry(keys: impl Into<String>, label: impl Into<String>) -> HelpEntry {
    HelpEntry {
        keys: keys.into(),
        label: label.into(),
    }
}

impl Keymap {
    fn or_unset(&self, action: Action) -> String {
        self.label(action).unwrap_or_else(|| UNSET.into())
    }

    fn navigate_or_unset(&self, action: Action) -> String {
        self.navigate_label(action).unwrap_or_else(|| UNSET.into())
    }

    fn rows(&self, rows: &[(Row, &str)]) -> Vec<HelpEntry> {
        rows.iter()
            .map(|(row, label)| {
                let keys = match row {
                    Row::Action(action) => self.or_unset(*action),
                    Row::Indexed(target) => {
                        self.indexed_label(*target).unwrap_or_else(|| UNSET.into())
                    }
                };
                entry(keys, *label)
            })
            .collect()
    }

    pub fn help_groups(&self) -> Vec<HelpGroup> {
        let global = vec![
            entry(self.prefixes_label(), "prefix mode"),
            entry(self.or_unset(Action::Help), "keybinds"),
            entry(self.or_unset(Action::Settings), "settings"),
            entry(self.or_unset(Action::Machines), "machines"),
            entry(self.or_unset(Action::Detach), "detach"),
            entry(self.or_unset(Action::ReloadConfig), "reload config"),
            entry(
                self.or_unset(Action::OpenNotificationTarget),
                "open notification target",
            ),
        ];
        let navigation = vec![
            entry("esc", "back"),
            entry(
                format!(
                    "{} / {}",
                    self.navigate_or_unset(Action::NavigateWorkspaceUp),
                    self.navigate_or_unset(Action::NavigateWorkspaceDown)
                ),
                "workspace list",
            ),
            entry(
                format!(
                    "{} / {} / {} / {} / left / right",
                    self.navigate_or_unset(Action::FocusPaneLeft),
                    self.navigate_or_unset(Action::FocusPaneDown),
                    self.navigate_or_unset(Action::FocusPaneUp),
                    self.navigate_or_unset(Action::FocusPaneRight)
                ),
                "move focus",
            ),
            entry("tab / shift+tab", "cycle pane"),
            entry("enter", "open workspace"),
            entry("1..9", "switch workspace"),
        ];
        let mut groups = vec![
            HelpGroup {
                title: "global",
                entries: global,
            },
            HelpGroup {
                title: "navigation",
                entries: navigation,
            },
            HelpGroup {
                title: "workspaces / tabs",
                entries: self.rows(WORKSPACES_TABS),
            },
            HelpGroup {
                title: "panes",
                entries: self.rows(PANES),
            },
        ];
        if !self.commands.is_empty() {
            groups.push(HelpGroup {
                title: "custom",
                entries: self
                    .commands
                    .iter()
                    .map(|command| {
                        entry(
                            command.label.clone(),
                            command
                                .description
                                .clone()
                                .unwrap_or_else(|| "custom command".into()),
                        )
                    })
                    .collect(),
            });
        }
        groups
    }
}

pub fn filter_help(groups: Vec<HelpGroup>, query: &str) -> Vec<HelpGroup> {
    if query.is_empty() {
        return groups;
    }
    let query = query.to_lowercase();
    groups
        .into_iter()
        .filter_map(|mut group| {
            group.entries.retain(|entry| {
                entry.keys.to_lowercase().contains(&query)
                    || entry.label.to_lowercase().contains(&query)
            });
            (!group.entries.is_empty()).then_some(group)
        })
        .collect()
}
