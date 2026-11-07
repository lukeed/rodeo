use std::io;
use std::path::Path;

use serde::de::DeserializeOwned;
use tracing::warn;

use crate::keymap::{Keymap, KeymapBuild};
use crate::model::Config;
use crate::tab_bar::tab_bar_diagnostics;
use crate::theme::{Appearance, Palette};
use crate::window_title::{WindowTitle, window_title_diagnostic};

pub const CHECK_COMMAND: &str = "drover config check";

const SECTIONS: &[&str] = &[
    "advanced",
    "experimental",
    "keys",
    "onboarding",
    "remote",
    "server",
    "session",
    "terminal",
    "theme",
    "ui",
    "update",
    "worktrees",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub config: Config,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadStatus {
    Applied,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reload {
    pub config: Config,
    pub diagnostics: Vec<String>,
    pub status: ReloadStatus,
}

pub fn strip_bom(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut text = text.to_string();
    if !text.contains('\u{feff}') {
        return text;
    }
    while let Err(error) = text.parse::<toml::Table>() {
        let Some(span) = error.span() else {
            break;
        };
        if text.get(span.clone()) != Some("\u{feff}") {
            break;
        }
        text.replace_range(span, "");
    }
    text
}

pub fn read_config(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(strip_bom(&text))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn check(path: &Path) -> Result<(), Vec<String>> {
    let diagnostics = Config::load(path).diagnostics;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn summary(diagnostics: &[String], path: &Path) -> Option<String> {
    if diagnostics.is_empty() {
        return None;
    }
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    let unreadable = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("config read error:"));
    let any = |needle: &str| {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains(needle))
    };
    let impact = if any("using defaults") {
        if unreadable {
            " unreadable; using defaults"
        } else {
            " invalid; using defaults"
        }
    } else if any("keeping current config") {
        if unreadable {
            " unreadable; keeping current config"
        } else {
            " invalid; keeping current config"
        }
    } else if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.starts_with("unknown config key "))
    {
        " has unknown keys"
    } else {
        ""
    };
    Some(format!("{file}{impact}; {CHECK_COMMAND}"))
}

fn note_all(diagnostics: &[String]) {
    for diagnostic in diagnostics {
        warn!(message = %diagnostic, "config diagnostic");
    }
}

impl Config {
    pub fn load(path: &Path) -> Loaded {
        let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let loaded = match read_config(path) {
            Ok(Some(text)) => Self::parse(&text, config_dir),
            Ok(None) => Loaded {
                config: Self::default(),
                diagnostics: Vec::new(),
            },
            Err(error) => Loaded {
                config: Self::default(),
                diagnostics: vec![format!("config read error: {error}; using defaults")],
            },
        };
        note_all(&loaded.diagnostics);
        loaded
    }

    pub fn parse(text: &str, config_dir: &Path) -> Loaded {
        let parsed = toml::Deserializer::parse(text).and_then(with_ignored::<Self, _>);
        let (config, ignored) = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                return Loaded {
                    config: Self::default(),
                    diagnostics: vec![format!("config parse error: {error}; using defaults")],
                };
            }
        };
        let table = text.parse::<toml::Table>().unwrap_or_default();
        let unknown: Vec<&String> = table
            .iter()
            .filter(|(key, value)| unknown_section(key, value).is_some())
            .map(|(key, _)| key)
            .collect();
        let mut diagnostics: Vec<String> = table
            .iter()
            .filter_map(|(key, value)| unknown_section(key, value))
            .collect();
        let ignored = ignored
            .into_iter()
            .filter(
                |path| !matches!(path.as_slice(), [Segment::Key(key)] if unknown.contains(&key)),
            )
            .collect();
        diagnostics.extend(unknown_keys(ignored, None));
        diagnostics.extend(config.diagnostics(config_dir));
        Loaded {
            config,
            diagnostics,
        }
    }

    pub fn diagnostics(&self, config_dir: &Path) -> Vec<String> {
        let mut diagnostics = self.keymap().all_diagnostics();
        diagnostics.extend(self.theme.diagnostics());
        diagnostics.extend(self.ui.sound.diagnostics(config_dir));
        diagnostics.extend(tab_bar_diagnostics(&self.ui.tab_bar_right));
        diagnostics.extend(window_title_diagnostic(&self.ui.window_title));
        diagnostics.extend(self.ui.sidebar_bounds_diagnostic());
        diagnostics
    }

    pub fn keymap(&self) -> KeymapBuild {
        Keymap::build(&self.keys)
    }

    pub fn palette(&self, appearance: Option<Appearance>) -> Palette {
        self.theme.palette(Some(&self.ui.accent), appearance)
    }

    pub fn window_title(&self) -> Option<WindowTitle> {
        WindowTitle::parse(&self.ui.window_title).ok().flatten()
    }

    pub fn reload(&self, path: &Path) -> Reload {
        let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let reload = match read_config(path) {
            Ok(Some(text)) => self.reload_text(&text, config_dir),
            Ok(None) => Reload {
                config: Self::default(),
                diagnostics: Vec::new(),
                status: ReloadStatus::Applied,
            },
            Err(error) => Reload {
                config: self.clone(),
                diagnostics: vec![format!(
                    "config read error: {error}; keeping current config"
                )],
                status: ReloadStatus::Failed,
            },
        };
        note_all(&reload.diagnostics);
        reload
    }

    pub fn reload_text(&self, text: &str, config_dir: &Path) -> Reload {
        let table = match text.parse::<toml::Table>() {
            Ok(table) => table,
            Err(error) => {
                return Reload {
                    config: self.clone(),
                    diagnostics: vec![format!(
                        "config parse error: {error}; keeping current config"
                    )],
                    status: ReloadStatus::Failed,
                };
            }
        };
        let mut sections = Sections {
            table: &table,
            diagnostics: table
                .iter()
                .filter_map(|(key, value)| unknown_section(key, value))
                .collect(),
            invalid: Vec::new(),
        };
        let stray_keys = table
            .iter()
            .filter(|(key, value)| {
                !SECTIONS.contains(&key.as_str()) && unknown_section(key, value).is_none()
            })
            .map(|(key, _)| vec![Segment::Key(key.clone())])
            .collect();
        sections.diagnostics.extend(unknown_keys(stray_keys, None));

        let next = self.read_sections(&mut sections);
        let invalid = sections.invalid;
        self.settle(next, sections.diagnostics, &invalid, config_dir)
    }

    fn read_sections(&self, sections: &mut Sections) -> Self {
        let mut next = Self::default();
        if let Some(value) = sections.table.get("onboarding") {
            match value.clone().try_into::<Option<bool>>() {
                Ok(onboarding) => next.onboarding = onboarding,
                Err(error) => {
                    next.onboarding = self.onboarding;
                    sections.diagnostics.push(format!(
                        "invalid onboarding setting: {error}; keeping current onboarding state"
                    ));
                }
            }
        }
        next.theme = sections.take("theme", "theme config", &self.theme);
        next.keys = sections.take("keys", "keybinding config", &self.keys);
        next.terminal = sections.take("terminal", "terminal config", &self.terminal);
        next.session = sections.take("session", "session config", &self.session);
        next.server = sections.take("server", "server config", &self.server);
        next.update = sections.take("update", "update config", &self.update);
        next.ui = sections.take("ui", "ui config", &self.ui);
        next.advanced = sections.take("advanced", "advanced config", &self.advanced);
        next.worktrees = sections.take("worktrees", "worktree config", &self.worktrees);
        next.experimental =
            sections.take("experimental", "experimental config", &self.experimental);
        next.remote = sections.take("remote", "remote config", &self.remote);
        next
    }

    fn settle(
        &self,
        mut next: Self,
        mut diagnostics: Vec<String>,
        invalid: &[&str],
        config_dir: &Path,
    ) -> Reload {
        let invalid = |section: &str| invalid.contains(&section);
        if !invalid("theme") {
            diagnostics.extend(next.theme.diagnostics());
        }
        if !invalid("keys") {
            let build = next.keymap();
            if build.prefix_error.is_some() {
                next.keys = self.keys.clone();
                diagnostics.extend(
                    build
                        .all_diagnostics()
                        .into_iter()
                        .map(|diagnostic| format!("{diagnostic}; kept current keybinds")),
                );
            } else {
                diagnostics.extend(build.diagnostics);
            }
        }
        if !invalid("ui") {
            if let Some(diagnostic) = next.ui.sidebar_bounds_diagnostic() {
                next.ui = self.ui.clone();
                diagnostics.push(format!("{diagnostic}; keeping previous [ui] settings"));
            } else {
                diagnostics.extend(next.ui.sound.diagnostics(config_dir));
                diagnostics.extend(tab_bar_diagnostics(&next.ui.tab_bar_right));
                diagnostics.extend(window_title_diagnostic(&next.ui.window_title));
            }
        }
        let graphics_valid = !invalid("terminal")
            && (next.terminal.kitty_graphics.is_some() || !invalid("experimental"));
        if graphics_valid && next.kitty_graphics() != self.kitty_graphics() {
            next.terminal.kitty_graphics = self.terminal.kitty_graphics;
            next.experimental.kitty_graphics = self.experimental.kitty_graphics;
            diagnostics.push(
                "terminal.kitty_graphics changes require restarting drover; kept current setting"
                    .into(),
            );
        }
        let status = if diagnostics.is_empty() {
            ReloadStatus::Applied
        } else {
            ReloadStatus::Partial
        };
        Reload {
            config: next,
            diagnostics,
            status,
        }
    }
}

