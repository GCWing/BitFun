//! Target-side Git workspace provisioning and result sync.
//!
//! A dispatch runs in a Git worktree of the controller's repository, checked out
//! here at the exact commit the controller branched from. Objects normally
//! arrive from the shared Git remote; only what the remote does not have —
//! unpushed commits, or every object when the repository has no remote at all —
//! is carried over the wire as a Git bundle.
//!
//! Results travel the same way in reverse: the worktree's commits become a
//! bundle the controller fetches into its own baseline worktree. Because both
//! sides share `base_commit`, that fetch is an ordinary fast-forward rather than
//! a file-by-file overwrite.

use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use base64::Engine as _;
use bitfun_services_core::dispatch_workspace::sha256_file;
use bitfun_services_core::process_manager;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::protocol::{
    DispatchWorkspaceBundleBeginRequest, DispatchWorkspaceBundleBeginResponse,
    DispatchWorkspaceBundleChunkRequest, DispatchWorkspaceBundleChunkResponse,
    DispatchWorkspaceBundleCommitRequest, DispatchWorkspaceBundleCommitResponse,
    DispatchWorkspaceProvisionRequest, DispatchWorkspaceProvisionResponse,
    DispatchWorkspaceSyncChunkRequest, DispatchWorkspaceSyncChunkResponse,
    DispatchWorkspaceSyncRequest, DispatchWorkspaceSyncResponse, DispatchWorkspaceSyncedChange,
    DISPATCH_PROTOCOL_VERSION,
};
use super::store::{
    atomic_write_json, create_private_dir, read_json, remove_file_if_present,
    set_private_file_permissions, sync_directory, DispatchStore, JobLock,
};

const PROVISION_RECORD_FILE: &str = "provision.json";
const PROVISION_OPERATION_FILE: &str = "provision-operation.json";
const BUNDLE_RECORD_FILE: &str = "bundle.json";
const SYNC_OPERATION_FILE: &str = "sync-operation.json";
const INCOMING_BUNDLE_FILE: &str = "incoming.bundle";
const RESULT_BUNDLE_FILE: &str = "result.bundle";
/// Backstop for one fetch from the project's Git remote.
///
/// The fetch used to be unbounded, so a dead transport parked the whole dispatch
/// on a single `git fetch` forever. This is deliberately not a performance
/// budget: a first fetch of a large project legitimately runs for many minutes,
/// and killing one that is still making progress would fall back to shipping the
/// same history over SSH instead — strictly slower than the fetch it replaced.
/// Stalls are caught by [`FETCH_STALL_SECONDS`]; this only has to fire before
/// the controller's own 30-minute workspace-operation ceiling, so that the
/// target reports the failure rather than the controller timing out on a target
/// that is still waiting.
const REMOTE_FETCH_TIMEOUT: Duration = Duration::from_secs(25 * 60);
/// Bytes per second below which an HTTP fetch counts as stalled.
const FETCH_STALL_BYTES_PER_SECOND: u32 = 1024;
/// How long an HTTP fetch may stay under [`FETCH_STALL_BYTES_PER_SECOND`].
///
/// Git aborts the transfer itself once both hold, which is the check that
/// actually wants to be tight: it separates "slow but arriving" from "hung",
/// which a total-time budget cannot tell apart. It only covers HTTP(S) remotes;
/// for SSH remotes the backstop above is the only bound.
const FETCH_STALL_SECONDS: u32 = 60;
/// How often a bounded Git child is checked for exit.
const GIT_WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Short per-job suffix that keeps two dispatches of one project apart, matching
/// the local managed-worktree convention.
const WORKTREE_SUFFIX_CHARS: usize = 8;
/// Upper bound on the readable half of a worktree directory name.
const WORKTREE_LABEL_MAX_CHARS: usize = 48;
const MAX_CHUNK_BYTES: usize = 256 * 1024;
const MAX_CHUNK_BASE64_BYTES: usize = 384 * 1024;
/// Ceiling for one delivered bundle. Generous for source history, small enough
/// that a hostile or broken controller cannot fill the target's disk.
const MAX_BUNDLE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Cap on the change list reported after a sync, so a huge refactor cannot push
/// the response past the smallest host transport envelope.
const MAX_REPORTED_CHANGES: usize = 2_000;
const DEFAULT_SYNC_COMMIT_MESSAGE: &str = "BitFun dispatch result";
/// A freshly spawned child may not have published an inspectable process
/// identity by the first poll. Keep that tiny start window from spawning a
/// duplicate worker while still allowing a dead child to be recovered.
const OPERATION_START_GRACE_SECONDS: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum BundleUploadState {
    Uploading,
    Committing,
    Committed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleUploadRecord {
    protocol_version: u32,
    job_id: String,
    sha256: String,
    size: u64,
    state: BundleUploadState,
    created_at: String,
    #[serde(default)]
    worker_pid: Option<u32>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    updated_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WorkspaceOperationState {
    Pending,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProvisionOperationRecord {
    request: DispatchWorkspaceProvisionRequest,
    state: WorkspaceOperationState,
    #[serde(default)]
    worker_pid: Option<u32>,
    #[serde(default)]
    response: Option<DispatchWorkspaceProvisionResponse>,
    #[serde(default)]
    last_error: Option<String>,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncOperationRecord {
    request: DispatchWorkspaceSyncRequest,
    state: WorkspaceOperationState,
    #[serde(default)]
    worker_pid: Option<u32>,
    #[serde(default)]
    response: Option<DispatchWorkspaceSyncResponse>,
    #[serde(default)]
    last_error: Option<String>,
    /// True only after the retained failure diagnostic was returned to a
    /// controller. A later operation may replace that failed generation once
    /// its worker is also gone.
    #[serde(default)]
    failure_reported: bool,
    updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProvisionRecord {
    protocol_version: u32,
    job_id: String,
    repo_key: String,
    #[serde(default)]
    remote_url: Option<String>,
    base_commit: String,
    branch: String,
    created_at: String,
    /// Resolved checkout directory. Recorded rather than recomputed so the path
    /// stays stable even if the naming rules change under an existing job.
    #[serde(default)]
    workspace_path: Option<String>,
}

/// Touch file that keeps a shared clone from being collected while in use.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RepoCacheRecord {
    #[serde(default)]
    pub(super) remote_url: Option<String>,
    pub(super) created_at: String,
    pub(super) last_used_at: String,
}

pub(crate) fn provision(
    request: DispatchWorkspaceProvisionRequest,
) -> Result<DispatchWorkspaceProvisionResponse> {
    validate_provision(&request)?;
    let store = DispatchStore::open_default()?;
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    create_private_dir(&job_dir)?;
    let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&request.job_id)?)?;
    let operation_path = job_dir.join(PROVISION_OPERATION_FILE);
    let mut operation = match read_optional_json::<ProvisionOperationRecord>(&operation_path)? {
        Some(existing) => {
            if existing.request != request {
                bail!("dispatch job is already bound to a different Git baseline");
            }
            existing
        }
        None => ProvisionOperationRecord {
            request: request.clone(),
            state: WorkspaceOperationState::Pending,
            worker_pid: None,
            response: None,
            last_error: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
    };

    if operation.state == WorkspaceOperationState::Succeeded {
        let response = operation
            .response
            .clone()
            .context("dispatch provision operation has no response")?;
        let bundle_committed =
            read_optional_json::<BundleUploadRecord>(&job_dir.join(BUNDLE_RECORD_FILE))?
                .is_some_and(|record| record.state == BundleUploadState::Committed);
        if !response.needs_bundle || !bundle_committed {
            return Ok(response);
        }
        // The first pass asked for objects and the upload is now committed.
        // Re-run the same immutable request to publish the worktree.
        operation.state = WorkspaceOperationState::Pending;
        operation.worker_pid = None;
        operation.response = None;
        operation.last_error = None;
    } else if operation.state == WorkspaceOperationState::Failed {
        let diagnostic = operation
            .last_error
            .clone()
            .unwrap_or_else(|| "target retained no diagnostic".to_string());
        // Surface this attempt's failure once, but leave a retryable marker so
        // the next controller action can recover from transient Git/IO errors.
        operation.state = WorkspaceOperationState::Pending;
        operation.worker_pid = None;
        operation.response = None;
        operation.last_error = None;
        operation.updated_at = chrono::Utc::now().to_rfc3339();
        atomic_write_json(&operation_path, &operation)?;
        bail!("dispatch workspace provisioning failed: {diagnostic}");
    } else if operation.worker_pid.is_some_and(|pid| {
        workspace_worker_is_active(
            pid,
            "__workspace_provision_run",
            &request.job_id,
            &operation.updated_at,
        )
    }) {
        return Ok(pending_provision_response(&request));
    }

    operation.state = WorkspaceOperationState::Pending;
    operation.worker_pid = None;
    operation.updated_at = chrono::Utc::now().to_rfc3339();
    atomic_write_json(&operation_path, &operation)?;
    match super::runner::spawn_workspace_provision(&request.job_id) {
        Ok(pid) => {
            operation.worker_pid = Some(pid);
            operation.updated_at = chrono::Utc::now().to_rfc3339();
            atomic_write_json(&operation_path, &operation)?;
            Ok(pending_provision_response(&request))
        }
        Err(error) => {
            operation.state = WorkspaceOperationState::Failed;
            operation.last_error = Some(truncate_utf8(&format!("{error:#}")));
            operation.updated_at = chrono::Utc::now().to_rfc3339();
            let _ = atomic_write_json(&operation_path, &operation);
            Err(error)
        }
    }
}

/// Detached half of `workspace-provision`.
pub(crate) fn run_provision(job_id: String) -> Result<()> {
    let store = DispatchStore::open_default()?;
    let job_dir = store.workspace_upload_dir(&job_id)?;
    let operation_path = job_dir.join(PROVISION_OPERATION_FILE);
    {
        let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&job_id)?)?;
        let mut operation: ProvisionOperationRecord = read_json(&operation_path)
            .context("dispatch workspace provision operation was not initialized")?;
        if operation.state == WorkspaceOperationState::Succeeded {
            return Ok(());
        }
        operation.state = WorkspaceOperationState::Running;
        operation.worker_pid = Some(std::process::id());
        operation.updated_at = chrono::Utc::now().to_rfc3339();
        atomic_write_json(&operation_path, &operation)?;
    }

    let request: DispatchWorkspaceProvisionRequest =
        read_json::<ProvisionOperationRecord>(&operation_path)?.request;
    let outcome = provision_in_store(&store, request);
    let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&job_id)?)?;
    let mut operation: ProvisionOperationRecord = read_json(&operation_path)?;
    operation.worker_pid = None;
    operation.updated_at = chrono::Utc::now().to_rfc3339();
    match outcome {
        Ok(response) => {
            operation.state = WorkspaceOperationState::Succeeded;
            operation.response = Some(response);
            operation.last_error = None;
            atomic_write_json(&operation_path, &operation)
        }
        Err(error) => {
            operation.state = WorkspaceOperationState::Failed;
            operation.last_error = Some(truncate_utf8(&format!("{error:#}")));
            atomic_write_json(&operation_path, &operation)?;
            Err(error)
        }
    }
}

fn pending_provision_response(
    request: &DispatchWorkspaceProvisionRequest,
) -> DispatchWorkspaceProvisionResponse {
    DispatchWorkspaceProvisionResponse {
        pending: true,
        provisioned: false,
        needs_bundle: false,
        workspace_path: None,
        base_commit: request.base_commit.clone(),
        branch: request.branch.clone(),
        have_tips: Vec::new(),
        fetch_error: None,
    }
}

