mod common;

use common::term;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MediaKeyCode, ModifierKeyCode,
    MouseButton, MouseEvent, MouseEventKind,
};

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn plain(code: KeyCode) -> KeyEvent {
    key(code, KeyModifiers::NONE)
}

fn kind(mut event: KeyEvent, kind: KeyEventKind) -> KeyEvent {
    event.kind = kind;
    event
}

fn keypad(code: KeyCode) -> KeyEvent {
    KeyEvent::new_with_kind_and_state(
        code,
        KeyModifiers::NONE,
        KeyEventKind::Press,
        KeyEventState::KEYPAD,
    )
}

fn mouse(kind: MouseEventKind, modifiers: KeyModifiers) -> MouseEvent {
    MouseEvent {
        kind,
        column: 0,
        row: 0,
        modifiers,
    }
}

const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const ALT: KeyModifiers = KeyModifiers::ALT;

#[test]
fn legacy_keys() {
    let mut t = term(10, 2);
    let cases: Vec<(KeyEvent, &[u8])> = vec![
        (plain(KeyCode::Char('a')), b"a"),
        (key(KeyCode::Char('A'), SHIFT), b"A"),
        (key(KeyCode::Char('!'), SHIFT), b"!"),
        (plain(KeyCode::Char('é')), "é".as_bytes()),
        (plain(KeyCode::Char('中')), "中".as_bytes()),
        (plain(KeyCode::Enter), b"\r"),
        (plain(KeyCode::Tab), b"\t"),
        (plain(KeyCode::BackTab), b"\x1b[Z"),
        (plain(KeyCode::Backspace), b"\x7f"),
        (plain(KeyCode::Esc), b"\x1b"),
        (plain(KeyCode::Up), b"\x1b[A"),
        (plain(KeyCode::Down), b"\x1b[B"),
        (plain(KeyCode::Right), b"\x1b[C"),
        (plain(KeyCode::Left), b"\x1b[D"),
        (plain(KeyCode::Home), b"\x1b[H"),
        (plain(KeyCode::End), b"\x1b[F"),
        (plain(KeyCode::PageUp), b"\x1b[5~"),
        (plain(KeyCode::PageDown), b"\x1b[6~"),
        (plain(KeyCode::Insert), b"\x1b[2~"),
        (plain(KeyCode::Delete), b"\x1b[3~"),
        (plain(KeyCode::F(1)), b"\x1bOP"),
        (plain(KeyCode::F(4)), b"\x1bOS"),
        (plain(KeyCode::F(5)), b"\x1b[15~"),
        (plain(KeyCode::F(12)), b"\x1b[24~"),
        (key(KeyCode::Char('c'), CTRL), b"\x03"),
        (key(KeyCode::Char(' '), CTRL), b"\x00"),
        (plain(KeyCode::Null), b"\x00"),
        (key(KeyCode::Char('a'), ALT), b"\x1ba"),
        (key(KeyCode::Char('a'), CTRL | ALT), b"\x1b\x01"),
        (key(KeyCode::Up, SHIFT), b"\x1b[1;2A"),
        (key(KeyCode::Right, CTRL), b"\x1b[1;5C"),
        (key(KeyCode::F(5), CTRL | SHIFT), b"\x1b[15;6~"),
        (key(KeyCode::Delete, ALT), b"\x1b[3;3~"),
    ];
    for (event, expected) in cases {
        assert_eq!(t.key(&event), expected, "{event:?}");
    }
}

#[test]
fn function_keys_past_twelve() {
    let mut t = term(10, 2);
    assert_eq!(t.key(&plain(KeyCode::F(13))), b"\x1b[1;2P");
    assert_eq!(t.key(&plain(KeyCode::F(24))), b"\x1b[24;2~");
    t.feed(b"\x1b[>1u");
    assert_eq!(t.key(&plain(KeyCode::F(13))), b"\x1b[57376u");
    assert_eq!(t.key(&plain(KeyCode::F(24))), b"\x1b[57387u");
}

#[test]
fn kitty_only_keys_are_silent_in_legacy_mode() {
    let mut t = term(10, 2);
    assert!(
        t.key(&plain(KeyCode::Media(MediaKeyCode::PlayPause)))
            .is_empty()
    );
    assert!(t.key(&plain(KeyCode::Menu)).is_empty());
    assert!(
        t.key(&plain(KeyCode::Modifier(ModifierKeyCode::LeftShift)))
            .is_empty()
    );
}

#[test]
fn releases_are_silent_in_legacy_mode() {
    let mut t = term(10, 2);
    assert!(
        t.key(&kind(plain(KeyCode::Char('a')), KeyEventKind::Release))
            .is_empty()
    );
    assert_eq!(
        t.key(&kind(plain(KeyCode::Char('a')), KeyEventKind::Repeat)),
        b"a"
    );
}

