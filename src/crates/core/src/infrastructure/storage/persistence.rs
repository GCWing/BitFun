//! Persistence storage service
//!
//! Provides data persistence with JSON support

use crate::infrastructure::{try_get_path_manager_arc, PathManager};
use crate::util::errors::*;
use log::warn;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

/// Global file lock map to prevent concurrent writes to the same file
static FILE_LOCKS: Lazy<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Get or create a lock for the specified file
async fn get_file_lock(path: &Path) -> Arc<Mutex<()>> {
    let mut locks = FILE_LOCKS.lock().await;
    locks
        .entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Persistence service
pub struct PersistenceService {
    base_dir: PathBuf,
    path_manager: Arc<PathManager>,
}

/// Storage options
#[derive(Debug, Clone)]
pub struct StorageOptions {
    pub create_backup: bool,
    pub backup_count: usize,
    pub compress: bool,
}

impl Default for StorageOptions {
    fn default() -> Self {
        Self {
            create_backup: true,
            backup_count: 5,
            compress: false,
        }
    }
}

impl PersistenceService {
    pub async fn new(base_dir: PathBuf) -> BitFunResult<Self> {
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir).await.map_err(|e| {
                BitFunError::service(format!("Failed to create storage directory: {}", e))
            })?;
        }

        let path_manager = try_get_path_manager_arc()?;

        Ok(Self {
            base_dir,
            path_manager,
        })
    }

    pub async fn new_user_level(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        let base_dir = path_manager.user_data_dir();
        path_manager.ensure_dir(&base_dir).await?;

        Ok(Self {
            base_dir,
            path_manager,
        })
    }

    pub async fn new_project_level(
        path_manager: Arc<PathManager>,
        workspace_path: PathBuf,
    ) -> BitFunResult<Self> {
        let base_dir = path_manager.project_root(&workspace_path);
        path_manager.ensure_dir(&base_dir).await?;

        Ok(Self {
            base_dir,
            path_manager,
        })
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    /// Save data as JSON (atomic write + file lock to prevent concurrency issues)
    pub async fn save_json<T: Serialize>(
        &self,
        key: &str,
        data: &T,
        options: StorageOptions,
    ) -> BitFunResult<()> {
        let file_path = self.base_dir.join(format!("{}.json", key));

        let lock = get_file_lock(&file_path).await;
        let _guard = lock.lock().await;

        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    BitFunError::service(format!("Failed to create directory {:?}: {}", parent, e))
                })?;
            }
        }

        if options.create_backup && file_path.exists() {
            self.create_backup(&file_path, options.backup_count).await?;
        }

        let json_data = serde_json::to_string_pretty(data)
            .map_err(|e| BitFunError::service(format!("Serialization failed: {}", e)))?;

        // Use atomic writes: write to a temp file first, then rename to avoid corruption on interruption.
        let temp_path = file_path.with_extension("json.tmp");

        fs::write(&temp_path, &json_data)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to write temp file: {}", e)))?;

        fs::rename(&temp_path, &file_path).await.map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            BitFunError::service(format!("Failed to rename temp file: {}", e))
        })?;

        Ok(())
    }

    pub async fn load_json<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> BitFunResult<Option<T>> {
        let file_path = self.base_dir.join(format!("{}.json", key));

        if !file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&file_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read file: {}", e)))?;

        let data: T = serde_json::from_str(&content)
            .map_err(|e| BitFunError::service(format!("Deserialization failed: {}", e)))?;

        Ok(Some(data))
    }

    pub async fn delete(&self, key: &str) -> BitFunResult<bool> {
        let json_path = self.base_dir.join(format!("{}.json", key));

        if json_path.exists() {
            fs::remove_file(&json_path)
                .await
                .map_err(|e| BitFunError::service(format!("Failed to delete JSON file: {}", e)))?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn create_backup(&self, file_path: &Path, max_backups: usize) -> BitFunResult<()> {
        let backup_dir = self.base_dir.join("backups");
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir).await.map_err(|e| {
                BitFunError::service(format!("Failed to create backup directory: {}", e))
            })?;
        }

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| BitFunError::service("Invalid file name".to_string()))?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("{}_{}", timestamp, file_name);
        let backup_path = backup_dir.join(backup_name);

        fs::copy(file_path, &backup_path)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create backup: {}", e)))?;

        self.cleanup_old_backups(&backup_dir, file_name, max_backups)
            .await?;

        Ok(())
    }

    async fn cleanup_old_backups(
        &self,
        backup_dir: &Path,
        file_pattern: &str,
        max_backups: usize,
    ) -> BitFunResult<()> {
        let mut backups = Vec::new();
        let mut read_dir = fs::read_dir(backup_dir)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read backup directory: {}", e)))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read backup entry: {}", e)))?
        {
            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(file_pattern) {
                    if let Ok(metadata) = entry.metadata().await {
                        if let Ok(modified) = metadata.modified() {
                            backups.push((entry.path(), modified));
                        }
                    }
                }
            }
        }

        backups.sort_by(|a, b| b.1.cmp(&a.1));

        if backups.len() > max_backups {
            for (path, _) in backups.into_iter().skip(max_backups) {
                if let Err(e) = fs::remove_file(&path).await {
                    warn!("Failed to remove old backup {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }
}
