use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::App;
use crate::view::View;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resume {
    pub space: Option<String>,
    pub tabs: HashMap<String, String>,
    pub focus: HashMap<String, String>,
    pub zoomed: Vec<String>,
}

impl Resume {
    pub fn path(prefs: &Path) -> PathBuf {
        prefs.with_extension("view.json")
    }

    pub fn of(view: &View) -> Self {
        let mut zoomed: Vec<String> = view.zoomed.iter().cloned().collect();
        zoomed.sort();
        Self {
            space: view.space.clone(),
            tabs: view.tabs.clone(),
            focus: view.focus.clone(),
            zoomed,
        }
    }

    pub fn load(path: &Path) -> Option<Self> {
        let value: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        let map = |key: &str| -> HashMap<String, String> {
            value[key]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                .collect()
        };
        Some(Self {
            space: value["space"].as_str().map(str::to_owned),
            tabs: map("tabs"),
            focus: map("focus"),
            zoomed: value["zoomed"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        })
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let object = |map: &HashMap<String, String>| -> Map<String, Value> {
            map.iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect()
        };
        let value = json!({
            "space": self.space,
            "tabs": object(&self.tabs),
            "focus": object(&self.focus),
            "zoomed": self.zoomed,
        });
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(&value)?)?;
        std::fs::rename(temp, path)
    }

    pub fn apply(self, view: &mut View) {
        if self.space.is_some() {
            view.space = self.space;
        }
        view.tabs.extend(self.tabs);
        view.focus.extend(self.focus);
        view.zoomed.extend(self.zoomed);
    }
}

impl App {
    pub(super) fn resume_view(&mut self) {
        if !self.machines.local().slot.is_ready() {
            return;
        }
        if let Some(resume) = self.resume.take() {
            resume.apply(&mut self.view);
        }
    }

    pub(super) fn save_view(&self) {
        let path = Resume::path(&self.prefs_path);
        if let Err(error) = Resume::of(&self.view).save(&path) {
            tracing::warn!(%error, path = %path.display(), "cannot save the view");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_round_trips_through_its_file() {
        let dir = std::env::temp_dir().join(format!("rodeo-resume-{}", std::process::id()));
        let path = Resume::path(&dir.join("default.json"));
        let resume = Resume {
            space: Some("sp1".into()),
            tabs: HashMap::from([("sp1".into(), "tb2".into())]),
            focus: HashMap::from([("tb2".into(), "p3".into())]),
            zoomed: vec!["tb2".into()],
        };
        resume.save(&path).unwrap();
        assert_eq!(path.file_name().unwrap(), "default.view.json");
        assert_eq!(Resume::load(&path), Some(resume.clone()));
        let mut view = View::default();
        resume.apply(&mut view);
        assert_eq!(view.space.as_deref(), Some("sp1"));
        assert!(view.zoomed.contains("tb2"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
