use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use base64::Engine as _;
use bitfun_services_core::dispatch_workspace::{
    create_workspace_result_bundle, extract_workspace_snapshot, WorkspaceSnapshotManifest,
    WorkspaceSnapshotMetadata, MAX_SNAPSHOT_ARCHIVE_BYTES, MAX_SNAPSHOT_DIRECTORIES,
    MAX_SNAPSHOT_FILES, MAX_SNAPSHOT_UNCOMPRESSED_BYTES, WORKSPACE_SNAPSHOT_FORMAT_VERSION,
};
use serde::{Deserialize, Serialize};

use super::protocol::{
    DispatchWorkspaceBeginRequest, DispatchWorkspaceBeginResponse, DispatchWorkspaceChunkRequest,
    DispatchWorkspaceChunkResponse, DispatchWorkspaceCommitRequest,
    DispatchWorkspaceCommitResponse, DispatchWorkspaceResultRequest,
    DispatchWorkspaceResultResponse, DISPATCH_PROTOCOL_VERSION,
};
use super::store::{
    atomic_write_json, create_private_dir, read_json, remove_file_if_present,
    set_private_file_permissions, sync_directory, DispatchStore, JobLock,
};

const UPLOAD_RECORD_FILE: &str = "upload.json";
const UPLOAD_ARCHIVE_FILE: &str = "workspace.tar.gz";
const CURRENT_WORKSPACE_DIR: &str = "current";
/// The delivered snapshot's manifest, kept as the baseline a result diff is
/// computed against.
const BASELINE_MANIFEST_FILE: &str = "baseline-manifest.json";
const RESULT_BUNDLE_FILE: &str = "result.tar.gz";
const MAX_CHUNK_BYTES: usize = 256 * 1024;
const MAX_CHUNK_BASE64_BYTES: usize = 384 * 1024;
const MAX_MATERIALIZATION_ERROR_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WorkspaceUploadState {
    Uploading,
    Committed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceUploadRecord {
    protocol_version: u32,
    job_id: String,
    metadata: WorkspaceSnapshotMetadata,
    state: WorkspaceUploadState,
    created_at: String,
    #[serde(default)]
    committed_at: Option<String>,
    #[serde(default)]
    workspace_path: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
}

pub(crate) fn begin(
    request: DispatchWorkspaceBeginRequest,
) -> Result<DispatchWorkspaceBeginResponse> {
    validate_begin(&request)?;
    let store = DispatchStore::open_default()?;
    let upload_dir = store.workspace_upload_dir(&request.job_id)?;
    let lock_path = workspace_upload_lock_path(&store, &request.job_id);
    let Some(_lock) = JobLock::try_exclusive(&lock_path)? else {
        let existing: WorkspaceUploadRecord = read_json(&upload_dir.join(UPLOAD_RECORD_FILE))
            .context("workspace upload is currently being initialized")?;
        ensure_begin_binding(&existing, &request)?;
        ensure_upload_not_failed(&existing)?;
        if existing.state == WorkspaceUploadState::Committed {
            let workspace_path =
                validate_committed_workspace(&upload_dir, existing.workspace_path.as_deref())?;
            return Ok(DispatchWorkspaceBeginResponse {
                accepted: true,
                offset: request.metadata.archive_size,
                upload_path: upload_dir
                    .join(UPLOAD_ARCHIVE_FILE)
                    .to_string_lossy()
                    .to_string(),
                committed: true,
                workspace_path: Some(workspace_path),
            });
        }
        let archive_path = upload_dir.join(UPLOAD_ARCHIVE_FILE);
        let offset = fs::symlink_metadata(&archive_path)
            .ok()
            .filter(|metadata| !metadata.file_type().is_symlink() && metadata.is_file())
            .map(|metadata| metadata.len().min(request.metadata.archive_size))
            .unwrap_or(0);
        return Ok(DispatchWorkspaceBeginResponse {
            accepted: true,
            offset,
            upload_path: archive_path.to_string_lossy().to_string(),
            committed: false,
            workspace_path: None,
        });
    };

    let record_path = upload_dir.join(UPLOAD_RECORD_FILE);
    if let Ok(existing) = read_json::<WorkspaceUploadRecord>(&record_path) {
        ensure_begin_binding(&existing, &request)?;
        ensure_upload_not_failed(&existing)?;
        if existing.state == WorkspaceUploadState::Committed {
            let workspace_path =
                validate_committed_workspace(&upload_dir, existing.workspace_path.as_deref())?;
            return Ok(DispatchWorkspaceBeginResponse {
                accepted: true,
                offset: request.metadata.archive_size,
                upload_path: upload_dir
                    .join(UPLOAD_ARCHIVE_FILE)
                    .to_string_lossy()
                    .to_string(),
                committed: true,
                workspace_path: Some(workspace_path),
            });
        }
    } else {
        match fs::symlink_metadata(&upload_dir) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    bail!("workspace upload path is not a private directory");
                }
                fs::remove_dir_all(&upload_dir).with_context(|| {
                    format!("reset incomplete workspace upload {}", upload_dir.display())
                })?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect workspace upload {}", upload_dir.display()))
            }
        }
        create_private_dir(&upload_dir)?;
        let record = WorkspaceUploadRecord {
            protocol_version: request.protocol_version,
            job_id: request.job_id.clone(),
            metadata: request.metadata.clone(),
            state: WorkspaceUploadState::Uploading,
            created_at: chrono::Utc::now().to_rfc3339(),
            committed_at: None,
            workspace_path: None,
            last_error: None,
        };
        atomic_write_json(&record_path, &record)?;
    }

    let archive_path = upload_dir.join(UPLOAD_ARCHIVE_FILE);
    let archive_metadata = fs::symlink_metadata(&archive_path);
    let offset = match archive_metadata {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("workspace upload archive is not a regular file");
            }
            set_private_file_permissions(&archive_path)?;
            if metadata.len() > request.metadata.archive_size {
                let file = OpenOptions::new()
                    .write(true)
                    .open(&archive_path)
                    .context("open oversized workspace upload archive")?;
                file.set_len(0)
                    .context("reset oversized workspace upload archive")?;
                0
            } else {
                metadata.len()
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&archive_path)
                .context("create workspace upload archive")?;
            drop(file);
            set_private_file_permissions(&archive_path)?;
            0
        }
        Err(error) => return Err(error).context("inspect workspace upload archive"),
    };
    Ok(DispatchWorkspaceBeginResponse {
        accepted: true,
        offset,
        upload_path: archive_path.to_string_lossy().to_string(),
        committed: false,
        workspace_path: None,
    })
}

