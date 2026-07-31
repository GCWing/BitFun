//! SSH task dispatch Tauri adapter.
//!
//! The remote CLI owns the authoritative job and session. These commands are
//! thin host adapters around the platform-neutral dispatch controller and its
//! observer-only outbound index.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use bitfun_core::infrastructure::PathManager;
use bitfun_core::service::dispatch::{
    answer_device_dispatch, answer_dispatch, append_device_dispatch, append_dispatch,
    apply_dispatch_result, cancel_device_dispatch, cancel_dispatch, cancel_dispatch_cli_install,
    get_device_dispatch_status, get_dispatch_status, list_device_dispatch_jobs, list_dispatch_jobs,
    list_dispatch_targets, poll_dispatch_cli_install, probe_device_dispatch_target,
    probe_dispatch_target, pull_device_dispatch_result, pull_dispatch_result,
    start_dispatch_cli_install, start_dispatch_cli_source_build, submit_device_dispatch,
    submit_dispatch, sync_dispatch_model_config, DeviceDispatchRpc, DispatchAnswerRequest,
    DispatchAppendRequest, DispatchApplyResultRequest, DispatchConnectionRequest,
    DispatchInstallPollRequest, DispatchInstallStartRequest, DispatchJobRequest,
    DispatchListJobsRequest, DispatchListTargetsRequest, DispatchProbeTargetRequest,
    DispatchSaveTranscriptRequest, DispatchStatusRequest, DispatchSubmitRequest, DispatchTarget,
    DispatchTargetOption, DispatchTargetRequest, DispatchTranscriptRequest, OutboundDispatchStore,
    WorkspaceResultApplyOutcome,
};
use bitfun_core::service::remote_ssh::dispatch_ssh::{
    DispatchInstallPoll, DispatchInstallStart, DispatchSshProbe,
};
use bitfun_services_integrations::remote_ssh::dispatch_ssh::install_cli_source_archive_start;
use serde_json::Value;
use tauri::State;

use super::app_state::AppState;

struct AccountDeviceDispatchRpc;

const MAX_CONTROLLER_SOURCE_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;

#[cfg(debug_assertions)]
fn controller_source_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)?
        .to_path_buf();
    (root.join("Cargo.toml").is_file() && root.join(".git").exists()).then_some(root)
}

#[cfg(not(debug_assertions))]
fn controller_source_root() -> Option<PathBuf> {
    None
}

async fn archive_controller_source(root: PathBuf) -> anyhow::Result<(Vec<u8>, String)> {
    tokio::task::spawn_blocking(move || {
        let status = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&root)
            .output()
            .map_err(|error| anyhow::anyhow!("inspect controller source checkout: {error}"))?;
        if !status.status.success() {
            anyhow::bail!(
                "inspect controller source checkout: {}",
                String::from_utf8_lossy(&status.stderr).trim()
            );
        }
        if !status.stdout.is_empty() {
            anyhow::bail!(
                "the controller source checkout has uncommitted changes; commit them and restart Desktop before updating the target CLI"
            );
        }

        let revision = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&root)
            .output()
            .map_err(|error| anyhow::anyhow!("resolve controller source revision: {error}"))?;
        if !revision.status.success() {
            anyhow::bail!(
                "resolve controller source revision: {}",
                String::from_utf8_lossy(&revision.stderr).trim()
            );
        }
        let revision = String::from_utf8(revision.stdout)
            .map_err(|error| anyhow::anyhow!("controller source revision is not UTF-8: {error}"))?
            .trim()
            .to_string();

        let archive = std::process::Command::new("git")
            .args(["archive", "--format=tar.gz", "HEAD"])
            .current_dir(&root)
            .output()
            .map_err(|error| anyhow::anyhow!("archive controller source: {error}"))?;
        if !archive.status.success() {
            anyhow::bail!(
                "archive controller source: {}",
                String::from_utf8_lossy(&archive.stderr).trim()
            );
        }
        if archive.stdout.is_empty() || archive.stdout.len() > MAX_CONTROLLER_SOURCE_ARCHIVE_BYTES {
            anyhow::bail!("controller source archive is empty or exceeds the 512 MB limit");
        }
        Ok((archive.stdout, revision))
    })
    .await
    .map_err(|error| anyhow::anyhow!("controller source archive task failed: {error}"))?
}

#[async_trait]
impl DeviceDispatchRpc for AccountDeviceDispatchRpc {
    async fn invoke(&self, device_id: &str, command: &str, args: Value) -> anyhow::Result<Value> {
        let command_json = serde_json::to_string(&serde_json::json!({
            "cmd": "host_invoke",
            "command": command,
            "args": args,
        }))?;
        let raw = super::remote_connect_api::account_device_rpc(
            device_id.to_string(),
            command_json,
            None,
        )
        .await
        .map_err(anyhow::Error::msg)?;
        decode_device_dispatch_rpc(&raw)
    }
}

