mod common;

use common::{lines, screen, term};
use vt::{CursorShape, Event, MouseFormat, MouseMode, Options, Point, Terminal};

const HERDR_FRAMES: &[u8] = include_bytes!("fixtures/herdr-frames.bin");

#[test]
fn renders_real_herdr_frames() {
    let mut t = term(60, 10);
    t.feed(HERDR_FRAMES);
    assert_eq!(screen(&t)[0], "red 🐑 中文");
    assert_eq!(
        t.cell(Point::new(t.viewport_top(), 4)),
        Some(("🐑".into(), 2))
    );
    assert_eq!(
        t.cell(Point::new(t.viewport_top(), 5)),
        Some((String::new(), 0))
    );
}

#[test]
fn can_repaint_replaces_the_screen_mid_sequence() {
    let mut t = term(20, 4);
    t.feed(b"old text\r\nmore\x1b[3");
    t.feed(b"\x18\x1b[!p\x1b[H\x1b[2Jfresh\r\nscreen");
    let shown = screen(&t);
    assert_eq!(&shown[..2], ["fresh", "screen"]);
    assert!(shown[2..].iter().all(String::is_empty));
}

#[test]
fn can_cancels_a_partial_osc_and_apc() {
    let mut t = term(20, 3);
    t.feed(b"\x1b]2;half title");
    t.feed(b"\x18done");
    t.feed(b"\x1b_Ga=T,f=24;AAAA");
    t.feed(b"\x18!");
    assert_eq!(screen(&t)[0], "done!");
    assert_eq!(t.title(), "");
}

#[test]
fn repaint_with_history_fills_scrollback() {
    let mut t = term(20, 3);
    t.feed(
        b"\x18\x1b[!p\x1b[H\x1b[2Jh1\r\nh2\r\nh3\r\nh4\x1b[0m\r\n\n\n\n\x1b[H\x1b[2Ja\r\nb\r\nc",
    );
    assert_eq!(screen(&t), ["a", "b", "c"]);
    let first = t.first_row();
    let history: Vec<String> = (first..t.viewport_top())
        .filter_map(|row| t.row_text(row))
        .filter(|text| !text.is_empty())
        .collect();
    assert_eq!(history, ["h1", "h2", "h3", "h4"]);
}

#[test]
fn answers_queries_the_core_passes_through() {
    let mut t = term(10, 2);
    t.feed(b"\x1b[6n");
    assert_eq!(t.take_replies(), b"\x1b[1;1R");
    assert!(t.take_replies().is_empty());
    t.feed(b"\x1b[16t");
    assert_eq!(t.take_replies(), b"\x1b[6;20;10t");
}

#[test]
fn resize_sends_in_band_report_when_asked() {
    let mut t = term(10, 2);
    t.feed(b"\x1b[?2048h");
    t.take_replies();
    t.resize(30, 5, (8, 16)).unwrap();
    assert_eq!(t.take_replies(), b"\x1b[48;5;30;80;240t");
}

#[test]
fn reports_events_in_stream_order() {
    let mut t = term(10, 2);
    t.feed(b"\x1b]2;agent\x07\x07\x1b]9;build done\x07\x1b]52;c;aGVsbG8=\x07");
    t.feed(b"\x1b]777;notify;Claude;needs input\x1b\\\x1b]7;file://host/tmp/work\x07");
    assert_eq!(
        t.take_events(),
        [
            Event::Title("agent".into()),
            Event::Bell,
            Event::Notify {
                title: String::new(),
                body: "build done".into()
            },
            Event::Clipboard("hello".into()),
            Event::Notify {
                title: "Claude".into(),
                body: "needs input".into()
            },
            Event::Cwd("file://host/tmp/work".into()),
        ]
    );
    assert_eq!(t.title(), "agent");
    assert_eq!(t.cwd().as_deref(), Some("file://host/tmp/work"));
}

#[test]
fn osc_9_subcommands_are_not_notifications() {
    let mut t = term(10, 2);
    t.feed(b"\x1b]9;4;1;50\x07\x1b]9;9;/tmp\x07");
    assert!(
        !t.take_events()
            .iter()
            .any(|event| matches!(event, Event::Notify { .. }))
    );
}

#[test]
fn kitty_notifications_join_chunks_and_answer_queries() {
    let mut t = term(10, 2);
    t.feed(b"\x1b]99;i=1:d=0;Hello\x1b\\");
    assert!(t.take_events().is_empty());
    t.feed(b"\x1b]99;i=1:p=body:e=1;d29ybGQ=\x1b\\");
    assert_eq!(
        t.take_events(),
        [Event::Notify {
            title: "Hello".into(),
            body: "world".into()
        }]
    );
    t.feed(b"\x1b]99;i=q:p=?;\x1b\\");
    assert_eq!(t.take_replies(), b"\x1b]99;i=q:p=?;p=title,body\x1b\\");
}

#[test]
fn notification_split_across_feeds() {
    let mut t = term(10, 2);
    t.feed(b"\x1b]9;par");
    t.feed(b"tial\x1b");
    t.feed(b"\\");
    assert_eq!(
        t.take_events(),
        [Event::Notify {
            title: String::new(),
            body: "partial".into()
        }]
    );
}

