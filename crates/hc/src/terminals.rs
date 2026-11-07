use std::collections::BTreeMap;

use crate::model::{Change, Ending, Event, Notice, Snapshot, Terminal};
use crate::stream::Filter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    Nothing,
    Changed(String),
    Removed(String),
    Reset,
    Doc {
        machine: Option<String>,
        name: String,
        version: u64,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Terminals {
    seq: u64,
    labels: BTreeMap<String, String>,
    records: BTreeMap<String, Terminal>,
}

impl Terminals {
    pub fn new(snapshot: Snapshot, filter: &Filter) -> Self {
        Self {
            seq: snapshot.seq,
            labels: filter.labels.clone(),
            records: by_address(snapshot.terminals),
        }
    }

    pub const fn seq(&self) -> u64 {
        self.seq
    }

    pub fn get(&self, address: &str) -> Option<&Terminal> {
        self.records.get(address)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Terminal> {
        self.records.values()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn apply(&mut self, notice: Notice) -> Update {
        match notice {
            Notice::Heartbeat { .. } => Update::Nothing,
            Notice::Missed(snapshot) => {
                self.seq = snapshot.seq;
                self.records = by_address(snapshot.terminals);
                Update::Reset
            }
            Notice::Event(event) if event.seq <= self.seq => Update::Nothing,
            Notice::Event(event) => {
                self.seq = event.seq;
                self.change(event)
            }
        }
    }

    fn change(&mut self, event: Event) -> Update {
        let Event {
            terminal,
            machine,
            change,
            ..
        } = event;
        match (change, terminal) {
            (Change::Doc { name, version }, _) => Update::Doc {
                machine,
                name,
                version,
            },
            (Change::Unknown { .. }, _) | (_, None) => Update::Nothing,
            (Change::Created(record) | Change::Restarted(record), Some(address)) => {
                self.records.insert(address.clone(), *record);
                Update::Changed(address)
            }
            (Change::Removed {}, Some(address)) => self.remove(address),
            (Change::Labels { labels, record }, Some(address)) => {
                self.relabel(address, labels, record.map(|record| *record))
            }
            (change, Some(address)) => match self.records.get_mut(&address) {
                Some(terminal) => {
                    edit(terminal, change);
                    Update::Changed(address)
                }
                None => Update::Nothing,
            },
        }
    }

    fn remove(&mut self, address: String) -> Update {
        if self.records.remove(&address).is_some() {
            Update::Removed(address)
        } else {
            Update::Nothing
        }
    }

    fn relabel(
        &mut self,
        address: String,
        labels: BTreeMap<String, String>,
        record: Option<Terminal>,
    ) -> Update {
        let matches = self
            .labels
            .iter()
            .all(|(key, value)| labels.get(key) == Some(value));
        if !matches {
            return self.remove(address);
        }
        if let Some(record) = record {
            self.records.insert(address.clone(), record);
        }
        let Some(terminal) = self.records.get_mut(&address) else {
            return Update::Nothing;
        };
        terminal.labels = labels;
        Update::Changed(address)
    }
}

fn by_address(terminals: Vec<Terminal>) -> BTreeMap<String, Terminal> {
    terminals
        .into_iter()
        .map(|terminal| (terminal.address.clone(), terminal))
        .collect()
}

fn edit(terminal: &mut Terminal, change: Change) {
    match change {
        Change::Lost(ending) => {
            end(terminal, ending);
            terminal.pid = None;
        }
        Change::Finished(ending) => {
            end(terminal, ending);
            "finished".clone_into(&mut terminal.state);
        }
        Change::Keep { keep } => terminal.keep = keep,
        Change::Status {
            status,
            records,
            reported,
        } => {
            if terminal.status.as_ref().map(|s| &s.state) != status.as_ref().map(|s| &s.state) {
                terminal.status_ms = Some(now_ms());
            }
            terminal.status = status;
            terminal.records = records;
            terminal.reported = reported;
        }
        Change::Turn { turn, .. } => terminal.turn = turn,
        Change::Title { title } => terminal.title = title,
        Change::Size { cols, rows } => (terminal.cols, terminal.rows) = (cols, rows),
        Change::Foreground(foreground) => terminal.foreground = foreground,
        Change::Created(_)
        | Change::Restarted(_)
        | Change::Removed {}
        | Change::Labels { .. }
        | Change::Doc { .. }
        | Change::Unknown { .. } => {}
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

fn end(terminal: &mut Terminal, ending: Ending) {
    terminal.reason = ending.reason;
    terminal.exit_code = ending.exit_code;
    terminal.signal = ending.signal;
    terminal.finished_ms = ending.finished_ms;
    terminal.foreground = None;
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn record(address: &str, labels: Value) -> Value {
        let mut record = json!({
            "address": address, "state": "live", "argv": ["sh"], "cwd": "/",
            "cols": 80, "rows": 24, "created_ms": 1, "pid": 9,
            "foreground": {"pid": 9, "name": "sh", "argv": ["sh"], "cwd": "/"},
        });
        record["labels"] = labels;
        record
    }

    fn event(seq: u64, terminal: &str, kind: &str, data: Value) -> Notice {
        let mut event = json!({"seq": seq, "terminal": terminal, "type": kind});
        event["data"] = data;
        Notice::Event(serde_json::from_value(event).expect("event"))
    }

    fn start(terminals: &[Value], labels: &[(&str, &str)]) -> Terminals {
        let snapshot: Snapshot =
            serde_json::from_value(json!({"seq": 10, "terminals": terminals})).expect("snapshot");
        let filter = Filter {
            labels: labels
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            ..Filter::default()
        };
        Terminals::new(snapshot, &filter)
    }

    #[test]
    fn applies_changes_to_the_snapshot() {
        let mut view = start(&[record("t1", json!({}))], &[]);
        let status =
            json!({"status": {"state": "working", "progress": 5}, "records": {}, "reported": true});
        assert_eq!(
            view.apply(event(11, "t1", "status", status)),
            Update::Changed("t1".into())
        );
        view.apply(event(12, "t1", "turn", json!({"turn": 3, "state": "done"})));
        view.apply(event(13, "t1", "title", json!({"title": "fix it"})));
        view.apply(event(14, "t1", "size", json!({"cols": 50, "rows": 20})));
        view.apply(event(15, "t1", "keep", json!({"keep": true})));
        view.apply(event(
            16,
            "t1",
            "foreground",
            json!({"pid": 10, "name": "vim", "argv": ["vim"], "cwd": "/src"}),
        ));
        let t = view.get("t1").expect("t1");
        assert_eq!(t.status.as_ref().map(|s| s.progress), Some(Some(5)));
        assert!(t.status_ms.is_some_and(|ms| ms > 1_700_000_000_000));
        assert_eq!(
            (t.turn, t.title.as_str(), t.cols, t.rows, t.keep),
            (3, "fix it", 50, 20, true)
        );
        assert_eq!(t.foreground.as_ref().map(|f| f.cwd.as_str()), Some("/src"));
        assert_eq!(view.seq(), 16);
    }

    #[test]
    fn drops_events_it_already_has() {
        let mut view = start(&[record("t1", json!({}))], &[]);
        assert_eq!(
            view.apply(event(10, "t1", "title", json!({"title": "old"}))),
            Update::Nothing
        );
        assert_eq!(view.get("t1").expect("t1").title, "");
        assert_eq!(view.apply(Notice::Heartbeat { seq: 99 }), Update::Nothing);
        assert_eq!(view.seq(), 10);
    }

    #[test]
    fn events_for_unknown_terminals_change_nothing() {
        let mut view = start(&[], &[]);
        assert_eq!(
            view.apply(event(11, "t5", "title", json!({"title": "x"}))),
            Update::Nothing
        );
        assert_eq!(
            view.apply(event(12, "t5", "someday", json!({}))),
            Update::Nothing
        );
        assert!(view.is_empty());
    }

    #[test]
    fn a_lost_terminal_stays_live_and_a_finished_one_ends() {
        let mut view = start(&[record("t1", json!({})), record("t2", json!({}))], &[]);
        view.apply(event(
            11,
            "t1",
            "lost",
            json!({"reason": "signaled", "signal": 9, "exit_code": null, "finished_ms": null}),
        ));
        view.apply(event(
            12,
            "t2",
            "finished",
            json!({"reason": "exited", "exit_code": 0, "signal": null, "finished_ms": 7}),
        ));
        let lost = view.get("t1").expect("t1");
        assert_eq!(
            (lost.state.as_str(), lost.pid, lost.signal),
            ("live", None, Some(9))
        );
        assert!(lost.is_lost() && lost.foreground.is_none());
        let done = view.get("t2").expect("t2");
        assert_eq!(
            (done.state.as_str(), done.exit_code, done.finished_ms),
            ("finished", Some(0), Some(7))
        );
        assert!(done.foreground.is_none());
    }

    #[test]
    fn created_restarted_and_removed_replace_records() {
        let mut view = start(&[], &[]);
        view.apply(event(11, "t4", "created", record("t4", json!({}))));
        view.apply(event(
            12,
            "t4",
            "restarted",
            record("t4", json!({"new": "1"})),
        ));
        assert_eq!(view.get("t4").expect("t4").labels["new"], "1");
        assert_eq!(
            view.apply(event(13, "t4", "removed", json!({}))),
            Update::Removed("t4".into())
        );
        assert!(view.is_empty());
        assert_eq!(
            view.apply(event(14, "t4", "removed", json!({}))),
            Update::Nothing
        );
    }

    #[test]
    fn labels_follow_the_filter() {
        let mut view = start(&[record("t1", json!({"proof": "x"}))], &[("proof", "x")]);
        assert_eq!(
            view.apply(event(11, "t1", "labels", json!({"labels": {"proof": "y"}}))),
            Update::Removed("t1".into())
        );
        let joined =
            json!({"labels": {"proof": "x"}, "record": record("t2", json!({"proof": "x"}))});
        assert_eq!(
            view.apply(event(12, "t2", "labels", joined)),
            Update::Changed("t2".into())
        );
        assert_eq!(view.len(), 1);
    }

    #[test]
    fn missed_replaces_everything_and_later_events_count_from_it() {
        let mut view = start(&[record("t1", json!({}))], &[]);
        let fresh: Snapshot =
            serde_json::from_value(json!({"seq": 40, "terminals": [record("t9", json!({}))]}))
                .expect("snapshot");
        assert_eq!(view.apply(Notice::Missed(fresh)), Update::Reset);
        assert!(view.get("t1").is_none() && view.get("t9").is_some());
        assert_eq!(
            view.apply(event(39, "t9", "title", json!({"title": "stale"}))),
            Update::Nothing
        );
        assert_eq!(
            view.apply(event(41, "t9", "title", json!({"title": "new"}))),
            Update::Changed("t9".into())
        );
    }

    #[test]
    fn doc_events_name_their_machine() {
        let mut view = start(&[], &[]);
        let doc = Notice::Event(
            serde_json::from_value(json!({
                "seq": 11, "type": "doc", "machine": "rohan", "data": {"name": "layout", "version": 3}
            }))
            .expect("doc"),
        );
        assert_eq!(
            view.apply(doc),
            Update::Doc {
                machine: Some("rohan".into()),
                name: "layout".into(),
                version: 3
            }
        );
    }
}