fn provision_in_store(
    store: &DispatchStore,
    request: DispatchWorkspaceProvisionRequest,
) -> Result<DispatchWorkspaceProvisionResponse> {
    validate_provision(&request)?;
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let _lock = JobLock::exclusive(&store.workspace_git_operation_lock_path(&request.job_id)?)?;
    create_private_dir(&job_dir)?;

    let record_path = job_dir.join(PROVISION_RECORD_FILE);
    match read_json::<ProvisionRecord>(&record_path) {
        Ok(existing) => ensure_provision_binding(&existing, &request)?,
        Err(_) => {
            atomic_write_json(
                &record_path,
                &ProvisionRecord {
                    protocol_version: request.protocol_version,
                    job_id: request.job_id.clone(),
                    repo_key: request.repo_key.clone(),
                    remote_url: request.remote_url.clone(),
                    base_commit: request.base_commit.clone(),
                    branch: request.branch.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    workspace_path: None,
                },
            )?;
        }
    }

    let _repo_lock = JobLock::exclusive(&store.repo_lock_path(&request.repo_key)?)?;

    // Reuse the recorded path when there is one: a job keeps the directory it
    // was first given, whatever the current naming rules would produce.
    let existing_record: ProvisionRecord = read_json(&record_path)?;
    let worktree_path = match existing_record.workspace_path.as_deref() {
        Some(path) => PathBuf::from(path),
        None => store.worktree_dir(
            &request.repo_key,
            &worktree_directory_name(
                request.project_label.as_deref(),
                request.remote_url.as_deref(),
                &request.job_id,
            ),
        )?,
    };
    if let Some(existing) =
        existing_worktree(&worktree_path, &request.branch, &request.base_commit)?
    {
        return Ok(DispatchWorkspaceProvisionResponse {
            pending: false,
            provisioned: true,
            needs_bundle: false,
            workspace_path: Some(existing),
            base_commit: request.base_commit,
            branch: request.branch,
            have_tips: Vec::new(),
            fetch_error: None,
        });
    }

    let repo = ensure_repository(store, &request.repo_key, request.remote_url.as_deref())?;
    let mut fetch_error = None;
    if request.remote_url.is_some() && !commit_exists(&repo, &request.base_commit)? {
        // A fetch failure is not fatal on its own: the controller can still
        // deliver the missing objects by bundle, which is also the only path for
        // a repository with no remote. It is reported rather than swallowed,
        // because "the target fell back to a full upload" and "the target could
        // not reach the remote" look identical from the controller otherwise.
        if let Err(error) = fetch_base_commit(&repo, &request.base_commit) {
            tracing::warn!("Dispatch target could not fetch from the Git remote: {error:#}");
            fetch_error = Some(truncate_utf8(&format!("{error:#}")));
        }
    }
    if !commit_exists(&repo, &request.base_commit)? {
        // A fetch that died partway leaves its pack under a temporary name, so
        // the bytes are neither usable nor visible to the tips below. Clearing
        // them keeps a repeatedly failing remote from filling the target's disk.
        remove_pack_temporaries(&repo);
        return Ok(DispatchWorkspaceProvisionResponse {
            pending: false,
            provisioned: false,
            needs_bundle: true,
            workspace_path: None,
            base_commit: request.base_commit,
            branch: request.branch,
            have_tips: repository_tips(&repo)?,
            fetch_error,
        });
    }

    let workspace_path =
        create_worktree(&repo, &worktree_path, &request.branch, &request.base_commit)?;
    let mut record: ProvisionRecord = read_json(&record_path)?;
    record.workspace_path = Some(workspace_path.clone());
    atomic_write_json(&record_path, &record)?;
    Ok(DispatchWorkspaceProvisionResponse {
        pending: false,
        provisioned: true,
        needs_bundle: false,
        workspace_path: Some(workspace_path),
        base_commit: request.base_commit,
        branch: request.branch,
        have_tips: Vec::new(),
        fetch_error: None,
    })
}

pub(crate) fn bundle_begin(
    request: DispatchWorkspaceBundleBeginRequest,
) -> Result<DispatchWorkspaceBundleBeginResponse> {
    let store = DispatchStore::open_default()?;
    bundle_begin_in_store(&store, request)
}

fn bundle_begin_in_store(
    store: &DispatchStore,
    request: DispatchWorkspaceBundleBeginRequest,
) -> Result<DispatchWorkspaceBundleBeginResponse> {
    if request.protocol_version != DISPATCH_PROTOCOL_VERSION {
        bail!(
            "unsupported dispatch protocolVersion {}; target requires {}",
            request.protocol_version,
            DISPATCH_PROTOCOL_VERSION
        );
    }
    super::store::validate_id("jobId", &request.job_id)?;
    validate_digest(&request.sha256)?;
    if request.size == 0 || request.size > MAX_BUNDLE_BYTES {
        bail!("dispatch bundle size is outside the target limit");
    }

    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&request.job_id)?)?;
    create_private_dir(&job_dir)?;
    let record_path = job_dir.join(BUNDLE_RECORD_FILE);
    let bundle_path = job_dir.join(INCOMING_BUNDLE_FILE);

    match read_json::<BundleUploadRecord>(&record_path) {
        Ok(mut existing) => {
            ensure_bundle_binding(&existing, &request)?;
            if existing.state == BundleUploadState::Failed {
                // The prior commit poll already received its diagnostic. A new
                // begin is a new controller attempt and may resume the retained
                // verified bytes instead of leaving the job permanently stuck.
                existing.state = BundleUploadState::Uploading;
                existing.worker_pid = None;
                existing.last_error = None;
                existing.updated_at = chrono::Utc::now().to_rfc3339();
                atomic_write_json(&record_path, &existing)?;
            }
            if existing.state == BundleUploadState::Committed {
                return Ok(DispatchWorkspaceBundleBeginResponse {
                    accepted: true,
                    offset: existing.size,
                    committed: true,
                });
            }
        }
        Err(_) => {
            // A begin for different bytes replaces whatever partial upload was
            // there: the digest is what binds an upload, and this one is new.
            remove_file_if_present(&bundle_path);
            atomic_write_json(
                &record_path,
                &BundleUploadRecord {
                    protocol_version: request.protocol_version,
                    job_id: request.job_id.clone(),
                    sha256: request.sha256.to_ascii_lowercase(),
                    size: request.size,
                    state: BundleUploadState::Uploading,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    worker_pid: None,
                    last_error: None,
                    updated_at: chrono::Utc::now().to_rfc3339(),
                },
            )?;
        }
    }

    let offset = match fs::symlink_metadata(&bundle_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("dispatch bundle upload path is not a regular file");
            }
            set_private_file_permissions(&bundle_path)?;
            if metadata.len() > request.size {
                OpenOptions::new()
                    .write(true)
                    .open(&bundle_path)
                    .context("open oversized dispatch bundle upload")?
                    .set_len(0)
                    .context("reset oversized dispatch bundle upload")?;
                0
            } else {
                metadata.len()
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            drop(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&bundle_path)
                    .context("create dispatch bundle upload")?,
            );
            set_private_file_permissions(&bundle_path)?;
            0
        }
        Err(error) => return Err(error).context("inspect dispatch bundle upload"),
    };
    Ok(DispatchWorkspaceBundleBeginResponse {
        accepted: true,
        offset,
        committed: false,
    })
}

pub(crate) fn bundle_chunk(
    request: DispatchWorkspaceBundleChunkRequest,
) -> Result<DispatchWorkspaceBundleChunkResponse> {
    if request.data_base64.len() > MAX_CHUNK_BASE64_BYTES {
        bail!("dispatch bundle chunk exceeds the encoded safety limit");
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(request.data_base64.as_bytes())
        .context("decode dispatch bundle chunk")?;
    if data.is_empty() || data.len() > MAX_CHUNK_BYTES {
        bail!("dispatch bundle chunk must contain 1-{MAX_CHUNK_BYTES} bytes");
    }
    let store = DispatchStore::open_default()?;
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&request.job_id)?)?;
    let record: BundleUploadRecord = read_json(&job_dir.join(BUNDLE_RECORD_FILE))
        .context("dispatch bundle upload was not initialized")?;
    if record.job_id != request.job_id {
        bail!("dispatch bundle upload identity mismatch");
    }
    ensure_bundle_not_failed(&record)?;
    if record.state != BundleUploadState::Uploading {
        bail!("dispatch bundle upload is not accepting chunks");
    }

    let bundle_path = job_dir.join(INCOMING_BUNDLE_FILE);
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&bundle_path)
        .context("open dispatch bundle upload")?;
    set_private_file_permissions(&bundle_path)?;
    let current = file.metadata()?.len();
    let chunk_end = request.offset.saturating_add(data.len() as u64);
    if chunk_end > record.size {
        bail!("dispatch bundle chunk exceeds the declared size");
    }
    if request.offset < current {
        if chunk_end > current {
            bail!("dispatch bundle chunk overlaps the retained tail");
        }
        // A retry that repeats bytes already on disk is accepted only when it
        // repeats them exactly; anything else means two different bundles are
        // racing for one job.
        file.seek(SeekFrom::Start(request.offset))?;
        let mut existing = vec![0_u8; data.len()];
        file.read_exact(&mut existing)?;
        if existing != data {
            bail!("dispatch bundle retry does not match retained bytes");
        }
        return Ok(DispatchWorkspaceBundleChunkResponse {
            accepted: true,
            offset: current,
        });
    }
    if request.offset != current {
        bail!(
            "dispatch bundle offset mismatch: expected {current}, received {}",
            request.offset
        );
    }
    file.seek(SeekFrom::End(0))?;
    file.write_all(&data)?;
    file.sync_data()?;
    Ok(DispatchWorkspaceBundleChunkResponse {
        accepted: true,
        offset: chunk_end,
    })
}

pub(crate) fn bundle_commit(
    request: DispatchWorkspaceBundleCommitRequest,
) -> Result<DispatchWorkspaceBundleCommitResponse> {
    let store = DispatchStore::open_default()?;
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&request.job_id)?)?;
    let record_path = job_dir.join(BUNDLE_RECORD_FILE);
    let mut record: BundleUploadRecord =
        read_json(&record_path).context("dispatch bundle upload was not initialized")?;
    if record.job_id != request.job_id {
        bail!("dispatch bundle upload identity mismatch");
    }
    if record.state == BundleUploadState::Failed {
        let diagnostic = record
            .last_error
            .clone()
            .unwrap_or_else(|| "target did not retain a diagnostic".to_string());
        record.state = BundleUploadState::Uploading;
        record.worker_pid = None;
        record.last_error = None;
        record.updated_at = chrono::Utc::now().to_rfc3339();
        atomic_write_json(&record_path, &record)?;
        bail!("dispatch bundle delivery failed: {diagnostic}");
    }
    if record.state == BundleUploadState::Committed {
        return Ok(DispatchWorkspaceBundleCommitResponse {
            committed: true,
            pending: false,
        });
    }
    if record.state == BundleUploadState::Committing
        && record.worker_pid.is_some_and(|pid| {
            workspace_worker_is_active(
                pid,
                "__workspace_bundle_commit_run",
                &request.job_id,
                &record.updated_at,
            )
        })
    {
        return Ok(DispatchWorkspaceBundleCommitResponse {
            committed: false,
            pending: true,
        });
    }

    record.state = BundleUploadState::Committing;
    record.worker_pid = None;
    record.last_error = None;
    record.updated_at = chrono::Utc::now().to_rfc3339();
    atomic_write_json(&record_path, &record)?;
    match super::runner::spawn_workspace_bundle_commit(&request.job_id) {
        Ok(pid) => {
            record.worker_pid = Some(pid);
            record.updated_at = chrono::Utc::now().to_rfc3339();
            atomic_write_json(&record_path, &record)?;
            Ok(DispatchWorkspaceBundleCommitResponse {
                committed: false,
                pending: true,
            })
        }
        Err(error) => {
            record.state = BundleUploadState::Failed;
            record.last_error = Some(truncate_utf8(&format!("{error:#}")));
            record.updated_at = chrono::Utc::now().to_rfc3339();
            let _ = atomic_write_json(&record_path, &record);
            Err(error)
        }
    }
}

