use config::{ClipboardToastPosition, ToastPosition};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::{App, Feedback, Notice, NoticeDot};
use crate::hits::Hits;
use crate::overlay::frame::{contrast, panel};
use crate::text::{truncate, width};

pub fn render(app: &App, buf: &mut Buffer, area: Rect, hits: &mut Hits) {
    let offset = banner(app, buf, area);
    let card = app
        .notice
        .as_ref()
        .and_then(|notice| card(app, buf, area, notice, offset));
    hits.toast = card;
    if let Some(feedback) = &app.feedback {
        copied(app, buf, area, feedback, offset, card);
    }
}

fn banner(app: &App, buf: &mut Buffer, area: Rect) -> u16 {
    let place = if app.regions.mobile {
        app.regions.surface
    } else {
        area
    };
    let Some(text) = app.banner().filter(|_| place.height > 0) else {
        return 0;
    };
    let text = truncate(&format!(" {text} "), usize::from(place.width));
    let style = Style::new()
        .fg(contrast(&app.palette))
        .bg(app.palette.yellow)
        .add_modifier(Modifier::BOLD);
    buf.set_string(place.right() - width(&text), place.y, text, style);
    1
}

fn card(app: &App, buf: &mut Buffer, area: Rect, notice: &Notice, offset: u16) -> Option<Rect> {
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
    let y = match position {
        ToastPosition::TopLeft | ToastPosition::TopRight => area.y + offset,
        ToastPosition::BottomLeft | ToastPosition::BottomRight => {
            area.bottom().saturating_sub(card_height)
        }
    };
    let rect = Rect::new(x, y, card_width, card_height.min(area.height));
    let inner = panel(buf, rect, palette.overlay0, palette.panel_bg)?;
    let dot = match notice.dot {
        NoticeDot::Red => palette.red,
        NoticeDot::Yellow => palette.yellow,
        NoticeDot::Blue => palette.blue,
        NoticeDot::Accent => palette.accent,
    };
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
) {
    let palette = &app.palette;
    let surface = if app.regions.mobile {
        area
    } else {
        app.regions.surface
    };
    let toast_width = (width(&feedback.text) + 4).min(surface.width);
    if toast_width < 4 || surface.height < 3 {
        return;
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
    let Some(inner) = panel(buf, rect, palette.green, palette.panel_bg) else {
        return;
    };
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
}
