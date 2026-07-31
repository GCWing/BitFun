//! Safe, transport-neutral workspace snapshot packaging for detached dispatch.
//!
//! A snapshot is a one-shot input boundary. It deliberately does not contain
//! Git metadata and never follows links outside the selected workspace.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path};

use anyhow::{anyhow, bail, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, EntryType, Header};

pub const WORKSPACE_SNAPSHOT_FORMAT_VERSION: u32 = 1;
pub const WORKSPACE_SNAPSHOT_SOURCE_FINGERPRINT_VERSION: u32 = 1;
pub const MAX_SNAPSHOT_FILES: u64 = 100_000;
pub const MAX_SNAPSHOT_DIRECTORIES: u64 = 100_000;
pub const MAX_SNAPSHOT_FILE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SNAPSHOT_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_SNAPSHOT_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MANIFEST_ARCHIVE_PATH: &str = ".bitfun-dispatch/manifest.json";
const RESULT_SUMMARY_ARCHIVE_PATH: &str = ".bitfun-dispatch/result.json";
const WORKSPACE_ARCHIVE_ROOT: &str = "workspace";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshotEntry {
    pub path: String,
    pub kind: WorkspaceSnapshotEntryKind,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub executable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceSnapshotEntryKind {
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshotManifest {
    pub format_version: u32,
    pub mode: String,
    pub includes_ignored_files: bool,
    pub excludes_git_metadata: bool,
    pub file_count: u64,
    pub directory_count: u64,
    pub uncompressed_bytes: u64,
    pub entries: Vec<WorkspaceSnapshotEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshotMetadata {
    pub format_version: u32,
    pub archive_size: u64,
    pub archive_sha256: String,
    pub manifest_sha256: String,
    pub file_count: u64,
    pub directory_count: u64,
    pub uncompressed_bytes: u64,
}

/// Cheaply recomputable state of the source tree used to create a snapshot.
///
/// This is a controller-local cache key, not part of the target wire metadata.
/// It covers the selected portable paths plus file identity, size, write/change
/// timestamps, and executable state without rereading file contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshotSourceFingerprint {
    pub format_version: u32,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedWorkspaceSnapshot {
    pub metadata: WorkspaceSnapshotMetadata,
    pub source_fingerprint: WorkspaceSnapshotSourceFingerprint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspaceSnapshotCaptureMode {
    Source,
    Exact,
}

impl WorkspaceSnapshotCaptureMode {
    fn manifest_mode(self) -> &'static str {
        // The transport envelope remains the existing exact-snapshot contract:
        // source filtering happens while the controller captures the input set,
        // then that complete captured set is signed and transferred exactly.
        "exact"
    }

    fn includes_ignored_files(self) -> bool {
        // True relative to the captured input set. Source mode has already
        // removed ignored paths before the manifest is constructed.
        true
    }
}

/// What the target changed, relative to the snapshot it was given.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceResultSummary {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    /// Snapshot digest of every path the target changed or removed.
    ///
    /// Carried so the controller can tell a clean apply from one that would
    /// discard local edits: if the local file still matches this, the target's
    /// change is the only one; if it does not, both sides moved.
    #[serde(default)]
    pub baseline_sha256: BTreeMap<String, String>,
    pub archive_size: u64,
    pub archive_sha256: String,
}

impl WorkspaceResultSummary {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }
}

/// Diff the terminal target tree against the delivered snapshot and package
/// only what changed.
///
/// Content-addressed rather than git-based: the baseline manifest already
/// carries a SHA-256 per file, so this works for workspaces that are not
/// repositories — which is most of the reason snapshot mode exists.
///
/// Produces the bundle only; applying it locally stays a separate, explicitly
/// confirmed step.
pub fn create_workspace_result_bundle(
    workspace: &Path,
    baseline: &WorkspaceSnapshotManifest,
    archive_path: &Path,
) -> Result<WorkspaceResultSummary> {
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("resolve dispatch workspace {}", workspace.display()))?;
    let baseline_files: BTreeMap<&str, &WorkspaceSnapshotEntry> = baseline
        .entries
        .iter()
        .filter(|entry| entry.kind == WorkspaceSnapshotEntryKind::File)
        .map(|entry| (entry.path.as_str(), entry))
        .collect();

    let archive_file = File::create(archive_path)
        .with_context(|| format!("create result bundle {}", archive_path.display()))?;
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut archive = Builder::new(encoder);
    archive.mode(tar::HeaderMode::Deterministic);

    let mut walk = WalkBuilder::new(&workspace);
    walk.hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .sort_by_file_path(|left, right| left.cmp(right));
    let filter_root = workspace.clone();
    walk.filter_entry(move |entry| {
        entry.path() == filter_root || entry.file_name().to_str() != Some(".git")
    });

    let mut summary = WorkspaceResultSummary::default();
    let mut seen = BTreeSet::new();
    let mut file_count = 0_u64;
    let mut changed_bytes = 0_u64;
    for walked in walk.build() {
        let walked = walked.context("walk dispatch workspace for result bundle")?;
        let path = walked.path();
        if path == workspace {
            continue;
        }
        let relative = path
            .strip_prefix(&workspace)
            .with_context(|| format!("resolve result path {}", path.display()))?;
        let relative_wire = portable_relative_path(relative)?;
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("inspect result entry {}", path.display()))?;
        // Same rule as packaging: an unsupported entry fails the whole
        // operation rather than yielding a bundle that silently omits it.
        if metadata.file_type().is_symlink() {
            bail!("dispatch result contains a symlink: {relative_wire}");
        }
        if metadata.is_dir() {
            continue;
        }
        if !metadata.is_file() {
            bail!("dispatch result contains an unsupported entry: {relative_wire}");
        }
        seen.insert(relative_wire.clone());
        file_count += 1;
        if file_count > MAX_SNAPSHOT_FILES {
            bail!("dispatch result exceeds the {MAX_SNAPSHOT_FILES} file limit");
        }
        if metadata.len() > MAX_SNAPSHOT_FILE_BYTES {
            bail!("dispatch result file exceeds the size limit: {relative_wire}");
        }

        let digest = sha256_file(path)?;
        let existing = baseline_files.get(relative_wire.as_str());
        let unchanged = existing
            .and_then(|entry| entry.sha256.as_deref())
            .is_some_and(|baseline_digest| baseline_digest.eq_ignore_ascii_case(&digest));
        if unchanged {
            continue;
        }

        changed_bytes = changed_bytes.saturating_add(metadata.len());
        if changed_bytes > MAX_SNAPSHOT_UNCOMPRESSED_BYTES {
            bail!("dispatch result exceeds the uncompressed size limit");
        }
        let executable = is_executable(&metadata);
        append_file(&mut archive, path, &relative_wire, &metadata, executable)?;
        if let Some(entry) = existing {
            if let Some(baseline_digest) = entry.sha256.as_deref() {
                summary
                    .baseline_sha256
                    .insert(relative_wire.clone(), baseline_digest.to_string());
            }
            summary.modified.push(relative_wire);
        } else {
            summary.added.push(relative_wire);
        }
    }

    for (path, entry) in &baseline_files {
        if seen.contains(*path) {
            continue;
        }
        summary.deleted.push((*path).to_string());
        if let Some(baseline_digest) = entry.sha256.as_deref() {
            summary
                .baseline_sha256
                .insert((*path).to_string(), baseline_digest.to_string());
        }
    }

    let summary_bytes = serde_json::to_vec(&WorkspaceResultSummary {
        archive_sha256: String::new(),
        archive_size: 0,
        ..summary.clone()
    })
    .context("encode dispatch result summary")?;
    append_bytes(
        &mut archive,
        RESULT_SUMMARY_ARCHIVE_PATH,
        &summary_bytes,
        false,
    )?;

    archive
        .into_inner()
        .context("finalize dispatch result bundle")?
        .finish()
        .context("compress dispatch result bundle")?
        .sync_all()
        .context("flush dispatch result bundle")?;

    let archive_metadata = fs::metadata(archive_path)
        .with_context(|| format!("inspect result bundle {}", archive_path.display()))?;
    summary.archive_size = archive_metadata.len();
    if summary.archive_size > MAX_SNAPSHOT_ARCHIVE_BYTES {
        bail!("dispatch result bundle exceeds the archive size limit");
    }
    summary.archive_sha256 = sha256_file(archive_path)?;
    Ok(summary)
}

