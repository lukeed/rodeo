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

pub struct Rename {
    target: RenameTarget,
    field: TextField,
    buttons: [Rect; 3],
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
            field,
            buttons: [Rect::default(); 3],
        }
    }

    pub fn target(&self) -> &RenameTarget {
        &self.target
    }

    pub fn text(&self) -> &str {
        self.field.text()
    }

    fn save(&self) -> Outcome {
        Outcome::Done(Request::Rename {
            target: self.target.clone(),
            text: self.field.text().trim().to_owned(),
        })
    }
}

impl Overlay for Rename {
    fn kind(&self) -> Kind {
        Kind::Rename
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let p = ctx.palette;
        let outer = popup(area, 56, 7)?;
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        let title = Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD);
        text(
            buf,
            inner.x,
            inner.y,
            inner.width,
            self.target.title(),
            title,
        );
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

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let sup = key.modifiers.contains(KeyModifiers::SUPER);
        match key.code {
            KeyCode::Enter => self.save(),
            KeyCode::Esc => Outcome::Close,
            KeyCode::Char('c') if ctrl => {
                self.field.clear();
                Outcome::Stay
            }
            KeyCode::Backspace if sup => {
                self.field.clear();
                Outcome::Stay
            }
            _ => {
                self.field.key(key);
                Outcome::Stay
            }
        }
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return Outcome::Stay;
        }
        let at = Position::new(event.column, event.row);
        if self.buttons[0].contains(at) {
            self.save()
        } else if self.buttons[1].contains(at) {
            self.field.clear();
            Outcome::Stay
        } else {
            Outcome::Close
        }
    }

    fn paste(&mut self, text: &str) -> Outcome {
        self.field.insert(text);
        Outcome::Stay
    }
}
