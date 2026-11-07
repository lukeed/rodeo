use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::env::Env;
use crate::error::{HcError, Mismatch, RemoteError};
use crate::model::Hello;
use crate::paths::Paths;
use crate::start::{find_hc, start_server};
use crate::wire::{CONTROL, Codec, Frame, Incoming, Kind, read_size};

pub const GENERATION: u32 = 1;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub const METHODS: [&str; 16] = [
    "hello",
    "server.status",
    "terminal.run",
    "terminal.list",
    "terminal.get",
    "terminal.kill",
    "terminal.keep",
    "terminal.label",
    "terminal.read",
    "terminal.put",
    "terminal.attach",
    "events.subscribe",
    "doc.get",
    "doc.set",
    "machine.list",
    "machine.exec",
];

#[derive(Debug)]
pub(crate) enum Chunk {
    Data(Bytes),
    Resize(u16, u16),
    Close(Bytes),
}

type Opened = (Value, Option<mpsc::UnboundedReceiver<Chunk>>);

struct Pending {
    reply: oneshot::Sender<Result<Opened, HcError>>,
    opens: bool,
}

#[derive(Default)]
struct State {
    pending: HashMap<u64, Pending>,
    abandoned: HashSet<u64>,
    streams: HashMap<u32, mpsc::UnboundedSender<Chunk>>,
    early: HashMap<u32, Vec<Chunk>>,
    closing: HashSet<u32>,
    gone: bool,
}

impl State {
    fn deliver(&mut self, channel: u32, chunk: Chunk) {
        let ends = matches!(chunk, Chunk::Close(_));
        let replied = matches!(&chunk, Chunk::Close(reason) if reason.is_empty())
            && self.closing.remove(&channel);
        match self.streams.get(&channel) {
            Some(stream) => {
                let _ = stream.send(chunk);
                if ends {
                    self.streams.remove(&channel);
                }
            }
            None if replied => {}
            None => self.early.entry(channel).or_default().push(chunk),
        }
    }

    fn claim(&mut self, channel: u32) -> mpsc::UnboundedReceiver<Chunk> {
        let (stream, chunks) = mpsc::unbounded_channel();
        let mut closed = false;
        for chunk in self.early.remove(&channel).unwrap_or_default() {
            closed |= matches!(chunk, Chunk::Close(_));
            let _ = stream.send(chunk);
        }
        if !closed {
            self.streams.insert(channel, stream);
        }
        chunks
    }
}

struct Inner {
    out: mpsc::UnboundedSender<Frame>,
    state: Mutex<State>,
    next_id: AtomicU64,
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn send(&self, frame: Frame) -> Result<(), HcError> {
        self.out.send(frame).map_err(|_| HcError::Closed)
    }

    fn notify(&self, method: &str, params: &Value) {
        let note = json!({"jsonrpc": "2.0", "method": method, "params": params});
        if let Ok(payload) = serde_json::to_vec(&note) {
            let _ = self.send(Frame::new(CONTROL, Kind::Data, payload));
        }
    }

