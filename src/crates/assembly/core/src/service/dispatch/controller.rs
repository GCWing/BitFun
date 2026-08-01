use bitfun_services_integrations::remote_ssh::{
    dispatch_ssh::{
        self, DispatchCliRelease, DispatchInstallPoll, DispatchInstallStart, DispatchSshProbe,
    },
    SSHConnectionManager,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::service::worktree::WorktreeService;

use super::baseline::{
    base_commit_is_published, build_base_bundle, ensure_baseline_branch, fetch_result_bundle,
    outbound_record_owns_baseline, prepare_baseline, release_prepared_baseline, PreparedBaseline,
};
use super::preparation::{DispatchPreparationRequest, DispatchPreparationTarget};
use super::{
    adopt_target_jobs, DispatchTarget, DispatchTargetRequest, OutboundDispatchRecord,
    OutboundDispatchStore,
};

pub(super) const DISPATCH_PROTOCOL_VERSION: u64 = 3;
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
    /// Revision used to create the controller baseline worktree. It is
    /// resolved once, then both sides use the resulting immutable commit.
    #[serde(default)]
    pub base_ref: Option<String>,
    /// Carry the baseline worktree's uncommitted changes into `base_commit`.
    #[serde(default)]
    pub include_uncommitted: bool,
    pub job_id: String,
    pub session_id: String,
    pub agent_type: String,
    pub prompt: String,
    pub approval_policy: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    /// Controller-side workspace that owns the observer session.
    #[serde(default)]
    pub source_workspace_path: Option<String>,
    #[serde(default)]
    pub source_workspace_id: Option<String>,
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
pub struct DispatchSyncResultRequest {
    pub job_id: String,
    /// Commit message used when the target still has uncommitted changes.
    #[serde(default)]
    pub message: Option<String>,
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

/// Start the next turn of an existing dispatch session.
///
/// Separate from `append`, which steers a turn that is still running: this one
/// is for a job whose previous turn has finished, and it is what makes a
/// dispatch session hold a conversation instead of a single exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchContinueRequest {
    pub job_id: String,
    /// Caller-generated identity so a retry cannot start two turns.
    pub turn_id: String,
    pub prompt: String,
    #[serde(default)]
    pub display_content: Option<String>,
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
        // The target path is the target's business now: dispatch always checks
        // out its own worktree there rather than reusing a directory.
        workspace_path: _,
    } = &request.target
    else {
        anyhow::bail!("SSH dispatch submission requires an SSH target");
    };
    if connection_id.trim().is_empty() {
        anyhow::bail!("SSH dispatch requires a connectionId");
    }

    let source_workspace_path = request
        .source_workspace_path
        .as_deref()
        .unwrap_or_default()
        .trim();
    let project_workspace_path =
        WorktreeService::resolve_project_workspace_path(source_workspace_path)
            .await
            .map_err(|error| anyhow::anyhow!("resolve the dispatch project workspace: {error}"))?;
    // One controller process at a time may install/provision/submit a job. The
    // JSON journal uses a different lock, so audit events can still be appended
    // atomically while this long-lived guard is held.
    let _preparation_run_lock = store.acquire_preparation_run_lock(&request.job_id).await?;
    if let Some(existing) = store.get(&request.job_id).await? {
        if existing.session_id != request.session_id
            || !matches!(
                &existing.target,
                DispatchTarget::Ssh {
                    connection_id: existing_connection,
                    ..
                } if existing_connection == connection_id
            )
        {
            anyhow::bail!("Dispatch jobId is already bound to another target or session");
        }
    }
    store
        .begin_preparation(DispatchPreparationRequest {
            job_id: request.job_id.clone(),
            session_id: request.session_id.clone(),
            target: DispatchPreparationTarget::ssh(connection_id.clone()),
            source_workspace_path: source_workspace_path.to_string(),
            project_workspace_path,
        })
        .await?;

    // Re-check the executable that will receive this submission, installing it
    // when missing. The picker probe can be stale, and headless callers bypass
    // the UI entirely. Every audit event is durably journaled before the
    // corresponding remote mutation, then replayed into the target event log.
    let audit_attempt = uuid::Uuid::new_v4().as_simple().to_string();
    let mut audit_sequence = 0_u32;
    let audit_store = store.clone();
    let audit_job_id = request.job_id.clone();
    let cli_probe =
        dispatch_ssh::ensure_target_cli(manager, connection_id, move |stage, release| {
            audit_sequence = audit_sequence.saturating_add(1);
            let event_id = format!("{audit_attempt}:{audit_sequence}");
            let stage = stage.to_string();
            let audit_store = audit_store.clone();
            let audit_job_id = audit_job_id.clone();
            async move {
                log::info!("Dispatch SSH CLI install: stage={stage} details={release}");
                audit_store
                    .append_preparation_setup_audit(
                        &audit_job_id,
                        &event_id,
                        json!({
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                            "action": "cli-install",
                            "details": {
                                "stage": stage,
                                "release": release,
                            },
                        }),
                    )
                    .await
            }
        })
        .await?;
    recover_interrupted_cli_install_audit(store, &request.job_id, &cli_probe).await?;
    let cli_protocol = cli_probe.protocol.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            cli_probe
                .protocol_error
                .as_deref()
                .or(cli_probe.install_error.as_deref())
                .unwrap_or("BitFun CLI dispatch protocol is unavailable on the SSH target")
        )
    })?;
    dispatch_ssh::validate_dispatch_protocol(cli_protocol, Some(&request.approval_policy))?;

    let baseline = prepare_baseline(
        store,
        &request.job_id,
        source_workspace_path,
        request.base_ref.as_deref(),
        request.include_uncommitted,
    )
    .await?;
    if let Err(error) = store
        .attach_preparation_baseline(
            &request.job_id,
            &baseline.delivery.baseline_worktree_id,
            &baseline.delivery.branch,
        )
        .await
    {
        release_prepared_baseline(store, &request.job_id, &baseline).await;
        return Err(error);
    }
    store.touch_preparation(&request.job_id).await?;

    let workspace_path =
        match provision_ssh_workspace(manager, store, connection_id, &request.job_id, &baseline)
            .await
        {
            Ok(path) => path,
            Err(error) => {
                release_unbound_preparation_baseline(store, &request.job_id, &baseline).await;
                return Err(error);
            }
        };
    store.touch_preparation(&request.job_id).await?;

    // Provision owns the canonical target path. Probe that exact worktree so
    // model readiness and Git identity are checked immediately before submit.
    let workspace_probe =
        match dispatch_ssh::probe(manager, connection_id, Some(&workspace_path)).await {
            Ok(probe) => probe,
            Err(error) => {
                release_unbound_preparation_baseline(store, &request.job_id, &baseline).await;
                return Err(error);
            }
        };
    let protocol = match workspace_probe.protocol.as_ref() {
        Some(protocol) => protocol,
        None => {
            release_unbound_preparation_baseline(store, &request.job_id, &baseline).await;
            anyhow::bail!(
                "{}",
                workspace_probe.protocol_error.as_deref().unwrap_or(
                    "BitFun CLI dispatch protocol is unavailable in the target worktree"
                )
            );
        }
    };
    if let Err(error) =
        dispatch_ssh::validate_dispatch_protocol(protocol, Some(&request.approval_policy))
            .and_then(|_| validate_submission_preflight(protocol, request.model.as_deref()))
    {
        release_unbound_preparation_baseline(store, &request.job_id, &baseline).await;
        return Err(error);
    }

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

    let requested_record = match OutboundDispatchRecord::new(
        request.job_id.clone(),
        resolved_target,
        request.session_id.clone(),
        workspace_path.clone(),
        &request.prompt,
        "submitting",
    ) {
        Ok(record) => record,
        Err(error) => {
            release_unbound_preparation_baseline(store, &request.job_id, &baseline).await;
            return Err(error.into());
        }
    }
    .with_submission_metadata(
        request.title.clone(),
        request.agent_type.clone(),
        request.approval_policy.clone(),
        request.model.clone(),
    )
    .with_source_workspace(
        request.source_workspace_path.clone(),
        request.source_workspace_id.clone(),
    )
    .with_baseline(&baseline.delivery, &baseline.worktree_path);
    let bound_record = bind_outbound_record(store, &requested_record, &baseline).await?;
    if bound_record.session_id != request.session_id
        || !same_target_identity(&bound_record.target, &requested_record.target)
    {
        anyhow::bail!("Dispatch jobId is already bound to another target or session");
    }
    store
        .mark_preparation_outbound_bound(&request.job_id)
        .await?;
    let setup_audit = store.preparation_setup_audit(&request.job_id).await?;

    let mut protocol_request = json!({
        "protocolVersion": DISPATCH_PROTOCOL_VERSION,
        "jobId": request.job_id.clone(),
        "sessionId": request.session_id.clone(),
        "workspacePath": workspace_path,
        "agentType": request.agent_type,
        "prompt": request.prompt,
        "approvalPolicy": request.approval_policy,
        "setupAudit": setup_audit,
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
    if let Err(error) = store.acknowledge_preparation(&request.job_id).await {
        // The target ACK is authoritative. Retaining a redundant journal is
        // safe and lets status/retry remove it later; failing the already-live
        // task here would be misleading.
        log::warn!(
            "Failed to remove acknowledged dispatch preparation: job_id={} error={}",
            request.job_id,
            error
        );
    }
    Ok(response)
}

async fn recover_interrupted_cli_install_audit(
    store: &OutboundDispatchStore,
    job_id: &str,
    probe: &DispatchSshProbe,
) -> anyhow::Result<()> {
    let events = store.preparation_setup_audit(job_id).await?;
    if events.is_empty() {
        return Ok(());
    }
    let last_stage = events.iter().rev().find_map(|event| {
        event
            .pointer("/details/stage")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|stage| !stage.is_empty())
    });
    if last_stage == Some("cli-install-succeeded") {
        return Ok(());
    }
    let version = probe
        .protocol
        .as_ref()
        .and_then(|protocol| protocol.get("cliVersion"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    store
        .append_preparation_setup_audit(
            job_id,
            &format!("recovered-{}", uuid::Uuid::new_v4().as_simple()),
            json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "action": "cli-install",
                "details": {
                    "stage": "cli-install-succeeded",
                    "release": {
                        "version": version,
                        "cliPath": probe.cli_path,
                        "recovered": true,
                    },
                },
            }),
        )
        .await
}

