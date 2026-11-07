use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub generation: u32,
    pub version: String,
    pub core: CoreInfo,
    pub framed: bool,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Hello {
    pub fn has_method(&self, method: &str) -> bool {
        self.methods.iter().any(|m| m == method)
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StatusForm")]
pub struct Status {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub app: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub msg: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StatusForm {
    Word(String),
    Record {
        state: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        progress: Option<u8>,
        #[serde(default)]
        app: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        msg: String,
    },
}

impl From<StatusForm> for Status {
    fn from(form: StatusForm) -> Self {
        match form {
            StatusForm::Word(state) => Self {
                state,
                ..Self::default()
            },
            StatusForm::Record {
                state,
                kind,
                progress,
                app,
                title,
                msg,
            } => Self {
                state,
                kind,
                progress,
                app,
                title,
                msg,
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foreground {
    pub pid: i32,
    pub name: String,
    #[serde(default)]
    pub argv: Vec<String>,
    pub cwd: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Terminal {
    pub address: String,
    pub name: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub argv: Vec<String>,
    pub cwd: String,
    pub keep: bool,
    pub state: String,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub reason: Option<String>,
    pub created_ms: u64,
    pub finished_ms: Option<u64>,
    pub screen: String,
    pub cols: u16,
    pub rows: u16,
    pub identity: String,
    pub status: Option<Status>,
    pub status_ms: Option<u64>,
    pub records: BTreeMap<String, Status>,
    pub turn: u64,
    pub reported: bool,
    pub title: String,
    pub parent: Option<String>,
    pub notify: bool,
    pub die_with_parent: bool,
    pub rm: bool,
    pub template: Option<String>,
    pub foreground: Option<Foreground>,
}

impl Terminal {
    pub fn is_live(&self) -> bool {
        self.state == "live"
    }

    pub fn is_finished(&self) -> bool {
        self.state == "finished"
    }

    pub fn is_lost(&self) -> bool {
        self.is_live()
            && self.pid.is_none()
            && self.reason.as_deref().is_some_and(|r| r != "spawn_pending")
    }

    pub fn machine(&self) -> Option<&str> {
        machine_of(&self.address)
    }

    pub fn label(&self, key: &str) -> Option<&str> {
        self.labels.get(key).map(String::as_str)
    }
}

pub(crate) fn machine_of(address: &str) -> Option<&str> {
    address.split_once('/').map(|(machine, _)| machine)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Display {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
}

impl Display {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ending {
    pub reason: Option<String>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub finished_ms: Option<u64>,
}

impl Ending {
    pub fn is_detach(&self) -> bool {
        self == &Self::default()
    }

    pub fn is_exit(&self) -> bool {
        matches!(self.reason.as_deref(), Some("exited" | "killed"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Change {
    Created(Box<Terminal>),
    Lost(Ending),
    Restarted(Box<Terminal>),
    Keep {
        keep: bool,
    },
    Finished(Ending),
    Removed {},
    Status {
        status: Option<Status>,
        #[serde(default)]
        records: BTreeMap<String, Status>,
        #[serde(default)]
        reported: bool,
    },
    Turn {
        turn: u64,
        #[serde(default)]
        state: Option<String>,
    },
    Title {
        title: String,
    },
    Labels {
        labels: BTreeMap<String, String>,
        #[serde(default)]
        record: Option<Box<Terminal>>,
    },
    Size {
        cols: u16,
        rows: u16,
    },
    Foreground(Option<Foreground>),
    Doc {
        name: String,
        version: u64,
    },
    #[serde(skip)]
    Unknown {
        kind: String,
        data: Value,
    },
}

const CHANGES: [&str; 13] = [
    "created",
    "lost",
    "restarted",
    "keep",
    "finished",
    "removed",
    "status",
    "turn",
    "title",
    "labels",
    "size",
    "foreground",
    "doc",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawEvent")]
pub struct Event {
    pub seq: u64,
    pub terminal: Option<String>,
    pub machine: Option<String>,
    pub change: Change,
}

#[derive(Deserialize)]
struct RawEvent {
    seq: u64,
    #[serde(default)]
    terminal: Option<String>,
    #[serde(default)]
    machine: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Value,
}

impl TryFrom<RawEvent> for Event {
    type Error = serde_json::Error;

    fn try_from(raw: RawEvent) -> Result<Self, Self::Error> {
        let change = if CHANGES.contains(&raw.kind.as_str()) {
            serde_json::from_value(json!({"type": raw.kind, "data": raw.data}))?
        } else {
            Change::Unknown {
                kind: raw.kind,
                data: raw.data,
            }
        };
        let machine = raw.machine.or_else(|| {
            raw.terminal
                .as_deref()
                .and_then(machine_of)
                .map(str::to_owned)
        });
        Ok(Self {
            seq: raw.seq,
            terminal: raw.terminal,
            machine,
            change,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Event(Event),
    Heartbeat { seq: u64 },
    Missed(Snapshot),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Snapshot {
    pub seq: u64,
    #[serde(default)]
    pub terminals: Vec<Terminal>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(value: Value) -> Event {
        serde_json::from_value(value).expect("event")
    }

    #[test]
    fn reads_every_documented_event_type() {
        let record = json!({"address": "t1", "state": "live", "argv": ["sh"], "cwd": "/", "cols": 80, "rows": 24, "created_ms": 1});
        let cases = [
            ("created", record.clone()),
            ("restarted", record.clone()),
            (
                "lost",
                json!({"reason": "signaled", "exit_code": null, "signal": 9, "finished_ms": null}),
            ),
            (
                "finished",
                json!({"reason": "exited", "exit_code": 0, "signal": null, "finished_ms": 5}),
            ),
            ("removed", json!({})),
            ("keep", json!({"keep": true})),
            (
                "status",
                json!({"status": {"state": "working"}, "records": {}, "reported": true}),
            ),
            ("turn", json!({"turn": 2, "state": "done"})),
            ("title", json!({"title": "x"})),
            ("labels", json!({"labels": {"a": "1"}, "record": record})),
            ("size", json!({"cols": 10, "rows": 5})),
            (
                "foreground",
                json!({"pid": 3, "name": "sh", "argv": ["sh"], "cwd": "/tmp"}),
            ),
            ("doc", json!({"name": "herdr-app", "version": 4})),
        ];
        for (kind, data) in cases {
            let parsed = event(json!({"seq": 1, "type": kind, "terminal": "t1", "data": data}));
            assert!(
                !matches!(parsed.change, Change::Unknown { .. }),
                "{kind} parsed as unknown"
            );
        }
    }

    #[test]
    fn reads_status_details() {
        let status = event(json!({
            "seq": 4, "type": "status", "terminal": "t7",
            "data": {"status": {"state": "blocked", "kind": "permission", "msg": "run tests"},
                     "records": {}, "reported": true}
        }));
        assert_eq!(status.terminal.as_deref(), Some("t7"));
        assert_eq!(status.machine, None);
        let Change::Status {
            status: Some(status),
            ..
        } = status.change
        else {
            panic!("status change");
        };
        assert_eq!(status.kind.as_deref(), Some("permission"));
    }

    #[test]
    fn docs_carry_name_version_and_machine() {
        let doc = event(json!({
            "seq": 5, "type": "doc", "machine": "rohan", "data": {"name": "herdr-app", "version": 2}
        }));
        assert_eq!(doc.machine.as_deref(), Some("rohan"));
        assert_eq!(
            doc.change,
            Change::Doc {
                name: "herdr-app".into(),
                version: 2
            }
        );
    }

    #[test]
    fn remote_terminals_name_their_machine() {
        let remote = event(
            json!({"seq": 6, "type": "title", "terminal": "rohan/t3", "data": {"title": "x"}}),
        );
        assert_eq!(remote.machine.as_deref(), Some("rohan"));
    }

    #[test]
    fn foreground_can_be_null_and_a_turn_can_have_no_word() {
        let foreground =
            event(json!({"seq": 6, "type": "foreground", "terminal": "t1", "data": null}));
        assert_eq!(foreground.change, Change::Foreground(None));
        let turn = event(
            json!({"seq": 14, "type": "turn", "terminal": "t2", "data": {"state": null, "turn": 1}}),
        );
        assert_eq!(
            turn.change,
            Change::Turn {
                turn: 1,
                state: None
            }
        );
    }

    #[test]
    fn keeps_reading_past_new_event_types() {
        let parsed =
            event(json!({"seq": 9, "type": "someday", "terminal": "t1", "data": {"x": 1}}));
        assert_eq!(
            parsed.change,
            Change::Unknown {
                kind: "someday".into(),
                data: json!({"x": 1})
            }
        );
    }

    #[test]
    fn an_old_status_may_be_a_plain_word() {
        let terminal: Terminal =
            serde_json::from_value(json!({"address": "t1", "status": "idle"})).expect("terminal");
        assert_eq!(terminal.status.map(|s| s.state), Some("idle".into()));
    }

    #[test]
    fn a_lost_terminal_is_live_without_a_program() {
        let lost: Terminal = serde_json::from_value(json!({
            "address": "rohan/t2", "state": "live", "pid": null, "reason": "holder_lost"
        }))
        .expect("terminal");
        assert!(lost.is_lost() && lost.is_live());
        assert_eq!(lost.machine(), Some("rohan"));
        let pending: Terminal = serde_json::from_value(json!({
            "address": "t2", "state": "live", "reason": "spawn_pending"
        }))
        .expect("terminal");
        assert!(!pending.is_lost());
    }
}
