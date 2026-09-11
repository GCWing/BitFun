mod builtin_clients;
mod config;
mod dsh_profile;
mod manager;
mod prompt;
mod remote_capability_store;
mod remote_session;
mod remote_shell;
mod requirements;
mod session_options;
mod session_persistence;
mod stream;
mod tool;
mod tool_card_bridge;
mod transport;

pub use config::{
    AcpClientConfig, AcpClientConfigFile, AcpClientInfo, AcpClientPermissionMode,
    AcpClientRequirementProbe, AcpClientStatus, AcpClientSubagentConfig, AcpRequirementProbeItem,
    RemoteAcpClientRequirementSnapshot,
};
pub use manager::{
    AcpClientPermissionResponse, AcpClientService, AcpSessionConfigValue,
    CreateAcpFlowSessionRecordResponse, SetAcpSessionConfigOptionRequest,
    SetAcpSessionModelRequest, SubmitAcpPermissionResponseRequest,
};
pub use session_options::{
    AcpAvailableCommand, AcpPlanEntry, AcpSessionConfigKind, AcpSessionConfigOption,
    AcpSessionConfigSelectOption, AcpSessionContextUsage, AcpSessionModelOption, AcpSessionOptions,
};
pub use stream::AcpClientStreamEvent;