pub(crate) fn chunk(
    request: DispatchWorkspaceChunkRequest,
) -> Result<DispatchWorkspaceChunkResponse> {
    if request.data_base64.len() > MAX_CHUNK_BASE64_BYTES {
        bail!("workspace upload chunk exceeds the encoded safety limit");
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(request.data_base64.as_bytes())
        .context("decode workspace upload chunk")?;
    if data.is_empty() || data.len() > MAX_CHUNK_BYTES {
        bail!(
            "workspace upload chunk must contain 1-{} bytes",
            MAX_CHUNK_BYTES
        );
    }
    let store = DispatchStore::open_default()?;
    let upload_dir = store.workspace_upload_dir(&request.job_id)?;
    let lock_path = workspace_upload_lock_path(&store, &request.job_id);
    let _lock = JobLock::exclusive(&lock_path)?;
    let record: WorkspaceUploadRecord = read_json(&upload_dir.join(UPLOAD_RECORD_FILE))
        .context("workspace upload was not initialized")?;
    ensure_upload_identity(&record, &request.job_id)?;
    ensure_upload_not_failed(&record)?;
    if record.state != WorkspaceUploadState::Uploading {
        bail!("workspace upload is not accepting chunks");
    }
    let archive_path = upload_dir.join(UPLOAD_ARCHIVE_FILE);
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&archive_path)
        .context("open workspace upload archive")?;
    set_private_file_permissions(&archive_path)?;
    let current = file.metadata()?.len();
    let chunk_end = request.offset.saturating_add(data.len() as u64);
    if chunk_end > record.metadata.archive_size {
        bail!("workspace upload chunk exceeds the declared archive size");
    }
    if request.offset < current {
        if chunk_end > current {
            bail!("workspace upload chunk overlaps the retained archive tail");
        }
        file.seek(SeekFrom::Start(request.offset))?;
        let mut existing = vec![0_u8; data.len()];
        file.read_exact(&mut existing)?;
        if existing != data {
            bail!("workspace upload retry does not match retained bytes");
        }
        return Ok(DispatchWorkspaceChunkResponse {
            accepted: true,
            offset: current,
        });
    }
    if request.offset != current {
        bail!(
            "workspace upload offset mismatch: expected {}, received {}",
            current,
            request.offset
        );
    }
    file.seek(SeekFrom::End(0))?;
    file.write_all(&data)?;
    file.sync_data()?;
    Ok(DispatchWorkspaceChunkResponse {
        accepted: true,
        offset: chunk_end,
    })
}

