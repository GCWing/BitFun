//! Automatic cleanup module
//!
//! Provides storage cleanup policies and scheduling

use crate::infrastructure::PathManager;
use crate::util::errors::*;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPolicy {
    pub temp_retention_days: u64,
    pub log_retention_days: u64,
    pub session_retention_days: u64,
    pub max_cache_size_mb: u64,
    pub backup_retention_count: usize,
    pub auto_cleanup_enabled: bool,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        Self {
            temp_retention_days: 7,
            log_retention_days: 30,
            session_retention_days: 90,
            max_cache_size_mb: 1024,
            backup_retention_count: 10,
            auto_cleanup_enabled: true,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CleanupResult {
    pub files_deleted: usize,
    pub directories_deleted: usize,
    pub bytes_freed: u64,
    pub categories: Vec<CleanupCategory>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CleanupCategory {
    pub name: String,
    pub files_deleted: usize,
    pub bytes_freed: u64,
}

pub struct CleanupService {
    path_manager: PathManager,
    policy: CleanupPolicy,
}

impl CleanupService {
    pub fn new(path_manager: PathManager, policy: CleanupPolicy) -> Self {
        Self {
            path_manager,
            policy,
        }
    }

    pub async fn cleanup_all(&self) -> BitFunResult<CleanupResult> {
        let mut result = CleanupResult::default();

        if !self.policy.auto_cleanup_enabled {
            return Ok(result);
        }

        info!("Starting cleanup process");

        if let Ok(temp_result) = self.cleanup_temp_files().await {
            result.merge(temp_result, "Temporary Files");
        }

        if let Ok(log_result) = self.cleanup_old_logs().await {
            result.merge(log_result, "Old Logs");
        }

        if let Ok(session_result) = self.cleanup_old_sessions().await {
            result.merge(session_result, "Expired Sessions");
        }

        if let Ok(cache_result) = self.cleanup_oversized_cache().await {
            result.merge(cache_result, "Oversized Cache");
        }

        info!(
            "Cleanup completed: {} files, {} dirs, {:.2} MB freed",
            result.files_deleted,
            result.directories_deleted,
            result.bytes_freed as f64 / 1_048_576.0
        );

        Ok(result)
    }

    async fn cleanup_temp_files(&self) -> BitFunResult<CleanupResult> {
        let temp_dir = self.path_manager.temp_dir();
        let retention = Duration::from_secs(self.policy.temp_retention_days * 24 * 3600);

        self.cleanup_old_files(&temp_dir, retention).await
    }

    async fn cleanup_old_logs(&self) -> BitFunResult<CleanupResult> {
        let logs_dir = self.path_manager.logs_dir();
        let retention = Duration::from_secs(self.policy.log_retention_days * 24 * 3600);

        self.cleanup_old_files(&logs_dir, retention).await
    }

    async fn cleanup_old_sessions(&self) -> BitFunResult<CleanupResult> {
        let mut result = CleanupResult::default();

        let workspaces_dir = self.path_manager.workspaces_dir();

        if !workspaces_dir.exists() {
            return Ok(result);
        }

        let retention = Duration::from_secs(self.policy.session_retention_days * 24 * 3600);

        let mut read_dir = fs::read_dir(&workspaces_dir)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read workspaces: {}", e)))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::service(format!("Failed to read workspace entry: {}", e)))?
        {
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                let session_result = self.cleanup_old_files(&entry.path(), retention).await?;
                result.files_deleted += session_result.files_deleted;
                result.directories_deleted += session_result.directories_deleted;
                result.bytes_freed += session_result.bytes_freed;
            }
        }

        Ok(result)
    }

    async fn cleanup_oversized_cache(&self) -> BitFunResult<CleanupResult> {
        let cache_dir = self.path_manager.cache_root();
        let max_size = self.policy.max_cache_size_mb * 1_048_576;

        let current_size = Self::calculate_dir_size(&cache_dir).await?;

        if current_size <= max_size {
            return Ok(CleanupResult::default());
        }

        debug!(
            "Cache size {:.2} MB exceeds limit {:.2} MB, cleaning up",
            current_size as f64 / 1_048_576.0,
            max_size as f64 / 1_048_576.0
        );

        self.cleanup_by_size(&cache_dir, max_size).await
    }

    async fn cleanup_old_files(
        &self,
        dir: &Path,
        retention: Duration,
    ) -> BitFunResult<CleanupResult> {
        let mut result = CleanupResult::default();

        if !dir.exists() {
            return Ok(result);
        }

        let cutoff_time = SystemTime::now()
            .checked_sub(retention)
            .unwrap_or(SystemTime::UNIX_EPOCH);

        self.cleanup_recursively(
            dir,
            |metadata| {
                metadata
                    .modified()
                    .map(|time| time < cutoff_time)
                    .unwrap_or(false)
            },
            &mut result,
        )
        .await?;

        Ok(result)
    }

    async fn cleanup_by_size(&self, dir: &Path, max_size: u64) -> BitFunResult<CleanupResult> {
        let mut result = CleanupResult::default();

        let mut files = Vec::new();
        self.collect_files_with_time(dir, &mut files).await?;

        files.sort_by(|a, b| b.1.cmp(&a.1));

        let mut current_size = 0u64;

        for (path, _, size) in files {
            current_size += size;

            if current_size > max_size {
                match fs::remove_file(&path).await {
                    Ok(_) => {
                        result.files_deleted += 1;
                        result.bytes_freed += size;
                    }
                    Err(e) => {
                        warn!("Failed to delete {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(result)
    }

    fn cleanup_recursively<'a, F>(
        &'a self,
        dir: &'a Path,
        should_delete: F,
        result: &'a mut CleanupResult,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitFunResult<()>> + Send + 'a>>
    where
        F: Fn(&std::fs::Metadata) -> bool + Copy + Send + 'a,
    {
        Box::pin(async move {
            let mut read_dir = match fs::read_dir(dir).await {
                Ok(d) => d,
                Err(_) => return Ok(()),
            };

            while let Some(entry) = read_dir
                .next_entry()
                .await
                .map_err(|e| BitFunError::service(format!("Failed to read entry: {}", e)))?
            {
                let path = entry.path();
                let metadata = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    self.cleanup_recursively(&path, should_delete, result)
                        .await?;

                    if Self::is_empty_dir(&path).await {
                        match fs::remove_dir(&path).await {
                            Ok(_) => {
                                result.directories_deleted += 1;
                            }
                            Err(e) => {
                                warn!("Failed to delete empty dir {:?}: {}", path, e);
                            }
                        }
                    }
                } else if should_delete(&metadata) {
                    let size = metadata.len();
                    match fs::remove_file(&path).await {
                        Ok(_) => {
                            result.files_deleted += 1;
                            result.bytes_freed += size;
                        }
                        Err(e) => {
                            warn!("Failed to delete {:?}: {}", path, e);
                        }
                    }
                }
            }

            Ok(())
        })
    }

    fn collect_files_with_time<'a>(
        &'a self,
        dir: &'a Path,
        files: &'a mut Vec<(PathBuf, SystemTime, u64)>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitFunResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut read_dir = match fs::read_dir(dir).await {
                Ok(d) => d,
                Err(_) => return Ok(()),
            };

            while let Some(entry) = read_dir
                .next_entry()
                .await
                .map_err(|e| BitFunError::service(format!("Failed to read entry: {}", e)))?
            {
                let path = entry.path();
                let metadata = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    self.collect_files_with_time(&path, files).await?;
                } else if let Ok(modified) = metadata.modified() {
                    files.push((path, modified, metadata.len()));
                }
            }

            Ok(())
        })
    }

    fn calculate_dir_size(
        dir: &Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitFunResult<u64>> + Send + '_>> {
        Box::pin(async move {
            let mut total = 0u64;

            let mut read_dir = match fs::read_dir(dir).await {
                Ok(d) => d,
                Err(_) => return Ok(0),
            };

            while let Some(entry) = read_dir
                .next_entry()
                .await
                .map_err(|e| BitFunError::service(format!("Failed to read entry: {}", e)))?
            {
                let metadata = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    total += Self::calculate_dir_size(&entry.path()).await?;
                } else {
                    total += metadata.len();
                }
            }

            Ok(total)
        })
    }

    async fn is_empty_dir(dir: &Path) -> bool {
        match fs::read_dir(dir).await {
            Ok(mut read_dir) => read_dir.next_entry().await.ok().flatten().is_none(),
            Err(_) => false,
        }
    }
}

impl CleanupResult {
    fn merge(&mut self, other: CleanupResult, category_name: &str) {
        self.files_deleted += other.files_deleted;
        self.directories_deleted += other.directories_deleted;
        self.bytes_freed += other.bytes_freed;

        if other.files_deleted > 0 || other.bytes_freed > 0 {
            self.categories.push(CleanupCategory {
                name: category_name.to_string(),
                files_deleted: other.files_deleted,
                bytes_freed: other.bytes_freed,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_policy_default() {
        let policy = CleanupPolicy::default();
        assert_eq!(policy.temp_retention_days, 7);
        assert_eq!(policy.log_retention_days, 30);
        assert!(policy.auto_cleanup_enabled);
    }
}
