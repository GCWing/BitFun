//! Snapshot / rollback HostInvoke handlers for CLI Peer Host.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use bitfun_core::service::snapshot::{
    get_snapshot_manager_for_workspace, initialize_snapshot_manager_for_workspace, SnapshotManager,
};

use crate::peer_host::args::{get_string, get_usize, optional_bool, request_value};
use crate::peer_host::fanout::fanout_peer_device_event;
use crate::peer_host::state::PeerHostState;

async fn ensure_snapshot_manager(workspace_path: &str) -> Result<Arc<SnapshotManager>, String> {
    let workspace_dir = PathBuf::from(workspace_path);
    if let Some(manager) = get_snapshot_manager_for_workspace(&workspace_dir) {
        return Ok(manager);
    }

    initialize_snapshot_manager_for_workspace(workspace_dir.clone(), None)
        .await
        .map_err(|e| {
            format!(
                "Failed to initialize snapshot system for workspace {}: {e}",
                workspace_dir.display()
            )
        })?;

    get_snapshot_manager_for_workspace(&workspace_dir).ok_or_else(|| {
        format!(
            "Failed to get snapshot manager for workspace {}",
            workspace_dir.display()
        )
    })
}

pub(crate) async fn get_session_files(args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let workspace_path = get_string(request, "workspacePath")?;

    let manager = ensure_snapshot_manager(&workspace_path).await?;
    let files = manager
        .get_session_files(&session_id)
        .await
        .map_err(|e| format!("Failed to get session files: {e}"))?;

    Ok(json!(
        files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
    ))
}

pub(crate) async fn rollback_to_turn(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let workspace_path = get_string(request, "workspacePath")?;
    let turn_index = get_usize(request, "turnIndex")?;
    let delete_turns = optional_bool(request, "deleteTurns").unwrap_or(false);

    if let Err(e) = state
        .coordinator
        .cancel_active_turn_for_session(&session_id, Duration::from_secs(2))
        .await
    {
        tracing::warn!(
            "Failed to cancel active turn before rollback: session_id={session_id}, turn_index={turn_index}, error={e}"
        );
    }

    let manager = ensure_snapshot_manager(&workspace_path).await?;
    let restored_files = manager
        .rollback_to_turn(&session_id, turn_index)
        .await
        .map_err(|e| format!("Failed to rollback turn: {e}"))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let mut deleted_turns_count = 0usize;
    if delete_turns {
        let workspace = PathBuf::from(&workspace_path);
        let mut rolled_back_parent_turn_ids = HashSet::new();

        match state
            .persistence
            .load_session_turns(&workspace, &session_id)
            .await
        {
            Ok(turns) => {
                rolled_back_parent_turn_ids = turns
                    .into_iter()
                    .filter(|turn| turn.turn_index >= turn_index)
                    .map(|turn| turn.turn_id)
                    .collect();
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load parent turns before rollback cleanup: session_id={session_id}, turn_index={turn_index}, error={e}"
                );
            }
        }

        if !rolled_back_parent_turn_ids.is_empty() {
            if let Err(e) = state
                .coordinator
                .delete_hidden_subagent_sessions_for_parent_turns(
                    &workspace,
                    &session_id,
                    &rolled_back_parent_turn_ids,
                )
                .await
            {
                tracing::warn!(
                    "Failed to delete hidden subagent sessions during rollback: session_id={session_id}, turn_index={turn_index}, error={e}"
                );
            }
        }

        if let Err(e) = state
            .coordinator
            .get_session_manager()
            .rollback_context_to_turn_start(&workspace, &session_id, turn_index)
            .await
        {
            tracing::warn!(
                "Rollback agentic context failed: session_id={session_id}, turn_index={turn_index}, error={e}"
            );
        }

        match state
            .persistence
            .delete_turns_from(&workspace, &session_id, turn_index)
            .await
        {
            Ok(count) => deleted_turns_count = count,
            Err(e) => {
                tracing::warn!(
                    "Failed to delete conversation turns: session_id={session_id}, turn_index={turn_index}, error={e}"
                );
            }
        }

        fanout_peer_device_event(
            "conversation_turns_deleted".to_string(),
            json!({
                "session_id": session_id,
                "remaining_turns": turn_index,
                "deleted_count": deleted_turns_count,
            }),
        );
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
    );

    Ok(json!(restored_files_str))
}
