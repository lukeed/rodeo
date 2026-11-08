use config::{Palette, StatusIndicators};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

use super::frame::{contrast, panel, text};
use super::help::typed;
use super::scroll::Bar;
use super::{Ctx, Drawn, Focus, Kind, Outcome, Overlay, Request, TextField};
use crate::agents::State;
use crate::ids;
use crate::model::{MachineRow, PaneRow, Snapshot, SpaceRow};
use crate::text::width;

const PAGE: isize = 8;
const WHEEL: isize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Machine(Option<String>),
    Space(String),
    Pane(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub depth: u16,
    pub label: String,
    pub meta: String,
    pub detail: String,
    pub agent: Option<String>,
    pub status: Option<State>,
    pub stale: bool,
    pub current: bool,
    pub target: Target,
}

impl Row {
    fn is_pane(&self) -> bool {
        matches!(self.target, Target::Pane(_))
    }
}

struct Search<'a> {
    words: Vec<String>,
    filter: Option<State>,
    machines: &'a [MachineRow],
}

impl Search<'_> {
    fn text(&self, value: &str) -> bool {
        let value = value.to_lowercase();
        self.words.iter().all(|word| value.contains(word.as_str()))
    }

    fn filtering(&self) -> bool {
        self.filter.is_some() || !self.words.is_empty()
    }

    fn status(&self, state: State) -> bool {
        self.filter.is_none_or(|f| f == state)
    }
}

fn pane_row(pane: &PaneRow, index: usize, multiple_tabs: bool, workspace: &str) -> Row {
    let status = pane.state.unwrap_or(State::Unknown);
    let agent = pane.agent.as_deref();
    let name = pane.title.as_deref();
    let title = pane
        .state
        .and(pane.agent_title.as_deref())
        .filter(|t| !t.is_empty());
    let tab_name =
        (pane.tab_custom || pane.tab_label.parse::<usize>().is_err()).then_some(&*pane.tab_label);
    let label = if pane.tab_panes == 1 {
        match name.or(tab_name).or(title) {
            Some(label) => label.to_owned(),
            None if multiple_tabs => {
                format!("{} · {}", agent.unwrap_or("terminal"), pane.tab_label)
            }
            None => workspace.to_owned(),
        }
    } else {
        let pane_name = name.or(title).or(agent).unwrap_or("terminal");
        match tab_name {
            Some(tab) if tab != pane_name => format!("{tab} · {pane_name} · {}", index + 1),
            _ => format!("{pane_name} · {}", index + 1),
        }
    };
    Row {
        depth: 1,
        meta: pane.cwd.clone(),
        detail: format!(
            "{workspace} / {} / {}",
            pane.tab_label,
            ids::split(&pane.key).1
        ),
        label,
        agent: agent.map(str::to_owned),
        status: Some(status),
        stale: false,
        current: pane.focused,
        target: Target::Pane(pane.key.clone()),
    }
}

fn pane_matches(row: &Row, pane: &PaneRow, search: &Search<'_>) -> bool {
    search.text(&row.label)
        || search.text(&row.meta)
        || pane.agent.as_deref().is_some_and(|a| search.text(a))
        || pane
            .state
            .and(pane.agent_title.as_deref())
            .is_some_and(|t| search.text(t))
        || search.text(ids::split(&pane.key).1)
}

fn space_rows(
    snapshot: &Snapshot,
    space: &SpaceRow,
    search: &Search<'_>,
    machine_hit: bool,
) -> Vec<Row> {
    let matched = machine_hit
        || search.text(&space.name)
        || space.branch.as_deref().is_some_and(|b| search.text(b));
    let panes: Vec<&PaneRow> = snapshot
        .panes
        .iter()
        .filter(|p| p.space == space.key)
        .collect();
    let multiple_tabs = space.tabs > 1;
    let mut children = Vec::new();
    let mut index = 0;
    let mut tab = None;
    for pane in panes {
        if tab != Some(pane.tab.as_str()) {
            tab = Some(pane.tab.as_str());
            index = 0;
        }
        let row = pane_row(pane, index, multiple_tabs, &space.name);
        index += 1;
        let tab_hit = matched || search.text(&pane.tab_label);
        if search.status(row.status.unwrap_or(State::Unknown))
            && (tab_hit || pane_matches(&row, pane, search))
        {
            children.push(row);
        }
    }
    let shown = !search.filtering()
        || !children.is_empty()
        || (search.filter.is_none() && !search.words.is_empty() && matched);
    if !shown {
        return Vec::new();
    }
    let mut rows = vec![Row {
        depth: 0,
        label: space.name.clone(),
        meta: space.branch.clone().unwrap_or_default(),
        detail: space.folder.clone(),
        agent: None,
        status: None,
        stale: false,
        current: false,
        target: Target::Space(space.key.clone()),
    }];
    rows.extend(children);
    rows
}

