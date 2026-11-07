use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub const fn plain(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let Self {
            mut code,
            mut modifiers,
        } = self;
        if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
            code = KeyCode::BackTab;
        }
        if code == KeyCode::BackTab {
            modifiers.remove(KeyModifiers::SHIFT);
        }
        Self { code, modifiers }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text == "+" {
            return Some(Self::plain(KeyCode::Char('+')));
        }
        if let Some(modifiers) = text.strip_suffix("++") {
            return Some(Self::new(KeyCode::Char('+'), parse_modifiers(modifiers)?));
        }
        let mut modifiers = KeyModifiers::NONE;
        let mut key = None;
        for token in text.split('+').map(str::trim) {
            if token.is_empty() {
                return None;
            }
            if let Some(modifier) = modifier(token) {
                modifiers |= modifier;
            } else if key.replace(token).is_some() {
                return None;
            }
        }
        let code = key_code(key?, &mut modifiers)?;
        Some(Self::new(code, modifiers).normalized())
    }

    pub fn label(self) -> String {
        self.label_for(cfg!(target_os = "macos"))
    }

    pub fn label_for(self, macos: bool) -> String {
        let mut parts = Vec::new();
        for (flag, name) in [
            (KeyModifiers::CONTROL, "ctrl"),
            (KeyModifiers::ALT, "alt"),
            (KeyModifiers::SHIFT, "shift"),
            (KeyModifiers::SUPER, if macos { "cmd" } else { "super" }),
            (KeyModifiers::HYPER, "hyper"),
            (KeyModifiers::META, "meta"),
        ] {
            let backtab_shift = flag == KeyModifiers::SHIFT && self.code == KeyCode::BackTab;
            if self.modifiers.contains(flag) && !backtab_shift {
                parts.push(name.to_string());
            }
        }
        parts.push(key_name(self.code));
        parts.join("+")
    }

    pub fn matches(self, key: KeyPress) -> bool {
        let actual = KeyCombo::new(key.code, key.modifiers).normalized();
        let expected = self.normalized();
        if actual.modifiers == expected.modifiers && codes_match(actual, expected, key.shifted) {
            return true;
        }
        let shifted_variant = actual.modifiers.contains(KeyModifiers::SHIFT)
            && actual.modifiers.difference(KeyModifiers::SHIFT) == expected.modifiers
            && shifted_char_matches(actual.code, key.shifted, expected.code);
        shifted_variant || legacy_uppercase_matches(actual, expected)
    }

    pub fn digit_index(self) -> Option<usize> {
        match self.code {
            KeyCode::Char(digit @ '1'..='9') => Some(digit as usize - '1' as usize),
            _ => None,
        }
    }

    pub fn is_unmodified_printable(self) -> bool {
        matches!(self.code, KeyCode::Char(ch) if !ch.is_control())
            && self.modifiers.difference(KeyModifiers::SHIFT).is_empty()
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub shifted: Option<char>,
    pub text: Option<char>,
}

impl KeyPress {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self {
            code,
            modifiers,
            shifted: None,
            text: None,
        }
    }

    #[must_use]
    pub const fn with_shifted(mut self, shifted: char) -> Self {
        self.shifted = Some(shifted);
        self
    }

    #[must_use]
    pub const fn with_text(mut self, text: char) -> Self {
        self.text = Some(text);
        self
    }

    pub fn typed(self) -> Option<Self> {
        self.text
            .filter(|ch| !ch.is_control())
            .map(|ch| Self::new(KeyCode::Char(ch), KeyModifiers::NONE))
    }
}

impl From<KeyEvent> for KeyPress {
    fn from(event: KeyEvent) -> Self {
        Self::new(event.code, event.modifiers)
    }
}

impl From<&KeyEvent> for KeyPress {
    fn from(event: &KeyEvent) -> Self {
        Self::from(*event)
    }
}

impl From<&KeyPress> for KeyPress {
    fn from(key: &KeyPress) -> Self {
        *key
    }
}

impl From<KeyCombo> for KeyPress {
    fn from(combo: KeyCombo) -> Self {
        Self::new(combo.code, combo.modifiers)
    }
}

pub(crate) fn modifier(token: &str) -> Option<KeyModifiers> {
    Some(match token.to_lowercase().as_str() {
        "ctrl" | "control" => KeyModifiers::CONTROL,
        "shift" => KeyModifiers::SHIFT,
        "alt" | "option" | "meta" => KeyModifiers::ALT,
        "cmd" | "command" | "super" => KeyModifiers::SUPER,
        "hyper" => KeyModifiers::HYPER,
        _ => return None,
    })
}

