mod action;
mod build;
mod combo;
mod help;
mod resolve;

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyModifiers};

pub use action::{Action, Indexed, binding_fields, default_binding};
pub use build::KeymapBuild;
pub use combo::{KeyCombo, KeyPress};
pub use help::{HelpEntry, HelpGroup, filter_help};
pub use resolve::{NavigateKey, PrefixKey, ResizeKey, TerminalKey};

use crate::keys::{CommandKind, PopupSize};

pub const DEFAULT_PREFIX_COMBO: KeyCombo = KeyCombo::new(KeyCode::Char('b'), KeyModifiers::CONTROL);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    Direct,
    Prefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub trigger: Trigger,
    pub combo: KeyCombo,
    pub label: String,
}

impl Binding {
    pub(crate) fn new(trigger: Trigger, combo: KeyCombo) -> Self {
        let key = combo.label();
        let label = match trigger {
            Trigger::Direct => key,
            Trigger::Prefix => format!("prefix+{key}"),
        };
        Self {
            trigger,
            combo,
            label,
        }
    }

    pub fn is_prefix(&self) -> bool {
        self.trigger == Trigger::Prefix
    }

    pub fn is_direct(&self) -> bool {
        self.trigger == Trigger::Direct
    }

    pub fn rhs_label(&self) -> &str {
        self.label.strip_prefix("prefix+").unwrap_or(&self.label)
    }

    pub(crate) fn fires(&self, trigger: Trigger, key: KeyPress) -> bool {
        self.trigger == trigger && self.combo.matches(key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCommand {
    pub index: usize,
    pub bindings: Vec<Binding>,
    pub label: String,
    pub command: String,
    pub kind: CommandKind,
    pub description: Option<String>,
    pub width: Option<PopupSize>,
    pub height: Option<PopupSize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    prefixes: Vec<KeyCombo>,
    actions: BTreeMap<Action, Vec<Binding>>,
    indexed: [Vec<Binding>; 3],
    navigate: BTreeMap<Action, Vec<Binding>>,
    commands: Vec<CustomCommand>,
    image_paste: Option<KeyCombo>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::build(&crate::keys::KeysConfig::default()).keymap
    }
}

impl Keymap {
    pub fn prefixes(&self) -> &[KeyCombo] {
        &self.prefixes
    }

    pub fn primary_prefix(&self) -> KeyCombo {
        self.prefixes
            .first()
            .copied()
            .unwrap_or(DEFAULT_PREFIX_COMBO)
    }

    pub fn prefix_label(&self) -> String {
        self.primary_prefix().label()
    }

    pub fn prefixes_label(&self) -> String {
        if self.prefixes.is_empty() {
            return DEFAULT_PREFIX_COMBO.label();
        }
        join(self.prefixes.iter().map(|combo| combo.label()))
    }

    pub fn bindings(&self, action: Action) -> &[Binding] {
        let found = match action {
            Action::SwitchTab(_) => Some(&self.indexed[Indexed::Tab.slot()]),
            Action::SwitchWorkspace(_) => Some(&self.indexed[Indexed::Workspace.slot()]),
            Action::FocusAgent(_) => Some(&self.indexed[Indexed::Agent.slot()]),
            Action::Command(index) => self
                .commands
                .iter()
                .find(|command| command.index == index)
                .map(|command| &command.bindings),
            Action::NavigateWorkspaceUp | Action::NavigateWorkspaceDown => {
                self.navigate.get(&action)
            }
            _ => self.actions.get(&action),
        };
        found.map_or(&[], Vec::as_slice)
    }

    pub fn indexed(&self, target: Indexed) -> &[Binding] {
        &self.indexed[target.slot()]
    }

    pub fn navigate_bindings(&self, action: Action) -> &[Binding] {
        self.navigate.get(&action).map_or(&[], Vec::as_slice)
    }

    pub fn commands(&self) -> &[CustomCommand] {
        &self.commands
    }

    pub fn command(&self, index: usize) -> Option<&CustomCommand> {
        self.commands.iter().find(|command| command.index == index)
    }

    pub fn image_paste(&self) -> Option<KeyCombo> {
        self.image_paste
    }

    pub fn label(&self, action: Action) -> Option<String> {
        let bindings = self.bindings(action);
        (!bindings.is_empty()).then(|| join(bindings.iter().map(|binding| binding.label.clone())))
    }

    pub fn rhs_label(&self, action: Action) -> Option<String> {
        let labels: Vec<String> = self
            .bindings(action)
            .iter()
            .filter(|binding| binding.is_prefix())
            .map(|binding| binding.rhs_label().to_string())
            .collect();
        (!labels.is_empty()).then(|| labels.join(" / "))
    }

    pub fn navigate_label(&self, action: Action) -> Option<String> {
        let bindings = self.navigate_bindings(action);
        (!bindings.is_empty()).then(|| join(bindings.iter().map(|binding| binding.label.clone())))
    }

    pub fn indexed_label(&self, target: Indexed) -> Option<String> {
        let bindings = self.indexed(target);
        if bindings.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        let mut rest = bindings;
        while let Some(first) = rest.first() {
            if let Some(stem) = range_stem(rest) {
                parts.push(format!("{stem}1..9"));
                rest = &rest[9..];
            } else {
                parts.push(first.label.clone());
                rest = &rest[1..];
            }
        }
        Some(parts.join(" / "))
    }
}

fn range_stem(bindings: &[Binding]) -> Option<&str> {
    let run = bindings.get(..9)?;
    let stem = run[0].label.strip_suffix('1')?;
    run.iter()
        .zip('1'..='9')
        .all(|(binding, digit)| binding.label.strip_suffix(digit) == Some(stem))
        .then_some(stem)
}

fn join(labels: impl Iterator<Item = String>) -> String {
    labels.collect::<Vec<_>>().join(" / ")
}
