//! Dialog / tool confirmation HostInvoke handlers.

use serde_json::{json, Value};

use bitfun_core::agentic::coordination::{DialogSubmissionPolicy, DialogTriggerSource};

use crate::peer_host::args::{get_string, optional_string, request_value};
use crate::peer_host::state::PeerHostState;

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
    let turn_id = optional_string(request, "turnId");
    let user_message_metadata = request.get("userMessageMetadata").cloned();

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi);
    state
        .scheduler
        .submit(
            session_id,
            user_input,
            original_user_input,
            turn_id,
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            policy,
            None,
            user_message_metadata,
            None,
        )
        .await
        .map_err(|e| format!("Failed to start dialog turn: {e}"))?;

    Ok(json!({ "success": true, "message": "Dialog turn started" }))
}

pub(crate) async fn cancel_dialog_turn(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let dialog_turn_id = get_string(request, "dialogTurnId")?;
    state
        .coordinator
        .cancel_dialog_turn(&session_id, &dialog_turn_id)
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
    let updated_input = request.get("updatedInput").cloned();
    state
        .coordinator
        .confirm_tool(&tool_id, updated_input)
        .await
        .map_err(|e| format!("Confirm tool failed: {e}"))?;
    Ok(Value::Null)
}

pub(crate) async fn reject_tool_execution(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let tool_id = get_string(request, "toolId")?;
    let reason = optional_string(request, "reason").unwrap_or_else(|| "User rejected".to_string());
    state
        .coordinator
        .reject_tool(&tool_id, reason)
        .await
        .map_err(|e| format!("Reject tool failed: {e}"))?;
    Ok(Value::Null)
}
