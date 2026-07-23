//! Snapshot / rollback HostInvoke handlers for CLI Peer Host.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::{json, Value};

use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use bitfun_runtime_ports::{
    LocalWorkspaceSnapshotPort, LocalWorkspaceSnapshotSessionRequest, LocalWorkspaceSnapshotStats,
    LocalWorkspaceSnapshotTurnRequest, PortError, PortErrorKind,
};

use crate::peer_host::args::{
    get_string, get_usize, optional_bool, optional_string, request_value,
};
use crate::peer_host::fanout::fanout_peer_device_event;
use crate::peer_host::state::PeerHostState;

use super::session::resolved_session_storage_path;

pub(super) async fn require_local_snapshot_workspace(
    request: &Value,
    workspace_path: &str,
) -> Result<(), String> {
    let is_remote = optional_string(request, "remoteConnectionId").is_some()
        || optional_string(request, "remoteSshHost").is_some()
        || is_remote_path(workspace_path).await;
    if is_remote {
        return Err(format!(
            "Snapshot system not supported for remote workspace: {workspace_path}"
        ));
    }
    Ok(())
}

pub(super) fn snapshot_compatibility_error(error: PortError) -> String {
    if error.kind == PortErrorKind::InvalidRequest {
        error.message
    } else {
        format!("Service error: {}", error.message)
    }
}

pub(super) async fn local_snapshot_session_files(
    port: &dyn LocalWorkspaceSnapshotPort,
    workspace_path: PathBuf,
    session_id: String,
) -> Result<Vec<PathBuf>, String> {
    port.get_session_files(LocalWorkspaceSnapshotSessionRequest {
        workspace_path,
        session_id,
    })
    .await
    .map_err(|error| {
        format!(
            "Failed to get session files: {}",
            snapshot_compatibility_error(error)
        )
    })
}

pub(super) async fn local_snapshot_session_stats(
    port: &dyn LocalWorkspaceSnapshotPort,
    workspace_path: PathBuf,
    session_id: String,
) -> Result<LocalWorkspaceSnapshotStats, String> {
    port.get_session_stats(LocalWorkspaceSnapshotSessionRequest {
        workspace_path,
        session_id,
    })
    .await
    .map_err(|error| {
        format!(
            "Failed to get session stats: {}",
            snapshot_compatibility_error(error)
        )
    })
}

async fn rollback_local_workspace_files(
    port: &dyn LocalWorkspaceSnapshotPort,
    workspace_path: PathBuf,
    session_id: String,
    turn_index: usize,
) -> Result<Vec<PathBuf>, String> {
    port.rollback_workspace_files_to_turn(LocalWorkspaceSnapshotTurnRequest {
        workspace_path,
        session_id,
        turn_index,
    })
    .await
    .map_err(|error| {
        format!(
            "Failed to rollback turn: {}",
            snapshot_compatibility_error(error)
        )
    })
}

fn history_rollback_partial_failure(error: impl std::fmt::Display) -> String {
    format!(
        "Workspace files were rolled back, but session history rollback failed. Reload the session before retrying: {error}"
    )
}

fn rollback_device_events(
    session_id: &str,
    turn_index: usize,
    files_count: usize,
    delete_turns: bool,
    deleted_turns_count: usize,
) -> Vec<(String, Value)> {
    let mut events = Vec::with_capacity(if delete_turns { 2 } else { 1 });
    if delete_turns {
        events.push((
            "conversation_turns_deleted".to_string(),
            json!({
                "session_id": session_id,
                "remaining_turns": turn_index,
                "deleted_count": deleted_turns_count,
            }),
        ));
    }
    events.push((
        "turn_rolled_back".to_string(),
        json!({
            "session_id": session_id,
            "turn_index": turn_index,
            "files_count": files_count,
            "deleted_turns": delete_turns,
            "deleted_turns_count": deleted_turns_count,
        }),
    ));
    events
}

