use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::client::{Chunk, Client, Link};
use crate::error::HcError;
use crate::model::{Display, Ending, Event, Notice, Snapshot, Terminal};
use crate::wire::{Kind, size};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AttachOptions {
    pub terminal: String,
    pub cols: u16,
    pub rows: u16,
    #[serde(skip_serializing_if = "is_zero")]
    pub history: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub observe: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub resize_notifications: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub resumed: bool,
    #[serde(skip_serializing_if = "Display::is_empty")]
    pub display: Display,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
}

impl AttachOptions {
    pub fn new(terminal: impl Into<String>, cols: u16, rows: u16) -> Self {
        Self {
            terminal: terminal.into(),
            cols,
            rows,
            ..Self::default()
        }
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes a reference"
)]
const fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Filter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_of: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Bytes(Bytes),
    Resize { cols: u16, rows: u16 },
    Closed(Ending),
}

pub struct Attach {
    pub terminal: Terminal,
    pub resize_notifications: bool,
    reader: AttachReader,
    writer: AttachWriter,
}

impl std::fmt::Debug for Attach {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Attach")
            .field("terminal", &self.terminal.address)
            .field("channel", &self.writer.link.id())
            .finish_non_exhaustive()
    }
}

impl Attach {
    pub async fn next(&mut self) -> Option<Output> {
        self.reader.next().await
    }

    pub const fn writer(&self) -> &AttachWriter {
        &self.writer
    }

    pub fn split(self) -> (AttachReader, AttachWriter) {
        (self.reader, self.writer)
    }
}

pub struct AttachReader {
    link: Arc<Link>,
    chunks: mpsc::UnboundedReceiver<Chunk>,
}

impl AttachReader {
    pub async fn next(&mut self) -> Option<Output> {
        Some(match self.chunks.recv().await? {
            Chunk::Data(bytes) => {
                self.link.credit(bytes.len());
                Output::Bytes(bytes)
            }
            Chunk::Resize(cols, rows) => Output::Resize { cols, rows },
            Chunk::Close(reason) => {
                self.link.ended();
                self.chunks.close();
                Output::Closed(serde_json::from_slice(&reason).unwrap_or_default())
            }
        })
    }
}

#[derive(Clone)]
pub struct AttachWriter {
    link: Arc<Link>,
    automatic_input: bool,
}

impl AttachWriter {
    pub const fn automatic_input_supported(&self) -> bool {
        self.automatic_input
    }

    pub fn input(&self, bytes: impl Into<Bytes>) -> Result<(), HcError> {
        self.bytes(Kind::Data, bytes.into())
    }

    pub fn automatic_input(&self, bytes: impl Into<Bytes>) -> Result<(), HcError> {
        let kind = if self.automatic_input {
            Kind::AutomaticInput
        } else {
            Kind::Data
        };
        self.bytes(kind, bytes.into())
    }

    fn bytes(&self, kind: Kind, bytes: Bytes) -> Result<(), HcError> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.link.send(kind, bytes)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), HcError> {
        self.link.send(Kind::Resize, size(cols, rows).to_vec())
    }

    pub fn focus(&self, focused: bool) -> Result<(), HcError> {
        self.link.send(Kind::Focus, vec![u8::from(focused)])
    }

    pub fn facts(&self, display: &Display) -> Result<(), HcError> {
        self.link.send(Kind::Facts, serde_json::to_vec(display)?)
    }

    pub fn close(&self) -> Result<(), HcError> {
        self.link.close()
    }
}

pub struct Events {
    link: Link,
    chunks: mpsc::UnboundedReceiver<Chunk>,
    lines: BytesMut,
}

#[derive(Deserialize)]
struct Notification {
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Deserialize)]
struct Seq {
    seq: u64,
}

