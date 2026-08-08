//! Reusable bounded local filesystem reads and non-symlink traversal.

use std::io::Read;
use std::path::{Path, PathBuf};

pub fn is_symlink_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedFileRead {
    Content(Vec<u8>),
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedTextRead {
    Content(String),
    TooLarge,
    InvalidUtf8,
}

/// Reads at most `max_bytes + 1` bytes so concurrent growth cannot cause an
/// unbounded allocation.
pub fn read_bounded_file(path: &Path, max_bytes: usize) -> std::io::Result<BoundedFileRead> {
    let file = std::fs::File::open(path)?;
    let read_limit = max_bytes.saturating_add(1) as u64;
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024).saturating_add(1));
    file.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        Ok(BoundedFileRead::TooLarge)
    } else {
        Ok(BoundedFileRead::Content(bytes))
    }
}

pub fn read_bounded_text(path: &Path, max_bytes: usize) -> std::io::Result<BoundedTextRead> {
    match read_bounded_file(path, max_bytes)? {
        BoundedFileRead::Content(bytes) => Ok(match String::from_utf8(bytes) {
            Ok(content) => BoundedTextRead::Content(content),
            Err(_) => BoundedTextRead::InvalidUtf8,
        }),
        BoundedFileRead::TooLarge => Ok(BoundedTextRead::TooLarge),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedDirectoryWalkLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_directories: usize,
    pub max_files: usize,
}

impl BoundedDirectoryWalkLimits {
    pub fn for_file_limit(max_files: usize) -> Self {
        Self {
            max_depth: 32,
            max_entries: max_files.saturating_mul(4).max(1),
            max_directories: max_files.max(1),
            max_files,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedDirectoryWalkLimit {
    Depth,
    Entries,
    Directories,
    Files,
}

#[derive(Debug)]
pub enum BoundedDirectoryWalkError {
    Io(std::io::Error),
    LimitExceeded(BoundedDirectoryWalkLimit),
}

impl std::fmt::Display for BoundedDirectoryWalkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::LimitExceeded(limit) => write!(formatter, "{limit:?} limit exceeded"),
        }
    }
}

impl std::error::Error for BoundedDirectoryWalkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::LimitExceeded(_) => None,
        }
    }
}

/// Iteratively collects matching regular files without following symlinks.
/// Limits apply to traversal cost, not only to matching files.
pub fn collect_bounded_regular_files(
    root: &Path,
    limits: BoundedDirectoryWalkLimits,
    matches: impl FnMut(&Path) -> bool,
) -> Result<Vec<PathBuf>, BoundedDirectoryWalkError> {
    collect_bounded_regular_files_with_prune(root, limits, |_| true, matches)
}

/// Iteratively collects matching regular files while allowing callers to prune
/// known-unrelated directories before they consume traversal limits.
pub fn collect_bounded_regular_files_with_prune(
    root: &Path,
    limits: BoundedDirectoryWalkLimits,
    mut should_descend: impl FnMut(&Path) -> bool,
    mut matches: impl FnMut(&Path) -> bool,
) -> Result<Vec<PathBuf>, BoundedDirectoryWalkError> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(BoundedDirectoryWalkError::Io(error)),
    };
    if !metadata.is_dir() || is_symlink_or_reparse(&metadata) {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut visited_entries = 0usize;
    let mut visited_directories = 1usize;
    while let Some((directory, depth)) = stack.pop() {
        let entries = std::fs::read_dir(&directory).map_err(BoundedDirectoryWalkError::Io)?;
        for entry in entries {
            let entry = entry.map_err(BoundedDirectoryWalkError::Io)?;
            visited_entries = visited_entries.saturating_add(1);
            if visited_entries > limits.max_entries {
                return Err(BoundedDirectoryWalkError::LimitExceeded(
                    BoundedDirectoryWalkLimit::Entries,
                ));
            }
            let path = entry.path();
            let metadata =
                std::fs::symlink_metadata(&path).map_err(BoundedDirectoryWalkError::Io)?;
            if is_symlink_or_reparse(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if !should_descend(&path) {
                    continue;
                }
                let next_depth = depth.saturating_add(1);
                if next_depth > limits.max_depth {
                    return Err(BoundedDirectoryWalkError::LimitExceeded(
                        BoundedDirectoryWalkLimit::Depth,
                    ));
                }
                visited_directories = visited_directories.saturating_add(1);
                if visited_directories > limits.max_directories {
                    return Err(BoundedDirectoryWalkError::LimitExceeded(
                        BoundedDirectoryWalkLimit::Directories,
                    ));
                }
                stack.push((path, next_depth));
            } else if metadata.is_file() && matches(&path) {
                if files.len() >= limits.max_files {
                    return Err(BoundedDirectoryWalkError::LimitExceeded(
                        BoundedDirectoryWalkLimit::Files,
                    ));
                }
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}
