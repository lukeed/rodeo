use ratatui::buffer::Buffer;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use crate::text::truncate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Icon,
    Git,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub kind: Kind,
    pub parts: Vec<(String, Style)>,
}

impl Span {
    pub fn icon(text: &str, style: Style) -> Self {
        Self {
            kind: Kind::Icon,
            parts: vec![(text.to_owned(), style)],
        }
    }

    pub fn git(parts: Vec<(String, Style)>) -> Self {
        Self {
            kind: Kind::Git,
            parts,
        }
    }

    pub fn text(text: &str, style: Style) -> Self {
        Self {
            kind: Kind::Text,
            parts: vec![(text.to_owned(), style)],
        }
    }

    fn width(&self) -> usize {
        self.parts.iter().map(|(t, _)| t.width()).sum()
    }
}

fn separator(before: Kind, after: Kind) -> &'static str {
    if before == Kind::Icon || after == Kind::Git {
        " "
    } else {
        " · "
    }
}

fn seps(spans: &[&Span]) -> usize {
    spans
        .windows(2)
        .map(|w| separator(w[0].kind, w[1].kind).width())
        .sum()
}

pub fn fit(spans: &[Span], width: u16, sep_style: Style) -> Vec<(String, Style)> {
    let width = usize::from(width);
    let fixed: usize = spans
        .iter()
        .filter(|s| s.kind != Kind::Text)
        .map(Span::width)
        .sum();
    let minimum = |shown: &[&Span]| {
        shown
            .iter()
            .map(|s| if s.kind == Kind::Text { 1 } else { s.width() })
            .sum::<usize>()
            + seps(shown)
    };
    let mut keep: Vec<bool> = vec![true; spans.len()];
    let all: Vec<&Span> = spans.iter().collect();
    if minimum(&all) > width {
        for (i, span) in spans.iter().enumerate() {
            keep[i] = span.kind != Kind::Text;
        }
        for i in (0..spans.len()).rev() {
            if spans[i].kind != Kind::Text {
                continue;
            }
            keep[i] = true;
            let shown: Vec<&Span> = spans
                .iter()
                .zip(&keep)
                .filter(|(_, k)| **k)
                .map(|(s, _)| s)
                .collect();
            if minimum(&shown) > width {
                keep[i] = false;
            }
        }
    }
    let shown: Vec<&Span> = spans
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(s, _)| s)
        .collect();
    let texts: Vec<usize> = shown
        .iter()
        .filter(|s| s.kind == Kind::Text)
        .map(|s| s.width())
        .collect();
    let mut given: Vec<usize> = vec![1; texts.len()];
    let shown_fixed: usize = shown
        .iter()
        .filter(|s| s.kind != Kind::Text)
        .map(|s| s.width())
        .sum();
    let mut left = width.saturating_sub(shown_fixed.min(fixed) + seps(&shown) + given.len());
    while left > 0 {
        let mut grew = false;
        for (g, full) in given.iter_mut().zip(&texts) {
            if left > 0 && *g < *full {
                *g += 1;
                left -= 1;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut out = Vec::new();
    let mut texts = given.into_iter();
    for (i, span) in shown.iter().enumerate() {
        if i > 0 {
            out.push((
                separator(shown[i - 1].kind, span.kind).to_owned(),
                sep_style,
            ));
        }
        match span.kind {
            Kind::Text => {
                let budget = texts.next().unwrap_or(1);
                let (text, style) = &span.parts[0];
                out.push((truncate(text, budget), *style));
            }
            _ => out.extend(span.parts.iter().cloned()),
        }
    }
    out
}

pub fn draw(buf: &mut Buffer, x: u16, y: u16, right: u16, parts: &[(String, Style)]) {
    let mut x = x;
    for (text, style) in parts {
        if x >= right {
            break;
        }
        let (nx, _) = buf.set_stringn(x, y, text, usize::from(right - x), *style);
        x = nx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(parts: &[(String, Style)]) -> String {
        parts.iter().map(|(t, _)| t.as_str()).collect()
    }

    #[test]
    fn rows_fit_their_width() {
        let style = Style::new();
        let spans = vec![
            Span::icon("●", style),
            Span::text("workspace", style),
            Span::text("tab", style),
        ];
        assert_eq!(joined(&fit(&spans, 30, style)), "● workspace · tab");
        assert_eq!(joined(&fit(&spans, 12, style)), "● wor… · tab");
        let git = vec![
            Span::text("main", style),
            Span::git(vec![("↑2".into(), style)]),
        ];
        assert_eq!(joined(&fit(&git, 20, style)), "main ↑2");
        assert_eq!(joined(&fit(&spans, 3, style)), "● …");
    }
}
