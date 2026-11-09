use std::ffi::OsString;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::brand::{self, Ink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run { session: Option<String> },
    Help,
    Version,
    DefaultConfig,
    ConfigCheck,
    ResetKeys,
}

pub const TITLE: &str = "a terminal UI for herdr-core";

pub const USAGE: &str = "usage:
  rodeo [--session <name>]   open rodeo on an hc session
  rodeo --default-config     print the default config
  rodeo config check         check the config file
  rodeo config reset-keys    remove [keys] from the config (keeps a backup)
  rodeo --help | --version

Detach with the detach key (prefix+q). Terminals keep running in hc.";

pub fn help(ink: Option<Ink>) -> String {
    let wordmark = brand::wordmark(ink.unwrap_or(Ink::PLAIN));
    format!("{}\n\n{USAGE}", lockup(&[wordmark, Line::from(TITLE)], ink))
}

pub fn version(ink: Option<Ink>) -> String {
    let name = Style::new()
        .fg(ink.unwrap_or(Ink::PLAIN).text)
        .add_modifier(Modifier::BOLD);
    let line = Line::from(vec![
        Span::styled("rodeo", name),
        Span::raw(format!(" {}", env!("CARGO_PKG_VERSION"))),
    ]);
    lockup(&[line], ink)
}

fn lockup(beside: &[Line<'_>], ink: Option<Ink>) -> String {
    brand::ansi(
        &brand::lockup(beside, ink.unwrap_or(Ink::PLAIN)),
        ink.is_some(),
    )
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let args: Vec<String> = args
        .into_iter()
        .map(|a| {
            a.into_string()
                .map_err(|a| format!("argument is not UTF-8: {}", a.to_string_lossy()))
        })
        .collect::<Result<_, _>>()?;
    let mut session = None;
    let mut rest = args.iter();
    let mut command = None;
    while let Some(arg) = rest.next() {
        let next = match arg.as_str() {
            "-h" | "--help" => Command::Help,
            "-V" | "--version" => Command::Version,
            "--default-config" => Command::DefaultConfig,
            "--session" => {
                let name = rest.next().ok_or("--session needs a name")?;
                session = Some(name.clone());
                continue;
            }
            "config" => match rest.next().map(String::as_str) {
                Some("check") => Command::ConfigCheck,
                Some("reset-keys") => Command::ResetKeys,
                Some(other) => return Err(format!("unknown config command: {other}")),
                None => return Err("config needs a command: check or reset-keys".into()),
            },
            other => match other.strip_prefix("--session=") {
                Some(name) => {
                    session = Some(name.to_owned());
                    continue;
                }
                None => return Err(format!("unknown argument: {other}")),
            },
        };
        if command.replace(next).is_some() {
            return Err("give one command at a time".into());
        }
    }
    if let Some(name) = &session
        && name.trim().is_empty()
    {
        return Err("--session needs a name".into());
    }
    Ok(command.unwrap_or(Command::Run { session }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(OsString::from))
    }

    const VERSION: &str = env!("CARGO_PKG_VERSION");

    #[test]
    fn plain_help_and_version_show_the_hat_shape() {
        let hat = [
            "    ██▄▄██",
            "█▄  ██████  ▄█",
            " ▀▀▀▀▀▀▀▀▀▀▀▀ ",
            "     ████",
            "     ████",
            "     ████",
        ];
        let help = [
            hat[0].to_owned(),
            hat[1].to_owned(),
            format!("{}   rodeo▍", hat[2]),
            format!("{}        {TITLE}", hat[3]),
            hat[4].to_owned(),
            hat[5].to_owned(),
            String::new(),
            USAGE.to_owned(),
        ];
        assert_eq!(super::help(None), help.join("\n"));
        let version = [
            hat[0].to_owned(),
            hat[1].to_owned(),
            format!("{}   rodeo {VERSION}", hat[2]),
            hat[3].to_owned(),
            hat[4].to_owned(),
            hat[5].to_owned(),
        ];
        assert_eq!(super::version(None), version.join("\n"));
    }

    #[test]
    fn colored_version_paints_the_hat_and_name() {
        let ink = Ink {
            text: ratatui::style::Color::Reset,
            accent: ratatui::style::Color::Rgb(232, 103, 60),
        };
        let accent = "\x1b[38;2;232;103;60m";
        let out = super::version(Some(ink));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "    ██▄▄██");
        assert_eq!(lines[1], "█▄  \x1b[48;2;232;103;60m▀▀▀▀▀▀\x1b[49m  ▄█");
        assert_eq!(
            lines[2],
            format!(" ▀▀▀▀▀▀▀▀▀▀▀▀    \x1b[1mrodeo\x1b[0m {VERSION}")
        );
        for line in &lines[3..] {
            assert_eq!(*line, format!("     {accent}████\x1b[0m"));
        }
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn colored_help_paints_the_wordmark() {
        let ink = Ink {
            text: ratatui::style::Color::Reset,
            accent: ratatui::style::Color::Blue,
        };
        let out = help(Some(ink));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[2],
            " ▀▀▀▀▀▀▀▀▀▀▀▀    \x1b[1mrodeo\x1b[0m\x1b[38;5;4m▍\x1b[0m"
        );
        assert_eq!(
            lines[3],
            format!("     \x1b[38;5;4m████\x1b[39m        {TITLE}")
        );
        assert!(out.ends_with(USAGE));
    }

    #[test]
    fn commands_parse() {
        assert_eq!(run(&[]), Ok(Command::Run { session: None }));
        assert_eq!(
            run(&["--session", "work"]),
            Ok(Command::Run {
                session: Some("work".into())
            })
        );
        assert_eq!(
            run(&["--session=work"]),
            Ok(Command::Run {
                session: Some("work".into())
            })
        );
        assert_eq!(run(&["config", "check"]), Ok(Command::ConfigCheck));
        assert_eq!(run(&["config", "reset-keys"]), Ok(Command::ResetKeys));
        assert_eq!(run(&["--default-config"]), Ok(Command::DefaultConfig));
        assert_eq!(run(&["-V"]), Ok(Command::Version));
        assert!(run(&["--session"]).is_err());
        assert!(run(&["config"]).is_err());
        assert!(run(&["--nope"]).is_err());
        assert!(run(&["--help", "--version"]).is_err());
    }
}