pub(super) async fn release_unbound_preparation_baseline(
    store: &OutboundDispatchStore,
    job_id: &str,
    baseline: &PreparedBaseline,
) {
    release_prepared_baseline(store, job_id, baseline).await;
    if let Err(error) = store.clear_preparation_baseline(job_id).await {
        // Keep the exact journal on any ambiguity. Expiry reconciliation can
        // retry the release, whereas deleting its identity could strand it.
        log::warn!(
            "Failed to clear released dispatch preparation baseline: job_id={} error={}",
            job_id,
            error
        );
    }
}

/// Bind the record that takes ownership of a prepared baseline claim.
///
/// A JSON-store error can be ambiguous (for example, permission hardening can
/// fail after the atomic rename). Re-read before releasing so a durable record
/// never loses the claim that keeps its baseline alive.
pub(super) async fn bind_outbound_record(
    store: &OutboundDispatchStore,
    record: &OutboundDispatchRecord,
    baseline: &PreparedBaseline,
) -> anyhow::Result<OutboundDispatchRecord> {
    match store.bind_if_absent(record).await {
        Ok(bound)
            if outbound_record_owns_baseline(
                &bound,
                &baseline.delivery.baseline_worktree_id,
                &baseline.delivery.base_commit,
                &baseline.delivery.branch,
            ) =>
        {
            Ok(bound)
        }
        Ok(_) => {
            release_prepared_baseline(store, &record.job_id, baseline).await;
            anyhow::bail!(
                "Dispatch jobId is already bound to a different baseline worktree, commit, or branch"
            );
        }
        Err(error) => {
            // The write may have become durable before permission hardening or
            // another post-rename step failed. The ownership-aware release
            // helper re-reads the record and preserves a matching claim.
            release_prepared_baseline(store, &record.job_id, baseline).await;
            Err(error.into())
        }
    }
}

