use config::{ClipboardToastPosition, ToastPosition};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::{App, NoticeDot};
use crate::hits::Hits;
use crate::overlay::frame::{contrast, panel};
use crate::text::{truncate, width};

pub fn render(app: &App, buf: &mut Buffer, area: Rect, hits: &mut Hits) {
    let p = &app.palette;
    let mobile = app.regions.mobile;
    let banner_area = if mobile { app.regions.surface } else { area };
    let mut offset = 0;
    if let Some(banner) = app.banner()
        && banner_area.height > 0
    {
        let text = truncate(&format!(" {banner} "), usize::from(banner_area.width));
        let x = banner_area.right() - width(&text);
        let style = Style::new()
            .fg(contrast(p))
            .bg(p.yellow)
            .add_modifier(Modifier::BOLD);
        buf.set_string(x, banner_area.y, text, style);
        offset = 1;
    }
    let mut card = None;
    if let Some(notice) = &app.notice {
        let body = (!notice.body.is_empty()).then_some(notice.body.as_str());
        let w = (width(&notice.title).max(body.map_or(0, width)) + 6).min(area.width);
        let h: u16 = if body.is_some() { 4 } else { 3 };
        let position = if notice.target.is_some() {
            app.config.ui.toast.herdr.position
        } else {
            ToastPosition::TopRight
        };
        let x = match position {
            ToastPosition::TopLeft | ToastPosition::BottomLeft => area.x,
            ToastPosition::TopRight | ToastPosition::BottomRight => area.right().saturating_sub(w),
        };
        let y = match position {
            ToastPosition::TopLeft | ToastPosition::TopRight => area.y + offset,
            ToastPosition::BottomLeft | ToastPosition::BottomRight => {
                area.bottom().saturating_sub(h)
            }
        };
        let rect = Rect::new(x, y, w, h.min(area.height));
        if let Some(inner) = panel(buf, rect, p.overlay0, p.panel_bg) {
            let dot = match notice.dot {
                NoticeDot::Red => p.red,
                NoticeDot::Yellow => p.yellow,
                NoticeDot::Blue => p.blue,
                NoticeDot::Accent => p.accent,
            };
            buf.set_string(inner.x, inner.y, "●", Style::new().fg(dot).bg(p.panel_bg));
            let room = usize::from(inner.width.saturating_sub(2));
            buf.set_stringn(
                inner.x + 2,
                inner.y,
                &notice.title,
                room,
                Style::new()
                    .fg(p.text)
                    .bg(p.panel_bg)
                    .add_modifier(Modifier::BOLD),
            );
            if let Some(body) = body
                && inner.height > 1
            {
                buf.set_stringn(
                    inner.x + 2,
                    inner.y + 1,
                    body,
                    room,
                    Style::new().fg(p.overlay0).bg(p.panel_bg),
                );
            }
            hits.toast = Some(rect);
            card = Some(rect);
        }
    }
    if let Some(feedback) = &app.feedback {
        let surface = if mobile { area } else { app.regions.surface };
        let w = (width(&feedback.text) + 6).min(surface.width);
        let h = 3u16.min(surface.height);
        if w < 4 || h < 3 {
            return;
        }
        let position = app.config.ui.toast.clipboard.position;
        let x = match position {
            ClipboardToastPosition::TopLeft | ClipboardToastPosition::BottomLeft => surface.x,
            ClipboardToastPosition::TopCenter | ClipboardToastPosition::BottomCenter => {
                surface.x + (surface.width - w) / 2
            }
            ClipboardToastPosition::TopRight | ClipboardToastPosition::BottomRight => {
                surface.right() - w
            }
        };
        let top = matches!(
            position,
            ClipboardToastPosition::TopLeft
                | ClipboardToastPosition::TopCenter
                | ClipboardToastPosition::TopRight
        );
        let mut y = if top {
            surface.y + offset
        } else {
            surface.bottom() - h
        };
        let mut rect = Rect::new(x, y, w, h);
        if let Some(card) = card
            && card.intersects(rect)
        {
            y = if top {
                card.bottom()
            } else {
                card.y.saturating_sub(h)
            };
            rect.y = y;
        }
        if let Some(inner) = panel(buf, rect, p.green, p.panel_bg) {
            buf.set_string(
                inner.x + 1,
                inner.y,
                "●",
                Style::new().fg(p.green).bg(p.panel_bg),
            );
            buf.set_stringn(
                inner.x + 3,
                inner.y,
                &feedback.text,
                usize::from(inner.width.saturating_sub(3)),
                Style::new()
                    .fg(p.text)
                    .bg(p.panel_bg)
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
}
