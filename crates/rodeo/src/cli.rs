use std::ffi::OsString;

use ratatui::style::Color;
use ratatui::text::Line;

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

pub fn help(color: bool) -> String {
    let ink = color.then_some(Ink {
        text: Color::Reset,
        accent: brand::RUST,
    });
    let wordmark = brand::wordmark(ink.unwrap_or(Ink::PLAIN));
    framed(&format!(
        "{}\n\n{USAGE}",
        lockup(&[wordmark, Line::from(TITLE)], ink)
    ))
}

pub fn version() -> String {
    format!("rodeo {}", env!("CARGO_PKG_VERSION"))
}

fn framed(text: &str) -> String {
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect();
    format!("\n{}\n", lines.join("\n"))
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
    fn version_is_one_plain_line() {
        assert_eq!(super::version(), format!("rodeo {VERSION}"));
    }

    #[test]
    fn plain_help_is_framed_and_indented() {
        let out = super::help(false);
        let lines: Vec<&str> = out.split('\n').collect();
        assert_eq!(lines[0], "");
        assert_eq!(lines[1], "      ██▄▄██");
        assert_eq!(lines[2], "  █▄  ██████  ▄█");
        assert_eq!(lines[3], "   ▀▀▀▀▀▀▀▀▀▀▀▀    rodeo▍");
        assert_eq!(lines[4], format!("       ████        {TITLE}"));
        assert_eq!(lines[6], "       ████");
        assert_eq!(lines[7], "");
        assert_eq!(lines[8], "  usage:");
        assert_eq!(
            lines[9],
            "    rodeo [--session <name>]   open rodeo on an hc session"
        );
        assert_eq!(lines[14], "");
        assert_eq!(
            lines[15],
            "  Detach with the detach key (prefix+q). Terminals keep running in hc."
        );
        assert_eq!(lines[16], "");
        assert_eq!(lines.len(), 17);
    }

    #[test]
    fn colored_help_paints_the_brand_rust() {
        let rust = "\x1b[38;2;232;103;60m";
        let out = help(true);
        let lines: Vec<&str> = out.split('\n').collect();
        assert_eq!(lines[2], "  █▄  \x1b[48;2;232;103;60m▀▀▀▀▀▀\x1b[49m  ▄█");
        assert_eq!(
            lines[3],
            format!("   ▀▀▀▀▀▀▀▀▀▀▀▀    \x1b[1mrodeo\x1b[0m{rust}▍\x1b[0m")
        );
        assert_eq!(
            lines[4],
            format!("       {rust}████\x1b[39m        {TITLE}")
        );
        assert!(out.ends_with("running in hc.\n"));
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
