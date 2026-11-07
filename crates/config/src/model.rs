use std::num::NonZeroUsize;

use crossterm::event::KeyModifiers;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::keys::KeysConfig;
use crate::sidebar::SidebarConfig;
use crate::sound::SoundConfig;
use crate::tab_bar::TabBarEntry;
use crate::theme::ThemeConfig;
use crate::window_title::DEFAULT_WINDOW_TITLE;

pub const MAX_TOAST_DELAY_SECONDS: u64 = 3600;
pub const DEFAULT_MOUSE_SCROLL_LINES: usize = 3;
pub const DEFAULT_SCROLLBACK_LIMIT_BYTES: usize = 10_000_000;
pub const DEFAULT_SIDEBAR_BOUNDS: (u16, u16) = (18, 36);

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub onboarding: Option<bool>,
    pub theme: ThemeConfig,
    pub terminal: TerminalConfig,
    pub session: SessionConfig,
    pub server: ServerConfig,
    pub update: UpdateConfig,
    pub keys: KeysConfig,
    pub ui: UiConfig,
    pub worktrees: WorktreesConfig,
    pub advanced: AdvancedConfig,
    pub experimental: ExperimentalConfig,
    pub remote: RemoteConfig,
}

impl Config {
    pub fn kitty_graphics(&self) -> bool {
        self.terminal
            .kitty_graphics
            .or(self.experimental.kitty_graphics)
            .unwrap_or(true)
    }

