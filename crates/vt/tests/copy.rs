mod common;

use common::term;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use vt::{CopyAction, CopyMode, Direction, Point, Selection, SelectionKind, Terminal};

fn press(copy: &mut CopyMode, t: &mut Terminal, code: KeyCode) -> CopyAction {
    copy.key(t, &KeyEvent::new(code, KeyModifiers::NONE))
}

fn ch(copy: &mut CopyMode, t: &mut Terminal, c: char) -> CopyAction {
    press(copy, t, KeyCode::Char(c))
}

fn ctrl(copy: &mut CopyMode, t: &mut Terminal, c: char) -> CopyAction {
    copy.key(t, &KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

fn sample() -> Terminal {
    let mut t = term(20, 4);
    t.feed(b"alpha beta.gamma\r\n  indented line\r\n\r\npara two\r\nx-y z\r\n");
    for i in 0..10 {
        t.feed(format!("l {i}\r\n").as_bytes());
    }
    t.feed(b"$ ");
    t
}

fn row_of(t: &Terminal, text: &str) -> u64 {
    (t.first_row()..t.end_row())
        .find(|&row| t.row_text(row).as_deref() == Some(text))
        .unwrap()
}

fn goto(copy: &mut CopyMode, t: &mut Terminal, row: u64) {
    ch(copy, t, 'g');
    for _ in t.first_row()..row {
        ch(copy, t, 'j');
    }
    ch(copy, t, '0');
}

#[test]
fn enters_at_the_terminal_cursor() {
    let mut t = sample();
    let copy = CopyMode::enter(&t);
    assert_eq!(copy.cursor(), Point::new(row_of(&t, "$"), 2));
    t.scroll(-5);
    let copy = CopyMode::enter(&t);
    assert_eq!(copy.cursor(), Point::new(t.viewport_top() + 3, 0));
}

#[test]
fn hjkl_and_arrows_move_and_clamp() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let start = copy.cursor();
    ch(&mut copy, &mut t, 'h');
    assert_eq!(copy.cursor(), Point::new(start.row, 1));
    press(&mut copy, &mut t, KeyCode::Left);
    press(&mut copy, &mut t, KeyCode::Left);
    assert_eq!(copy.cursor().col, 0);
    ch(&mut copy, &mut t, 'l');
    press(&mut copy, &mut t, KeyCode::Right);
    assert_eq!(copy.cursor().col, 2);
    for _ in 0..30 {
        ch(&mut copy, &mut t, 'l');
    }
    assert_eq!(copy.cursor().col, 19);
    ch(&mut copy, &mut t, 'k');
    press(&mut copy, &mut t, KeyCode::Up);
    assert_eq!(copy.cursor().row, start.row - 2);
    ch(&mut copy, &mut t, 'j');
    press(&mut copy, &mut t, KeyCode::Down);
    press(&mut copy, &mut t, KeyCode::Down);
    assert_eq!(copy.cursor().row, start.row);
    assert_eq!(copy.cursor().row, t.end_row() - 1);
}

#[test]
fn moving_up_scrolls_the_view() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let top = t.viewport_top();
    for _ in 0..5 {
        ch(&mut copy, &mut t, 'k');
    }
    assert_eq!(t.viewport_top(), copy.cursor().row);
    assert!(t.viewport_top() < top);
}

#[test]
fn line_motions() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let row = row_of(&t, "  indented line");
    goto(&mut copy, &mut t, row);
    ch(&mut copy, &mut t, '$');
    assert_eq!(copy.cursor(), Point::new(row, 14));
    ch(&mut copy, &mut t, '0');
    assert_eq!(copy.cursor(), Point::new(row, 0));
    ch(&mut copy, &mut t, '^');
    assert_eq!(copy.cursor(), Point::new(row, 2));
    press(&mut copy, &mut t, KeyCode::End);
    assert_eq!(copy.cursor(), Point::new(row, 14));
    press(&mut copy, &mut t, KeyCode::Home);
    assert_eq!(copy.cursor(), Point::new(row, 0));
    copy.key(
        &mut t,
        &KeyEvent::new(KeyCode::Char('4'), KeyModifiers::SHIFT),
    );
    assert_eq!(copy.cursor(), Point::new(row, 14));
}

