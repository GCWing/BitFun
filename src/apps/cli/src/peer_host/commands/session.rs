//! Session HostInvoke handlers for CLI Peer Host.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use bitfun_agent_runtime::sdk::{AgentSessionRestoreRequest, AgentSessionRestoreResult};
use bitfun_core::agentic::core::Session;
use bitfun_core::agentic::get_agent_registry;
use bitfun_runtime_ports::{
    AgentSessionArchiveRequest, AgentSessionCreateRequest, AgentSessionDeleteRequest,
    AgentSessionModelUpdateRequest, AgentSessionRenameRequest, AgentThreadGoalGetRequest,
    SessionStoragePathRequest,
};

use crate::peer_host::args::{get_string, optional_bool, optional_string, request_value};
use crate::peer_host::state::PeerHostState;

use super::snapshot::{local_snapshot_session_stats, require_local_snapshot_workspace};

fn session_storage_request(request: &Value) -> Result<SessionStoragePathRequest, String> {
    let workspace_path = get_string(request, "workspacePath")?;
    let workspace_path = workspace_path.trim();
    if workspace_path.is_empty() {
        return Err("workspace_path is required".to_string());
    }
    Ok(SessionStoragePathRequest {
        workspace_path: PathBuf::from(workspace_path),
        remote_connection_id: optional_string(request, "remoteConnectionId"),
        remote_ssh_host: optional_string(request, "remoteSshHost"),
    })
}

pub(super) async fn resolved_session_storage_path(
    state: &PeerHostState,
    request: &Value,
) -> Result<PathBuf, String> {
    state
        .compatibility
        .resolve_persisted_session_storage_path(session_storage_request(request)?)
        .await
        .map_err(|error| format!("Failed to resolve session storage path: {error}"))
}

fn validated_session_id(request: &Value) -> Result<String, String> {
    let session_id = get_string(request, "sessionId")?;
    bitfun_agent_runtime::session_control::validate_session_id(&session_id)?;
    Ok(session_id)
}

fn system_time_to_unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn session_to_json(session: Session, turn_count: usize) -> Value {
    json!({
        "sessionId": session.session_id,
        "sessionName": session.session_name,
        "agentType": session.agent_type,
        "modelName": session.config.model_id,
        "lastUserDialogAgentType": session.last_user_dialog_agent_type,
        "lastSubmittedAgentType": session.last_submitted_agent_type,
        "state": format!("{:?}", session.state),
        "turnCount": turn_count,
        "createdAt": system_time_to_unix_secs(session.created_at),
    })
}

fn overlay_live_session_state(restored: &mut Session, live: Option<Session>) {
    let Some(live) = live else {
        return;
    };
    if live.session_id == restored.session_id {
        restored.state = live.state;
    }
}

fn restored_session_to_json(restored: AgentSessionRestoreResult) -> Value {
    let session = restored.session;
    json!({
        "sessionId": session.session_id,
        "sessionName": session.session_name,
        "agentType": session.agent_type,
        "modelName": session.model_id,
        "lastUserDialogAgentType": session.last_user_dialog_agent_type,
        "lastSubmittedAgentType": session.last_submitted_agent_type,
        "state": format!("{:?}", restored.state),
        "turnCount": session.turn_count,
        "createdAt": session.created_at_ms / 1000,
    })
}

pub(crate) async fn list_persisted_sessions(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let workspace_path = resolved_session_storage_path(state, request).await?;
    let list = state
        .compatibility
        .list_persisted_sessions(&workspace_path)
        .await
        .map_err(|e| format!("Failed to list persisted sessions: {e}"))?;
    serde_json::to_value(list).map_err(|e| format!("serialize sessions: {e}"))
}

pub(crate) async fn list_persisted_sessions_page(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let workspace_path = resolved_session_storage_path(state, request).await?;
    let limit = request.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let cursor = optional_string(request, "cursor");
    let page = state
        .compatibility
        .list_persisted_sessions_page(&workspace_path, cursor.as_deref(), limit)
        .await
        .map_err(|e| format!("Failed to list persisted session page: {e}"))?;
    serde_json::to_value(page).map_err(|e| format!("serialize session page: {e}"))
}

pub(crate) async fn list_persisted_sessions_count(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let workspace_path = resolved_session_storage_path(state, request).await?;
    let list = state
        .compatibility
        .list_persisted_sessions(&workspace_path)
        .await
        .map_err(|e| format!("Failed to count persisted sessions: {e}"))?;
    Ok(json!(list.len()))
}

