use crate::{LegacyMigrationError, LegacyMigrationResult, MigrationRoots};
use fs2::FileExt;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MigrationLayout {
    root: PathBuf,
    run_id: String,
}

impl MigrationLayout {
    pub fn new(roots: &MigrationRoots, run_id: impl Into<String>) -> Self {
        Self {
            root: roots.migration_root(),
            run_id: run_id.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn run_root(&self) -> PathBuf {
        self.root.join("runs").join(&self.run_id)
    }

    pub fn request_path(&self) -> PathBuf {
        self.run_root().join("request.json")
    }

    pub fn consumed_nonce_path(&self) -> PathBuf {
        self.run_root().join("nonce-consumed.json")
    }

    pub fn plan_path(&self) -> PathBuf {
        self.run_root().join("plan.json")
    }

    pub fn journal_path(&self) -> PathBuf {
        self.run_root().join("journal.jsonl")
    }

    pub fn report_path(&self) -> PathBuf {
        self.run_root().join("report.json")
    }

    pub fn release_observation_path(&self) -> PathBuf {
        self.run_root().join("release-observation.json")
    }

    pub fn failure_diagnostics_path(&self) -> PathBuf {
        self.run_root().join("failure-diagnostics.json")
    }

    pub fn stage_root(&self) -> PathBuf {
        self.run_root().join("stage")
    }

    pub fn backup_root(&self) -> PathBuf {
        self.run_root().join("backup")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.root.join("lock")
    }

    pub fn initialize(&self) -> LegacyMigrationResult<()> {
        for path in [self.run_root(), self.stage_root(), self.backup_root()] {
            fs::create_dir_all(&path).map_err(|error| LegacyMigrationError::io(&path, error))?;
        }
        Ok(())
    }

    pub fn append_journal<T: Serialize>(&self, event: &T) -> LegacyMigrationResult<()> {
        let path = self.journal_path();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| LegacyMigrationError::io(&path, error))?;
        serde_json::to_writer(&mut file, event)
            .map_err(|error| LegacyMigrationError::json(&path, error))?;
        file.write_all(b"\n")
            .map_err(|error| LegacyMigrationError::io(&path, error))?;
        file.sync_data()
            .map_err(|error| LegacyMigrationError::io(&path, error))
    }

    pub fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &Path,
    ) -> LegacyMigrationResult<Option<T>> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(LegacyMigrationError::io(path, error)),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| LegacyMigrationError::json(path, error))
    }
}

pub struct MigrationLock {
    file: File,
}

impl MigrationLock {
    pub fn acquire(layout: &MigrationLayout) -> LegacyMigrationResult<Self> {
        fs::create_dir_all(layout.root())
            .map_err(|error| LegacyMigrationError::io(layout.root(), error))?;
        let path = layout.lock_path();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| LegacyMigrationError::io(&path, error))?;
        file.try_lock_exclusive()
            .map_err(|_| LegacyMigrationError::LockUnavailable)?;
        Ok(Self { file })
    }
}

impl Drop for MigrationLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> LegacyMigrationResult<()> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| LegacyMigrationError::json(path, error))?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

/// Atomically replace a file with caller-provided bytes on the target volume.
pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> LegacyMigrationResult<()> {
    atomic_write(path, bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> LegacyMigrationResult<()> {
    let parent = path.parent().ok_or_else(|| {
        LegacyMigrationError::InvalidRequest(format!("path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|error| LegacyMigrationError::io(parent, error))?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|error| LegacyMigrationError::io(&temp, error))?;
    file.write_all(bytes)
        .map_err(|error| LegacyMigrationError::io(&temp, error))?;
    file.sync_all()
        .map_err(|error| LegacyMigrationError::io(&temp, error))?;
    drop(file);
    if let Err(error) = replace_file(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> LegacyMigrationResult<()> {
    fs::rename(source, target).map_err(|error| LegacyMigrationError::io(target, error))
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> LegacyMigrationResult<()> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide =
        windows_extended_path(source).map_err(|error| LegacyMigrationError::io(source, error))?;
    let target_wide =
        windows_extended_path(target).map_err(|error| LegacyMigrationError::io(target, error))?;
    unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(target_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| LegacyMigrationError::io(target, std::io::Error::other(error.to_string())))
    }
}

#[cfg(windows)]
fn windows_extended_path(path: &Path) -> std::io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let absolute = std::path::absolute(path)?;
    let path = absolute.as_os_str().encode_wide().collect::<Vec<_>>();
    let slash = b'\\' as u16;
    let mut extended = if path.starts_with(&[slash, slash, b'?' as u16, slash])
        || path.starts_with(&[slash, slash, b'.' as u16, slash])
    {
        path
    } else if path.starts_with(&[slash, slash]) {
        r"\\?\UNC\"
            .encode_utf16()
            .chain(path.into_iter().skip(2))
            .collect()
    } else if path.len() >= 3 && path[1] == b':' as u16 && path[2] == slash {
        r"\\?\".encode_utf16().chain(path).collect()
    } else {
        path
    };
    extended.push(0);
    Ok(extended)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn atomic_write_supports_windows_paths_beyond_max_path() {
        let temp = tempfile::Builder::new()
            .prefix("openbitfun-legacy-migration-long-path-")
            .tempdir()
            .unwrap();
        let mut parent = temp.path().to_path_buf();
        while parent
            .join("migration-owned-session-state.json")
            .as_os_str()
            .len()
            < 270
        {
            parent.push("workspace-session-runtime-segment");
        }
        let target = parent.join("migration-owned-session-state.json");

        atomic_write_bytes(&target, b"first").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"first");

        atomic_write_bytes(&target, b"second").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second");
    }
}
