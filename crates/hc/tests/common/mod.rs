#![allow(
    dead_code,
    reason = "each test binary uses a different part of the fixture"
)]

use std::collections::BTreeMap;
use std::time::Duration;

use hc::{
    Attach, AttachReader, Change, Client, Ending, Event, Events, Notice, Output, RunParams,
    Terminal,
};

pub const WITHIN: Duration = Duration::from_secs(15);

pub fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

pub async fn run(client: &Client, line: &str, pairs: &[(&str, &str)]) -> Terminal {
    client
        .run(&RunParams {
            line: Some(line.to_owned()),
            labels: labels(pairs),
            ..RunParams::default()
        })
        .await
        .expect("run")
}

#[derive(Default)]
pub struct Screen {
    pub raw: Vec<u8>,
    pub resizes: Vec<(u16, u16)>,
    pub ended: Option<Ending>,
    pub gone: bool,
}

pub trait Source {
    async fn output(&mut self) -> Option<Output>;
}

impl Source for Attach {
    async fn output(&mut self) -> Option<Output> {
        self.next().await
    }
}

impl Source for AttachReader {
    async fn output(&mut self) -> Option<Output> {
        self.next().await
    }
}

impl Screen {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.raw).into_owned()
    }

    pub fn hex(&self) -> Vec<u8> {
        self.text()
            .split_whitespace()
            .filter_map(|token| u8::from_str_radix(token, 16).ok())
            .collect()
    }

    pub async fn until(
        &mut self,
        source: &mut impl Source,
        what: &str,
        done: impl Fn(&Self) -> bool,
    ) {
        let waited = tokio::time::timeout(WITHIN, async {
            while !done(self) {
                match source.output().await {
                    Some(Output::Bytes(bytes)) => self.raw.extend_from_slice(&bytes),
                    Some(Output::Resize { cols, rows }) => self.resizes.push((cols, rows)),
                    Some(Output::Closed(ending)) => self.ended = Some(ending),
                    None => {
                        self.gone = true;
                        return;
                    }
                }
            }
        })
        .await;
        assert!(
            waited.is_ok() && done(self),
            "waited for {what} after {} bytes, end {:?}; raw:\n{:?}",
            self.raw.len(),
            self.ended,
            self.text()
        );
    }

    pub async fn shows(&mut self, source: &mut impl Source, needle: &str) {
        self.until(source, needle, |screen| screen.text().contains(needle))
            .await;
    }

    pub async fn ends(&mut self, source: &mut impl Source) -> Ending {
        self.until(source, "the end", |screen| screen.ended.is_some())
            .await;
        self.ended.clone().expect("ended")
    }
}

pub struct Watch {
    pub events: Events,
    pub seq: u64,
}

impl Watch {
    pub async fn start(client: &Client, filter: &hc::Filter) -> Self {
        let (snapshot, events) = client.subscribe(filter).await.expect("subscribe");
        Self {
            events,
            seq: snapshot.seq,
        }
    }

    pub async fn until(&mut self, what: &str, wanted: impl Fn(&Event) -> bool) -> Vec<Event> {
        let mut seen = Vec::new();
        let found = tokio::time::timeout(WITHIN, async {
            while let Some(notice) = self.events.next().await {
                let Notice::Event(event) = notice.expect("notice") else {
                    continue;
                };
                assert!(event.seq > self.seq, "events come in order");
                self.seq = event.seq;
                let done = wanted(&event);
                seen.push(event);
                if done {
                    return true;
                }
            }
            false
        })
        .await;
        assert!(
            matches!(found, Ok(true)),
            "waited for {what}; saw {seen:#?}"
        );
        seen
    }
}

pub fn size_of(event: &Event) -> Option<(u16, u16)> {
    match event.change {
        Change::Size { cols, rows } => Some((cols, rows)),
        _ => None,
    }
}

pub fn sized(cols: u16, rows: u16) -> impl Fn(&Event) -> bool {
    move |event| size_of(event) == Some((cols, rows))
}

pub async fn eventually<T>(what: &str, mut probe: impl AsyncFnMut() -> Option<T>) -> T {
    let found = tokio::time::timeout(WITHIN, async {
        loop {
            if let Some(found) = probe().await {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    found.unwrap_or_else(|_| panic!("waited for {what}"))
}
