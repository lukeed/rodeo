use std::path::Path;

use config::{
    AgentToken, Condition, Config, SpaceToken, TabBarEntry, TitlePart, TitleToken, TitleValues,
    Token, TokenStyle, WindowTitle, parse_datetime_format, sanitize_title, shown_entries,
    tab_bar_diagnostics,
};
use ratatui::style::{Color, Modifier, Style};

fn ui(text: &str) -> Result<Config, String> {
    let loaded = Config::parse(text, Path::new("/"));
    match loaded.diagnostics.first() {
        Some(error) if error.starts_with("config parse error") => Err(error.clone()),
        _ => Ok(loaded.config),
    }
}

fn spaces(rows: &str) -> Result<Vec<Vec<Token<SpaceToken>>>, String> {
    ui(&format!("[ui.sidebar.spaces]\nrows = {rows}\n")).map(|config| config.ui.sidebar.spaces.rows)
}

fn agents(rows: &str) -> Result<Vec<Vec<Token<AgentToken>>>, String> {
    ui(&format!("[ui.sidebar.agents]\nrows = {rows}\n")).map(|config| config.ui.sidebar.agents.rows)
}

fn rejects(result: Result<impl std::fmt::Debug, String>, needle: &str) {
    let error = result.expect_err("expected a parse error");
    assert!(error.contains(needle), "{error}");
}

#[test]
fn token_names() {
    let rows = agents(
        "[[\"state_icon\", \"state_text\", \"machine\", \"workspace\", \"tab\", \"pane\", \"agent\", \"terminal_title\", \"terminal_title_stripped\", \"$ci-status_1\"]]",
    )
    .unwrap();
    let kinds: Vec<_> = rows[0].iter().map(|token| token.kind.clone()).collect();
    assert_eq!(
        kinds,
        [
            AgentToken::StateIcon,
            AgentToken::StateText,
            AgentToken::Machine,
            AgentToken::Workspace,
            AgentToken::Tab,
            AgentToken::Pane,
            AgentToken::Agent,
            AgentToken::TerminalTitle,
            AgentToken::TerminalTitleStripped,
            AgentToken::Custom("ci-status_1".into()),
        ]
    );
    let rows = spaces(
        "[[\"state_icon\", \"state_text\", \"workspace\", \"branch\", \"git_status\", \"$x\"]]",
    )
    .unwrap();
    assert_eq!(rows[0].len(), 6);
    assert_eq!(rows[0][3].kind, SpaceToken::Branch);
    rejects(spaces("[[\"agent\"]]"), "unknown sidebar token `agent`");
    rejects(agents("[[\"branch\"]]"), "unknown sidebar token `branch`");
    rejects(agents("[[\"$\"]]"), "invalid custom sidebar token `$`");
    rejects(agents("[[\"$a.b\"]]"), "invalid custom sidebar token");
    let long = "x".repeat(33);
    rejects(
        agents(&format!("[[\"${long}\"]]")),
        "invalid custom sidebar token",
    );
    assert!(agents(&format!("[[\"${}\"]]", "x".repeat(32))).is_ok());
}

#[test]
fn limits() {
    let row = format!("[{}]", vec!["\"workspace\""; 16].join(","));
    assert!(spaces(&format!("[{row}]")).is_ok());
    let wide = format!("[{}]", vec!["\"workspace\""; 17].join(","));
    rejects(spaces(&format!("[{wide}]")), "at most 16 tokens");
    let tall = vec!["[\"workspace\"]"; 17].join(",");
    rejects(spaces(&format!("[{tall}]")), "at most 16 rows");
    let rules = vec!["{ equals = \"a\" }"; 17].join(",");
    rejects(
        spaces(&format!("[[{{ token = \"branch\", rules = [{rules}] }}]]")),
        "at most 16 rules",
    );
    rejects(
        ui(
            "[ui.sidebar.agents.rows_by_agent]\nclaude = [[\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"], [\"agent\"]]\n",
        ),
        "at most 16 rows",
    );
}