struct Sections<'a> {
    table: &'a toml::Table,
    diagnostics: Vec<String>,
    invalid: Vec<&'static str>,
}

impl Sections<'_> {
    fn take<T: DeserializeOwned + Default + Clone>(
        &mut self,
        section: &'static str,
        label: &str,
        current: &T,
    ) -> T {
        let Some(value) = self.table.get(section) else {
            return T::default();
        };
        match with_ignored::<T, _>(value.clone()) {
            Ok((parsed, ignored)) => {
                self.diagnostics
                    .extend(unknown_keys(ignored, Some(section)));
                parsed
            }
            Err(error) => {
                self.diagnostics.push(format!(
                    "invalid {label}: {error}; keeping current {section} settings"
                ));
                self.invalid.push(section);
                current.clone()
            }
        }
    }
}

fn unknown_section(key: &str, value: &toml::Value) -> Option<String> {
    if SECTIONS.contains(&key) {
        return None;
    }
    let header = match value {
        toml::Value::Table(_) => format!("[{key}]"),
        toml::Value::Array(items)
            if !items.is_empty() && items.iter().all(toml::Value::is_table) =>
        {
            format!("[[{key}]]")
        }
        _ => return None,
    };
    Some(if key == "toast" {
        format!("unknown config section {header}; did you mean [ui.toast]? ignoring section")
    } else {
        format!("unknown config section {header}; ignoring section")
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Segment {
    Key(String),
    Index(usize),
}

fn segments(path: &serde_ignored::Path<'_>, out: &mut Vec<Segment>) {
    match path {
        serde_ignored::Path::Root => {}
        serde_ignored::Path::Seq { parent, index } => {
            segments(parent, out);
            out.push(Segment::Index(*index));
        }
        serde_ignored::Path::Map { parent, key } => {
            segments(parent, out);
            out.push(Segment::Key(key.clone()));
        }
        serde_ignored::Path::Some { parent }
        | serde_ignored::Path::NewtypeStruct { parent }
        | serde_ignored::Path::NewtypeVariant { parent } => segments(parent, out),
    }
}

fn with_ignored<'de, T, D>(deserializer: D) -> Result<(T, Vec<Vec<Segment>>), D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let mut ignored = Vec::new();
    let value = serde_ignored::deserialize(deserializer, |path| {
        let mut out = Vec::new();
        segments(&path, &mut out);
        ignored.push(out);
    })?;
    Ok((value, ignored))
}

fn unknown_keys(paths: Vec<Vec<Segment>>, section: Option<&str>) -> Vec<String> {
    let mut paths: Vec<Vec<Segment>> = paths
        .into_iter()
        .map(|mut path| {
            if let Some(section) = section {
                path.insert(0, Segment::Key(section.into()));
            }
            path
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
        .iter()
        .map(|path| format!("unknown config key {}; ignoring key", dotted(path)))
        .collect()
}

fn dotted(path: &[Segment]) -> String {
    path.iter()
        .map(|segment| match segment {
            Segment::Key(key)
                if !key.is_empty()
                    && key.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
                    }) =>
            {
                key.clone()
            }
            Segment::Key(key) => toml::Value::String(key.clone()).to_string(),
            Segment::Index(index) => index.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}