pub(crate) async fn get_session_files(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let workspace_path = get_string(request, "workspacePath")?;

    bitfun_agent_runtime::session_control::validate_session_id(&session_id)?;
    require_local_snapshot_workspace(request, &workspace_path).await?;
    let files = local_snapshot_session_files(
        state.local_workspace_snapshot.as_ref(),
        PathBuf::from(&workspace_path),
        session_id,
    )
    .await?;

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
    require_local_snapshot_workspace(request, &workspace_path).await?;
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

    let restored_files = rollback_local_workspace_files(
        state.local_workspace_snapshot.as_ref(),
        workspace,
        session_id.clone(),
        turn_index,
    )
    .await?;

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
            return Err(history_rollback_partial_failure(error));
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

    for (event_name, payload) in rollback_device_events(
        &session_id,
        turn_index,
        restored_files_str.len(),
        delete_turns,
        deleted_turns_count,
    ) {
        fanout_peer_device_event(event_name, payload).await;
    }

    Ok(json!(restored_files_str))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use bitfun_runtime_ports::{
        LocalWorkspaceSnapshotPort, LocalWorkspaceSnapshotSessionRequest,
        LocalWorkspaceSnapshotStats, LocalWorkspaceSnapshotTurnRequest, PortError, PortErrorKind,
        PortResult,
    };
    use serde_json::json;

    use super::{
        history_rollback_partial_failure, local_snapshot_session_files,
        local_snapshot_session_stats, require_local_snapshot_workspace, rollback_device_events,
        rollback_local_workspace_files, snapshot_compatibility_error,
    };

    #[derive(Default)]
    struct RecordingSnapshotPort {
        file_calls: AtomicUsize,
        stats_calls: AtomicUsize,
        rollback_calls: AtomicUsize,
        file_request: Mutex<Option<LocalWorkspaceSnapshotSessionRequest>>,
        stats_request: Mutex<Option<LocalWorkspaceSnapshotSessionRequest>>,
        rollback_request: Mutex<Option<LocalWorkspaceSnapshotTurnRequest>>,
    }

    #[async_trait::async_trait]
    impl LocalWorkspaceSnapshotPort for RecordingSnapshotPort {
        async fn prepare_local_workspace(&self, _workspace_path: PathBuf) -> PortResult<()> {
            Ok(())
        }

        async fn get_session_files(
            &self,
            request: LocalWorkspaceSnapshotSessionRequest,
        ) -> PortResult<Vec<PathBuf>> {
            self.file_calls.fetch_add(1, Ordering::SeqCst);
            *self.file_request.lock().expect("file request lock") = Some(request);
            Ok(vec![PathBuf::from("changed.txt")])
        }

        async fn get_session_stats(
            &self,
            request: LocalWorkspaceSnapshotSessionRequest,
        ) -> PortResult<LocalWorkspaceSnapshotStats> {
            self.stats_calls.fetch_add(1, Ordering::SeqCst);
            let session_id = request.session_id.clone();
            *self.stats_request.lock().expect("stats request lock") = Some(request);
            Ok(LocalWorkspaceSnapshotStats {
                session_id,
                total_files: 1,
                total_turns: 2,
                total_changes: 3,
            })
        }

        async fn rollback_workspace_files_to_turn(
            &self,
            request: LocalWorkspaceSnapshotTurnRequest,
        ) -> PortResult<Vec<PathBuf>> {
            self.rollback_calls.fetch_add(1, Ordering::SeqCst);
            *self.rollback_request.lock().expect("rollback request lock") = Some(request);
            Ok(vec![PathBuf::from("restored.txt")])
        }
    }

    #[tokio::test]
    async fn explicit_remote_snapshot_identity_returns_an_honest_unsupported_error() {
        for request in [
            json!({ "remoteConnectionId": "remote-1" }),
            json!({ "remoteSshHost": "host-1" }),
        ] {
            let error = require_local_snapshot_workspace(&request, "local-looking-path")
                .await
                .expect_err("remote snapshot requests must not report no-op success");
            assert_eq!(
                error,
                "Snapshot system not supported for remote workspace: local-looking-path"
            );
        }

        let source = include_str!("snapshot.rs");
        let rollback_source = &source[source
            .find("pub(crate) async fn rollback_to_turn")
            .expect("rollback handler must exist")..];
        let remote_guard = rollback_source
            .find("require_local_snapshot_workspace(request, &workspace_path).await?")
            .expect("rollback must have an explicit remote guard");
        let maintenance = rollback_source
            .find("begin_session_maintenance")
            .expect("host-owned maintenance must remain in the rollback flow");
        let cancellation = rollback_source
            .find("cancel_peer_turns")
            .expect("descendant cancellation must remain in the rollback flow");
        let file_rollback = rollback_source
            .find("rollback_local_workspace_files(")
            .expect("workspace-file rollback must remain in the rollback flow");
        let history_rollback = rollback_source
            .find("rollback_persisted_session_context_to_turn_start")
            .expect("history rollback must remain in the rollback flow");
        let event_projection = rollback_source
            .find("rollback_device_events(")
            .expect("rollback events must remain host-projected");
        assert!(
            remote_guard < maintenance
                && maintenance < cancellation
                && cancellation < file_rollback
                && file_rollback < history_rollback
                && history_rollback < event_projection,
            "rollback must preserve remote guard, maintenance, cancellation, files, history, and event order"
        );
    }

    #[tokio::test]
    async fn local_snapshot_adapter_calls_each_port_operation_once_with_typed_requests() {
        let port = RecordingSnapshotPort::default();
        let workspace = PathBuf::from("workspace");

        let files = local_snapshot_session_files(&port, workspace.clone(), "session-1".to_string())
            .await
            .expect("file projection should succeed");
        let stats = local_snapshot_session_stats(&port, workspace.clone(), "session-1".to_string())
            .await
            .expect("stats projection should succeed");
        let restored =
            rollback_local_workspace_files(&port, workspace.clone(), "session-1".to_string(), 4)
                .await
                .expect("rollback projection should succeed");

        assert_eq!(port.file_calls.load(Ordering::SeqCst), 1);
        assert_eq!(port.stats_calls.load(Ordering::SeqCst), 1);
        assert_eq!(port.rollback_calls.load(Ordering::SeqCst), 1);
        assert_eq!(files, vec![PathBuf::from("changed.txt")]);
        assert_eq!(stats.total_changes, 3);
        assert_eq!(restored, vec![PathBuf::from("restored.txt")]);
        assert_eq!(
            port.file_request
                .lock()
                .expect("file request lock")
                .as_ref()
                .expect("file request")
                .workspace_path,
            workspace
        );
        assert_eq!(
            port.rollback_request
                .lock()
                .expect("rollback request lock")
                .as_ref()
                .expect("rollback request")
                .turn_index,
            4
        );
    }

    #[test]
    fn rollback_projection_preserves_partial_failure_and_event_order() {
        assert_eq!(
            history_rollback_partial_failure("history backend failed"),
            "Workspace files were rolled back, but session history rollback failed. Reload the session before retrying: history backend failed"
        );

        let events = rollback_device_events("session-1", 4, 2, true, 3);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "conversation_turns_deleted");
        assert_eq!(events[0].1["remaining_turns"], 4);
        assert_eq!(events[0].1["deleted_count"], 3);
        assert_eq!(events[1].0, "turn_rolled_back");
        assert_eq!(events[1].1["files_count"], 2);
        assert_eq!(events[1].1["deleted_turns"], true);

        let file_only = rollback_device_events("session-1", 4, 2, false, 0);
        assert_eq!(file_only.len(), 1);
        assert_eq!(file_only[0].0, "turn_rolled_back");
    }

    #[test]
    fn port_errors_keep_the_existing_peer_host_error_categories() {
        let invalid = snapshot_compatibility_error(PortError::new(
            PortErrorKind::InvalidRequest,
            "Validation error: invalid session_id",
        ));
        assert_eq!(invalid, "Validation error: invalid session_id");

        let backend = snapshot_compatibility_error(PortError::new(
            PortErrorKind::Backend,
            "snapshot backend failed",
        ));
        assert_eq!(backend, "Service error: snapshot backend failed");
    }
}
