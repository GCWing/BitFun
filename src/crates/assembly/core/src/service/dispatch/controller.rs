use bitfun_services_integrations::remote_ssh::{
    dispatch_ssh::{
        self, DispatchCliRelease, DispatchInstallPoll, DispatchInstallStart, DispatchSshProbe,
    },
    SSHConnectionManager,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{DispatchTarget, DispatchTargetRequest, OutboundDispatchRecord, OutboundDispatchStore};

const DISPATCH_PROTOCOL_VERSION: u64 = 1;

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
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_workspace: Option<String>,
}

pub async fn list_targets(
    manager: &SSHConnectionManager,
    _request: DispatchListTargetsRequest,
) -> anyhow::Result<Vec<DispatchTargetOption>> {
    let mut targets = vec![DispatchTargetOption {
        kind: "local".to_string(),
        connection_id: None,
        display_name: "Local".to_string(),
        description: None,
        default_workspace: None,
    }];
    targets.extend(
        manager
            .get_saved_connections()
            .await
            .into_iter()
            .map(|connection| DispatchTargetOption {
                kind: "ssh".to_string(),
                connection_id: Some(connection.id),
                display_name: connection.name,
                description: Some(format!(
                    "{}@{}:{}",
                    connection.username, connection.host, connection.port
                )),
                default_workspace: connection.default_workspace,
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
        anyhow::bail!("Phase-one dispatch probing supports SSH targets only");
    };
    dispatch_ssh::probe(manager, &connection_id, nonempty(&workspace_path)).await
}

pub async fn install_cli_start(
    manager: &SSHConnectionManager,
    request: DispatchInstallStartRequest,
) -> anyhow::Result<DispatchInstallStart> {
    dispatch_ssh::install_cli_start(manager, request.connection_id.trim(), &request.release).await
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

pub async fn submit(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchSubmitRequest,
) -> anyhow::Result<Value> {
    if !matches!(
        request.approval_policy.as_str(),
        "auto" | "reject-and-report"
    ) {
        anyhow::bail!(
            "Dispatch approvalPolicy must be explicitly set to auto or reject-and-report"
        );
    }
    if request.prompt.trim().is_empty() {
        anyhow::bail!("Dispatch prompt cannot be empty");
    }

    let DispatchTargetRequest::Ssh {
        connection_id,
        workspace_path,
    } = &request.target
    else {
        anyhow::bail!("Phase-one dispatch submission supports SSH targets only");
    };
    if connection_id.trim().is_empty() || workspace_path.trim().is_empty() {
        anyhow::bail!("SSH dispatch requires a connectionId and workspacePath");
    }

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
    )?;
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
        anyhow::bail!("Phase-one dispatch status supports SSH targets only");
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
    Ok(response)
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
        anyhow::bail!("Phase-one dispatch cancellation supports SSH targets only");
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

pub async fn list_jobs(
    manager: &SSHConnectionManager,
    store: &OutboundDispatchStore,
    request: DispatchListJobsRequest,
) -> anyhow::Result<Value> {
    let Some(target) = request.target else {
        return Ok(serde_json::to_value(store.list().await?)?);
    };
    let DispatchTargetRequest::Ssh { connection_id, .. } = target else {
        anyhow::bail!("Phase-one dispatch listing supports SSH targets only");
    };
    dispatch_ssh::list(manager, &connection_id, &json!({})).await
}

fn validate_submit_ack(response: &Value, job_id: &str, session_id: &str) -> anyhow::Result<()> {
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

fn same_target_identity(left: &DispatchTarget, right: &DispatchTarget) -> bool {
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

fn validate_submission_preflight(
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