/// Detached half of `workspace-bundle-commit`.
pub(crate) fn run_bundle_commit(job_id: String) -> Result<()> {
    bundle_commit_in_store(
        &DispatchStore::open_default()?,
        DispatchWorkspaceBundleCommitRequest { job_id },
    )
    .map(|_| ())
}

fn bundle_commit_in_store(
    store: &DispatchStore,
    request: DispatchWorkspaceBundleCommitRequest,
) -> Result<DispatchWorkspaceBundleCommitResponse> {
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let _git_lock = JobLock::exclusive(&store.workspace_git_operation_lock_path(&request.job_id)?)?;
    let record_path = job_dir.join(BUNDLE_RECORD_FILE);
    let record: BundleUploadRecord =
        read_json(&record_path).context("dispatch bundle upload was not initialized")?;
    if record.job_id != request.job_id {
        bail!("dispatch bundle upload identity mismatch");
    }
    ensure_bundle_not_failed(&record)?;
    if record.state == BundleUploadState::Committed {
        return Ok(DispatchWorkspaceBundleCommitResponse {
            committed: true,
            pending: false,
        });
    }

    let provision: ProvisionRecord = read_json(&job_dir.join(PROVISION_RECORD_FILE))
        .context("dispatch bundle requires a provisioned job")?;
    let _repo_lock = JobLock::exclusive(&store.repo_lock_path(&provision.repo_key)?)?;
    let bundle_path = job_dir.join(INCOMING_BUNDLE_FILE);
    let outcome = (|| -> Result<()> {
        let metadata = fs::symlink_metadata(&bundle_path).context("inspect dispatch bundle")?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!("dispatch bundle is not a regular file");
        }
        if metadata.len() != record.size {
            bail!(
                "dispatch bundle is incomplete: expected {} bytes, received {}",
                record.size,
                metadata.len()
            );
        }
        if !sha256_file(&bundle_path)?.eq_ignore_ascii_case(&record.sha256) {
            bail!("dispatch bundle SHA-256 mismatch");
        }
        let repo = ensure_repository(store, &provision.repo_key, provision.remote_url.as_deref())?;
        // `git bundle verify` checks the bundle's own integrity and that every
        // prerequisite commit is already present, so a bundle that would leave
        // a broken history is rejected before it touches the object store.
        git(&repo, &["bundle", "verify", path_arg(&bundle_path)?])
            .context("verify dispatch bundle")?;
        git(
            &repo,
            &[
                "fetch",
                "--no-tags",
                path_arg(&bundle_path)?,
                &format!("+refs/heads/{0}:refs/heads/{0}", provision.branch),
            ],
        )
        .context("fetch dispatch bundle into the target repository")?;
        if !commit_exists(&repo, &provision.base_commit)? {
            bail!("dispatch bundle did not deliver the requested base commit");
        }
        Ok(())
    })();

    let _state_lock = JobLock::exclusive(&store.workspace_operation_lock_path(&request.job_id)?)?;
    // Re-read after the long Git operation so a poller's atomic state update is
    // never overwritten by a stale in-memory copy.
    let mut record: BundleUploadRecord = read_json(&record_path)?;
    match outcome {
        Ok(()) => {
            record.state = BundleUploadState::Committed;
            record.worker_pid = None;
            record.updated_at = chrono::Utc::now().to_rfc3339();
            atomic_write_json(&record_path, &record)?;
            // The objects live in the repository now; the transfer artifact is
            // pure duplication.
            remove_file_if_present(&bundle_path);
            Ok(DispatchWorkspaceBundleCommitResponse {
                committed: true,
                pending: false,
            })
        }
        Err(error) => {
            record.state = BundleUploadState::Failed;
            record.worker_pid = None;
            record.last_error = Some(truncate_utf8(&format!("{error:#}")));
            record.updated_at = chrono::Utc::now().to_rfc3339();
            let _ = atomic_write_json(&record_path, &record);
            Err(error)
        }
    }
}

/// Commit whatever the agent changed and package it as a bundle.
///
/// Read-only with respect to the controller: it only ever adds commits on the
/// job's own branch, so a controller that never syncs leaves no trace here.
pub(crate) fn sync(request: DispatchWorkspaceSyncRequest) -> Result<DispatchWorkspaceSyncResponse> {
    let store = DispatchStore::open_default()?;
    start_sync_in_store(&store, request, super::runner::spawn_workspace_sync)
}

fn start_sync_in_store<F>(
    store: &DispatchStore,
    request: DispatchWorkspaceSyncRequest,
    spawn_sync: F,
) -> Result<DispatchWorkspaceSyncResponse>
where
    F: FnOnce(&str) -> Result<u32>,
{
    super::store::validate_id("jobId", &request.job_id)?;
    super::store::validate_id("operationId", &request.operation_id)?;
    if request
        .known_head
        .as_deref()
        .is_some_and(|head| validate_commit(head).is_err())
    {
        bail!("dispatch knownHead must be a full 40-character commit id");
    }
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&request.job_id)?)?;
    let provision: ProvisionRecord = read_json(&job_dir.join(PROVISION_RECORD_FILE))
        .context("this job did not receive a Git workspace")?;
    let operation_path = job_dir.join(SYNC_OPERATION_FILE);
    let mut operation = match read_optional_json::<SyncOperationRecord>(&operation_path)? {
        // Polls from one controller invocation must observe its durable result,
        // especially a clean result whose head is identical to `knownHead`.
        // Checking this before the generation boundary prevents every poll
        // from reopening that completed no-op.
        Some(existing) if existing.request.operation_id == request.operation_id => {
            if existing.request != request {
                bail!("dispatch operationId is already bound to a different sync request");
            }
            existing
        }
        // Before the controller records a changed head, a retry uses a new
        // operation id but the old known head. Return the retained bundle
        // response instead of starting over so transfer/apply failures remain
        // safely retryable.
        Some(existing)
            if existing.state == WorkspaceOperationState::Succeeded
                && existing
                    .response
                    .as_ref()
                    .is_some_and(|response| response.changed)
                && existing.request.known_head == request.known_head =>
        {
            existing
        }
        // An acknowledged head is the generation boundary. A later click gets
        // a new operation id and intentionally re-opens the operation so a
        // still-running agent's newer commits can be discovered.
        Some(existing)
            if existing.state == WorkspaceOperationState::Succeeded
                && request.known_head.as_deref()
                    == existing
                        .response
                        .as_ref()
                        .map(|response| response.head_commit.as_str()) =>
        {
            SyncOperationRecord {
                request: request.clone(),
                state: WorkspaceOperationState::Pending,
                worker_pid: None,
                response: None,
                last_error: None,
                failure_reported: false,
                updated_at: chrono::Utc::now().to_rfc3339(),
            }
        }
        // A failed generation remains durable until its diagnostic has been
        // returned. Older v3 journals did not carry operationId, so the first
        // current request acts as that final poll before a retry can take over.
        Some(existing)
            if existing.state == WorkspaceOperationState::Failed
                && existing.request.operation_id.is_empty()
                && !existing.failure_reported =>
        {
            existing
        }
        // Replacing an abandoned generation is safe only while holding this
        // job's operation lock and after its detached worker is verifiably
        // gone. Pending/Running without a live worker covers process crashes
        // and the failure-consumption state written by early v3 builds.
        Some(existing) if sync_operation_can_be_replaced(&existing, &request.job_id) => {
            new_sync_operation(request.clone())
        }
        Some(_) => bail!("dispatch sync is already bound to a different request"),
        None => new_sync_operation(request.clone()),
    };

    match operation.state {
        WorkspaceOperationState::Succeeded => {
            return operation
                .response
                .context("dispatch sync operation has no response")
        }
        WorkspaceOperationState::Failed => {
            let diagnostic = operation
                .last_error
                .clone()
                .unwrap_or_else(|| "target retained no diagnostic".to_string());
            if !operation.failure_reported {
                operation.failure_reported = true;
                operation.updated_at = chrono::Utc::now().to_rfc3339();
                atomic_write_json(&operation_path, &operation)?;
            }
            bail!("dispatch workspace sync failed: {diagnostic}");
        }
        WorkspaceOperationState::Pending | WorkspaceOperationState::Running
            if operation.worker_pid.is_some_and(|pid| {
                workspace_worker_is_active(
                    pid,
                    "__workspace_sync_run",
                    &request.job_id,
                    &operation.updated_at,
                )
            }) =>
        {
            return Ok(pending_sync_response(&provision));
        }
        WorkspaceOperationState::Pending | WorkspaceOperationState::Running => {}
    }

    operation.state = WorkspaceOperationState::Pending;
    operation.worker_pid = None;
    operation.response = None;
    operation.last_error = None;
    operation.failure_reported = false;
    operation.updated_at = chrono::Utc::now().to_rfc3339();
    atomic_write_json(&operation_path, &operation)?;
    match spawn_sync(&request.job_id) {
        Ok(pid) => {
            operation.worker_pid = Some(pid);
            operation.updated_at = chrono::Utc::now().to_rfc3339();
            atomic_write_json(&operation_path, &operation)?;
            Ok(pending_sync_response(&provision))
        }
        Err(error) => {
            operation.state = WorkspaceOperationState::Failed;
            operation.last_error = Some(truncate_utf8(&format!("{error:#}")));
            // This synchronous failure is returned by the current call, so it
            // does not need one more poll before a new operation can retry.
            operation.failure_reported = true;
            operation.updated_at = chrono::Utc::now().to_rfc3339();
            let _ = atomic_write_json(&operation_path, &operation);
            Err(error)
        }
    }
}

/// Detached half of `workspace-sync`.
pub(crate) fn run_sync(job_id: String) -> Result<()> {
    let store = DispatchStore::open_default()?;
    let job_dir = store.workspace_upload_dir(&job_id)?;
    let operation_path = job_dir.join(SYNC_OPERATION_FILE);
    let request = {
        let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&job_id)?)?;
        let mut operation: SyncOperationRecord = read_json(&operation_path)
            .context("dispatch workspace sync operation was not initialized")?;
        if operation.state == WorkspaceOperationState::Succeeded {
            return Ok(());
        }
        operation.state = WorkspaceOperationState::Running;
        operation.worker_pid = Some(std::process::id());
        operation.updated_at = chrono::Utc::now().to_rfc3339();
        let request = operation.request.clone();
        atomic_write_json(&operation_path, &operation)?;
        request
    };

    let outcome = sync_in_store(&store, request.clone());
    let _lock = JobLock::exclusive(&store.workspace_operation_lock_path(&job_id)?)?;
    let mut operation: SyncOperationRecord = read_json(&operation_path)?;
    // A replacement is allowed only after the previous worker is gone, but
    // retain a generation fence as defense in depth against stale/corrupt PIDs.
    if operation.request != request {
        return Ok(());
    }
    operation.worker_pid = None;
    operation.updated_at = chrono::Utc::now().to_rfc3339();
    match outcome {
        Ok(response) => {
            operation.state = WorkspaceOperationState::Succeeded;
            operation.response = Some(response);
            operation.last_error = None;
            operation.failure_reported = false;
            atomic_write_json(&operation_path, &operation)
        }
        Err(error) => {
            operation.state = WorkspaceOperationState::Failed;
            operation.last_error = Some(truncate_utf8(&format!("{error:#}")));
            operation.failure_reported = false;
            atomic_write_json(&operation_path, &operation)?;
            Err(error)
        }
    }
}

