use std::collections::HashMap;

use config::{Action, Palette, StatusIndicators};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::agents::State;
use crate::app::App;
use crate::model::Snapshot;
use crate::overlay::scroll::Bar;
use crate::render::mobile::{SWITCH, switch_button};
use crate::text::truncate;
use crate::view::Mode;

const WHEEL: usize = 2;
const MENU: [(&str, Action); 4] = [
    ("settings", Action::Settings),
    ("keybinds", Action::Help),
    ("reload config", Action::ReloadConfig),
    ("detach", Action::Detach),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Machine(Option<String>),
    Agent(String),
    NewWorkspace,
    Space(String),
    NewTab,
    Tab(String),
    Menu(usize),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hits {
    pub close: Option<Rect>,
    pub targets: Vec<(Rect, Target)>,
    pub max: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub lines: Vec<Line<'static>>,
    pub bg: Color,
    pub target: Option<Target>,
}

fn section(label: &str, p: &Palette) -> Item {
    Item {
        lines: vec![Line::from(Span::styled(
            format!(" {label} "),
            Style::new()
                .fg(p.overlay1)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))],
        bg: p.panel_bg,
        target: None,
    }
}

fn action(label: &'static str, target: Target, p: &Palette) -> Item {
    Item {
        lines: vec![Line::from(Span::styled(
            label,
            Style::new()
                .fg(p.accent)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD),
        ))],
        bg: p.panel_bg,
        target: Some(target),
    }
}

fn machine_word(state: &str) -> (&'static str, &str, fn(&Palette) -> Color) {
    match state {
        "online" | "connected" => ("●", "online", |p| p.green),
        "connecting" => ("◐", "connecting", |p| p.yellow),
        "reconnecting" => ("◐", "reconnecting", |p| p.yellow),
        "disabled" => ("·", "disabled", |p| p.overlay0),
        state => ("!", state, |p| p.red),
    }
}

pub struct Ctx<'a> {
    pub snapshot: &'a Snapshot,
    pub palette: &'a Palette,
    pub indicators: StatusIndicators,
    pub nav: Option<&'a str>,
    pub tabs: &'a HashMap<String, String>,
    pub width: u16,
}