pub(crate) fn commit(
    request: DispatchWorkspaceCommitRequest,
) -> Result<DispatchWorkspaceCommitResponse> {
    let store = DispatchStore::open_default()?;
    let upload_dir = store.workspace_upload_dir(&request.job_id)?;
    let lock_path = workspace_upload_lock_path(&store, &request.job_id);
    let Some(_lock) = JobLock::try_exclusive(&lock_path)? else {
        let record: WorkspaceUploadRecord = read_json(&upload_dir.join(UPLOAD_RECORD_FILE))
            .context("workspace upload was not initialized")?;
        ensure_upload_identity(&record, &request.job_id)?;
        ensure_upload_not_failed(&record)?;
        return Ok(pending_commit_response(&record));
    };
    let record_path = upload_dir.join(UPLOAD_RECORD_FILE);
    let mut record: WorkspaceUploadRecord =
        read_json(&record_path).context("workspace upload was not initialized")?;
    ensure_upload_identity(&record, &request.job_id)?;
    ensure_upload_not_failed(&record)?;
    if record.state == WorkspaceUploadState::Committed {
        let workspace_path =
            validate_committed_workspace(&upload_dir, record.workspace_path.as_deref())?;
        return Ok(DispatchWorkspaceCommitResponse {
            committed: true,
            workspace_path: Some(workspace_path),
            metadata: record.metadata,
        });
    }

    if managed_workspace_exists(&upload_dir)? {
        // Extraction publishes this directory only after every digest and
        // manifest check succeeds. Recover the narrow crash window between
        // directory publication and record publication.
        let workspace_path = mark_workspace_committed(&record_path, &upload_dir, &mut record)?;
        return Ok(DispatchWorkspaceCommitResponse {
            committed: true,
            workspace_path: Some(workspace_path),
            metadata: record.metadata,
        });
    }

    let archive_path = upload_dir.join(UPLOAD_ARCHIVE_FILE);
    validate_complete_archive(&archive_path, &record.metadata)?;
    super::runner::spawn_workspace_materializer(&request.job_id)?;
    Ok(pending_commit_response(&record))
}

/// Diff the terminal workspace against the snapshot it was given and package
/// what changed.
///
/// Only valid for snapshot-delivered jobs: a job that ran against a directory
/// the user already had has no baseline to diff against, and BitFun never took
/// ownership of that directory.
pub(crate) fn result(
    request: DispatchWorkspaceResultRequest,
) -> Result<DispatchWorkspaceResultResponse> {
    let store = DispatchStore::open_default()?;
    let upload_dir = store.workspace_upload_dir(&request.job_id)?;
    let lock_path = workspace_upload_lock_path(&store, &request.job_id);
    let _lock = JobLock::exclusive(&lock_path)?;

    let record: WorkspaceUploadRecord = read_json(&upload_dir.join(UPLOAD_RECORD_FILE))
        .context("this job did not receive a workspace snapshot")?;
    ensure_upload_identity(&record, &request.job_id)?;
    if record.state != WorkspaceUploadState::Committed {
        bail!("workspace snapshot is not committed yet");
    }
    let baseline: WorkspaceSnapshotManifest =
        read_json(&upload_dir.join(BASELINE_MANIFEST_FILE)).context(
            "this job predates result bundles; its baseline manifest was not recorded",
        )?;

    let current = upload_dir.join(CURRENT_WORKSPACE_DIR);
    if !is_real_directory(&current) {
        bail!("managed dispatch workspace is missing");
    }
    let bundle_path = upload_dir.join(RESULT_BUNDLE_FILE);
    let summary = create_workspace_result_bundle(&current, &baseline, &bundle_path)?;
    set_private_file_permissions(&bundle_path)?;
    Ok(DispatchWorkspaceResultResponse {
        bundle_path: bundle_path.to_string_lossy().to_string(),
        workspace_path: current.to_string_lossy().to_string(),
        summary,
    })
}

/// Detached target-side materialization. The short `workspace-commit` RPC
/// starts this process and subsequent commit calls poll the durable record, so
/// extraction is not bounded by an SSH or Relay request timeout.
pub(crate) fn materialize(job_id: String) -> Result<()> {
    let store = DispatchStore::open_default()?;
    materialize_in_store(&store, &job_id)
}

