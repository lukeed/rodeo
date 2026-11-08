use doc::Axis;
use ratatui::layout::{Position, Rect};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneHit {
    pub key: String,
    pub address: Option<String>,
    pub rect: Rect,
    pub inner: Rect,
    pub content: Rect,
    pub gutter: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitHit {
    pub machine: Option<String>,
    pub tab: String,
    pub container: String,
    pub index: usize,
    pub axis: Axis,
    pub hit: Rect,
    pub pos: u16,
    pub origin: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabHits {
    pub row: Option<Rect>,
    pub tabs: Vec<(Rect, String)>,
    pub new_tab: Option<Rect>,
    pub scroll_left: Option<Rect>,
    pub scroll_right: Option<Rect>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hits {
    pub panes: Vec<PaneHit>,
    pub splits: Vec<SplitHit>,
    pub tabs: TabHits,
    pub sidebar: crate::sidebar::Hits,
    pub toast: Option<Rect>,
    pub covers: Vec<Rect>,
    pub surface: Rect,
}

pub fn inside(rect: Rect, column: u16, row: u16) -> bool {
    rect.contains(Position::new(column, row))
}

impl Hits {
    pub fn pane_at(&self, column: u16, row: u16) -> Option<&PaneHit> {
        self.panes.iter().find(|p| inside(p.rect, column, row))
    }

    pub fn split_at(&self, column: u16, row: u16) -> Option<&SplitHit> {
        self.splits.iter().find(|s| inside(s.hit, column, row))
    }

    pub fn tab_at(&self, column: u16, row: u16) -> Option<&str> {
        self.tabs
            .tabs
            .iter()
            .find(|(rect, _)| inside(*rect, column, row))
            .map(|(_, key)| key.as_str())
    }

    pub fn clear_chrome(&mut self) {
        self.tabs = TabHits {
            row: self.tabs.row,
            ..TabHits::default()
        };
        self.sidebar.clear_chrome();
    }
}
