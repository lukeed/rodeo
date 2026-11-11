use config::Palette;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::frame::{button, contrast, panel, popup, row, text};
use super::{Ctx, Drawn, Kind, Outcome, Overlay, Request, TextField};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameTarget {
    NewWorkspace {
        suggested: String,
    },
    Workspace {
        space: String,
    },
    NewTab {
        space: String,
        suggested: String,
    },
    Tab {
        tab: String,
        original: String,
        custom: bool,
    },
    Pane {
        pane: String,
    },
}

impl RenameTarget {
    pub fn title(&self) -> &'static str {
        match self {
            Self::NewWorkspace { .. } => "new workspace",
            Self::Workspace { .. } => "rename workspace",
            Self::NewTab { .. } => "new tab",
            Self::Tab { .. } => "rename tab",
            Self::Pane { .. } => "rename pane",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Stay,
    Yes,
    No,
}

pub struct FieldDialog {
    pub field: TextField,
    buttons: [Rect; 3],
}

impl FieldDialog {
    pub fn new(field: TextField) -> Self {
        Self {
            field,
            buttons: [Rect::default(); 3],
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        p: &Palette,
        title: &str,
    ) -> Option<Drawn> {
        let outer = popup(area, 56, 7)?;
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        let bold = Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD);
        text(buf, inner.x, inner.y, inner.width, title, bold);
        let input = Rect::new(inner.x, inner.y + 2, inner.width, 1);
        let style = Style::new().fg(p.text).bg(p.surface0);
        buf.set_style(input, style);
        let cursor = self.field.render(
            Rect::new(input.x + 1, input.y, input.width.saturating_sub(1), 1),
            buf,
            style,
        );
        let rects = row(inner, &[8, 10, 12], 2, 3);
        let plain = Style::new()
            .fg(p.text)
            .bg(p.surface0)
            .add_modifier(Modifier::BOLD);
        let primary = Style::new()
            .fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD);
        button(buf, rects[0], " ↵ save ", primary);
        button(buf, rects[1], " ^c clear ", plain);
        button(buf, rects[2], " esc cancel ", plain);
        self.buttons = [rects[0], rects[1], rects[2]];
        Some(Drawn::new(outer, cursor))
    }

    pub fn key(&mut self, key: &KeyEvent) -> Answer {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let sup = key.modifiers.contains(KeyModifiers::SUPER);
        match key.code {
            KeyCode::Enter => Answer::Yes,
            KeyCode::Esc => Answer::No,
            KeyCode::Char('c') if ctrl => {
                self.field.clear();
                Answer::Stay
            }
            KeyCode::Backspace if sup => {
                self.field.clear();
                Answer::Stay
            }
            _ => {
                self.field.key(key);
                Answer::Stay
            }
        }
    }

    pub fn mouse(&mut self, event: MouseEvent) -> Answer {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return Answer::Stay;
        }
        let at = Position::new(event.column, event.row);
        if self.buttons[0].contains(at) {
            Answer::Yes
        } else if self.buttons[1].contains(at) {
            self.field.clear();
            Answer::Stay
        } else {
            Answer::No
        }
    }
}

pub struct Rename {
    target: RenameTarget,
    dialog: FieldDialog,
}

impl Rename {
    pub fn new(target: RenameTarget, initial: &str) -> Self {
        let field = match &target {
            RenameTarget::NewWorkspace { .. } | RenameTarget::NewTab { .. } => {
                TextField::suggested(initial)
            }
            RenameTarget::Pane { .. } if initial.is_empty() => TextField::suggested(""),
            _ => TextField::new(initial),
        };
        Self {
            target,
            dialog: FieldDialog::new(field),
        }
    }

    pub fn target(&self) -> &RenameTarget {
        &self.target
    }

    pub fn text(&self) -> &str {
        self.dialog.field.text()
    }

    fn answer(&self, answer: Answer) -> Outcome {
        match answer {
            Answer::Stay => Outcome::Stay,
            Answer::Yes => Outcome::Done(Request::Rename {
                target: self.target.clone(),
                text: self.dialog.field.text().trim().to_owned(),
            }),
            Answer::No => Outcome::Close,
        }
    }
}

impl Overlay for Rename {
    fn kind(&self) -> Kind {
        Kind::Rename
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        self.dialog
            .render(area, buf, ctx.palette, self.target.title())
    }

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        let answer = self.dialog.key(key);
        self.answer(answer)
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        let answer = self.dialog.mouse(*event);
        self.answer(answer)
    }

    fn paste(&mut self, text: &str) -> Outcome {
        self.dialog.field.insert(text);
        Outcome::Stay
    }
}
