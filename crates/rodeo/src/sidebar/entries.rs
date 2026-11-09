use std::collections::HashSet;
use std::hash::BuildHasher;

use crate::agents::{self, State};
use crate::model::SpaceRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub index: usize,
    pub indented: bool,
    pub last_child: bool,
}

fn key(space: &SpaceRow) -> Option<&str> {
    space.worktree.as_ref().map(|w| w.key.as_str())
}

fn linked(space: &SpaceRow) -> bool {
    space.worktree.as_ref().is_some_and(|w| w.linked)
}

fn members<'a>(spaces: &'a [&SpaceRow], group: &'a str) -> impl Iterator<Item = usize> + 'a {
    spaces
        .iter()
        .enumerate()
        .filter(move |(_, s)| key(s) == Some(group))
        .map(|(i, _)| i)
}

fn is_group(spaces: &[&SpaceRow], group: &str) -> bool {
    let mut children = false;
    let mut parents = false;
    for i in members(spaces, group) {
        if linked(spaces[i]) {
            children = true;
        } else {
            parents = true;
        }
    }
    children && parents
}

pub fn entries<S: BuildHasher>(spaces: &[&SpaceRow], collapsed: &HashSet<String, S>) -> Vec<Entry> {
    let mut done: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for (index, space) in spaces.iter().enumerate() {
        let Some(group) = key(space).filter(|g| is_group(spaces, g)) else {
            out.push(Entry {
                index,
                indented: false,
                last_child: false,
            });
            continue;
        };
        if !done.insert(group) {
            continue;
        }
        for parent in members(spaces, group).filter(|&i| !linked(spaces[i])) {
            out.push(Entry {
                index: parent,
                indented: false,
                last_child: false,
            });
        }
        let children: Vec<usize> = members(spaces, group)
            .filter(|&i| linked(spaces[i]))
            .collect();
        if collapsed.contains(group) {
            if let Some(&focused) = children.iter().find(|&&i| spaces[i].focused) {
                out.push(Entry {
                    index: focused,
                    indented: true,
                    last_child: true,
                });
            }
            continue;
        }
        for (n, &child) in children.iter().enumerate() {
            out.push(Entry {
                index: child,
                indented: true,
                last_child: n + 1 == children.len(),
            });
        }
    }
    out
}

pub fn parent_key<'a>(spaces: &[&'a SpaceRow], index: usize) -> Option<&'a str> {
    let space = spaces.get(index)?;
    if linked(space) {
        return None;
    }
    let group = key(space)?;
    spaces
        .iter()
        .any(|s| key(s) == Some(group) && linked(s))
        .then_some(group)
}

pub fn first_parent(spaces: &[&SpaceRow], group: &str) -> Option<usize> {
    members(spaces, group).find(|&i| !linked(spaces[i]))
}

pub fn state<S: BuildHasher>(
    spaces: &[&SpaceRow],
    index: usize,
    collapsed: &HashSet<String, S>,
) -> State {
    let space = spaces[index];
    let own = space.state.unwrap_or(State::Unknown);
    let Some(group) = key(space).filter(|g| !linked(space) && collapsed.contains(*g)) else {
        return own;
    };
    agents::rollup(
        members(spaces, group)
            .filter(|&i| i == index || linked(spaces[i]))
            .map(|i| spaces[i].state.unwrap_or(State::Unknown)),
    )
    .unwrap_or(own)
}

pub fn label(space: &SpaceRow, indented: bool) -> &str {
    if indented
        && !space.custom
        && let Some(branch) = &space.branch
    {
        return branch.strip_prefix("worktree/").unwrap_or(branch);
    }
    &space.name
}

pub fn block(spaces: &[&SpaceRow], index: usize) -> Vec<String> {
    let space = spaces[index];
    let mut ids = vec![space.key.clone()];
    if let Some(group) = key(space) {
        ids.extend(
            members(spaces, group)
                .filter(|&i| i != index)
                .map(|i| spaces[i].key.clone()),
        );
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Worktree;

    fn space(key: &str, group: Option<(&str, bool)>, state: Option<State>) -> SpaceRow {
        SpaceRow {
            key: key.into(),
            name: key.into(),
            worktree: group.map(|(k, linked)| Worktree {
                key: k.into(),
                linked,
            }),
            state,
            ..SpaceRow::default()
        }
    }

    fn shape(entries: &[Entry]) -> Vec<(usize, bool, bool)> {
        entries
            .iter()
            .map(|e| (e.index, e.indented, e.last_child))
            .collect()
    }

    #[test]
    fn groups_gather_at_their_first_member() {
        let rows = [
            space("child-one", Some(("r", true)), None),
            space("plain", None, None),
            space("parent", Some(("r", false)), None),
            space("child-two", Some(("r", true)), None),
            space("solo", Some(("s", false)), None),
        ];
        let spaces: Vec<&SpaceRow> = rows.iter().collect();
        let open = entries(&spaces, &HashSet::new());
        assert_eq!(
            shape(&open),
            [
                (2, false, false),
                (0, true, false),
                (3, true, true),
                (1, false, false),
                (4, false, false),
            ]
        );
        assert_eq!(parent_key(&spaces, 2), Some("r"));
        assert_eq!(parent_key(&spaces, 4), None);
        assert_eq!(parent_key(&spaces, 0), None);
        assert_eq!(block(&spaces, 2), ["parent", "child-one", "child-two"]);
    }

    #[test]
    fn collapsed_groups_keep_the_focused_child_and_roll_up_states() {
        let mut rows = [
            space("parent", Some(("r", false)), Some(State::Idle)),
            space("a", Some(("r", true)), Some(State::Blocked)),
            space("b", Some(("r", true)), Some(State::Working)),
        ];
        let collapsed: HashSet<String> = ["r".to_owned()].into();
        let spaces: Vec<&SpaceRow> = rows.iter().collect();
        assert_eq!(shape(&entries(&spaces, &collapsed)), [(0, false, false)]);
        assert_eq!(state(&spaces, 0, &collapsed), State::Blocked);
        assert_eq!(state(&spaces, 0, &HashSet::new()), State::Idle);
        rows[2].focused = true;
        let spaces: Vec<&SpaceRow> = rows.iter().collect();
        assert_eq!(
            shape(&entries(&spaces, &collapsed)),
            [(0, false, false), (2, true, true)]
        );
    }

    #[test]
    fn children_show_their_branch() {
        let mut row = space("x", Some(("r", true)), None);
        row.branch = Some("worktree/feat".into());
        assert_eq!(label(&row, true), "feat");
        assert_eq!(label(&row, false), "x");
        row.custom = true;
        assert_eq!(label(&row, true), "x");
    }
}
