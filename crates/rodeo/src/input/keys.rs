use config::{Action, NavigateKey, PrefixKey, ResizeKey, TerminalKey};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::App;
use crate::view::{Mode, Nav};

impl App {
    pub(super) fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if self.popup_key(&key) {
            return;
        }
        if let Some(mut overlay) = self.view.overlay.take() {
            let snapshot = self.snapshot();
            let ctx = crate::overlay::Ctx {
                palette: &self.palette,
                keymap: &self.keymap,
                config: &self.config,
                snapshot: &snapshot,
            };
            let outcome = overlay.key(&key, &ctx);
            self.view.overlay = Some(overlay);
            self.overlay_outcome(outcome);
            return;
        }
        if matches!(key.code, KeyCode::Modifier(_)) {
            return;
        }
        if self.retained_selection_copy(&key) {
            return;
        }
        if self.view.mode != Mode::Copy && self.view.selection.as_ref().is_some_and(|s| s.visible) {
            self.view.selection = None;
        }
        match self.view.mode {
            Mode::Terminal => self.terminal_key(key),
            Mode::Prefix => self.prefix_key(key),
            Mode::Navigate => self.navigate_key(key),
            Mode::Resize => self.resize_key(key),
            Mode::Copy => self.copy_key(key),
        }
    }

    fn retained_selection_copy(&mut self, key: &KeyEvent) -> bool {
        if self.config.ui.copy_on_select {
            return false;
        }
        let copy = key.code == KeyCode::Char('c')
            && (key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::SUPER);
        if !copy {
            return false;
        }
        let Some(selection) = self.view.selection.take().filter(|s| s.visible) else {
            return false;
        };
        let text = self
            .panes
            .get(&selection.address)
            .map(|p| p.term.selection_text(&selection.selection))
            .unwrap_or_default();
        self.copied(&text);
        true
    }

    pub fn copied(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.copy_to_host(text);
        if self.config.ui.toast.clipboard.enabled {
            self.toast_feedback("copied to clipboard");
        }
    }

    fn terminal_key(&mut self, key: KeyEvent) {
        if self.keymap.is_image_paste(key) {
            self.image_paste(Some(key));
            return;
        }
        match self.keymap.resolve_terminal(key) {
            Some(TerminalKey::Run(action)) => self.run_action(action),
            Some(TerminalKey::EnterPrefix) => self.view.mode = Mode::Prefix,
            None => self.send_key(&key),
        }
    }

    pub fn send_key(&mut self, key: &KeyEvent) {
        let Some(address) = self.focused_address() else {
            return;
        };
        if let Some(pane) = self.panes.get_mut(&address) {
            let bytes = pane.term.key(key);
            if !bytes.is_empty() {
                pane.term.scroll_to_bottom();
                pane.input(&bytes);
            }
        }
    }

    fn prefix_key(&mut self, key: KeyEvent) {
        let resolved = self.keymap.resolve_prefix(key);
        self.leave_mode();
        match resolved {
            PrefixKey::SendPrefix => self.send_key(&key),
            PrefixKey::Run(action) => self.run_action(action),
            PrefixKey::Cancel | PrefixKey::Drop => {}
        }
    }

    fn navigate_key(&mut self, key: KeyEvent) {
        if key.modifiers.is_empty()
            && matches!(key.code, KeyCode::Left | KeyCode::Right)
            && self.nav_arrow(key.code == KeyCode::Right)
        {
            return;
        }
        let Some(resolved) = self.keymap.resolve_navigate(key, false) else {
            return;
        };
        match resolved {
            NavigateKey::Cancel => self.leave_navigate(),
            NavigateKey::Open => self.nav_open(),
            NavigateKey::Blocked => {}
            NavigateKey::Stay(action) => self.run_action(action),
            NavigateKey::Run(Action::SwitchWorkspace(index)) => {
                if index < self.machines.spaces().len() {
                    self.leave_navigate();
                    self.run_action(Action::SwitchWorkspace(index));
                }
            }
            NavigateKey::Run(action @ (Action::SwitchTab(_) | Action::FocusAgent(_))) => {
                let exists = match action {
                    Action::SwitchTab(index) => self.snapshot().tabs.len() > index,
                    Action::FocusAgent(index) => self.snapshot().agents.len() > index,
                    _ => false,
                };
                if exists {
                    self.leave_navigate();
                    self.run_action(action);
                }
            }
            NavigateKey::Run(Action::Machines) => {
                let picked = match &self.view.nav {
                    Some(Nav::Machine(name)) => name.clone(),
                    _ => None,
                };
                self.leave_navigate();
                self.open_machines(picked);
            }
            NavigateKey::Run(action) => {
                self.leave_navigate();
                self.run_action(action);
            }
        }
    }

    fn leave_navigate(&mut self) {
        self.view.nav = None;
        self.leave_mode();
    }

    fn resize_key(&mut self, key: KeyEvent) {
        match self.keymap.resolve_resize(key) {
            Some(ResizeKey::Leave) => self.view.mode = Mode::Terminal,
            Some(ResizeKey::Step(action)) => self.run_action(action),
            None => {}
        }
    }
}
