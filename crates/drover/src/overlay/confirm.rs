use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::frame::{button, contrast, panel, popup, row, text};
use super::{Ctx, Kind, Outcome, Overlay, Request};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseTarget {
    Workspace { space: String },
    Tab { tab: String },
    Pane { pane: String },
}

pub struct ConfirmClose {
    target: CloseTarget,
    title: String,
    detail: String,
    confirm: Rect,
}

impl ConfirmClose {
    pub fn new(target: CloseTarget, workspace: &str, panes: usize) -> Self {
        let scope = if panes == 1 {
            "1 pane".to_owned()
        } else {
            format!("{panes} panes")
        };
        Self {
            target,
            title: "Close workspace?".to_owned(),
            detail: format!("{workspace} — {scope}"),
            confirm: Rect::default(),
        }
    }

    pub fn target(&self) -> &CloseTarget {
        &self.target
    }
}

impl Overlay for ConfirmClose {
    fn kind(&self) -> Kind {
        Kind::ConfirmClose
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<(u16, u16)> {
        let p = ctx.palette;
        let outer = popup(area, 64, 6)?;
        let inner = panel(buf, outer, p.red, p.panel_bg)?;
        let title = Style::new()
            .fg(p.red)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD);
        text(
            buf,
            inner.x,
            inner.y,
            inner.width,
            &format!(" {}", self.title),
            title,
        );
        let detail = Style::new().fg(p.text).bg(p.panel_bg);
        text(
            buf,
            inner.x,
            inner.y + 1,
            inner.width,
            &format!(" {}", self.detail),
            detail,
        );
        let rects = row(inner, &[13, 12], 2, 3);
        let danger = Style::new()
            .fg(contrast(p))
            .bg(p.red)
            .add_modifier(Modifier::BOLD);
        let plain = Style::new()
            .fg(p.text)
            .bg(p.surface0)
            .add_modifier(Modifier::BOLD);
        button(buf, rects[0], " ↵ confirm ", danger);
        button(buf, rects[1], " esc cancel ", plain);
        self.confirm = rects[0];
        None
    }

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        match key.code {
            KeyCode::Enter => Outcome::Done(Request::Close(self.target.clone())),
            KeyCode::Esc => Outcome::Done(Request::Navigate),
            _ => Outcome::Stay,
        }
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return Outcome::Stay;
        }
        if self
            .confirm
            .contains(Position::new(event.column, event.row))
        {
            Outcome::Done(Request::Close(self.target.clone()))
        } else {
            Outcome::Close
        }
    }
}
