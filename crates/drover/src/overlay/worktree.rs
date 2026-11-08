use std::sync::atomic::{AtomicU64, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;

use super::frame::{button, contrast, panel, popup, row, text};
use super::{Ctx, Kind, Outcome, Overlay, Request, TextField};
use crate::view::Mode;

const ADJECTIVES: [&str; 8] = [
    "brave", "calm", "clear", "green", "lucky", "quiet", "rapid", "silver",
];
const NOUNS: [&str; 8] = [
    "river", "cloud", "field", "forest", "harbor", "meadow", "stone", "valley",
];

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeOp {
    Create {
        dialog: u64,
        space: String,
        branch: String,
        path: String,
        top: String,
        main: String,
    },
    Open {
        dialog: u64,
        space: String,
        path: String,
        shown_in: Option<String>,
    },
    Remove {
        dialog: u64,
        space: String,
        path: String,
        main: String,
        force: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    pub path: String,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub prunable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub branch: Option<String>,
    pub label: String,
    pub status: &'static str,
    pub shown_in: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub home: String,
    pub top: String,
    pub main: String,
    pub checkouts: Vec<Checkout>,
}

impl Repo {
    pub fn name(&self) -> &str {
        folder_name(&self.main)
    }

    pub fn linked(&self) -> bool {
        self.top != self.main
    }
}

pub fn folder_name(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

pub fn generated_branch(seed: u64) -> String {
    let adjective = ADJECTIVES[usize::try_from(seed % 8).unwrap_or(0)];
    let noun = NOUNS[usize::try_from((seed / 8) % 8).unwrap_or(0)];
    format!("worktree/{adjective}-{noun}-{:04x}", seed & 0xffff)
}

pub fn slug(branch: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in branch.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "worktree".to_owned()
    } else {
        trimmed.to_owned()
    }
}

pub fn worktree_root(directory: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    let directory = directory.trim();
    let root = if directory.is_empty() || directory == "~" {
        home.to_owned()
    } else if let Some(rest) = directory.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else if directory.starts_with('/') {
        directory.to_owned()
    } else {
        format!("{home}/{directory}")
    };
    let root = root.trim_end_matches('/');
    if root.is_empty() {
        "/".to_owned()
    } else {
        root.to_owned()
    }
}

pub fn checkout_path(root: &str, repo: &str, branch: &str) -> String {
    format!("{}/{repo}/{}", root.trim_end_matches('/'), slug(branch))
}

pub fn parse_porcelain(out: &str) -> Vec<Checkout> {
    let mut list = Vec::new();
    for block in out.split("\n\n") {
        let mut current: Option<Checkout> = None;
        for line in block.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                current = Some(Checkout {
                    path: path.to_owned(),
                    branch: None,
                    bare: false,
                    detached: false,
                    prunable: false,
                });
                continue;
            }
            let Some(checkout) = current.as_mut() else {
                continue;
            };
            if let Some(branch) = line.strip_prefix("branch ") {
                checkout.branch = Some(
                    branch
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch)
                        .to_owned(),
                );
            } else if line == "bare" {
                checkout.bare = true;
            } else if line == "detached" {
                checkout.detached = true;
            } else if line.starts_with("prunable") {
                checkout.prunable = true;
            }
        }
        list.extend(current);
    }
    list
}

pub fn inside(path: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches('/');
    path == dir
        || path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub fn shown_in<'a>(
    checkout: &str,
    all: &[Checkout],
    places: &'a [(String, String)],
) -> Option<&'a str> {
    places
        .iter()
        .find(|(_, at)| {
            inside(at, checkout)
                && !all.iter().any(|other| {
                    other.path != checkout
                        && other.path.len() > checkout.len()
                        && inside(at, &other.path)
                })
        })
        .map(|(key, _)| key.as_str())
}