pub fn rows(snapshot: &Snapshot, query: &str, filter: Option<State>) -> Vec<Row> {
    let query = query.trim().to_lowercase();
    let search = Search {
        words: query.split_whitespace().map(str::to_owned).collect(),
        filter,
        machines: &snapshot.machines,
    };
    let federated = search.machines.len() > 1;
    let mut out = Vec::new();
    let machines: Vec<Option<&MachineRow>> = if search.machines.is_empty() {
        vec![None]
    } else {
        search.machines.iter().map(Some).collect()
    };
    for machine in machines {
        let name = machine.and_then(|m| m.name.clone());
        let stale = machine.is_some_and(|m| !m.connected);
        let machine_hit =
            !search.words.is_empty() && machine.is_some_and(|m| search.text(&m.label));
        let mut rows = Vec::new();
        for space in snapshot.spaces.iter().filter(|s| s.machine == name) {
            rows.extend(space_rows(snapshot, space, &search, machine_hit));
        }
        for row in &mut rows {
            row.stale = stale;
            row.depth += u16::from(federated);
            row.current &= !stale;
        }
        if !search.filtering() || machine_hit || !rows.is_empty() {
            if federated && let Some(machine) = machine {
                out.push(Row {
                    depth: 0,
                    label: machine.label.clone(),
                    meta: String::new(),
                    detail: String::new(),
                    agent: None,
                    status: None,
                    stale,
                    current: false,
                    target: Target::Machine(name),
                });
            }
            out.extend(rows);
        }
    }
    out
}

fn signal(machine: &MachineRow, p: &Palette) -> (String, Color) {
    match machine.state.as_str() {
        "online" | "connected" => ("●".to_owned(), p.green),
        state @ ("connecting" | "reconnecting") => (format!("◐ {state}"), p.yellow),
        "disabled" => ("· disabled".to_owned(), p.overlay0),
        state => (format!("! {state}"), p.red),
    }
}

#[derive(Default)]
pub struct Goto {
    query: TextField,
    searching: bool,
    selected: Option<Target>,
    scroll: usize,
    filter: Option<State>,
    shown: usize,
    popup: Rect,
    search: Rect,
    hits: Vec<(Rect, Target)>,
    bar: Option<Bar>,
    grab: Option<u16>,
}

impl Goto {
    pub fn new(snapshot: &Snapshot) -> Self {
        let selected = rows(snapshot, "", None)
            .into_iter()
            .find(|r| r.current)
            .map(|r| r.target);
        Self {
            selected,
            ..Self::default()
        }
    }

    pub fn query(&self) -> &str {
        self.query.text()
    }

    pub fn filter(&self) -> Option<State> {
        self.filter
    }

    pub fn searching(&self) -> bool {
        self.searching
    }

    pub fn selected(&self) -> Option<&Target> {
        self.selected.as_ref()
    }

    fn rows(&self, snapshot: &Snapshot) -> Vec<Row> {
        rows(snapshot, self.query.text(), self.filter)
    }

    fn index(&self, rows: &[Row]) -> Option<usize> {
        match &self.selected {
            Some(target) => rows.iter().position(|r| &r.target == target),
            None => rows
                .iter()
                .position(Row::is_pane)
                .or_else(|| (!rows.is_empty()).then_some(0)),
        }
    }