#[test]
fn styled_tokens() {
    let rows = spaces(
        "[[{ token = \"branch\", fg = \"#a6e3a1\", bold = true, dim = false }, { token = \"git_status\", fg = \"#FFF\" }]]",
    )
    .unwrap();
    assert_eq!(
        rows[0][0].style,
        TokenStyle {
            fg: Some(Color::Rgb(0xa6, 0xe3, 0xa1)),
            bold: Some(true),
            dim: Some(false),
        }
    );
    assert_eq!(rows[0][1].style.fg, Some(Color::Rgb(255, 255, 255)));
    rejects(
        spaces("[[{ token = \"branch\", fg = \"red\" }]]"),
        "#RGB or #RRGGBB",
    );
    rejects(
        spaces("[[{ token = \"branch\", fg = \"#12345\" }]]"),
        "#RGB or #RRGGBB",
    );
    rejects(
        spaces("[[{ token = \"branch\", fg = \"#+12345\" }]]"),
        "#RGB or #RRGGBB",
    );
    assert!(spaces("[[{ token = \"branch\", italic = true }]]").is_err());
    rejects(
        spaces("[[{ token = \"git_status\", rules = [{ equals = \"x\" }] }]]"),
        "text-valued token",
    );
    rejects(
        agents("[[{ token = \"state_icon\", rules = [{ equals = \"x\" }] }]]"),
        "text-valued token",
    );
    assert!(spaces("[[{ token = \"git_status\", rules = [] }]]").is_ok());
}

#[test]
fn rule_validation() {
    let rule = |body: &str| {
        spaces(&format!(
            "[[{{ token = \"branch\", rules = [{{ {body} }}] }}]]"
        ))
    };
    rejects(rule("fg = \"#fff\""), "exactly one of");
    rejects(rule("equals = \"a\", contains = \"b\""), "exactly one of");
    rejects(rule("gt = nan"), "must be finite");
    rejects(rule("lt = inf"), "must be finite");
    rejects(
        rule("gt = 1.0, ignore_case = true"),
        "only to sidebar text conditions",
    );
    assert!(rule("equals = \"a\", wat = 1").is_err());
    let rows = rule("starts_with = \"feat/\", ignore_case = true, fg = \"#123456\", hide = false")
        .unwrap();
    let parsed = &rows[0][0].rules[0];
    assert_eq!(parsed.condition, Condition::StartsWith("feat/".into()));
    assert!(parsed.ignore_case);
    assert!(!parsed.hide);
}

#[test]
fn rule_evaluation() {
    let rows = spaces(
        "[[{ token = \"branch\", fg = \"#111111\", bold = true, rules = [\
            { equals = \"main\", hide = true },\
            { starts_with = \"FEAT/\", ignore_case = true, fg = \"#222222\" },\
            { contains = \"wip\", dim = true },\
            { gt = 10, bold = false },\
            { lt = -1.5, fg = \"#333333\" },\
            { contains = \"\", ignore_case = true, dim = false }\
        ] }]]",
    )
    .unwrap();
    let token = &rows[0][0];
    let base = TokenStyle {
        fg: Some(Color::Rgb(0x11, 0x11, 0x11)),
        bold: Some(true),
        dim: None,
    };
    assert_eq!(token.style_for("main"), None);
    assert_eq!(
        token.style_for("feat/login"),
        Some(TokenStyle {
            fg: Some(Color::Rgb(0x22, 0x22, 0x22)),
            ..base
        })
    );
    assert_eq!(
        token.style_for("my-wip"),
        Some(TokenStyle {
            dim: Some(true),
            ..base
        })
    );
    assert_eq!(
        token.style_for("10.5"),
        Some(TokenStyle {
            bold: Some(false),
            ..base
        })
    );
    assert_eq!(
        token.style_for("-2"),
        Some(TokenStyle {
            fg: Some(Color::Rgb(0x33, 0x33, 0x33)),
            ..base
        })
    );
    assert_eq!(
        token.style_for("10"),
        Some(TokenStyle {
            dim: Some(false),
            ..base
        })
    );
    assert_eq!(
        token.style_for("inf"),
        Some(TokenStyle {
            dim: Some(false),
            ..base
        })
    );
    assert_eq!(
        token.style_for("MAIN"),
        Some(TokenStyle {
            dim: Some(false),
            ..base
        })
    );
    let plain = Token::plain(SpaceToken::Workspace);
    assert_eq!(plain.style_for("anything"), Some(TokenStyle::default()));
    let unmatched =
        spaces("[[{ token = \"branch\", fg = \"#010101\", rules = [{ equals = \"x\" }] }]]")
            .unwrap();
    assert_eq!(
        unmatched[0][0].style_for("y").unwrap().fg,
        Some(Color::Rgb(1, 1, 1))
    );
}

