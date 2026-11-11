use config::Palette;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::frame::{button, contrast, panel, popup, row, text};
use super::rename::Answer;
use super::{Ctx, Drawn, Kind, Outcome, Overlay, Request};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseTarget {
    Workspace { space: String },
    Group { space: String },
    Tab { tab: String },
    Pane { pane: String },
}

pub struct Confirm {
    title: String,
    detail: String,
    yes: Rect,
}

impl Confirm {
    pub fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            yes: Rect::default(),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, p: &Palette) -> Option<Drawn> {
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
        self.yes = rects[0];
        Some(Drawn::new(outer, None))
    }

    pub fn key(key: &KeyEvent) -> Answer {
        match key.code {
            KeyCode::Enter => Answer::Yes,
            KeyCode::Esc => Answer::No,
            _ => Answer::Stay,
        }
    }

    pub fn mouse(&self, event: MouseEvent) -> Answer {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            Answer::Stay
        } else if self.yes.contains(Position::new(event.column, event.row)) {
            Answer::Yes
        } else {
            Answer::No
        }
    }
}

fn panes_word(panes: usize) -> String {
    if panes == 1 {
        "1 pane".to_owned()
    } else {
        format!("{panes} panes")
    }
}

pub struct ConfirmClose {
    target: CloseTarget,
    confirm: Confirm,
}

impl ConfirmClose {
    pub fn new(target: CloseTarget, workspace: &str, panes: usize) -> Self {
        Self {
            target,
            confirm: Confirm::new(
                "Close workspace?",
                format!("{workspace} — {}", panes_word(panes)),
            ),
        }
    }

    pub fn group(target: CloseTarget, workspace: &str, workspaces: usize, panes: usize) -> Self {
        Self {
            target,
            confirm: Confirm::new(
                "Close worktree group?",
                format!(
                    "{workspace} — {workspaces} workspaces, {}",
                    panes_word(panes)
                ),
            ),
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

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        self.confirm.render(area, buf, ctx.palette)
    }

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        match Confirm::key(key) {
            Answer::Yes => Outcome::Done(Request::Close(self.target.clone())),
            Answer::No => Outcome::Done(Request::Navigate),
            Answer::Stay => Outcome::Stay,
        }
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        match self.confirm.mouse(*event) {
            Answer::Yes => Outcome::Done(Request::Close(self.target.clone())),
            Answer::No => Outcome::Close,
            Answer::Stay => Outcome::Stay,
        }
    }
}
