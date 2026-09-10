use openbitfun_legacy_migration::{
    atomic_write_bytes, LegacyMigrationError, LegacyMigrationResult,
};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn read_bounded_json<T: DeserializeOwned>(
    root: &Path,
    path: &Path,
) -> LegacyMigrationResult<T> {
    validate_regular_file(root, path)?;
    let metadata = fs::metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.len() > MAX_JSON_BYTES {
        return Err(LegacyMigrationError::ResourceLimit(format!(
            "JSON file exceeds {} bytes: {}",
            MAX_JSON_BYTES,
            relative_display(root, path)
        )));
    }
    let bytes = fs::read(path).map_err(|error| io_error(path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        LegacyMigrationError::InvalidRequest(format!(
            "invalid JSON at {}: {error}",
            relative_display(root, path)
        ))
    })
}

pub(crate) fn read_optional_bounded_json<T: DeserializeOwned>(
    root: &Path,
    path: &Path,
) -> LegacyMigrationResult<Option<T>> {
    match fs::symlink_metadata(path) {
        Ok(_) => read_bounded_json(root, path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(path, error)),
    }
}

pub(crate) fn validate_regular_file(root: &Path, path: &Path) -> LegacyMigrationResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(LegacyMigrationError::LinkedPath(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(LegacyMigrationError::InvalidRequest(format!(
            "expected a regular file at {}",
            relative_display(root, path)
        )));
    }
    let canonical_root = fs::canonicalize(root).map_err(|error| io_error(root, error))?;
    let canonical_path = fs::canonicalize(path).map_err(|error| io_error(path, error))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(LegacyMigrationError::PathEscape(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

pub(crate) fn backup_file_once(target: &Path, backup: &Path) -> LegacyMigrationResult<()> {
    if backup.exists() || !target.exists() {
        return Ok(());
    }
    let bytes = fs::read(target).map_err(|error| io_error(target, error))?;
    atomic_write_bytes(backup, &bytes)
}

pub(crate) fn restore_unverified_file(
    target: &Path,
    backup: &Path,
    target_existed: bool,
) -> LegacyMigrationResult<()> {
    if backup.exists() {
        let bytes = fs::read(backup).map_err(|error| io_error(backup, error))?;
        return atomic_write_bytes(target, &bytes);
    }
    if !target_existed {
        match fs::remove_file(target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(target, error)),
        }
    }
    Ok(())
}

pub(crate) fn stage_domain_dir(
    context: &openbitfun_legacy_migration::DomainContext<'_>,
    name: &str,
) -> PathBuf {
    context.layout.stage_root().join(name)
}

pub(crate) fn backup_domain_dir(
    context: &openbitfun_legacy_migration::DomainContext<'_>,
    name: &str,
) -> PathBuf {
    context.layout.backup_root().join(name)
}

pub(crate) fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn io_error(path: &Path, error: std::io::Error) -> LegacyMigrationError {
    LegacyMigrationError::InvalidRequest(format!("I/O failed at {}: {error}", path.display()))
}
