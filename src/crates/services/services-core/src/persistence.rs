use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use tokio::fs;
use tokio::sync::Mutex;

static FILE_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

async fn get_file_lock(path: &Path) -> Arc<Mutex<()>> {
    let mut locks = FILE_LOCKS.lock().await;
    locks
        .entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

pub struct PersistenceService {
    base_dir: PathBuf,
}

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
    pub fn from_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub async fn new(base_dir: PathBuf) -> Result<Self, String> {
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir).await.map_err(|e| {
                format!(
                    "Failed to create storage directory {}: {}",
                    base_dir.display(),
                    e
                )
            })?;
        }

        Ok(Self { base_dir })
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub async fn save_json<T: Serialize>(
        &self,
        key: &str,
        data: &T,
        options: StorageOptions,
    ) -> Result<(), String> {
        let file_path = self.base_dir.join(format!("{}.json", key));

        let lock = get_file_lock(&file_path).await;
        let _guard = lock.lock().await;

        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
            }
        }

        if options.create_backup && file_path.exists() {
            self.create_backup(&file_path, options.backup_count).await?;
        }

        let json_data = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Serialization failed: {}", e))?;

        let temp_path = file_path.with_extension("json.tmp");

        fs::write(&temp_path, &json_data)
            .await
            .map_err(|e| format!("Failed to write temp file: {}", e))?;

        fs::rename(&temp_path, &file_path).await.map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            format!("Failed to rename temp file: {}", e)
        })?;

        Ok(())
    }

    pub async fn load_json<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Result<Option<T>, String> {
        let file_path = self.base_dir.join(format!("{}.json", key));

        if !file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&file_path)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let data: T =
            serde_json::from_str(&content).map_err(|e| format!("Deserialization failed: {}", e))?;

        Ok(Some(data))
    }

    pub async fn delete(&self, key: &str) -> Result<bool, String> {
        let json_path = self.base_dir.join(format!("{}.json", key));

        if json_path.exists() {
            fs::remove_file(&json_path)
                .await
                .map_err(|e| format!("Failed to delete JSON file: {}", e))?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn create_backup(&self, file_path: &Path, max_backups: usize) -> Result<(), String> {
        let backup_dir = self.base_dir.join("backups");
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir)
                .await
                .map_err(|e| format!("Failed to create backup directory: {}", e))?;
        }

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "Invalid file name".to_string())?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("{}_{}", timestamp, file_name);
        let backup_path = backup_dir.join(backup_name);

        fs::copy(file_path, &backup_path)
            .await
            .map_err(|e| format!("Failed to create backup: {}", e))?;

        self.cleanup_old_backups(&backup_dir, file_name, max_backups)
            .await?;

        Ok(())
    }

    async fn cleanup_old_backups(
        &self,
        backup_dir: &Path,
        file_pattern: &str,
        max_backups: usize,
    ) -> Result<(), String> {
        let mut backups = Vec::new();
        let mut read_dir = fs::read_dir(backup_dir)
            .await
            .map_err(|e| format!("Failed to read backup directory: {}", e))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read backup entry: {}", e))?
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