impl Events {
    pub async fn next(&mut self) -> Option<Result<Notice, HcError>> {
        loop {
            if let Some(end) = self.lines.iter().position(|b| *b == b'\n') {
                let line = self.lines.split_to(end + 1);
                if let Some(notice) = parse(&line[..end]).transpose() {
                    return Some(notice);
                }
                continue;
            }
            match self.chunks.recv().await? {
                Chunk::Data(bytes) => {
                    self.link.credit(bytes.len());
                    self.lines.extend_from_slice(&bytes);
                }
                Chunk::Resize(..) => {}
                Chunk::Close(_) => {
                    self.link.ended();
                    self.chunks.close();
                    return None;
                }
            }
        }
    }
}

pub(crate) fn parse(line: &[u8]) -> Result<Option<Notice>, HcError> {
    if line.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let note: Notification = serde_json::from_slice(line)?;
    Ok(Some(match note.method.as_str() {
        "event" => Notice::Event(serde_json::from_value::<Event>(note.params)?),
        "heartbeat" => Notice::Heartbeat {
            seq: serde_json::from_value::<Seq>(note.params)?.seq,
        },
        "missed" => Notice::Missed(serde_json::from_value::<Snapshot>(note.params)?),
        _ => return Ok(None),
    }))
}

#[derive(Deserialize)]
struct Attached {
    terminal: Terminal,
    #[serde(default)]
    resize_notifications: bool,
    #[serde(default)]
    automatic_input: bool,
}

impl Client {
    pub async fn attach(&self, options: &AttachOptions) -> Result<Attach, HcError> {
        let (result, link, chunks) = self
            .open("terminal.attach", serde_json::to_value(options)?)
            .await?;
        let link = Arc::new(link);
        let attached: Attached = serde_json::from_value(result)?;
        Ok(Attach {
            terminal: attached.terminal,
            resize_notifications: attached.resize_notifications,
            reader: AttachReader {
                link: Arc::clone(&link),
                chunks,
            },
            writer: AttachWriter {
                link,
                automatic_input: attached.automatic_input
                    && self.hello().has_feature("automatic_input"),
            },
        })
    }

    pub async fn subscribe(&self, filter: &Filter) -> Result<(Snapshot, Events), HcError> {
        let (result, link, chunks) = self
            .open("events.subscribe", serde_json::to_value(filter)?)
            .await?;
        let snapshot = serde_json::from_value(result)?;
        let events = Events {
            link,
            chunks,
            lines: BytesMut::new(),
        };
        Ok((snapshot, events))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn attach_options_send_only_what_is_set() {
        let plain = serde_json::to_value(AttachOptions::new("t1", 80, 24)).unwrap();
        assert_eq!(plain, json!({"terminal": "t1", "cols": 80, "rows": 24}));
        let full = AttachOptions {
            history: 2000,
            observe: true,
            resize_notifications: true,
            display: Display {
                bg: Some("#000000".into()),
                ..Display::default()
            },
            ..AttachOptions::new("t1", 80, 24)
        };
        let full = serde_json::to_value(full).unwrap();
        assert_eq!(full["history"], 2000);
        assert_eq!(full["display"], json!({"bg": "#000000"}));
        assert_eq!(full["resize_notifications"], true);
    }

    #[test]
    fn parses_notifications_and_skips_others() {
        let heartbeat = parse(br#"{"jsonrpc":"2.0","method":"heartbeat","params":{"seq":42}}"#);
        assert_eq!(heartbeat.unwrap(), Some(Notice::Heartbeat { seq: 42 }));
        let missed =
            parse(br#"{"method":"missed","params":{"seq":57,"terminals":[{"address":"t1"}]}}"#)
                .unwrap();
        let Some(Notice::Missed(snapshot)) = missed else {
            panic!("missed");
        };
        assert_eq!(
            (snapshot.seq, snapshot.terminals[0].address.as_str()),
            (57, "t1")
        );
        assert_eq!(parse(br#"{"method":"later","params":{}}"#).unwrap(), None);
        assert_eq!(parse(b"  ").unwrap(), None);
        assert!(parse(b"{nope").is_err());
    }
}
