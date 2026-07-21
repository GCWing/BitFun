//! Peer Device Mode: proxy product Tauri commands onto this host.
//!
//! Commands are executed through the same frontend invoke surface as local UI
//! (peer webview → `invoke`), so handler signatures stay single-sourced.
//! Local-only / controller-only commands are denied before any bridge call.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use super::remote_connect_api::{account_app_handle, current_device_id_for_peer};

const DEFAULT_INVOKE_TIMEOUT: Duration = Duration::from_secs(120);

/// Commands that must never run on a peer on behalf of a controller.
///
/// Keep aligned with:
/// - FE deny list in `src/web-ui/.../adapters/peer-device-adapter.ts`
/// - CLI deny list in `src/apps/cli/src/peer_host/deny.rs`
/// - invariants in `src/web-ui/src/infrastructure/peer-device/README.md`
///
/// Account identity + cloud session/turn APIs stay on the controller. Peer
/// history is restored via HostInvoke (`restore_session_view`), not by
/// forwarding `account_fetch_session_turns` to the peer host.
static LOCAL_ONLY_COMMANDS: &[&str] = &[
    // Window / tray / process chrome
    "show_main_window",
    "hide_main_window_after_close_request",
    "quit_app",
    "minimize_to_tray",
    "initialize_tray_after_startup",
    "startup_window_control",
    "toggle_main_window_fullscreen",
    "restart_app",
    "check_for_updates",
    "install_update",
    // Account identity / peer mode control (stay on controller)
    "account_login",
    "account_finalize_login",
    "account_logout",
    "account_status",
    "account_get_credential_hint",
    "account_token_expired",
    "account_connect_devices",
    "account_online_devices",
    "account_list_devices",
    "account_delete_device",
    "account_device_rpc",
    "account_delegate_to_paired",
    "account_auto_sync",
    "account_sync_settings",
    "account_fetch_settings",
    "account_sync_session",
    "account_fetch_synced_sessions",
    "account_delete_synced_session",
    "account_export_local_session",
    "account_export_all_sessions",
    "account_import_remote_sessions",
    "account_fetch_session_turns",
    "account_send_session_to_device",
    "account_execute_on_device",
    "peer_host_invoke_complete",
    "peer_control_attach",
    "peer_control_detach",
    "peer_mode_ping",
    "peer_controller_set_active",
    // Remote-connect control plane (must not run on peer for a controller)
    "remote_connect_get_device_info",
    "remote_connect_get_lan_ip",
    "remote_connect_get_lan_network_info",
    "remote_connect_get_methods",
    "remote_connect_start",
    "remote_connect_stop",
    "remote_connect_stop_bot",
    "remote_connect_status",
    "remote_connect_get_form_state",
    "remote_connect_set_form_state",
    "remote_connect_configure_custom_server",
    "remote_connect_configure_bot",
    "remote_connect_weixin_qr_start",
    "remote_connect_weixin_qr_poll",
    "remote_connect_get_bot_verbose_mode",
    "remote_connect_set_bot_verbose_mode",
    // This-machine computer-use / OS permission prompts
    "computer_use_request_permissions",
    "computer_use_open_system_settings",
    // One-click relay deploy SSHes from the controller to a user host
    "relay_deploy_preflight",
    "relay_deploy_install_docker",
    "relay_deploy_start",
    "relay_deploy_poll",
    "relay_deploy_cancel",
    "relay_deploy_register",
    "relay_deploy_verify",
];

static PENDING: OnceLock<Mutex<HashMap<String, oneshot::Sender<HostInvokeBridgeResult>>>> =
    OnceLock::new();

#[derive(Default)]
struct PeerControlState {
    controllers: HashSet<String>,
    permission_request_ids: HashSet<String>,
}

/// Controllers currently attached for DeviceEvent fan-out and the pending
/// permission requests projected to them.
static PEER_CONTROL_STATE: OnceLock<Mutex<PeerControlState>> = OnceLock::new();

/// True while this process is acting as a Peer Mode controller (Remote: B).
/// Used to pause cloud settings pull that would rewrite local disk mid-remote.
static PEER_CONTROLLER_ACTIVE: AtomicBool = AtomicBool::new(false);

fn pending_map() -> &'static Mutex<HashMap<String, oneshot::Sender<HostInvokeBridgeResult>>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn peer_control_state() -> &'static Mutex<PeerControlState> {
    PEER_CONTROL_STATE.get_or_init(|| Mutex::new(PeerControlState::default()))
}

