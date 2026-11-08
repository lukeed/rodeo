use config::{HelpGroup, Palette, filter_help};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use unicode_width::UnicodeWidthStr;

use super::frame::{button, contrast, panel, popup, text};
use super::scroll::Bar;
use super::{Ctx, Drawn, Kind, Outcome, Overlay, TextField};

const PAGE: isize = 8;
const WHEEL: usize = 3;

#[derive(Default)]
pub struct Help {
    query: TextField,
    searching: bool,
    scroll: usize,
    max: usize,
    popup: Rect,
    close: Rect,
    bar: Option<Bar>,
    grab: Option<u16>,
}

pub fn typed(key: &KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(ch)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) =>
        {
            Some(ch)
        }
        _ => None,
    }
}

fn lines(groups: &[HelpGroup], p: &Palette) -> Vec<(usize, Line<'static>)> {
    if groups.is_empty() {
        let message = " no matching keybinds";
        return vec![(
            message.width(),
            Line::from(Span::styled(
                message,
                Style::new().fg(p.overlay1).bg(p.panel_bg),
            )),
        )];
    }
    let keys = groups
        .iter()
        .flat_map(|g| g.entries.iter().map(|e| e.keys.chars().count()))
        .max()
        .unwrap_or(8);
    let mut out = Vec::new();
    for group in groups {
        out.push((
            group.title.width() + 1,
            Line::from(Span::styled(
                format!(" {}", group.title),
                Style::new()
                    .fg(p.accent)
                    .bg(p.panel_bg)
                    .add_modifier(Modifier::BOLD),
            )),
        ));
        for entry in &group.entries {
            let key = format!(" {:<keys$} ", entry.keys);
            out.push((
                key.chars().count() + entry.label.chars().count(),
                Line::from(vec![
                    Span::styled(
                        key,
                        Style::new()
                            .fg(p.mauve)
                            .bg(p.panel_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(entry.label.clone(), Style::new().fg(p.text).bg(p.panel_bg)),
                ]),
            ));
        }
        out.push((0, Line::raw("")));
    }
    out
}

fn wrapped(lines: &[(usize, Line<'_>)], width: u16) -> usize {
    let width = usize::from(width.max(1));
    lines.iter().map(|(w, _)| (*w).max(1).div_ceil(width)).sum()
}

impl Help {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query(&self) -> &str {
        self.query.text()
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn searching(&self) -> bool {
        self.searching
    }

    fn step(&mut self, delta: isize) {
        self.scroll = self.scroll.saturating_add_signed(delta).min(self.max);
    }

    fn leave_search(&mut self) {
        self.searching = false;
        self.query.clear();
        self.scroll = 0;
    }

    fn body(&mut self, buf: &mut Buffer, body: Rect, ctx: &Ctx<'_>) {
        let p = ctx.palette;
        let groups = filter_help(ctx.keymap.help_groups(), self.query.text());
        let lines = lines(&groups, p);
        let viewport = usize::from(body.height.max(1));
        let overflow = wrapped(&lines, body.width) > viewport;
        let area = if overflow {
            Rect::new(body.x, body.y, body.width - 1, body.height)
        } else {
            body
        };
        self.max = wrapped(&lines, area.width).saturating_sub(viewport);
        self.scroll = self.scroll.min(self.max);
        Paragraph::new(lines.into_iter().map(|(_, l)| l).collect::<Vec<_>>())
            .wrap(Wrap { trim: false })
            .scroll((u16::try_from(self.scroll).unwrap_or(u16::MAX), 0))
            .render(area, buf);
        self.bar = overflow.then(|| {
            Bar::new(
                Rect::new(body.right() - 1, body.y, 1, body.height),
                self.max,
                viewport,
            )
        });
        if let Some(bar) = &self.bar {
            bar.render(
                buf,
                self.scroll,
                ("▐", p.overlay0),
                ("▐", p.overlay1),
                p.panel_bg,
            );
        }
    }

    fn search_key(&mut self, key: &KeyEvent) -> Outcome {
        let before = self.query.text().to_owned();
        if self.query.key(key) {
            if self.query.text() != before {
                self.scroll = 0;
            }
            return Outcome::Stay;
        }
        let ctrl = key.modifiers == KeyModifiers::CONTROL;
        match key.code {
            KeyCode::Esc => self.leave_search(),
            KeyCode::Enter => return Outcome::Close,
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            KeyCode::Char('p') if ctrl => self.step(-1),
            KeyCode::Char('n') if ctrl => self.step(1),
            KeyCode::PageUp => self.step(-PAGE),
            KeyCode::PageDown => self.step(PAGE),
            _ => {}
        }
        Outcome::Stay
    }
}

impl Overlay for Help {
    fn kind(&self) -> Kind {
        Kind::Help
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let p = ctx.palette;
        let outer = popup(area, 76, 22)?;
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        self.popup = outer;
        if inner.width < 20 || inner.height < 6 {
            return Some(Drawn::new(outer, None));
        }
        let bold = Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD);
        text(buf, inner.x, inner.y, inner.width, "keybinds", bold);
        self.close = Rect::new(inner.right() - 13, inner.y, 13, 1);
        let label = if self.searching {
            " esc back "
        } else {
            " esc close "
        };
        button(
            buf,
            self.close,
            label,
            Style::new()
                .fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD),
        );
        let search = inner.y + 1;
        let (hint, fg) = if self.searching {
            (" / ", p.text)
        } else {
            (" / press / to filter by command or shortcut", p.overlay0)
        };
        text(
            buf,
            inner.x,
            search,
            inner.width,
            hint,
            Style::new().fg(fg).bg(p.panel_bg),
        );
        let cursor = self
            .searching
            .then(|| {
                self.query.render(
                    Rect::new(inner.x + 3, search, inner.width.saturating_sub(3), 1),
                    buf,
                    Style::new().fg(p.text).bg(p.panel_bg),
                )
            })
            .flatten();
        let body = Rect::new(
            inner.x,
            inner.y + 3,
            inner.width,
            inner.height.saturating_sub(5),
        );
        self.body(buf, body, ctx);
        let footer = if self.searching {
            " edit ←→/home/end · kill ^u/^k · yank ^y · scroll ↑↓ · back esc"
        } else {
            " search / · scroll j/k/↑↓/pgup/pgdn · close esc/enter"
        };
        text(
            buf,
            inner.x,
            inner.bottom() - 1,
            inner.width,
            footer,
            Style::new().fg(p.overlay0).bg(p.panel_bg),
        );
        Some(Drawn::new(outer, cursor))
    }

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        if self.searching {
            return self.search_key(key);
        }
        match key.code {
            KeyCode::Esc | KeyCode::Enter => return Outcome::Close,
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll = self.max,
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            KeyCode::PageUp => self.step(-PAGE),
            KeyCode::PageDown => self.step(PAGE),
            _ => match typed(key) {
                Some('k') => self.step(-1),
                Some('j') => self.step(1),
                Some('/') => {
                    self.searching = true;
                    self.scroll = 0;
                }
                Some('?') => return Outcome::Close,
                _ => {}
            },
        }
        Outcome::Stay
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        let at = Position::new(event.column, event.row);
        match event.kind {
            MouseEventKind::ScrollUp => self.scroll = self.scroll.saturating_sub(WHEEL),
            MouseEventKind::ScrollDown => self.scroll = (self.scroll + WHEEL).min(self.max),
            MouseEventKind::Drag(MouseButton::Left) => {
                if let (Some(grab), Some(bar)) = (self.grab, self.bar) {
                    self.scroll = bar.drag(self.scroll, event.row, grab);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => self.grab = None,
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(bar) = self.bar.filter(|b| b.track.contains(at)) {
                    match bar.grab(self.scroll, event.row) {
                        Some(grab) => self.grab = Some(grab),
                        None => self.scroll = bar.jump(self.scroll, event.row),
                    }
                } else if self.close.contains(at) {
                    if !self.searching {
                        return Outcome::Close;
                    }
                    self.leave_search();
                } else if !self.popup.contains(at) {
                    return Outcome::Close;
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn paste(&mut self, text: &str) -> Outcome {
        if self.searching {
            self.query.insert(text);
            self.scroll = 0;
        }
        Outcome::Stay
    }
}

pub fn open(kind: &Kind, _ctx: &Ctx<'_>) -> Option<Box<dyn Overlay>> {
    (*kind == Kind::Help).then(|| Box::new(Help::new()) as Box<dyn Overlay>)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_count_wrapped_lines() {
        let lines = vec![(10, Line::raw("")), (0, Line::raw("")), (25, Line::raw(""))];
        assert_eq!(wrapped(&lines, 10), 1 + 1 + 3);
    }
}
