use std::path::Path;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use base64::Engine as _;
use bitfun_services_integrations::remote_ssh::dispatch_ssh::{self, DispatchSshProbe};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::controller::{
    same_target_identity, validate_answer_request, validate_append_request,
    validate_submission_preflight, validate_submit_ack, validate_submit_request,
    DispatchAnswerRequest, DispatchAppendRequest, DispatchJobRequest, DispatchListJobsRequest,
    DispatchProbeTargetRequest, DispatchStatusRequest, DispatchSubmitRequest,
    DISPATCH_PROTOCOL_VERSION,
};
use super::{
    adopt_target_jobs, DispatchTarget, DispatchTargetRequest, DispatchWorkspaceDeliveryRequest,
    OutboundDispatchRecord, OutboundDispatchStore,
};

const DEVICE_WORKSPACE_CHUNK_BYTES: usize = 256 * 1024;
const DEVICE_WORKSPACE_COMMIT_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(750);
const DEVICE_WORKSPACE_COMMIT_WAIT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Account-device routing is a platform adapter. The product controller owns
/// dispatch semantics while Desktop supplies the encrypted Relay RPC.
#[async_trait]
pub trait DeviceDispatchRpc: Send + Sync {
    async fn invoke(&self, device_id: &str, command: &str, args: Value) -> anyhow::Result<Value>;
}

pub async fn probe_device(
    rpc: &dyn DeviceDispatchRpc,
    request: DispatchProbeTargetRequest,
) -> anyhow::Result<DispatchSshProbe> {
    let DispatchTargetRequest::Device {
        device_id,
        workspace_path,
    } = request.target
    else {
        anyhow::bail!("Device dispatch probe requires a device target");
    };
    if device_id.trim().is_empty() {
        anyhow::bail!("Device dispatch requires a deviceId");
    }
    let args = if workspace_path.trim().is_empty() {
        json!({})
    } else {
        json!({ "workspacePath": workspace_path })
    };
    let protocol = rpc
        .invoke(&device_id, "dispatch_target_probe", args)
        .await?;
    let protocol_error = dispatch_ssh::validate_dispatch_protocol(&protocol, None)
        .err()
        .map(|error| error.to_string());
    Ok(DispatchSshProbe {
        cli_installed: true,
        cli_path: None,
        os: protocol
            .get("os")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        arch: protocol
            .get("arch")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        install_supported: false,
        install_error: None,
        protocol_error,
        release: None,
        protocol: Some(protocol),
        // An account device runs its own already-installed CLI; this controller
        // neither installs nor builds anything for it.
        prebuilt_incompatible: None,
        source_build: None,
    })
}

pub async fn submit_device(
    rpc: &dyn DeviceDispatchRpc,
    store: &OutboundDispatchStore,
    display_name: String,
    request: DispatchSubmitRequest,
) -> anyhow::Result<Value> {
    validate_submit_request(&request)?;
    let DispatchTargetRequest::Device {
        device_id,
        workspace_path: requested_workspace_path,
    } = &request.target
    else {
        anyhow::bail!("Device dispatch submission requires a device target");
    };
    if device_id.trim().is_empty() {
        anyhow::bail!("Device dispatch requires a deviceId");
    }

    let workspace_path = resolve_device_workspace(
        rpc,
        store,
        device_id,
        requested_workspace_path,
        &request.workspace_delivery,
        &request.job_id,
    )
    .await?;
    let protocol = rpc
        .invoke(
            device_id,
            "dispatch_target_probe",
            json!({ "workspacePath": workspace_path }),
        )
        .await
        .context("probe device immediately before dispatch submission")?;
    dispatch_ssh::validate_dispatch_protocol(&protocol, Some(&request.approval_policy))?;
    validate_submission_preflight(&protocol, request.model.as_deref())?;
    let workspace_path = protocol
        .pointer("/workspace/path")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| anyhow!("Device dispatch target returned no canonical workspace path"))?
        .to_string();

    let resolved_target = DispatchTarget::Device {
        device_id: device_id.clone(),
        workspace_path: workspace_path.clone(),
        display_name,
    };
    let requested_record = OutboundDispatchRecord::new(
        request.job_id.clone(),
        resolved_target,
        request.session_id.clone(),
        workspace_path.clone(),
        &request.prompt,
        "submitting",
    )?
    .with_submission_metadata(
        request.title.clone(),
        request.agent_type.clone(),
        request.approval_policy.clone(),
        request.model.clone(),
    );
    let bound_record = store.bind_if_absent(&requested_record).await?;
    if bound_record.session_id != request.session_id
        || !same_target_identity(&bound_record.target, &requested_record.target)
    {
        anyhow::bail!("Dispatch jobId is already bound to another target or session");
    }

    let mut payload = json!({
        "protocolVersion": DISPATCH_PROTOCOL_VERSION,
        "jobId": request.job_id,
        "sessionId": request.session_id,
        "workspacePath": workspace_path,
        "agentType": request.agent_type,
        "prompt": request.prompt,
        "approvalPolicy": request.approval_policy,
    });
    if let Some(model) = request.model.filter(|value| !value.trim().is_empty()) {
        payload["model"] = Value::String(model);
    }
    if let Some(title) = request.title.filter(|value| !value.trim().is_empty()) {
        payload["title"] = Value::String(title);
    }

    let response = match rpc
        .invoke(device_id, "dispatch_target_submit", payload)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let _ = store
                .update_progress(&requested_record.job_id, 0, "submission_unknown")
                .await;
            return Err(error);
        }
    };
    if let Err(error) = validate_submit_ack(
        &response,
        &requested_record.job_id,
        &requested_record.session_id,
    ) {
        let _ = store
            .update_progress(&requested_record.job_id, 0, "submission_unknown")
            .await;
        return Err(error);
    }
    let state = response
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("queued")
        .to_string();
    store
        .update_progress(&requested_record.job_id, 0, state)
        .await?;
    Ok(response)
}