/// A local path both sides changed since the snapshot was taken.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceResultConflict {
    pub path: String,
    /// Why the local file no longer matches the snapshot.
    pub reason: WorkspaceResultConflictReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceResultConflictReason {
    /// Edited locally after the snapshot, and edited on the target too.
    LocallyModified,
    /// Deleted locally, but the target changed it.
    LocallyMissing,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResultApplyOutcome {
    pub written: Vec<String>,
    pub removed: Vec<String>,
    pub conflicts: Vec<WorkspaceResultConflict>,
    /// True when nothing was touched because conflicts were found.
    pub aborted: bool,
}

/// Report which paths a result bundle would overwrite that also changed locally.
///
/// The controller and the target diverged independently after `S0`, so this is
/// the difference between "apply the target's work" and "silently discard mine".
pub fn inspect_workspace_result_conflicts(
    workspace: &Path,
    summary: &WorkspaceResultSummary,
) -> Result<Vec<WorkspaceResultConflict>> {
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("resolve workspace {}", workspace.display()))?;
    let mut conflicts = Vec::new();
    for (path, baseline_digest) in &summary.baseline_sha256 {
        let local = resolve_workspace_child(&workspace, path)?;
        match fs::symlink_metadata(&local) {
            Ok(metadata) if metadata.is_file() => {
                if !sha256_file(&local)?.eq_ignore_ascii_case(baseline_digest) {
                    conflicts.push(WorkspaceResultConflict {
                        path: path.clone(),
                        reason: WorkspaceResultConflictReason::LocallyModified,
                    });
                }
            }
            // A path the snapshot contained that is now gone or is no longer a
            // regular file: applying would resurrect or clobber it.
            Ok(_) | Err(_) => conflicts.push(WorkspaceResultConflict {
                path: path.clone(),
                reason: WorkspaceResultConflictReason::LocallyMissing,
            }),
        }
    }
    Ok(conflicts)
}

/// Apply a verified result bundle to a local workspace.
///
/// Refuses to touch anything when a conflict is found unless `overwrite` is
/// set, so the default outcome of a surprise is nothing rather than a
/// half-merged tree.
pub fn apply_workspace_result_bundle(
    bundle_path: &Path,
    workspace: &Path,
    summary: &WorkspaceResultSummary,
    overwrite: bool,
) -> Result<WorkspaceResultApplyOutcome> {
    let actual = sha256_file(bundle_path)?;
    if !actual.eq_ignore_ascii_case(&summary.archive_sha256) {
        bail!("dispatch result bundle does not match the reported digest");
    }
    let conflicts = inspect_workspace_result_conflicts(workspace, summary)?;
    if !conflicts.is_empty() && !overwrite {
        return Ok(WorkspaceResultApplyOutcome {
            conflicts,
            aborted: true,
            ..Default::default()
        });
    }
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("resolve workspace {}", workspace.display()))?;

    let expected: BTreeSet<&str> = summary
        .added
        .iter()
        .chain(summary.modified.iter())
        .map(String::as_str)
        .collect();
    let file = File::open(bundle_path)
        .with_context(|| format!("open result bundle {}", bundle_path.display()))?;
    let mut archive = Archive::new(GzDecoder::new(file));
    let mut outcome = WorkspaceResultApplyOutcome {
        conflicts,
        ..Default::default()
    };
    for entry in archive.entries().context("read result bundle")? {
        let mut entry = entry.context("read result bundle entry")?;
        let entry_path = entry.path().context("read result bundle entry path")?;
        let Ok(relative) = entry_path.strip_prefix(WORKSPACE_ARCHIVE_ROOT) else {
            continue; // the bundle's own metadata
        };
        let relative_wire = portable_relative_path(relative)?;
        if !expected.contains(relative_wire.as_str()) {
            bail!("dispatch result bundle contains an unreported path: {relative_wire}");
        }
        if entry.header().entry_type() != EntryType::Regular {
            bail!("dispatch result bundle contains a non-regular entry: {relative_wire}");
        }
        let destination = resolve_workspace_child(&workspace, &relative_wire)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .with_context(|| format!("read {relative_wire} from result bundle"))?;
        fs::write(&destination, &bytes)
            .with_context(|| format!("write {}", destination.display()))?;
        outcome.written.push(relative_wire);
    }

    for path in &summary.deleted {
        let destination = resolve_workspace_child(&workspace, path)?;
        match fs::symlink_metadata(&destination) {
            Ok(metadata) if metadata.is_file() => {
                fs::remove_file(&destination)
                    .with_context(|| format!("remove {}", destination.display()))?;
                outcome.removed.push(path.clone());
            }
            // Already gone locally: the desired end state, nothing to do.
            _ => {}
        }
    }
    Ok(outcome)
}

/// Join a manifest-relative path under the workspace, refusing anything that
/// would land outside it.
fn resolve_workspace_child(workspace: &Path, relative_wire: &str) -> Result<std::path::PathBuf> {
    if relative_wire.is_empty() {
        bail!("dispatch result path is empty");
    }
    let mut resolved = workspace.to_path_buf();
    for part in relative_wire.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            bail!("dispatch result path is not workspace-relative: {relative_wire}");
        }
        resolved.push(part);
    }
    ensure_path_below(workspace, &resolved)?;
    Ok(resolved)
}

/// Package every regular workspace file, including hidden and ignored files.
///
/// `.git` entries are the one explicit metadata exclusion. Unsupported entries
/// fail the whole operation instead of producing an incomplete snapshot.
pub fn create_exact_workspace_snapshot(
    source: &Path,
    archive_path: &Path,
) -> Result<WorkspaceSnapshotMetadata> {
    Ok(prepare_exact_workspace_snapshot(source, archive_path)?.metadata)
}

pub fn prepare_exact_workspace_snapshot(
    source: &Path,
    archive_path: &Path,
) -> Result<PreparedWorkspaceSnapshot> {
    create_workspace_snapshot(source, archive_path, WorkspaceSnapshotCaptureMode::Exact)
}

/// Package workspace source while honoring repository ignore rules.
///
/// Hidden source files remain eligible (for example `.github/workflows`), but
/// ignored dependency caches and build output are not transferred. Callers
/// that need byte-for-byte workspace contents must use the explicit exact
/// snapshot path instead.
pub fn create_source_workspace_snapshot(
    source: &Path,
    archive_path: &Path,
) -> Result<WorkspaceSnapshotMetadata> {
    Ok(prepare_source_workspace_snapshot(source, archive_path)?.metadata)
}

pub fn prepare_source_workspace_snapshot(
    source: &Path,
    archive_path: &Path,
) -> Result<PreparedWorkspaceSnapshot> {
    create_workspace_snapshot(source, archive_path, WorkspaceSnapshotCaptureMode::Source)
}

