use config::StatusIndicators;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::agents::State;
use crate::app::App;
use crate::model::Snapshot;
use crate::text::{truncate, width};

pub const SWITCH: u16 = 10;

pub fn switch_button(area: Rect) -> Rect {
    let w = SWITCH.min(area.width);
    Rect::new(area.right() - w, area.y, w, area.height)
}

pub fn tab_status(snapshot: &Snapshot) -> String {
    let active = snapshot.tabs.iter().position(|t| t.focused).unwrap_or(0);
    let label = snapshot.tabs.get(active).map_or("1", |t| t.label.as_str());
    if snapshot.tabs.len() <= 1 {
        format!("tab {label}")
    } else {
        format!("tab {label} · {}/{}", active + 1, snapshot.tabs.len())
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, room: u16, text: &str, style: Style) {
    buf.set_stringn(x, y, text, usize::from(room), style);
}

pub fn header(app: &App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect) {
    let p = &app.palette;
    if area.width == 0 || area.height == 0 {
        return;
    }
    buf.set_style(area, Style::new().bg(p.panel_bg));
    let button = switch_button(area);
    let status = Rect::new(
        area.x,
        area.y,
        (button.x - area.x).saturating_sub(1),
        area.height,
    );
    status_line(app, snapshot, buf, status);
    switch(app, snapshot, buf, button);
}

fn status_line(app: &App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect) {
    let p = &app.palette;
    if area.width == 0 {
        return;
    }
    let Some(space) = snapshot.active() else {
        put(
            buf,
            area.x,
            area.y,
            area.width,
            " no workspace",
            Style::new().fg(p.text).bg(p.panel_bg),
        );
        return;
    };
    let indicators = app.config.ui.status_indicators;
    let tab = tab_status(snapshot);
    let tab_width = (width(&tab) + 1).min(area.width);
    let name_width = area.width - tab_width;
    let state = space.state.unwrap_or(State::Unknown);
    put(
        buf,
        area.x,
        area.y,
        name_width.min(3),
        &format!(" {} ", state.icon(indicators)),
        Style::new().fg(state.color(p)).bg(p.panel_bg),
    );
    put(
        buf,
        area.x + 3,
        area.y,
        name_width.saturating_sub(3),
        &truncate(&space.name, usize::from(name_width.saturating_sub(4))),
        Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD),
    );
    put(
        buf,
        area.right() - tab_width + 1,
        area.y,
        tab_width.saturating_sub(1),
        &tab,
        Style::new().fg(p.overlay1).bg(p.panel_bg),
    );
    if area.height > 1 {
        summary(
            app,
            snapshot,
            buf,
            Rect::new(area.x, area.y + 1, area.width, 1),
        );
    }
}

fn switch(app: &App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect) {
    let p = &app.palette;
    if area.width == 0 {
        return;
    }
    buf.set_style(area, Style::new().bg(p.surface0));
    for y in area.top()..area.bottom() {
        put(
            buf,
            area.x,
            y,
            1,
            "│",
            Style::new().fg(p.surface_dim).bg(p.surface0),
        );
    }
    let y = if area.height > 1 { area.y + 1 } else { area.y };
    put(
        buf,
        area.x + 1 + area.width.saturating_sub(7) / 2,
        y,
        area.width - 1,
        "switch",
        Style::new()
            .fg(p.text)
            .bg(p.surface0)
            .add_modifier(Modifier::BOLD),
    );
    if snapshot.agents.iter().any(|a| a.state == State::Blocked) {
        put(
            buf,
            area.right() - 1,
            area.y,
            1,
            State::Blocked.icon(app.config.ui.status_indicators),
            Style::new().fg(p.red).bg(p.surface0),
        );
    }
}

fn summary(app: &App, snapshot: &Snapshot, buf: &mut Buffer, row: Rect) {
    let p = &app.palette;
    let indicators = app.config.ui.status_indicators;
    let states = [State::Blocked, State::Done, State::Working, State::Idle];
    let counts = states.map(|s| (s, snapshot.agents.iter().filter(|a| a.state == s).count()));
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let pending: usize = counts[..3].iter().map(|(_, n)| n).sum();
    let quiet = Style::new().fg(p.overlay1).bg(p.panel_bg);
    if total == 0 {
        put(buf, row.x, row.y, row.width, " no agents", quiet);
        return;
    }
    if pending == 0 {
        put(buf, row.x, row.y, row.width, " all idle", quiet);
        return;
    }
    let mut x = row.x + 1;
    let mut shown = 0;
    let mut omitted = false;
    for (state, n) in counts {
        if n == 0 {
            continue;
        }
        let glyph = match (indicators, state) {
            (StatusIndicators::Dots, State::Blocked) => Some("◉"),
            (StatusIndicators::Dots, State::Done) => Some("●"),
            (StatusIndicators::Dots, _) => None,
            _ => Some(state.icon(indicators)),
        };
        let text = glyph.map_or_else(
            || format!("{n} {}", state.text()),
            |g| format!("{g} {n} {}", state.text()),
        );
        let separator = if shown == 0 { "" } else { " · " };
        if row.right().saturating_sub(x) < width(separator) + width(&text) {
            omitted = true;
            break;
        }
        if !separator.is_empty() {
            put(
                buf,
                x,
                row.y,
                row.right() - x,
                separator,
                Style::new().fg(p.overlay0).bg(p.panel_bg),
            );
            x += width(separator);
        }
        let style = if shown == 0 {
            let fg = if state == State::Done {
                p.blue
            } else {
                state.color(p)
            };
            Style::new()
                .fg(fg)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            quiet
        };
        put(buf, x, row.y, row.right() - x, &text, style);
        x += width(&text);
        shown += 1;
    }
    if omitted && row.right().saturating_sub(x) >= 2 {
        put(
            buf,
            x,
            row.y,
            2,
            " …",
            Style::new().fg(p.overlay0).bg(p.panel_bg),
        );
    }
}
