use config::{AgentPanelSort, AgentToken, SpaceToken, Token};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::tokens::{Span, draw, fit};
use super::{Ctx, Hits, spaces_height};
use crate::agents::State;
use crate::model::{AgentRow, SpaceRow};

pub fn render(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    hits.divider = Some(Rect::new(area.right() - 1, area.y, 1, area.height));
    let content = Rect::new(area.x, area.y, area.width - 1, area.height);
    let top = spaces_height(content.height, ctx.split);
    let spaces = Rect::new(content.x, content.y, content.width, top);
    let agents = Rect::new(
        content.x,
        content.y + top,
        content.width,
        content.height - top,
    );
    render_spaces(buf, spaces, ctx, hits);
    render_agents(buf, agents, ctx, hits, content.height >= 6);
    let toggle = Rect::new(area.right().saturating_sub(2), area.bottom() - 1, 1, 1);
    buf[(toggle.x, toggle.y)]
        .set_symbol("«")
        .set_style(Style::new().fg(p.overlay0).bg(p.sidebar_bg));
    hits.toggle = Some(toggle);
}

fn header(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str, ctx: &Ctx<'_>) {
    let style = Style::new()
        .fg(ctx.palette.overlay0)
        .add_modifier(Modifier::BOLD);
    buf.set_stringn(x, y, text, usize::from(width), style);
}

fn row_bg(ctx: &Ctx<'_>, selected: bool, focused: bool) -> Option<Color> {
    let p = ctx.palette;
    let navigating = ctx.snapshot.mode == crate::view::Mode::Navigate;
    if selected {
        Some(if p.selection_bg == Color::Reset {
            p.active_row_bg
        } else {
            p.selection_bg
        })
    } else if focused {
        Some(if navigating && p.selection_bg == Color::Reset {
            p.sidebar_bg
        } else {
            p.active_row_bg
        })
    } else {
        None
    }
}

fn space_lines(space: &SpaceRow, ctx: &Ctx<'_>) -> Vec<Vec<Span>> {
    let p = ctx.palette;
    let state = space.state.unwrap_or(State::Unknown);
    let mut lines = Vec::new();
    for row in &ctx.config.ui.sidebar.spaces.rows {
        let mut spans = Vec::new();
        for token in row {
            if let Some(span) = space_span(token, space, state, ctx) {
                spans.push(span);
            }
        }
        if !spans.is_empty() {
            lines.push(spans);
        }
    }
    if lines.is_empty() {
        lines.push(vec![Span::icon(
            state.icon(ctx.config.ui.status_indicators),
            Style::new().fg(state.color(p)),
        )]);
    }
    lines
}

fn styled<T>(token: &Token<T>, value: &str, base: Style) -> Option<Style> {
    token.style_for(value).map(|s| s.patch(base))
}

fn space_span(
    token: &Token<SpaceToken>,
    space: &SpaceRow,
    state: State,
    ctx: &Ctx<'_>,
) -> Option<Span> {
    let p = ctx.palette;
    match &token.kind {
        SpaceToken::StateIcon => Some(Span::icon(
            state.icon(ctx.config.ui.status_indicators),
            token.style.patch(Style::new().fg(state.color(p))),
        )),
        SpaceToken::StateText => {
            let text = state.text();
            styled(token, text, Style::new().fg(state.color(p))).map(|s| Span::text(text, s))
        }
        SpaceToken::Workspace => {
            let base = if space.focused {
                Style::new().fg(p.text).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(p.subtext0)
            };
            styled(token, &space.name, base).map(|s| Span::text(&space.name, s))
        }
        SpaceToken::Branch => {
            let branch = space.branch.as_deref()?;
            let base = Style::new().fg(if space.focused { p.mauve } else { p.overlay0 });
            styled(token, branch, base).map(|s| Span::text(branch, s))
        }
        SpaceToken::GitStatus => {
            if space.ahead == 0 && space.behind == 0 {
                return None;
            }
            let mut parts = Vec::new();
            let pick = |color: Color| token.style.patch(Style::new().fg(color));
            if space.ahead > 0 {
                parts.push((format!("↑{}", space.ahead), pick(p.green)));
            }
            if space.ahead > 0 && space.behind > 0 {
                parts.push((" ".to_owned(), Style::new()));
            }
            if space.behind > 0 {
                parts.push((format!("↓{}", space.behind), pick(p.red)));
            }
            Some(Span::git(parts))
        }
        SpaceToken::Custom(_) => None,
    }
}

