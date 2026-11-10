use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::expanded::{collapse_toggle, footer, header};
use super::spaces::{self, Group, List, Row};
use super::{
    AgentHit, Ctx, Hits, MachineHit, Metrics, SpaceHit, compact_sections, ordered_agents, put,
    put_right, sections, status_look,
};
use crate::agents::State;
use crate::model::{MachineRow, MachineStatus, SpaceRow};

fn collapsed(machine: &MachineRow, ctx: &Ctx<'_>) -> bool {
    ctx.state.collapsed_machines.contains(&machine.name)
}

fn hovered(machine: &MachineRow, ctx: &Ctx<'_>) -> bool {
    ctx.state.hover.as_ref() == Some(&machine.name) && has_diagnostic(machine)
}

pub fn has_diagnostic(machine: &MachineRow) -> bool {
    machine.name.is_some() && machine.why.as_deref().is_some_and(|w| !w.is_empty())
}

fn badge_style(machine: &MachineRow, base: Style, ctx: &Ctx<'_>) -> Style {
    if hovered(machine, ctx) {
        base.bg(ctx.palette.active_row_bg)
            .add_modifier(Modifier::REVERSED)
    } else {
        base
    }
}

fn paint_row(buf: &mut Buffer, rect: Rect, machine: &MachineRow, folded: bool, ctx: &Ctx<'_>) {
    let shown = !folded
        && ctx
            .machine_spaces(machine.name.as_deref())
            .iter()
            .any(|s| s.focused);
    if machine.selected {
        buf.set_style(rect, Style::new().bg(ctx.selection_bg()));
    } else if machine.active && !shown {
        buf.set_style(rect, Style::new().bg(ctx.active_bg()));
    }
}

pub fn expanded(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    hits.divider = Some(Rect::new(area.right() - 1, area.y, 1, area.height));
    let (top, bottom) = sections(area, ctx.split);
    if area.height >= 6 && top.width > 0 {
        hits.section_divider = Some(Rect::new(bottom.x, bottom.y, bottom.width, 1));
    }
    header(buf, top, ctx);
    let mut groups = Vec::new();
    let mut rows = Vec::new();
    for machine in &ctx.snapshot.machines {
        rows.push(Row::Machine(machine));
        if collapsed(machine, ctx) {
            continue;
        }
        let group = Group::new(
            machine.name.clone(),
            machine.status == MachineStatus::Online,
            ctx.machine_spaces(machine.name.as_deref()),
            ctx,
        );
        let index = groups.len();
        rows.extend((0..group.entries.len()).map(|pos| Row::Space { group: index, pos }));
        groups.push(group);
    }
    let list = List {
        groups,
        rows,
        nested: true,
    };
    spaces::render(buf, top, &list, ctx, hits);
    let label = ctx
        .snapshot
        .machines
        .iter()
        .find(|m| m.active)
        .map_or(crate::machines::LOCAL_LABEL, |m| m.label.as_str());
    footer(buf, top, ctx, hits, Some(label));
    super::agents::render(buf, bottom, ctx, hits);
    collapse_toggle(buf, area, ctx, hits);
}

pub fn machine_row(
    buf: &mut Buffer,
    rect: Rect,
    machine: &MachineRow,
    ctx: &Ctx<'_>,
    hits: &mut Hits,
) {
    let p = ctx.palette;
    let folded = collapsed(machine, ctx);
    paint_row(buf, rect, machine, folded, ctx);
    let (glyph, word, color) = status_look(machine.status, p);
    let signal = if machine.auth {
        "! auth".to_owned()
    } else if machine.status == MachineStatus::Attention {
        "! error".to_owned()
    } else if machine.name.is_none() {
        String::new()
    } else if machine.status == MachineStatus::Online {
        glyph.to_owned()
    } else {
        format!("{glyph} {word}")
    };
    let signal_width = crate::text::width(&signal).min(rect.width);
    let marker = if folded { "▸" } else { "▾" };
    let fg = if machine.status == MachineStatus::Disabled {
        p.overlay0
    } else {
        p.text
    };
    let reserve = if signal.is_empty() {
        0
    } else {
        signal_width + 1
    };
    let mut x = put(
        buf,
        rect.x,
        rect.y,
        rect.width.saturating_sub(reserve),
        &format!(" {marker} {}", machine.label),
        Style::new().fg(fg).add_modifier(Modifier::BOLD),
    );
    let mut badge = Rect::new(x, rect.y, 0, 1);
    if !signal.is_empty() {
        let start = (x + 1).min(rect.right());
        x = put(
            buf,
            start,
            rect.y,
            rect.right() - start,
            &signal,
            badge_style(machine, Style::new().fg(color), ctx),
        );
        badge = Rect::new(start, rect.y, x - start, 1);
    }
    let rule = (x + 1).min(rect.right());
    put(
        buf,
        rule,
        rect.y,
        rect.right() - rule,
        &"─".repeat(usize::from(rect.right() - rule)),
        Style::new().fg(p.surface_dim),
    );
    hits.machines.push(MachineHit {
        rect,
        badge,
        toggle: Rect::new(rect.x + 1, rect.y, u16::from(rect.width > 1), 1),
        name: machine.name.clone(),
    });
}

enum Line<'a> {
    Machine(usize, &'a MachineRow),
    Space(&'a MachineRow, usize, &'a SpaceRow),
}

pub fn compact(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    let (top, divider, bottom) = compact_sections(area);
    let rows = super::compact::header(buf, top, ctx);
    compact_spaces(buf, rows, ctx, hits);
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
    compact_agents(buf, bottom, ctx, hits);
    if top.width > 0 {
        let toggle = Rect::new(top.x + top.width / 2, area.bottom() - 1, 1, 1);
        put(buf, toggle.x, toggle.y, 1, "»", Style::new().fg(p.overlay0));
        hits.toggle = Some(toggle);
    }
}

