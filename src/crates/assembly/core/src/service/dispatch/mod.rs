#[cfg(feature = "ssh-remote")]
mod controller;
mod target;

use std::path::{Path, PathBuf};

use bitfun_services_core::json_store::{JsonFileStore, JsonFileStoreError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

use crate::infrastructure::PathManager;

#[cfg(feature = "ssh-remote")]
pub use controller::{
    cancel as cancel_dispatch, install_cli_cancel as cancel_dispatch_cli_install,
    install_cli_poll as poll_dispatch_cli_install, install_cli_start as start_dispatch_cli_install,
    list_jobs as list_dispatch_jobs, list_targets as list_dispatch_targets,
    probe_target as probe_dispatch_target, status as get_dispatch_status,
    submit as submit_dispatch, DispatchConnectionRequest, DispatchInstallPollRequest,
    DispatchInstallStartRequest, DispatchJobRequest, DispatchListJobsRequest,
    DispatchListTargetsRequest, DispatchProbeTargetRequest, DispatchStatusRequest,
    DispatchSubmitRequest, DispatchTargetOption,
};
pub use target::{DispatchTarget, DispatchTargetRequest};

const PROMPT_PREVIEW_CHARS: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundDispatchRecord {
    pub job_id: String,
    pub target: DispatchTarget,
    pub session_id: String,
    pub workspace_path: String,
    pub prompt_preview: String,
    pub last_cursor: u64,
    pub last_state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OutboundDispatchRecord {
    pub fn new(
        job_id: String,
        target: DispatchTarget,
        session_id: String,
        workspace_path: String,
        prompt: &str,
        state: impl Into<String>,
    ) -> Result<Self, DispatchStoreError> {
        validate_id(&job_id)?;
        let now = Utc::now();
        Ok(Self {
            job_id,
            target,
            session_id,
            workspace_path,
            prompt_preview: prompt.chars().take(PROMPT_PREVIEW_CHARS).collect(),
            last_cursor: 0,
            last_state: state.into(),
            created_at: now,
            updated_at: now,
        })
    }
}

#[derive(Debug, Error)]
pub enum DispatchStoreError {
    #[error("Invalid dispatch job id")]
    InvalidJobId,
    #[error("Failed to access outbound dispatch index: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to persist outbound dispatch index: {0}")]
    Json(#[from] JsonFileStoreError),
}

/// Durable observer-only index for jobs submitted to other BitFun processes.
///
/// This store intentionally lives outside every workspace/session directory.
/// Writing a record here must never acquire runtime ownership or create a local
/// backend session.
#[derive(Debug, Clone)]
pub struct OutboundDispatchStore {
    root: PathBuf,
    json_store: JsonFileStore,
}

impl OutboundDispatchStore {
    pub fn new(path_manager: &PathManager) -> Self {
        Self::from_root(
            path_manager
                .bitfun_home_dir()
                .join("dispatch")
                .join("outbound"),
        )
    }

    fn from_root(root: PathBuf) -> Self {
        Self {
            root,
            json_store: JsonFileStore,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_in_root_for_tests(root: PathBuf) -> Self {
        Self::from_root(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Atomically bind a job id to its first outbound record.
    ///
    /// Dispatch submission is idempotent across renderer retries and may race
    /// across multiple controller processes. The first binding wins; callers
    /// must compare the returned record with their requested target/session
    /// before contacting a target.
    pub async fn bind_if_absent(
        &self,
        record: &OutboundDispatchRecord,
    ) -> Result<OutboundDispatchRecord, DispatchStoreError> {
        let path = self.record_path(&record.job_id)?;
        self.ensure_root().await?;
        let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
        if let Some(existing) = self
            .json_store
            .read_optional::<OutboundDispatchRecord>(&path)
            .await?
        {
            return Ok(existing);
        }
        self.json_store.write_atomic_strict(&path, record).await?;
        harden_file_permissions(&path).await?;
        Ok(record.clone())
    }

    pub async fn get(
        &self,
        job_id: &str,
    ) -> Result<Option<OutboundDispatchRecord>, DispatchStoreError> {
        let path = self.record_path(job_id)?;
        Ok(self.json_store.read_optional(&path).await?)
    }

    pub async fn update_progress(
        &self,
        job_id: &str,
        cursor: u64,
        state: impl Into<String>,
    ) -> Result<OutboundDispatchRecord, DispatchStoreError> {
        let path = self.record_path(job_id)?;
        let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
        let mut record = self
            .json_store
            .read_optional::<OutboundDispatchRecord>(&path)
            .await?
            .ok_or_else(|| {
                DispatchStoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("dispatch job {job_id} is not in the outbound index"),
                ))
            })?;
        record.last_cursor = record.last_cursor.max(cursor);
        let next_state = state.into();
        if !is_terminal_state(&record.last_state) || record.last_state == next_state {
            record.last_state = next_state;
        }
        record.updated_at = Utc::now();
        self.json_store.write_atomic_strict(&path, &record).await?;
        harden_file_permissions(&path).await?;
        Ok(record)
    }

    pub async fn list(&self) -> Result<Vec<OutboundDispatchRecord>, DispatchStoreError> {
        let mut entries = match fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut records = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json")
                || !entry.file_type().await?.is_file()
            {
                continue;
            }
            match self
                .json_store
                .read_optional::<OutboundDispatchRecord>(&path)
                .await
            {
                Ok(Some(record)) => records.push(record),
                Ok(None) => {}
                Err(error) => {
                    log::warn!(
                        "Skipping unreadable outbound dispatch record: path={} error={}",
                        path.display(),
                        error
                    );
                }
            }
        }
        records.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.job_id.cmp(&right.job_id))
        });
        Ok(records)
    }

    pub async fn remove(&self, job_id: &str) -> Result<bool, DispatchStoreError> {
        let path = self.record_path(job_id)?;
        let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
        match fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn record_path(&self, job_id: &str) -> Result<PathBuf, DispatchStoreError> {
        validate_id(job_id)?;
        Ok(self.root.join(format!("{job_id}.json")))
    }

    async fn ensure_root(&self) -> Result<(), DispatchStoreError> {
        fs::create_dir_all(&self.root).await?;
        harden_directory_permissions(&self.root).await?;
        Ok(())
    }
}

fn validate_id(value: &str) -> Result<(), DispatchStoreError> {
    if value.is_empty()
        || value.len() > 128
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(DispatchStoreError::InvalidJobId);
    }
    Ok(())
}

fn is_terminal_state(state: &str) -> bool {
    matches!(state, "succeeded" | "failed" | "cancelled")
}

#[cfg(unix)]
async fn harden_directory_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await
}

