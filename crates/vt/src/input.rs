use std::fmt::Write;

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MediaKeyCode, ModifierKeyCode,
    MouseButton, MouseEvent, MouseEventKind,
};
use libghostty_vt::key::{self, Key, KittyKeyFlags, Mods, OptionAsAlt};
use libghostty_vt::mouse::{self, Button, EncoderSize};

use crate::term::{Error, Terminal, Vt};

pub(crate) struct Input {
    keys: key::Encoder<'static>,
    key: key::Event<'static>,
    mouse: mouse::Encoder<'static>,
    pointer: mouse::Event<'static>,
}

impl Input {
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self {
            keys: key::Encoder::new()?,
            key: key::Event::new()?,
            mouse: mouse::Encoder::new()?,
            pointer: mouse::Event::new()?,
        })
    }

    fn encode_key(&mut self, vt: &Vt, event: &KeyEvent) -> Vec<u8> {
        let flags = vt.kitty_keyboard_flags().unwrap_or(KittyKeyFlags::empty());
        if let Some(code) = kitty_only(event.code) {
            return kitty_sequence(code, event, flags);
        }
        if let KeyCode::F(n @ 13..=24) = event.code
            && flags.is_empty()
        {
            let mut shifted = *event;
            shifted.code = KeyCode::F(n - 12);
            shifted.modifiers |= KeyModifiers::SHIFT;
            return self.encode_key(vt, &shifted);
        }
        let Some(physical) = physical(event) else {
            return fallback(event);
        };
        let mut mods = mods(event.modifiers) | physical.mods;
        if event.state.contains(KeyEventState::CAPS_LOCK) {
            mods |= Mods::CAPS_LOCK;
        }
        if event.state.contains(KeyEventState::NUM_LOCK) {
            mods |= Mods::NUM_LOCK;
        }
        let consumed = match (physical.text, physical.unshifted) {
            (Some(text), Some(base)) if mods.contains(Mods::SHIFT) && text != base => Mods::SHIFT,
            _ => Mods::empty(),
        };
        self.key
            .set_action(action(event.kind))
            .set_key(physical.key)
            .set_mods(mods)
            .set_consumed_mods(consumed)
            .set_utf8(physical.text.map(String::from))
            .set_unshifted_codepoint(physical.unshifted.unwrap_or('\0'));
        self.keys
            .set_options_from_terminal(vt)
            .set_macos_option_as_alt(OptionAsAlt::True);
        let mut out = Vec::with_capacity(16);
        if self.keys.encode_to_vec(&self.key, &mut out).is_err() {
            return fallback(event);
        }
        out
    }

    fn encode_mouse(
        &mut self,
        vt: &Vt,
        cell_px: (u32, u32),
        event: MouseEvent,
        col: u16,
        row: u16,
    ) -> Vec<u8> {
        let (Ok(cols), Ok(rows)) = (vt.cols(), vt.rows()) else {
            return Vec::new();
        };
        let (cell_width, cell_height) = (cell_px.0.max(1), cell_px.1.max(1));
        let (action, button, pressed) = match event.kind {
            MouseEventKind::Down(button) => {
                (mouse::Action::Press, Some(mouse_button(button)), true)
            }
            MouseEventKind::Up(button) => {
                (mouse::Action::Release, Some(mouse_button(button)), false)
            }
            MouseEventKind::Drag(button) => {
                (mouse::Action::Motion, Some(mouse_button(button)), true)
            }
            MouseEventKind::Moved => (mouse::Action::Motion, None, false),
            MouseEventKind::ScrollUp => (mouse::Action::Press, Some(Button::Four), false),
            MouseEventKind::ScrollDown => (mouse::Action::Press, Some(Button::Five), false),
            MouseEventKind::ScrollLeft => (mouse::Action::Press, Some(Button::Six), false),
            MouseEventKind::ScrollRight => (mouse::Action::Press, Some(Button::Seven), false),
        };
        self.mouse
            .set_options_from_terminal(vt)
            .set_any_button_pressed(pressed)
            .set_size(EncoderSize {
                screen_width: u32::from(cols) * cell_width,
                screen_height: u32::from(rows) * cell_height,
                cell_width,
                cell_height,
                padding_top: 0,
                padding_bottom: 0,
                padding_right: 0,
                padding_left: 0,
            });
        let center = |cell: u16, size: u32| {
            let size = f32::from(u16::try_from(size).unwrap_or(u16::MAX));
            f32::from(cell) * size + size / 2.0
        };
        self.pointer
            .set_action(action)
            .set_button(button)
            .set_mods(mods(event.modifiers))
            .set_position(mouse::Position {
                x: center(col, cell_width),
                y: center(row, cell_height),
            });
        let mut out = Vec::with_capacity(16);
        if self.mouse.encode_to_vec(&self.pointer, &mut out).is_err() {
            out.clear();
        }
        out
    }
}