#[test]
fn style_patch() {
    let style = Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::BOLD);
    let patched = TokenStyle {
        fg: Some(Color::Red),
        bold: Some(false),
        dim: Some(true),
    }
    .patch(style);
    assert_eq!(patched.fg, Some(Color::Red));
    assert!(!patched.add_modifier.contains(Modifier::BOLD));
    assert!(patched.add_modifier.contains(Modifier::DIM));
    assert_eq!(TokenStyle::default().patch(style), style);
}

#[test]
fn rows_by_agent() {
    let config =
        ui("[ui.sidebar.agents.rows_by_agent]\nclaude = [[\"agent\"]]\nopencode = [[\"pane\"]]\n")
            .unwrap();
    let agents = &config.ui.sidebar.agents;
    assert_eq!(
        agents.rows_for(Some("claude"))[0][0].kind,
        AgentToken::Agent
    );
    assert_eq!(
        agents.rows_for(Some("opencode"))[0][0].kind,
        AgentToken::Pane
    );
    assert_eq!(agents.rows_for(Some("codex")), &agents.rows);
    assert_eq!(agents.rows_for(None), &agents.rows);
    for id in config::CANONICAL_AGENT_IDS {
        assert!(
            ui(&format!(
                "[ui.sidebar.agents.rows_by_agent]\n{id} = [[\"agent\"]]\n"
            ))
            .is_ok()
        );
    }
    rejects(
        ui("[ui.sidebar.agents.rows_by_agent]\nopen_code = [[\"agent\"]]\n"),
        "unknown canonical agent id `open_code`",
    );
}

#[test]
fn tab_bar_entries() {
    let config = ui("[ui]\ntab_bar_right = [\
            { type = \"zoom\" }, { type = \"hostname\" }, { type = \"datetime\" },\
            { type = \"datetime\", format = \"%Y-%m-%d\" }, { type = \"text\", text = \"hi\" },\
            { type = \"command\", command = \"date\" }]\ntab_bar_right_separator = \" | \"\n")
    .unwrap();
    let entries = &config.ui.tab_bar_right;
    assert_eq!(
        entries,
        &vec![
            TabBarEntry::Zoom,
            TabBarEntry::Hostname,
            TabBarEntry::Datetime {
                format: "%H:%M".into()
            },
            TabBarEntry::Datetime {
                format: "%Y-%m-%d".into()
            },
            TabBarEntry::Text { text: "hi".into() },
            TabBarEntry::Command {
                command: "date".into(),
                interval_seconds: 5,
                timeout_seconds: 2,
            },
        ]
    );
    assert_eq!(config.ui.tab_bar_right_separator, " | ");
    assert!(
        tab_bar_diagnostics(entries).is_empty(),
        "{:?}",
        tab_bar_diagnostics(entries)
    );
    assert_eq!(shown_entries(entries).count(), 6);
    assert!(ui("[ui]\ntab_bar_right = [{ type = \"text\" }]\n").is_err());
    assert!(ui("[ui]\ntab_bar_right = [{ type = \"zoom\", text = \"x\" }]\n").is_ok());
    assert!(parse_datetime_format("%H:%M").is_ok());
}

