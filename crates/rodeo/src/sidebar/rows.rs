use config::{AgentToken, SpaceToken, Token};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::Ctx;
use super::entries::Entry;
use super::tokens::{Span, draw, fit};
use crate::agents::State;
use crate::model::{AgentRow, SpaceRow};

pub struct Look {
    pub focused: bool,
    pub dragged: bool,
}

fn styled<T>(token: &Token<T>, value: &str, base: Style) -> Option<Span> {
    token
        .style_for(value)
        .map(|s| Span::text(value, s.patch(base)))
}

fn icon(state: State, ctx: &Ctx<'_>) -> (&'static str, Style) {
    (
        state.icon(ctx.config.ui.status_indicators),
        Style::new().fg(state.color(ctx.palette)),
    )
}

pub fn space_lines(
    space: &SpaceRow,
    label: &str,
    state: State,
    indented: bool,
    look: &Look,
    ctx: &Ctx<'_>,
) -> Vec<Vec<Span>> {
    let p = ctx.palette;
    let highlighted = look.focused || look.dragged;
    let workspace = if highlighted {
        Style::new().fg(p.text).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(p.subtext0)
    };
    let secondary = Style::new().fg(if look.focused { p.mauve } else { p.overlay0 });
    let one = |token: &Token<SpaceToken>| -> Option<Span> {
        match &token.kind {
            SpaceToken::StateIcon => {
                let (glyph, style) = icon(state, ctx);
                Some(Span::icon(glyph, token.style.patch(style)))
            }
            SpaceToken::StateText => styled(token, state.text(), Style::new().fg(state.color(p))),
            SpaceToken::Workspace => styled(token, label, workspace),
            SpaceToken::Branch if !indented => styled(
                token,
                space.branch.as_deref().filter(|b| !b.is_empty())?,
                secondary,
            ),
            SpaceToken::GitStatus if !indented && (space.ahead > 0 || space.behind > 0) => {
                Some(git(space, token, ctx))
            }
            SpaceToken::Branch | SpaceToken::GitStatus | SpaceToken::Custom(_) => None,
        }
    };
    ctx.config
        .ui
        .sidebar
        .spaces
        .rows
        .iter()
        .map(|row| row.iter().filter_map(one).collect::<Vec<_>>())
        .filter(|spans| !spans.is_empty())
        .collect()
}

fn git(space: &SpaceRow, token: &Token<SpaceToken>, ctx: &Ctx<'_>) -> Span {
    let p = ctx.palette;
    let pick = |color: Color| token.style.patch(Style::new().fg(color));
    let mut parts = Vec::new();
    if space.ahead > 0 {
        parts.push((format!("↑{}", space.ahead), pick(p.green)));
    }
    if space.ahead > 0 && space.behind > 0 {
        parts.push((" ".to_owned(), token.style.patch(Style::new())));
    }
    if space.behind > 0 {
        parts.push((format!("↓{}", space.behind), pick(p.red)));
    }
    Span::git(parts)
}

pub fn draw_space(
    buf: &mut Buffer,
    area: Rect,
    entry: &Entry,
    lines: &[Vec<Span>],
    bg: Option<Color>,
    ctx: &Ctx<'_>,
) {
    let p = ctx.palette;
    let sep = Style::new().fg(p.overlay0);
    for (n, spans) in lines.iter().enumerate() {
        let Ok(offset) = u16::try_from(n) else {
            break;
        };
        let y = area.y + offset;
        if y >= area.bottom() {
            break;
        }
        let x = if entry.indented {
            let prefix = match (n == 0, entry.last_child) {
                (true, true) => "   └─ ",
                (true, false) => "   ├─ ",
                (false, true) => "        ",
                (false, false) => "   │    ",
            };
            super::put(buf, area.x, y, area.width, prefix, sep)
        } else {
            area.x + if n == 0 { 1 } else { 3 }
        };
        let right = area.right().saturating_sub(2);
        if x < right {
            draw(buf, x, y, right, &fit(spans, right - x, sep));
        }
    }
    if let Some(bg) = bg {
        buf.set_style(area, Style::new().bg(bg));
    }
}

fn agent_span(token: &Token<AgentToken>, agent: &AgentRow, ctx: &Ctx<'_>) -> Option<Span> {
    let p = ctx.palette;
    let secondary = Style::new().fg(p.overlay0);
    let (value, base) = match &token.kind {
        AgentToken::StateIcon => {
            let (glyph, style) = icon(agent.state, ctx);
            return Some(Span::icon(glyph, token.style.patch(style)));
        }
        AgentToken::StateText => {
            let text = if agent.state == State::Unknown {
                "idle"
            } else {
                agent.state.text()
            };
            (Some(text), Style::new().fg(agent.state.color(p)))
        }
        AgentToken::Machine => (ctx.multi().then_some(agent.machine.as_str()), secondary),
        AgentToken::Workspace => {
            let fg = if agent.focused { p.text } else { p.subtext0 };
            (
                Some(agent.workspace.as_str()),
                Style::new().fg(fg).add_modifier(Modifier::BOLD),
            )
        }
        AgentToken::Tab => (agent.tab_label.as_deref(), secondary),
        AgentToken::Pane => (agent.pane_label.as_deref(), secondary),
        AgentToken::Agent => (Some(agent.agent.as_str()), secondary),
        AgentToken::TerminalTitle => (Some(agent.terminal_title.as_str()), secondary),
        AgentToken::TerminalTitleStripped => {
            (Some(agent.terminal_title_stripped.as_str()), secondary)
        }
        AgentToken::Custom(name) => (agent.labels.get(name).map(String::as_str), secondary),
    };
    styled(token, value.filter(|v| !v.is_empty())?, base)
}

pub fn agent_lines(agent: &AgentRow, ctx: &Ctx<'_>) -> Vec<Vec<Span>> {
    let rows = ctx
        .config
        .ui
        .sidebar
        .agents
        .rows_for(Some(agent.agent.as_str()).filter(|a| !a.is_empty()));
    let lines: Vec<Vec<Span>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .filter_map(|token| agent_span(token, agent, ctx))
                .collect::<Vec<_>>()
        })
        .filter(|spans| !spans.is_empty())
        .collect();
    if lines.is_empty() {
        let (glyph, style) = icon(agent.state, ctx);
        return vec![vec![Span::icon(glyph, style)]];
    }
    lines
}

pub fn draw_agent(
    buf: &mut Buffer,
    rect: Rect,
    agent: &AgentRow,
    lines: &[Vec<Span>],
    ctx: &Ctx<'_>,
) {
    let p = ctx.palette;
    if agent.focused {
        buf.set_style(rect, Style::new().bg(p.active_row_bg));
    }
    let sep = Style::new().fg(p.overlay0);
    for (n, spans) in lines.iter().take(usize::from(rect.height)).enumerate() {
        let indent = if n == 0 { 1 } else { 3 };
        let x = rect.x + indent;
        let y = rect.y + u16::try_from(n).unwrap_or(0);
        let right = rect.right();
        if x < right {
            draw(buf, x, y, right, &fit(spans, right - x, sep));
        }
    }
    if agent.stale {
        buf.set_style(
            rect,
            Style::new().fg(p.overlay0).add_modifier(Modifier::DIM),
        );
    }
}
