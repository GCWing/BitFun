#[cfg(feature = "ssh-remote")]
mod controller;
#[cfg(feature = "ssh-remote")]
mod device_controller;
mod target;

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use bitfun_services_core::dispatch_workspace::{
    exact_workspace_matches_manifest, exact_workspace_snapshot_source_fingerprint,
    prepare_exact_workspace_snapshot, prepare_source_workspace_snapshot, sha256_file,
    source_workspace_matches_manifest, source_workspace_snapshot_source_fingerprint,
    WorkspaceSnapshotManifest, WorkspaceSnapshotMetadata, WorkspaceSnapshotSourceFingerprint,
};
use bitfun_services_core::json_store::{JsonFileStore, JsonFileStoreError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
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
    answer as answer_dispatch, append as append_dispatch, apply_result as apply_dispatch_result,
    cancel as cancel_dispatch, install_cli_cancel as cancel_dispatch_cli_install,
    install_cli_poll as poll_dispatch_cli_install,
    install_cli_source_start as start_dispatch_cli_source_build,
    install_cli_start as start_dispatch_cli_install, list_jobs as list_dispatch_jobs,
    list_targets as list_dispatch_targets, probe_target as probe_dispatch_target,
    pull_result as pull_dispatch_result, status as get_dispatch_status, submit as submit_dispatch,
    sync_model_config as sync_dispatch_model_config, DispatchAnswerRequest, DispatchAppendRequest,
    DispatchApplyResultRequest, DispatchConnectionRequest, DispatchInstallPollRequest,
    DispatchInstallStartRequest, DispatchJobRequest, DispatchListJobsRequest,
    DispatchListTargetsRequest, DispatchPermissionReplyKind, DispatchProbeTargetRequest,
    DispatchStatusRequest, DispatchSubmitRequest, DispatchTargetOption,
};
#[cfg(feature = "ssh-remote")]
pub use device_controller::{
    answer_device as answer_device_dispatch, append_device as append_device_dispatch,
    cancel_device as cancel_device_dispatch, list_device_jobs as list_device_dispatch_jobs,
    probe_device as probe_device_dispatch_target,
    pull_device_result as pull_device_dispatch_result, status_device as get_device_dispatch_status,
    submit_device as submit_device_dispatch, DeviceDispatchRpc,
};
pub use target::{DispatchTarget, DispatchTargetRequest, DispatchWorkspaceDeliveryRequest};

