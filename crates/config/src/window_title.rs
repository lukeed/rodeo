pub const DEFAULT_WINDOW_TITLE: &str = "{hostname}: {workspace}";
pub const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleToken {
    Hostname,
    Workspace,
    Tab,
    Pane,
    TerminalTitle,
}

impl TitleToken {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "hostname" => Self::Hostname,
            "workspace" => Self::Workspace,
            "tab" => Self::Tab,
            "pane" => Self::Pane,
            "terminal_title" => Self::TerminalTitle,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitlePart {
    Text(String),
    Token(TitleToken),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TitleValues<'a> {
    pub hostname: &'a str,
    pub workspace: &'a str,
    pub tab: &'a str,
    pub pane: &'a str,
    pub terminal_title: &'a str,
}

impl<'a> TitleValues<'a> {
    fn get(&self, token: TitleToken) -> &'a str {
        match token {
            TitleToken::Hostname => self.hostname,
            TitleToken::Workspace => self.workspace,
            TitleToken::Tab => self.tab,
            TitleToken::Pane => self.pane,
            TitleToken::TerminalTitle => self.terminal_title,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTitle {
    parts: Vec<TitlePart>,
}

impl WindowTitle {
    pub fn parse(template: &str) -> Result<Option<Self>, String> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut chars = template.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '{' | '}' if chars.peek() == Some(&ch) => {
                    chars.next();
                    text.push(ch);
                }
                '{' => {
                    let mut name = String::new();
                    let closed = chars.by_ref().any(|next| {
                        if next == '}' {
                            return true;
                        }
                        name.push(next);
                        false
                    });
                    if !closed {
                        return Err("has an unclosed '{'".into());
                    }
                    let token = TitleToken::parse(name.trim())
                        .ok_or_else(|| format!("has unknown token '{{{name}}}'"))?;
                    if !text.is_empty() {
                        parts.push(TitlePart::Text(std::mem::take(&mut text)));
                    }
                    parts.push(TitlePart::Token(token));
                }
                '}' => return Err("has an unmatched '}'".into()),
                _ => text.push(ch),
            }
        }
        if !text.is_empty() {
            parts.push(TitlePart::Text(text));
        }
        Ok((!parts.is_empty()).then_some(Self { parts }))
    }

    pub fn parts(&self) -> &[TitlePart] {
        &self.parts
    }

    pub fn uses(&self, token: TitleToken) -> bool {
        self.parts.contains(&TitlePart::Token(token))
    }

    pub fn render(&self, values: &TitleValues) -> Option<String> {
        let raw: String = self
            .parts
            .iter()
            .map(|part| match part {
                TitlePart::Text(text) => text.as_str(),
                TitlePart::Token(token) => values.get(*token),
            })
            .collect();
        sanitize_title(&raw)
    }
}

pub fn sanitize_title(value: &str) -> Option<String> {
    let clean: String = value
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TITLE_CHARS)
        .collect();
    let clean = clean.trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

pub fn window_title_diagnostic(template: &str) -> Option<String> {
    WindowTitle::parse(template)
        .err()
        .map(|error| format!("ui.window_title {error}; leaving the outer terminal title alone"))
}