    fn close(&self, channel: u32) -> Result<(), HcError> {
        self.state().closing.insert(channel);
        self.send(Frame::new(channel, Kind::Close, Bytes::new()))
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RemoteError>,
}

fn outcome(response: Response) -> Result<Value, HcError> {
    match (response.result, response.error) {
        (_, Some(error)) => Err(HcError::Remote(error)),
        (Some(result), None) => Ok(result),
        (None, None) => Ok(Value::Null),
    }
}

fn channel_of(result: &Value) -> Option<u32> {
    result
        .get("channel")
        .and_then(Value::as_u64)
        .and_then(|id| u32::try_from(id).ok())
}

#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
    hello: Arc<Hello>,
    timeout: Duration,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("core", &self.hello.core)
            .field("version", &self.hello.version)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl Client {
    pub async fn connect<T>(io: T) -> Result<Self, HcError>
    where
        T: AsyncRead + AsyncWrite + Send + 'static,
    {
        tokio::time::timeout(DEFAULT_TIMEOUT, handshake(io))
            .await
            .map_err(|_| HcError::Timeout {
                method: "hello".into(),
                after: DEFAULT_TIMEOUT,
            })?
    }

    pub async fn connect_unix(socket: &Path) -> Result<Self, HcError> {
        Self::connect(tokio::net::UnixStream::connect(socket).await?).await
    }

    pub async fn connect_env(env: &Env) -> Result<Self, HcError> {
        let paths = Paths::resolve(env)?;
        if let Ok(stream) = tokio::net::UnixStream::connect(&paths.socket).await {
            return Self::connect(stream).await;
        }
        if let Some(actual) = paths.listening_at().filter(|actual| {
            actual != &paths.socket && std::os::unix::net::UnixStream::connect(actual).is_ok()
        }) {
            return Err(HcError::Elsewhere {
                actual,
                expected: paths.socket,
            });
        }
        let bin = find_hc(env).await.ok_or(HcError::NoBinary)?;
        start_server(&bin, env).await?;
        Self::connect_unix(&paths.socket).await
    }

    pub fn hello(&self) -> &Hello {
        &self.hello
    }

    pub fn is_closed(&self) -> bool {
        self.inner.state().gone
    }

    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, HcError> {
        self.call_within(method, params, self.timeout).await
    }

    pub async fn call_within<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<T, HcError> {
        let (result, _) = self.request(method, params, false, timeout).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub(crate) async fn open(
        &self,
        method: &str,
        params: Value,
    ) -> Result<(Value, Link, mpsc::UnboundedReceiver<Chunk>), HcError> {
        let (result, chunks) = self.request(method, params, true, self.timeout).await?;
        let (Some(id), Some(chunks)) = (channel_of(&result), chunks) else {
            return Err(HcError::NoChannel(method.to_owned()));
        };
        let link = Link {
            id,
            inner: Arc::clone(&self.inner),
            open: AtomicBool::new(true),
        };
        Ok((result, link, chunks))
    }

    async fn request(
        &self,
        method: &str,
        params: Value,
        opens: bool,
        timeout: Duration,
    ) -> Result<Opened, HcError> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = oneshot::channel();
        {
            let mut state = self.inner.state();
            if state.gone {
                return Err(HcError::Closed);
            }
            state.pending.insert(id, Pending { reply, opens });
        }
        let mut waiting = Waiting {
            inner: &self.inner,
            id,
            opens,
            answer,
            done: false,
        };
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.inner.send(Frame::new(
            CONTROL,
            Kind::Data,
            serde_json::to_vec(&request)?,
        ))?;
        let answered = tokio::time::timeout(timeout, &mut waiting.answer).await;
        waiting.done = answered.is_ok();
        match answered {
            Ok(Ok(answer)) => answer,
            Ok(Err(_)) => Err(HcError::Closed),
            Err(_) => Err(HcError::Timeout {
                method: method.to_owned(),
                after: timeout,
            }),
        }
    }
}

struct Waiting<'a> {
    inner: &'a Inner,
    id: u64,
    opens: bool,
    answer: oneshot::Receiver<Result<Opened, HcError>>,
    done: bool,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let mut state = self.inner.state();
        if state.pending.remove(&self.id).is_some() {
            if self.opens {
                state.abandoned.insert(self.id);
            }
            drop(state);
            self.inner
                .notify("$/cancelRequest", &json!({"id": self.id}));
            return;
        }
        drop(state);
        if let Ok(Ok((result, Some(_)))) = self.answer.try_recv()
            && let Some(channel) = channel_of(&result)
        {
            let _ = self.inner.close(channel);
        }
    }
}

async fn handshake<T>(io: T) -> Result<Client, HcError>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    let (read, mut write) = tokio::io::split(io);
    let hello = json!({
        "jsonrpc": "2.0", "id": 0, "method": "hello",
        "params": {"generation": GENERATION, "framed": true},
    });
    let mut line = serde_json::to_vec(&hello)?;
    line.push(b'\n');
    write.write_all(&line).await?;
    write.flush().await?;

    let mut frames = FramedRead::new(read, Codec::default());
    let Some(Incoming::Hello(reply)) = frames.next().await.transpose()? else {
        return Err(HcError::Closed);
    };
    let hello = match outcome(serde_json::from_slice(&reply)?) {
        Err(HcError::Remote(error)) if error.kind == "unsupported_generation" => {
            return Err(Mismatch::Generation(None).into());
        }
        result => serde_json::from_value::<Hello>(result?)?,
    };
    check(&hello)?;

    let (out, outbox) = mpsc::unbounded_channel();
    let inner = Arc::new(Inner {
        out,
        state: Mutex::default(),
        next_id: AtomicU64::new(1),
    });
    tokio::spawn(send_frames(
        FramedWrite::new(write, Codec::default()),
        outbox,
    ));
    tokio::spawn(read_frames(frames, Arc::clone(&inner)));
    Ok(Client {
        inner,
        hello: Arc::new(hello),
        timeout: DEFAULT_TIMEOUT,
    })
}

