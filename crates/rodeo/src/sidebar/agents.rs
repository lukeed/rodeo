use config::AgentPanelSort;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::rows::{agent_lines, draw_agent};
use super::scroll;
use super::{AgentHit, Ctx, Hits, put};
use crate::model::AgentRow;

pub fn render(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    if area.height == 0 {
        return;
    }
    let p = ctx.palette;
    put(
        buf,
        area.x,
        area.y,
        area.width,
        &"─".repeat(usize::from(area.width)),
        Style::new().fg(p.surface_dim),
    );
    if area.height < 2 {
        return;
    }
    let bold = Style::new().fg(p.overlay0).add_modifier(Modifier::BOLD);
    put(buf, area.x, area.y + 1, area.width, " agents", bold);
    let label = match ctx.snapshot.agent_sort {
        AgentPanelSort::Spaces => "grouped",
        AgentPanelSort::Priority => "priority",
    };
    let sort = super::put_right(buf, area, area.y + 1, label, bold);
    hits.agent_sort = Some(sort);
    list(buf, area, ctx, hits);
}

fn list(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let body = Rect::new(
        area.x,
        area.y + 3.min(area.height),
        area.width,
        area.height.saturating_sub(3),
    );
    hits.agents_body = Some(body);
    let agents = ctx.agents();
    if body.is_empty() || agents.is_empty() {
        return;
    }
    let rows: Vec<(&AgentRow, Vec<Vec<super::Span>>)> = agents
        .into_iter()
        .map(|a| (a, agent_lines(a, ctx)))
        .collect();
    let heights: Vec<u16> = rows
        .iter()
        .map(|(_, lines)| u16::try_from(lines.len()).unwrap_or(u16::MAX).max(1))
        .collect();
    let gap = ctx.config.ui.sidebar.agents.row_gap;
    let gaps: Vec<u16> = (0..rows.len())
        .map(|i| if i + 1 < rows.len() { gap } else { 0 })
        .collect();
    let metrics = scroll::metrics(&heights, &gaps, body.height, ctx.agent_scroll);
    hits.agents_metrics = metrics;
    let bar = metrics.overflows() && body.width > 1;
    let width = body.width - u16::from(bar);
    let mut y = body.y;
    for (i, (agent, lines)) in rows.iter().enumerate().skip(metrics.start) {
        let height = heights[i].min(body.height);
        if y + height > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, width, height);
        draw_agent(buf, rect, agent, lines, ctx);
        hits.agents.push(AgentHit {
            rect,
            pane: agent.pane.clone(),
            machine: ctx.machine_name(&agent.machine),
            stale: agent.stale,
        });
        y += height + gaps[i];
    }
    if bar {
        let track = Rect::new(body.right() - 1, body.y, 1, body.height);
        hits.agents_bar = Some(track);
        scroll::draw(buf, track, metrics, ctx.palette);
    }
}
