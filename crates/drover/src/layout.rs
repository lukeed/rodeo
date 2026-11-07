use std::collections::{BTreeMap, HashMap};

use config::{PaneBorders, TabBarPosition, UiConfig};
use doc::{Axis, Cell, Kind, Node, Slot, Tab};
use ratatui::layout::Rect;
use ratatui::widgets::Borders;

pub const COMPACT_SIDEBAR: u16 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Regions {
    pub sidebar: Rect,
    pub tab_bar: Option<Rect>,
    pub header: Option<Rect>,
    pub surface: Rect,
    pub mobile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarWidth {
    Expanded(u16),
    Compact,
    Hidden,
}

impl SidebarWidth {
    pub fn columns(self) -> u16 {
        match self {
            Self::Expanded(width) => width,
            Self::Compact => COMPACT_SIDEBAR,
            Self::Hidden => 0,
        }
    }
}

pub fn is_mobile(area: Rect, ui: &UiConfig) -> bool {
    area.width <= ui.mobile_width_threshold
}

pub fn regions(area: Rect, ui: &UiConfig, sidebar: SidebarWidth, tabs: usize) -> Regions {
    if is_mobile(area, ui) {
        let header = Rect::new(area.x, area.y, area.width, area.height.min(2));
        let surface = Rect::new(
            area.x,
            area.y + header.height,
            area.width,
            area.height - header.height,
        );
        return Regions {
            sidebar: Rect::new(area.x, area.y, 0, area.height),
            tab_bar: None,
            header: Some(header),
            surface,
            mobile: true,
        };
    }
    let width = sidebar.columns().min(area.width.saturating_sub(1));
    let sidebar = Rect::new(area.x, area.y, width, area.height);
    let main = Rect::new(area.x + width, area.y, area.width - width, area.height);
    let shows_tabs = main.height > 1 && !(ui.hide_tab_bar_when_single_tab && tabs == 1);
    if !shows_tabs {
        return Regions {
            sidebar,
            tab_bar: None,
            header: None,
            surface: main,
            mobile: false,
        };
    }
    let rest = Rect::new(main.x, main.y, main.width, main.height - 1);
    let (tab_bar, surface) = match ui.tab_bar_position {
        TabBarPosition::Top => (
            Rect::new(main.x, main.y, main.width, 1),
            Rect::new(main.x, main.y + 1, main.width, main.height - 1),
        ),
        TabBarPosition::Bottom => (Rect::new(main.x, main.bottom() - 1, main.width, 1), rest),
    };
    Regions {
        sidebar,
        tab_bar: Some(tab_bar),
        header: None,
        surface,
        mobile: false,
    }
}

pub fn expanded_width(ui: &UiConfig, saved: Option<u16>) -> u16 {
    let (min, max) = ui.sidebar_bounds_or_default();
    saved.unwrap_or(ui.sidebar_width).clamp(min, max)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chrome {
    pub borders: PaneBorders,
    pub outer: bool,
    pub gaps: bool,
    pub scrollbars: bool,
}

impl Chrome {
    pub fn from_ui(ui: &UiConfig) -> Self {
        Self {
            borders: ui.pane_borders,
            outer: ui.pane_outer_borders,
            gaps: ui.pane_gaps,
            scrollbars: ui.pane_scrollbars,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneGeom {
    pub id: String,
    pub rect: Rect,
    pub inner: Rect,
    pub content: Rect,
    pub gutter: Option<Rect>,
    pub borders: Borders,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitGeom {
    pub container: String,
    pub index: usize,
    pub axis: Axis,
    pub pos: u16,
    pub area: Rect,
    pub first: Rect,
    pub hit: Option<Rect>,
}

impl SplitGeom {
    pub fn origin(&self) -> u16 {
        match self.axis {
            Axis::Row => self.first.x,
            Axis::Column => self.first.y,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabGeom {
    pub panes: Vec<PaneGeom>,
    pub splits: Vec<SplitGeom>,
    pub nodes: HashMap<String, Rect>,
    pub zoomed: bool,
}

impl TabGeom {
    pub fn pane(&self, id: &str) -> Option<&PaneGeom> {
        self.panes.iter().find(|p| p.id == id)
    }

    pub fn rect_of(&self, id: &str) -> Option<doc::Rect> {
        self.nodes.get(id).map(|r| to_doc(*r))
    }

    pub fn neighbor(&self, from: &str, direction: doc::Direction) -> Option<String> {
        let rects: Vec<(&str, doc::Rect)> = self
            .panes
            .iter()
            .map(|p| (p.id.as_str(), to_doc(p.rect)))
            .collect();
        doc::neighbor(from, direction, &rects).map(str::to_owned)
    }
}

pub fn to_doc(r: Rect) -> doc::Rect {
    doc::Rect::new(r.x, r.y, r.width, r.height)
}

pub fn from_doc(r: doc::Rect) -> Rect {
    Rect::new(r.x, r.y, r.width, r.height)
}

pub fn tab(
    tab: &Tab,
    area: Rect,
    chrome: Chrome,
    zoom: Option<&str>,
    cell: Cell,
    gutter: &dyn Fn(&str) -> bool,
) -> TabGeom {
    let mut geom = TabGeom::default();
    let mut frames = Vec::new();
    place(&tab.root, area, cell, &mut geom, &mut frames);
    let multi = frames.len() > 1;
    if let Some(zoomed) = zoom.filter(|z| frames.iter().any(|(id, _)| id == z)) {
        let borders = if chrome.borders.shows_borders(multi) && chrome.outer {
            Borders::ALL
        } else {
            Borders::NONE
        };
        geom.panes = vec![finish(zoomed.to_owned(), area, borders, chrome, gutter)];
        geom.splits.clear();
        geom.zoomed = true;
        return geom;
    }
    let bordered = chrome.borders.shows_borders(multi);
    let outer_left = frames.iter().map(|(_, r)| r.x).min().unwrap_or(0);
    let outer_top = frames.iter().map(|(_, r)| r.y).min().unwrap_or(0);
    let outer_right = frames.iter().map(|(_, r)| r.right()).max().unwrap_or(0);
    let outer_bottom = frames.iter().map(|(_, r)| r.bottom()).max().unwrap_or(0);
    let rects: Vec<Rect> = frames.iter().map(|(_, r)| *r).collect();
    for (id, rect) in &frames {
        let mut rect = *rect;
        let right = multi
            && rects.iter().any(|o| {
                *o != rect && o.x == rect.right() && overlap(rect.y, rect.height, o.y, o.height)
            });
        let below = multi
            && rects.iter().any(|o| {
                *o != rect && o.y == rect.bottom() && overlap(rect.x, rect.width, o.x, o.width)
            });
        if multi && chrome.gaps && !chrome.borders.draws_borders() {
            if right && rect.width > 1 {
                rect.width -= 1;
            }
            if below && rect.height > 1 {
                rect.height -= 1;
            }
        }
        let borders = if bordered {
            let mut borders = Borders::ALL;
            if !chrome.gaps {
                if right {
                    borders.remove(Borders::RIGHT);
                }
                if below {
                    borders.remove(Borders::BOTTOM);
                }
            }
            if !chrome.outer {
                if rect.x == outer_left {
                    borders.remove(Borders::LEFT);
                }
                if rect.y == outer_top {
                    borders.remove(Borders::TOP);
                }
                if rect.right() == outer_right {
                    borders.remove(Borders::RIGHT);
                }
                if rect.bottom() == outer_bottom {
                    borders.remove(Borders::BOTTOM);
                }
            }
            borders
        } else {
            Borders::NONE
        };
        geom.panes
            .push(finish(id.clone(), rect, borders, chrome, gutter));
    }
    let frames: Vec<Rect> = geom.panes.iter().map(|p| p.rect).collect();
    for split in &mut geom.splits {
        split.hit = split_hit(split, chrome, &frames);
    }
    geom
}

fn overlap(a: u16, a_len: u16, b: u16, b_len: u16) -> bool {
    a < b.saturating_add(b_len) && b < a.saturating_add(a_len)
}

fn finish(
    id: String,
    rect: Rect,
    borders: Borders,
    chrome: Chrome,
    gutter: &dyn Fn(&str) -> bool,
) -> PaneGeom {
    let inner = inner(rect, borders);
    let (content, gutter) = if chrome.scrollbars && inner.width > 4 && gutter(&id) {
        (
            Rect::new(inner.x, inner.y, inner.width - 1, inner.height),
            Some(Rect::new(inner.right() - 1, inner.y, 1, inner.height)),
        )
    } else {
        (inner, None)
    };
    PaneGeom {
        id,
        rect,
        inner,
        content,
        gutter,
        borders,
    }
}

pub fn inner(rect: Rect, borders: Borders) -> Rect {
    let left = u16::from(borders.contains(Borders::LEFT));
    let top = u16::from(borders.contains(Borders::TOP));
    let right = u16::from(borders.contains(Borders::RIGHT));
    let bottom = u16::from(borders.contains(Borders::BOTTOM));
    let x = rect.x.saturating_add(left).min(rect.right());
    let y = rect.y.saturating_add(top).min(rect.bottom());
    Rect::new(
        x,
        y,
        rect.width.saturating_sub(left + right),
        rect.height.saturating_sub(top + bottom),
    )
}

fn place(
    node: &Node,
    area: Rect,
    cell: Cell,
    geom: &mut TabGeom,
    frames: &mut Vec<(String, Rect)>,
) {
    if node.hidden {
        return;
    }
    geom.nodes.insert(node.id.clone(), area);
    match &node.kind {
        Kind::Block { .. } => frames.push((node.id.clone(), area)),
        Kind::Stack { .. } => {
            if let Some(child) = node.active_child() {
                place(child, area, cell, geom, frames);
            }
        }
        Kind::Row { children } | Kind::Column { children } => {
            let axis = node.axis().unwrap_or(Axis::Row);
            let shown: Vec<&Node> = children.iter().filter(|c| !c.hidden).collect();
            let slots: Vec<Slot> = shown.iter().map(|c| Slot::of(c)).collect();
            let rects = doc::split_rect(to_doc(area), axis, &slots, cell);
            for (i, (child, rect)) in shown.iter().zip(&rects).enumerate() {
                let rect = from_doc(*rect);
                if let Some(next) = rects.get(i + 1) {
                    let next = from_doc(*next);
                    geom.splits.push(SplitGeom {
                        container: node.id.clone(),
                        index: i,
                        axis,
                        pos: match axis {
                            Axis::Row => next.x,
                            Axis::Column => next.y,
                        },
                        area,
                        first: rect,
                        hit: None,
                    });
                }
                place(child, rect, cell, geom, frames);
            }
        }
    }
}

fn split_hit(split: &SplitGeom, chrome: Chrome, frames: &[Rect]) -> Option<Rect> {
    let borders = chrome.borders.draws_borders();
    let area = split.area;
    let hit = match (split.axis, borders, chrome.gaps) {
        (Axis::Row, true, false) => Rect::new(split.pos, area.y, 1, area.height),
        (Axis::Row, true, true) => {
            let start = split.pos.saturating_sub(1);
            Rect::new(start, area.y, split.pos - start + 1, area.height)
        }
        (Axis::Row, false, true) => Rect::new(split.pos.checked_sub(1)?, area.y, 1, area.height),
        (Axis::Column, true, false) => Rect::new(area.x, split.pos, area.width, 1),
        (Axis::Column, true, true) => {
            let start = split.pos.saturating_sub(1);
            Rect::new(area.x, start, area.width, split.pos - start + 1)
        }
        (Axis::Column, false, true) => Rect::new(area.x, split.pos.checked_sub(1)?, area.width, 1),
        (_, false, false) => return None,
    };
    if !borders && frames.iter().any(|f| f.intersects(hit)) {
        return None;
    }
    Some(hit)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Line {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl Line {
    pub fn symbol(self) -> &'static str {
        match (self.up, self.down, self.left, self.right) {
            (true, true, true, true) => "┼",
            (true, true, true, false) => "┤",
            (true, true, false, true) => "├",
            (true, false, true, true) => "┴",
            (false, true, true, true) => "┬",
            (true, true, false, false)
            | (true, false, false, false)
            | (false, true, false, false) => "│",
            (false, false, true, true)
            | (false, false, true, false)
            | (false, false, false, true) => "─",
            (false, true, false, true) => "┌",
            (false, true, true, false) => "┐",
            (true, false, false, true) => "└",
            (true, false, true, false) => "┘",
            (false, false, false, false) => "",
        }
    }
}

pub fn border_lines(geom: &TabGeom, gaps: bool) -> BTreeMap<(u16, u16), Line> {
    let mut cells: BTreeMap<(u16, u16), Line> = BTreeMap::new();
    for pane in &geom.panes {
        let r = pane.rect;
        if r.width == 0 || r.height == 0 {
            continue;
        }
        let right = r.right() - 1;
        let bottom = r.bottom() - 1;
        if pane.borders.contains(Borders::TOP) {
            for x in r.x..=right {
                let cell = cells.entry((x, r.y)).or_default();
                cell.left |= x > r.x;
                cell.right |= x < right;
            }
        }
        if pane.borders.contains(Borders::BOTTOM) {
            for x in r.x..=right {
                let cell = cells.entry((x, bottom)).or_default();
                cell.left |= x > r.x;
                cell.right |= x < right;
            }
        }
        if pane.borders.contains(Borders::LEFT) {
            for y in r.y..=bottom {
                let cell = cells.entry((r.x, y)).or_default();
                cell.up |= y > r.y;
                cell.down |= y < bottom;
            }
        }
        if pane.borders.contains(Borders::RIGHT) {
            for y in r.y..=bottom {
                let cell = cells.entry((right, y)).or_default();
                cell.up |= y > r.y;
                cell.down |= y < bottom;
            }
        }
    }
    if !gaps {
        for split in &geom.splits {
            join_split(split, &mut cells);
        }
    }
    cells
}

fn join_split(split: &SplitGeom, cells: &mut BTreeMap<(u16, u16), Line>) {
    let horizontal = |c: Option<&Line>| c.is_some_and(|c| c.left || c.right);
    let vertical = |c: Option<&Line>| c.is_some_and(|c| c.up || c.down);
    match split.axis {
        Axis::Row => {
            let x = split.pos;
            let end = split.area.bottom();
            for y in split.area.y..=end {
                if !cells.contains_key(&(x, y)) {
                    continue;
                }
                let left = x
                    .checked_sub(1)
                    .is_some_and(|lx| horizontal(cells.get(&(lx, y))));
                let right = horizontal(cells.get(&(x + 1, y)));
                let cell = cells.entry((x, y)).or_default();
                cell.up |= y > split.area.y;
                cell.down |= y + 1 < end;
                cell.left |= left;
                cell.right |= right;
            }
        }
        Axis::Column => {
            let y = split.pos;
            let end = split.area.right();
            for x in split.area.x..=end {
                if !cells.contains_key(&(x, y)) {
                    continue;
                }
                let up = y
                    .checked_sub(1)
                    .is_some_and(|uy| vertical(cells.get(&(x, uy))));
                let down = vertical(cells.get(&(x, y + 1)));
                let cell = cells.entry((x, y)).or_default();
                cell.left |= x > split.area.x;
                cell.right |= x + 1 < end;
                cell.up |= up;
                cell.down |= down;
            }
        }
    }
}

pub fn touches(x: u16, y: u16, rect: Rect, gaps: bool) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let right = rect.right() - 1;
    let bottom = rect.bottom() - 1;
    let in_rows = y >= rect.y && y <= bottom;
    let in_cols = x >= rect.x && x <= right;
    let own = (in_rows && (x == rect.x || x == right)) || (in_cols && (y == rect.y || y == bottom));
    if gaps {
        return own;
    }
    own || (in_rows && x == rect.right())
        || (in_cols && y == rect.bottom())
        || (x == rect.right() && y == rect.bottom())
}

#[cfg(test)]
mod tests {
    use doc::{Node, Size};

    use super::*;

    const CELL: Cell = Cell {
        width: 8,
        height: 16,
    };

    fn chrome(borders: PaneBorders, gaps: bool) -> Chrome {
        Chrome {
            borders,
            outer: true,
            gaps,
            scrollbars: true,
        }
    }

    fn two_side_by_side() -> Tab {
        let mut tab = Tab::new("tb", "", Node::terminal("a", "t1"));
        tab.root
            .children_mut()
            .unwrap()
            .push(Node::terminal("b", "t2"));
        tab
    }

    #[test]
    fn desktop_regions_follow_the_sidebar_and_tab_bar() {
        let ui = UiConfig::default();
        let area = Rect::new(0, 0, 120, 40);
        let r = regions(area, &ui, SidebarWidth::Expanded(26), 1);
        assert_eq!(r.sidebar, Rect::new(0, 0, 26, 40));
        assert_eq!(r.tab_bar, Some(Rect::new(26, 0, 94, 1)));
        assert_eq!(r.surface, Rect::new(26, 1, 94, 39));
        let bottom = UiConfig {
            tab_bar_position: TabBarPosition::Bottom,
            hide_tab_bar_when_single_tab: true,
            ..UiConfig::default()
        };
        let r = regions(area, &bottom, SidebarWidth::Compact, 2);
        assert_eq!(r.tab_bar, Some(Rect::new(4, 39, 116, 1)));
        assert_eq!(r.surface, Rect::new(4, 0, 116, 39));
        let r = regions(area, &bottom, SidebarWidth::Hidden, 1);
        assert_eq!(r.tab_bar, None);
        assert_eq!(r.surface, area);
    }

    #[test]
    fn narrow_screens_use_the_mobile_layout() {
        let ui = UiConfig::default();
        let r = regions(Rect::new(0, 0, 60, 20), &ui, SidebarWidth::Expanded(26), 3);
        assert!(r.mobile);
        assert_eq!(r.header, Some(Rect::new(0, 0, 60, 2)));
        assert_eq!(r.surface, Rect::new(0, 2, 60, 18));
        assert_eq!(r.sidebar.width, 0);
    }

    #[test]
    fn a_lone_pane_has_no_borders_in_auto_mode() {
        let tab = Tab::new("tb", "", Node::terminal("a", "t1"));
        let area = Rect::new(0, 1, 80, 23);
        let g = super::tab(
            &tab,
            area,
            chrome(PaneBorders::Auto, true),
            None,
            CELL,
            &|_| true,
        );
        assert_eq!(g.panes[0].rect, area);
        assert_eq!(g.panes[0].borders, Borders::NONE);
        assert_eq!(g.panes[0].content, Rect::new(0, 1, 79, 23));
        assert_eq!(g.panes[0].gutter, Some(Rect::new(79, 1, 1, 23)));
        let always = super::tab(
            &tab,
            area,
            chrome(PaneBorders::Always, true),
            None,
            CELL,
            &|_| false,
        );
        assert_eq!(always.panes[0].borders, Borders::ALL);
        assert_eq!(always.panes[0].content, Rect::new(1, 2, 78, 21));
    }

    #[test]
    fn split_panes_get_boxes_and_a_divider_hit() {
        let area = Rect::new(0, 0, 80, 24);
        let g = super::tab(
            &two_side_by_side(),
            area,
            chrome(PaneBorders::Auto, true),
            None,
            CELL,
            &|_| false,
        );
        assert_eq!(g.panes[0].rect, Rect::new(0, 0, 40, 24));
        assert_eq!(g.panes[1].rect, Rect::new(40, 0, 40, 24));
        assert_eq!(g.panes[0].inner, Rect::new(1, 1, 38, 22));
        assert_eq!(g.splits.len(), 1);
        assert_eq!(g.splits[0].hit, Some(Rect::new(39, 0, 2, 24)));
        assert_eq!(g.rect_of("tb"), Some(doc::Rect::new(0, 0, 80, 24)));
        assert_eq!(g.neighbor("a", doc::Direction::Right).as_deref(), Some("b"));
        let shared = super::tab(
            &two_side_by_side(),
            area,
            chrome(PaneBorders::Auto, false),
            None,
            CELL,
            &|_| false,
        );
        assert!(!shared.panes[0].borders.contains(Borders::RIGHT));
        assert_eq!(shared.splits[0].hit, Some(Rect::new(40, 0, 1, 24)));
        let lines = border_lines(&shared, false);
        assert_eq!(lines[&(40, 0)].symbol(), "┬");
        assert_eq!(lines[&(40, 23)].symbol(), "┴");
        assert_eq!(lines[&(0, 0)].symbol(), "┌");
        assert_eq!(lines[&(40, 5)].symbol(), "│");
    }

    #[test]
    fn borderless_gaps_leave_a_blank_column() {
        let area = Rect::new(0, 0, 80, 24);
        let g = super::tab(
            &two_side_by_side(),
            area,
            chrome(PaneBorders::Off, true),
            None,
            CELL,
            &|_| false,
        );
        assert_eq!(g.panes[0].rect, Rect::new(0, 0, 39, 24));
        assert_eq!(g.panes[0].borders, Borders::NONE);
        assert_eq!(g.splits[0].hit, Some(Rect::new(39, 0, 1, 24)));
    }

    #[test]
    fn weights_and_zoom_shape_the_rects() {
        let mut tab = two_side_by_side();
        tab.root.children_mut().unwrap()[0].size = Some(Size::Weight { weight: 3.0 });
        let area = Rect::new(0, 0, 80, 24);
        let g = super::tab(
            &tab,
            area,
            chrome(PaneBorders::Auto, true),
            None,
            CELL,
            &|_| false,
        );
        assert_eq!(g.panes[0].rect.width, 60);
        let z = super::tab(
            &tab,
            area,
            chrome(PaneBorders::Auto, true),
            Some("b"),
            CELL,
            &|_| false,
        );
        assert!(z.zoomed);
        assert_eq!(z.panes.len(), 1);
        assert_eq!(z.panes[0].rect, area);
        assert_eq!(z.panes[0].borders, Borders::ALL);
        assert!(z.splits.is_empty());
    }

    #[test]
    fn focus_color_touches_shared_dividers_only_without_gaps() {
        let rect = Rect::new(0, 0, 40, 24);
        assert!(touches(39, 3, rect, true));
        assert!(!touches(40, 3, rect, true));
        assert!(touches(40, 3, rect, false));
    }
}
