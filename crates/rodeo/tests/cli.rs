use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_rodeo");
const ACCENT: &str = "\x1b[38;2;137;180;250m";
const BRIM: &str = " ▀▀▀▀▀▀▀▀▀▀▀▀ ";

struct Root(PathBuf);

impl Root {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("rodeo-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("config")).expect("a temp root");
        Self(path)
    }

    fn env(&self) -> Vec<String> {
        vec![
            format!("HOME={}", self.0.display()),
            format!("XDG_CONFIG_HOME={}", self.0.join("config").display()),
            format!("PATH={}", std::env::var("PATH").unwrap_or_default()),
            "TERM=xterm-256color".to_owned(),
        ]
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tmux(socket: &str, args: &[&str]) -> Output {
    let mut command = Command::new("tmux");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("HERDR_") {
            command.env_remove(key);
        }
    }
    command
        .arg("-L")
        .arg(socket)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("tmux runs")
}

fn on_a_tty(name: &str, extra: &[&str]) -> String {
    let root = Root::new(name);
    let socket = format!("rodeo-cli-{name}-{}", std::process::id());
    let mut command = vec!["env".to_owned(), "-i".to_owned()];
    command.extend(root.env());
    command.extend(extra.iter().map(|&s| s.to_owned()));
    command.extend([BIN.to_owned(), "--version".to_owned()]);
    let line = format!("sh -c 'read go && exec {}'", command.join(" "));
    let started = tmux(
        &socket,
        &[
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-x",
            "100",
            "-y",
            "30",
            &line,
        ],
    );
    assert!(started.status.success(), "{started:?}");
    tmux(&socket, &["set-option", "remain-on-exit", "on"]);
    tmux(&socket, &["send-keys", "Enter"]);
    let begun = Instant::now();
    let screen = loop {
        let dead = tmux(&socket, &["display-message", "-p", "#{pane_dead}"]);
        let out = tmux(&socket, &["capture-pane", "-p", "-e"]);
        let screen = String::from_utf8_lossy(&out.stdout).into_owned();
        if String::from_utf8_lossy(&dead.stdout).trim() == "1" && screen.contains("████") {
            break screen;
        }
        assert!(begun.elapsed() < Duration::from_secs(20), "{screen:?}");
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = tmux(&socket, &["kill-server"]);
    screen
}

#[test]
fn a_tty_gets_the_colored_hat() {
    let screen = on_a_tty("color", &[]);
    assert!(screen.contains(ACCENT), "{screen:?}");
    assert!(screen.contains("\x1b[1mrodeo"), "{screen:?}");
    assert!(
        screen.contains("\x1b[48;2;137;180;250m▀▀▀▀▀▀"),
        "{screen:?}"
    );
}

#[test]
fn no_color_gets_the_plain_hat_on_a_tty() {
    let screen = on_a_tty("nocolor", &["NO_COLOR=1"]);
    assert!(!screen.contains("\x1b["), "{screen:?}");
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        screen.contains(&format!("{BRIM}   rodeo {version}")),
        "{screen:?}"
    );
    assert!(
        screen.starts_with("\n      ██▄▄██\n  █▄  ██████  ▄█\n"),
        "{screen:?}"
    );
}

#[test]
fn a_pipe_gets_plain_help_and_version() {
    let root = Root::new("pipe");
    for (arg, beside) in [
        ("--help", "rodeo▍".to_owned()),
        ("--version", format!("rodeo {}", env!("CARGO_PKG_VERSION"))),
    ] {
        let out = Command::new("env")
            .arg("-i")
            .args(root.env())
            .arg(BIN)
            .arg(arg)
            .output()
            .expect("rodeo runs");
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).expect("utf-8");
        assert!(!text.contains('\x1b'), "{text:?}");
        assert!(text.starts_with("\n      ██▄▄██\n"), "{text:?}");
        assert!(
            text.ends_with(".\n\n") || text.ends_with("████\n\n"),
            "{text:?}"
        );
        let lines: Vec<&str> = text.split('\n').collect();
        assert_eq!(lines[0], "");
        assert_eq!(lines[1], "      ██▄▄██");
        assert_eq!(lines[2], "  █▄  ██████  ▄█");
        assert_eq!(lines[3], format!("  {BRIM}   {beside}"));
        assert_eq!(lines[6], "       ████");
        assert_eq!(lines[lines.len() - 2], "");
        assert_eq!(lines[lines.len() - 1], "");
    }
}
