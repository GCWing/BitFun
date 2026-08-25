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
    "set_main_window_transient_geometry",
    "get_prevent_sleep_enabled",
    "set_prevent_sleep_enabled",
    "restart_app",
    "check_for_updates",
    "install_update",
    "appearance_market_browse",
    "appearance_market_download_release",
    "appearance_market_get_listing",
    "appearance_market_get_review_submission",
    "appearance_market_list_review_submissions",
    "appearance_market_list_submissions",
    "appearance_market_review_submission",
    "appearance_market_submit_package",
    "appearance_market_withdraw_submission",
    // Account identity / peer mode control (stay on controller)
    "account_login",
    "account_finalize_login",
    "account_cancel_pending_login",
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
    // Computer-use OS permission prompts + system-settings are intentionally NOT
    // local-only: under Desktop Peer Mode they must run on the peer host B (B
    // surfaces B's own OS permission prompts / settings), reached via
    // bridge_via_webview. CLI Peer refuses them in deny.rs. See SessionConfig.
    // Detached dispatch uses controller-owned SSH credentials and observers.
    "dispatch_list_targets",
    "dispatch_probe_target",
    "dispatch_install_cli_start",
    "dispatch_install_cli_poll",
    "dispatch_install_cli_cancel",
    "dispatch_provision_target",
    "dispatch_sync_model_config",
    "dispatch_submit",
    "dispatch_status",
    "dispatch_query",
    "dispatch_cancel",
    "dispatch_sync_result",
    "dispatch_list_jobs",
    "dispatch_answer",
    "dispatch_append",
    "dispatch_continue",
    "dispatch_load_transcript",
    "dispatch_save_transcript",
    // One-click relay deploy SSHes from the controller to a user host
    "relay_deploy_preflight",
    "relay_deploy_install_docker",
    "relay_deploy_start",
    "relay_deploy_poll",
    "relay_deploy_cancel",
    "relay_deploy_register",
    "relay_deploy_verify",
    // Speech capture and model files belong to the machine the user speaks at.
    "speech_list_models",
    "speech_download_model",
    "speech_cancel_model_download",
    "speech_delete_model",
    "speech_verify_model",
    "speech_start_input_session",
    "speech_append_audio_chunk",
    "speech_finish_input_session",
    "speech_cancel_input_session",
    // Granting Git ownership trust writes to the peer user's global Git
    // configuration and tells Git to run hooks from a tree they do not own.
    // That decision stays with the person at that machine; a controller can
    // still read `git_get_repository_trust` and relay the manual command.
    "git_trust_repository",
    // Controller app-shell state mirrored from the FE deny list. An older or
    // non-Web-UI controller can still HostInvoke these onto this peer, so the
    // peer host must refuse them independently of the FE optimization. Keep in
    // sync with `src/web-ui/.../adapters/peer-device-adapter.ts`
    // LOCAL_ONLY_COMMANDS and `src/apps/cli/src/peer_host/deny.rs`.
    // UI locale writes the controller's config and rebuilds THIS machine's
    // macOS menubar/tray; routing it to a peer writes the wrong config.
    "i18n_get_current_language",
    "i18n_set_language",
    "i18n_get_supported_languages",
    "i18n_get_config",
    "i18n_set_config",
    // Announcement scheduler/state: get_pending / get_tips run the scheduler
    // (mutate app_open_count + persist); seen / dismiss / never-show write
    // controller announcement state. Refused on the peer.
    "get_pending_announcements",
    "get_announcement_tips",
    "mark_announcement_seen",
    "dismiss_announcement",
    "never_show_announcement",
    "trigger_announcement",
    // Companion pets live on the controller's desktop; the import zip path is
    // picked by a local dialog on the controller and is not readable here.
    "list_agent_companion_pets",
    "import_agent_companion_pet_package",
    "delete_agent_companion_pet_package",
    // Insights is the controller's own usage report: it reads the controller's
    // session history and writes the HTML to the controller's user_data_dir.
    "generate_insights",
    "get_latest_insights",
    "load_insights_report",
    "has_insights_data",
    "cancel_insights_generation",
    // IDE control events drive the controller window's panels; the result
    // report must settle on the controller's transport, not here.
    "report_ide_control_result",
    // Controller app-shell / local-device commands (embedded webview/DevTools/
    // desktop-pet/diagnostics) operate on the controller's OWN surfaces and a
    // peer host has no implementation for them, so they stay local-only.
    //
    // NOTE: the runtime-owning Browser Control and Computer Use commands are
    // NOT local-only — they run the agent Tool, so under Desktop Peer Mode they
    // route to the peer host B via bridge_via_webview (reads B's own browser
    // and OS). CLI Peer refuses them in deny.rs and the UI gates the section on
    // host type. See SessionConfig + cli deny.rs.
    "browser_webview_create",
    "browser_webview_eval",
    "browser_webview_navigate",
    "browser_webview_reload",
    "browser_webview_set_bounds",
    "debug_devtools_available",
    "debug_open_devtools",
    "resize_agent_companion_desktop_pet",
    "show_agent_companion_desktop_pet",
    "hide_agent_companion_desktop_pet",
    "append_flow_chat_diagnostics",
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
    LOCAL_ONLY_COMMANDS.contains(&command)
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
        // Declares which kind of host answered so the controller can resolve
        // capabilities that an older host did not advertise. An older Desktop
        // (pre-`50b76516`) omits `cancel_tool`/`tool_catalog` but still reports
        // `host_type: "desktop"` — and Desktop has always implemented both — so
        // the controller keeps the Interrupt button / tool list. An older CLI
        // reports `host_type: "cli"` and never implemented them, so the
        // controller gates them off instead of showing an action that silently
        // fails. See PR #2428 round 5 #1.
        "host_type": "desktop",
        "capabilities": {
            "idempotent_dialog_submit": true,
            "targeted_session_rollback": true,
            "token_usage_statistics": true,
            // Desktop implements both per-tool cancel and the tool catalog
            // (agentic_api::cancel_tool, tool_api::get_all_tools_info), so the
            // controller can gate the Terminal Interrupt button and the tool
            // catalog UI on these the same way it does on the CLI peer host.
            "cancel_tool": true,
            "tool_catalog": true,
        },
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

    #[tokio::test]
    async fn peer_ping_advertises_mutation_capabilities() {
        let value = peer_mode_ping().await.expect("peer ping");
        assert_eq!(
            value.get("host_type").and_then(Value::as_str),
            Some("desktop")
        );
        assert_eq!(
            value
                .pointer("/capabilities/idempotent_dialog_submit")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            value
                .pointer("/capabilities/targeted_session_rollback")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            value
                .pointer("/capabilities/token_usage_statistics")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            value
                .pointer("/capabilities/cancel_tool")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            value
                .pointer("/capabilities/tool_catalog")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn appearance_market_stays_on_the_controller_device() {
        for command in [
            "appearance_market_browse",
            "appearance_market_get_listing",
            "appearance_market_download_release",
            "appearance_market_get_review_submission",
            "appearance_market_list_review_submissions",
            "appearance_market_list_submissions",
            "appearance_market_review_submission",
            "appearance_market_submit_package",
            "appearance_market_withdraw_submission",
        ] {
            assert!(is_local_only_command(command), "{command}");
        }
    }

    /// The controller-side FE deny list is an optimization, not the boundary.
    /// A controller on an older build (or a non-FE controller) still reaches
    /// this host, so every controller-owned command must be refused here too.
    #[test]
    fn controller_owned_capture_and_dispatch_commands_are_refused_on_the_peer() {
        for command in [
            // Capture and model files belong to the machine the user speaks at.
            "speech_list_models",
            "speech_download_model",
            "speech_cancel_model_download",
            "speech_delete_model",
            "speech_verify_model",
            "speech_start_input_session",
            "speech_append_audio_chunk",
            "speech_finish_input_session",
            "speech_cancel_input_session",
            // Same controller-owned observer/credential family as the other
            // dispatch verbs already denied here.
            "dispatch_continue",
        ] {
            assert!(is_local_only_command(command), "{command}");
        }
    }

    /// Reading why Git refuses a repository is safe to answer for a
    /// controller; granting the exception writes this user's global Git
    /// configuration and must be decided at this machine.
    #[test]
    fn granting_git_ownership_trust_is_refused_on_the_peer() {
        assert!(is_local_only_command("git_trust_repository"));
        assert!(!is_local_only_command("git_get_repository_trust"));
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
