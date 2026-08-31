use crate::service::snapshot::types::{SnapshotError, SnapshotResult};
use crate::service::workspace_runtime::{WorkspaceRuntimeContext, WorkspaceRuntimeTarget};
use bitfun_runtime_ports::{WorkspaceFileSystem, WorkspacePathKind};
use log::info;
use std::fs;
use std::path::{Path, PathBuf};

/// Git isolation manager
pub struct IsolationManager {
    runtime_context: WorkspaceRuntimeContext,
    workspace_dir: PathBuf,
}

impl IsolationManager {
    /// Creates a new isolation manager.
    pub fn new(workspace_dir: PathBuf, runtime_context: WorkspaceRuntimeContext) -> Self {
        Self {
            runtime_context,
            workspace_dir,
        }
    }

    /// Ensures complete isolation.
    pub async fn ensure_complete_isolation(&mut self) -> SnapshotResult<()> {
        info!("Ensuring complete Git isolation");

        self.verify_runtime_layout().await?;
        self.verify_no_git_operations().await?;
        self.set_directory_permissions().await?;
        self.create_isolation_status_file().await?;

        info!("Git isolation ensured");
        Ok(())
    }

    async fn verify_runtime_layout(&self) -> SnapshotResult<()> {
        for dir in self.runtime_context.required_directories() {
            if !dir.exists() {
                return Err(SnapshotError::ConfigError(format!(
                    "Workspace runtime directory is missing: {}",
                    dir.display()
                )));
            }
        }
        Ok(())
    }

    /// Verifies no Git operations are impacted.
    async fn verify_no_git_operations(&self) -> SnapshotResult<()> {
        // Remote workspace paths and the controller's metadata directories are
        // different filesystems, even when their textual paths happen to match.
        if matches!(
            self.runtime_context.target,
            WorkspaceRuntimeTarget::LocalWorkspace { .. }
        ) {
            let git_dir = self.workspace_dir.join(".git");
            if git_dir.exists() && self.runtime_context.runtime_root.starts_with(&git_dir) {
                return Err(SnapshotError::GitIsolationFailure(
                    "Snapshot runtime directory should not be inside .git directory".to_string(),
                ));
            }
        }

        self.verify_isolation_integrity().await?;

        Ok(())
    }

    /// Verifies isolation integrity.
    async fn verify_isolation_integrity(&self) -> SnapshotResult<()> {
        let forbidden_files = [".git", ".gitignore", ".gitmodules"];

        for entry in fs::read_dir(&self.runtime_context.runtime_root)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if forbidden_files
                .iter()
                .any(|&forbidden| file_name_str.starts_with(forbidden))
            {
                return Err(SnapshotError::GitIsolationFailure(format!(
                    "Found Git-related file in .bitfun directory: {}",
                    file_name_str
                )));
            }
        }