#[test]
fn application_cursor_and_keypad() {
    let mut t = term(10, 2);
    assert_eq!(t.key(&keypad(KeyCode::Char('5'))), b"5");
    assert_eq!(t.key(&keypad(KeyCode::Enter)), b"\r");
    t.feed(b"\x1b[?1h");
    assert_eq!(t.key(&plain(KeyCode::Up)), b"\x1bOA");
    assert_eq!(t.key(&plain(KeyCode::Home)), b"\x1bOH");
    t.feed(b"\x1b=\x1b[?1035l");
    assert_eq!(t.key(&keypad(KeyCode::Char('5'))), b"\x1bOu");
    assert_eq!(t.key(&keypad(KeyCode::Enter)), b"\x1bOM");
    assert_eq!(t.key(&keypad(KeyCode::Char('+'))), b"\x1bOk");
}

#[test]
fn kitty_keyboard_flags() {
    let mut t = term(10, 2);
    t.feed(b"\x1b[>1u");
    assert_eq!(t.key(&plain(KeyCode::Esc)), b"\x1b[27u");
    assert_eq!(t.key(&key(KeyCode::Char('c'), CTRL)), b"\x1b[99;5u");
    assert_eq!(t.key(&plain(KeyCode::Char('a'))), b"a");
    assert!(
        t.key(&kind(plain(KeyCode::Char('a')), KeyEventKind::Release))
            .is_empty()
    );
    t.feed(b"\x1b[>3u");
    assert_eq!(
        t.key(&kind(key(KeyCode::Char('c'), CTRL), KeyEventKind::Repeat)),
        b"\x1b[99;5:2u"
    );
    assert_eq!(
        t.key(&kind(key(KeyCode::Char('c'), CTRL), KeyEventKind::Release)),
        b"\x1b[99;5:3u"
    );
    t.feed(b"\x1b[>11u");
    assert_eq!(t.key(&plain(KeyCode::Char('a'))), b"\x1b[97u");
    assert_eq!(t.key(&plain(KeyCode::Enter)), b"\x1b[13u");
    assert_eq!(
        t.key(&kind(plain(KeyCode::Enter), KeyEventKind::Release)),
        b"\x1b[13;1:3u"
    );
    assert_eq!(
        t.key(&key(KeyCode::Modifier(ModifierKeyCode::LeftShift), SHIFT)),
        b"\x1b[57441;2u"
    );
    assert_eq!(
        t.key(&plain(KeyCode::Media(MediaKeyCode::PlayPause))),
        b"\x1b[57430u"
    );
    assert_eq!(
        t.key(&kind(
            key(KeyCode::Media(MediaKeyCode::RaiseVolume), CTRL),
            KeyEventKind::Release
        )),
        b"\x1b[57439;5:3u"
    );
    assert_eq!(t.key(&plain(KeyCode::Menu)), b"\x1b[57363u");
    assert_eq!(t.key(&keypad(KeyCode::Char('5'))), b"\x1b[57404u");
    assert_eq!(t.key(&plain(KeyCode::CapsLock)), b"\x1b[57358u");
    t.feed(b"\x1b[<u\x1b[<u\x1b[<u");
    assert_eq!(t.key(&plain(KeyCode::Esc)), b"\x1b");
}

#[test]
fn kitty_shifted_keys_report_alternates() {
    let mut t = term(10, 2);
    t.feed(b"\x1b[>15u");
    assert_eq!(t.key(&key(KeyCode::Char('A'), SHIFT)), b"\x1b[97:65;2u");
    t.feed(b"\x1b[=31u");
    assert_eq!(t.key(&key(KeyCode::Char('A'), SHIFT)), b"\x1b[97:65;2;65u");
    assert_eq!(t.key(&key(KeyCode::Char('!'), SHIFT)), b"\x1b[49:33;2;33u");
    t.feed(b"\x1b[=1u");
    assert_eq!(t.key(&key(KeyCode::Char('A'), SHIFT)), b"A");
}

#[test]
fn modify_other_keys() {
    let mut t = term(10, 2);
    assert_eq!(t.key(&key(KeyCode::Char('a'), ALT)), b"\x1ba");
    assert_eq!(t.key(&key(KeyCode::Char('i'), CTRL)), b"\x1b[105;5u");
    t.feed(b"\x1b[>4;2m");
    assert_eq!(t.key(&key(KeyCode::Char('a'), ALT)), b"\x1b[27;3;97~");
    assert_eq!(t.key(&key(KeyCode::Char('i'), CTRL)), b"\x1b[27;5;105~");
    assert_eq!(t.key(&key(KeyCode::Char('c'), CTRL)), b"\x03");
    assert_eq!(t.key(&key(KeyCode::Enter, SHIFT)), b"\x1b[27;2;13~");
    t.feed(b"\x1b[>4;0m");
    assert_eq!(t.key(&key(KeyCode::Char('a'), ALT)), b"\x1ba");
}

#[test]
fn paste_brackets_when_asked() {
    let mut t = term(10, 2);
    assert_eq!(t.paste("ls\n"), b"ls\r");
    assert_eq!(t.paste("a\x1bb\x03c"), b"a b c");
    t.feed(b"\x1b[?2004h");
    assert_eq!(t.paste("ls\n"), b"\x1b[200~ls\n\x1b[201~");
    assert_eq!(t.paste("x\x1b[201~y"), b"\x1b[200~x [201~y\x1b[201~");
}

