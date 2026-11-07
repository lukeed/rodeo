use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;

use crate::env::{Env, path_dirs, usual_dirs};
use crate::error::{HcError, RemoteError};

const NAMES: [&str; 2] = ["herdr-core", "hc"];
const START_WAIT: Duration = Duration::from_secs(30);
const VERSION_WAIT: Duration = Duration::from_secs(5);

pub async fn find_hc(env: &Env) -> Option<PathBuf> {
    let mut dirs = path_dirs(env);
    dirs.extend(usual_dirs(&env.home()));
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|dir| seen.insert(dir.clone()));
    for dir in dirs {
        for name in NAMES {
            let candidate = dir.join(name);
            if candidate.is_file() && hc_version(&candidate).await.is_some() {
                return Some(candidate);
            }
        }
    }
    None
}

pub async fn hc_version(bin: &Path) -> Option<String> {
    let run = tokio::process::Command::new(bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(VERSION_WAIT, run).await.ok()?.ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && text.starts_with("hc ")).then_some(text)
}

#[derive(Deserialize)]
struct Failure {
    error: RemoteError,
}

pub async fn start_server(bin: &Path, env: &Env) -> Result<(), HcError> {
    let run = env
        .command(bin)
        .args(["server", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(START_WAIT, run)
        .await
        .map_err(|_| HcError::Timeout {
            method: "hc server start".into(),
            after: START_WAIT,
        })??;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(serde_json::from_str::<Failure>(stderr.trim()).map_or_else(
        |_| {
            HcError::Remote(RemoteError {
                kind: "start".into(),
                message: format!("hc server start failed ({}): {}", out.status, stderr.trim()),
                hint: None,
            })
        },
        |failure| HcError::Remote(failure.error),
    ))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[tokio::test]
    async fn only_a_binary_that_says_hc_counts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let other = dir.path().join("other");
        let real = dir.path().join("real");
        std::fs::create_dir_all(&other).expect("mkdir");
        std::fs::create_dir_all(&real).expect("mkdir");
        script(&other, "hc", "echo 'herbstclient 0.9'");
        let core = script(&real, "herdr-core", "echo 'hc 9.9.9'");
        let path = std::env::join_paths([&other, &real]).expect("path");
        let env = Env::new().with("PATH", path).with("HOME", dir.path());
        assert_eq!(find_hc(&env).await, Some(core.clone()));
        assert_eq!(hc_version(&core).await.as_deref(), Some("hc 9.9.9"));
        assert_eq!(hc_version(&other.join("hc")).await, None);
    }

    #[tokio::test]
    async fn a_failed_start_reports_hcs_own_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = script(
            dir.path(),
            "hc",
            r#"echo '{"error":{"kind":"usage","message":"no","hint":"fix it"}}' >&2; exit 2"#,
        );
        let failed = start_server(&bin, &Env::new()).await.expect_err("fails");
        assert_eq!(failed.kind(), Some("usage"));
        assert_eq!(
            failed.remote().and_then(|r| r.hint.as_deref()),
            Some("fix it")
        );
    }
}
