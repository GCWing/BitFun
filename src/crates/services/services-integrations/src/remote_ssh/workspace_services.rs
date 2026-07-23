//! SSH-backed workspace runtime services.
//!
//! This module adapts the remote SSH file and command services to the
//! workspace runtime ports. Product crates select when these providers are
//! used; this crate owns the concrete SSH-backed implementation.

use async_trait::async_trait;
use bitfun_runtime_ports::{
    WorkspaceCommandOptions, WorkspaceCommandResult, WorkspaceDirEntry, WorkspaceFileSystem,
    WorkspaceServices, WorkspaceShell,
};
use std::sync::Arc;

use super::{RemoteFileService, SSHCommandOptions, SSHCommandResult, SSHConnectionManager};
use crate::remote_ssh::shell;

/// SSH-backed filesystem implementation of [`WorkspaceFileSystem`].
pub struct RemoteWorkspaceFs {
    connection_id: String,
    file_service: RemoteFileService,
}

impl RemoteWorkspaceFs {
    pub fn new(connection_id: String, file_service: RemoteFileService) -> Self {
        Self {
            connection_id,
            file_service,
        }
    }
}

#[async_trait]
impl WorkspaceFileSystem for RemoteWorkspaceFs {
    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        self.file_service.read_file(&self.connection_id, path).await
    }

    async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
        let bytes = self.read_file(path).await?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()> {
        if let Some((parent, _)) = path
            .rsplit_once('/')
            .filter(|(parent, _)| !parent.is_empty())
        {
            self.file_service
                .create_dir_all(&self.connection_id, parent)
                .await?;
        }
        self.file_service
            .write_file(&self.connection_id, path, contents)
            .await
    }

    async fn exists(&self, path: &str) -> anyhow::Result<bool> {
        self.file_service.exists(&self.connection_id, path).await
    }

    async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
        self.file_service.is_file(&self.connection_id, path).await
    }

    async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
        self.file_service.is_dir(&self.connection_id, path).await
    }

    async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
        let entries = self
            .file_service
            .read_dir(&self.connection_id, path)
            .await?;
        Ok(entries
            .into_iter()
            .map(|entry| WorkspaceDirEntry {
                name: entry.name,
                path: entry.path,
                is_dir: entry.is_dir,
                is_symlink: entry.is_symlink,
            })
            .collect())
    }
}

/// SSH-backed shell implementation of [`WorkspaceShell`].
pub struct RemoteWorkspaceShell {
    ssh_manager: SSHConnectionManager,
    connection_id: String,
    workspace_root: String,
}

impl RemoteWorkspaceShell {
    pub fn new(
        connection_id: String,
        ssh_manager: SSHConnectionManager,
        workspace_root: String,
    ) -> Self {
        Self {
            connection_id,
            ssh_manager,
            workspace_root,
        }
    }
}

#[async_trait]
impl WorkspaceShell for RemoteWorkspaceShell {
    async fn exec_with_options(
        &self,
        command: &str,
        options: WorkspaceCommandOptions,
    ) -> anyhow::Result<WorkspaceCommandResult> {
        let wrapped = remote_workspace_command(&self.workspace_root, command);
        let result = self
            .ssh_manager
            .execute_command_with_options(
                &self.connection_id,
                &wrapped,
                SSHCommandOptions {
                    timeout_ms: options.timeout_ms,
                    cancellation_token: options.cancellation_token,
                },
            )
            .await?;

        Ok(workspace_result_from_ssh(result))
    }
}

/// Build [`WorkspaceServices`] backed by SSH for a remote workspace.
pub fn remote_workspace_services(
    connection_id: String,
    file_service: RemoteFileService,
    ssh_manager: SSHConnectionManager,
    workspace_root: String,
) -> WorkspaceServices {
    WorkspaceServices {
        fs: Arc::new(RemoteWorkspaceFs::new(connection_id.clone(), file_service)),
        shell: Arc::new(RemoteWorkspaceShell::new(
            connection_id,
            ssh_manager,
            workspace_root,
        )),
    }
}

fn remote_workspace_command(workspace_root: &str, command: &str) -> String {
    shell::cd_and(workspace_root, command)
}

fn workspace_result_from_ssh(result: SSHCommandResult) -> WorkspaceCommandResult {
    WorkspaceCommandResult {
        stdout: result.stdout,
        stderr: result.stderr,
        exit_code: result.exit_code,
        interrupted: result.interrupted,
        timed_out: result.timed_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_workspace_command_preserves_legacy_cd_wrapping_and_escaping() {
        assert_eq!(
            remote_workspace_command("/tmp/has space/it's", "pwd"),
            "cd '/tmp/has space/it'\\''s' && pwd"
        );
    }

    #[test]
    fn workspace_result_from_ssh_preserves_structured_status() {
        let result = workspace_result_from_ssh(SSHCommandResult {
            stdout: "out".to_string(),
            stderr: "err".to_string(),
            exit_code: 124,
            interrupted: false,
            timed_out: true,
        });

        assert_eq!(result.stdout, "out");
        assert_eq!(result.stderr, "err");
        assert_eq!(result.exit_code, 124);
        assert!(!result.interrupted);
        assert!(result.timed_out);
    }
}
