use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run { session: Option<String> },
    Help,
    Version,
    DefaultConfig,
    ConfigCheck,
    ResetKeys,
}

pub const USAGE: &str = "rodeo: a terminal UI for herdr-core

usage:
  rodeo [--session <name>]   open rodeo on an hc session
  rodeo --default-config     print the default config
  rodeo config check         check the config file
  rodeo config reset-keys    remove [keys] from the config (keeps a backup)
  rodeo --help | --version

Detach with the detach key (prefix+q). Terminals keep running in hc.";

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
