use config::{ClipboardToastPosition, ToastPosition};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::app::{App, Feedback, Notice, NoticeDot};
use crate::hits::Hits;
use crate::overlay::frame::{contrast, panel};
use crate::text::{truncate, width};

pub fn render(app: &App, buf: &mut Buffer, area: Rect, hits: &mut Hits) {
    let (config, banner) = banner(app, buf, area);
    let header = app.regions.header.map_or(0, |h| h.height);
    let lifecycle = super::lifecycle::render(app, buf, area, config + header);
    let offset = config + u16::from(lifecycle.is_some());
    let card = app
        .notice
        .as_ref()
        .and_then(|notice| card(app, buf, area, notice, offset));
    hits.toast = card;
    let copied = app
        .feedback
        .as_ref()
        .and_then(|feedback| copied(app, buf, area, feedback, offset, card));
    hits.covers
        .extend([banner, lifecycle, card, copied].into_iter().flatten());
}

fn banner(app: &App, buf: &mut Buffer, area: Rect) -> (u16, Option<Rect>) {
    let place = if app.regions.mobile {
        app.regions.surface
    } else {
        area
    };
    let Some(text) = app.banner().filter(|_| place.height > 0) else {
        return (0, None);
    };
    let text = truncate(&format!(" {text} "), usize::from(place.width));
    let style = Style::new()
        .fg(contrast(&app.palette))
        .bg(app.palette.yellow)
        .add_modifier(Modifier::BOLD);
    let wide = width(&text);
    buf.set_string(place.right() - wide, place.y, text, style);
    (1, Some(Rect::new(place.right() - wide, place.y, wide, 1)))
}

fn dot_color(app: &App, notice: &Notice) -> Color {
    match notice.dot {
        NoticeDot::Red => app.palette.red,
        NoticeDot::Yellow => app.palette.yellow,
        NoticeDot::Blue => app.palette.blue,
        NoticeDot::Accent => app.palette.accent,
    }
}

pub fn short_title(notice: &Notice) -> String {
    let short = match notice.dot {
        NoticeDot::Red => notice
            .title
            .strip_suffix(" needs attention")
            .map(|agent| format!("{agent} waiting")),
        NoticeDot::Blue => notice
            .title
            .strip_suffix(" finished")
            .map(|agent| format!("{agent} done")),
        NoticeDot::Yellow | NoticeDot::Accent => None,
    };
    short.unwrap_or_else(|| notice.title.clone())
}

fn banner_row(
    app: &App,
    buf: &mut Buffer,
    area: Rect,
    notice: &Notice,
    offset: u16,
) -> Option<Rect> {
    if area.is_empty() {
        return None;
    }
    let palette = &app.palette;
    let y = area.y + area.height.saturating_sub(1 + offset.min(1));
    let rect = Rect::new(area.x, y, area.width, 1);
    let bg = Style::new().bg(palette.surface0);
    buf.set_style(rect, bg);
    for x in rect.left()..rect.right() {
        buf[(x, y)].set_symbol(" ");
    }
    let room = usize::from(area.width);
    let mut x = area.x;
    let mut put = |buf: &mut Buffer, text: &str, style: Style| {
        let used = usize::from(x - area.x);
        if used < room {
            let (end, _) = buf.set_stringn(x, y, text, room - used, style);
            x = end;
        }
    };
    put(buf, " ", bg);
    put(buf, "●", bg.fg(dot_color(app, notice)));
    put(buf, " ", bg);
    put(
        buf,
        &short_title(notice),
        bg.fg(palette.text).add_modifier(Modifier::BOLD),
    );
    if !notice.body.is_empty() {
        put(buf, " · ", bg.fg(palette.overlay0));
        put(buf, &notice.body, bg.fg(palette.overlay0));
    }
    Some(rect)
}