pub fn exact_workspace_snapshot_source_fingerprint(
    source: &Path,
) -> Result<WorkspaceSnapshotSourceFingerprint> {
    workspace_snapshot_source_fingerprint(source, WorkspaceSnapshotCaptureMode::Exact)
}

pub fn source_workspace_snapshot_source_fingerprint(
    source: &Path,
) -> Result<WorkspaceSnapshotSourceFingerprint> {
    workspace_snapshot_source_fingerprint(source, WorkspaceSnapshotCaptureMode::Source)
}

fn create_workspace_snapshot(
    source: &Path,
    archive_path: &Path,
    capture_mode: WorkspaceSnapshotCaptureMode,
) -> Result<PreparedWorkspaceSnapshot> {
    let result = create_workspace_snapshot_inner(source, archive_path, capture_mode);
    if result.is_err() {
        let _ = fs::remove_file(archive_path);
    }
    result
}

fn create_workspace_snapshot_inner(
    source: &Path,
    archive_path: &Path,
    capture_mode: WorkspaceSnapshotCaptureMode,
) -> Result<PreparedWorkspaceSnapshot> {
    let source_metadata = fs::symlink_metadata(source)
        .with_context(|| format!("inspect workspace {}", source.display()))?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        bail!(
            "workspace snapshot source is not a real directory: {}",
            source.display()
        );
    }
    let source = source
        .canonicalize()
        .with_context(|| format!("resolve workspace {}", source.display()))?;
    let archive_parent = archive_path
        .parent()
        .ok_or_else(|| anyhow!("workspace archive path has no parent"))?;
    fs::create_dir_all(archive_parent)
        .with_context(|| format!("create snapshot staging {}", archive_parent.display()))?;

    let archive_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(archive_path)
        .with_context(|| format!("create workspace snapshot {}", archive_path.display()))?;
    set_private_file_permissions(archive_path)?;
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut archive = Builder::new(encoder);
    archive.mode(tar::HeaderMode::Deterministic);

    let mut entries = Vec::new();
    let mut file_count = 0_u64;
    let mut directory_count = 0_u64;
    let mut uncompressed_bytes = 0_u64;
    let mut source_fingerprint = new_source_fingerprint(capture_mode);
    for walked in workspace_walk(&source, capture_mode) {
        let walked = walked.context("walk workspace for dispatch snapshot")?;
        let path = walked.path();
        if path == source {
            continue;
        }
        let relative = path
            .strip_prefix(&source)
            .with_context(|| format!("resolve snapshot path {}", path.display()))?;
        let relative_wire = portable_relative_path(relative)?;
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("inspect snapshot entry {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!(
                "workspace snapshot does not support symbolic link '{}'",
                relative_wire
            );
        }
        if metadata.is_dir() {
            directory_count = directory_count.saturating_add(1);
            if directory_count > MAX_SNAPSHOT_DIRECTORIES {
                bail!(
                    "workspace snapshot exceeds the {} directory limit",
                    MAX_SNAPSHOT_DIRECTORIES
                );
            }
            append_directory(&mut archive, &relative_wire)?;
            update_directory_source_fingerprint(&mut source_fingerprint, &relative_wire);
            entries.push(WorkspaceSnapshotEntry {
                path: relative_wire,
                kind: WorkspaceSnapshotEntryKind::Directory,
                size: 0,
                sha256: None,
                executable: false,
            });
            continue;
        }
        if !metadata.is_file() {
            bail!(
                "workspace snapshot contains unsupported special file '{}'",
                relative_wire
            );
        }
        let size = metadata.len();
        if size > MAX_SNAPSHOT_FILE_BYTES {
            bail!(
                "workspace snapshot file '{}' exceeds the {} MiB per-file limit",
                relative_wire,
                MAX_SNAPSHOT_FILE_BYTES / (1024 * 1024)
            );
        }
        file_count = file_count.saturating_add(1);
        if file_count > MAX_SNAPSHOT_FILES {
            bail!(
                "workspace snapshot exceeds the {} file limit",
                MAX_SNAPSHOT_FILES
            );
        }
        uncompressed_bytes = uncompressed_bytes.saturating_add(size);
        if uncompressed_bytes > MAX_SNAPSHOT_UNCOMPRESSED_BYTES {
            bail!(
                "workspace snapshot exceeds the {} MiB uncompressed limit",
                MAX_SNAPSHOT_UNCOMPRESSED_BYTES / (1024 * 1024)
            );
        }
        let executable = is_executable(&metadata);
        update_file_source_fingerprint(
            &mut source_fingerprint,
            &relative_wire,
            &metadata,
            executable,
        )?;
        let sha256 = append_file(&mut archive, path, &relative_wire, &metadata, executable)?;
        entries.push(WorkspaceSnapshotEntry {
            path: relative_wire,
            kind: WorkspaceSnapshotEntryKind::File,
            size,
            sha256: Some(sha256),
            executable,
        });
    }

    let manifest = WorkspaceSnapshotManifest {
        format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
        mode: capture_mode.manifest_mode().to_string(),
        includes_ignored_files: capture_mode.includes_ignored_files(),
        excludes_git_metadata: true,
        file_count,
        directory_count,
        uncompressed_bytes,
        entries,
    };
    let manifest_bytes =
        serde_json::to_vec(&manifest).context("encode workspace snapshot manifest")?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        bail!("workspace snapshot manifest exceeds the safety limit");
    }
    append_bytes(&mut archive, MANIFEST_ARCHIVE_PATH, &manifest_bytes, false)?;
    let manifest_sha256 = sha256_bytes(&manifest_bytes);

    let encoder = archive
        .into_inner()
        .context("finish workspace snapshot tar stream")?;
    let archive_file = encoder
        .finish()
        .context("finish workspace snapshot compression")?;
    archive_file
        .sync_all()
        .context("sync workspace snapshot archive")?;
    let archive_size = archive_file
        .metadata()
        .context("inspect workspace snapshot archive")?
        .len();
    drop(archive_file);
    if archive_size > MAX_SNAPSHOT_ARCHIVE_BYTES {
        bail!(
            "workspace snapshot archive exceeds the {} MiB compressed limit",
            MAX_SNAPSHOT_ARCHIVE_BYTES / (1024 * 1024)
        );
    }
    let archive_sha256 = sha256_file(archive_path)?;
    Ok(PreparedWorkspaceSnapshot {
        metadata: WorkspaceSnapshotMetadata {
            format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
            archive_size,
            archive_sha256,
            manifest_sha256,
            file_count,
            directory_count,
            uncompressed_bytes,
        },
        source_fingerprint: finish_source_fingerprint(source_fingerprint),
    })
}