/// Check out the baseline commit on the target, shipping objects if needed.
///
/// The target answers `needsBundle` when its own clone cannot reach the commit.
/// Only then does anything cross the wire, so the common case — a commit that
/// is already on the shared remote — costs one round trip and no transfer.
async fn provision_ssh_workspace(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    connection_id: &str,
    job_id: &str,
    baseline: &PreparedBaseline,
) -> anyhow::Result<String> {
    // Both attempts must send an identical request: the target treats a
    // differing request for one job as a conflicting baseline and refuses it.
    let request = json!({
        "protocolVersion": DISPATCH_PROTOCOL_VERSION,
        "jobId": job_id,
        "repoKey": baseline.repo_key,
        "projectLabel": baseline.project_label,
        "remoteUrl": baseline.delivery.remote_url,
        "baseCommit": baseline.delivery.base_commit,
        "branch": baseline.delivery.branch,
    });

    let response = dispatch_ssh::provision_workspace(manager, connection_id, &request).await?;
    if let Some(path) = provisioned_path(&response) {
        return Ok(path);
    }
    if response.get("needsBundle").and_then(Value::as_bool) != Some(true) {
        anyhow::bail!("dispatch target neither provisioned a workspace nor asked for a bundle");
    }

    let have_tips = target_have_tips(&response);
    if base_commit_is_published(&baseline.worktree_path, &baseline.delivery.base_commit).await {
        // Worth saying out loud: the commit is on the remote, so the target
        // asking for it means its clone is stale or its network is down.
        log::info!(
            "Dispatch target could not reach a published base commit; delivering it by bundle"
        );
    }
    let bundle = build_base_bundle(store, baseline, &have_tips).await?;
    let upload = dispatch_ssh::upload_bundle(
        manager,
        connection_id,
        job_id,
        &bundle.sha256,
        bundle.size,
        &bundle.path,
    )
    .await;
    // The objects are in the target repository now; the local artifact is pure
    // duplication of history this machine already owns. Remove it either way so
    // a failed upload does not leave a stale bundle behind.
    let _ = std::fs::remove_file(&bundle.path);
    upload?;

    let response = dispatch_ssh::provision_workspace(manager, connection_id, &request).await?;
    provisioned_path(&response).ok_or_else(|| {
        anyhow::anyhow!("dispatch target could not check out the base commit after the bundle")
    })
}

