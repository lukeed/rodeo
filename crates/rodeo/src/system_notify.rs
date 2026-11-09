use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(10);

pub fn commands(title: &str, body: &str) -> Vec<Vec<String>> {
    let own = |parts: &[&str]| parts.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>();
    if cfg!(target_os = "macos") {
        let script = format!(
            "display notification {} with title {}",
            apple_string(body),
            apple_string(title)
        );
        vec![
            own(&[
                "terminal-notifier",
                "-title",
                title,
                "-message",
                body,
                "-group",
                "rodeo",
            ]),
            own(&["osascript", "-e", &script]),
        ]
    } else {
        vec![
            own(&["notify-send", "--app-name=rodeo", "--", title, body]),
            own(&[
                "gdbus",
                "call",
                "--session",
                "--dest=org.freedesktop.Notifications",
                "--object-path=/org/freedesktop/Notifications",
                "--method=org.freedesktop.Notifications.Notify",
                "rodeo",
                "0",
                "",
                title,
                body,
                "[]",
                "{}",
                "-1",
            ]),
        ]
    }
}

fn apple_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

pub fn show(title: &str, body: &str) {
    let commands = commands(&clean(title), &clean(body));
    let spawned = std::thread::Builder::new()
        .name("rodeo-notify".into())
        .spawn(move || {
            for argv in commands {
                if run(&argv) {
                    return;
                }
            }
            tracing::debug!("no system notification service answered");
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "cannot start the notification thread");
    }
}

fn run(argv: &[String]) -> bool {
    let Some((program, rest)) = argv.split_first() else {
        return false;
    };
    let Ok(mut child) = Command::new(program)
        .args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if started.elapsed() < LIMIT => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_carry_the_text_as_arguments() {
        let commands = commands("claude finished", "repo · 1");
        assert!(commands.len() >= 2);
        for argv in &commands {
            assert!(
                argv.iter().any(|a| a.contains("claude finished")),
                "{argv:?}"
            );
            assert!(argv.iter().any(|a| a.contains("repo · 1")), "{argv:?}");
        }
        assert_eq!(apple_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(clean("a\x1b]0;x\x07b"), "a]0;xb");
    }
}
