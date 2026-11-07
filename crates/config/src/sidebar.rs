use std::collections::BTreeMap;

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Deserializer, de};

pub const MAX_ROWS: usize = 16;
pub const MAX_TOKENS_PER_ROW: usize = 16;
pub const MAX_RULES: usize = 16;

pub const CANONICAL_AGENT_IDS: &[&str] = &[
    "pi",
    "claude",
    "codex",
    "gemini",
    "cursor",
    "devin",
    "agy",
    "cline",
    "omp",
    "mastracode",
    "opencode",
    "copilot",
    "kimi",
    "kiro",
    "droid",
    "amp",
    "grok",
    "hermes",
    "kilo",
    "qodercli",
    "qwen",
    "letta",
    "maki",
    "muse",
];

pub trait TokenKind: Sized + Clone + PartialEq {
    fn builtin(name: &str) -> Option<Self>;
    fn custom(name: String) -> Self;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentToken {
    StateIcon,
    StateText,
    Machine,
    Workspace,
    Tab,
    Pane,
    Agent,
    TerminalTitle,
    TerminalTitleStripped,
    Custom(String),
}

impl TokenKind for AgentToken {
    fn builtin(name: &str) -> Option<Self> {
        Some(match name {
            "state_icon" => Self::StateIcon,
            "state_text" => Self::StateText,
            "machine" => Self::Machine,
            "workspace" => Self::Workspace,
            "tab" => Self::Tab,
            "pane" => Self::Pane,
            "agent" => Self::Agent,
            "terminal_title" => Self::TerminalTitle,
            "terminal_title_stripped" => Self::TerminalTitleStripped,
            _ => return None,
        })
    }