#[test]
fn mode_queries_follow_the_program() {
    let mut t = term(10, 3);
    assert!(!t.alternate_screen());
    assert_eq!(t.mouse_mode(), MouseMode::None);
    assert!(!t.bracketed_paste());
    assert!(!t.focus_reporting());
    t.feed(b"\x1b[?1049h\x1b[?1002h\x1b[?1006h\x1b[?2004h\x1b[?1004h\x1b[?1h\x1b[?66h");
    assert!(t.alternate_screen());
    assert_eq!(t.mouse_mode(), MouseMode::Button);
    assert_eq!(t.mouse_format(), MouseFormat::Sgr);
    assert!(t.wants_mouse());
    assert!(t.bracketed_paste());
    assert!(t.focus_reporting());
    assert!(t.application_cursor());
    assert!(t.application_keypad());
    t.feed(b"\x1b[?1003h\x1b[?1016h");
    assert_eq!(t.mouse_mode(), MouseMode::Any);
    assert_eq!(t.mouse_format(), MouseFormat::SgrPixels);
}

#[test]
fn cursor_state_follows_the_program() {
    let mut t = term(10, 3);
    t.feed(b"ab\r\ncd\x1b[5 q");
    let cursor = t.cursor();
    assert_eq!((cursor.col, cursor.row), (2, 1));
    assert!(cursor.visible);
    assert_eq!(cursor.shape, CursorShape::Bar);
    assert!(cursor.blinking);
    t.feed(b"\x1b[2 q\x1b[?25l");
    let cursor = t.cursor();
    assert_eq!(cursor.shape, CursorShape::Block);
    assert!(!cursor.blinking);
    assert!(!cursor.visible);
}

#[test]
fn resize_reflows() {
    let mut t = term(10, 3);
    t.feed(b"0123456789abc");
    t.resize(20, 3, (8, 16)).unwrap();
    assert_eq!(t.size(), (20, 3));
    assert_eq!(t.cell_px(), (8, 16));
    assert_eq!(screen(&t)[0], "0123456789abc");
}

#[test]
fn scrollback_follows_output_at_the_bottom() {
    let mut t = term(10, 3);
    lines(&mut t, 10);
    assert_eq!(screen(&t), ["line 8", "line 9", ""]);
    assert!(!t.is_scrolled_back());
    t.feed(b"tail");
    assert_eq!(screen(&t), ["line 8", "line 9", "tail"]);
}

#[test]
fn scrolled_back_view_stays_pinned_on_new_output() {
    let mut t = term(10, 3);
    lines(&mut t, 10);
    t.scroll(-4);
    assert!(t.is_scrolled_back());
    let pinned = screen(&t);
    assert_eq!(pinned, ["line 4", "line 5", "line 6"]);
    let top = t.viewport_top();
    t.feed(b"new a\r\nnew b\r\nnew c\r\nnew d\r\nnew e\r\n");
    assert_eq!(screen(&t), pinned);
    assert_eq!(t.viewport_top(), top);
    t.scroll_to_bottom();
    assert!(!t.is_scrolled_back());
    assert_eq!(screen(&t), ["new d", "new e", ""]);
}

#[test]
fn scroll_by_page_and_to_edges() {
    let mut t = term(10, 3);
    lines(&mut t, 10);
    t.scroll_page(-1);
    assert_eq!(screen(&t), ["line 5", "line 6", "line 7"]);
    t.scroll_to_top();
    assert_eq!(screen(&t), ["line 0", "line 1", "line 2"]);
    let bar = t.scrollbar();
    assert_eq!((bar.total, bar.offset, bar.len), (11, 0, 3));
    assert_eq!(bar.offset_from_bottom(), 8);
    t.scroll(2);
    assert_eq!(t.scrollbar().offset, 2);
    t.scroll_to_row(t.first_row() + 5);
    assert_eq!(screen(&t)[0], "line 5");
    t.scroll_to_row(u64::MAX);
    assert!(!t.is_scrolled_back());
}

#[test]
fn absolute_rows_survive_scrollback_pruning() {
    let mut t = Terminal::new(
        40,
        5,
        Options {
            scrollback_bytes: 1,
            ..Options::default()
        },
    )
    .unwrap();
    let find = |t: &Terminal, text: &str| {
        (t.first_row()..t.end_row()).find(|&row| t.row_text(row).as_deref() == Some(text))
    };
    for i in 0..2000 {
        t.feed(format!("row {i}\r\n").as_bytes());
    }
    let first = t.first_row();
    let row = find(&t, "row 1999").expect("row in buffer");
    for i in 2000..6000 {
        t.feed(format!("row {i}\r\n").as_bytes());
    }
    assert!(t.first_row() > first);
    assert_eq!(find(&t, "row 5999"), Some(row + 4000));
    assert!(t.row_text(first).is_none());
}

#[test]
fn absolute_rows_hold_while_output_scrolls() {
    let mut t = term(20, 3);
    lines(&mut t, 5);
    let row = (t.first_row()..t.end_row())
        .find(|&row| t.row_text(row).as_deref() == Some("line 2"))
        .unwrap();
    lines(&mut t, 50);
    assert_eq!(t.row_text(row).as_deref(), Some("line 2"));
    t.feed(b"\x1b[3J");
    assert!(t.row_text(row).is_none());
    assert!(t.first_row() > row);
}

#[test]
fn clear_keeps_the_cursor_line() {
    let mut t = term(20, 4);
    lines(&mut t, 10);
    t.feed(b"$ prompt");
    t.clear();
    assert_eq!(t.first_row(), t.viewport_top());
    assert_eq!(screen(&t), ["$ prompt", "", "", ""]);
    let cursor = t.cursor();
    assert_eq!((cursor.col, cursor.row), (8, 0));
}
