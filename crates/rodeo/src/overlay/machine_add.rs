use config::Palette;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::frame::{button, contrast, panel, popup, row, text};
use super::machines::MachineOp;
use super::rename::Answer;
use super::{Drawn, TextField};

pub(super) struct AddForm {
    fields: [TextField; 2],
    focus: usize,
    error: Option<String>,
    inputs: [Rect; 2],
    buttons: [Rect; 3],
}

impl AddForm {
    pub(super) fn new() -> Self {
        Self {
            fields: [TextField::default(), TextField::default()],
            focus: 0,
            error: None,
            inputs: [Rect::default(); 2],
            buttons: [Rect::default(); 3],
        }
    }

    pub(super) fn submit(&mut self) -> Option<MachineOp> {
        let target = self.fields[0].text().trim().to_owned();
        if target.is_empty() {
            self.error = Some("ssh target is required".to_owned());
            self.focus = 0;
            return None;
        }
        let name = self.fields[1].text().trim().to_owned();
        Some(MachineOp::Add {
            target,
            name: (!name.is_empty()).then_some(name),
        })
    }

    pub(super) fn render(&mut self, area: Rect, buf: &mut Buffer, p: &Palette) -> Option<Drawn> {
        let outer = popup(area, 56, 11)?;
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        let bold = Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD);
        let dim = Style::new().fg(p.overlay0).bg(p.panel_bg);
        text(buf, inner.x, inner.y, inner.width, "add machine", bold);
        let input = Style::new().fg(p.text).bg(p.surface0);
        let mut cursor = None;
        for (index, label) in [" ssh target", " name (optional)"].into_iter().enumerate() {
            let y = inner.y + 2 + 2 * u16::try_from(index).unwrap_or(0);
            text(buf, inner.x, y, inner.width, label, dim);
            let rect = Rect::new(inner.x, y + 1, inner.width, 1);
            buf.set_style(rect, input);
            let at = self.fields[index].render(
                Rect::new(rect.x + 1, rect.y, rect.width.saturating_sub(1), 1),
                buf,
                input,
            );
            if index == self.focus {
                cursor = at;
            }
            self.inputs[index] = rect;
        }
        if let Some(error) = &self.error {
            text(
                buf,
                inner.x,
                inner.y + 6,
                inner.width,
                &format!(" {error}"),
                Style::new().fg(p.red).bg(p.panel_bg),
            );
        }
        let rects = row(inner, &[8, 10, 12], 2, 7);
        let plain = Style::new()
            .fg(p.text)
            .bg(p.surface0)
            .add_modifier(Modifier::BOLD);
        let primary = Style::new()
            .fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD);
        button(buf, rects[0], " ↵ add ", primary);
        button(buf, rects[1], " tab next ", plain);
        button(buf, rects[2], " esc cancel ", plain);
        self.buttons = [rects[0], rects[1], rects[2]];
        Some(Drawn::new(outer, cursor))
    }

    pub(super) fn paste(&mut self, pasted: &str) {
        self.fields[self.focus].insert(pasted);
        self.error = None;
    }

    pub(super) fn key(&mut self, key: &KeyEvent) -> Answer {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => return Answer::Yes,
            KeyCode::Esc => return Answer::No,
            KeyCode::Tab | KeyCode::Down | KeyCode::BackTab | KeyCode::Up => {
                self.focus = 1 - self.focus;
            }
            KeyCode::Char('c') if ctrl => self.fields[self.focus].clear(),
            _ => {
                self.fields[self.focus].key(key);
                self.error = None;
            }
        }
        Answer::Stay
    }

    pub(super) fn mouse(&mut self, at: Position) -> Answer {
        if self.buttons[0].contains(at) {
            Answer::Yes
        } else if self.buttons[1].contains(at) {
            self.focus = 1 - self.focus;
            Answer::Stay
        } else if let Some(index) = self.inputs.iter().position(|r| r.contains(at)) {
            self.focus = index;
            Answer::Stay
        } else {
            Answer::No
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_add_form_needs_a_target() {
        let mut form = AddForm::new();
        assert_eq!(form.submit(), None);
        assert_eq!(form.error.as_deref(), Some("ssh target is required"));
        form.fields[0].insert(" me@vps ");
        assert_eq!(
            form.submit(),
            Some(MachineOp::Add {
                target: "me@vps".into(),
                name: None
            })
        );
        form.fields[1].insert("vps");
        assert_eq!(
            form.submit(),
            Some(MachineOp::Add {
                target: "me@vps".into(),
                name: Some("vps".into())
            })
        );
    }
}