fn machines(ctx: &Ctx<'_>, items: &mut Vec<Item>) {
    let p = ctx.palette;
    if ctx.snapshot.machines.len() <= 1 {
        return;
    }
    items.push(section("machines", p));
    for machine in &ctx.snapshot.machines {
        let (glyph, state, color) = machine_word(&machine.state);
        items.push(Item {
            lines: vec![
                Line::from(vec![
                    Span::styled("  ", Style::new().bg(p.panel_bg)),
                    Span::styled(glyph, Style::new().fg(color(p)).bg(p.panel_bg)),
                    Span::styled(
                        format!(" {}", machine.label),
                        Style::new()
                            .fg(p.text)
                            .bg(p.panel_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(Span::styled(
                    format!("    {state}"),
                    Style::new().fg(p.overlay0).bg(p.panel_bg),
                )),
            ],
            bg: p.panel_bg,
            target: Some(Target::Machine(machine.name.clone())),
        });
    }
}

fn stale(ctx: &Ctx<'_>, machine: Option<&str>) -> Option<&'static str> {
    let row = ctx
        .snapshot
        .machines
        .iter()
        .find(|m| m.name.as_deref() == machine)?;
    (!row.connected)
        .then(|| machine_word(&row.state).1)
        .map(|s| match s {
            "connecting" => "connecting",
            "reconnecting" => "reconnecting",
            "disabled" => "disabled",
            _ => "attention",
        })
}

fn agents(ctx: &Ctx<'_>, items: &mut Vec<Item>) {
    let p = ctx.palette;
    let agents = ctx.snapshot.ordered_agents();
    if agents.is_empty() {
        return;
    }
    items.push(section("agents", p));
    for agent in agents {
        let space = ctx.snapshot.space(&agent.space);
        let machine = space.and_then(|s| s.machine.as_deref());
        let stale = stale(ctx, machine);
        let mut detail = Vec::new();
        if let Some(tab) = &agent.tab_label {
            detail.push(tab.clone());
        }
        detail.push(
            if agent.state == State::Unknown {
                "idle"
            } else {
                agent.state.text()
            }
            .to_owned(),
        );
        let label = if agent.agent.is_empty() {
            "agent"
        } else {
            agent.agent.as_str()
        };
        detail.push(label.to_owned());
        if let Some(state) = stale {
            detail.push(state.to_owned());
        }
        let bg = if agent.focused {
            p.surface_dim
        } else {
            p.panel_bg
        };
        let (fg, icon, dim) = if stale.is_some() {
            (p.overlay0, p.overlay0, Modifier::DIM)
        } else {
            (p.text, agent.state.color(p), Modifier::empty())
        };
        let primary = if agent.workspace.is_empty() {
            label
        } else {
            agent.workspace.as_str()
        };
        items.push(Item {
            lines: vec![
                Line::from(vec![
                    Span::styled("  ", Style::new().bg(bg)),
                    Span::styled(
                        agent.state.icon(ctx.indicators),
                        Style::new().fg(icon).bg(bg).add_modifier(dim),
                    ),
                    Span::styled(" ", Style::new().bg(bg)),
                    Span::styled(
                        truncate(
                            &format!("{} · {primary}", agent.machine),
                            usize::from(ctx.width.saturating_sub(5)),
                        ),
                        Style::new()
                            .fg(fg)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD | dim),
                    ),
                ]),
                Line::from(Span::styled(
                    truncate(&format!("  {}", detail.join(" · ")), usize::from(ctx.width)),
                    Style::new().fg(p.overlay0).bg(bg).add_modifier(dim),
                )),
            ],
            bg,
            target: Some(Target::Agent(agent.pane.clone())),
        });
    }
}

fn spaces(ctx: &Ctx<'_>, items: &mut Vec<Item>) {
    let p = ctx.palette;
    items.push(section("spaces", p));
    items.push(action("  + new workspace", Target::NewWorkspace, p));
    for space in &ctx.snapshot.spaces {
        let selected = ctx.nav == Some(space.key.as_str());
        let bg = if selected {
            if p.surface0 == Color::Reset {
                p.active_row_bg
            } else {
                p.surface0
            }
        } else if space.focused {
            if ctx.nav.is_some() && p.surface0 == Color::Reset {
                p.panel_bg
            } else {
                p.surface_dim
            }
        } else {
            p.panel_bg
        };
        let stale = stale(ctx, space.machine.as_deref());
        let dim = if stale.is_some() {
            Modifier::DIM
        } else {
            Modifier::empty()
        };
        let state = space.state.unwrap_or(State::Unknown);
        let (fg, icon) = if stale.is_some() {
            (p.overlay0, p.overlay0)
        } else {
            (p.text, state.color(p))
        };
        let tab = ctx.tabs.get(&space.key).map_or("tab 1", String::as_str);
        let stale_detail = stale.map(|s| format!(" · {s}")).unwrap_or_default();
        let branch = space.branch.as_deref().unwrap_or("shell");
        items.push(Item {
            lines: vec![
                Line::from(vec![
                    Span::styled("  ", Style::new().fg(p.overlay0).bg(bg).add_modifier(dim)),
                    Span::styled(
                        state.icon(ctx.indicators),
                        Style::new().fg(icon).bg(bg).add_modifier(dim),
                    ),
                    Span::styled(" ", Style::new().bg(bg)),
                    Span::styled(
                        truncate(
                            &format!("{} · {}", space.machine_label, space.name),
                            usize::from(ctx.width.saturating_sub(5)),
                        ),
                        Style::new()
                            .fg(fg)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD | dim),
                    ),
                ]),
                Line::from(Span::styled(
                    truncate(
                        &format!("  {branch} · {tab}{stale_detail}"),
                        usize::from(ctx.width),
                    ),
                    Style::new().fg(p.overlay0).bg(bg).add_modifier(dim),
                )),
            ],
            bg,
            target: Some(Target::Space(space.key.clone())),
        });
    }
}

fn tabs(ctx: &Ctx<'_>, items: &mut Vec<Item>) {
    let p = ctx.palette;
    if ctx.snapshot.active_space.is_none() {
        return;
    }
    items.push(section("tabs", p));
    items.push(action("  + new tab", Target::NewTab, p));
    for (index, tab) in ctx.snapshot.tabs.iter().enumerate() {
        let bg = if tab.focused {
            p.surface_dim
        } else {
            p.panel_bg
        };
        let label = if tab.custom {
            format!("{} · {}", index + 1, tab.label)
        } else {
            format!("tab {}", tab.label)
        };
        items.push(Item {
            lines: vec![Line::from(Span::styled(
                format!(
                    "  {}",
                    truncate(&label, usize::from(ctx.width.saturating_sub(3)))
                ),
                Style::new().fg(p.text).bg(bg).add_modifier(Modifier::BOLD),
            ))],
            bg,
            target: Some(Target::Tab(tab.key.clone())),
        });
    }
}

pub fn items(ctx: &Ctx<'_>) -> Vec<Item> {
    let p = ctx.palette;
    let mut items = Vec::new();
    machines(ctx, &mut items);
    agents(ctx, &mut items);
    spaces(ctx, &mut items);
    tabs(ctx, &mut items);
    items.push(section("menu", p));
    for (index, (label, _)) in MENU.iter().enumerate() {
        items.push(Item {
            lines: vec![Line::from(Span::styled(
                format!("  {label}"),
                Style::new().fg(p.overlay1).bg(p.panel_bg),
            ))],
            bg: p.panel_bg,
            target: Some(Target::Menu(index)),
        });
    }
    items
}

fn close_button(buf: &mut Buffer, area: Rect, p: &Palette) {
    buf.set_style(area, Style::new().bg(p.surface0));
    for y in area.top()..area.bottom() {
        buf.set_stringn(
            area.x,
            y,
            "│",
            1,
            Style::new().fg(p.surface_dim).bg(p.surface0),
        );
    }
    buf.set_stringn(
        area.x + 1 + area.width.saturating_sub(6) / 2,
        area.y,
        "close",
        usize::from(area.width.saturating_sub(1)),
        Style::new()
            .fg(p.overlay1)
            .bg(p.surface0)
            .add_modifier(Modifier::BOLD),
    );
    if area.height > 1 {
        buf.set_stringn(
            area.x + area.width / 2,
            area.y + 1,
            "×",
            1,
            Style::new()
                .fg(p.text)
                .bg(p.surface0)
                .add_modifier(Modifier::BOLD),
        );
    }
}

fn rule(buf: &mut Buffer, area: Rect, y: u16, p: &Palette) {
    buf.set_stringn(
        area.x,
        y,
        "─".repeat(usize::from(area.width)),
        usize::from(area.width),
        Style::new().fg(p.surface_dim).bg(p.panel_bg),
    );
}

fn tab_statuses(app: &App) -> HashMap<String, String> {
    app.machines
        .spaces()
        .iter()
        .map(|space| {
            let key = space.key();
            let tabs: Vec<String> = space.tabs().map(|t| t.key()).collect();
            let active = app.view.tab_of_space(&app.machines, &key);
            let at = active
                .and_then(|a| tabs.iter().position(|t| *t == a))
                .unwrap_or(0);
            let label = space
                .tabs()
                .nth(at)
                .map_or_else(|| "1".to_owned(), |t| t.name());
            let status = if tabs.len() <= 1 {
                format!("tab {label}")
            } else {
                format!("tab {label} · {}/{}", at + 1, tabs.len())
            };
            (key, status)
        })
        .collect()
}

pub fn render(app: &mut App, snapshot: &Snapshot, buf: &mut Buffer, area: Rect) -> Hits {
    let p = &app.palette;
    let mut hits = Hits::default();
    if area.width == 0 || area.height == 0 {
        return hits;
    }
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].reset();
        }
    }
    buf.set_style(area, Style::new().bg(p.panel_bg));
    if area.height <= 2 {
        app.view.switcher_scroll = 0;
        rule(buf, area, area.y, p);
        return hits;
    }
    let close = switch_button(Rect::new(area.x, area.y, area.width, 2));
    hits.close = Some(close);
    buf.set_stringn(
        area.x,
        area.y,
        " switch",
        usize::from(close.x - area.x),
        Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD),
    );
    close_button(buf, close, p);
    rule(buf, area, area.y + 2, p);
    let viewport = Rect::new(area.x, area.y + 3, area.width, area.height - 3);
    if viewport.height == 0 {
        app.view.switcher_scroll = 0;
        return hits;
    }
    let nav = (app.view.mode == Mode::Navigate)
        .then(|| app.view.nav.clone())
        .flatten();
    let tabs = tab_statuses(app);
    let p = &app.palette;
    let items = items(&Ctx {
        snapshot,
        palette: p,
        indicators: app.config.ui.status_indicators,
        nav: nav.as_deref(),
        tabs: &tabs,
        width: viewport.width.saturating_sub(1),
    });
    let height = usize::from(viewport.height);
    let total: usize = items.iter().map(|i| i.lines.len()).sum();
    let max = total.saturating_sub(height);
    let mut scroll = app.view.switcher_scroll.min(max);
    if app.view.reveal_space {
        let mut start = 0;
        for item in &items {
            let end = start + item.lines.len();
            if matches!(&item.target, Some(Target::Space(key)) if nav.as_deref() == Some(key.as_str()))
            {
                if start < scroll {
                    scroll = start;
                } else if end > scroll + height {
                    scroll = (end - height).min(max);
                }
                break;
            }
            start = end;
        }
        app.view.reveal_space = false;
    }
    app.view.switcher_scroll = scroll;
    hits.max = max;
    if max > 0 {
        let track = Rect::new(viewport.x, viewport.y, 1, viewport.height);
        let bar = Bar::new(track, max, height);
        for y in track.top()..track.bottom() {
            buf[(track.x, y)]
                .set_symbol("│")
                .set_style(Style::new().fg(p.surface_dim).bg(p.panel_bg));
        }
        bar.render(
            buf,
            scroll,
            ("│", p.surface_dim),
            ("▌", p.accent),
            p.panel_bg,
        );
    }
    if viewport.width <= 1 {
        return hits;
    }
    let content = Rect::new(
        viewport.x + 1,
        viewport.y,
        viewport.width - 1,
        viewport.height,
    );
    let mut row = 0;
    for item in items {
        let (from, to) = (row, row + item.lines.len());
        row = to;
        let (shown_from, shown_to) = (from.max(scroll), to.min(scroll + height));
        if shown_from >= shown_to {
            continue;
        }
        let y = content.y + u16::try_from(shown_from - scroll).unwrap_or(0);
        let h = u16::try_from(shown_to - shown_from).unwrap_or(0);
        let rect = Rect::new(content.x, y, content.width, h);
        buf.set_style(rect, Style::new().bg(item.bg));
        for line in shown_from..shown_to {
            let y = content.y + u16::try_from(line - scroll).unwrap_or(0);
            buf.set_line(content.x, y, &item.lines[line - from], content.width);
        }
        if let Some(target) = item.target {
            hits.targets.push((rect, target));
        }
    }
    hits
}

