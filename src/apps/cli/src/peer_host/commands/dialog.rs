//! Dialog / tool confirmation HostInvoke handlers.

use serde_json::{json, Value};

use bitfun_runtime_ports::{
    AgentDialogTurnRequest, AgentSubmissionSource, AgentTurnCancellationRequest,
    DialogSubmissionPolicy, DialogTriggerSource,
};

use crate::peer_host::args::{get_string, optional_string, request_value};
use crate::peer_host::control::{attached_controller_lease, is_controller_lease_current};
use crate::peer_host::state::{PeerHostState, PeerTurnKey};

fn peer_dialog_metadata(request: &Value) -> Result<serde_json::Map<String, Value>, String> {
    let mut metadata = match request.get("userMessageMetadata") {
        Some(Value::Object(metadata)) => metadata.clone(),
        Some(Value::Null) | None => serde_json::Map::new(),
        Some(_) => return Err("userMessageMetadata must be an object".to_string()),
    };
    for reserved_key in [
        "acp_transport",
        "backgroundTaskId",
        "parentSessionId",
        "parentDialogTurnId",
        "subagentSessionId",
        "subagentDialogTurnId",
    ] {
        metadata.remove(reserved_key);
    }
    metadata.insert("require_tool_confirmation".to_string(), Value::Bool(true));
    Ok(metadata)
}

pub(crate) async fn start_dialog_turn(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let user_input = get_string(request, "userInput")?;
    let original_user_input = optional_string(request, "originalUserInput");
    let agent_type = get_string(request, "agentType")?;
    let workspace_path = optional_string(request, "workspacePath");
    let remote_connection_id = optional_string(request, "remoteConnectionId");
    let remote_ssh_host = optional_string(request, "remoteSshHost");
    let controller_lease = attached_controller_lease()?;
    let turn_id =
        optional_string(request, "turnId").unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let metadata = peer_dialog_metadata(request)?;
    let turn = PeerTurnKey::new(session_id.clone(), turn_id.clone());
    let stream_generation = state.turns.register_root(turn.clone())?;
    if !state
        .turns
        .is_event_stream_generation_current(stream_generation)
        || !is_controller_lease_current(controller_lease)
    {
        state.turns.finish_turn(&turn);
        return Err(
            "Peer controller or event stream continuity was lost before dialog submission"
                .to_string(),
        );
    }

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi);
    let submit_result = state
        .agent_runtime
        .submit_dialog_turn(AgentDialogTurnRequest {
            session_id: session_id.clone(),
            message: user_input,
            original_message: original_user_input,
            turn_id: Some(turn_id.clone()),
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            policy,
            reply_route: None,
            prepended_reminders: Vec::new(),
            attachments: Vec::new(),
            metadata,
        })
        .await;
    if let Err(error) = submit_result {
        state.turns.finish_turn(&turn);
        return Err(format!("Failed to start dialog turn: {error}"));
    }
    if !state
        .turns
        .is_event_stream_generation_current(stream_generation)
        || !is_controller_lease_current(controller_lease)
    {
        let cancellation = state
            .agent_runtime
            .cancel_turn(AgentTurnCancellationRequest {
                session_id: session_id.clone(),
                turn_id: Some(turn_id.clone()),
                source: Some(AgentSubmissionSource::Cli),
                requester_session_id: None,
                reason: Some("Peer controller or event stream lost continuity".to_string()),
                wait_timeout_ms: Some(1_500),
            })
            .await;
        if let Err(error) = cancellation {
            return Err(format!(
                "Peer continuity was lost after dialog submission and cancellation could not be confirmed: session_id={session_id}, turn_id={turn_id}, error={error}"
            ));
        }
        return Err(
            "Peer controller or event stream lost continuity while starting the dialog turn"
                .to_string(),
        );
    }

    Ok(json!({ "success": true, "message": "Dialog turn started" }))
}

