use crate::schema::{Document, Space};

pub fn folder_name(path: &str, home: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    let home = home.trim_end_matches(['/', '\\']);
    let is_home = if cfg!(windows) {
        trimmed.eq_ignore_ascii_case(home)
    } else {
        trimmed == home
    };
    if path.is_empty() || is_home {
        return "~".to_owned();
    }
    if trimmed.is_empty() {
        return path[..1].to_owned();
    }
    trimmed
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(trimmed)
        .to_owned()
}

pub fn where_label(space: &str, tab: &str) -> String {
    format!("{space} / {tab}")
}

impl Space {
    pub fn display_name(&self, resolved_folder: Option<&str>, home: &str) -> String {
        if self.name.is_empty() {
            folder_name(resolved_folder.unwrap_or(&self.folder), home)
        } else {
            self.name.clone()
        }
    }

    pub fn tab_name(&self, tab: &str) -> Option<String> {
        let i = self.tab_index(tab)?;
        let name = &self.tabs[i].name;
        Some(if name.is_empty() {
            (i + 1).to_string()
        } else {
            name.clone()
        })
    }
}

impl Document {
    pub fn where_label(
        &self,
        tab: &str,
        resolved_folder: Option<&str>,
        home: &str,
    ) -> Option<String> {
        let space = self.spaces.iter().find(|s| s.tab(tab).is_some())?;
        Some(where_label(
            &space.display_name(resolved_folder, home),
            &space.tab_name(tab)?,
        ))
    }
}
