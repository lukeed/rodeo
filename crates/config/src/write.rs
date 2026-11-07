use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

use crate::load::read_config;
use crate::model::{StatusIndicators, ToastDelivery};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Theme(String),
    StatusIndicators(StatusIndicators),
    Sound(bool),
    ToastDelivery(ToastDelivery),
}

#[derive(Debug)]
pub enum WriteError {
    Io(io::Error),
    Toml(toml_edit::TomlError),
    Shape(String),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Toml(error) => write!(f, "config.toml is not valid TOML: {error}"),
            Self::Shape(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WriteError {}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<toml_edit::TomlError> for WriteError {
    fn from(error: toml_edit::TomlError) -> Self {
        Self::Toml(error)
    }
}

impl Edit {
    pub fn description(&self) -> &'static str {
        match self {
            Self::Theme(_) => "theme",
            Self::StatusIndicators(_) => "status indicators",
            Self::Sound(_) => "sound setting",
            Self::ToastDelivery(_) => "toast setting",
        }
    }

    pub fn apply(&self, text: &str) -> Result<String, WriteError> {
        let mut doc = text.parse::<DocumentMut>()?;
        match self {
            Self::Theme(name) => {
                let theme = section(&mut doc, &["theme"])?;
                set(theme, "name", name.as_str().into());
                set(theme, "auto_switch", false.into());
            }
            Self::StatusIndicators(style) => {
                set(
                    section(&mut doc, &["ui"])?,
                    "status_indicators",
                    style.as_str().into(),
                );
            }
            Self::Sound(enabled) => {
                set(
                    section(&mut doc, &["ui", "sound"])?,
                    "enabled",
                    (*enabled).into(),
                );
            }
            Self::ToastDelivery(delivery) => {
                let toast = section(&mut doc, &["ui", "toast"])?;
                set(toast, "delivery", delivery.as_str().into());
                toast.remove("enabled");
            }
        }
        Ok(doc.to_string())
    }

    pub fn write(&self, path: &Path) -> Result<(), WriteError> {
        let text = read_config(path)?.unwrap_or_default();
        let updated = self.apply(&text)?;
        write_atomic(path, updated.as_bytes())?;
        Ok(())
    }
}

fn set(table: &mut dyn TableLike, key: &str, new: Value) {
    match table.get_mut(key).and_then(Item::as_value_mut) {
        Some(old) => {
            let decor = old.decor().clone();
            *old = new;
            *old.decor_mut() = decor;
        }
        None => {
            table.insert(key, Item::Value(new));
        }
    }
}

fn section<'a>(
    doc: &'a mut DocumentMut,
    path: &[&str],
) -> Result<&'a mut dyn TableLike, WriteError> {
    let comments = doc
        .as_table()
        .is_empty()
        .then(|| doc.trailing().as_str().unwrap_or_default().to_string())
        .filter(|comments| !comments.is_empty());
    if comments.is_some() {
        doc.set_trailing("");
    }
    let mut current: &mut dyn TableLike = doc.as_table_mut();
    for (depth, key) in path.iter().enumerate() {
        let leaf = depth + 1 == path.len();
        if !current.contains_key(key) {
            let mut table = Table::new();
            table.set_implicit(!leaf);
            if let Some(comments) = comments.as_deref().filter(|_| leaf) {
                table.decor_mut().set_prefix(comments);
            }
            current.insert(key, Item::Table(table));
        }
        let item = current.get_mut(key).expect("section was inserted above");
        current = item.as_table_like_mut().ok_or_else(|| {
            WriteError::Shape(format!(
                "config.toml has `{}` set to a value that is not a table",
                path[..=depth].join(".")
            ))
        })?;
    }
    Ok(current)
}

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let name = path.file_name().map_or_else(
        || "config".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(".{name}.tmp-{}-{sequence}", std::process::id()));
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetKeys {
    NoFile,
    NoKeys,
    Reset { backup: PathBuf },
}

pub fn reset_keys(path: &Path) -> Result<ResetKeys, WriteError> {
    let Some(text) = read_config(path)? else {
        return Ok(ResetKeys::NoFile);
    };
    let mut doc = text.parse::<DocumentMut>()?;
    if doc.as_table_mut().remove("keys").is_none() {
        return Ok(ResetKeys::NoKeys);
    }
    let backup = create_backup(path)?;
    write_atomic(path, doc.to_string().as_bytes())?;
    Ok(ResetKeys::Reset { backup })
}

fn create_backup(path: &Path) -> io::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let name = path.file_name().map_or_else(
        || "config.toml".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let bytes = std::fs::read(path)?;
    for attempt in 0u32.. {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let backup = path.with_file_name(format!("{name}.bak-keys-{stamp}{suffix}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
        {
            Ok(mut file) => {
                io::Write::write_all(&mut file, &bytes)?;
                return Ok(backup);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    unreachable!("backup names are unbounded")
}
