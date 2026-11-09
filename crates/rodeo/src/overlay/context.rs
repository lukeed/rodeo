use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::menu::{List, clamp};
use super::{Ctx, Drawn, Kind, MenuTarget, Outcome, Overlay, Request};
use crate::model::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Rename,
    Close,
    NewWorktree,
    OpenWorktree,
    RemoveWorktree,
    ToggleGroup,
    NewTab,
    RenamePane,
    ClearPaneName,
    Swap,
    SplitRight,
    SplitDown,
    Zoom,
    RightClick,
    ClosePane,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpaceFlags {
    pub git: bool,
    pub linked: bool,
    pub children: bool,
    pub close_group: bool,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaneFlags {
    pub named: bool,
    pub focused: bool,
    pub right_click: bool,
}

pub fn space_items(flags: SpaceFlags) -> Vec<(&'static str, Pick)> {
    let mut items = vec![("Rename", Pick::Rename)];
    if !flags.git {
        items.push(("Close", Pick::Close));
        return items;
    }
    if flags.linked {
        items.push(("Close", Pick::Close));
        items.push(("Delete worktree checkout...", Pick::RemoveWorktree));
        return items;
    }
    let close = if flags.children && flags.close_group {
        "Close group"
    } else {
        "Close"
    };
    items.push((close, Pick::Close));
    items.push(("New worktree", Pick::NewWorktree));
    items.push(("Open worktree...", Pick::OpenWorktree));
    if flags.children {
        let toggle = if flags.collapsed {
            "Expand"
        } else {
            "Collapse"
        };
        items.push((toggle, Pick::ToggleGroup));
    }
    items
}

pub fn tab_items() -> Vec<(&'static str, Pick)> {
    vec![
        ("New tab", Pick::NewTab),
        ("Rename", Pick::Rename),
        ("Close", Pick::Close),
    ]
}

pub fn pane_items(flags: PaneFlags) -> Vec<(&'static str, Pick)> {
    let mut items = vec![("Rename pane", Pick::RenamePane)];
    if flags.named {
        items.push(("Clear pane name", Pick::ClearPaneName));
    }
    if !flags.focused {
        items.push(("Swap with focused pane", Pick::Swap));
    }
    let right_click = if flags.right_click {
        "Use Herdr right-click menu"
    } else {
        "Send right-clicks to pane"
    };
    items.extend([
        ("Split right", Pick::SplitRight),
        ("Split down", Pick::SplitDown),
        ("Zoom", Pick::Zoom),
        (right_click, Pick::RightClick),
        ("Close pane", Pick::ClosePane),
    ]);
    items
}

pub fn pane_flags(snapshot: &Snapshot, pane: &str) -> Option<PaneFlags> {
    let row = snapshot.panes.iter().find(|p| p.key == pane)?;
    Some(PaneFlags {
        named: row.title.is_some(),
        focused: snapshot.focused_pane.as_deref() == Some(pane),
        right_click: row.right_click,
    })
}

pub struct ContextMenu {
    target: MenuTarget,
    picks: Vec<Pick>,
    list: List,
    at: (u16, u16),
}

impl ContextMenu {
    pub fn new(target: MenuTarget, items: Vec<(&'static str, Pick)>, at: (u16, u16)) -> Self {
        let (labels, picks): (Vec<String>, Vec<Pick>) =
            items.into_iter().map(|(l, p)| (l.to_owned(), p)).unzip();
        Self {
            target,
            picks,
            list: List::new(labels),
            at,
        }
    }

    pub fn target(&self) -> &MenuTarget {
        &self.target
    }

    pub fn labels(&self) -> &[String] {
        &self.list.labels
    }

    pub fn highlighted(&self) -> usize {
        self.list.highlighted
    }

    fn pick(&self, index: usize) -> Outcome {
        self.picks.get(index).map_or(Outcome::Close, |pick| {
            Outcome::Done(Request::Menu {
                target: self.target.clone(),
                pick: *pick,
            })
        })
    }
}

impl Overlay for ContextMenu {
    fn kind(&self) -> Kind {
        Kind::ContextMenu(self.target.clone())
    }

    fn dims(&self) -> bool {
        false
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let rect = clamp(area, self.at.0, self.at.1, self.list.size(4, 14));
        self.list.render(buf, rect, ctx.palette);
        Some(Drawn::new(rect, None))
    }

    fn key(&mut self, key: &KeyEvent, _ctx: &Ctx<'_>) -> Outcome {
        match self.list.key(key, false) {
            Some(Some(index)) => self.pick(index),
            Some(None) => Outcome::Close,
            None => Outcome::Stay,
        }
    }

    fn mouse(&mut self, event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        let hit = self.list.at(event.column, event.row);
        match event.kind {
            MouseEventKind::Moved => {
                if let Some(index) = hit {
                    self.list.highlighted = index;
                }
                Outcome::Stay
            }
            MouseEventKind::Down(MouseButton::Left) => {
                hit.map_or(Outcome::Close, |index| self.pick(index))
            }
            _ => Outcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[(&'static str, Pick)]) -> Vec<&'static str> {
        items.iter().map(|(l, _)| *l).collect()
    }

    #[test]
    fn workspace_items_follow_git_state() {
        assert_eq!(
            labels(&space_items(SpaceFlags::default())),
            ["Rename", "Close"]
        );
        let git = SpaceFlags {
            git: true,
            ..SpaceFlags::default()
        };
        assert_eq!(
            labels(&space_items(git)),
            ["Rename", "Close", "New worktree", "Open worktree..."]
        );
        let linked = SpaceFlags {
            linked: true,
            ..git
        };
        assert_eq!(
            labels(&space_items(linked)),
            ["Rename", "Close", "Delete worktree checkout..."]
        );
        let parent = SpaceFlags {
            children: true,
            close_group: true,
            collapsed: true,
            ..git
        };
        assert_eq!(
            labels(&space_items(parent)),
            [
                "Rename",
                "Close group",
                "New worktree",
                "Open worktree...",
                "Expand"
            ]
        );
    }

    #[test]
    fn pane_items_follow_the_pane() {
        assert_eq!(
            labels(&pane_items(PaneFlags {
                focused: true,
                ..PaneFlags::default()
            })),
            [
                "Rename pane",
                "Split right",
                "Split down",
                "Zoom",
                "Send right-clicks to pane",
                "Close pane"
            ]
        );
        let other = pane_items(PaneFlags {
            named: true,
            focused: false,
            right_click: true,
        });
        assert_eq!(
            labels(&other)[..3],
            ["Rename pane", "Clear pane name", "Swap with focused pane"]
        );
        assert!(labels(&other).contains(&"Use Herdr right-click menu"));
    }
}
