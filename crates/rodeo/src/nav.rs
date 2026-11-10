use std::collections::HashSet;

use crate::app::App;
use crate::model::SpaceRow;
use crate::sidebar::{entries, parent_key};
use crate::view::{Mode, Nav};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavHint {
    Move,
    Fold,
    Host,
}

impl App {
    pub fn nav_hint(&self) -> Option<NavHint> {
        if !self.machine_view() {
            return None;
        }
        Some(match self.nav_current() {
            Some(Nav::Space(key)) if self.group_parent(&key).is_none() => NavHint::Host,
            Some(_) => NavHint::Fold,
            None => NavHint::Move,
        })
    }

    pub fn machine_view(&self) -> bool {
        if self.regions.mobile || self.regions.sidebar.is_empty() {
            return false;
        }
        let snapshot = self.snapshot();
        snapshot.machines.len() > 1 || !snapshot.online
    }

    pub fn nav_order(&self) -> Vec<Nav> {
        if !self.machine_view() {
            return self.space_order().into_iter().map(Nav::Space).collect();
        }
        let snapshot = self.snapshot();
        let groups: HashSet<String> = if snapshot.collapsed {
            HashSet::new()
        } else {
            self.prefs.collapsed_groups.iter().cloned().collect()
        };
        let mut order = Vec::new();
        for machine in &snapshot.machines {
            order.push(Nav::Machine(machine.name.clone()));
            if self.view.sidebar.collapsed_machines.contains(&machine.name) {
                continue;
            }
            let spaces: Vec<&SpaceRow> = snapshot
                .spaces
                .iter()
                .filter(|s| s.machine == machine.name)
                .collect();
            if snapshot.collapsed {
                order.extend(spaces.iter().map(|s| Nav::Space(s.key.clone())));
            } else {
                order.extend(
                    entries(&spaces, &groups)
                        .into_iter()
                        .map(|e| Nav::Space(spaces[e.index].key.clone())),
                );
            }
        }
        order
    }

    pub fn nav_start(&self) -> Option<Nav> {
        if self.is_picked() {
            Some(Nav::Machine(self.target_machine()))
        } else {
            self.view.space.clone().map(Nav::Space)
        }
    }

    pub fn enter_navigate(&mut self) {
        self.view.nav = self.nav_start();
        self.view.mode = Mode::Navigate;
    }

    fn nav_current(&self) -> Option<Nav> {
        self.view
            .nav
            .clone()
            .or_else(|| self.view.space.clone().map(Nav::Space))
    }

    fn host_row(&self, space: &str) -> Option<Nav> {
        let found = self.machines.space(space)?;
        Some(Nav::Machine(found.machine.map(str::to_owned)))
    }

    pub fn step_nav(&mut self, forward: bool) {
        let order = self.nav_order();
        if order.is_empty() {
            return;
        }
        let at = self.nav_current().and_then(|current| {
            order.iter().position(|n| *n == current).or_else(|| {
                let host = self.host_row(current.space()?)?;
                order.iter().position(|n| *n == host)
            })
        });
        let last = order.len() - 1;
        let wrap = !self.regions.mobile;
        let next = match (at, forward) {
            (None, _) => 0,
            (Some(i), true) if i < last => i + 1,
            (Some(i), true) => {
                if wrap {
                    0
                } else {
                    i
                }
            }
            (Some(0), false) => {
                if wrap {
                    last
                } else {
                    0
                }
            }
            (Some(i), false) => i - 1,
        };
        self.nav_select(order[next].clone());
    }

    fn nav_select(&mut self, nav: Nav) {
        if let Nav::Machine(name) = &nav {
            self.view.sidebar.picked = Some((name.clone(), self.view.space.clone()));
        }
        self.view.nav = Some(nav);
        self.view.reveal_space = true;
    }

    pub fn nav_open(&mut self) {
        let target = match self.view.nav.clone() {
            Some(Nav::Space(key)) => Some(key),
            Some(Nav::Machine(name)) => match self.machine_space(name.as_deref()) {
                Some(key) => Some(key),
                None => return,
            },
            None => None,
        };
        self.view.nav = None;
        self.leave_mode();
        if let Some(key) = target {
            self.focus_space(&key);
        }
    }

    fn group_parent(&self, space: &str) -> Option<String> {
        let snapshot = self.snapshot();
        if snapshot.collapsed {
            return None;
        }
        let row = snapshot.space(space)?;
        let spaces: Vec<&SpaceRow> = snapshot
            .spaces
            .iter()
            .filter(|s| s.machine == row.machine)
            .collect();
        let index = spaces.iter().position(|s| s.key == space)?;
        parent_key(&spaces, index).map(str::to_owned)
    }

    pub fn nav_arrow(&mut self, right: bool) -> bool {
        if !self.machine_view() {
            return false;
        }
        match self.nav_current() {
            Some(Nav::Machine(name)) => {
                let folded = &mut self.view.sidebar.collapsed_machines;
                if right {
                    folded.remove(&name);
                } else {
                    folded.insert(name);
                }
                true
            }
            Some(Nav::Space(key)) => {
                if let Some(group) = self.group_parent(&key) {
                    let folded = self.prefs.collapsed_groups.contains(&group);
                    if folded == right {
                        self.toggle_group(&group);
                    }
                    return true;
                }
                if right {
                    return false;
                }
                match self.host_row(&key) {
                    Some(host) => {
                        self.nav_select(host);
                        true
                    }
                    None => false,
                }
            }
            None => false,
        }
    }
}