pub async fn status_device(
    rpc: &dyn DeviceDispatchRpc,
    store: &OutboundDispatchStore,
    request: DispatchStatusRequest,
) -> anyhow::Result<Value> {
    let record = load_device_record(store, &request.job_id).await?;
    let DispatchTarget::Device { device_id, .. } = &record.target else {
        unreachable!("load_device_record validates target kind")
    };
    let response = rpc
        .invoke(
            device_id,
            "dispatch_target_status",
            json!({ "jobId": request.job_id, "cursor": request.cursor }),
        )
        .await?;
    let state = response
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or(record.last_state.as_str())
        .to_string();
    store
        .update_progress(&record.job_id, request.cursor, state)
        .await?;
    let _ = store.remove_workspace_snapshot(&record.job_id).await;
    Ok(response)
}

pub async fn cancel_device(
    rpc: &dyn DeviceDispatchRpc,
    store: &OutboundDispatchStore,
    request: DispatchJobRequest,
) -> anyhow::Result<Value> {
    let record = load_device_record(store, &request.job_id).await?;
    let DispatchTarget::Device { device_id, .. } = &record.target else {
        unreachable!("load_device_record validates target kind")
    };
    let response = rpc
        .invoke(
            device_id,
            "dispatch_target_cancel",
            json!({ "jobId": request.job_id }),
        )
        .await?;
    if response
        .get("cancelled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        store
            .update_progress(&record.job_id, record.last_cursor, "cancelled")
            .await?;
    }
    Ok(response)
}

pub async fn answer_device(
    rpc: &dyn DeviceDispatchRpc,
    store: &OutboundDispatchStore,
    request: DispatchAnswerRequest,
) -> anyhow::Result<Value> {
    validate_answer_request(&request)?;
    let record = load_device_record(store, &request.job_id).await?;
    let DispatchTarget::Device { device_id, .. } = &record.target else {
        unreachable!("load_device_record validates target kind")
    };
    rpc.invoke(
        device_id,
        "dispatch_target_answer",
        serde_json::to_value(request)?,
    )
    .await
}

pub async fn append_device(
    rpc: &dyn DeviceDispatchRpc,
    store: &OutboundDispatchStore,
    request: DispatchAppendRequest,
) -> anyhow::Result<Value> {
    validate_append_request(&request)?;
    let record = load_device_record(store, &request.job_id).await?;
    let DispatchTarget::Device { device_id, .. } = &record.target else {
        unreachable!("load_device_record validates target kind")
    };
    rpc.invoke(
        device_id,
        "dispatch_target_append",
        serde_json::to_value(request)?,
    )
    .await
}

pub async fn list_device_jobs(
    rpc: &dyn DeviceDispatchRpc,
    store: &OutboundDispatchStore,
    display_name: String,
    request: DispatchListJobsRequest,
) -> anyhow::Result<Value> {
    let Some(DispatchTargetRequest::Device { device_id, .. }) = request.target else {
        anyhow::bail!("Device dispatch listing requires a device target");
    };
    let response = rpc
        .invoke(&device_id, "dispatch_target_list", json!({}))
        .await?;
    adopt_target_jobs(
        store,
        &DispatchTarget::Device {
            device_id,
            workspace_path: String::new(),
            display_name,
        },
        &response,
    )
    .await?;
    Ok(response)
}

async fn resolve_device_workspace(
    rpc: &dyn DeviceDispatchRpc,
    store: &OutboundDispatchStore,
    device_id: &str,
    requested_workspace_path: &str,
    delivery: &DispatchWorkspaceDeliveryRequest,
    job_id: &str,
) -> anyhow::Result<String> {
    match delivery {
        DispatchWorkspaceDeliveryRequest::Existing => {
            let path = requested_workspace_path.trim();
            if path.is_empty() {
                anyhow::bail!("existing device dispatch requires a workspacePath");
            }
            Ok(path.to_string())
        }
        DispatchWorkspaceDeliveryRequest::SnapshotExact {
            source_workspace_path,
            sensitive_files_confirmed,
        } => {
            if !sensitive_files_confirmed {
                anyhow::bail!(
                    "exact workspace snapshot requires confirmation that ignored and sensitive files may be transferred"
                );
            }
            let prepared = store
                .prepare_workspace_snapshot(job_id, source_workspace_path)
                .await?;
            upload_device_workspace(
                rpc,
                device_id,
                job_id,
                &prepared.archive_path,
                &prepared.metadata,
            )
            .await
        }
    }
}

