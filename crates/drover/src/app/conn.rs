use std::time::{Duration, Instant};

use hc::{Change, Filter, Notice, Update};

use super::{App, Msg};
use crate::panes::Link;

const FIRST_RETRY: Duration = Duration::from_millis(250);
const LAST_RETRY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Connecting,
    Online,
    Down,
}

#[derive(Debug, Default)]
pub struct Conn {
    pub client: Option<hc::Client>,
    pub generation: u64,
    pub state: State,
    pub error: Option<String>,
    pub retry_at: Option<Instant>,
    pub backoff: Duration,
    pub connecting: bool,
}

impl Conn {
    pub fn is_online(&self) -> bool {
        self.state == State::Online
    }

    pub fn label(&self) -> &'static str {
        match self.state {
            State::Connecting => "connecting",
            State::Online => "online",
            State::Down => "reconnecting",
        }
    }
}

impl App {
    pub fn connect(&mut self) {
        if self.conn.connecting {
            return;
        }
        self.conn.connecting = true;
        self.conn.retry_at = None;
        let env = self.env.clone();
        let tx = self.tx.clone();
        tokio::task::spawn_local(async move {
            let result = hc::Client::connect_env(&env).await;
            let _ = tx.send(Msg::Apply {
                generation: None,
                apply: Box::new(move |app: &mut App| app.connected(result)),
            });
        });
    }

    fn connected(&mut self, result: Result<hc::Client, hc::HcError>) {
        self.conn.connecting = false;
        let client = match result {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(%error, "cannot connect to hc");
                self.conn.error = Some(error.to_string());
                self.schedule_retry();
                return;
            }
        };
        tracing::info!(version = %client.hello().version, core = %client.hello().core.name, "connected to hc");
        self.conn.generation += 1;
        self.conn.client = Some(client.clone());
        self.conn.state = State::Online;
        self.conn.error = None;
        self.conn.backoff = Duration::ZERO;
        for machine in self.machines.iter_mut() {
            machine.slot.reset();
        }
        self.subscribe(client);
        self.poll_machines();
        self.pump_all();
    }

    fn subscribe(&self, client: hc::Client) {
        let tx = self.tx.clone();
        let generation = self.conn.generation;
        tokio::task::spawn_local(async move {
            let apply = |apply: Box<dyn FnOnce(&mut App)>| Msg::Apply {
                generation: Some(generation),
                apply,
            };
            match client.subscribe(&Filter::default()).await {
                Ok((snapshot, mut events)) => {
                    let _ = tx.send(apply(Box::new(move |app| app.snapshot_arrived(snapshot))));
                    while let Some(notice) = events.next().await {
                        match notice {
                            Ok(notice) => {
                                if tx.send(Msg::Notice { generation, notice }).is_err() {
                                    return;
                                }
                            }
                            Err(error) => tracing::warn!(%error, "bad event line"),
                        }
                    }
                    let _ = tx.send(apply(Box::new(|app| {
                        app.disconnected("hc closed the event stream");
                    })));
                }
                Err(error) => {
                    let text = error.to_string();
                    let _ = tx.send(apply(Box::new(move |app| app.disconnected(&text))));
                }
            }
        });
    }

    fn snapshot_arrived(&mut self, snapshot: hc::Snapshot) {
        self.terminals = hc::Terminals::new(snapshot, &Filter::default());
        self.terminals_ready = true;
        for machine in self.machines.iter_mut() {
            machine.slot.refresh();
        }
        self.pump_all();
        self.sync_panes();
        self.reconcile();
    }

    pub(super) fn notice_event(&mut self, notice: Notice) {
        let restarted = match &notice {
            Notice::Event(event) if matches!(event.change, Change::Restarted(_)) => {
                event.terminal.clone()
            }
            _ => None,
        };
        let titled =
            matches!(&notice, Notice::Event(event) if matches!(event.change, Change::Title { .. }));
        match self.terminals.apply(notice) {
            Update::Nothing => {}
            Update::Doc {
                machine,
                name,
                version,
            } => {
                if name == doc::NAME
                    && let Some(m) = self.machines.get_mut(machine.as_deref())
                {
                    m.slot.doc_event(version);
                    self.pump(machine.as_deref());
                }
            }
            Update::Reset => {
                for machine in self.machines.iter_mut() {
                    machine.slot.refresh();
                }
                self.pump_all();
                self.sync_panes();
                self.reconcile();
            }
            Update::Changed(address) => {
                if restarted.as_deref() == Some(address.as_str()) {
                    self.reattach(&address);
                }
                if !titled {
                    self.reconcile();
                }
            }
            Update::Removed(_) => self.reconcile(),
        }
    }

    pub fn disconnected(&mut self, why: &str) {
        if self.conn.client.is_none() {
            return;
        }
        tracing::warn!(why, "lost hc");
        self.conn.client = None;
        self.conn.generation += 1;
        self.conn.state = State::Down;
        self.conn.error = Some(why.to_owned());
        self.terminals_ready = false;
        for pane in self.panes.iter_mut() {
            pane.detach();
            pane.link = Link::Idle;
        }
        self.adopting.clear();
        self.checking.clear();
        self.launching.clear();
        self.schedule_retry();
    }

    fn schedule_retry(&mut self) {
        if self.conn.state == State::Online {
            self.conn.state = State::Down;
        }
        self.conn.backoff = if self.conn.backoff.is_zero() {
            FIRST_RETRY
        } else {
            (self.conn.backoff * 2).min(LAST_RETRY)
        };
        self.conn.retry_at = Some(Instant::now() + self.conn.backoff);
        let wait = self.conn.backoff;
        self.later(wait, App::conn_tick);
    }

    pub(super) fn conn_tick(&mut self) {
        if self.conn.client.as_ref().is_some_and(hc::Client::is_closed) {
            self.disconnected("hc closed the connection");
        }
        if self.conn.client.is_none()
            && !self.conn.connecting
            && self.conn.retry_at.is_some_and(|at| at <= Instant::now())
        {
            self.connect();
        }
        if self.conn.client.is_some() && self.ticks.is_multiple_of(8) {
            self.retry_attaches();
        }
    }

    pub fn poll_machines(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        self.spawn(async move {
            let result = client.machines().await;
            move |app: &mut App| match result {
                Ok(views) => {
                    let changed = app.machines.update(views);
                    for machine in changed {
                        app.pump(machine.as_deref());
                    }
                    app.after_docs();
                }
                Err(error) => tracing::debug!(%error, "machine.list failed"),
            }
        });
    }
}
