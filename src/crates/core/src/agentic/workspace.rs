use crate::service::remote_ssh::workspace_state::WorkspaceSessionIdentity;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Describes whether the workspace is local or remote via SSH.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkspaceBackend {
    Local,
    Remote {
        connection_id: String,
        connection_name: String,
    },
}

/// Session-bound workspace information used during agent execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceBinding {
    pub workspace_id: Option<String>,
    /// For local workspaces this is a local path; for remote workspaces it is
    /// the path on the remote server (e.g. `/root/project`).
    pub root_path: PathBuf,
    pub backend: WorkspaceBackend,
    /// Unified identity for session persistence. Local and remote workspaces
    /// share the same model; the only semantic difference is hostname.
    pub session_identity: WorkspaceSessionIdentity,
}

impl WorkspaceBinding {
    pub fn new(workspace_id: Option<String>, root_path: PathBuf) -> Self {
        let workspace_path = root_path.to_string_lossy().to_string();
        let session_identity =
            crate::service::remote_ssh::workspace_state::workspace_session_identity(
                &workspace_path,
                None,
                None,
            )
            .unwrap_or(WorkspaceSessionIdentity {
                hostname: crate::service::remote_ssh::workspace_state::LOCAL_WORKSPACE_SSH_HOST
                    .to_string(),
                workspace_path,
                remote_connection_id: None,
            });
        Self {
            workspace_id,
            root_path,
            backend: WorkspaceBackend::Local,
            session_identity,
        }
    }

    pub fn new_remote(
        workspace_id: Option<String>,
        root_path: PathBuf,
        connection_id: String,
        connection_name: String,
        session_identity: WorkspaceSessionIdentity,
    ) -> Self {
        Self {
            workspace_id,
            root_path,
            backend: WorkspaceBackend::Remote {
                connection_id,
                connection_name,
            },
            session_identity,
        }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn root_path_string(&self) -> String {
        self.root_path.to_string_lossy().to_string()
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.backend, WorkspaceBackend::Remote { .. })
    }

    pub fn connection_id(&self) -> Option<&str> {
        match &self.backend {
            WorkspaceBackend::Remote { connection_id, .. } => Some(connection_id),
            WorkspaceBackend::Local => None,
        }
    }

    /// The path to use for session persistence.
    pub fn session_storage_path(&self) -> &Path {
        Path::new(&self.session_identity.workspace_path)
    }
}

// ============================================================
// Workspace-level I/O abstractions — tools program against these
// traits instead of checking is_remote themselves.
// ============================================================

/// One row from [`WorkspaceFileSystem::read_dir`] (POSIX paths when backend is remote SSH).
#[derive(Debug, Clone)]
pub struct WorkspaceDirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Unified file system operations that work for both local and remote workspaces.
#[async_trait]
pub trait WorkspaceFileSystem: Send + Sync {
    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>>;
    async fn read_file_text(&self, path: &str) -> anyhow::Result<String>;
    async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()>;
    async fn exists(&self, path: &str) -> anyhow::Result<bool>;
    async fn is_file(&self, path: &str) -> anyhow::Result<bool>;
    async fn is_dir(&self, path: &str) -> anyhow::Result<bool>;
    /// List immediate children (non-recursive). Symlinks may be included; callers often skip them.
    async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>>;
}

/// Unified shell execution for both local and remote workspaces.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceCommandOptions {
    pub timeout_ms: Option<u64>,
    pub cancellation_token: Option<CancellationToken>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub interrupted: bool,
    pub timed_out: bool,
}

impl WorkspaceCommandResult {
    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

#[async_trait]
pub trait WorkspaceShell: Send + Sync {
    /// Execute a command and return a structured result.
    async fn exec_with_options(
        &self,
        command: &str,
        options: WorkspaceCommandOptions,
    ) -> anyhow::Result<WorkspaceCommandResult>;

    /// Execute a command and return (stdout, stderr, exit_code).
    async fn exec(
        &self,
        command: &str,
        timeout_ms: Option<u64>,
    ) -> anyhow::Result<(String, String, i32)> {
        let result = self
            .exec_with_options(
                command,
                WorkspaceCommandOptions {
                    timeout_ms,
                    ..Default::default()
                },
            )
            .await?;

        if result.timed_out {
            anyhow::bail!(
                "Command timed out after {}ms",
                timeout_ms.unwrap_or_default()
            );
        }
        if result.interrupted {
            anyhow::bail!("Command was cancelled");
        }

        Ok((result.stdout, result.stderr, result.exit_code))
    }
}

/// Bundle of workspace I/O services injected into ToolUseContext.
/// Tools call `context.workspace_services()` and use these trait objects
/// instead of directly checking `get_remote_workspace_manager()`.
pub struct WorkspaceServices {
    pub fs: Arc<dyn WorkspaceFileSystem>,
    pub shell: Arc<dyn WorkspaceShell>,
}

impl Clone for WorkspaceServices {
    fn clone(&self) -> Self {
        Self {
            fs: Arc::clone(&self.fs),
            shell: Arc::clone(&self.shell),
        }
    }
}

impl std::fmt::Debug for WorkspaceServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceServices")
            .field("fs", &"<dyn WorkspaceFileSystem>")
            .field("shell", &"<dyn WorkspaceShell>")
            .finish()
    }
}

// ============================================================
// Local implementations
// ============================================================

/// Local file system implementation of `WorkspaceFileSystem`.
pub struct LocalWorkspaceFs;

