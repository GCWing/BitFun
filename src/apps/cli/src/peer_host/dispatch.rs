//! HostInvoke / DeviceEvent dispatch for CLI Peer Host.

use serde_json::{json, Value};

use bitfun_core::service::remote_connect::remote_server::RemoteResponse;

use super::commands;
use super::control::{
    attach_controller, detach_controller, parse_controller_device_id, peer_mode_ping_value,
};
use super::deny::{is_cli_unsupported_command, is_local_only_command};
use super::state::{peer_host_state, try_peer_host_state};

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
        if detach_controller(&controller_id).await {
            if let Some(state) = try_peer_host_state() {
                if let Err(error) = state
                    .cancel_and_drain_peer_turns("last Peer controller detached")
                    .await
                {
                    return HostInvokeBridgeResult::err(format!(
                        "Peer controller detached, but active work was not fully cancelled: {error}"
                    ));
                }
            }
        }
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
