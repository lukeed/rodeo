use config::{Action, Palette};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::frame::{contrast, panel, text};
use super::{Ctx, Drawn, Kind, Outcome, Overlay, Request};
use crate::text::width;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct List {
    pub labels: Vec<String>,
    pub highlighted: usize,
    pub rows: Vec<Rect>,
    pub area: Rect,
}

impl List {
    pub fn new(labels: Vec<String>) -> Self {
        Self {
            labels,
            ..Self::default()
        }
    }

    pub fn size(&self, pad: u16, min: u16) -> (u16, u16) {
        let widest = self.labels.iter().map(|l| width(l)).max().unwrap_or(0);
        let rows = u16::try_from(self.labels.len()).unwrap_or(u16::MAX);
        ((widest + pad).max(min), rows.saturating_add(2))
    }

    pub fn step(&mut self, delta: isize) {
        let last = self.labels.len().saturating_sub(1);
        self.highlighted = self.highlighted.saturating_add_signed(delta).min(last);
    }

    pub fn at(&self, column: u16, row: u16) -> Option<usize> {
        let at = Position::new(column, row);
        self.rows.iter().position(|r| r.contains(at))
    }

    pub fn render(&mut self, buf: &mut Buffer, rect: Rect, p: &Palette) {
        self.area = rect;
        self.rows.clear();
        let Some(inner) = panel(buf, rect, p.accent, p.panel_bg) else {
            return;
        };
        for (index, label) in self.labels.iter().enumerate() {
            let y = inner.y + u16::try_from(index).unwrap_or(u16::MAX);
            if y >= inner.bottom() {
                break;
            }
            let row = Rect::new(inner.x, y, inner.width, 1);
            let style = if index == self.highlighted {
                Style::new()
                    .fg(contrast(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(p.text).bg(p.panel_bg)
            };
            buf.set_style(row, style);
            text(buf, row.x, row.y, row.width, label, style);
            self.rows.push(row);
        }
    }

    pub fn key(&mut self, key: &KeyEvent, vim: bool) -> Option<Option<usize>> {
        match key.code {
            KeyCode::Esc => Some(None),
            KeyCode::Enter => Some(Some(self.highlighted)),
            KeyCode::Up => {
                self.step(-1);
                None
            }
            KeyCode::Down => {
                self.step(1);
                None
            }
            KeyCode::Char('k') if vim => {
                self.step(-1);
                None
            }
            KeyCode::Char('j') if vim => {
                self.step(1);
                None
            }
            _ => None,
        }
    }
}

pub fn clamp(screen: Rect, x: u16, y: u16, (w, h): (u16, u16)) -> Rect {
    let w = w.min(screen.width.max(1));
    let h = h.min(screen.height.max(1));
    Rect::new(
        x.min(screen.right().saturating_sub(w)),
        y.min(screen.bottom().saturating_sub(h)),
        w,
        h,
    )
}

const GLOBAL: &[(&str, Action)] = &[
    ("settings", Action::Settings),
    ("keybinds", Action::Help),
    ("reload config", Action::ReloadConfig),
    ("detach", Action::Detach),
];

pub struct GlobalMenu {
    list: List,
    launcher: Rect,
}

impl GlobalMenu {
    pub fn new(launcher: Rect) -> Self {
        Self {
            list: List::new(GLOBAL.iter().map(|(l, _)| format!(" {l}")).collect()),
            launcher,
        }
    }

    pub fn highlighted(&self) -> usize {
        self.list.highlighted
    }

    fn pick(index: usize) -> Outcome {
        GLOBAL.get(index).map_or(Outcome::Close, |(_, action)| {
            Outcome::Done(Request::Action(*action))
        })
    }
}

impl Overlay for GlobalMenu {
    fn kind(&self) -> Kind {
        Kind::GlobalMenu
    }

    fn dims(&self) -> bool {
        false
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let (w, h) = self.list.size(3, 0);
        let w = w.min(area.width.max(1));
        let h = h.min(area.height.max(1));
        let x = self
            .launcher
            .right()
            .saturating_sub(w)
            .min(area.right().saturating_sub(w));
        let y = self.launcher.y.saturating_sub(h).max(area.y);
        let rect = Rect::new(x, y, w, h);
        self.list.render(buf, rect, ctx.palette);
        Some(Drawn::new(rect, None))
    }

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        match self.list.key(key, true) {
            Some(Some(index)) => Self::pick(index),
            Some(None) => Outcome::Close,
            None => Outcome::Stay,
        }
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        let hit = self.list.at(event.column, event.row);
        match event.kind {
            MouseEventKind::Moved => {
                if let Some(index) = hit {
                    self.list.highlighted = index;
                }
                Outcome::Stay
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self
                    .launcher
                    .contains(Position::new(event.column, event.row))
                {
                    Outcome::Close
                } else {
                    hit.map_or(Outcome::Close, Self::pick)
                }
            }
            _ => Outcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_clamp_at_both_ends() {
        let mut list = List::new(vec!["a".into(), "b".into()]);
        list.step(-1);
        assert_eq!(list.highlighted, 0);
        list.step(5);
        assert_eq!(list.highlighted, 1);
        assert_eq!(list.size(4, 14), (14, 4));
    }

    #[test]
    fn menus_stay_on_screen() {
        let screen = Rect::new(0, 0, 80, 24);
        assert_eq!(clamp(screen, 75, 20, (14, 6)), Rect::new(66, 18, 14, 6));
        assert_eq!(clamp(screen, 3, 4, (14, 6)), Rect::new(3, 4, 14, 6));
    }
}
