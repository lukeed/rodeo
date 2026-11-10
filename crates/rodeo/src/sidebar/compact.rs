use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::{AgentHit, Ctx, Hits, SpaceHit, compact_sections, ordered_agents, put};
use crate::agents::State;
use crate::brand::{Ink, mini_wordmark};

pub fn render(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    let (top, divider, bottom) = compact_sections(area);
    let rows = header(buf, top, ctx);
    spaces(buf, rows, ctx, hits);
    if let Some(divider) = divider {
        put(
            buf,
            top.x,
            divider,
            top.width,
            &"─".repeat(usize::from(top.width)),
            Style::new().fg(p.surface_dim),
        );
    }
    agents(buf, bottom, ctx, hits);
    if top.width > 0 {
        let toggle = Rect::new(top.x + top.width / 2, area.bottom() - 1, 1, 1);
        let style = if ctx.snapshot.menu_attention {
            Style::new().fg(p.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(p.overlay0)
        };
        put(buf, toggle.x, toggle.y, 1, "»", style);
        hits.toggle = Some(toggle);
    }
}

pub fn header(buf: &mut Buffer, top: Rect, ctx: &Ctx<'_>) -> Rect {
    if top.height < 2 || top.width == 0 {
        return top;
    }
    let mark = mini_wordmark(Ink::of(ctx.palette));
    buf.set_line(top.x, top.y, &mark, top.width);
    let border = top.height > 2;
    if border {
        let line = "─".repeat(usize::from(top.width));
        let style = Style::new().fg(ctx.palette.surface_dim);
        put(buf, top.x, top.y + 1, top.width, &line, style);
    }
    let used = 1 + u16::from(border);
    Rect::new(top.x, top.y + used, top.width, top.height - used)
}

fn spaces(buf: &mut Buffer, top: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    let indicators = ctx.config.ui.status_indicators;
    for (i, space) in ctx
        .snapshot
        .spaces
        .iter()
        .take(usize::from(top.height))
        .enumerate()
    {
        let y = top.y + u16::try_from(i).unwrap_or(0);
        let rect = Rect::new(top.x, y, top.width, 1);
        let number = if space.selected {
            let bg = ctx.selection_bg();
            buf.set_style(rect, Style::new().bg(bg));
            Style::new().fg(p.overlay1).bg(bg)
        } else if space.focused {
            let bg = ctx.active_bg();
            buf.set_style(rect, Style::new().bg(bg));
            Style::new().fg(p.text).bg(bg)
        } else {
            Style::new().fg(p.overlay0)
        };
        put(
            buf,
            rect.x,
            y,
            rect.width.min(2),
            &format!("{:<2}", i + 1),
            number,
        );
        let state = space.state.unwrap_or(State::Unknown);
        put(
            buf,
            rect.x + 2,
            y,
            rect.width.saturating_sub(2),
            state.icon(indicators),
            Style::new().fg(state.color(p)),
        );
        hits.spaces.push(SpaceHit {
            rect,
            key: space.key.clone(),
            machine: space.machine.clone(),
            indented: false,
            stale: false,
            group: None,
        });
    }
}

fn agents(buf: &mut Buffer, bottom: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    let indicators = ctx.config.ui.status_indicators;
    let rows = usize::from(bottom.height.saturating_sub(1));
    for (i, agent) in ordered_agents(ctx.snapshot)
        .into_iter()
        .take(rows)
        .enumerate()
    {
        let y = bottom.y + u16::try_from(i).unwrap_or(0);
        let rect = Rect::new(bottom.x, y, bottom.width, 1);
        if agent.focused {
            buf.set_style(rect, Style::new().bg(p.active_row_bg));
        }
        let fg = if agent.focused { p.text } else { p.overlay0 };
        put(
            buf,
            rect.x,
            y,
            rect.width.min(2),
            &format!("{:<2}", i + 1),
            Style::new().fg(fg),
        );
        put(
            buf,
            rect.x + 2,
            y,
            rect.width.saturating_sub(2),
            agent.state.icon(indicators),
            Style::new().fg(agent.state.color(p)),
        );
        hits.agents.push(AgentHit {
            rect,
            pane: agent.pane.clone(),
            machine: None,
            stale: false,
        });
    }
}
