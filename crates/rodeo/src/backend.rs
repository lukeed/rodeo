use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

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

const SYNC_BEGIN: &[u8] = b"\x1b[?2026h";
const SYNC_END: &[u8] = b"\x1b[?2026l";

#[derive(Clone, Default)]
pub struct Pending(Rc<RefCell<Vec<u8>>>);

impl Write for Pending {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Synced<W: Write> {
    pending: Pending,
    out: W,
}

impl<W: Write> Synced<W> {
    pub fn new(pending: Pending, out: W) -> Self {
        Self { pending, out }
    }
}

impl<W: Write> Write for Synced<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let body = std::mem::take(&mut *self.pending.0.borrow_mut());
        if !body.is_empty() {
            let mut frame = Vec::with_capacity(SYNC_BEGIN.len() + body.len() + SYNC_END.len());
            frame.extend_from_slice(SYNC_BEGIN);
            frame.extend_from_slice(&body);
            frame.extend_from_slice(SYNC_END);
            self.out.write_all(&frame)?;
        }
        self.out.flush()
    }
}

pub struct HostBackend<W: Write> {
    out: W,
    cursor: Position,
}

impl<W: Write> HostBackend<W> {
    pub fn new(out: W) -> Self {
        Self {
            out,
            cursor: Position::ORIGIN,
        }
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

fn symbol_width(cell: &Cell) -> u16 {
    u16::try_from(unicode_width::UnicodeWidthStr::width(cell.symbol())).unwrap_or(u16::MAX)
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
            self.cursor = Position::new(x.saturating_add(symbol_width(cell)), y);
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
        // A host query would race the input thread for the reply and stall up to 2s.
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        execute!(self.out, MoveTo(x, y))?;
        self.cursor = Position::new(x, y);
        Ok(())
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

    #[derive(Clone, Default)]
    struct Writes(Rc<RefCell<Vec<Vec<u8>>>>);

    impl Write for Writes {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().push(buf.to_vec());
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_frame_and_its_extras_reach_the_host_in_one_synchronized_write() {
        let pending = Pending::default();
        let writes = Writes::default();
        let mut host = Synced::new(pending.clone(), writes.clone());
        let backend = HostBackend::new(pending);
        let mut terminal = ratatui::Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 8, 2)),
            },
        )
        .unwrap();
        terminal.backend_mut().clear().unwrap();
        terminal
            .draw(|frame| {
                frame.buffer_mut().set_string(0, 0, "frame", Style::new());
                frame.set_cursor_position((1, 1));
            })
            .unwrap();
        host.write_all(b"\x1b7extra\x1b8").unwrap();
        assert!(writes.0.borrow().is_empty());
        host.flush().unwrap();
        let writes = writes.0.borrow();
        assert_eq!(writes.len(), 1);
        let out = String::from_utf8_lossy(&writes[0]);
        assert!(out.starts_with("\x1b[?2026h\x1b[2J"), "{out:?}");
        assert!(out.ends_with("\x1b7extra\x1b8\x1b[?2026l"), "{out:?}");
        assert!(out.find("frame") < out.find("extra"), "{out:?}");
    }

    #[test]
    fn an_empty_flush_writes_nothing() {
        let writes = Writes::default();
        let mut host = Synced::new(Pending::default(), writes.clone());
        host.flush().unwrap();
        assert!(writes.0.borrow().is_empty());
    }

    #[test]
    fn clearing_never_asks_the_host_where_the_cursor_is() {
        let viewport = ratatui::Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 4, 2));
        let mut terminal = ratatui::Terminal::with_options(
            HostBackend::new(Vec::new()),
            ratatui::TerminalOptions { viewport },
        )
        .unwrap();
        terminal.backend_mut().set_cursor_position((2, 1)).unwrap();
        let started = std::time::Instant::now();
        terminal.clear().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
        let out = String::from_utf8(terminal.backend().out.clone()).unwrap();
        assert!(!out.contains("\x1b[6n"), "{out:?}");
        assert!(out.ends_with("\x1b[2;3H"), "{out:?}");
    }

    #[test]
    fn the_cursor_is_where_the_last_cell_ended() {
        let mut backend = HostBackend::new(Vec::new());
        let (a, wide) = (Cell::new("a"), Cell::new("中"));
        backend
            .draw([(0, 1, &a), (1, 1, &wide)].into_iter())
            .unwrap();
        assert_eq!(backend.get_cursor_position().unwrap(), Position::new(3, 1));
        backend.set_cursor_position((7, 0)).unwrap();
        assert_eq!(backend.get_cursor_position().unwrap(), Position::new(7, 0));
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
