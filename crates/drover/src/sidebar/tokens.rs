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

    fn fixed(&self) -> usize {
        if self.kind == Kind::Text {
            0
        } else {
            self.width()
        }
    }

    fn flexible(&self) -> usize {
        if self.kind == Kind::Text {
            self.width()
        } else {
            0
        }
    }
}

fn separator(before: Kind, after: Kind) -> &'static str {
    if before == Kind::Icon || after == Kind::Git {
        " "
    } else {
        " · "
    }
}

fn separators(spans: &[Span], shown: &[usize]) -> usize {
    shown
        .windows(2)
        .map(|w| separator(spans[w[0]].kind, spans[w[1]].kind).width())
        .sum()
}

fn minimum(spans: &[Span], shown: &[usize]) -> usize {
    shown
        .iter()
        .map(|&i| spans[i].fixed() + usize::from(spans[i].flexible() > 0))
        .sum::<usize>()
        + separators(spans, shown)
}

fn active(spans: &[Span], width: usize) -> Vec<usize> {
    let all: Vec<usize> = (0..spans.len()).collect();
    if minimum(spans, &all) <= width {
        return all;
    }
    let mut keep: Vec<bool> = spans.iter().map(|s| s.flexible() == 0).collect();
    for i in (0..spans.len()).rev() {
        if spans[i].flexible() == 0 {
            continue;
        }
        keep[i] = true;
        let shown: Vec<usize> = (0..spans.len()).filter(|&j| keep[j]).collect();
        if minimum(spans, &shown) > width {
            keep[i] = false;
        }
    }
    (0..spans.len()).filter(|&j| keep[j]).collect()
}

pub fn fit(spans: &[Span], width: u16, sep_style: Style) -> Vec<(String, Style)> {
    let width = usize::from(width);
    let shown = active(spans, width);
    let fixed: usize = shown.iter().map(|&i| spans[i].fixed()).sum();
    let mut budgets: Vec<usize> = spans
        .iter()
        .enumerate()
        .map(|(i, s)| usize::from(shown.contains(&i) && s.flexible() > 0))
        .collect();
    let mut left = width
        .saturating_sub(separators(spans, &shown) + fixed)
        .saturating_sub(budgets.iter().sum());
    while left > 0 {
        let mut grew = false;
        for (budget, span) in budgets.iter_mut().zip(spans) {
            if *budget > 0 && *budget < span.flexible() {
                *budget += 1;
                left -= 1;
                grew = true;
                if left == 0 {
                    break;
                }
            }
        }
        if !grew {
            break;
        }
    }
    let mut out = Vec::new();
    for (n, &i) in shown.iter().enumerate() {
        if n > 0 {
            let before = spans[shown[n - 1]].kind;
            out.push((separator(before, spans[i].kind).to_owned(), sep_style));
        }
        match spans[i].kind {
            Kind::Text => {
                let (text, style) = &spans[i].parts[0];
                out.push((truncate(text, budgets[i]), *style));
            }
            Kind::Icon | Kind::Git => out.extend(spans[i].parts.iter().cloned()),
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
        assert_eq!(joined(&fit(&spans, 7, style)), "● … · …");
        assert_eq!(joined(&fit(&spans, 6, style)), "● tab");
        assert_eq!(joined(&fit(&spans, 3, style)), "● …");
        assert_eq!(joined(&fit(&spans, 2, style)), "●");
        let git = vec![
            Span::text("main", style),
            Span::git(vec![("↑2".into(), style)]),
        ];
        assert_eq!(joined(&fit(&git, 20, style)), "main ↑2");
        assert_eq!(joined(&fit(&git, 4, style)), "… ↑2");
        assert_eq!(joined(&fit(&git, 3, style)), "↑2");
    }

    #[test]
    fn spare_width_is_shared_round_robin() {
        let style = Style::new();
        let spans = vec![Span::text("aaaaaa", style), Span::text("bbbbbb", style)];
        assert_eq!(joined(&fit(&spans, 11, style)), "aaa… · bbb…");
        assert_eq!(joined(&fit(&spans, 15, style)), "aaaaaa · bbbbbb");
    }
}