pub(super) fn target_have_tips(response: &Value) -> Vec<String> {
    response
        .get("haveTips")
        .and_then(Value::as_array)
        .map(|tips| {
            tips.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn provisioned_path(response: &Value) -> Option<String> {
    if response.get("provisioned").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    response
        .get("workspacePath")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
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
    if let Err(error) = store.acknowledge_preparation(&record.job_id).await {
        log::warn!(
            "Failed to remove dispatch preparation after status confirmation: job_id={} error={}",
            record.job_id,
            error
        );
    }

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
    Ok(response)
}

/// Bring the target's work back into this controller's baseline worktree.
///
/// One button, two halves: the target commits and bundles its branch, then the
/// controller fast-forwards its baseline onto it. There is no conflict handling
/// because there is no conflict to have — both sides share the base commit, so
/// the fetch either fast-forwards or fails loudly because the user committed
/// into the baseline themselves.
pub async fn sync_result(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchSyncResultRequest,
) -> anyhow::Result<Value> {
    let record = store
        .get(&request.job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Outbound dispatch job was not found"))?;
    let DispatchTarget::Ssh { connection_id, .. } = &record.target else {
        anyhow::bail!("SSH dispatch sync requires an SSH target");
    };
    let destination = result_bundle_path(store, &request.job_id).await?;
    let response = dispatch_ssh::sync_workspace(
        manager,
        connection_id,
        &request.job_id,
        request.message.as_deref(),
        record.synced_head_commit.as_deref(),
        &destination,
    )
    .await?;
    finish_sync(store, &record, response, &destination).await
}

/// Fast-forward the baseline worktree and record what was synced.
///
/// Shared by both transports: only fetching the bundle differs between SSH and
/// an account device.
pub(super) async fn finish_sync(
    store: &OutboundDispatchStore,
    record: &OutboundDispatchRecord,
    mut response: Value,
    bundle: &std::path::Path,
) -> anyhow::Result<Value> {
    if response.get("changed").and_then(Value::as_bool) != Some(true) {
        // A clean sync is still an acknowledgement of the target head. Carry
        // it into the next request so a later click starts a fresh detached
        // sync operation if the still-running agent has since added commits.
        if let Some(head) = response.get("headCommit").and_then(Value::as_str) {
            store.record_synced_head(&record.job_id, head).await?;
        }
        return Ok(response);
    }
    let (Some(worktree_path), Some(branch)) = (
        record.baseline_worktree_path.as_deref(),
        record.branch.as_deref(),
    ) else {
        anyhow::bail!(
            "This dispatch has no recorded baseline worktree, so its result cannot be synced.              It was submitted before Git-worktree delivery."
        );
    };
    if !std::path::Path::new(worktree_path).is_dir() {
        anyhow::bail!(
            "The baseline worktree for this dispatch is missing ({worktree_path}).              Recreate it before syncing."
        );
    }

    ensure_baseline_branch(worktree_path, branch).await?;
    let head = fetch_result_bundle(worktree_path, branch, bundle).await?;
    store.record_synced_head(&record.job_id, &head).await?;
    // The bundle's objects are in the repository now, so keeping the file only
    // duplicates history the user already has.
    let _ = std::fs::remove_file(bundle);
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "baselineWorktreePath".to_string(),
            Value::String(worktree_path.to_string()),
        );
        object.insert("syncedHeadCommit".to_string(), Value::String(head));
    }
    Ok(response)
}

pub(super) async fn result_bundle_path(
    store: &OutboundDispatchStore,
    job_id: &str,
) -> anyhow::Result<std::path::PathBuf> {
    Ok(store.results_dir().await?.join(format!("{job_id}.bundle")))
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

/// Send the next turn of a dispatch session to its SSH target.
pub async fn continue_job(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchContinueRequest,
) -> anyhow::Result<Value> {
    validate_continue_request(&request)?;
    let record = store
        .get(&request.job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Outbound dispatch job was not found"))?;
    let DispatchTarget::Ssh { connection_id, .. } = &record.target else {
        anyhow::bail!("SSH dispatch follow-up requires an SSH target");
    };
    let response =
        dispatch_ssh::continue_job(manager, connection_id, &continue_payload(&request)).await?;
    record_follow_up_state(store, &record, &response).await;
    Ok(response)
}

/// The wire payload both transports send for a follow-up turn.
pub(super) fn continue_payload(request: &DispatchContinueRequest) -> Value {
    let mut payload = json!({
        "protocolVersion": DISPATCH_PROTOCOL_VERSION,
        "jobId": request.job_id,
        "turnId": request.turn_id,
        "prompt": request.prompt,
    });
    if let Some(display) = request
        .display_content
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        payload["displayContent"] = Value::String(display.to_string());
    }
    payload
}

/// Move the observer record back out of its terminal state.
///
/// Best effort: the next status poll reconciles it from the target anyway, but
/// updating here keeps the composer from briefly re-offering "send" as if the
/// follow-up had not been accepted.
pub(super) async fn record_follow_up_state(
    store: &OutboundDispatchStore,
    record: &OutboundDispatchRecord,
    response: &Value,
) {
    let Some(state) = response.get("state").and_then(Value::as_str) else {
        return;
    };
    if let Err(error) = store
        .update_progress(&record.job_id, record.last_cursor, state)
        .await
    {
        log::warn!(
            "Failed to record dispatch follow-up state: job_id={} error={error}",
            record.job_id
        );
    }
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
    if request.base_ref.as_ref().is_some_and(|base_ref| {
        base_ref.trim().is_empty()
            || base_ref.len() > 512
            || base_ref.bytes().any(|byte| byte.is_ascii_control())
    }) {
        anyhow::bail!("Dispatch baseRef is invalid");
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

pub(super) fn validate_continue_request(request: &DispatchContinueRequest) -> anyhow::Result<()> {
    if request.turn_id.trim().is_empty() || request.turn_id.len() > 128 {
        anyhow::bail!("Dispatch turnId must contain 1-128 bytes");
    }
    if request.prompt.trim().is_empty() {
        anyhow::bail!("Dispatch follow-up prompt cannot be empty");
    }
    let total_bytes = request
        .prompt
        .len()
        .saturating_add(request.display_content.as_ref().map_or(0, String::len));
    if total_bytes > MAX_DISPATCH_TEXT_BYTES {
        anyhow::bail!("Dispatch follow-up exceeds the 32 KiB request limit");
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
        || workspace.get("isGitRepository").and_then(Value::as_bool) != Some(true)
    {
        anyhow::bail!("Dispatch workspace is not a Git worktree on the target");
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
            "workspace": { "exists": true, "isDirectory": true, "isGitRepository": true },
            "modelConfigured": true,
            "availableModels": ["target-model"]
        });
        validate_submission_preflight(&ready, None).expect("target default");
        validate_submission_preflight(&ready, Some("target-model")).expect("selected model");
        assert!(validate_submission_preflight(&ready, Some("local-only-model")).is_err());

        let missing_workspace = json!({
            "workspace": { "exists": false, "isDirectory": false, "isGitRepository": false },
            "modelConfigured": true,
            "availableModels": []
        });
        assert!(validate_submission_preflight(&missing_workspace, None).is_err());

        let missing_model = json!({
            "workspace": { "exists": true, "isDirectory": true, "isGitRepository": true },
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

    #[tokio::test]
    async fn finish_sync_checks_the_recorded_branch_before_reading_the_bundle() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repository = temp.path().join("baseline");
        std::fs::create_dir_all(&repository).expect("repository");
        let init = std::process::Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["init", "--quiet", "--initial-branch=main"])
            .output()
            .expect("run git init");
        assert!(
            init.status.success(),
            "git init failed: {}",
            String::from_utf8_lossy(&init.stderr)
        );

        let store =
            OutboundDispatchStore::new_in_root_for_tests(temp.path().join("dispatch-outbound"));
        let mut record = OutboundDispatchRecord::new(
            "job-branch-guard".to_string(),
            DispatchTarget::Local,
            "session-1".to_string(),
            "/target".to_string(),
            "prompt",
            "succeeded",
        )
        .expect("record");
        record.baseline_worktree_path = Some(repository.to_string_lossy().to_string());
        record.branch = Some("bitfun/dispatch/job-branch-guard".to_string());

        let error = finish_sync(
            &store,
            &record,
            json!({"changed": true}),
            &temp.path().join("missing.bundle"),
        )
        .await
        .expect_err("wrong branch must stop sync before bundle inspection");

        assert!(
            error.to_string().contains("baseline is on branch 'main'"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn compatible_reprobe_recovers_a_durable_started_install_audit_once() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = OutboundDispatchStore::new_in_root_for_tests(temp.path().to_path_buf());
        store
            .begin_preparation(DispatchPreparationRequest {
                job_id: "job-install-recovery".to_string(),
                session_id: "session-1".to_string(),
                target: DispatchPreparationTarget::ssh("server-1"),
                source_workspace_path: "/repo/linked".to_string(),
                project_workspace_path: "/repo/main".to_string(),
            })
            .await
            .expect("begin preparation");
        store
            .append_preparation_setup_audit(
                "job-install-recovery",
                "attempt-1:1",
                json!({
                    "timestamp": "2026-07-31T00:00:00Z",
                    "action": "cli-install",
                    "details": {
                        "stage": "cli-install-started",
                        "release": { "version": "1.2.3" },
                    },
                }),
            )
            .await
            .expect("started audit");
        let probe = DispatchSshProbe {
            cli_installed: true,
            cli_path: Some("/home/user/.bitfun/bin/bitfun".to_string()),
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            install_supported: true,
            install_error: None,
            protocol_error: None,
            release: None,
            protocol: Some(json!({ "cliVersion": "1.2.3" })),
            prebuilt_incompatible: None,
            source_build: None,
        };

        recover_interrupted_cli_install_audit(&store, "job-install-recovery", &probe)
            .await
            .expect("recover audit");
        recover_interrupted_cli_install_audit(&store, "job-install-recovery", &probe)
            .await
            .expect("idempotent recovery");

        let events = store
            .preparation_setup_audit("job-install-recovery")
            .await
            .expect("load audit");
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[1].pointer("/details/stage").and_then(Value::as_str),
            Some("cli-install-succeeded")
        );
        assert_eq!(
            events[1]
                .pointer("/details/release/recovered")
                .and_then(Value::as_bool),
            Some(true)
        );
    }
}
