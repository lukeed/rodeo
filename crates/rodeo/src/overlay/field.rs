use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
    text: String,
    cursor: usize,
    replace_on_type: bool,
    killed: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Other,
}

fn class(grapheme: &str) -> Class {
    let ch = grapheme.chars().next().unwrap_or(' ');
    if ch.is_whitespace() {
        Class::Space
    } else if ch.is_alphanumeric() || ch == '_' {
        Class::Word
    } else {
        Class::Other
    }
}

impl TextField {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            cursor: text.len(),
            text,
            replace_on_type: false,
            killed: String::new(),
        }
    }

    pub fn suggested(text: impl Into<String>) -> Self {
        Self {
            replace_on_type: true,
            ..Self::new(text)
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.replace_on_type = false;
    }

    pub fn insert(&mut self, text: &str) {
        let clean: String = text
            .chars()
            .filter_map(|ch| match ch {
                '\r' | '\n' | '\t' => Some(' '),
                ch if ch.is_control() => None,
                ch => Some(ch),
            })
            .collect();
        if clean.is_empty() {
            return;
        }
        if self.replace_on_type {
            self.clear();
        }
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }

    fn prev(&self, at: usize) -> usize {
        self.text[..at]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self, at: usize) -> usize {
        self.text[at..]
            .graphemes(true)
            .next()
            .map_or(at, |g| at + g.len())
    }

    fn word_left(&self, at: usize) -> usize {
        let graphemes: Vec<(usize, &str)> = self.text[..at].grapheme_indices(true).collect();
        let mut i = graphemes.len();
        while i > 0 && class(graphemes[i - 1].1) == Class::Space {
            i -= 1;
        }
        if i == 0 {
            return 0;
        }
        let kind = class(graphemes[i - 1].1);
        while i > 0 && class(graphemes[i - 1].1) == kind {
            i -= 1;
        }
        graphemes.get(i).map_or(at, |(pos, _)| *pos)
    }

    fn word_right(&self, at: usize) -> usize {
        let mut pos = at;
        let mut graphemes = self.text[at..].graphemes(true).peekable();
        while graphemes.peek().is_some_and(|g| class(g) == Class::Space) {
            pos += graphemes.next().map_or(0, str::len);
        }
        let Some(kind) = graphemes.peek().map(|g| class(g)) else {
            return pos;
        };
        while graphemes.peek().is_some_and(|g| class(g) == kind) {
            pos += graphemes.next().map_or(0, str::len);
        }
        pos
    }

    fn kill(&mut self, from: usize, to: usize) {
        if from >= to {
            return;
        }
        self.killed = self.text[from..to].to_owned();
        self.text.replace_range(from..to, "");
        self.cursor = from;
    }

    pub fn key(&mut self, key: &KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let moved = |field: &mut Self| {
            field.replace_on_type = false;
            true
        };
        match key.code {
            KeyCode::Left => {
                self.cursor = self.prev(self.cursor);
                moved(self)
            }
            KeyCode::Right => {
                self.cursor = self.next(self.cursor);
                moved(self)
            }
            KeyCode::Home => {
                self.cursor = 0;
                moved(self)
            }
            KeyCode::End => {
                self.cursor = self.text.len();
                moved(self)
            }
            KeyCode::Backspace if ctrl || alt => {
                let from = self.word_left(self.cursor);
                self.kill(from, self.cursor);
                true
            }
            KeyCode::Backspace => {
                if self.replace_on_type {
                    self.clear();
                } else {
                    let from = self.prev(self.cursor);
                    self.text.replace_range(from..self.cursor, "");
                    self.cursor = from;
                }
                true
            }
            KeyCode::Delete => {
                let to = self.next(self.cursor);
                self.text.replace_range(self.cursor..to, "");
                self.replace_on_type = false;
                true
            }
            KeyCode::Char(ch) if ctrl => self.control(ch),
            KeyCode::Char(ch) if alt => match ch {
                'b' => {
                    self.cursor = self.word_left(self.cursor);
                    moved(self)
                }
                'f' => {
                    self.cursor = self.word_right(self.cursor);
                    moved(self)
                }
                'd' => {
                    let to = self.word_right(self.cursor);
                    self.kill(self.cursor, to);
                    true
                }
                _ => false,
            },
            KeyCode::Char(ch) if key.modifiers.difference(KeyModifiers::SHIFT).is_empty() => {
                self.insert(ch.encode_utf8(&mut [0; 4]));
                true
            }
            _ => false,
        }
    }

    fn control(&mut self, ch: char) -> bool {
        match ch {
            'b' => {
                self.cursor = self.prev(self.cursor);
                self.replace_on_type = false;
            }
            'f' => {
                self.cursor = self.next(self.cursor);
                self.replace_on_type = false;
            }
            'a' => {
                self.cursor = 0;
                self.replace_on_type = false;
            }
            'e' => {
                self.cursor = self.text.len();
                self.replace_on_type = false;
            }
            'h' => {
                return self.key(&KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }
            'd' => return self.key(&KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)),
            'u' => self.kill(0, self.cursor),
            'k' => self.kill(self.cursor, self.text.len()),
            'w' => {
                let from = self.word_left(self.cursor);
                self.kill(from, self.cursor);
            }
            'y' => {
                let killed = self.killed.clone();
                self.replace_on_type = false;
                self.insert(&killed);
            }
            _ => return false,
        }
        true
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, style: Style) -> Option<(u16, u16)> {
        if area.width == 0 || area.height == 0 {
            return None;
        }
        buf.set_style(area, style);
        let before = &self.text[..self.cursor];
        let budget = usize::from(area.width.saturating_sub(1));
        let mut start = 0;
        while before[start..].width() > budget {
            start = self.next(start);
        }
        let shown = &self.text[start..];
        buf.set_stringn(area.x, area.y, shown, usize::from(area.width), style);
        let x = area.x + u16::try_from(before[start..].width()).unwrap_or(0);
        Some((x.min(area.right().saturating_sub(1)), area.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(field: &mut TextField, code: KeyCode, modifiers: KeyModifiers) {
        field.key(&KeyEvent::new(code, modifiers));
    }

    fn typed(field: &mut TextField, text: &str) {
        for ch in text.chars() {
            press(field, KeyCode::Char(ch), KeyModifiers::NONE);
        }
    }

    #[test]
    fn suggested_text_is_replaced_by_typing() {
        let mut field = TextField::suggested("3");
        typed(&mut field, "logs");
        assert_eq!(field.text(), "logs");
        let mut field = TextField::suggested("3");
        press(&mut field, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(field.text(), "");
    }

    #[test]
    fn editing_keys_follow_herdr() {
        let mut field = TextField::new("hello big world");
        press(&mut field, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(field.text(), "hello big ");
        press(&mut field, KeyCode::Char('b'), KeyModifiers::ALT);
        assert_eq!(field.cursor(), 6);
        press(&mut field, KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(field.text(), "hello ");
        press(&mut field, KeyCode::Char('y'), KeyModifiers::CONTROL);
        assert_eq!(field.text(), "hello big ");
        press(&mut field, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut field, KeyCode::Char('d'), KeyModifiers::ALT);
        assert_eq!(field.text(), " big ");
        field.insert("a\tb\u{7}");
        assert_eq!(field.text(), "a b big ");
    }

    #[test]
    fn rendering_keeps_the_cursor_in_view() {
        let field = TextField::new("abcdefghij");
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
        let cursor = field.render(Rect::new(0, 0, 5, 1), &mut buf, Style::default());
        assert_eq!(cursor, Some((4, 0)));
        assert_eq!(buf[(0, 0)].symbol(), "g");
    }
}
