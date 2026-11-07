use std::collections::VecDeque;
use std::ops::Range;

use crate::grid::{Grid, Point, Selection, SelectionKind, TextRow};

const MAX_LINE_ROWS: u64 = 4096;
const MAX_QUERY_CHARS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Forward,
    Backward,
}

impl Direction {
    pub fn reverse(self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Match {
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchHit {
    pub current: Option<Match>,
    pub index: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub uri: String,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Copy)]
struct LineCell {
    ch: char,
    bytes: (usize, usize),
    point: Point,
    width: u8,
}

impl LineCell {
    fn end(&self) -> Point {
        Point::new(
            self.point.row,
            self.point.col + u16::from(self.width.max(1)) - 1,
        )
    }

    fn covers(&self, point: Point) -> bool {
        self.point.row == point.row && (self.point.col..=self.end().col).contains(&point.col)
    }
}

#[derive(Debug, Default)]
struct Line {
    text: String,
    cells: Vec<LineCell>,
}

impl Line {
    fn push_row(&mut self, row: u64, text: &TextRow) {
        for col in 0..text.len() {
            let Some((cell, width)) = text.cell(u16::try_from(col).unwrap_or(u16::MAX)) else {
                break;
            };
            if width == 0 {
                continue;
            }
            let start = self.text.len();
            self.text.push_str(cell);
            self.cells.push(LineCell {
                ch: cell.chars().next().unwrap_or(' '),
                bytes: (start, self.text.len()),
                point: Point::new(row, u16::try_from(col).unwrap_or(u16::MAX)),
                width,
            });
        }
    }

    fn index_of(&self, point: Point) -> Option<usize> {
        self.cells
            .iter()
            .position(|cell| cell.covers(point))
            .or_else(|| {
                self.cells
                    .iter()
                    .rposition(|cell| cell.point.row == point.row && cell.point.col <= point.col)
            })
    }

    fn slice(&self, start: usize, end: usize) -> &str {
        &self.text[self.cells[start].bytes.0..self.cells[end].bytes.1]
    }

    fn trimmed_len(&self) -> usize {
        self.cells
            .iter()
            .rposition(|cell| {
                !self.text[cell.bytes.0..cell.bytes.1]
                    .chars()
                    .all(char::is_whitespace)
            })
            .map_or(0, |last| last + 1)
    }
}

fn line_start<G: Grid + ?Sized>(grid: &G, row: u64) -> u64 {
    let first = grid.rows().start;
    let mut start = row;
    while start > first
        && row - start < MAX_LINE_ROWS
        && grid.row(start - 1).is_some_and(|prev| prev.wrapped)
    {
        start -= 1;
    }
    start
}

fn line_at<G: Grid + ?Sized>(grid: &G, row: u64) -> Option<Line> {
    let rows = grid.rows();
    if !rows.contains(&row) {
        return None;
    }
    let mut line = Line::default();
    let start = line_start(grid, row);
    let mut current = start;
    while current < rows.end {
        let text = grid.row(current)?;
        line.push_row(current, &text);
        if !text.wrapped || current - start >= 2 * MAX_LINE_ROWS {
            break;
        }
        current += 1;
    }
    Some(line)
}

struct Lines<'g, G: Grid + ?Sized> {
    grid: &'g G,
    next: u64,
    end: u64,
}

impl<G: Grid + ?Sized> Iterator for Lines<'_, G> {
    type Item = Line;

    fn next(&mut self) -> Option<Line> {
        let rows = self.grid.rows();
        if self.next >= self.end.min(rows.end) {
            return None;
        }
        let mut line = Line::default();
        let first = self.next;
        while self.next < rows.end {
            let row = self.next;
            self.next += 1;
            let Some(text) = self.grid.row(row) else {
                break;
            };
            line.push_row(row, &text);
            if !text.wrapped || self.next - first >= MAX_LINE_ROWS {
                break;
            }
        }
        Some(line)
    }
}

fn lines<G: Grid + ?Sized>(grid: &G, rows: Range<u64>) -> Lines<'_, G> {
    let all = grid.rows();
    let start = rows.start.max(all.start);
    Lines {
        grid,
        next: if start < all.end {
            line_start(grid, start)
        } else {
            start
        },
        end: rows.end,
    }
}

pub(crate) fn word_bounds<G: Grid + ?Sized>(grid: &G, point: Point) -> Option<(Point, Point)> {
    let line = line_at(grid, point.row)?;
    let at = line.index_of(point)?;
    let chars: Vec<char> = line.cells.iter().map(|cell| cell.ch).collect();
    let (start, end) = url_span(&chars, at)
        .or_else(|| quoted_path_span(&chars, at))
        .or_else(|| token_span(&chars, at))?;
    Some((line.cells[start].point, line.cells[end].end()))
}

