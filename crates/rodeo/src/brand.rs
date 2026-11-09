use config::Palette;
use crossterm::queue;
use crossterm::style::{Attribute, SetAttribute, SetBackgroundColor, SetForegroundColor};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::backend::{attributes, host};

const PIXELS: [&str; 12] = [
    "....tt..tt....",
    "....tttttt....",
    "t...tttttt...t",
    "tt..aaaaaa..tt",
    ".tttttttttttt.",
    "..............",
    ".....aaaa.....",
    ".....aaaa.....",
    ".....aaaa.....",
    ".....aaaa.....",
    ".....aaaa.....",
    ".....aaaa.....",
];

pub const HAT_WIDTH: u16 = 14;
pub const HAT_HEIGHT: u16 = 6;
const GAP: u16 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ink {
    pub text: Color,
    pub accent: Color,
}

impl Ink {
    pub const PLAIN: Self = Self {
        text: Color::Reset,
        accent: Color::Reset,
    };

    pub fn of(palette: &Palette) -> Self {
        Self {
            text: palette.text,
            accent: palette.accent,
        }
    }

    fn pixel(self, row: usize, col: usize) -> Option<Color> {
        match PIXELS[row].as_bytes()[col] {
            b't' => Some(self.text),
            b'a' => Some(self.accent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    Optional,
    Always,
}

pub fn hat(buf: &mut Buffer, x: u16, y: u16, ink: Ink) {
    for row in 0..HAT_HEIGHT {
        for col in 0..HAT_WIDTH {
            let at = (x + col, y + row);
            if !buf.area.contains(at.into()) {
                continue;
            }
            let (r, c) = (usize::from(row) * 2, usize::from(col));
            let cell = &mut buf[at];
            match (ink.pixel(r, c), ink.pixel(r + 1, c)) {
                (None, None) => {}
                (Some(top), None) => {
                    cell.set_symbol("▀").set_fg(top);
                }
                (None, Some(bottom)) => {
                    cell.set_symbol("▄").set_fg(bottom);
                }
                (Some(top), Some(bottom)) if top == bottom => {
                    cell.set_symbol("█").set_fg(top);
                }
                (Some(top), Some(Color::Reset)) => {
                    cell.set_symbol("▄").set_fg(Color::Reset).set_bg(top);
                }
                (Some(top), Some(bottom)) => {
                    cell.set_symbol("▀").set_fg(top).set_bg(bottom);
                }
            }
        }
    }
}

pub fn wordmark(ink: Ink) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "rodeo",
            Style::new().fg(ink.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled("▍", Style::new().fg(ink.accent)),
    ])
}

pub fn splash(buf: &mut Buffer, area: Rect, ink: Ink, notes: &[Line<'_>], keep: Keep) {
    let fits = |width: usize| width <= usize::from(area.width);
    let notes_width = notes.iter().map(Line::width).max().unwrap_or(0);
    let notes_shown = !notes.is_empty() && (keep == Keep::Always || fits(notes_width));
    let mut shown = [
        fits(usize::from(HAT_WIDTH)),
        fits(wordmark(ink).width()),
        notes_shown,
    ];
    let heights = [
        HAT_HEIGHT,
        1,
        u16::try_from(notes.len()).unwrap_or(u16::MAX),
    ];
    let drop_order: &[usize] = match keep {
        Keep::Optional => &[2, 1, 0],
        Keep::Always => &[1, 0],
    };
    let height = |shown: &[bool; 3]| {
        let blocks = (0..3).filter(|&i| shown[i]);
        let gaps = u16::try_from(blocks.clone().count().saturating_sub(1)).unwrap_or(0);
        blocks.map(|i| heights[i]).sum::<u16>() + gaps
    };
    for &part in drop_order {
        if height(&shown) <= area.height {
            break;
        }
        shown[part] = false;
    }
    let total = height(&shown).min(area.height);
    let mut y = area.y + (area.height - total) / 2;
    let centered = |width: usize| {
        let width = u16::try_from(width).unwrap_or(u16::MAX).min(area.width);
        area.x + (area.width - width) / 2
    };
    if shown[0] {
        hat(buf, centered(usize::from(HAT_WIDTH)), y, ink);
        y += HAT_HEIGHT + 1;
    }
    if shown[1] {
        let line = wordmark(ink);
        buf.set_line(centered(line.width()), y, &line, area.width);
        y += 2;
    }
    if !shown[2] {
        return;
    }
    for line in notes {
        if y >= area.bottom() {
            return;
        }
        if fits(line.width()) {
            buf.set_line(centered(line.width()), y, line, area.width);
        } else {
            buf.set_line(area.x + 1, y, line, area.width.saturating_sub(1));
        }
        y += 1;
    }
}

pub fn lockup(beside: &[Line<'_>], ink: Ink) -> Buffer {
    let text = beside.iter().map(Line::width).max().unwrap_or(0);
    let width = HAT_WIDTH + GAP + u16::try_from(text).unwrap_or(u16::MAX - HAT_WIDTH - GAP);
    let mut buf = Buffer::empty(Rect::new(0, 0, width, HAT_HEIGHT));
    hat(&mut buf, 0, 0, ink);
    let rows = u16::try_from(beside.len())
        .unwrap_or(HAT_HEIGHT)
        .min(HAT_HEIGHT);
    let top = (HAT_HEIGHT - rows) / 2;
    for (y, line) in (top..top + rows).zip(beside) {
        buf.set_line(HAT_WIDTH + GAP, y, line, width - HAT_WIDTH - GAP);
    }
    buf
}

pub fn ansi(buf: &Buffer, color: bool) -> String {
    let area = buf.area;
    let mut out = Vec::new();
    for y in area.top()..area.bottom() {
        let lit = |x: u16| {
            let cell = &buf[(x, y)];
            cell.symbol() != " " || color && cell.bg != Color::Reset
        };
        let end = (area.left()..area.right())
            .rev()
            .find(|&x| lit(x))
            .map_or(area.left(), |x| x + 1);
        let mut pen = Pen::default();
        for x in area.left()..end {
            let cell = &buf[(x, y)];
            if color {
                let _ = pen.paint(&mut out, cell.modifier, cell.fg, cell.bg);
            }
            out.extend_from_slice(cell.symbol().as_bytes());
        }
        if pen != Pen::default() {
            let _ = queue!(out, SetAttribute(Attribute::Reset));
        }
        if y + 1 < area.bottom() {
            out.push(b'\n');
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Debug, PartialEq, Eq)]
struct Pen {
    modifier: Modifier,
    fg: Color,
    bg: Color,
}

impl Default for Pen {
    fn default() -> Self {
        Self {
            modifier: Modifier::empty(),
            fg: Color::Reset,
            bg: Color::Reset,
        }
    }
}

impl Pen {
    fn paint(
        &mut self,
        out: &mut Vec<u8>,
        modifier: Modifier,
        fg: Color,
        bg: Color,
    ) -> std::io::Result<()> {
        if self.modifier != modifier {
            if *self != Self::default() {
                queue!(out, SetAttribute(Attribute::Reset))?;
            }
            for attribute in attributes(modifier) {
                queue!(out, SetAttribute(attribute))?;
            }
            *self = Self {
                modifier,
                ..Self::default()
            };
        }
        if self.fg != fg {
            queue!(out, SetForegroundColor(host(fg)))?;
            self.fg = fg;
        }
        if self.bg != bg {
            queue!(out, SetBackgroundColor(host(bg)))?;
            self.bg = bg;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(buf: &Buffer) -> Vec<String> {
        let area = buf.area;
        (area.top()..area.bottom())
            .map(|y| {
                (area.left()..area.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    fn palette(name: &str) -> Palette {
        config::builtin_palette(name).expect("a built-in theme")
    }

    const SHAPE: [&str; 6] = [
        "    ██▄▄██    ",
        "█▄  ██████  ▄█",
        " ▀▀▀▀▀▀▀▀▀▀▀▀ ",
        "     ████     ",
        "     ████     ",
        "     ████     ",
    ];

    #[test]
    fn the_plain_hat_is_the_half_block_shape() {
        let mut buf = Buffer::empty(Rect::new(0, 0, HAT_WIDTH, HAT_HEIGHT));
        hat(&mut buf, 0, 0, Ink::PLAIN);
        assert_eq!(rows(&buf), SHAPE);
    }

    #[test]
    fn two_colored_cells_split_text_and_accent() {
        let ink = Ink {
            text: Color::Rgb(1, 1, 1),
            accent: Color::Rgb(2, 2, 2),
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, HAT_WIDTH + 2, HAT_HEIGHT + 1));
        hat(&mut buf, 1, 1, ink);
        let cell = |x: u16, y: u16| {
            let c = &buf[(x + 1, y + 1)];
            (c.symbol().to_owned(), c.fg, c.bg)
        };
        let (text, accent) = (ink.text, ink.accent);
        assert_eq!(cell(4, 0), ("█".into(), text, Color::Reset));
        assert_eq!(cell(6, 0), ("▄".into(), text, Color::Reset));
        assert_eq!(cell(0, 1), ("█".into(), text, Color::Reset));
        assert_eq!(cell(1, 1), ("▄".into(), text, Color::Reset));
        for x in 4..10 {
            assert_eq!(cell(x, 1), ("▀".into(), text, accent));
        }
        assert_eq!(cell(1, 2), ("▀".into(), text, Color::Reset));
        assert_eq!(cell(5, 3), ("█".into(), accent, Color::Reset));
        assert_eq!(cell(0, 0), (" ".into(), Color::Reset, Color::Reset));
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    #[test]
    fn a_reset_text_color_stays_in_the_foreground() {
        let ink = Ink::of(&palette("terminal"));
        let mut buf = Buffer::empty(Rect::new(0, 0, HAT_WIDTH, HAT_HEIGHT));
        hat(&mut buf, 0, 0, ink);
        let band = &buf[(4, 1)];
        assert_eq!(band.symbol(), "▀");
        assert_eq!((band.fg, band.bg), (Color::Reset, Color::Blue));
        assert_eq!(buf[(5, 3)].fg, Color::Blue);
    }

    #[test]
    fn the_hat_keeps_the_background_under_half_cells() {
        let mut buf = Buffer::empty(Rect::new(0, 0, HAT_WIDTH, HAT_HEIGHT));
        buf.set_style(buf.area, Style::new().bg(Color::Rgb(9, 9, 9)));
        let catppuccin = palette("catppuccin");
        hat(&mut buf, 0, 0, Ink::of(&catppuccin));
        assert_eq!(buf[(6, 0)].bg, Color::Rgb(9, 9, 9));
        assert_eq!(buf[(1, 2)].bg, Color::Rgb(9, 9, 9));
        assert_eq!(buf[(4, 1)].bg, catppuccin.accent);
    }

    #[test]
    fn the_hat_clips_at_the_buffer_edge() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 2));
        hat(&mut buf, 0, 0, Ink::PLAIN);
        assert_eq!(rows(&buf), ["    ██", "█▄  ██"]);
    }

    #[test]
    fn plain_ansi_is_the_glyphs_without_trailing_space() {
        let buf = lockup(&[Line::from("rodeo 1.2.3")], Ink::PLAIN);
        assert_eq!(
            ansi(&buf, false),
            [
                "    ██▄▄██",
                "█▄  ██████  ▄█",
                " ▀▀▀▀▀▀▀▀▀▀▀▀    rodeo 1.2.3",
                "     ████",
                "     ████",
                "     ████",
            ]
            .join("\n")
        );
    }

    #[test]
    fn colored_ansi_sets_and_resets_styles() {
        let ink = Ink {
            text: Color::Reset,
            accent: Color::Rgb(232, 103, 60),
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
        buf.set_line(0, 0, &wordmark(ink), 8);
        assert_eq!(
            ansi(&buf, true),
            "\x1b[1mrodeo\x1b[0m\x1b[38;2;232;103;60m▍\x1b[0m"
        );
    }
}
