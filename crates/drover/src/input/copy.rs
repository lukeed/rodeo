use crossterm::event::{KeyCode, KeyEvent};
use vt::CopyAction;

use crate::app::App;
use crate::overlay::TextField;
use crate::view::{Mode, Prompt};

impl App {
    pub(super) fn copy_key(&mut self, key: KeyEvent) {
        let Some(copy) = self.view.copy.as_mut() else {
            self.view.mode = Mode::Terminal;
            return;
        };
        let Some(pane) = self.panes.get_mut(&copy.address) else {
            self.view.copy = None;
            self.view.mode = Mode::Terminal;
            return;
        };
        if let Some(prompt) = copy.prompt.as_mut() {
            match key.code {
                KeyCode::Esc => copy.prompt = None,
                KeyCode::Enter => {
                    let query = prompt.field.text().to_owned();
                    let direction = prompt.direction;
                    copy.prompt = None;
                    copy.mode.find(&mut pane.term, &query, direction);
                }
                _ => {
                    prompt.field.key(&key);
                }
            }
            return;
        }
        if self.keymap.is_prefix(key) {
            self.view.mode = Mode::Prefix;
            return;
        }
        match copy.mode.key(&mut pane.term, &key) {
            CopyAction::None | CopyAction::Redraw => {}
            CopyAction::Prompt(direction) => {
                copy.prompt = Some(Prompt {
                    direction,
                    field: TextField::default(),
                });
            }
            CopyAction::Copy(selection) => {
                let text = selection
                    .map(|s| pane.term.selection_text(&s))
                    .unwrap_or_default();
                self.view.copy = None;
                self.view.mode = Mode::Terminal;
                self.copied(&text);
            }
            CopyAction::Exit => {
                self.view.copy = None;
                self.view.mode = Mode::Terminal;
            }
        }
    }
}
