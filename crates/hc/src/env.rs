use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env {
    vars: BTreeMap<OsString, OsString>,
}

const OUTSIDE: [&str; 2] = ["HC_TERMINAL_ID", "HC_TERMINAL_SOCKET"];

impl Env {
    pub fn current() -> Self {
        Self {
            vars: std::env::vars_os().collect(),
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.set(key, value);
        self
    }

    pub fn set(&mut self, key: impl Into<OsString>, value: impl Into<OsString>) {
        self.vars.insert(key.into(), value.into());
    }

    pub fn remove(&mut self, key: impl AsRef<OsStr>) {
        self.vars.remove(key.as_ref());
    }

    pub fn get(&self, key: impl AsRef<OsStr>) -> Option<&OsStr> {
        self.vars.get(key.as_ref()).map(OsString::as_os_str)
    }

    pub(crate) fn nonempty(&self, key: &str) -> Option<&OsStr> {
        self.get(key).filter(|value| !value.is_empty())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars
            .iter()
            .map(|(key, value)| (key.as_os_str(), value.as_os_str()))
    }

    pub fn home(&self) -> PathBuf {
        self.get("HOME").map_or_else(|| "/".into(), PathBuf::from)
    }

    pub fn command(&self, program: impl AsRef<OsStr>) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(program);
        command.env_clear().envs(
            self.iter()
                .filter(|(key, _)| !OUTSIDE.iter().any(|outside| key == outside)),
        );
        command
    }
}

impl<K: Into<OsString>, V: Into<OsString>> FromIterator<(K, V)> for Env {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            vars: iter
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

pub(crate) fn path_dirs(env: &Env) -> Vec<PathBuf> {
    env.get("PATH")
        .map(|path| std::env::split_paths(path).collect())
        .unwrap_or_default()
}

pub(crate) fn usual_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        home.join(".local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join(".local/share/mise/shims"),
        home.join(".nix-profile/bin"),
        PathBuf::from("/run/current-system/sw/bin"),
        home.join(".cargo/bin"),
    ];
    if cfg!(target_os = "macos") {
        dirs.push(home.join(".local/share/herdr-core/current/Contents/MacOS"));
        dirs.push(home.join(".local/share/herdr-core/Herdr Core.app/Contents/MacOS"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_get_exactly_this_env_minus_the_callers_terminal() {
        let env = Env::new()
            .with("HOME", "/h")
            .with("HC_TERMINAL_ID", "t3")
            .with("HC_TERMINAL_SOCKET", "/s");
        let command = env.command("true");
        let set: Vec<_> = command.as_std().get_envs().collect();
        assert!(set.contains(&(OsStr::new("HOME"), Some(OsStr::new("/h")))));
        assert!(
            set.iter()
                .all(|(key, _)| !key.to_string_lossy().starts_with("HC_TERMINAL"))
        );
    }

    #[test]
    fn home_defaults_to_root() {
        assert_eq!(Env::new().home(), PathBuf::from("/"));
        assert_eq!(Env::new().with("HOME", "/u").home(), PathBuf::from("/u"));
    }
}