    fn step(&mut self, snapshot: &Snapshot, delta: isize) {
        let rows = self.rows(snapshot);
        if rows.is_empty() {
            self.selected = None;
            return;
        }
        let at = self.index(&rows).unwrap_or(0);
        let next = at.saturating_add_signed(delta).min(rows.len() - 1);
        self.selected = Some(rows[next].target.clone());
    }

    fn section(&mut self, snapshot: &Snapshot, forward: bool) {
        let rows = self.rows(snapshot);
        let Some(at) = self.index(&rows) else {
            return;
        };
        let section = rows[..=at].iter().rposition(|r| !r.is_pane()).unwrap_or(at);
        let mut starts = rows.windows(2).enumerate().filter(|(i, pair)| {
            matches!(pair[0].target, Target::Space(_))
                && pair[1].is_pane()
                && if forward { *i > section } else { *i < section }
        });
        let found = if forward {
            starts.next()
        } else {
            starts.next_back()
        };
        if let Some((_, pair)) = found {
            self.selected = Some(pair[1].target.clone());
        }
    }

    fn open(&self, snapshot: &Snapshot) -> Outcome {
        let rows = self.rows(snapshot);
        let Some(row) = self.index(&rows).map(|i| &rows[i]) else {
            return Outcome::Stay;
        };
        if row.stale {
            return Outcome::Stay;
        }
        let focus = match &row.target {
            Target::Pane(pane) => Focus::Pane(pane.clone()),
            Target::Space(space) => Focus::Space(space.clone()),
            Target::Machine(name) => {
                let Some(space) = snapshot.spaces.iter().find(|s| &s.machine == name) else {
                    return Outcome::Stay;
                };
                Focus::Space(space.key.clone())
            }
        };
        Outcome::Done(Request::Focus(focus))
    }

    fn search_key(&mut self, key: &KeyEvent, snapshot: &Snapshot) -> Outcome {
        let before = self.query.text().to_owned();
        if self.query.key(key) {
            if self.query.text() != before {
                self.filter = None;
                self.selected = None;
            }
            return Outcome::Stay;
        }
        let ctrl = key.modifiers == KeyModifiers::CONTROL;
        match key.code {
            KeyCode::Up => self.step(snapshot, -1),
            KeyCode::Down => self.step(snapshot, 1),
            KeyCode::Char('p') if ctrl => self.step(snapshot, -1),
            KeyCode::Char('n') if ctrl => self.step(snapshot, 1),
            _ => {}
        }
        Outcome::Stay
    }

    fn set_filter(&mut self, filter: Option<State>) {
        self.query.clear();
        self.filter = filter;
        self.selected = None;
    }

    fn trailer(
        buf: &mut Buffer,
        rect: Rect,
        row: &Row,
        chosen: bool,
        style: Style,
        label: u16,
        ctx: &Ctx<'_>,
    ) {
        let p = ctx.palette;
        let machine = match &row.target {
            Target::Machine(Some(name)) => ctx
                .snapshot
                .machines
                .iter()
                .find(|m| m.name.as_deref() == Some(name.as_str())),
            _ => None,
        };
        if let Some(machine) = machine {
            let (glyph, color) = signal(machine, p);
            let look = if chosen {
                style
            } else {
                let look = Style::new().fg(color).bg(p.panel_bg);
                if row.stale {
                    look.add_modifier(Modifier::DIM)
                } else {
                    look
                }
            };
            right(buf, rect, &glyph, look);
        } else if row.status.is_none() && !row.meta.is_empty() {
            let used = label.min(rect.width) + 1;
            let meta = Rect::new(
                rect.x + used.min(rect.width),
                rect.y,
                rect.width.saturating_sub(used),
                1,
            );
            right(buf, meta, &row.meta, style);
        }
    }

