use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use git2::{
    Delta, DiffFindOptions, DiffFlags, DiffOptions, Patch, Repository, Status, StatusOptions,
};
use openbitfun_runtime_ports::{
    GitPort, PortError, PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
    WorkspaceDiffContent, WorkspaceDiffFile, WorkspaceDiffFileStatus, WorkspaceDiffSnapshot,
};

use super::utils::discover_repository;
use super::GitError;

const MAX_WORKSPACE_DIFF_FILES: usize = 256;
const MAX_WORKSPACE_DIFF_FILE_BYTES: usize = 1024 * 1024;
const MAX_WORKSPACE_DIFF_TOTAL_BYTES: usize = 3 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct GitWorkspaceDiffPort {
    workspace_root: PathBuf,
}

impl GitWorkspaceDiffPort {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }
}

impl RuntimeServicePort for GitWorkspaceDiffPort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Git
    }
}

#[async_trait::async_trait]
impl GitPort for GitWorkspaceDiffPort {
    async fn workspace_diff(&self) -> PortResult<WorkspaceDiffSnapshot> {
        let workspace_root = self.workspace_root.clone();
        tokio::task::spawn_blocking(move || collect_workspace_diff(&workspace_root))
            .await
            .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?
            .map_err(map_git_error)
    }
}