pub fn set_peer_controller_active(active: bool) {
    PEER_CONTROLLER_ACTIVE.store(active, Ordering::SeqCst);
}

pub fn is_peer_controller_active() -> bool {
    PEER_CONTROLLER_ACTIVE.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInvokeBridgeResult {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostInvokeBridgeRequest {
    id: String,
    command: String,
    args: Value,
}

pub fn is_local_only_command(command: &str) -> bool {
    LOCAL_ONLY_COMMANDS.iter().any(|denied| *denied == command)
}

/// Register a controller device id to receive peer UI events.
pub fn attach_controller(device_id: String) {
    if let Ok(mut state) = peer_control_state().lock() {
        state.controllers.insert(device_id);
    }
}

fn detach_from_state(state: &mut PeerControlState, device_id: &str) -> Vec<String> {
    let removed = state.controllers.remove(device_id);
    if removed && state.controllers.is_empty() {
        return state.permission_request_ids.drain().collect();
    }
    Vec::new()
}

/// Detach one controller and return requests that must fail closed when it was
/// the final controller.
pub fn detach_controller(device_id: &str) -> Vec<String> {
    let Ok(mut state) = peer_control_state().lock() else {
        return Vec::new();
    };
    detach_from_state(&mut state, device_id)
}

fn retain_online_in_state(
    state: &mut PeerControlState,
    online_device_ids: &HashSet<String>,
) -> Vec<String> {
    let had_controllers = !state.controllers.is_empty();
    state
        .controllers
        .retain(|device_id| online_device_ids.contains(device_id));
    if had_controllers && state.controllers.is_empty() {
        return state.permission_request_ids.drain().collect();
    }
    Vec::new()
}

/// Remove controllers missing from account presence and return requests that
/// lost their final control surface.
pub fn retain_online_controllers(online_device_ids: &HashSet<String>) -> Vec<String> {
    let Ok(mut state) = peer_control_state().lock() else {
        return Vec::new();
    };
    retain_online_in_state(&mut state, online_device_ids)
}

/// Clear all attached controllers after the device-routing stream closes.
pub fn disconnect_controllers() -> Vec<String> {
    let Ok(mut state) = peer_control_state().lock() else {
        return Vec::new();
    };
    state.controllers.clear();
    state.permission_request_ids.drain().collect()
}

pub fn track_permission_event(event: &bitfun_agent_runtime::sdk::PermissionRequestEvent) -> bool {
    let Ok(mut state) = peer_control_state().lock() else {
        return false;
    };
    match event {
        bitfun_agent_runtime::sdk::PermissionRequestEvent::Asked { request } => {
            if !state.controllers.is_empty() {
                state
                    .permission_request_ids
                    .insert(request.request_id.clone());
                true
            } else {
                false
            }
        }
        bitfun_agent_runtime::sdk::PermissionRequestEvent::Replied { request_id, .. }
        | bitfun_agent_runtime::sdk::PermissionRequestEvent::Cancelled { request_id, .. } => {
            let was_tracked = state.permission_request_ids.remove(request_id);
            was_tracked && !state.controllers.is_empty()
        }
    }
}

pub fn take_tracked_permission_requests() -> Vec<String> {
    peer_control_state()
        .lock()
        .map(|mut state| state.permission_request_ids.drain().collect())
        .unwrap_or_default()
}

pub async fn fail_closed_permission_requests(
    request_ids: Vec<String>,
    reason: &str,
) -> Result<(), String> {
    if request_ids.is_empty() {
        return Ok(());
    }
    let manager = bitfun_core::product_runtime::core_permission_request_manager()?;
    let mut failures = Vec::new();
    for request_id in request_ids {
        if let Err(error) = manager
            .cancel_request(&request_id, reason.to_string())
            .await
        {
            failures.push(format!("{request_id}: {error}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Failed to cancel Peer permission requests: {}",
            failures.join("; ")
        ))
    }
}

pub fn attached_controllers() -> Vec<String> {
    peer_control_state()
        .lock()
        .map(|state| state.controllers.iter().cloned().collect())
        .unwrap_or_default()
}

/// Complete a bridged invoke from the peer webview.
#[tauri::command]
pub async fn peer_host_invoke_complete(
    id: String,
    ok: bool,
    value: Option<Value>,
    error: Option<String>,
) -> Result<(), String> {
    let sender = pending_map()
        .lock()
        .map_err(|e| format!("peer host invoke lock poisoned: {e}"))?
        .remove(&id);
    if let Some(tx) = sender {
        let _ = tx.send(HostInvokeBridgeResult { ok, value, error });
        Ok(())
    } else {
        Err(format!("unknown peer host invoke id: {id}"))
    }
}

#[tauri::command]
pub async fn peer_control_attach(controller_device_id: String) -> Result<(), String> {
    if controller_device_id.trim().is_empty() {
        return Err("controller_device_id is required".to_string());
    }
    attach_controller(controller_device_id);
    Ok(())
}

#[tauri::command]
pub async fn peer_control_detach(controller_device_id: String) -> Result<(), String> {
    let request_ids = detach_controller(&controller_device_id);
    fail_closed_permission_requests(request_ids, "Last Peer controller detached").await
}

#[tauri::command]
pub async fn peer_mode_ping() -> Result<Value, String> {
    Ok(serde_json::json!({
        "ok": true,
        "peer": true,
        "device_id": current_device_id_for_peer()
            .unwrap_or_else(|_| "unknown".to_string()),
    }))
}

/// Mark this process as a Peer Mode controller so cloud pull does not rewrite local settings.
#[tauri::command]
pub async fn peer_controller_set_active(active: bool) -> Result<(), String> {
    set_peer_controller_active(active);
    Ok(())
}

/// Dispatch an allowlisted (non-local-only) product command on this peer.
pub async fn dispatch(command: &str, args: Value) -> HostInvokeBridgeResult {
    if command.is_empty() {
        return HostInvokeBridgeResult {
            ok: false,
            value: None,
            error: Some("HostInvoke command is empty".to_string()),
        };
    }
    if is_local_only_command(command) {
        return HostInvokeBridgeResult {
            ok: false,
            value: None,
            error: Some(format!(
                "command '{command}' is local-only and cannot run on peer"
            )),
        };
    }

    let app = match account_app_handle() {
        Some(app) => app.clone(),
        None => {
            return HostInvokeBridgeResult {
                ok: false,
                value: None,
                error: Some("peer app handle not ready".to_string()),
            };
        }
    };

    match bridge_via_webview(&app, command, args).await {
        Ok(result) => result,
        Err(error) => HostInvokeBridgeResult {
            ok: false,
            value: None,
            error: Some(error),
        },
    }
}

async fn bridge_via_webview(
    app: &AppHandle,
    command: &str,
    args: Value,
) -> Result<HostInvokeBridgeResult, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    pending_map()
        .lock()
        .map_err(|e| format!("peer host invoke lock poisoned: {e}"))?
        .insert(id.clone(), tx);

    let request = HostInvokeBridgeRequest {
        id: id.clone(),
        command: command.to_string(),
        args,
    };

    if let Err(e) = app.emit("peer-host-invoke://request", &request) {
        pending_map().lock().ok().map(|mut map| map.remove(&id));
        return Err(format!("failed to emit peer host invoke request: {e}"));
    }

    match tokio::time::timeout(DEFAULT_INVOKE_TIMEOUT, rx).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err("peer host invoke channel closed".to_string()),
        Err(_) => {
            pending_map().lock().ok().map(|mut map| map.remove(&id));
            Err(format!(
                "peer host invoke timed out after {}s for '{command}'",
                DEFAULT_INVOKE_TIMEOUT.as_secs()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control_state(controllers: &[&str], requests: &[&str]) -> PeerControlState {
        PeerControlState {
            controllers: controllers.iter().map(|value| value.to_string()).collect(),
            permission_request_ids: requests.iter().map(|value| value.to_string()).collect(),
        }
    }

    #[test]
    fn only_the_final_detach_drains_peer_permission_requests() {
        let mut state = control_state(&["controller-a", "controller-b"], &["request-1"]);

        assert!(detach_from_state(&mut state, "controller-a").is_empty());
        assert_eq!(
            detach_from_state(&mut state, "controller-b"),
            vec!["request-1".to_string()]
        );
    }

    #[test]
    fn presence_loss_drains_requests_only_when_every_controller_is_offline() {
        let mut state = control_state(&["controller-a", "controller-b"], &["request-1"]);
        let online = HashSet::from(["controller-b".to_string()]);
        assert!(retain_online_in_state(&mut state, &online).is_empty());

        assert_eq!(
            retain_online_in_state(&mut state, &HashSet::new()),
            vec!["request-1".to_string()]
        );
    }
}
