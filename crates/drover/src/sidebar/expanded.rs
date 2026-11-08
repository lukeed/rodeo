use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::spaces::{self, Group, List, Row};
use super::{Ctx, Hits, put, put_right, sections};

pub fn render(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    let p = ctx.palette;
    hits.divider = Some(Rect::new(area.right() - 1, area.y, 1, area.height));
    let (top, bottom) = sections(area, ctx.split);
    if area.height >= 6 && top.width > 0 {
        hits.section_divider = Some(Rect::new(bottom.x, bottom.y, bottom.width, 1));
    }
    let bold = Style::new().fg(p.overlay0).add_modifier(Modifier::BOLD);
    put(buf, top.x, top.y, top.width, " spaces", bold);
    let group = Group::new(None, true, ctx.snapshot.spaces.iter().collect(), ctx);
    let rows = (0..group.entries.len())
        .map(|pos| Row::Space { group: 0, pos })
        .collect();
    let list = List {
        groups: vec![group],
        rows,
        nested: false,
    };
    spaces::render(buf, top, &list, ctx, hits);
    footer(buf, top, ctx, hits, None);
    super::agents::render(buf, bottom, ctx, hits);
    collapse_toggle(buf, area, ctx, hits);
}

pub fn footer(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits, machine: Option<&str>) {
    if !ctx.mouse() || area.height == 0 {
        return;
    }
    let p = ctx.palette;
    let y = area.bottom() - 1;
    let dim = Style::new().fg(p.overlay0);
    let attention = ctx.snapshot.menu_attention;
    let new = match machine {
        Some(label) => format!(" new · {label}"),
        None => " new".to_owned(),
    };
    let new_width = if machine.is_some() {
        crate::text::width(&new)
    } else {
        5
    };
    put(buf, area.x, y, area.width, &new, dim);
    hits.new_workspace = Some(Rect::new(area.x, y, new_width.min(area.width), 1));
    let menu_width: u16 = if attention { 8 } else { 6 };
    hits.menu = Some(Rect::new(
        area.right().saturating_sub(menu_width),
        y,
        menu_width.min(area.width),
        1,
    ));
    match (attention, machine.is_some()) {
        (true, false) => {
            let x = area.right().saturating_sub(6);
            put(
                buf,
                x,
                y,
                2,
                "● ",
                Style::new().fg(p.accent).add_modifier(Modifier::BOLD),
            );
            put(buf, x + 2, y, 4, "menu", dim);
        }
        (true, true) => {
            put_right(buf, area, y, "● menu", Style::new().fg(p.accent));
        }
        (false, _) => {
            put_right(buf, area, y, "menu", dim);
        }
    }
}

pub fn collapse_toggle(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    if area.width < 2 {
        return;
    }
    let toggle = Rect::new(area.right() - 2, area.bottom() - 1, 1, 1);
    put(
        buf,
        toggle.x,
        toggle.y,
        1,
        "«",
        Style::new().fg(ctx.palette.overlay0),
    );
    hits.toggle = Some(toggle);
}