#[test]
fn word_motions() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let row = row_of(&t, "alpha beta.gamma");
    goto(&mut copy, &mut t, row);
    let cols = |copy: &CopyMode| (copy.cursor().row - row, copy.cursor().col);
    ch(&mut copy, &mut t, 'w');
    assert_eq!(cols(&copy), (0, 6));
    ch(&mut copy, &mut t, 'w');
    assert_eq!(cols(&copy), (0, 10));
    ch(&mut copy, &mut t, 'w');
    assert_eq!(cols(&copy), (0, 11));
    ch(&mut copy, &mut t, 'w');
    assert_eq!(cols(&copy), (1, 2));
    ch(&mut copy, &mut t, 'b');
    assert_eq!(cols(&copy), (0, 11));
    ch(&mut copy, &mut t, 'b');
    assert_eq!(cols(&copy), (0, 10));
    ch(&mut copy, &mut t, 'e');
    assert_eq!(cols(&copy), (0, 15));
    ch(&mut copy, &mut t, 'e');
    assert_eq!(cols(&copy), (1, 9));
    ch(&mut copy, &mut t, 'g');
    ch(&mut copy, &mut t, '0');
    ch(&mut copy, &mut t, 'W');
    assert_eq!(cols(&copy), (0, 6));
    ch(&mut copy, &mut t, 'E');
    assert_eq!(cols(&copy), (0, 15));
    ch(&mut copy, &mut t, 'W');
    assert_eq!(cols(&copy), (1, 2));
    ch(&mut copy, &mut t, 'B');
    assert_eq!(cols(&copy), (0, 6));
    ch(&mut copy, &mut t, 'B');
    assert_eq!(cols(&copy), (0, 0));
    ch(&mut copy, &mut t, 'b');
    assert_eq!(cols(&copy), (0, 0));
}

#[test]
fn paragraph_motions() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let first = row_of(&t, "alpha beta.gamma");
    goto(&mut copy, &mut t, first);
    ch(&mut copy, &mut t, 'l');
    ch(&mut copy, &mut t, '}');
    assert_eq!(copy.cursor(), Point::new(first + 2, 1));
    ch(&mut copy, &mut t, '}');
    assert_eq!(copy.cursor(), Point::new(first + 2, 1));
    ch(&mut copy, &mut t, 'j');
    ch(&mut copy, &mut t, '{');
    assert_eq!(copy.cursor(), Point::new(first + 2, 1));
    ch(&mut copy, &mut t, '{');
    assert_eq!(copy.cursor(), Point::new(first + 2, 1));
}

#[test]
fn history_and_page_motions() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let last = t.end_row() - 1;
    ch(&mut copy, &mut t, 'g');
    assert_eq!(copy.cursor().row, t.first_row());
    assert_eq!(t.viewport_top(), t.first_row());
    copy.key(
        &mut t,
        &KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
    );
    assert_eq!(copy.cursor().row, last);
    assert!(!t.is_scrolled_back());
    press(&mut copy, &mut t, KeyCode::PageUp);
    assert_eq!(copy.cursor().row, last - 2);
    ctrl(&mut copy, &mut t, 'b');
    assert_eq!(copy.cursor().row, last - 4);
    ctrl(&mut copy, &mut t, 'u');
    assert_eq!(copy.cursor().row, last - 6);
    ctrl(&mut copy, &mut t, 'd');
    assert_eq!(copy.cursor().row, last - 4);
    ctrl(&mut copy, &mut t, 'f');
    press(&mut copy, &mut t, KeyCode::PageDown);
    assert_eq!(copy.cursor().row, last);
    let (top, height) = (t.viewport_top(), u64::from(t.size().1));
    assert!((top..top + height).contains(&copy.cursor().row));
}

