use super::{
    ensure_memory_store_for_target, format_manifest_path, list_memory_files_recursive,
    memory_store_dir_path_for_target, MemoryScope, MemoryStoreTarget, MEMORY_INDEX_FILE,
    MEMORY_MANIFEST_MAX_FILES,
};
use crate::util::errors::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) async fn build_memory_manifest_for_target(
    target: MemoryStoreTarget<'_>,
) -> BitFunResult<Option<String>> {
    ensure_memory_store_for_target(target).await?;
    let memory_dir = memory_store_dir_path_for_target(target);
    let mut memory_files = vec![memory_dir.join(MEMORY_INDEX_FILE)];
    memory_files.extend(list_memory_files_recursive(&memory_dir).await?);

    Ok(render_memory_manifest(
        target.scope(),
        &memory_dir,
        memory_files,
    ))
}

fn render_memory_manifest(
    scope: MemoryScope,
    memory_dir: &Path,
    memory_files: Vec<PathBuf>,
) -> Option<String> {
    let mut seen = HashSet::new();
    let mut ordinary = Vec::new();
    let mut workspace_overviews = Vec::new();

    for path in memory_files {
        let relative_path = format_manifest_path(&path, memory_dir);
        if relative_path.is_empty() || !seen.insert(relative_path.clone()) {
            continue;
        }

        if scope == MemoryScope::GlobalAgenticOs
            && relative_path.starts_with("workspaces_overview/")
        {
            workspace_overviews.push(relative_path);
        } else {
            ordinary.push(relative_path);
        }
    }

    ordinary.sort();
    workspace_overviews.sort();

    if ordinary.first().map(String::as_str) != Some(MEMORY_INDEX_FILE) {
        ordinary.retain(|path| path != MEMORY_INDEX_FILE);
        ordinary.insert(0, MEMORY_INDEX_FILE.to_string());
    }

    let ordinary_limit = MEMORY_MANIFEST_MAX_FILES.min(ordinary.len());
    let ordinary = ordinary
        .into_iter()
        .take(ordinary_limit)
        .collect::<Vec<_>>();
    let remaining = MEMORY_MANIFEST_MAX_FILES.saturating_sub(ordinary.len());
    let workspace_overviews = workspace_overviews
        .into_iter()
        .take(remaining)
        .collect::<Vec<_>>();

    match scope {
        MemoryScope::WorkspaceProject => {
            if ordinary.is_empty() {
                None
            } else {
                Some(render_file_list(&ordinary))
            }
        }
        MemoryScope::GlobalAgenticOs => {
            let mut sections = Vec::new();
            if !ordinary.is_empty() {
                sections.push(format!(
                    "### Ordinary memories\n\n{}",
                    render_file_list(&ordinary)
                ));
            }
            if !workspace_overviews.is_empty() {
                sections.push(format!(
                    "### Workspace overview files\n\n{}",
                    render_file_list(&workspace_overviews)
                ));
            }

            if sections.is_empty() {
                None
            } else {
                Some(sections.join("\n\n"))
            }
        }
    }
}

fn render_file_list(paths: &[String]) -> String {
    paths
        .iter()
        .map(|path| format!("- {}", path))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::render_memory_manifest;
    use crate::service::memory_store::MemoryScope;
    use std::path::PathBuf;

    #[test]
    fn workspace_manifest_lists_memory_index_first() {
        let memory_dir = PathBuf::from("/memory");
        let manifest = render_memory_manifest(
            MemoryScope::WorkspaceProject,
            &memory_dir,
            vec![
                memory_dir.join("feedback/testing.md"),
                memory_dir.join("MEMORY.md"),
                memory_dir.join("user/profile.md"),
            ],
        )
        .expect("workspace manifest should exist");

        assert_eq!(
            manifest,
            "- MEMORY.md\n- feedback/testing.md\n- user/profile.md"
        );
    }

    #[test]
    fn global_manifest_groups_workspace_overview_files_separately() {
        let memory_dir = PathBuf::from("/memory");
        let manifest = render_memory_manifest(
            MemoryScope::GlobalAgenticOs,
            &memory_dir,
            vec![
                memory_dir.join("reference/tooling.md"),
                memory_dir.join("workspaces_overview/bitfun--1234abcd.md"),
                memory_dir.join("MEMORY.md"),
                memory_dir.join("feedback/style.md"),
            ],
        )
        .expect("global manifest should exist");

        assert_eq!(
            manifest,
            "### Ordinary memories\n\n- MEMORY.md\n- feedback/style.md\n- reference/tooling.md\n\n### Workspace overview files\n\n- workspaces_overview/bitfun--1234abcd.md"
        );
    }
}