        Ok(())
    }

    /// Sets directory permissions.
    async fn set_directory_permissions(&self) -> SnapshotResult<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let permissions = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&self.runtime_context.runtime_root, permissions)?;
        }

        Ok(())
    }

    /// Creates the isolation status file.
    async fn create_isolation_status_file(&self) -> SnapshotResult<()> {
        let status_file = self.runtime_context.isolation_status_file.clone();
        let status = serde_json::json!({
            "git_isolated": true,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "version": "1.0"
        });

        fs::write(status_file, serde_json::to_string_pretty(&status)?)?;

        Ok(())
    }

    /// Checks isolation status.
    pub async fn check_isolation_status(&self) -> SnapshotResult<bool> {
        let status_file = self.runtime_context.isolation_status_file.clone();

        if !status_file.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(status_file)?;
        let status: serde_json::Value = serde_json::from_str(&content)?;

        Ok(status
            .get("git_isolated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Returns the snapshot runtime directory path.
    pub fn get_bitfun_dir(&self) -> &Path {
        &self.runtime_context.runtime_root
    }

    /// Returns the workspace directory path.
    pub fn get_workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    /// Validates that a file path is within the snapshot system scope.
    pub fn is_path_in_sandbox(&self, path: &Path) -> bool {
        path.starts_with(&self.runtime_context.runtime_root)
    }

    /// Validates that a file path is safe (does not impact Git).
    pub fn is_path_safe_for_modification(&self, path: &Path) -> bool {
        if matches!(
            self.runtime_context.target,
            WorkspaceRuntimeTarget::RemoteWorkspaceMirror { .. }
        ) {
            // Remote validation needs provider metadata; never canonicalize on
            // the controller when a caller uses the synchronous legacy entry.
            return false;
        }
        let git_dir = self.workspace_dir.join(".git");
        if path_starts_with_scope(path, &git_dir)
            || path_starts_with_scope(path, &self.runtime_context.runtime_root)
        {
            return false;
        }

        let Some(path) = canonicalize_for_scope(path) else {
            return false;
        };
        let Some(workspace_dir) = canonicalize_for_scope(&self.workspace_dir) else {
            return false;
        };
        let Some(git_dir) = canonicalize_for_scope(&git_dir) else {
            return false;
        };
        let Some(runtime_root) = canonicalize_for_scope(&self.runtime_context.runtime_root) else {
            return false;
        };

        path_starts_with_scope(&path, &workspace_dir)
            && !path_starts_with_scope(&path, &git_dir)
            && !path_starts_with_scope(&path, &runtime_root)
    }

    pub(crate) async fn validate_workspace_path(
        &self,
        path: &Path,
        workspace_fs: &dyn WorkspaceFileSystem,
    ) -> SnapshotResult<()> {
        let WorkspaceRuntimeTarget::RemoteWorkspaceMirror { remote_root, .. } =
            &self.runtime_context.target
        else {
            return if self.is_path_safe_for_modification(path) {
                Ok(())
            } else {
                Err(SnapshotError::GitIsolationFailure(format!(
                    "File path is outside the snapshot workspace scope: {}",
                    path.display()
                )))
            };
        };
        let path = path.to_str().ok_or_else(|| {
            SnapshotError::ConfigError("Remote snapshot paths must be UTF-8".into())
        })?;
        let root = remote_root.trim_end_matches('/');
        let relative = path
            .strip_prefix(root)
            .and_then(|suffix| suffix.strip_prefix('/'));
        let Some(relative) = relative.filter(|suffix| !suffix.is_empty()) else {
            return Err(SnapshotError::GitIsolationFailure(format!(
                "Remote snapshot path is outside the bound workspace: {path}"
            )));
        };
        if !path.starts_with('/')
            || path.contains('\0')
            || relative
                .split('/')
                .any(|part| matches!(part, "" | "." | ".." | ".git"))
        {
            return Err(SnapshotError::GitIsolationFailure(format!(
                "Unsafe remote snapshot path: {path}"
            )));
        }

        // Without a provider canonicalize/no-follow-open capability, following
        // a symlink could snapshot or overwrite files outside the bound root.
        // Reject it explicitly rather than consulting the controller's disk.
        let mut current = if root.is_empty() {
            "/".to_string()
        } else {
            root.to_string()
        };
        let mut components = relative.split('/').peekable();
        loop {
            match workspace_fs.metadata(&current, false).await? {
                Some(metadata)
                    if metadata.kind == WorkspacePathKind::Symlink
                        || metadata.kind == WorkspacePathKind::Other =>
                {
                    return Err(SnapshotError::GitIsolationFailure(format!("Remote snapshot paths cannot traverse symbolic links or special files: {current}")));
                }
                Some(metadata)
                    if components.peek().is_some()
                        && metadata.kind != WorkspacePathKind::Directory =>
                {
                    return Err(SnapshotError::GitIsolationFailure(format!(
                        "Remote snapshot ancestor is not a directory: {current}"
                    )));
                }
                None => break,
                _ => {}
            }
            let Some(component) = components.next() else {
                break;
            };
            current = workspace_fs.join_path(&current, &[component]);
        }
        Ok(())
    }

    /// Returns a path relative to the workspace directory.
    pub fn get_relative_path(&self, absolute_path: &Path) -> SnapshotResult<PathBuf> {
        absolute_path
            .strip_prefix(&self.workspace_dir)
            .map(|p| p.to_path_buf())
            .map_err(|_| {
                SnapshotError::ConfigError(format!(
                    "Path is not within workspace directory: {}",
                    absolute_path.display()
                ))
            })
    }
}

fn canonicalize_for_scope(path: &Path) -> Option<PathBuf> {
    let mut ancestor = path;
    let mut missing_suffix = Vec::new();

    loop {
        if let Ok(mut resolved) = dunce::canonicalize(ancestor) {
            for component in missing_suffix.iter().rev() {
                resolved.push(component);
            }
            return Some(resolved);
        }

        missing_suffix.push(ancestor.file_name()?.to_os_string());
        ancestor = ancestor.parent()?;
    }
}

#[cfg(not(windows))]
fn path_starts_with_scope(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(windows)]
fn path_starts_with_scope(path: &Path, root: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    fn lower_ascii(unit: u16) -> u16 {
        if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
            unit + u16::from(b'a' - b'A')
        } else {
            unit
        }
    }

    let mut path_components = path.components();
    root.components().all(|root_component| {
        path_components.next().is_some_and(|path_component| {
            path_component
                .as_os_str()
                .encode_wide()
                .map(lower_ascii)
                .eq(root_component.as_os_str().encode_wide().map(lower_ascii))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::IsolationManager;
    use crate::service::workspace_runtime::{WorkspaceRuntimeContext, WorkspaceRuntimeTarget};
    use std::path::{Path, PathBuf};

    fn manager(workspace_dir: PathBuf, runtime_root: PathBuf) -> IsolationManager {
        let runtime_context = WorkspaceRuntimeContext::new(
            WorkspaceRuntimeTarget::LocalWorkspace {
                workspace_root: workspace_dir.clone(),
            },
            runtime_root,
        );
        IsolationManager::new(workspace_dir, runtime_context)
    }

    fn aliased_workspace(root: &Path) -> PathBuf {
        let anchor = root.join("alias-anchor");
        std::fs::create_dir_all(&anchor).expect("alias anchor");
        anchor.join("..")
    }

    #[test]
    fn accepts_existing_file_through_workspace_alias() {
        let workspace = tempfile::tempdir().expect("workspace");
        let workspace_root = dunce::canonicalize(workspace.path()).expect("canonical workspace");
        let alias = aliased_workspace(workspace.path());
        let file = workspace.path().join("tracked.txt");
        std::fs::write(&file, "tracked").expect("tracked file");
        let manager = manager(workspace_root, workspace.path().join(".bitfun"));

        assert!(manager.is_path_safe_for_modification(&alias.join("tracked.txt")));
    }

    #[test]
    fn accepts_nested_new_file_through_workspace_alias() {
        let workspace = tempfile::tempdir().expect("workspace");
        let workspace_root = dunce::canonicalize(workspace.path()).expect("canonical workspace");
        let alias = aliased_workspace(workspace.path());
        let manager = manager(workspace_root, workspace.path().join(".bitfun"));

        assert!(manager.is_path_safe_for_modification(&alias.join("new/deep/file.txt")));
    }

    #[test]
    fn rejects_runtime_path_before_alias_resolution() {
        let workspace = tempfile::tempdir().expect("workspace");
        let workspace_root = dunce::canonicalize(workspace.path()).expect("canonical workspace");
        let alias = aliased_workspace(workspace.path());
        let runtime_root = alias.join(".bitfun");
        std::fs::create_dir_all(&runtime_root).expect("runtime root");
        let manager = manager(workspace_root, runtime_root.clone());

        assert!(!manager.is_path_safe_for_modification(&runtime_root.join("state.json")));
    }

    #[cfg(windows)]
    #[test]
    fn rejects_case_variant_missing_git_directory() {
        let workspace = tempfile::tempdir().expect("workspace");
        let workspace_root = dunce::canonicalize(workspace.path()).expect("canonical workspace");
        let manager = manager(workspace_root, workspace.path().join(".bitfun"));

        assert!(!manager.is_path_safe_for_modification(&workspace.path().join(".GIT/config")));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_git_symlink_target_inside_workspace() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let metadata = workspace.path().join("metadata");
        std::fs::create_dir_all(&metadata).expect("metadata target");
        std::fs::write(metadata.join("config"), "config").expect("git config");
        symlink(&metadata, workspace.path().join(".git")).expect("git symlink");
        let workspace_root = dunce::canonicalize(workspace.path()).expect("canonical workspace");
        let manager = manager(workspace_root, workspace.path().join(".bitfun"));

        assert!(!manager.is_path_safe_for_modification(&workspace.path().join(".git/config")));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_workspace_symlink_that_escapes_scope() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("outside.txt"), "outside").expect("outside file");
        symlink(outside.path(), workspace.path().join("escape")).expect("escape symlink");
        let workspace_root = dunce::canonicalize(workspace.path()).expect("canonical workspace");
        let manager = manager(workspace_root, workspace.path().join(".bitfun"));

        assert!(
            !manager.is_path_safe_for_modification(&workspace.path().join("escape/outside.txt"))
        );
    }
}
