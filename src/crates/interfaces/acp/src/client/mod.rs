mod builtin_clients;
mod cli_detect;
mod config;
mod launch_policy;
mod manager;
mod probe;
mod remote_capability_store;
mod remote_session;
mod remote_shell;
mod requirements;
mod session_options;
mod session_persistence;
mod stream;
mod tool;
mod tool_card_bridge;

pub use config::{
    AcpClientConfig, AcpClientConfigFile, AcpClientInfo, AcpClientPermissionMode,
    AcpClientRequirementProbe, AcpClientStatus, AcpRequirementProbeItem,
    RemoteAcpClientRequirementSnapshot,
};
pub use launch_policy::{apply_launch_policy, LaunchPolicyResult};
pub use manager::{
    AcpClientPermissionResponse, AcpClientService, AcpSessionConfigValue,
    CreateAcpFlowSessionRecordResponse, SetAcpSessionConfigOptionRequest,
    SetAcpSessionModelRequest, SubmitAcpPermissionResponseRequest,
};
pub use probe::{
    TryConnectResult, ACP_HANDSHAKE_TIMEOUT_SECS, CLI_DETECT_TIMEOUT_SECS,
    TRY_CONNECT_TOTAL_TIMEOUT_SECS,
};
pub use session_options::{
    AcpAvailableCommand, AcpPlanEntry, AcpSessionConfigKind, AcpSessionConfigOption,
    AcpSessionConfigSelectOption, AcpSessionContextUsage, AcpSessionModelOption, AcpSessionOptions,
};
pub use stream::AcpClientStreamEvent;
