use std::collections::{HashMap, HashSet};
use std::time::Instant;

use doc::Axis;
use vt::{CopyMode, Direction, Point, Selection};

use crate::machines::{Machines, SpaceRef};
use crate::overlay::{Overlay, TextField};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Terminal,
    Prefix,
    Navigate,
    Resize,
    Copy,
}

pub struct CopyState {
    pub address: String,
    pub mode: CopyMode,
    pub prompt: Option<Prompt>,
}

pub struct Prompt {
    pub direction: Direction,
    pub field: TextField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseSelection {
    pub address: String,
    pub selection: Selection,
    pub dragging: bool,
    pub visible: bool,
    pub clear_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Drag {
    Split {
        machine: Option<String>,
        tab: String,
        container: String,
        index: usize,
        axis: Axis,
        grab: i32,
        origin: u16,
        preview: Option<f32>,
    },
    SidebarEdge,
    SectionDivider,
    Select {
        address: String,
        anchor: Point,
        word: bool,
    },
    Program {
        address: String,
    },
    Scrollbar {
        address: String,
        grab: u16,
    },
    Press {
        target: Press,
        at: (u16, u16),
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Press {
    Space(String),
    Tab(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    pub at: (u16, u16),
    pub when: Instant,
    pub target: ClickTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickTarget {
    Pane,
    SidebarEdge,
}

pub struct View {
    pub space: Option<String>,
    pub tabs: HashMap<String, String>,
    pub focus: HashMap<String, String>,
    pub zoomed: HashSet<String>,
    pub previous: Option<String>,
    pub mode: Mode,
    pub nav: Option<String>,
    pub highlight: Option<(String, Instant)>,
    pub tab_scroll: usize,
    pub reveal_tab: bool,
    pub host_focused: bool,
    pub copy: Option<CopyState>,
    pub selection: Option<MouseSelection>,
    pub drag: Option<Drag>,
    pub click: Option<Click>,
    pub overlay: Option<Box<dyn Overlay>>,
    pub sidebar_scroll: usize,
    pub agent_scroll: usize,
    pub want_focus: Option<String>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            space: None,
            tabs: HashMap::new(),
            focus: HashMap::new(),
            zoomed: HashSet::new(),
            previous: None,
            mode: Mode::Terminal,
            nav: None,
            highlight: None,
            tab_scroll: 0,
            reveal_tab: true,
            host_focused: true,
            copy: None,
            selection: None,
            drag: None,
            click: None,
            overlay: None,
            sidebar_scroll: 0,
            agent_scroll: 0,
            want_focus: None,
        }
    }
}

impl View {
    pub fn active_tab(&self, machines: &Machines) -> Option<String> {
        let space = machines.space(self.space.as_deref()?)?;
        let chosen = self
            .tabs
            .get(&space.key())
            .filter(|t| space.space.tabs.iter().any(|tab| &space.tab_key(tab) == *t));
        chosen
            .cloned()
            .or_else(|| space.space.tabs.first().map(|t| space.tab_key(t)))
    }

    pub fn tab_of_space(&self, machines: &Machines, space: &str) -> Option<String> {
        let space = machines.space(space)?;
        self.tabs
            .get(&space.key())
            .filter(|t| machines.tab(t).is_some())
            .cloned()
            .or_else(|| space.space.tabs.first().map(|t| space.tab_key(t)))
    }

    pub fn focused_in(&self, machines: &Machines, tab: &str) -> Option<String> {
        let found = machines.tab(tab)?;
        self.focus
            .get(tab)
            .filter(|p| found.panes().any(|pane| &pane.key() == *p))
            .cloned()
            .or_else(|| found.panes().next().map(|p| p.key()))
    }

    pub fn focused_pane(&self, machines: &Machines) -> Option<String> {
        self.focused_in(machines, &self.active_tab(machines)?)
    }

    pub fn fix(&mut self, machines: &Machines) {
        let spaces = machines.spaces();
        if self
            .space
            .as_ref()
            .is_none_or(|s| machines.space(s).is_none())
        {
            self.space = spaces.first().map(SpaceRef::key);
        }
        let tabs: HashSet<String> = machines.tabs().map(|t| t.key()).collect();
        let panes: HashSet<String> = machines.panes().map(|p| p.key()).collect();
        let spaces: HashSet<String> = spaces.iter().map(SpaceRef::key).collect();
        self.tabs
            .retain(|space, tab| spaces.contains(space) && tabs.contains(tab));
        self.focus
            .retain(|tab, pane| tabs.contains(tab) && panes.contains(pane));
        self.zoomed.retain(|tab| tabs.contains(tab));
        if self.previous.as_ref().is_some_and(|p| !panes.contains(p)) {
            self.previous = None;
        }
        for tab in machines.tabs() {
            let key = tab.key();
            if self.focus.contains_key(&key) {
                continue;
            }
            let previous = self
                .previous
                .clone()
                .filter(|p| tab.panes().any(|pane| &pane.key() == p));
            if let Some(pane) = previous.or_else(|| tab.panes().next().map(|p| p.key())) {
                self.focus.insert(key, pane);
            }
        }
        for space in machines.spaces() {
            if let Some(first) = space.space.tabs.first() {
                self.tabs
                    .entry(space.key())
                    .or_insert_with(|| space.tab_key(first));
            }
        }
        if self.nav.as_ref().is_some_and(|s| !spaces.contains(s)) {
            self.nav = None;
        }
    }

    pub fn show_space(&mut self, space: &str) {
        self.space = Some(space.to_owned());
        self.reveal_tab = true;
    }

    pub fn show_tab(&mut self, space: &str, tab: &str) {
        self.show_space(space);
        self.tabs.insert(space.to_owned(), tab.to_owned());
    }

    pub fn is_zoomed(&self, tab: &str) -> bool {
        self.zoomed.contains(tab)
    }
}