pub(crate) fn url_at<G: Grid + ?Sized>(grid: &G, point: Point) -> Option<Link> {
    let line = line_at(grid, point.row)?;
    let at = line.index_of(point)?;
    let chars: Vec<char> = line.cells.iter().map(|cell| cell.ch).collect();
    let (start, end) = url_span(&chars, at)?;
    Some(Link {
        uri: line.slice(start, end).to_owned(),
        start: line.cells[start].point,
        end: line.cells[end].end(),
    })
}

pub(crate) fn selection_bounds<G: Grid + ?Sized>(
    grid: &G,
    selection: &Selection,
) -> (Point, Point) {
    let (start, end) = selection.ordered();
    match selection.kind {
        SelectionKind::Char => (start, end),
        SelectionKind::Word => (
            word_bounds(grid, start).map_or(start, |bounds| bounds.0),
            word_bounds(grid, end).map_or(end, |bounds| bounds.1),
        ),
        SelectionKind::Line => (
            Point::new(start.row, 0),
            Point::new(end.row, grid.cols().saturating_sub(1)),
        ),
    }
}

fn url_span(chars: &[char], at: usize) -> Option<(usize, usize)> {
    let mut start = 0;
    while start < chars.len() {
        if !starts_with(&chars[start..], "http://") && !starts_with(&chars[start..], "https://") {
            start += 1;
            continue;
        }
        let mut end = start;
        while end + 1 < chars.len() && !chars[end + 1].is_whitespace() {
            end += 1;
        }
        if (start..=end).contains(&at) {
            while end > start && trailing_url_junk(&chars[start..=end]) {
                end -= 1;
            }
            return (start..=end).contains(&at).then_some((start, end));
        }
        start = end + 1;
    }
    None
}

fn trailing_url_junk(url: &[char]) -> bool {
    let last = url[url.len() - 1];
    let body = &url[..url.len() - 1];
    let balanced = |open: char, close: char| {
        body.iter().fold(0i32, |depth, &ch| {
            depth + i32::from(ch == open) - i32::from(ch == close)
        }) > 0
    };
    match last {
        '"' | '\'' | '`' | '.' | ',' | ';' | ':' | '!' | '?' => true,
        ')' => !balanced('(', ')'),
        ']' => !balanced('[', ']'),
        '}' => !balanced('{', '}'),
        '>' => !balanced('<', '>'),
        _ => false,
    }
}

fn quoted_path_span(chars: &[char], at: usize) -> Option<(usize, usize)> {
    if matches!(chars.get(at)?, '"' | '\'' | '`') {
        return None;
    }
    for quote in ['"', '\'', '`'] {
        let mut open = None;
        for (index, &ch) in chars.iter().enumerate() {
            if ch != quote || escaped(chars, index) {
                continue;
            }
            match open.take() {
                Some(begin)
                    if begin < at && at < index && chars[begin + 1..index].contains(&'/') =>
                {
                    return Some((begin + 1, index - 1));
                }
                Some(_) => {}
                None => open = Some(index),
            }
        }
    }
    None
}

fn escaped(chars: &[char], index: usize) -> bool {
    chars[..index]
        .iter()
        .rev()
        .take_while(|&&ch| ch == '\\')
        .count()
        % 2
        == 1
}

fn token_span(chars: &[char], at: usize) -> Option<(usize, usize)> {
    if separator(*chars.get(at)?) {
        return None;
    }
    let mut start = at;
    while start > 0 && !separator(chars[start - 1]) {
        start -= 1;
    }
    let mut end = at;
    while end + 1 < chars.len() && !separator(chars[end + 1]) {
        end += 1;
    }
    while start <= end && matches!(chars[start], '(' | '[' | '{' | '<' | '"' | '\'' | '`') {
        start += 1;
    }
    let closer = |ch: char| {
        matches!(
            ch,
            ')' | ']' | '}' | '>' | '"' | '\'' | '`' | '.' | ',' | ';' | ':' | '!' | '?'
        )
    };
    if start < end && chars[end] == '$' && closer(chars[end - 1]) {
        end -= 1;
    }
    while start <= end && closer(chars[end]) {
        end = end.checked_sub(1)?;
    }
    (start <= end && (start..=end).contains(&at)).then_some((start, end))
}

fn separator(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '|' | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | ','
                | ';'
                | '!'
                | '（'
                | '）'
                | '：'
                | '、'
                | '。'
                | '，'
        )
}

fn starts_with(chars: &[char], prefix: &str) -> bool {
    let mut rest = chars.iter();
    prefix
        .chars()
        .all(|expected| rest.next() == Some(&expected))
}

struct Needle {
    chars: Vec<char>,
    fold: bool,
}

