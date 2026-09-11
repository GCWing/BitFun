//! Ordinary directory IO for migration staging. No product/schema inspection.
use crate::{LegacyMigrationError, LegacyMigrationResult};
use std::fs;
use std::path::Path;

fn io_error(path: &Path, source: std::io::Error) -> LegacyMigrationError {
    LegacyMigrationError::Io {
        path: path.into(),
        source,
    }
}

/// Visit directories and regular files without content, depth, or count limits.
/// Links remain explicit errors: migration must not follow content outside its
/// declared source. Callers own conflict handling, cancellation and publication.
pub fn visit_directory(
    root: &Path,
    mut visit: impl FnMut(&Path, bool) -> LegacyMigrationResult<()>,
) -> LegacyMigrationResult<()> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).map_err(|e| io_error(&path, e))?;
        #[cfg(windows)]
        let reparse = {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes() & 0x0400 != 0
        };
        #[cfg(not(windows))]
        let reparse = false;
        if metadata.file_type().is_symlink() || reparse {
            return Err(LegacyMigrationError::LinkedPath(path));
        }
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(LegacyMigrationError::UnsupportedSource(format!(
                "unsupported filesystem entry: {}",
                path.display()
            )));
        }
        visit(&path, metadata.is_dir())?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(|e| io_error(&path, e))? {
                pending.push(entry.map_err(|e| io_error(&path, e))?.path());
            }
        }
    }
    Ok(())
}

/// Copy a source tree to an isolated staging directory using OS-backed file IO.
/// The migration layout must keep destination outside source; the caller owns
/// hash validation and atomic directory publication/recovery.
pub fn copy_directory(source: &Path, target: &Path) -> LegacyMigrationResult<()> {
    let source_root = fs::canonicalize(source).map_err(|e| io_error(source, e))?;
    if !source_root.is_dir() {
        return Err(LegacyMigrationError::UnsupportedSource(format!(
            "expected directory: {}",
            source.display()
        )));
    }
    // Resolve the existing destination ancestor before creating anything, so an
    // accidental destination inside source cannot recursively copy its own output.
    let mut ancestor = target;
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        let name = ancestor
            .file_name()
            .ok_or_else(|| LegacyMigrationError::PathEscape(target.into()))?;
        suffix.push(name);
        ancestor = ancestor
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
    }
    let mut destination_root = fs::canonicalize(ancestor).map_err(|e| io_error(ancestor, e))?;
    for name in suffix.into_iter().rev() {
        destination_root.push(name);
    }
    if destination_root.starts_with(&source_root) || source_root.starts_with(&destination_root) {
        return Err(LegacyMigrationError::PathEscape(target.into()));
    }
    visit_directory(source, |path, directory| {
        let relative = path
            .strip_prefix(source)
            .map_err(|_| LegacyMigrationError::PathEscape(path.into()))?;
        let destination = target.join(relative);
        if directory {
            fs::create_dir_all(&destination).map_err(|e| io_error(&destination, e))?;
        } else {
            fs::copy(path, &destination).map_err(|e| io_error(&destination, e))?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_large_deep_trees_and_empty_directories_without_size_gates() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        // Exceed the former workspace entry ceiling; these are ordinary files.
        for index in 0..32_769 {
            fs::write(source.join(format!("file-{index}")), []).unwrap();
        }
        let deep = (0..24).fold(source.clone(), |path, _| path.join("d"));
        fs::create_dir_all(deep.join("empty")).unwrap();
        fs::write(deep.join("content"), b"copied unchanged").unwrap();
        let target = temp.path().join("target");
        copy_directory(&source, &target).unwrap();
        assert!(target.join("file-32768").is_file());
        let relative = deep.strip_prefix(&source).unwrap();
        assert!(target.join(relative).join("empty").is_dir());
        assert_eq!(
            fs::read(target.join(relative).join("content")).unwrap(),
            b"copied unchanged"
        );
        assert_eq!(fs::read(deep.join("content")).unwrap(), b"copied unchanged");
    }

    #[test]
    fn refuses_overlapping_copy_before_creating_output() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("nested/output");
        assert!(matches!(
            copy_directory(temp.path(), &target),
            Err(LegacyMigrationError::PathEscape(_))
        ));
        assert!(!target.exists());
    }
}
