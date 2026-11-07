use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::wire::WireError;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawRemote")]
pub struct RemoteError {
    pub kind: String,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Deserialize)]
struct RawRemote {
    #[serde(default)]
    message: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    hint: Option<String>,
    #[serde(default)]
    data: Option<RawData>,
}

#[derive(Deserialize)]
struct RawData {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    hint: Option<String>,
}

impl From<RawRemote> for RemoteError {
    fn from(raw: RawRemote) -> Self {
        let (data_kind, data_hint) = raw.data.map_or((None, None), |d| (d.kind, d.hint));
        Self {
            kind: data_kind.or(raw.kind).unwrap_or_default(),
            message: raw.message,
            hint: data_hint.or(raw.hint).filter(|hint| !hint.is_empty()),
        }
    }
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Mismatch {
    #[error("hc speaks protocol generation {}, drover speaks {}", .0.map_or_else(|| "another".to_owned(), |g| g.to_string()), crate::GENERATION)]
    Generation(Option<u32>),
    #[error("hc lacks methods drover needs: {}", .0.join(", "))]
    Methods(Vec<String>),
}

#[derive(Debug, thiserror::Error)]
pub enum HcError {
    #[error("cannot reach hc: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error("hc closed the connection")]
    Closed,
    #[error("hc sent malformed json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hc {kind}: {0}", kind = .0.kind)]
    Remote(RemoteError),
    #[error("hc doc conflict: {0}")]
    Conflict(RemoteError),
    #[error("hc gave no answer to {method} within {}s", .after.as_secs_f32())]
    Timeout { method: String, after: Duration },
    #[error("hc gave no channel for {0}")]
    NoChannel(String),
    #[error("hc cannot serve drover: {0}")]
    Unsupported(#[from] Mismatch),
    #[error("{0}")]
    Paths(String),
    #[error("cannot find hc: install herdr-core, or put hc on PATH")]
    NoBinary,
    #[error("hc listens at {}, but this env looks at {}", .actual.display(), .expected.display())]
    Elsewhere { actual: PathBuf, expected: PathBuf },
}

impl HcError {
    pub fn kind(&self) -> Option<&str> {
        match self {
            Self::Remote(remote) | Self::Conflict(remote) => Some(&remote.kind),
            _ => None,
        }
    }

    pub fn remote(&self) -> Option<&RemoteError> {
        match self {
            Self::Remote(remote) | Self::Conflict(remote) => Some(remote),
            _ => None,
        }
    }

    pub fn is_kind(&self, kind: &str) -> bool {
        self.kind() == Some(kind)
    }

    pub fn is_conflict(&self) -> bool {
        self.is_kind("conflict")
    }

    pub fn is_not_found(&self) -> bool {
        self.is_kind("not_found")
    }

    pub fn is_finished(&self) -> bool {
        self.is_kind("finished")
    }

    pub fn is_unavailable(&self) -> bool {
        self.is_kind("unavailable")
    }

    pub fn is_machine_unknown(&self) -> bool {
        self.is_kind("machine_unknown")
    }

    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_kind_and_hint_from_rpc_data_and_from_cli_errors() {
        let rpc: RemoteError = serde_json::from_value(json!({
            "code": -32000, "message": "document \"x\" is at version 3, not 2",
            "data": {"kind": "conflict", "hint": "read doc.get and retry with its version"}
        }))
        .unwrap();
        assert_eq!(rpc.kind, "conflict");
        assert_eq!(
            rpc.hint.as_deref(),
            Some("read doc.get and retry with its version")
        );
        let cli: RemoteError = serde_json::from_value(json!({
            "kind": "usage", "message": "bad session", "hint": ""
        }))
        .unwrap();
        assert_eq!((cli.kind.as_str(), cli.hint), ("usage", None));
    }

    #[test]
    fn helpers_match_on_kind_only() {
        let remote = RemoteError {
            kind: "conflict".into(),
            message: "not_found".into(),
            hint: None,
        };
        assert!(HcError::Conflict(remote.clone()).is_conflict());
        assert!(HcError::Remote(remote.clone()).is_conflict());
        assert!(!HcError::Remote(remote).is_not_found());
        assert!(!HcError::Closed.is_conflict());
    }
}
