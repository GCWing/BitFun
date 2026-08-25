//! HostInvoke / DeviceEvent dispatch for CLI Peer Host.

use serde_json::{json, Value};

use bitfun_core::service::remote_connect::remote_server::RemoteResponse;

use super::commands;
use super::control::{
    attach_controller, detach_controller, parse_controller_device_id, peer_mode_ping_value,
};
use super::deny::{is_cli_unsupported_command, is_local_only_command};
use super::state::peer_host_state;

#[derive(Debug, Clone)]
struct HostInvokeBridgeResult {
    ok: bool,
    value: Option<Value>,
    error: Option<String>,
}

impl HostInvokeBridgeResult {
    fn ok_value(value: Value) -> Self {
        Self {
            ok: true,
            value: Some(value),
            error: None,
        }
    }

    fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(message.into()),
        }
    }

    fn into_remote_response(self) -> RemoteResponse {
        RemoteResponse::HostInvokeResult {
            ok: self.ok,
            value: self.value,
            error: self.error,
        }
    }
}

/// Handle `RemoteCommand::HostInvoke` and return a HostInvokeResult envelope.
pub(crate) async fn handle_host_invoke(command: &str, args: Value) -> RemoteResponse {
    handle_host_invoke_inner(command, args)
        .await
        .into_remote_response()
}

async fn handle_host_invoke_inner(command: &str, args: Value) -> HostInvokeBridgeResult {
    if command.is_empty() {
        return HostInvokeBridgeResult::err("HostInvoke command is empty");
    }

    // Detached dispatch is target-owned and must not acquire a Peer controller
    // lease. Route the distinct target command family directly to the durable
    // CLI runner before applying the generic HostInvoke deny list.
    if let Some(verb) = dispatch_target_verb(command) {
        return match crate::dispatch::run_dispatch_verb(verb, args).await {
            Ok(value) => HostInvokeBridgeResult::ok_value(value),
            Err(error) => HostInvokeBridgeResult::err(format!("{error:#}")),
        };
    }

    // Control plane — same special-case path as desktop execute_local_remote_command.
    if command == "peer_control_attach" {
        let controller_id = parse_controller_device_id(&args);
        if controller_id.trim().is_empty() {
            return HostInvokeBridgeResult::err("controller_device_id is required");
        }
        if let Err(error) = attach_controller(controller_id).await {
            return HostInvokeBridgeResult::err(error);
        }
        return HostInvokeBridgeResult::ok_value(json!({ "attached": true }));
    }
    if command == "peer_control_detach" {
        let controller_id = parse_controller_device_id(&args);
        detach_controller(&controller_id).await;
        return HostInvokeBridgeResult::ok_value(json!({ "detached": true }));
    }
    if command == "peer_mode_ping" {
        return HostInvokeBridgeResult::ok_value(peer_mode_ping_value());
    }

    if is_local_only_command(command) {
        return HostInvokeBridgeResult::err(format!(
            "command '{command}' is local-only and cannot run on peer"
        ));
    }
    if is_cli_unsupported_command(command) {
        return HostInvokeBridgeResult::err(format!(
            "command '{command}' is not supported on CLI peer host"
        ));
    }

    let state = match peer_host_state() {
        Ok(s) => s,
        Err(e) => return HostInvokeBridgeResult::err(e),
    };

    match commands::dispatch(command, &args, state).await {
        Ok(value) => HostInvokeBridgeResult::ok_value(value),
        Err(error) => HostInvokeBridgeResult::err(error),
    }
}

fn dispatch_target_verb(command: &str) -> Option<&'static str> {
    match command {
        "dispatch_target_probe" => Some("probe"),
        "dispatch_target_submit" => Some("submit"),
        "dispatch_target_status" => Some("status"),
        "dispatch_target_cancel" => Some("cancel"),
        "dispatch_target_list" => Some("list"),
        "dispatch_target_answer" => Some("answer"),
        "dispatch_target_append" => Some("append"),
        "dispatch_target_continue" => Some("continue"),
        "dispatch_target_query" => Some("query"),
        "dispatch_target_workspace_provision" => Some("workspace-provision"),
        "dispatch_target_workspace_bundle_begin" => Some("workspace-bundle-begin"),
        "dispatch_target_workspace_bundle_chunk" => Some("workspace-bundle-chunk"),
        "dispatch_target_workspace_bundle_commit" => Some("workspace-bundle-commit"),
        "dispatch_target_workspace_sync" => Some("workspace-sync"),
        "dispatch_target_workspace_sync_chunk" => Some("workspace-sync-chunk"),
        _ => None,
    }
}

