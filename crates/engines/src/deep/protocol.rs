//! The NDJSON wire protocol between Craft Anything and an external Deep-engine bridge.
//!
//! Every message is one JSON object on one line (`\n`-terminated). A request carries exactly four
//! keys; the token is checked in constant time by the bridge. A response echoes the request id and
//! either an `ok` result or an `error` with a stable code.

use craft_anything_evidence::{ErrorCode, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A request we send to the bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub token: String,
    pub method: String,
    pub params: Value,
}

/// A response from the bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireError {
    pub code: String,
    pub message: String,
}

/// Why a bridge request failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeepError {
    #[error("could not start the Deep engine: {0}")]
    Launch(String),
    #[error("the Deep engine timed out: {0}")]
    Timeout(String),
    #[error("the Deep bridge connection failed: {0}")]
    Connection(String),
    #[error("the Deep bridge sent something we couldn't read: {0}")]
    Protocol(String),
    #[error("the bridge rejected the request ({code}): {message}")]
    Bridge { code: String, message: String },
    #[error("cancelled")]
    Cancelled,
}

impl DeepError {
    /// Turn a bridge failure into the engine's error contract.
    pub fn into_tool_error(self) -> ToolError {
        match self {
            DeepError::Launch(m) => ToolError::new(ErrorCode::EngineMissing, format!("The Deep engine could not start: {m}"))
                .with_remediation("Check the external engine in Settings → Engines."),
            DeepError::Timeout(m) => {
                ToolError::new(ErrorCode::TimedOut, m).with_remediation("Give the external engine more time, or analyse a smaller target.")
            }
            DeepError::Connection(m) => ToolError::new(ErrorCode::EngineMissing, format!("Lost the Deep bridge: {m}")),
            DeepError::Protocol(m) => ToolError::new(ErrorCode::GarbledOutput, format!("Deep bridge protocol error: {m}")),
            DeepError::Bridge { code, message } => match code.as_str() {
                "not_found" => ToolError::new(ErrorCode::BadInput, message),
                "unknown_method" => ToolError::new(ErrorCode::FeatureMissing, message),
                "analysis_unfinished" => ToolError::new(ErrorCode::TimedOut, message),
                "decompile_stopped" => ToolError::cancelled(),
                _ => ToolError::new(ErrorCode::ProgramFailed, message),
            },
            DeepError::Cancelled => ToolError::cancelled(),
        }
    }
}

impl From<DeepError> for ToolError {
    fn from(e: DeepError) -> Self {
        e.into_tool_error()
    }
}

/// Constant-time byte comparison, so a token check leaks nothing through timing.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_matches_only_identical() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn requests_round_trip() {
        let r = Request { id: 7, token: "tok".into(), method: "ping".into(), params: serde_json::json!({}) };
        let line = serde_json::to_string(&r).unwrap();
        let back: Request = serde_json::from_str(&line).unwrap();
        assert_eq!(back.id, 7);
        assert_eq!(back.method, "ping");
    }
}
