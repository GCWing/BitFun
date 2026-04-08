use crate::service::snapshot::types::{SnapshotError, SnapshotResult};
use log::{debug, info};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Git isolation manager
pub struct IsolationManager {
    bitfun_dir: PathBuf,
    workspace_dir: PathBuf,
    gitignore_managed: bool,
}

impl IsolationManager {
    /// Creates a new isolation manager.
    pub fn new(workspace_dir: PathBuf) -> Self {
        let bitfun_dir = workspace_dir.join(".bitfun");

        Self {
            bitfun_dir,
            workspace_dir,
            gitignore_managed: false,
        }
    }

    /// Ensures complete isolation.
    pub async fn ensure_complete_isolation(&mut self) -> SnapshotResult<()> {
        info!("Ensuring complete Git isolation");

        self.create_bitfun_directory_structure().await?;
        self.ensure_gitignore_entry().await?;
        self.verify_no_git_operations().await?;
        self.set_directory_permissions().await?;
        self.create_isolation_status_file().await?;

        info!("Git isolation ensured");
        Ok(())
    }

    /// Creates the `.bitfun` directory structure.
    async fn create_bitfun_directory_structure(&self) -> SnapshotResult<()> {
        let directories = [
            &self.bitfun_dir,
            &self.bitfun_dir.join("snapshots"),
            &self.bitfun_dir.join("snapshots/by_hash"),
            &self.bitfun_dir.join("snapshots/metadata"),
            &self.bitfun_dir.join("sessions"),
            &self.bitfun_dir.join("diffs"),
            &self.bitfun_dir.join("diffs/small"),
            &self.bitfun_dir.join("diffs/large"),
            &self.bitfun_dir.join("checkpoints"),
            &self.bitfun_dir.join("temp"),
            &self.bitfun_dir.join("config"),
        ];

        for dir in &directories {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
                debug!("Created directory: path={}", dir.display());
            }
        }

        Ok(())
    }

    /// Automatically manages `.gitignore`.
    async fn ensure_gitignore_entry(&mut self) -> SnapshotResult<()> {
        let gitignore_path = self.workspace_dir.join(".gitignore");
        let bitfun_entry = ".bitfun/";
        let comment = "# BitFun snapshot data - auto managed";

        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path)?;

            if !content.contains(bitfun_entry) {
                debug!("Adding .bitfun entry to existing .gitignore");
                let mut file = OpenOptions::new().append(true).open(&gitignore_path)?;

                writeln!(file, "\n{}", comment)?;
                writeln!(file, "{}", bitfun_entry)?;

                self.gitignore_managed = true;
            } else {
                debug!(".bitfun entry already exists in .gitignore");
                self.gitignore_managed = true;
            }
        } else {
            debug!("Creating new .gitignore file");
            let content = format!("{}\n{}\n", comment, bitfun_entry);
            fs::write(&gitignore_path, content)?;
            self.gitignore_managed = true;
        }

        Ok(())
    }

    /// Verifies no Git operations are impacted.
    async fn verify_no_git_operations(&self) -> SnapshotResult<()> {
        let git_dir = self.workspace_dir.join(".git");
        if git_dir.exists() && self.bitfun_dir.starts_with(&git_dir) {
            return Err(SnapshotError::GitIsolationFailure(
                ".bitfun directory should not be inside .git directory".to_string(),
            ));
        }

        self.verify_isolation_integrity().await?;

        Ok(())
    }

    /// Verifies isolation integrity.
    async fn verify_isolation_integrity(&self) -> SnapshotResult<()> {
        let forbidden_files = [".git", ".gitignore", ".gitmodules"];

        for entry in fs::read_dir(&self.bitfun_dir)? {
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
            fs::set_permissions(&self.bitfun_dir, permissions)?;
        }

        Ok(())
    }

    /// Creates the isolation status file.
    async fn create_isolation_status_file(&self) -> SnapshotResult<()> {
        let status_file = self.bitfun_dir.join("config/isolation_status.json");
        let status = serde_json::json!({
            "git_isolated": true,
            "gitignore_managed": self.gitignore_managed,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "version": "1.0"
        });

        fs::write(status_file, serde_json::to_string_pretty(&status)?)?;

        Ok(())
    }

    /// Checks isolation status.
    pub async fn check_isolation_status(&self) -> SnapshotResult<bool> {
        let status_file = self.bitfun_dir.join("config/isolation_status.json");

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

    /// Returns the `.bitfun` directory path.
    pub fn get_bitfun_dir(&self) -> &Path {
        &self.bitfun_dir
    }

    /// Returns the workspace directory path.
    pub fn get_workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    /// Cleans snapshot data (while preserving Git isolation).
    pub async fn cleanup_snapshot_data(&self, keep_recent_days: u64) -> SnapshotResult<()> {
        info!(
            "Cleaning up snapshot data: keep_recent_days={}",
            keep_recent_days
        );

        let cutoff_time = std::time::SystemTime::now()
            - std::time::Duration::from_secs(keep_recent_days * 24 * 3600);

        let sessions_dir = self.bitfun_dir.join("sessions");
        self.cleanup_directory_by_time(&sessions_dir, cutoff_time)
            .await?;

        let checkpoints_dir = self.bitfun_dir.join("checkpoints");
        self.cleanup_directory_by_time(&checkpoints_dir, cutoff_time)
            .await?;

        let temp_dir = self.bitfun_dir.join("temp");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
            fs::create_dir(&temp_dir)?;
        }

        Ok(())
    }

    /// Cleans directories by time.
    async fn cleanup_directory_by_time(
        &self,
        dir: &Path,
        cutoff_time: std::time::SystemTime,
    ) -> SnapshotResult<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if let Ok(modified_time) = metadata.modified() {
                if modified_time < cutoff_time {
                    let path = entry.path();
                    if path.is_file() {
                        fs::remove_file(&path)?;
                        debug!("Removed expired file: path={}", path.display());
                    } else if path.is_dir() {
                        fs::remove_dir_all(&path)?;
                        debug!("Removed expired directory: path={}", path.display());
                    }
                }
            }
        }

        Ok(())
    }

    /// Validates that a file path is within the snapshot system scope.
    pub fn is_path_in_sandbox(&self, path: &Path) -> bool {
        path.starts_with(&self.bitfun_dir)
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

        if path.starts_with(&self.bitfun_dir) {
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