fn materialize_in_store(store: &DispatchStore, job_id: &str) -> Result<()> {
    let upload_dir = store.workspace_upload_dir(job_id)?;
    let lock_path = workspace_upload_lock_path(store, job_id);
    let _lock = JobLock::exclusive(&lock_path)?;
    let record_path = upload_dir.join(UPLOAD_RECORD_FILE);
    let mut record: WorkspaceUploadRecord =
        read_json(&record_path).context("workspace upload was not initialized")?;
    ensure_upload_identity(&record, job_id)?;
    ensure_upload_not_failed(&record)?;
    if record.state == WorkspaceUploadState::Committed {
        validate_committed_workspace(&upload_dir, record.workspace_path.as_deref())?;
        return Ok(());
    }
    let current = upload_dir.join(CURRENT_WORKSPACE_DIR);
    let result = (|| -> Result<()> {
        if managed_workspace_exists(&upload_dir)? {
            mark_workspace_committed(&record_path, &upload_dir, &mut record)?;
            return Ok(());
        }

        remove_stale_staging_directories(&upload_dir)?;
        let archive_path = upload_dir.join(UPLOAD_ARCHIVE_FILE);
        validate_complete_archive(&archive_path, &record.metadata)?;
        let staging = upload_dir.join(format!(".staging-{}", uuid::Uuid::new_v4().as_simple()));
        let manifest = extract_workspace_snapshot(&archive_path, &staging, &record.metadata)?;
        // Persist S0 as the baseline for a later result diff. The controller
        // deletes its own copy of the archive as soon as the job is durably
        // owned here, so this is the only surviving record of what was sent —
        // and its per-file digests are what make a diff possible for workspaces
        // that are not git repositories.
        atomic_write_json(&upload_dir.join(BASELINE_MANIFEST_FILE), &manifest)
            .context("persist dispatch workspace baseline manifest")?;
        fs::rename(&staging, &current).with_context(|| {
            format!(
                "publish dispatch workspace {} -> {}",
                staging.display(),
                current.display()
            )
        })?;
        sync_directory(&upload_dir)?;
        mark_workspace_committed(&record_path, &upload_dir, &mut record)?;
        remove_file_if_present(&archive_path);
        Ok(())
    })();
    if let Err(error) = result {
        // Once `current` exists, a later commit can recover the narrow crash
        // window between atomic publication and record publication. Before
        // publication, persist a bounded diagnostic so controllers do not
        // poll an irrecoverably bad archive until their transport timeout.
        if !is_real_directory(&current) {
            record.state = WorkspaceUploadState::Failed;
            record.last_error = Some(truncate_utf8(&format!("{error:#}")));
            let _ = atomic_write_json(&record_path, &record);
        }
        return Err(error);
    }
    Ok(())
}

fn ensure_upload_identity(record: &WorkspaceUploadRecord, job_id: &str) -> Result<()> {
    if record.job_id != job_id {
        bail!("workspace upload identity mismatch");
    }
    Ok(())
}

fn ensure_begin_binding(
    record: &WorkspaceUploadRecord,
    request: &DispatchWorkspaceBeginRequest,
) -> Result<()> {
    if record.protocol_version != request.protocol_version
        || record.job_id != request.job_id
        || record.metadata != request.metadata
    {
        bail!("workspace upload job is already bound to different snapshot metadata");
    }
    Ok(())
}

fn ensure_upload_not_failed(record: &WorkspaceUploadRecord) -> Result<()> {
    if record.state == WorkspaceUploadState::Failed {
        bail!(
            "workspace materialization failed: {}",
            record
                .last_error
                .as_deref()
                .unwrap_or("target did not retain a diagnostic")
        );
    }
    Ok(())
}

fn pending_commit_response(record: &WorkspaceUploadRecord) -> DispatchWorkspaceCommitResponse {
    DispatchWorkspaceCommitResponse {
        committed: false,
        workspace_path: None,
        metadata: record.metadata.clone(),
    }
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
}

fn managed_workspace_exists(upload_dir: &Path) -> Result<bool> {
    let current = upload_dir.join(CURRENT_WORKSPACE_DIR);
    match fs::symlink_metadata(&current) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => Ok(true),
        Ok(_) => bail!("managed dispatch workspace path is not a real directory"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("inspect managed dispatch workspace"),
    }
}