fn card(app: &App, buf: &mut Buffer, area: Rect, notice: &Notice, offset: u16) -> Option<Rect> {
    if app.regions.mobile {
        return banner_row(app, buf, area, notice, offset);
    }
    let palette = &app.palette;
    let body = Some(notice.body.as_str()).filter(|b| !b.is_empty());
    let card_width = (width(&notice.title).max(body.map_or(0, width)) + 6).min(area.width);
    let card_height: u16 = if body.is_some() { 4 } else { 3 };
    let position = if notice.target.is_some() {
        app.config.ui.toast.herdr.position
    } else {
        ToastPosition::TopRight
    };
    let x = match position {
        ToastPosition::TopLeft | ToastPosition::BottomLeft => area.x,
        ToastPosition::TopRight | ToastPosition::BottomRight => {
            area.right().saturating_sub(card_width)
        }
    };
    let card_height = card_height.min(area.height);
    let lowest = area.bottom().saturating_sub(card_height).max(area.y);
    let y = match position {
        ToastPosition::TopLeft | ToastPosition::TopRight => area.y + offset,
        ToastPosition::BottomLeft | ToastPosition::BottomRight => {
            area.bottom().saturating_sub(card_height + offset)
        }
    }
    .clamp(area.y, lowest);
    let rect = Rect::new(x, y, card_width, card_height);
    let inner = panel(buf, rect, palette.overlay0, palette.panel_bg)?;
    let dot = dot_color(app, notice);
    buf.set_string(
        inner.x,
        inner.y,
        "●",
        Style::new().fg(dot).bg(palette.panel_bg),
    );
    let room = usize::from(inner.width.saturating_sub(2));
    let title = Style::new()
        .fg(palette.text)
        .bg(palette.panel_bg)
        .add_modifier(Modifier::BOLD);
    buf.set_stringn(inner.x + 2, inner.y, &notice.title, room, title);
    if let Some(body) = body.filter(|_| inner.height > 1) {
        let style = Style::new().fg(palette.overlay0).bg(palette.panel_bg);
        buf.set_stringn(inner.x + 2, inner.y + 1, body, room, style);
    }
    Some(rect)
}

fn copied(
    app: &App,
    buf: &mut Buffer,
    area: Rect,
    feedback: &Feedback,
    offset: u16,
    card: Option<Rect>,
) -> Option<Rect> {
    let palette = &app.palette;
    let surface = if app.regions.mobile {
        area
    } else {
        app.regions.surface
    };
    let toast_width = (width(&feedback.text) + 4).min(surface.width);
    if toast_width < 4 || surface.height < 3 {
        return None;
    }
    let position = app.config.ui.toast.clipboard.position;
    let x = match position {
        ClipboardToastPosition::TopLeft | ClipboardToastPosition::BottomLeft => surface.x,
        ClipboardToastPosition::TopCenter | ClipboardToastPosition::BottomCenter => {
            surface.x + (surface.width - toast_width) / 2
        }
        ClipboardToastPosition::TopRight | ClipboardToastPosition::BottomRight => {
            surface.right() - toast_width
        }
    };
    let top = matches!(
        position,
        ClipboardToastPosition::TopLeft
            | ClipboardToastPosition::TopCenter
            | ClipboardToastPosition::TopRight
    );
    let mut rect = Rect::new(
        x,
        if top {
            surface.y + offset
        } else {
            surface.bottom() - 3
        },
        toast_width,
        3,
    );
    if let Some(card) = card.filter(|c| c.intersects(rect)) {
        rect.y = if top {
            card.bottom()
        } else {
            card.y.saturating_sub(3)
        };
    }
    let inner = panel(buf, rect, palette.green, palette.panel_bg)?;
    buf.set_string(
        inner.x,
        inner.y,
        "●",
        Style::new().fg(palette.green).bg(palette.panel_bg),
    );
    let style = Style::new()
        .fg(palette.text)
        .bg(palette.panel_bg)
        .add_modifier(Modifier::BOLD);
    let room = usize::from(inner.width.saturating_sub(2));
    buf.set_stringn(inner.x + 2, inner.y, &feedback.text, room, style);
    Some(rect)
}