#[async_trait]
impl WorkspaceFileSystem for LocalWorkspaceFs {
    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
        Ok(tokio::fs::read_to_string(path).await?)
    }

    async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        Ok(tokio::fs::write(path, contents).await?)
    }

    async fn exists(&self, path: &str) -> anyhow::Result<bool> {
        Ok(tokio::fs::try_exists(path).await.unwrap_or(false))
    }

    async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
        match tokio::fs::metadata(path).await {
            Ok(m) => Ok(m.is_file()),
            Err(_) => Ok(false),
        }
    }

    async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
        match tokio::fs::metadata(path).await {
            Ok(m) => Ok(m.is_dir()),
            Err(_) => Ok(false),
        }
    }

    async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
        let mut out = Vec::new();
        let mut rd = tokio::fs::read_dir(path).await?;
        while let Ok(Some(entry)) = rd.next_entry().await {
            let p = entry.path();
            let meta = tokio::fs::symlink_metadata(&p).await?;
            if meta.file_type().is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let path_str = p.to_string_lossy().to_string();
            let is_dir = meta.is_dir();
            out.push(WorkspaceDirEntry {
                name,
                path: path_str,
                is_dir,
                is_symlink: false,
            });
        }
        Ok(out)
    }
}

/// Local shell implementation of `WorkspaceShell`.
pub struct LocalWorkspaceShell {
    workspace_root: String,
}

impl LocalWorkspaceShell {
    pub fn new(workspace_root: String) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl WorkspaceShell for LocalWorkspaceShell {
    async fn exec_with_options(
        &self,
        command: &str,
        options: WorkspaceCommandOptions,
    ) -> anyhow::Result<WorkspaceCommandResult> {
        use std::process::Stdio;
        use tokio::io::AsyncReadExt;

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd.current_dir(&self.workspace_root);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture command stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture command stderr"))?;

        let stdout_task = tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer).await?;
            Ok::<Vec<u8>, std::io::Error>(buffer)
        });
        let stderr_task = tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer).await?;
            Ok::<Vec<u8>, std::io::Error>(buffer)
        });

        let mut interrupted = false;
        let mut timed_out = false;
        let mut exit_code = -1;
        let deadline = options
            .timeout_ms
            .map(|ms| tokio::time::Instant::now() + std::time::Duration::from_millis(ms));

        loop {
            if let Some(token) = options.cancellation_token.as_ref() {
                if token.is_cancelled() {
                    interrupted = true;
                    let _ = child.start_kill();
                    break;
                }
            }

            if let Some(deadline) = deadline {
                if tokio::time::Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.start_kill();
                    break;
                }
            }

            if let Some(status) = child.try_wait()? {
                exit_code = status.code().unwrap_or(-1);
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }

        if interrupted || timed_out {
            let _ = child.wait().await;
            if interrupted {
                #[cfg(windows)]
                {
                    exit_code = -1073741510;
                }
                #[cfg(not(windows))]
                {
                    exit_code = 130;
                }
            } else if timed_out {
                exit_code = 124;
            }
        }

        let stdout = String::from_utf8_lossy(
            &stdout_task
                .await
                .map_err(|e| anyhow::anyhow!("Failed to join stdout task: {}", e))??,
        )
        .to_string();
        let stderr = String::from_utf8_lossy(
            &stderr_task
                .await
                .map_err(|e| anyhow::anyhow!("Failed to join stderr task: {}", e))??,
        )
        .to_string();

        Ok(WorkspaceCommandResult {
            stdout,
            stderr,
            exit_code,
            interrupted,
            timed_out,
        })
    }
}

/// Build `WorkspaceServices` backed by the local filesystem and shell.
pub fn local_workspace_services(workspace_root: String) -> WorkspaceServices {
    WorkspaceServices {
        fs: Arc::new(LocalWorkspaceFs),
        shell: Arc::new(LocalWorkspaceShell::new(workspace_root)),
    }
}

// ============================================================
// Remote (SSH) implementations
// ============================================================

use crate::service::remote_ssh::{RemoteFileService, SSHConnectionManager};

/// SSH-backed file system implementation.
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
        self.file_service
            .read_file(&self.connection_id, path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
        let bytes = self.read_file(path).await?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()> {
        self.file_service
            .write_file(&self.connection_id, path, contents)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn exists(&self, path: &str) -> anyhow::Result<bool> {
        self.file_service
            .exists(&self.connection_id, path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
        self.file_service
            .is_file(&self.connection_id, path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
        self.file_service
            .is_dir(&self.connection_id, path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
        let entries = self
            .file_service
            .read_dir(&self.connection_id, path)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(entries
            .into_iter()
            .map(|e| WorkspaceDirEntry {
                name: e.name,
                path: e.path,
                is_dir: e.is_dir,
                is_symlink: e.is_symlink,
            })
            .collect())
    }
}

/// SSH-backed shell implementation.
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
        // Wrap the command with cd to workspace root so all commands
        // execute in the correct working directory on the remote server.
        let wrapped = format!("cd {} && {}", shell_escape(&self.workspace_root), command);
        let result = self
            .ssh_manager
            .execute_command_with_options(
                &self.connection_id,
                &wrapped,
                crate::service::remote_ssh::SSHCommandOptions {
                    timeout_ms: options.timeout_ms,
                    cancellation_token: options.cancellation_token,
                },
            )
            .await?;

        Ok(WorkspaceCommandResult {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            interrupted: result.interrupted,
            timed_out: result.timed_out,
        })
    }
}

/// Escape a string for safe use in a shell command.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build `WorkspaceServices` backed by SSH for a remote workspace.
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