pub(crate) async fn cancel_dialog_turn(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let dialog_turn_id = get_string(request, "dialogTurnId")?;
    if !state.turns.owns(&session_id, Some(&dialog_turn_id)) {
        return Err("The dialog turn is not owned by the Peer controller".to_string());
    }
    state
        .agent_runtime
        .cancel_turn(AgentTurnCancellationRequest {
            session_id,
            turn_id: Some(dialog_turn_id),
            source: Some(AgentSubmissionSource::Cli),
            requester_session_id: None,
            reason: Some("Peer controller requested cancellation".to_string()),
            wait_timeout_ms: Some(1_500),
        })
        .await
        .map_err(|e| format!("Failed to cancel dialog turn: {e}"))?;
    Ok(json!({ "success": true }))
}

pub(crate) async fn confirm_tool_execution(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let tool_id = get_string(request, "toolId")?;
    let ownership = state
        .turns
        .claim_confirmation(&tool_id)
        .ok_or_else(|| "Tool confirmation is not owned by an active Peer turn".to_string())?;
    if optional_string(request, "sessionId")
        .is_some_and(|value| value.as_str() != ownership.session_id.as_str())
        || optional_string(request, "dialogTurnId")
            .is_some_and(|value| value.as_str() != ownership.turn_id.as_str())
    {
        state.turns.restore_confirmation(tool_id, ownership);
        return Err("Tool confirmation session or turn does not match its Peer owner".to_string());
    }
    let updated_input = request.get("updatedInput").cloned();
    if let Err(error) = state
        .compatibility
        .confirm_tool(&tool_id, updated_input)
        .await
    {
        state.turns.restore_confirmation(tool_id, ownership);
        return Err(format!("Confirm tool failed: {error}"));
    }
    Ok(Value::Null)
}

pub(crate) async fn reject_tool_execution(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let tool_id = get_string(request, "toolId")?;
    let ownership = state
        .turns
        .claim_confirmation(&tool_id)
        .ok_or_else(|| "Tool confirmation is not owned by an active Peer turn".to_string())?;
    if optional_string(request, "sessionId")
        .is_some_and(|value| value.as_str() != ownership.session_id.as_str())
        || optional_string(request, "dialogTurnId")
            .is_some_and(|value| value.as_str() != ownership.turn_id.as_str())
    {
        state.turns.restore_confirmation(tool_id, ownership);
        return Err("Tool confirmation session or turn does not match its Peer owner".to_string());
    }
    let reason = optional_string(request, "reason").unwrap_or_else(|| "User rejected".to_string());
    if let Err(error) = state.compatibility.reject_tool(&tool_id, reason).await {
        state.turns.restore_confirmation(tool_id, ownership);
        return Err(format!("Reject tool failed: {error}"));
    }
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::peer_dialog_metadata;

    #[test]
    fn peer_metadata_forces_confirmation_and_cannot_claim_acp_transport() {
        let metadata = peer_dialog_metadata(&json!({
            "userMessageMetadata": {
                "acp_transport": true,
                "kind": "background_result",
                "sourceKind": "subagent",
                "backgroundTaskId": "background-task",
                "parentSessionId": "parent-session",
                "parentDialogTurnId": "parent-turn",
                "subagentSessionId": "subagent-session",
                "subagentDialogTurnId": "subagent-turn",
                "require_tool_confirmation": false,
                "caller": "desktop",
            }
        }))
        .expect("metadata");

        assert_eq!(
            metadata.get("require_tool_confirmation"),
            Some(&json!(true))
        );
        assert!(!metadata.contains_key("acp_transport"));
        for reserved_key in [
            "backgroundTaskId",
            "parentSessionId",
            "parentDialogTurnId",
            "subagentSessionId",
            "subagentDialogTurnId",
        ] {
            assert!(!metadata.contains_key(reserved_key));
        }
        assert_eq!(metadata.get("kind"), Some(&json!("background_result")));
        assert_eq!(metadata.get("sourceKind"), Some(&json!("subagent")));
        assert_eq!(metadata.get("caller"), Some(&json!("desktop")));
    }

    #[test]
    fn peer_metadata_preserves_non_lineage_classification() {
        let metadata = peer_dialog_metadata(&json!({
            "userMessageMetadata": {
                "kind": "manual_compaction",
                "sourceKind": "user",
            }
        }))
        .expect("metadata");

        assert_eq!(metadata.get("kind"), Some(&json!("manual_compaction")));
        assert_eq!(metadata.get("sourceKind"), Some(&json!("user")));
    }
}
