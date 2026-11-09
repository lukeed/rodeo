use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::{Duration, Instant};

use config::ToastDelivery;

use super::{App, Notice, NoticeDot};
use crate::agents::State;
use crate::sound::{Sound, Sounds};

const QUEUE: usize = 8;
const GRACE: Duration = Duration::from_secs(1);
const RECHECK: Duration = Duration::from_millis(50);
const SENT: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Attention,
    Finished,
}

impl Kind {
    fn sound(self) -> Sound {
        match self {
            Self::Attention => Sound::Request,
            Self::Finished => Sound::Done,
        }
    }

    fn ttl(self) -> Duration {
        Duration::from_secs(match self {
            Self::Attention => 8,
            Self::Finished => 5,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub delivery: ToastDelivery,
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
pub struct Notices {
    pub queue: VecDeque<(Notice, Duration)>,
    pub sounds: Sounds,
    pub sent: Vec<Sent>,
    pub testing: bool,
    pending: HashMap<String, u64>,
    seq: u64,
}

impl Notices {
    pub fn new(env: &hc::Env) -> Self {
        Self {
            queue: VecDeque::new(),
            sounds: Sounds::from_env(env),
            sent: Vec::new(),
            testing: env.get("NEXTEST").is_some() || std::env::var_os("NEXTEST").is_some(),
            pending: HashMap::new(),
            seq: 0,
        }
    }

    fn record(&mut self, delivery: ToastDelivery, title: &str, body: &str) {
        if self.sent.len() == SENT {
            self.sent.remove(0);
        }
        self.sent.push(Sent {
            delivery,
            title: title.to_owned(),
            body: body.to_owned(),
        });
    }
}

pub fn kind(before: Option<State>, after: Option<State>) -> Option<Kind> {
    if before == after {
        return None;
    }
    match (before, after) {
        (_, Some(State::Blocked)) => Some(Kind::Attention),
        (Some(State::Working | State::Blocked), Some(State::Idle | State::Done)) => {
            Some(Kind::Finished)
        }
        _ => None,
    }
}

impl App {
    pub(super) fn agent_changed(&mut self, address: &str, before: Option<State>) {
        let Some(kind) = kind(before, self.agent_state(address)) else {
            return;
        };
        if self.config.ui.toast.delivery == ToastDelivery::Off && !self.config.ui.sound.enabled {
            return;
        }
        self.notices.seq += 1;
        let seq = self.notices.seq;
        self.notices.pending.insert(address.to_owned(), seq);
        if let Some(pane) = self.machines.pane_of_address(address).map(|p| p.key()) {
            self.notices
                .queue
                .retain(|(n, _)| n.target.as_deref() != Some(pane.as_str()));
            if self
                .notice
                .as_ref()
                .is_some_and(|n| n.target.as_deref() == Some(pane.as_str()))
            {
                self.next_notice();
            }
        }
        let received = Instant::now();
        let delay = Duration::from_secs(self.config.ui.toast.delay_seconds);
        let check = delay > Duration::ZERO || kind == Kind::Finished;
        let address = address.to_owned();
        if delay.is_zero() {
            self.settle(&address, kind, seq, received, check);
        } else {
            self.later(delay, move |app: &mut App| {
                app.settle(&address, kind, seq, received, check);
            });
        }
    }

    fn settle(&mut self, address: &str, kind: Kind, seq: u64, received: Instant, check: bool) {
        if self.notices.pending.get(address) != Some(&seq) {
            return;
        }
        let now = self.agent_state(address);
        let holds = !check
            || match kind {
                Kind::Attention => now == Some(State::Blocked),
                Kind::Finished => now == Some(State::Done),
            };
        let waiting =
            kind == Kind::Finished && matches!(now, None | Some(State::Working | State::Idle));
        if !holds && waiting && received.elapsed() < GRACE {
            let address = address.to_owned();
            self.later(RECHECK, move |app: &mut App| {
                app.settle(&address, kind, seq, received, check);
            });
            return;
        }
        self.notices.pending.remove(address);
        if holds {
            self.deliver(address, kind);
        }
    }

    fn deliver(&mut self, address: &str, kind: Kind) {
        let Some(pane) = self.machines.pane_of_address(address) else {
            return;
        };
        let pane_key = pane.key();
        let tab_key = pane.tab_key();
        let space_key = pane.space_key();
        let tabs = pane.space.tabs.len();
        let tab_name = self
            .machines
            .tab(&tab_key)
            .map(|t| t.name())
            .unwrap_or_default();
        let terminal = self.terminals.get(address);
        let agent = terminal
            .and_then(crate::agents::label)
            .unwrap_or_else(|| "agent".to_owned());
        let agent_kind = terminal
            .and_then(|t| t.status.as_ref())
            .map(|s| crate::agents::canonical(&s.app))
            .filter(|k| !k.is_empty());
        let number = self
            .machines
            .spaces()
            .iter()
            .position(|s| s.key() == space_key)
            .map_or(0, |i| i + 1);
        let space_name = self.space_name(&space_key, &self.home());
        let body = if tabs > 1 {
            format!("{space_name} · {number} · {tab_name}")
        } else {
            format!("{space_name} · {number}")
        };
        let (title, dot) = match kind {
            Kind::Attention => (format!("{agent} needs attention"), NoticeDot::Red),
            Kind::Finished => (format!("{agent} finished"), NoticeDot::Blue),
        };
        let active = self.view.active_tab(&self.machines).as_deref() == Some(tab_key.as_str());
        let quiet = active && self.view.host_focused;
        let sound = &self.config.ui.sound;
        if sound.allows(agent_kind.as_deref()) && !(kind == Kind::Finished && quiet) {
            let dir = self.config_path.parent().unwrap_or_else(|| Path::new("."));
            let custom = sound.path_for(kind.sound(), dir);
            self.notices.sounds.play(kind.sound(), custom);
        }
        let delivery = self.config.ui.toast.delivery;
        match delivery {
            ToastDelivery::Herdr if !active => {
                self.queue_notice(
                    Notice {
                        title,
                        body,
                        dot,
                        until: Instant::now(),
                        target: Some(pane_key),
                    },
                    kind.ttl(),
                );
            }
            ToastDelivery::Terminal if !quiet => {
                self.notify_host(&title, &body);
                self.notices.record(delivery, &title, &body);
            }
            ToastDelivery::System if !quiet => {
                if !self.notices.testing {
                    crate::system_notify::show(&title, &body);
                }
                self.notices.record(delivery, &title, &body);
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn queue_notice(&mut self, notice: Notice, ttl: Duration) {
        if self.notice.is_some() {
            if self.notices.queue.len() == QUEUE {
                self.notices.queue.pop_front();
            }
            self.notices.queue.push_back((notice, ttl));
        } else {
            self.notice = Some(Notice {
                until: Instant::now() + ttl,
                ..notice
            });
        }
    }

    pub fn next_notice(&mut self) {
        self.notice = self.notices.queue.pop_front().map(|(notice, ttl)| Notice {
            until: Instant::now() + ttl,
            ..notice
        });
    }

    pub fn take_notice(&mut self) -> Option<Notice> {
        let notice = self.notice.take();
        if notice.is_some() {
            self.next_notice();
        }
        notice
    }

    fn notify_host(&mut self, title: &str, body: &str) {
        let (title, body) = (
            crate::system_notify::clean(title),
            crate::system_notify::clean(body),
        );
        let var = |key: &str| self.env.get(key).map(|v| v.to_string_lossy().into_owned());
        let program = var("TERM_PROGRAM").unwrap_or_default();
        let term = var("TERM").unwrap_or_default();
        let osc9 = matches!(program.as_str(), "ghostty" | "iTerm.app" | "WezTerm");
        let kitty = var("KITTY_WINDOW_ID").is_some();
        let sequence = if osc9 {
            format!("\x1b]9;{title}: {body}\x1b\\")
        } else if kitty || term == "xterm-kitty" {
            format!("\x1b]99;i=1:d=0;{title}\x1b\\\x1b]99;i=1:p=body;{body}\x1b\\")
        } else if term == "xterm-ghostty" || term.contains("wezterm") {
            format!("\x1b]9;{title}: {body}\x1b\\")
        } else {
            return;
        };
        let wrapped = if var("TMUX").is_some() {
            format!("\x1bPtmux;{}\x1b\\", sequence.replace('\x1b', "\x1b\x1b"))
        } else {
            sequence
        };
        self.out.extend_from_slice(wrapped.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions_pick_the_notice_kind() {
        use State::{Blocked, Done, Idle, Working};
        assert_eq!(kind(Some(Working), Some(Blocked)), Some(Kind::Attention));
        assert_eq!(kind(None, Some(Blocked)), Some(Kind::Attention));
        assert_eq!(kind(Some(Blocked), Some(Blocked)), None);
        assert_eq!(kind(Some(Working), Some(Done)), Some(Kind::Finished));
        assert_eq!(kind(Some(Blocked), Some(Idle)), Some(Kind::Finished));
        assert_eq!(kind(Some(Idle), Some(Done)), None);
        assert_eq!(kind(None, Some(Idle)), None);
        assert_eq!(kind(Some(Done), Some(Working)), None);
    }
}