fn mark_workspace_committed(
    record_path: &Path,
    upload_dir: &Path,
    record: &mut WorkspaceUploadRecord,
) -> Result<String> {
    let workspace_path = validate_committed_workspace(upload_dir, None)?;
    record.state = WorkspaceUploadState::Committed;
    record.committed_at = Some(chrono::Utc::now().to_rfc3339());
    record.workspace_path = Some(workspace_path.clone());
    atomic_write_json(record_path, record)?;
    Ok(workspace_path)
}

fn validate_complete_archive(
    archive_path: &Path,
    metadata: &WorkspaceSnapshotMetadata,
) -> Result<()> {
    let archive =
        fs::symlink_metadata(archive_path).context("inspect complete workspace upload archive")?;
    if archive.file_type().is_symlink() || !archive.is_file() {
        bail!("workspace upload archive is not a regular file");
    }
    if archive.len() != metadata.archive_size {
        bail!(
            "workspace upload is incomplete: expected {} bytes, received {}",
            metadata.archive_size,
            archive.len()
        );
    }
    Ok(())
}

fn remove_stale_staging_directories(upload_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(upload_dir)
        .with_context(|| format!("read workspace upload directory {}", upload_dir.display()))?
    {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if !name.starts_with(".staging-") {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!("workspace upload contains an unsafe staging path");
        }
        fs::remove_dir_all(&path)
            .with_context(|| format!("remove stale workspace staging {}", path.display()))?;
    }
    Ok(())
}

fn validate_begin(request: &DispatchWorkspaceBeginRequest) -> Result<()> {
    if request.protocol_version != DISPATCH_PROTOCOL_VERSION {
        bail!(
            "unsupported dispatch protocolVersion {}; target requires {}",
            request.protocol_version,
            DISPATCH_PROTOCOL_VERSION
        );
    }
    super::store::validate_id("jobId", &request.job_id)?;
    let metadata = &request.metadata;
    if metadata.format_version != WORKSPACE_SNAPSHOT_FORMAT_VERSION {
        bail!("unsupported workspace snapshot format");
    }
    if metadata.archive_size == 0 || metadata.archive_size > MAX_SNAPSHOT_ARCHIVE_BYTES {
        bail!("workspace snapshot archive size is outside the target limit");
    }
    if metadata.file_count > MAX_SNAPSHOT_FILES
        || metadata.directory_count > MAX_SNAPSHOT_DIRECTORIES
        || metadata.uncompressed_bytes > MAX_SNAPSHOT_UNCOMPRESSED_BYTES
    {
        bail!("workspace snapshot summary exceeds target safety limits");
    }
    for digest in [&metadata.archive_sha256, &metadata.manifest_sha256] {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("workspace snapshot metadata contains an invalid SHA-256 digest");
        }
    }
    Ok(())
}

fn validate_committed_workspace(
    upload_dir: &Path,
    recorded_workspace_path: Option<&str>,
) -> Result<String> {
    let current = upload_dir.join(CURRENT_WORKSPACE_DIR);
    let metadata =
        fs::symlink_metadata(&current).context("inspect committed dispatch workspace")?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("committed dispatch workspace is not a real directory");
    }
    let canonical = current
        .canonicalize()
        .context("resolve committed dispatch workspace")?;
    if recorded_workspace_path.is_some_and(|recorded| Path::new(recorded) != canonical) {
        bail!("committed dispatch workspace path no longer matches its durable record");
    }
    canonical
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("committed dispatch workspace path is not valid UTF-8"))
}

fn workspace_upload_lock_path(store: &DispatchStore, job_id: &str) -> PathBuf {
    store
        .root()
        .join("workspaces")
        .join(format!(".{job_id}.upload.lock"))
}

