//! Snapshot / rollback HostInvoke handlers for CLI Peer Host.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::peer_host::args::{get_string, get_usize, optional_bool, request_value};
use crate::peer_host::fanout::fanout_peer_device_event;
use crate::peer_host::state::PeerHostState;

use super::session::resolved_session_storage_path;

pub(crate) async fn get_session_files(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let workspace_path = get_string(request, "workspacePath")?;

    bitfun_agent_runtime::session_control::validate_session_id(&session_id)?;
    let files = state
        .compatibility
        .get_session_snapshot_files(&PathBuf::from(&workspace_path), &session_id)
        .await
        .map_err(|e| format!("Failed to get session files: {e}"))?;

    Ok(json!(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()))
}

pub(crate) async fn rollback_to_turn(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let workspace_path = get_string(request, "workspacePath")?;
    let turn_index = get_usize(request, "turnIndex")?;
    let delete_turns = optional_bool(request, "deleteTurns").unwrap_or(false);

    bitfun_agent_runtime::session_control::validate_session_id(&session_id)?;
    let workspace = PathBuf::from(&workspace_path);
    let session_storage_path = resolved_session_storage_path(state, request).await?;
    if delete_turns {
        state
            .compatibility
            .ensure_session_loaded_from_storage_path(&session_storage_path, &session_id, false)
            .await
            .map_err(|error| format!("Failed to load session before rollback: {error}"))?;
    }
    let maintenance = state
        .compatibility
        .begin_session_maintenance(&session_storage_path, &session_id, 2_000)
        .await
        .map_err(|error| format!("Failed to quiesce session before rollback: {error}"))?;
    let mut descendant_cancellation = state.turns.session_turns_for_cancellation(&session_id);
    descendant_cancellation
        .turns
        .retain(|turn| turn.session_id != session_id);
    state
        .cancel_peer_turns(descendant_cancellation, "Peer session rollback")
        .await
        .map_err(|error| format!("Failed to cancel Peer descendants before rollback: {error}"))?;
    state.turns.drain_session_turns(&session_id);

    let mutation = if delete_turns {
        Some(
            state
                .compatibility
                .begin_persisted_session_mutation(&session_storage_path, &session_id)
                .await
                .map_err(|error| format!("Failed to lock session rollback: {error}"))?,
        )
    } else {
        None
    };

    let rolled_back_parent_turn_ids = if delete_turns {
        let turns = state
            .compatibility
            .load_persisted_session_turns(&session_storage_path, &session_id, None)
            .await
            .map_err(|error| format!("Failed to load turns before rollback: {error}"))?;
        state
            .compatibility
            .validate_persisted_session_context_rollback(
                mutation
                    .as_ref()
                    .expect("mutation exists when deleting turns"),
                turn_index,
            )
            .await
            .map_err(|error| format!("Failed to validate session rollback: {error}"))?;
        turns
            .into_iter()
            .filter(|turn| turn.turn_index >= turn_index)
            .map(|turn| turn.turn_id)
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };

    let restored_files = state
        .compatibility
        .rollback_workspace_files_to_turn(&workspace, &session_id, turn_index)
        .await
        .map_err(|e| format!("Failed to rollback turn: {e}"))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let deleted_turns_count = rolled_back_parent_turn_ids.len();
    if delete_turns {
        if let Err(error) = state
            .compatibility
            .rollback_persisted_session_context_to_turn_start(
                mutation
                    .as_ref()
                    .expect("mutation exists when deleting turns"),
                turn_index,
            )
            .await
        {
            return Err(format!(
                "Workspace files were rolled back, but session history rollback failed. Reload the session before retrying: {error}"
            ));
        }

        if !rolled_back_parent_turn_ids.is_empty() {
            if let Err(error) = state
                .compatibility
                .delete_hidden_subagent_sessions_for_parent_turns(
                    &session_storage_path,
                    &session_id,
                    &rolled_back_parent_turn_ids,
                )
                .await
            {
                tracing::warn!(
                    "Failed to delete hidden subagent sessions during rollback: session_id={session_id}, turn_index={turn_index}, error={error}"
                );
            }
        }
    }

    drop(mutation);
    drop(maintenance);

    if delete_turns {
        fanout_peer_device_event(
            "conversation_turns_deleted".to_string(),
            json!({
                "session_id": session_id,
                "remaining_turns": turn_index,
                "deleted_count": deleted_turns_count,
            }),
        )
        .await;
    }

    fanout_peer_device_event(
        "turn_rolled_back".to_string(),
        json!({
            "session_id": session_id,
            "turn_index": turn_index,
            "files_count": restored_files_str.len(),
            "deleted_turns": delete_turns,
            "deleted_turns_count": deleted_turns_count,
        }),
    )
    .await;

    Ok(json!(restored_files_str))
}
