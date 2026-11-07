use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

use libghostty_vt::fmt::Format;
use libghostty_vt::screen::{CellWide, Screen};
use libghostty_vt::selection::FormatOptions;
use libghostty_vt::terminal::{
    Mode, Point as VtPoint, PointCoordinate, ScrollViewport, SizeReportSize,
};
use libghostty_vt::{TerminalOptions, paste};

use crate::grid::{Grid, Point, Selection, TextRow};
use crate::input::Input;
use crate::kitty::Images;
use crate::render::{CursorShape, Renderer};
use crate::rows::Rows;
use crate::stream::{Found, Op, Scanner};
use crate::text;

pub(crate) type Vt = libghostty_vt::Terminal<'static, 'static>;

const KITTY_PLACEHOLDER: char = '\u{10EEEE}';

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("libghostty: {0}")]
    Vt(#[from] libghostty_vt::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Bell,
    Title(String),
    Cwd(String),
    Clipboard(String),
    Notify { title: String, body: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub scrollback_bytes: usize,
    pub image_bytes: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scrollback_bytes: 10_000_000,
            image_bytes: 320_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMode {
    None,
    X10,
    Normal,
    Button,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseFormat {
    X10,
    Utf8,
    Sgr,
    Urxvt,
    SgrPixels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorState {
    pub col: u16,
    pub row: u16,
    pub visible: bool,
    pub shape: CursorShape,
    pub blinking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Scrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

impl Scrollbar {
    pub fn max_offset(&self) -> u64 {
        self.total.saturating_sub(self.len)
    }

    pub fn offset_from_bottom(&self) -> u64 {
        self.max_offset().saturating_sub(self.offset)
    }
}

#[derive(Default)]
struct Effects {
    replies: Vec<u8>,
    events: Vec<Event>,
}

pub struct Terminal {
    vt: Vt,
    effects: Rc<RefCell<Effects>>,
    cell_px: Rc<Cell<(u32, u32)>>,
    stream: Scanner,
    rows: Rows,
    input: Input,
    renderer: RefCell<Renderer>,
    images: RefCell<Images>,
}

impl Terminal {
    pub fn new(cols: u16, rows: u16, options: Options) -> Result<Self, Error> {
        let mut vt = Vt::new(TerminalOptions {
            cols: cols.max(1),
            rows: rows.max(1),
            max_scrollback: options.scrollback_bytes,
        })?;
        vt.set_mode(Mode::GRAPHEME_CLUSTER, true)?;
        crate::png::install()?;
        vt.set_kitty_image_storage_limit(options.image_bytes)?
            .set_kitty_image_from_file_allowed(false)?
            .set_kitty_image_from_temp_file_allowed(false)?
            .set_kitty_image_from_shared_mem_allowed(false)?;
        let effects = Rc::new(RefCell::new(Effects::default()));
        let cell_px = Rc::new(Cell::new((0, 0)));
        install_effects(&mut vt, &effects, &cell_px)?;
        let mut term = Self {
            vt,
            effects,
            cell_px,
            stream: Scanner::new()?,
            rows: Rows::default(),
            input: Input::new()?,
            renderer: RefCell::new(Renderer::new()?),
            images: RefCell::new(Images::new()?),
        };
        term.rows.sync(&mut term.vt);
        Ok(term)
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for op in self.stream.scan(bytes) {
            match op {
                Op::Pass(range) => self.vt.vt_write(&bytes[range]),
                Op::Held(held) => self.vt.vt_write(&held),
                Op::Found(Found::Notify { title, body }) => {
                    self.effects
                        .borrow_mut()
                        .events
                        .push(Event::Notify { title, body });
                }
                Op::Found(Found::Reply(reply)) => {
                    self.effects.borrow_mut().replies.extend_from_slice(&reply);
                }
                Op::Reset => {
                    let _ = self.vt.set_mode(Mode::GRAPHEME_CLUSTER, true);
                }
            }
        }
        self.rows.sync(&mut self.vt);
    }

    pub fn resize(&mut self, cols: u16, rows: u16, cell_px: (u32, u32)) -> Result<(), Error> {
        self.cell_px.set(cell_px);
        self.vt
            .resize(cols.max(1), rows.max(1), cell_px.0, cell_px.1)?;
        self.rows.sync(&mut self.vt);
        Ok(())
    }

    pub fn reset(&mut self) {
        if self.stream.cancel() {
            self.vt.vt_write(b"\x18");
        }
        self.vt.reset();
        let _ = self.vt.set_mode(Mode::GRAPHEME_CLUSTER, true);
        self.rows.sync(&mut self.vt);
    }

    pub fn clear(&mut self) {
        if self.alternate_screen() {
            return;
        }
        let row = self.vt.cursor_y().unwrap_or(0);
        let col = self.vt.cursor_x().unwrap_or(0) + 1;
        let scroll = if row > 0 {
            format!("\x1b[{row}S")
        } else {
            String::new()
        };
        let abort = if self.stream.cancel() { "\x18" } else { "" };
        self.vt
            .vt_write(format!("{abort}{scroll}\x1b[3J\x1b[1;{col}H").as_bytes());
        self.vt.scroll_viewport(ScrollViewport::Bottom);
        self.rows.sync(&mut self.vt);
    }

    pub fn size(&self) -> (u16, u16) {
        (
            self.vt.cols().unwrap_or(1).max(1),
            self.vt.rows().unwrap_or(1).max(1),
        )
    }

    pub fn cell_px(&self) -> (u32, u32) {
        self.cell_px.get()
    }

    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.effects.borrow_mut().replies)
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.effects.borrow_mut().events)
    }

    pub fn title(&self) -> String {
        self.vt.title().unwrap_or_default().to_owned()
    }

    pub fn cwd(&self) -> Option<String> {
        self.vt
            .pwd()
            .ok()
            .filter(|pwd| !pwd.is_empty())
            .map(str::to_owned)
    }

    pub(crate) fn vt(&self) -> &Vt {
        &self.vt
    }

    pub(crate) fn input(&mut self) -> (&Vt, &mut Input) {
        (&self.vt, &mut self.input)
    }

    pub(crate) fn renderer(&self) -> &RefCell<Renderer> {
        &self.renderer
    }

    pub(crate) fn images(&self) -> &RefCell<Images> {
        &self.images
    }

    pub(crate) fn mode(&self, mode: Mode) -> bool {
        self.vt.mode(mode).unwrap_or(false)
    }

    pub fn alternate_screen(&self) -> bool {
        self.vt.active_screen().ok() == Some(Screen::Alternate)
    }

    pub fn mouse_mode(&self) -> MouseMode {
        [
            (Mode::ANY_MOUSE, MouseMode::Any),
            (Mode::BUTTON_MOUSE, MouseMode::Button),
            (Mode::NORMAL_MOUSE, MouseMode::Normal),
            (Mode::X10_MOUSE, MouseMode::X10),
        ]
        .into_iter()
        .find(|(mode, _)| self.mode(*mode))
        .map_or(MouseMode::None, |(_, kind)| kind)
    }

    pub fn mouse_format(&self) -> MouseFormat {
        [
            (Mode::SGR_PIXELS_MOUSE, MouseFormat::SgrPixels),
            (Mode::SGR_MOUSE, MouseFormat::Sgr),
            (Mode::URXVT_MOUSE, MouseFormat::Urxvt),
            (Mode::UTF8_MOUSE, MouseFormat::Utf8),
        ]
        .into_iter()
        .find(|(mode, _)| self.mode(*mode))
        .map_or(MouseFormat::X10, |(_, kind)| kind)
    }

    pub fn wants_mouse(&self) -> bool {
        self.vt.is_mouse_tracking().unwrap_or(false)
    }

    pub fn alternate_scroll(&self) -> bool {
        self.mode(Mode::ALT_SCROLL)
    }

    pub fn bracketed_paste(&self) -> bool {
        self.mode(Mode::BRACKETED_PASTE)
    }

    pub fn focus_reporting(&self) -> bool {
        self.mode(Mode::FOCUS_EVENT)
    }

    pub fn application_cursor(&self) -> bool {
        self.mode(Mode::DECCKM)
    }

    pub fn application_keypad(&self) -> bool {
        self.mode(Mode::KEYPAD_KEYS)
    }

    pub fn synchronized_output(&self) -> bool {
        self.mode(Mode::SYNC_OUTPUT)
    }

    pub fn cursor(&self) -> CursorState {
        let (shape, blinking) = self.renderer.borrow_mut().cursor_style(&self.vt);
        CursorState {
            col: self.vt.cursor_x().unwrap_or(0),
            row: self.vt.cursor_y().unwrap_or(0),
            visible: self.vt.is_cursor_visible().unwrap_or(true),
            shape,
            blinking,
        }
    }

    pub fn paste(&self, text: &str) -> Vec<u8> {
        let bracketed = self.bracketed_paste();
        let mut data = text.as_bytes().to_vec();
        let mut out = vec![0; data.len() + 16];
        let written = match paste::encode(&mut data, bracketed, &mut out) {
            Err(libghostty_vt::Error::OutOfSpace { required }) => {
                out.resize(required, 0);
                paste::encode(&mut data, bracketed, &mut out)
            }
            result => result,
        };
        out.truncate(written.unwrap_or(0));
        out
    }

    pub fn focus(&self, focused: bool) -> Option<Vec<u8>> {
        if !self.focus_reporting() {
            return None;
        }
        Some(if focused { b"\x1b[I" } else { b"\x1b[O" }.to_vec())
    }

    pub fn scrollbar(&self) -> Scrollbar {
        self.vt
            .scrollbar()
            .map(|bar| Scrollbar {
                total: bar.total,
                offset: bar.offset,
                len: bar.len,
            })
            .unwrap_or_default()
    }

    pub fn is_scrolled_back(&self) -> bool {
        let bar = self.scrollbar();
        bar.offset + bar.len < bar.total
    }

    pub fn scroll(&mut self, lines: isize) {
        self.vt.scroll_viewport(ScrollViewport::Delta(lines));
    }

    pub fn scroll_page(&mut self, pages: isize) {
        let rows = isize::try_from(self.size().1).unwrap_or(isize::MAX);
        self.scroll(pages.saturating_mul(rows));
    }

    pub fn scroll_to_top(&mut self) {
        self.vt.scroll_viewport(ScrollViewport::Top);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.vt.scroll_viewport(ScrollViewport::Bottom);
    }

    pub fn scroll_to_row(&mut self, top: u64) {
        let bar = self.scrollbar();
        let offset = top.saturating_sub(self.rows.base()).min(bar.max_offset());
        if offset >= bar.max_offset() {
            self.scroll_to_bottom();
        } else {
            self.vt
                .scroll_viewport(ScrollViewport::Row(usize::try_from(offset).unwrap_or(0)));
        }
    }

    pub fn viewport_top(&self) -> u64 {
        self.rows.base() + self.scrollbar().offset
    }

    pub fn first_row(&self) -> u64 {
        self.rows.base()
    }

    pub fn end_row(&self) -> u64 {
        self.rows.base() + self.vt.total_rows().map_or(0, |rows| rows as u64)
    }

    pub fn viewport_point(&self, col: u16, row: u16) -> Point {
        Point::new(self.viewport_top() + u64::from(row), col)
    }

    pub fn to_viewport(&self, point: Point) -> Option<(u16, u16)> {
        let row = point.row.checked_sub(self.viewport_top())?;
        let row = u16::try_from(row).ok()?;
        let (cols, rows) = self.size();
        (row < rows && point.col < cols).then_some((point.col, row))
    }

    pub(crate) fn screen_point(&self, point: Point) -> Option<VtPoint> {
        let y = point.row.checked_sub(self.rows.base())?;
        (point.row < self.end_row()).then_some(())?;
        Some(VtPoint::Screen(PointCoordinate {
            x: point.col.min(self.size().0 - 1),
            y: u32::try_from(y).ok()?,
        }))
    }

    pub fn row_text(&self, row: u64) -> Option<String> {
        self.row(row).map(|row| row.trimmed().to_owned())
    }

    pub fn cell(&self, point: Point) -> Option<(String, u8)> {
        if point.col >= self.size().0 {
            return None;
        }
        let VtPoint::Screen(at) = self.screen_point(point)? else {
            return None;
        };
        let mut text = String::new();
        let width = self.read_cell(at, &mut text)?;
        Some((text, width))
    }

    fn read_cell(&self, at: PointCoordinate, text: &mut String) -> Option<u8> {
        let grid_ref = self.vt.grid_ref(VtPoint::Screen(at)).ok()?;
        let width = match grid_ref
            .cell()
            .and_then(libghostty_vt::screen::Cell::wide)
            .ok()?
        {
            CellWide::Narrow => 1,
            CellWide::Wide => 2,
            CellWide::SpacerTail | CellWide::SpacerHead => 0,
        };
        text.clear();
        let mut chars = ['\0'; 16];
        match grid_ref.graphemes(&mut chars) {
            Ok(len) => text.extend(&chars[..len]),
            Err(libghostty_vt::Error::OutOfSpace { required }) => {
                let mut chars = vec!['\0'; required];
                let len = grid_ref.graphemes(&mut chars).unwrap_or(0);
                text.extend(&chars[..len]);
            }
            Err(_) => {}
        }
        if width > 0 && (text.is_empty() || text.starts_with(KITTY_PLACEHOLDER)) {
            text.clear();
            text.push(' ');
        }
        Some(width)
    }

    pub fn selection_bounds(&self, selection: &Selection) -> (Point, Point) {
        text::selection_bounds(self, selection)
    }

    pub fn selection_text(&self, selection: &Selection) -> String {
        let (start, end) = self.selection_bounds(selection);
        let (cols, last) = (self.size().0, self.end_row().saturating_sub(1));
        let start = start.max(Point::new(self.first_row(), 0));
        let end = end.min(Point::new(last, cols - 1));
        if start > end {
            return String::new();
        }
        let (Some(a), Some(b)) = (self.screen_point(start), self.screen_point(end)) else {
            return String::new();
        };
        let (Ok(a), Ok(b)) = (self.vt.grid_ref(a), self.vt.grid_ref(b)) else {
            return String::new();
        };
        let selection = libghostty_vt::selection::Selection::new(a, b, false);
        let options = FormatOptions::new()
            .with_emit_format(Format::Plain)
            .with_unwrap(true)
            .with_trim(true)
            .with_selection(&selection);
        match self.vt.format_selection_alloc(None, options) {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).into_owned(),
            _ => String::new(),
        }
    }

    pub fn word_bounds(&self, point: Point) -> Option<(Point, Point)> {
        text::word_bounds(self, point)
    }

    pub fn url_at(&self, point: Point) -> Option<text::Link> {
        text::url_at(self, point)
    }

    pub fn hyperlink_at(&self, point: Point) -> Option<text::Link> {
        let uri = self.hyperlink(point)?;
        let cols = self.size().0;
        let (first, end) = (self.first_row(), self.end_row());
        let neighbor = |p: Point, forward: bool| -> Option<Point> {
            if forward {
                if p.col + 1 < cols {
                    Some(Point::new(p.row, p.col + 1))
                } else {
                    (p.row + 1 < end).then(|| Point::new(p.row + 1, 0))
                }
            } else if p.col > 0 {
                Some(Point::new(p.row, p.col - 1))
            } else {
                (p.row > first).then(|| Point::new(p.row - 1, cols - 1))
            }
        };
        let mut limit = 8192;
        let mut start = point;
        while let Some(prev) = neighbor(start, false) {
            if limit == 0 || self.hyperlink(prev).as_deref() != Some(uri.as_str()) {
                break;
            }
            limit -= 1;
            start = prev;
        }
        let mut last = point;
        while let Some(next) = neighbor(last, true) {
            if limit == 0 || self.hyperlink(next).as_deref() != Some(uri.as_str()) {
                break;
            }
            limit -= 1;
            last = next;
        }
        Some(text::Link {
            uri,
            start,
            end: last,
        })
    }

    fn hyperlink(&self, point: Point) -> Option<String> {
        let grid_ref = self.vt.grid_ref(self.screen_point(point)?).ok()?;
        let wide = grid_ref
            .cell()
            .and_then(libghostty_vt::screen::Cell::wide)
            .ok()?;
        if wide == CellWide::SpacerTail && point.col > 0 {
            return self.hyperlink(Point::new(point.row, point.col - 1));
        }
        let mut buf = vec![0; 256];
        let len = match grid_ref.hyperlink_uri(&mut buf) {
            Err(libghostty_vt::Error::OutOfSpace { required }) => {
                buf.resize(required, 0);
                grid_ref.hyperlink_uri(&mut buf).ok()?
            }
            result => result.ok()?,
        };
        (len > 0).then(|| String::from_utf8_lossy(&buf[..len]).into_owned())
    }

    pub fn search(
        &self,
        query: &str,
        origin: Point,
        direction: text::Direction,
    ) -> text::SearchHit {
        text::search(self, query, origin, direction)
    }

    pub fn matches(&self, query: &str, rows: Range<u64>) -> Vec<text::Match> {
        text::matches(self, query, rows)
    }

    pub fn visible_matches(&self, query: &str) -> Vec<text::Match> {
        let top = self.viewport_top();
        self.matches(query, top..top + u64::from(self.size().1))
    }
}

impl Grid for Terminal {
    fn cols(&self) -> u16 {
        self.size().0
    }

    fn rows(&self) -> Range<u64> {
        self.first_row()..self.end_row()
    }

    fn row(&self, row: u64) -> Option<TextRow> {
        let start = self.screen_point(Point::new(row, 0))?;
        let VtPoint::Screen(start) = start else {
            return None;
        };
        let wrapped = self
            .vt
            .grid_ref(VtPoint::Screen(start))
            .and_then(|grid_ref| grid_ref.row())
            .and_then(libghostty_vt::screen::Row::is_wrapped)
            .ok()?;
        let mut out = TextRow::new(wrapped);
        let mut text = String::new();
        for x in 0..self.size().0 {
            let width = self.read_cell(PointCoordinate { x, y: start.y }, &mut text)?;
            out.push(&text, width);
        }
        Some(out)
    }

    fn viewport(&self) -> (u64, u16) {
        (self.viewport_top(), self.size().1)
    }

    fn scroll_to(&mut self, top: u64) {
        self.scroll_to_row(top);
    }

    fn cursor(&self) -> Option<Point> {
        if !self.vt.is_cursor_visible().unwrap_or(false) {
            return None;
        }
        let rows = u64::from(self.size().1);
        let active_top = self.end_row().saturating_sub(rows);
        let point = Point::new(
            active_top + u64::from(self.vt.cursor_y().ok()?),
            self.vt.cursor_x().ok()?,
        );
        self.to_viewport(point).map(|_| point)
    }
}

fn install_effects(
    vt: &mut Vt,
    effects: &Rc<RefCell<Effects>>,
    cell_px: &Rc<Cell<(u32, u32)>>,
) -> Result<(), Error> {
    let replies = Rc::clone(effects);
    let bell = Rc::clone(effects);
    let title = Rc::clone(effects);
    let pwd = Rc::clone(effects);
    let clipboard = Rc::clone(effects);
    let px = Rc::clone(cell_px);
    vt.on_pty_write(move |_, data| replies.borrow_mut().replies.extend_from_slice(data))?
        .on_bell(move |_| bell.borrow_mut().events.push(Event::Bell))?
        .on_title_changed(move |term| {
            let text = term.title().unwrap_or_default().to_owned();
            title.borrow_mut().events.push(Event::Title(text));
        })?
        .on_pwd_changed(move |term| {
            let text = term.pwd().unwrap_or_default().to_owned();
            pwd.borrow_mut().events.push(Event::Cwd(text));
        })?
        .on_size(move |term| {
            let (cell_width, cell_height) = px.get();
            Some(SizeReportSize {
                rows: term.rows().ok()?,
                columns: term.cols().ok()?,
                cell_width,
                cell_height,
            })
        })?
        .on_clipboard_write(move |_, write| {
            let text = write
                .contents()
                .find(|content| content.mime.starts_with("text/"))
                .map(|content| content.data.to_owned())
                .unwrap_or_default();
            clipboard.borrow_mut().events.push(Event::Clipboard(text));
            Ok(())
        })?;
    Ok(())
}
