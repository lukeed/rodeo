use serde::{Deserialize, Deserializer, de};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Binding {
    One(String),
    Many(Vec<String>),
}

impl Default for Binding {
    fn default() -> Self {
        Self::One(String::new())
    }
}

impl Binding {
    pub fn values(&self) -> &[String] {
        match self {
            Self::One(value) => std::slice::from_ref(value),
            Self::Many(values) => values,
        }
    }

    pub fn is_set(&self) -> bool {
        self.values().iter().any(|value| !value.trim().is_empty())
    }
}

impl From<&str> for Binding {
    fn from(value: &str) -> Self {
        Self::One(value.into())
    }
}

macro_rules! keys_config {
    ($($field:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
        #[serde(default)]
        pub struct KeysConfig {
            pub prefix: Option<Binding>,
            pub extra_prefixes: Option<Binding>,
            pub remote_image_paste: Option<String>,
            $(pub $field: Option<Binding>,)+
            #[serde(alias = "fullscreen")]
            pub zoom: Option<Binding>,
            pub indexed: Option<IndexedKeys>,
            pub command: Vec<CommandConfig>,
        }

        impl KeysConfig {
            pub fn binding(&self, field: &str) -> Option<&Binding> {
                match field {
                    $(stringify!($field) => self.$field.as_ref(),)+
                    "zoom" => self.zoom.as_ref(),
                    _ => None,
                }
            }
        }
    };
}

keys_config!(
    help,
    settings,
    machines,
    new_workspace,
    new_worktree,
    open_worktree,
    remove_worktree,
    rename_workspace,
    close_workspace,
    workspace_picker,
    goto,
    navigate_workspace_up,
    navigate_workspace_down,
    navigate_pane_left,
    navigate_pane_down,
    navigate_pane_up,
    navigate_pane_right,
    detach,
    reload_config,
    open_notification_target,
    previous_workspace,
    next_workspace,
    previous_agent,
    next_agent,
    focus_agent,
    new_tab,
    rename_tab,
    previous_tab,
    next_tab,
    move_tab_previous,
    move_tab_next,
    switch_tab,
    switch_workspace,
    close_tab,
    rename_pane,
    edit_scrollback,
    clear_pane,
    copy_mode,
    focus_pane_left,
    focus_pane_down,
    focus_pane_up,
    focus_pane_right,
    swap_pane_left,
    swap_pane_down,
    swap_pane_up,
    swap_pane_right,
    cycle_pane_next,
    cycle_pane_previous,
    last_pane,
    split_vertical,
    split_horizontal,
    close_pane,
    resize_mode,
    resize_pane_left,
    resize_pane_down,
    resize_pane_up,
    resize_pane_right,
    toggle_sidebar,
);

pub const DEFAULT_PREFIX: &str = "ctrl+b";
pub const DEFAULT_REMOTE_IMAGE_PASTE: &str = "ctrl+v";

impl KeysConfig {
    pub fn remote_image_paste(&self) -> &str {
        self.remote_image_paste
            .as_deref()
            .unwrap_or(DEFAULT_REMOTE_IMAGE_PASTE)
    }

    pub fn indexed(&self) -> IndexedKeys {
        self.indexed.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct IndexedKeys {
    pub tabs: String,
    pub workspaces: String,
    pub agents: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct CommandConfig {
    pub key: Binding,
    pub command: String,
    #[serde(rename = "type")]
    pub kind: CommandKind,
    pub description: Option<String>,
    pub width: Option<PopupSize>,
    pub height: Option<PopupSize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    #[default]
    Shell,
    Pane,
    Popup,
    PluginAction,
}

impl CommandKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Pane => "pane",
            Self::Popup => "popup",
            Self::PluginAction => "plugin_action",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupSize {
    Cells(u16),
    Percent(u8),
}

impl PopupSize {
    pub fn resolve(self, available: u16) -> u16 {
        match self {
            Self::Cells(cells) => cells,
            Self::Percent(percent) => {
                u16::try_from(u32::from(available) * u32::from(percent) / 100).unwrap_or(u16::MAX)
            }
        }
    }

    pub fn parse_percent(value: &str) -> Result<Self, String> {
        let Some(number) = value.strip_suffix('%') else {
            return Err("string sizes must be percentages like 80%; use a number for cells".into());
        };
        let percent = number
            .parse::<u8>()
            .map_err(|_| "must be a number of cells or a percentage like 80%".to_string())?;
        if !(1..=100).contains(&percent) {
            return Err("percentage must be between 1% and 100%".into());
        }
        Ok(Self::Percent(percent))
    }
}

impl<'de> Deserialize<'de> for PopupSize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = PopupSize;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a cell count or percentage string like 80%")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<PopupSize, E> {
                u16::try_from(value)
                    .map(PopupSize::Cells)
                    .map_err(|_| E::custom("cell count must fit in u16"))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<PopupSize, E> {
                u16::try_from(value)
                    .map(PopupSize::Cells)
                    .map_err(|_| E::custom("cell count must be between 0 and 65535"))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<PopupSize, E> {
                PopupSize::parse_percent(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}