fn workspace_snapshot_source_fingerprint(
    source: &Path,
    capture_mode: WorkspaceSnapshotCaptureMode,
) -> Result<WorkspaceSnapshotSourceFingerprint> {
    let source_metadata = fs::symlink_metadata(source)
        .with_context(|| format!("inspect workspace {}", source.display()))?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        bail!(
            "workspace snapshot source is not a real directory: {}",
            source.display()
        );
    }
    let source = source
        .canonicalize()
        .with_context(|| format!("resolve workspace {}", source.display()))?;
    let mut fingerprint = new_source_fingerprint(capture_mode);
    let mut file_count = 0_u64;
    let mut directory_count = 0_u64;
    let mut uncompressed_bytes = 0_u64;
    for walked in workspace_walk(&source, capture_mode) {
        let walked = walked.context("walk workspace for dispatch snapshot fingerprint")?;
        let path = walked.path();
        if path == source {
            continue;
        }
        let relative = path
            .strip_prefix(&source)
            .with_context(|| format!("resolve snapshot path {}", path.display()))?;
        let relative_wire = portable_relative_path(relative)?;
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("inspect snapshot entry {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!(
                "workspace snapshot does not support symbolic link '{}'",
                relative_wire
            );
        }
        if metadata.is_dir() {
            directory_count = directory_count.saturating_add(1);
            if directory_count > MAX_SNAPSHOT_DIRECTORIES {
                bail!(
                    "workspace snapshot exceeds the {} directory limit",
                    MAX_SNAPSHOT_DIRECTORIES
                );
            }
            update_directory_source_fingerprint(&mut fingerprint, &relative_wire);
            continue;
        }
        if !metadata.is_file() {
            bail!(
                "workspace snapshot contains unsupported special file '{}'",
                relative_wire
            );
        }
        if metadata.len() > MAX_SNAPSHOT_FILE_BYTES {
            bail!(
                "workspace snapshot file '{}' exceeds the {} MiB per-file limit",
                relative_wire,
                MAX_SNAPSHOT_FILE_BYTES / (1024 * 1024)
            );
        }
        file_count = file_count.saturating_add(1);
        if file_count > MAX_SNAPSHOT_FILES {
            bail!(
                "workspace snapshot exceeds the {} file limit",
                MAX_SNAPSHOT_FILES
            );
        }
        uncompressed_bytes = uncompressed_bytes.saturating_add(metadata.len());
        if uncompressed_bytes > MAX_SNAPSHOT_UNCOMPRESSED_BYTES {
            bail!(
                "workspace snapshot exceeds the {} MiB uncompressed limit",
                MAX_SNAPSHOT_UNCOMPRESSED_BYTES / (1024 * 1024)
            );
        }
        update_file_source_fingerprint(
            &mut fingerprint,
            &relative_wire,
            &metadata,
            is_executable(&metadata),
        )?;
    }
    Ok(finish_source_fingerprint(fingerprint))
}

fn workspace_walk(source: &Path, capture_mode: WorkspaceSnapshotCaptureMode) -> ignore::Walk {
    let mut walk = WalkBuilder::new(source);
    walk.hidden(false).follow_links(false);
    match capture_mode {
        WorkspaceSnapshotCaptureMode::Source => {
            walk.ignore(true)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .require_git(false)
                .parents(false);
        }
        WorkspaceSnapshotCaptureMode::Exact => {
            walk.ignore(false)
                .git_ignore(false)
                .git_global(false)
                .git_exclude(false)
                .parents(false);
        }
    }
    walk.sort_by_file_path(|left, right| left.cmp(right));
    let filter_root = source.to_path_buf();
    walk.filter_entry(move |entry| {
        entry.path() == filter_root || entry.file_name().to_str() != Some(".git")
    });
    walk.build()
}

fn new_source_fingerprint(capture_mode: WorkspaceSnapshotCaptureMode) -> Sha256 {
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"bitfun-dispatch-workspace-source-fingerprint");
    fingerprint.update(WORKSPACE_SNAPSHOT_SOURCE_FINGERPRINT_VERSION.to_le_bytes());
    fingerprint.update(match capture_mode {
        WorkspaceSnapshotCaptureMode::Source => b"source".as_slice(),
        WorkspaceSnapshotCaptureMode::Exact => b"exact".as_slice(),
    });
    fingerprint
}

fn update_directory_source_fingerprint(fingerprint: &mut Sha256, relative_wire: &str) {
    update_fingerprint_field(fingerprint, b"directory");
    update_fingerprint_field(fingerprint, relative_wire.as_bytes());
}

fn update_file_source_fingerprint(
    fingerprint: &mut Sha256,
    relative_wire: &str,
    metadata: &fs::Metadata,
    executable: bool,
) -> Result<()> {
    update_fingerprint_field(fingerprint, b"file");
    update_fingerprint_field(fingerprint, relative_wire.as_bytes());
    fingerprint.update(metadata.len().to_le_bytes());
    fingerprint.update([u8::from(executable)]);
    update_platform_file_fingerprint(fingerprint, metadata)
}

#[cfg(unix)]
fn update_platform_file_fingerprint(
    fingerprint: &mut Sha256,
    metadata: &fs::Metadata,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    fingerprint.update(metadata.dev().to_le_bytes());
    fingerprint.update(metadata.ino().to_le_bytes());
    fingerprint.update(metadata.mode().to_le_bytes());
    fingerprint.update(metadata.mtime().to_le_bytes());
    fingerprint.update(metadata.mtime_nsec().to_le_bytes());
    fingerprint.update(metadata.ctime().to_le_bytes());
    fingerprint.update(metadata.ctime_nsec().to_le_bytes());
    Ok(())
}

#[cfg(windows)]
fn update_platform_file_fingerprint(
    fingerprint: &mut Sha256,
    metadata: &fs::Metadata,
) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    fingerprint.update(metadata.file_attributes().to_le_bytes());
    fingerprint.update(metadata.creation_time().to_le_bytes());
    fingerprint.update(metadata.last_write_time().to_le_bytes());
    fingerprint.update(metadata.file_size().to_le_bytes());
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn update_platform_file_fingerprint(
    fingerprint: &mut Sha256,
    metadata: &fs::Metadata,
) -> Result<()> {
    use std::time::UNIX_EPOCH;
    let modified = metadata
        .modified()
        .context("read workspace file modification time")?;
    let (before_epoch, duration) = match modified.duration_since(UNIX_EPOCH) {
        Ok(duration) => (false, duration),
        Err(error) => (true, error.duration()),
    };
    fingerprint.update([u8::from(before_epoch)]);
    fingerprint.update(duration.as_secs().to_le_bytes());
    fingerprint.update(duration.subsec_nanos().to_le_bytes());
    fingerprint.update([u8::from(metadata.permissions().readonly())]);
    Ok(())
}

fn update_fingerprint_field(fingerprint: &mut Sha256, value: &[u8]) {
    fingerprint.update((value.len() as u64).to_le_bytes());
    fingerprint.update(value);
}

fn finish_source_fingerprint(fingerprint: Sha256) -> WorkspaceSnapshotSourceFingerprint {
    WorkspaceSnapshotSourceFingerprint {
        format_version: WORKSPACE_SNAPSHOT_SOURCE_FINGERPRINT_VERSION,
        sha256: format!("{:x}", fingerprint.finalize()),
    }
}

/// Verify and extract a snapshot into a brand-new staging directory.
///
/// Callers publish the directory atomically only after this returns. This
/// function never removes or overwrites an existing destination.
pub fn extract_workspace_snapshot(
    archive_path: &Path,
    destination: &Path,
    expected: &WorkspaceSnapshotMetadata,
) -> Result<WorkspaceSnapshotManifest> {
    if expected.format_version != WORKSPACE_SNAPSHOT_FORMAT_VERSION {
        bail!(
            "unsupported workspace snapshot format {}; target requires {}",
            expected.format_version,
            WORKSPACE_SNAPSHOT_FORMAT_VERSION
        );
    }
    let archive_metadata = fs::symlink_metadata(archive_path)
        .with_context(|| format!("inspect workspace archive {}", archive_path.display()))?;
    if archive_metadata.file_type().is_symlink() || !archive_metadata.is_file() {
        bail!("workspace snapshot archive is not a regular file");
    }
    if archive_metadata.len() != expected.archive_size {
        bail!(
            "workspace snapshot archive size mismatch: expected {}, received {}",
            expected.archive_size,
            archive_metadata.len()
        );
    }
    if archive_metadata.len() > MAX_SNAPSHOT_ARCHIVE_BYTES {
        bail!("workspace snapshot archive exceeds the target safety limit");
    }
    let actual_archive_sha256 = sha256_file(archive_path)?;
    if !actual_archive_sha256.eq_ignore_ascii_case(&expected.archive_sha256) {
        bail!("workspace snapshot archive SHA-256 mismatch");
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => bail!(
            "workspace snapshot destination already exists: {}",
            destination.display()
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("inspect snapshot destination {}", destination.display()))
        }
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("create snapshot destination {}", destination.display()))?;
    set_private_directory_permissions(destination)?;

    let result = extract_workspace_snapshot_inner(archive_path, destination, expected);
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}