impl Terminal {
    pub fn key(&mut self, event: &KeyEvent) -> Vec<u8> {
        let (vt, input) = self.input();
        input.encode_key(vt, event)
    }

    pub fn mouse(&mut self, event: &MouseEvent, col: u16, row: u16) -> Option<Vec<u8>> {
        let cell_px = self.cell_px();
        if self.wants_mouse() {
            let (vt, input) = self.input();
            let out = input.encode_mouse(vt, cell_px, *event, col, row);
            return (!out.is_empty()).then_some(out);
        }
        let arrow = match event.kind {
            MouseEventKind::ScrollUp => KeyCode::Up,
            MouseEventKind::ScrollDown => KeyCode::Down,
            _ => return None,
        };
        if !self.alternate_screen() || !self.alternate_scroll() {
            return None;
        }
        let out = self.key(&KeyEvent::new(arrow, KeyModifiers::NONE));
        (!out.is_empty()).then_some(out)
    }
}

struct Physical {
    key: Key,
    text: Option<char>,
    unshifted: Option<char>,
    mods: Mods,
}

impl Physical {
    const fn named(key: Key) -> Self {
        Self {
            key,
            text: None,
            unshifted: None,
            mods: Mods::empty(),
        }
    }

    const fn printable(key: Key, text: char, unshifted: char) -> Self {
        Self {
            key,
            text: Some(text),
            unshifted: Some(unshifted),
            mods: Mods::empty(),
        }
    }
}

fn physical(event: &KeyEvent) -> Option<Physical> {
    let keypad = event.state.contains(KeyEventState::KEYPAD);
    if keypad && let Some(found) = keypad_key(event.code) {
        return Some(found);
    }
    Some(match event.code {
        KeyCode::Char(ch) => char_key(ch),
        KeyCode::Null => Physical {
            mods: Mods::CTRL,
            ..Physical::printable(Key::Space, ' ', ' ')
        },
        KeyCode::BackTab => Physical {
            mods: Mods::SHIFT,
            ..Physical::named(Key::Tab)
        },
        KeyCode::F(n) => Physical::named(function_key(n)?),
        KeyCode::Modifier(modifier) => Physical::named(modifier_key(modifier)?),
        code => Physical::named(named_key(code)?),
    })
}

fn named_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::ArrowLeft,
        KeyCode::Right => Key::ArrowRight,
        KeyCode::Up => Key::ArrowUp,
        KeyCode::Down => Key::ArrowDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Insert => Key::Insert,
        KeyCode::Esc => Key::Escape,
        KeyCode::CapsLock => Key::CapsLock,
        KeyCode::ScrollLock => Key::ScrollLock,
        KeyCode::NumLock => Key::NumLock,
        KeyCode::PrintScreen => Key::PrintScreen,
        KeyCode::Pause => Key::Pause,
        KeyCode::KeypadBegin => Key::NumpadBegin,
        _ => return None,
    })
}

