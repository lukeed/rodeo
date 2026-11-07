mod keymap;
mod keys;
mod load;
mod model;
mod paths;
mod prefs;
mod sidebar;
mod sound;
mod tab_bar;
mod theme;
mod window_title;
mod write;

pub use keymap::{
    Action, Binding as KeyBinding, CustomCommand, DEFAULT_PREFIX_COMBO, HelpEntry, HelpGroup,
    Indexed, KeyCombo, KeyPress, Keymap, KeymapBuild, NavigateKey, PrefixKey, ResizeKey,
    TerminalKey, Trigger, binding_fields, default_binding, filter_help,
};
pub use keys::{
    Binding, CommandConfig, CommandKind, DEFAULT_PREFIX, DEFAULT_REMOTE_IMAGE_PASTE, IndexedKeys,
    KeysConfig, PopupSize,
};
pub use load::{
    CHECK_COMMAND, Loaded, Reload, ReloadStatus, check, read_config, strip_bom, summary,
};
pub use model::*;
pub use paths::{APP_DIR, CONFIG_PATH_ENV, HERDR_CONFIG_PATH_ENV, Migration, Paths};
pub use prefs::{DEFAULT_SECTION_SPLIT, Preferences};
pub use sidebar::{
    AgentToken, AgentsSidebar, CANONICAL_AGENT_IDS, Condition, Rows, SidebarConfig, SpaceToken,
    SpacesSidebar, Token, TokenKind, TokenRule, TokenStyle,
};
pub use sound::{AgentSound, AgentSounds, Sound, SoundConfig};
pub use tab_bar::{TabBarEntry, parse_datetime_format, shown_entries, tab_bar_diagnostics};
pub use theme::{
    Appearance, COLOR_ROLES, CustomColors, ModeColors, Palette, THEME_NAMES, ThemeConfig,
    builtin_palette, canonical_theme_name, parse_color, sibling_theme_names,
};
pub use time;
pub use window_title::{
    DEFAULT_WINDOW_TITLE, MAX_TITLE_CHARS, TitlePart, TitleToken, TitleValues, WindowTitle,
    sanitize_title, window_title_diagnostic,
};
pub use write::{Edit, ResetKeys, WriteError, reset_keys};

pub fn default_config_text() -> &'static str {
    include_str!("default.toml")
}
