use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    /// Local path used for session persistence when the workspace is remote.
    /// For local workspaces this is `None` (we use `root_path` directly).
    pub local_session_path: Option<PathBuf>,
}

impl WorkspaceBinding {
    pub fn new(workspace_id: Option<String>, root_path: PathBuf) -> Self {
        Self {
            workspace_id,
            root_path,
            backend: WorkspaceBackend::Local,
            local_session_path: None,
        }
    }

    pub fn new_remote(
        workspace_id: Option<String>,
        root_path: PathBuf,
        connection_id: String,
        connection_name: String,
        local_session_path: PathBuf,
    ) -> Self {
        Self {
            workspace_id,
            root_path,
            backend: WorkspaceBackend::Remote {
                connection_id,
                connection_name,
            },
            local_session_path: Some(local_session_path),
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
    /// Remote workspaces store sessions locally; local workspaces use root_path.
    pub fn session_storage_path(&self) -> &Path {
        self.local_session_path.as_deref().unwrap_or(&self.root_path)
    }
}

// ============================================================
// Workspace-level I/O abstractions — tools program against these
// traits instead of checking is_remote themselves.
// ============================================================

/// Unified file system operations that work for both local and remote workspaces.
#[async_trait]
pub trait WorkspaceFileSystem: Send + Sync {
    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>>;
    async fn read_file_text(&self, path: &str) -> anyhow::Result<String>;
    async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()>;
    async fn exists(&self, path: &str) -> anyhow::Result<bool>;
    async fn is_file(&self, path: &str) -> anyhow::Result<bool>;
    async fn is_dir(&self, path: &str) -> anyhow::Result<bool>;
}

/// Unified shell execution for both local and remote workspaces.
#[async_trait]
pub trait WorkspaceShell: Send + Sync {
    /// Execute a command and return (stdout, stderr, exit_code).
    async fn exec(&self, command: &str, timeout_ms: Option<u64>) -> anyhow::Result<(String, String, i32)>;
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
}

/// Local shell implementation of `WorkspaceShell`.
pub struct LocalWorkspaceShell;

#[async_trait]
impl WorkspaceShell for LocalWorkspaceShell {
    async fn exec(&self, command: &str, timeout_ms: Option<u64>) -> anyhow::Result<(String, String, i32)> {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);

        let timeout = std::time::Duration::from_millis(timeout_ms.unwrap_or(30_000));

        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| anyhow::anyhow!("Command timed out after {}ms", timeout.as_millis()))??;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        Ok((stdout, stderr, exit_code))
    }
}

/// Build `WorkspaceServices` backed by the local filesystem and shell.
pub fn local_workspace_services() -> WorkspaceServices {
    WorkspaceServices {
        fs: Arc::new(LocalWorkspaceFs),
        shell: Arc::new(LocalWorkspaceShell),
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
        Self { connection_id, file_service }
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
}

/// SSH-backed shell implementation.
pub struct RemoteWorkspaceShell {
    ssh_manager: SSHConnectionManager,
    connection_id: String,
    workspace_root: String,
}

impl RemoteWorkspaceShell {
    pub fn new(connection_id: String, ssh_manager: SSHConnectionManager, workspace_root: String) -> Self {
        Self { connection_id, ssh_manager, workspace_root }
    }
}

#[async_trait]
impl WorkspaceShell for RemoteWorkspaceShell {
    async fn exec(&self, command: &str, _timeout_ms: Option<u64>) -> anyhow::Result<(String, String, i32)> {
        // Wrap the command with cd to workspace root so all commands
        // execute in the correct working directory on the remote server.
        let wrapped = format!("cd {} && {}", shell_escape(&self.workspace_root), command);
        self.ssh_manager
            .execute_command(&self.connection_id, &wrapped)
            .await
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
        shell: Arc::new(RemoteWorkspaceShell::new(connection_id, ssh_manager, workspace_root)),
    }
}

