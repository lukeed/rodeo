use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::{Ctx, Hits};
use crate::agents::State;

pub fn render(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    let h = content.height;
    let split = h >= 7;
    let spaces_rows = if split {
        h.div_ceil(2)
    } else {
        h.saturating_sub(1)
    };
    let indicators = ctx.config.ui.status_indicators;
    for (i, space) in ctx.snapshot.spaces.iter().enumerate() {
        let Ok(offset) = u16::try_from(i) else {
            break;
        };
        if offset >= spaces_rows {
            break;
        }
        let y = content.y + offset;
        let row = Rect::new(content.x, y, content.width, 1);
        let (fg, bg) = if space.selected {
            let bg = if p.selection_bg == Color::Reset {
                p.active_row_bg
            } else {
                p.selection_bg
            };
            (p.overlay1, Some(bg))
        } else if space.focused {
            (p.text, Some(p.active_row_bg))
        } else {
            (p.overlay0, None)
        };
        if let Some(bg) = bg {
            buf.set_style(row, Style::new().bg(bg));
        }
        buf.set_stringn(row.x, y, format!("{:<2}", i + 1), 2, Style::new().fg(fg));
        let state = space.state.unwrap_or(State::Unknown);
        if content.width >= 3 {
            buf.set_string(
                row.x + 2,
                y,
                state.icon(indicators),
                Style::new().fg(state.color(p)),
            );
        }
        hits.spaces.push((row, space.key.clone()));
    }
    if split {
        let divider = content.y + spaces_rows;
        buf.set_stringn(
            content.x,
            divider,
            "─".repeat(usize::from(content.width)),
            usize::from(content.width),
            Style::new().fg(p.surface_dim),
        );
        let last = content.bottom().saturating_sub(1);
        for (i, agent) in ctx.snapshot.ordered_agents().into_iter().enumerate() {
            let Ok(offset) = u16::try_from(i) else {
                break;
            };
            let y = divider + 1 + offset;
            if y >= last {
                break;
            }
            let row = Rect::new(content.x, y, content.width, 1);
            if agent.focused {
                buf.set_style(row, Style::new().bg(p.active_row_bg));
            }
            let fg = if agent.focused { p.text } else { p.overlay0 };
            buf.set_stringn(row.x, y, format!("{:<2}", i + 1), 2, Style::new().fg(fg));
            if content.width >= 3 {
                buf.set_string(
                    row.x + 2,
                    y,
                    agent.state.icon(indicators),
                    Style::new().fg(agent.state.color(p)),
                );
            }
            hits.agents.push((row, agent.pane.clone()));
        }
    }
    let toggle = Rect::new(content.x + 1, area.bottom() - 1, 1, 1);
    let style = if ctx.snapshot.menu_attention {
        Style::new().fg(p.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(p.overlay0)
    };
    buf[(toggle.x, toggle.y)].set_symbol("»").set_style(style);
    hits.toggle = Some(toggle);
}
