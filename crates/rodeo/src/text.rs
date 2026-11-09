use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub fn width(text: &str) -> u16 {
    u16::try_from(text.width()).unwrap_or(u16::MAX)
}

pub fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let w = grapheme.width();
        if used + w + 1 > width {
            break;
        }
        out.push_str(grapheme);
        used += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_ends_with_an_ellipsis() {
        assert_eq!(truncate("workspace", 9), "workspace");
        assert_eq!(truncate("workspace", 5), "work…");
        assert_eq!(truncate("workspace", 1), "…");
        assert_eq!(truncate("界界界", 4), "界…");
    }
}