fn render_spaces(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    if area.height == 0 {
        return;
    }
    let p = ctx.palette;
    header(buf, area.x, area.y, area.width, " spaces", ctx);
    let footer = ctx.mouse() && area.height >= 4;
    let body_height = area.height.saturating_sub(if footer { 3 } else { 2 });
    let body = Rect::new(area.x, area.y + 2, area.width, body_height);
    hits.spaces_body = Some(body);
    let gap = ctx.config.ui.sidebar.spaces.row_gap;
    let entries: Vec<(&SpaceRow, Vec<Vec<Span>>)> = ctx
        .snapshot
        .spaces
        .iter()
        .map(|s| (s, space_lines(s, ctx)))
        .collect();
    let heights: Vec<u16> = entries
        .iter()
        .map(|(_, lines)| {
            u16::try_from(lines.len())
                .unwrap_or(1)
                .clamp(1, body.height.max(1))
        })
        .collect();
    let total: u16 = heights.iter().sum::<u16>()
        + gap * u16::try_from(heights.len().saturating_sub(1)).unwrap_or(0);
    let overflow = total > body.height && body.width > 1;
    let width = if overflow { body.width - 1 } else { body.width };
    let focused = ctx
        .snapshot
        .spaces
        .iter()
        .position(|s| s.focused)
        .unwrap_or(0);
    let mut first = ctx.scroll.min(entries.len().saturating_sub(1));
    if overflow {
        while first > focused {
            first -= 1;
        }
        loop {
            let used: u16 = heights[first..=focused.max(first)]
                .iter()
                .map(|h| h + gap)
                .sum();
            if used <= body.height + gap || first >= focused {
                break;
            }
            first += 1;
        }
    } else {
        first = 0;
    }
    let mut y = body.y;
    for (i, (space, lines)) in entries.iter().enumerate().skip(first) {
        let height = heights[i];
        if y + height > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, width, height);
        for (n, spans) in lines.iter().take(usize::from(height)).enumerate() {
            let indent = if n == 0 { 1 } else { 3 };
            let x = rect.x + indent;
            let right = rect.right().saturating_sub(1);
            let parts = fit(spans, right.saturating_sub(x), Style::new().fg(p.overlay0));
            draw(buf, x, y + u16::try_from(n).unwrap_or(0), right, &parts);
        }
        if let Some(bg) = row_bg(ctx, space.selected, space.focused) {
            buf.set_style(rect, Style::new().bg(bg));
        }
        hits.spaces.push((rect, space.key.clone()));
        y += height + gap;
    }
    if overflow {
        scrollbar(
            buf,
            Rect::new(body.right() - 1, body.y, 1, body.height),
            first,
            entries.len(),
            ctx,
        );
    }
    if footer {
        let y = area.bottom() - 1;
        let style = Style::new().fg(p.overlay0);
        buf.set_stringn(area.x, y, " new", usize::from(area.width), style);
        hits.new_workspace = Some(Rect::new(area.x, y, 5.min(area.width), 1));
        if area.width >= 10 {
            let x = area.right() - 4;
            buf.set_string(x, y, "menu", style);
            hits.menu = Some(Rect::new(area.right() - 6, y, 6, 1));
        }
    }
}

fn scrollbar(buf: &mut Buffer, track: Rect, first: usize, total: usize, ctx: &Ctx<'_>) {
    let p = ctx.palette;
    if track.height == 0 || total == 0 {
        return;
    }
    let h = usize::from(track.height);
    let len = (h * h / total.max(h)).clamp(1, h);
    let top = if total <= 1 {
        0
    } else {
        (h - len) * first / (total - 1)
    };
    for i in 0..h {
        let thumb = i >= top && i < top + len;
        let color = if thumb { p.overlay0 } else { p.surface_dim };
        let y = track.y + u16::try_from(i).unwrap_or(0);
        buf[(track.x, y)]
            .set_symbol("▕")
            .set_style(Style::new().fg(color));
    }
}

