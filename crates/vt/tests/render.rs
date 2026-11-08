mod common;

use common::{lines, term};
use ratatui_core::buffer::{Buffer, CellDiffOption};
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier, Style};
use vt::{Cursor, CursorShape, Highlight, Point, RenderOptions, Terminal, UnderlineStyle};

fn draw(t: &Terminal, area: Rect) -> (Buffer, Option<Cursor>) {
    let mut buf = Buffer::empty(Rect::new(0, 0, area.right() + 2, area.bottom() + 2));
    let rendered = t.render(area, &mut buf, &RenderOptions::default());
    (buf, rendered.cursor)
}

fn text(buf: &Buffer, y: u16, from: u16, to: u16) -> String {
    (from..to).map(|x| buf[(x, y)].symbol()).collect()
}

#[test]
fn styles_map_to_modifiers() {
    let mut t = term(20, 2);
    t.feed(b"\x1b[1mB\x1b[0m\x1b[2mF\x1b[0m\x1b[3mI\x1b[0m\x1b[4mU\x1b[0m\x1b[5mK\x1b[0m");
    t.feed(b"\x1b[7mR\x1b[0m\x1b[8mH\x1b[0m\x1b[9mS\x1b[0m\x1b[4:3;58;5;2mC\x1b[0mN");
    let (buf, _) = draw(&t, Rect::new(0, 0, 20, 2));
    let expect = [
        Modifier::BOLD,
        Modifier::DIM,
        Modifier::ITALIC,
        Modifier::UNDERLINED,
        Modifier::SLOW_BLINK,
        Modifier::REVERSED,
        Modifier::HIDDEN,
        Modifier::CROSSED_OUT,
        Modifier::UNDERLINED,
        Modifier::empty(),
    ];
    for (x, modifier) in expect.into_iter().enumerate() {
        assert_eq!(
            buf[(u16::try_from(x).unwrap(), 0)].modifier & Modifier::all(),
            modifier,
            "col {x}"
        );
    }
    assert_eq!(
        UnderlineStyle::of(buf[(3, 0)].modifier),
        Some(UnderlineStyle::Single)
    );
    assert_eq!(
        UnderlineStyle::of(buf[(8, 0)].modifier),
        Some(UnderlineStyle::Curly)
    );
    assert_eq!(UnderlineStyle::of(buf[(0, 0)].modifier), None);
    assert_eq!(buf[(8, 0)].underline_color, Color::Indexed(2));
    assert_eq!(buf[(3, 0)].underline_color, Color::Reset);
    assert_eq!(text(&buf, 0, 0, 10), "BFIUKRHSCN");
}

#[test]
fn colors_keep_the_host_palette() {
    let mut t = term(20, 2);
    t.feed(b"\x1b[31;42ma\x1b[91mb\x1b[38;5;196;48;5;17mc\x1b[38;2;1;2;3;48;2;4;5;6md\x1b[0me");
    let (buf, _) = draw(&t, Rect::new(0, 0, 20, 2));
    let colors: Vec<(Color, Color)> = (0..5).map(|x| (buf[(x, 0)].fg, buf[(x, 0)].bg)).collect();
    assert_eq!(
        colors,
        [
            (Color::Indexed(1), Color::Indexed(2)),
            (Color::Indexed(9), Color::Indexed(2)),
            (Color::Indexed(196), Color::Indexed(17)),
            (Color::Rgb(1, 2, 3), Color::Rgb(4, 5, 6)),
            (Color::Reset, Color::Reset),
        ]
    );
}

#[test]
fn program_palette_changes_resolve_to_rgb() {
    let mut t = term(20, 2);
    t.feed(b"\x1b]4;1;rgb:12/34/56\x07\x1b]10;rgb:aa/bb/cc\x07\x1b]11;rgb:01/02/03\x07");
    t.feed(b"\x1b[31ma\x1b[32mb\x1b[0mc");
    let (buf, _) = draw(&t, Rect::new(0, 0, 20, 2));
    assert_eq!(buf[(0, 0)].fg, Color::Rgb(0x12, 0x34, 0x56));
    assert_eq!(buf[(1, 0)].fg, Color::Indexed(2));
    assert_eq!(buf[(2, 0)].fg, Color::Rgb(0xaa, 0xbb, 0xcc));
    assert_eq!(buf[(2, 0)].bg, Color::Rgb(1, 2, 3));
    t.feed(b"\x1b]104\x07\x1b]110\x07\x1b]111\x07");
    let (buf, _) = draw(&t, Rect::new(0, 0, 20, 2));
    assert_eq!(buf[(0, 0)].fg, Color::Indexed(1));
    assert_eq!(buf[(2, 0)].fg, Color::Reset);
    assert_eq!(buf[(2, 0)].bg, Color::Reset);
}

#[test]
fn erased_cells_keep_their_background() {
    let mut t = term(6, 2);
    t.feed(b"\x1b[44m\x1b[2K\x1b[0m");
    let (buf, _) = draw(&t, Rect::new(0, 0, 6, 2));
    assert!((0..6).all(|x| buf[(x, 0)].bg == Color::Indexed(4)));
    assert_eq!(buf[(0, 1)].bg, Color::Reset);
}

