//! Snapshot / rollback HostInvoke handlers for CLI Peer Host.

use std::path::PathBuf;

use serde_json::{json, Value};

use openbitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use openbitfun_runtime_ports::{
    AgentSessionRollbackToTurnRequest, AgentSessionWorkspaceLocation, LocalWorkspaceSnapshotPort,
    LocalWorkspaceSnapshotSessionRequest, LocalWorkspaceSnapshotStats, PortError, PortErrorKind,
};

use crate::peer_host::args::{get_string, optional_string, request_value};
use crate::peer_host::state::PeerHostState;

use super::session::{ensure_session_workspace_runtime_ownership, resolved_session_storage_scope};

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

async fn require_complete_rollback_workspace(
    request: &Value,
    workspace_path: &str,
    explicit_location: Option<AgentSessionWorkspaceLocation>,
) -> Result<(), String> {
    let is_remote = optional_string(request, "remoteConnectionId").is_some()
        || optional_string(request, "remoteSshHost").is_some()
        || explicit_location == Some(AgentSessionWorkspaceLocation::Remote)
        || (explicit_location.is_none() && is_remote_path(workspace_path).await);
    if is_remote {
        return Err(format!(
            "Complete rollback is not supported for remote workspaces because remote file snapshots are not recorded. No workspace files or session messages were changed: {workspace_path}"
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
    max_turn_exclusive: Option<usize>,
) -> Result<Vec<PathBuf>, String> {
    port.get_session_files(LocalWorkspaceSnapshotSessionRequest {
        workspace_path,
        session_id,
        max_turn_exclusive,
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
    max_turn_exclusive: Option<usize>,
) -> Result<LocalWorkspaceSnapshotStats, String> {
    port.get_session_stats(LocalWorkspaceSnapshotSessionRequest {
        workspace_path,
        session_id,
        max_turn_exclusive,
    })
    .await
    .map_err(|error| {
        format!(
            "Failed to get session stats: {}",
            snapshot_compatibility_error(error)
        )
    })
}

pub(crate) async fn get_session_files(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let session_id = get_string(request, "sessionId")?;
    let workspace_path = get_string(request, "workspacePath")?;

    openbitfun_agent_runtime::session_control::validate_session_id(&session_id)?;
    require_local_snapshot_workspace(request, &workspace_path).await?;
    let scope = ensure_session_workspace_runtime_ownership(state, request)?;
    let storage_path = resolved_session_storage_scope(state, scope).await?;
    let read = state
        .compatibility
        .begin_persisted_session_read(&storage_path, &session_id)
        .await
        .map_err(|error| format!("Failed to open a consistent snapshot view: {error}"))?;
    let files = local_snapshot_session_files(
        state.local_workspace_snapshot.as_ref(),
        PathBuf::from(&workspace_path),
        session_id,
        read.visible_turn_end(),
    )
    .await?;

    Ok(json!(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()))
}

pub(crate) async fn rollback_session_to_turn(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = request_value(args);
    let rollback_request: AgentSessionRollbackToTurnRequest =
        serde_json::from_value(request.clone())
            .map_err(|error| format!("Invalid targeted Session rollback request: {error}"))?;

    openbitfun_agent_runtime::session_control::validate_session_id(&rollback_request.session_id)?;
    require_complete_rollback_workspace(
        request,
        &rollback_request.workspace_path,
        rollback_request.explicit_workspace_location(),
    )
    .await?;
    ensure_session_workspace_runtime_ownership(state, request)?;
    let outcome = state
        .agent_runtime
        .rollback_session_to_turn(rollback_request)
        .await
        .map_err(|error| error.into_message())?;
    serde_json::to_value(outcome).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use openbitfun_runtime_ports::{
        LocalWorkspaceSnapshotPort, LocalWorkspaceSnapshotSessionRequest,
        LocalWorkspaceSnapshotStats, LocalWorkspaceSnapshotTurnRequest, PortError, PortErrorKind,
        PortResult,
    };
    use serde_json::json;

    use super::{
        local_snapshot_session_files, local_snapshot_session_stats,
        require_complete_rollback_workspace, require_local_snapshot_workspace,
        snapshot_compatibility_error,
    };

    #[derive(Default)]
    struct RecordingSnapshotPort {
        file_calls: AtomicUsize,
        stats_calls: AtomicUsize,
        file_request: Mutex<Option<LocalWorkspaceSnapshotSessionRequest>>,
        stats_request: Mutex<Option<LocalWorkspaceSnapshotSessionRequest>>,
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
            _request: LocalWorkspaceSnapshotTurnRequest,
        ) -> PortResult<Vec<PathBuf>> {
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

        let rollback_error = require_complete_rollback_workspace(
            &json!({ "remoteConnectionId": "remote-1" }),
            "/root/repos",
            None,
        )
        .await
        .expect_err("complete remote rollback must report missing snapshot coverage");
        assert_eq!(
            rollback_error,
            "Complete rollback is not supported for remote workspaces because remote file snapshots are not recorded. No workspace files or session messages were changed: /root/repos"
        );

        let source = include_str!("snapshot.rs");
        let rollback_source = &source[source
            .find("pub(crate) async fn rollback_session_to_turn")
            .expect("rollback handler must exist")..];
        let remote_guard = rollback_source
            .find("require_complete_rollback_workspace(")
            .expect("rollback must have an explicit remote guard");
        let runtime_call = rollback_source
            .find(".rollback_session_to_turn(")
            .expect("rollback must delegate to the Agent Session runtime");
        assert!(remote_guard < runtime_call);
    }

    #[tokio::test]
    async fn explicit_local_rollback_identity_wins_over_a_remote_path_collision() {
        let workspace = tempfile::tempdir().expect("create local workspace");
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let remote =
            openbitfun_core::service::remote_ssh::workspace_state::init_remote_workspace_manager();
        remote
            .register_remote_workspace(
                workspace_path.clone(),
                "peer-rollback-path-collision".to_string(),
                "Peer rollback collision test".to_string(),
                "remote.example".to_string(),
            )
            .await;

        require_complete_rollback_workspace(
            &json!({ "workspaceId": "local_workspace-1" }),
            &workspace_path,
            Some(openbitfun_runtime_ports::AgentSessionWorkspaceLocation::Local),
        )
        .await
        .expect("explicit local identity must disambiguate the registered remote path");

        remote
            .unregister_remote_workspace("peer-rollback-path-collision", &workspace_path)
            .await;
    }

    #[tokio::test]
    async fn local_snapshot_adapter_calls_each_port_operation_once_with_typed_requests() {
        let port = RecordingSnapshotPort::default();
        let workspace = PathBuf::from("workspace");

        let files = local_snapshot_session_files(
            &port,
            workspace.clone(),
            "session-1".to_string(),
            Some(2),
        )
        .await
        .expect("file projection should succeed");
        let stats = local_snapshot_session_stats(
            &port,
            workspace.clone(),
            "session-1".to_string(),
            Some(2),
        )
        .await
        .expect("stats projection should succeed");
        assert_eq!(port.file_calls.load(Ordering::SeqCst), 1);
        assert_eq!(port.stats_calls.load(Ordering::SeqCst), 1);
        assert_eq!(files, vec![PathBuf::from("changed.txt")]);
        assert_eq!(stats.total_changes, 3);
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
            port.file_request
                .lock()
                .expect("file request lock")
                .as_ref()
                .expect("file request")
                .max_turn_exclusive,
            Some(2)
        );
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
