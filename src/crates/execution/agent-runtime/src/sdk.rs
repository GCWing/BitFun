//! Narrow Agent Runtime SDK facade.
//!
//! This module is the stable entrypoint for embedding the portable agent
//! runtime with caller-provided ports. Concrete product assembly remains
//! outside this crate. Plugin runtime facts are exposed as non-executable
//! availability only until host gates land.

pub const AGENT_RUNTIME_SDK_API_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AgentRuntimeSdkStability {
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AgentRuntimeSdkCompatibility {
    pub api_version: u32,
    pub crate_version: &'static str,
    pub stability: AgentRuntimeSdkStability,
}

impl AgentRuntimeSdkCompatibility {
    pub const fn current() -> Self {
        Self {
            api_version: AGENT_RUNTIME_SDK_API_VERSION,
            crate_version: env!("CARGO_PKG_VERSION"),
            stability: AgentRuntimeSdkStability::Preview,
        }
    }
}

pub use crate::context_profile::{ContextProfile, ContextProfilePolicy, ModelCapabilityProfile};
pub use crate::post_call_hooks::{
    RuntimeHookErrorPolicy, RuntimeHookKind, RuntimeHookPlan, RuntimeHookRegistry,
    RuntimeHookRegistryBuildError,
};
pub use crate::runtime::{
    AgentEventStream, AgentRunHandle, AgentRunRequest, AgentRuntime, AgentRuntimeBuilder,
    RuntimeAgentRegistry, RuntimeAgentRegistryQuery, RuntimeBuildError, RuntimeError,
    RuntimeToolRegistry, SessionSelector,
};
pub use crate::session_state::{session_state_label_for_state, ProcessingPhase, SessionState};
pub use bitfun_agent_tools::{ToolRegistry, ToolRegistryItem};
pub use bitfun_harness::{
    build_descriptor_harness_registry, HarnessCapability, HarnessProviderDescriptor,
    HarnessRegistry, HarnessWorkflow,
};
pub use bitfun_runtime_ports::{
    AgentDialogTurnPort, AgentDialogTurnRequest, AgentInputAttachment, AgentLifecycleDeliveryPort,
    AgentSessionCreateRequest, AgentSessionCreateResult, AgentSessionDeleteRequest,
    AgentSessionListRequest, AgentSessionManagementPort, AgentSessionSummary,
    AgentSessionWorkspaceRequest, AgentSubmissionPort, AgentSubmissionRequest,
    AgentSubmissionResult, AgentSubmissionSource, AgentThreadGoalCreateRequest,
    AgentThreadGoalDeliveryRequest, AgentThreadGoalGetRequest, AgentThreadGoalManagementPort,
    AgentThreadGoalUpdateStatusRequest, AgentTurnCancellationPort, AgentTurnCancellationRequest,
    AgentTurnCancellationResult, ClockPort, DialogSubmitOutcome, FileSystemPort, GitPort,
    McpCatalogPort, NetworkPort, PermissionDecision, PermissionPort, PermissionRequest,
    PluginRuntimeAvailability, PluginRuntimeUnavailableReason, PortError, PortResult,
    RemoteAssistantWorkspaceFacts, RemoteCapabilityPort, RemoteConnectionPort,
    RemoteProjectionPort, RemoteRecentWorkspaceFacts, RemoteWorkspaceFacts,
    RemoteWorkspaceFileRuntimeHost, RemoteWorkspaceKind, RemoteWorkspacePort,
    RemoteWorkspaceRuntimeHost, RemoteWorkspaceUpdate, RuntimeEventEnvelope, RuntimeEventSink,
    RuntimeEventType, RuntimeServiceCapability, RuntimeServicePort, SessionStorageKind,
    SessionStoragePathRequest, SessionStoragePathResolution, SessionStorePort, TerminalPort,
    ThreadGoal, ThreadGoalStatus, UiExtensionAvailability, WorkspacePort,
};
pub use bitfun_runtime_services::{
    CapabilityAvailability, RuntimeServices, RuntimeServicesBuilder, RuntimeServicesError,
    RuntimeServicesProvider, RuntimeServicesRegistry,
};