    fn custom(name: String) -> Self {
        Self::Custom(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaceToken {
    StateIcon,
    StateText,
    Workspace,
    Branch,
    GitStatus,
    Custom(String),
}

impl TokenKind for SpaceToken {
    fn builtin(name: &str) -> Option<Self> {
        Some(match name {
            "state_icon" => Self::StateIcon,
            "state_text" => Self::StateText,
            "workspace" => Self::Workspace,
            "branch" => Self::Branch,
            "git_status" => Self::GitStatus,
            _ => return None,
        })
    }

    fn custom(name: String) -> Self {
        Self::Custom(name)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenStyle {
    pub fg: Option<Color>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
}

impl TokenStyle {
    #[must_use]
    pub fn or(self, base: Self) -> Self {
        Self {
            fg: self.fg.or(base.fg),
            bold: self.bold.or(base.bold),
            dim: self.dim.or(base.dim),
        }
    }

    pub fn patch(self, mut style: Style) -> Style {
        if let Some(fg) = self.fg {
            style = style.fg(fg);
        }
        for (flag, modifier) in [(self.bold, Modifier::BOLD), (self.dim, Modifier::DIM)] {
            style = match flag {
                Some(true) => style.add_modifier(modifier),
                Some(false) => style.remove_modifier(modifier),
                None => style,
            };
        }
        style
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Equals(String),
    Contains(String),
    StartsWith(String),
    GreaterThan(f64),
    LessThan(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenRule {
    pub condition: Condition,
    pub ignore_case: bool,
    pub style: TokenStyle,
    pub hide: bool,
}

impl TokenRule {
    fn matches(&self, value: &str, number: Option<f64>) -> bool {
        let fold = self.ignore_case;
        match &self.condition {
            Condition::Equals(expected) => {
                if fold {
                    value.eq_ignore_ascii_case(expected)
                } else {
                    value == expected
                }
            }
            Condition::StartsWith(expected) => {
                if fold {
                    value
                        .as_bytes()
                        .get(..expected.len())
                        .is_some_and(|head| head.eq_ignore_ascii_case(expected.as_bytes()))
                } else {
                    value.starts_with(expected.as_str())
                }
            }
            Condition::Contains(expected) => {
                if fold {
                    expected.is_empty()
                        || value
                            .as_bytes()
                            .windows(expected.len())
                            .any(|part| part.eq_ignore_ascii_case(expected.as_bytes()))
                } else {
                    value.contains(expected.as_str())
                }
            }
            Condition::GreaterThan(threshold) => number.is_some_and(|number| number > *threshold),
            Condition::LessThan(threshold) => number.is_some_and(|number| number < *threshold),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawRule {
    equals: Option<String>,
    contains: Option<String>,
    starts_with: Option<String>,
    gt: Option<f64>,
    lt: Option<f64>,
    ignore_case: Option<bool>,
    fg: Option<TokenColor>,
    bold: Option<bool>,
    dim: Option<bool>,
    hide: Option<bool>,
}

impl<'de> Deserialize<'de> for TokenRule {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawRule::deserialize(deserializer)?;
        let mut conditions = Vec::new();
        conditions.extend(raw.equals.map(Condition::Equals));
        conditions.extend(raw.contains.map(Condition::Contains));
        conditions.extend(raw.starts_with.map(Condition::StartsWith));
        conditions.extend(raw.gt.map(Condition::GreaterThan));
        conditions.extend(raw.lt.map(Condition::LessThan));
        let [condition] = <[Condition; 1]>::try_from(conditions).map_err(|_| {
            de::Error::custom(
                "sidebar rule requires exactly one of equals, contains, starts_with, gt, lt",
            )
        })?;
        if let Condition::GreaterThan(threshold) | Condition::LessThan(threshold) = condition {
            if !threshold.is_finite() {
                return Err(de::Error::custom(
                    "sidebar numeric rule threshold must be finite",
                ));
            }
            if raw.ignore_case.is_some() {
                return Err(de::Error::custom(
                    "ignore_case applies only to sidebar text conditions",
                ));
            }
        }
        Ok(Self {
            condition,
            ignore_case: raw.ignore_case.unwrap_or(false),
            style: TokenStyle {
                fg: raw.fg.map(|color| color.0),
                bold: raw.bold,
                dim: raw.dim,
            },
            hide: raw.hide == Some(true),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TokenColor(Color);

impl<'de> Deserialize<'de> for TokenColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value
            .strip_prefix('#')
            .filter(|hex| hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .and_then(crate::theme::parse_hex)
            .map(Self)
            .ok_or_else(|| de::Error::custom("sidebar token fg must be #RGB or #RRGGBB"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token<T> {
    pub kind: T,
    pub style: TokenStyle,
    pub rules: Vec<TokenRule>,
}

impl<T> Token<T> {
    pub fn plain(kind: T) -> Self {
        Self {
            kind,
            style: TokenStyle::default(),
            rules: Vec::new(),
        }
    }

    pub fn style_for(&self, value: &str) -> Option<TokenStyle> {
        let number = value
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite());
        for rule in &self.rules {
            if rule.matches(value, number) {
                return (!rule.hide).then(|| rule.style.or(self.style));
            }
        }
        Some(self.style)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStyledToken {
    token: String,
    #[serde(default)]
    fg: Option<TokenColor>,
    #[serde(default)]
    bold: Option<bool>,
    #[serde(default)]
    dim: Option<bool>,
    #[serde(default)]
    rules: Vec<TokenRule>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawToken {
    Plain(String),
    Styled(RawStyledToken),
}

fn token_kind<T: TokenKind>(name: &str) -> Result<T, String> {
    if let Some(kind) = T::builtin(name) {
        return Ok(kind);
    }
    let Some(custom) = name.strip_prefix('$') else {
        return Err(format!(
            "unknown sidebar token `{name}`; custom tokens must start with `$`"
        ));
    };
    let valid = (1..=32).contains(&custom.len())
        && custom
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'));
    if !valid {
        return Err(format!("invalid custom sidebar token `{name}`"));
    }
    Ok(T::custom(custom.to_string()))
}

impl<'de, T: TokenKind> Deserialize<'de> for Token<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match RawToken::deserialize(deserializer)? {
            RawToken::Plain(name) => token_kind(&name).map(Self::plain),
            RawToken::Styled(raw) => {
                if raw.rules.len() > MAX_RULES {
                    return Err(de::Error::custom(format!(
                        "sidebar tokens may contain at most {MAX_RULES} rules"
                    )));
                }
                if !raw.rules.is_empty()
                    && matches!(raw.token.as_str(), "state_icon" | "git_status")
                {
                    return Err(de::Error::custom(
                        "sidebar rules require a text-valued token",
                    ));
                }
                token_kind(&raw.token).map(|kind| Self {
                    kind,
                    style: TokenStyle {
                        fg: raw.fg.map(|color| color.0),
                        bold: raw.bold,
                        dim: raw.dim,
                    },
                    rules: raw.rules,
                })
            }
        }
        .map_err(de::Error::custom)
    }
}

pub type Rows<T> = Vec<Vec<Token<T>>>;

fn check_rows<T>(rows: &Rows<T>) -> Result<(), String> {
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "sidebar layouts may contain at most {MAX_ROWS} rows"
        ));
    }
    if rows.iter().any(|row| row.len() > MAX_TOKENS_PER_ROW) {
        return Err(format!(
            "sidebar rows may contain at most {MAX_TOKENS_PER_ROW} tokens"
        ));
    }
    Ok(())
}

fn rows<'de, D: Deserializer<'de>, T: TokenKind>(deserializer: D) -> Result<Rows<T>, D::Error> {
    let rows = Rows::<T>::deserialize(deserializer)?;
    check_rows(&rows).map_err(de::Error::custom)?;
    Ok(rows)
}

fn rows_by_agent<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Rows<AgentToken>>, D::Error> {
    let map = BTreeMap::<String, Rows<AgentToken>>::deserialize(deserializer)?;
    for (id, rows) in &map {
        if !CANONICAL_AGENT_IDS.contains(&id.as_str()) {
            return Err(de::Error::custom(format!(
                "unknown canonical agent id `{id}` in sidebar rows_by_agent"
            )));
        }
        check_rows(rows).map_err(de::Error::custom)?;
    }
    Ok(map)
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SidebarConfig {
    pub agents: AgentsSidebar,
    pub spaces: SpacesSidebar,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct AgentsSidebar {
    #[serde(deserialize_with = "rows")]
    pub rows: Rows<AgentToken>,
    #[serde(deserialize_with = "rows_by_agent")]
    pub rows_by_agent: BTreeMap<String, Rows<AgentToken>>,
    pub row_gap: u16,
}

impl AgentsSidebar {
    pub fn rows_for(&self, agent: Option<&str>) -> &Rows<AgentToken> {
        agent
            .and_then(|agent| self.rows_by_agent.get(agent))
            .unwrap_or(&self.rows)
    }
}

impl Default for AgentsSidebar {
    fn default() -> Self {
        Self {
            rows: vec![
                vec![
                    Token::plain(AgentToken::StateIcon),
                    Token::plain(AgentToken::Machine),
                    Token::plain(AgentToken::Workspace),
                    Token::plain(AgentToken::Tab),
                ],
                vec![Token::plain(AgentToken::Agent)],
            ],
            rows_by_agent: BTreeMap::new(),
            row_gap: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct SpacesSidebar {
    #[serde(deserialize_with = "rows")]
    pub rows: Rows<SpaceToken>,
    pub row_gap: u16,
}

impl Default for SpacesSidebar {
    fn default() -> Self {
        Self {
            rows: vec![
                vec![
                    Token::plain(SpaceToken::StateIcon),
                    Token::plain(SpaceToken::Workspace),
                ],
                vec![
                    Token::plain(SpaceToken::Branch),
                    Token::plain(SpaceToken::GitStatus),
                ],
            ],
            row_gap: 0,
        }
    }
}
