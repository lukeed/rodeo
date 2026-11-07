use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::grid::{Grid, Point, Selection, SelectionKind};
use crate::text::{self, Direction, Match, WordMotion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyAction {
    None,
    Redraw,
    Prompt(Direction),
    Copy(Option<Selection>),
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopySearch {
    pub query: String,
    pub direction: Direction,
    pub current: Option<Match>,
    pub index: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Char(Point),
    Line(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyMode {
    cursor: Point,
    mark: Option<Mark>,
    search: Option<CopySearch>,
    entry_offset: u64,
}

impl CopyMode {
    pub fn enter<G: Grid + ?Sized>(grid: &G) -> Self {
        let (top, height) = grid.viewport();
        let cursor = grid
            .cursor()
            .unwrap_or_else(|| Point::new(top + u64::from(height.saturating_sub(1)), 0));
        Self {
            cursor: clamp(grid, cursor),
            mark: None,
            search: None,
            entry_offset: offset_from_bottom(grid),
        }
    }

    pub fn cursor(&self) -> Point {
        self.cursor
    }

    pub fn search(&self) -> Option<&CopySearch> {
        self.search.as_ref()
    }

    pub fn is_selecting(&self) -> bool {
        self.mark.is_some()
    }

    pub fn selection<G: Grid + ?Sized>(&self, grid: &G) -> Option<Selection> {
        match self.mark? {
            Mark::Char(anchor) => Some(Selection::new(anchor, self.cursor, SelectionKind::Char)),
            Mark::Line(row) => Some(Selection::new(
                Point::new(row, 0),
                Point::new(self.cursor.row, grid.cols().saturating_sub(1)),
                SelectionKind::Line,
            )),
        }
    }

    pub fn key<G: Grid + ?Sized>(&mut self, grid: &mut G, key: &KeyEvent) -> CopyAction {
        if key.kind == KeyEventKind::Release {
            return CopyAction::None;
        }
        match key.code {
            KeyCode::Esc => return self.escape(grid),
            KeyCode::Enter => return self.copy(grid),
            KeyCode::Left => return self.step(grid, 0, -1),
            KeyCode::Right => return self.step(grid, 0, 1),
            KeyCode::Up => return self.step(grid, -1, 0),
            KeyCode::Down => return self.step(grid, 1, 0),
            KeyCode::PageUp => return self.page(grid, Direction::Backward, false),
            KeyCode::PageDown => return self.page(grid, Direction::Forward, false),
            KeyCode::Home => return self.line_start(grid),
            KeyCode::End => return self.line_end(grid),
            _ => {}
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('b') => self.page(grid, Direction::Backward, false),
                KeyCode::Char('f') => self.page(grid, Direction::Forward, false),
                KeyCode::Char('u') => self.page(grid, Direction::Backward, true),
                KeyCode::Char('d') => self.page(grid, Direction::Forward, true),
                _ => CopyAction::None,
            };
        }
        let Some(command) = command(key) else {
            return CopyAction::None;
        };
        match command {
            'q' => self.exit(grid),
            'y' => self.copy(grid),
            'v' | ' ' => self.mark(Mark::Char(self.cursor)),
            'V' => self.mark(Mark::Line(self.cursor.row)),
            'h' => self.step(grid, 0, -1),
            'j' => self.step(grid, 1, 0),
            'k' => self.step(grid, -1, 0),
            'l' => self.step(grid, 0, 1),
            'g' => self.top(grid),
            'G' => self.bottom(grid),
            '0' => self.line_start(grid),
            '^' => self.first_non_blank(grid),
            '$' => self.line_end(grid),
            'w' => self.word(grid, WordMotion::NextStart, false),
            'b' => self.word(grid, WordMotion::PreviousStart, false),
            'e' => self.word(grid, WordMotion::NextEnd, false),
            'W' => self.word(grid, WordMotion::NextStart, true),
            'B' => self.word(grid, WordMotion::PreviousStart, true),
            'E' => self.word(grid, WordMotion::NextEnd, true),
            '{' => self.paragraph(grid, Direction::Backward),
            '}' => self.paragraph(grid, Direction::Forward),
            '/' => CopyAction::Prompt(Direction::Forward),
            '?' => CopyAction::Prompt(Direction::Backward),
            'n' => self.repeat(grid, false),
            'N' => self.repeat(grid, true),
            _ => CopyAction::None,
        }
    }

    pub fn find<G: Grid + ?Sized>(
        &mut self,
        grid: &mut G,
        query: &str,
        direction: Direction,
    ) -> CopyAction {
        if query.is_empty() {
            return CopyAction::None;
        }
        self.run_search(grid, query.to_owned(), direction, direction)
    }

    pub fn exit<G: Grid + ?Sized>(&self, grid: &mut G) -> CopyAction {
        let (_, height) = grid.viewport();
        let top = grid
            .rows()
            .end
            .saturating_sub(u64::from(height))
            .saturating_sub(self.entry_offset);
        grid.scroll_to(top);
        CopyAction::Exit
    }

    fn escape<G: Grid + ?Sized>(&mut self, grid: &mut G) -> CopyAction {
        if self.mark.is_none() && self.search.is_none() {
            return self.exit(grid);
        }
        self.mark = None;
        self.search = None;
        CopyAction::Redraw
    }

    fn copy<G: Grid + ?Sized>(&self, grid: &mut G) -> CopyAction {
        let selection = self.selection(grid).or_else(|| {
            let hit = self.search.as_ref()?.current?;
            Some(Selection::new(hit.start, hit.end, SelectionKind::Char))
        });
        self.exit(grid);
        CopyAction::Copy(selection)
    }

    fn mark(&mut self, mark: Mark) -> CopyAction {
        self.mark = Some(mark);
        CopyAction::Redraw
    }

    fn step<G: Grid + ?Sized>(&mut self, grid: &mut G, rows: i64, cols: i32) -> CopyAction {
        let row = self.cursor.row.saturating_add_signed(rows);
        let col = u16::try_from((i32::from(self.cursor.col) + cols).max(0)).unwrap_or(u16::MAX);
        self.place(grid, Point::new(row, col), false)
    }

    fn place<G: Grid + ?Sized>(&mut self, grid: &mut G, point: Point, reserve: bool) -> CopyAction {
        self.cursor = clamp(grid, point);
        reveal(grid, self.cursor.row, reserve);
        CopyAction::Redraw
    }

    fn page<G: Grid + ?Sized>(
        &mut self,
        grid: &mut G,
        direction: Direction,
        half: bool,
    ) -> CopyAction {
        let (top, height) = grid.viewport();
        let lines = page_lines(height, half);
        let (row, top) = match direction {
            Direction::Backward => (
                self.cursor.row.saturating_sub(lines),
                top.saturating_sub(lines).max(grid.rows().start),
            ),
            Direction::Forward => (self.cursor.row + lines, top + lines),
        };
        grid.scroll_to(top);
        self.place(grid, Point::new(row, self.cursor.col), false)
    }

    fn top<G: Grid + ?Sized>(&mut self, grid: &mut G) -> CopyAction {
        let first = grid.rows().start;
        grid.scroll_to(first);
        self.place(grid, Point::new(first, self.cursor.col), false)
    }

    fn bottom<G: Grid + ?Sized>(&mut self, grid: &mut G) -> CopyAction {
        let last = grid.rows().end.saturating_sub(1);
        grid.scroll_to(last);
        self.place(grid, Point::new(last, self.cursor.col), false)
    }

    fn line_start<G: Grid + ?Sized>(&mut self, grid: &mut G) -> CopyAction {
        self.place(grid, Point::new(self.cursor.row, 0), false)
    }

    fn first_non_blank<G: Grid + ?Sized>(&mut self, grid: &mut G) -> CopyAction {
        let col = text::first_non_blank(grid, self.cursor.row);
        self.place(grid, Point::new(self.cursor.row, col), false)
    }

    fn line_end<G: Grid + ?Sized>(&mut self, grid: &mut G) -> CopyAction {
        let col = text::line_end(grid, self.cursor.row);
        self.place(grid, Point::new(self.cursor.row, col), false)
    }

    fn word<G: Grid + ?Sized>(
        &mut self,
        grid: &mut G,
        motion: WordMotion,
        big: bool,
    ) -> CopyAction {
        let target = text::word_motion(grid, self.cursor, motion, big).unwrap_or(self.cursor);
        self.place(grid, target, false)
    }

    fn paragraph<G: Grid + ?Sized>(&mut self, grid: &mut G, direction: Direction) -> CopyAction {
        let row = text::paragraph(grid, self.cursor.row, direction).unwrap_or(self.cursor.row);
        self.place(grid, Point::new(row, self.cursor.col), false)
    }

    fn repeat<G: Grid + ?Sized>(&mut self, grid: &mut G, reverse: bool) -> CopyAction {
        let Some(search) = self.search.as_ref() else {
            return CopyAction::None;
        };
        let (query, direction) = (search.query.clone(), search.direction);
        let toward = if reverse {
            direction.reverse()
        } else {
            direction
        };
        self.run_search(grid, query, direction, toward)
    }

    fn run_search<G: Grid + ?Sized>(
        &mut self,
        grid: &mut G,
        query: String,
        direction: Direction,
        toward: Direction,
    ) -> CopyAction {
        let hit = text::search(grid, &query, self.cursor, toward);
        self.search = Some(CopySearch {
            query,
            direction,
            current: hit.current,
            index: hit.index,
            total: hit.total,
        });
        if let Some(found) = hit.current {
            self.place(grid, found.start, true);
        }
        CopyAction::Redraw
    }
}

pub fn page_lines(height: u16, half: bool) -> u64 {
    match height {
        0..=2 => 1,
        _ if half => u64::from(height / 2),
        _ => u64::from(height - 2),
    }
}

fn command(key: &KeyEvent) -> Option<char> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    let KeyCode::Char(ch) = key.code else {
        return None;
    };
    if !key.modifiers.contains(KeyModifiers::SHIFT) {
        return Some(ch);
    }
    const SHIFTED: [(char, char); 21] = [
        ('1', '!'),
        ('2', '@'),
        ('3', '#'),
        ('4', '$'),
        ('5', '%'),
        ('6', '^'),
        ('7', '&'),
        ('8', '*'),
        ('9', '('),
        ('0', ')'),
        ('-', '_'),
        ('=', '+'),
        ('[', '{'),
        (']', '}'),
        ('\\', '|'),
        (';', ':'),
        ('\'', '"'),
        (',', '<'),
        ('.', '>'),
        ('/', '?'),
        ('`', '~'),
    ];
    Some(
        SHIFTED
            .iter()
            .find(|(base, _)| *base == ch)
            .map_or_else(|| ch.to_ascii_uppercase(), |(_, shifted)| *shifted),
    )
}

fn clamp<G: Grid + ?Sized>(grid: &G, point: Point) -> Point {
    let rows = grid.rows();
    Point::new(
        point
            .row
            .clamp(rows.start, rows.end.saturating_sub(1).max(rows.start)),
        point.col.min(grid.cols().saturating_sub(1)),
    )
}

fn offset_from_bottom<G: Grid + ?Sized>(grid: &G) -> u64 {
    let (top, height) = grid.viewport();
    grid.rows()
        .end
        .saturating_sub(u64::from(height))
        .saturating_sub(top)
}

fn reveal<G: Grid + ?Sized>(grid: &mut G, row: u64, reserve: bool) {
    let (top, height) = grid.viewport();
    let span = u64::from(height.saturating_sub(if reserve { 2 } else { 1 }));
    let target = if row < top {
        row
    } else if row > top + span {
        row - span
    } else {
        return;
    };
    grid.scroll_to(target);
}
