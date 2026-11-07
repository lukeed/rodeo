use std::fmt;

use serde_json::Value;

use crate::id::Prefix;
use crate::schema::{Axis, Document, Node, Space, Tab};
use crate::tree::Split;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NoSpace(String),
    NoTab(String),
    NoPane(String),
    NoNode(String),
    NotClosable(String),
    IdTaken(String),
    SamePane(String),
    OtherTab(String),
    NoDivider(String, usize),
    Unmeasured(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoSpace(id) => write!(f, "no space {id:?}"),
            Error::NoTab(id) => write!(f, "no tab {id:?}"),
            Error::NoPane(id) => write!(f, "no pane {id:?}"),
            Error::NoNode(id) => write!(f, "no row or column {id:?}"),
            Error::NotClosable(id) => write!(f, "pane {id:?} cannot be closed"),
            Error::IdTaken(id) => write!(f, "id {id:?} is already in the layout"),
            Error::SamePane(id) => write!(f, "pane {id:?} cannot swap with itself"),
            Error::OtherTab(id) => write!(f, "pane {id:?} is in another tab"),
            Error::NoDivider(id, i) => write!(f, "{id:?} has no divider after child {i}"),
            Error::Unmeasured(id) => write!(f, "no size known for {id:?}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Closed {
    pub panes: Vec<String>,
    pub tabs: Vec<String>,
    pub space: Option<String>,
    pub terminals: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub space: String,
    pub tab: Option<String>,
}

impl Place {
    pub fn from_labels(labels: &Value) -> Option<Place> {
        let label = |key: &str| {
            labels
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        Some(Place {
            space: label("herdr.space")?,
            tab: label("herdr.tab"),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Adopted {
    Present {
        pane: String,
    },
    NewSpace {
        space: String,
        tab: String,
        pane: String,
    },
    Split {
        tab: String,
        pane: String,
        container: Option<String>,
    },
    NewTab {
        tab: String,
        pane: String,
    },
}

impl Document {
    pub fn new_space(&mut self, id: &str, name: &str, folder: &str, tab: Tab) -> Result<(), Error> {
        self.free(id)?;
        self.tab_ids_free(&tab)?;
        self.spaces.push(Space {
            id: id.to_owned(),
            name: name.trim().to_owned(),
            folder: folder.to_owned(),
            tabs: vec![trimmed(tab)],
        });
        Ok(())
    }

    pub fn close_space(&mut self, id: &str) -> Result<Closed, Error> {
        let i = self
            .spaces
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| Error::NoSpace(id.to_owned()))?;
        let space = self.spaces.remove(i);
        Ok(Closed {
            panes: space
                .tabs
                .iter()
                .flat_map(|t| t.panes().into_iter().map(|p| p.id.clone()))
                .collect(),
            tabs: space.tabs.iter().map(|t| t.id.clone()).collect(),
            terminals: space.addresses(),
            space: Some(space.id),
        })
    }

    pub fn rename_space(&mut self, id: &str, name: &str) -> Result<(), Error> {
        let space = self
            .space_mut(id)
            .ok_or_else(|| Error::NoSpace(id.to_owned()))?;
        name.trim().clone_into(&mut space.name);
        Ok(())
    }

    pub fn new_tab(&mut self, space: &str, tab: Tab, after: Option<&str>) -> Result<(), Error> {
        self.tab_ids_free(&tab)?;
        let target = self
            .space_mut(space)
            .ok_or_else(|| Error::NoSpace(space.to_owned()))?;
        let at = after
            .and_then(|a| target.tab_index(a))
            .map_or(target.tabs.len(), |i| i + 1);
        target.tabs.insert(at, trimmed(tab));
        Ok(())
    }

    pub fn close_tab(&mut self, id: &str) -> Result<Closed, Error> {
        let (si, ti) = self
            .spaces
            .iter()
            .enumerate()
            .find_map(|(si, s)| s.tab_index(id).map(|ti| (si, ti)))
            .ok_or_else(|| Error::NoTab(id.to_owned()))?;
        if self.spaces[si].tabs.len() == 1 {
            let space = self.spaces[si].id.clone();
            return self.close_space(&space);
        }
        let tab = self.spaces[si].tabs.remove(ti);
        Ok(Closed {
            panes: tab.panes().into_iter().map(|p| p.id.clone()).collect(),
            terminals: tab.addresses(),
            tabs: vec![tab.id],
            space: None,
        })
    }

    pub fn rename_tab(&mut self, id: &str, name: &str) -> Result<(), Error> {
        let tab = self
            .tab_mut(id)
            .ok_or_else(|| Error::NoTab(id.to_owned()))?;
        name.trim().clone_into(&mut tab.name);
        Ok(())
    }

    pub fn move_tab(&mut self, id: &str, to: usize) -> Result<(), Error> {
        let space = self
            .spaces
            .iter_mut()
            .find(|s| s.tab(id).is_some())
            .ok_or_else(|| Error::NoTab(id.to_owned()))?;
        let from = space.tab_index(id).expect("the space holds the tab");
        let tab = space.tabs.remove(from);
        let to = to.min(space.tabs.len());
        space.tabs.insert(to, tab);
        Ok(())
    }

    pub fn step_tab(&mut self, id: &str, forward: bool) -> Result<(), Error> {
        let space = self
            .spaces
            .iter()
            .find(|s| s.tab(id).is_some())
            .ok_or_else(|| Error::NoTab(id.to_owned()))?;
        let (from, n) = (
            space.tab_index(id).expect("the space holds the tab"),
            space.tabs.len(),
        );
        let to = match (forward, from) {
            (true, i) if i + 1 >= n => 0,
            (true, i) => i + 1,
            (false, 0) => n - 1,
            (false, i) => i - 1,
        };
        self.move_tab(id, to)
    }

    pub fn split_pane(
        &mut self,
        target: &str,
        axis: Axis,
        pane: Node,
    ) -> Result<Option<String>, Error> {
        self.pane(target)
            .ok_or_else(|| Error::NoPane(target.to_owned()))?;
        self.free(&pane.id)?;
        let container = self.fresh_id(Prefix::Split);
        let tab = self.tab_of_mut(target).expect("the pane is in a tab");
        Ok(match tab.root.split(target, axis, pane, container) {
            Split::Wrapped(id) => Some(id),
            Split::Inserted | Split::Missing => None,
        })
    }

    pub fn close_pane(&mut self, pane: &str) -> Result<Closed, Error> {
        let found = self
            .pane(pane)
            .ok_or_else(|| Error::NoPane(pane.to_owned()))?;
        if !found.node.closable() {
            return Err(Error::NotClosable(pane.to_owned()));
        }
        self.remove_pane(pane)
    }

    pub fn remove_pane(&mut self, pane: &str) -> Result<Closed, Error> {
        self.pane(pane)
            .ok_or_else(|| Error::NoPane(pane.to_owned()))?;
        let tab = self.tab_of_mut(pane).expect("the pane is in a tab");
        let removed = tab.root.remove(pane).expect("the tab holds the pane");
        let mut closed = Closed {
            panes: vec![pane.to_owned()],
            terminals: removed.addresses(),
            ..Closed::default()
        };
        if tab.root.panes().is_empty() {
            let id = tab.id.clone();
            let rest = self.close_tab(&id)?;
            closed.tabs = rest.tabs;
            closed.space = rest.space;
            closed.panes.extend(rest.panes);
            closed.terminals.extend(rest.terminals);
        }
        Ok(closed)
    }

    pub fn swap_panes(&mut self, a: &str, b: &str) -> Result<(), Error> {
        self.pane(a).ok_or_else(|| Error::NoPane(a.to_owned()))?;
        self.pane(b).ok_or_else(|| Error::NoPane(b.to_owned()))?;
        if a == b {
            return Err(Error::SamePane(a.to_owned()));
        }
        let tab = self.tab_of_mut(a).expect("the pane is in a tab");
        if tab.root.find(b).is_none() {
            return Err(Error::OtherTab(b.to_owned()));
        }
        tab.root.swap(a, b);
        Ok(())
    }

    pub fn set_pane_title(&mut self, pane: &str, title: &str) -> Result<(), Error> {
        self.pane(pane)
            .ok_or_else(|| Error::NoPane(pane.to_owned()))?;
        let tab = self.tab_of_mut(pane).expect("the pane is in a tab");
        let node = tab.root.find_mut(pane).expect("the tab holds the pane");
        let title = title.trim();
        node.title = (!title.is_empty()).then(|| title.to_owned());
        Ok(())
    }

    pub fn adopt(
        &mut self,
        address: &str,
        place: &Place,
        folder: &str,
        shown_tab: Option<&str>,
    ) -> Result<Adopted, Error> {
        if let Some(found) = self.pane_of_terminal(address) {
            return Ok(Adopted::Present {
                pane: found.node.id.clone(),
            });
        }
        let pane = self.fresh_id(Prefix::Pane);
        let block = Node::terminal(pane.clone(), address);
        let labeled = place
            .tab
            .clone()
            .unwrap_or_else(|| self.fresh_id(Prefix::Tab));
        if self.space(&place.space).is_none() {
            let tab = if self.contains_id(&labeled) {
                self.fresh_id(Prefix::Tab)
            } else {
                labeled
            };
            self.new_space(&place.space, "", folder, Tab::new(tab.clone(), "", block))?;
            return Ok(Adopted::NewSpace {
                space: place.space.clone(),
                tab,
                pane,
            });
        }
        if let Some(tab) = self.tab(&labeled) {
            let id = tab.id.clone();
            if let Some(last) = tab.panes().last().map(|p| p.id.clone()) {
                let container = self.split_pane(&last, Axis::Row, block)?;
                return Ok(Adopted::Split {
                    tab: id,
                    pane,
                    container,
                });
            }
            if let Some(children) = self.tab_mut(&id).and_then(|t| t.root.children_mut()) {
                children.push(block);
                return Ok(Adopted::Split {
                    tab: id,
                    pane,
                    container: None,
                });
            }
        }
        let tab = if self.contains_id(&labeled) {
            self.fresh_id(Prefix::Tab)
        } else {
            labeled
        };
        self.new_tab(&place.space, Tab::new(tab.clone(), "", block), shown_tab)?;
        Ok(Adopted::NewTab { tab, pane })
    }

    pub fn remove_gone(&mut self, gone: impl Fn(&str) -> bool) -> Vec<Closed> {
        let doomed: Vec<(String, String)> = self
            .tabs()
            .flat_map(Tab::terminals)
            .filter_map(|n| {
                let address = n.address()?;
                gone(address).then(|| (n.id.clone(), address.to_owned()))
            })
            .collect();
        doomed
            .into_iter()
            .filter_map(|(pane, address)| {
                let holds = self
                    .pane(&pane)
                    .is_some_and(|p| p.node.address() == Some(address.as_str()));
                holds.then(|| self.remove_pane(&pane).ok()).flatten()
            })
            .collect()
    }

    fn free(&self, id: &str) -> Result<(), Error> {
        if self.contains_id(id) {
            Err(Error::IdTaken(id.to_owned()))
        } else {
            Ok(())
        }
    }

    fn tab_ids_free(&self, tab: &Tab) -> Result<(), Error> {
        self.free(&tab.id)?;
        tab.root.walk().try_for_each(|n| self.free(&n.id))
    }
}

fn trimmed(mut tab: Tab) -> Tab {
    tab.name = tab.name.trim().to_owned();
    tab
}