/// Peer-side DeviceEvent is a no-op ack (controller is the consumer).
pub(crate) fn handle_device_event_command() -> RemoteResponse {
    RemoteResponse::DeviceEventAccepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn peer_mode_ping_returns_ok_peer_payload() {
        let resp = handle_host_invoke("peer_mode_ping", json!({})).await;
        match resp {
            RemoteResponse::HostInvokeResult {
                ok: true,
                value: Some(value),
                error: None,
            } => {
                assert_eq!(value.get("ok"), Some(&json!(true)));
                assert_eq!(value.get("peer"), Some(&json!(true)));
                assert!(value.get("device_id").and_then(|v| v.as_str()).is_some());
                assert_eq!(
                    value.pointer("/capabilities/targeted_session_rollback"),
                    Some(&json!(true))
                );
                assert_eq!(
                    value.pointer("/capabilities/token_usage_statistics"),
                    Some(&json!(true))
                );
                assert_eq!(
                    value.pointer("/capabilities/cancel_tool"),
                    Some(&json!(true))
                );
                assert_eq!(
                    value.pointer("/capabilities/tool_catalog"),
                    Some(&json!(true))
                );
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn peer_mode_ping_advertises_cli_host_type() {
        // An older CLI did not advertise `cancel_tool`/`tool_catalog`; the
        // `host_type: "cli"` field lets the controller resolve those missing
        // capabilities as unsupported instead of optimistically invoking a
        // command the CLI never implemented. See PR #2428 round 5 #1.
        let resp = handle_host_invoke("peer_mode_ping", json!({})).await;
        match resp {
            RemoteResponse::HostInvokeResult {
                ok: true,
                value: Some(value),
                error: None,
            } => {
                assert_eq!(value.get("host_type").and_then(|v| v.as_str()), Some("cli"));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn local_only_commands_are_denied() {
        let resp = handle_host_invoke("account_logout", json!({})).await;
        match resp {
            RemoteResponse::HostInvokeResult {
                ok: false,
                error: Some(err),
                ..
            } => {
                assert!(err.contains("local-only"));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn detached_dispatch_uses_a_distinct_target_command_family() {
        assert_eq!(
            dispatch_target_verb("dispatch_target_submit"),
            Some("submit")
        );
        assert_eq!(dispatch_target_verb("dispatch_submit"), None);
        assert_eq!(dispatch_target_verb("dispatch_target_unknown"), None);
    }

    /// `cancel_tool` and `get_all_tools_info` were previously unimplemented on
    /// the CLI peer host, so a controller rendering a CLI Peer session saw an
    /// ineffective Interrupt button and an empty tool list. They are now
    /// implemented in `commands::dialog::cancel_tool` and
    /// `commands::tools::get_all_tools_info`; this test pins that neither is
    /// refused by the local-only or CLI-unsupported gate before reaching the
    /// implemented handler. A future regression that removes the handler but
    /// leaves the command routable would land in the unsupported fallthrough
    /// branch, not here — that is caught by the capability advertisement +
    /// frontend gate instead.
    #[test]
    fn cancel_tool_and_tool_catalog_are_not_refused_before_dispatch() {
        for command in ["cancel_tool", "get_all_tools_info"] {
            assert!(
                !is_local_only_command(command),
                "{command} must be routable to the peer host"
            );
            assert!(
                !is_cli_unsupported_command(command),
                "{command} must reach its implemented handler, not the unsupported gate"
            );
        }
    }

    #[tokio::test]
    async fn attach_detach_updates_subscribers() {
        let _ = handle_host_invoke(
            "peer_control_attach",
            json!({ "controller_device_id": "ctrl-test-1" }),
        )
        .await;
        assert!(super::super::control::attached_controllers()
            .iter()
            .any(|id| id == "ctrl-test-1"));
        let _ = handle_host_invoke(
            "peer_control_detach",
            json!({ "controller_device_id": "ctrl-test-1" }),
        )
        .await;
        assert!(!super::super::control::attached_controllers()
            .iter()
            .any(|id| id == "ctrl-test-1"));
    }
}
