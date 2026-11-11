use config::Palette;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::confirm::Confirm;
use super::frame::{contrast, panel, popup, text};
use super::help::typed;
use super::machine_add::AddForm;
use super::rename::{Answer, FieldDialog};
use super::{Ctx, Drawn, Kind, Outcome, Overlay, Request, TextField};
use crate::model::MachineRow;
use crate::sidebar::status_text;
use crate::text::{truncate, width};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineOp {
    Add {
        target: String,
        name: Option<String>,
    },
    Rename {
        machine: String,
        name: String,
    },
    Remove {
        machine: String,
    },
    Connect {
        machine: String,
    },
    Login {
        machine: String,
    },
    Update {
        machine: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Add,
    Rename,
    Remove,
    Connect,
    Login,
    Update,
    Menu,
    Close,
}

const ACTIONS: &[(&str, char, Key)] = &[
    ("reconnect", 'c', Key::Connect),
    ("log in again", 'l', Key::Login),
    ("update hc there", 'u', Key::Update),
    ("rename", 'r', Key::Rename),
    ("remove", 'x', Key::Remove),
];

const LOCAL_HINTS: &[(&str, &str, Key)] = &[("a", "add", Key::Add), ("esc", "close", Key::Close)];
const REMOTE_HINTS: &[(&str, &str, Key)] = &[
    ("↵", "actions", Key::Menu),
    ("a", "add", Key::Add),
    ("esc", "close", Key::Close),
];

fn remote(machine: Option<&MachineRow>) -> bool {
    machine.is_some_and(|m| m.name.is_some())
}

fn hints_for(machine: Option<&MachineRow>) -> &'static [(&'static str, &'static str, Key)] {
    if remote(machine) {
        REMOTE_HINTS
    } else {
        LOCAL_HINTS
    }
}

enum Stage {
    List,
    Add(AddForm),
    Rename {
        machine: String,
        dialog: FieldDialog,
    },
    Remove {
        machine: String,
        confirm: Confirm,
    },
    Actions {
        highlighted: usize,
    },
}

pub struct MachineManager {
    picked: Option<String>,
    seek: Option<String>,
    index: usize,
    stage: Stage,
    busy: Option<String>,
    error: Option<String>,
    note: Option<String>,
    popup: Rect,
    menu: Option<Rect>,
    rows: Vec<(Rect, usize)>,
    hints: Vec<(Rect, Key)>,
    items: Vec<(Rect, usize)>,
}

impl MachineManager {
    pub fn new(picked: Option<String>) -> Self {
        Self {
            picked,
            seek: None,
            index: 0,
            stage: Stage::List,
            busy: None,
            error: None,
            note: None,
            popup: Rect::default(),
            menu: None,
            rows: Vec::new(),
            hints: Vec::new(),
            items: Vec::new(),
        }
    }

    #[must_use]
    pub fn seeking(mut self, target: Option<String>) -> Self {
        self.seek = target;
        self
    }