fn decode_device_dispatch_rpc(raw: &str) -> anyhow::Result<Value> {
    let envelope: Value =
        serde_json::from_str(raw).map_err(|error| anyhow::anyhow!("decode device RPC: {error}"))?;
    match envelope.get("resp").and_then(Value::as_str) {
        Some("host_invoke_result") if envelope.get("ok").and_then(Value::as_bool) == Some(true) => {
            Ok(envelope.get("value").cloned().unwrap_or(Value::Null))
        }
        Some("host_invoke_result") => Err(anyhow::anyhow!(
            "{}",
            envelope
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Device dispatch target rejected the command")
        )),
        Some("error") => Err(anyhow::anyhow!(
            "{}",
            envelope
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Device dispatch RPC failed")
        )),
        _ => Err(anyhow::anyhow!(
            "Device dispatch target returned an unexpected RPC envelope"
        )),
    }
}

#[tauri::command]
pub async fn dispatch_list_targets(
    state: State<'_, AppState>,
    request: DispatchListTargetsRequest,
) -> Result<Vec<DispatchTargetOption>, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    let mut targets = list_dispatch_targets(&manager, request)
        .await
        .map_err(|error| error.to_string())?;
    let current_device_id = super::remote_connect_api::remote_connect_get_device_info()
        .await
        .ok()
        .map(|device| device.device_id);
    if let Ok(devices) = super::remote_connect_api::account_list_devices().await {
        targets.extend(devices.into_iter().filter_map(|device| {
            if current_device_id.as_deref() == Some(device.device_id.as_str()) {
                return None;
            }
            Some(DispatchTargetOption {
                kind: "device".to_string(),
                connection_id: None,
                device_id: Some(device.device_id),
                display_name: device.device_name,
                description: None,
                default_workspace: None,
                online: Some(device.online),
            })
        }));
    }
    Ok(targets)
}

