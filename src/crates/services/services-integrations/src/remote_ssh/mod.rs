//! Remote SSH service contracts.
//!
//! `bitfun-core::service::remote_ssh` remains as the compatibility facade for
//! the legacy public path.

pub mod paths;
pub mod remote_git;
mod shell;
#[cfg(feature = "remote-ssh-concrete")]
mod transport;
pub mod types;
pub mod workspace_registry;
#[cfg(feature = "workspace-search")]
pub mod workspace_search;
mod workspace_services;

#[cfg(not(feature = "remote-ssh-concrete"))]
mod disabled;
#[cfg(feature = "remote-ssh-concrete")]
pub mod manager;
#[cfg(feature = "remote-ssh-concrete")]
mod password_vault;
#[cfg(feature = "remote-ssh-concrete")]
pub mod relay_deploy;
#[cfg(feature = "remote-ssh-concrete")]
mod remote_exec;
#[cfg(feature = "remote-ssh-concrete")]
mod remote_exec_runtime_port;
#[cfg(feature = "remote-ssh-concrete")]
pub mod remote_fs;
#[cfg(feature = "remote-ssh-concrete")]
pub mod remote_terminal;

pub use paths::*;
pub use remote_git::{build_remote_git_command, shell_quote_posix};
#[cfg(feature = "remote-ssh-concrete")]
pub use transport::{
    WorkspaceProcessCompletion, WorkspaceProcessControl, WorkspaceProcessExit,
    WorkspaceProcessSignal, WorkspaceReader, WorkspaceStdio, WorkspaceWriter,
};
pub use types::*;
pub use workspace_registry::*;
pub use workspace_services::{remote_workspace_services, RemoteWorkspaceFs, RemoteWorkspaceShell};

#[cfg(not(feature = "remote-ssh-concrete"))]
pub use disabled::{
    get_global_remote_exec_process_manager, KnownHostEntry, PTYSession, PortForward,
    PortForwardDirection, PortForwardManager, RemoteExecCommandRequest, RemoteExecCommandResponse,
    RemoteExecControlAction, RemoteExecControlOrigin, RemoteExecControlRequest, RemoteExecError,
    RemoteExecProcessLifecycleEvent, RemoteExecProcessLifecycleStatus, RemoteExecProcessManager,
    RemoteExecResult, RemoteExecSessionCompletion, RemoteExecSessionCompletionSource,
    RemoteExecSessionCompletionStatus, RemoteFileService, RemoteSendStdinRequest,
    RemoteTerminalManager, RemoteTerminalSession, RemoteWriteStdinRequest, SSHConnectionManager,
    SessionStatus,
};
#[cfg(feature = "remote-ssh-concrete")]
pub use manager::{
    KnownHostEntry, PTYSession, PortForward, PortForwardDirection, PortForwardManager,
    SSHConnectionManager,
};
#[cfg(feature = "remote-ssh-concrete")]
pub use remote_exec::{
    get_global_remote_exec_process_manager, RemoteExecCommandRequest, RemoteExecCommandResponse,
    RemoteExecControlAction, RemoteExecControlOrigin, RemoteExecControlRequest, RemoteExecError,
    RemoteExecProcessLifecycleEvent, RemoteExecProcessLifecycleStatus, RemoteExecProcessManager,
    RemoteExecResult, RemoteExecSessionCompletion, RemoteExecSessionCompletionSource,
    RemoteExecSessionCompletionStatus, RemoteSendStdinRequest, RemoteWriteStdinRequest,
};
#[cfg(feature = "remote-ssh-concrete")]
pub use remote_exec_runtime_port::{RemoteExecRuntimePort, RemoteExecSshManagerProvider};
#[cfg(feature = "remote-ssh-concrete")]
pub use remote_fs::RemoteFileService;
#[cfg(feature = "remote-ssh-concrete")]
pub use remote_terminal::{RemoteTerminalManager, RemoteTerminalSession, SessionStatus};