pub(crate) fn parse_modifiers(text: &str) -> Option<KeyModifiers> {
    let mut modifiers = KeyModifiers::NONE;
    for token in text.split('+').map(str::trim) {
        modifiers |= modifier(token)?;
    }
    (!modifiers.is_empty()).then_some(modifiers)
}

fn key_code(token: &str, modifiers: &mut KeyModifiers) -> Option<KeyCode> {
    let lower = token.to_lowercase();
    let code = match lower.as_str() {
        "space" => KeyCode::Char(' '),
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" if modifiers.contains(KeyModifiers::SHIFT) => {
            modifiers.remove(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "tab" => KeyCode::Tab,
        "backspace" | "bs" => KeyCode::Backspace,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "minus" => KeyCode::Char('-'),
        "comma" => KeyCode::Char(','),
        "period" => KeyCode::Char('.'),
        "slash" => KeyCode::Char('/'),
        "backslash" => KeyCode::Char('\\'),
        "quote" => KeyCode::Char('\''),
        "double_quote" | "double-quote" => KeyCode::Char('"'),
        "semicolon" => KeyCode::Char(';'),
        "colon" => KeyCode::Char(':'),
        "percent" => KeyCode::Char('%'),
        "ampersand" => KeyCode::Char('&'),
        "backtick" => KeyCode::Char('`'),
        "plus" => KeyCode::Char('+'),
        _ => return single_or_function_key(token, &lower, modifiers),
    };
    Some(code)
}

fn single_or_function_key(
    token: &str,
    lower: &str,
    modifiers: &mut KeyModifiers,
) -> Option<KeyCode> {
    let mut chars = token.chars();
    if let (Some(ch), None) = (chars.next(), chars.next()) {
        if ch.is_ascii_uppercase() {
            *modifiers |= KeyModifiers::SHIFT;
            return Some(KeyCode::Char(ch.to_ascii_lowercase()));
        }
        return Some(KeyCode::Char(ch));
    }
    lower.strip_prefix('f')?.parse::<u8>().ok().map(KeyCode::F)
}

fn key_name(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "space".into(),
        KeyCode::Char(ch) => ch.to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => "shift+tab".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::F(n) => format!("f{n}"),
        other => format!("{other:?}").to_lowercase(),
    }
}

fn codes_match(actual: KeyCombo, expected: KeyCombo, shifted: Option<char>) -> bool {
    match (actual.code, expected.code) {
        (KeyCode::Char(a), KeyCode::Char(e))
            if a.is_ascii_alphabetic() && e.is_ascii_alphabetic() =>
        {
            a == e
                || actual.modifiers.contains(KeyModifiers::SHIFT)
                    && expected.modifiers.contains(KeyModifiers::SHIFT)
                    && a.eq_ignore_ascii_case(&e)
        }
        (KeyCode::Char(a), KeyCode::Char(_)) => {
            actual.code == expected.code
                || shifted_char_matches(KeyCode::Char(a), shifted, expected.code)
        }
        (a, e) => a == e,
    }
}

fn shifted_char_matches(actual: KeyCode, shifted: Option<char>, expected: KeyCode) -> bool {
    let KeyCode::Char(expected) = expected else {
        return false;
    };
    match shifted {
        Some(shifted) => shifted == expected,
        None => actual == KeyCode::Char(expected) && is_shifted_punctuation(expected),
    }
}

fn is_shifted_punctuation(ch: char) -> bool {
    "!@#$%^&*()_+{}|:\"<>?~".contains(ch)
}

fn legacy_uppercase_matches(actual: KeyCombo, expected: KeyCombo) -> bool {
    if actual.modifiers.contains(KeyModifiers::SHIFT) {
        return false;
    }
    let (KeyCode::Char(a), KeyCode::Char(e)) = (actual.code, expected.code) else {
        return false;
    };
    a.is_ascii_uppercase()
        && e.is_ascii_lowercase()
        && a.to_ascii_lowercase() == e
        && actual.modifiers | KeyModifiers::SHIFT == expected.modifiers
}

pub(crate) fn shifted_digit(symbol: char) -> Option<char> {
    const SYMBOLS: &str = "!@#$%^&*(";
    SYMBOLS
        .chars()
        .position(|ch| ch == symbol)
        .and_then(|index| char::from_digit(u32::try_from(index).ok()? + 1, 10))
}