    fn draw_row(
        buf: &mut Buffer,
        rect: Rect,
        rows: &[Row],
        index: usize,
        selected: usize,
        ctx: &Ctx<'_>,
    ) {
        let p = ctx.palette;
        let row = &rows[index];
        let chosen = index == selected;
        let mut style = if row.stale {
            Style::new()
                .fg(p.overlay0)
                .bg(if chosen { p.surface0 } else { p.panel_bg })
                .add_modifier(Modifier::DIM)
        } else if chosen {
            Style::new()
                .fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            let fg = if matches!(row.target, Target::Machine(_)) {
                p.subtext0
            } else {
                p.text
            };
            Style::new().fg(fg).bg(p.panel_bg)
        };
        if row.status.is_none() {
            style = style.add_modifier(Modifier::BOLD);
        }
        let plain = row.stale || chosen;
        let pane = row.is_pane();
        let connector = match (pane, rows.get(index + 1)) {
            (false, _) => "",
            (true, Some(next)) if next.is_pane() => "├─ ",
            (true, _) => "└─ ",
        };
        let padding = row.depth.saturating_sub(u16::from(pane)) * 2 + 1;
        let indent = format!("{:width$}{connector}", "", width = usize::from(padding));
        let current = if row.current { "◆ " } else { "" };
        let dot = row
            .status
            .map(|s| s.icon(StatusIndicators::Dots))
            .unwrap_or_default();
        let gap = if dot.is_empty() { "" } else { " " };
        let label = format!("{indent}{current}{dot}{gap}{}", row.label);
        buf.set_style(rect, style);
        let columns = match (row.status, rect.width) {
            (Some(_), w) if w >= 64 => 24,
            (Some(_), w) if w >= 36 => 12,
            _ => 0,
        };
        text(
            buf,
            rect.x,
            rect.y,
            rect.width.saturating_sub(columns),
            &label,
            style,
        );
        if pane {
            let x = rect.x + padding;
            let fg = if plain { style } else { style.fg(p.overlay0) };
            text(
                buf,
                x,
                rect.y,
                rect.right().saturating_sub(x).min(2),
                connector,
                fg,
            );
        }
        if let Some(state) = row.status {
            let x = rect.x + width(&format!("{indent}{current}"));
            let dot_style = if plain {
                style
            } else {
                Style::new().fg(state.color(p)).bg(p.panel_bg)
            };
            text(buf, x, rect.y, width(dot), dot, dot_style);
            let meta = if plain { style } else { style.fg(p.overlay0) };
            if columns > 0 {
                let agent = row.agent.as_deref().unwrap_or("terminal");
                text(buf, rect.right() - columns + 1, rect.y, 11, agent, meta);
            }
            if columns == 24 {
                let word = if row.agent.is_some() {
                    state.text()
                } else {
                    "shell"
                };
                text(buf, rect.right() - 11, rect.y, 11, word, meta);
            }
        }
        Self::trailer(buf, rect, row, chosen, style, width(&label), ctx);
    }
}

fn right(buf: &mut Buffer, rect: Rect, value: &str, style: Style) {
    let w = width(value).min(rect.width);
    text(buf, rect.right() - w, rect.y, w, value, style);
}

impl Overlay for Goto {
    fn kind(&self) -> Kind {
        Kind::Goto
    }

    fn dims(&self) -> bool {
        false
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let p = ctx.palette;
        let w = area.width.saturating_sub(4).min(116);
        let h = area.height.saturating_sub(2).min(42);
        if w < 4 || h < 9 {
            return None;
        }
        let outer = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        self.popup = outer;
        text(
            buf,
            outer.x + 2,
            outer.y,
            outer.width.saturating_sub(4),
            " Go to ",
            Style::new().fg(p.accent).bg(p.panel_bg),
        );
        let rows = self.rows(ctx.snapshot);
        let cursor = self.header(buf, inner, &rows, p);
        let selected = self.body(buf, inner, &rows, ctx);
        self.footer(buf, inner, rows.get(selected), p);
        Some(Drawn::new(outer, cursor))
    }