#[test]
fn wide_chars_take_two_cells() {
    let mut t = term(8, 2);
    t.feed("中a🐑".as_bytes());
    let (buf, _) = draw(&t, Rect::new(0, 0, 8, 2));
    assert_eq!(buf[(0, 0)].symbol(), "中");
    assert_eq!(buf[(1, 0)].symbol(), " ");
    assert_eq!(buf[(2, 0)].symbol(), "a");
    assert_eq!(buf[(3, 0)].symbol(), "🐑");
    let diff = Buffer::empty(buf.area).diff(&buf);
    assert!(
        diff.iter()
            .all(|(x, y, _)| !(*y == 0 && (*x == 1 || *x == 4)))
    );
}

#[test]
fn grapheme_clusters_stay_in_one_cell() {
    let mut t = term(8, 2);
    t.feed("e\u{301}x✍\u{fe0f}y".as_bytes());
    let (buf, _) = draw(&t, Rect::new(0, 0, 8, 2));
    assert_eq!(buf[(0, 0)].symbol(), "e\u{301}");
    assert_eq!(buf[(1, 0)].symbol(), "x");
    assert_eq!(buf[(2, 0)].symbol(), "✍\u{fe0f}");
    assert_eq!(buf[(2, 0)].diff_option, CellDiffOption::None);
    assert_eq!(buf[(4, 0)].symbol(), "y");
}

#[test]
fn wide_char_cut_by_the_area_becomes_blank() {
    let mut t = term(8, 2);
    t.feed("ab中".as_bytes());
    let (buf, _) = draw(&t, Rect::new(0, 0, 3, 2));
    assert_eq!(text(&buf, 0, 0, 4), "ab  ");
}

#[test]
fn draws_at_an_offset_and_clears_the_rest() {
    let mut t = term(4, 2);
    t.feed(b"abcd\r\nef");
    let mut buf = Buffer::filled(Rect::new(0, 0, 10, 5), ratatui_core::buffer::Cell::new("x"));
    let rendered = t.render(Rect::new(2, 1, 6, 3), &mut buf, &RenderOptions::default());
    assert_eq!(text(&buf, 0, 0, 10), "xxxxxxxxxx");
    assert_eq!(text(&buf, 1, 0, 10), "xxabcd  xx");
    assert_eq!(text(&buf, 2, 0, 10), "xxef    xx");
    assert_eq!(text(&buf, 3, 0, 10), "xx      xx");
    assert_eq!(text(&buf, 4, 0, 10), "xxxxxxxxxx");
    let cursor = rendered.cursor.unwrap();
    assert_eq!((cursor.x, cursor.y), (4, 2));
}

#[test]
fn cursor_shape_and_visibility() {
    let mut t = term(10, 3);
    t.feed(b"hi\x1b[4 q");
    let (_, cursor) = draw(&t, Rect::new(0, 0, 10, 3));
    assert_eq!(
        cursor,
        Some(Cursor {
            x: 2,
            y: 0,
            shape: CursorShape::Underline,
            blinking: false
        })
    );
    t.feed(b"\x1b[?25l");
    assert_eq!(draw(&t, Rect::new(0, 0, 10, 3)).1, None);
}

#[test]
fn cursor_hides_while_scrolled_back() {
    let mut t = term(10, 3);
    lines(&mut t, 10);
    assert!(draw(&t, Rect::new(0, 0, 10, 3)).1.is_some());
    t.scroll(-2);
    assert_eq!(draw(&t, Rect::new(0, 0, 10, 3)).1, None);
}

#[test]
fn renders_the_scrolled_viewport() {
    let mut t = term(10, 3);
    lines(&mut t, 10);
    t.scroll(-3);
    let (buf, _) = draw(&t, Rect::new(0, 0, 10, 3));
    assert_eq!(text(&buf, 0, 0, 6), "line 5");
    assert_eq!(text(&buf, 2, 0, 6), "line 7");
}

#[test]
fn highlights_patch_absolute_ranges() {
    let mut t = term(6, 3);
    t.feed(b"abcdef\r\nghijkl\r\nmnopqr");
    let top = t.viewport_top();
    let mut buf = Buffer::empty(Rect::new(0, 0, 6, 3));
    let selection = Style::new().bg(Color::Indexed(4));
    let current = Style::new().add_modifier(Modifier::BOLD);
    let highlights = [
        Highlight {
            start: Point::new(top + 1, 4),
            end: Point::new(top, 3),
            style: selection,
        },
        Highlight {
            start: Point::new(top + 2, 1),
            end: Point::new(top + 2, 1),
            style: current,
        },
        Highlight {
            start: Point::new(top + 50, 0),
            end: Point::new(top + 60, 0),
            style: current,
        },
    ];
    t.render(
        buf.area,
        &mut buf,
        &RenderOptions {
            highlights: &highlights,
        },
    );
    let marked: Vec<String> = (0..3)
        .map(|y| {
            (0..6)
                .map(|x| {
                    if buf[(x, y)].bg == Color::Indexed(4) {
                        '#'
                    } else {
                        '.'
                    }
                })
                .collect()
        })
        .collect();
    assert_eq!(marked, ["...###", "#####.", "......"]);
    assert!(buf[(1, 2)].modifier.contains(Modifier::BOLD));
    assert!(!buf[(2, 2)].modifier.contains(Modifier::BOLD));
}