fn extract_workspace_snapshot_inner(
    archive_path: &Path,
    destination: &Path,
    expected: &WorkspaceSnapshotMetadata,
) -> Result<WorkspaceSnapshotManifest> {
    let decoder = GzDecoder::new(
        File::open(archive_path)
            .with_context(|| format!("open workspace archive {}", archive_path.display()))?,
    );
    let mut archive = Archive::new(decoder);
    let mut actual_entries: BTreeMap<String, WorkspaceSnapshotEntry> = BTreeMap::new();
    let mut seen_archive_paths = HashSet::new();
    let mut manifest_bytes: Option<Vec<u8>> = None;
    let mut file_count = 0_u64;
    let mut directory_count = 0_u64;
    let mut uncompressed_bytes = 0_u64;

    for entry in archive
        .entries()
        .context("read workspace snapshot archive")?
    {
        let mut entry = entry.context("read workspace snapshot entry")?;
        let entry_path = entry
            .path()
            .context("decode workspace snapshot path")?
            .into_owned();
        let entry_wire = portable_relative_path(&entry_path)?;
        if !seen_archive_paths.insert(entry_wire.clone()) {
            bail!("workspace snapshot contains duplicate entry '{entry_wire}'");
        }
        if entry_wire == MANIFEST_ARCHIVE_PATH {
            if entry.header().entry_type() != EntryType::Regular {
                bail!("workspace snapshot manifest is not a regular file");
            }
            if entry.size() > MAX_MANIFEST_BYTES {
                bail!("workspace snapshot manifest exceeds the safety limit");
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut bytes)
                .context("read workspace snapshot manifest")?;
            manifest_bytes = Some(bytes);
            continue;
        }

        let relative = entry_path
            .strip_prefix(WORKSPACE_ARCHIVE_ROOT)
            .with_context(|| format!("unexpected workspace snapshot entry '{entry_wire}'"))?;
        let relative_wire = portable_relative_path(relative)?;
        if relative_wire.is_empty() {
            bail!("workspace snapshot contains an empty workspace entry");
        }
        if contains_git_metadata(relative) {
            bail!("workspace snapshot contains forbidden Git metadata '{relative_wire}'");
        }
        if let Some(parent) = relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            let parent_wire = portable_relative_path(parent)?;
            if !matches!(
                actual_entries.get(&parent_wire).map(|entry| entry.kind),
                Some(WorkspaceSnapshotEntryKind::Directory)
            ) {
                bail!(
                    "workspace snapshot entry '{}' appears before its declared parent directory '{}'",
                    relative_wire,
                    parent_wire
                );
            }
        }
        let output_path = destination.join(relative);
        ensure_path_below(destination, &output_path)?;
        match entry.header().entry_type() {
            EntryType::Directory => {
                directory_count = directory_count.saturating_add(1);
                if directory_count > MAX_SNAPSHOT_DIRECTORIES {
                    bail!("workspace snapshot exceeds the target directory limit");
                }
                fs::create_dir_all(&output_path).with_context(|| {
                    format!("create snapshot directory {}", output_path.display())
                })?;
                set_private_directory_permissions(&output_path)?;
                actual_entries.insert(
                    relative_wire.clone(),
                    WorkspaceSnapshotEntry {
                        path: relative_wire,
                        kind: WorkspaceSnapshotEntryKind::Directory,
                        size: 0,
                        sha256: None,
                        executable: false,
                    },
                );
            }
            EntryType::Regular => {
                let size = entry.size();
                if size > MAX_SNAPSHOT_FILE_BYTES {
                    bail!("workspace snapshot file '{relative_wire}' exceeds the target limit");
                }
                file_count = file_count.saturating_add(1);
                if file_count > MAX_SNAPSHOT_FILES {
                    bail!("workspace snapshot exceeds the target file limit");
                }
                uncompressed_bytes = uncompressed_bytes.saturating_add(size);
                if uncompressed_bytes > MAX_SNAPSHOT_UNCOMPRESSED_BYTES {
                    bail!("workspace snapshot exceeds the target uncompressed limit");
                }
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("create snapshot parent {}", parent.display()))?;
                    set_private_directory_permissions(parent)?;
                }
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output_path)
                    .with_context(|| format!("create snapshot file {}", output_path.display()))?;
                set_private_file_permissions(&output_path)?;
                let mut hashing = HashingReader::new(&mut entry);
                io::copy(&mut hashing, &mut output)
                    .with_context(|| format!("extract snapshot file '{relative_wire}'"))?;
                let (actual_size, sha256) = hashing.finish();
                if actual_size != size {
                    bail!(
                        "workspace snapshot file '{}' size mismatch: expected {}, extracted {}",
                        relative_wire,
                        size,
                        actual_size
                    );
                }
                output
                    .sync_all()
                    .with_context(|| format!("sync snapshot file {}", output_path.display()))?;
                let executable = entry.header().mode().unwrap_or(0) & 0o111 != 0;
                set_snapshot_file_permissions(&output_path, executable)?;
                actual_entries.insert(
                    relative_wire.clone(),
                    WorkspaceSnapshotEntry {
                        path: relative_wire,
                        kind: WorkspaceSnapshotEntryKind::File,
                        size,
                        sha256: Some(sha256),
                        executable,
                    },
                );
            }
            other => bail!(
                "workspace snapshot entry '{}' has unsupported archive type {:?}",
                relative_wire,
                other
            ),
        }
    }

    let manifest_bytes =
        manifest_bytes.ok_or_else(|| anyhow!("workspace snapshot manifest is missing"))?;
    if !sha256_bytes(&manifest_bytes).eq_ignore_ascii_case(&expected.manifest_sha256) {
        bail!("workspace snapshot manifest SHA-256 mismatch");
    }
    let manifest: WorkspaceSnapshotManifest =
        serde_json::from_slice(&manifest_bytes).context("decode workspace snapshot manifest")?;
    validate_manifest(&manifest, expected)?;
    let expected_entries = manifest
        .entries
        .iter()
        .map(|entry| (entry.path.clone(), entry.clone()))
        .collect::<BTreeMap<_, _>>();
    if expected_entries.len() != manifest.entries.len() {
        bail!("workspace snapshot manifest contains duplicate paths");
    }
    if expected_entries != actual_entries {
        bail!("workspace snapshot contents do not match the signed manifest");
    }
    sync_directory(destination)?;
    Ok(manifest)
}