    #[must_use]
    pub fn with_error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }

    pub fn picked(&self) -> Option<&str> {
        self.picked.as_deref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_busy(&self) -> bool {
        self.busy.is_some()
    }

    pub fn is_listing(&self) -> bool {
        matches!(self.stage, Stage::List)
    }

    pub fn finished(&mut self, result: Result<Option<String>, String>) {
        self.busy = None;
        match result {
            Ok(note) => {
                self.error = None;
                self.note = note;
            }
            Err(error) => {
                self.error = Some(error);
                self.note = None;
            }
        }
    }

    fn sync(&mut self, list: &[MachineRow]) -> usize {
        if let Some(seek) = &self.seek
            && let Some(found) = list.iter().find(|m| &m.target == seek && m.name.is_some())
        {
            self.picked.clone_from(&found.name);
            self.seek = None;
        }
        if let Some(index) = list.iter().position(|m| m.name == self.picked) {
            self.index = index;
        } else {
            self.index = self.index.min(list.len().saturating_sub(1));
            if self.busy.is_none()
                && let Some(row) = list.get(self.index)
            {
                self.picked.clone_from(&row.name);
            }
        }
        self.index
    }

    fn step(&mut self, list: &[MachineRow], delta: isize) {
        let at = self.sync(list);
        let end = list.len().saturating_sub(1);
        self.pick(list, at.saturating_add_signed(delta).min(end));
    }

    fn pick(&mut self, list: &[MachineRow], index: usize) {
        if let Some(machine) = list.get(index) {
            self.index = index;
            self.picked.clone_from(&machine.name);
        }
    }

    fn act(&mut self, key: Key, list: &[MachineRow]) -> Outcome {
        if key == Key::Close {
            return Outcome::Close;
        }
        if self.busy.is_some() {
            return Outcome::Stay;
        }
        let at = self.sync(list);
        let selected = list.get(at);
        if key != Key::Add && !remote(selected) {
            return Outcome::Stay;
        }
        if key == Key::Menu {
            self.stage = Stage::Actions { highlighted: 0 };
            return Outcome::Stay;
        }
        self.note = None;
        self.error = None;
        if key == Key::Add {
            self.stage = Stage::Add(AddForm::new());
            return Outcome::Stay;
        }
        let Some(machine) = selected.and_then(|m| m.name.clone()) else {
            return Outcome::Stay;
        };
        let (op, busy) = match key {
            Key::Rename => {
                self.stage = Stage::Rename {
                    dialog: FieldDialog::new(TextField::new(machine.clone())),
                    machine,
                };
                return Outcome::Stay;
            }
            Key::Remove => {
                self.stage = Stage::Remove {
                    confirm: Confirm::new(
                        "Remove machine?",
                        format!("{machine} — its terminals keep running there"),
                    ),
                    machine,
                };
                return Outcome::Stay;
            }
            Key::Connect => (
                MachineOp::Connect {
                    machine: machine.clone(),
                },
                format!("reconnecting {machine}…"),
            ),
            Key::Login => (
                MachineOp::Login {
                    machine: machine.clone(),
                },
                format!("logging in to {machine}…"),
            ),
            Key::Update => (
                MachineOp::Update {
                    machine: machine.clone(),
                },
                format!("updating {machine}…"),
            ),
            Key::Add | Key::Close | Key::Menu => return Outcome::Stay,
        };
        self.busy = Some(busy);
        Outcome::Keep(Request::Machine(op))
    }

    fn list_key(&mut self, key: &KeyEvent, list: &[MachineRow]) -> Outcome {
        match key.code {
            KeyCode::Esc => return Outcome::Close,
            KeyCode::Enter => return self.act(Key::Menu, list),
            KeyCode::Up => self.step(list, -1),
            KeyCode::Down => self.step(list, 1),
            KeyCode::Home => self.pick(list, 0),
            KeyCode::End => self.pick(list, list.len().saturating_sub(1)),
            _ => {
                let key = match typed(key) {
                    Some('k') => return self.moved(list, -1),
                    Some('j') => return self.moved(list, 1),
                    Some('a') => Key::Add,
                    Some('r') => Key::Rename,
                    Some('x') => Key::Remove,
                    Some('c') => Key::Connect,
                    Some('l') => Key::Login,
                    Some('u') => Key::Update,
                    _ => return Outcome::Stay,
                };
                return self.act(key, list);
            }
        }
        Outcome::Stay
    }

    fn menu_key(&mut self, key: &KeyEvent, list: &[MachineRow]) -> Outcome {
        let Stage::Actions { highlighted } = &mut self.stage else {
            return Outcome::Stay;
        };
        let end = ACTIONS.len() - 1;
        let up = matches!(key.code, KeyCode::Up) || typed(key) == Some('k');
        let down = matches!(key.code, KeyCode::Down) || typed(key) == Some('j');
        if up {
            *highlighted = highlighted.saturating_sub(1);
        } else if down {
            *highlighted = (*highlighted + 1).min(end);
        } else if key.code == KeyCode::Enter {
            let action = ACTIONS[*highlighted].2;
            return self.run(action, list);
        } else if key.code == KeyCode::Esc {
            self.stage = Stage::List;
        } else if let Some(&(_, _, action)) =
            typed(key).and_then(|c| ACTIONS.iter().find(|(_, k, _)| *k == c))
        {
            return self.run(action, list);
        }
        Outcome::Stay
    }

    fn run(&mut self, key: Key, list: &[MachineRow]) -> Outcome {
        self.stage = Stage::List;
        self.act(key, list)
    }

    fn moved(&mut self, list: &[MachineRow], delta: isize) -> Outcome {
        self.step(list, delta);
        Outcome::Stay
    }

    fn answer_rename(&mut self, answer: Answer) -> Outcome {
        let Stage::Rename { machine, dialog } = &self.stage else {
            return Outcome::Stay;
        };
        match answer {
            Answer::Stay => Outcome::Stay,
            Answer::No => {
                self.stage = Stage::List;
                Outcome::Stay
            }
            Answer::Yes => {
                let name = dialog.field.text().trim().to_owned();
                let machine = machine.clone();
                self.stage = Stage::List;
                if name.is_empty() || name == machine {
                    return Outcome::Stay;
                }
                self.busy = Some(format!("renaming {machine}…"));
                self.picked = Some(name.clone());
                Outcome::Keep(Request::Machine(MachineOp::Rename { machine, name }))
            }
        }
    }

    fn answer_remove(&mut self, answer: Answer) -> Outcome {
        let Stage::Remove { machine, .. } = &self.stage else {
            return Outcome::Stay;
        };
        let machine = machine.clone();
        match answer {
            Answer::Stay => Outcome::Stay,
            Answer::No => {
                self.stage = Stage::List;
                Outcome::Stay
            }
            Answer::Yes => {
                self.stage = Stage::List;
                self.busy = Some(format!("removing {machine}…"));
                Outcome::Keep(Request::Machine(MachineOp::Remove { machine }))
            }
        }
    }

    fn answer_add(&mut self, answer: Answer) -> Outcome {
        let Stage::Add(form) = &mut self.stage else {
            return Outcome::Stay;
        };
        match answer {
            Answer::Stay => Outcome::Stay,
            Answer::No => {
                self.stage = Stage::List;
                Outcome::Stay
            }
            Answer::Yes => match form.submit() {
                Some(op) => {
                    self.busy = Some("adding…".to_owned());
                    self.stage = Stage::List;
                    Outcome::Keep(Request::Machine(op))
                }
                None => Outcome::Stay,
            },
        }
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let p = ctx.palette;
        let list = &ctx.snapshot.machine_list;
        let selected = self.sync(list);
        let rows = u16::try_from(list.len().max(1)).unwrap_or(u16::MAX);
        let said = self
            .status_line(list.get(selected), p)
            .map_or(0, |(line, _)| width(&line) + 4);
        let wide = list_width(list, p).max(said.min(76));
        let mut tall = rows.saturating_add(6);
        let open = matches!(self.stage, Stage::Actions { .. });
        if open && area.width < wide + menu_width() + 5 {
            let row = u16::try_from(selected).unwrap_or(0);
            tall = tall.max(row + 5 + menu_height());
        }
        let outer = if open {
            let group = popup(area, wide + menu_width() + 1, tall)?;
            Rect::new(group.x, group.y, wide.min(group.width), group.height)
        } else {
            popup(area, wide, tall)?
        };
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        self.popup = outer;
        self.menu = None;
        self.rows.clear();
        self.hints.clear();
        self.items.clear();
        let title = " machines";
        let bold = Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD);
        text(buf, inner.x, inner.y, inner.width, title, bold);
        self.status(buf, inner, width(title) + 2, list.get(selected), p);
        if inner.height < 4 {
            return Some(Drawn::new(outer, None));
        }
        text(
            buf,
            inner.x,
            inner.y + 1,
            inner.width,
            &"─".repeat(usize::from(inner.width)),
            Style::new().fg(p.surface0).bg(p.panel_bg),
        );
        let body = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 4);
        self.rows = machine_rows(buf, body, list, selected, p);
        self.footer(buf, inner, list.get(selected), p);
        if let Stage::Actions { highlighted } = self.stage {
            let anchor = self
                .rows
                .iter()
                .find(|(_, index)| *index == selected)
                .map_or(body.y, |(rect, _)| rect.y);
            self.render_menu(area, buf, anchor, highlighted, p);
        }
        let covered = self.menu.map_or(outer, |menu| outer.union(menu));
        Some(Drawn::new(covered, None))
    }

    fn footer(&mut self, buf: &mut Buffer, inner: Rect, machine: Option<&MachineRow>, p: &Palette) {
        let y = inner.bottom() - 1;
        let mut x = inner.x + 1;
        for (key, label, action) in hints_for(machine) {
            let start = x;
            let room = inner.right().saturating_sub(x);
            text(
                buf,
                x,
                y,
                room,
                key,
                Style::new()
                    .fg(p.accent)
                    .bg(p.panel_bg)
                    .add_modifier(Modifier::BOLD),
            );
            x = x.saturating_add(width(key) + 1);
            let room = inner.right().saturating_sub(x);
            text(
                buf,
                x,
                y,
                room,
                label,
                Style::new().fg(p.subtext0).bg(p.panel_bg),
            );
            x = x.saturating_add(width(label));
            self.hints
                .push((Rect::new(start, y, x.saturating_sub(start), 1), *action));
            x = x.saturating_add(2);
        }
    }

    fn render_menu(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        row: u16,
        highlighted: usize,
        p: &Palette,
    ) {
        let wide = menu_width();
        let label = wide - 7;
        let tall = menu_height();
        let beside = self.popup.right() + 1;
        let left = if beside + wide <= area.right() {
            beside
        } else {
            self.popup.right().saturating_sub(wide + 2).max(area.x)
        };
        let top = row.min(area.bottom().saturating_sub(tall)).max(area.y);
        let rect = Rect::new(left, top, wide.min(area.width), tall.min(area.height));
        let Some(inner) = panel(buf, rect, p.accent, p.panel_bg) else {
            return;
        };
        self.menu = Some(rect);
        for (index, (name, key, action)) in ACTIONS.iter().enumerate() {
            let y = inner.y + u16::try_from(index).unwrap_or(0);
            if y >= inner.bottom() {
                break;
            }
            let line = Rect::new(inner.x, y, inner.width, 1);
            let chosen = index == highlighted;
            let base = if chosen {
                Style::new()
                    .fg(contrast(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(p.text).bg(p.panel_bg)
            };
            buf.set_style(line, base);
            let named = if !chosen && *action == Key::Remove {
                base.fg(p.red)
            } else {
                base
            };
            text(buf, line.x + 1, y, label, name, named);
            let hint = if chosen { base } else { base.fg(p.overlay0) };
            text(
                buf,
                line.right().saturating_sub(2),
                y,
                1,
                &key.to_string(),
                hint,
            );
            self.items.push((line, index));
        }
    }

    fn status_line(
        &self,
        machine: Option<&MachineRow>,
        p: &Palette,
    ) -> Option<(String, ratatui::style::Color)> {
        if let Some(busy) = &self.busy {
            Some((busy.clone(), p.accent))
        } else if let Some(error) = &self.error {
            Some((first_line(error), p.red))
        } else if let Some(note) = &self.note {
            Some((note.clone(), p.green))
        } else {
            machine
                .and_then(|m| m.why.as_deref().filter(|_| m.name.is_some()))
                .map(|why| (first_line(why), p.overlay0))
        }
    }

    fn status(
        &self,
        buf: &mut Buffer,
        inner: Rect,
        used: u16,
        machine: Option<&MachineRow>,
        p: &Palette,
    ) {
        let Some((line, fg)) = self.status_line(machine, p) else {
            return;
        };
        let style = Style::new().fg(fg).bg(p.panel_bg);
        let w = width(&line);
        if w + used < inner.width {
            text(buf, inner.right() - w - 1, inner.y, w, &line, style);
        } else if inner.height >= 5 {
            let room = inner.width.saturating_sub(2);
            let line = truncate(&line, usize::from(room));
            text(buf, inner.x + 1, inner.bottom() - 2, room, &line, style);
        }
    }
}

fn menu_height() -> u16 {
    u16::try_from(ACTIONS.len()).unwrap_or(0) + 2
}

fn menu_width() -> u16 {
    ACTIONS.iter().map(|(l, _, _)| width(l)).max().unwrap_or(0) + 7
}

fn list_width(list: &[MachineRow], p: &Palette) -> u16 {
    let names = list.iter().map(|m| width(&label_of(m))).max().unwrap_or(0);
    let targets = list
        .iter()
        .map(|m| width(&shown_target(m)))
        .max()
        .unwrap_or(0);
    let states = list
        .iter()
        .map(|m| width(&status_text(m, p).0))
        .max()
        .unwrap_or(0);
    let gap = if targets > 0 { targets + 2 } else { 0 };
    (names + gap + states + 6).clamp(52, 76)
}

fn first_line(text: &str) -> String {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(text)
        .trim()
        .to_owned()
}

fn label_of(machine: &MachineRow) -> String {
    if machine.temporary {
        format!("{}  temp", machine.label)
    } else {
        machine.label.clone()
    }
}

fn shown_target(machine: &MachineRow) -> String {
    if machine.via {
        format!("via {}", machine.target)
    } else if machine.target == machine.label {
        String::new()
    } else {
        machine.target.clone()
    }
}

fn machine_rows(
    buf: &mut Buffer,
    body: Rect,
    list: &[MachineRow],
    selected: usize,
    p: &Palette,
) -> Vec<(Rect, usize)> {
    let visible = usize::from(body.height);
    if visible == 0 {
        return Vec::new();
    }
    let start = selected
        .saturating_sub(visible - 1)
        .min(list.len().saturating_sub(visible));
    let names = list
        .iter()
        .map(|m| width(&label_of(m)))
        .max()
        .unwrap_or(0)
        .min(body.width / 3);
    let states: Vec<(String, ratatui::style::Color)> =
        list.iter().map(|m| status_text(m, p)).collect();
    let state_width = states.iter().map(|(s, _)| width(s)).max().unwrap_or(0);
    let mut rows = Vec::new();
    for (n, (index, machine)) in list
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .enumerate()
    {
        let rect = Rect::new(
            body.x,
            body.y + u16::try_from(n).unwrap_or(0),
            body.width,
            1,
        );
        rows.push((rect, index));
        let chosen = index == selected;
        let base = if chosen {
            Style::new()
                .fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(p.text).bg(p.panel_bg)
        };
        buf.set_style(rect, base);
        let label = truncate(&machine.label, usize::from(names));
        let x = rect.x + 1;
        text(
            buf,
            x,
            rect.y,
            names,
            &label,
            base.add_modifier(Modifier::BOLD),
        );
        if machine.temporary {
            let at = x + width(&label) + 2;
            let tag = if chosen {
                base
            } else {
                Style::new().fg(p.yellow).bg(p.panel_bg)
            };
            text(
                buf,
                at,
                rect.y,
                rect.right().saturating_sub(at),
                "temp",
                tag,
            );
        }
        let (state, color) = &states[index];
        let state_x = rect.right().saturating_sub(state_width + 1);
        let state_style = if chosen { base } else { base.fg(*color) };
        text(buf, state_x, rect.y, state_width, state, state_style);
        let target_x = x + names + 2;
        let room = state_x.saturating_sub(target_x + 1);
        if room > 0 {
            let target_style = if chosen {
                base.remove_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(p.overlay0).bg(p.panel_bg)
            };
            text(
                buf,
                target_x,
                rect.y,
                room,
                &truncate(&shown_target(machine), usize::from(room)),
                target_style,
            );
        }
    }
    rows
}

impl Overlay for MachineManager {
    fn kind(&self) -> Kind {
        Kind::Machines
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let p = ctx.palette;
        let title = match &self.stage {
            Stage::Rename { .. } => Some("rename machine"),
            _ => None,
        };
        match &mut self.stage {
            Stage::List | Stage::Actions { .. } => self.render_list(area, buf, ctx),
            Stage::Add(form) => form.render(area, buf, p),
            Stage::Rename { dialog, .. } => dialog.render(area, buf, p, title.unwrap_or_default()),
            Stage::Remove { confirm, .. } => confirm.render(area, buf, p),
        }
    }

    fn key(&mut self, key: &KeyEvent, ctx: &Ctx<'_>) -> Outcome {
        let list = &ctx.snapshot.machine_list;
        match &mut self.stage {
            Stage::List => self.list_key(key, list),
            Stage::Add(form) => {
                let answer = form.key(key);
                self.answer_add(answer)
            }
            Stage::Rename { dialog, .. } => {
                let answer = dialog.key(key);
                self.answer_rename(answer)
            }
            Stage::Remove { .. } => self.answer_remove(Confirm::key(key)),
            Stage::Actions { .. } => self.menu_key(key, list),
        }
    }

    fn mouse(&mut self, event: &MouseEvent, ctx: &Ctx<'_>) -> Outcome {
        let list = &ctx.snapshot.machine_list;
        let at = Position::new(event.column, event.row);
        match &mut self.stage {
            Stage::List => {}
            Stage::Actions { .. } => {
                if event.kind == MouseEventKind::Down(MouseButton::Left) {
                    if let Some(&(_, index)) = self.items.iter().find(|(r, _)| r.contains(at)) {
                        return self.run(ACTIONS[index].2, list);
                    }
                    self.stage = Stage::List;
                }
                return Outcome::Stay;
            }
            Stage::Add(form) => {
                if event.kind != MouseEventKind::Down(MouseButton::Left) {
                    return Outcome::Stay;
                }
                let answer = form.mouse(at);
                return self.answer_add(answer);
            }
            Stage::Rename { dialog, .. } => {
                let answer = dialog.mouse(*event);
                return self.answer_rename(answer);
            }
            Stage::Remove { confirm, .. } => {
                let answer = confirm.mouse(*event);
                return self.answer_remove(answer);
            }
        }
        match event.kind {
            MouseEventKind::ScrollUp => self.step(list, -1),
            MouseEventKind::ScrollDown => self.step(list, 1),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((_, index)) = self.rows.iter().find(|(r, _)| r.contains(at)) {
                    let index = *index;
                    self.pick(list, index);
                } else if let Some((_, key)) = self.hints.iter().find(|(r, _)| r.contains(at)) {
                    let key = *key;
                    return self.act(key, list);
                } else if !self.popup.contains(at) {
                    return Outcome::Close;
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(&(_, index)) = self.rows.iter().find(|(r, _)| r.contains(at)) {
                    self.pick(list, index);
                    return self.act(Key::Menu, list);
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn paste(&mut self, pasted: &str) -> Outcome {
        match &mut self.stage {
            Stage::Add(form) => form.paste(pasted),
            Stage::Rename { dialog, .. } => dialog.field.insert(pasted),
            Stage::List | Stage::Remove { .. } | Stage::Actions { .. } => {}
        }
        Outcome::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: Option<&str>, target: &str) -> MachineRow {
        MachineRow {
            name: name.map(str::to_owned),
            label: name.unwrap_or("Local").to_owned(),
            target: target.to_owned(),
            ..MachineRow::default()
        }
    }

    #[test]
    fn the_pick_follows_its_machine_and_then_a_new_target() {
        let list = vec![row(None, ""), row(Some("arc"), "a"), row(Some("box"), "b")];
        let mut manager = MachineManager::new(Some("box".into()));
        assert_eq!(manager.sync(&list), 2);
        manager.step(&list, -5);
        assert_eq!(manager.picked(), None);
        let gone = vec![row(None, ""), row(Some("arc"), "a")];
        let mut manager = MachineManager::new(Some("box".into()));
        manager.index = 2;
        assert_eq!(manager.sync(&gone), 1);
        let mut manager = MachineManager::new(None).seeking(Some("b".into()));
        assert_eq!(manager.sync(&list), 2);
        assert_eq!(manager.picked(), Some("box"));
    }

    #[test]
    fn local_rows_only_offer_add() {
        let list = vec![row(None, ""), row(Some("box"), "b")];
        let mut manager = MachineManager::new(None);
        assert_eq!(manager.act(Key::Rename, &list), Outcome::Stay);
        assert!(manager.is_listing());
        assert_eq!(manager.act(Key::Connect, &list), Outcome::Stay);
        assert!(!manager.is_busy());
        manager.pick(&list, 1);
        assert_eq!(
            manager.act(Key::Connect, &list),
            Outcome::Keep(Request::Machine(MachineOp::Connect {
                machine: "box".into()
            }))
        );
        assert!(manager.is_busy());
        assert_eq!(manager.act(Key::Update, &list), Outcome::Stay);
    }

    #[test]
    fn enter_opens_the_actions_and_their_keys_run() {
        let list = vec![row(None, ""), row(Some("box"), "b")];
        let mut manager = MachineManager::new(None);
        assert_eq!(hints_for(list.first()), LOCAL_HINTS);
        let enter = KeyEvent::from(KeyCode::Enter);
        manager.list_key(&enter, &list);
        assert!(manager.is_listing());
        manager.pick(&list, 1);
        assert_eq!(hints_for(list.get(1)), REMOTE_HINTS);
        manager.list_key(&enter, &list);
        assert!(matches!(manager.stage, Stage::Actions { highlighted: 0 }));
        manager.menu_key(&KeyEvent::from(KeyCode::Down), &list);
        assert!(matches!(manager.stage, Stage::Actions { highlighted: 1 }));
        manager.menu_key(&KeyEvent::from(KeyCode::Esc), &list);
        assert!(manager.is_listing());
        manager.list_key(&enter, &list);
        assert_eq!(
            manager.menu_key(&KeyEvent::from(KeyCode::Enter), &list),
            Outcome::Keep(Request::Machine(MachineOp::Connect {
                machine: "box".into()
            }))
        );
        manager.finished(Ok(None));
        manager.list_key(&KeyEvent::from(KeyCode::Char('x')), &list);
        assert!(matches!(manager.stage, Stage::Remove { .. }));
    }
}
