use crate::service::snapshot::events::{emit_snapshot_session_event, SnapshotEvent};
use crate::service::snapshot::file_lock_manager::FileLockManager;
use crate::service::snapshot::isolation_manager::IsolationManager;
use crate::service::snapshot::snapshot_core::{SessionStats, SnapshotCore};
use crate::service::snapshot::snapshot_system::FileSnapshotSystem;
use crate::service::snapshot::types::{
    OperationType, SessionInfo, SnapshotConfig, SnapshotError, SnapshotResult,
};
use crate::service::workspace_runtime::WorkspaceRuntimeContext;
use log::info;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Snapshot-based change tracking service (operation-history + file snapshots, git-isolated).
pub struct SnapshotService {
    config: SnapshotConfig,
    isolation_manager: Arc<RwLock<IsolationManager>>,
    file_lock_manager: Arc<FileLockManager>,
    snapshot_core: Arc<RwLock<SnapshotCore>>,
    workspace_dir: PathBuf,
    runtime_context: WorkspaceRuntimeContext,
    initialized: bool,
}

impl SnapshotService {
    pub fn new(
        workspace_dir: PathBuf,
        runtime_context: WorkspaceRuntimeContext,
        config: Option<SnapshotConfig>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let isolation_manager = Arc::new(RwLock::new(IsolationManager::new(
            workspace_dir.clone(),
            runtime_context.clone(),
        )));
        let snapshot_system = FileSnapshotSystem::new(runtime_context.clone());
        let snapshot_core = Arc::new(RwLock::new(SnapshotCore::new(
            runtime_context.clone(),
            snapshot_system,
        )));
        let file_lock_manager = Arc::new(FileLockManager::new(runtime_context.clone()));

        Self {
            config,
            isolation_manager,
            file_lock_manager,
            snapshot_core,
            workspace_dir,
            runtime_context,
            initialized: false,
        }
    }

    pub async fn initialize(&mut self) -> SnapshotResult<()> {
        if self.initialized {
            return Ok(());
        }

        info!("Initializing snapshot/operation history service");

        {
            let mut isolation_manager = self.isolation_manager.write().await;
            isolation_manager.ensure_complete_isolation().await?;
        }

        {
            let mut snapshot_core = self.snapshot_core.write().await;
            snapshot_core.initialize().await?;
        }

        self.file_lock_manager.initialize().await?;
        self.initialized = true;

        let isolation_status = {
            let isolation_manager = self.isolation_manager.read().await;
            isolation_manager.check_isolation_status().await?
        };
        info!(
            "Snapshot service initialized: git_isolated={} bitfun_dir={}",
            isolation_status,
            self.runtime_context.runtime_root.display()
        );

        Ok(())
    }

    /// Record a file change (before the actual change). Returns operation_id.
    pub async fn record_file_change(
        &self,
        session_id: &str,
        turn_index: usize,
        file_path: PathBuf,
        operation_type: OperationType,
        tool_name: String,
    ) -> SnapshotResult<String> {
        self.ensure_initialized().await?;
        self.validate_file_path(&file_path).await?;

        let mut snapshot_core = self.snapshot_core.write().await;
        snapshot_core
            .start_file_operation(
                session_id,
                turn_index,
                file_path,
                operation_type,
                tool_name,
                serde_json::json!({}),
                None,
            )
            .await
    }

    /// Intercept a tool call before it modifies the file system.
    #[allow(clippy::too_many_arguments)]
    pub async fn intercept_file_modification(
        &self,
        session_id: &str,
        turn_index: usize,
        tool_name: &str,
        tool_input: serde_json::Value,
        file_path: &Path,
        operation_type: OperationType,
        operation_id_override: Option<String>,
    ) -> SnapshotResult<String> {
        self.ensure_initialized().await?;
        self.validate_file_path(file_path).await?;

        let operation_id = {
            let mut snapshot_core = self.snapshot_core.write().await;
            snapshot_core
                .start_file_operation(
                    session_id,
                    turn_index,
                    file_path.to_path_buf(),
                    operation_type.clone(),
                    tool_name.to_string(),
                    tool_input,
                    operation_id_override,
                )
                .await?
        };

        emit_snapshot_session_event(
            session_id,
            SnapshotEvent::file_modification_started(
                session_id.to_string(),
                operation_id.clone(),
                file_path.to_path_buf(),
                format!("{:?}", operation_type),
            ),
        )
        .await;

        Ok(operation_id)
    }