pub fn entries(repo: &Repo, places: &[(String, String)]) -> Vec<Entry> {
    repo.checkouts
        .iter()
        .filter(|c| !c.bare && !c.prunable)
        .map(|c| {
            let shown = shown_in(&c.path, &repo.checkouts, places).map(str::to_owned);
            let status = if shown.is_some() {
                "open"
            } else if c.branch.is_some() {
                ""
            } else if c.detached && c.path != repo.main {
                "detached"
            } else {
                "root"
            };
            Entry {
                label: c
                    .branch
                    .clone()
                    .unwrap_or_else(|| folder_name(&c.path).to_owned()),
                path: c.path.clone(),
                branch: c.branch.clone(),
                status,
                shown_in: shown,
            }
        })
        .collect()
}

pub fn is_force_needed(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    (lower.contains("contains modified or untracked files") && lower.contains("use --force"))
        || lower.contains("working trees containing submodules cannot be moved or removed")
        || lower.contains("is not a working tree")
        || lower.contains("is not a worktree")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    Remove,
}

impl Action {
    fn kind(self) -> Kind {
        match self {
            Self::New => Kind::NewWorktree,
            Self::Open => Kind::OpenWorktree,
            Self::Remove => Kind::RemoveWorktree,
        }
    }
}

struct Create {
    root: String,
    repo: String,
    top: String,
    main: String,
    branch: TextField,
    error: Option<String>,
    busy: bool,
}

impl Create {
    fn path(&self) -> String {
        checkout_path(&self.root, &self.repo, self.branch.text().trim())
    }
}

struct Open {
    entries: Vec<Entry>,
    selected: usize,
    query: TextField,
    searching: bool,
    error: Option<String>,
    busy: bool,
}