impl App {
    pub fn switcher_open(&self) -> bool {
        self.regions.mobile && self.view.mode == Mode::Navigate && self.view.overlay.is_none()
    }

    pub fn switcher_mouse(&mut self, event: &MouseEvent) -> bool {
        if !self.regions.mobile || self.view.overlay.is_some() {
            return false;
        }
        let at = Position::new(event.column, event.row);
        let down = event.kind == MouseEventKind::Down(MouseButton::Left);
        if self.view.mode != Mode::Navigate {
            let button = self
                .regions
                .header
                .filter(|h| h.width > SWITCH + 4)
                .map(switch_button);
            if matches!(self.view.mode, Mode::Terminal | Mode::Resize)
                && down
                && button.is_some_and(|b| b.contains(at))
            {
                self.view.switcher_scroll = 0;
                self.view.reveal_space = false;
                self.view.highlight = None;
                self.view.nav = self.view.space.clone();
                self.view.mode = Mode::Navigate;
                return true;
            }
            return false;
        }
        match event.kind {
            MouseEventKind::ScrollUp => {
                self.view.switcher_scroll = self.view.switcher_scroll.saturating_sub(WHEEL);
                return true;
            }
            MouseEventKind::ScrollDown => {
                self.view.switcher_scroll =
                    (self.view.switcher_scroll + WHEEL).min(self.hits.switcher.max);
                return true;
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return true,
        }
        if self.hits.switcher.close.is_some_and(|c| c.contains(at)) {
            self.leave_switcher();
            return true;
        }
        let target = self
            .hits
            .switcher
            .targets
            .iter()
            .find(|(rect, _)| rect.contains(at))
            .map(|(_, t)| t.clone());
        if let Some(target) = target {
            self.switcher_pick(target);
        }
        true
    }

    fn leave_switcher(&mut self) {
        self.view.nav = None;
        self.leave_mode();
    }

    fn switcher_pick(&mut self, target: Target) {
        match target {
            Target::Machine(name) => {
                let first = self
                    .snapshot()
                    .spaces
                    .iter()
                    .find(|s| s.machine == name)
                    .map(|s| s.key.clone());
                self.leave_switcher();
                if let Some(space) = first {
                    self.focus_space(&space);
                }
            }
            Target::Agent(pane) => {
                self.leave_switcher();
                self.focus_pane(&pane);
            }
            Target::Space(space) => {
                self.leave_switcher();
                self.focus_space(&space);
            }
            Target::Tab(tab) => {
                self.leave_switcher();
                self.focus_tab(&tab);
            }
            Target::NewWorkspace => {
                self.leave_switcher();
                self.run_action(Action::NewWorkspace);
            }
            Target::NewTab => {
                self.leave_switcher();
                self.run_action(Action::NewTab);
            }
            Target::Menu(index) => {
                self.leave_switcher();
                if let Some((_, action)) = MENU.get(index) {
                    self.run_action(*action);
                }
            }
        }
    }
}
