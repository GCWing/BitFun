use crate::agentic::session::revert::{SessionRevertState, SessionWorkspaceCheckpoint};
use crate::service::snapshot::snapshot_system::FileSnapshotSystem;
use crate::service::snapshot::types::{
    DiffSummary, FileOperation, OperationType, SessionFileDiffStats, SnapshotError, SnapshotResult,
    ToolContext,
};
use crate::service::workspace_runtime::WorkspaceRuntimeContext;
use log::{debug, info, warn};
use openbitfun_services_core::json_store::JsonFileStore;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub session_id: String,
    pub total_files: usize,
    pub total_turns: usize,
    pub total_changes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeEntry {
    pub session_id: String,
    pub turn_index: usize,
    pub snapshot_id: String,
    pub timestamp: SystemTime,
    pub operation_type: OperationType,
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeQueue {
    pub file_path: PathBuf,
    pub changes: Vec<FileChangeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TurnHistory {
    turn_index: usize,
    operations: Vec<FileOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionHistory {
    session_id: String,
    turns: BTreeMap<usize, TurnHistory>,
    created_at: SystemTime,
    last_updated: SystemTime,
}

/// Per-side size budget: above this we avoid loading baseline/disk texts for UI badge stats.
const SESSION_FILE_DIFF_STATS_MAX_SOURCE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone)]
struct SessionFileBoundary {
    before_snapshot_id: Option<String>,
    after_snapshot_id: Option<String>,
    file_created_in_session: bool,
    file_deleted_in_session: bool,
}

impl SessionHistory {
    fn new(session_id: String) -> Self {
        let now = SystemTime::now();
        Self {
            session_id,
            turns: BTreeMap::new(),
            created_at: now,
            last_updated: now,
        }
    }

    fn ensure_turn_mut(&mut self, turn_index: usize) -> &mut TurnHistory {
        self.turns.entry(turn_index).or_insert_with(|| TurnHistory {
            turn_index,
            operations: Vec::new(),
        })
    }

    fn all_operations_iter(&self) -> impl Iterator<Item = &FileOperation> {
        self.turns.values().flat_map(|t| t.operations.iter())
    }

    #[allow(dead_code)]
    fn all_operations_iter_mut(&mut self) -> impl Iterator<Item = &mut FileOperation> {
        self.turns
            .values_mut()
            .flat_map(|t| t.operations.iter_mut())
    }
}

/// Snapshot core: keep operation history and snapshots (before/after).
pub struct SnapshotCore {
    sessions: HashMap<String, SessionHistory>,
    operation_index: HashMap<String, (String, usize, usize)>,
    snapshot_system: FileSnapshotSystem,
    sessions_dir: PathBuf,
}

impl SnapshotCore {
    pub fn new(
        runtime_context: WorkspaceRuntimeContext,
        snapshot_system: FileSnapshotSystem,
    ) -> Self {
        let sessions_dir = runtime_context.snapshot_operations_dir.clone();
        Self {
            sessions: HashMap::new(),
            operation_index: HashMap::new(),
            snapshot_system,
            sessions_dir,
        }
    }

    pub async fn initialize(&mut self) -> SnapshotResult<()> {
        let total_started_at = Instant::now();
        info!("Initializing operation history system");

        let snapshot_system_started_at = Instant::now();
        self.snapshot_system.initialize().await?;
        debug!(
            "Operation history initialize step completed: step=file_snapshot_system duration_ms={}",
            snapshot_system_started_at.elapsed().as_millis()
        );

        let sessions_started_at = Instant::now();
        self.load_all_sessions().await?;
        debug!(
            "Operation history initialize step completed: step=load_sessions duration_ms={}",
            sessions_started_at.elapsed().as_millis()
        );
        info!(
            "Operation history system initialized: loaded_sessions={} duration_ms={}",
            self.sessions.len(),
            total_started_at.elapsed().as_millis()
        );
        Ok(())
    }

    /// Start a file operation (before snapshot), returns operation_id.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_file_operation(
        &mut self,
        session_id: &str,
        turn_index: usize,
        file_path: PathBuf,
        operation_type: OperationType,
        tool_name: String,
        tool_input: serde_json::Value,
        operation_id_override: Option<String>,
    ) -> SnapshotResult<String> {
        let operation_id = operation_id_override
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        if self.operation_index.contains_key(&operation_id) {
            return Err(SnapshotError::ConfigError(format!(
                "operation_id already exists: {}",
                operation_id
            )));
        }
        let before_snapshot_id = if self.snapshot_system.workspace_exists(&file_path).await? {
            Some(
                self.snapshot_system
                    .create_owned_snapshot(&file_path)
                    .await?,
            )
        } else {
            None
        };

        if !self.snapshot_system.has_baseline(&file_path).await {
            match &before_snapshot_id {
                Some(before_id) => match self
                    .snapshot_system
                    .create_baseline_from_snapshot(&file_path, before_id)
                    .await
                {
                    Ok(baseline_id) => {
                        debug!(
                            "Created baseline snapshot: file_path={:?} baseline_id={}",
                            file_path, baseline_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to create baseline snapshot: file_path={:?} error={}",
                            file_path, e
                        );
                    }
                },
                None if operation_type == OperationType::Create => {
                    match self.snapshot_system.create_empty_baseline(&file_path).await {
                        Ok(baseline_id) => {
                            debug!(
                                "Created empty baseline snapshot for new file: file_path={:?} baseline_id={}",
                                file_path, baseline_id
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Failed to create empty baseline snapshot: file_path={:?} error={}",
                                file_path, e
                            );
                        }
                    }
                }
                None => {}
            }
        } else {
            debug!(
                "Baseline snapshot already exists: file_path={:?}",
                file_path
            );
        }

        let session = self
            .sessions
            .entry(session_id.to_string())
            .or_insert_with(|| SessionHistory::new(session_id.to_string()));
        let turn = session.ensure_turn_mut(turn_index);
        let seq_in_turn = turn.operations.len();

        turn.operations.push(FileOperation {
            operation_id: operation_id.clone(),
            session_id: session_id.to_string(),
            turn_index,
            seq_in_turn,
            file_path: file_path.clone(),
            operation_type,
            tool_context: ToolContext {
                tool_name,
                tool_input,
                execution_time_ms: 0,
            },
            before_snapshot_id,
            after_snapshot_id: None,
            completed: Some(false),
            timestamp: SystemTime::now(),
            diff_summary: DiffSummary::default(),
            path_before: None,
            path_after: None,
        });

        session.last_updated = SystemTime::now();
        self.operation_index.insert(
            operation_id.clone(),
            (session_id.to_string(), turn_index, seq_in_turn),
        );
        self.persist_session(session_id).await?;

        Ok(operation_id)
    }

    pub fn get_operation(
        &self,
        session_id: &str,
        operation_id: &str,
    ) -> SnapshotResult<FileOperation> {
        self.get_operation_before(session_id, operation_id, None)
    }

    pub fn get_operation_before(
        &self,
        session_id: &str,
        operation_id: &str,
        max_turn_exclusive: Option<usize>,
    ) -> SnapshotResult<FileOperation> {
        let Some((sid, turn_index, seq)) = self.operation_index.get(operation_id).cloned() else {
            return Err(SnapshotError::OperationNotFound(operation_id.to_string()));
        };
        if sid != session_id {
            return Err(SnapshotError::ConfigError(format!(
                "operation_id does not belong to current session: op={} session={} actual={}",
                operation_id, session_id, sid
            )));
        }
        if max_turn_exclusive.is_some_and(|end| turn_index >= end) {
            return Err(SnapshotError::OperationNotFound(operation_id.to_string()));
        }
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SnapshotError::SessionNotFound(session_id.to_string()))?;
        let turn = session
            .turns
            .get(&turn_index)
            .ok_or_else(|| SnapshotError::ConfigError("turn not found".to_string()))?;
        let op = turn
            .operations
            .get(seq)
            .ok_or_else(|| SnapshotError::ConfigError("seq_in_turn out of bounds".to_string()))?;
        Ok(op.clone())
    }

    /// Complete a file operation (after snapshot + diff summary).
    pub async fn complete_file_operation(
        &mut self,
        session_id: &str,
        operation_id: &str,
        execution_time_ms: u64,
    ) -> SnapshotResult<FileOperation> {
        let (sid, turn_index, seq) = self
            .operation_index
            .get(operation_id)
            .cloned()
            .ok_or_else(|| SnapshotError::OperationNotFound(operation_id.to_string()))?;
        if sid != session_id {
            return Err(SnapshotError::ConfigError(format!(
                "operation_id does not belong to current session: op={} session={} actual={}",
                operation_id, session_id, sid
            )));
        }

        let (before_snapshot_id, after_snapshot_id) = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SnapshotError::SessionNotFound(session_id.to_string()))?;
            let turn = session
                .turns
                .get_mut(&turn_index)
                .ok_or_else(|| SnapshotError::ConfigError("turn not found".to_string()))?;
            let op = turn.operations.get_mut(seq).ok_or_else(|| {
                SnapshotError::ConfigError("seq_in_turn out of bounds".to_string())
            })?;

            op.tool_context.execution_time_ms = execution_time_ms;

            let after_snapshot_id = if self.snapshot_system.workspace_exists(&op.file_path).await? {
                Some(
                    self.snapshot_system
                        .create_owned_snapshot(&op.file_path)
                        .await?,
                )
            } else {
                None
            };
            op.after_snapshot_id = after_snapshot_id;

            (op.before_snapshot_id.clone(), op.after_snapshot_id.clone())
        };

        let before_text = self.load_snapshot_text(before_snapshot_id.as_deref()).await;
        // Diff the exact bytes just recorded, avoiding a second workspace
        // transfer and a race with edits after the after-snapshot was taken.
        let after_text = self.load_snapshot_text(after_snapshot_id.as_deref()).await;
        let diff_summary = compute_diff_summary(&before_text, &after_text);

        let (completed_op, previous_completed) = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| SnapshotError::SessionNotFound(session_id.to_string()))?;
            let turn = session
                .turns
                .get_mut(&turn_index)
                .ok_or_else(|| SnapshotError::ConfigError("turn not found".to_string()))?;
            let op = turn.operations.get_mut(seq).ok_or_else(|| {
                SnapshotError::ConfigError("seq_in_turn out of bounds".to_string())
            })?;

            op.diff_summary = diff_summary;
            let previous_completed = op.completed;
            op.completed = Some(true);
            session.last_updated = SystemTime::now();
            (op.clone(), previous_completed)
        };

        if let Err(error) = self.persist_session(session_id).await {
            // A reader must not observe completion when its durable record
            // failed to commit. The tool itself has already executed once.
            if let Some(op) = self
                .sessions
                .get_mut(session_id)
                .and_then(|session| session.turns.get_mut(&turn_index))
                .and_then(|turn| turn.operations.get_mut(seq))
            {
                op.completed = previous_completed;
            }
            return Err(error);
        }

        Ok(completed_op)
    }

    pub fn get_session_turns(&self, session_id: &str) -> Vec<usize> {
        self.get_session_turns_before(session_id, None)
    }

    pub fn get_session_turns_before(
        &self,
        session_id: &str,
        max_turn_exclusive: Option<usize>,
    ) -> Vec<usize> {
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        session
            .turns
            .keys()
            .copied()
            .filter(|turn| max_turn_exclusive.is_none_or(|end| *turn < end))
            .collect()
    }

    pub fn get_turn_files(&self, session_id: &str, turn_index: usize) -> Vec<PathBuf> {
        self.get_turn_files_before(session_id, turn_index, None)
    }

    pub fn get_turn_files_before(
        &self,
        session_id: &str,
        turn_index: usize,
        max_turn_exclusive: Option<usize>,
    ) -> Vec<PathBuf> {
        if max_turn_exclusive.is_some_and(|end| turn_index >= end) {
            return Vec::new();
        }
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        let Some(turn) = session.turns.get(&turn_index) else {
            return Vec::new();
        };
        unique_paths(
            turn.operations
                .iter()
                .filter(|op| operation_is_completed_for_session_file(op))
                .map(|op| op.file_path.clone()),
        )
    }

    pub fn get_session_files(&self, session_id: &str) -> Vec<PathBuf> {
        self.get_session_files_before(session_id, None)
    }

    pub fn get_session_files_before(
        &self,
        session_id: &str,
        max_turn_exclusive: Option<usize>,
    ) -> Vec<PathBuf> {
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        unique_paths(
            session
                .all_operations_iter()
                .filter(|op| operation_is_before(op, max_turn_exclusive))
                .filter(|op| operation_is_completed_for_session_file(op))
                .map(|op| op.file_path.clone()),
        )
    }

    pub fn get_session_operations(&self, session_id: &str) -> Vec<FileOperation> {
        self.get_session_operations_before(session_id, None)
    }

    pub fn get_session_operations_before(
        &self,
        session_id: &str,
        max_turn_exclusive: Option<usize>,
    ) -> Vec<FileOperation> {
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        session
            .all_operations_iter()
            .filter(|op| operation_is_before(op, max_turn_exclusive))
            .cloned()
            .collect()
    }

    pub fn get_all_modified_files(&self) -> Vec<PathBuf> {
        let mut all = Vec::new();
        for session in self.sessions.values() {
            all.extend(
                session
                    .all_operations_iter()
                    .filter(|op| operation_is_completed_for_session_file(op))
                    .map(|op| op.file_path.clone()),
            );
        }
        unique_paths(all.into_iter())
    }

    pub fn get_session_stats(&self, session_id: &str) -> SessionStats {
        self.get_session_stats_before(session_id, None)
    }

    pub fn get_session_stats_before(
        &self,
        session_id: &str,
        max_turn_exclusive: Option<usize>,
    ) -> SessionStats {
        let ops: Vec<FileOperation> = self
            .get_session_operations_before(session_id, max_turn_exclusive)
            .into_iter()
            .filter(operation_is_completed_for_session_file)
            .collect();
        let total_changes = ops.len();
        let total_files = unique_paths(ops.iter().map(|op| op.file_path.clone())).len();
        let total_turns = self
            .get_session_turns_before(session_id, max_turn_exclusive)
            .len();
        SessionStats {
            session_id: session_id.to_string(),
            total_files,
            total_turns,
            total_changes,
        }
    }

    pub fn list_session_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.sessions.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub async fn get_snapshot_content(&self, snapshot_id: &str) -> SnapshotResult<String> {
        self.snapshot_system.get_snapshot_content(snapshot_id).await
    }

    /// Returns the baseline snapshot ID for a file.
    pub async fn get_baseline_snapshot_id(&self, file_path: &Path) -> Option<String> {
        self.snapshot_system
            .get_baseline_snapshot_id(file_path)
            .await
    }

    /// Returns the baseline diff for a file.
    /// Original: baseline (state before the first AI modification)
    /// Modified: current file content
    pub async fn get_baseline_snapshot_diff(
        &self,
        file_path: &Path,
    ) -> SnapshotResult<(String, String)> {
        let baseline_content = if let Some(baseline_id) = self
            .snapshot_system
            .get_baseline_snapshot_id(file_path)
            .await
        {
            debug!(
                "Found baseline snapshot: file_path={:?} baseline_id={}",
                file_path, baseline_id
            );
            match self
                .snapshot_system
                .get_snapshot_content(&baseline_id)
                .await
            {
                Ok(content) => content,
                Err(e) => {
                    warn!(
                        "Failed to read baseline snapshot: baseline_id={} error={}",
                        baseline_id, e
                    );
                    String::new()
                }
            }
        } else {
            debug!(
                "No baseline snapshot found, file may not have been modified: file_path={:?}",
                file_path
            );
            String::new()
        };

        let current_content = self.load_path_text(file_path).await?;

        Ok((baseline_content, current_content))
    }

    pub async fn get_file_diff(
        &self,
        file_path: &Path,
        session_id: &str,
    ) -> SnapshotResult<(String, String)> {
        self.get_file_diff_before(file_path, session_id, None).await
    }

    pub async fn get_file_diff_before(
        &self,
        file_path: &Path,
        session_id: &str,
        max_turn_exclusive: Option<usize>,
    ) -> SnapshotResult<(String, String)> {
        let Some(session) = self.sessions.get(session_id) else {
            return Err(SnapshotError::SessionNotFound(session_id.to_string()));
        };

        let Some(boundary) = session_file_boundary(session, file_path, max_turn_exclusive) else {
            debug!(
                "No completed session file operation found for diff: file_path={:?} session_id={}",
                file_path, session_id
            );
            return Ok((String::new(), String::new()));
        };

        let before = self
            .load_snapshot_text(boundary.before_snapshot_id.as_deref())
            .await;
        let after = if boundary.file_deleted_in_session {
            String::new()
        } else {
            self.load_snapshot_text(boundary.after_snapshot_id.as_deref())
                .await
        };

        debug!(
            "get_file_diff result: file_path={:?} session_id={} before_len={} after_len={} identical={} file_created_in_session={} file_deleted_in_session={}",
            file_path,
            session_id,
            before.len(),
            after.len(),
            before == after,
            boundary.file_created_in_session,
            boundary.file_deleted_in_session
        );

        Ok((before, after))
    }

    pub async fn get_file_diff_with_anchor(
        &self,
        file_path: &Path,
        session_id: &str,
        anchor_operation_id: Option<&str>,
    ) -> SnapshotResult<(String, String, Option<usize>)> {
        self.get_file_diff_with_anchor_before(file_path, session_id, anchor_operation_id, None)
            .await
    }

    /// Read one operation's immutable snapshots, without consulting current
    /// workspace files or another operation on the same file.
    pub async fn get_operation_diff_before(
        &self,
        file_path: &Path,
        session_id: &str,
        operation_id: &str,
        max_turn_exclusive: Option<usize>,
    ) -> SnapshotResult<(String, String, Option<usize>)> {
        let operation = self.get_operation_before(session_id, operation_id, max_turn_exclusive)?;
        if operation.file_path.as_os_str() != file_path.as_os_str() {
            return Err(SnapshotError::ConfigError(format!(
                "Snapshot operation does not refer to the requested file: operation_id={operation_id}, file_path={}",
                file_path.display()
            )));
        }
        if !operation_is_completed_for_session_file(&operation) {
            return Err(SnapshotError::ConfigError(format!(
                "Snapshot operation has no completed file snapshot: {operation_id}"
            )));
        }
        let before = self
            .read_recorded_snapshot_text(operation.before_snapshot_id.as_deref())
            .await?;
        let after = self
            .read_recorded_snapshot_text(operation.after_snapshot_id.as_deref())
            .await?;
        let anchor = compute_anchor_line(&before, &after).or(Some(1));
        Ok((before, after, anchor))
    }

    async fn read_recorded_snapshot_text(
        &self,
        snapshot_id: Option<&str>,
    ) -> SnapshotResult<String> {
        match snapshot_id {
            None => Ok(String::new()),
            Some(id) if id.starts_with("empty_snapshot_") => Ok(String::new()),
            Some(id) => self.snapshot_system.get_snapshot_content(id).await,
        }
    }

    pub async fn get_file_diff_with_anchor_before(
        &self,
        file_path: &Path,
        session_id: &str,
        anchor_operation_id: Option<&str>,
        max_turn_exclusive: Option<usize>,
    ) -> SnapshotResult<(String, String, Option<usize>)> {
        let (before, after) = self
            .get_file_diff_before(file_path, session_id, max_turn_exclusive)
            .await?;

        let Some(operation_id) = anchor_operation_id.filter(|s| !s.is_empty()) else {
            return Ok((before, after, None));
        };

        let op = self.get_operation_before(session_id, operation_id, max_turn_exclusive)?;
        if op.file_path.as_os_str() != file_path.as_os_str() {
            return Ok((before, after, None));
        }

        let op_before_text = self
            .load_snapshot_text(op.before_snapshot_id.as_deref())
            .await;
        let op_after_text = self
            .load_snapshot_text(op.after_snapshot_id.as_deref())
            .await;

        let op_anchor_line = if op_after_text.is_empty() {
            Some(1)
        } else {
            compute_anchor_line(&op_before_text, &op_after_text).or(Some(1))
        };

        let mapped_anchor = op_anchor_line.and_then(|line| {
            if after.is_empty() {
                Some(1)
            } else {
                find_anchor_in_current(&op_after_text, &after, line).or_else(|| {
                    let current_lines = split_lines_preserve_trailing(&after);
                    Some(line.min(current_lines.len().max(1)))
                })
            }
        });

        Ok((before, after, mapped_anchor))
    }

    /// Line insert/delete counts versus session baseline vs workspace, without returning file bodies.
    /// Large files skip full reads and aggregate per-operation diff summaries (`approximate: true`).
    pub async fn get_session_file_diff_stats(
        &self,
        session_id: &str,
        file_path: &Path,
    ) -> SnapshotResult<SessionFileDiffStats> {
        self.get_session_file_diff_stats_before(session_id, file_path, None)
            .await
    }

    pub async fn get_session_file_diff_stats_before(
        &self,
        session_id: &str,
        file_path: &Path,
        max_turn_exclusive: Option<usize>,
    ) -> SnapshotResult<SessionFileDiffStats> {
        let Some(session) = self.sessions.get(session_id) else {
            return Err(SnapshotError::SessionNotFound(session_id.to_string()));
        };

        let Some(boundary) = session_file_boundary(session, file_path, max_turn_exclusive) else {
            return Ok(SessionFileDiffStats {
                file_path: file_path.to_string_lossy().to_string(),
                lines_added: 0,
                lines_removed: 0,
                approximate: false,
                change_kind: "modify".to_string(),
            });
        };

        let before_bytes = self
            .session_snapshot_recorded_size(boundary.before_snapshot_id.as_deref())
            .await;
        let after_bytes = if boundary.file_deleted_in_session {
            0
        } else {
            self.session_snapshot_recorded_size(boundary.after_snapshot_id.as_deref())
                .await
        };

        let too_large = after_bytes > SESSION_FILE_DIFF_STATS_MAX_SOURCE_BYTES
            || before_bytes > SESSION_FILE_DIFF_STATS_MAX_SOURCE_BYTES;

        if too_large {
            let agg =
                aggregate_operations_diff_summary_for_file(session, file_path, max_turn_exclusive);
            let change_kind = change_kind_from_session_boundary(&boundary);
            debug!(
                "get_session_file_diff_stats: approximate session_id={} file_path={:?} after_bytes={} before_bytes={} lines_added={} lines_removed={}",
                session_id,
                file_path,
                after_bytes,
                before_bytes,
                agg.lines_added,
                agg.lines_removed
            );
            return Ok(SessionFileDiffStats {
                file_path: file_path.to_string_lossy().to_string(),
                lines_added: agg.lines_added,
                lines_removed: agg.lines_removed,
                approximate: true,
                change_kind: change_kind.to_string(),
            });
        }

        let (before, after) = self
            .get_file_diff_before(file_path, session_id, max_turn_exclusive)
            .await?;
        let summary = compute_diff_summary(&before, &after);
        let change_kind = change_kind_from_session_boundary(&boundary);
        debug!(
            "get_session_file_diff_stats: exact session_id={} file_path={:?} lines_added={} lines_removed={}",
            session_id,
            file_path,
            summary.lines_added,
            summary.lines_removed
        );
        Ok(SessionFileDiffStats {
            file_path: file_path.to_string_lossy().to_string(),
            lines_added: summary.lines_added,
            lines_removed: summary.lines_removed,
            approximate: false,
            change_kind: change_kind.to_string(),
        })
    }

    async fn session_snapshot_recorded_size(&self, snapshot_id: Option<&str>) -> u64 {
        let Some(snapshot_id) = snapshot_id else {
            return 0;
        };
        if snapshot_id.starts_with("empty_snapshot_") {
            return 0;
        }
        self.snapshot_system
            .get_snapshot_recorded_size_bytes(snapshot_id)
            .await
            .unwrap_or(SESSION_FILE_DIFF_STATS_MAX_SOURCE_BYTES.saturating_add(1))
    }

    pub fn get_file_change_history(&self, file_path: &Path) -> Vec<FileChangeEntry> {
        let mut entries = Vec::new();
        for session in self.sessions.values() {
            for op in session.all_operations_iter() {
                if op.file_path.as_os_str() == file_path.as_os_str() {
                    entries.push(FileChangeEntry {
                        session_id: op.session_id.clone(),
                        turn_index: op.turn_index,
                        snapshot_id: op
                            .before_snapshot_id
                            .clone()
                            .unwrap_or_else(|| format!("empty_snapshot_{}", op.operation_id)),
                        timestamp: op.timestamp,
                        operation_type: op.operation_type.clone(),
                        tool_name: op.tool_context.tool_name.clone(),
                    });
                }
            }
        }
        entries.sort_by_key(|e| (e.session_id.clone(), e.turn_index, e.timestamp));
        entries
    }

    pub async fn rollback_session(&mut self, session_id: &str) -> SnapshotResult<Vec<PathBuf>> {
        info!("Rolling back session: session_id={}", session_id);
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(Vec::new());
        };

        let mut to_rollback: Vec<FileOperation> = session.all_operations_iter().cloned().collect();
        to_rollback.sort_by_key(|op| (op.turn_index, op.seq_in_turn));
        to_rollback.reverse();

        let restored = self.apply_rollback_ops(&to_rollback).await?;

        self.sessions.remove(session_id);
        self.delete_session_file(session_id).await?;
        self.rebuild_operation_index();
        Ok(restored)
    }

    /// Rollback to the start of `target_turn` (undo target_turn and later turns).
    pub async fn rollback_workspace_files_to_boundary(
        &mut self,
        session_id: &str,
        target_turn: usize,
    ) -> SnapshotResult<Vec<PathBuf>> {
        info!(
            "Rolling back to turn: session_id={} turn_index={}",
            session_id, target_turn
        );
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(Vec::new());
        };

        let mut to_rollback: Vec<FileOperation> = session
            .all_operations_iter()
            .filter(|op| op.turn_index >= target_turn)
            .cloned()
            .collect();
        to_rollback.sort_by_key(|op| (op.turn_index, op.seq_in_turn));
        to_rollback.reverse();

        let restored = self.apply_rollback_ops(&to_rollback).await?;

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SnapshotError::SessionNotFound(session_id.to_string()))?;
        session
            .turns
            .retain(|turn_index, _| *turn_index < target_turn);
        session.last_updated = SystemTime::now();
        self.persist_session(session_id).await?;
        self.rebuild_operation_index();

        Ok(restored)
    }

    /// Extends the pre-undo workspace checkpoint for every path affected at or
    /// after the requested boundary. Repeated undo can safely discover earlier
    /// paths because paths not already checkpointed are still at their original
    /// pre-undo state before that earlier boundary is applied.
    pub(crate) async fn prepare_workspace_revert(
        &mut self,
        session_id: &str,
        state: &mut SessionRevertState,
    ) -> SnapshotResult<()> {
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(());
        };
        let existing = state
            .workspace_checkpoint
            .iter()
            .map(|checkpoint| checkpoint.path.as_os_str().to_os_string())
            .collect::<HashSet<_>>();
        let mut affected = session
            .all_operations_iter()
            .filter(|operation| operation.turn_index >= state.boundary_turn)
            .flat_map(|operation| {
                [
                    operation
                        .path_before
                        .clone()
                        .unwrap_or_else(|| operation.file_path.clone()),
                    operation
                        .path_after
                        .clone()
                        .unwrap_or_else(|| operation.file_path.clone()),
                ]
            })
            .filter(|path| !existing.contains(path.as_os_str()))
            .collect::<Vec<_>>();
        affected.sort_by(|left, right| left.as_os_str().cmp(right.as_os_str()));
        affected.dedup_by(|left, right| left.as_os_str() == right.as_os_str());

        for path in affected {
            let snapshot_id = if self.snapshot_system.workspace_exists(&path).await? {
                Some(self.snapshot_system.create_owned_snapshot(&path).await?)
            } else {
                None
            };
            state
                .workspace_checkpoint
                .push(SessionWorkspaceCheckpoint { path, snapshot_id });
        }
        state
            .workspace_checkpoint
            .sort_by(|left, right| left.path.as_os_str().cmp(right.path.as_os_str()));
        Ok(())
    }

    /// Applies a staged Session boundary without deleting operation history.
    /// Restoring the original checkpoint first makes retries and redo idempotent.
    pub(crate) async fn apply_workspace_revert(
        &self,
        session_id: &str,
        state: &SessionRevertState,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let mut restored = self.restore_workspace_revert(state).await?;
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(restored);
        };
        let mut operations = session
            .all_operations_iter()
            .filter(|operation| operation.turn_index >= state.boundary_turn)
            .cloned()
            .collect::<Vec<_>>();
        operations.sort_by_key(|operation| (operation.turn_index, operation.seq_in_turn));
        operations.reverse();
        restored.extend(self.apply_rollback_ops(&operations).await?);
        Ok(unique_paths(restored.into_iter()))
    }

    pub(crate) async fn restore_workspace_revert(
        &self,
        state: &SessionRevertState,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let mut restored = Vec::new();
        for checkpoint in &state.workspace_checkpoint {
            match checkpoint.snapshot_id.as_deref() {
                Some(snapshot_id) => {
                    self.snapshot_system
                        .restore_file(snapshot_id, &checkpoint.path)
                        .await?;
                    restored.push(checkpoint.path.clone());
                }
                None if self
                    .snapshot_system
                    .workspace_exists(&checkpoint.path)
                    .await? =>
                {
                    self.snapshot_system
                        .remove_workspace_file(&checkpoint.path)
                        .await?;
                    restored.push(checkpoint.path.clone());
                }
                None => {}
            }
        }
        Ok(unique_paths(restored.into_iter()))
    }

    pub(crate) async fn commit_workspace_revert(
        &mut self,
        session_id: &str,
        state: &SessionRevertState,
    ) -> SnapshotResult<()> {
        let discarded_snapshot_ids = self
            .sessions
            .get(session_id)
            .into_iter()
            .flat_map(|session| {
                session
                    .turns
                    .range(state.boundary_turn..)
                    .flat_map(|(_, turn)| turn.operations.iter())
            })
            .flat_map(Self::operation_snapshot_ids)
            .collect::<HashSet<_>>();
        let retained_snapshot_ids = self
            .sessions
            .iter()
            .flat_map(|(candidate_session_id, session)| {
                session.all_operations_iter().filter(move |operation| {
                    candidate_session_id != session_id || operation.turn_index < state.boundary_turn
                })
            })
            .flat_map(Self::operation_snapshot_ids)
            .collect::<HashSet<_>>();
        // Release suffix-owned handles before pruning their durable history.
        // A retry can still rediscover every handle if deletion or persistence
        // fails part-way through this committing phase.
        for snapshot_id in discarded_snapshot_ids.difference(&retained_snapshot_ids) {
            match self.snapshot_system.delete_snapshot(snapshot_id).await {
                Ok(()) | Err(SnapshotError::SnapshotNotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        if let Some(session) = self.sessions.get_mut(session_id) {
            session
                .turns
                .retain(|turn_index, _| *turn_index < state.boundary_turn);
            session.last_updated = SystemTime::now();
            self.persist_session(session_id).await?;
            self.rebuild_operation_index();
        }
        self.delete_workspace_revert_checkpoint(state).await
    }

    pub(crate) async fn delete_workspace_revert_checkpoint(
        &mut self,
        state: &SessionRevertState,
    ) -> SnapshotResult<()> {
        let operation_snapshot_ids = self.referenced_operation_snapshot_ids();
        for checkpoint in &state.workspace_checkpoint {
            if let Some(snapshot_id) = checkpoint.snapshot_id.as_deref() {
                // Markers written by older builds could borrow a deduplicated
                // operation snapshot ID. Preserve that handle until its
                // operation history owner releases it.
                if operation_snapshot_ids.contains(snapshot_id) {
                    continue;
                }
                match self.snapshot_system.delete_snapshot(snapshot_id).await {
                    Ok(()) | Err(SnapshotError::SnapshotNotFound(_)) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }

    pub async fn cleanup_session(&mut self, session_id: &str) -> SnapshotResult<()> {
        let owned_snapshot_ids = self
            .sessions
            .get(session_id)
            .into_iter()
            .flat_map(SessionHistory::all_operations_iter)
            .flat_map(Self::operation_snapshot_ids)
            .collect::<HashSet<_>>();

        self.sessions.remove(session_id);
        let retained_snapshot_ids = self.referenced_operation_snapshot_ids();
        let snapshot_ids_to_delete = owned_snapshot_ids
            .difference(&retained_snapshot_ids)
            .cloned()
            .collect::<Vec<_>>();

        for snapshot_id in &snapshot_ids_to_delete {
            match self.snapshot_system.delete_snapshot(snapshot_id).await {
                Ok(()) | Err(SnapshotError::SnapshotNotFound(_)) => {}
                Err(error) => warn!(
                    "Failed to delete snapshot: snapshot_id={} error={}",
                    snapshot_id, error
                ),
            }
        }

        if !snapshot_ids_to_delete.is_empty() {
            info!(
                "Cleaned up {} snapshot files: session_id={}",
                snapshot_ids_to_delete.len(),
                session_id
            );
        }

        self.delete_session_file(session_id).await?;

        self.rebuild_operation_index();

        Ok(())
    }

    fn operation_snapshot_ids(operation: &FileOperation) -> impl Iterator<Item = String> + '_ {
        [
            operation.before_snapshot_id.as_ref(),
            operation.after_snapshot_id.as_ref(),
        ]
        .into_iter()
        .flatten()
        .filter(|snapshot_id| !snapshot_id.starts_with("empty_snapshot_"))
        .cloned()
    }

    fn referenced_operation_snapshot_ids(&self) -> HashSet<String> {
        self.sessions
            .values()
            .flat_map(SessionHistory::all_operations_iter)
            .flat_map(Self::operation_snapshot_ids)
            .collect()
    }

    pub async fn cleanup_file_session(
        &mut self,
        session_id: &str,
        file_path: &Path,
    ) -> SnapshotResult<()> {
        let Some(session) = self.sessions.get_mut(session_id) else {
            return Ok(());
        };

        for turn in session.turns.values_mut() {
            turn.operations
                .retain(|op| !Self::operation_matches_file_path(op, file_path));
        }
        session.turns.retain(|_, t| !t.operations.is_empty());
        session.last_updated = SystemTime::now();
        self.persist_session(session_id).await?;
        self.rebuild_operation_index();
        Ok(())
    }

    pub async fn rollback_file_session(
        &mut self,
        session_id: &str,
        file_path: &Path,
    ) -> SnapshotResult<Vec<PathBuf>> {
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(Vec::new());
        };

        let mut to_rollback: Vec<FileOperation> = session
            .all_operations_iter()
            .filter(|op| Self::operation_matches_file_path(op, file_path))
            .cloned()
            .collect();
        to_rollback.sort_by_key(|op| (op.turn_index, op.seq_in_turn));
        to_rollback.reverse();

        let restored = self.apply_rollback_ops(&to_rollback).await?;
        self.cleanup_file_session(session_id, file_path).await?;
        Ok(restored)
    }

    fn operation_matches_file_path(op: &FileOperation, file_path: &Path) -> bool {
        op.file_path.as_os_str() == file_path.as_os_str()
            || op
                .path_before
                .as_ref()
                .is_some_and(|path| path.as_os_str() == file_path.as_os_str())
            || op
                .path_after
                .as_ref()
                .is_some_and(|path| path.as_os_str() == file_path.as_os_str())
    }

    async fn apply_rollback_ops(&self, ops: &[FileOperation]) -> SnapshotResult<Vec<PathBuf>> {
        let mut restored_files = Vec::new();
        for op in ops {
            let before_path = op.path_before.as_ref().unwrap_or(&op.file_path);
            let after_path = op.path_after.as_ref().unwrap_or(&op.file_path);
            if before_path.as_os_str() != after_path.as_os_str()
                && self.snapshot_system.workspace_exists(after_path).await?
            {
                self.snapshot_system
                    .remove_workspace_file(after_path)
                    .await?;
            }
            match op.before_snapshot_id.as_deref() {
                None => {
                    if self.snapshot_system.workspace_exists(after_path).await? {
                        self.snapshot_system
                            .remove_workspace_file(after_path)
                            .await?;
                        restored_files.push(after_path.clone());
                    }
                }
                Some(snapshot_id) if snapshot_id.starts_with("empty_snapshot_") => {
                    if self.snapshot_system.workspace_exists(after_path).await? {
                        self.snapshot_system
                            .remove_workspace_file(after_path)
                            .await?;
                        restored_files.push(after_path.clone());
                    }
                }
                Some(snapshot_id) => {
                    self.snapshot_system
                        .restore_file(snapshot_id, before_path)
                        .await?;
                    restored_files.push(before_path.clone());
                }
            }
        }
        Ok(unique_paths(restored_files.into_iter()))
    }

    async fn load_snapshot_text(&self, snapshot_id: Option<&str>) -> String {
        let Some(snapshot_id) = snapshot_id else {
            return String::new();
        };
        if snapshot_id.starts_with("empty_snapshot_") {
            return String::new();
        }
        self.snapshot_system
            .get_snapshot_content(snapshot_id)
            .await
            .unwrap_or_default()
    }

    async fn load_path_text(&self, path: &Path) -> SnapshotResult<String> {
        if !self.snapshot_system.workspace_exists(path).await? {
            return Ok(String::new());
        }
        self.snapshot_system.read_workspace_text(path).await
    }

    async fn load_all_sessions(&mut self) -> SnapshotResult<()> {
        let started_at = Instant::now();
        if !self.sessions_dir.exists() {
            return Ok(());
        }
        let mut dir = tokio::fs::read_dir(&self.sessions_dir)
            .await
            .map_err(SnapshotError::Io)?;
        let mut loaded = 0usize;
        while let Some(entry) = dir.next_entry().await.map_err(SnapshotError::Io)? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => match serde_json::from_str::<SessionHistory>(&content) {
                    Ok(session) => {
                        self.sessions.insert(session.session_id.clone(), session);
                        loaded += 1;
                    }
                    Err(e) => warn!(
                        "Failed to parse session file: path={} error={}",
                        path.display(),
                        e
                    ),
                },
                Err(e) => warn!(
                    "Failed to read session file: path={} error={}",
                    path.display(),
                    e
                ),
            }
        }
        debug!(
            "Loaded session files: count={} duration_ms={}",
            loaded,
            started_at.elapsed().as_millis()
        );
        self.rebuild_operation_index();
        Ok(())
    }

    async fn persist_session(&self, session_id: &str) -> SnapshotResult<()> {
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(());
        };
        let path = self.session_file_path(session_id);
        JsonFileStore
            .write_atomic_strict(&path, session)
            .await
            .map_err(|error| {
                SnapshotError::ConfigError(format!(
                    "Failed to persist snapshot session history {}: {error}",
                    path.display()
                ))
            })?;
        Ok(())
    }

    async fn delete_session_file(&self, session_id: &str) -> SnapshotResult<()> {
        let path = self.session_file_path(session_id);
        if path.exists() {
            tokio::fs::remove_file(path)
                .await
                .map_err(SnapshotError::Io)?;
        }
        Ok(())
    }

    fn session_file_path(&self, session_id: &str) -> PathBuf {
        let safe = sanitize_id(session_id);
        self.sessions_dir.join(format!("{}.json", safe))
    }

    fn rebuild_operation_index(&mut self) {
        self.operation_index.clear();
        for (session_id, session) in &self.sessions {
            for (turn_index, turn) in &session.turns {
                for op in &turn.operations {
                    self.operation_index.insert(
                        op.operation_id.clone(),
                        (session_id.clone(), *turn_index, op.seq_in_turn),
                    );
                }
            }
        }
    }
}