    fn key(&mut self, key: &KeyEvent, ctx: &Ctx<'_>) -> Outcome {
        let snapshot = ctx.snapshot;
        match key.code {
            KeyCode::Esc => {
                if !self.searching {
                    return Outcome::Close;
                }
                self.searching = false;
                return Outcome::Stay;
            }
            KeyCode::Enter => return self.open(snapshot),
            _ => {}
        }
        if self.searching {
            return self.search_key(key, snapshot);
        }
        let bare = key.modifiers.is_empty();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Left if bare => self.section(snapshot, false),
            KeyCode::Right if bare => self.section(snapshot, true),
            KeyCode::Backspace if bare => {
                if self.filter.take().is_some() {
                    self.selected = None;
                }
            }
            KeyCode::Home if bare => {
                self.selected = None;
                self.scroll = 0;
            }
            KeyCode::End if bare => self.last(snapshot),
            KeyCode::Down if bare => self.step(snapshot, 1),
            KeyCode::Up if bare => self.step(snapshot, -1),
            KeyCode::Char('d') if ctrl => self.step(snapshot, PAGE),
            KeyCode::Char('u') if ctrl => self.step(snapshot, -PAGE),
            _ => match (typed(key), bare) {
                (Some('/'), _) => {
                    self.searching = true;
                    self.filter = None;
                }
                (Some('G'), _) => self.last(snapshot),
                (Some('j'), true) => self.step(snapshot, 1),
                (Some('k'), true) => self.step(snapshot, -1),
                (Some('b'), true) => self.set_filter(Some(State::Blocked)),
                (Some('w'), true) => self.set_filter(Some(State::Working)),
                (Some('i'), true) => self.set_filter(Some(State::Idle)),
                (Some('d'), true) => self.set_filter(Some(State::Done)),
                (Some('a'), true) => self.set_filter(None),
                _ => {}
            },
        }
        Outcome::Stay
    }

    fn mouse(&mut self, event: &MouseEvent, ctx: &Ctx<'_>) -> Outcome {
        let at = Position::new(event.column, event.row);
        let hit = self
            .hits
            .iter()
            .find(|(rect, _)| rect.contains(at))
            .map(|(_, target)| target.clone());
        match event.kind {
            MouseEventKind::Moved => {
                if let Some(target) = hit {
                    self.selected = Some(target);
                }
            }
            MouseEventKind::ScrollUp => self.step(ctx.snapshot, -WHEEL),
            MouseEventKind::ScrollDown => self.step(ctx.snapshot, WHEEL),
            MouseEventKind::Drag(MouseButton::Left) => {
                if let (Some(grab), Some(bar)) = (self.grab, self.bar) {
                    let scroll = bar.drag(self.shown, event.row, grab);
                    self.scroll_to(ctx.snapshot, scroll, bar.viewport);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => self.grab = None,
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(bar) = self.bar.filter(|b| b.track.contains(at)) {
                    self.grab = bar.grab(self.shown, event.row);
                    if self.grab.is_none() {
                        let scroll = bar.jump(self.shown, event.row);
                        self.scroll_to(ctx.snapshot, scroll, bar.viewport);
                    }
                } else if self.search.contains(at) {
                    self.searching = true;
                    self.filter = None;
                } else if let Some(target) = hit {
                    self.selected = Some(target);
                    return self.open(ctx.snapshot);
                } else if !self.popup.contains(at) {
                    return Outcome::Close;
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn paste(&mut self, text: &str) -> Outcome {
        if self.searching {
            self.query.insert(text);
            self.filter = None;
            self.selected = None;
        }
        Outcome::Stay
    }
}

impl Goto {
    fn header(
        &mut self,
        buf: &mut Buffer,
        inner: Rect,
        rows: &[Row],
        p: &Palette,
    ) -> Option<(u16, u16)> {
        let search = if self.searching {
            " / ".to_owned()
        } else if let Some(filter) = self.filter {
            format!(" / {}", filter.text())
        } else if self.query.text().is_empty() {
            " / search agents and terminals".to_owned()
        } else {
            format!(" / {}", self.query.text())
        };
        let terminals = rows.iter().filter(|r| r.is_pane()).count();
        let count = format!(
            "{terminals} {}",
            if terminals == 1 {
                "terminal"
            } else {
                "terminals"
            }
        );
        let count_width = width(&count);
        let fg = if self.searching { p.text } else { p.overlay0 };
        text(
            buf,
            inner.x,
            inner.y,
            inner.width.saturating_sub(count_width + 1),
            &search,
            Style::new().fg(fg).bg(p.panel_bg),
        );
        let cursor = self
            .searching
            .then(|| {
                self.query.render(
                    Rect::new(
                        inner.x + 3,
                        inner.y,
                        inner.width.saturating_sub(4 + count_width),
                        1,
                    ),
                    buf,
                    Style::new().fg(p.text).bg(p.panel_bg),
                )
            })
            .flatten();
        right(
            buf,
            Rect::new(inner.x, inner.y, inner.width, 1),
            &count,
            Style::new().fg(p.overlay0).bg(p.panel_bg),
        );
        self.search = Rect::new(inner.x, inner.y, inner.width, 1);
        text(
            buf,
            inner.x,
            inner.y + 1,
            inner.width,
            &"─".repeat(usize::from(inner.width)),
            Style::new().fg(p.surface1).bg(p.panel_bg),
        );
        cursor
    }

    fn body(&mut self, buf: &mut Buffer, inner: Rect, rows: &[Row], ctx: &Ctx<'_>) -> usize {
        let p = ctx.palette;
        let body = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(5),
        );
        let height = usize::from(body.height);
        let selected = self.index(rows).unwrap_or(0);
        let max = rows.len().saturating_sub(height);
        let scroll = self
            .scroll
            .max(selected.saturating_sub(height.saturating_sub(1)))
            .min(selected)
            .min(max);
        self.bar = (max > 0 && body.width > 1).then(|| {
            Bar::new(
                Rect::new(body.right() - 1, body.y, 1, body.height),
                max,
                height,
            )
        });
        let row_width = body.width - u16::from(self.bar.is_some());
        if rows.is_empty() {
            text(
                buf,
                body.x,
                body.y,
                body.width,
                " No matching agents or terminals",
                Style::new().fg(p.overlay0).bg(p.panel_bg),
            );
        }
        self.hits.clear();
        for index in (scroll..rows.len()).take(height) {
            let y = body.y + u16::try_from(index - scroll).unwrap_or(0);
            let rect = Rect::new(body.x, y, row_width, 1);
            self.hits.push((rect, rows[index].target.clone()));
            Self::draw_row(buf, rect, rows, index, selected, ctx);
        }
        if let Some(bar) = &self.bar {
            bar.render(
                buf,
                scroll,
                ("▕", p.overlay0),
                ("▐", p.overlay1),
                p.panel_bg,
            );
        }
        self.shown = scroll;
        selected
    }

    fn footer(&self, buf: &mut Buffer, inner: Rect, row: Option<&Row>, p: &Palette) {
        if let Some(row) = row {
            text(
                buf,
                inner.x,
                inner.bottom() - 3,
                inner.width,
                &format!(" {}", row.detail),
                Style::new().fg(p.subtext0).bg(p.panel_bg),
            );
            text(
                buf,
                inner.x,
                inner.bottom() - 2,
                inner.width,
                &format!(" {}", row.meta),
                Style::new().fg(p.overlay0).bg(p.panel_bg),
            );
        }
        let footer = if self.searching {
            " search type · move ↑↓/ctrl+n/p · open enter · back esc"
        } else {
            " ↑↓/j/k rows · ←→ workspace · / search · a/b/w/i/d filter · enter open · esc close"
        };
        text(
            buf,
            inner.x,
            inner.bottom() - 1,
            inner.width,
            footer,
            Style::new().fg(p.overlay0).bg(p.panel_bg),
        );
    }

    fn last(&mut self, snapshot: &Snapshot) {
        self.selected = self.rows(snapshot).pop().map(|r| r.target);
    }

    fn scroll_to(&mut self, snapshot: &Snapshot, scroll: usize, viewport: usize) {
        let rows = self.rows(snapshot);
        let viewport = viewport.max(1);
        self.scroll = scroll.min(rows.len().saturating_sub(viewport));
        let at = self
            .index(&rows)
            .unwrap_or(0)
            .clamp(self.scroll, self.scroll + viewport - 1);
        self.selected = rows.get(at).map(|r| r.target.clone());
    }
}

pub fn open(kind: &Kind, ctx: &Ctx<'_>) -> Option<Box<dyn Overlay>> {
    (*kind == Kind::Goto).then(|| Box::new(Goto::new(ctx.snapshot)) as Box<dyn Overlay>)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(key: &str, name: &str, tabs: usize) -> SpaceRow {
        SpaceRow {
            key: key.into(),
            name: name.into(),
            folder: format!("/src/{name}"),
            branch: Some("main".into()),
            tabs,
            ..SpaceRow::default()
        }
    }

    fn pane(key: &str, space: &str, tab: &str, panes: usize) -> PaneRow {
        PaneRow {
            key: key.into(),
            space: space.into(),
            tab: tab.into(),
            tab_label: "1".into(),
            tab_panes: panes,
            cwd: "/src".into(),
            ..PaneRow::default()
        }
    }

    fn snapshot() -> Snapshot {
        let mut agent = pane("p3", "s2", "t2", 1);
        agent.agent = Some("claude".into());
        agent.state = Some(State::Blocked);
        agent.agent_title = Some("fixing tests".into());
        let mut focused = pane("p2", "s1", "t1", 2);
        focused.focused = true;
        Snapshot {
            machines: vec![MachineRow {
                label: "Local".into(),
                connected: true,
                ..MachineRow::default()
            }],
            spaces: vec![space("s1", "api", 1), space("s2", "web", 1)],
            panes: vec![pane("p1", "s1", "t1", 2), focused, agent],
            ..Snapshot::default()
        }
    }

    fn labels(rows: &[Row]) -> Vec<String> {
        rows.iter().map(|r| r.label.clone()).collect()
    }

    #[test]
    fn rows_group_panes_under_workspaces() {
        let rows = rows(&snapshot(), "", None);
        assert_eq!(
            labels(&rows),
            ["api", "terminal · 1", "terminal · 2", "web", "fixing tests"]
        );
        assert_eq!(rows[2].detail, "api / 1 / p2");
        assert_eq!(rows[0].meta, "main");
        assert!(rows[2].current);
        assert_eq!(
            Goto::new(&snapshot()).selected(),
            Some(&Target::Pane("p2".into()))
        );
    }

    #[test]
    fn search_and_filters_narrow_rows() {
        let snap = snapshot();
        assert_eq!(
            labels(&rows(&snap, "claude", None)),
            ["web", "fixing tests"]
        );
        assert_eq!(labels(&rows(&snap, "web", None)), ["web", "fixing tests"]);
        assert_eq!(
            labels(&rows(&snap, "", Some(State::Blocked))),
            ["web", "fixing tests"]
        );
        assert_eq!(rows(&snap, "", Some(State::Done)), Vec::<Row>::new());
        assert_eq!(rows(&snap, "nothing here", None), Vec::<Row>::new());
    }

    #[test]
    fn machines_head_their_spaces_when_there_are_several() {
        let mut snap = snapshot();
        snap.machines.push(MachineRow {
            name: Some("box".into()),
            label: "box".into(),
            state: "reconnecting".into(),
            ..MachineRow::default()
        });
        snap.spaces[1].machine = Some("box".into());
        let rows = rows(&snap, "", None);
        assert_eq!(rows[0].target, Target::Machine(None));
        assert_eq!(rows[1].depth, 1);
        let boxed = rows
            .iter()
            .position(|r| r.target == Target::Machine(Some("box".into())))
            .expect("the box row");
        assert!(rows[boxed + 1].stale);
    }

    #[test]
    fn sections_jump_between_workspaces() {
        let snap = snapshot();
        let mut goto = Goto::new(&snap);
        goto.section(&snap, true);
        assert_eq!(goto.selected(), Some(&Target::Pane("p3".into())));
        goto.section(&snap, true);
        assert_eq!(goto.selected(), Some(&Target::Pane("p3".into())));
        goto.section(&snap, false);
        assert_eq!(goto.selected(), Some(&Target::Pane("p1".into())));
        goto.step(&snap, 10);
        assert_eq!(goto.selected(), Some(&Target::Pane("p3".into())));
    }
}
