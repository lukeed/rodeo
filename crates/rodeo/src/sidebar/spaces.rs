use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::entries::{self, Entry};
use super::rows::{Look, draw_space, space_lines};
use super::scroll;
use super::{Ctx, Hits, Slot, SpaceHit, put};
use crate::model::{MachineRow, SpaceRow};

pub struct Group<'a> {
    pub machine: Option<String>,
    pub online: bool,
    pub spaces: Vec<&'a SpaceRow>,
    pub entries: Vec<Entry>,
}

impl<'a> Group<'a> {
    pub fn new(
        machine: Option<String>,
        online: bool,
        spaces: Vec<&'a SpaceRow>,
        ctx: &Ctx<'_>,
    ) -> Self {
        let entries = entries::entries(&spaces, ctx.groups);
        Self {
            machine,
            online,
            spaces,
            entries,
        }
    }

    fn space(&self, pos: usize) -> &'a SpaceRow {
        self.spaces[self.entries[pos].index]
    }
}

pub enum Row<'a> {
    Machine(&'a MachineRow),
    Space { group: usize, pos: usize },
}

pub struct List<'a> {
    pub groups: Vec<Group<'a>>,
    pub rows: Vec<Row<'a>>,
    pub nested: bool,
}

pub fn header_rows(height: u16) -> u16 {
    height.saturating_sub(2).clamp(1, 2)
}

fn lines_of(group: &Group<'_>, pos: usize, look: &Look, ctx: &Ctx<'_>) -> Vec<Vec<super::Span>> {
    let entry = group.entries[pos];
    let space = group.space(pos);
    let state = entries::state(&group.spaces, entry.index, ctx.groups);
    let label = entries::label(space, entry.indented);
    space_lines(space, label, state, entry.indented, look, ctx)
}

fn measure(list: &List<'_>, room: u16, ctx: &Ctx<'_>) -> (Vec<u16>, Vec<u16>) {
    let plain = Look {
        focused: false,
        dragged: false,
    };
    let heights = list
        .rows
        .iter()
        .map(|row| match row {
            Row::Machine(_) => 1,
            Row::Space { group, pos } => {
                let n = lines_of(&list.groups[*group], *pos, &plain, ctx).len();
                u16::try_from(n).unwrap_or(u16::MAX).max(1).min(room.max(1))
            }
        })
        .collect();
    let gap = ctx.config.ui.sidebar.spaces.row_gap;
    let gaps = list
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| match (row, list.rows.get(i + 1)) {
            (Row::Space { group, .. }, Some(Row::Space { group: next, pos }))
                if group == next && !list.groups[*next].entries[*pos].indented =>
            {
                gap
            }
            _ => 0,
        })
        .collect();
    (heights, gaps)
}

fn first_row(list: &List<'_>, heights: &[u16], gaps: &[u16], room: u16, ctx: &Ctx<'_>) -> usize {
    if !ctx.reveal || room == 0 {
        return ctx.scroll;
    }
    let target = |want: fn(&SpaceRow) -> bool| {
        list.rows.iter().position(|row| match row {
            Row::Space { group, pos } => want(list.groups[*group].space(*pos)),
            Row::Machine(_) => false,
        })
    };
    target(|s| s.selected)
        .or_else(|| target(|s| s.focused))
        .map_or(ctx.scroll, |target| {
            scroll::reveal(heights, gaps, room, ctx.scroll, target)
        })
}

pub fn render(buf: &mut Buffer, area: Rect, list: &List<'_>, ctx: &Ctx<'_>, hits: &mut Hits) {
    let header = header_rows(area.height);
    let body = Rect::new(
        area.x,
        area.y + header.min(area.height),
        area.width,
        area.height.saturating_sub(header + 1),
    );
    hits.spaces_body = Some(body);
    hits.footer = Some(area.bottom().saturating_sub(1));
    let (heights, gaps) = measure(list, body.height, ctx);
    let start = first_row(list, &heights, &gaps, body.height, ctx);
    let metrics = scroll::metrics(&heights, &gaps, body.height, start);
    hits.spaces_metrics = metrics;
    let bar = metrics.overflows() && body.width > 1;
    let width = body.width - u16::from(bar);
    let mut placed: Vec<(usize, Rect)> = Vec::new();
    let mut y = body.y;
    for (i, row) in list.rows.iter().enumerate().skip(metrics.start) {
        let rect = Rect::new(body.x, y, width, heights[i]);
        if rect.bottom() > body.bottom() {
            break;
        }
        placed.push((i, rect));
        if let Row::Space { group, pos } = row {
            let group = &list.groups[*group];
            let space = group.space(*pos);
            hits.spaces.push(SpaceHit {
                rect,
                key: space.key.clone(),
                machine: group.machine.clone(),
                indented: group.entries[*pos].indented,
                stale: space.stale,
                group: None,
            });
        }
        y = rect.bottom() + gaps[i];
    }
    hits.slots = slots(list, &placed, hits);
    let dragged = dragged(list, ctx, hits);
    let mut n = 0;
    for &(i, rect) in &placed {
        match &list.rows[i] {
            Row::Machine(machine) => super::machines::machine_row(buf, rect, machine, ctx, hits),
            Row::Space { group, pos } => {
                let group = &list.groups[*group];
                let is_dragged = dragged.as_deref() == Some(group.space(*pos).key.as_str());
                hits.spaces[n].group = paint(buf, rect, group, *pos, is_dragged, list.nested, ctx);
                n += 1;
            }
        }
    }
    if let Some(press) = ctx
        .press
        .as_ref()
        .filter(|_| !list.nested && dragged.is_some())
        && let Some(slot) = hits.drop_target(press.now.0, press.now.1)
        && slot.row > area.y
        && slot.row + 1 < area.bottom()
    {
        let line = "─".repeat(usize::from(body.width));
        let style = Style::new().fg(ctx.palette.accent);
        put(buf, body.x, slot.row, body.width, &line, style);
    }
    if bar {
        let track = Rect::new(body.right() - 1, body.y, 1, body.height);
        hits.spaces_bar = Some(track);
        scroll::draw(buf, track, metrics, ctx.palette);
    }
}

