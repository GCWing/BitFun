use super::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteWorkspaceKind {
    Normal,
    Assistant,
    Remote,
}

impl RemoteWorkspaceKind {
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Assistant => "assistant",
            Self::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceFacts {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    pub kind: RemoteWorkspaceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteSessionWorkspaceIdentity {
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
}

impl RemoteSessionWorkspaceIdentity {
    pub fn new(remote_connection_id: Option<String>, remote_ssh_host: Option<String>) -> Self {
        Self {
            remote_connection_id,
            remote_ssh_host,
        }
    }

    pub fn from_workspace(workspace: &RemoteWorkspaceFacts) -> Self {
        Self::new(
            workspace.remote_connection_id.clone(),
            workspace.remote_ssh_host.clone(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.remote_connection_id.is_none() && self.remote_ssh_host.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRecentWorkspaceFacts {
    pub path: String,
    pub name: String,
    pub last_opened: String,
    pub kind: RemoteWorkspaceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAssistantWorkspaceFacts {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceUpdate {
    pub path: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSessionMetadata {
    pub session_id: String,
    pub name: String,
    pub agent_type: String,
    pub created_at_ms: u64,
    pub last_active_at_ms: u64,
    pub turn_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileContent {
    pub name: String,
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileChunk {
    pub name: String,
    pub bytes: Vec<u8>,
    pub offset: u64,
    pub chunk_size: u64,
    pub total_size: u64,
    pub mime_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileInfo {
    pub name: String,
    pub size: u64,
    pub mime_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteFileChunkRange {
    pub start: usize,
    pub end: usize,
    pub chunk_size: u64,
}

/// Old remote-connect host compatibility trait for workspace commands.
#[async_trait::async_trait]
pub trait RemoteWorkspaceRuntimeHost: Send + Sync {
    async fn current_workspace(&self) -> Option<RemoteWorkspaceFacts>;
    async fn recent_workspaces(&self) -> Vec<RemoteRecentWorkspaceFacts>;
    async fn open_workspace(
        &self,
        path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> Result<RemoteWorkspaceUpdate, String>;
    async fn assistant_workspaces(&self) -> Vec<RemoteAssistantWorkspaceFacts>;
    async fn open_assistant_workspace(&self, path: &str) -> Result<RemoteWorkspaceUpdate, String>;
}

/// Typed registration boundary for remote workspace providers.
pub trait RemoteWorkspacePort: RuntimeServicePort + RemoteWorkspaceRuntimeHost {}

impl<T> RemoteWorkspacePort for T where T: RuntimeServicePort + RemoteWorkspaceRuntimeHost + ?Sized {}

/// Old remote-connect host compatibility trait for initial sync.
#[async_trait::async_trait]
pub trait RemoteInitialSyncRuntimeHost: Send + Sync {
    async fn current_workspace(&self) -> Option<RemoteWorkspaceFacts>;
    async fn list_session_metadata(
        &self,
        workspace_path: &Path,
        workspace_identity: RemoteSessionWorkspaceIdentity,
    ) -> Result<Vec<RemoteSessionMetadata>, String>;
}

/// Old remote-connect host compatibility trait for remote file projection.
#[async_trait::async_trait]
pub trait RemoteWorkspaceFileRuntimeHost: Send + Sync {
    async fn resolve_remote_file_workspace_root(&self, session_id: Option<&str>)
        -> Option<PathBuf>;
}

/// Typed registration boundary for remote filesystem/terminal/image projection providers.
pub trait RemoteProjectionPort: RuntimeServicePort + RemoteWorkspaceFileRuntimeHost {}

impl<T> RemoteProjectionPort for T where
    T: RuntimeServicePort + RemoteWorkspaceFileRuntimeHost + ?Sized
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_workspace_contracts_preserve_workspace_and_session_facts() {
        let workspace = RemoteWorkspaceFacts {
            path: "/workspace/project".to_string(),
            name: "project".to_string(),
            git_branch: Some("main".to_string()),
            kind: RemoteWorkspaceKind::Remote,
            assistant_id: Some("assistant_1".to_string()),
            remote_connection_id: Some("conn-1".to_string()),
            remote_ssh_host: Some("host-1".to_string()),
        };
        let session = RemoteSessionMetadata {
            session_id: "session_1".to_string(),
            name: "Research".to_string(),
            agent_type: "CodeAgent".to_string(),
            created_at_ms: 10,
            last_active_at_ms: 20,
            turn_count: 3,
        };

        assert_eq!(workspace.kind.as_wire_str(), "remote");
        assert_eq!(workspace.assistant_id.as_deref(), Some("assistant_1"));
        assert_eq!(workspace.remote_connection_id.as_deref(), Some("conn-1"));
        assert_eq!(workspace.remote_ssh_host.as_deref(), Some("host-1"));
        assert_eq!(session.turn_count, 3);
    }

    #[test]
    fn remote_projection_contract_preserves_file_chunk_identity() {
        let chunk = RemoteWorkspaceFileChunk {
            name: "report.md".to_string(),
            bytes: b"chunk".to_vec(),
            offset: 6,
            chunk_size: 5,
            total_size: 11,
            mime_type: "text/markdown",
        };

        assert_eq!(chunk.name, "report.md");
        assert_eq!(chunk.bytes, b"chunk");
        assert_eq!(chunk.offset + chunk.chunk_size, chunk.total_size);
    }
}
