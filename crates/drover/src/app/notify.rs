use std::time::Duration;

use config::ToastDelivery;

use super::{App, NoticeDot};
use crate::agents::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Attention,
    Finished,
}

impl App {
    pub(super) fn agent_changed(&mut self, address: &str, before: Option<State>) {
        let after = self.agent_state(address);
        if after == before || self.config.ui.toast.delivery == ToastDelivery::Off {
            return;
        }
        let kind = match (before, after) {
            (_, Some(State::Blocked)) => Kind::Attention,
            (Some(State::Working | State::Blocked), Some(State::Idle | State::Done)) => {
                Kind::Finished
            }
            _ => return,
        };
        let delay = Duration::from_secs(self.config.ui.toast.delay_seconds);
        let address = address.to_owned();
        self.later(delay, move |app: &mut App| app.deliver(&address, kind));
    }

    fn deliver(&mut self, address: &str, kind: Kind) {
        let now = self.agent_state(address);
        let holds = match kind {
            Kind::Attention => now == Some(State::Blocked),
            Kind::Finished => matches!(now, Some(State::Done | State::Idle)),
        };
        if !holds {
            return;
        }
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
        let agent = self
            .terminals
            .get(address)
            .and_then(crate::agents::label)
            .unwrap_or_else(|| "agent".to_owned());
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
        let (title, dot, ttl) = match kind {
            Kind::Attention => (format!("{agent} needs attention"), NoticeDot::Red, 8),
            Kind::Finished => (format!("{agent} finished"), NoticeDot::Blue, 5),
        };
        let active = self.view.active_tab(&self.machines).as_deref() == Some(tab_key.as_str());
        let suppressed = active && self.view.host_focused;
        match self.config.ui.toast.delivery {
            ToastDelivery::Herdr if !active => {
                self.show_notice(&title, &body, dot, Some(pane_key), Duration::from_secs(ttl));
            }
            ToastDelivery::Terminal if !suppressed => self.notify_host(&title, &body),
            _ => {}
        }
        self.dirty = true;
    }

    fn notify_host(&mut self, title: &str, body: &str) {
        let clean = |text: &str| -> String { text.chars().filter(|c| !c.is_control()).collect() };
        let (title, body) = (clean(title), clean(body));
        let var = |key: &str| self.env.get(key).map(|v| v.to_string_lossy().into_owned());
        let program = var("TERM_PROGRAM").unwrap_or_default();
        let term = var("TERM").unwrap_or_default();
        let kitty = var("KITTY_WINDOW_ID").is_some() || term == "xterm-kitty";
        let osc9 = matches!(program.as_str(), "ghostty" | "iTerm.app" | "WezTerm")
            || term == "xterm-ghostty"
            || term.contains("wezterm");
        let sequence = if kitty {
            format!("\x1b]99;i=1:d=0;{title}\x1b\\\x1b]99;i=1:p=body;{body}\x1b\\")
        } else if osc9 {
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