fn paint(
    buf: &mut Buffer,
    rect: Rect,
    group: &Group<'_>,
    pos: usize,
    dragged: bool,
    nested: bool,
    ctx: &Ctx<'_>,
) -> Option<(Rect, String)> {
    let space = group.space(pos);
    let look = Look {
        focused: space.focused,
        dragged,
    };
    let lines = lines_of(group, pos, &look, ctx);
    let bg = if space.selected {
        Some(ctx.selection_bg())
    } else if look.dragged {
        Some(ctx.palette.surface1)
    } else if look.focused {
        Some(ctx.active_bg())
    } else {
        None
    };
    let inner = if nested {
        Rect::new(
            rect.x + 2,
            rect.y,
            rect.width.saturating_sub(2),
            rect.height,
        )
    } else {
        rect
    };
    draw_space(buf, inner, &group.entries[pos], &lines, bg, ctx);
    if !group.online || space.stale {
        let dim = Style::new()
            .fg(ctx.palette.overlay0)
            .add_modifier(Modifier::DIM);
        buf.set_style(rect, dim);
    }
    let key = entries::parent_key(&group.spaces, group.entries[pos].index)?;
    if rect.width == 0 {
        return None;
    }
    let at = Rect::new(rect.right() - 1, rect.y, 1, 1);
    let glyph = if ctx.groups.contains(key) {
        "▸"
    } else {
        "▾"
    };
    put(
        buf,
        at.x,
        at.y,
        1,
        glyph,
        Style::new().fg(ctx.palette.accent),
    );
    Some((at, key.to_owned()))
}

fn slots(list: &List<'_>, placed: &[(usize, Rect)], hits: &Hits) -> Vec<Slot> {
    let Some(active) = list.groups.iter().position(|g| g.machine == hits.active) else {
        return Vec::new();
    };
    let group = &list.groups[active];
    let mine: Vec<(usize, Rect)> = placed
        .iter()
        .filter_map(|&(i, rect)| match list.rows[i] {
            Row::Space { group: g, pos } if g == active => Some((pos, rect)),
            _ => None,
        })
        .collect();
    let mut out: Vec<Slot> = mine
        .iter()
        .filter(|(pos, _)| !group.entries[*pos].indented)
        .filter(|(pos, _)| {
            let index = group.entries[*pos].index;
            entries::parent_key(&group.spaces, index)
                .is_none_or(|key| entries::first_parent(&group.spaces, key) == Some(index))
        })
        .map(|(pos, rect)| Slot {
            before: Some(group.space(*pos).key.clone()),
            row: rect.y.saturating_sub(1),
        })
        .collect();
    let Some(&(tail, rect)) = mine.last() else {
        return out;
    };
    let next = group.entries.get(tail + 1);
    let own = entries::parent_key(&group.spaces, group.entries[tail].index);
    let joined = next.is_some_and(|n| {
        n.indented
            || own.is_some_and(|key| {
                group.spaces[n.index]
                    .worktree
                    .as_ref()
                    .is_some_and(|w| w.key == key)
            })
    });
    if !joined && hits.footer.is_some_and(|footer| rect.bottom() < footer) {
        out.push(Slot {
            before: next.map(|n| group.spaces[n.index].key.clone()),
            row: rect.bottom(),
        });
    }
    out
}

pub fn draggable(space: &SpaceRow, active: Option<&String>) -> bool {
    space.machine.as_ref() == active
        && !space.stale
        && !space.worktree.as_ref().is_some_and(|w| w.linked)
}

fn dragged(list: &List<'_>, ctx: &Ctx<'_>, hits: &Hits) -> Option<String> {
    let press = ctx.press.as_ref().filter(|p| p.moved())?;
    let space = list
        .groups
        .iter()
        .flat_map(|g| g.spaces.iter())
        .find(|s| s.key == press.key)?;
    (draggable(space, hits.active.as_ref()) && hits.drop_target(press.now.0, press.now.1).is_some())
        .then(|| press.key.clone())
}
