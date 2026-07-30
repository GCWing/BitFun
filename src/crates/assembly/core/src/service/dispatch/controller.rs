use anyhow::Context as _;
use bitfun_services_core::dispatch_workspace::{
    apply_workspace_result_bundle, WorkspaceResultApplyOutcome, WorkspaceResultSummary,
};
use bitfun_services_integrations::remote_ssh::{
    dispatch_ssh::{
        self, DispatchCliRelease, DispatchInstallPoll, DispatchInstallStart, DispatchSshProbe,
    },
    SSHConnectionManager,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    adopt_target_jobs, DispatchTarget, DispatchTargetRequest, DispatchWorkspaceDeliveryRequest,
    OutboundDispatchRecord, OutboundDispatchStore,
};

pub(super) const DISPATCH_PROTOCOL_VERSION: u64 = 2;
pub(super) const MAX_DISPATCH_TEXT_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispatchListTargetsRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchProbeTargetRequest {
    pub target: DispatchTargetRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchConnectionRequest {
    pub connection_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchInstallStartRequest {
    pub connection_id: String,
    pub release: DispatchCliRelease,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchInstallPollRequest {
    pub connection_id: String,
    #[serde(default)]
    pub cursor: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchSubmitRequest {
    pub target: DispatchTargetRequest,
    #[serde(default)]
    pub workspace_delivery: DispatchWorkspaceDeliveryRequest,
    pub job_id: String,
    pub session_id: String,
    pub agent_type: String,
    pub prompt: String,
    pub approval_policy: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStatusRequest {
    pub job_id: String,
    #[serde(default)]
    pub cursor: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchJobRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispatchApplyResultRequest {
    pub job_id: String,
    /// Local workspace the bundle is applied to.
    pub workspace_path: String,
    /// Take the target's version for paths that changed on both sides.
    #[serde(default)]
    pub overwrite_conflicts: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchPermissionReplyKind {
    Once,
    Always,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchAnswerRequest {
    pub job_id: String,
    pub request_id: String,
    pub reply: DispatchPermissionReplyKind,
    #[serde(default)]
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchAppendRequest {
    pub job_id: String,
    pub message_id: String,
    pub content: String,
    #[serde(default)]
    pub display_content: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchListJobsRequest {
    #[serde(default)]
    pub target: Option<DispatchTargetRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchTargetOption {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,
}

pub async fn list_targets(
    manager: &SSHConnectionManager,
    _request: DispatchListTargetsRequest,
) -> anyhow::Result<Vec<DispatchTargetOption>> {
    let mut targets = vec![DispatchTargetOption {
        kind: "local".to_string(),
        connection_id: None,
        device_id: None,
        display_name: "Local".to_string(),
        description: None,
        default_workspace: None,
        online: None,
    }];
    targets.extend(
        manager
            .get_saved_connections()
            .await
            .into_iter()
            .map(|connection| DispatchTargetOption {
                kind: "ssh".to_string(),
                connection_id: Some(connection.id),
                device_id: None,
                display_name: connection.name,
                description: Some(format!(
                    "{}@{}:{}",
                    connection.username, connection.host, connection.port
                )),
                default_workspace: connection.default_workspace,
                online: None,
            }),
    );
    Ok(targets)
}

pub async fn probe_target(
    manager: &SSHConnectionManager,
    request: DispatchProbeTargetRequest,
) -> anyhow::Result<DispatchSshProbe> {
    let DispatchTargetRequest::Ssh {
        connection_id,
        workspace_path,
    } = request.target
    else {
        anyhow::bail!("SSH dispatch probing requires an SSH target");
    };
    dispatch_ssh::probe(manager, &connection_id, nonempty(&workspace_path)).await
}

pub async fn install_cli_start(
    manager: &SSHConnectionManager,
    request: DispatchInstallStartRequest,
) -> anyhow::Result<DispatchInstallStart> {
    dispatch_ssh::install_cli_start(manager, request.connection_id.trim(), &request.release).await
}

/// Build and install the CLI from source, for targets no published binary fits.
pub async fn install_cli_source_start(
    manager: &SSHConnectionManager,
    request: DispatchConnectionRequest,
) -> anyhow::Result<DispatchInstallStart> {
    dispatch_ssh::install_cli_source_start(manager, request.connection_id.trim()).await
}

pub async fn install_cli_poll(
    manager: &SSHConnectionManager,
    request: DispatchInstallPollRequest,
) -> anyhow::Result<DispatchInstallPoll> {
    dispatch_ssh::install_cli_poll(manager, request.connection_id.trim(), request.cursor).await
}

pub async fn install_cli_cancel(
    manager: &SSHConnectionManager,
    request: DispatchConnectionRequest,
) -> anyhow::Result<()> {
    dispatch_ssh::install_cli_cancel(manager, request.connection_id.trim()).await
}

/// Copy this controller's model configuration (catalog, credentials, and
/// default-model selections) onto the SSH target so its CLI can resolve a
/// ready model. Explicit, credential-bearing operation: the UI must confirm
/// before calling it, mirroring CLI installation.
pub async fn sync_model_config(
    manager: &SSHConnectionManager,
    request: DispatchConnectionRequest,
) -> anyhow::Result<()> {
    crate::service::config::initialize_global_config()
        .await
        .map_err(|error| anyhow::anyhow!("initialize controller configuration: {error}"))?;
    let config_service = crate::service::config::get_global_config_service()
        .await
        .map_err(|error| anyhow::anyhow!("read controller configuration: {error}"))?;
    let config: crate::service::config::GlobalConfig = config_service
        .get_config(None)
        .await
        .map_err(|error| anyhow::anyhow!("load controller configuration: {error}"))?;
    if !config.ai.models.iter().any(|model| model.enabled) {
        anyhow::bail!("no enabled AI model is configured on this device to sync");
    }
    let ai = serde_json::to_value(&config.ai)
        .map_err(|error| anyhow::anyhow!("encode controller model configuration: {error}"))?;
    let mut payload = serde_json::Map::new();
    for key in [
        "models",
        "default_models",
        "agent_model_defaults",
        "func_agent_models",
    ] {
        if let Some(value) = ai.get(key) {
            payload.insert(key.to_string(), value.clone());
        }
    }
    dispatch_ssh::sync_model_config(
        manager,
        request.connection_id.trim(),
        &Value::Object(payload),
    )
    .await
}

pub async fn submit(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchSubmitRequest,
) -> anyhow::Result<Value> {
    validate_submit_request(&request)?;

    let DispatchTargetRequest::Ssh {
        connection_id,
        workspace_path: requested_workspace_path,
    } = &request.target
    else {
        anyhow::bail!("SSH dispatch submission requires an SSH target");
    };
    if connection_id.trim().is_empty() {
        anyhow::bail!("SSH dispatch requires a connectionId");
    }
    let workspace_path = resolve_ssh_workspace(
        manager,
        store,
        connection_id,
        requested_workspace_path,
        &request.workspace_delivery,
        &request.job_id,
    )
    .await?;

    // Re-check the executable that will receive this submission. The picker
    // probe can be stale, and headless callers can bypass the UI entirely.
    let preflight =
        dispatch_ssh::probe(manager, connection_id, Some(workspace_path.trim())).await?;
    let protocol = preflight.protocol.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            preflight
                .protocol_error
                .as_deref()
                .or(preflight.install_error.as_deref())
                .unwrap_or("BitFun CLI dispatch protocol is unavailable on the SSH target")
        )
    })?;
    dispatch_ssh::validate_dispatch_protocol(protocol, Some(&request.approval_policy))?;
    validate_submission_preflight(protocol, request.model.as_deref())?;
    let workspace_path = protocol
        .pointer("/workspace/path")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .unwrap_or(workspace_path.as_str())
        .to_string();

    let display_name = manager
        .get_saved_connections()
        .await
        .into_iter()
        .find(|connection| connection.id == *connection_id)
        .map(|connection| connection.name)
        .unwrap_or_else(|| connection_id.clone());
    let resolved_target = DispatchTarget::Ssh {
        connection_id: connection_id.clone(),
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

    let mut protocol_request = json!({
        "protocolVersion": DISPATCH_PROTOCOL_VERSION,
        "jobId": request.job_id.clone(),
        "sessionId": request.session_id.clone(),
        "workspacePath": workspace_path,
        "agentType": request.agent_type,
        "prompt": request.prompt,
        "approvalPolicy": request.approval_policy,
    });
    if let Some(model) = request.model.filter(|value| !value.trim().is_empty()) {
        protocol_request["model"] = Value::String(model);
    }
    if let Some(title) = request.title.filter(|value| !value.trim().is_empty()) {
        protocol_request["title"] = Value::String(title);
    }

    let response = match dispatch_ssh::submit(manager, connection_id, &protocol_request).await {
        Ok(response) => response,
        Err(error) => {
            // The SSH response can be lost after the target has durably
            // accepted and detached the worker. Preserve an observable,
            // retryable state instead of freezing the outbound record at a
            // false terminal failure; status or an idempotent re-submit will
            // reconcile the authoritative target state.
            let _ = store
                .update_progress(&request.job_id, 0, "submission_unknown")
                .await;
            return Err(error);
        }
    };
    if let Err(error) = validate_submit_ack(&response, &request.job_id, &request.session_id) {
        let _ = store
            .update_progress(&request.job_id, 0, "submission_unknown")
            .await;
        return Err(error);
    }
    let state = response
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("queued")
        .to_string();
    store.update_progress(&request.job_id, 0, state).await?;
    Ok(response)
}

async fn resolve_ssh_workspace(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    connection_id: &str,
    requested_workspace_path: &str,
    delivery: &DispatchWorkspaceDeliveryRequest,
    job_id: &str,
) -> anyhow::Result<String> {
    match delivery {
        DispatchWorkspaceDeliveryRequest::Existing => {
            let workspace_path = requested_workspace_path.trim();
            if workspace_path.is_empty() {
                anyhow::bail!("existing SSH dispatch requires a workspacePath");
            }
            Ok(workspace_path.to_string())
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
            let begin_request = json!({
                "protocolVersion": DISPATCH_PROTOCOL_VERSION,
                "jobId": job_id,
                "metadata": prepared.metadata,
            });
            let committed = dispatch_ssh::upload_workspace_snapshot(
                manager,
                connection_id,
                &begin_request,
                &prepared.archive_path,
            )
            .await?;
            committed
                .get("workspacePath")
                .and_then(Value::as_str)
                .filter(|path| !path.trim().is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "dispatch target did not return the materialized workspace path"
                    )
                })
        }
    }
}

pub async fn status(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchStatusRequest,
) -> anyhow::Result<Value> {
    let record = store
        .get(&request.job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Outbound dispatch job was not found"))?;
    let DispatchTarget::Ssh { connection_id, .. } = &record.target else {
        anyhow::bail!("SSH dispatch status requires an SSH target");
    };
    let response = dispatch_ssh::status(
        manager,
        connection_id,
        &json!({ "jobId": request.job_id, "cursor": request.cursor }),
    )
    .await?;

    // The request cursor is the last cursor the observer already applied. The
    // response cursor is deliberately not persisted until the next poll, so a
    // controller crash cannot skip events.
    let state = response
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or(record.last_state.as_str())
        .to_string();
    store
        .update_progress(&record.job_id, request.cursor, state)
        .await?;
    // A successful status proves that the target durably owns the job and its
    // materialized snapshot. The controller no longer needs the source archive.
    let _ = store.remove_workspace_snapshot(&record.job_id).await;
    Ok(response)
}

/// Fetch what a finished snapshot job changed on its target.
///
/// Download and inspection only. The bundle lands in this controller's own
/// staging area; nothing touches the user's workspace until they review the
/// reported diff and explicitly apply it. The target tree and the local tree
/// have diverged independently since the snapshot, so silently merging would
/// be the one thing detached execution must never do.
pub async fn pull_result(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchJobRequest,
) -> anyhow::Result<Value> {
    let record = store
        .get(&request.job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Outbound dispatch job was not found"))?;
    let DispatchTarget::Ssh { connection_id, .. } = &record.target else {
        anyhow::bail!("SSH dispatch result pull requires an SSH target");
    };
    let destination = result_bundle_path(store, &request.job_id);
    let response =
        dispatch_ssh::pull_result(manager, connection_id, &request.job_id, &destination).await?;
    record_result_summary(store, &request.job_id, &response)?;
    Ok(response)
}

/// Persist the summary next to the bundle so applying reads both from disk.
///
/// The digests that decide whether a local file may be overwritten must come
/// from the verified pull, not from whatever the caller hands back later.
pub(super) fn record_result_summary(
    store: &OutboundDispatchStore,
    job_id: &str,
    response: &Value,
) -> anyhow::Result<()> {
    if let Some(summary) = response.get("summary") {
        // Owner-only like the bundle beside it: this records which paths of the
        // user's workspace changed.
        let summary_path = result_summary_path(store, job_id);
        dispatch_ssh::write_private_file(&summary_path, &serde_json::to_vec(summary)?)
            .with_context(|| format!("record result summary {}", summary_path.display()))?;
    }
    Ok(())
}

pub(super) fn result_bundle_path(
    store: &OutboundDispatchStore,
    job_id: &str,
) -> std::path::PathBuf {
    store
        .root()
        .join(super::OUTBOUND_RESULTS_DIR)
        .join(format!("{job_id}.tar.gz"))
}

fn result_summary_path(store: &OutboundDispatchStore, job_id: &str) -> std::path::PathBuf {
    store
        .root()
        .join(super::OUTBOUND_RESULTS_DIR)
        .join(format!("{job_id}.json"))
}

/// Apply a pulled result bundle to a local workspace.
///
/// Refuses to write anything when a path changed on both sides unless the user
/// explicitly chose to take the target's version.
pub async fn apply_result(
    store: &OutboundDispatchStore,
    request: DispatchApplyResultRequest,
) -> anyhow::Result<WorkspaceResultApplyOutcome> {
    let workspace = request.workspace_path.trim();
    if workspace.is_empty() {
        anyhow::bail!("Applying dispatch results requires a workspacePath");
    }
    let bundle = result_bundle_path(store, &request.job_id);
    if !bundle.is_file() {
        anyhow::bail!("Pull the dispatch result before applying it");
    }
    let summary: WorkspaceResultSummary =
        serde_json::from_slice(&std::fs::read(result_summary_path(store, &request.job_id))?)
            .context("read recorded dispatch result summary")?;
    apply_workspace_result_bundle(
        &bundle,
        std::path::Path::new(workspace),
        &summary,
        request.overwrite_conflicts,
    )
}

pub async fn cancel(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchJobRequest,
) -> anyhow::Result<Value> {
    let record = store
        .get(&request.job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Outbound dispatch job was not found"))?;
    let DispatchTarget::Ssh { connection_id, .. } = &record.target else {
        anyhow::bail!("SSH dispatch cancellation requires an SSH target");
    };
    let response =
        dispatch_ssh::cancel(manager, connection_id, &json!({ "jobId": request.job_id })).await?;
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

pub async fn answer(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchAnswerRequest,
) -> anyhow::Result<Value> {
    validate_answer_request(&request)?;
    let record = store
        .get(&request.job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Outbound dispatch job was not found"))?;
    let DispatchTarget::Ssh { connection_id, .. } = &record.target else {
        anyhow::bail!("SSH dispatch permission answers require an SSH target");
    };
    let mut payload = json!({
        "jobId": request.job_id,
        "requestId": request.request_id,
        "reply": request.reply,
    });
    if let Some(feedback) = request.feedback.filter(|value| !value.trim().is_empty()) {
        payload["feedback"] = Value::String(feedback);
    }
    dispatch_ssh::answer(manager, connection_id, &payload).await
}

pub async fn append(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchAppendRequest,
) -> anyhow::Result<Value> {
    validate_append_request(&request)?;
    let record = store
        .get(&request.job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Outbound dispatch job was not found"))?;
    let DispatchTarget::Ssh { connection_id, .. } = &record.target else {
        anyhow::bail!("SSH dispatch message append requires an SSH target");
    };
    dispatch_ssh::append(manager, connection_id, &serde_json::to_value(request)?).await
}

pub async fn list_jobs(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchListJobsRequest,
) -> anyhow::Result<Value> {
    let Some(target) = request.target else {
        return Ok(serde_json::to_value(store.list().await?)?);
    };
    let DispatchTargetRequest::Ssh { connection_id, .. } = target else {
        anyhow::bail!("SSH dispatch listing requires an SSH target");
    };
    let display_name = manager
        .get_saved_connections()
        .await
        .into_iter()
        .find(|connection| connection.id == connection_id)
        .map(|connection| connection.name)
        .unwrap_or_else(|| connection_id.clone());
    let response = dispatch_ssh::list(manager, &connection_id, &json!({})).await?;
    adopt_target_jobs(
        store,
        &DispatchTarget::Ssh {
            connection_id,
            workspace_path: String::new(),
            display_name,
        },
        &response,
    )
    .await?;
    Ok(response)
}

pub(super) fn validate_submit_request(request: &DispatchSubmitRequest) -> anyhow::Result<()> {
    if !matches!(
        request.approval_policy.as_str(),
        "auto" | "reject-and-report" | "remote"
    ) {
        anyhow::bail!(
            "Dispatch approvalPolicy must be explicitly set to auto, reject-and-report, or remote"
        );
    }
    if request.job_id.trim().is_empty() || request.session_id.trim().is_empty() {
        anyhow::bail!("Dispatch jobId and sessionId cannot be empty");
    }
    if request.agent_type.trim().is_empty() {
        anyhow::bail!("Dispatch agentType cannot be empty");
    }
    if request.prompt.trim().is_empty() {
        anyhow::bail!("Dispatch prompt cannot be empty");
    }
    if request.prompt.len() > MAX_DISPATCH_TEXT_BYTES {
        anyhow::bail!("Dispatch prompt exceeds the 32 KiB request limit");
    }
    Ok(())
}

pub(super) fn validate_append_request(request: &DispatchAppendRequest) -> anyhow::Result<()> {
    if request.message_id.trim().is_empty() || request.message_id.len() > 128 {
        anyhow::bail!("Dispatch messageId must contain 1-128 bytes");
    }
    if request.content.trim().is_empty() {
        anyhow::bail!("Dispatch appended message cannot be empty");
    }
    let total_bytes = request
        .content
        .len()
        .saturating_add(request.display_content.as_ref().map_or(0, String::len));
    if total_bytes > MAX_DISPATCH_TEXT_BYTES {
        anyhow::bail!("Dispatch appended message exceeds the 32 KiB request limit");
    }
    Ok(())
}

pub(super) fn validate_answer_request(request: &DispatchAnswerRequest) -> anyhow::Result<()> {
    if request.request_id.trim().is_empty() || request.request_id.len() > 512 {
        anyhow::bail!("Dispatch permission requestId is invalid");
    }
    if request
        .feedback
        .as_ref()
        .is_some_and(|feedback| feedback.len() > MAX_DISPATCH_TEXT_BYTES)
    {
        anyhow::bail!("Dispatch permission feedback exceeds the 32 KiB request limit");
    }
    Ok(())
}

pub(super) fn validate_submit_ack(
    response: &Value,
    job_id: &str,
    session_id: &str,
) -> anyhow::Result<()> {
    if response.get("accepted").and_then(Value::as_bool) != Some(true) {
        anyhow::bail!("Dispatch target did not accept the job");
    }
    if response.get("jobId").and_then(Value::as_str) != Some(job_id)
        || response.get("sessionId").and_then(Value::as_str) != Some(session_id)
    {
        anyhow::bail!("Dispatch target returned a mismatched acknowledgement");
    }
    Ok(())
}

pub(super) fn same_target_identity(left: &DispatchTarget, right: &DispatchTarget) -> bool {
    match (left, right) {
        (DispatchTarget::Local, DispatchTarget::Local) => true,
        (
            DispatchTarget::Ssh {
                connection_id: left_connection,
                workspace_path: left_workspace,
                ..
            },
            DispatchTarget::Ssh {
                connection_id: right_connection,
                workspace_path: right_workspace,
                ..
            },
        ) => left_connection == right_connection && left_workspace == right_workspace,
        (
            DispatchTarget::Device {
                device_id: left_device,
                workspace_path: left_workspace,
                ..
            },
            DispatchTarget::Device {
                device_id: right_device,
                workspace_path: right_workspace,
                ..
            },
        ) => left_device == right_device && left_workspace == right_workspace,
        _ => false,
    }
}

pub(super) fn validate_submission_preflight(
    protocol: &Value,
    requested_model: Option<&str>,
) -> anyhow::Result<()> {
    let workspace = protocol
        .get("workspace")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("Dispatch target did not report workspace readiness"))?;
    if workspace.get("exists").and_then(Value::as_bool) != Some(true)
        || workspace.get("isDirectory").and_then(Value::as_bool) != Some(true)
    {
        anyhow::bail!("Dispatch workspace does not exist or is not a directory on the target");
    }
    if let Some(requested_model) = requested_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        let available = protocol
            .get("availableModels")
            .and_then(Value::as_array)
            .is_some_and(|models| {
                models
                    .iter()
                    .any(|model| model.as_str() == Some(requested_model))
            });
        if !available {
            anyhow::bail!(
                "Requested model '{requested_model}' is not ready on the dispatch target"
            );
        }
    } else if protocol.get("modelConfigured").and_then(Value::as_bool) != Some(true) {
        let diagnostic = protocol
            .get("modelDiagnostic")
            .and_then(Value::as_str)
            .unwrap_or("No ready default model is configured on the dispatch target");
        anyhow::bail!("{diagnostic}");
    }
    Ok(())
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_false_or_mismatched_submit_acknowledgements() {
        assert!(validate_submit_ack(
            &json!({"accepted": false, "jobId": "j", "sessionId": "s"}),
            "j",
            "s"
        )
        .is_err());
        assert!(validate_submit_ack(
            &json!({"accepted": true, "jobId": "other", "sessionId": "s"}),
            "j",
            "s"
        )
        .is_err());
        assert!(validate_submit_ack(
            &json!({"accepted": true, "jobId": "j", "sessionId": "s"}),
            "j",
            "s"
        )
        .is_ok());
    }

    #[test]
    fn submission_preflight_requires_workspace_and_target_model_readiness() {
        let ready = json!({
            "workspace": { "exists": true, "isDirectory": true },
            "modelConfigured": true,
            "availableModels": ["target-model"]
        });
        validate_submission_preflight(&ready, None).expect("target default");
        validate_submission_preflight(&ready, Some("target-model")).expect("selected model");
        assert!(validate_submission_preflight(&ready, Some("local-only-model")).is_err());

        let missing_workspace = json!({
            "workspace": { "exists": false, "isDirectory": false },
            "modelConfigured": true,
            "availableModels": []
        });
        assert!(validate_submission_preflight(&missing_workspace, None).is_err());

        let missing_model = json!({
            "workspace": { "exists": true, "isDirectory": true },
            "modelConfigured": false,
            "modelDiagnostic": "configure a model",
            "availableModels": []
        });
        assert!(validate_submission_preflight(&missing_model, None).is_err());
    }

    #[test]
    fn target_identity_ignores_mutable_display_names() {
        let before = DispatchTarget::Ssh {
            connection_id: "server-a".to_string(),
            workspace_path: "/srv/app".to_string(),
            display_name: "Old label".to_string(),
        };
        let renamed = DispatchTarget::Ssh {
            connection_id: "server-a".to_string(),
            workspace_path: "/srv/app".to_string(),
            display_name: "New label".to_string(),
        };
        assert!(same_target_identity(&before, &renamed));
    }
}
