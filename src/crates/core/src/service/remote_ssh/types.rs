//! Type definitions for Remote SSH service

use serde::{Deserialize, Serialize};

/// Workspace backend type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum WorkspaceBackend {
    /// Local workspace (default)
    Local,
    /// Remote SSH workspace
    Remote(RemoteWorkspaceInfo),
}

/// Remote workspace information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceInfo {
    /// SSH connection ID
    pub connection_id: String,
    /// Connection name (display name)
    pub connection_name: String,
    /// Remote path on the server
    pub remote_path: String,
}

/// SSH connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SSHConnectionConfig {
    /// Unique identifier for this connection
    pub id: String,
    /// Display name for the connection
    pub name: String,
    /// Remote host address (hostname or IP)
    pub host: String,
    /// SSH port (default: 22)
    pub port: u16,
    /// SSH username
    pub username: String,
    /// Authentication method
    pub auth: SSHAuthMethod,
    /// Default remote working directory
    #[serde(rename = "defaultWorkspace")]
    pub default_workspace: Option<String>,
}

/// SSH authentication method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SSHAuthMethod {
    /// Password authentication
    Password {
        password: String,
    },
    /// Private key authentication
    PrivateKey {
        /// Path to private key file on local machine
        #[serde(rename = "keyPath")]
        key_path: String,
        /// Optional passphrase for encrypted private key
        passphrase: Option<String>,
    },
    /// SSH agent authentication (uses system SSH agent)
    Agent,
}

/// Connection state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connection in progress
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection failed with error
    Failed { error: String },
}

/// Saved connection (without sensitive data like passwords)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(rename = "authType")]
    pub auth_type: SavedAuthType,
    #[serde(rename = "defaultWorkspace")]
    pub default_workspace: Option<String>,
    #[serde(rename = "lastConnected")]
    pub last_connected: Option<u64>,
}

/// Saved auth type (excludes sensitive credentials)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SavedAuthType {
    Password,  // Password is stored in system keychain
    PrivateKey {
        #[serde(rename = "keyPath")]
        key_path: String,
    },
    Agent,
}

/// Remote file entry information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    #[serde(rename = "isFile")]
    pub is_file: bool,
    #[serde(rename = "isSymlink")]
    pub is_symlink: bool,
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub permissions: Option<String>,
}

/// Remote file tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTreeNode {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    pub children: Option<Vec<RemoteTreeNode>>,
}

/// Remote directory entry (for read_dir operations)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    #[serde(rename = "isFile")]
    pub is_file: bool,
    #[serde(rename = "isSymlink")]
    pub is_symlink: bool,
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub permissions: Option<String>,
}

/// Result of SSH connection attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SSHConnectionResult {
    pub success: bool,
    #[serde(rename = "connectionId")]
    pub connection_id: Option<String>,
    pub error: Option<String>,
    #[serde(rename = "serverInfo")]
    pub server_info: Option<ServerInfo>,
}

/// Remote server information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    #[serde(rename = "osType")]
    pub os_type: String,
    pub hostname: String,
    #[serde(rename = "homeDir")]
    pub home_dir: String,
}

/// Result of remote file operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Result of remote directory listing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteListResult {
    pub entries: Vec<RemoteFileEntry>,
    pub error: Option<String>,
}

/// Request to open a remote workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceRequest {
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    #[serde(rename = "remotePath")]
    pub remote_path: String,
}

/// Remote workspace info (persisted in `remote_workspace.json`).
/// `#[serde(default)]` keeps older files loadable if a field was absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspace {
    #[serde(default)]
    pub connection_id: String,
    #[serde(default)]
    pub remote_path: String,
    #[serde(default)]
    pub connection_name: String,
}

/// SSH config entry parsed from ~/.ssh/config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SSHConfigEntry {
    /// Host name (alias from SSH config)
    pub host: String,
    /// Actual hostname or IP
    pub hostname: Option<String>,
    /// SSH port
    pub port: Option<u16>,
    /// Username
    pub user: Option<String>,
    /// Path to identity file (private key)
    pub identity_file: Option<String>,
    /// Whether to use SSH agent
    pub agent: Option<bool>,
}

/// Result of looking up SSH config for a host
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SSHConfigLookupResult {
    /// Whether a config entry was found
    pub found: bool,
    /// Config entry if found
    pub config: Option<SSHConfigEntry>,
}