fn validate_manifest(
    manifest: &WorkspaceSnapshotManifest,
    expected: &WorkspaceSnapshotMetadata,
) -> Result<()> {
    let compatible_capture_mode = match manifest.mode.as_str() {
        "exact" => manifest.includes_ignored_files,
        "source" => !manifest.includes_ignored_files,
        _ => false,
    };
    if manifest.format_version != WORKSPACE_SNAPSHOT_FORMAT_VERSION
        || !compatible_capture_mode
        || !manifest.excludes_git_metadata
    {
        bail!("workspace snapshot manifest contract is incompatible");
    }
    if manifest.file_count != expected.file_count
        || manifest.directory_count != expected.directory_count
        || manifest.uncompressed_bytes != expected.uncompressed_bytes
    {
        bail!("workspace snapshot manifest summary does not match upload metadata");
    }
    if manifest.file_count > MAX_SNAPSHOT_FILES
        || manifest.directory_count > MAX_SNAPSHOT_DIRECTORIES
        || manifest.uncompressed_bytes > MAX_SNAPSHOT_UNCOMPRESSED_BYTES
    {
        bail!("workspace snapshot manifest exceeds target safety limits");
    }
    let mut entry_file_count = 0_u64;
    let mut entry_directory_count = 0_u64;
    let mut entry_uncompressed_bytes = 0_u64;
    for entry in &manifest.entries {
        let path = Path::new(&entry.path);
        if entry.path.is_empty()
            || portable_relative_path(path)? != entry.path
            || contains_git_metadata(path)
        {
            bail!(
                "workspace snapshot manifest contains invalid path '{}'",
                entry.path
            );
        }
        match entry.kind {
            WorkspaceSnapshotEntryKind::File => {
                entry_file_count = entry_file_count.saturating_add(1);
                entry_uncompressed_bytes = entry_uncompressed_bytes.saturating_add(entry.size);
                if entry.size > MAX_SNAPSHOT_FILE_BYTES {
                    bail!("workspace snapshot manifest contains an oversized file");
                }
                if entry
                    .sha256
                    .as_deref()
                    .is_none_or(|hash| !valid_sha256(hash))
                {
                    bail!("workspace snapshot manifest has an invalid file digest");
                }
            }
            WorkspaceSnapshotEntryKind::Directory => {
                entry_directory_count = entry_directory_count.saturating_add(1);
                if entry.size != 0 || entry.sha256.is_some() || entry.executable {
                    bail!("workspace snapshot manifest has invalid directory metadata");
                }
            }
        }
    }
    if entry_file_count != manifest.file_count
        || entry_directory_count != manifest.directory_count
        || entry_uncompressed_bytes != manifest.uncompressed_bytes
    {
        bail!("workspace snapshot manifest summary does not match its entries");
    }
    Ok(())
}

fn append_directory<W: Write>(archive: &mut Builder<W>, relative_wire: &str) -> Result<()> {
    let archive_path = format!("{WORKSPACE_ARCHIVE_ROOT}/{relative_wire}");
    let mut header = safe_header(0, 0o755, EntryType::Directory);
    archive
        .append_data(&mut header, &archive_path, io::empty())
        .with_context(|| format!("append snapshot directory '{relative_wire}'"))
}

fn append_file<W: Write>(
    archive: &mut Builder<W>,
    path: &Path,
    relative_wire: &str,
    before: &fs::Metadata,
    executable: bool,
) -> Result<String> {
    let mut input =
        File::open(path).with_context(|| format!("open snapshot file {}", path.display()))?;
    let mut hashing = HashingReader::new(&mut input);
    let archive_path = format!("{WORKSPACE_ARCHIVE_ROOT}/{relative_wire}");
    let mut header = safe_header(
        before.len(),
        if executable { 0o755 } else { 0o644 },
        EntryType::Regular,
    );
    archive
        .append_data(&mut header, &archive_path, &mut hashing)
        .with_context(|| format!("append snapshot file '{relative_wire}'"))?;
    let (read_size, sha256) = hashing.finish();
    if read_size != before.len() {
        bail!(
            "workspace file '{}' changed size while the snapshot was being created",
            relative_wire
        );
    }
    let after = fs::symlink_metadata(path)
        .with_context(|| format!("reinspect snapshot file {}", path.display()))?;
    if !after.is_file()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        bail!(
            "workspace file '{}' changed while the snapshot was being created",
            relative_wire
        );
    }
    Ok(sha256)
}

fn append_bytes<W: Write>(
    archive: &mut Builder<W>,
    path: &str,
    bytes: &[u8],
    executable: bool,
) -> Result<()> {
    let mut header = safe_header(
        bytes.len() as u64,
        if executable { 0o755 } else { 0o600 },
        EntryType::Regular,
    );
    archive
        .append_data(&mut header, path, bytes)
        .with_context(|| format!("append snapshot metadata '{path}'"))
}

fn safe_header(size: u64, mode: u32, entry_type: EntryType) -> Header {
    let mut header = Header::new_gnu();
    header.set_size(size);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_entry_type(entry_type);
    header
}

fn portable_relative_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    anyhow!(
                        "workspace snapshot path is not portable UTF-8: {}",
                        path.display()
                    )
                })?;
                if part.is_empty() || part == "." || part == ".." {
                    bail!("workspace snapshot path is unsafe: {}", path.display());
                }
                parts.push(part);
            }
            _ => bail!(
                "workspace snapshot path is not relative: {}",
                path.display()
            ),
        }
    }
    Ok(parts.join("/"))
}

fn ensure_path_below(root: &Path, path: &Path) -> Result<()> {
    if path == root || !path.starts_with(root) {
        bail!(
            "workspace snapshot entry escapes target directory: {}",
            path.display()
        );
    }
    Ok(())
}

fn contains_git_metadata(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, Component::Normal(part) if part == std::ffi::OsStr::new(".git"))
    })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Digest of an in-memory buffer, the counterpart of [`sha256_file`].
