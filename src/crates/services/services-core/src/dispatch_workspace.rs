//! Safe, transport-neutral workspace snapshot packaging for detached dispatch.
//!
//! A snapshot is a one-shot input boundary. It deliberately does not contain
//! Git metadata and never follows links outside the selected workspace.

use std::collections::{BTreeMap, HashSet};
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
pub const MAX_SNAPSHOT_FILES: u64 = 100_000;
pub const MAX_SNAPSHOT_DIRECTORIES: u64 = 100_000;
pub const MAX_SNAPSHOT_FILE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SNAPSHOT_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_SNAPSHOT_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MANIFEST_ARCHIVE_PATH: &str = ".bitfun-dispatch/manifest.json";
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

/// Package every regular workspace file, including hidden and ignored files.
///
/// `.git` entries are the one explicit metadata exclusion. Unsupported entries
/// fail the whole operation instead of producing an incomplete snapshot.
pub fn create_exact_workspace_snapshot(
    source: &Path,
    archive_path: &Path,
) -> Result<WorkspaceSnapshotMetadata> {
    let result = create_exact_workspace_snapshot_inner(source, archive_path);
    if result.is_err() {
        let _ = fs::remove_file(archive_path);
    }
    result
}

fn create_exact_workspace_snapshot_inner(
    source: &Path,
    archive_path: &Path,
) -> Result<WorkspaceSnapshotMetadata> {
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

    let mut walk = WalkBuilder::new(&source);
    walk.hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .sort_by_file_path(|left, right| left.cmp(right));
    let filter_root = source.clone();
    walk.filter_entry(move |entry| {
        entry.path() == filter_root || entry.file_name().to_str() != Some(".git")
    });

    let mut entries = Vec::new();
    let mut file_count = 0_u64;
    let mut directory_count = 0_u64;
    let mut uncompressed_bytes = 0_u64;
    for walked in walk.build() {
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
        mode: "exact".to_string(),
        includes_ignored_files: true,
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
    Ok(WorkspaceSnapshotMetadata {
        format_version: WORKSPACE_SNAPSHOT_FORMAT_VERSION,
        archive_size,
        archive_sha256,
        manifest_sha256,
        file_count,
        directory_count,
        uncompressed_bytes,
    })
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
    if manifest.format_version != WORKSPACE_SNAPSHOT_FORMAT_VERSION
        || manifest.mode != "exact"
        || !manifest.includes_ignored_files
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

fn sha256_bytes(bytes: &[u8]) -> String {
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