fn keypad_key(code: KeyCode) -> Option<Physical> {
    let digit = |key: Key, ch: char| Physical::printable(key, ch, ch);
    Some(match code {
        KeyCode::Char(ch @ '0'..='9') => digit(NUMPAD_DIGITS[(ch as u8 - b'0') as usize], ch),
        KeyCode::Char(ch @ '+') => digit(Key::NumpadAdd, ch),
        KeyCode::Char(ch @ '-') => digit(Key::NumpadSubtract, ch),
        KeyCode::Char(ch @ '*') => digit(Key::NumpadMultiply, ch),
        KeyCode::Char(ch @ '/') => digit(Key::NumpadDivide, ch),
        KeyCode::Char(ch @ '.') => digit(Key::NumpadDecimal, ch),
        KeyCode::Char(ch @ ',') => digit(Key::NumpadComma, ch),
        KeyCode::Char(ch @ '=') => digit(Key::NumpadEqual, ch),
        KeyCode::Enter => Physical::named(Key::NumpadEnter),
        KeyCode::Left => Physical::named(Key::NumpadLeft),
        KeyCode::Right => Physical::named(Key::NumpadRight),
        KeyCode::Up => Physical::named(Key::NumpadUp),
        KeyCode::Down => Physical::named(Key::NumpadDown),
        KeyCode::Home => Physical::named(Key::NumpadHome),
        KeyCode::End => Physical::named(Key::NumpadEnd),
        KeyCode::PageUp => Physical::named(Key::NumpadPageUp),
        KeyCode::PageDown => Physical::named(Key::NumpadPageDown),
        KeyCode::Insert => Physical::named(Key::NumpadInsert),
        KeyCode::Delete => Physical::named(Key::NumpadDelete),
        KeyCode::KeypadBegin => Physical::named(Key::NumpadBegin),
        _ => return None,
    })
}

const NUMPAD_DIGITS: [Key; 10] = [
    Key::Numpad0,
    Key::Numpad1,
    Key::Numpad2,
    Key::Numpad3,
    Key::Numpad4,
    Key::Numpad5,
    Key::Numpad6,
    Key::Numpad7,
    Key::Numpad8,
    Key::Numpad9,
];

const LETTERS: [Key; 26] = [
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
];

const DIGITS: [Key; 10] = [
    Key::Digit0,
    Key::Digit1,
    Key::Digit2,
    Key::Digit3,
    Key::Digit4,
    Key::Digit5,
    Key::Digit6,
    Key::Digit7,
    Key::Digit8,
    Key::Digit9,
];

const PUNCTUATION: [(char, char, Key); 11] = [
    ('`', '~', Key::Backquote),
    ('-', '_', Key::Minus),
    ('=', '+', Key::Equal),
    ('[', '{', Key::BracketLeft),
    (']', '}', Key::BracketRight),
    ('\\', '|', Key::Backslash),
    (';', ':', Key::Semicolon),
    ('\'', '"', Key::Quote),
    (',', '<', Key::Comma),
    ('.', '>', Key::Period),
    ('/', '?', Key::Slash),
];

const SHIFTED_DIGITS: [char; 10] = [')', '!', '@', '#', '$', '%', '^', '&', '*', '('];

fn char_key(ch: char) -> Physical {
    if ch.is_ascii_alphabetic() {
        let lower = ch.to_ascii_lowercase();
        return Physical::printable(LETTERS[usize::from(lower as u8 - b'a')], ch, lower);
    }
    if let Some(digit) = ch.to_digit(10) {
        return Physical::printable(DIGITS[digit as usize], ch, ch);
    }
    if let Some(digit) = SHIFTED_DIGITS.iter().position(|&c| c == ch) {
        let base = char::from_digit(u32::try_from(digit).unwrap_or(0), 10).unwrap_or('0');
        return Physical::printable(DIGITS[digit], ch, base);
    }
    if ch == ' ' {
        return Physical::printable(Key::Space, ' ', ' ');
    }
    if let Some(&(base, _, key)) = PUNCTUATION
        .iter()
        .find(|(base, shifted, _)| *base == ch || *shifted == ch)
    {
        return Physical::printable(key, ch, base);
    }
    let unshifted = ch.to_lowercase().next().unwrap_or(ch);
    Physical::printable(Key::Unidentified, ch, unshifted)
}

fn function_key(n: u8) -> Option<Key> {
    const KEYS: [Key; 25] = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
        Key::F13,
        Key::F14,
        Key::F15,
        Key::F16,
        Key::F17,
        Key::F18,
        Key::F19,
        Key::F20,
        Key::F21,
        Key::F22,
        Key::F23,
        Key::F24,
        Key::F25,
    ];
    KEYS.get(usize::from(n.checked_sub(1)?)).copied()
}

