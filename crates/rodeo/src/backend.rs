use std::io::{self, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::style::{
    Attribute, Color as HostColor, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
    SetUnderlineColor,
};
use crossterm::terminal::{self, Clear};
use crossterm::{execute, queue};
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier};
use vt::UnderlineStyle;

pub struct HostBackend<W: Write> {
    out: W,
}

impl<W: Write> HostBackend<W> {
    pub fn new(out: W) -> Self {
        Self { out }
    }
}

pub(crate) fn host(color: Color) -> HostColor {
    match color {
        Color::Reset => HostColor::Reset,
        Color::Black => HostColor::Black,
        Color::Red => HostColor::DarkRed,
        Color::Green => HostColor::DarkGreen,
        Color::Yellow => HostColor::DarkYellow,
        Color::Blue => HostColor::DarkBlue,
        Color::Magenta => HostColor::DarkMagenta,
        Color::Cyan => HostColor::DarkCyan,
        Color::Gray => HostColor::Grey,
        Color::DarkGray => HostColor::DarkGrey,
        Color::LightRed => HostColor::Red,
        Color::LightGreen => HostColor::Green,
        Color::LightBlue => HostColor::Blue,
        Color::LightYellow => HostColor::Yellow,
        Color::LightMagenta => HostColor::Magenta,
        Color::LightCyan => HostColor::Cyan,
        Color::White => HostColor::White,
        Color::Indexed(i) => HostColor::AnsiValue(i),
        Color::Rgb(r, g, b) => HostColor::Rgb { r, g, b },
    }
}

pub(crate) fn attributes(modifier: Modifier) -> impl Iterator<Item = Attribute> {
    let underline = UnderlineStyle::of(modifier).map(|style| match style {
        UnderlineStyle::Single => Attribute::Underlined,
        UnderlineStyle::Double => Attribute::DoubleUnderlined,
        UnderlineStyle::Curly => Attribute::Undercurled,
        UnderlineStyle::Dotted => Attribute::Underdotted,
        UnderlineStyle::Dashed => Attribute::Underdashed,
    });
    [
        (Modifier::BOLD, Attribute::Bold),
        (Modifier::DIM, Attribute::Dim),
        (Modifier::ITALIC, Attribute::Italic),
        (Modifier::SLOW_BLINK, Attribute::SlowBlink),
        (Modifier::RAPID_BLINK, Attribute::RapidBlink),
        (Modifier::REVERSED, Attribute::Reverse),
        (Modifier::HIDDEN, Attribute::Hidden),
        (Modifier::CROSSED_OUT, Attribute::CrossedOut),
    ]
    .into_iter()
    .filter(move |(flag, _)| modifier.contains(*flag))
    .map(|(_, attribute)| attribute)
    .chain(underline)
}

impl<W: Write> Backend for HostBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let out = &mut self.out;
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut underline = Color::Reset;
        let mut modifier = Modifier::empty();
        let mut last: Option<(u16, u16)> = None;
        for (x, y, cell) in content {
            if !matches!(last, Some((lx, ly)) if x == lx + 1 && y == ly) {
                queue!(out, MoveTo(x, y))?;
            }
            last = Some((x, y));
            if cell.modifier != modifier {
                queue!(out, SetAttribute(Attribute::Reset))?;
                for attribute in attributes(cell.modifier) {
                    queue!(out, SetAttribute(attribute))?;
                }
                modifier = cell.modifier;
                (fg, bg, underline) = (Color::Reset, Color::Reset, Color::Reset);
            }
            if cell.fg != fg {
                queue!(out, SetForegroundColor(host(cell.fg)))?;
                fg = cell.fg;
            }
            if cell.bg != bg {
                queue!(out, SetBackgroundColor(host(cell.bg)))?;
                bg = cell.bg;
            }
            if cell.underline_color != underline {
                queue!(out, SetUnderlineColor(host(cell.underline_color)))?;
                underline = cell.underline_color;
            }
            queue!(out, Print(cell.symbol()))?;
        }
        queue!(
            out,
            SetForegroundColor(HostColor::Reset),
            SetBackgroundColor(HostColor::Reset),
            SetUnderlineColor(HostColor::Reset),
            SetAttribute(Attribute::Reset),
        )
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.out, Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.out, Show)
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        crossterm::cursor::position().map(|(x, y)| Position { x, y })
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        execute!(self.out, MoveTo(x, y))
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        let region = match clear_type {
            ClearType::All => terminal::ClearType::All,
            ClearType::AfterCursor => terminal::ClearType::FromCursorDown,
            ClearType::BeforeCursor => terminal::ClearType::FromCursorUp,
            ClearType::CurrentLine => terminal::ClearType::CurrentLine,
            ClearType::UntilNewLine => terminal::ClearType::UntilNewLine,
        };
        execute!(self.out, Clear(region))
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        for _ in 0..n {
            queue!(self.out, Print("\n"))?;
        }
        self.out.flush()
    }

    fn size(&self) -> io::Result<Size> {
        let (width, height) = terminal::size()?;
        Ok(Size { width, height })
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let size = terminal::window_size()?;
        Ok(WindowSize {
            columns_rows: Size {
                width: size.columns,
                height: size.rows,
            },
            pixels: Size {
                width: size.width,
                height: size.height,
            },
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::*;

    fn drawn(cells: &[Cell]) -> String {
        let mut backend = HostBackend::new(Vec::new());
        let content = cells
            .iter()
            .enumerate()
            .map(|(x, cell)| (u16::try_from(x).unwrap(), 0, cell));
        backend.draw(content).unwrap();
        String::from_utf8(backend.out).unwrap()
    }

    fn cell(symbol: &'static str, style: Style) -> Cell {
        let mut cell = Cell::new(symbol);
        cell.set_style(style);
        cell
    }

    #[test]
    fn curly_underlines_reach_the_host() {
        let mut t = vt::Terminal::new(4, 1, vt::Options::default()).unwrap();
        t.feed(b"\x1b[4:3;58;2;255;0;0mC\x1b[0m\x1b[4mU\x1b[0mN");
        let mut buf = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 3, 1));
        t.render(buf.area, &mut buf, &vt::RenderOptions::default());
        let out = drawn(&buf.content);
        assert!(out.contains("\x1b[4:3m"), "{out:?}");
        assert!(out.contains("\x1b[58;2;255;0;0m"), "{out:?}");
        assert!(out.contains("\x1b[4mU"), "{out:?}");
        assert!(out.ends_with("N\x1b[39m\x1b[49m\x1b[59m\x1b[0m"), "{out:?}");
    }

    #[test]
    fn styles_change_only_when_cells_differ() {
        let red = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
        let out = drawn(&[cell("a", red), cell("b", red), cell("c", Style::new())]);
        assert_eq!(
            out,
            "\x1b[1;1H\x1b[0m\x1b[1m\x1b[38;5;1mab\x1b[0mc\x1b[39m\x1b[49m\x1b[59m\x1b[0m"
        );
    }
}