#[tauri::command]
pub async fn dispatch_probe_target(
    state: State<'_, AppState>,
    request: DispatchProbeTargetRequest,
) -> Result<DispatchSshProbe, String> {
    if matches!(&request.target, DispatchTargetRequest::Device { .. }) {
        return probe_device_dispatch_target(&AccountDeviceDispatchRpc, request)
            .await
            .map_err(|error| error.to_string());
    }
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    let mut probe = probe_dispatch_target(&manager, request)
        .await
        .map_err(|error| error.to_string())?;
    if controller_source_root().is_some() {
        if let Some(source_build) = probe.source_build.as_mut() {
            source_build.git_ref = "current-controller-checkout".to_string();
        }
    }
    Ok(probe)
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

/// Build the CLI from source on the target. Offered when no published binary
/// can run there.
#[tauri::command]
pub async fn dispatch_install_cli_source_start(
    state: State<'_, AppState>,
    request: DispatchConnectionRequest,
) -> Result<DispatchInstallStart, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    if let Some(root) = controller_source_root() {
        let (archive, revision) = archive_controller_source(root)
            .await
            .map_err(|error| error.to_string())?;
        return install_cli_source_archive_start(
            &manager,
            request.connection_id.trim(),
            &archive,
            &revision,
        )
        .await
        .map_err(|error| error.to_string());
    }
    start_dispatch_cli_source_build(&manager, request)
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
pub async fn dispatch_sync_model_config(
    state: State<'_, AppState>,
    request: DispatchConnectionRequest,
) -> Result<(), String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    sync_dispatch_model_config(&manager, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_submit(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchSubmitRequest,
) -> Result<Value, String> {
    if let DispatchTargetRequest::Device { device_id, .. } = &request.target {
        let display_name = super::remote_connect_api::account_list_devices()
            .await
            .map_err(|error| format!("list account devices before dispatch: {error}"))?
            .into_iter()
            .find(|device| device.device_id == *device_id)
            .ok_or_else(|| "Dispatch device is not registered on the current account".to_string())
            .and_then(|device| {
                device.online.then_some(device.device_name).ok_or_else(|| {
                    "Dispatch device is offline; no local fallback was attempted".to_string()
                })
            })?;
        let store = OutboundDispatchStore::new(path_manager.as_ref());
        return submit_device_dispatch(&AccountDeviceDispatchRpc, &store, display_name, request)
            .await
            .map_err(|error| error.to_string());
    }
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
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    if matches!(
        store
            .get(&request.job_id)
            .await
            .map_err(|error| error.to_string())?
            .map(|record| record.target),
        Some(DispatchTarget::Device { .. })
    ) {
        return get_device_dispatch_status(&AccountDeviceDispatchRpc, &store, request)
            .await
            .map_err(|error| error.to_string());
    }
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    get_dispatch_status(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

/// Download what a finished snapshot job changed on its target.
///
/// Fetch and report only — the caller shows the diff and the user decides
/// whether any of it reaches their workspace.
#[tauri::command]
pub async fn dispatch_pull_result(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchJobRequest,
) -> Result<Value, String> {
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    // Both transports stage the bundle and its summary identically, so the
    // apply step below is transport-blind.
    if matches!(
        store
            .get(&request.job_id)
            .await
            .map_err(|error| error.to_string())?
            .map(|record| record.target),
        Some(DispatchTarget::Device { .. })
    ) {
        return pull_device_dispatch_result(&AccountDeviceDispatchRpc, &store, request)
            .await
            .map_err(|error| error.to_string());
    }
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    pull_dispatch_result(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

/// Apply a pulled result bundle to a local workspace.
///
/// Aborts without writing when a path changed on both sides, unless the user
/// explicitly chose to take the target's version.
#[tauri::command]
pub async fn dispatch_apply_result(
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchApplyResultRequest,
) -> Result<WorkspaceResultApplyOutcome, String> {
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    apply_dispatch_result(&store, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_cancel(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchJobRequest,
) -> Result<Value, String> {
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    if matches!(
        store
            .get(&request.job_id)
            .await
            .map_err(|error| error.to_string())?
            .map(|record| record.target),
        Some(DispatchTarget::Device { .. })
    ) {
        return cancel_device_dispatch(&AccountDeviceDispatchRpc, &store, request)
            .await
            .map_err(|error| error.to_string());
    }
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
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
    if let Some(DispatchTargetRequest::Device { device_id, .. }) = &request.target {
        let display_name = super::remote_connect_api::account_list_devices()
            .await
            .map_err(|error| format!("list account devices before observing dispatch: {error}"))?
            .into_iter()
            .find(|device| device.device_id == *device_id)
            .ok_or_else(|| "Dispatch device is not registered on the current account".to_string())
            .and_then(|device| {
                device.online.then_some(device.device_name).ok_or_else(|| {
                    "Dispatch device is offline; its jobs cannot be listed".to_string()
                })
            })?;
        let store = OutboundDispatchStore::new(path_manager.as_ref());
        return list_device_dispatch_jobs(&AccountDeviceDispatchRpc, &store, display_name, request)
            .await
            .map_err(|error| error.to_string());
    }
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    list_dispatch_jobs(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_answer(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchAnswerRequest,
) -> Result<Value, String> {
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    if matches!(
        store
            .get(&request.job_id)
            .await
            .map_err(|error| error.to_string())?
            .map(|record| record.target),
        Some(DispatchTarget::Device { .. })
    ) {
        return answer_device_dispatch(&AccountDeviceDispatchRpc, &store, request)
            .await
            .map_err(|error| error.to_string());
    }
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    answer_dispatch(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn dispatch_append(
    state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchAppendRequest,
) -> Result<Value, String> {
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    if matches!(
        store
            .get(&request.job_id)
            .await
            .map_err(|error| error.to_string())?
            .map(|record| record.target),
        Some(DispatchTarget::Device { .. })
    ) {
        return append_device_dispatch(&AccountDeviceDispatchRpc, &store, request)
            .await
            .map_err(|error| error.to_string());
    }
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|error| error.to_string())?;
    append_dispatch(&manager, &store, request)
        .await
        .map_err(|error| error.to_string())
}

/// Read this controller's cached observer transcript for one dispatch job.
///
/// Purely local: it touches neither the target nor any local session runtime.
/// A missing or unreadable cache returns `null` and the observer replays the
/// job from the beginning.
#[tauri::command]
pub async fn dispatch_load_transcript(
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchTranscriptRequest,
) -> Result<Option<Value>, String> {
    OutboundDispatchStore::new(path_manager.as_ref())
        .read_transcript(&request.job_id)
        .await
        .map_err(|error| error.to_string())
}

/// Persist this controller's observer transcript for one dispatch job.
///
/// A `null` transcript erases the cache instead, which is how deleting a
/// projection drops its cached content right away.
///
/// Returns `false` when the transcript exceeds the cache ceiling, in which case
/// the previous entry is kept and the renderer keeps polling as before.
#[tauri::command]
pub async fn dispatch_save_transcript(
    path_manager: State<'_, Arc<PathManager>>,
    request: DispatchSaveTranscriptRequest,
) -> Result<bool, String> {
    let store = OutboundDispatchStore::new(path_manager.as_ref());
    let Some(transcript) = request.transcript else {
        return store
            .remove_transcript(&request.job_id)
            .await
            .map(|()| true)
            .map_err(|error| error.to_string());
    };
    store
        .write_transcript(&request.job_id, &transcript)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::decode_device_dispatch_rpc;

    #[test]
    fn device_dispatch_requires_a_correlated_host_invoke_acknowledgement() {
        let value = decode_device_dispatch_rpc(
            r#"{"resp":"host_invoke_result","ok":true,"value":{"accepted":true,"jobId":"job-1","sessionId":"session-1"}}"#,
        )
        .expect("true acknowledgement");
        assert_eq!(value["jobId"], "job-1");
        assert_eq!(value["sessionId"], "session-1");

        let rejected = decode_device_dispatch_rpc(
            r#"{"resp":"host_invoke_result","ok":false,"error":"target rejected"}"#,
        )
        .expect_err("negative acknowledgement");
        assert!(rejected.to_string().contains("target rejected"));
        assert!(decode_device_dispatch_rpc(r#"{"sent":true}"#).is_err());
    }
}
