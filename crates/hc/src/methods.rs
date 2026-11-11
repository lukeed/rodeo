use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::error::HcError;
use crate::model::{CoreInfo, Display, Terminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RunParams {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<Display>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_parent: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_notify: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub outlive_parent: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rm: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub keep: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListParams {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TerminalList {
    pub terminals: Vec<Terminal>,
    #[serde(default)]
    pub hidden_finished: usize,
    #[serde(default)]
    pub machines: Vec<MachineView>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct MachineView {
    pub name: String,
    pub target: String,
    pub via: bool,
    pub temporary: bool,
    pub session: Option<String>,
    pub id: String,
    pub host: String,
    pub version: String,
    pub state: String,
    pub since_ms: Option<u64>,
    pub why: Option<String>,
}

impl MachineView {
    pub fn is_connected(&self) -> bool {
        self.state == "connected"
    }
}

#[derive(Deserialize)]
struct Machines {
    machines: Vec<MachineView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Put {
    pub path: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Read {
    pub text: String,
    pub complete: bool,
    pub how: String,
    pub why: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Doc {
    pub value: Value,
    pub version: u64,
}

#[derive(Deserialize)]
struct DocVersion {
    version: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ExecParams {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "seconds")]
    pub timeout: Option<Duration>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "base64_bytes"
    )]
    pub input: Option<Vec<u8>>,
}

const EXEC_DEFAULT: Duration = Duration::from_secs(60);
const EXEC_GRACE: Duration = Duration::from_secs(10);
const MACHINE_CONNECT: Duration = Duration::from_secs(30);

#[allow(clippy::ref_option, reason = "serde passes a reference to the field")]
fn seconds<S: serde::Serializer>(timeout: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
    match timeout {
        Some(timeout) => s.serialize_f64(timeout.as_secs_f64()),
        None => s.serialize_none(),
    }
}

#[allow(clippy::ref_option, reason = "serde passes a reference to the field")]
fn base64_bytes<S: serde::Serializer>(bytes: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
    match bytes {
        Some(bytes) => s.serialize_str(&STANDARD.encode(bytes)),
        None => s.serialize_none(),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exec {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub truncated: bool,
    pub timed_out: bool,
    pub ms: u64,
}

impl Exec {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

#[derive(Deserialize)]
struct RawExec {
    code: Option<i32>,
    #[serde(default)]
    signal: Option<i32>,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
    #[serde(default)]
    truncated: bool,
    #[serde(default)]
    timed_out: bool,
    #[serde(default)]
    ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Holder {
    pub pid: u32,
    pub instance: String,
    pub current: bool,
    pub terminals: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ServerStatus {
    pub core: CoreInfo,
    pub version: String,
    pub pid: u32,
    pub uptime_ms: u64,
    pub holder: Option<Holder>,
    pub holders: Vec<Holder>,
    pub live: usize,
    pub finished: usize,
    pub installed_by: Option<String>,
}

fn decode(field: &str, text: &str) -> Result<Vec<u8>, HcError> {
    STANDARD.decode(text).map_err(|error| {
        HcError::Json(serde::de::Error::custom(format!(
            "{field} is not base64: {error}"
        )))
    })
}

fn on_machine(mut params: Value, on: Option<&str>) -> Value {
    if let Some(on) = on {
        params["on"] = on.into();
    }
    params
}

impl Client {
    pub async fn run(&self, params: &RunParams) -> Result<Terminal, HcError> {
        self.call("terminal.run", serde_json::to_value(params)?)
            .await
    }

    pub async fn list(&self, params: &ListParams) -> Result<TerminalList, HcError> {
        self.call("terminal.list", serde_json::to_value(params)?)
            .await
    }

    pub async fn get(&self, terminal: &str) -> Result<Terminal, HcError> {
        self.call("terminal.get", json!({"terminal": terminal}))
            .await
    }

    pub async fn kill(&self, terminal: &str) -> Result<Terminal, HcError> {
        self.call("terminal.kill", json!({"terminal": terminal}))
            .await
    }

    pub async fn keep(&self, terminal: &str, keep: bool) -> Result<Terminal, HcError> {
        self.call("terminal.keep", json!({"terminal": terminal, "keep": keep}))
            .await
    }

    pub async fn label<K, V>(
        &self,
        terminal: &str,
        labels: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Terminal, HcError>
    where
        K: Into<String>,
        V: Into<String>,
    {
        let labels: BTreeMap<String, String> = labels
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        self.call(
            "terminal.label",
            json!({"terminal": terminal, "labels": labels}),
        )
        .await
    }

    pub async fn put(&self, terminal: &str, name: &str, data: &[u8]) -> Result<Put, HcError> {
        self.call(
            "terminal.put",
            json!({"terminal": terminal, "name": name, "data": STANDARD.encode(data)}),
        )
        .await
    }

    pub async fn read(&self, terminal: &str, lines: usize) -> Result<Read, HcError> {
        self.call(
            "terminal.read",
            json!({"terminal": terminal, "lines": lines}),
        )
        .await
    }

    pub async fn doc_get(&self, name: &str, on: Option<&str>) -> Result<Doc, HcError> {
        self.call("doc.get", on_machine(json!({"name": name}), on))
            .await
    }

    pub async fn doc_set(
        &self,
        name: &str,
        value: &Value,
        version: u64,
        on: Option<&str>,
    ) -> Result<u64, HcError> {
        let params = on_machine(
            json!({"name": name, "value": value, "version": version}),
            on,
        );
        match self.call::<DocVersion>("doc.set", params).await {
            Ok(set) => Ok(set.version),
            Err(HcError::Remote(remote)) if remote.kind == "conflict" => {
                Err(HcError::Conflict(remote))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn machines(&self) -> Result<Vec<MachineView>, HcError> {
        let list: Machines = self.call("machine.list", json!({})).await?;
        Ok(list.machines)
    }

    pub async fn machine_rename(
        &self,
        machine: &str,
        new_name: &str,
    ) -> Result<MachineView, HcError> {
        self.call(
            "machine.rename",
            json!({"machine": machine, "new_name": new_name}),
        )
        .await
    }

    pub async fn machine_rm(&self, machine: &str) -> Result<(), HcError> {
        let _: serde::de::IgnoredAny = self.call("machine.rm", json!({"machine": machine})).await?;
        Ok(())
    }

    pub async fn machine_connect(&self, machine: &str) -> Result<MachineView, HcError> {
        self.call_within(
            "machine.connect",
            json!({"machine": machine}),
            MACHINE_CONNECT.max(self.timeout()),
        )
        .await
    }

    pub async fn exec(&self, params: &ExecParams) -> Result<Exec, HcError> {
        let within = params.timeout.unwrap_or(EXEC_DEFAULT) + EXEC_GRACE;
        let raw: RawExec = self
            .call_within(
                "machine.exec",
                serde_json::to_value(params)?,
                within.max(self.timeout()),
            )
            .await?;
        Ok(Exec {
            code: raw.code,
            signal: raw.signal,
            stdout: decode("stdout", &raw.stdout)?,
            stderr: decode("stderr", &raw.stderr)?,
            truncated: raw.truncated,
            timed_out: raw.timed_out,
            ms: raw.ms,
        })
    }

    pub async fn status(&self) -> Result<ServerStatus, HcError> {
        self.call("server.status", json!({})).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_params_send_only_what_is_set() {
        assert_eq!(
            serde_json::to_value(RunParams::default()).unwrap(),
            json!({})
        );
        let full = RunParams {
            argv: vec!["sh".into()],
            line: Some("echo hi".into()),
            size: Some(Size { cols: 80, rows: 24 }),
            display: Some(Display {
                fg: Some("#ffffff".into()),
                ..Display::default()
            }),
            cwd: Some("/tmp".into()),
            on: Some("rohan".into()),
            env: BTreeMap::from([("TERM".into(), "xterm-256color".into())]),
            labels: BTreeMap::from([("herdr.space".into(), "sp1".into())]),
            keep: true,
            ..RunParams::default()
        };
        assert_eq!(
            serde_json::to_value(full).unwrap(),
            json!({
                "argv": ["sh"], "line": "echo hi", "size": {"cols": 80, "rows": 24},
                "display": {"fg": "#ffffff"}, "cwd": "/tmp", "on": "rohan",
                "env": {"TERM": "xterm-256color"}, "labels": {"herdr.space": "sp1"}, "keep": true
            })
        );
    }

    #[test]
    fn exec_params_carry_seconds_and_base64_input() {
        let params = ExecParams {
            line: Some("cat".into()),
            timeout: Some(Duration::from_millis(1500)),
            input: Some(b"hi\n".to_vec()),
            ..ExecParams::default()
        };
        assert_eq!(
            serde_json::to_value(params).unwrap(),
            json!({"line": "cat", "timeout": 1.5, "input": "aGkK"})
        );
    }

    #[test]
    fn machine_views_carry_their_line_state() {
        let view: MachineView = serde_json::from_value(json!({
            "name": "rohan", "target": "me@rohan", "via": false, "temporary": false, "session": null,
            "id": "abc", "host": "rohan", "version": "0.1.18", "state": "unreachable",
            "since_ms": 5, "why": "ssh failed"
        }))
        .unwrap();
        assert!(!view.is_connected());
        assert_eq!(view.why.as_deref(), Some("ssh failed"));
    }
}
