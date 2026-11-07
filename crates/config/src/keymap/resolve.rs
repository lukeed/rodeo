use crossterm::event::{KeyCode, KeyModifiers};

use super::action::RESOLVE_ORDER;
use super::combo::shifted_digit;
use super::{Action, Binding, Indexed, KeyCombo, KeyPress, Keymap, Trigger};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKey {
    Run(Action),
    EnterPrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixKey {
    SendPrefix,
    Cancel,
    Run(Action),
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigateKey {
    Cancel,
    Open,
    Blocked,
    Stay(Action),
    Run(Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeKey {
    Leave,
    Step(Action),
}

const FOCUS_PANE: [Action; 4] = [
    Action::FocusPaneLeft,
    Action::FocusPaneDown,
    Action::FocusPaneUp,
    Action::FocusPaneRight,
];

impl Keymap {
    pub fn is_prefix(&self, key: impl Into<KeyPress>) -> bool {
        let key = key.into();
        self.prefixes.iter().any(|combo| combo.matches(key))
    }

    pub fn is_image_paste(&self, key: impl Into<KeyPress>) -> bool {
        self.image_paste
            .is_some_and(|combo| combo.matches(key.into()))
    }

    pub fn resolve_terminal(&self, key: impl Into<KeyPress>) -> Option<TerminalKey> {
        let key = key.into();
        if let Some(action) = self.lookup(key, Trigger::Direct) {
            return Some(TerminalKey::Run(action));
        }
        self.is_prefix(key).then_some(TerminalKey::EnterPrefix)
    }

    pub fn resolve_prefix(&self, key: impl Into<KeyPress>) -> PrefixKey {
        let key = key.into();
        if self.is_prefix(key) {
            return PrefixKey::SendPrefix;
        }
        if key.code == KeyCode::Esc {
            return PrefixKey::Cancel;
        }
        self.prefixed(key).map_or(PrefixKey::Drop, PrefixKey::Run)
    }

    pub fn prefixed(&self, key: impl Into<KeyPress>) -> Option<Action> {
        let key = key.into();
        self.lookup(key, Trigger::Prefix).or_else(|| {
            key.typed()
                .and_then(|typed| self.lookup(typed, Trigger::Prefix))
        })
    }

    pub fn direct(&self, key: impl Into<KeyPress>) -> Option<Action> {
        self.lookup(key.into(), Trigger::Direct)
    }

    pub fn resolve_navigate(
        &self,
        key: impl Into<KeyPress>,
        preview_blocked: bool,
    ) -> Option<NavigateKey> {
        let key = key.into();
        if key.code == KeyCode::Esc || self.is_prefix(key) {
            return Some(NavigateKey::Cancel);
        }
        for action in [Action::NavigateWorkspaceUp, Action::NavigateWorkspaceDown] {
            if fires(self.navigate_bindings(action), Trigger::Direct, key) {
                return Some(NavigateKey::Stay(action));
            }
        }
        let combo = KeyCombo::new(key.code, key.modifiers).normalized();
        if combo == KeyCombo::plain(KeyCode::Enter) {
            return Some(NavigateKey::Open);
        }
        let ctrl_bracket = key.code == KeyCode::Char('[') && key.modifiers == KeyModifiers::CONTROL;
        if preview_blocked {
            return Some(if ctrl_bracket {
                NavigateKey::Cancel
            } else {
                NavigateKey::Blocked
            });
        }
        if let Some(index) =
            ('1'..='9').position(|digit| KeyCombo::plain(KeyCode::Char(digit)).matches(key))
        {
            return Some(NavigateKey::Run(Action::SwitchWorkspace(index)));
        }
        if combo.modifiers.is_empty() {
            match combo.code {
                KeyCode::Tab => return Some(NavigateKey::Run(Action::CyclePaneNext)),
                KeyCode::BackTab => return Some(NavigateKey::Run(Action::CyclePanePrevious)),
                KeyCode::Left => return Some(NavigateKey::Stay(Action::FocusPaneLeft)),
                KeyCode::Right => return Some(NavigateKey::Stay(Action::FocusPaneRight)),
                _ => {}
            }
        }
        if let Some(action) = FOCUS_PANE
            .into_iter()
            .find(|action| fires(self.navigate_bindings(*action), Trigger::Direct, key))
        {
            return Some(NavigateKey::Stay(action));
        }
        let action = self
            .named(key, Trigger::Prefix)
            .filter(|action| !FOCUS_PANE.contains(action))
            .or_else(|| self.custom(key, Trigger::Prefix))
            .or_else(|| self.indexed_action(key, Trigger::Prefix));
        match action {
            Some(action) => Some(NavigateKey::Run(action)),
            None => ctrl_bracket.then_some(NavigateKey::Cancel),
        }
    }

    pub fn resolve_resize(&self, key: impl Into<KeyPress>) -> Option<ResizeKey> {
        let key = key.into();
        let toggle = self
            .bindings(Action::ResizeMode)
            .iter()
            .any(|binding| binding.combo.matches(key));
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter) || toggle {
            return Some(ResizeKey::Leave);
        }
        let action = match key.code {
            KeyCode::Char('h') | KeyCode::Left => Action::ResizePaneLeft,
            KeyCode::Char('j') | KeyCode::Down => Action::ResizePaneDown,
            KeyCode::Char('k') | KeyCode::Up => Action::ResizePaneUp,
            KeyCode::Char('l') | KeyCode::Right => Action::ResizePaneRight,
            _ => return None,
        };
        Some(ResizeKey::Step(action))
    }

    fn lookup(&self, key: KeyPress, trigger: Trigger) -> Option<Action> {
        self.named(key, trigger)
            .or_else(|| self.custom(key, trigger))
            .or_else(|| self.indexed_action(key, trigger))
    }

    fn named(&self, key: KeyPress, trigger: Trigger) -> Option<Action> {
        RESOLVE_ORDER
            .iter()
            .copied()
            .find(|action| fires(self.bindings(*action), trigger, key))
    }

    fn custom(&self, key: KeyPress, trigger: Trigger) -> Option<Action> {
        self.commands
            .iter()
            .find(|command| fires(&command.bindings, trigger, key))
            .map(|command| Action::Command(command.index))
    }

    fn indexed_action(&self, key: KeyPress, trigger: Trigger) -> Option<Action> {
        let modifiers = KeyCombo::new(key.code, key.modifiers)
            .normalized()
            .modifiers;
        for exact in [true, false] {
            for target in [Indexed::Tab, Indexed::Workspace, Indexed::Agent] {
                let found = self.indexed(target).iter().find_map(|binding| {
                    let same = binding.combo.normalized().modifiers == modifiers;
                    (binding.trigger == trigger && same == exact)
                        .then(|| index_for(binding, key))
                        .flatten()
                });
                if let Some(index) = found {
                    return Some(target.action(index));
                }
            }
        }
        None
    }
}

fn fires(bindings: &[Binding], trigger: Trigger, key: KeyPress) -> bool {
    bindings.iter().any(|binding| binding.fires(trigger, key))
}

fn index_for(binding: &Binding, key: KeyPress) -> Option<usize> {
    let expected = binding.combo.normalized();
    let index = expected.digit_index()?;
    let KeyCode::Char(digit) = expected.code else {
        return None;
    };
    let legacy_symbol = matches!(key.code, KeyCode::Char(ch) if shifted_digit(ch) == Some(digit))
        && expected.modifiers.contains(KeyModifiers::SHIFT)
        && key.modifiers == expected.modifiers.difference(KeyModifiers::SHIFT);
    (expected.matches(key) || legacy_symbol).then_some(index)
}