fn truncate_utf8(value: &str) -> String {
    if value.len() <= MAX_MATERIALIZATION_ERROR_BYTES {
        return value.to_string();
    }
    let mut end = MAX_MATERIALIZATION_ERROR_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_services_core::dispatch_workspace::create_exact_workspace_snapshot;

    #[test]
    fn validation_rejects_unbounded_or_malformed_uploads() {
        let request = DispatchWorkspaceBeginRequest {
            protocol_version: DISPATCH_PROTOCOL_VERSION,
            job_id: "job-1".to_string(),
            metadata: WorkspaceSnapshotMetadata {
                format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
                archive_size: 1,
                archive_sha256: "x".repeat(64),
                manifest_sha256: "0".repeat(64),
                file_count: 0,
                directory_count: 0,
                uncompressed_bytes: 0,
            },
        };
        assert!(validate_begin(&request).is_err());
    }

    #[test]
    fn snapshot_fixture_metadata_is_accepted() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("file.txt"), b"hello").expect("file");
        let metadata =
            create_exact_workspace_snapshot(&source, &temp.path().join("snapshot.tar.gz"))
                .expect("snapshot");
        validate_begin(&DispatchWorkspaceBeginRequest {
            protocol_version: DISPATCH_PROTOCOL_VERSION,
            job_id: "job-1".to_string(),
            metadata,
        })
        .expect("valid metadata");
    }

    #[test]
    fn materializer_verifies_and_atomically_publishes_the_uploaded_snapshot() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("file.txt"), b"materialized").expect("source file");
        let source_archive = temp.path().join("source.tar.gz");
        let metadata = create_exact_workspace_snapshot(&source, &source_archive).expect("snapshot");
        let upload_dir = store.workspace_upload_dir("job-1").expect("upload path");
        create_private_dir(&upload_dir).expect("upload directory");
        fs::copy(&source_archive, upload_dir.join(UPLOAD_ARCHIVE_FILE)).expect("stage archive");
        atomic_write_json(
            &upload_dir.join(UPLOAD_RECORD_FILE),
            &WorkspaceUploadRecord {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                metadata: metadata.clone(),
                state: WorkspaceUploadState::Uploading,
                created_at: chrono::Utc::now().to_rfc3339(),
                committed_at: None,
                workspace_path: None,
                last_error: None,
            },
        )
        .expect("upload record");

        materialize_in_store(&store, "job-1").expect("materialize");

        assert_eq!(
            fs::read(upload_dir.join(CURRENT_WORKSPACE_DIR).join("file.txt"))
                .expect("materialized file"),
            b"materialized"
        );
        assert!(!upload_dir.join(UPLOAD_ARCHIVE_FILE).exists());
        let record: WorkspaceUploadRecord =
            read_json(&upload_dir.join(UPLOAD_RECORD_FILE)).expect("committed record");
        assert_eq!(record.state, WorkspaceUploadState::Committed);
        assert_eq!(record.metadata, metadata);
        assert!(record.workspace_path.is_some());
    }

    #[test]
    fn materialization_failure_is_persisted_for_commit_pollers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = DispatchStore::open(temp.path().join("dispatch")).expect("store");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("file.txt"), b"original").expect("source file");
        let source_archive = temp.path().join("source.tar.gz");
        let metadata = create_exact_workspace_snapshot(&source, &source_archive).expect("snapshot");
        let upload_dir = store.workspace_upload_dir("job-1").expect("upload path");
        create_private_dir(&upload_dir).expect("upload directory");
        let staged_archive = upload_dir.join(UPLOAD_ARCHIVE_FILE);
        fs::copy(&source_archive, &staged_archive).expect("stage archive");
        let mut bytes = fs::read(&staged_archive).expect("archive");
        bytes[0] ^= 1;
        fs::write(&staged_archive, bytes).expect("tamper archive");
        atomic_write_json(
            &upload_dir.join(UPLOAD_RECORD_FILE),
            &WorkspaceUploadRecord {
                protocol_version: DISPATCH_PROTOCOL_VERSION,
                job_id: "job-1".to_string(),
                metadata,
                state: WorkspaceUploadState::Uploading,
                created_at: chrono::Utc::now().to_rfc3339(),
                committed_at: None,
                workspace_path: None,
                last_error: None,
            },
        )
        .expect("upload record");

        materialize_in_store(&store, "job-1").expect_err("tampering must fail");

        let record: WorkspaceUploadRecord =
            read_json(&upload_dir.join(UPLOAD_RECORD_FILE)).expect("failed record");
        assert_eq!(record.state, WorkspaceUploadState::Failed);
        assert!(record
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("SHA-256 mismatch")));
        assert!(ensure_upload_not_failed(&record).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn committed_workspace_validation_rejects_a_replaced_symlink() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let upload = temp.path().join("upload");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&upload).expect("upload");
        fs::create_dir_all(&outside).expect("outside");
        symlink(&outside, upload.join(CURRENT_WORKSPACE_DIR)).expect("replace current");

        assert!(validate_committed_workspace(&upload, None).is_err());
    }
}
