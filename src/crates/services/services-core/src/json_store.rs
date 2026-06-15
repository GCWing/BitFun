//! Generic JSON file store with best-effort atomic writes.
//!
//! This module owns reusable local JSON file IO. Product/session code should
//! keep schema decisions outside this module and use it only for file-level
//! read/write behavior.

use log::{debug, warn};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::sync::Mutex;

const JSON_WRITE_MAX_RETRIES: usize = 5;
const JSON_WRITE_RETRY_BASE_DELAY_MS: u64 = 30;

static JSON_FILE_WRITE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum JsonFileStoreError {
    #[error("Target path has no parent directory: {path}")]
    NoParentDirectory { path: PathBuf },
    #[error("Failed to read JSON metadata {path}: {source}")]
    ReadMetadata {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read JSON file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to deserialize JSON file {path}: {source}")]
    Deserialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Failed to create parent directory: {source}")]
    CreateParent {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to serialize JSON: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("Failed to write temp JSON file: {source}")]
    WriteTemp {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed fallback JSON overwrite {path}: {source}")]
    FallbackOverwrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to replace JSON file: {source}")]
    Replace {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to replace JSON file {path}: unknown error")]
    ReplaceUnknown { path: PathBuf },
}

impl JsonFileStoreError {
    pub fn is_deserialization(&self) -> bool {
        matches!(self, Self::Deserialize { .. })
    }

    pub fn is_serialization(&self) -> bool {
        matches!(self, Self::Serialize { .. })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct JsonFileStore;

impl JsonFileStore {
    pub async fn read_optional<T: DeserializeOwned>(
        &self,
        path: &Path,
    ) -> Result<Option<T>, JsonFileStoreError> {
        let started_at = Instant::now();
        let metadata_started_at = Instant::now();
        let metadata = match fs::metadata(path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(JsonFileStoreError::ReadMetadata {
                    path: path.to_path_buf(),
                    source: error,
                });
            }
        };
        let metadata_duration = metadata_started_at.elapsed();

        let read_started_at = Instant::now();
        let content =
            fs::read_to_string(path)
                .await
                .map_err(|source| JsonFileStoreError::Read {
                    path: path.to_path_buf(),
                    source,
                })?;
        let read_duration = read_started_at.elapsed();

        let parse_started_at = Instant::now();
        let value = serde_json::from_str::<T>(&content).map_err(|source| {
            JsonFileStoreError::Deserialize {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let parse_duration = parse_started_at.elapsed();
        let total_duration = started_at.elapsed();

        if total_duration >= Duration::from_millis(80) || metadata.len() >= 1024 * 1024 {
            debug!(
                "Read JSON file: path={} type={} size_bytes={} metadata_duration_ms={} read_duration_ms={} parse_duration_ms={} total_duration_ms={}",
                path.display(),
                std::any::type_name::<T>(),
                metadata.len(),
                metadata_duration.as_millis(),
                read_duration.as_millis(),
                parse_duration.as_millis(),
                total_duration.as_millis()
            );
        }

        Ok(Some(value))
    }

    pub async fn write_atomic<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), JsonFileStoreError> {
        let parent = path
            .parent()
            .ok_or_else(|| JsonFileStoreError::NoParentDirectory {
                path: path.to_path_buf(),
            })?;

        fs::create_dir_all(parent)
            .await
            .map_err(|source| JsonFileStoreError::CreateParent { source })?;

        let json = serde_json::to_string_pretty(value)
            .map_err(|source| JsonFileStoreError::Serialize { source })?;
        let lock = Self::get_file_write_lock(path).await;
        let _lock_guard = lock.lock().await;

        let json_bytes = json.into_bytes();
        let mut last_replace_error: Option<std::io::Error> = None;

        for attempt in 0..=JSON_WRITE_MAX_RETRIES {
            let tmp_path = Self::build_temp_json_path(path, attempt)?;
            if let Err(source) = fs::write(&tmp_path, &json_bytes).await {
                return Err(JsonFileStoreError::WriteTemp { source });
            }

            match Self::replace_file_from_temp(path, &tmp_path).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let should_retry =
                        Self::is_retryable_write_error(&error) && attempt < JSON_WRITE_MAX_RETRIES;
                    last_replace_error = Some(error);
                    let _ = fs::remove_file(&tmp_path).await;

                    if should_retry {
                        tokio::time::sleep(Self::retry_delay(attempt)).await;
                        continue;
                    }

                    break;
                }
            }
        }

        if let Some(error) = last_replace_error {
            // On Windows, external scanners/file indexers may temporarily hold a
            // non-shareable handle, making delete/rename fail with
            // PermissionDenied. Fallback to direct write to avoid losing session
            // persistence while keeping best-effort atomic behavior.
            if error.kind() == ErrorKind::PermissionDenied {
                warn!(
                    "Atomic JSON replace permission denied for {}, fallback to direct overwrite",
                    path.display()
                );
                fs::write(path, &json_bytes).await.map_err(|source| {
                    JsonFileStoreError::FallbackOverwrite {
                        path: path.to_path_buf(),
                        source,
                    }
                })?;
                return Ok(());
            }

            return Err(JsonFileStoreError::Replace { source: error });
        }

        Err(JsonFileStoreError::ReplaceUnknown {
            path: path.to_path_buf(),
        })
    }

    async fn get_file_write_lock(path: &Path) -> Arc<Mutex<()>> {
        let registry = JSON_FILE_WRITE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut registry_guard = registry.lock().await;
        registry_guard
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn build_temp_json_path(path: &Path, attempt: usize) -> Result<PathBuf, JsonFileStoreError> {
        let parent = path
            .parent()
            .ok_or_else(|| JsonFileStoreError::NoParentDirectory {
                path: path.to_path_buf(),
            })?;

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "data.json".to_string());
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_name = format!(
            ".{}.{}.{}.{}.tmp",
            file_name,
            std::process::id(),
            nonce,
            attempt
        );
        Ok(parent.join(temp_name))
    }

    async fn replace_file_from_temp(target_path: &Path, tmp_path: &Path) -> std::io::Result<()> {
        if let Ok(()) = fs::rename(tmp_path, target_path).await {
            return Ok(());
        }

        if target_path.exists() {
            match fs::remove_file(target_path).await {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }

        fs::rename(tmp_path, target_path).await
    }

    fn is_retryable_write_error(error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            ErrorKind::PermissionDenied
                | ErrorKind::WouldBlock
                | ErrorKind::Interrupted
                | ErrorKind::TimedOut
                | ErrorKind::AlreadyExists
                | ErrorKind::Other
        )
    }

    fn retry_delay(attempt: usize) -> Duration {
        let exp = attempt.min(6) as u32;
        Duration::from_millis(JSON_WRITE_RETRY_BASE_DELAY_MS * (1u64 << exp))
    }
}
