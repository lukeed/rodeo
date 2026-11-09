use crate::app::App;
use crate::model::{Snapshot, SpaceRow};
use crate::overlay::CloseTarget;
use crate::overlay::context::SpaceFlags;

fn group_of(row: &SpaceRow) -> Option<(&str, bool)> {
    row.worktree.as_ref().map(|w| (w.key.as_str(), w.linked))
}

pub fn has_children(snapshot: &Snapshot, row: &SpaceRow) -> bool {
    let Some((key, false)) = group_of(row) else {
        return false;
    };
    snapshot
        .spaces
        .iter()
        .any(|s| group_of(s) == Some((key, true)))
}

pub fn close_group(snapshot: &Snapshot, space: &str) -> Option<Vec<String>> {
    let row = snapshot.space(space)?;
    let (key, false) = group_of(row)? else {
        return None;
    };
    let mut members = vec![row.key.clone()];
    for other in snapshot.spaces.iter().filter(|s| s.key != row.key) {
        match group_of(other) {
            Some((k, false)) if k == key => return None,
            Some((k, true)) if k == key => members.push(other.key.clone()),
            _ => {}
        }
    }
    (members.len() > 1).then_some(members)
}

pub fn space_flags(snapshot: &Snapshot, space: &str, collapsed: &[String]) -> Option<SpaceFlags> {
    let row = snapshot.space(space)?;
    let group = group_of(row);
    Some(SpaceFlags {
        git: group.is_some() || row.branch.is_some(),
        linked: group.is_some_and(|(_, linked)| linked),
        children: has_children(snapshot, row),
        close_group: close_group(snapshot, space).is_some(),
        collapsed: group.is_some_and(|(key, _)| collapsed.iter().any(|c| c == key)),
    })
}

impl App {
    pub fn close_space_target(&self, space: String) -> CloseTarget {
        if close_group(&self.snapshot(), &space).is_some() {
            CloseTarget::Group { space }
        } else {
            CloseTarget::Workspace { space }
        }
    }

    pub fn group_members(&self, space: &str) -> Vec<String> {
        close_group(&self.snapshot(), space).unwrap_or_else(|| vec![space.to_owned()])
    }

    pub fn group_key(&self, space: &str) -> Option<String> {
        self.snapshot()
            .space(space)?
            .worktree
            .as_ref()
            .map(|w| w.key.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Worktree;

    fn row(key: &str, group: Option<(&str, bool)>) -> SpaceRow {
        SpaceRow {
            key: key.to_owned(),
            worktree: group.map(|(k, linked)| Worktree {
                key: k.to_owned(),
                linked,
            }),
            ..SpaceRow::default()
        }
    }

    fn snapshot(spaces: Vec<SpaceRow>) -> Snapshot {
        Snapshot {
            spaces,
            ..Snapshot::default()
        }
    }

    #[test]
    fn a_parent_with_linked_children_closes_as_a_group() {
        let s = snapshot(vec![
            row("p", Some(("/r/.git", false))),
            row("c1", Some(("/r/.git", true))),
            row("x", None),
            row("c2", Some(("/r/.git", true))),
        ]);
        assert_eq!(
            close_group(&s, "p"),
            Some(vec!["p".into(), "c1".into(), "c2".into()])
        );
        assert_eq!(close_group(&s, "c1"), None);
        assert_eq!(close_group(&s, "x"), None);
        let flags = space_flags(&s, "p", &["/r/.git".to_owned()]).expect("flags");
        assert!(flags.git && flags.children && flags.close_group && flags.collapsed);
        let child = space_flags(&s, "c1", &[]).expect("flags");
        assert!(child.linked && !child.children && !child.collapsed);
    }

    #[test]
    fn two_parents_close_one_at_a_time() {
        let s = snapshot(vec![
            row("p1", Some(("/r/.git", false))),
            row("p2", Some(("/r/.git", false))),
            row("c", Some(("/r/.git", true))),
        ]);
        assert_eq!(close_group(&s, "p1"), None);
        let flags = space_flags(&s, "p1", &[]).expect("flags");
        assert!(flags.children && !flags.close_group);
    }

    #[test]
    fn a_lone_repo_is_git_without_children() {
        let s = snapshot(vec![row("p", Some(("/r/.git", false)))]);
        let flags = space_flags(&s, "p", &[]).expect("flags");
        assert!(flags.git && !flags.children && !flags.close_group);
        assert!(close_group(&s, "p").is_none());
    }
}