fn compact_lines<'a>(ctx: &Ctx<'a>) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    for (index, machine) in ctx.snapshot.machines.iter().enumerate() {
        lines.push(Line::Machine(index, machine));
        if collapsed(machine, ctx) {
            continue;
        }
        let spaces = ctx.machine_spaces(machine.name.as_deref());
        lines.extend(
            spaces
                .into_iter()
                .enumerate()
                .map(|(n, space)| Line::Space(machine, n + 1, space)),
        );
    }
    lines
}

fn compact_spaces(buf: &mut Buffer, top: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let lines = compact_lines(ctx);
    let height = usize::from(top.height);
    let max = lines.len().saturating_sub(height);
    let mut start = ctx.scroll.min(max);
    let selected = lines.iter().position(|l| match l {
        Line::Machine(_, m) => m.selected,
        Line::Space(_, _, s) => s.selected,
    });
    if ctx.reveal
        && let Some(row) = selected
    {
        if row < start {
            start = row;
        } else if row >= start + height {
            start = (row + 1 - height).min(max);
        }
    }
    hits.spaces_metrics = Metrics {
        start,
        max,
        visible: height.min(lines.len()),
    };
    for (offset, line) in lines.iter().skip(start).take(height).enumerate() {
        let y = top.y + u16::try_from(offset).unwrap_or(0);
        let rect = Rect::new(top.x, y, top.width, 1);
        match line {
            Line::Machine(index, machine) => compact_machine(buf, rect, *index, machine, ctx, hits),
            Line::Space(machine, number, space) => {
                compact_space(buf, rect, machine, *number, space, ctx);
                hits.spaces.push(SpaceHit {
                    rect,
                    key: space.key.clone(),
                    machine: machine.name.clone(),
                    indented: false,
                    stale: space.stale,
                    group: None,
                });
            }
        }
    }
}

fn compact_machine(
    buf: &mut Buffer,
    rect: Rect,
    index: usize,
    machine: &MachineRow,
    ctx: &Ctx<'_>,
    hits: &mut Hits,
) {
    let p = ctx.palette;
    let folded = collapsed(machine, ctx);
    paint_row(buf, rect, machine, folded, ctx);
    let label = if machine.name.is_none() {
        "L".to_owned()
    } else {
        (index + 1).to_string()
    };
    let marker = if folded { "▸" } else { "▾" };
    let fg = if machine.status == MachineStatus::Online {
        p.text
    } else {
        p.overlay0
    };
    put(
        buf,
        rect.x,
        rect.y,
        rect.width.saturating_sub(1),
        &format!("{marker}{label}"),
        Style::new().fg(fg),
    );
    let mut badge = Rect::default();
    if machine.name.is_some() {
        let (glyph, _, color) = status_look(machine.status, p);
        let style = badge_style(machine, Style::new().fg(color), ctx);
        badge = put_right(buf, rect, rect.y, glyph, style);
    }
    hits.machines.push(MachineHit {
        rect,
        badge,
        toggle: Rect::new(rect.x, rect.y, u16::from(rect.width > 1), 1),
        name: machine.name.clone(),
    });
}

fn compact_space(
    buf: &mut Buffer,
    rect: Rect,
    machine: &MachineRow,
    number: usize,
    space: &SpaceRow,
    ctx: &Ctx<'_>,
) {
    let p = ctx.palette;
    if space.selected {
        buf.set_style(rect, Style::new().bg(ctx.selection_bg()));
    } else if space.focused {
        buf.set_style(rect, Style::new().bg(ctx.active_bg()));
    }
    let offline = machine.status != MachineStatus::Online || space.stale;
    let dim = if offline {
        Modifier::DIM
    } else {
        Modifier::empty()
    };
    let fg = if space.focused && !offline {
        p.text
    } else {
        p.overlay0
    };
    let number = format!(" {number}");
    let x = put(
        buf,
        rect.x,
        rect.y,
        rect.width,
        &number,
        Style::new().fg(fg).add_modifier(dim),
    );
    let state = space.state.unwrap_or(State::Unknown);
    let color = if offline { p.overlay0 } else { state.color(p) };
    put(
        buf,
        x,
        rect.y,
        rect.right().saturating_sub(x),
        state.icon(ctx.config.ui.status_indicators),
        Style::new().fg(color).add_modifier(dim),
    );
}

fn compact_agents(buf: &mut Buffer, bottom: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    let agents = ordered_agents(ctx.snapshot);
    for (offset, agent) in agents
        .into_iter()
        .take(usize::from(bottom.height))
        .enumerate()
    {
        let y = bottom.y + u16::try_from(offset).unwrap_or(0);
        let rect = Rect::new(bottom.x, y, bottom.width, 1);
        if agent.focused {
            buf.set_style(rect, Style::new().bg(p.active_row_bg));
        }
        let initial = agent.machine.chars().next().unwrap_or('?');
        let (color, dim) = if agent.stale {
            (p.overlay0, Modifier::DIM)
        } else {
            (agent.state.color(p), Modifier::empty())
        };
        let icon = agent.state.icon(ctx.config.ui.status_indicators);
        put(
            buf,
            rect.x,
            y,
            rect.width,
            &format!("{initial}{icon}"),
            Style::new().fg(color).add_modifier(dim),
        );
        hits.agents.push(AgentHit {
            rect,
            pane: agent.pane.clone(),
            machine: ctx.machine_name(&agent.machine),
            stale: agent.stale,
        });
    }
}
