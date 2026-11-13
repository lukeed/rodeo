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
use ratatui::text::Line;

use crate::app::App;
use crate::brand::{self, Ink, Keep};
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
    app.fit(area);
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
    } else if app.machines.spaces().is_empty() {
        empty(app, buf, regions.surface);
    }
    if let Some(popup) = crate::popup::render(app, buf) {
        cursor = Some(popup);
    } else if app.popup.is_some() {
        cursor = None;
    }
    hits.covers
        .extend(app.popup.as_ref().and_then(|p| p.geometry).map(|g| g.outer));
    if app.view.overlay.is_none() {
        modebar::render(app, buf, &mut hits);
    }
    toast::render(app, buf, area, &mut hits);
    if app.switcher_open() {
        hits.switcher = crate::switcher::render(app, &snapshot, buf, area);
        hits.covers.push(area);
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
        let drawn = open.render(area, buf, &ctx);
        hits.covers.extend(drawn.map(|d| d.area));
        cursor = drawn.and_then(|d| d.cursor).map(|(x, y)| Cursor {
            x,
            y,
            shape: vt::CursorShape::Bar,
            blinking: true,
        });
        app.view.overlay = Some(open);
    }
    place_cursor(app, frame, cursor);
    let covers = hits.covers.clone();
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
    let notes: Vec<Line> = std::iter::once(text)
        .chain(detail)
        .map(|note| Line::styled(note, style))
        .collect();
    let area = clear_of_chrome(app, buf.area, area);
    brand::splash(buf, area, Ink::of(p), &notes, Keep::Always);
}

fn empty(app: &App, buf: &mut Buffer, area: Rect) {
    let keys = app
        .keymap
        .label(config::Action::NewWorkspace)
        .unwrap_or_else(|| "unset".to_owned());
    let hint = Line::styled(
        format!("{keys}  new workspace"),
        Style::new().fg(app.palette.overlay0),
    );
    let area = clear_of_chrome(app, buf.area, area);
    brand::splash(buf, area, Ink::of(&app.palette), &[hint], Keep::Optional);
}

fn clear_of_chrome(app: &App, screen: Rect, area: Rect) -> Rect {
    let header = app.regions.header.map_or(0, |h| h.height);
    let banners = u16::from(app.banner().is_some()) + u16::from(lifecycle::shown(app));
    let top = (screen.y + header + banners).max(area.y);
    let mut bottom = area.bottom();
    if let Some(bar) = modebar::bar(app).filter(|bar| bar.intersects(area)) {
        bottom = bottom.min(bar.y);
    }
    Rect::new(area.x, top, area.width, bottom.saturating_sub(top))
}
