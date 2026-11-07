#![allow(dead_code)]

use vt::{Options, Terminal};

pub fn term(cols: u16, rows: u16) -> Terminal {
    let mut term = Terminal::new(cols, rows, Options::default()).expect("terminal");
    term.resize(cols, rows, (10, 20)).expect("resize");
    term
}

pub fn lines(term: &mut Terminal, count: usize) {
    for i in 0..count {
        term.feed(format!("line {i}\r\n").as_bytes());
    }
}

pub fn screen(term: &Terminal) -> Vec<String> {
    let top = term.viewport_top();
    (0..u64::from(term.size().1))
        .map(|row| term.row_text(top + row).unwrap_or_default())
        .collect()
}
