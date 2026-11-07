use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

pub const CONFIG_PATH_ENV: &str = "DROVER_CONFIG_PATH";
pub const HERDR_CONFIG_PATH_ENV: &str = "HERDR_CONFIG_PATH";
pub const APP_DIR: &str = "drover";
const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
    pub herdr_config: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub from: PathBuf,
    pub to: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Self {
        let var = |name: &str| {
            lookup(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        let home = var("HOME");
        let config_home =
            var("XDG_CONFIG_HOME").or_else(|| home.as_ref().map(|home| home.join(".config")));
        let config_dir = |app: &str| {
            config_home
                .as_ref()
                .map_or_else(|| std::env::temp_dir().join(app), |base| base.join(app))
        };
        let state = var("XDG_STATE_HOME")
            .map(|base| base.join(APP_DIR))
            .or_else(|| {
                home.as_ref()
                    .map(|home| home.join(".local/state").join(APP_DIR))
            })
            .unwrap_or_else(|| std::env::temp_dir().join(format!("{APP_DIR}-state")));
        Self {
            config: var(CONFIG_PATH_ENV).unwrap_or_else(|| config_dir(APP_DIR).join(CONFIG_FILE)),
            state,
            herdr_config: var(HERDR_CONFIG_PATH_ENV)
                .unwrap_or_else(|| config_dir("herdr").join(CONFIG_FILE)),
        }
    }

    pub fn config_dir(&self) -> &Path {
        self.config.parent().unwrap_or_else(|| Path::new("."))
    }

    pub fn preferences(&self, session: &str) -> PathBuf {
        let name: String = session
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let name = if name.is_empty() || name.chars().all(|ch| ch == '.') {
            "default".to_string()
        } else {
            name
        };
        self.state.join("preferences").join(format!("{name}.json"))
    }

    pub fn migrate(&self) -> io::Result<Option<Migration>> {
        if self.config.exists() || !self.herdr_config.is_file() {
            return Ok(None);
        }
        if let Some(parent) = self.config.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = std::fs::read(&self.herdr_config)?;
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.config)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(None),
            Err(error) => return Err(error),
        };
        let written = io::Write::write_all(&mut file, &content).and_then(|()| file.sync_all());
        if let Err(error) = written {
            drop(file);
            let _ = std::fs::remove_file(&self.config);
            return Err(error);
        }
        Ok(Some(Migration {
            from: self.herdr_config.clone(),
            to: self.config.clone(),
        }))
    }
}