async fn upload_device_workspace(
    rpc: &dyn DeviceDispatchRpc,
    device_id: &str,
    job_id: &str,
    archive_path: &Path,
    metadata: &bitfun_services_core::dispatch_workspace::WorkspaceSnapshotMetadata,
) -> anyhow::Result<String> {
    let begin = rpc
        .invoke(
            device_id,
            "dispatch_target_workspace_begin",
            json!({
                "protocolVersion": DISPATCH_PROTOCOL_VERSION,
                "jobId": job_id,
                "metadata": metadata,
            }),
        )
        .await?;
    if begin
        .get("committed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return required_workspace_path(&begin);
    }
    if begin.get("accepted").and_then(Value::as_bool) != Some(true) {
        anyhow::bail!("Device dispatch target did not accept the workspace snapshot");
    }
    let mut offset = begin
        .get("offset")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("Device dispatch target returned no workspace upload offset"))?;
    if offset > metadata.archive_size {
        anyhow::bail!("Device dispatch target returned an invalid workspace upload offset");
    }

    let mut archive = tokio::fs::File::open(archive_path)
        .await
        .with_context(|| format!("open workspace snapshot {}", archive_path.display()))?;
    archive.seek(std::io::SeekFrom::Start(offset)).await?;
    let mut buffer = vec![0_u8; DEVICE_WORKSPACE_CHUNK_BYTES];
    while offset < metadata.archive_size {
        let remaining = (metadata.archive_size - offset) as usize;
        let read_limit = remaining.min(buffer.len());
        let read = archive.read(&mut buffer[..read_limit]).await?;
        if read == 0 {
            anyhow::bail!(
                "Workspace snapshot ended at {offset} of {} bytes",
                metadata.archive_size
            );
        }
        let next_offset = offset + read as u64;
        let response = rpc
            .invoke(
                device_id,
                "dispatch_target_workspace_chunk",
                json!({
                    "jobId": job_id,
                    "offset": offset,
                    "dataBase64": base64::engine::general_purpose::STANDARD.encode(&buffer[..read]),
                }),
            )
            .await?;
        if response.get("accepted").and_then(Value::as_bool) != Some(true)
            || response.get("offset").and_then(Value::as_u64) != Some(next_offset)
        {
            anyhow::bail!(
                "Device dispatch target returned a mismatched workspace chunk acknowledgement"
            );
        }
        offset = next_offset;
    }

    let deadline = tokio::time::Instant::now() + DEVICE_WORKSPACE_COMMIT_WAIT;
    loop {
        let committed = rpc
            .invoke(
                device_id,
                "dispatch_target_workspace_commit",
                json!({ "jobId": job_id }),
            )
            .await?;
        if committed
            .pointer("/metadata/archiveSha256")
            .and_then(Value::as_str)
            != Some(metadata.archive_sha256.as_str())
        {
            anyhow::bail!("Device dispatch target returned mismatched workspace snapshot metadata");
        }
        if committed.get("committed").and_then(Value::as_bool) == Some(true) {
            return required_workspace_path(&committed);
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!(
                "Device dispatch target workspace materialization did not finish within 15 minutes"
            );
        }
        tokio::time::sleep(DEVICE_WORKSPACE_COMMIT_POLL_INTERVAL).await;
    }
}

fn required_workspace_path(response: &Value) -> anyhow::Result<String> {
    response
        .get("workspacePath")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("Device dispatch target returned no materialized workspace path"))
}

async fn load_device_record(
    store: &OutboundDispatchStore,
    job_id: &str,
) -> anyhow::Result<OutboundDispatchRecord> {
    let record = store
        .get(job_id)
        .await?
        .ok_or_else(|| anyhow!("Outbound dispatch job was not found"))?;
    if !matches!(record.target, DispatchTarget::Device { .. }) {
        anyhow::bail!("Outbound dispatch job is not bound to a device target");
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    #[test]
    fn target_command_names_are_separate_from_outbound_commands() {
        for command in [
            "dispatch_target_probe",
            "dispatch_target_submit",
            "dispatch_target_status",
            "dispatch_target_cancel",
            "dispatch_target_list",
            "dispatch_target_answer",
            "dispatch_target_append",
            "dispatch_target_workspace_begin",
            "dispatch_target_workspace_chunk",
            "dispatch_target_workspace_commit",
        ] {
            assert!(command.starts_with("dispatch_target_"));
            assert_ne!(command, "dispatch_submit");
        }
    }
}
