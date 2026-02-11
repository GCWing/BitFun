use crate::service::snapshot::snapshot_system::FileSnapshotSystem;
use crate::service::snapshot::types::{
    DiffSummary, FileOperation, OperationType, SnapshotError, SnapshotResult, ToolContext,
};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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
    pub fn new(bitfun_dir: &Path, snapshot_system: FileSnapshotSystem) -> Self {
        let sessions_dir = bitfun_dir.join("sessions").join("operations");
        Self {
            sessions: HashMap::new(),
            operation_index: HashMap::new(),
            snapshot_system,
            sessions_dir,
        }
    }

    pub async fn initialize(&mut self) -> SnapshotResult<()> {
        info!("Initializing operation history system");
        if !self.sessions_dir.exists() {
            tokio::fs::create_dir_all(&self.sessions_dir)
                .await
                .map_err(SnapshotError::Io)?;
        }

        self.snapshot_system.initialize().await?;
        self.load_all_sessions().await?;
        info!(
            "Operation history system initialized: loaded_sessions={}",
            self.sessions.len()
        );
        Ok(())
    }

    /// Start a file operation (before snapshot), returns operation_id.
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
        let before_snapshot_id = if file_path.exists() {
            Some(self.snapshot_system.create_snapshot(&file_path).await?)
        } else {
            None
        };

        if let Some(before_id) = &before_snapshot_id {
            if !self.snapshot_system.has_baseline(&file_path).await {
                match self
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
                }
            } else {
                debug!(
                    "Baseline snapshot already exists: file_path={:?}",
                    file_path
                );
            }
        }

        let session = self
            .sessions
            .entry(session_id.to_string())
            .or_insert_with(|| SessionHistory::new(session_id.to_string()));
        let turn = session.ensure_turn_mut(turn_index);
        let seq_in_turn = turn.operations.len();
        let operation_id = operation_id_override
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        if self.operation_index.contains_key(&operation_id) {
            return Err(SnapshotError::ConfigError(format!(
                "operation_id already exists: {}",
                operation_id
            )));
        }

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
        let Some((sid, turn_index, seq)) = self.operation_index.get(operation_id).cloned() else {
            return Err(SnapshotError::OperationNotFound(operation_id.to_string()));
        };
        if sid != session_id {
            return Err(SnapshotError::ConfigError(format!(
                "operation_id does not belong to current session: op={} session={} actual={}",
                operation_id, session_id, sid
            )));
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

        let (before_snapshot_id, file_path) = {
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

            let after_snapshot_id = if op.file_path.exists() {
                Some(self.snapshot_system.create_snapshot(&op.file_path).await?)
            } else {
                None
            };
            op.after_snapshot_id = after_snapshot_id;

            (op.before_snapshot_id.clone(), op.file_path.clone())
        };

        let before_text = self.load_snapshot_text(before_snapshot_id.as_deref()).await;
        let after_text = self.load_path_text(&file_path).await;
        let diff_summary = compute_diff_summary(&before_text, &after_text);

        let completed_op = {
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
            session.last_updated = SystemTime::now();
            op.clone()
        };

        self.persist_session(session_id).await?;

        Ok(completed_op)
    }

    pub fn get_session_turns(&self, session_id: &str) -> Vec<usize> {
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        session.turns.keys().cloned().collect()
    }

    pub fn get_turn_files(&self, session_id: &str, turn_index: usize) -> Vec<PathBuf> {
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        let Some(turn) = session.turns.get(&turn_index) else {
            return Vec::new();
        };
        unique_paths(turn.operations.iter().map(|op| op.file_path.clone()))
    }

    pub fn get_session_files(&self, session_id: &str) -> Vec<PathBuf> {
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        unique_paths(session.all_operations_iter().map(|op| op.file_path.clone()))
    }

    pub fn get_session_operations(&self, session_id: &str) -> Vec<FileOperation> {
        let Some(session) = self.sessions.get(session_id) else {
            return Vec::new();
        };
        session.all_operations_iter().cloned().collect()
    }

    pub fn get_all_modified_files(&self) -> Vec<PathBuf> {
        let mut all = Vec::new();
        for session in self.sessions.values() {
            all.extend(session.all_operations_iter().map(|op| op.file_path.clone()));
        }
        unique_paths(all.into_iter())
    }

    pub fn get_session_stats(&self, session_id: &str) -> SessionStats {
        let ops = self.get_session_operations(session_id);
        let total_changes = ops.len();
        let total_files = unique_paths(ops.iter().map(|op| op.file_path.clone())).len();
        let total_turns = self.get_session_turns(session_id).len();
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

        let current_content = if file_path.exists() {
            tokio::fs::read_to_string(file_path)
                .await
                .map_err(SnapshotError::Io)?
        } else {
            String::new()
        };

        Ok((baseline_content, current_content))
    }

    pub async fn get_file_diff(
        &self,
        file_path: &Path,
        session_id: &str,
    ) -> SnapshotResult<(String, String)> {
        let Some(session) = self.sessions.get(session_id) else {
            return Err(SnapshotError::SessionNotFound(session_id.to_string()));
        };

        let load_first_before = || async {
            let first_before = session
                .all_operations_iter()
                .find(|op| op.file_path == file_path)
                .and_then(|op| op.before_snapshot_id.clone());

            let Some(snapshot_id) = first_before else {
                return String::new();
            };

            self.snapshot_system
                .get_snapshot_content(&snapshot_id)
                .await
                .unwrap_or_default()
        };

        let before = if let Some(baseline_id) = self
            .snapshot_system
            .get_baseline_snapshot_id(file_path)
            .await
        {
            debug!(
                "Using baseline snapshot for diff: file_path={:?} baseline_id={}",
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
                        "Failed to read baseline snapshot, falling back to first before snapshot: baseline_id={} error={}",
                        baseline_id, e
                    );
                    load_first_before().await
                }
            }
        } else {
            load_first_before().await
        };

        let after = if file_path.exists() {
            tokio::fs::read_to_string(file_path)
                .await
                .map_err(SnapshotError::Io)?
        } else {
            String::new()
        };

        Ok((before, after))
    }

    pub async fn get_file_diff_with_anchor(
        &self,
        file_path: &Path,
        session_id: &str,
        anchor_operation_id: Option<&str>,
    ) -> SnapshotResult<(String, String, Option<usize>)> {
        let (before, after) = self.get_file_diff(file_path, session_id).await?;

        let Some(operation_id) = anchor_operation_id.filter(|s| !s.is_empty()) else {
            return Ok((before, after, None));
        };

        let op = self.get_operation(session_id, operation_id)?;
        if op.file_path != file_path {
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

    pub fn get_file_change_history(&self, file_path: &Path) -> Vec<FileChangeEntry> {
        let mut entries = Vec::new();
        for session in self.sessions.values() {
            for op in session.all_operations_iter() {
                if op.file_path == file_path {
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
    pub async fn rollback_to_turn(
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

    pub async fn cleanup_session(&mut self, session_id: &str) -> SnapshotResult<()> {
        let snapshot_ids_to_delete: Vec<String> =
            if let Some(session) = self.sessions.get(session_id) {
                session
                    .all_operations_iter()
                    .flat_map(|op| {
                        let mut ids = Vec::new();
                        if let Some(ref id) = op.before_snapshot_id {
                            if !id.starts_with("empty_snapshot_") {
                                ids.push(id.clone());
                            }
                        }
                        if let Some(ref id) = op.after_snapshot_id {
                            if !id.starts_with("empty_snapshot_") {
                                ids.push(id.clone());
                            }
                        }
                        ids
                    })
                    .collect()
            } else {
                Vec::new()
            };

        for snapshot_id in &snapshot_ids_to_delete {
            if let Err(e) = self.snapshot_system.delete_snapshot(snapshot_id).await {
                warn!(
                    "Failed to delete snapshot: snapshot_id={} error={}",
                    snapshot_id, e
                );
            }
        }

        if !snapshot_ids_to_delete.is_empty() {
            info!(
                "Cleaned up {} snapshot files: session_id={}",
                snapshot_ids_to_delete.len(),
                session_id
            );
        }

        self.sessions.remove(session_id);

        self.delete_session_file(session_id).await?;

        self.rebuild_operation_index();

        Ok(())
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
            turn.operations.retain(|op| op.file_path != file_path);
        }
        session.turns.retain(|_, t| !t.operations.is_empty());
        session.last_updated = SystemTime::now();
        self.persist_session(session_id).await?;
        self.rebuild_operation_index();
        Ok(())
    }

    async fn apply_rollback_ops(&self, ops: &[FileOperation]) -> SnapshotResult<Vec<PathBuf>> {
        let mut restored_files: Vec<PathBuf> = Vec::new();

        for op in ops {
            let before_path = op
                .path_before
                .as_ref()
                .unwrap_or(&op.file_path)
                .to_path_buf();
            let after_path = op
                .path_after
                .as_ref()
                .unwrap_or(&op.file_path)
                .to_path_buf();

            if before_path != after_path && after_path.exists() {
                if let Err(e) = tokio::fs::remove_file(&after_path).await {
                    warn!(
                        "Failed to delete after_path: path={} error={}",
                        after_path.display(),
                        e
                    );
                }
            }

            match op.before_snapshot_id.as_deref() {
                None => {
                    if after_path.exists() {
                        if let Err(e) = tokio::fs::remove_file(&after_path).await {
                            warn!(
                                "Failed to delete file: path={} error={}",
                                after_path.display(),
                                e
                            );
                        } else {
                            restored_files.push(after_path.clone());
                        }
                    }
                }
                Some(snapshot_id) if snapshot_id.starts_with("empty_snapshot_") => {
                    if after_path.exists() {
                        let _ = tokio::fs::remove_file(&after_path).await;
                        restored_files.push(after_path.clone());
                    }
                }
                Some(snapshot_id) => {
                    self.snapshot_system
                        .restore_file(snapshot_id, &before_path)
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

    async fn load_path_text(&self, path: &Path) -> String {
        if !path.exists() {
            return String::new();
        }
        tokio::fs::read_to_string(path).await.unwrap_or_default()
    }

    async fn load_all_sessions(&mut self) -> SnapshotResult<()> {
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
        debug!("Loaded {} session files", loaded);
        self.rebuild_operation_index();
        Ok(())
    }

    async fn persist_session(&self, session_id: &str) -> SnapshotResult<()> {
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(());
        };
        let path = self.session_file_path(session_id);
        let data =
            serde_json::to_string_pretty(session).map_err(|e| SnapshotError::Serialization(e))?;
        tokio::fs::write(path, data)
            .await
            .map_err(SnapshotError::Io)?;
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
    let mut seen = HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    for p in iter {
        if seen.insert(p.clone()) {
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
