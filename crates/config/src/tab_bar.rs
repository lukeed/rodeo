use serde::Deserialize;
use time::format_description::OwnedFormatItem;

pub const MAX_ENTRIES: usize = 16;
pub const MAX_COMMAND_INTERVAL_SECONDS: u64 = 31_536_000;
pub const MAX_COMMAND_TIMEOUT_SECONDS: u64 = 3_600;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum TabBarEntry {
    Zoom,
    Hostname,
    Datetime {
        #[serde(default = "default_format")]
        format: String,
    },
    Text {
        text: String,
    },
    Command {
        command: String,
        #[serde(default = "default_interval")]
        interval_seconds: u64,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
}

fn default_format() -> String {
    "%H:%M".into()
}

fn default_interval() -> u64 {
    5
}

fn default_timeout() -> u64 {
    2
}

impl TabBarEntry {
    pub fn problems(&self) -> Vec<String> {
        match self {
            Self::Datetime { format } if format.is_empty() => {
                vec!["datetime format is empty".into()]
            }
            Self::Datetime { format } => parse_datetime_format(format)
                .err()
                .map(|error| format!("has {error}"))
                .into_iter()
                .collect(),
            Self::Command {
                command,
                interval_seconds,
                timeout_seconds,
            } => {
                let mut problems = Vec::new();
                if command.trim().is_empty() {
                    problems.push("command is empty".into());
                }
                if *interval_seconds == 0 {
                    problems.push("interval_seconds must be at least 1".into());
                }
                if *interval_seconds > MAX_COMMAND_INTERVAL_SECONDS {
                    problems.push(format!(
                        "interval_seconds may be at most {MAX_COMMAND_INTERVAL_SECONDS}"
                    ));
                }
                if *timeout_seconds == 0 {
                    problems.push("timeout_seconds must be at least 1".into());
                }
                if *timeout_seconds > MAX_COMMAND_TIMEOUT_SECONDS {
                    problems.push(format!(
                        "timeout_seconds may be at most {MAX_COMMAND_TIMEOUT_SECONDS}"
                    ));
                }
                problems
            }
            Self::Zoom | Self::Hostname | Self::Text { .. } => Vec::new(),
        }
    }

    pub fn is_shown(&self) -> bool {
        self.problems().is_empty()
    }
}

pub fn parse_datetime_format(value: &str) -> Result<OwnedFormatItem, String> {
    if value.is_empty() {
        return Err("datetime format is empty".into());
    }
    let format = time::format_description::parse_strftime_owned(value)
        .map_err(|error| format!("invalid datetime format: {error}"))?;
    time::PrimitiveDateTime::MIN
        .format(&format)
        .map_err(|error| format!("unsupported datetime format: {error}"))?;
    Ok(format)
}

pub fn shown_entries(entries: &[TabBarEntry]) -> impl Iterator<Item = &TabBarEntry> {
    entries
        .iter()
        .take(MAX_ENTRIES)
        .filter(|entry| entry.is_shown())
}

pub fn tab_bar_diagnostics(entries: &[TabBarEntry]) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if entries.len() > MAX_ENTRIES {
        diagnostics.push(format!(
            "ui.tab_bar_right may contain at most {MAX_ENTRIES} entries; ignoring extras"
        ));
    }
    for (index, entry) in entries.iter().enumerate().take(MAX_ENTRIES) {
        for problem in entry.problems() {
            diagnostics.push(format!("ui.tab_bar_right[{index}] {problem}; hiding entry"));
        }
    }
    diagnostics
}