fn operation_is_completed_for_session_file(op: &FileOperation) -> bool {
    if let Some(completed) = op.completed {
        return completed;
    }

    if op.after_snapshot_id.is_some() {
        return true;
    }

    if op.operation_type != OperationType::Delete {
        return false;
    }

    op.tool_context.execution_time_ms > 0
        || op.diff_summary.lines_added > 0
        || op.diff_summary.lines_removed > 0
        || op.diff_summary.lines_modified > 0
}

fn operation_is_before(op: &FileOperation, max_turn_exclusive: Option<usize>) -> bool {
    max_turn_exclusive.is_none_or(|end| op.turn_index < end)
}

fn completed_session_operations_for_file<'a>(
    session: &'a SessionHistory,
    file_path: &Path,
    max_turn_exclusive: Option<usize>,
) -> Vec<&'a FileOperation> {
    let mut operations: Vec<&FileOperation> = session
        .all_operations_iter()
        .filter(|op| operation_is_before(op, max_turn_exclusive))
        .filter(|op| SnapshotCore::operation_matches_file_path(op, file_path))
        .filter(|op| operation_is_completed_for_session_file(op))
        .collect();

    operations.sort_by_key(|op| (op.turn_index, op.seq_in_turn));
    operations
}

fn session_file_boundary(
    session: &SessionHistory,
    file_path: &Path,
    max_turn_exclusive: Option<usize>,
) -> Option<SessionFileBoundary> {
    let operations = completed_session_operations_for_file(session, file_path, max_turn_exclusive);
    let first = operations.first()?;
    let last = operations.last()?;

    Some(SessionFileBoundary {
        before_snapshot_id: first.before_snapshot_id.clone(),
        after_snapshot_id: last.after_snapshot_id.clone(),
        file_created_in_session: first.before_snapshot_id.is_none(),
        file_deleted_in_session: last.operation_type == OperationType::Delete
            && last.after_snapshot_id.is_none(),
    })
}