#[cfg(not(unix))]
async fn harden_directory_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(unix)]
async fn harden_file_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await
}

#[cfg(not(unix))]
async fn harden_file_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> DispatchTarget {
        DispatchTarget::Ssh {
            connection_id: "server-a".to_string(),
            workspace_path: "/srv/app".to_string(),
            display_name: "Build server".to_string(),
        }
    }

    #[tokio::test]
    async fn outbound_index_is_separate_and_cursor_is_monotonic() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = OutboundDispatchStore::from_root(temp.path().join("dispatch/outbound"));
        let record = OutboundDispatchRecord::new(
            "job-1".to_string(),
            target(),
            "session-1".to_string(),
            "/srv/app".to_string(),
            "Summarize the repository",
            "queued",
        )
        .expect("record");

        store.bind_if_absent(&record).await.expect("persist");
        let first = store
            .update_progress("job-1", 42, "running")
            .await
            .expect("first progress");
        let stale = store
            .update_progress("job-1", 12, "running")
            .await
            .expect("stale progress");
        let terminal = store
            .update_progress("job-1", 50, "succeeded")
            .await
            .expect("terminal progress");
        let regressed = store
            .update_progress("job-1", 55, "running")
            .await
            .expect("stale state after terminal");

        assert_eq!(first.last_cursor, 42);
        assert_eq!(stale.last_cursor, 42);
        assert_eq!(terminal.last_state, "succeeded");
        assert_eq!(regressed.last_state, "succeeded");
        assert_eq!(regressed.last_cursor, 55);
        assert_eq!(store.list().await.expect("list").len(), 1);
        assert!(store.root().ends_with("dispatch/outbound"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(store.root())
                    .expect("outbound directory")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(store.root().join("job-1.json"))
                    .expect("outbound record")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    async fn rejects_path_traversal_job_ids() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = OutboundDispatchStore::from_root(temp.path().to_path_buf());
        let error = store.get("../sessions").await.expect_err("must reject");
        assert!(matches!(error, DispatchStoreError::InvalidJobId));
    }

    #[tokio::test]
    async fn concurrent_job_binding_has_one_immutable_winner() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = OutboundDispatchStore::from_root(temp.path().to_path_buf());
        let first = OutboundDispatchRecord::new(
            "job-1".to_string(),
            target(),
            "session-1".to_string(),
            "/srv/app".to_string(),
            "first",
            "submitting",
        )
        .expect("first record");
        let mut conflicting = first.clone();
        conflicting.session_id = "session-2".to_string();

        let (left, right) = tokio::join!(
            store.bind_if_absent(&first),
            store.bind_if_absent(&conflicting)
        );
        let left = left.expect("left bind");
        let right = right.expect("right bind");
        let persisted = store
            .get("job-1")
            .await
            .expect("read")
            .expect("persisted winner");
        assert_eq!(left, persisted);
        assert_eq!(right, persisted);
        assert!(
            persisted == first || persisted == conflicting,
            "the persisted record must be one complete contender"
        );
    }

    #[test]
    fn prompt_preview_is_unicode_safe_and_bounded() {
        let prompt = "界".repeat(PROMPT_PREVIEW_CHARS + 10);
        let record = OutboundDispatchRecord::new(
            "job-1".to_string(),
            target(),
            "session-1".to_string(),
            "/srv/app".to_string(),
            &prompt,
            "queued",
        )
        .expect("record");
        assert_eq!(record.prompt_preview.chars().count(), PROMPT_PREVIEW_CHARS);
    }
}