#[test]
fn focus_reports_only_when_asked() {
    let mut t = term(10, 2);
    assert_eq!(t.focus(true), None);
    t.feed(b"\x1b[?1004h");
    assert_eq!(t.focus(true).as_deref(), Some(&b"\x1b[I"[..]));
    assert_eq!(t.focus(false).as_deref(), Some(&b"\x1b[O"[..]));
}

#[test]
fn mouse_is_local_without_tracking() {
    let mut t = term(80, 24);
    let click = mouse(MouseEventKind::Down(MouseButton::Left), KeyModifiers::NONE);
    assert_eq!(t.mouse(&click, 3, 2), None);
    assert_eq!(
        t.mouse(&mouse(MouseEventKind::ScrollUp, KeyModifiers::NONE), 3, 2),
        None
    );
}

#[test]
fn mouse_normal_tracking() {
    let mut t = term(80, 24);
    t.feed(b"\x1b[?1000h");
    let down = mouse(MouseEventKind::Down(MouseButton::Left), KeyModifiers::NONE);
    assert_eq!(t.mouse(&down, 3, 2).unwrap(), b"\x1b[M $#");
    let up = mouse(MouseEventKind::Up(MouseButton::Left), KeyModifiers::NONE);
    assert_eq!(t.mouse(&up, 3, 2).unwrap(), b"\x1b[M#$#");
    let drag = mouse(MouseEventKind::Drag(MouseButton::Left), KeyModifiers::NONE);
    assert_eq!(t.mouse(&drag, 4, 2), None);
    let wheel = mouse(MouseEventKind::ScrollDown, KeyModifiers::NONE);
    assert_eq!(t.mouse(&wheel, 3, 2).unwrap(), b"\x1b[Ma$#");
}

#[test]
fn mouse_sgr_modes() {
    let mut t = term(80, 24);
    t.feed(b"\x1b[?1000h\x1b[?1006h");
    let down = mouse(MouseEventKind::Down(MouseButton::Left), KeyModifiers::NONE);
    assert_eq!(t.mouse(&down, 3, 2).unwrap(), b"\x1b[<0;4;3M");
    let up = mouse(MouseEventKind::Up(MouseButton::Right), KeyModifiers::NONE);
    assert_eq!(t.mouse(&up, 3, 2).unwrap(), b"\x1b[<2;4;3m");
    let ctrl = mouse(MouseEventKind::Down(MouseButton::Middle), CTRL);
    assert_eq!(t.mouse(&ctrl, 0, 0).unwrap(), b"\x1b[<17;1;1M");
    let wheel = mouse(MouseEventKind::ScrollUp, KeyModifiers::NONE);
    assert_eq!(t.mouse(&wheel, 0, 0).unwrap(), b"\x1b[<64;1;1M");
    let moved = mouse(MouseEventKind::Moved, KeyModifiers::NONE);
    assert_eq!(t.mouse(&moved, 5, 5), None);
}

#[test]
fn mouse_button_and_any_motion() {
    let mut t = term(80, 24);
    t.feed(b"\x1b[?1002h\x1b[?1006h");
    let drag = mouse(MouseEventKind::Drag(MouseButton::Left), KeyModifiers::NONE);
    assert_eq!(t.mouse(&drag, 4, 2).unwrap(), b"\x1b[<32;5;3M");
    let moved = mouse(MouseEventKind::Moved, KeyModifiers::NONE);
    assert_eq!(t.mouse(&moved, 5, 5), None);
    t.feed(b"\x1b[?1003h");
    assert_eq!(t.mouse(&moved, 5, 5).unwrap(), b"\x1b[<35;6;6M");
}

#[test]
fn mouse_sgr_pixels() {
    let mut t = term(80, 24);
    t.feed(b"\x1b[?1000h\x1b[?1016h");
    let down = mouse(MouseEventKind::Down(MouseButton::Left), KeyModifiers::NONE);
    assert_eq!(t.mouse(&down, 3, 2).unwrap(), b"\x1b[<0;35;50M");
}

#[test]
fn alternate_scroll_sends_arrows() {
    let mut t = term(80, 24);
    let up = mouse(MouseEventKind::ScrollUp, KeyModifiers::NONE);
    let down = mouse(MouseEventKind::ScrollDown, KeyModifiers::NONE);
    t.feed(b"\x1b[?1049h");
    assert!(t.alternate_scroll());
    assert_eq!(t.mouse(&up, 0, 0).unwrap(), b"\x1b[A");
    assert_eq!(t.mouse(&down, 0, 0).unwrap(), b"\x1b[B");
    t.feed(b"\x1b[?1h");
    assert_eq!(t.mouse(&up, 0, 0).unwrap(), b"\x1bOA");
    t.feed(b"\x1b[?1007l");
    assert_eq!(t.mouse(&up, 0, 0), None);
}
