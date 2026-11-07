use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::agents::State;
use crate::app::App;
use crate::model::Snapshot;
use crate::text::{truncate, width};

const SWITCH: u16 = 10;

pub fn header(app: &App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect) {
    let p = &app.palette;
    buf.set_style(area, Style::new().bg(p.panel_bg));
    if area.height == 0 || area.width <= SWITCH + 4 {
        return;
    }
    let button = Rect::new(area.right() - SWITCH, area.y, SWITCH, area.height);
    buf.set_style(button, Style::new().bg(p.surface0));
    for y in button.top()..button.bottom() {
        buf[(button.x, y)]
            .set_symbol("│")
            .set_style(Style::new().fg(p.surface_dim).bg(p.surface0));
    }
    buf.set_string(
        button.x + 2,
        area.y,
        "switch",
        Style::new().fg(p.text).bg(p.surface0),
    );
    if snapshot.agents.iter().any(|a| a.state == State::Blocked) {
        buf.set_string(
            button.right() - 1,
            area.y,
            "●",
            Style::new().fg(p.red).bg(p.surface0),
        );
    }
    let room = button.x - area.x;
    let indicators = app.config.ui.status_indicators;
    let left = match snapshot.active() {
        Some(space) => {
            let state = space.state.unwrap_or(State::Unknown);
            buf.set_string(
                area.x + 1,
                area.y,
                state.icon(indicators),
                Style::new().fg(state.color(p)),
            );
            space.name.clone()
        }
        None => "no workspace".to_owned(),
    };
    let tab = snapshot
        .tabs
        .iter()
        .position(|t| t.focused)
        .map(|i| {
            let label = &snapshot.tabs[i].label;
            if snapshot.tabs.len() > 1 {
                format!("tab {label} · {}/{}", i + 1, snapshot.tabs.len())
            } else {
                format!("tab {label}")
            }
        })
        .unwrap_or_default();
    let tab_width = width(&tab);
    let name_room = usize::from(room.saturating_sub(tab_width + 5));
    buf.set_string(
        area.x + 3,
        area.y,
        truncate(&left, name_room),
        Style::new().fg(p.text).add_modifier(Modifier::BOLD),
    );
    if tab_width + 4 < room {
        buf.set_string(
            button.x - tab_width - 1,
            area.y,
            tab,
            Style::new().fg(p.overlay1),
        );
    }
    if area.height < 2 {
        return;
    }
    let y = area.y + 1;
    if snapshot.agents.is_empty() {
        buf.set_string(area.x, y, " no agents", Style::new().fg(p.overlay0));
        return;
    }
    let count = |s: State| snapshot.agents.iter().filter(|a| a.state == s).count();
    if snapshot
        .agents
        .iter()
        .all(|a| matches!(a.state, State::Idle | State::Unknown))
    {
        buf.set_string(area.x, y, " all idle", Style::new().fg(p.overlay0));
        return;
    }
    let mut x = area.x + 1;
    let mut first = true;
    for state in [State::Blocked, State::Done, State::Working, State::Idle] {
        let n = count(state);
        if n == 0 {
            continue;
        }
        if !first {
            buf.set_string(x, y, " · ", Style::new().fg(p.overlay0));
            x += 3;
        }
        let glyph = match (state, indicators) {
            (State::Blocked, config::StatusIndicators::Dots) => "◉",
            _ => state.icon(indicators),
        };
        let color = if state == State::Done {
            p.blue
        } else {
            state.color(p)
        };
        let mut style = Style::new().fg(color);
        if first {
            style = style.add_modifier(Modifier::BOLD);
        }
        let text = format!("{glyph} {n} {}", state.text());
        if x + width(&text) >= button.x {
            break;
        }
        buf.set_string(x, y, &text, style);
        x += width(&text);
        first = false;
    }
}
