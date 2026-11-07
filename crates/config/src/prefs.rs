use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::AgentPanelSort;
use crate::write::write_atomic;

pub const DEFAULT_SECTION_SPLIT: f32 = 0.5;
const SECTION_SPLIT_RANGE: (f32, f32) = (0.1, 0.9);

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidebar_width: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidebar_section_split: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidebar_collapsed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_panel_sort: Option<AgentPanelSort>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collapsed_groups: Vec<String>,
}

impl Preferences {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        write_atomic(path, &bytes)
    }

    pub fn section_split(&self) -> f32 {
        self.sidebar_section_split
            .filter(|split| split.is_finite())
            .map_or(DEFAULT_SECTION_SPLIT, |split| {
                split.clamp(SECTION_SPLIT_RANGE.0, SECTION_SPLIT_RANGE.1)
            })
    }
}
