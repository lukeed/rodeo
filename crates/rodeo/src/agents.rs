use config::{Palette, StatusIndicators};
use ratatui::style::Color;

pub const SEEN: &str = "herdr.seen";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    Unknown,
    Idle,
    Working,
    Done,
    Blocked,
}

impl State {
    pub fn priority(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Idle => 1,
            Self::Working => 2,
            Self::Done => 3,
            Self::Blocked => 4,
        }
    }

    pub fn icon(self, style: StatusIndicators) -> &'static str {
        match (self, style) {
            (Self::Blocked, StatusIndicators::Symbols) => "×",
            (Self::Done, StatusIndicators::Symbols) => "✓",
            (Self::Working, StatusIndicators::Symbols) => "◐",
            (Self::Blocked | Self::Done | Self::Working, StatusIndicators::Dots) => "●",
            (Self::Idle, _) => "○",
            (Self::Unknown, _) => "·",
        }
    }

    pub fn color(self, palette: &Palette) -> Color {
        match self {
            Self::Blocked => palette.red,
            Self::Done => palette.teal,
            Self::Working => palette.yellow,
            Self::Idle => palette.green,
            Self::Unknown => palette.overlay0,
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Working => "working",
            Self::Idle => "idle",
            Self::Unknown => "unknown",
        }
    }
}

pub fn seen_turn(terminal: &hc::Terminal) -> Option<u64> {
    terminal
        .label(SEEN)
        .and_then(|seen| seen.trim().parse().ok())
}

pub fn state(terminal: &hc::Terminal, looked: Option<u64>) -> Option<State> {
    let status = terminal.status.as_ref()?;
    Some(match status.state.as_str() {
        "working" => State::Working,
        "blocked" | "error" => State::Blocked,
        _ => {
            let seen = seen_turn(terminal).max(looked);
            let done = seen.map_or(terminal.turn > 0, |seen| terminal.turn > seen);
            if done { State::Done } else { State::Idle }
        }
    })
}

pub fn rollup(states: impl IntoIterator<Item = State>) -> Option<State> {
    states.into_iter().max_by_key(|s| s.priority())
}

pub fn label(terminal: &hc::Terminal) -> Option<String> {
    let status = terminal.status.as_ref()?;
    if !status.app.is_empty() {
        return Some(canonical(&status.app));
    }
    terminal
        .foreground
        .as_ref()
        .map(|f| f.name.clone())
        .filter(|name| !name.is_empty())
        .or_else(|| terminal.argv.first().map(|a| program_name(a)))
}

fn program_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

pub fn canonical(app: &str) -> String {
    match app {
        "claude-code" => "claude".to_owned(),
        "github-copilot" | "copilot-cli" => "copilot".to_owned(),
        "open-code" => "opencode".to_owned(),
        other => other.to_owned(),
    }
}

pub fn strip_activity(title: &str) -> &str {
    let mut chars = title.chars();
    let Some(first) = chars.next() else {
        return title;
    };
    let spinner = ('\u{2800}'..='\u{28FF}').contains(&first) || "·✢✳✶✻✽◐◓◑◒".contains(first);
    let rest = chars.as_str();
    if spinner && (rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        rest.trim_start()
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn agent(word: &str, turn: u64, seen: Option<&str>) -> hc::Terminal {
        hc::Terminal {
            status: Some(hc::Status {
                state: word.into(),
                app: "claude-code".into(),
                ..hc::Status::default()
            }),
            turn,
            labels: seen
                .map(|s| BTreeMap::from([(SEEN.to_owned(), s.to_owned())]))
                .unwrap_or_default(),
            ..hc::Terminal::default()
        }
    }

    #[test]
    fn states_follow_the_app() {
        assert_eq!(state(&hc::Terminal::default(), None), None);
        assert_eq!(
            state(&agent("working", 1, Some("0")), None),
            Some(State::Working)
        );
        assert_eq!(
            state(&agent("error", 1, Some("1")), None),
            Some(State::Blocked)
        );
        assert_eq!(
            state(&agent("blocked", 1, Some("1")), None),
            Some(State::Blocked)
        );
        assert_eq!(state(&agent("idle", 2, Some("1")), None), Some(State::Done));
        assert_eq!(state(&agent("done", 2, Some("2")), None), Some(State::Idle));
        assert_eq!(
            state(&agent("idle", 2, Some("1")), Some(2)),
            Some(State::Idle)
        );
        assert_eq!(state(&agent("idle", 0, None), None), Some(State::Idle));
        assert_eq!(state(&agent("idle", 3, None), None), Some(State::Done));
    }

    #[test]
    fn rollups_take_the_most_urgent() {
        assert_eq!(
            rollup([State::Idle, State::Done, State::Working]),
            Some(State::Done)
        );
        assert_eq!(
            rollup([State::Working, State::Blocked]),
            Some(State::Blocked)
        );
        assert_eq!(rollup([]), None);
    }

    #[test]
    fn labels_and_titles() {
        assert_eq!(label(&agent("idle", 0, None)).as_deref(), Some("claude"));
        assert_eq!(strip_activity("⠋ building"), "building");
        assert_eq!(strip_activity("✳ Claude"), "Claude");
        assert_eq!(strip_activity("plain"), "plain");
        assert_eq!(State::Done.icon(StatusIndicators::Symbols), "✓");
    }
}