    pub async fn get_file_diff_with_anchor(
        &self,
        session_id: &str,
        file_path: &Path,
        anchor_operation_id: Option<&str>,
    ) -> SnapshotResult<(String, String, Option<usize>)> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        snapshot_core
            .get_file_diff_with_anchor(file_path, session_id, anchor_operation_id)
            .await
    }

    pub async fn get_session_file_diff_stats(
        &self,
        session_id: &str,
        file_path: &Path,
    ) -> SnapshotResult<crate::service::snapshot::types::SessionFileDiffStats> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        snapshot_core
            .get_session_file_diff_stats(session_id, file_path)
            .await
    }

    pub async fn get_operation_summary(
        &self,
        session_id: &str,
        operation_id: &str,
    ) -> SnapshotResult<crate::service::snapshot::types::FileOperation> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        snapshot_core.get_operation(session_id, operation_id)
    }

    /// Complete a file modification (after snapshot + diff summary).
    pub async fn complete_file_modification(
        &self,
        session_id: &str,
        operation_id: &str,
        execution_time_ms: u64,
    ) -> SnapshotResult<()> {
        self.ensure_initialized().await?;

        let completed_op = {
            let mut snapshot_core = self.snapshot_core.write().await;
            snapshot_core
                .complete_file_operation(session_id, operation_id, execution_time_ms)
                .await?
        };

        emit_snapshot_session_event(
            session_id,
            SnapshotEvent::file_modification_completed(
                session_id.to_string(),
                operation_id.to_string(),
                completed_op.file_path.clone(),
                completed_op.diff_summary.lines_added,
                completed_op.diff_summary.lines_removed,
            ),
        )
        .await;

        let status = if completed_op.before_snapshot_id.is_none()
            && completed_op.after_snapshot_id.is_some()
        {
            "created"
        } else if completed_op.before_snapshot_id.is_some()
            && completed_op.after_snapshot_id.is_none()
        {
            "deleted"
        } else {
            "modified"
        };

        emit_snapshot_session_event(
            session_id,
            SnapshotEvent::file_state_updated(
                session_id.to_string(),
                completed_op.file_path.clone(),
                status.to_string(),
                completed_op.diff_summary.lines_added,
                completed_op.diff_summary.lines_removed,
            ),
        )
        .await;

        Ok(())
    }

    pub async fn rollback_session(&self, session_id: &str) -> SnapshotResult<Vec<PathBuf>> {
        self.ensure_initialized().await?;
        info!("Rolling back session: session_id={}", session_id);

        let mut snapshot_core = self.snapshot_core.write().await;
        let restored_files = snapshot_core.rollback_session(session_id).await?;

        let released_count = self
            .file_lock_manager
            .release_session_locks(session_id)
            .await?;
        if released_count > 0 {
            info!(
                "Released {} file locks: session_id={}",
                released_count, session_id
            );
        }

        Ok(restored_files)
    }

    pub async fn rollback_to_turn(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> SnapshotResult<Vec<PathBuf>> {
        self.ensure_initialized().await?;
        info!(
            "Rolling back to turn: session_id={} turn_index={}",
            session_id, turn_index
        );

        let mut snapshot_core = self.snapshot_core.write().await;
        snapshot_core.rollback_to_turn(session_id, turn_index).await
    }

    pub async fn accept_session(&self, session_id: &str) -> SnapshotResult<()> {
        self.ensure_initialized().await?;
        info!("Accepting session changes: session_id={}", session_id);

        let mut snapshot_core = self.snapshot_core.write().await;
        snapshot_core.cleanup_session(session_id).await?;

        let released_count = self
            .file_lock_manager
            .release_session_locks(session_id)
            .await?;
        if released_count > 0 {
            info!(
                "Released {} file locks: session_id={}",
                released_count, session_id
            );
        }

        Ok(())
    }

    pub async fn accept_file(&self, session_id: &str, file_path: &Path) -> SnapshotResult<()> {
        self.ensure_initialized().await?;
        self.validate_file_path(file_path).await?;

        let mut snapshot_core = self.snapshot_core.write().await;
        snapshot_core
            .cleanup_file_session(session_id, file_path)
            .await?;

        self.file_lock_manager
            .release_lock(&file_path.to_path_buf(), session_id)
            .await?;

        Ok(())
    }

    pub async fn reject_file(
        &self,
        session_id: &str,
        file_path: &Path,
    ) -> SnapshotResult<Vec<PathBuf>> {
        self.ensure_initialized().await?;
        self.validate_file_path(file_path).await?;

        let mut snapshot_core = self.snapshot_core.write().await;
        let restored_files = snapshot_core
            .rollback_file_session(session_id, file_path)
            .await?;

        self.file_lock_manager
            .release_lock(&file_path.to_path_buf(), session_id)
            .await?;

        Ok(restored_files)
    }

    pub async fn get_session_files(&self, session_id: &str) -> SnapshotResult<Vec<PathBuf>> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        Ok(snapshot_core.get_session_files(session_id))
    }

    pub async fn get_session_turns(&self, session_id: &str) -> SnapshotResult<Vec<usize>> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        Ok(snapshot_core.get_session_turns(session_id))
    }

    pub async fn get_turn_files(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> SnapshotResult<Vec<PathBuf>> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        Ok(snapshot_core.get_turn_files(session_id, turn_index))
    }

    pub async fn get_file_diff(
        &self,
        session_id: &str,
        file_path: &Path,
    ) -> SnapshotResult<(String, String)> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        snapshot_core.get_file_diff(file_path, session_id).await
    }

    pub async fn get_session_stats(&self, session_id: &str) -> SnapshotResult<SessionStats> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        Ok(snapshot_core.get_session_stats(session_id))
    }

    pub async fn get_system_stats(&self) -> SnapshotResult<SystemStats> {
        self.ensure_initialized().await?;
        let isolation_status = {
            let isolation_manager = self.isolation_manager.read().await;
            isolation_manager.check_isolation_status().await?
        };
        Ok(SystemStats {
            git_isolated: isolation_status,
            bitfun_dir: self.runtime_context.runtime_root.clone(),
        })
    }

    pub async fn list_sessions(&self) -> SnapshotResult<Vec<String>> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        Ok(snapshot_core.list_session_ids())
    }

    pub async fn get_file_change_history(
        &self,
        file_path: &Path,
    ) -> SnapshotResult<Vec<crate::service::snapshot::snapshot_core::FileChangeEntry>> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        Ok(snapshot_core.get_file_change_history(file_path))
    }

    pub async fn get_all_modified_files(&self) -> SnapshotResult<Vec<PathBuf>> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        Ok(snapshot_core.get_all_modified_files())
    }

    pub async fn get_session(&self, session_id: &str) -> SnapshotResult<SessionInfo> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        let operations = snapshot_core.get_session_operations(session_id);
        Ok(SessionInfo {
            session_id: session_id.to_string(),
            operations,
        })
    }

    pub async fn update_file_diff_incremental(
        &self,
        _session_id: &str,
        _operation: &crate::service::snapshot::types::FileOperation,
        _before_content: Option<&str>,
        _after_content: Option<&str>,
    ) -> SnapshotResult<()> {
        Ok(())
    }

    pub async fn get_snapshot_content(&self, snapshot_id: &str) -> SnapshotResult<String> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        snapshot_core.get_snapshot_content(snapshot_id).await
    }

    pub async fn get_snapshot_core(&self) -> tokio::sync::RwLockReadGuard<'_, SnapshotCore> {
        self.snapshot_core.read().await
    }

    pub async fn try_acquire_file_lock(
        &self,
        session_id: &str,
        file_path: &Path,
        tool_name: &str,
    ) -> SnapshotResult<bool> {
        self.ensure_initialized().await?;
        self.file_lock_manager
            .try_acquire_lock(&file_path.to_path_buf(), session_id, tool_name)
            .await
    }

    pub async fn release_file_lock(
        &self,
        session_id: &str,
        file_path: &Path,
    ) -> SnapshotResult<()> {
        self.ensure_initialized().await?;
        self.file_lock_manager
            .release_lock(&file_path.to_path_buf(), session_id)
            .await
    }

    pub async fn get_file_lock_status(
        &self,
        file_path: &Path,
    ) -> SnapshotResult<Option<serde_json::Value>> {
        self.ensure_initialized().await?;
        let lock = self
            .file_lock_manager
            .get_lock_status(&file_path.to_path_buf())
            .await;
        if let Some(lock) = lock {
            Ok(Some(serde_json::to_value(lock)?))
        } else {
            Ok(None)
        }
    }

    pub async fn detect_file_conflict(
        &self,
        session_id: &str,
        file_path: &Path,
        _tool_name: &str,
    ) -> SnapshotResult<Option<serde_json::Value>> {
        self.ensure_initialized().await?;
        let file_path_buf = file_path.to_path_buf();
        if let Some(existing_lock) = self.file_lock_manager.get_lock_status(&file_path_buf).await {
            if existing_lock.session_id != session_id {
                let conflict_info = serde_json::json!({
                    "conflicting_file": file_path.to_string_lossy(),
                    "current_session": session_id,
                    "blocking_session": existing_lock.session_id,
                    "blocking_operation": {
                        "tool_name": existing_lock.tool_name,
                        "locked_at": existing_lock.locked_at,
                        "operation_type": existing_lock.operation_type
                    }
                });
                return Ok(Some(conflict_info));
            }
        }
        Ok(None)
    }

    async fn ensure_initialized(&self) -> SnapshotResult<()> {
        if !self.initialized {
            return Err(SnapshotError::ConfigError(
                "snapshot service not initialized, please call initialize() first".to_string(),
            ));
        }
        Ok(())
    }

    async fn validate_file_path(&self, file_path: &Path) -> SnapshotResult<()> {
        let isolation_manager = self.isolation_manager.read().await;
        if !isolation_manager.is_path_safe_for_modification(file_path) {
            return Err(SnapshotError::GitIsolationFailure(format!(
                "file path is not safe, may affect Git repository: {}",
                file_path.display()
            )));
        }
        Ok(())
    }

    pub fn get_config(&self) -> &SnapshotConfig {
        &self.config
    }

    pub fn get_workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    pub fn get_bitfun_dir(&self) -> &Path {
        &self.runtime_context.runtime_root
    }

    pub async fn check_git_isolation(&self) -> SnapshotResult<bool> {
        let isolation_manager = self.isolation_manager.read().await;
        isolation_manager.check_isolation_status().await
    }

    /// Returns the baseline snapshot ID for a file.
    pub async fn get_baseline_snapshot_id(&self, file_path: &Path) -> Option<String> {
        let snapshot_core = self.snapshot_core.read().await;
        snapshot_core.get_baseline_snapshot_id(file_path).await
    }

    /// Returns the baseline diff for a file.
    /// Returns: (original_content, modified_content)
    pub async fn get_baseline_snapshot_diff(
        &self,
        file_path: &Path,
    ) -> SnapshotResult<(String, String)> {
        self.ensure_initialized().await?;
        let snapshot_core = self.snapshot_core.read().await;
        snapshot_core.get_baseline_snapshot_diff(file_path).await
    }
}

/// System stats
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemStats {
    pub git_isolated: bool,
    pub bitfun_dir: PathBuf,
}