fn new_sync_operation(request: DispatchWorkspaceSyncRequest) -> SyncOperationRecord {
    SyncOperationRecord {
        request,
        state: WorkspaceOperationState::Pending,
        worker_pid: None,
        response: None,
        last_error: None,
        failure_reported: false,
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn sync_operation_can_be_replaced(operation: &SyncOperationRecord, job_id: &str) -> bool {
    let worker_is_active = operation.worker_pid.is_some_and(|pid| {
        workspace_worker_is_active(pid, "__workspace_sync_run", job_id, &operation.updated_at)
    });
    if worker_is_active {
        return false;
    }
    match operation.state {
        WorkspaceOperationState::Pending | WorkspaceOperationState::Running => true,
        WorkspaceOperationState::Failed => operation.failure_reported,
        WorkspaceOperationState::Succeeded => false,
    }
}

fn pending_sync_response(provision: &ProvisionRecord) -> DispatchWorkspaceSyncResponse {
    DispatchWorkspaceSyncResponse {
        pending: true,
        changed: false,
        branch: provision.branch.clone(),
        base_commit: provision.base_commit.clone(),
        head_commit: provision.base_commit.clone(),
        commit_count: 0,
        changes: Vec::new(),
        truncated_changes: false,
        bundle_path: None,
        bundle_sha256: None,
        bundle_size: 0,
    }
}

fn sync_in_store(
    store: &DispatchStore,
    request: DispatchWorkspaceSyncRequest,
) -> Result<DispatchWorkspaceSyncResponse> {
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let _git_lock = JobLock::exclusive(&store.workspace_git_operation_lock_path(&request.job_id)?)?;
    let provision: ProvisionRecord = read_json(&job_dir.join(PROVISION_RECORD_FILE))
        .context("this job did not receive a Git workspace")?;
    let _repo_lock = JobLock::exclusive(&store.repo_lock_path(&provision.repo_key)?)?;
    let worktree = PathBuf::from(
        provision
            .workspace_path
            .as_deref()
            .context("this job's Git workspace was never checked out")?,
    );
    if !is_real_directory(&worktree) {
        bail!("the dispatch worktree is missing");
    }

    let current_branch = git(&worktree, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .context("verify the dispatch worktree branch before syncing")?
        .trim()
        .to_string();
    if current_branch != provision.branch {
        bail!(
            "dispatch worktree is on branch '{}' instead of its managed branch '{}'",
            current_branch,
            provision.branch
        );
    }

    git(&worktree, &["add", "-A"]).context("stage dispatch worktree changes")?;
    if !git_succeeds(&worktree, &["diff", "--cached", "--quiet"])? {
        let message = request
            .message
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_SYNC_COMMIT_MESSAGE);
        git(
            &worktree,
            &[
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=BitFun Dispatch",
                "-c",
                "user.email=dispatch@bitfun.local",
                "commit",
                "--no-verify",
                "-m",
                message,
            ],
        )
        .context("commit dispatch worktree changes")?;
    }

    let head_commit = git(&worktree, &["rev-parse", "HEAD"])?.trim().to_string();
    if !git_succeeds(
        &worktree,
        &[
            "merge-base",
            "--is-ancestor",
            &provision.base_commit,
            &head_commit,
        ],
    )? {
        bail!("dispatch branch no longer descends from its immutable base commit");
    }
    let sync_base = request
        .known_head
        .as_deref()
        .unwrap_or(&provision.base_commit)
        .to_string();
    if !commit_exists(&worktree, &sync_base)?
        || !git_succeeds(
            &worktree,
            &["merge-base", "--is-ancestor", &sync_base, &head_commit],
        )?
    {
        bail!("dispatch knownHead is not an ancestor of the managed branch");
    }
    if head_commit == sync_base {
        return Ok(DispatchWorkspaceSyncResponse {
            pending: false,
            changed: false,
            branch: provision.branch,
            base_commit: sync_base,
            head_commit,
            commit_count: 0,
            changes: Vec::new(),
            truncated_changes: false,
            bundle_path: None,
            bundle_sha256: None,
            bundle_size: 0,
        });
    }

    let range = format!("{sync_base}..{head_commit}");
    let commit_count = git(&worktree, &["rev-list", "--count", &range])?
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    let (changes, truncated_changes) = collect_changes(&worktree, &sync_base, &head_commit)?;

    let bundle_path = job_dir.join(RESULT_BUNDLE_FILE);
    remove_file_if_present(&bundle_path);
    // The wanted side must name a ref, not a commit id: a bundle carries refs,
    // and `git bundle` refuses to create one that would contain none.
    let bundle_range = format!("{sync_base}..{}", provision.branch);
    git(
        &worktree,
        &["bundle", "create", path_arg(&bundle_path)?, &bundle_range],
    )
    .context("package dispatch result bundle")?;
    set_private_file_permissions(&bundle_path)?;
    let bundle_size = fs::symlink_metadata(&bundle_path)
        .context("inspect dispatch result bundle")?
        .len();
    let bundle_sha256 = sha256_file(&bundle_path)?;

    Ok(DispatchWorkspaceSyncResponse {
        pending: false,
        changed: true,
        branch: provision.branch,
        base_commit: sync_base,
        head_commit,
        commit_count,
        changes,
        truncated_changes,
        bundle_path: Some(bundle_path.to_string_lossy().to_string()),
        bundle_sha256: Some(bundle_sha256),
        bundle_size,
    })
}

/// Stream back a slice of the bundle `sync` already produced.
///
/// Never rebuilds the bundle, so the digest the controller verified stays the
/// digest it receives.
pub(crate) fn sync_chunk(
    request: DispatchWorkspaceSyncChunkRequest,
) -> Result<DispatchWorkspaceSyncChunkResponse> {
    if request.length == 0 || request.length > MAX_CHUNK_BYTES as u64 {
        bail!("dispatch sync chunk length must be between 1 and {MAX_CHUNK_BYTES} bytes");
    }
    let store = DispatchStore::open_default()?;
    let job_dir = store.workspace_upload_dir(&request.job_id)?;
    let bundle_path = job_dir.join(RESULT_BUNDLE_FILE);
    let mut file =
        fs::File::open(&bundle_path).context("run the dispatch sync before reading its bundle")?;
    let size = file.metadata()?.len();
    if request.offset > size {
        bail!("dispatch sync chunk offset is past the end of the bundle");
    }
    file.seek(SeekFrom::Start(request.offset))?;
    let remaining = size - request.offset;
    let take = request.length.min(remaining) as usize;
    let mut buffer = vec![0_u8; take];
    file.read_exact(&mut buffer)
        .context("read dispatch result bundle")?;
    let next_offset = request.offset + take as u64;
    Ok(DispatchWorkspaceSyncChunkResponse {
        offset: next_offset,
        data_base64: base64::engine::general_purpose::STANDARD.encode(&buffer),
        eof: next_offset >= size,
    })
}

fn collect_changes(
    worktree: &Path,
    base_commit: &str,
    head_commit: &str,
) -> Result<(Vec<DispatchWorkspaceSyncedChange>, bool)> {
    let raw = git(
        worktree,
        &[
            "diff",
            "--name-status",
            "--no-renames",
            base_commit,
            head_commit,
        ],
    )?;
    let mut changes = Vec::new();
    let mut truncated = false;
    for line in raw.lines() {
        let mut parts = line.splitn(2, '\t');
        let (Some(status), Some(path)) = (parts.next(), parts.next()) else {
            continue;
        };
        if changes.len() >= MAX_REPORTED_CHANGES {
            truncated = true;
            break;
        }
        changes.push(DispatchWorkspaceSyncedChange {
            status: status.trim().to_string(),
            path: path.trim().to_string(),
        });
    }
    Ok((changes, truncated))
}

fn existing_worktree(
    worktree_path: &Path,
    branch: &str,
    base_commit: &str,
) -> Result<Option<String>> {
    match fs::symlink_metadata(worktree_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("inspect the dispatch worktree path"),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            bail!(
                "dispatch worktree path exists but is not a safe directory: {}",
                worktree_path.display()
            );
        }
        Ok(_) => {}
    }
    // An existing directory only counts as this job's worktree when Git agrees.
    // A non-Git directory under this managed root is a partial `worktree add`
    // left by a crash. Quarantine it so the idempotent retry can rebuild.
    if !git_succeeds(worktree_path, &["rev-parse", "--git-dir"])? {
        quarantine_partial_directory(worktree_path, "worktree")?;
        return Ok(None);
    }
    // Everything below reports the directory it is judging. A dispatch worktree
    // is only ever reached through a name derived from the job, so an occupant
    // that fails these checks is either another job's checkout or a hand-edited
    // one — and naming which is which is the whole difference between a
    // recoverable report and a dead end.
    if !commit_exists(worktree_path, base_commit)? {
        bail!(
            "dispatch worktree {} exists without the requested base commit {base_commit}",
            worktree_path.display()
        );
    }
    let current_branch = git(
        worktree_path,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
    )
    .context("inspect the existing dispatch worktree branch")?
    .trim()
    .to_string();
    if current_branch != branch {
        bail!(
            "dispatch worktree {} is on branch '{current_branch}' instead of its managed branch '{branch}'",
            worktree_path.display()
        );
    }
    let head = git(worktree_path, &["rev-parse", "HEAD"])?;
    if !git_succeeds(
        worktree_path,
        &["merge-base", "--is-ancestor", base_commit, head.trim()],
    )? {
        bail!(
            "dispatch worktree {} does not descend from its requested base commit {base_commit}",
            worktree_path.display()
        );
    }
    Ok(Some(canonical_utf8(worktree_path)?))
}

fn ensure_repository(
    store: &DispatchStore,
    repo_key: &str,
    remote_url: Option<&str>,
) -> Result<PathBuf> {
    let repo_root = store.repo_dir(repo_key)?;
    create_private_dir(&repo_root)?;
    let repo = repo_root.join("git");
    let initialize = match fs::symlink_metadata(&repo) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error).context("inspect the dispatch repository cache"),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            bail!(
                "dispatch repository cache exists but is not a safe directory: {}",
                repo.display()
            );
        }
        Ok(_) => {
            let valid = git(&repo, &["rev-parse", "--is-bare-repository"])
                .map(|value| value.trim() == "true")
                .unwrap_or(false);
            if !valid {
                quarantine_partial_directory(&repo, "repository")?;
            }
            !valid
        }
    };
    if initialize {
        let parent = repo
            .parent()
            .ok_or_else(|| anyhow::anyhow!("dispatch repository path has no parent"))?;
        git(parent, &["init", "--bare", "--quiet", "git"])
            .context("initialize the dispatch target repository")?;
    }
    if let Some(url) = remote_url {
        set_origin(&repo, url)?;
    }
    let now = chrono::Utc::now().to_rfc3339();
    let record_path = repo_root.join(super::store::REPO_CACHE_RECORD_FILE);
    let created_at = read_json::<RepoCacheRecord>(&record_path)
        .map(|record| record.created_at)
        .unwrap_or_else(|_| now.clone());
    atomic_write_json(
        &record_path,
        &RepoCacheRecord {
            remote_url: remote_url.map(ToOwned::to_owned),
            created_at,
            last_used_at: now,
        },
    )?;
    sync_directory(&repo_root)?;
    Ok(repo)
}

fn set_origin(repo: &Path, url: &str) -> Result<()> {
    if git_succeeds(repo, &["remote", "get-url", "origin"])? {
        git(repo, &["remote", "set-url", "origin", url])?;
    } else {
        git(repo, &["remote", "add", "origin", url])?;
    }
    Ok(())
}

