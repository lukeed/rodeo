mod common;

use common::term;
use vt::{Direction, Grid, Link, Match, Point, Selection, SelectionKind, Terminal};

fn at(t: &Terminal, row: u64, col: u16) -> Point {
    Point::new(t.viewport_top() + row, col)
}

fn sel(t: &Terminal, a: (u64, u16), b: (u64, u16), kind: SelectionKind) -> Selection {
    Selection::new(at(t, a.0, a.1), at(t, b.0, b.1), kind)
}

fn word(t: &Terminal, row: u64, col: u16) -> Option<String> {
    let (start, end) = t.word_bounds(at(t, row, col))?;
    Some(t.selection_text(&Selection::new(start, end, SelectionKind::Char)))
}

#[test]
fn char_selection_text() {
    let mut t = term(20, 4);
    t.feed(b"hello world   \r\nsecond line\r\nthird");
    assert_eq!(
        t.selection_text(&sel(&t, (0, 6), (0, 10), SelectionKind::Char)),
        "world"
    );
    assert_eq!(
        t.selection_text(&sel(&t, (1, 7), (0, 6), SelectionKind::Char)),
        "world\nsecond l"
    );
    assert_eq!(
        t.selection_text(&sel(&t, (0, 0), (2, 19), SelectionKind::Char)),
        "hello world\nsecond line\nthird"
    );
}

#[test]
fn line_selection_text() {
    let mut t = term(20, 4);
    t.feed(b"  one  \r\ntwo\r\nthree");
    assert_eq!(
        t.selection_text(&sel(&t, (1, 2), (0, 5), SelectionKind::Line)),
        "  one\ntwo"
    );
}

#[test]
fn word_selection_extends_by_whole_words() {
    let mut t = term(30, 2);
    t.feed(b"alpha beta gamma delta");
    assert_eq!(
        t.selection_text(&sel(&t, (0, 8), (0, 13), SelectionKind::Word)),
        "beta gamma"
    );
    assert_eq!(
        t.selection_bounds(&sel(&t, (0, 13), (0, 8), SelectionKind::Word)),
        (at(&t, 0, 6), at(&t, 0, 15))
    );
}

#[test]
fn wide_chars_and_wrapped_lines_join() {
    let mut t = term(6, 4);
    t.feed("ab中文cdefgh\r\nnext".as_bytes());
    assert_eq!(t.row_text(t.viewport_top()).as_deref(), Some("ab中文"));
    assert_eq!(
        t.selection_text(&sel(&t, (0, 2), (1, 3), SelectionKind::Char)),
        "中文cdef"
    );
    assert_eq!(
        t.selection_text(&sel(&t, (0, 0), (2, 5), SelectionKind::Line)),
        "ab中文cdefgh\nnext"
    );
    assert_eq!(
        t.selection_text(&sel(&t, (0, 3), (0, 3), SelectionKind::Char)),
        "中"
    );
}

#[test]
fn word_bounds_follow_herdr_rules() {
    let mut t = term(80, 4);
    t.feed(b"see https://example.com/a_(b)?x=1). and (foo/bar.rs:12) then\r\n");
    t.feed(b"cat 'my dir/file name.txt' now | pipe [item],next $HOME\r\n");
    t.feed("中文（测试）。done".as_bytes());
    assert_eq!(
        word(&t, 0, 10).as_deref(),
        Some("https://example.com/a_(b)?x=1")
    );
    assert_eq!(word(&t, 0, 2).as_deref(), Some("see"));
    assert_eq!(word(&t, 0, 43).as_deref(), Some("foo/bar.rs:12"));
    assert_eq!(word(&t, 0, 3), None);
    assert_eq!(word(&t, 1, 10).as_deref(), Some("my dir/file name.txt"));
    assert_eq!(word(&t, 1, 4), None);
    assert_eq!(word(&t, 1, 30).as_deref(), None);
    assert_eq!(word(&t, 1, 36).as_deref(), Some("pipe"));
    assert_eq!(word(&t, 1, 42).as_deref(), Some("item"));
    assert_eq!(word(&t, 1, 48).as_deref(), Some("next"));
    assert_eq!(word(&t, 1, 54).as_deref(), Some("$HOME"));
    assert_eq!(word(&t, 2, 1).as_deref(), Some("中文"));
    assert_eq!(word(&t, 2, 7).as_deref(), Some("测试"));
    assert_eq!(word(&t, 2, 15).as_deref(), Some("done"));
}

#[test]
fn word_bounds_cross_wrapped_rows() {
    let mut t = term(10, 3);
    t.feed(b"go https://ex.com/long/path ok");
    let (start, end) = t.word_bounds(at(&t, 1, 2)).unwrap();
    assert_eq!((start, end), (at(&t, 0, 3), at(&t, 2, 6)));
}

