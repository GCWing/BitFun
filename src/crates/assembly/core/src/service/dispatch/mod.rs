#[cfg(feature = "ssh-remote")]
mod controller;
#[cfg(feature = "ssh-remote")]
mod device_controller;
mod target;

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use bitfun_services_core::dispatch_workspace::{
    create_exact_workspace_snapshot, sha256_file, WorkspaceSnapshotMetadata,
};
use bitfun_services_core::json_store::{JsonFileStore, JsonFileStoreError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

use crate::infrastructure::PathManager;

/// Result-bundle shapes the desktop layer returns to the renderer.
#[cfg(feature = "ssh-remote")]
pub use bitfun_services_core::dispatch_workspace::{
    WorkspaceResultApplyOutcome, WorkspaceResultConflict, WorkspaceResultConflictReason,
    WorkspaceResultSummary,
};
#[cfg(feature = "ssh-remote")]
pub use controller::{
    answer as answer_dispatch, append as append_dispatch, cancel as cancel_dispatch,
    install_cli_cancel as cancel_dispatch_cli_install,
    install_cli_poll as poll_dispatch_cli_install,
    install_cli_source_start as start_dispatch_cli_source_build,
    install_cli_start as start_dispatch_cli_install,
    list_jobs as list_dispatch_jobs, list_targets as list_dispatch_targets,
    apply_result as apply_dispatch_result, probe_target as probe_dispatch_target,
    pull_result as pull_dispatch_result,
    status as get_dispatch_status,
    submit as submit_dispatch, sync_model_config as sync_dispatch_model_config,
    DispatchAnswerRequest, DispatchApplyResultRequest, DispatchAppendRequest,
    DispatchConnectionRequest, DispatchInstallPollRequest, DispatchInstallStartRequest,
    DispatchJobRequest, DispatchListJobsRequest, DispatchListTargetsRequest,
    DispatchPermissionReplyKind, DispatchProbeTargetRequest, DispatchStatusRequest,
    DispatchSubmitRequest, DispatchTargetOption,
};
#[cfg(feature = "ssh-remote")]
pub use device_controller::{
    answer_device as answer_device_dispatch, append_device as append_device_dispatch,
    cancel_device as cancel_device_dispatch, list_device_jobs as list_device_dispatch_jobs,
    probe_device as probe_device_dispatch_target, status_device as get_device_dispatch_status,
    submit_device as submit_device_dispatch, DeviceDispatchRpc,
};
pub use target::{DispatchTarget, DispatchTargetRequest, DispatchWorkspaceDeliveryRequest};

const PROMPT_PREVIEW_CHARS: usize = 160;
const OUTBOUND_WORKSPACE_UPLOADS_DIR: &str = ".workspace-uploads";
const TERMINAL_OUTBOUND_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Clone)]
pub struct PreparedOutboundWorkspaceSnapshot {
    pub archive_path: PathBuf,
    pub metadata: WorkspaceSnapshotMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutboundWorkspaceSnapshotRecord {
    source_workspace_path: String,
    metadata: WorkspaceSnapshotMetadata,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DispatchTargetJobEntry {
    job_id: String,
    session_id: String,
    state: String,
    #[serde(default)]
    started_at: Option<String>,
    workspace_path: String,
    title: String,
    #[serde(default)]
    agent_type: Option<String>,
    #[serde(default)]
    approval_policy: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundDispatchRecord {
    pub job_id: String,
    pub target: DispatchTarget,
    pub session_id: String,
    pub workspace_path: String,
    pub prompt_preview: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
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
            title: None,
            agent_type: None,
            approval_policy: None,
            model: None,
            last_cursor: 0,
            last_state: state.into(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn with_submission_metadata(
        mut self,
        title: Option<String>,
        agent_type: String,
        approval_policy: String,
        model: Option<String>,
    ) -> Self {
        self.title = title.filter(|value| !value.trim().is_empty());
        self.agent_type = Some(agent_type);
        self.approval_policy = Some(approval_policy);
        self.model = model.filter(|value| !value.trim().is_empty());
        self
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
    /// before asking the target to execute the job.
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
                Ok(Some(record))
                    if is_terminal_state(&record.last_state)
                        && Utc::now()
                            .signed_duration_since(record.updated_at)
                            .num_days()
                            >= TERMINAL_OUTBOUND_RETENTION_DAYS =>
                {
                    if let Err(error) = self.remove_workspace_snapshot(&record.job_id).await {
                        log::warn!(
                            "Failed to remove expired outbound dispatch snapshot: job_id={} error={}",
                            record.job_id,
                            error
                        );
                        records.push(record);
                    } else if let Err(error) = self.remove(&record.job_id).await {
                        log::warn!(
                            "Failed to remove expired outbound dispatch record: job_id={} error={}",
                            record.job_id,
                            error
                        );
                        records.push(record);
                    }
                }
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

    /// Build or reopen the immutable snapshot bound to one outbound job.
    ///
    /// Keeping the verified artifact after an ambiguous submit is essential:
    /// an idempotent retry must not capture a newer local tree and conflict
    /// with the snapshot that the target may already have committed.
    pub async fn prepare_workspace_snapshot(
        &self,
        job_id: &str,
        source_workspace_path: &str,
    ) -> anyhow::Result<PreparedOutboundWorkspaceSnapshot> {
        validate_id(job_id)?;
        let source = std::path::PathBuf::from(source_workspace_path.trim());
        if !source.is_absolute() {
            anyhow::bail!("snapshot sourceWorkspacePath must be absolute");
        }
        let source = tokio::task::spawn_blocking(move || source.canonicalize())
            .await
            .map_err(|error| anyhow::anyhow!("snapshot path task failed: {error}"))?
            .map_err(|error| anyhow::anyhow!("resolve snapshot source: {error}"))?;
        if !source.is_dir() {
            anyhow::bail!("snapshot source is not a directory");
        }
        let source_wire = source
            .to_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow::anyhow!("snapshot source path is not valid UTF-8"))?;
        let uploads = self.root.join(OUTBOUND_WORKSPACE_UPLOADS_DIR);
        fs::create_dir_all(&uploads).await?;
        harden_directory_permissions(&uploads).await?;
        let uploads = tokio::task::spawn_blocking(move || uploads.canonicalize())
            .await
            .map_err(|error| anyhow::anyhow!("snapshot staging path task failed: {error}"))?
            .map_err(|error| anyhow::anyhow!("resolve snapshot staging directory: {error}"))?;
        if uploads.starts_with(&source) {
            anyhow::bail!(
                "snapshot source cannot contain the controller dispatch staging directory"
            );
        }
        let record_path = uploads.join(format!("{job_id}.json"));
        let archive_path = uploads.join(format!("{job_id}.tar.gz"));
        let _lock = self
            .json_store
            .acquire_cross_process_lock(&record_path)
            .await?;

        if let Some(record) = self
            .json_store
            .read_optional::<OutboundWorkspaceSnapshotRecord>(&record_path)
            .await?
        {
            if record.source_workspace_path != source_wire {
                anyhow::bail!(
                    "dispatch jobId is already bound to a snapshot from another source workspace"
                );
            }
            let archive = archive_path.clone();
            let expected = record.metadata.clone();
            let valid = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
                let metadata = match std::fs::symlink_metadata(&archive) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                    Err(error) => return Err(error.into()),
                };
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() != expected.archive_size
                {
                    return Ok(false);
                }
                Ok(sha256_file(&archive)?.eq_ignore_ascii_case(&expected.archive_sha256))
            })
            .await
            .map_err(|error| anyhow::anyhow!("snapshot verification task failed: {error}"))??;
            if valid {
                return Ok(PreparedOutboundWorkspaceSnapshot {
                    archive_path,
                    metadata: record.metadata,
                });
            }
            let _ = fs::remove_file(&record_path).await;
            let _ = fs::remove_file(&archive_path).await;
        } else {
            let _ = fs::remove_file(&archive_path).await;
        }

        let package_source = source.clone();
        let package_archive = archive_path.clone();
        let metadata = tokio::task::spawn_blocking(move || {
            create_exact_workspace_snapshot(&package_source, &package_archive)
        })
        .await
        .map_err(|error| anyhow::anyhow!("snapshot packaging task failed: {error}"))??;
        let record = OutboundWorkspaceSnapshotRecord {
            source_workspace_path: source_wire,
            metadata: metadata.clone(),
        };
        self.json_store
            .write_atomic_strict(&record_path, &record)
            .await?;
        harden_file_permissions(&record_path).await?;
        harden_file_permissions(&archive_path).await?;
        Ok(PreparedOutboundWorkspaceSnapshot {
            archive_path,
            metadata,
        })
    }

    pub async fn remove_workspace_snapshot(&self, job_id: &str) -> anyhow::Result<()> {
        validate_id(job_id)?;
        let uploads = self.root.join(OUTBOUND_WORKSPACE_UPLOADS_DIR);
        let record_path = uploads.join(format!("{job_id}.json"));
        let archive_path = uploads.join(format!("{job_id}.tar.gz"));
        let _lock = self
            .json_store
            .acquire_cross_process_lock(&record_path)
            .await?;
        for path in [record_path, archive_path] {
            match fs::remove_file(&path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
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

async fn adopt_target_jobs(
    store: &OutboundDispatchStore,
    target: &DispatchTarget,
    response: &serde_json::Value,
) -> anyhow::Result<()> {
    let entries: Vec<DispatchTargetJobEntry> =
        serde_json::from_value(response.clone()).context("decode target dispatch job list")?;
    for entry in entries {
        if !matches!(
            entry.state.as_str(),
            "queued" | "running" | "succeeded" | "failed" | "cancelled"
        ) {
            anyhow::bail!("dispatch target returned an invalid job state");
        }
        bitfun_agent_runtime::session_control::validate_session_id(&entry.session_id)
            .map_err(anyhow::Error::msg)?;
        let workspace_path = entry.workspace_path.trim();
        if !target_workspace_path_is_absolute(workspace_path) {
            anyhow::bail!("dispatch target returned an invalid workspace path");
        }
        let resolved_target = match target {
            DispatchTarget::Ssh {
                connection_id,
                display_name,
                ..
            } => DispatchTarget::Ssh {
                connection_id: connection_id.clone(),
                workspace_path: workspace_path.to_string(),
                display_name: display_name.clone(),
            },
            DispatchTarget::Device {
                device_id,
                display_name,
                ..
            } => DispatchTarget::Device {
                device_id: device_id.clone(),
                workspace_path: workspace_path.to_string(),
                display_name: display_name.clone(),
            },
            DispatchTarget::Local => {
                anyhow::bail!("local jobs cannot be adopted as outbound dispatch observers")
            }
        };
        let title = entry
            .title
            .chars()
            .take(PROMPT_PREVIEW_CHARS)
            .collect::<String>();
        if entry
            .approval_policy
            .as_deref()
            .is_some_and(|policy| !matches!(policy, "auto" | "reject-and-report" | "remote"))
        {
            anyhow::bail!("dispatch target returned an invalid approval policy");
        }
        let mut requested = OutboundDispatchRecord::new(
            entry.job_id,
            resolved_target,
            entry.session_id,
            workspace_path.to_string(),
            &title,
            entry.state.clone(),
        )?;
        requested.title = (!title.trim().is_empty()).then_some(title);
        requested.agent_type = entry.agent_type.filter(|value| !value.trim().is_empty());
        requested.approval_policy = entry.approval_policy;
        requested.model = entry.model.filter(|value| !value.trim().is_empty());
        if let Some(started_at) = entry
            .started_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
        {
            requested.created_at = started_at;
            requested.updated_at = started_at;
        }
        let bound = store.bind_if_absent(&requested).await?;
        if bound.session_id != requested.session_id
            || !same_target_identity_for_store(&bound.target, &requested.target)
        {
            anyhow::bail!(
                "dispatch jobId is already bound to another target or session on this controller"
            );
        }
        store
            .update_progress(&requested.job_id, 0, entry.state)
            .await?;
    }
    Ok(())
}

/// Validate a path returned by the target without applying the controller
/// process's host path semantics. The target may run POSIX while the controller
/// runs Windows, or vice versa.
fn target_workspace_path_is_absolute(path: &str) -> bool {
    let path = path.trim();
    if path.starts_with('/') {
        return true;
    }

    let bytes = path.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
    {
        return true;
    }

    let Some(unc_path) = path.strip_prefix(r"\\") else {
        return false;
    };
    let mut components = unc_path
        .split(['\\', '/'])
        .filter(|component| !component.is_empty());
    components.next().is_some() && components.next().is_some()
}

fn same_target_identity_for_store(left: &DispatchTarget, right: &DispatchTarget) -> bool {
    match (left, right) {
        (
            DispatchTarget::Ssh {
                connection_id: left,
                ..
            },
            DispatchTarget::Ssh {
                connection_id: right,
                ..
            },
        ) => left == right,
        (
            DispatchTarget::Device {
                device_id: left, ..
            },
            DispatchTarget::Device {
                device_id: right, ..
            },
        ) => left == right,
        (DispatchTarget::Local, DispatchTarget::Local) => true,
        _ => false,
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

    #[test]
    fn target_workspace_paths_use_target_platform_semantics() {
        assert!(target_workspace_path_is_absolute("/srv/app"));
        assert!(target_workspace_path_is_absolute(r"C:\work\app"));
        assert!(target_workspace_path_is_absolute("D:/work/app"));
        assert!(target_workspace_path_is_absolute(r"\\server\share\app"));

        assert!(!target_workspace_path_is_absolute(""));
        assert!(!target_workspace_path_is_absolute("relative/app"));
        assert!(!target_workspace_path_is_absolute(r"C:relative\app"));
        assert!(!target_workspace_path_is_absolute(r"\root-relative"));
        assert!(!target_workspace_path_is_absolute(r"\\server"));
    }

    #[tokio::test]
    async fn listing_a_target_adopts_observer_records_without_runtime_ownership() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = OutboundDispatchStore::from_root(temp.path().join("outbound"));
        adopt_target_jobs(
            &store,
            &target(),
            &serde_json::json!([{
                "jobId": "job-observed",
                "sessionId": "00000000-0000-4000-8000-000000000001",
                "state": "running",
                "startedAt": "2026-07-28T00:00:00Z",
                "workspacePath": "/srv/canonical-app",
                "title": "Observed task",
                "agentType": "review",
                "approvalPolicy": "remote",
                "model": "target-model"
            }]),
        )
        .await
        .expect("adopt target jobs");

        let record = store
            .get("job-observed")
            .await
            .expect("read observer")
            .expect("observer record");
        assert_eq!(record.last_state, "running");
        assert_eq!(record.title.as_deref(), Some("Observed task"));
        assert_eq!(record.agent_type.as_deref(), Some("review"));
        assert_eq!(record.approval_policy.as_deref(), Some("remote"));
        assert_eq!(record.model.as_deref(), Some("target-model"));
        assert!(matches!(
            record.target,
            DispatchTarget::Ssh { workspace_path, .. }
                if workspace_path == "/srv/canonical-app"
        ));
    }
}