fn agent_lines(agent: &AgentRow, ctx: &Ctx<'_>) -> Vec<Vec<Span>> {
    let p = ctx.palette;
    let rows = ctx
        .config
        .ui
        .sidebar
        .agents
        .rows_for(Some(agent.agent.as_str()).filter(|a| !a.is_empty()));
    let multi = ctx.snapshot.machines.len() > 1;
    let mut lines = Vec::new();
    for row in rows {
        let mut spans = Vec::new();
        for token in row {
            let secondary = Style::new().fg(p.overlay0);
            let span = match &token.kind {
                AgentToken::StateIcon => Some(Span::icon(
                    agent.state.icon(ctx.config.ui.status_indicators),
                    token.style.patch(Style::new().fg(agent.state.color(p))),
                )),
                AgentToken::StateText => {
                    let text = if agent.state == State::Unknown {
                        "idle"
                    } else {
                        agent.state.text()
                    };
                    styled(token, text, Style::new().fg(agent.state.color(p)))
                        .map(|s| Span::text(text, s))
                }
                AgentToken::Machine if multi => {
                    styled(token, &agent.machine, secondary).map(|s| Span::text(&agent.machine, s))
                }
                AgentToken::Machine => None,
                AgentToken::Workspace => {
                    let base = if agent.focused {
                        Style::new().fg(p.text).add_modifier(Modifier::BOLD)
                    } else {
                        Style::new().fg(p.subtext0).add_modifier(Modifier::BOLD)
                    };
                    styled(token, &agent.workspace, base).map(|s| Span::text(&agent.workspace, s))
                }
                AgentToken::Tab => agent
                    .tab_label
                    .as_deref()
                    .and_then(|t| styled(token, t, secondary).map(|s| Span::text(t, s))),
                AgentToken::Pane => agent
                    .pane_label
                    .as_deref()
                    .and_then(|t| styled(token, t, secondary).map(|s| Span::text(t, s))),
                AgentToken::Agent => Some(agent.agent.as_str())
                    .filter(|a| !a.is_empty())
                    .and_then(|t| styled(token, t, secondary).map(|s| Span::text(t, s))),
                AgentToken::TerminalTitle => Some(agent.terminal_title.as_str())
                    .filter(|t| !t.is_empty())
                    .and_then(|t| {
                        styled(token, t, Style::new().fg(p.overlay1)).map(|s| Span::text(t, s))
                    }),
                AgentToken::TerminalTitleStripped => Some(agent.terminal_title_stripped.as_str())
                    .filter(|t| !t.is_empty())
                    .and_then(|t| {
                        styled(token, t, Style::new().fg(p.overlay1)).map(|s| Span::text(t, s))
                    }),
                AgentToken::Custom(_) => None,
            };
            spans.extend(span);
        }
        if !spans.is_empty() {
            lines.push(spans);
        }
    }
    if lines.is_empty() {
        lines.push(vec![Span::icon(
            agent.state.icon(ctx.config.ui.status_indicators),
            Style::new().fg(agent.state.color(p)),
        )]);
    }
    lines
}

fn render_agents(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits, divider: bool) {
    if area.height == 0 {
        return;
    }
    let p = ctx.palette;
    buf.set_stringn(
        area.x,
        area.y,
        "─".repeat(usize::from(area.width)),
        usize::from(area.width),
        Style::new().fg(p.surface_dim),
    );
    if divider {
        hits.section_divider = Some(Rect::new(area.x, area.y, area.width, 1));
    }
    if area.height < 2 {
        return;
    }
    header(buf, area.x, area.y + 1, area.width, " agents", ctx);
    let sort = match ctx.snapshot.agent_sort {
        AgentPanelSort::Spaces => "grouped",
        AgentPanelSort::Priority => "priority",
    };
    let sort_width = u16::try_from(sort.len()).unwrap_or(0);
    if area.width > sort_width + 8 {
        let x = area.right() - sort_width;
        buf.set_string(
            x,
            area.y + 1,
            sort,
            Style::new().fg(p.overlay0).add_modifier(Modifier::BOLD),
        );
        hits.agent_sort = Some(Rect::new(x, area.y + 1, sort_width, 1));
    }
    if area.height < 4 {
        return;
    }
    let body = Rect::new(area.x, area.y + 3, area.width, area.height - 3);
    hits.agents_body = Some(body);
    let gap = ctx.config.ui.sidebar.agents.row_gap;
    let agents = ctx.snapshot.ordered_agents();
    let first = ctx.agent_scroll.min(agents.len().saturating_sub(1));
    let mut y = body.y;
    for agent in agents.into_iter().skip(first) {
        let lines = agent_lines(agent, ctx);
        let height = u16::try_from(lines.len()).unwrap_or(1);
        if y + height > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, body.width, height);
        if agent.focused {
            buf.set_style(rect, Style::new().bg(p.active_row_bg));
        }
        for (n, spans) in lines.iter().enumerate() {
            let x = rect.x + if n == 0 { 1 } else { 3 };
            let right = rect.right().saturating_sub(1);
            let parts = fit(spans, right.saturating_sub(x), Style::new().fg(p.overlay0));
            draw(buf, x, y + u16::try_from(n).unwrap_or(0), right, &parts);
        }
        hits.agents.push((rect, agent.pane.clone()));
        y += height + gap;
    }
}