impl Needle {
    fn new(query: &str) -> Option<Self> {
        let fold = !query.chars().any(char::is_uppercase);
        let chars: Vec<char> = if fold {
            query.chars().flat_map(char::to_lowercase).collect()
        } else {
            query.chars().collect()
        };
        (!chars.is_empty() && chars.len() <= MAX_QUERY_CHARS).then_some(Self { chars, fold })
    }

    fn find(&self, line: &Line, out: &mut Vec<Match>) {
        let len = line.trimmed_len();
        let mut hay: Vec<(char, usize, bool, bool)> = Vec::new();
        for (index, cell) in line.cells[..len].iter().enumerate() {
            let text = &line.text[cell.bytes.0..cell.bytes.1];
            let start = hay.len();
            for ch in text.chars() {
                if self.fold {
                    hay.extend(ch.to_lowercase().map(|lower| (lower, index, false, false)));
                } else {
                    hay.push((ch, index, false, false));
                }
            }
            if hay.len() > start {
                hay[start].2 = true;
                let last = hay.len() - 1;
                hay[last].3 = true;
            }
        }
        let n = self.chars.len();
        let mut i = 0;
        while i + n <= hay.len() {
            let window = &hay[i..i + n];
            let hit = window[0].2
                && window[n - 1].3
                && window.iter().zip(&self.chars).all(|(h, c)| h.0 == *c);
            if hit {
                out.push(Match {
                    start: line.cells[window[0].1].point,
                    end: line.cells[window[n - 1].1].end(),
                });
                i += n;
            } else {
                i += 1;
            }
        }
    }
}

pub(crate) fn search<G: Grid + ?Sized>(
    grid: &G,
    query: &str,
    origin: Point,
    direction: Direction,
) -> SearchHit {
    let Some(needle) = Needle::new(query) else {
        return SearchHit::default();
    };
    let mut found = Vec::new();
    let mut total = 0;
    let mut target: Option<(usize, Match)> = None;
    let mut first = None;
    let mut last = None;
    for line in lines(grid, grid.rows()) {
        found.clear();
        needle.find(&line, &mut found);
        for &hit in &found {
            first.get_or_insert((total, hit));
            last = Some((total, hit));
            match direction {
                Direction::Forward if target.is_none() && hit.start > origin => {
                    target = Some((total, hit))
                }
                Direction::Backward if hit.end < origin => target = Some((total, hit)),
                _ => {}
            }
            total += 1;
        }
    }
    let wrapped = match direction {
        Direction::Forward => first,
        Direction::Backward => last,
    };
    match target.or(wrapped) {
        Some((index, hit)) => SearchHit {
            current: Some(hit),
            index,
            total,
        },
        None => SearchHit::default(),
    }
}

pub(crate) fn matches<G: Grid + ?Sized>(grid: &G, query: &str, rows: Range<u64>) -> Vec<Match> {
    let Some(needle) = Needle::new(query) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in lines(grid, rows.clone()) {
        needle.find(&line, &mut out);
    }
    out.retain(|hit| hit.end.row >= rows.start && hit.start.row < rows.end);
    out
}

pub(crate) fn first_non_blank<G: Grid + ?Sized>(grid: &G, row: u64) -> u16 {
    let Some(text) = grid.row(row) else {
        return 0;
    };
    (0..u16::try_from(text.len()).unwrap_or(u16::MAX))
        .find(|&col| {
            text.cell(col)
                .is_some_and(|(cell, width)| width > 0 && !cell.trim().is_empty())
        })
        .unwrap_or(0)
}

pub(crate) fn line_end<G: Grid + ?Sized>(grid: &G, row: u64) -> u16 {
    let Some(text) = grid.row(row) else {
        return 0;
    };
    (0..u16::try_from(text.len()).unwrap_or(u16::MAX))
        .rev()
        .find(|&col| {
            text.cell(col)
                .is_some_and(|(cell, width)| width > 0 && !cell.trim().is_empty())
        })
        .unwrap_or(0)
}