impl Open {
    fn filtered(&self) -> Vec<usize> {
        let query = self.query.text().trim().to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                query.is_empty()
                    || format!(
                        "{} {} {} {}",
                        e.label,
                        e.branch.as_deref().unwrap_or(""),
                        e.path,
                        e.status
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn reselect(&mut self) {
        if let Some(first) = self.filtered().first() {
            self.selected = *first;
        }
    }

    fn step(&mut self, delta: isize) {
        let filtered = self.filtered();
        if filtered.is_empty() {
            self.selected = 0;
            return;
        }
        let at = filtered
            .iter()
            .position(|i| *i == self.selected)
            .unwrap_or(0);
        let last = filtered.len() - 1;
        let next = at.saturating_add_signed(delta).min(last);
        self.selected = filtered[next];
    }

    fn current(&self) -> Option<&Entry> {
        let filtered = self.filtered();
        filtered
            .contains(&self.selected)
            .then(|| &self.entries[self.selected])
            .or_else(|| filtered.first().map(|i| &self.entries[*i]))
    }
}

struct Remove {
    path: String,
    main: String,
    force: bool,
    error: Option<String>,
    busy: bool,
}

enum State {
    Loading { started: bool },
    Create(Create),
    Open(Open),
    Remove(Remove),
}

pub struct Worktree {
    id: u64,
    action: Action,
    space: String,
    state: State,
    primary: Rect,
    cancel: Rect,
    search: Rect,
    rows: Vec<(Rect, usize)>,
}

pub fn open(kind: &Kind, ctx: &Ctx<'_>) -> Option<Box<dyn Overlay>> {
    let action = match kind {
        Kind::NewWorktree => Action::New,
        Kind::OpenWorktree => Action::Open,
        Kind::RemoveWorktree => Action::Remove,
        _ => return None,
    };
    let snapshot = ctx.snapshot;
    let space = if snapshot.mode == Mode::Navigate {
        snapshot
            .spaces
            .iter()
            .find(|s| s.selected)
            .map(|s| s.key.clone())
            .or_else(|| snapshot.active_space.clone())
    } else {
        snapshot.active_space.clone()
    }?;
    Some(Box::new(Worktree::new(action, space)))
}

impl Worktree {
    pub fn new(action: Action, space: String) -> Self {
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            action,
            space,
            state: State::Loading { started: false },
            primary: Rect::default(),
            cancel: Rect::default(),
            search: Rect::default(),
            rows: Vec::new(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn action(&self) -> Action {
        self.action
    }

    pub fn space(&self) -> &str {
        &self.space
    }

    pub fn start(&mut self) -> bool {
        match &mut self.state {
            State::Loading { started } if !*started => {
                *started = true;
                true
            }
            _ => false,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, State::Loading { .. })
    }

    pub fn load(&mut self, repo: &Repo, directory: &str, places: &[(String, String)], seed: u64) {
        self.state = match self.action {
            Action::New => State::Create(Create {
                root: worktree_root(directory, &repo.home),
                repo: repo.name().to_owned(),
                top: repo.top.clone(),
                main: repo.main.clone(),
                branch: TextField::suggested(generated_branch(seed)),
                error: None,
                busy: false,
            }),
            Action::Open => State::Open(Open {
                entries: entries(repo, places),
                selected: 0,
                query: TextField::default(),
                searching: false,
                error: None,
                busy: false,
            }),
            Action::Remove => State::Remove(Remove {
                path: repo.top.clone(),
                main: repo.main.clone(),
                force: false,
                error: None,
                busy: false,
            }),
        };
    }

    pub fn failed(&mut self, error: String) {
        match &mut self.state {
            State::Create(create) => {
                create.busy = false;
                create.error = Some(error);
            }
            State::Open(open) => {
                open.busy = false;
                open.error = Some(error);
            }
            State::Remove(remove) => {
                remove.busy = false;
                remove.error = Some(error);
            }
            State::Loading { .. } => {}
        }
    }

    pub fn needs_force(&mut self) {
        if let State::Remove(remove) = &mut self.state {
            remove.busy = false;
            remove.force = true;
            remove.error = None;
        }
    }

    pub fn branch(&self) -> Option<&str> {
        match &self.state {
            State::Create(create) => Some(create.branch.text()),
            _ => None,
        }
    }

    pub fn checkout_preview(&self) -> Option<String> {
        match &self.state {
            State::Create(create) => Some(create.path()),
            _ => None,
        }
    }

    pub fn entries(&self) -> &[Entry] {
        match &self.state {
            State::Open(open) => &open.entries,
            _ => &[],
        }
    }

    pub fn forcing(&self) -> bool {
        matches!(&self.state, State::Remove(remove) if remove.force)
    }

    fn busy(&self) -> bool {
        match &self.state {
            State::Create(c) => c.busy,
            State::Open(o) => o.busy,
            State::Remove(r) => r.busy,
            State::Loading { .. } => false,
        }
    }

    fn submit(&mut self) -> Outcome {
        let (dialog, space) = (self.id, self.space.clone());
        match &mut self.state {
            State::Create(create) if !create.busy => {
                let branch = create.branch.text().trim().to_owned();
                if branch.is_empty() {
                    create.error = Some("branch is required".to_owned());
                    return Outcome::Stay;
                }
                create.branch = TextField::new(branch.clone());
                create.busy = true;
                create.error = None;
                Outcome::Keep(Request::Worktree(WorktreeOp::Create {
                    dialog,
                    space,
                    path: create.path(),
                    top: create.top.clone(),
                    main: create.main.clone(),
                    branch,
                }))
            }
            State::Open(open) if !open.busy => {
                let Some(entry) = open.current().cloned() else {
                    return Outcome::Stay;
                };
                open.selected = open
                    .entries
                    .iter()
                    .position(|e| e.path == entry.path)
                    .unwrap_or(0);
                open.busy = true;
                open.error = None;
                Outcome::Keep(Request::Worktree(WorktreeOp::Open {
                    dialog,
                    space,
                    path: entry.path,
                    shown_in: entry.shown_in,
                }))
            }
            State::Remove(remove) if !remove.busy => {
                remove.busy = true;
                remove.error = None;
                Outcome::Keep(Request::Worktree(WorktreeOp::Remove {
                    dialog,
                    space,
                    path: remove.path.clone(),
                    main: remove.main.clone(),
                    force: remove.force,
                }))
            }
            _ => Outcome::Stay,
        }
    }

    fn render_create(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<(u16, u16)> {
        let State::Create(create) = &self.state else {
            return None;
        };
        let p = ctx.palette;
        let outer = popup(area, 68, 12)?;
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        let bold = Style::new()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD);
        let dim = Style::new().fg(p.overlay0).bg(p.panel_bg);
        text(buf, inner.x, inner.y, inner.width, "new worktree", bold);
        text(buf, inner.x, inner.y + 2, inner.width, " branch", dim);
        let input = Rect::new(inner.x, inner.y + 3, inner.width, 1);
        let field = Style::new().fg(p.text).bg(p.surface0);
        buf.set_style(input, field);
        let cursor = create.branch.render(
            Rect::new(input.x + 1, input.y, input.width.saturating_sub(1), 1),
            buf,
            field,
        );
        text(buf, inner.x, inner.y + 5, inner.width, " checkout", dim);
        text(
            buf,
            inner.x,
            inner.y + 6,
            inner.width,
            &format!(" {}", create.path()),
            Style::new().fg(p.subtext0).bg(p.panel_bg),
        );
        status(
            buf,
            inner,
            inner.y + 8,
            create.busy,
            "creating…",
            create.error.as_deref(),
            ctx,
        );
        let rects = row(inner, &[20, 12], 2, 9);
        button(
            buf,
            rects[0],
            " ↵ create and open ",
            primary_style(ctx, false),
        );
        button(buf, rects[1], " esc cancel ", plain_style(ctx));
        (self.primary, self.cancel) = (rects[0], rects[1]);
        cursor.filter(|_| !create.busy)
    }

    fn render_open(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<(u16, u16)> {
        let State::Open(open) = &self.state else {
            return None;
        };
        let p = ctx.palette;
        let height = u16::try_from((open.entries.len() * 2 + 7).clamp(12, 26)).unwrap_or(26);
        let outer = popup(area, 96, height)?;
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        text(
            buf,
            inner.x,
            inner.y,
            inner.width,
            "open worktree",
            Style::new()
                .fg(p.text)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD),
        );
        let search = Rect::new(inner.x, inner.y + 1, inner.width, 1);
        let filtered = open.filtered();
        let prompt = if open.searching {
            " / ".to_owned()
        } else if open.query.text().is_empty() {
            " / filter worktrees".to_owned()
        } else {
            format!(" / {}", open.query.text())
        };
        let prompt_style = Style::new()
            .fg(if open.searching { p.text } else { p.overlay0 })
            .bg(p.panel_bg);
        text(buf, search.x, search.y, search.width, &prompt, prompt_style);
        let count = if filtered.len() == open.entries.len() {
            format!("{} checkouts", open.entries.len())
        } else {
            format!("{}/{} checkouts", filtered.len(), open.entries.len())
        };
        let count_width = u16::try_from(count.width()).unwrap_or(0);
        let cursor = if open.searching {
            open.query.render(
                Rect::new(
                    search.x + 3,
                    search.y,
                    search.width.saturating_sub(4 + count_width),
                    1,
                ),
                buf,
                Style::new().fg(p.text).bg(p.panel_bg),
            )
        } else {
            None
        };
        right(
            buf,
            search,
            &count,
            Style::new().fg(p.overlay0).bg(p.panel_bg),
        );
        text(
            buf,
            inner.x,
            inner.y + 2,
            inner.width,
            &"─".repeat(usize::from(inner.width)),
            Style::new().fg(p.surface1).bg(p.panel_bg),
        );
        let body = Rect::new(
            inner.x,
            inner.y + 3,
            inner.width,
            inner.height.saturating_sub(6),
        );
        let rows = entry_rows(open, &filtered, body, buf, ctx);
        status(
            buf,
            inner,
            inner.bottom().saturating_sub(3),
            open.busy,
            "opening…",
            open.error.as_deref(),
            ctx,
        );
        let rects = row(inner, &[10, 12], 2, inner.height.saturating_sub(1));
        button(buf, rects[0], " ↵ open ", primary_style(ctx, false));
        button(buf, rects[1], " esc cancel ", plain_style(ctx));
        let busy = open.busy;
        (self.primary, self.cancel, self.search, self.rows) = (rects[0], rects[1], search, rows);
        cursor.filter(|_| !busy)
    }

    fn render_remove(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<(u16, u16)> {
        let State::Remove(remove) = &self.state else {
            return None;
        };
        let p = ctx.palette;
        let outer = popup(area, 72, 10)?;
        let inner = panel(buf, outer, p.red, p.panel_bg)?;
        let plain = Style::new().fg(p.text).bg(p.panel_bg);
        text(
            buf,
            inner.x,
            inner.y,
            inner.width,
            " delete worktree checkout?",
            Style::new()
                .fg(p.red)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD),
        );
        text(
            buf,
            inner.x,
            inner.y + 1,
            inner.width,
            " This removes the checkout folder:",
            plain,
        );
        text(
            buf,
            inner.x,
            inner.y + 2,
            inner.width,
            &format!(" {}", remove.path),
            Style::new().fg(p.subtext0).bg(p.panel_bg),
        );
        text(
            buf,
            inner.x,
            inner.y + 3,
            inner.width,
            " The branch is not deleted. The Herdr workspace will close.",
            plain,
        );
        if remove.force {
            text(
                buf,
                inner.x,
                inner.y + 4,
                inner.width,
                " Dirty or untracked files will be permanently deleted.",
                Style::new().fg(p.red).bg(p.panel_bg),
            );
        }
        status(
            buf,
            inner,
            inner.y + 5,
            remove.busy,
            "removing…",
            remove.error.as_deref(),
            ctx,
        );
        let rects = row(inner, &[18, 12], 2, 7);
        let label = if remove.force {
            " ↵ delete anyway "
        } else {
            " ↵ remove "
        };
        button(buf, rects[0], label, primary_style(ctx, true));
        button(buf, rects[1], " esc cancel ", plain_style(ctx));
        (self.primary, self.cancel) = (rects[0], rects[1]);
        None
    }
}

fn entry_rows(
    open: &Open,
    filtered: &[usize],
    body: Rect,
    buf: &mut Buffer,
    ctx: &Ctx<'_>,
) -> Vec<(Rect, usize)> {
    let p = ctx.palette;
    let visible = usize::from(body.height / 2).max(1);
    let at = filtered
        .iter()
        .position(|i| *i == open.selected)
        .unwrap_or(0);
    let start = at
        .saturating_sub(visible - 1)
        .min(filtered.len().saturating_sub(visible));
    let mut rows = Vec::new();
    for (n, index) in filtered
        .iter()
        .copied()
        .skip(start)
        .take(visible)
        .enumerate()
    {
        let entry = &open.entries[index];
        let rect = Rect::new(
            body.x,
            body.y + u16::try_from(n * 2).unwrap_or(0),
            body.width,
            2,
        );
        rows.push((rect, index));
        let selected = index == open.selected;
        let style = if selected {
            Style::new().fg(contrast(p)).bg(p.accent)
        } else {
            Style::new().fg(p.text).bg(p.panel_bg)
        };
        buf.set_style(rect, style);
        text(
            buf,
            rect.x,
            rect.y,
            rect.width,
            &format!(" {}", entry.label),
            style.add_modifier(Modifier::BOLD),
        );
        if !entry.status.is_empty() {
            right(
                buf,
                Rect::new(rect.x, rect.y, rect.width, 1),
                entry.status,
                style,
            );
        }
        let path_style = if selected {
            style
        } else {
            Style::new().fg(p.overlay0).bg(p.panel_bg)
        };
        text(
            buf,
            rect.x,
            rect.y + 1,
            rect.width,
            &format!(" {}", entry.path),
            path_style,
        );
    }
    if filtered.is_empty() {
        text(
            buf,
            body.x,
            body.y,
            body.width,
            " no matching worktrees",
            Style::new().fg(p.overlay0).bg(p.panel_bg),
        );
    }
    rows
}

fn edits_query(key: &KeyEvent) -> bool {
    let moves =
        matches!(key.code, KeyCode::Char('n' | 'p')) && key.modifiers == KeyModifiers::CONTROL;
    !moves
        && !matches!(
            key.code,
            KeyCode::Enter | KeyCode::Esc | KeyCode::Up | KeyCode::Down
        )
}

fn primary_style(ctx: &Ctx<'_>, danger: bool) -> Style {
    let p = ctx.palette;
    Style::new()
        .fg(contrast(p))
        .bg(if danger { p.red } else { p.accent })
        .add_modifier(Modifier::BOLD)
}

fn plain_style(ctx: &Ctx<'_>) -> Style {
    let p = ctx.palette;
    Style::new()
        .fg(p.text)
        .bg(p.surface0)
        .add_modifier(Modifier::BOLD)
}

fn status(
    buf: &mut Buffer,
    inner: Rect,
    y: u16,
    busy: bool,
    working: &str,
    error: Option<&str>,
    ctx: &Ctx<'_>,
) {
    let p = ctx.palette;
    if busy {
        text(
            buf,
            inner.x,
            y,
            inner.width,
            &format!(" {working}"),
            Style::new().fg(p.accent).bg(p.panel_bg),
        );
    } else if let Some(error) = error {
        let line = error
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or(error);
        text(
            buf,
            inner.x,
            y,
            inner.width,
            &format!(" {line}"),
            Style::new().fg(p.red).bg(p.panel_bg),
        );
    }
}

fn right(buf: &mut Buffer, rect: Rect, label: &str, style: Style) {
    let width = u16::try_from(label.width()).unwrap_or(u16::MAX);
    if width >= rect.width {
        return;
    }
    let x = rect.right() - width - 1;
    text(buf, x, rect.y, width, label, style);
}

impl Overlay for Worktree {
    fn kind(&self) -> Kind {
        self.action.kind()
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<(u16, u16)> {
        match self.state {
            State::Loading { .. } => None,
            State::Create(_) => self.render_create(area, buf, ctx),
            State::Open(_) => self.render_open(area, buf, ctx),
            State::Remove(_) => self.render_remove(area, buf, ctx),
        }
    }

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        let busy = self.busy();
        if let State::Loading { .. } = self.state {
            return if key.code == KeyCode::Esc {
                Outcome::Close
            } else {
                Outcome::Stay
            };
        }
        match &mut self.state {
            State::Create(create)
                if !busy && !matches!(key.code, KeyCode::Enter | KeyCode::Esc) =>
            {
                create.branch.key(key);
                create.error = None;
                return Outcome::Stay;
            }
            State::Open(open) if !busy && open.searching && edits_query(key) => {
                let before = open.query.text().to_owned();
                open.query.key(key);
                if open.query.text() != before {
                    open.reselect();
                }
                return Outcome::Stay;
            }
            _ => {}
        }
        match key.code {
            KeyCode::Esc if !busy => Outcome::Close,
            KeyCode::Enter => self.submit(),
            _ => {
                if let State::Open(open) = &mut self.state
                    && !busy
                {
                    let ctrl = key.modifiers == KeyModifiers::CONTROL;
                    match key.code {
                        KeyCode::Up => open.step(-1),
                        KeyCode::Down => open.step(1),
                        KeyCode::Char('p') if ctrl => open.step(-1),
                        KeyCode::Char('n') if ctrl => open.step(1),
                        KeyCode::Char('/') if !open.searching => open.searching = true,
                        _ => {}
                    }
                }
                Outcome::Stay
            }
        }
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        let at = Position::new(event.column, event.row);
        let busy = self.busy();
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.cancel.contains(at) {
                    return if busy { Outcome::Stay } else { Outcome::Close };
                }
                if self.primary.contains(at) {
                    return self.submit();
                }
                let (search, rows) = (self.search, self.rows.clone());
                if let State::Open(open) = &mut self.state
                    && !busy
                {
                    if search.contains(at) {
                        open.searching = true;
                    } else if let Some((_, index)) = rows.iter().find(|(r, _)| r.contains(at)) {
                        open.selected = *index;
                        return self.submit();
                    }
                }
                Outcome::Stay
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                if let State::Open(open) = &mut self.state
                    && !busy
                {
                    open.step(if event.kind == MouseEventKind::ScrollUp {
                        -1
                    } else {
                        1
                    });
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }

    fn paste(&mut self, pasted: &str) -> Outcome {
        if self.busy() {
            return Outcome::Stay;
        }
        match &mut self.state {
            State::Create(create) => {
                create.branch.insert(pasted);
                create.error = None;
            }
            State::Open(open) if open.searching => {
                open.query.insert(pasted);
                open.reselect();
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn dims(&self) -> bool {
        !self.is_loading()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_seeds_use_the_word_lists() {
        assert_eq!(generated_branch(0), "worktree/brave-river-0000");
        assert_eq!(generated_branch(9 + 0x1_0000), "worktree/calm-cloud-0009");
    }

    #[test]
    fn slugs_match_the_app() {
        assert_eq!(slug("feature/a"), "feature-a");
        assert_eq!(slug("Fix: the  bug!"), "Fix-the-bug");
        assert_eq!(slug("v1.2_x"), "v1.2_x");
        assert_eq!(slug("///"), "worktree");
    }

    #[test]
    fn roots_expand_home() {
        assert_eq!(
            worktree_root("~/.herdr/worktrees", "/h/"),
            "/h/.herdr/worktrees"
        );
        assert_eq!(worktree_root("/w/", "/h"), "/w");
        assert_eq!(worktree_root("wt", "/h"), "/h/wt");
        assert_eq!(
            checkout_path("/h/.herdr/worktrees", "repo", "feature/a"),
            "/h/.herdr/worktrees/repo/feature-a"
        );
    }

    #[test]
    fn porcelain_parses_every_kind() {
        let out = "worktree /r\nHEAD abc\nbranch refs/heads/main\n\nworktree /w/a\nHEAD def\ndetached\n\nworktree /w/b\nbranch refs/heads/b\nprunable gitdir file points to non-existent location\n\nworktree /bare\nbare\n";
        let list = parse_porcelain(out);
        assert_eq!(list.len(), 4);
        assert_eq!(list[0].branch.as_deref(), Some("main"));
        assert!(list[1].detached && list[1].branch.is_none());
        assert!(list[2].prunable);
        assert!(list[3].bare);
    }

    #[test]
    fn entries_mark_open_checkouts_by_the_deepest_match() {
        let repo = Repo {
            home: "/h".into(),
            top: "/r".into(),
            main: "/r".into(),
            checkouts: parse_porcelain(
                "worktree /r\nbranch refs/heads/main\n\nworktree /r/nested\nbranch refs/heads/n\n\nworktree /w/d\ndetached\n",
            ),
        };
        let places = vec![
            ("sp1".to_owned(), "/r/src".to_owned()),
            ("sp2".to_owned(), "/r/nested/x".to_owned()),
        ];
        let list = entries(&repo, &places);
        assert_eq!(list[0].shown_in.as_deref(), Some("sp1"));
        assert_eq!(list[0].status, "open");
        assert_eq!(list[1].shown_in.as_deref(), Some("sp2"));
        assert_eq!(list[2].status, "detached");
        assert_eq!(list[2].label, "d");
    }

    #[test]
    fn force_is_offered_for_dirty_and_missing_checkouts() {
        assert!(is_force_needed(
            "fatal: '/w/a' contains modified or untracked files, use --force to delete it"
        ));
        assert!(is_force_needed("fatal: '/w/a' is not a working tree"));
        assert!(!is_force_needed("fatal: permission denied"));
    }

    #[test]
    fn open_filter_and_steps_stay_in_range() {
        let entries = ["main", "alpha", "beta"]
            .iter()
            .map(|b| Entry {
                path: format!("/w/{b}"),
                branch: Some((*b).to_owned()),
                label: (*b).to_owned(),
                status: "",
                shown_in: None,
            })
            .collect();
        let mut open = Open {
            entries,
            selected: 0,
            query: TextField::default(),
            searching: true,
            error: None,
            busy: false,
        };
        open.step(-1);
        assert_eq!(open.selected, 0);
        open.step(5);
        assert_eq!(open.selected, 2);
        open.query.insert("alp");
        open.reselect();
        assert_eq!(open.current().map(|e| e.label.as_str()), Some("alpha"));
    }
}
