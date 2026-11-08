mod lifecycle;
pub mod mobile;
mod modebar;
mod mods;
mod panes;
mod tabs;
mod toast;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

use crate::app::App;
use crate::hits::Hits;
use crate::overlay::{self, frame};
use crate::sidebar;

pub use panes::selection_style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub shape: vt::CursorShape,
    pub blinking: bool,
}

pub fn draw(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let snapshot = app.snapshot();
    let mut hits = Hits {
        surface: app.regions.surface,
        ..Hits::default()
    };
    let regions = app.regions;
    let buf = frame.buffer_mut();
    let online = app.conn.is_online();
    if let Some(header) = regions.header {
        mobile::header(app, &snapshot, buf, header);
    }
    hits.sidebar = sidebar::draw(app, &snapshot, buf, regions.sidebar);
    if let Some(tab_bar) = regions.tab_bar {
        tabs::render(app, &snapshot, buf, tab_bar, &mut hits);
    }
    let mut cursor = None;
    if online && snapshot.active_tab.is_some() {
        cursor = panes::render(app, buf, &mut hits);
    } else if !online || app.machines.spaces().is_empty() && !app.machines.local().slot.is_ready() {
        unavailable(app, buf, regions.surface);
    }
    if let Some(popup) = crate::popup::render(app, buf) {
        cursor = Some(popup);
    } else if app.popup.is_some() {
        cursor = None;
    }
    if app.view.overlay.is_none() {
        modebar::render(app, buf, &mut hits);
    }
    toast::render(app, buf, area, &mut hits);
    if app.switcher_open() {
        hits.switcher = crate::switcher::render(app, &snapshot, buf, area);
    }
    if !app.config.ui.mouse_capture {
        hits.clear_chrome();
    }
    if let Some(mut open) = app.view.overlay.take() {
        if open.dims() {
            frame::dim(buf, area);
        }
        let ctx = overlay::Ctx {
            palette: &app.palette,
            keymap: &app.keymap,
            config: &app.config,
            snapshot: &snapshot,
        };
        cursor = open.render(area, buf, &ctx).map(|(x, y)| Cursor {
            x,
            y,
            shape: vt::CursorShape::Bar,
            blinking: true,
        });
        app.view.overlay = Some(open);
    }
    place_cursor(app, frame, cursor);
    let covers = std::mem::take(&mut hits.covers);
    app.hits = hits;
    app.compose_graphics(&covers);
}

fn place_cursor(app: &mut App, frame: &mut Frame, cursor: Option<Cursor>) {
    let drawn = app.config.ui.host_cursor == config::HostCursor::Drawn;
    match cursor {
        Some(cursor) if drawn => {
            let buf = frame.buffer_mut();
            if buf.area.contains(Position::new(cursor.x, cursor.y)) {
                let cell = &mut buf[(cursor.x, cursor.y)];
                cell.set_style(
                    cell.style()
                        .add_modifier(ratatui::style::Modifier::REVERSED),
                );
            }
        }
        Some(cursor) => {
            frame.set_cursor_position(Position::new(cursor.x, cursor.y));
            app.set_cursor_shape(cursor.shape, cursor.blinking);
        }
        None => {}
    }
}

fn unavailable(app: &App, buf: &mut Buffer, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let p = &app.palette;
    buf.set_style(area, Style::new().bg(p.panel_bg));
    let style = Style::new().fg(p.overlay0).bg(p.panel_bg);
    let room = usize::from(area.width.saturating_sub(1));
    let (text, detail) = if app.conn.is_online() {
        match app.machines.local().slot.state() {
            crate::sync::State::Undecodable(why) | crate::sync::State::Unreadable(why) => {
                ("Local: layout unavailable.".to_owned(), Some(why.clone()))
            }
            _ => ("Local: loading the layout.".to_owned(), None),
        }
    } else {
        (
            format!("Local: {}. Select a connected machine.", app.conn.label()),
            app.conn.error.clone(),
        )
    };
    buf.set_stringn(area.x + 1, area.y, text, room, style);
    if let Some(detail) = detail.filter(|_| area.height > 1) {
        buf.set_stringn(area.x + 1, area.y + 1, detail, room, style);
    }
}