const PROMPT_PREVIEW_CHARS: usize = 160;
const OUTBOUND_WORKSPACE_UPLOADS_DIR: &str = ".workspace-uploads";
const OUTBOUND_WORKSPACE_CACHE_DIR: &str = ".workspace-cache";
/// Where pulled result bundles are staged before the user applies them.
pub(super) const OUTBOUND_RESULTS_DIR: &str = ".results";
/// Where the renderer's observer transcript cache lives.
const OUTBOUND_TRANSCRIPTS_DIR: &str = ".transcripts";
const TERMINAL_OUTBOUND_RETENTION_DAYS: i64 = 30;
/// Ceiling for one cached observer transcript.
///
/// A transcript that outgrows this stops being cached rather than growing
/// without bound; the observer then replays that job from the beginning, which
/// is exactly the behavior that existed before the cache.
const MAX_OUTBOUND_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PreparedOutboundWorkspaceSnapshot {
    pub archive_path: PathBuf,
    pub metadata: WorkspaceSnapshotMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutboundWorkspaceSnapshotRecord {
    source_workspace_path: String,
    #[serde(default)]
    capture_mode: DispatchWorkspaceSnapshotCaptureMode,
    metadata: WorkspaceSnapshotMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_fingerprint: Option<WorkspaceSnapshotSourceFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutboundWorkspaceCacheRecord {
    source_workspace_path: String,
    capture_mode: DispatchWorkspaceSnapshotCaptureMode,
    source_fingerprint: WorkspaceSnapshotSourceFingerprint,
    metadata: WorkspaceSnapshotMetadata,
    created_at: DateTime<Utc>,
    last_used_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchWorkspaceSnapshotCaptureMode {
    Source,
    #[default]
    Exact,
}

impl DispatchWorkspaceSnapshotCaptureMode {
    fn cache_key_label(self) -> &'static [u8] {
        match self {
            Self::Source => b"source",
            Self::Exact => b"exact",
        }
    }
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
    /// Controller-side workspace that owns the observer session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_workspace_id: Option<String>,
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
            source_workspace_path: None,
            source_workspace_id: None,
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

    pub fn with_source_workspace(
        mut self,
        source_workspace_path: Option<String>,
        source_workspace_id: Option<String>,
    ) -> Self {
        self.source_workspace_path = source_workspace_path.filter(|value| !value.trim().is_empty());
        self.source_workspace_id = source_workspace_id.filter(|value| !value.trim().is_empty());
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchTranscriptRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchSaveTranscriptRequest {
    pub job_id: String,
    /// Stored verbatim. The controller never interprets the renderer's
    /// projection, so its shape is versioned by the renderer alone.
    ///
    /// `null` erases the cached transcript. That is how deleting a projection
    /// drops its cached content immediately instead of waiting for retention.
    #[serde(default)]
    pub transcript: Option<Value>,
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
                    // Best effort: a stranded bundle is disk waste, not a
                    // correctness problem, and must not keep the expired record
                    // alive forever.
                    if let Err(error) = self.remove_result_bundle(&record.job_id).await {
                        log::warn!(
                            "Failed to remove expired dispatch result bundle: job_id={} error={}",
                            record.job_id,
                            error
                        );
                    }
                    if let Err(error) = self.remove_transcript(&record.job_id).await {
                        log::warn!(
                            "Failed to remove expired dispatch observer transcript: job_id={} error={}",
                            record.job_id,
                            error
                        );
                    }
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
        capture_mode: DispatchWorkspaceSnapshotCaptureMode,
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
        let cache = self.root.join(OUTBOUND_WORKSPACE_CACHE_DIR);
        fs::create_dir_all(&cache).await?;
        harden_directory_permissions(&cache).await?;
        let cache = tokio::task::spawn_blocking(move || cache.canonicalize())
            .await
            .map_err(|error| anyhow::anyhow!("snapshot cache path task failed: {error}"))?
            .map_err(|error| anyhow::anyhow!("resolve snapshot cache directory: {error}"))?;
        if cache.starts_with(&source) {
            anyhow::bail!("snapshot source cannot contain the controller snapshot cache");
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
            if record.source_workspace_path != source_wire || record.capture_mode != capture_mode {
                anyhow::bail!("dispatch jobId is already bound to another workspace snapshot");
            }
            let valid =
                snapshot_archive_is_valid(archive_path.clone(), record.metadata.clone()).await?;
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

        let cache_key = outbound_workspace_cache_key(&source_wire, capture_mode);
        let cache_record_path = cache.join(format!("{cache_key}.json"));
        let cache_archive_path = cache.join(format!("{cache_key}.tar.gz"));
        let _cache_lock = self
            .json_store
            .acquire_cross_process_lock(&cache_record_path)
            .await?;
        let mut cached = match self
            .json_store
            .read_optional::<OutboundWorkspaceCacheRecord>(&cache_record_path)
            .await
        {
            Ok(record) => record,
            Err(error) => {
                log::warn!(
                    "Ignoring unreadable outbound workspace cache record: path={} error={}",
                    cache_record_path.display(),
                    error
                );
                None
            }
        };
        if cached.as_ref().is_some_and(|record| {
            record.source_workspace_path != source_wire || record.capture_mode != capture_mode
        }) {
            log::warn!(
                "Ignoring outbound workspace cache identity mismatch: path={}",
                cache_record_path.display()
            );
            cached = None;
        }
        let cache_manifest_path = cache.join(format!("{cache_key}.manifest.json"));
        let decision_started_at = std::time::Instant::now();
        if let Some(mut cached) = cached {
            let current_fingerprint =
                workspace_source_fingerprint(source.clone(), capture_mode).await?;
            // The fingerprint is metadata-only, so it also reports a change for
            // content-neutral operations such as chmod, a git checkout round
            // trip, or an editor's write-then-rename. Ask the per-file manifest
            // for a second opinion before paying for a full repack and a full
            // retransfer to the target.
            let fingerprint_matched = current_fingerprint == cached.source_fingerprint;
            let reusable = if fingerprint_matched {
                true
            } else {
                match self
                    .cached_workspace_manifest(&cache_manifest_path, &cached.metadata)
                    .await
                {
                    // A cache written before this sidecar existed, or one whose
                    // sidecar no longer belongs to the cached archive, simply
                    // degrades to the previous full-repack behavior.
                    None => false,
                    Some(manifest) => {
                        workspace_matches_manifest(source.clone(), capture_mode, manifest).await?
                    }
                }
            };
            if reusable
                && snapshot_archive_is_valid(cache_archive_path.clone(), cached.metadata.clone())
                    .await?
            {
                replace_snapshot_archive(&cache_archive_path, &archive_path).await?;
                // Adopt the current fingerprint so the next dispatch takes the
                // cheap path instead of rereading the tree every time.
                cached.source_fingerprint = current_fingerprint;
                cached.last_used_at = Utc::now();
                self.json_store
                    .write_atomic_strict(&cache_record_path, &cached)
                    .await?;
                harden_file_permissions(&cache_record_path).await?;
                let record = OutboundWorkspaceSnapshotRecord {
                    source_workspace_path: source_wire,
                    capture_mode,
                    metadata: cached.metadata.clone(),
                    source_fingerprint: Some(cached.source_fingerprint),
                };
                self.json_store
                    .write_atomic_strict(&record_path, &record)
                    .await?;
                harden_file_permissions(&record_path).await?;
                // Reported so the cost of each decision layer is observable
                // before deciding whether per-file delta transfer is worth its
                // protocol change: a "content" reuse is the layer this cache
                // gained, and its elapsed time is what that layer costs.
                log::info!(
                    "Reused cached dispatch workspace snapshot: job_id={job_id} mode={capture_mode:?} matched_by={} bytes={} elapsed_ms={}",
                    if fingerprint_matched {
                        "metadata"
                    } else {
                        "content"
                    },
                    cached.metadata.archive_size,
                    decision_started_at.elapsed().as_millis()
                );
                return Ok(PreparedOutboundWorkspaceSnapshot {
                    archive_path,
                    metadata: cached.metadata,
                });
            }
        }

        remove_file_if_present(&cache_record_path).await?;
        remove_file_if_present(&cache_archive_path).await?;
        remove_file_if_present(&cache_manifest_path).await?;
        let package_source = source.clone();
        let package_archive = archive_path.clone();
        let prepared = tokio::task::spawn_blocking(move || match capture_mode {
            DispatchWorkspaceSnapshotCaptureMode::Source => {
                prepare_source_workspace_snapshot(&package_source, &package_archive)
            }
            DispatchWorkspaceSnapshotCaptureMode::Exact => {
                prepare_exact_workspace_snapshot(&package_source, &package_archive)
            }
        })
        .await
        .map_err(|error| anyhow::anyhow!("snapshot packaging task failed: {error}"))??;
        // The counterpart of the reuse log. These two lines together are what
        // says how often a repack is genuinely earned and how many bytes a
        // delta would have saved.
        log::info!(
            "Repacked dispatch workspace snapshot: job_id={job_id} mode={capture_mode:?} files={} bytes={} elapsed_ms={}",
            prepared.metadata.file_count,
            prepared.metadata.archive_size,
            decision_started_at.elapsed().as_millis()
        );
        harden_file_permissions(&archive_path).await?;
        publish_snapshot_cache_archive(
            &archive_path,
            &cache_archive_path,
            &cache,
            &cache_key,
            job_id,
        )
        .await?;
        // Written before the record so a reader can never observe a cache
        // record that claims a manifest sidecar which is not there yet.
        self.json_store
            .write_atomic(&cache_manifest_path, &prepared.manifest)
            .await?;
        harden_file_permissions(&cache_manifest_path).await?;
        let now = Utc::now();
        let cache_record = OutboundWorkspaceCacheRecord {
            source_workspace_path: source_wire.clone(),
            capture_mode,
            source_fingerprint: prepared.source_fingerprint.clone(),
            metadata: prepared.metadata.clone(),
            created_at: now,
            last_used_at: now,
        };
        self.json_store
            .write_atomic_strict(&cache_record_path, &cache_record)
            .await?;
        harden_file_permissions(&cache_record_path).await?;
        let record = OutboundWorkspaceSnapshotRecord {
            source_workspace_path: source_wire,
            capture_mode,
            metadata: prepared.metadata.clone(),
            source_fingerprint: Some(prepared.source_fingerprint),
        };
        self.json_store
            .write_atomic_strict(&record_path, &record)
            .await?;
        harden_file_permissions(&record_path).await?;
        Ok(PreparedOutboundWorkspaceSnapshot {
            archive_path,
            metadata: prepared.metadata,
        })
    }

    /// Drop a pulled result bundle and its summary.
    ///
    /// Separate from `remove_workspace_snapshot` on purpose: that one runs as
    /// soon as the target durably owns the job, which is long before the user
    /// has had a chance to look at the results.
    pub async fn remove_result_bundle(&self, job_id: &str) -> anyhow::Result<()> {
        validate_id(job_id)?;
        let results = self.root.join(OUTBOUND_RESULTS_DIR);
        for path in [
            results.join(format!("{job_id}.tar.gz")),
            results.join(format!("{job_id}.json")),
        ] {
            match fs::remove_file(&path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    /// Read the renderer's cached observer transcript for one job.
    ///
    /// This is the "UI cache" half of the outbound store: it exists so a
    /// restarted renderer can resume from its persisted cursor instead of
    /// replaying the target's whole event log. The controller stores it
    /// verbatim and never interprets it — the target CLI remains the only owner
    /// of the durable session, and reading this must not create a local one.
    pub async fn read_transcript(&self, job_id: &str) -> anyhow::Result<Option<Value>> {
        let path = self.transcript_path(job_id)?;
        match self.json_store.read_optional::<Value>(&path).await {
            Ok(value) => Ok(value),
            Err(error) => {
                // A damaged cache is not a failure; the observer simply replays
                // the job from the beginning.
                log::warn!(
                    "Ignoring unreadable dispatch observer transcript: job_id={job_id} error={error}"
                );
                Ok(None)
            }
        }
    }

    /// Persist the renderer's observer transcript for one job.
    ///
    /// Returns `false` when the transcript is too large to cache. The previous
    /// entry is then left in place: it pairs an older cursor with the turns for
    /// exactly that cursor, so it stays internally consistent and still saves
    /// the observer part of the replay.
    pub async fn write_transcript(&self, job_id: &str, transcript: &Value) -> anyhow::Result<bool> {
        let path = self.transcript_path(job_id)?;
        let encoded = serde_json::to_vec(transcript).context("encode observer transcript")?;
        if encoded.len() > MAX_OUTBOUND_TRANSCRIPT_BYTES {
            log::debug!(
                "Skipping dispatch observer transcript above the cache limit: job_id={job_id} bytes={}",
                encoded.len()
            );
            return Ok(false);
        }
        let transcripts = self.root.join(OUTBOUND_TRANSCRIPTS_DIR);
        fs::create_dir_all(&transcripts).await?;
        harden_directory_permissions(&transcripts).await?;
        let _lock = self.json_store.acquire_cross_process_lock(&path).await?;
        self.json_store.write_atomic(&path, transcript).await?;
        harden_file_permissions(&path).await?;
        Ok(true)
    }

    pub async fn remove_transcript(&self, job_id: &str) -> anyhow::Result<()> {
        remove_file_if_present(&self.transcript_path(job_id)?).await
    }

    fn transcript_path(&self, job_id: &str) -> Result<PathBuf, DispatchStoreError> {
        validate_id(job_id)?;
        Ok(self
            .root
            .join(OUTBOUND_TRANSCRIPTS_DIR)
            .join(format!("{job_id}.json")))
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

    /// Load the per-file manifest that belongs to a cached snapshot archive.
    ///
    /// The sidecar is only trusted when it re-encodes to the manifest digest the
    /// cached archive was sealed with. That binding is what makes it safe to
    /// hand the archive to a target after comparing against this manifest
    /// instead of repacking. Anything unreadable, unparseable, or mismatched
    /// yields `None`, which degrades to a full repack.
    async fn cached_workspace_manifest(
        &self,
        manifest_path: &Path,
        expected: &WorkspaceSnapshotMetadata,
    ) -> Option<WorkspaceSnapshotManifest> {
        let manifest = match self
            .json_store
            .read_optional::<WorkspaceSnapshotManifest>(manifest_path)
            .await
        {
            Ok(Some(manifest)) => manifest,
            Ok(None) => return None,
            Err(error) => {
                log::warn!(
                    "Ignoring unreadable outbound workspace manifest: path={} error={}",
                    manifest_path.display(),
                    error
                );
                return None;
            }
        };
        let encoded = serde_json::to_vec(&manifest).ok()?;
        let mut digest = Sha256::new();
        digest.update(&encoded);
        let digest = format!("{:x}", digest.finalize());
        if !digest.eq_ignore_ascii_case(&expected.manifest_sha256) {
            log::warn!(
                "Ignoring outbound workspace manifest that does not match its cached archive: \
                 path={}",
                manifest_path.display()
            );
            return None;
        }
        Some(manifest)
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

fn outbound_workspace_cache_key(
    source_workspace_path: &str,
    capture_mode: DispatchWorkspaceSnapshotCaptureMode,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"bitfun-dispatch-outbound-workspace-cache");
    digest.update(capture_mode.cache_key_label());
    digest.update((source_workspace_path.len() as u64).to_le_bytes());
    digest.update(source_workspace_path.as_bytes());
    format!("{:x}", digest.finalize())
}

async fn workspace_source_fingerprint(
    source: PathBuf,
    capture_mode: DispatchWorkspaceSnapshotCaptureMode,
) -> anyhow::Result<WorkspaceSnapshotSourceFingerprint> {
    tokio::task::spawn_blocking(move || match capture_mode {
        DispatchWorkspaceSnapshotCaptureMode::Source => {
            source_workspace_snapshot_source_fingerprint(&source)
        }
        DispatchWorkspaceSnapshotCaptureMode::Exact => {
            exact_workspace_snapshot_source_fingerprint(&source)
        }
    })
    .await
    .map_err(|error| anyhow::anyhow!("snapshot fingerprint task failed: {error}"))?
}

async fn workspace_matches_manifest(
    source: PathBuf,
    capture_mode: DispatchWorkspaceSnapshotCaptureMode,
    manifest: WorkspaceSnapshotManifest,
) -> anyhow::Result<bool> {
    tokio::task::spawn_blocking(move || match capture_mode {
        DispatchWorkspaceSnapshotCaptureMode::Source => {
            source_workspace_matches_manifest(&source, &manifest)
        }
        DispatchWorkspaceSnapshotCaptureMode::Exact => {
            exact_workspace_matches_manifest(&source, &manifest)
        }
    })
    .await
    .map_err(|error| anyhow::anyhow!("snapshot manifest comparison task failed: {error}"))?
}

async fn snapshot_archive_is_valid(
    archive: PathBuf,
    expected: WorkspaceSnapshotMetadata,
) -> anyhow::Result<bool> {
    tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
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
    .map_err(|error| anyhow::anyhow!("snapshot verification task failed: {error}"))?
}

async fn replace_snapshot_archive(source: &Path, destination: &Path) -> anyhow::Result<()> {
    remove_file_if_present(destination).await?;
    if let Err(link_error) = fs::hard_link(source, destination).await {
        if let Err(copy_error) = fs::copy(source, destination).await {
            let _ = fs::remove_file(destination).await;
            anyhow::bail!(
                "copy snapshot archive {} to {} after hard-link failed ({link_error}): \
                 {copy_error}",
                source.display(),
                destination.display()
            );
        }
    }
    harden_file_permissions(destination).await?;
    Ok(())
}

async fn publish_snapshot_cache_archive(
    source: &Path,
    destination: &Path,
    cache_directory: &Path,
    cache_key: &str,
    job_id: &str,
) -> anyhow::Result<()> {
    let staging = cache_directory.join(format!(".{cache_key}.{job_id}.tmp"));
    remove_file_if_present(&staging).await?;
    replace_snapshot_archive(source, &staging).await?;
    remove_file_if_present(destination).await?;
    if let Err(error) = fs::rename(&staging, destination).await {
        let _ = fs::remove_file(&staging).await;
        return Err(error)
            .with_context(|| format!("publish snapshot cache archive {}", destination.display()));
    }
    harden_file_permissions(destination).await?;
    Ok(())
}

async fn remove_file_if_present(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
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
        .expect("record")
        .with_source_workspace(
            Some("/Users/test/projects/BitFun".to_string()),
            Some("workspace-1".to_string()),
        );

        store.bind_if_absent(&record).await.expect("persist");
        let persisted = store
            .get("job-1")
            .await
            .expect("load persisted record")
            .expect("persisted record");
        assert_eq!(
            persisted.source_workspace_path.as_deref(),
            Some("/Users/test/projects/BitFun")
        );
        assert_eq!(
            persisted.source_workspace_id.as_deref(),
            Some("workspace-1")
        );
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
    async fn expired_jobs_do_not_strand_their_result_bundles() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = OutboundDispatchStore::new_in_root_for_tests(temp.path().to_path_buf());
        let results = temp.path().join(OUTBOUND_RESULTS_DIR);
        fs::create_dir_all(&results).await.expect("results dir");
        let bundle = results.join("job-1.tar.gz");
        let summary = results.join("job-1.json");
        fs::write(&bundle, b"bundle").await.expect("bundle");
        fs::write(&summary, b"{}").await.expect("summary");
        // A second job's bundle must survive the first job's cleanup.
        let other = results.join("job-2.tar.gz");
        fs::write(&other, b"other").await.expect("other");

        store.remove_result_bundle("job-1").await.expect("remove");
        assert!(!bundle.exists(), "expired bundle must be removed");
        assert!(!summary.exists(), "expired summary must be removed");
        assert!(other.exists(), "an unrelated job must be untouched");

        // Removing twice is how GC behaves after a partial failure.
        store
            .remove_result_bundle("job-1")
            .await
            .expect("removing an absent bundle is not an error");

        assert!(
            store.remove_result_bundle("../escape").await.is_err(),
            "job ids must stay validated on this path too"
        );
    }

    #[tokio::test]
    async fn observer_transcript_round_trips_and_deletion_is_idempotent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = OutboundDispatchStore::new_in_root_for_tests(temp.path().to_path_buf());
        let transcript = serde_json::json!({
            "schemaVersion": 1,
            "jobId": "job-1",
            "cursor": 4096,
            "dialogTurns": [{ "id": "turn-1" }],
        });

        assert!(
            store
                .write_transcript("job-1", &transcript)
                .await
                .expect("write transcript"),
            "a small transcript must be cached"
        );
        assert_eq!(
            store
                .read_transcript("job-1")
                .await
                .expect("read transcript"),
            Some(transcript),
            "the controller stores the renderer projection verbatim"
        );
        assert_eq!(
            store
                .read_transcript("job-2")
                .await
                .expect("read absent transcript"),
            None,
            "an uncached job replays from the beginning"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = temp
                .path()
                .join(OUTBOUND_TRANSCRIPTS_DIR)
                .join("job-1.json");
            assert_eq!(
                std::fs::metadata(&path)
                    .expect("transcript file")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        store.remove_transcript("job-1").await.expect("remove");
        assert_eq!(
            store
                .read_transcript("job-1")
                .await
                .expect("read removed transcript"),
            None
        );
        store
            .remove_transcript("job-1")
            .await
            .expect("removing an absent transcript is not an error");
        assert!(
            store.remove_transcript("../escape").await.is_err(),
            "job ids must stay validated on this path too"
        );
    }

    #[tokio::test]
    async fn oversized_observer_transcript_keeps_the_previous_cache() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = OutboundDispatchStore::new_in_root_for_tests(temp.path().to_path_buf());
        let cached = serde_json::json!({ "schemaVersion": 1, "cursor": 128 });
        store
            .write_transcript("job-1", &cached)
            .await
            .expect("cache the first transcript");

        let oversized = serde_json::json!({
            "schemaVersion": 1,
            "cursor": 4096,
            "dialogTurns": "x".repeat(MAX_OUTBOUND_TRANSCRIPT_BYTES + 1),
        });
        assert!(
            !store
                .write_transcript("job-1", &oversized)
                .await
                .expect("an oversized transcript is not an error"),
            "a transcript above the ceiling must not be cached"
        );
        assert_eq!(
            store
                .read_transcript("job-1")
                .await
                .expect("read transcript"),
            Some(cached),
            "the older entry stays because its cursor and turns still agree"
        );
    }

    #[tokio::test]
    async fn unreadable_observer_transcript_falls_back_to_full_replay() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = OutboundDispatchStore::new_in_root_for_tests(temp.path().to_path_buf());
        let transcripts = temp.path().join(OUTBOUND_TRANSCRIPTS_DIR);
        fs::create_dir_all(&transcripts)
            .await
            .expect("transcripts dir");
        fs::write(transcripts.join("job-1.json"), b"{\"dialogTurns\": [")
            .await
            .expect("truncated transcript");

        assert_eq!(
            store
                .read_transcript("job-1")
                .await
                .expect("a damaged cache must not fail the observer"),
            None
        );
    }

    #[tokio::test]
    async fn expired_jobs_do_not_strand_their_observer_transcripts() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = OutboundDispatchStore::new_in_root_for_tests(temp.path().to_path_buf());
        let mut expired = OutboundDispatchRecord::new(
            "job-1".to_string(),
            target(),
            "session-1".to_string(),
            "/srv/app".to_string(),
            "Summarize the repository",
            "succeeded",
        )
        .expect("record");
        expired.updated_at = Utc::now() - chrono::Duration::days(TERMINAL_OUTBOUND_RETENTION_DAYS);
        store.bind_if_absent(&expired).await.expect("persist");
        let live = OutboundDispatchRecord::new(
            "job-2".to_string(),
            target(),
            "session-2".to_string(),
            "/srv/app".to_string(),
            "Still running",
            "running",
        )
        .expect("record");
        store.bind_if_absent(&live).await.expect("persist");
        for job_id in ["job-1", "job-2"] {
            store
                .write_transcript(job_id, &serde_json::json!({ "schemaVersion": 1 }))
                .await
                .expect("cache transcript");
        }

        let records = store.list().await.expect("list");

        assert_eq!(
            records
                .iter()
                .map(|record| &record.job_id)
                .collect::<Vec<_>>(),
            vec!["job-2"],
            "retention must drop the expired record"
        );
        assert_eq!(
            store
                .read_transcript("job-1")
                .await
                .expect("read expired transcript"),
            None,
            "the expired job's transcript must go with its record"
        );
        assert!(
            store
                .read_transcript("job-2")
                .await
                .expect("read live transcript")
                .is_some(),
            "a live job must keep its cached transcript"
        );
    }

    #[tokio::test]
    async fn workspace_cache_reuses_unchanged_source_across_dispatch_jobs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("outbound");
        let store = OutboundDispatchStore::new_in_root_for_tests(root.clone());
        let source = temp.path().join("workspace");
        std::fs::create_dir_all(source.join(".git")).expect("repository marker");
        std::fs::create_dir_all(source.join("target")).expect("ignored directory");
        std::fs::write(source.join(".gitignore"), b"target/\n").expect("gitignore");
        std::fs::write(source.join("main.rs"), b"fn main() {}").expect("source");
        std::fs::write(source.join("target/app"), b"first build").expect("ignored output");
        let source_wire = source
            .canonicalize()
            .expect("canonical source")
            .to_string_lossy()
            .to_string();
        let cache_key = outbound_workspace_cache_key(
            &source_wire,
            DispatchWorkspaceSnapshotCaptureMode::Source,
        );
        let cache_archive = root
            .join(OUTBOUND_WORKSPACE_CACHE_DIR)
            .join(format!("{cache_key}.tar.gz"));

        let first = store
            .prepare_workspace_snapshot(
                "job-1",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("first snapshot");
        let first_cache_metadata = std::fs::metadata(&cache_archive).expect("first cached archive");
        store
            .remove_workspace_snapshot("job-1")
            .await
            .expect("remove first job snapshot");
        assert!(
            cache_archive.exists(),
            "removing a completed job must retain the reusable cache"
        );

        std::fs::write(source.join("target/app"), b"a different ignored build")
            .expect("change ignored output");
        let second = store
            .prepare_workspace_snapshot(
                "job-2",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("cached snapshot");
        let second_cache_metadata =
            std::fs::metadata(&cache_archive).expect("reused cached archive");
        assert_eq!(first.metadata, second.metadata);
        assert_eq!(
            first_cache_metadata.modified().expect("first modified"),
            second_cache_metadata.modified().expect("second modified"),
            "a cache hit must not recreate the archive"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                second_cache_metadata.ino(),
                std::fs::metadata(&second.archive_path)
                    .expect("job archive")
                    .ino(),
                "each job should hard-link the immutable cached archive"
            );
        }

        std::fs::write(
            source.join("main.rs"),
            b"fn main() { println!(\"changed\"); }",
        )
        .expect("change included source");
        let third = store
            .prepare_workspace_snapshot(
                "job-3",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("invalidated snapshot");
        assert_ne!(
            second.metadata.archive_sha256, third.metadata.archive_sha256,
            "an included source change must invalidate the cached archive"
        );
    }

    /// Fixture shared by the metadata-churn cache tests.
    ///
    /// Returns the canonical source path, the cache key directory entries, and
    /// a store rooted inside the same temp dir.
    fn snapshot_cache_fixture(
        temp: &tempfile::TempDir,
    ) -> (OutboundDispatchStore, PathBuf, String, PathBuf, PathBuf) {
        let root = temp.path().join("outbound");
        let store = OutboundDispatchStore::new_in_root_for_tests(root.clone());
        let source = temp.path().join("workspace");
        std::fs::create_dir_all(source.join(".git")).expect("repository marker");
        std::fs::create_dir_all(source.join("src")).expect("source directory");
        std::fs::write(source.join(".gitignore"), b"target/\n").expect("gitignore");
        std::fs::write(source.join("src/main.rs"), b"fn main() {}").expect("source");
        let canonical = source.canonicalize().expect("canonical source");
        let source_wire = canonical.to_string_lossy().to_string();
        let cache_key = outbound_workspace_cache_key(
            &source_wire,
            DispatchWorkspaceSnapshotCaptureMode::Source,
        );
        let cache_dir = root.join(OUTBOUND_WORKSPACE_CACHE_DIR);
        (
            store,
            canonical,
            source_wire,
            cache_dir.join(format!("{cache_key}.tar.gz")),
            cache_dir.join(format!("{cache_key}.manifest.json")),
        )
    }

    /// The source fingerprint is metadata-only, so operations that leave every
    /// byte intact still change it: `chmod`, an editor's write-then-rename, a
    /// `git checkout` round trip. Those must not force a full repack and a full
    /// retransfer to the target.
    #[cfg(unix)]
    #[tokio::test]
    async fn workspace_cache_survives_content_neutral_metadata_changes() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let temp = tempfile::tempdir().expect("temp dir");
        let (store, source, source_wire, cache_archive, cache_manifest) =
            snapshot_cache_fixture(&temp);

        let first = store
            .prepare_workspace_snapshot(
                "job-1",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("first snapshot");
        let first_cache_ino = std::fs::metadata(&cache_archive)
            .expect("first cached archive")
            .ino();
        assert!(
            cache_manifest.exists(),
            "packaging must publish the manifest sidecar the comparison relies on"
        );

        // chmod, without touching the executable bit the manifest records.
        std::fs::set_permissions(
            source.join(".gitignore"),
            std::fs::Permissions::from_mode(0o640),
        )
        .expect("chmod");
        // Write-then-rename: identical bytes, brand new inode.
        let staging = temp.path().join("main.rs.tmp");
        std::fs::write(&staging, b"fn main() {}").expect("staging write");
        std::fs::rename(&staging, source.join("src/main.rs")).expect("rename over source");

        let second = store
            .prepare_workspace_snapshot(
                "job-2",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("second snapshot");

        assert_eq!(
            first.metadata, second.metadata,
            "content-neutral churn must reuse the cached snapshot"
        );
        assert_eq!(
            first_cache_ino,
            std::fs::metadata(&cache_archive)
                .expect("reused cached archive")
                .ino(),
            "a content match must not repack the cached archive"
        );

        // The adopted fingerprint is what keeps the next dispatch on the cheap
        // path instead of rehashing the tree every single time.
        let cache_key = outbound_workspace_cache_key(
            &source_wire,
            DispatchWorkspaceSnapshotCaptureMode::Source,
        );
        let cache_record: OutboundWorkspaceCacheRecord = JsonFileStore
            .read_optional(
                &temp
                    .path()
                    .join("outbound")
                    .join(OUTBOUND_WORKSPACE_CACHE_DIR)
                    .join(format!("{cache_key}.json")),
            )
            .await
            .expect("read cache record")
            .expect("cache record present");
        assert_eq!(
            cache_record.source_fingerprint,
            source_workspace_snapshot_source_fingerprint(&source).expect("current fingerprint"),
            "a content match must adopt the current fingerprint"
        );
    }

    /// The structural pass only compares stat data, so an edit that preserves
    /// file size has to be caught by the content pass.
    #[tokio::test]
    async fn workspace_cache_invalidates_on_same_size_content_change() {
        let temp = tempfile::tempdir().expect("temp dir");
        let (store, source, source_wire, _cache_archive, _cache_manifest) =
            snapshot_cache_fixture(&temp);

        std::fs::write(source.join("src/config.rs"), b"const N: u8 = 1;").expect("config");
        let first = store
            .prepare_workspace_snapshot(
                "job-1",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("first snapshot");

        // Same byte count, different content: only the content pass can see it.
        std::fs::write(source.join("src/config.rs"), b"const N: u8 = 2;").expect("same-size edit");

        let second = store
            .prepare_workspace_snapshot(
                "job-2",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("second snapshot");
        assert_ne!(
            first.metadata.archive_sha256, second.metadata.archive_sha256,
            "a same-size content change must still invalidate the cache"
        );
    }

    /// A cache written before the manifest sidecar existed, or one whose
    /// sidecar no longer matches its archive, must fall back to the previous
    /// full-repack behavior rather than reusing an unverified archive.
    #[cfg(unix)]
    #[tokio::test]
    async fn workspace_cache_without_manifest_falls_back_to_repacking() {
        use std::os::unix::fs::MetadataExt;

        let temp = tempfile::tempdir().expect("temp dir");
        let (store, source, source_wire, cache_archive, cache_manifest) =
            snapshot_cache_fixture(&temp);

        store
            .prepare_workspace_snapshot(
                "job-1",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("first snapshot");
        let first_cache_ino = std::fs::metadata(&cache_archive)
            .expect("first cached archive")
            .ino();
        std::fs::remove_file(&cache_manifest).expect("simulate a pre-sidecar cache");

        let staging = temp.path().join("main.rs.tmp");
        std::fs::write(&staging, b"fn main() {}").expect("staging write");
        std::fs::rename(&staging, source.join("src/main.rs")).expect("rename over source");

        store
            .prepare_workspace_snapshot(
                "job-2",
                &source_wire,
                DispatchWorkspaceSnapshotCaptureMode::Source,
            )
            .await
            .expect("second snapshot");
        assert_ne!(
            first_cache_ino,
            std::fs::metadata(&cache_archive)
                .expect("republished cached archive")
                .ino(),
            "a cache with no manifest must repack instead of reusing the archive"
        );
        assert!(
            cache_manifest.exists(),
            "the repack must publish a sidecar so the next dispatch can compare"
        );
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