fn aggregate_operations_diff_summary_for_file(
    session: &SessionHistory,
    file_path: &Path,
    max_turn_exclusive: Option<usize>,
) -> DiffSummary {
    let mut out = DiffSummary::default();
    for op in session.all_operations_iter() {
        if operation_is_before(op, max_turn_exclusive)
            && SnapshotCore::operation_matches_file_path(op, file_path)
            && operation_is_completed_for_session_file(op)
        {
            out.lines_added += op.diff_summary.lines_added;
            out.lines_removed += op.diff_summary.lines_removed;
            out.lines_modified += op.diff_summary.lines_modified;
        }
    }
    out
}

fn change_kind_from_session_boundary(boundary: &SessionFileBoundary) -> &'static str {
    if boundary.file_created_in_session {
        "create"
    } else if boundary.file_deleted_in_session {
        "delete"
    } else {
        "modify"
    }
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn unique_paths<I: Iterator<Item = PathBuf>>(iter: I) -> Vec<PathBuf> {
    let mut seen = HashSet::<OsString>::new();
    let mut out = Vec::new();
    for p in iter {
        if seen.insert(p.as_os_str().to_os_string()) {
            out.push(p);
        }
    }
    out
}

fn compute_diff_summary(before: &str, after: &str) -> DiffSummary {
    let diff = similar::TextDiff::from_lines(before, after);
    let mut summary = DiffSummary::default();
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Delete => summary.lines_removed += 1,
            similar::ChangeTag::Insert => summary.lines_added += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    summary
}