pub(crate) fn check(hello: &Hello) -> Result<(), Mismatch> {
    if hello.generation != GENERATION {
        return Err(Mismatch::Generation(Some(hello.generation)));
    }
    let missing: Vec<String> = METHODS
        .iter()
        .filter(|method| !hello.has_method(method))
        .map(|method| (*method).to_owned())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Mismatch::Methods(missing))
    }
}

async fn send_frames<W: AsyncWrite + Unpin>(
    mut sink: FramedWrite<W, Codec>,
    mut outbox: mpsc::UnboundedReceiver<Frame>,
) {
    while let Some(frame) = outbox.recv().await {
        if sink.feed(frame).await.is_err() {
            return;
        }
        while let Ok(frame) = outbox.try_recv() {
            if sink.feed(frame).await.is_err() {
                return;
            }
        }
        if sink.flush().await.is_err() {
            return;
        }
    }
}

async fn read_frames<R: AsyncRead + Unpin>(mut frames: FramedRead<R, Codec>, inner: Arc<Inner>) {
    while let Some(Ok(Incoming::Frame(frame))) = frames.next().await {
        route(&inner, frame);
    }
    let (pending, streams) = {
        let mut state = inner.state();
        state.gone = true;
        state.early.clear();
        state.abandoned.clear();
        state.closing.clear();
        (
            std::mem::take(&mut state.pending),
            std::mem::take(&mut state.streams),
        )
    };
    drop(streams);
    for (_, waiting) in pending {
        let _ = waiting.reply.send(Err(HcError::Closed));
    }
}

fn route(inner: &Inner, frame: Frame) {
    let chunk = match (frame.channel, frame.kind) {
        (CONTROL, Kind::Data) => return reply(inner, &frame.payload),
        (CONTROL, _) => return,
        (_, Kind::Data) => Chunk::Data(frame.payload),
        (_, Kind::Close) => Chunk::Close(frame.payload),
        (_, Kind::Resize) => match read_size(&frame.payload) {
            Some((cols, rows)) => Chunk::Resize(cols, rows),
            None => return,
        },
        _ => return,
    };
    inner.state().deliver(frame.channel, chunk);
}

fn reply(inner: &Inner, payload: &[u8]) {
    let Ok(response) = serde_json::from_slice::<Response>(payload) else {
        return;
    };
    let Some(id) = response.id else {
        return;
    };
    let mut state = inner.state();
    if state.abandoned.remove(&id) {
        if let Ok(result) = outcome(response)
            && let Some(channel) = channel_of(&result)
        {
            let _ = state.claim(channel);
            drop(state);
            let _ = inner.close(channel);
        }
        return;
    }
    let Some(waiting) = state.pending.remove(&id) else {
        return;
    };
    let answer = outcome(response).map(|result| {
        let chunks = channel_of(&result)
            .filter(|_| waiting.opens)
            .map(|channel| state.claim(channel));
        (result, chunks)
    });
    let _ = waiting.reply.send(answer);
}

pub(crate) struct Link {
    id: u32,
    inner: Arc<Inner>,
    open: AtomicBool,
}

impl Link {
    pub(crate) const fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn send(&self, kind: Kind, payload: impl Into<Bytes>) -> Result<(), HcError> {
        self.inner.send(Frame::new(self.id, kind, payload))
    }

    pub(crate) fn credit(&self, consumed: usize) {
        if consumed > 0 {
            let consumed = u32::try_from(consumed).unwrap_or(u32::MAX);
            let _ = self.send(Kind::Credit, consumed.to_le_bytes().to_vec());
        }
    }

    pub(crate) fn ended(&self) {
        self.open.store(false, Ordering::Release);
    }

    pub(crate) fn close(&self) -> Result<(), HcError> {
        if self.open.swap(false, Ordering::AcqRel) {
            self.inner.close(self.id)
        } else {
            Ok(())
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(methods: &[&str], generation: u32) -> Hello {
        Hello {
            generation,
            version: "0.1.18".into(),
            core: crate::CoreInfo::default(),
            framed: true,
            methods: methods.iter().map(|m| (*m).to_owned()).collect(),
            features: Vec::new(),
        }
    }

    #[test]
    fn gates_on_generation_and_methods() {
        assert_eq!(check(&hello(&METHODS, 1)), Ok(()));
        assert_eq!(
            check(&hello(&METHODS, 2)),
            Err(Mismatch::Generation(Some(2)))
        );
        assert_eq!(
            check(&hello(&METHODS[..15], 1)),
            Err(Mismatch::Methods(vec!["machine.exec".into()]))
        );
    }
}