fn collect_workspace_diff(workspace_root: &Path) -> Result<WorkspaceDiffSnapshot, GitError> {
    let repository = discover_repository(workspace_root)?;
    let repository_root = repository
        .workdir()
        .ok_or_else(|| GitError::InvalidPath("Repository has no working directory".to_string()))?
        .canonicalize()?;
    let canonical_workspace = workspace_root.canonicalize()?;
    let workspace_prefix = canonical_workspace
        .strip_prefix(&repository_root)
        .map_err(|_| {
            GitError::InvalidPath("Workspace is outside the discovered repository".to_string())
        })?;

    let head_tree = match repository.head() {
        Ok(head) => Some(head.peel_to_tree()?),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => None,
        Err(error) => return Err(error.into()),
    };
    let mut options = DiffOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true)
        .include_typechange(true)
        .max_size(MAX_WORKSPACE_DIFF_FILE_BYTES as i64);
    if !workspace_prefix.as_os_str().is_empty() {
        options
            .disable_pathspec_match(true)
            .pathspec(normalize_path(workspace_prefix));
    }
    let mut statuses = collect_workspace_statuses(&repository, workspace_prefix)?;
    let mut diff =
        repository.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut options))?;
    diff.find_similar(Some(DiffFindOptions::new().renames(true)))?;

    let mut files = Vec::new();
    let mut total_patch_bytes = 0usize;
    let mut patch_budget_exhausted = false;
    let mut truncated = diff.deltas().len() > MAX_WORKSPACE_DIFF_FILES;

    for (index, delta) in diff.deltas().take(MAX_WORKSPACE_DIFF_FILES).enumerate() {
        let new_repository_path = delta.new_file().path();
        let old_repository_path = delta.old_file().path();
        let new_path = scoped_path(new_repository_path, workspace_prefix);
        let old_path = scoped_path(old_repository_path, workspace_prefix);
        let (display_repository_path, display_path) = new_repository_path
            .zip(new_path)
            .or_else(|| old_repository_path.zip(old_path))
            .ok_or_else(|| {
                GitError::ParseError("Git diff entry escaped the bound workspace".to_string())
            })?;
        let path = normalize_path(display_path);
        let mut status = statuses.remove(&path).unwrap_or(Status::CURRENT);
        if let Some(old_path) = old_path {
            status |= statuses
                .remove(&normalize_path(old_path))
                .unwrap_or(Status::CURRENT);
        }
        if status == Status::CURRENT {
            status = repository
                .status_file(display_repository_path)
                .unwrap_or(Status::CURRENT);
        }
        let file_status = workspace_diff_status(
            delta.status(),
            status,
            old_path.is_some(),
            new_path.is_some(),
        );
        let (additions, deletions, content) = if file_status == WorkspaceDiffFileStatus::Conflicted
        {
            (
                0,
                0,
                WorkspaceDiffContent::Text {
                    patch: String::new(),
                },
            )
        } else if patch_budget_exhausted {
            (0, 0, WorkspaceDiffContent::TooLarge)
        } else if delta.old_file().size() > MAX_WORKSPACE_DIFF_FILE_BYTES as u64
            || delta.new_file().size() > MAX_WORKSPACE_DIFF_FILE_BYTES as u64
        {
            truncated = true;
            (0, 0, WorkspaceDiffContent::TooLarge)
        } else {
            workspace_diff_content(&diff, index, delta.flags())?
        };

        let content = match content {
            WorkspaceDiffContent::Text { patch } if patch.len() > MAX_WORKSPACE_DIFF_FILE_BYTES => {
                truncated = true;
                WorkspaceDiffContent::TooLarge
            }
            WorkspaceDiffContent::Text { patch }
                if total_patch_bytes.saturating_add(patch.len())
                    > MAX_WORKSPACE_DIFF_TOTAL_BYTES =>
            {
                truncated = true;
                patch_budget_exhausted = true;
                WorkspaceDiffContent::TooLarge
            }
            WorkspaceDiffContent::Text { patch } => {
                total_patch_bytes = total_patch_bytes.saturating_add(patch.len());
                WorkspaceDiffContent::Text { patch }
            }
            other => other,
        };

        files.push(WorkspaceDiffFile {
            path,
            old_path: (delta.status() == Delta::Renamed)
                .then_some(old_path)
                .flatten()
                .map(normalize_path),
            status: file_status,
            staged: status.intersects(index_statuses()),
            unstaged: status.intersects(unstaged_statuses()),
            untracked: status.contains(Status::WT_NEW) || delta.status() == Delta::Untracked,
            additions,
            deletions,
            content,
        });
    }

    for (path, status) in statuses {
        if status == Status::CURRENT {
            continue;
        }
        if files.len() >= MAX_WORKSPACE_DIFF_FILES {
            truncated = true;
            break;
        }
        files.push(WorkspaceDiffFile {
            path,
            old_path: None,
            status: workspace_status_from_flags(status),
            staged: status.intersects(index_statuses()),
            unstaged: status.intersects(unstaged_statuses()),
            untracked: status.contains(Status::WT_NEW),
            additions: 0,
            deletions: 0,
            content: WorkspaceDiffContent::Text {
                patch: String::new(),
            },
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(WorkspaceDiffSnapshot { files, truncated })
}

fn collect_workspace_statuses(
    repository: &Repository,
    workspace_prefix: &Path,
) -> Result<BTreeMap<String, Status>, GitError> {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    if !workspace_prefix.as_os_str().is_empty() {
        options
            .disable_pathspec_match(true)
            .pathspec(normalize_path(workspace_prefix));
    }

    let statuses = repository.statuses(Some(&mut options))?;
    let mut by_path = BTreeMap::new();
    for entry in statuses.iter() {
        let repository_path =
            PathBuf::from(String::from_utf8_lossy(entry.path_bytes()).into_owned());
        let Some(path) = scoped_path(Some(&repository_path), workspace_prefix) else {
            continue;
        };
        by_path
            .entry(normalize_path(path))
            .and_modify(|status| *status |= entry.status())
            .or_insert(entry.status());
    }
    Ok(by_path)
}

fn scoped_path<'a>(path: Option<&'a Path>, workspace_prefix: &Path) -> Option<&'a Path> {
    path?.strip_prefix(workspace_prefix).ok()
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn workspace_diff_content(
    diff: &git2::Diff<'_>,
    index: usize,
    flags: DiffFlags,
) -> Result<(usize, usize, WorkspaceDiffContent), GitError> {
    if flags.contains(DiffFlags::BINARY) {
        return Ok((0, 0, WorkspaceDiffContent::Binary));
    }
    let Some(mut patch) = Patch::from_diff(diff, index)? else {
        return Ok((0, 0, WorkspaceDiffContent::Binary));
    };
    let patch_delta = patch.delta();
    if patch_delta.old_file().is_binary() || patch_delta.new_file().is_binary() {
        return Ok((0, 0, WorkspaceDiffContent::Binary));
    }
    let (_, additions, deletions) = patch.line_stats()?;
    let buffer = patch.to_buf()?;
    let Ok(text) = buffer.as_str() else {
        return Ok((additions, deletions, WorkspaceDiffContent::Binary));
    };
    Ok((
        additions,
        deletions,
        WorkspaceDiffContent::Text {
            patch: text.to_string(),
        },
    ))
}

fn workspace_diff_status(
    delta: Delta,
    status: Status,
    old_in_workspace: bool,
    new_in_workspace: bool,
) -> WorkspaceDiffFileStatus {
    if status.contains(Status::CONFLICTED) || delta == Delta::Conflicted {
        return WorkspaceDiffFileStatus::Conflicted;
    }
    match delta {
        Delta::Renamed if !old_in_workspace && new_in_workspace => WorkspaceDiffFileStatus::Added,
        Delta::Renamed if old_in_workspace && !new_in_workspace => WorkspaceDiffFileStatus::Deleted,
        Delta::Added | Delta::Untracked => WorkspaceDiffFileStatus::Added,
        Delta::Deleted => WorkspaceDiffFileStatus::Deleted,
        Delta::Renamed => WorkspaceDiffFileStatus::Renamed,
        _ => WorkspaceDiffFileStatus::Modified,
    }
}

fn workspace_status_from_flags(status: Status) -> WorkspaceDiffFileStatus {
    if status.contains(Status::CONFLICTED) {
        WorkspaceDiffFileStatus::Conflicted
    } else if status.intersects(Status::INDEX_NEW | Status::WT_NEW) {
        WorkspaceDiffFileStatus::Added
    } else if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
        WorkspaceDiffFileStatus::Deleted
    } else if status.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
        WorkspaceDiffFileStatus::Renamed
    } else {
        WorkspaceDiffFileStatus::Modified
    }
}

fn index_statuses() -> Status {
    Status::INDEX_NEW
        | Status::INDEX_MODIFIED
        | Status::INDEX_DELETED
        | Status::INDEX_RENAMED
        | Status::INDEX_TYPECHANGE
        | Status::CONFLICTED
}

fn unstaged_statuses() -> Status {
    Status::WT_MODIFIED
        | Status::WT_DELETED
        | Status::WT_RENAMED
        | Status::WT_TYPECHANGE
        | Status::CONFLICTED
}

fn map_git_error(error: GitError) -> PortError {
    let kind = match &error {
        GitError::RepositoryNotFound(_) => PortErrorKind::NotFound,
        // The repository is reachable; Git refuses it until the user grants
        // ownership trust, so this is a permission decision, not a lookup miss.
        GitError::RepositoryUntrusted { .. } => PortErrorKind::PermissionDenied,
        GitError::InvalidPath(_) => PortErrorKind::InvalidRequest,
        GitError::CommandFailed(message) if message.contains("timed out") => PortErrorKind::Timeout,
        _ => PortErrorKind::Backend,
    };
    PortError::new(kind, error.to_string())
}
