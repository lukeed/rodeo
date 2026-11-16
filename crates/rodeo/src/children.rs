//! Terminals an agent started with `hc run` (hc's `parent`), listed under the pane they came from.

use std::collections::{HashMap, HashSet};

use crate::agents::{self, State};
use crate::model::ChildRow;

const DEPTH: usize = 32;

/// A parent named the way the viewing core names terminals. hc can leave a core
/// id or host name in front; the desktop app maps them the same way (`src/hc.rs`).
pub fn parent_address(
    parent: &str,
    core: Option<&hc::CoreInfo>,
    machines: &[hc::MachineView],
) -> String {
    let Some((prefix, id)) = parent.split_once('/') else {
        return parent.to_owned();
    };
    if core.is_some_and(|c| c.id == prefix || c.name == prefix) {
        return id.to_owned();
    }
    let found = machines
        .iter()
        .find(|m| m.name == prefix || m.id == prefix)
        .or_else(|| {
            let mut hosts = machines.iter().filter(|m| m.host == prefix);
            match (hosts.next(), hosts.next()) {
                (Some(machine), None) => Some(machine),
                _ => None,
            }
        });
    found.map_or_else(|| parent.to_owned(), |m| format!("{}/{id}", m.name))
}

/// A child's state as the app's agents list shows it: `error` is blocked, and
/// `done` is idle, because the parent's done is the news.
pub fn state(terminal: &hc::Terminal) -> State {
    match terminal.status.as_ref().map(|s| s.state.as_str()) {
        None => State::Unknown,
        Some("working") => State::Working,
        Some("blocked" | "error") => State::Blocked,
        Some(_) => State::Idle,
    }
}

pub fn name(terminal: &hc::Terminal) -> String {
    terminal
        .name
        .clone()
        .filter(|n| !n.is_empty())
        .or_else(|| terminal.template.clone().filter(|t| !t.is_empty()))
        .or_else(|| agents::label(terminal).filter(|l| !l.is_empty()))
        .unwrap_or_else(|| crate::ids::split(&terminal.address).1.to_owned())
}

fn title(terminal: &hc::Terminal) -> String {
    let status = terminal
        .status
        .as_ref()
        .map(|s| s.title.as_str())
        .filter(|t| !t.is_empty());
    agents::strip_activity(status.unwrap_or(&terminal.title)).to_owned()
}

/// Live terminals without a pane, depth first under each root (a pane's terminal),
/// each with the tree lines in front of it.
pub fn rows<'a>(
    roots: impl IntoIterator<Item = &'a str>,
    terminals: impl IntoIterator<Item = &'a hc::Terminal>,
    parent_of: impl Fn(&hc::Terminal) -> Option<String>,
) -> Vec<ChildRow> {
    let roots: Vec<&str> = roots.into_iter().collect();
    let in_pane: HashSet<&str> = roots.iter().copied().collect();
    let mut kids: HashMap<String, Vec<&hc::Terminal>> = HashMap::new();
    for terminal in terminals {
        if !terminal.is_live()
            || in_pane.contains(terminal.address.as_str())
            || terminal.label(crate::popup::LABEL).is_some()
        {
            continue;
        }
        if let Some(parent) = parent_of(terminal) {
            kids.entry(parent).or_default().push(terminal);
        }
    }
    for list in kids.values_mut() {
        list.sort_by(|a, b| (a.created_ms, &a.address).cmp(&(b.created_ms, &b.address)));
    }
    let mut out = Vec::new();
    for root in roots {
        let mut seen = HashSet::from([root.to_owned()]);
        walk(root, root, "", &kids, &mut seen, &mut out, 0);
    }
    out
}