fn compute_anchor_line(before: &str, after: &str) -> Option<usize> {
    let diff = similar::TextDiff::from_lines(before, after);
    let mut new_line: usize = 1;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Equal => {
                new_line = new_line.saturating_add(1);
            }
            similar::ChangeTag::Insert => {
                return Some(new_line.max(1));
            }
            similar::ChangeTag::Delete => {
                return Some(new_line.max(1));
            }
        }
    }
    None
}

fn split_lines_preserve_trailing(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

fn find_anchor_in_current(
    op_after: &str,
    current_after: &str,
    op_anchor_line: usize,
) -> Option<usize> {
    if current_after.is_empty() {
        return Some(1);
    }

    let op_lines = split_lines_preserve_trailing(op_after);
    let current_lines = split_lines_preserve_trailing(current_after);
    let op_len = op_lines.len().max(1);
    let current_len = current_lines.len().max(1);

    let anchor_idx = op_anchor_line
        .saturating_sub(1)
        .min(op_len.saturating_sub(1));
    let start = anchor_idx.saturating_sub(1);
    let end = (anchor_idx + 2).min(op_lines.len());
    let context = &op_lines[start..end];

    if !context.is_empty()
        && context.iter().any(|l| !l.is_empty())
        && context.len() <= current_lines.len()
    {
        for i in 0..=current_lines.len().saturating_sub(context.len()) {
            if &current_lines[i..i + context.len()] == context {
                return Some(i + 1);
            }
        }
    }

    let anchor_line_text = op_lines.get(anchor_idx).copied().unwrap_or_default();
    if !anchor_line_text.is_empty() {
        for (i, line) in current_lines.iter().enumerate() {
            if *line == anchor_line_text {
                return Some(i + 1);
            }
        }
    }

    Some(op_anchor_line.min(current_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::session::revert::{
        SessionRevertPhase, SessionRevertState, SESSION_REVERT_SCHEMA_VERSION,
    };
    use crate::service::snapshot::snapshot_system::FileSnapshotSystem;
    use crate::service::workspace_runtime::{WorkspaceRuntimeContext, WorkspaceRuntimeTarget};
    use serde_json::json;
    use std::fs;

    struct TestRuntime {
        core: SnapshotCore,
        root: PathBuf,
        workspace: PathBuf,
    }

    impl Drop for TestRuntime {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    async fn make_test_runtime(name: &str) -> TestRuntime {
        let root = std::env::temp_dir().join(format!(
            "openbitfun_snapshot_core_{}_{}",
            name,
            Uuid::new_v4()
        ));
        let workspace = root.join("workspace");
        let runtime_root = root.join("runtime");
        fs::create_dir_all(&workspace).unwrap();

        let runtime_context = WorkspaceRuntimeContext::new(
            WorkspaceRuntimeTarget::LocalWorkspace {
                workspace_root: workspace.clone(),
            },
            runtime_root,
        );
        for dir in runtime_context.required_directories() {
            fs::create_dir_all(dir).unwrap();
        }

        let snapshot_system = FileSnapshotSystem::new(runtime_context.clone());
        let mut core = SnapshotCore::new(runtime_context, snapshot_system);
        core.initialize().await.unwrap();

        TestRuntime {
            core,
            root,
            workspace,
        }
    }

    #[tokio::test]
    async fn session_file_diff_stats_use_completed_session_snapshots_not_current_workspace() {
        let mut runtime = make_test_runtime("session_snapshots").await;
        let file_path = runtime.workspace.join("src/lib.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        tokio::fs::write(&file_path, "base\n").await.unwrap();

        let operation_id = runtime
            .core
            .start_file_operation(
                "session-1",
                0,
                file_path.clone(),
                OperationType::Modify,
                "Edit".to_string(),
                json!({ "file_path": "src/lib.rs" }),
                None,
            )
            .await
            .unwrap();
        tokio::fs::write(&file_path, "base\nsession\n")
            .await
            .unwrap();
        runtime
            .core
            .complete_file_operation("session-1", &operation_id, 1)
            .await
            .unwrap();

        tokio::fs::write(&file_path, "base\nsession\noutside\noutside2\n")
            .await
            .unwrap();

        let stats = runtime
            .core
            .get_session_file_diff_stats("session-1", &file_path)
            .await
            .unwrap();
        assert_eq!(stats.lines_added, 1);
        assert_eq!(stats.lines_removed, 0);
        assert_eq!(stats.change_kind, "modify");

        let (before, after) = runtime
            .core
            .get_file_diff(&file_path, "session-1")
            .await
            .unwrap();
        assert_eq!(before, "base\n");
        assert_eq!(after, "base\nsession\n");
    }

    #[tokio::test]
    async fn bounded_session_view_hides_suffix_operations_from_every_projection() {
        let mut runtime = make_test_runtime("bounded_session_view").await;
        let file_path = runtime.workspace.join("src/lib.rs");
        let hidden_only_path = runtime.workspace.join("src/hidden.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        tokio::fs::write(&file_path, "base\n").await.unwrap();

        let visible_operation = runtime
            .core
            .start_file_operation(
                "session-1",
                0,
                file_path.clone(),
                OperationType::Modify,
                "Edit".to_string(),
                json!({}),
                None,
            )
            .await
            .unwrap();
        tokio::fs::write(&file_path, "visible\n").await.unwrap();
        runtime
            .core
            .complete_file_operation("session-1", &visible_operation, 1)
            .await
            .unwrap();

        let hidden_operation = runtime
            .core
            .start_file_operation(
                "session-1",
                1,
                file_path.clone(),
                OperationType::Modify,
                "Edit".to_string(),
                json!({}),
                None,
            )
            .await
            .unwrap();
        tokio::fs::write(&file_path, "hidden\n").await.unwrap();
        runtime
            .core
            .complete_file_operation("session-1", &hidden_operation, 1)
            .await
            .unwrap();

        let hidden_only_operation = runtime
            .core
            .start_file_operation(
                "session-1",
                1,
                hidden_only_path.clone(),
                OperationType::Create,
                "Write".to_string(),
                json!({}),
                None,
            )
            .await
            .unwrap();
        tokio::fs::write(&hidden_only_path, "hidden only\n")
            .await
            .unwrap();
        runtime
            .core
            .complete_file_operation("session-1", &hidden_only_operation, 1)
            .await
            .unwrap();

        assert_eq!(
            runtime.core.get_session_turns_before("session-1", Some(1)),
            vec![0]
        );
        assert!(runtime
            .core
            .get_turn_files_before("session-1", 1, Some(1))
            .is_empty());
        assert_eq!(
            runtime.core.get_session_files_before("session-1", Some(1)),
            vec![file_path.clone()]
        );
        assert_eq!(
            runtime
                .core
                .get_session_operations_before("session-1", Some(1))
                .len(),
            1
        );
        let stats = runtime.core.get_session_stats_before("session-1", Some(1));
        assert_eq!(
            (stats.total_turns, stats.total_files, stats.total_changes),
            (1, 1, 1)
        );

        let (before, after) = runtime
            .core
            .get_file_diff_before(&file_path, "session-1", Some(1))
            .await
            .unwrap();
        assert_eq!((before.as_str(), after.as_str()), ("base\n", "visible\n"));
        assert!(matches!(
            runtime
                .core
                .get_file_diff_with_anchor_before(
                    &file_path,
                    "session-1",
                    Some(&hidden_operation),
                    Some(1),
                )
                .await,
            Err(SnapshotError::OperationNotFound(_))
        ));
    }

    #[tokio::test]
    async fn session_files_ignore_unfinished_operations() {
        let mut runtime = make_test_runtime("unfinished_ops").await;
        let file_path = runtime.workspace.join("src/lib.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        tokio::fs::write(&file_path, "base\n").await.unwrap();

        runtime
            .core
            .start_file_operation(
                "session-1",
                0,
                file_path.clone(),
                OperationType::Modify,
                "Edit".to_string(),
                json!({ "file_path": "src/lib.rs" }),
                None,
            )
            .await
            .unwrap();
        tokio::fs::write(&file_path, "base\noutside\n")
            .await
            .unwrap();

        assert!(runtime.core.get_session_files("session-1").is_empty());

        let stats = runtime
            .core
            .get_session_file_diff_stats("session-1", &file_path)
            .await
            .unwrap();
        assert_eq!(stats.lines_added, 0);
        assert_eq!(stats.lines_removed, 0);
    }

    #[tokio::test]
    async fn legacy_checkpoint_cleanup_preserves_an_operation_owned_snapshot() {
        let mut runtime = make_test_runtime("legacy_checkpoint_owner").await;
        let file_path = runtime.workspace.join("src/lib.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        tokio::fs::write(&file_path, "base\n").await.unwrap();
        let operation_id = runtime
            .core
            .start_file_operation(
                "session-1",
                0,
                file_path.clone(),
                OperationType::Modify,
                "Edit".to_string(),
                json!({ "file_path": "src/lib.rs" }),
                None,
            )
            .await
            .unwrap();
        let snapshot_id = runtime
            .core
            .get_operation("session-1", &operation_id)
            .unwrap()
            .before_snapshot_id
            .expect("before snapshot");
        let legacy_state = SessionRevertState {
            schema_version: SESSION_REVERT_SCHEMA_VERSION,
            boundary_turn: 0,
            original_turn_end: 1,
            phase: SessionRevertPhase::Staged,
            workspace_checkpoint: vec![SessionWorkspaceCheckpoint {
                path: file_path,
                snapshot_id: Some(snapshot_id.clone()),
            }],
        };

        runtime
            .core
            .delete_workspace_revert_checkpoint(&legacy_state)
            .await
            .expect("legacy checkpoint cleanup");

        assert_eq!(
            runtime
                .core
                .snapshot_system
                .get_snapshot_content(&snapshot_id)
                .await
                .expect("operation snapshot must survive"),
            "base\n"
        );
    }

    #[tokio::test]
    async fn staged_workspace_revert_moves_between_boundaries_and_restores_original_state() {
        let mut runtime = make_test_runtime("staged_revert").await;
        let file_path = runtime.workspace.join("src/lib.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        tokio::fs::write(&file_path, "base\n").await.unwrap();

        // A -> B -> A exercises content deduplication: the Session checkpoint
        // must not borrow and later delete the first operation's A snapshot.
        for (turn_index, next) in [(0, "first\n"), (2, "base\n")] {
            let operation_id = runtime
                .core
                .start_file_operation(
                    "session-1",
                    turn_index,
                    file_path.clone(),
                    OperationType::Modify,
                    "Edit".to_string(),
                    json!({ "file_path": "src/lib.rs" }),
                    None,
                )
                .await
                .unwrap();
            tokio::fs::write(&file_path, next).await.unwrap();
            runtime
                .core
                .complete_file_operation("session-1", &operation_id, 1)
                .await
                .unwrap();
        }

        let mut state = SessionRevertState {
            schema_version: SESSION_REVERT_SCHEMA_VERSION,
            boundary_turn: 2,
            original_turn_end: 3,
            phase: SessionRevertPhase::Staged,
            workspace_checkpoint: Vec::new(),
        };
        runtime
            .core
            .prepare_workspace_revert("session-1", &mut state)
            .await
            .expect("latest workspace state should be checkpointed");
        runtime
            .core
            .apply_workspace_revert("session-1", &state)
            .await
            .expect("latest turn should be reverted");
        assert_eq!(
            tokio::fs::read_to_string(&file_path).await.unwrap(),
            "first\n"
        );

        state.boundary_turn = 0;
        runtime
            .core
            .prepare_workspace_revert("session-1", &mut state)
            .await
            .expect("earlier affected files should join the checkpoint");
        runtime
            .core
            .apply_workspace_revert("session-1", &state)
            .await
            .expect("all turns should be reverted");
        assert_eq!(
            tokio::fs::read_to_string(&file_path).await.unwrap(),
            "base\n"
        );

        state.boundary_turn = 2;
        runtime
            .core
            .apply_workspace_revert("session-1", &state)
            .await
            .expect("redo should advance to the later boundary");
        assert_eq!(
            tokio::fs::read_to_string(&file_path).await.unwrap(),
            "first\n"
        );

        runtime
            .core
            .restore_workspace_revert(&state)
            .await
            .expect("final redo should restore the pre-undo workspace");
        assert_eq!(
            tokio::fs::read_to_string(&file_path).await.unwrap(),
            "base\n"
        );

        runtime
            .core
            .delete_workspace_revert_checkpoint(&state)
            .await
            .expect("full redo should release only its owned checkpoint handle");

        let mut repeated = SessionRevertState {
            schema_version: SESSION_REVERT_SCHEMA_VERSION,
            boundary_turn: 0,
            original_turn_end: 3,
            phase: SessionRevertPhase::Staged,
            workspace_checkpoint: Vec::new(),
        };
        runtime
            .core
            .prepare_workspace_revert("session-1", &mut repeated)
            .await
            .expect("a later undo should create a fresh checkpoint");
        runtime
            .core
            .apply_workspace_revert("session-1", &repeated)
            .await
            .expect("operation snapshots must survive a previous checkpoint release");
        assert_eq!(
            tokio::fs::read_to_string(&file_path).await.unwrap(),
            "base\n"
        );
    }
}
