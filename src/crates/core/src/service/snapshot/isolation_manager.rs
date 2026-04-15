use crate::service::snapshot::types::{SnapshotError, SnapshotResult};
use crate::service::workspace_runtime::WorkspaceRuntimeContext;
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
        let git_dir = self.workspace_dir.join(".git");
        if git_dir.exists() && self.runtime_context.runtime_root.starts_with(&git_dir) {
            return Err(SnapshotError::GitIsolationFailure(
                "Snapshot runtime directory should not be inside .git directory".to_string(),
            ));
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
        if !path.starts_with(&self.workspace_dir) {
            return false;
        }

        let git_dir = self.workspace_dir.join(".git");
        if path.starts_with(&git_dir) {
            return false;
        }

        if path.starts_with(&self.runtime_context.runtime_root) {
            return false;
        }

        true
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
