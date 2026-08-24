//! Bot-side ACP session policy helpers.
//!
//! IM bots must not resume or drive externally projected ACP sessions through
//! the native SessionManager / `send_message` path. These helpers are pure so
//! product assembly and contract tests can share one decision.

use bitfun_core_types::SESSION_PROVIDER_ACP;
use bitfun_runtime_ports::RemoteSessionKind;
use serde_json::Value;

/// True when persisted custom metadata marks the session as ACP-owned.
pub fn is_acp_session_provider(provider: Option<&str>) -> bool {
    provider == Some(SESSION_PROVIDER_ACP)
}

/// True when a remote `SessionInfo` JSON row is an ACP session.
///
/// Prefer `session_kind`; never treat missing/unknown kind as native — use the
/// `acp_remote_control` capability as a secondary signal for older payloads.
pub fn remote_session_json_is_acp(session: &Value) -> bool {
    match session.get("session_kind").and_then(Value::as_str) {
        Some(kind) if kind == RemoteSessionKind::Acp.as_wire_str() => true,
        Some(kind) if kind == RemoteSessionKind::Native.as_wire_str() => false,
        _ => session
            .get("capabilities")
            .and_then(Value::as_array)
            .is_some_and(|caps| {
                caps.iter().any(|cap| {
                    cap.as_str() == Some(bitfun_runtime_ports::REMOTE_CAPABILITY_ACP_REMOTE_CONTROL)
                })
            }),
    }
}

/// When a remote RPC body is `RemoteResponse::Error`, return its message.
pub fn remote_rpc_error_message(resp_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(resp_json).ok()?;
    if value.get("resp").and_then(Value::as_str) != Some("error") {
        return None;
    }
    Some(
        value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Remote command failed")
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_marks_only_acp() {
        assert!(is_acp_session_provider(Some("acp")));
        assert!(!is_acp_session_provider(Some("native")));
        assert!(!is_acp_session_provider(None));
    }

    #[test]
    fn remote_json_uses_session_kind_and_capability_fallback() {
        assert!(remote_session_json_is_acp(&json!({
            "session_id": "a",
            "session_kind": "acp"
        })));
        assert!(!remote_session_json_is_acp(&json!({
            "session_id": "a",
            "session_kind": "native"
        })));
        assert!(!remote_session_json_is_acp(&json!({
            "session_id": "a",
            "session_kind": "unknown"
        })));
        assert!(remote_session_json_is_acp(&json!({
            "session_id": "a",
            "session_kind": "unknown",
            "capabilities": ["acp_remote_control"]
        })));
        assert!(remote_session_json_is_acp(&json!({
            "session_id": "a",
            "capabilities": ["acp_remote_control"]
        })));
        assert!(!remote_session_json_is_acp(&json!({
            "session_id": "a",
            "capabilities": ["other"]
        })));
    }

    #[test]
    fn remote_rpc_error_is_detected_and_message_extracted() {
        assert_eq!(
            remote_rpc_error_message(
                r#"{"resp":"error","message":"ACP sessions require acp_remote_control"}"#
            )
            .as_deref(),
            Some("ACP sessions require acp_remote_control")
        );
        assert_eq!(
            remote_rpc_error_message(r#"{"resp":"message_sent","turn_id":"t1"}"#),
            None
        );
    }
}
