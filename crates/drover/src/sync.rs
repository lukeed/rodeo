use std::convert::Infallible;
use std::rc::Rc;

use doc::{Document, PlanError};
use serde_json::Value;

pub type Edit = Rc<dyn Fn(&mut Document) -> Result<(), doc::Error>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Loading,
    Ready,
    Undecodable(String),
    Unreadable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Read,
    Write { value: Value, version: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Written {
    Ok(u64),
    Conflict,
    Failed(String),
}

pub struct Slot {
    state: State,
    base: Value,
    version: u64,
    doc: Document,
    queue: Vec<Edit>,
    sent: Option<(usize, Value)>,
    reading: bool,
    stale: bool,
    known: u64,
    errors: Vec<String>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            state: State::Loading,
            base: Value::Null,
            version: 0,
            doc: Document::default(),
            queue: Vec::new(),
            sent: None,
            reading: false,
            stale: true,
            known: 0,
            errors: Vec::new(),
        }
    }
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot")
            .field("state", &self.state)
            .field("version", &self.version)
            .field("queued", &self.queue.len())
            .field("sending", &self.sent.as_ref().map(|(n, _)| *n))
            .finish_non_exhaustive()
    }
}

impl Slot {
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn is_ready(&self) -> bool {
        self.state == State::Ready
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn is_busy(&self) -> bool {
        self.reading || self.sent.is_some()
    }

    pub fn is_settled(&self) -> bool {
        !self.is_busy() && self.queue.is_empty() && !self.stale
    }

    pub fn take_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.errors)
    }

    pub fn edit(&mut self, edit: Edit) -> Result<bool, String> {
        match &self.state {
            State::Ready => {}
            State::Loading => return Err("the layout is still loading".into()),
            State::Undecodable(why) | State::Unreadable(why) => {
                return Err(format!("layout unavailable: {why}"));
            }
        }
        let before = self.doc.clone();
        edit(&mut self.doc).map_err(|error| error.to_string())?;
        if self.doc == before {
            return Ok(false);
        }
        self.queue.push(edit);
        Ok(true)
    }

    pub fn reset(&mut self) {
        self.state = State::Loading;
        self.reading = false;
        self.sent = None;
        self.stale = true;
        self.known = 0;
    }

    pub fn doc_event(&mut self, version: u64) {
        self.known = self.known.max(version);
        if version != self.version {
            self.stale = true;
        }
    }

    pub fn refresh(&mut self) {
        self.stale = true;
    }

    pub fn command(&mut self) -> Option<Command> {
        if self.is_busy() {
            return None;
        }
        if self.stale {
            self.stale = false;
            self.reading = true;
            return Some(Command::Read);
        }
        if self.queue.is_empty() || !self.is_ready() {
            return None;
        }
        let queue = self.queue.clone();
        let planned = doc::plan(&self.base, self.version, |doc| {
            for edit in &queue {
                let _ = edit(doc);
            }
            Ok::<(), Infallible>(())
        });
        match planned {
            Ok(planned) => {
                if let Some(write) = planned.write {
                    self.sent = Some((queue.len(), write.value.clone()));
                    Some(Command::Write {
                        value: write.value,
                        version: write.version,
                    })
                } else {
                    self.queue.clear();
                    self.doc = planned.doc;
                    None
                }
            }
            Err(PlanError::Undecodable(error)) => {
                self.queue.clear();
                self.state = State::Undecodable(error.to_string());
                self.doc = Document::default();
                None
            }
            Err(error) => {
                self.errors.push(error.to_string());
                self.queue.clear();
                self.rebase();
                None
            }
        }
    }

    pub fn read_done(&mut self, result: Result<(Value, u64), String>) {
        self.reading = false;
        match result {
            Ok((value, version)) => {
                let changed = version != self.version || !self.is_ready();
                if changed {
                    self.base = value;
                    self.version = version;
                    self.rebase();
                }
                self.stale = self.known > self.version;
            }
            Err(error) => {
                if self.state == State::Loading {
                    self.state = State::Unreadable(error);
                }
            }
        }
    }

    pub fn write_done(&mut self, result: Written) {
        let Some((count, value)) = self.sent.take() else {
            return;
        };
        match result {
            Written::Ok(version) => {
                self.queue.drain(..count.min(self.queue.len()));
                self.base = value;
                self.version = version;
                self.stale = self.known > version;
                self.rebase();
            }
            Written::Conflict => self.stale = true,
            Written::Failed(error) => {
                self.errors.push(error);
                self.queue.drain(..count.min(self.queue.len()));
                self.stale = true;
                self.rebase();
            }
        }
    }

    fn rebase(&mut self) {
        match Document::parse(&self.base) {
            Ok(mut doc) => {
                for edit in &self.queue {
                    let _ = edit(&mut doc);
                }
                self.doc = doc;
                self.state = State::Ready;
            }
            Err(error) => {
                self.doc = Document::default();
                self.queue.clear();
                self.state = State::Undecodable(error.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use doc::{Node, Tab};
    use serde_json::json;

    use super::*;

    fn space(id: &str) -> Edit {
        let id = id.to_owned();
        Rc::new(move |doc: &mut Document| {
            doc.new_space(
                &id,
                "",
                "/w",
                Tab::new(
                    format!("tb{id}"),
                    "",
                    Node::terminal(format!("p{id}"), "t1"),
                ),
            )
        })
    }

    fn loaded(value: Value, version: u64) -> Slot {
        let mut slot = Slot::default();
        assert_eq!(slot.command(), Some(Command::Read));
        slot.read_done(Ok((value, version)));
        slot
    }

    #[test]
    fn edits_apply_at_once_and_write_once() {
        let mut slot = loaded(Value::Null, 0);
        assert!(slot.edit(space("a")).unwrap());
        assert_eq!(slot.doc().spaces.len(), 1);
        let Some(Command::Write { version, value }) = slot.command() else {
            panic!("expected a write");
        };
        assert_eq!(version, 0);
        assert_eq!(value["spaces"][0]["id"], "a");
        assert!(slot.edit(space("b")).unwrap());
        assert_eq!(slot.command(), None, "one write in flight");
        slot.write_done(Written::Ok(1));
        assert_eq!(slot.doc().spaces.len(), 2);
        let Some(Command::Write { version, value }) = slot.command() else {
            panic!("expected the queued write");
        };
        assert_eq!(version, 1);
        assert_eq!(value["spaces"].as_array().unwrap().len(), 2);
        slot.write_done(Written::Ok(2));
        assert!(slot.is_settled());
    }

    #[test]
    fn nothing_changed_means_no_write() {
        let mut slot = loaded(json!({"spaces": []}), 3);
        let noop: Edit = Rc::new(|_: &mut Document| Ok(()));
        assert!(!slot.edit(noop).unwrap());
        assert_eq!(slot.command(), None);
    }

    #[test]
    fn conflict_rereads_and_keeps_both_edits() {
        let mut slot = loaded(Value::Null, 0);
        slot.edit(space("mine")).unwrap();
        assert!(matches!(slot.command(), Some(Command::Write { .. })));
        slot.write_done(Written::Conflict);
        assert_eq!(slot.command(), Some(Command::Read));
        let theirs = json!({"spaces": [{"id": "theirs", "name": "", "folder": "/t", "tabs": []}]});
        slot.read_done(Ok((theirs, 5)));
        let ids: Vec<_> = slot.doc().spaces.iter().map(|s| s.id.clone()).collect();
        assert_eq!(ids, ["theirs", "mine"]);
        let Some(Command::Write { version, value }) = slot.command() else {
            panic!("expected a retry");
        };
        assert_eq!(version, 5);
        assert_eq!(value["spaces"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn doc_events_wait_for_the_write_in_flight() {
        let mut slot = loaded(Value::Null, 0);
        slot.edit(space("a")).unwrap();
        assert!(matches!(slot.command(), Some(Command::Write { .. })));
        slot.doc_event(1);
        assert_eq!(slot.command(), None);
        slot.write_done(Written::Ok(1));
        assert_eq!(slot.command(), None, "the event was our own write");
        slot.doc_event(2);
        assert_eq!(slot.command(), Some(Command::Read));
    }

    #[test]
    fn undecodable_docs_refuse_edits() {
        let slot_value = json!({"spaces": [{"tabs": [{"root": {"type": "nope"}}]}]});
        let mut slot = loaded(slot_value, 4);
        assert!(matches!(slot.state(), State::Undecodable(_)));
        assert!(slot.edit(space("a")).is_err());
    }

    #[test]
    fn failed_writes_drop_their_edits_and_reread() {
        let mut slot = loaded(Value::Null, 0);
        slot.edit(space("a")).unwrap();
        slot.command();
        slot.write_done(Written::Failed("boom".into()));
        assert_eq!(slot.take_errors(), ["boom"]);
        assert_eq!(slot.doc().spaces.len(), 0);
        assert_eq!(slot.command(), Some(Command::Read));
    }
}