/// Bring one commit into the repository cache from the project's remote.
///
/// Asks the server for exactly the commit this job needs. The previous refspec
/// — `+refs/heads/*:refs/remotes/origin/*` — made the first fetch of a project
/// download every branch the server has (192 of them for this repository) when
/// a dispatch only ever checks out `base_commit`. Servers that will not serve a
/// bare object id fall back to the old refspec, so an older or restricted host
/// still works, just as slowly as before.
///
/// A successful fetch is anchored under `refs/dispatch/bases/`. Asking for a
/// bare object id writes no ref of its own, and the job branch that would hold
/// it goes away with the job, which would leave the cache holding a project's
/// whole history with nothing pointing at it — invisible to `have_tips`, so the
/// next job bundles everything again, and eligible for `gc` to throw away.
fn fetch_base_commit(repo: &Path, base_commit: &str) -> Result<()> {
    remove_pack_temporaries(repo);
    let targeted = fetch_with_stall_guard(repo, &["--no-tags", "origin", base_commit]);
    let Err(error) = targeted else {
        anchor_base_commit(repo, base_commit);
        return Ok(());
    };
    tracing::debug!(
        "Dispatch target could not fetch {base_commit} directly, retrying with every branch: {error:#}"
    );
    // The targeted attempt may have died partway through indexing.
    remove_pack_temporaries(repo);
    fetch_with_stall_guard(
        repo,
        &[
            "--no-tags",
            "--prune",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    )
}

/// Keep a fetched base commit reachable after its job is gone.
///
/// Best effort on purpose: the fetch already succeeded, and the worktree about
/// to be created keeps the objects alive for this job either way. Failing here
/// would trade a warm cache for a failed dispatch.
fn anchor_base_commit(repo: &Path, base_commit: &str) {
    if let Err(error) = git(
        repo,
        &[
            "update-ref",
            &format!("refs/dispatch/bases/{base_commit}"),
            base_commit,
        ],
    ) {
        tracing::debug!("Could not anchor dispatch base commit {base_commit}: {error:#}");
    }
}

/// `git fetch` with the stall guard and the deadline both applied.
fn fetch_with_stall_guard(repo: &Path, fetch_args: &[&str]) -> Result<()> {
    let low_speed_limit = format!("http.lowSpeedLimit={FETCH_STALL_BYTES_PER_SECOND}");
    let low_speed_time = format!("http.lowSpeedTime={FETCH_STALL_SECONDS}");
    let mut args = vec![
        "-c",
        low_speed_limit.as_str(),
        "-c",
        low_speed_time.as_str(),
        "fetch",
    ];
    args.extend_from_slice(fetch_args);
    git_within(repo, REMOTE_FETCH_TIMEOUT, &args)
}

/// Drop `tmp_pack_*` files left behind by a fetch that died while indexing.
///
/// Git writes the incoming pack under a temporary name and only renames it once
/// the index is complete, so a killed or timed-out fetch strands the whole
/// download. Nothing else ever collects them — `git gc` does not consider them
/// its business, and they are invisible to every object count — so one broken
/// first fetch of a large project parks hundreds of megabytes in the cache
/// permanently, and every retry adds another copy.
///
/// Callers hold the repository lock, which serializes every Git operation on
/// this clone, so a temporary seen here cannot belong to a live fetch.
fn remove_pack_temporaries(repo: &Path) {
    let pack_dir = repo.join("objects").join("pack");
    let Ok(entries) = fs::read_dir(&pack_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("tmp_pack_") {
            continue;
        }
        if let Err(error) = fs::remove_file(entry.path()) {
            tracing::debug!("Could not remove stale dispatch pack temporary {name}: {error}");
        }
    }
}

fn repository_tips(repo: &Path) -> Result<Vec<String>> {
    let raw = git(repo, &["rev-list", "--max-count=64", "--all"])?;
    Ok(raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn create_worktree(
    repo: &Path,
    worktree_path: &Path,
    branch: &str,
    base_commit: &str,
) -> Result<String> {
    if let Some(parent) = worktree_path.parent() {
        create_private_dir(parent)?;
    }
    // Registered-but-missing worktrees survive a crashed job and would make
    // `worktree add` refuse the same path forever.
    let _ = git(repo, &["worktree", "prune"]);
    let branch_ref = format!("refs/heads/{branch}");
    if git_succeeds(repo, &["show-ref", "--verify", "--quiet", &branch_ref])? {
        // A missing checkout may still leave the job branch with valuable
        // commits. Reattach it when it descends from the immutable baseline;
        // never reset it back to the base and make that work unreachable.
        if !git_succeeds(
            repo,
            &["merge-base", "--is-ancestor", base_commit, &branch_ref],
        )? {
            bail!("existing dispatch branch does not descend from the requested base commit");
        }
    } else {
        git(repo, &["update-ref", &branch_ref, base_commit])
            .context("point the dispatch branch at the requested base commit")?;
    }
    git(repo, &["worktree", "add", path_arg(worktree_path)?, branch])
        .context("create the dispatch worktree")?;
    canonical_utf8(worktree_path)
}

/// Leaf directory name for a job's checkout.
///
/// Mirrors the local managed-worktree convention (`<project>-<short id>`) so a
/// target directory is recognizable rather than a bare job UUID. The label is
/// advisory input from the controller, so it is sanitized here and falls back to
/// the remote URL's basename and finally to a constant — the path must never be
/// shaped by an untrusted string.
///
/// The suffix digests the job id rather than slicing it. Job ids are minted as
/// `dispatch-<uuid>`, so the leading alphanumerics every id shares — `dispatch`
/// — were all that survived the slice, and every job of one project resolved to
/// the same directory. The second session to start then met the first session's
/// checkout and provisioning refused it. A digest depends on the whole id, so no
/// shared prefix, suffix, or length can collapse two jobs onto one directory.
fn worktree_directory_name(
    project_label: Option<&str>,
    remote_url: Option<&str>,
    job_id: &str,
) -> String {
    let label = sanitize_label(project_label.unwrap_or_default())
        .or_else(|| sanitize_label(&remote_basename(remote_url.unwrap_or_default())))
        .unwrap_or_else(|| "workspace".to_string());
    match job_directory_suffix(job_id) {
        Some(suffix) => format!("{label}-{suffix}"),
        None => label,
    }
}

fn job_directory_suffix(job_id: &str) -> Option<String> {
    if job_id.is_empty() {
        return None;
    }
    let digest = Sha256::digest(job_id.as_bytes());
    Some(
        format!("{digest:x}")
            .chars()
            .take(WORKTREE_SUFFIX_CHARS)
            .collect(),
    )
}

fn sanitize_label(value: &str) -> Option<String> {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = cleaned.trim_matches(|character| character == '-' || character == '.');
    let bounded = trimmed
        .chars()
        .take(WORKTREE_LABEL_MAX_CHARS)
        .collect::<String>();
    let bounded = bounded.trim_end_matches(['-', '.']).to_string();
    (!bounded.is_empty()).then_some(bounded)
}

/// `git@host:acme/app.git` and `https://host/acme/app.git` both yield `app`.
fn remote_basename(remote_url: &str) -> String {
    remote_url
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default()
        .trim_end_matches(".git")
        .to_string()
}

fn commit_exists(repo: &Path, commit: &str) -> Result<bool> {
    git_succeeds(repo, &["cat-file", "-e", &format!("{commit}^{{commit}}")])
}

fn git_command(dir: &Path) -> Command {
    let mut command = process_manager::create_command("git");
    command
        .current_dir(dir)
        // A detached dispatch worker has nobody to answer a credential or
        // host-key prompt, so every one of them must fail fast instead of
        // blocking the job until its transport times out.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        .env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::null());
    command
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = git_command(dir)
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed: {}",
            args.join(" "),
            truncate_utf8(stderr.trim())
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run Git with a deadline, killing it if the deadline passes.
///
/// Only the remote-facing operations need this. Everything else here works on
/// local objects and finishes in bounded time on its own, whereas a fetch is at
/// the mercy of a network that may never answer — and an unbounded one holds the
/// whole dispatch, and the caller's progress display, hostage.
fn git_within(dir: &Path, timeout: Duration, args: &[&str]) -> Result<()> {
    let mut command = git_command(dir);
    command
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // `git fetch` is a supervisor: the transfer really happens in the transport
    // helper and `index-pack` that it spawns. Killing only the parent leaves
    // those downloading into the cache with nobody waiting for them, which is
    // how a cancelled fetch ends up stranding hundreds of megabytes. Giving the
    // whole thing its own process group makes it killable as a unit on Unix;
    // Windows keeps the old parent-only kill, and the sweep above is what
    // reclaims whatever a surviving helper leaves behind.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    // Drain stderr on its own thread: a child that fills the pipe while nobody
    // reads it blocks forever, which would defeat the deadline below.
    let mut pipe = child.stderr.take();
    let drain = std::thread::spawn(move || {
        let mut captured = String::new();
        if let Some(pipe) = pipe.as_mut() {
            let _ = pipe.read_to_string(&mut captured);
        }
        captured
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                return Err(error).with_context(|| format!("wait for git {}", args.join(" ")))
            }
        }
        if Instant::now() >= deadline {
            kill_process_tree(&mut child);
            // Deliberately not joining the drain: any helper that outlived the
            // kill still holds the write end, and waiting on it here would put
            // the deadline right back where it started.
            bail!(
                "git {} did not finish within {} seconds",
                args.join(" "),
                timeout.as_secs()
            );
        }
        std::thread::sleep(GIT_WAIT_POLL_INTERVAL);
    };
    let stderr = drain.join().unwrap_or_default();
    if !status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            truncate_utf8(stderr.trim())
        );
    }
    Ok(())
}

/// Stop a timed-out Git child and everything it spawned.
fn kill_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The child leads its own group (see `process_group` above), so its pid
        // doubles as the group id.
        let group = child.id() as i32;
        if group > 1 {
            // SAFETY: `kill` takes a pid and a signal by value and borrows
            // nothing; a group that already exited just yields ESRCH.
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn git_succeeds(dir: &Path, args: &[&str]) -> Result<bool> {
    let status = git_command(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    Ok(status.success())
}

fn path_arg(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow::anyhow!("dispatch path is not valid UTF-8: {}", path.display()))
}

fn canonical_utf8(path: &Path) -> Result<String> {
    dunce::canonicalize(path)
        .with_context(|| format!("resolve dispatch path {}", path.display()))?
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("dispatch path is not valid UTF-8"))
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
}

fn quarantine_partial_directory(path: &Path, label: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("managed dispatch {label} path has no parent"))?;
    let tombstone = parent.join(format!(
        ".partial-{label}-{}",
        uuid::Uuid::new_v4().as_simple()
    ));
    fs::rename(path, &tombstone)
        .with_context(|| format!("quarantine partial dispatch {label} {}", path.display()))?;
    fs::remove_dir_all(&tombstone)
        .with_context(|| format!("remove partial dispatch {label} {}", tombstone.display()))
}

fn workspace_worker_is_active(pid: u32, action: &str, job_id: &str, updated_at: &str) -> bool {
    super::runner::workspace_operation_process_alive(pid, action, job_id)
        || chrono::DateTime::parse_from_rfc3339(updated_at)
            .ok()
            .map(|updated| {
                chrono::Utc::now()
                    .signed_duration_since(updated)
                    .num_seconds()
            })
            .is_some_and(|age| (0..OPERATION_START_GRACE_SECONDS).contains(&age))
}

fn ensure_provision_binding(
    record: &ProvisionRecord,
    request: &DispatchWorkspaceProvisionRequest,
) -> Result<()> {
    if record.job_id != request.job_id
        || record.repo_key != request.repo_key
        || record.base_commit != request.base_commit
        || record.branch != request.branch
    {
        bail!("dispatch job is already bound to a different Git baseline");
    }
    Ok(())
}

fn read_optional_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("dispatch operation record is not a regular file");
            }
            read_json(path).map(Some)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
    }
}

fn ensure_bundle_binding(
    record: &BundleUploadRecord,
    request: &DispatchWorkspaceBundleBeginRequest,
) -> Result<()> {
    if record.job_id != request.job_id
        || !record.sha256.eq_ignore_ascii_case(&request.sha256)
        || record.size != request.size
    {
        bail!("dispatch job is already bound to a different bundle");
    }
    Ok(())
}

fn ensure_bundle_not_failed(record: &BundleUploadRecord) -> Result<()> {
    if record.state == BundleUploadState::Failed {
        bail!(
            "dispatch bundle delivery failed: {}",
            record
                .last_error
                .as_deref()
                .unwrap_or("target did not retain a diagnostic")
        );
    }
    Ok(())
}

