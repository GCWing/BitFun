//! SSH task dispatch Tauri adapter.
//!
//! The remote CLI owns the authoritative job and session. These commands are
//! thin host adapters around the platform-neutral dispatch controller and its
//! observer-only outbound index.

use std::sync::Arc;

use bitfun_core::infrastructure::PathManager;
use bitfun_core::service::dispatch::{
    cancel_dispatch, cancel_dispatch_cli_install, get_dispatch_status, list_dispatch_jobs,
    list_dispatch_targets, poll_dispatch_cli_install, probe_dispatch_target,
    start_dispatch_cli_install, submit_dispatch, DispatchConnectionRequest,
    DispatchInstallPollRequest, DispatchInstallStartRequest, DispatchJobRequest,
    DispatchListJobsRequest, DispatchListTargetsRequest, DispatchProbeTargetRequest,
    DispatchStatusRequest, DispatchSubmitRequest, DispatchTargetOption, OutboundDispatchStore,
};
use bitfun_core::service::remote_ssh::dispatch_ssh::{
    DispatchInstallPoll, DispatchInstallStart, DispatchSshProbe,
};
use serde_json::Value;
use tauri::State;

use super::app_state::AppState;

#[tauri::command]
pub async fn dispatch_list_targets(
    state: State<'_, AppState>,
    request: DispatchListTargetsRequest,
) -> Result<Vec<DispatchTargetOption>, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    list_dispatch_targets(&manager, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_probe_target(
    state: State<'_, AppState>,
    request: DispatchProbeTargetRequest,
) -> Result<DispatchSshProbe, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    probe_dispatch_target(&manager, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_install_cli_start(
    state: State<'_, AppState>,
    request: DispatchInstallStartRequest,
) -> Result<DispatchInstallStart, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    start_dispatch_cli_install(&manager, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_install_cli_poll(
    state: State<'_, AppState>,
    request: DispatchInstallPollRequest,
) -> Result<DispatchInstallPoll, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    poll_dispatch_cli_install(&manager, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_install_cli_cancel(
    state: State<'_, AppState>,
    request: DispatchConnectionRequest,
) -> Result<(), String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    cancel_dispatch_cli_install(&manager, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_submit(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchSubmitRequest,
) -> Result<Value, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    submit_dispatch(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_status(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchStatusRequest,
) -> Result<Value, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    get_dispatch_status(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_cancel(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchJobRequest,
) -> Result<Value, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    cancel_dispatch(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_list_jobs(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchListJobsRequest,
) -> Result<Value, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    list_dispatch_jobs(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}
