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
];

static PENDING: OnceLock<Mutex<HashMap<String, oneshot::Sender<HostInvokeBridgeResult>>>> =
    OnceLock::new();

/// Controllers currently attached for DeviceEvent fan-out (device ids).
static CONTROL_SUBSCRIBERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// True while this process is acting as a Peer Mode controller (Remote: B).
/// Used to pause cloud settings pull that would rewrite local disk mid-remote.
static PEER_CONTROLLER_ACTIVE: AtomicBool = AtomicBool::new(false);

fn pending_map() -> &'static Mutex<HashMap<String, oneshot::Sender<HostInvokeBridgeResult>>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn control_subscribers() -> &'static Mutex<HashSet<String>> {
    CONTROL_SUBSCRIBERS.get_or_init(|| Mutex::new(HashSet::new()))
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
    if let Ok(mut set) = control_subscribers().lock() {
        set.insert(device_id);
    }
}

pub fn detach_controller(device_id: &str) {
    if let Ok(mut set) = control_subscribers().lock() {
        set.remove(device_id);
    }
}

pub fn attached_controllers() -> Vec<String> {
    control_subscribers()
        .lock()
        .map(|set| set.iter().cloned().collect())
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
    detach_controller(&controller_device_id);
    Ok(())
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