fn validate_provision(request: &DispatchWorkspaceProvisionRequest) -> Result<()> {
    if request.protocol_version != DISPATCH_PROTOCOL_VERSION {
        bail!(
            "unsupported dispatch protocolVersion {}; target requires {}",
            request.protocol_version,
            DISPATCH_PROTOCOL_VERSION
        );
    }
    super::store::validate_id("jobId", &request.job_id)?;
    validate_repo_key(&request.repo_key)?;
    validate_commit(&request.base_commit)?;
    validate_branch(&request.branch)?;
    if let Some(url) = request.remote_url.as_deref() {
        validate_remote_url(url)?;
    }
    Ok(())
}

/// The repo key names a directory, so it is restricted to a hex digest the
/// controller derives rather than anything user-controlled.
fn validate_repo_key(repo_key: &str) -> Result<()> {
    if repo_key.len() < 8
        || repo_key.len() > 64
        || !repo_key.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("dispatch repoKey must be an 8-64 character hex digest");
    }
    Ok(())
}

fn validate_commit(commit: &str) -> Result<()> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("dispatch baseCommit must be a full 40-character commit id");
    }
    Ok(())
}

fn validate_digest(digest: &str) -> Result<()> {
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("dispatch bundle digest must be a SHA-256 hex string");
    }
    Ok(())
}

/// Reject anything Git itself would not accept as a branch, plus leading
/// dashes, which Git would read as an option rather than a ref.
fn validate_branch(branch: &str) -> Result<()> {
    if branch.is_empty() || branch.len() > 255 || branch.starts_with('-') {
        bail!("dispatch branch name is outside the accepted range");
    }
    if !branch
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
    {
        bail!("dispatch branch name contains unsupported characters");
    }
    if branch.contains("..") || branch.ends_with('/') || branch.ends_with(".lock") {
        bail!("dispatch branch name is not a valid Git ref");
    }
    Ok(())
}

/// A remote URL becomes a `git remote` argument, so it must not look like an
/// option, and `ext::` would let a URL execute an arbitrary command.
fn validate_remote_url(url: &str) -> Result<()> {
    if url.is_empty() || url.len() > 2048 {
        bail!("dispatch remoteUrl is outside the accepted length");
    }
    if url.starts_with('-') {
        bail!("dispatch remoteUrl must not start with a dash");
    }
    if url.bytes().any(|byte| byte.is_ascii_control()) {
        bail!("dispatch remoteUrl must not contain control characters");
    }
    if url.to_ascii_lowercase().starts_with("ext::") {
        bail!("dispatch remoteUrl must not use the ext transport");
    }
    Ok(())
}

