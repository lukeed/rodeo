use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::client::Client;
use crate::env::Env;
use crate::paths::Paths;
use crate::start::{find_hc, start_server};

const SESSION: &str = "t";
const STRAGGLERS: Duration = Duration::from_secs(2);

pub struct TestCore {
    root: tempfile::TempDir,
    env: Env,
    bin: PathBuf,
}

impl std::fmt::Debug for TestCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestCore")
            .field("root", &self.root.path())
            .finish_non_exhaustive()
    }
}

impl TestCore {
    pub async fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("hc")
            .tempdir_in(std::fs::canonicalize("/tmp").expect("/tmp resolves"))
            .expect("a temp dir for the test core");
        let path = root.path();
        let env = Env::new()
            .with("HOME", path)
            .with("PATH", std::env::var_os("PATH").unwrap_or_default())
            .with("SHELL", "/bin/sh")
            .with("HC_SESSION", SESSION)
            .with("HC_SETTLE_MS", "0")
            .with("XDG_RUNTIME_DIR", path.join("run"))
            .with("XDG_STATE_HOME", path.join("state"))
            .with("XDG_CONFIG_HOME", path.join("config"));
        let bin = find_hc(&env)
            .await
            .expect("hc must be installed for live hc tests");
        Self { root, env, bin }
    }

    pub async fn start() -> Self {
        Self::start_with(|_, _| {}).await
    }

    pub async fn start_with(prepare: impl FnOnce(&Path, &mut Env)) -> Self {
        let mut core = Self::new().await;
        prepare(core.root.path(), &mut core.env);
        if let Err(error) = start_server(&core.bin, &core.env).await {
            panic!("hc server start: {error}");
        }
        core
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub const fn env(&self) -> &Env {
        &self.env
    }

    pub fn paths(&self) -> Paths {
        Paths::resolve(&self.env).expect("test core paths")
    }

    pub fn bin(&self) -> &Path {
        &self.bin
    }

    pub async fn client(&self) -> Client {
        match Client::connect_env(&self.env).await {
            Ok(client) => client,
            Err(error) => panic!("connect to the test core: {error}"),
        }
    }

    pub fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command.env_clear().envs(self.env.iter());
        command
    }

    pub fn hc(&self) -> Command {
        self.command(&self.bin)
    }

    fn stop(&self) -> Vec<String> {
        let _ = self
            .hc()
            .args(["server", "stop"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let marker = format!("XDG_STATE_HOME={}", self.root().join("state").display());
        let started = Instant::now();
        while !holding(&marker).is_empty() && started.elapsed() < STRAGGLERS {
            std::thread::sleep(Duration::from_millis(50));
        }
        holding(&marker)
            .into_iter()
            .map(|pid| {
                let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
                let _ = Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .stderr(Stdio::null())
                    .status();
                format!(
                    "pid {pid} ({})",
                    String::from_utf8_lossy(&cmdline).replace('\0', " ").trim()
                )
            })
            .collect()
    }
}

impl Drop for TestCore {
    fn drop(&mut self) {
        let leaks = self.stop();
        assert!(
            leaks.is_empty() || std::thread::panicking(),
            "processes outlived hc server stop:\n{}",
            leaks.join("\n")
        );
    }
}

fn holding(marker: &str) -> Vec<u32> {
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let marker = marker.as_bytes();
    procs
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| *pid != std::process::id())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/environ"))
                .is_ok_and(|environ| environ.split(|b| *b == 0).any(|var| var == marker))
        })
        .collect()
}
