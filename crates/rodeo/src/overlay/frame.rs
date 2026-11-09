use config::Palette;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

pub fn contrast(palette: &Palette) -> Color {
    match palette.panel_bg {
        Color::Reset => palette.surface_dim,
        color => color,
    }
}

pub fn popup(area: Rect, width: u16, height: u16) -> Option<Rect> {
    let width = width.min(area.width.saturating_sub(4));
    let height = height.min(area.height.saturating_sub(2));
    (width >= 4 && height >= 4).then(|| {
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        )
    })
}

pub fn dim(buf: &mut Buffer, area: Rect) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_style(cell.style().add_modifier(Modifier::DIM));
        }
    }
}

pub fn panel(buf: &mut Buffer, rect: Rect, border: Color, bg: Color) -> Option<Rect> {
    if rect.width < 2 || rect.height < 2 {
        return None;
    }
    let fill = Style::new().bg(bg).remove_modifier(Modifier::DIM);
    let line = Style::new()
        .fg(border)
        .bg(bg)
        .remove_modifier(Modifier::DIM);
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_symbol(" ").set_style(fill);
        }
    }
    let (right, bottom) = (rect.right() - 1, rect.bottom() - 1);
    for x in rect.left()..=right {
        let (top, low) = match x {
            x if x == rect.x => ("┌", "└"),
            x if x == right => ("┐", "┘"),
            _ => ("─", "─"),
        };
        buf[(x, rect.y)].set_symbol(top).set_style(line);
        buf[(x, bottom)].set_symbol(low).set_style(line);
    }
    for y in rect.y + 1..bottom {
        buf[(rect.x, y)].set_symbol("│").set_style(line);
        buf[(right, y)].set_symbol("│").set_style(line);
    }
    Some(Rect::new(
        rect.x + 1,
        rect.y + 1,
        rect.width - 2,
        rect.height - 2,
    ))
}

pub fn text(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) {
    buf.set_stringn(x, y, text, usize::from(width), style);
}

pub fn button(buf: &mut Buffer, rect: Rect, label: &str, style: Style) {
    buf.set_style(rect, style);
    let width = u16::try_from(label.width())
        .unwrap_or(u16::MAX)
        .min(rect.width);
    text(
        buf,
        rect.x + (rect.width - width) / 2,
        rect.y,
        width,
        label,
        style,
    );
}

pub fn row(inner: Rect, widths: &[u16], gap: u16, offset: u16) -> Vec<Rect> {
    let gaps = gap * u16::try_from(widths.len().saturating_sub(1)).unwrap_or(0);
    let total = widths.iter().sum::<u16>() + gaps;
    let mut x = inner.x + inner.width.saturating_sub(total) / 2;
    let y = inner.y + offset.min(inner.height.saturating_sub(1));
    widths
        .iter()
        .map(|width| {
            let rect = Rect::new(x, y, (*width).min(inner.right().saturating_sub(x)), 1);
            x += width + gap;
            rect
        })
        .collect()
}
