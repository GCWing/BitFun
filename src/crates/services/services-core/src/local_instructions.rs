//! Bounded local reads shared by ecosystem-specific instruction adapters.

use crate::bounded_fs::{read_bounded_text, BoundedTextRead};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const MAX_LOCAL_INSTRUCTION_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_LOCAL_INSTRUCTION_FILES: usize = 256;
pub const MAX_LOCAL_INSTRUCTION_TOTAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalInstructionFile {
    pub canonical_path: PathBuf,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct LocalInstructionFiles {
    files: Vec<LocalInstructionFile>,
    canonical_paths: HashSet<PathBuf>,
    total_bytes: usize,
}

impl LocalInstructionFiles {
    pub fn push(&mut self, file: LocalInstructionFile) -> bool {
        if self.files.len() >= MAX_LOCAL_INSTRUCTION_FILES
            || self.canonical_paths.contains(&file.canonical_path)
        {
            return false;
        }
        if self.total_bytes.saturating_add(file.content.len()) > MAX_LOCAL_INSTRUCTION_TOTAL_BYTES {
            self.total_bytes = MAX_LOCAL_INSTRUCTION_TOTAL_BYTES;
            return false;
        }
        self.canonical_paths.insert(file.canonical_path.clone());
        self.total_bytes += file.content.len();
        self.files.push(file);
        true
    }

    pub fn extend(&mut self, files: impl IntoIterator<Item = LocalInstructionFile>) {
        for file in files {
            self.push(file);
            if self.is_at_capacity() {
                break;
            }
        }
    }

    pub fn contains_path(&self, path: &Path) -> bool {
        self.canonical_paths.contains(path)
    }

    pub fn is_at_capacity(&self) -> bool {
        self.files.len() >= MAX_LOCAL_INSTRUCTION_FILES
            || self.total_bytes >= MAX_LOCAL_INSTRUCTION_TOTAL_BYTES
    }

    pub fn into_files(self) -> Vec<LocalInstructionFile> {
        self.files
    }
}

pub fn local_instruction_path_exists(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_file() || metadata.file_type().is_symlink()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "Failed to inspect local instruction file {}: {error}",
            path.display()
        )),
    }
}

pub fn read_local_instruction_file(
    path: &Path,
    allowed_root: &Path,
    name: impl Into<String>,
) -> Result<Option<LocalInstructionFile>, String> {
    Ok(read_local_text_file(path, allowed_root, name)?
        .filter(|file| !file.content.trim().is_empty()))
}

pub fn read_local_text_file(
    path: &Path,
    allowed_root: &Path,
    name: impl Into<String>,
) -> Result<Option<LocalInstructionFile>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            return Ok(None);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Failed to inspect local instruction file {}: {error}",
                path.display()
            ));
        }
    }

    let canonical_root = std::fs::canonicalize(allowed_root).map_err(|error| {
        format!(
            "Failed to resolve local instruction root {}: {error}",
            allowed_root.display()
        )
    })?;
    let canonical_path = std::fs::canonicalize(path).map_err(|error| {
        format!(
            "Failed to resolve local instruction file {}: {error}",
            path.display()
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "Rejected local instruction file outside its declared root: {}",
            path.display()
        ));
    }
    let metadata = std::fs::metadata(&canonical_path).map_err(|error| {
        format!(
            "Failed to inspect resolved local instruction file {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Ok(None);
    }
    let content = match read_bounded_text(&canonical_path, MAX_LOCAL_INSTRUCTION_FILE_BYTES)
        .map_err(|error| {
            format!(
                "Failed to read local instruction file {}: {error}",
                path.display()
            )
        })? {
        BoundedTextRead::Content(content) => content,
        BoundedTextRead::TooLarge => {
            return Err(format!(
                "Local instruction file exceeds the per-file byte limit: {}",
                path.display()
            ));
        }
        BoundedTextRead::InvalidUtf8 => {
            return Err(format!(
                "Local instruction file is not valid UTF-8: {}",
                path.display()
            ));
        }
    };
    Ok(Some(LocalInstructionFile {
        canonical_path,
        name: name.into(),
        content,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        read_local_instruction_file, read_local_text_file, MAX_LOCAL_INSTRUCTION_FILE_BYTES,
    };

    #[test]
    fn empty_text_files_remain_distinguishable_from_empty_instruction_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("empty.json");
        std::fs::write(&path, "").expect("empty file");

        assert!(read_local_text_file(&path, temp.path(), "empty.json")
            .expect("text read")
            .is_some());
        assert!(read_local_instruction_file(&path, temp.path(), "empty.md")
            .expect("instruction read")
            .is_none());
    }

    #[test]
    fn explicit_paths_outside_the_declared_root_are_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        let outside = temp.path().join("outside.md");
        std::fs::create_dir_all(&root).expect("root directory");
        std::fs::write(&outside, "outside\n").expect("outside file");

        let error = read_local_instruction_file(&outside, &root, "outside.md")
            .expect_err("outside path must fail closed");

        assert!(error.contains("outside its declared root"));
    }

    #[test]
    fn oversized_local_sources_fail_instead_of_looking_absent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("oversized.md");
        std::fs::write(&path, vec![b'x'; MAX_LOCAL_INSTRUCTION_FILE_BYTES + 1])
            .expect("oversized file");

        let error = read_local_instruction_file(&path, temp.path(), "oversized.md")
            .expect_err("oversized source must fail closed");

        assert!(error.contains("exceeds the per-file byte limit"));
    }
}