    pub fn show_onboarding(&self) -> bool {
        self.onboarding.unwrap_or(true)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    pub default_shell: String,
    pub shell_mode: ShellMode,
    pub new_cwd: NewCwd,
    pub kitty_graphics: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellMode {
    #[default]
    Auto,
    Login,
    NonLogin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum NewCwd {
    #[default]
    Follow,
    Home,
    Current,
    Path(String),
}

impl<'de> Deserialize<'de> for NewCwd {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.trim() {
            "" | "follow" => Self::Follow,
            "home" => Self::Home,
            "current" => Self::Current,
            _ => Self::Path(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub resume_agents_on_restore: bool,
    pub startup_per_agent_delay_ms: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            resume_agents_on_restore: true,
            startup_per_agent_delay_ms: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub headless_cols: u16,
    pub headless_rows: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            headless_cols: 120,
            headless_rows: 40,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    pub channel: UpdateChannel,
    pub version_check: bool,
    pub manifest_check: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            channel: UpdateChannel::Stable,
            version_check: true,
            manifest_check: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Preview,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct WorktreesConfig {
    pub directory: String,
}

impl Default for WorktreesConfig {
    fn default() -> Self {
        Self {
            directory: "~/.herdr/worktrees".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AdvancedConfig {
    #[serde(alias = "scrollback_lines")]
    pub scrollback_limit_bytes: usize,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            scrollback_limit_bytes: DEFAULT_SCROLLBACK_LIMIT_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct RemoteConfig {
    pub manage_ssh_config: bool,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            manage_ssh_config: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ExperimentalConfig {
    pub allow_nested: bool,
    pub kitty_graphics: Option<bool>,
    pub pane_history: bool,
    pub reveal_hidden_cursor_for_cjk_ime: bool,
    pub cjk_ime_agents: Vec<String>,
    pub cjk_ime_cursor_shape: CursorShape,
    pub switch_ascii_input_source_in_prefix: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    Block,
    #[default]
    SteadyBlock,
    Underline,
    SteadyUnderline,
    Bar,
    SteadyBar,
}

impl CursorShape {
    pub fn decscusr(self) -> u8 {
        match self {
            Self::Block => 1,
            Self::SteadyBlock => 2,
            Self::Underline => 3,
            Self::SteadyUnderline => 4,
            Self::Bar => 5,
            Self::SteadyBar => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub sidebar_width: u16,
    pub sidebar_min_width: u16,
    pub sidebar_max_width: u16,
    pub sidebar_start_collapsed: bool,
    pub sidebar_collapsed_mode: SidebarCollapsedMode,
    pub mobile_width_threshold: u16,
    pub mouse_capture: bool,
    pub copy_on_select: bool,
    pub host_cursor: HostCursor,
    pub right_click_passthrough_modifier: PassthroughModifier,
    pub redraw_on_focus_gained: bool,
    pub mouse_scroll_lines: Option<NonZeroUsize>,
    pub confirm_close: bool,
    pub prompt_new_tab_name: bool,
    pub prompt_new_workspace_name: bool,
    pub pane_borders: PaneBorders,
    pub pane_outer_borders: bool,
    pub pane_scrollbars: bool,
    pub pane_gaps: bool,
    pub show_agent_labels_on_pane_borders: bool,
    pub hide_tab_bar_when_single_tab: bool,
    pub tab_bar_position: TabBarPosition,
    pub tab_bar_right: Vec<TabBarEntry>,
    pub tab_bar_right_separator: String,
    pub window_title: String,
    pub agent_panel_sort: AgentPanelSort,
    pub agent_panel_scope: Option<LegacyAgentPanelScope>,
    pub status_indicators: StatusIndicators,
    pub sidebar: SidebarConfig,
    pub accent: String,
    pub toast: ToastConfig,
    pub sound: SoundConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            sidebar_width: 26,
            sidebar_min_width: DEFAULT_SIDEBAR_BOUNDS.0,
            sidebar_max_width: DEFAULT_SIDEBAR_BOUNDS.1,
            sidebar_start_collapsed: false,
            sidebar_collapsed_mode: SidebarCollapsedMode::Compact,
            mobile_width_threshold: 64,
            mouse_capture: true,
            copy_on_select: true,
            host_cursor: HostCursor::Auto,
            right_click_passthrough_modifier: PassthroughModifier::default(),
            redraw_on_focus_gained: true,
            mouse_scroll_lines: None,
            confirm_close: true,
            prompt_new_tab_name: true,
            prompt_new_workspace_name: false,
            pane_borders: PaneBorders::Auto,
            pane_outer_borders: true,
            pane_scrollbars: true,
            pane_gaps: true,
            show_agent_labels_on_pane_borders: false,
            hide_tab_bar_when_single_tab: false,
            tab_bar_position: TabBarPosition::Top,
            tab_bar_right: Vec::new(),
            tab_bar_right_separator: " ".into(),
            window_title: DEFAULT_WINDOW_TITLE.into(),
            agent_panel_sort: AgentPanelSort::Spaces,
            agent_panel_scope: None,
            status_indicators: StatusIndicators::Dots,
            sidebar: SidebarConfig::default(),
            accent: "cyan".into(),
            toast: ToastConfig::default(),
            sound: SoundConfig::default(),
        }
    }
}

impl UiConfig {
    pub fn mouse_scroll_lines(&self) -> usize {
        self.mouse_scroll_lines
            .map_or(DEFAULT_MOUSE_SCROLL_LINES, NonZeroUsize::get)
    }

    pub fn sidebar_bounds(&self) -> Option<(u16, u16)> {
        (self.sidebar_min_width <= self.sidebar_max_width)
            .then_some((self.sidebar_min_width, self.sidebar_max_width))
    }

    pub fn sidebar_bounds_or_default(&self) -> (u16, u16) {
        self.sidebar_bounds().unwrap_or(DEFAULT_SIDEBAR_BOUNDS)
    }

    pub fn sidebar_bounds_diagnostic(&self) -> Option<String> {
        self.sidebar_bounds().is_none().then(|| {
            format!(
                "ui.sidebar_min_width ({}) is greater than sidebar_max_width ({})",
                self.sidebar_min_width, self.sidebar_max_width
            )
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarCollapsedMode {
    #[default]
    Compact,
    Hidden,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HostCursor {
    #[default]
    Auto,
    Native,
    Drawn,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TabBarPosition {
    #[default]
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentPanelSort {
    #[default]
    #[serde(alias = "workspaces")]
    Spaces,
    Priority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyAgentPanelScope {
    Current,
    All,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusIndicators {
    #[default]
    Dots,
    Symbols,
}

impl StatusIndicators {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dots => "dots",
            Self::Symbols => "symbols",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PaneBorders {
    #[default]
    Auto,
    Always,
    Off,
}

impl PaneBorders {
    pub fn draws_borders(self) -> bool {
        self != Self::Off
    }

    pub fn shows_borders(self, multi_pane: bool) -> bool {
        self.draws_borders() && (multi_pane || self == Self::Always)
    }
}

impl<'de> Deserialize<'de> for PaneBorders {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = PaneBorders;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("\"auto\", \"always\", \"off\", or a legacy boolean")
            }

            fn visit_bool<E: de::Error>(self, value: bool) -> Result<PaneBorders, E> {
                Ok(if value {
                    PaneBorders::Auto
                } else {
                    PaneBorders::Off
                })
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<PaneBorders, E> {
                match value {
                    "auto" => Ok(PaneBorders::Auto),
                    "always" => Ok(PaneBorders::Always),
                    "off" => Ok(PaneBorders::Off),
                    other => Err(E::invalid_value(de::Unexpected::Str(other), &self)),
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassthroughModifier(pub Option<KeyModifiers>);

impl PassthroughModifier {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty()
            || ["off", "none", "disabled"]
                .iter()
                .any(|word| value.eq_ignore_ascii_case(word))
        {
            return Some(Self(None));
        }
        let mut modifiers = KeyModifiers::empty();
        for token in value.split('+') {
            modifiers |= match token.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" => KeyModifiers::CONTROL,
                "alt" | "option" => KeyModifiers::ALT,
                "cmd" | "command" | "super" => KeyModifiers::SUPER,
                "meta" => KeyModifiers::META,
                "hyper" => KeyModifiers::HYPER,
                _ => return None,
            };
        }
        Some(Self(Some(modifiers)))
    }
}

impl<'de> Deserialize<'de> for PassthroughModifier {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| {
            de::Error::custom(
                "right_click_passthrough_modifier must be empty, off, none, disabled, ctrl/control, alt/option, cmd/command/super, meta, hyper, or a + separated combination without shift",
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastConfig {
    pub delivery: ToastDelivery,
    pub delay_seconds: u64,
    pub herdr: HerdrToastConfig,
    pub clipboard: ClipboardToastConfig,
}

impl Default for ToastConfig {
    fn default() -> Self {
        Self {
            delivery: ToastDelivery::Off,
            delay_seconds: 1,
            herdr: HerdrToastConfig::default(),
            clipboard: ClipboardToastConfig::default(),
        }
    }
}

impl<'de> Deserialize<'de> for ToastConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Raw {
            delivery: Option<ToastDelivery>,
            enabled: Option<bool>,
            delay_seconds: Option<u64>,
            herdr: HerdrToastConfig,
            clipboard: ClipboardToastConfig,
        }

        let raw = Raw::deserialize(deserializer)?;
        let delivery = raw.delivery.unwrap_or(if raw.enabled == Some(true) {
            ToastDelivery::Herdr
        } else {
            ToastDelivery::Off
        });
        let delay_seconds = raw.delay_seconds.unwrap_or(1);
        if delay_seconds > MAX_TOAST_DELAY_SECONDS {
            return Err(de::Error::custom(format!(
                "ui.toast.delay_seconds must be between 0 and {MAX_TOAST_DELAY_SECONDS}"
            )));
        }
        Ok(Self {
            delivery,
            delay_seconds,
            herdr: raw.herdr,
            clipboard: raw.clipboard,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastDelivery {
    #[default]
    Off,
    Herdr,
    Terminal,
    System,
}

impl ToastDelivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Herdr => "herdr",
            Self::Terminal => "terminal",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct HerdrToastConfig {
    pub position: ToastPosition,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToastPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ClipboardToastConfig {
    pub enabled: bool,
    pub position: ClipboardToastPosition,
}

impl Default for ClipboardToastConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: ClipboardToastPosition::BottomCenter,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClipboardToastPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
}
