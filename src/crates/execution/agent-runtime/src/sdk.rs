//! Narrow Agent Runtime SDK facade.
//!
//! This module is the stable entrypoint for embedding the portable agent
//! runtime with caller-provided ports. Concrete product assembly remains
//! outside this crate.

pub use crate::post_call_hooks::{
    RuntimeHookErrorPolicy, RuntimeHookKind, RuntimeHookPlan, RuntimeHookRegistry,
    RuntimeHookRegistryBuildError,
};
pub use crate::runtime::{
    AgentEventStream, AgentRunHandle, AgentRunRequest, AgentRuntime, AgentRuntimeBuilder,
    RuntimeAgentRegistry, RuntimeAgentRegistryQuery, RuntimeBuildError, RuntimeError,
    RuntimeToolRegistry, SessionSelector,
};
pub use bitfun_runtime_ports::{
    AgentDialogTurnPort, AgentDialogTurnRequest, AgentInputAttachment, AgentLifecycleDeliveryPort,
    AgentSessionCreateRequest, AgentSessionCreateResult, AgentSessionDeleteRequest,
    AgentSessionListRequest, AgentSessionManagementPort, AgentSessionSummary,
    AgentSessionWorkspaceRequest, AgentSubmissionPort, AgentSubmissionRequest,
    AgentSubmissionResult, AgentSubmissionSource, AgentThreadGoalDeliveryRequest,
    AgentTurnCancellationPort, AgentTurnCancellationRequest, AgentTurnCancellationResult,
    DialogSubmitOutcome, PortError, RuntimeEventEnvelope, RuntimeEventType,
};
