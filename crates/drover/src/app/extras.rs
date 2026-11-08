use std::collections::HashMap;
use std::time::{Duration, Instant};

use config::TabBarEntry;
use doc::Content;
use hc::ExecParams;

use super::App;
use crate::ids::qualify;
use crate::machines::SpaceRef;

const GIT_EVERY: Duration = Duration::from_secs(10);
const FILES_EVERY: Duration = Duration::from_secs(5);
const GIT_SCRIPT: &str = r#"cd "$1" 2>/dev/null || exit 1
git rev-parse --show-toplevel --abbrev-ref HEAD 2>/dev/null || exit 1
git rev-list --left-right --count 'HEAD...@{upstream}' 2>/dev/null || true"#;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Git {
    pub cwd: String,
    pub toplevel: Option<String>,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub at: Option<Instant>,
    pub pending: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    pub entries: Vec<String>,
    pub error: Option<String>,
    pub at: Option<Instant>,
    pub pending: bool,
}

#[derive(Debug, Default)]
pub struct Files {
    pub map: HashMap<(Option<String>, String), Listing>,
}

#[derive(Debug, Default)]
pub struct Status {
    pub commands: HashMap<usize, (String, Option<Instant>, bool)>,
}

fn parse_git(cwd: &str, out: &str) -> Git {
    let mut lines = out.lines();
    let toplevel = lines.next().map(str::to_owned).filter(|l| !l.is_empty());
    let branch = lines
        .next()
        .map(str::to_owned)
        .filter(|l| !l.is_empty() && l != "HEAD");
    let (ahead, behind) = lines
        .next()
        .and_then(|l| {
            let mut parts = l.split_whitespace();
            Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
        })
        .unwrap_or((0, 0));
    Git {
        cwd: cwd.to_owned(),
        toplevel,
        branch,
        ahead,
        behind,
        at: Some(Instant::now()),
        pending: false,
    }
}