#[test]
fn detects_plain_urls() {
    let mut t = term(40, 2);
    t.feed(b"open <https://a.dev/x>, ftp://no");
    let link = t.url_at(at(&t, 0, 12)).unwrap();
    assert_eq!(
        link,
        Link {
            uri: "https://a.dev/x".into(),
            start: at(&t, 0, 6),
            end: at(&t, 0, 20),
        }
    );
    assert_eq!(t.url_at(at(&t, 0, 2)), None);
    assert_eq!(t.url_at(at(&t, 0, 28)), None);
}

#[test]
fn detects_osc8_links_across_rows() {
    let mut t = term(8, 3);
    t.feed(b"x \x1b]8;;https://h.dev/p\x1b\\link text\x1b]8;;\x1b\\ y");
    let link = t.hyperlink_at(at(&t, 1, 1)).unwrap();
    assert_eq!(
        link,
        Link {
            uri: "https://h.dev/p".into(),
            start: at(&t, 0, 2),
            end: at(&t, 1, 2),
        }
    );
    assert_eq!(t.hyperlink_at(at(&t, 0, 0)), None);
    assert_eq!(t.hyperlink_at(at(&t, 1, 4)), None);
}

#[test]
fn osc8_link_on_wide_char_spacer() {
    let mut t = term(8, 2);
    t.feed("\x1b]8;;https://w.dev\x1b\\中\x1b]8;;\x1b\\".as_bytes());
    assert_eq!(
        t.hyperlink_at(at(&t, 0, 1)).map(|link| link.uri).as_deref(),
        Some("https://w.dev")
    );
}

#[test]
fn search_is_smart_case_and_wraps() {
    let mut t = term(20, 4);
    t.feed(b"Foo foo\r\nbar FOO\r\nfoo");
    let origin = at(&t, 0, 0);
    let hit = t.search("foo", origin, Direction::Forward);
    assert_eq!(hit.total, 4);
    assert_eq!(hit.index, 1);
    assert_eq!(
        hit.current,
        Some(Match {
            start: at(&t, 0, 4),
            end: at(&t, 0, 6)
        })
    );
    let hit = t.search("FOO", origin, Direction::Forward);
    assert_eq!((hit.total, hit.index), (1, 0));
    assert_eq!(hit.current.unwrap().start, at(&t, 1, 4));
    let hit = t.search("foo", origin, Direction::Backward);
    assert_eq!((hit.total, hit.index), (4, 3));
    assert_eq!(hit.current.unwrap().start, at(&t, 2, 0));
    let hit = t.search("foo", at(&t, 1, 4), Direction::Backward);
    assert_eq!(hit.current.unwrap().start, at(&t, 0, 4));
    let hit = t.search("missing", origin, Direction::Forward);
    assert_eq!((hit.total, hit.current), (0, None));
}

#[test]
fn search_spans_wrapped_rows_and_wide_chars() {
    let mut t = term(6, 4);
    t.feed("xx中文abcdef".as_bytes());
    let hit = t.search("文abc", at(&t, 0, 0), Direction::Forward);
    assert_eq!(
        hit.current,
        Some(Match {
            start: at(&t, 0, 4),
            end: at(&t, 1, 2)
        })
    );
}

#[test]
fn search_reaches_scrollback() {
    let mut t = term(20, 3);
    t.feed(b"needle here\r\n");
    common::lines(&mut t, 20);
    let hit = t.search("needle", at(&t, 2, 0), Direction::Backward);
    let found = hit.current.unwrap();
    assert_eq!(t.row_text(found.start.row).as_deref(), Some("needle here"));
    assert!(found.start.row < t.viewport_top());
}

#[test]
fn visible_matches_cover_the_view_only() {
    let mut t = term(20, 3);
    t.feed(b"ab ab\r\n");
    common::lines(&mut t, 5);
    t.feed(b"ab");
    let visible = t.visible_matches("ab");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].start, at(&t, 2, 0));
    t.scroll_to_top();
    assert_eq!(t.visible_matches("ab").len(), 2);
}

#[test]
fn rows_are_readable_through_the_grid_trait() {
    let mut t = term(5, 3);
    t.feed("a中b\r\nwrapped!".as_bytes());
    let top = t.viewport_top();
    let row = t.row(top).unwrap();
    assert_eq!(row.cell(1), Some(("中", 2)));
    assert_eq!(row.cell(2), Some(("", 0)));
    assert_eq!(row.cell(4), Some((" ", 1)));
    assert!(!row.wrapped);
    assert!(t.row(top + 1).unwrap().wrapped);
    assert_eq!(t.row(top + 2).unwrap().trimmed(), "ed!");
    assert_eq!(Grid::cols(&t), 5);
    assert!(t.row(t.end_row()).is_none());
}
