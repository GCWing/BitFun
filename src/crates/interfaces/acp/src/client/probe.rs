//! Two-step probe for ACP agent connectivity.
//!
//! Step 1: `which` check — detect CLI on system PATH (5 s timeout).
//! Step 2: Spawn + ACP initialize + session/new handshake (30 s timeout).
//!
//! The probe always cleans up the spawned process, including any
//! grandchild processes orphaned by wrapper CLIs.

use serde::{Deserialize, Serialize};

/// Two-step probe result for ACP agent connectivity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum TryConnectResult {
    /// Both steps succeeded — agent is reachable and usable.
    Success,
    /// Step 1 failed — the CLI command was not found on PATH.
    FailCli { error: String },
    /// Step 2 failed — ACP initialize or session/new failed.
    FailAcp { error: String },
    /// Step 2 reached initialize but session/new failed with auth.
    FailAuth {
        error: String,
        /// Login guidance for the client when the provider exposes one.
        #[serde(default)]
        login_hint: Option<String>,
    },
}

/// Timeout for Step 1: CLI detect on PATH.
pub const CLI_DETECT_TIMEOUT_SECS: u64 = 5;

/// Timeout for Step 2: ACP initialize + session/new handshake.
pub const ACP_HANDSHAKE_TIMEOUT_SECS: u64 = 30;

/// Total probe timeout (Step 1 + Step 2 upper bound).
pub const TRY_CONNECT_TOTAL_TIMEOUT_SECS: u64 = 35;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_auth_serializes_with_login_hint() {
        let result = TryConnectResult::FailAuth {
            error: "session/new failed: auth_required".to_string(),
            login_hint: Some("Run `codex login` in a terminal".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(
            json,
            r#"{"step":"fail_auth","error":"session/new failed: auth_required","login_hint":"Run `codex login` in a terminal"}"#
        );
    }

    #[test]
    fn fail_auth_deserializes_legacy_json_without_login_hint() {
        let legacy = r#"{"step":"fail_auth","error":"session/new failed: auth_required"}"#;

        let result: TryConnectResult = serde_json::from_str(legacy).unwrap();

        match result {
            TryConnectResult::FailAuth { error, login_hint } => {
                assert_eq!(error, "session/new failed: auth_required");
                assert!(login_hint.is_none());
            }
            other => panic!("expected FailAuth, got {other:?}"),
        }
    }
}
