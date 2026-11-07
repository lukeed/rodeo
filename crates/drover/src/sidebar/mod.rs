mod compact;
mod expanded;
mod tokens;

use config::{Config, Palette};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::layout::COMPACT_SIDEBAR;
use crate::model::Snapshot;

pub use tokens::{Span, fit};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hits {
    pub area: Option<Rect>,
    pub toggle: Option<Rect>,
    pub divider: Option<Rect>,
    pub section_divider: Option<Rect>,
    pub new_workspace: Option<Rect>,
    pub menu: Option<Rect>,
    pub agent_sort: Option<Rect>,
    pub spaces: Vec<(Rect, String)>,
    pub agents: Vec<(Rect, String)>,
    pub spaces_body: Option<Rect>,
    pub agents_body: Option<Rect>,
    pub group_toggles: Vec<(Rect, String)>,
    pub machines: Vec<(Rect, Option<String>)>,
}

impl Hits {
    pub fn clear_chrome(&mut self) {
        *self = Self {
            area: self.area,
            toggle: self.toggle,
            ..Self::default()
        };
    }
}

pub struct Ctx<'a> {
    pub palette: &'a Palette,
    pub config: &'a Config,
    pub snapshot: &'a Snapshot,
    pub split: f32,
    pub scroll: usize,
    pub agent_scroll: usize,
}

impl Ctx<'_> {
    pub fn mouse(&self) -> bool {
        self.config.ui.mouse_capture
    }
}

pub fn render(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    *hits = Hits::default();
    if area.width == 0 || area.height == 0 {
        return;
    }
    hits.area = Some(area);
    background(buf, area, ctx.palette);
    if ctx.snapshot.collapsed && area.width <= COMPACT_SIDEBAR {
        compact::render(buf, area, ctx, hits);
    } else {
        expanded::render(buf, area, ctx, hits);
    }
    if !ctx.mouse() {
        hits.clear_chrome();
    }
}

fn background(buf: &mut Buffer, area: Rect, palette: &Palette) {
    buf.set_style(area, Style::new().bg(palette.sidebar_bg));
    let x = area.right() - 1;
    for y in area.top()..area.bottom() {
        buf[(x, y)]
            .set_symbol("│")
            .set_style(Style::new().fg(palette.surface_dim).bg(palette.sidebar_bg));
    }
}

pub fn spaces_height(h: u16, split: f32) -> u16 {
    if h < 6 {
        return h.div_ceil(2);
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the split is clamped to 0.1..0.9 of a u16 height"
    )]
    let rows = (f32::from(h) * split).round() as u16;
    rows.clamp(3, h - 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_section_split_follows_herdr() {
        assert_eq!(spaces_height(5, 0.5), 3);
        assert_eq!(spaces_height(40, 0.5), 20);
        assert_eq!(spaces_height(40, 0.9), 36);
        assert_eq!(spaces_height(10, 0.1), 3);
    }
}
