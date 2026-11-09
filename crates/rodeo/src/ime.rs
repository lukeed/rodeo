use ratatui::layout::Rect;

use crate::app::App;
use crate::render::Cursor;

const AGENTS: &[(&str, &[&str])] = &[
    ("pi", &["pi"]),
    ("claude", &["claude", "claude-code"]),
    ("codex", &["codex"]),
    ("gemini", &["gemini"]),
    ("cursor", &["cursor", "cursor-agent"]),
    ("devin", &["devin", "devin-cli", "devin cli"]),
    ("agy", &["agy", "antigravity", "antigravity-cli"]),
    ("cline", &["cline", ".cline"]),
    ("omp", &["omp"]),
    ("mastracode", &["mastracode", "mastra-code", "mastra code"]),
    ("opencode", &["opencode", "opencode2", "open-code"]),
    (
        "copilot",
        &["copilot", "github-copilot", "ghcs", "copilot-cli"],
    ),
    ("kimi", &["kimi", "kimi-code", "kimi code"]),
    ("kiro", &["kiro", "kiro-cli"]),
    ("droid", &["droid"]),
    ("amp", &["amp", "amp-local"]),
    ("grok", &["grok", "grok-build"]),
    ("hermes", &["hermes", "hermes-agent"]),
    ("kilo", &["kilo", "kilo-code", "kilo code"]),
    ("qodercli", &["qodercli", "qoderclicn", "qoder", "qodercn"]),
    ("qwen", &["qwen", "qwen-code", "qwen code"]),
    ("letta", &["letta", "letta-code", "letta code"]),
    ("maki", &["maki"]),
    ("muse", &["muse", "muse-code", "muse-cli"]),
];

pub fn agent(name: &str) -> Option<&'static str> {
    let name = name.trim().to_lowercase();
    let name = name.rsplit('/').next().unwrap_or(&name);
    AGENTS
        .iter()
        .find(|(_, aliases)| aliases.contains(&name))
        .map(|(key, _)| *key)
}

pub fn shape(shape: config::CursorShape) -> (vt::CursorShape, bool) {
    match shape {
        config::CursorShape::Block => (vt::CursorShape::Block, true),
        config::CursorShape::SteadyBlock => (vt::CursorShape::Block, false),
        config::CursorShape::Underline => (vt::CursorShape::Underline, true),
        config::CursorShape::SteadyUnderline => (vt::CursorShape::Underline, false),
        config::CursorShape::Bar => (vt::CursorShape::Bar, true),
        config::CursorShape::SteadyBar => (vt::CursorShape::Bar, false),
    }
}

impl App {
    pub fn reveals_ime_cursor(&self, address: &str) -> bool {
        let experimental = &self.config.experimental;
        if !experimental.reveal_hidden_cursor_for_cjk_ime {
            return false;
        }
        if experimental.cjk_ime_agents.is_empty() {
            return true;
        }
        let Some(kind) = self
            .terminals
            .get(address)
            .and_then(crate::agents::label)
            .and_then(|label| agent(&label))
        else {
            return false;
        };
        experimental
            .cjk_ime_agents
            .iter()
            .any(|name| agent(name) == Some(kind))
    }

    pub fn ime_cursor(
        &self,
        address: &str,
        content: Rect,
        drawn: Option<Cursor>,
    ) -> Option<Cursor> {
        if !self.reveals_ime_cursor(address) {
            return drawn;
        }
        let term = &self.panes.get(address)?.term;
        if term.is_scrolled_back() {
            return None;
        }
        let (shape, blinking) = shape(self.config.experimental.cjk_ime_cursor_shape);
        let state = term.cursor();
        let (x, y) = drawn.map_or_else(
            || {
                (
                    content.x + state.col.min(content.width.saturating_sub(1)),
                    content.y + state.row.min(content.height.saturating_sub(1)),
                )
            },
            |c| (c.x, c.y),
        );
        Some(Cursor {
            x,
            y,
            shape,
            blinking,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_names_and_aliases_match() {
        assert_eq!(agent("Claude"), Some("claude"));
        assert_eq!(agent("claude-code"), Some("claude"));
        assert_eq!(agent("Qwen Code"), Some("qwen"));
        assert_eq!(agent("/usr/bin/codex"), Some("codex"));
        assert_eq!(agent("github-copilot"), Some("copilot"));
        assert_eq!(agent("vim"), None);
    }

    #[test]
    fn shapes_follow_decscusr() {
        assert_eq!(
            shape(config::CursorShape::SteadyBlock),
            (vt::CursorShape::Block, false)
        );
        assert_eq!(
            shape(config::CursorShape::Bar),
            (vt::CursorShape::Bar, true)
        );
    }
}
