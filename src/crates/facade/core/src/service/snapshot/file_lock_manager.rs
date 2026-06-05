use crate::service::snapshot::types::{SnapshotError, SnapshotResult};
use crate::service::workspace_runtime::WorkspaceRuntimeContext;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::sync::RwLock;

/// File lock info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLock {
    pub session_id: String,
    pub locked_at: SystemTime,
    pub operation_type: String,
    pub tool_name: String,
}

/// Waiting queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitingQueueItem {
    pub session_id: String,
    pub requested_at: SystemTime,
    pub tool_name: String,
}

/// File lock state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLockStatus {
    pub locks: HashMap<String, FileLock>,
    pub waiting_queue: HashMap<String, Vec<WaitingQueueItem>>,
}

/// File lock manager - provides a minimal file locking mechanism for the snapshot system.
pub struct FileLockManager {
    locks: RwLock<HashMap<PathBuf, FileLock>>,
    waiting_queue: RwLock<HashMap<PathBuf, Vec<WaitingQueueItem>>>,
    runtime_context: WorkspaceRuntimeContext,
}

impl FileLockManager {
    /// Creates a new file lock manager.
    pub fn new(runtime_context: WorkspaceRuntimeContext) -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            waiting_queue: RwLock::new(HashMap::new()),
            runtime_context,
        }
    }

    /// Initializes the file lock manager.
    pub async fn initialize(&self) -> SnapshotResult<()> {
        info!("Initializing file lock manager");

        self.load_lock_state().await?;

        info!("File lock manager initialized");
        Ok(())
    }

    /// Tries to acquire a file lock.
    pub async fn try_acquire_lock(
        &self,
        file_path: &PathBuf,
        session_id: &str,
        tool_name: &str,
    ) -> SnapshotResult<bool> {
        let mut locks = self.locks.write().await;

        if let Some(existing_lock) = locks.get(file_path) {
            if existing_lock.session_id == session_id {
                debug!(
                    "Session re-acquiring file lock: session_id={} file_path={}",
                    session_id,
                    file_path.display()
                );
                return Ok(true);
            }

            debug!("File locked by another session, adding to waiting queue: file_path={} session_id={}", file_path.display(), session_id);
            self.add_to_waiting_queue(file_path, session_id, tool_name)
                .await?;
            return Ok(false);
        }

        let lock = FileLock {
            session_id: session_id.to_string(),
            locked_at: SystemTime::now(),
            operation_type: "file_modification".to_string(),
            tool_name: tool_name.to_string(),
        };

        locks.insert(file_path.clone(), lock);

        self.save_lock_state().await?;

        info!(
            "Acquired file lock: session_id={} file_path={}",
            session_id,
            file_path.display()
        );
        Ok(true)
    }

    /// Releases a file lock.
    pub async fn release_lock(&self, file_path: &PathBuf, session_id: &str) -> SnapshotResult<()> {
        let mut locks = self.locks.write().await;

        if let Some(existing_lock) = locks.get(file_path) {
            if existing_lock.session_id != session_id {
                return Err(SnapshotError::ConfigError(format!(
                    "Attempt to release lock not belonging to current session: {} vs {}",
                    existing_lock.session_id, session_id
                )));
            }
        } else {
            warn!(
                "Attempted to release non-existent lock: file_path={}",
                file_path.display()
            );
            return Ok(());
        }

        locks.remove(file_path);

        self.process_waiting_queue(file_path).await?;

        self.save_lock_state().await?;

        info!(
            "Released file lock: session_id={} file_path={}",
            session_id,
            file_path.display()
        );
        Ok(())
    }

    /// Returns the file lock status.
    pub async fn get_lock_status(&self, file_path: &PathBuf) -> Option<FileLock> {
        let locks = self.locks.read().await;
        locks.get(file_path).cloned()
    }

    /// Returns all locks held by a session.
    pub async fn get_session_locks(&self, session_id: &str) -> Vec<(PathBuf, FileLock)> {
        let locks = self.locks.read().await;
        locks
            .iter()
            .filter(|(_, lock)| lock.session_id == session_id)
            .map(|(path, lock)| (path.clone(), lock.clone()))
            .collect()
    }

    /// Releases all locks held by a session.
    pub async fn release_session_locks(&self, session_id: &str) -> SnapshotResult<usize> {
        let file_paths: Vec<PathBuf> = {
            let locks = self.locks.read().await;
            locks
                .iter()
                .filter(|(_, lock)| lock.session_id == session_id)
                .map(|(path, _)| path.clone())
                .collect()
        };

        let mut released_count = 0;
        for file_path in file_paths {
            self.release_lock(&file_path, session_id).await?;
            released_count += 1;
        }

        info!(
            "Released {} file locks for session: session_id={}",
            released_count, session_id
        );
        Ok(released_count)
    }

    /// Returns the full lock status.
    pub async fn get_full_lock_status(&self) -> FileLockStatus {
        let locks = self.locks.read().await;
        let waiting_queue = self.waiting_queue.read().await;

        let locks_map: HashMap<String, FileLock> = locks
            .iter()
            .map(|(path, lock)| (path.to_string_lossy().to_string(), lock.clone()))
            .collect();

        let queue_map: HashMap<String, Vec<WaitingQueueItem>> = waiting_queue
            .iter()
            .map(|(path, items)| (path.to_string_lossy().to_string(), items.clone()))
            .collect();

        FileLockStatus {
            locks: locks_map,
            waiting_queue: queue_map,
        }
    }

    /// Adds an item to the waiting queue.
    async fn add_to_waiting_queue(
        &self,
        file_path: &Path,
        session_id: &str,
        tool_name: &str,
    ) -> SnapshotResult<()> {
        let mut waiting_queue = self.waiting_queue.write().await;

        let queue_item = WaitingQueueItem {
            session_id: session_id.to_string(),
            requested_at: SystemTime::now(),
            tool_name: tool_name.to_string(),
        };

        waiting_queue
            .entry(file_path.to_path_buf())
            .or_insert_with(Vec::new)
            .push(queue_item);

        Ok(())
    }

    /// Processes the waiting queue.
    async fn process_waiting_queue(&self, file_path: &PathBuf) -> SnapshotResult<()> {
        let mut waiting_queue = self.waiting_queue.write().await;

        if let Some(queue) = waiting_queue.get_mut(file_path) {
            if let Some(next_item) = queue.first() {
                debug!(
                    "Notifying next session in waiting queue: session_id={}",
                    next_item.session_id
                );
            }

            if queue.is_empty() {
                waiting_queue.remove(file_path);
            }
        }

        Ok(())
    }

    /// Loads lock state.
    async fn load_lock_state(&self) -> SnapshotResult<()> {
        let locks_file = self.runtime_context.locks_dir.join("file_locks.json");

        if !locks_file.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&locks_file)?;
        let lock_status: FileLockStatus = serde_json::from_str(&content)?;

        let mut locks = self.locks.write().await;
        let mut waiting_queue = self.waiting_queue.write().await;

        for (path_str, lock) in lock_status.locks {
            locks.insert(PathBuf::from(path_str), lock);
        }

        for (path_str, items) in lock_status.waiting_queue {
            waiting_queue.insert(PathBuf::from(path_str), items);
        }

        debug!("Loaded {} file locks", locks.len());
        Ok(())
    }

    /// Saves lock state.
    async fn save_lock_state(&self) -> SnapshotResult<()> {
        let lock_status = self.get_full_lock_status().await;
        let locks_file = self.runtime_context.locks_dir.join("file_locks.json");

        let content = serde_json::to_string_pretty(&lock_status)?;
        std::fs::write(&locks_file, content)?;

        Ok(())
    }

    /// Cleans up expired waiting queue items.
    pub async fn cleanup_expired_queue_items(
        &self,
        max_wait_minutes: u64,
    ) -> SnapshotResult<usize> {
        let mut waiting_queue = self.waiting_queue.write().await;
        let cutoff_time = SystemTime::now() - std::time::Duration::from_secs(max_wait_minutes * 60);
        let mut cleaned_count = 0;

        for (_, queue) in waiting_queue.iter_mut() {
            let original_len = queue.len();
            queue.retain(|item| item.requested_at > cutoff_time);
            cleaned_count += original_len - queue.len();
        }

        waiting_queue.retain(|_, queue| !queue.is_empty());

        if cleaned_count > 0 {
            info!("Cleaned up {} expired waiting queue items", cleaned_count);
        }

        Ok(cleaned_count)
    }
}
