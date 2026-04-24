mod global_overview;
mod manifest;
mod paths;
mod policy;
mod prompt_context;

use crate::util::errors::*;
use std::path::{Path, PathBuf};
use tokio::fs;

pub(crate) use global_overview::build_global_workspace_overviews_context;
pub(crate) use manifest::build_memory_manifest_for_target;
pub(crate) use paths::{ensure_memory_store_for_target, memory_store_dir_path_for_target};
pub(crate) use policy::{
    build_global_memory_policy_sections, build_workspace_memory_policy_sections,
    SharedMemoryPolicyProfile,
};
pub(crate) use prompt_context::{
    build_memory_files_context_for_target, build_memory_prompt_for_target,
};

pub(crate) const MEMORY_INDEX_FILE: &str = "MEMORY.md";
const MEMORY_DIR_NAME: &str = "memory";
const LEGACY_MEMORY_INDEX_FILE: &str = "memory.md";
const MEMORY_INDEX_TEMPLATE: &str = "";
const MEMORY_INDEX_MAX_LINES: usize = 200;
const MEMORY_MANIFEST_MAX_FILES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    WorkspaceProject,
    GlobalAgenticOs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemoryStoreTarget<'a> {
    WorkspaceProject(&'a Path),
    GlobalAgenticOs,
}

impl MemoryScope {
    pub(crate) fn as_label(self) -> &'static str {
        match self {
            Self::WorkspaceProject => "workspace",
            Self::GlobalAgenticOs => "global",
        }
    }
}

impl<'a> MemoryStoreTarget<'a> {
    pub(crate) fn scope(self) -> MemoryScope {
        match self {
            Self::WorkspaceProject(_) => MemoryScope::WorkspaceProject,
            Self::GlobalAgenticOs => MemoryScope::GlobalAgenticOs,
        }
    }
}

pub(super) fn format_path_for_prompt(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) async fn ensure_markdown_placeholder(path: &Path, content: &str) -> BitFunResult<bool> {
    if path.exists() {
        return Ok(false);
    }

    fs::write(path, content)
        .await
        .map_err(|e| BitFunError::service(format!("Failed to create {}: {}", path.display(), e)))?;

    Ok(true)
}

pub(super) async fn migrate_legacy_memory_index(memory_dir: &Path) -> BitFunResult<()> {
    let legacy_path = memory_dir.join(LEGACY_MEMORY_INDEX_FILE);
    let canonical_path = memory_dir.join(MEMORY_INDEX_FILE);

    if !legacy_path.exists() || canonical_path.exists() {
        return Ok(());
    }

    fs::rename(&legacy_path, &canonical_path)
        .await
        .map_err(|e| {
            BitFunError::service(format!(
                "Failed to migrate legacy memory index {} -> {}: {}",
                legacy_path.display(),
                canonical_path.display(),
                e
            ))
        })?;

    Ok(())
}

pub(super) async fn list_memory_files_recursive(memory_dir: &Path) -> BitFunResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending_dirs = vec![memory_dir.to_path_buf()];

    while let Some(dir) = pending_dirs.pop() {
        let mut entries = fs::read_dir(&dir).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to read memory directory {}: {}",
                dir.display(),
                e
            ))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to iterate memory directory {}: {}",
                dir.display(),
                e
            ))
        })? {
            let file_type = entry.file_type().await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect memory entry {}: {}",
                    entry.path().display(),
                    e
                ))
            })?;

            if file_type.is_dir() {
                pending_dirs.push(entry.path());
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.ends_with(".md") && !file_name.eq_ignore_ascii_case(MEMORY_INDEX_FILE) {
                files.push(entry.path());
            }
        }
    }

    files.sort();
    Ok(files)
}

pub(super) fn format_manifest_path(path: &Path, memory_dir: &Path) -> String {
    path.strip_prefix(memory_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