fn walk(
    root: &str,
    parent: &str,
    lead: &str,
    kids: &HashMap<String, Vec<&hc::Terminal>>,
    seen: &mut HashSet<String>,
    out: &mut Vec<ChildRow>,
    depth: usize,
) {
    let Some(siblings) = kids.get(parent).filter(|_| depth < DEPTH) else {
        return;
    };
    for (i, terminal) in siblings.iter().enumerate() {
        if !seen.insert(terminal.address.clone()) {
            continue;
        }
        let last = i + 1 == siblings.len();
        out.push(ChildRow {
            address: terminal.address.clone(),
            root: root.to_owned(),
            prefix: format!("{lead}{}", if last { "└─ " } else { "├─ " }),
            name: name(terminal),
            title: title(terminal),
            state: state(terminal),
        });
        let next = format!("{lead}{}", if last { "   " } else { "│  " });
        walk(root, &terminal.address, &next, kids, seen, out, depth + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal(address: &str, parent: Option<&str>, status: Option<&str>) -> hc::Terminal {
        hc::Terminal {
            address: address.into(),
            state: "live".into(),
            parent: parent.map(str::to_owned),
            status: status.map(|state| hc::Status {
                state: state.into(),
                ..hc::Status::default()
            }),
            ..hc::Terminal::default()
        }
    }

    fn machine(name: &str, id: &str, host: &str) -> hc::MachineView {
        hc::MachineView {
            name: name.into(),
            id: id.into(),
            host: host.into(),
            ..hc::MachineView::default()
        }
    }

    #[test]
    fn parents_use_the_viewing_cores_names() {
        let core = hc::CoreInfo {
            id: "local-id".into(),
            name: "host-local".into(),
        };
        let machines = [
            machine("alpha", "alpha-id", "host-alpha"),
            machine("gamma", "gamma-id", "shared-host"),
            machine("delta", "delta-id", "shared-host"),
        ];
        let parent = |p: &str| parent_address(p, Some(&core), &machines);
        assert_eq!(parent("host-local/t1"), "t1");
        assert_eq!(parent("local-id/t1"), "t1");
        assert_eq!(parent("host-alpha/t4"), "alpha/t4");
        assert_eq!(parent("alpha-id/t4"), "alpha/t4");
        assert_eq!(parent("alpha/t4"), "alpha/t4");
        assert_eq!(parent("t7"), "t7");
        assert_eq!(parent("shared-host/t4"), "shared-host/t4");
        assert_eq!(parent("unknown/t1"), "unknown/t1");
    }

    #[test]
    fn children_nest_depth_first_under_their_pane() {
        let mut gone = terminal("t9", Some("t1"), None);
        gone.state = "finished".into();
        let mut popup = terminal("t8", Some("t1"), None);
        popup.labels.insert(crate::popup::LABEL.into(), "1".into());
        let terminals = [
            terminal("t1", None, Some("working")),
            terminal("t2", Some("t1"), None),
            terminal("t3", Some("t2"), Some("done")),
            terminal("t4", Some("t1"), Some("error")),
            terminal("t5", Some("t1"), None),
            gone,
            popup,
        ];
        let rows = rows(["t1", "t5"], &terminals, |t| t.parent.clone());
        let shown: Vec<(&str, &str, State)> = rows
            .iter()
            .map(|r| (r.address.as_str(), r.prefix.as_str(), r.state))
            .collect();
        assert_eq!(
            shown,
            [
                ("t2", "├─ ", State::Unknown),
                ("t3", "│  └─ ", State::Idle),
                ("t4", "└─ ", State::Blocked),
            ]
        );
        assert!(rows.iter().all(|r| r.root == "t1"));
    }

    #[test]
    fn older_children_come_first_and_parent_loops_stay_out() {
        let mut older = terminal("t6", Some("t1"), None);
        older.created_ms = 1;
        let mut newer = terminal("t2", Some("t1"), None);
        newer.created_ms = 2;
        let terminals = [
            terminal("t1", None, None),
            newer,
            older,
            terminal("t3", Some("t4"), None),
            terminal("t4", Some("t3"), None),
        ];
        let rows = rows(["t1"], &terminals, |t| t.parent.clone());
        let shown: Vec<&str> = rows.iter().map(|r| r.address.as_str()).collect();
        assert_eq!(shown, ["t6", "t2"]);
    }
}
