//! Remote SSH Service Module
//!
//! Provides SSH connection management and SFTP-based remote file operations.
//! This allows BitFun to work with files on remote servers via SSH,
//! similar to VSCode's Remote SSH extension.

pub mod manager;
mod password_vault;
pub mod remote_fs;
pub mod remote_terminal;
pub mod types;
pub mod workspace_state;

pub use manager::{
    KnownHostEntry, PTYSession, PortForward, PortForwardDirection, PortForwardManager,
    SSHConnectionManager,
};
pub use remote_fs::RemoteFileService;
pub use remote_terminal::{RemoteTerminalManager, RemoteTerminalSession, SessionStatus};
pub use types::*;
pub use workspace_state::{
    canonicalize_local_workspace_root, get_remote_workspace_manager, init_remote_workspace_manager,
    is_remote_path, is_remote_workspace_active, local_workspace_roots_equal,
    local_workspace_stable_storage_id, lookup_remote_connection,
    lookup_remote_connection_with_hint, normalize_local_workspace_root_for_stable_id,
    normalize_remote_workspace_path, remote_workspace_stable_id, workspace_logical_key,
    RemoteWorkspaceEntry, RemoteWorkspaceState, RemoteWorkspaceStateManager,
    LOCAL_WORKSPACE_SSH_HOST,
};