fn truncate_utf8(value: &str) -> String {
    const MAX_ERROR_BYTES: usize = 16 * 1024;
    if value.len() <= MAX_ERROR_BYTES {
        return value.to_string();
    }
    let mut end = MAX_ERROR_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_source_repository(path: &Path) -> String {
        fs::create_dir_all(path).expect("source directory");
        git(path, &["init", "--quiet", "--initial-branch=main"]).expect("init");
        git(path, &["config", "user.email", "dispatch@example.com"]).expect("email");
        git(path, &["config", "user.name", "Dispatch Test"]).expect("name");
        fs::write(path.join("file.txt"), b"base").expect("seed file");
        git(path, &["add", "-A"]).expect("stage");
        git(path, &["commit", "--quiet", "-m", "base"]).expect("commit");
        git(path, &["rev-parse", "HEAD"])
            .expect("head")
            .trim()
            .to_string()
    }

    fn bundle_everything(source: &Path, bundle: &Path) {
        git(
            source,
            &["bundle", "create", path_arg(bundle).expect("path"), "main"],
        )
        .expect("bundle");
    }

    #[test]
    fn provision_asks_for_a_bundle_when_the_commit_is_unreachable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let response = provision_in_store(
            &store,
            DispatchWorkspaceProvisionRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                repo_key: "abcdef0123456789".to_string(),
                project_label: Some("BitFun".to_string()),
                remote_url: None,
                base_commit: "0".repeat(40),
                branch: "bitfun/dispatch/job-1".to_string(),
            },
        )
        .expect("provision");

        assert!(!response.provisioned);
        assert!(response.needs_bundle);
        assert!(response.workspace_path.is_none());
        // No remote to try, so nothing to explain.
        assert_eq!(response.fetch_error, None);
    }

    /// A local path is a valid Git remote, so the fast path is testable without
    /// a network: the target should pull the commit itself and never ask for a
    /// bundle.
    #[test]
    fn provision_fetches_the_base_commit_from_the_remote_instead_of_bundling() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);

        let response = provision_in_store(
            &store,
            DispatchWorkspaceProvisionRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                repo_key: "abcdef0123456789".to_string(),
                project_label: Some("BitFun".to_string()),
                remote_url: Some(source.to_string_lossy().to_string()),
                base_commit: base_commit.clone(),
                branch: "bitfun/dispatch/job-1".to_string(),
            },
        )
        .expect("provision");

        assert!(response.provisioned, "the remote had the commit");
        assert!(
            !response.needs_bundle,
            "no upload should have been asked for"
        );
        assert_eq!(response.fetch_error, None);
        let workspace = response.workspace_path.expect("workspace path");
        assert_eq!(
            fs::read(Path::new(&workspace).join("file.txt")).expect("checked out file"),
            b"base"
        );

        // The fetched history must outlive the job that pulled it, or the next
        // dispatch of this project pays for the whole download again.
        let repo = store
            .repo_dir("abcdef0123456789")
            .expect("repo dir")
            .join("git");
        let anchored = git(
            &repo,
            &["rev-parse", &format!("refs/dispatch/bases/{base_commit}")],
        )
        .expect("the base commit should be anchored");
        assert_eq!(anchored.trim(), base_commit);
        assert!(
            repository_tips(&repo).expect("tips").contains(&base_commit),
            "an anchored base must count as a tip the controller can bundle against"
        );
    }

    /// An unreachable remote must degrade to the bundle path *and say why*: on a
    /// cold cache that fallback re-sends the project's whole history.
    #[test]
    fn an_unreachable_remote_reports_why_it_fell_back_to_a_bundle() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let missing = temp.path().join("no-such-repository");

        let response = provision_in_store(
            &store,
            DispatchWorkspaceProvisionRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                repo_key: "abcdef0123456789".to_string(),
                project_label: Some("BitFun".to_string()),
                remote_url: Some(missing.to_string_lossy().to_string()),
                base_commit: "0".repeat(40),
                branch: "bitfun/dispatch/job-1".to_string(),
            },
        )
        .expect("provision");

        assert!(response.needs_bundle);
        // The reason has to name the operation and carry Git's own diagnosis;
        // "the target fell back to a bundle" on its own is not actionable.
        let reason = response.fetch_error.expect("the fallback reason");
        assert!(reason.contains("fetch"), "unhelpful reason: {reason}");
        assert!(
            reason.contains("does not appear to be a git repository"),
            "the reason dropped Git's own diagnosis: {reason}"
        );
    }

    /// A fetch killed while indexing strands its pack under a temporary name.
    /// Nothing else collects those, so one broken fetch of a large project used
    /// to park its whole download in the cache permanently.
    #[test]
    fn a_failed_fetch_does_not_leave_its_partial_pack_behind() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let repo_root = store.repo_dir("abcdef0123456789").expect("repo dir");
        // Seed a real cache: a directory that is not a valid bare repository is
        // quarantined and rebuilt, which would retire the packs before the
        // sweep under test ever sees them.
        create_private_dir(&repo_root).expect("repo root");
        git(&repo_root, &["init", "--bare", "--quiet", "git"]).expect("bare repo");
        let pack_dir = repo_root.join("git").join("objects").join("pack");
        fs::create_dir_all(&pack_dir).expect("pack dir");
        fs::write(pack_dir.join("tmp_pack_abandoned"), b"partial download").expect("stranded pack");
        // A real pack must survive: it is the cache this whole path exists for.
        fs::write(pack_dir.join("pack-real.pack"), b"kept").expect("real pack");

        provision_in_store(
            &store,
            DispatchWorkspaceProvisionRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                repo_key: "abcdef0123456789".to_string(),
                project_label: Some("BitFun".to_string()),
                remote_url: Some(
                    temp.path()
                        .join("no-such-repository")
                        .to_string_lossy()
                        .to_string(),
                ),
                base_commit: "0".repeat(40),
                branch: "bitfun/dispatch/job-1".to_string(),
            },
        )
        .expect("provision");

        assert!(
            !pack_dir.join("tmp_pack_abandoned").exists(),
            "the stranded pack survived"
        );
        assert!(
            pack_dir.join("pack-real.pack").exists(),
            "a real pack was deleted"
        );
    }

    #[test]
    fn a_git_command_that_never_finishes_is_killed_at_its_deadline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().to_path_buf();
        git(&repo, &["init", "--quiet", "--bare"]).expect("init");

        let started = Instant::now();
        // `--stdin` with a null stdin returns immediately; a long sleep does not.
        let error = git_within(
            &repo,
            Duration::from_millis(300),
            &["-c", "alias.stall=!sleep 30", "stall"],
        )
        .expect_err("the deadline should have fired");

        assert!(
            started.elapsed() < Duration::from_secs(10),
            "it waited too long"
        );
        assert!(
            format!("{error:#}").contains("did not finish within"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn a_delivered_bundle_provisions_a_worktree_at_the_requested_commit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        let bundle = temp.path().join("base.bundle");
        bundle_everything(&source, &bundle);

        let request = DispatchWorkspaceProvisionRequest {
            protocol_version: DISPATCH_PROTOCOL_VERSION,
            job_id: "job-1".to_string(),
            repo_key: "abcdef0123456789".to_string(),
            remote_url: None,
            project_label: Some("BitFun".to_string()),
            base_commit: base_commit.clone(),
            branch: "main".to_string(),
        };
        assert!(
            provision_in_store(&store, request.clone())
                .expect("first provision")
                .needs_bundle
        );

        let job_dir = store.workspace_upload_dir("job-1").expect("job dir");
        let size = fs::symlink_metadata(&bundle)
            .expect("bundle metadata")
            .len();
        bundle_begin_in_store(
            &store,
            DispatchWorkspaceBundleBeginRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                sha256: sha256_file(&bundle).expect("digest"),
                size,
            },
        )
        .expect("bundle begin");
        fs::copy(&bundle, job_dir.join(INCOMING_BUNDLE_FILE)).expect("stage bundle");
        assert!(
            bundle_commit_in_store(
                &store,
                DispatchWorkspaceBundleCommitRequest {
                    job_id: "job-1".to_string()
                },
            )
            .expect("bundle commit")
            .committed
        );

        let response = provision_in_store(&store, request).expect("second provision");
        assert!(response.provisioned);
        assert!(!response.needs_bundle);
        let workspace = response.workspace_path.expect("workspace path");
        assert_eq!(
            fs::read(Path::new(&workspace).join("file.txt")).expect("checked out file"),
            b"base"
        );
    }

    #[test]
    fn reprovisioning_a_missing_checkout_never_resets_the_job_branch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        provision_from_bundle(&store, &source, &base_commit);

        let worktree = provisioned_worktree(&store, "job-1");
        fs::write(worktree.join("agent.txt"), b"valuable work").expect("edit");
        git(&worktree, &["add", "-A"]).expect("stage");
        git(
            &worktree,
            &[
                "-c",
                "user.name=Dispatch Test",
                "-c",
                "user.email=dispatch@example.com",
                "commit",
                "--quiet",
                "-m",
                "agent work",
            ],
        )
        .expect("commit");
        let advanced = git(&worktree, &["rev-parse", "HEAD"])
            .expect("head")
            .trim()
            .to_string();
        let repo = store
            .repo_dir("abcdef0123456789")
            .expect("repo")
            .join("git");
        git(
            &repo,
            &[
                "worktree",
                "remove",
                "--force",
                path_arg(&worktree).unwrap(),
            ],
        )
        .expect("remove checkout only");

        let response = provision_in_store(
            &store,
            DispatchWorkspaceProvisionRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                repo_key: "abcdef0123456789".to_string(),
                project_label: Some("BitFun".to_string()),
                remote_url: None,
                base_commit,
                branch: "main".to_string(),
            },
        )
        .expect("reprovision");
        let restored = PathBuf::from(response.workspace_path.expect("restored path"));
        assert_eq!(
            git(&restored, &["rev-parse", "HEAD"]).unwrap().trim(),
            advanced
        );
        assert_eq!(
            fs::read(restored.join("agent.txt")).unwrap(),
            b"valuable work"
        );
    }

    #[test]
    fn sync_reports_no_change_for_an_untouched_worktree() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        provision_from_bundle(&store, &source, &base_commit);

        let response = sync_in_store(
            &store,
            DispatchWorkspaceSyncRequest {
                job_id: "job-1".to_string(),
                operation_id: "sync-untouched".to_string(),
                message: None,
                known_head: None,
            },
        )
        .expect("sync");

        assert!(!response.changed);
        assert_eq!(response.commit_count, 0);
        assert!(response.bundle_path.is_none());
    }

    #[test]
    fn sync_commits_agent_edits_and_bundles_only_the_new_history() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        provision_from_bundle(&store, &source, &base_commit);

        let worktree = provisioned_worktree(&store, "job-1");
        git(&worktree, &["config", "user.email", "dispatch@example.com"]).expect("email");
        git(&worktree, &["config", "user.name", "Dispatch Test"]).expect("name");
        fs::write(worktree.join("file.txt"), b"changed by the agent").expect("edit");
        fs::write(worktree.join("added.txt"), b"new").expect("add");

        let response = sync_in_store(
            &store,
            DispatchWorkspaceSyncRequest {
                job_id: "job-1".to_string(),
                operation_id: "sync-agent-edits".to_string(),
                message: Some("agent work".to_string()),
                known_head: None,
            },
        )
        .expect("sync");

        assert!(response.changed);
        assert_eq!(response.commit_count, 1);
        assert_eq!(response.base_commit, base_commit);
        assert_ne!(response.head_commit, base_commit);
        let mut paths = response
            .changes
            .iter()
            .map(|change| change.path.clone())
            .collect::<Vec<_>>();
        paths.sort();
        assert_eq!(paths, vec!["added.txt".to_string(), "file.txt".to_string()]);

        // The bundle carries only what the controller is missing, so applying it
        // is a fast-forward rather than a re-delivery of the whole repository.
        let bundle = PathBuf::from(response.bundle_path.expect("bundle path"));
        assert!(bundle.is_file());
        let prerequisites = git(
            &worktree,
            &["bundle", "list-heads", path_arg(&bundle).unwrap()],
        )
        .expect("list heads");
        assert!(prerequisites.contains("refs/heads/main"));
    }

    #[test]
    fn sync_uses_known_head_as_the_incremental_generation_boundary() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        provision_from_bundle(&store, &source, &base_commit);
        let worktree = provisioned_worktree(&store, "job-1");

        fs::write(worktree.join("first.txt"), b"first checkpoint").expect("first edit");
        let first = sync_in_store(
            &store,
            DispatchWorkspaceSyncRequest {
                job_id: "job-1".to_string(),
                operation_id: "sync-first".to_string(),
                message: None,
                known_head: None,
            },
        )
        .expect("first sync");
        assert!(first.changed);

        let unchanged = sync_in_store(
            &store,
            DispatchWorkspaceSyncRequest {
                job_id: "job-1".to_string(),
                operation_id: "sync-clean".to_string(),
                message: None,
                known_head: Some(first.head_commit.clone()),
            },
        )
        .expect("clean incremental sync");
        assert!(!unchanged.changed);
        assert_eq!(unchanged.base_commit, first.head_commit);

        fs::write(worktree.join("second.txt"), b"second checkpoint").expect("second edit");
        let second = sync_in_store(
            &store,
            DispatchWorkspaceSyncRequest {
                job_id: "job-1".to_string(),
                operation_id: "sync-second".to_string(),
                message: None,
                known_head: Some(first.head_commit.clone()),
            },
        )
        .expect("second sync");
        assert!(second.changed);
        assert_eq!(second.base_commit, first.head_commit);
        assert_eq!(second.commit_count, 1);
        assert_eq!(
            second
                .changes
                .iter()
                .map(|change| change.path.as_str())
                .collect::<Vec<_>>(),
            vec!["second.txt"]
        );
    }

    #[test]
    fn completed_clean_sync_poll_returns_the_durable_result() {
        const CHILD_ENV: &str = "BITFUN_DISPATCH_CLEAN_SYNC_POLL_CHILD";
        if let Some(bitfun_home) = std::env::var_os(CHILD_ENV) {
            let store = DispatchStore::open_default().expect("open isolated default store");
            let source = PathBuf::from(bitfun_home).join("source");
            let base_commit = init_source_repository(&source);
            provision_from_bundle(&store, &source, &base_commit);

            // Model the second user-requested sync: the controller has already
            // acknowledged the target head, and this invocation finds no newer
            // work. Its later polls must return this clean response instead of
            // opening another detached operation forever.
            let request = DispatchWorkspaceSyncRequest {
                job_id: "job-1".to_string(),
                operation_id: "sync-clean-poll".to_string(),
                message: None,
                known_head: Some(base_commit),
            };
            let response = sync_in_store(&store, request.clone()).expect("clean sync");
            assert!(!response.changed);

            let operation_path = store
                .workspace_upload_dir("job-1")
                .expect("workspace path")
                .join(SYNC_OPERATION_FILE);
            atomic_write_json(
                &operation_path,
                &SyncOperationRecord {
                    request: request.clone(),
                    state: WorkspaceOperationState::Succeeded,
                    worker_pid: None,
                    response: Some(response.clone()),
                    last_error: None,
                    failure_reported: false,
                    updated_at: chrono::Utc::now().to_rfc3339(),
                },
            )
            .expect("seed completed operation");

            let polled = sync(request.clone()).expect("poll completed operation");
            assert_eq!(polled, response);
            assert!(!polled.pending);
            let retained: SyncOperationRecord =
                read_json(&operation_path).expect("read retained operation");
            assert_eq!(retained.state, WorkspaceOperationState::Succeeded);
            assert_eq!(retained.request.operation_id, "sync-clean-poll");

            let mut mismatched = request;
            mismatched.message = Some("different request".to_string());
            let error = sync(mismatched).expect_err("one operation id binds one request");
            assert!(error.to_string().contains("operationId is already bound"));
            return;
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let bitfun_home = dir.path().join("bitfun-home");
        let user_root = dir.path().join("user-root");
        let output = bitfun_services_core::process_manager::create_command(
            std::env::current_exe().expect("test executable"),
        )
        .args([
            "--exact",
            "dispatch::workspace::tests::completed_clean_sync_poll_returns_the_durable_result",
            "--nocapture",
        ])
        .env(CHILD_ENV, &bitfun_home)
        .env("BITFUN_HOME", &bitfun_home)
        .env("BITFUN_USER_ROOT", &user_root)
        .env("BITFUN_E2E_STORAGE_GUARD", "1")
        .env_remove("BITFUN_E2E_HOME")
        .env_remove("BITFUN_E2E_USER_ROOT")
        .output()
        .expect("run isolated clean-sync poll test");
        assert!(
            output.status.success(),
            "isolated child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn reported_sync_failure_allows_a_new_operation_to_take_over() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        provision_from_bundle(&store, &source, &base_commit);
        let operation_path = store
            .workspace_upload_dir("job-1")
            .expect("workspace path")
            .join(SYNC_OPERATION_FILE);
        let failed_request = DispatchWorkspaceSyncRequest {
            job_id: "job-1".to_string(),
            operation_id: "sync-failed-generation".to_string(),
            message: None,
            known_head: Some(base_commit.clone()),
        };
        atomic_write_json(
            &operation_path,
            &SyncOperationRecord {
                request: failed_request.clone(),
                state: WorkspaceOperationState::Failed,
                worker_pid: None,
                response: None,
                last_error: Some("transient Git lock".to_string()),
                failure_reported: false,
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .expect("seed failed operation");

        let error = start_sync_in_store(&store, failed_request, |_| {
            panic!("reporting a retained failure must not spawn a worker")
        })
        .expect_err("the failed generation must be reported");
        assert!(error.to_string().contains("transient Git lock"));
        let mut retained: SyncOperationRecord =
            read_json(&operation_path).expect("read reported failure");
        assert_eq!(retained.state, WorkspaceOperationState::Failed);
        assert!(retained.failure_reported);
        assert_eq!(retained.last_error.as_deref(), Some("transient Git lock"));

        let retry = DispatchWorkspaceSyncRequest {
            job_id: "job-1".to_string(),
            operation_id: "sync-retry-generation".to_string(),
            message: None,
            known_head: Some(base_commit),
        };
        retained.worker_pid = Some(std::process::id());
        retained.updated_at = chrono::Utc::now().to_rfc3339();
        atomic_write_json(&operation_path, &retained).expect("seed active worker marker");
        let error = start_sync_in_store(&store, retry.clone(), |_| {
            panic!("an active generation must not spawn a replacement worker")
        })
        .expect_err("an active generation must retain ownership");
        assert!(error
            .to_string()
            .contains("already bound to a different request"));

        retained.worker_pid = None;
        atomic_write_json(&operation_path, &retained).expect("retire failed worker marker");
        let response = start_sync_in_store(&store, retry.clone(), |job_id| {
            assert_eq!(job_id, "job-1");
            Ok(42)
        })
        .expect("retry with a new operation id");
        assert!(response.pending);
        let replacement: SyncOperationRecord =
            read_json(&operation_path).expect("read replacement operation");
        assert_eq!(replacement.request, retry);
        assert_eq!(replacement.worker_pid, Some(42));
        assert!(matches!(
            replacement.state,
            WorkspaceOperationState::Pending | WorkspaceOperationState::Running
        ));
        assert!(!replacement.failure_reported);
    }

    #[test]
    fn legacy_sync_failure_without_operation_id_is_reported_then_retryable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        provision_from_bundle(&store, &source, &base_commit);
        let operation_path = store
            .workspace_upload_dir("job-1")
            .expect("workspace path")
            .join(SYNC_OPERATION_FILE);
        // Early protocol-v3 development builds wrote neither operationId
        // nor failureReported. Keep that exact JSON shape readable.
        atomic_write_json(
            &operation_path,
            &serde_json::json!({
                "request": {
                    "jobId": "job-1",
                    "message": null,
                    "knownHead": base_commit,
                },
                "state": "failed",
                "workerPid": null,
                "response": null,
                "lastError": "legacy sync failure",
                "updatedAt": chrono::Utc::now().to_rfc3339(),
            }),
        )
        .expect("seed legacy operation");

        let first_retry = DispatchWorkspaceSyncRequest {
            job_id: "job-1".to_string(),
            operation_id: "sync-after-upgrade-1".to_string(),
            message: None,
            known_head: Some(base_commit.clone()),
        };
        let error = start_sync_in_store(&store, first_retry, |_| {
            panic!("reporting a legacy failure must not spawn a worker")
        })
        .expect_err("legacy failure must be surfaced once");
        assert!(error.to_string().contains("legacy sync failure"));
        let reported: SyncOperationRecord =
            read_json(&operation_path).expect("read upgraded legacy operation");
        assert!(reported.request.operation_id.is_empty());
        assert_eq!(reported.state, WorkspaceOperationState::Failed);
        assert!(reported.failure_reported);

        let second_retry = DispatchWorkspaceSyncRequest {
            job_id: "job-1".to_string(),
            operation_id: "sync-after-upgrade-2".to_string(),
            message: None,
            known_head: Some(base_commit),
        };
        let response = start_sync_in_store(&store, second_retry.clone(), |job_id| {
            assert_eq!(job_id, "job-1");
            Ok(43)
        })
        .expect("replace legacy generation");
        assert!(response.pending);
        let replacement: SyncOperationRecord =
            read_json(&operation_path).expect("read replacement operation");
        assert_eq!(replacement.request, second_retry);
        assert_eq!(replacement.worker_pid, Some(43));
        assert!(!replacement.failure_reported);
    }

    #[test]
    fn sync_fails_loudly_when_the_agent_switches_away_from_the_managed_branch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        provision_from_bundle(&store, &source, &base_commit);
        let worktree = provisioned_worktree(&store, "job-1");
        git(&worktree, &["switch", "--quiet", "-c", "agent/other"]).expect("switch branch");

        let error = sync_in_store(
            &store,
            DispatchWorkspaceSyncRequest {
                job_id: "job-1".to_string(),
                operation_id: "sync-wrong-branch".to_string(),
                message: None,
                known_head: None,
            },
        )
        .expect_err("a different branch must not be bundled under the managed ref");
        assert!(error.to_string().contains("instead of its managed branch"));
    }

    #[test]
    fn provision_repairs_a_partial_bare_repository_left_by_a_crash() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let repo = store
            .repo_dir("abcdef0123456789")
            .expect("repo root")
            .join("git");
        fs::create_dir_all(&repo).expect("partial repository directory");

        let response = provision_in_store(
            &store,
            DispatchWorkspaceProvisionRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                repo_key: "abcdef0123456789".to_string(),
                project_label: Some("BitFun".to_string()),
                remote_url: None,
                base_commit: "0".repeat(40),
                branch: "bitfun/dispatch/job-1".to_string(),
            },
        )
        .expect("partial repository should be rebuilt");

        assert!(response.needs_bundle);
        assert_eq!(
            git(&repo, &["rev-parse", "--is-bare-repository"])
                .expect("inspect repaired repository")
                .trim(),
            "true"
        );
    }

    #[test]
    fn polling_and_long_git_operations_use_distinct_locks() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        assert_ne!(
            store.workspace_operation_lock_path("job-1").unwrap(),
            store.workspace_git_operation_lock_path("job-1").unwrap()
        );
    }

    /// Resolve a job's checkout the way production does: from its record.
    fn provisioned_worktree(store: &DispatchStore, job_id: &str) -> PathBuf {
        let record: ProvisionRecord = read_json(
            &store
                .workspace_upload_dir(job_id)
                .expect("job dir")
                .join(PROVISION_RECORD_FILE),
        )
        .expect("provision record");
        PathBuf::from(record.workspace_path.expect("checked-out workspace"))
    }

    fn provision_from_bundle(store: &DispatchStore, source: &Path, base_commit: &str) {
        let bundle = source.parent().expect("parent").join("base.bundle");
        bundle_everything(source, &bundle);
        let request = DispatchWorkspaceProvisionRequest {
            protocol_version: DISPATCH_PROTOCOL_VERSION,
            job_id: "job-1".to_string(),
            repo_key: "abcdef0123456789".to_string(),
            remote_url: None,
            project_label: Some("BitFun".to_string()),
            base_commit: base_commit.to_string(),
            branch: "main".to_string(),
        };
        provision_in_store(store, request.clone()).expect("first provision");
        let job_dir = store.workspace_upload_dir("job-1").expect("job dir");
        bundle_begin_in_store(
            store,
            DispatchWorkspaceBundleBeginRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                sha256: sha256_file(&bundle).expect("digest"),
                size: fs::symlink_metadata(&bundle).expect("metadata").len(),
            },
        )
        .expect("bundle begin");
        fs::copy(&bundle, job_dir.join(INCOMING_BUNDLE_FILE)).expect("stage bundle");
        bundle_commit_in_store(
            store,
            DispatchWorkspaceBundleCommitRequest {
                job_id: "job-1".to_string(),
            },
        )
        .expect("bundle commit");
        provision_in_store(store, request).expect("second provision");
    }

    /// Two sessions started from one workspace, with the job ids the controller
    /// actually mints. Before the directory suffix digested the id, the second
    /// one landed on the first one's checkout and provisioning refused it.
    #[test]
    fn a_second_session_on_one_workspace_provisions_alongside_the_first() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        let base_commit = init_source_repository(&source);
        let first = "dispatch-3d82ff46-bbf9-44c3-9c0f-2a1b0c4d5e6f";
        let second = "dispatch-ac8fe8a3-85c7-4091-9ba4-856db2d55c2b";

        // The controller branches its baseline before bundling, so the managed
        // branch is what the target fetches out of the bundle.
        let bundle = temp.path().join("base.bundle");
        git(&source, &["branch", &format!("bitfun/dispatch/{first}")]).expect("managed branch");
        git(
            &source,
            &[
                "bundle",
                "create",
                path_arg(&bundle).expect("path"),
                &format!("bitfun/dispatch/{first}"),
            ],
        )
        .expect("bundle");

        let request_for = |job_id: &str| DispatchWorkspaceProvisionRequest {
            protocol_version: DISPATCH_PROTOCOL_VERSION,
            job_id: job_id.to_string(),
            repo_key: "abcdef0123456789".to_string(),
            remote_url: None,
            project_label: Some("BitFun".to_string()),
            base_commit: base_commit.clone(),
            branch: format!("bitfun/dispatch/{job_id}"),
        };

        // The first session has to carry the objects over: nothing is cached yet.
        assert!(
            provision_in_store(&store, request_for(first))
                .expect("first provision")
                .needs_bundle
        );
        bundle_begin_in_store(
            &store,
            DispatchWorkspaceBundleBeginRequest {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: first.to_string(),
                sha256: sha256_file(&bundle).expect("digest"),
                size: fs::symlink_metadata(&bundle).expect("metadata").len(),
            },
        )
        .expect("bundle begin");
        fs::copy(
            &bundle,
            store
                .workspace_upload_dir(first)
                .expect("job dir")
                .join(INCOMING_BUNDLE_FILE),
        )
        .expect("stage bundle");
        bundle_commit_in_store(
            &store,
            DispatchWorkspaceBundleCommitRequest {
                job_id: first.to_string(),
            },
        )
        .expect("bundle commit");
        let first_response = provision_in_store(&store, request_for(first)).expect("first publish");
        assert!(first_response.provisioned);

        // The second session shares the repository cache, so it needs no bundle
        // — but it must still get a checkout of its own.
        let second_response =
            provision_in_store(&store, request_for(second)).expect("second session provision");
        assert!(second_response.provisioned, "second session was refused");
        assert!(!second_response.needs_bundle);

        let first_path = first_response.workspace_path.expect("first workspace");
        let second_path = second_response.workspace_path.expect("second workspace");
        assert_ne!(
            first_path, second_path,
            "both sessions shared one worktree directory"
        );
        for path in [&first_path, &second_path] {
            assert_eq!(
                fs::read(Path::new(path).join("file.txt")).expect("checked out file"),
                b"base"
            );
        }
        // Each checkout is parked on its own managed branch.
        for (path, job_id) in [(&first_path, first), (&second_path, second)] {
            let branch = git(
                Path::new(path),
                &["symbolic-ref", "--quiet", "--short", "HEAD"],
            )
            .expect("branch");
            assert_eq!(branch.trim(), format!("bitfun/dispatch/{job_id}"));
        }

        // And re-provisioning either one is still idempotent.
        let repeat = provision_in_store(&store, request_for(first)).expect("first reprovision");
        assert_eq!(repeat.workspace_path.as_deref(), Some(first_path.as_str()));
    }

    #[test]
    fn worktree_directories_lead_with_the_project_label() {
        let label = |name: &str| {
            name.rsplit_once('-')
                .map(|(head, _)| head.to_string())
                .expect("suffixed name")
        };
        assert_eq!(
            label(&worktree_directory_name(
                Some("BitFun"),
                None,
                "dispatch-3d82ff46-bbf9-44c3"
            )),
            "BitFun"
        );
        // No label: the remote's own basename is the next most recognizable name.
        assert_eq!(
            label(&worktree_directory_name(
                None,
                Some("git@example.com:acme/app.git"),
                "abcdef123456"
            )),
            "app"
        );
        assert_eq!(
            label(&worktree_directory_name(
                None,
                Some("https://example.com/acme/app/"),
                "abcdef123456"
            )),
            "app"
        );
        // Neither available: a constant, never an empty or job-shaped path.
        assert_eq!(
            label(&worktree_directory_name(None, None, "abcdef123456")),
            "workspace"
        );
        // An id with no characters to digest still yields a usable directory.
        assert_eq!(worktree_directory_name(Some("BitFun"), None, ""), "BitFun");
    }

    /// Every job id is minted as `dispatch-<uuid>`, so a suffix sliced off the
    /// front of the id is the same for all of them. That collapsed every session
    /// of a project onto one directory and made the second one fail to provision.
    #[test]
    fn every_job_of_one_project_gets_its_own_worktree_directory() {
        let name = |job_id: &str| worktree_directory_name(Some("BitFun"), None, job_id);
        let first = name("dispatch-3d82ff46-bbf9-44c3-9c0f-2a1b0c4d5e6f");
        let second = name("dispatch-ac8fe8a3-85c7-4091-9ba4-856db2d55c2b");

        assert_ne!(first, second);
        assert!(first.starts_with("BitFun-"), "{first} lost its label");
        assert!(second.starts_with("BitFun-"), "{second} lost its label");
        // Stable across calls: a retry must land on the directory it already has.
        assert_eq!(first, name("dispatch-3d82ff46-bbf9-44c3-9c0f-2a1b0c4d5e6f"));
        // Ids that differ only past the slice window still separate.
        assert_ne!(name("dispatch-aaaaaaaa-1"), name("dispatch-aaaaaaaa-2"));
    }

    #[test]
    fn worktree_directory_names_stay_safe_path_components() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let name = worktree_directory_name(
            Some("BitFun"),
            None,
            "dispatch-3d82ff46-bbf9-44c3-9c0f-2a1b0c4d5e6f",
        );

        // `worktree_dir` is the real gate; the digest must clear it unchanged.
        let path = store
            .worktree_dir("abcdef0123456789", &name)
            .expect("worktree path");
        assert!(path.starts_with(store.worktrees_root()));
        assert!(path.ends_with(&name));
    }

    #[test]
    fn a_hostile_project_label_cannot_shape_the_worktree_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");

        for label in ["../../etc", "..", "/absolute", ".hidden", "", "   "] {
            let name = worktree_directory_name(Some(label), None, "job1");
            assert!(!name.contains('/'), "{label} produced a path separator");
            assert!(!name.contains(".."), "{label} produced a traversal");
            assert!(!name.starts_with('.'), "{label} produced a hidden entry");
            let path = store.worktree_dir("abcdef0123456789", &name).expect("path");
            assert!(path.starts_with(store.worktrees_root()));
        }

        // The store refuses anything the workspace layer did not sanitize.
        assert!(store.worktree_dir("abcdef0123456789", "../escape").is_err());
        assert!(store.worktree_dir("abcdef0123456789", ".git").is_err());
        assert!(store.worktree_dir("abcdef0123456789", "").is_err());
    }

    #[test]
    fn hostile_provisioning_inputs_are_rejected_before_git_runs() {
        assert!(validate_repo_key("../escape").is_err());
        assert!(validate_repo_key("zz").is_err());
        assert!(validate_commit("HEAD").is_err());
        assert!(validate_branch("--upload-pack=touch").is_err());
        assert!(validate_branch("feature/..\\/etc").is_err());
        assert!(validate_remote_url("ext::sh -c whoami").is_err());
        assert!(validate_remote_url("--upload-pack=touch").is_err());
        assert!(validate_remote_url("https://example.com/acme/app.git").is_ok());
    }
}