#[test]
fn tab_bar_problems() {
    let entries = vec![
        TabBarEntry::Datetime {
            format: "%z".into(),
        },
        TabBarEntry::Datetime {
            format: "%s".into(),
        },
        TabBarEntry::Command {
            command: "x".into(),
            interval_seconds: 31_536_001,
            timeout_seconds: 0,
        },
    ];
    let diagnostics = tab_bar_diagnostics(&entries);
    assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("ui.tab_bar_right[0] has unsupported datetime format"));
    assert!(diagnostics[0].ends_with("; hiding entry"));
    assert!(diagnostics[1].starts_with("ui.tab_bar_right[1] has "));
    assert_eq!(
        diagnostics[2],
        "ui.tab_bar_right[2] interval_seconds may be at most 31536000; hiding entry"
    );
    assert_eq!(
        diagnostics[3],
        "ui.tab_bar_right[2] timeout_seconds must be at least 1; hiding entry"
    );
    assert_eq!(shown_entries(&entries).count(), 0);
    assert!(parse_datetime_format("%Q").is_err());

    let many = vec![TabBarEntry::Zoom; 17];
    assert_eq!(
        tab_bar_diagnostics(&many),
        vec!["ui.tab_bar_right may contain at most 16 entries; ignoring extras"]
    );
    assert_eq!(shown_entries(&many).count(), 16);
}

#[test]
fn window_title_parse() {
    let title = WindowTitle::parse("{hostname}: {workspace}")
        .unwrap()
        .unwrap();
    assert_eq!(
        title.parts(),
        [
            TitlePart::Token(TitleToken::Hostname),
            TitlePart::Text(": ".into()),
            TitlePart::Token(TitleToken::Workspace),
        ]
    );
    assert!(title.uses(TitleToken::Hostname));
    assert!(!title.uses(TitleToken::TerminalTitle));
    let braces = WindowTitle::parse("{{a}} { tab }{pane}{terminal_title}")
        .unwrap()
        .unwrap();
    assert_eq!(
        braces.parts(),
        [
            TitlePart::Text("{a} ".into()),
            TitlePart::Token(TitleToken::Tab),
            TitlePart::Token(TitleToken::Pane),
            TitlePart::Token(TitleToken::TerminalTitle),
        ]
    );
    assert_eq!(WindowTitle::parse(""), Ok(None));
    assert_eq!(
        WindowTitle::parse("{oops}"),
        Err("has unknown token '{oops}'".into())
    );
    assert_eq!(
        WindowTitle::parse("a {tab"),
        Err("has an unclosed '{'".into())
    );
    assert_eq!(
        WindowTitle::parse("a } b"),
        Err("has an unmatched '}'".into())
    );
    assert_eq!(
        config::window_title_diagnostic("{"),
        Some("ui.window_title has an unclosed '{'; leaving the outer terminal title alone".into())
    );
    assert_eq!(config::window_title_diagnostic(""), None);
    assert!(Config::default().window_title().is_some());
    let empty = ui("[ui]\nwindow_title = \"\"\n").unwrap();
    assert_eq!(empty.window_title(), None);
}

#[test]
fn window_title_render() {
    let values = TitleValues {
        hostname: "box",
        workspace: "drover",
        tab: "2",
        pane: "",
        terminal_title: "vim",
    };
    let title = WindowTitle::parse("{hostname}: {workspace} [{tab}]{pane} {{{terminal_title}}}")
        .unwrap()
        .unwrap();
    assert_eq!(
        title.render(&values).as_deref(),
        Some("box: drover [2] {vim}")
    );
    let only_pane = WindowTitle::parse(" {pane} ").unwrap().unwrap();
    assert_eq!(only_pane.render(&values), None);
    let dirty = TitleValues {
        workspace: "a\u{1b}]0;x\u{7}b\u{9c}\nc",
        ..values
    };
    let workspace = WindowTitle::parse("{workspace}").unwrap().unwrap();
    assert_eq!(workspace.render(&dirty).as_deref(), Some("a]0;xbc"));
    let long = "é".repeat(250);
    let capped = sanitize_title(&long).unwrap();
    assert_eq!(capped.chars().count(), 200);
    assert_eq!(sanitize_title("  \t "), None);
    assert_eq!(sanitize_title("  x  ").as_deref(), Some("x"));
}