pub fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("hash {}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

struct HashingReader<R> {
    inner: R,
    digest: Sha256,
    bytes: u64,
}

impl<R> HashingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
            bytes: 0,
        }
    }

    fn finish(self) -> (u64, String) {
        (self.bytes, format!("{:x}", self.digest.finalize()))
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.digest.update(&buffer[..read]);
        self.bytes = self.bytes.saturating_add(read as u64);
        Ok(read)
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("set private permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_snapshot_file_permissions(path: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
    )
    .with_context(|| format!("set snapshot permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_snapshot_file_permissions(_path: &Path, _executable: bool) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("set private permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)
            .with_context(|| format!("open directory {}", path.display()))?
            .sync_all()
            .with_context(|| format!("sync directory {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_snapshot_round_trips_hidden_and_ignored_files_without_git_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(source.join("empty")).expect("empty directory");
        fs::create_dir_all(source.join(".git/objects")).expect("git metadata");
        fs::write(source.join("visible.txt"), b"visible").expect("visible");
        fs::write(source.join(".env"), b"SECRET=test").expect("ignored-like file");
        fs::write(source.join(".git/config"), b"credential=never").expect("git config");
        let archive = temp.path().join("snapshot.tar.gz");

        let metadata = create_exact_workspace_snapshot(&source, &archive).expect("create snapshot");
        assert_eq!(metadata.file_count, 2);
        let destination = temp.path().join("destination");
        let manifest =
            extract_workspace_snapshot(&archive, &destination, &metadata).expect("extract");

        assert_eq!(
            fs::read(destination.join("visible.txt")).expect("visible output"),
            b"visible"
        );
        assert_eq!(
            fs::read(destination.join(".env")).expect("hidden output"),
            b"SECRET=test"
        );
        assert!(destination.join("empty").is_dir());
        assert!(!destination.join(".git").exists());
        assert!(manifest.includes_ignored_files);
        assert!(manifest.excludes_git_metadata);
    }

    #[test]
    fn source_snapshot_keeps_hidden_source_and_excludes_ignored_build_output() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(source.join(".git")).expect("repository marker");
        fs::create_dir_all(source.join(".github/workflows")).expect("hidden source directory");
        fs::create_dir_all(source.join("target/debug")).expect("ignored build directory");
        fs::write(source.join(".gitignore"), b"target/\n.env\n").expect("gitignore");
        fs::write(source.join(".github/workflows/check.yml"), b"name: check")
            .expect("hidden source file");
        fs::write(source.join("source.rs"), b"fn main() {}").expect("source");
        fs::write(source.join(".env"), b"SECRET=test").expect("ignored secret");
        fs::write(source.join("target/debug/app"), b"build output").expect("build output");
        let archive = temp.path().join("source-snapshot.tar.gz");

        let metadata =
            create_source_workspace_snapshot(&source, &archive).expect("create source snapshot");
        let destination = temp.path().join("destination");
        let manifest =
            extract_workspace_snapshot(&archive, &destination, &metadata).expect("extract");

        assert_eq!(manifest.mode, "exact");
        assert!(manifest.includes_ignored_files);
        assert_eq!(
            fs::read(destination.join(".github/workflows/check.yml")).expect("workflow"),
            b"name: check"
        );
        assert!(destination.join("source.rs").is_file());
        assert!(destination.join(".gitignore").is_file());
        assert!(!destination.join(".env").exists());
        assert!(!destination.join("target").exists());
    }

    #[test]
    fn source_fingerprint_reuses_ignored_state_and_invalidates_included_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(source.join(".git")).expect("repository marker");
        fs::create_dir_all(source.join("target")).expect("ignored directory");
        fs::write(source.join(".gitignore"), b"target/\n").expect("gitignore");
        fs::write(source.join("source.rs"), b"fn main() {}").expect("source");
        fs::write(source.join("target/app"), b"first build").expect("ignored output");
        let archive = temp.path().join("source-snapshot.tar.gz");

        let prepared =
            prepare_source_workspace_snapshot(&source, &archive).expect("prepare source snapshot");
        let unchanged =
            source_workspace_snapshot_source_fingerprint(&source).expect("source fingerprint");
        assert_eq!(prepared.source_fingerprint, unchanged);

        fs::write(source.join("target/app"), b"a different ignored build")
            .expect("change ignored output");
        assert_eq!(
            unchanged,
            source_workspace_snapshot_source_fingerprint(&source)
                .expect("fingerprint after ignored change")
        );

        let exact_before =
            exact_workspace_snapshot_source_fingerprint(&source).expect("exact fingerprint");
        fs::write(
            source.join("target/app"),
            b"another ignored build with a different size",
        )
        .expect("change exact input");
        assert_ne!(
            exact_before,
            exact_workspace_snapshot_source_fingerprint(&source)
                .expect("exact fingerprint after ignored change")
        );

        fs::write(
            source.join("source.rs"),
            b"fn main() { println!(\"changed\"); }",
        )
        .expect("change source");
        assert_ne!(
            unchanged,
            source_workspace_snapshot_source_fingerprint(&source)
                .expect("fingerprint after source change")
        );
    }

    #[test]
    fn result_bundle_reports_adds_edits_and_deletes_without_git() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(source.join("nested")).expect("nested");
        fs::write(source.join("keep.txt"), b"unchanged").expect("keep");
        fs::write(source.join("edit.txt"), b"before").expect("edit");
        fs::write(source.join("gone.txt"), b"remove me").expect("gone");
        fs::write(source.join("nested/deep.txt"), b"deep").expect("deep");
        let archive = temp.path().join("snapshot.tar.gz");
        let metadata = create_exact_workspace_snapshot(&source, &archive).expect("snapshot");
        let target = temp.path().join("current");
        let baseline = extract_workspace_snapshot(&archive, &target, &metadata).expect("extract");

        // Stand in for what the agent did on the target.
        fs::write(target.join("edit.txt"), b"after").expect("modify");
        fs::remove_file(target.join("gone.txt")).expect("delete");
        fs::write(target.join("new.txt"), b"created").expect("add");
        // A rewrite with identical bytes must not count as a change.
        fs::write(target.join("keep.txt"), b"unchanged").expect("rewrite");

        let bundle = temp.path().join("result.tar.gz");
        let summary =
            create_workspace_result_bundle(&target, &baseline, &bundle).expect("result bundle");

        assert_eq!(summary.added, vec!["new.txt".to_string()]);
        assert_eq!(summary.modified, vec!["edit.txt".to_string()]);
        assert_eq!(summary.deleted, vec!["gone.txt".to_string()]);
        assert!(!summary.is_empty());
        assert_eq!(summary.archive_sha256.len(), 64);
        assert!(summary.archive_size > 0);

        // Only changed content travels back; untouched files are not resent.
        let listed = list_archive_paths(&bundle);
        assert!(listed.contains(&"new.txt".to_string()));
        assert!(listed.contains(&"edit.txt".to_string()));
        assert!(
            !listed.contains(&"keep.txt".to_string()),
            "unchanged files must not be included: {listed:?}"
        );
        assert!(!listed.contains(&"nested/deep.txt".to_string()));
    }

    #[test]
    fn an_untouched_workspace_produces_an_empty_result() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("a.txt"), b"a").expect("a");
        let archive = temp.path().join("snapshot.tar.gz");
        let metadata = create_exact_workspace_snapshot(&source, &archive).expect("snapshot");
        let target = temp.path().join("current");
        let baseline = extract_workspace_snapshot(&archive, &target, &metadata).expect("extract");

        let bundle = temp.path().join("result.tar.gz");
        let summary =
            create_workspace_result_bundle(&target, &baseline, &bundle).expect("result bundle");
        assert!(
            summary.is_empty(),
            "a job that changed nothing must not report changes: {summary:?}"
        );
    }

    /// Snapshot a workspace, mutate the "target" copy, and bundle the result.
    fn snapshot_and_diff(
        temp: &Path,
        seed: &[(&str, &[u8])],
        mutate: impl FnOnce(&Path),
    ) -> (
        WorkspaceResultSummary,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let source = temp.join("source");
        fs::create_dir_all(&source).expect("source");
        for (name, bytes) in seed {
            fs::write(source.join(name), bytes).expect("seed file");
        }
        let archive = temp.join("snapshot.tar.gz");
        let metadata = create_exact_workspace_snapshot(&source, &archive).expect("snapshot");
        let target = temp.join("current");
        let baseline = extract_workspace_snapshot(&archive, &target, &metadata).expect("extract");
        mutate(&target);
        let bundle = temp.join("result.tar.gz");
        let summary = create_workspace_result_bundle(&target, &baseline, &bundle).expect("bundle");
        // A second extraction stands in for the controller's own copy of S0.
        let local = temp.join("local");
        extract_workspace_snapshot(&archive, &local, &metadata).expect("extract local");
        (summary, bundle, local)
    }

    #[test]
    fn applying_a_result_writes_adds_and_edits_and_removes_deletions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let (summary, bundle, local) = snapshot_and_diff(
            temp.path(),
            &[
                ("keep.txt", b"same"),
                ("edit.txt", b"before"),
                ("gone.txt", b"bye"),
            ],
            |target| {
                fs::write(target.join("edit.txt"), b"after").expect("edit");
                fs::remove_file(target.join("gone.txt")).expect("delete");
                fs::write(target.join("new.txt"), b"created").expect("add");
            },
        );

        let outcome =
            apply_workspace_result_bundle(&bundle, &local, &summary, false).expect("apply");
        assert!(!outcome.aborted, "an untouched local tree has no conflicts");
        assert!(outcome.conflicts.is_empty());
        assert_eq!(fs::read(local.join("edit.txt")).expect("edit"), b"after");
        assert_eq!(fs::read(local.join("new.txt")).expect("new"), b"created");
        assert!(
            !local.join("gone.txt").exists(),
            "deletions must be applied"
        );
        assert_eq!(
            fs::read(local.join("keep.txt")).expect("keep"),
            b"same",
            "untouched files must be left alone"
        );
    }

    #[test]
    fn a_locally_edited_file_blocks_the_apply_instead_of_being_overwritten() {
        let temp = tempfile::tempdir().expect("tempdir");
        let (summary, bundle, local) =
            snapshot_and_diff(temp.path(), &[("shared.txt", b"before")], |target| {
                fs::write(target.join("shared.txt"), b"target edit").expect("edit");
            });
        // The user kept working locally while the job ran.
        fs::write(local.join("shared.txt"), b"my local work").expect("local edit");

        let outcome =
            apply_workspace_result_bundle(&bundle, &local, &summary, false).expect("apply");
        assert!(outcome.aborted, "a conflict must stop the apply");
        assert_eq!(
            outcome.conflicts,
            vec![WorkspaceResultConflict {
                path: "shared.txt".to_string(),
                reason: WorkspaceResultConflictReason::LocallyModified,
            }]
        );
        assert!(outcome.written.is_empty() && outcome.removed.is_empty());
        assert_eq!(
            fs::read(local.join("shared.txt")).expect("local"),
            b"my local work",
            "nothing may be written when the apply aborts"
        );

        // The user can still choose the target's version explicitly.
        let forced =
            apply_workspace_result_bundle(&bundle, &local, &summary, true).expect("forced apply");
        assert!(!forced.aborted);
        assert_eq!(
            fs::read(local.join("shared.txt")).expect("local"),
            b"target edit"
        );
    }

    #[test]
    fn a_tampered_bundle_is_rejected_before_anything_is_written() {
        let temp = tempfile::tempdir().expect("tempdir");
        let (summary, bundle, local) =
            snapshot_and_diff(temp.path(), &[("a.txt", b"before")], |target| {
                fs::write(target.join("a.txt"), b"after").expect("edit");
            });
        fs::write(&bundle, b"not the bundle you verified").expect("tamper");

        let error = apply_workspace_result_bundle(&bundle, &local, &summary, false)
            .expect_err("a tampered bundle must be refused");
        assert!(
            error
                .to_string()
                .contains("does not match the reported digest"),
            "{error}"
        );
        assert_eq!(fs::read(local.join("a.txt")).expect("local"), b"before");
    }

    #[test]
    fn result_paths_cannot_escape_the_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("ws");
        fs::create_dir_all(&workspace).expect("workspace");
        for hostile in ["../outside", "a/../../outside", "/etc/passwd", ""] {
            assert!(
                resolve_workspace_child(&workspace, hostile).is_err(),
                "must reject {hostile:?}"
            );
        }
        assert!(resolve_workspace_child(&workspace, "nested/ok.txt").is_ok());
    }

    fn list_archive_paths(archive_path: &Path) -> Vec<String> {
        let file = File::open(archive_path).expect("open bundle");
        let mut archive = Archive::new(GzDecoder::new(file));
        archive
            .entries()
            .expect("entries")
            .map(|entry| {
                let entry = entry.expect("entry");
                entry
                    .path()
                    .expect("path")
                    .strip_prefix(WORKSPACE_ARCHIVE_ROOT)
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .collect()
    }

    #[test]
    fn exact_snapshot_round_trips_paths_longer_than_a_legacy_tar_header() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let long_directory = "directory-segment-".repeat(8);
        let relative = Path::new(&long_directory).join("long-file-name.txt");
        fs::create_dir_all(source.join(&long_directory)).expect("long directory");
        fs::write(source.join(&relative), b"long path").expect("long path file");
        assert!(
            format!("{WORKSPACE_ARCHIVE_ROOT}/{}", relative.to_string_lossy()).len() > 100,
            "fixture must require a GNU long-name record"
        );
        let archive = temp.path().join("snapshot.tar.gz");

        let metadata = create_exact_workspace_snapshot(&source, &archive).expect("create snapshot");
        let destination = temp.path().join("destination");
        extract_workspace_snapshot(&archive, &destination, &metadata).expect("extract snapshot");

        assert_eq!(
            fs::read(destination.join(relative)).expect("long path output"),
            b"long path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_fail_instead_of_escaping_or_disappearing() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(temp.path().join("outside"), b"private").expect("outside");
        symlink(temp.path().join("outside"), source.join("link")).expect("link");
        let error = create_exact_workspace_snapshot(&source, &temp.path().join("snapshot.tar.gz"))
            .expect_err("link must fail");
        assert!(error.to_string().contains("symbolic link"));
    }

    #[test]
    fn tampered_archive_is_rejected_before_extraction() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("file"), b"original").expect("file");
        let archive = temp.path().join("snapshot.tar.gz");
        let metadata = create_exact_workspace_snapshot(&source, &archive).expect("create snapshot");
        let mut bytes = fs::read(&archive).expect("archive");
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&archive, bytes).expect("tamper");

        let error = extract_workspace_snapshot(&archive, &temp.path().join("out"), &metadata)
            .expect_err("tampering must fail");
        assert!(error.to_string().contains("SHA-256 mismatch"));
        assert!(!temp.path().join("out").exists());
    }

    #[test]
    fn manifest_rejects_nested_git_metadata() {
        let manifest = WorkspaceSnapshotManifest {
            format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
            mode: "exact".to_string(),
            includes_ignored_files: true,
            excludes_git_metadata: true,
            file_count: 1,
            directory_count: 0,
            uncompressed_bytes: 1,
            entries: vec![WorkspaceSnapshotEntry {
                path: "nested/.git/config".to_string(),
                kind: WorkspaceSnapshotEntryKind::File,
                size: 1,
                sha256: Some("0".repeat(64)),
                executable: false,
            }],
        };
        let metadata = WorkspaceSnapshotMetadata {
            format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
            archive_size: 1,
            archive_sha256: "0".repeat(64),
            manifest_sha256: "0".repeat(64),
            file_count: 1,
            directory_count: 0,
            uncompressed_bytes: 1,
        };
        assert!(validate_manifest(&manifest, &metadata).is_err());
    }

    #[test]
    fn manifest_summary_must_match_its_entries() {
        let manifest = WorkspaceSnapshotManifest {
            format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
            mode: "exact".to_string(),
            includes_ignored_files: true,
            excludes_git_metadata: true,
            file_count: 0,
            directory_count: 0,
            uncompressed_bytes: 0,
            entries: vec![WorkspaceSnapshotEntry {
                path: "file.txt".to_string(),
                kind: WorkspaceSnapshotEntryKind::File,
                size: 1,
                sha256: Some("0".repeat(64)),
                executable: false,
            }],
        };
        let metadata = WorkspaceSnapshotMetadata {
            format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
            archive_size: 1,
            archive_sha256: "0".repeat(64),
            manifest_sha256: "0".repeat(64),
            file_count: 0,
            directory_count: 0,
            uncompressed_bytes: 0,
        };
        assert!(validate_manifest(&manifest, &metadata).is_err());
    }
}
