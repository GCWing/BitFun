//! Type definitions for Remote SSH service

use serde::{Deserialize, Deserializer, Serialize};
use tokio_util::sync::CancellationToken;

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
    #[serde(deserialize_with = "deserialize_ssh_auth_method")]
    pub auth: SSHAuthMethod,
    /// Default remote working directory
    #[serde(rename = "defaultWorkspace")]
    pub default_workspace: Option<String>,
    /// OpenSSH-compatible comma-separated jump host chain.
    ///
    /// Each entry may be an alias from `~/.ssh/config` or
    /// `[user@]host[:port]`. Jump authentication is resolved from the matching
    /// SSH config entry, so every hop may use a different user and identity.
    #[serde(default)]
    pub proxy_jump: Option<String>,
    /// Optional Docker container that becomes the effective workspace target.
    #[serde(default)]
    pub container: Option<ContainerWorkspaceConfig>,
}

impl SSHConnectionConfig {
    /// Compare the connection parameters that affect the underlying SSH session
    /// (host, port, username, auth type). Used to detect config drift between
    /// an active connection and the latest saved config so that a reconnect
    /// can be triggered when the user changes the port or other params.
    pub fn connection_params_equal(&self, other: &Self) -> bool {
        self.host == other.host
            && self.port == other.port
            && self.username == other.username
            && std::mem::discriminant(&self.auth) == std::mem::discriminant(&other.auth)
            && self.proxy_jump == other.proxy_jump
            && self.container == other.container
    }

    pub fn uses_local_docker(&self) -> bool {
        self.container
            .as_ref()
            .is_some_and(|container| container.local)
    }

    pub fn uses_docker_exec(&self) -> bool {
        self.container.as_ref().is_some_and(|container| {
            matches!(
                container.access,
                ContainerAccess::DockerExec | ContainerAccess::Auto
            )
        })
    }
}

fn default_docker_path() -> String {
    "docker".to_string()
}

fn default_container_shell() -> String {
    "/bin/sh".to_string()
}

fn default_true() -> bool {
    true
}

/// How BitFun enters a configured container workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerAccess {
    /// The top-level SSH host/port/user fields point at sshd in the container.
    Sshd,
    /// Execute every workspace operation through `docker exec`.
    DockerExec,
    /// Prefer the configured container target. P0 currently resolves this to
    /// `docker exec`; the serialized value leaves room for sshd probing later.
    Auto,
}

/// Docker container workspace configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContainerWorkspaceConfig {
    /// Container name or ID.
    pub name: String,
    pub access: ContainerAccess,
    /// Run Docker on the local BitFun machine instead of an SSH host.
    #[serde(default)]
    pub local: bool,
    /// Docker CLI path on the machine that owns the container.
    #[serde(default = "default_docker_path")]
    pub docker_path: String,
    /// Shell inside the container.
    #[serde(default = "default_container_shell")]
    pub shell: String,
    /// Optional container user passed to `docker exec --user`.
    #[serde(default)]
    pub user: Option<String>,
    /// Keep stdin open with `docker exec -i`.
    #[serde(default = "default_true")]
    pub interactive: bool,
}

/// SSH authentication method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SSHAuthMethod {
    /// Password authentication
    Password { password: String },
    /// Private key authentication
    PrivateKey {
        /// Path to private key file on local machine
        #[serde(rename = "keyPath")]
        key_path: String,
        /// Optional passphrase for encrypted private key
        passphrase: Option<String>,
    },
}

/// Legacy `{"type":"Agent"}` in saved config maps to default private key path.
fn deserialize_ssh_auth_method<'de, D>(deserializer: D) -> Result<SSHAuthMethod, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(tag = "type")]
    enum Helper {
        Password {
            password: String,
        },
        PrivateKey {
            #[serde(rename = "keyPath")]
            key_path: String,
            passphrase: Option<String>,
        },
        Agent,
    }
    match Helper::deserialize(deserializer)? {
        Helper::Password { password } => Ok(SSHAuthMethod::Password { password }),
        Helper::PrivateKey {
            key_path,
            passphrase,
        } => Ok(SSHAuthMethod::PrivateKey {
            key_path,
            passphrase,
        }),
        Helper::Agent => Ok(SSHAuthMethod::PrivateKey {
            key_path: "~/.ssh/id_rsa".to_string(),
            passphrase: None,
        }),
    }
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
    #[serde(rename = "authType", deserialize_with = "deserialize_saved_auth_type")]
    pub auth_type: SavedAuthType,
    #[serde(rename = "defaultWorkspace")]
    pub default_workspace: Option<String>,
    #[serde(rename = "lastConnected")]
    pub last_connected: Option<u64>,
    #[serde(default)]
    pub proxy_jump: Option<String>,
    #[serde(default)]
    pub container: Option<ContainerWorkspaceConfig>,
}

/// Saved auth type (excludes sensitive credentials; password ciphertext is in `ssh_password_vault.json`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SavedAuthType {
    Password,
    PrivateKey {
        #[serde(rename = "keyPath")]
        key_path: String,
    },
}

fn deserialize_saved_auth_type<'de, D>(deserializer: D) -> Result<SavedAuthType, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(tag = "type")]
    enum Helper {
        Password,
        PrivateKey {
            #[serde(rename = "keyPath")]
            key_path: String,
        },
        Agent,
    }
    match Helper::deserialize(deserializer)? {
        Helper::Password => Ok(SavedAuthType::Password),
        Helper::PrivateKey { key_path } => Ok(SavedAuthType::PrivateKey { key_path }),
        Helper::Agent => Ok(SavedAuthType::PrivateKey {
            key_path: "~/.ssh/id_rsa".to_string(),
        }),
    }
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

/// Options for executing a remote SSH command.
#[derive(Debug, Clone, Default)]
pub struct SSHCommandOptions {
    pub timeout_ms: Option<u64>,
    pub cancellation_token: Option<CancellationToken>,
}

/// Result of executing a remote SSH command.
#[derive(Debug, Clone)]
pub struct SSHCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub interrupted: bool,
    pub timed_out: bool,
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
    /// SSH config `host`; used for `~/.bitfun/remote_ssh/{host}/...` session storage.
    #[serde(default)]
    pub ssh_host: String,
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
    /// OpenSSH ProxyJump chain, preserving aliases and order.
    #[serde(default)]
    pub proxy_jump: Option<String>,
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
