mod agents;
mod compact;
mod entries;
mod expanded;
mod machines;
mod rows;
pub mod scroll;
mod spaces;
mod tokens;

use std::collections::{HashMap, HashSet};

use config::{AgentPanelSort, Config, Palette};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};

use crate::app::App;
use crate::layout::COMPACT_SIDEBAR;
use crate::model::{AgentRow, MachineStatus, Snapshot, SpaceRow};

pub use entries::{Entry, block, entries, parent_key};
pub use machines::has_diagnostic;
pub use scroll::Metrics;
pub use spaces::draggable;
pub use tokens::{Span, fit};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub collapsed_machines: HashSet<Option<String>>,
    pub hover: Option<Option<String>>,
    pub last: HashMap<Option<String>, String>,
    pub picked: Option<(Option<String>, Option<String>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drag {
    Spaces { grab: u16 },
    Agents { grab: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceHit {
    pub rect: Rect,
    pub key: String,
    pub machine: Option<String>,
    pub indented: bool,
    pub stale: bool,
    pub group: Option<(Rect, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentHit {
    pub rect: Rect,
    pub pane: String,
    pub machine: Option<String>,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineHit {
    pub rect: Rect,
    pub badge: Rect,
    pub toggle: Rect,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub before: Option<String>,
    pub row: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hits {
    pub area: Option<Rect>,
    pub toggle: Option<Rect>,
    pub divider: Option<Rect>,
    pub section_divider: Option<Rect>,
    pub new_workspace: Option<Rect>,
    pub menu: Option<Rect>,
    pub agent_sort: Option<Rect>,
    pub spaces: Vec<SpaceHit>,
    pub agents: Vec<AgentHit>,
    pub machines: Vec<MachineHit>,
    pub spaces_body: Option<Rect>,
    pub agents_body: Option<Rect>,
    pub spaces_bar: Option<Rect>,
    pub agents_bar: Option<Rect>,
    pub spaces_metrics: Metrics,
    pub agents_metrics: Metrics,
    pub slots: Vec<Slot>,
    pub footer: Option<u16>,
    pub active: Option<String>,
}

impl Hits {
    pub fn clear_chrome(&mut self) {
        *self = Self {
            area: self.area,
            toggle: self.toggle,
            spaces_metrics: self.spaces_metrics,
            agents_metrics: self.agents_metrics,
            ..Self::default()
        };
    }

    pub fn space_at(&self, x: u16, y: u16) -> Option<&SpaceHit> {
        self.spaces.iter().find(|h| inside(h.rect, x, y))
    }

    pub fn agent_at(&self, x: u16, y: u16) -> Option<&AgentHit> {
        self.agents.iter().find(|h| inside(h.rect, x, y))
    }

    pub fn machine_at(&self, x: u16, y: u16) -> Option<&MachineHit> {
        self.machines.iter().find(|h| inside(h.rect, x, y))
    }

    pub fn drop_target(&self, x: u16, y: u16) -> Option<&Slot> {
        let body = self.spaces_body?;
        let footer = self.footer?;
        if body.height == 0 || y + 1 < body.y || y >= footer {
            return None;
        }
        if self
            .spaces
            .iter()
            .any(|h| h.machine != self.active && inside(h.rect, x, y))
        {
            return None;
        }
        self.slots.iter().min_by_key(|slot| slot.row.abs_diff(y))
    }
}

pub fn inside(rect: Rect, x: u16, y: u16) -> bool {
    rect.contains(Position::new(x, y))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Press {
    pub key: String,
    pub at: (u16, u16),
    pub now: (u16, u16),
}

impl Press {
    pub fn moved(&self) -> bool {
        self.at
            .0
            .abs_diff(self.now.0)
            .max(self.at.1.abs_diff(self.now.1))
            >= 1
    }
}

pub struct Ctx<'a> {
    pub palette: &'a Palette,
    pub config: &'a Config,
    pub snapshot: &'a Snapshot,
    pub split: f32,
    pub scroll: usize,
    pub agent_scroll: usize,
    pub reveal: bool,
    pub groups: &'a HashSet<String>,
    pub state: &'a State,
    pub press: Option<Press>,
}

impl<'a> Ctx<'a> {
    pub fn mouse(&self) -> bool {
        self.config.ui.mouse_capture
    }

    pub fn multi(&self) -> bool {
        self.snapshot.machines.len() > 1 || !self.snapshot.online
    }

    pub fn navigating(&self) -> bool {
        self.snapshot.spaces.iter().any(|s| s.selected)
    }

    pub fn selection_bg(&self) -> Color {
        if self.palette.selection_bg == Color::Reset {
            self.palette.active_row_bg
        } else {
            self.palette.selection_bg
        }
    }

    pub fn active_bg(&self) -> Color {
        if self.navigating() && self.palette.selection_bg == Color::Reset {
            self.palette.sidebar_bg
        } else {
            self.palette.active_row_bg
        }
    }

    pub fn machine_spaces(&self, name: Option<&str>) -> Vec<&'a SpaceRow> {
        self.snapshot
            .spaces
            .iter()
            .chain(&self.snapshot.stale_spaces)
            .filter(|s| s.machine.as_deref() == name)
            .collect()
    }

    pub fn machine_name(&self, label: &str) -> Option<String> {
        self.snapshot
            .machines
            .iter()
            .find(|m| m.label == label)
            .and_then(|m| m.name.clone())
    }

    pub fn agents(&self) -> Vec<&'a AgentRow> {
        ordered_agents(self.snapshot)
    }
}

pub fn ordered_agents(snapshot: &Snapshot) -> Vec<&AgentRow> {
    let machine = |agent: &AgentRow| {
        snapshot
            .machines
            .iter()
            .position(|m| m.label == agent.machine)
            .unwrap_or(0)
    };
    let mut agents: Vec<&AgentRow> = snapshot
        .agents
        .iter()
        .chain(&snapshot.stale_agents)
        .collect();
    match snapshot.agent_sort {
        AgentPanelSort::Spaces => agents.sort_by_key(|a| machine(a)),
        AgentPanelSort::Priority => agents.sort_by(|a, b| {
            a.stale
                .cmp(&b.stale)
                .then(b.state.priority().cmp(&a.state.priority()))
                .then(b.changed_ms.cmp(&a.changed_ms))
        }),
    }
    agents
}

pub fn status_look(
    status: MachineStatus,
    palette: &Palette,
) -> (&'static str, &'static str, Color) {
    match status {
        MachineStatus::Connecting => ("◐", "connecting", palette.yellow),
        MachineStatus::Online => ("●", "online", palette.green),
        MachineStatus::Reconnecting => ("◐", "reconnecting", palette.yellow),
        MachineStatus::Attention => ("!", "attention", palette.red),
        MachineStatus::Disabled => ("·", "disabled", palette.overlay0),
    }
}

pub fn draw(app: &mut App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect) -> Hits {
    if let Some(space) = &app.view.space
        && let Some(row) = snapshot.space(space)
    {
        app.view
            .sidebar
            .last
            .insert(row.machine.clone(), space.clone());
    }
    let picked = picked(app, snapshot);
    app.view.sidebar.picked.clone_from(&picked);
    let shown;
    let snapshot = match picked {
        Some((machine, _)) => {
            let mut copy = snapshot.clone();
            for row in &mut copy.machines {
                row.active = row.name == machine;
            }
            for space in &mut copy.spaces {
                space.focused = false;
            }
            for agent in &mut copy.agents {
                agent.focused = false;
            }
            shown = copy;
            &shown
        }
        None => snapshot,
    };
    let groups: HashSet<String> = app.prefs.collapsed_groups.iter().cloned().collect();
    let press = match &app.view.drag {
        Some(crate::view::Drag::Press {
            target: crate::view::Press::Space(key),
            at,
            now,
        }) => Some(Press {
            key: key.clone(),
            at: *at,
            now: *now,
        }),
        _ => None,
    };
    let ctx = Ctx {
        palette: &app.palette,
        config: &app.config,
        snapshot,
        split: app.prefs.section_split(),
        scroll: app.view.sidebar_scroll,
        agent_scroll: app.view.agent_scroll,
        reveal: app.view.reveal_space,
        groups: &groups,
        state: &app.view.sidebar,
        press,
    };
    let mut hits = Hits::default();
    render(buf, area, &ctx, &mut hits);
    if hits.area.is_some() {
        app.view.sidebar_scroll = hits.spaces_metrics.start;
        app.view.agent_scroll = hits.agents_metrics.start;
        app.view.reveal_space = false;
    }
    hits
}

impl App {
    pub fn space_order(&self) -> Vec<String> {
        let snapshot = self.snapshot();
        let mobile = self.regions.mobile;
        if snapshot.collapsed && !mobile {
            return snapshot.spaces.iter().map(|s| s.key.clone()).collect();
        }
        let groups: HashSet<String> = if mobile {
            HashSet::new()
        } else {
            self.prefs.collapsed_groups.iter().cloned().collect()
        };
        let mut order = Vec::new();
        for machine in &snapshot.machines {
            let spaces: Vec<&SpaceRow> = snapshot
                .spaces
                .iter()
                .filter(|s| s.machine == machine.name)
                .collect();
            order.extend(
                entries(&spaces, &groups)
                    .into_iter()
                    .map(|e| spaces[e.index].key.clone()),
            );
        }
        order
    }
}

pub fn picked(app: &App, snapshot: &Snapshot) -> Option<(Option<String>, Option<String>)> {
    let (machine, space) = app.view.sidebar.picked.clone()?;
    let empty = !snapshot.spaces.iter().any(|s| s.machine == machine);
    let online = snapshot
        .machines
        .iter()
        .any(|m| m.name == machine && m.status == MachineStatus::Online);
    (empty && online && space == app.view.space).then_some((machine, space))
}

pub fn render(buf: &mut Buffer, area: Rect, ctx: &Ctx<'_>, hits: &mut Hits) {
    *hits = Hits::default();
    if area.width == 0 || area.height == 0 {
        return;
    }
    hits.area = Some(area);
    hits.active = ctx
        .snapshot
        .machines
        .iter()
        .find(|m| m.active)
        .and_then(|m| m.name.clone());
    background(buf, area, ctx.palette);
    let compact = ctx.snapshot.collapsed && area.width <= COMPACT_SIDEBAR;
    match (ctx.multi(), compact) {
        (true, true) => machines::compact(buf, area, ctx, hits),
        (true, false) => machines::expanded(buf, area, ctx, hits),
        (false, true) => compact::render(buf, area, ctx, hits),
        (false, false) => expanded::render(buf, area, ctx, hits),
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
    let rows = (f32::from(h) * split.clamp(0.1, 0.9)).round() as u16;
    rows.clamp(3, h - 3)
}

pub fn sections(area: Rect, split: f32) -> (Rect, Rect) {
    let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    let top = spaces_height(content.height, split);
    (
        Rect::new(content.x, content.y, content.width, top),
        Rect::new(
            content.x,
            content.y + top,
            content.width,
            content.height - top,
        ),
    )
}

pub fn compact_sections(area: Rect) -> (Rect, Option<u16>, Rect) {
    let content = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    if content.is_empty() {
        return (Rect::default(), None, Rect::default());
    }
    if content.height < 7 {
        return (content, None, Rect::default());
    }
    let top = content.height.div_ceil(2);
    let divider = content.y + top;
    (
        Rect::new(content.x, content.y, content.width, top),
        Some(divider),
        Rect::new(
            content.x,
            divider + 1,
            content.width,
            content.height - top - 1,
        ),
    )
}

pub fn put(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) -> u16 {
    if width == 0 || !buf.area.contains(Position::new(x, y)) {
        return x;
    }
    buf.set_stringn(x, y, text, usize::from(width), style).0
}

pub fn put_right(buf: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) -> Rect {
    let width = crate::text::width(text).min(area.width);
    let x = area.right() - width;
    put(buf, x, y, width, text, style);
    Rect::new(x, y, width, 1)
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

    #[test]
    fn compact_sections_need_seven_rows() {
        let (top, divider, rest) = compact_sections(Rect::new(0, 0, 4, 20));
        assert_eq!(
            (top.height, divider, rest),
            (10, Some(10), Rect::new(0, 11, 3, 9))
        );
        let (top, divider, _) = compact_sections(Rect::new(0, 0, 4, 6));
        assert_eq!((top.height, divider), (6, None));
    }
}