pub(crate) async fn load_session_turns(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let workspace_path = resolved_session_storage_path(state, request).await?;
    let limit = request
        .get("limit")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize);
    let turns = state
        .compatibility
        .load_persisted_session_turns(&workspace_path, &session_id, limit)
        .await
        .map_err(|e| format!("Failed to load session turns: {e}"))?;
    serde_json::to_value(turns).map_err(|e| format!("serialize turns: {e}"))
}

pub(crate) async fn restore_session_view(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let storage_request = session_storage_request(request)?;
    let include_internal = request
        .get("includeInternal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let tail_turn_count = request
        .get("tailTurnCount")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .filter(|n| *n > 0)
        .map(|n| n.min(16));

    let (mut session, turns, total_turn_count, timings) = state
        .compatibility
        .restore_session_view_for_workspace(
            storage_request,
            &session_id,
            include_internal,
            tail_turn_count,
        )
        .await
        .map_err(|e| format!("Failed to restore session view: {e}"))?;
    let live_session = state
        .compatibility
        .loaded_session_snapshot(&session_id)
        .map_err(|e| format!("Failed to read live session state: {e}"))?;
    overlay_live_session_state(&mut session, live_session);

    let loaded_turn_count = turns.len();
    let is_partial = loaded_turn_count < total_turn_count;
    Ok(json!({
        "session": session_to_json(session, total_turn_count),
        "turns": turns,
        "contextRestoreState": "pending",
        "isPartial": is_partial,
        "loadedTurnCount": loaded_turn_count,
        "totalTurnCount": total_turn_count,
        "timings": timings,
    }))
}

pub(crate) async fn restore_session_with_turns(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let storage_request = session_storage_request(request)?;
    let include_internal = request
        .get("includeInternal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let (session, turns) = state
        .compatibility
        .restore_session_with_turns_for_workspace(storage_request, &session_id, include_internal)
        .await
        .map_err(|e| format!("Failed to restore session with turns: {e}"))?;

    let turn_count = turns.len();
    Ok(json!({
        "session": session_to_json(session, turn_count),
        "turns": turns,
    }))
}

pub(crate) async fn restore_session(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let storage_request = session_storage_request(request)?;
    let include_internal = request
        .get("includeInternal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let restored = state
        .agent_runtime
        .restore_session(AgentSessionRestoreRequest {
            workspace_path: storage_request.workspace_path.to_string_lossy().to_string(),
            session_id,
            include_internal,
            remote_connection_id: storage_request.remote_connection_id,
            remote_ssh_host: storage_request.remote_ssh_host,
        })
        .await
        .map_err(|error| format!("Failed to restore session: {}", error.into_message()))?;

    Ok(restored_session_to_json(restored))
}

pub(crate) async fn create_session(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let session_name = get_string(request, "sessionName")?;
    let agent_type = get_string(request, "agentType")?;
    let workspace_path = get_string(request, "workspacePath")?;
    let session_id = optional_string(request, "sessionId");
    let workspace_id = optional_string(request, "workspaceId");
    let remote_connection_id = optional_string(request, "remoteConnectionId");
    let remote_ssh_host = optional_string(request, "remoteSshHost");

    let model_id = request
        .get("config")
        .and_then(|c| {
            c.get("modelName")
                .or_else(|| c.get("model_name"))
                .or_else(|| c.get("modelId"))
                .or_else(|| c.get("model_id"))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| optional_string(request, "modelName"));

    let create_request = AgentSessionCreateRequest {
        session_name,
        agent_type,
        workspace_path: Some(workspace_path),
        workspace_id,
        remote_connection_id,
        remote_ssh_host,
        model_id,
        metadata: serde_json::Map::new(),
    };
    let session = match session_id {
        Some(session_id) => {
            state
                .agent_runtime
                .create_session_with_id(session_id, create_request)
                .await
        }
        None => state.agent_runtime.create_session(create_request).await,
    }
    .map_err(|error| format!("Failed to create session: {}", error.into_message()))?;

    Ok(json!({
        "sessionId": session.session_id,
        "sessionName": session.session_name,
        "agentType": session.agent_type,
    }))
}

pub(crate) async fn delete_session(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let workspace_path = get_string(request, "workspacePath")?;
    state
        .agent_runtime
        .delete_session(AgentSessionDeleteRequest {
            workspace_path,
            session_id,
            remote_connection_id: optional_string(request, "remoteConnectionId"),
            remote_ssh_host: optional_string(request, "remoteSshHost"),
        })
        .await
        .map_err(|error| format!("Failed to delete session: {}", error.into_message()))?;
    Ok(Value::Null)
}

pub(crate) async fn rename_session(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let storage_request = session_storage_request(request)?;
    let title = get_string(request, "sessionName")
        .or_else(|_| get_string(request, "title"))
        .or_else(|_| get_string(request, "name"))?;
    state
        .agent_runtime
        .rename_session(AgentSessionRenameRequest {
            workspace_path: storage_request.workspace_path.to_string_lossy().to_string(),
            session_id,
            session_name: title,
            remote_connection_id: storage_request.remote_connection_id,
            remote_ssh_host: storage_request.remote_ssh_host,
        })
        .await
        .map_err(|error| format!("Failed to rename session: {}", error.into_message()))?;
    Ok(Value::Null)
}

pub(crate) async fn archive_session(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let storage_request = session_storage_request(request)?;
    state
        .agent_runtime
        .archive_session(AgentSessionArchiveRequest {
            workspace_path: storage_request.workspace_path.to_string_lossy().to_string(),
            session_id,
            remote_connection_id: storage_request.remote_connection_id,
            remote_ssh_host: storage_request.remote_ssh_host,
        })
        .await
        .map_err(|error| format!("Failed to archive session: {}", error.into_message()))?;
    Ok(Value::Null)
}

pub(crate) async fn touch_session_activity(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let workspace_path = resolved_session_storage_path(state, request).await?;
    let _mutation = state
        .compatibility
        .begin_persisted_session_mutation(&workspace_path, &session_id)
        .await
        .map_err(|error| format!("Failed to lock session activity update: {error}"))?;
    state
        .compatibility
        .touch_persisted_session(&workspace_path, &session_id)
        .await
        .map_err(|e| format!("Failed to update session activity: {e}"))?;
    Ok(Value::Null)
}

pub(crate) async fn get_session_thread_goal(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let storage_request = if optional_string(request, "workspacePath").is_some() {
        session_storage_request(request)?
    } else {
        SessionStoragePathRequest {
            workspace_path: PathBuf::from("."),
            remote_connection_id: None,
            remote_ssh_host: None,
        }
    };
    let goal = state
        .agent_runtime
        .get_thread_goal(AgentThreadGoalGetRequest {
            session_id,
            workspace_path: storage_request
                .workspace_path
                .to_string_lossy()
                .into_owned(),
            remote_connection_id: storage_request.remote_connection_id,
            remote_ssh_host: storage_request.remote_ssh_host,
        })
        .await
        .map_err(|error| error.into_message())?;
    Ok(json!({ "goal": goal }))
}

pub(crate) async fn update_session_model(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    let model_name = get_string(request, "modelName")?;
    state
        .agent_runtime
        .update_session_model(AgentSessionModelUpdateRequest {
            session_id,
            model_id: model_name,
        })
        .await
        .map_err(|error| format!("Failed to update session model: {}", error.into_message()))?;
    Ok(Value::Null)
}

pub(crate) async fn ensure_coordinator_session(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = validated_session_id(request)?;
    if state
        .compatibility
        .is_session_loaded_in_memory(&session_id)
        .map_err(|error| error.to_string())?
    {
        return Ok(Value::Null);
    }
    let storage = resolved_session_storage_path(state, request).await?;
    let include_internal = optional_bool(request, "includeInternal").unwrap_or(false);

    state
        .compatibility
        .ensure_session_loaded_from_storage_path(&storage, &session_id, include_internal)
        .await
        .map(|_| Value::Null)
        .map_err(|e| e.to_string())
}

pub(crate) async fn get_available_modes() -> Result<Value, String> {
    let mode_infos = get_agent_registry().get_modes_info().await;
    let dtos: Vec<Value> = mode_infos
        .into_iter()
        .map(|info| {
            let config_profile_id = info
                .config_profile_id
                .clone()
                .unwrap_or_else(|| info.id.clone());
            json!({
                "id": info.id,
                "name": info.name,
                "description": info.description,
                "isReadonly": info.is_readonly,
                "toolCount": info.tool_count,
                "defaultTools": info.default_tools,
                "promptCacheScopeKey": info.prompt_cache_scope_key,
                "configProfileId": config_profile_id,
                "configProfileLabel": info.config_profile_label,
                "configProfileMemberModeIds": info.config_profile_member_mode_ids,
                "source": info.source,
                "path": info.path,
                "model": info.model,
            })
        })
        .collect();
    Ok(Value::Array(dtos))
}

pub(crate) async fn get_session_stats(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let workspace_path = get_string(request, "workspacePath")?;
    bitfun_agent_runtime::session_control::validate_session_id(&session_id)
        .map_err(session_stats_validation_error)?;
    require_local_snapshot_workspace(request, &workspace_path).await?;

    let stats = local_snapshot_session_stats(
        state.local_workspace_snapshot.as_ref(),
        PathBuf::from(&workspace_path),
        session_id,
    )
    .await?;

    Ok(json!({
        "session_id": stats.session_id,
        "total_files": stats.total_files,
        "total_turns": stats.total_turns,
        "total_changes": stats.total_changes
    }))
}

fn session_stats_validation_error(error: impl std::fmt::Display) -> String {
    format!("Failed to get session stats: Validation error: {error}")
}

pub(crate) async fn save_session_turn(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let workspace_path = resolved_session_storage_path(state, request).await?;
    let turn_data = request
        .get("turnData")
        .or_else(|| request.get("turn_data"))
        .cloned()
        .ok_or_else(|| "Missing 'turn_data' field".to_string())?;

    let turn: bitfun_core::service::session::DialogTurnData =
        serde_json::from_value(turn_data).map_err(|e| format!("Invalid turn_data: {e}"))?;
    bitfun_agent_runtime::session_control::validate_session_id(&turn.session_id)?;
    if let Some(request_session_id) = optional_string(request, "sessionId") {
        bitfun_agent_runtime::session_control::validate_session_id(&request_session_id)?;
        if request_session_id != turn.session_id {
            return Err("turn_data session_id does not match request session_id".to_string());
        }
    }
    let _mutation = state
        .compatibility
        .begin_persisted_session_mutation(&workspace_path, &turn.session_id)
        .await
        .map_err(|error| format!("Failed to lock session turn save: {error}"))?;

    state
        .compatibility
        .save_persisted_dialog_turn(&workspace_path, &turn)
        .await
        .map_err(|e| format!("Failed to save session turn: {e}"))?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::{
        overlay_live_session_state, restored_session_to_json, session_stats_validation_error,
    };
    use bitfun_agent_runtime::sdk::{AgentSessionRestoreResult, AgentSessionSummary, SessionState};
    use bitfun_core::agentic::core::{
        ProcessingPhase, Session as CoreSession, SessionConfig, SessionState as CoreSessionState,
    };

    #[test]
    fn basic_restore_keeps_peer_host_session_shape() {
        let value = restored_session_to_json(AgentSessionRestoreResult {
            session: AgentSessionSummary {
                session_id: "session_1".to_string(),
                session_name: "Main".to_string(),
                agent_type: "agentic".to_string(),
                model_id: Some("provider/model".to_string()),
                last_user_dialog_agent_type: Some("plan".to_string()),
                last_submitted_agent_type: Some("agentic".to_string()),
                turn_count: 3,
                created_at_ms: 12_345,
                last_active_at_ms: 20_000,
            },
            state: SessionState::Idle,
        });

        assert_eq!(value["sessionId"], "session_1");
        assert_eq!(value["sessionName"], "Main");
        assert_eq!(value["agentType"], "agentic");
        assert_eq!(value["modelName"], "provider/model");
        assert_eq!(value["lastUserDialogAgentType"], "plan");
        assert_eq!(value["lastSubmittedAgentType"], "agentic");
        assert_eq!(value["state"], "Idle");
        assert_eq!(value["turnCount"], 3);
        assert_eq!(value["createdAt"], 12);
        assert!(value.get("lastActiveAt").is_none());
    }

    #[test]
    fn session_stats_validation_keeps_compatibility_error_category() {
        assert_eq!(
            session_stats_validation_error("session_id cannot contain path separators"),
            "Failed to get session stats: Validation error: session_id cannot contain path separators"
        );
    }

    #[test]
    fn view_restore_uses_the_cli_hosts_live_processing_state() {
        let mut restored = CoreSession::new_with_id(
            "session_1".to_string(),
            "Main".to_string(),
            "agentic".to_string(),
            SessionConfig::default(),
        );
        let mut live = restored.clone();
        live.state = CoreSessionState::Processing {
            current_turn_id: "turn_1".to_string(),
            phase: ProcessingPhase::Streaming,
        };

        overlay_live_session_state(&mut restored, Some(live));

        assert!(matches!(
            restored.state,
            CoreSessionState::Processing {
                ref current_turn_id,
                ..
            } if current_turn_id == "turn_1"
        ));
    }
}