pub fn parse_listing(out: &str) -> Vec<String> {
    let mut entries: Vec<String> = out
        .lines()
        .filter(|l| !l.is_empty() && *l != "./" && *l != "../")
        .map(str::to_owned)
        .collect();
    entries.sort_by(|a, b| {
        b.ends_with('/')
            .cmp(&a.ends_with('/'))
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    entries
}

impl App {
    pub fn refresh_extras(&mut self) {
        if self.client().is_none() {
            return;
        }
        self.refresh_git();
        self.refresh_files();
        self.refresh_status();
    }

    pub(crate) fn space_cwd(&self, key: &str) -> Option<(Option<String>, String)> {
        let space = self.machines.space(key)?;
        let machine = space.machine.map(str::to_owned);
        let cwd = space
            .space
            .first_terminal()
            .and_then(|n| n.address())
            .and_then(|a| self.terminals.get(&qualify(a, space.machine)))
            .and_then(|t| {
                t.foreground
                    .as_ref()
                    .map(|f| f.cwd.clone())
                    .filter(|c| !c.is_empty())
                    .or_else(|| Some(t.cwd.clone()).filter(|c| !c.is_empty()))
            })
            .unwrap_or_else(|| space.space.folder.clone());
        (!cwd.is_empty()).then_some((machine, cwd))
    }

    fn refresh_git(&mut self) {
        let keys: Vec<String> = self.machines.spaces().iter().map(SpaceRef::key).collect();
        self.git.retain(|key, _| keys.contains(key));
        for key in keys {
            let Some((machine, cwd)) = self.space_cwd(&key) else {
                continue;
            };
            let due = self.git.get(&key).is_none_or(|g| {
                !g.pending && (g.cwd != cwd || g.at.is_none_or(|at| at.elapsed() >= GIT_EVERY))
            });
            if !due {
                continue;
            }
            let Some(client) = self.client() else {
                return;
            };
            self.git.entry(key.clone()).or_default().pending = true;
            let params = ExecParams {
                argv: vec![
                    "sh".into(),
                    "-c".into(),
                    GIT_SCRIPT.into(),
                    "sh".into(),
                    cwd.clone(),
                ],
                on: machine,
                timeout: Some(Duration::from_secs(5)),
                ..ExecParams::default()
            };
            self.spawn(async move {
                let result = client.exec(&params).await;
                move |app: &mut App| {
                    let git = match result {
                        Ok(exec) if exec.success() => {
                            parse_git(&cwd, &String::from_utf8_lossy(&exec.stdout))
                        }
                        _ => Git {
                            cwd,
                            at: Some(Instant::now()),
                            ..Git::default()
                        },
                    };
                    app.git.insert(key, git);
                }
            });
        }
    }

    pub fn files_dir(&self, pane: &str) -> Option<(Option<String>, String)> {
        let found = self.machines.pane(pane)?;
        let machine = found.machine.map(str::to_owned);
        let focused = self
            .view
            .focused_in(&self.machines, &found.tab_key())
            .filter(|p| p != pane)
            .and_then(|p| self.live_cwd(&p));
        let dir = focused.unwrap_or_else(|| found.space.folder.clone());
        let dir = if dir.is_empty() && machine.is_none() {
            self.home()
        } else {
            dir
        };
        Some((machine, dir))
    }

    fn refresh_files(&mut self) {
        let Some(tab) = self.view.active_tab(&self.machines) else {
            return;
        };
        let Some(found) = self.machines.tab(&tab) else {
            return;
        };
        let keys: Vec<String> = found
            .panes()
            .filter(
                |p| matches!(p.node.content(), Some(Content::Mod { name, .. }) if name == "files"),
            )
            .map(|p| p.key())
            .collect();
        for key in keys {
            let Some(target) = self.files_dir(&key) else {
                continue;
            };
            let due =
                self.files.map.get(&target).is_none_or(|l| {
                    !l.pending && l.at.is_none_or(|at| at.elapsed() >= FILES_EVERY)
                });
            let Some(client) = self.client().filter(|_| due) else {
                continue;
            };
            self.files.map.entry(target.clone()).or_default().pending = true;
            let params = ExecParams {
                argv: vec!["ls".into(), "-1p".into(), "--".into(), target.1.clone()],
                on: target.0.clone(),
                timeout: Some(Duration::from_secs(5)),
                ..ExecParams::default()
            };
            self.spawn(async move {
                let result = client.exec(&params).await;
                move |app: &mut App| {
                    let listing = match result {
                        Ok(exec) if exec.success() => Listing {
                            entries: parse_listing(&String::from_utf8_lossy(&exec.stdout)),
                            at: Some(Instant::now()),
                            ..Listing::default()
                        },
                        Ok(exec) => Listing {
                            error: Some(String::from_utf8_lossy(&exec.stderr).trim().to_owned()),
                            at: Some(Instant::now()),
                            ..Listing::default()
                        },
                        Err(error) => Listing {
                            error: Some(error.to_string()),
                            at: Some(Instant::now()),
                            ..Listing::default()
                        },
                    };
                    app.files.map.insert(target, listing);
                }
            });
        }
    }

    fn refresh_status(&mut self) {
        let entries: Vec<(usize, String, u64, u64)> = self
            .config
            .ui
            .tab_bar_right
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| match entry {
                TabBarEntry::Command {
                    command,
                    interval_seconds,
                    timeout_seconds,
                } if entry.is_shown() => {
                    Some((i, command.clone(), *interval_seconds, *timeout_seconds))
                }
                _ => None,
            })
            .collect();
        let machine = self.active_machine();
        for (index, command, every, timeout) in entries {
            let due = self
                .status
                .commands
                .get(&index)
                .is_none_or(|(_, at, pending)| {
                    !pending && at.is_none_or(|at| at.elapsed() >= Duration::from_secs(every))
                });
            let Some(client) = self.client().filter(|_| due) else {
                continue;
            };
            let previous = self
                .status
                .commands
                .get(&index)
                .map(|c| c.0.clone())
                .unwrap_or_default();
            self.status.commands.insert(index, (previous, None, true));
            let params = ExecParams {
                argv: vec!["/bin/sh".into(), "-lc".into(), command],
                on: machine.clone(),
                timeout: Some(Duration::from_secs(timeout)),
                ..ExecParams::default()
            };
            self.spawn(async move {
                let result = client.exec(&params).await;
                move |app: &mut App| {
                    let text = match result {
                        Ok(exec) if exec.success() => {
                            last_line(&String::from_utf8_lossy(&exec.stdout))
                        }
                        _ => String::new(),
                    };
                    app.status
                        .commands
                        .insert(index, (text, Some(Instant::now()), false));
                }
            });
        }
    }
}

pub fn last_line(out: &str) -> String {
    let line = out
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    strip_controls(line).trim().to_owned()
}

pub fn strip_controls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']' | 'P' | '_' | '^') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if !ch.is_control() {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_output_parses_branch_and_counts() {
        let git = parse_git("/w", "/w/repo\nmain\n2\t1\n");
        assert_eq!(git.toplevel.as_deref(), Some("/w/repo"));
        assert_eq!(git.branch.as_deref(), Some("main"));
        assert_eq!((git.ahead, git.behind), (2, 1));
        let detached = parse_git("/w", "/w/repo\nHEAD\n");
        assert_eq!(detached.branch, None);
    }

    #[test]
    fn listings_put_folders_first() {
        assert_eq!(
            parse_listing("b.txt\nsrc/\nA.md\nlib/\n"),
            ["lib/", "src/", "A.md", "b.txt"]
        );
    }

    #[test]
    fn status_lines_lose_their_escapes() {
        assert_eq!(last_line("one\n\x1b[31mtwo\x1b[0m\n\n"), "two");
        assert_eq!(strip_controls("\x1b]0;t\x07ok"), "ok");
    }
}
