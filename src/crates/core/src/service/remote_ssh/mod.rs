//! Remote SSH Service Module
//!
//! Provides SSH connection management and SFTP-based remote file operations.
//! This allows BitFun to work with files on remote servers via SSH,
//! similar to VSCode's Remote SSH extension.

pub mod manager;
pub mod remote_fs;
pub mod remote_terminal;
pub mod types;
pub mod workspace_state;

pub use manager::{
    KnownHostEntry, PortForward, PortForwardDirection, PortForwardManager, PTYSession,
    SSHConnectionManager,
};
pub use remote_fs::RemoteFileService;
pub use remote_terminal::{RemoteTerminalManager, RemoteTerminalSession, SessionStatus};
pub use types::*;
pub use workspace_state::{
    get_remote_workspace_manager, init_remote_workspace_manager, is_remote_workspace_active,
    is_remote_path, lookup_remote_connection, lookup_remote_connection_with_hint,
    normalize_remote_workspace_path, RemoteWorkspaceEntry, RemoteWorkspaceState,
    RemoteWorkspaceStateManager,
};
