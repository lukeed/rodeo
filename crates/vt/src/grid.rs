use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Point {
    pub row: u64,
    pub col: u16,
}

impl Point {
    pub const fn new(row: u64, col: u16) -> Self {
        Self { row, col }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextRow {
    text: String,
    cells: Vec<(u32, u8)>,
    pub wrapped: bool,
}

impl TextRow {
    pub fn new(wrapped: bool) -> Self {
        Self {
            wrapped,
            ..Self::default()
        }
    }

    pub fn push(&mut self, text: &str, width: u8) {
        self.text.push_str(text);
        let end = u32::try_from(self.text.len()).unwrap_or(u32::MAX);
        self.cells.push((end, width));
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn cell(&self, col: u16) -> Option<(&str, u8)> {
        let col = usize::from(col);
        let &(end, width) = self.cells.get(col)?;
        let start = col.checked_sub(1).map_or(0, |prev| self.cells[prev].0);
        Some((&self.text[start as usize..end as usize], width))
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn trimmed(&self) -> &str {
        self.text.trim_end()
    }

    pub fn is_blank(&self) -> bool {
        self.text.chars().all(char::is_whitespace)
    }
}

pub trait Grid {
    fn cols(&self) -> u16;
    fn rows(&self) -> Range<u64>;
    fn row(&self, row: u64) -> Option<TextRow>;
    fn viewport(&self) -> (u64, u16);
    fn scroll_to(&mut self, top: u64);
    fn cursor(&self) -> Option<Point>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SelectionKind {
    #[default]
    Char,
    Word,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: Point,
    pub head: Point,
    pub kind: SelectionKind,
}

impl Selection {
    pub const fn new(anchor: Point, head: Point, kind: SelectionKind) -> Self {
        Self { anchor, head, kind }
    }

    pub fn ordered(&self) -> (Point, Point) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}