#[test]
fn char_and_line_selection_copy() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let row = row_of(&t, "alpha beta.gamma");
    goto(&mut copy, &mut t, row);
    ch(&mut copy, &mut t, 'w');
    ch(&mut copy, &mut t, 'v');
    assert!(copy.is_selecting());
    ch(&mut copy, &mut t, 'e');
    let selection = copy.selection(&t).unwrap();
    assert_eq!(t.selection_text(&selection), "beta");
    ch(&mut copy, &mut t, 'V');
    ch(&mut copy, &mut t, 'j');
    let action = ch(&mut copy, &mut t, 'y');
    let CopyAction::Copy(Some(selection)) = action else {
        panic!("{action:?}");
    };
    assert_eq!(selection.kind, SelectionKind::Line);
    assert_eq!(
        t.selection_text(&selection),
        "alpha beta.gamma\n  indented line"
    );
    assert!(!t.is_scrolled_back());
}

#[test]
fn space_starts_selection_and_enter_copies() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let row = row_of(&t, "x-y z");
    goto(&mut copy, &mut t, row);
    ch(&mut copy, &mut t, ' ');
    ch(&mut copy, &mut t, 'l');
    ch(&mut copy, &mut t, 'l');
    let action = press(&mut copy, &mut t, KeyCode::Enter);
    assert_eq!(
        action,
        CopyAction::Copy(Some(Selection::new(
            Point::new(row, 0),
            Point::new(row, 2),
            SelectionKind::Char
        )))
    );
}

#[test]
fn escape_clears_then_exits() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    ch(&mut copy, &mut t, 'v');
    assert_eq!(press(&mut copy, &mut t, KeyCode::Esc), CopyAction::Redraw);
    assert!(!copy.is_selecting());
    assert_eq!(press(&mut copy, &mut t, KeyCode::Esc), CopyAction::Exit);
}

#[test]
fn exit_restores_the_entry_scroll() {
    let mut t = sample();
    t.scroll(-3);
    let offset = t.scrollbar().offset_from_bottom();
    let mut copy = CopyMode::enter(&t);
    ch(&mut copy, &mut t, 'g');
    assert_eq!(t.viewport_top(), t.first_row());
    assert_eq!(ch(&mut copy, &mut t, 'q'), CopyAction::Exit);
    assert_eq!(t.scrollbar().offset_from_bottom(), offset);
}

#[test]
fn search_prompt_find_and_repeat() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    assert_eq!(
        ch(&mut copy, &mut t, '/'),
        CopyAction::Prompt(Direction::Forward)
    );
    assert_eq!(
        copy.key(
            &mut t,
            &KeyEvent::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
        ),
        CopyAction::Prompt(Direction::Backward)
    );
    let first = row_of(&t, "l 0");
    copy.find(&mut t, "l ", Direction::Backward);
    let search = copy.search().unwrap();
    assert_eq!((search.total, search.index), (10, 9));
    assert_eq!(copy.cursor(), Point::new(first + 9, 0));
    ch(&mut copy, &mut t, 'n');
    assert_eq!(copy.cursor(), Point::new(first + 8, 0));
    copy.key(
        &mut t,
        &KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
    );
    assert_eq!(copy.cursor(), Point::new(first + 9, 0));
    assert_eq!(copy.search().unwrap().index, 9);
    copy.find(&mut t, "GAMMA", Direction::Forward);
    assert_eq!(copy.search().unwrap().total, 0);
    copy.find(&mut t, "Gamma", Direction::Forward);
    assert_eq!(copy.search().unwrap().total, 0);
    copy.find(&mut t, "gamma", Direction::Forward);
    let current = copy.search().unwrap().current.unwrap();
    let (top, height) = (t.viewport_top(), u64::from(t.size().1));
    assert!((top..top + height - 1).contains(&current.start.row));
    let action = ch(&mut copy, &mut t, 'y');
    let CopyAction::Copy(Some(selection)) = action else {
        panic!("{action:?}");
    };
    assert_eq!(t.selection_text(&selection), "gamma");
}

#[test]
fn modified_keys_are_ignored() {
    let mut t = sample();
    let mut copy = CopyMode::enter(&t);
    let start = copy.cursor();
    let action = copy.key(
        &mut t,
        &KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT),
    );
    assert_eq!(action, CopyAction::None);
    assert_eq!(ctrl(&mut copy, &mut t, 'x'), CopyAction::None);
    assert_eq!(copy.cursor(), start);
}