fn kitty_only(code: KeyCode) -> Option<u32> {
    Some(match code {
        KeyCode::Menu => 57363,
        KeyCode::Media(media) => match media {
            MediaKeyCode::Play => 57428,
            MediaKeyCode::Pause => 57429,
            MediaKeyCode::PlayPause => 57430,
            MediaKeyCode::Reverse => 57431,
            MediaKeyCode::Stop => 57432,
            MediaKeyCode::FastForward => 57433,
            MediaKeyCode::Rewind => 57434,
            MediaKeyCode::TrackNext => 57435,
            MediaKeyCode::TrackPrevious => 57436,
            MediaKeyCode::Record => 57437,
            MediaKeyCode::LowerVolume => 57438,
            MediaKeyCode::RaiseVolume => 57439,
            MediaKeyCode::MuteVolume => 57440,
        },
        KeyCode::Modifier(modifier) => match modifier {
            ModifierKeyCode::LeftHyper => 57445,
            ModifierKeyCode::LeftMeta => 57446,
            ModifierKeyCode::RightHyper => 57451,
            ModifierKeyCode::RightMeta => 57452,
            ModifierKeyCode::IsoLevel3Shift => 57453,
            ModifierKeyCode::IsoLevel5Shift => 57454,
            _ => return None,
        },
        _ => return None,
    })
}

fn kitty_sequence(code: u32, event: &KeyEvent, flags: KittyKeyFlags) -> Vec<u8> {
    if flags.is_empty() {
        return Vec::new();
    }
    let events = flags.contains(KittyKeyFlags::REPORT_EVENTS);
    let kind = match event.kind {
        KeyEventKind::Release if !events => return Vec::new(),
        KeyEventKind::Release => 3,
        KeyEventKind::Repeat if events => 2,
        _ => 1,
    };
    let bits = [
        (KeyModifiers::SHIFT, 1),
        (KeyModifiers::ALT, 2),
        (KeyModifiers::CONTROL, 4),
        (KeyModifiers::SUPER, 8),
        (KeyModifiers::HYPER, 16),
        (KeyModifiers::META, 32),
    ]
    .into_iter()
    .filter(|(flag, _)| event.modifiers.contains(*flag))
    .map(|(_, bit)| bit)
    .sum::<u32>()
        + [
            (KeyEventState::CAPS_LOCK, 64),
            (KeyEventState::NUM_LOCK, 128),
        ]
        .into_iter()
        .filter(|(flag, _)| event.state.contains(*flag))
        .map(|(_, bit)| bit)
        .sum::<u32>();
    let mut out = format!("\x1b[{code}");
    if bits > 0 || kind > 1 {
        let _ = write!(out, ";{}", bits + 1);
    }
    if kind > 1 {
        let _ = write!(out, ":{kind}");
    }
    out.push('u');
    out.into_bytes()
}

fn modifier_key(modifier: ModifierKeyCode) -> Option<Key> {
    Some(match modifier {
        ModifierKeyCode::LeftShift => Key::ShiftLeft,
        ModifierKeyCode::RightShift => Key::ShiftRight,
        ModifierKeyCode::LeftControl => Key::ControlLeft,
        ModifierKeyCode::RightControl => Key::ControlRight,
        ModifierKeyCode::LeftAlt => Key::AltLeft,
        ModifierKeyCode::RightAlt => Key::AltRight,
        ModifierKeyCode::LeftSuper => Key::MetaLeft,
        ModifierKeyCode::RightSuper => Key::MetaRight,
        _ => return None,
    })
}

fn mods(modifiers: KeyModifiers) -> Mods {
    [
        (KeyModifiers::SHIFT, Mods::SHIFT),
        (KeyModifiers::CONTROL, Mods::CTRL),
        (KeyModifiers::ALT, Mods::ALT),
        (KeyModifiers::SUPER, Mods::SUPER),
    ]
    .into_iter()
    .filter(|(from, _)| modifiers.contains(*from))
    .fold(Mods::empty(), |out, (_, to)| out | to)
}

fn action(kind: KeyEventKind) -> key::Action {
    match kind {
        KeyEventKind::Press => key::Action::Press,
        KeyEventKind::Repeat => key::Action::Repeat,
        KeyEventKind::Release => key::Action::Release,
    }
}

fn mouse_button(button: MouseButton) -> Button {
    match button {
        MouseButton::Left => Button::Left,
        MouseButton::Right => Button::Right,
        MouseButton::Middle => Button::Middle,
    }
}

fn fallback(event: &KeyEvent) -> Vec<u8> {
    match (event.kind, event.code) {
        (KeyEventKind::Release, _) => Vec::new(),
        (_, KeyCode::Char(ch)) => ch.to_string().into_bytes(),
        _ => Vec::new(),
    }
}