pub(crate) fn paragraph<G: Grid + ?Sized>(grid: &G, row: u64, direction: Direction) -> Option<u64> {
    let rows = grid.rows();
    (1..1000u64)
        .map_while(|distance| match direction {
            Direction::Forward => Some(row + distance).filter(|r| *r < rows.end),
            Direction::Backward => row.checked_sub(distance).filter(|r| *r >= rows.start),
        })
        .find(|&candidate| grid.row(candidate).is_some_and(|text| text.is_blank()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Blank,
    Punct,
    Word,
}

#[derive(Debug, Clone, Copy)]
struct Atom {
    point: Option<Point>,
    end: u16,
    class: Class,
}

impl Atom {
    fn class(&self, big: bool) -> Class {
        match self.class {
            Class::Punct if big => Class::Word,
            class => class,
        }
    }
}

fn class(text: &str) -> Class {
    match text.chars().next() {
        None => Class::Blank,
        Some(ch) if ch.is_whitespace() => Class::Blank,
        Some(ch) if ch.is_alphanumeric() || ch == '_' => Class::Word,
        Some(_) => Class::Punct,
    }
}

struct Tape<'g, G: Grid + ?Sized> {
    grid: &'g G,
    atoms: VecDeque<Atom>,
    offset: isize,
    first: u64,
    last: u64,
}

impl<'g, G: Grid + ?Sized> Tape<'g, G> {
    fn new(grid: &'g G, row: u64) -> Option<Self> {
        let mut tape = Self {
            grid,
            atoms: VecDeque::new(),
            offset: 0,
            first: row,
            last: row,
        };
        let atoms = tape.load(row)?;
        tape.atoms.extend(atoms);
        Some(tape)
    }

    fn load(&self, row: u64) -> Option<Vec<Atom>> {
        let text = self.grid.row(row)?;
        let mut atoms = Vec::with_capacity(text.len() + 1);
        for col in 0..u16::try_from(text.len()).unwrap_or(u16::MAX) {
            let Some((cell, width)) = text.cell(col) else {
                break;
            };
            if width == 0 {
                continue;
            }
            atoms.push(Atom {
                point: Some(Point::new(row, col)),
                end: col + u16::from(width) - 1,
                class: class(cell),
            });
        }
        if !text.wrapped {
            atoms.push(Atom {
                point: None,
                end: 0,
                class: Class::Blank,
            });
        }
        Some(atoms)
    }

    fn get(&mut self, index: isize) -> Option<Atom> {
        let rows = self.grid.rows();
        while index < self.offset {
            let row = self.first.checked_sub(1).filter(|row| *row >= rows.start)?;
            let atoms = self.load(row)?;
            self.offset -= isize::try_from(atoms.len()).ok()?;
            for atom in atoms.into_iter().rev() {
                self.atoms.push_front(atom);
            }
            self.first = row;
        }
        while index >= self.offset + isize::try_from(self.atoms.len()).ok()? {
            let row = Some(self.last + 1).filter(|row| *row < rows.end)?;
            self.atoms.extend(self.load(row)?);
            self.last = row;
        }
        self.atoms
            .get(usize::try_from(index - self.offset).ok()?)
            .copied()
    }

    fn locate(&self, point: Point) -> Option<isize> {
        let index = self
            .atoms
            .iter()
            .position(|atom| {
                atom.point
                    .is_some_and(|p| p.row == point.row && (p.col..=atom.end).contains(&point.col))
            })
            .or_else(|| {
                self.atoms.iter().rposition(|atom| {
                    atom.point
                        .is_some_and(|p| p.row == point.row && p.col <= point.col)
                })
            })?;
        Some(self.offset + isize::try_from(index).ok()?)
    }

    fn point_from(&mut self, mut index: isize, step: isize) -> Option<Point> {
        loop {
            if let Some(point) = self.get(index)?.point {
                return Some(point);
            }
            index += step;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordMotion {
    NextStart,
    PreviousStart,
    NextEnd,
}

pub(crate) fn word_motion<G: Grid + ?Sized>(
    grid: &G,
    from: Point,
    motion: WordMotion,
    big: bool,
) -> Option<Point> {
    let mut tape = Tape::new(grid, from.row)?;
    let at = tape.locate(from)?;
    let is = |tape: &mut Tape<'_, G>, index: isize, class: Class| {
        tape.get(index).is_some_and(|atom| atom.class(big) == class)
    };
    let is_blank = |tape: &mut Tape<'_, G>, index: isize| is(tape, index, Class::Blank);
    match motion {
        WordMotion::NextStart => {
            let current = tape.get(at)?.class(big);
            let mut next = at + 1;
            if current != Class::Blank {
                while is(&mut tape, next, current) {
                    next += 1;
                }
            }
            while is_blank(&mut tape, next) {
                next += 1;
            }
            tape.point_from(next, 1)
        }
        WordMotion::PreviousStart => {
            let mut previous = at - 1;
            while is_blank(&mut tape, previous) {
                previous -= 1;
            }
            let current = tape.get(previous)?.class(big);
            while is(&mut tape, previous - 1, current) {
                previous -= 1;
            }
            tape.point_from(previous, -1)
        }
        WordMotion::NextEnd => {
            let mut next = at + 1;
            while is_blank(&mut tape, next) {
                next += 1;
            }
            let current = tape.get(next)?.class(big);
            while is(&mut tape, next + 1, current) {
                next += 1;
            }
            tape.point_from(next, -1)
        }
    }
}
