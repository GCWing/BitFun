//! Permission HostInvoke handlers for CLI Peer Host.

use serde_json::{json, Value};

use bitfun_agent_runtime::sdk::{PermissionGrantKey, PermissionReply};
use bitfun_core::service::remote_ssh::workspace_state::resolve_workspace_session_identity;
use bitfun_core::service::workspace::WorkspaceKind;

use crate::peer_host::args::{get_string, request_value};
use crate::peer_host::control::attached_controller_lease;
use crate::peer_host::state::PeerHostState;

fn permission_reply(request: &Value) -> Result<PermissionReply, String> {
    match get_string(request, "reply")?.as_str() {
        "once" => Ok(PermissionReply::Once),
        "always" => Ok(PermissionReply::Always),
        "reject" => Ok(PermissionReply::Reject {
            feedback: request
                .get("feedback")
                .and_then(Value::as_str)
                .map(str::to_string),
        }),
        value => Err(format!("Unsupported permission reply: {value}")),
    }
}

fn pagination_value(request: &Value, key: &str, default: usize) -> usize {
    request
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(default)
}

async fn permission_project_id_for_workspace(
    state: &PeerHostState,
    workspace_id: &str,
) -> Result<String, String> {
    let workspace = state
        .workspace_service
        .get_workspace(workspace_id)
        .await
        .ok_or_else(|| format!("Workspace not found: {workspace_id}"))?;
    let is_remote = workspace.workspace_kind == WorkspaceKind::Remote;
    let connection_id = workspace
        .metadata
        .get("connectionId")
        .and_then(Value::as_str);
    let ssh_host = workspace.metadata.get("sshHost").and_then(Value::as_str);
    let identity = resolve_workspace_session_identity(
        &workspace.root_path.to_string_lossy(),
        connection_id,
        ssh_host,
    )
    .await
    .ok_or_else(|| format!("Workspace identity is unavailable: {workspace_id}"))?;
    bitfun_core::agentic::tools::pipeline::permission_project_id_for_workspace_identity(
        &identity, is_remote,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn list_pending_permission_requests(state: &PeerHostState) -> Result<Value, String> {
    let requests = state
        .agent_runtime
        .pending_permission_requests()
        .map_err(|error| error.into_message())?
        .into_iter()
        .filter(|request| state.turns.owns_permission_request(request))
        .collect::<Vec<_>>();
    serde_json::to_value(requests)
        .map_err(|error| format!("Failed to serialize permission requests: {error}"))
}

pub(crate) fn subscribe_permission_requests() -> Result<Value, String> {
    attached_controller_lease()?;
    Ok(Value::Null)
}

pub(crate) async fn respond_permission(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let lease = attached_controller_lease()?;
    let request = request_value(args);
    let request_id = get_string(request, "requestId")?;
    let pending = state
        .agent_runtime
        .pending_permission_requests()
        .map_err(|error| error.into_message())?
        .into_iter()
        .find(|pending| pending.request_id == request_id)
        .ok_or_else(|| format!("Permission request not found: {request_id}"))?;
    if !state.turns.owns_permission_request(&pending) {
        return Err("The permission request is not owned by the Peer controller".to_string());
    }
    if !crate::peer_host::control::is_controller_lease_current(lease) {
        return Err("Peer controller continuity was lost before permission response".to_string());
    }
    let reply = permission_reply(request)?;
    state
        .agent_runtime
        .respond_permission(&request_id, reply)
        .await
        .map_err(|error| error.into_message())?;
    Ok(Value::Null)
}

pub(crate) async fn respond_permission_batch(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let lease = attached_controller_lease()?;
    let request = request_value(args);
    let request_id = get_string(request, "requestId")?;
    let pending = state
        .agent_runtime
        .pending_permission_requests()
        .map_err(|error| error.into_message())?
        .into_iter()
        .find(|pending| pending.request_id == request_id)
        .ok_or_else(|| format!("Permission request not found: {request_id}"))?;
    if !state.turns.owns_permission_request(&pending) {
        return Err("The permission request is not owned by the Peer controller".to_string());
    }
    if !crate::peer_host::control::is_controller_lease_current(lease) {
        return Err("Peer controller continuity was lost before permission response".to_string());
    }
    let reply = permission_reply(request)?;
    let resolved_request_ids = state
        .agent_runtime
        .respond_permission_batch(&request_id, reply)
        .await
        .map_err(|error| error.into_message())?;
    serde_json::to_value(resolved_request_ids).map_err(|error| error.to_string())
}

pub(crate) async fn list_project_permission_grants(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let workspace_id = get_string(request, "workspaceId")?;
    let project_id = permission_project_id_for_workspace(state, &workspace_id).await?;
    let grants = state
        .agent_runtime
        .list_project_permission_grants(&project_id)
        .await
        .map_err(|error| error.into_message())?;
    serde_json::to_value(grants)
        .map_err(|error| format!("Failed to serialize permission grants: {error}"))
}

pub(crate) async fn remove_project_permission_grant(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    attached_controller_lease()?;
    let request = request_value(args);
    let workspace_id = get_string(request, "workspaceId")?;
    let project_id = permission_project_id_for_workspace(state, &workspace_id).await?;
    let removed = state
        .agent_runtime
        .remove_project_permission_grant(PermissionGrantKey {
            project_id,
            action: get_string(request, "action")?,
            resource: get_string(request, "resource")?,
        })
        .await
        .map_err(|error| error.into_message())?;
    Ok(json!(removed))
}

pub(crate) async fn clear_project_permission_grants(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    attached_controller_lease()?;
    let request = request_value(args);
    let workspace_id = get_string(request, "workspaceId")?;
    let project_id = permission_project_id_for_workspace(state, &workspace_id).await?;
    let removed = state
        .agent_runtime
        .clear_project_permission_grants(&project_id)
        .await
        .map_err(|error| error.into_message())?;
    Ok(json!(removed))
}

pub(crate) async fn list_project_permission_audit(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let workspace_id = get_string(request, "workspaceId")?;
    let project_id = permission_project_id_for_workspace(state, &workspace_id).await?;
    let mut records = state
        .agent_runtime
        .list_project_permission_audit(&project_id)
        .await
        .map_err(|error| error.into_message())?;
    records.sort_by(|left, right| {
        right
            .timestamp_ms
            .cmp(&left.timestamp_ms)
            .then_with(|| right.audit_id.cmp(&left.audit_id))
    });
    let page = pagination_value(request, "page", 0);
    let page_size = pagination_value(request, "pageSize", 50).clamp(1, 100);
    let total = records.len();
    let offset = page.saturating_mul(page_size).min(total);
    let records = records
        .into_iter()
        .skip(offset)
        .take(page_size)
        .collect::<Vec<_>>();
    Ok(json!({
        "projectId": project_id,
        "records": records,
        "page": page,
        "pageSize": page_size,
        "total": total,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_reply_rejects_unknown_values() {
        let error = permission_reply(&json!({ "reply": "later" })).unwrap_err();
        assert_eq!(error, "Unsupported permission reply: later");
    }

    #[test]
    fn permission_reply_preserves_rejection_feedback() {
        assert_eq!(
            permission_reply(&json!({ "reply": "reject", "feedback": "not now" })).unwrap(),
            PermissionReply::Reject {
                feedback: Some("not now".to_string()),
            }
        );
    }

    #[test]
    fn permission_audit_page_size_is_bounded() {
        assert_eq!(
            pagination_value(&json!({}), "pageSize", 50).clamp(1, 100),
            50
        );
        assert_eq!(
            pagination_value(&json!({ "pageSize": 0 }), "pageSize", 50).clamp(1, 100),
            1
        );
        assert_eq!(
            pagination_value(&json!({ "pageSize": 500 }), "pageSize", 50).clamp(1, 100),
            100
        );
    }
}
