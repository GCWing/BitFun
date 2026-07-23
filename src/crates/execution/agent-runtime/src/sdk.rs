//! Narrow Agent Runtime SDK facade.
//!
//! This module is the stable entrypoint for embedding the portable agent
//! runtime with caller-provided ports. Concrete product assembly remains
//! outside this crate. The SDK facade exposes stable agent/session/event ports;
//! product assembly owns plugin-host handoff through the internal runtime
//! builder, not through this SDK surface.

use std::sync::Arc;

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
pub use crate::event_source::{AgentEventReceiver, AgentEventSource, AgentSessionEventReceiver};
pub use crate::permission::{
    PermissionReplyResolution, PermissionRequestEventReceiver, PermissionRequestManager,
    PermissionRequestManagerError, AUTO_APPROVE_ASK_CONTEXT_KEY,
};
pub use crate::post_call_hooks::{
    RuntimeHookErrorPolicy, RuntimeHookKind, RuntimeHookPlan, RuntimeHookRegistry,
    RuntimeHookRegistryBuildError,
};
pub use crate::runtime::{
    AgentEventStream, AgentInteractionResponsePort, AgentRunHandle, AgentRunRequest,
    AgentSessionRestorePort, AgentSessionRestoreRequest, AgentSessionRestoreResult,
    AgentUserAnswersRequest, RuntimeAgentRegistry, RuntimeAgentRegistryQuery, RuntimeBuildError,
    RuntimeError, RuntimeToolRegistry, SessionSelector,
};
pub use crate::session_state::{session_state_label_for_state, ProcessingPhase, SessionState};
pub use bitfun_agent_tools::{ToolRegistry, ToolRegistryItem};
pub use bitfun_core_types::SessionUsageReport;
pub use bitfun_harness::{
    build_descriptor_harness_registry, HarnessCapability, HarnessProviderDescriptor,
    HarnessRegistry, HarnessWorkflow,
};
pub use bitfun_runtime_ports::{
    AgentBackgroundResultRequest, AgentDialogTurnPort, AgentDialogTurnRequest,
    AgentInputAttachment, AgentLifecycleDeliveryPort, AgentLocalCommandTurnPort,
    AgentLocalCommandTurnRecordRequest, AgentSessionArchiveRequest,
    AgentSessionArchiveStateRequest, AgentSessionClosePort, AgentSessionCreateRequest,
    AgentSessionCreateResult, AgentSessionDeleteRequest, AgentSessionForkAtTurnRequest,
    AgentSessionForkPort, AgentSessionForkRequest, AgentSessionForkResult, AgentSessionListRequest,
    AgentSessionManagementPort, AgentSessionModePort, AgentSessionModeUpdateRequest,
    AgentSessionModelPort, AgentSessionModelUpdateRequest, AgentSessionRenameRequest,
    AgentSessionSummary, AgentSessionUsagePort, AgentSessionUsageRequest,
    AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest, AgentSubmissionPort,
    AgentSubmissionRequest, AgentSubmissionResult, AgentSubmissionSource,
    AgentThreadGoalCreateRequest, AgentThreadGoalDeliveryRequest, AgentThreadGoalGetRequest,
    AgentThreadGoalManagementPort, AgentThreadGoalUpdateStatusRequest,
    AgentTransientSessionDiscardRequest, AgentTurnCancellationPort, AgentTurnCancellationRequest,
    AgentTurnCancellationResult, AgentTurnSettlementPort, AgentTurnSettlementRequest, ClockPort,
    DialogSubmissionPolicy, DialogSubmitOutcome, FileSystemPort, GitPort, McpCatalogPort,
    NetworkPort, PermissionAuditRecord, PermissionDelegationContext, PermissionGrant,
    PermissionGrantKey, PermissionReply, PermissionReplySource, PermissionRequest,
    PermissionRequestEvent, PermissionRequestSource, PermissionRequestSourceKind, PortError,
    PortErrorKind, PortResult, RemoteAssistantWorkspaceFacts, RemoteCapabilityPort,
    RemoteConnectionPort, RemoteProjectionPort, RemoteRecentWorkspaceFacts, RemoteWorkspaceFacts,
    RemoteWorkspaceFileRuntimeHost, RemoteWorkspaceKind, RemoteWorkspacePort,
    RemoteWorkspaceRuntimeHost, RemoteWorkspaceUpdate, RuntimeEventEnvelope, RuntimeEventSink,
    RuntimeEventType, RuntimeServiceCapability, RuntimeServicePort, SessionStorageKind,
    SessionStoragePathRequest, SessionStoragePathResolution, SessionStorePort, SessionTranscript,
    SessionTranscriptReader, SessionTranscriptRequest, TerminalPort, ThreadGoal, ThreadGoalStatus,
    TranscriptContent, TranscriptMessage, TranscriptToolCall, WorkspacePort,
};
pub use bitfun_runtime_services::{
    CapabilityAvailability, RuntimeServices, RuntimeServicesBuilder, RuntimeServicesError,
    RuntimeServicesProvider, RuntimeServicesRegistry,
};

#[derive(Clone)]
pub struct AgentRuntime {
    inner: crate::runtime::AgentRuntime,
}

impl std::fmt::Debug for AgentRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntime").finish_non_exhaustive()
    }
}

#[derive(Default, Clone)]
pub struct AgentRuntimeBuilder {
    inner: crate::runtime::AgentRuntimeBuilder,
}

impl AgentRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            inner: crate::runtime::AgentRuntimeBuilder::new(),
        }
    }

    pub fn with_submission_port(mut self, port: Arc<dyn AgentSubmissionPort>) -> Self {
        self.inner = self.inner.with_submission_port(port);
        self
    }

    pub fn with_session_management_port(
        mut self,
        port: Arc<dyn AgentSessionManagementPort>,
    ) -> Self {
        self.inner = self.inner.with_session_management_port(port);
        self
    }

    pub fn with_session_close_port(mut self, port: Arc<dyn AgentSessionClosePort>) -> Self {
        self.inner = self.inner.with_session_close_port(port);
        self
    }

    pub fn with_session_model_port(mut self, port: Arc<dyn AgentSessionModelPort>) -> Self {
        self.inner = self.inner.with_session_model_port(port);
        self
    }

    pub fn with_session_mode_port(mut self, port: Arc<dyn AgentSessionModePort>) -> Self {
        self.inner = self.inner.with_session_mode_port(port);
        self
    }

    pub fn with_session_fork_port(mut self, port: Arc<dyn AgentSessionForkPort>) -> Self {
        self.inner = self.inner.with_session_fork_port(port);
        self
    }

    pub fn with_session_usage_port(mut self, port: Arc<dyn AgentSessionUsagePort>) -> Self {
        self.inner = self.inner.with_session_usage_port(port);
        self
    }

    pub fn with_turn_settlement_port(mut self, port: Arc<dyn AgentTurnSettlementPort>) -> Self {
        self.inner = self.inner.with_turn_settlement_port(port);
        self
    }

    pub fn with_session_restore_port(mut self, port: Arc<dyn AgentSessionRestorePort>) -> Self {
        self.inner = self.inner.with_session_restore_port(port);
        self
    }

    pub fn with_local_command_turn_port(
        mut self,
        port: Arc<dyn AgentLocalCommandTurnPort>,
    ) -> Self {
        self.inner = self.inner.with_local_command_turn_port(port);
        self
    }

    pub fn with_session_transcript_reader(
        mut self,
        reader: Arc<dyn SessionTranscriptReader>,
    ) -> Self {
        self.inner = self.inner.with_session_transcript_reader(reader);
        self
    }

    pub fn with_thread_goal_management_port(
        mut self,
        port: Arc<dyn AgentThreadGoalManagementPort>,
    ) -> Self {
        self.inner = self.inner.with_thread_goal_management_port(port);
        self
    }

    pub fn with_dialog_turn_port(mut self, port: Arc<dyn AgentDialogTurnPort>) -> Self {
        self.inner = self.inner.with_dialog_turn_port(port);
        self
    }

    pub fn with_lifecycle_delivery_port(
        mut self,
        port: Arc<dyn AgentLifecycleDeliveryPort>,
    ) -> Self {
        self.inner = self.inner.with_lifecycle_delivery_port(port);
        self
    }

    pub fn with_cancellation_port(mut self, port: Arc<dyn AgentTurnCancellationPort>) -> Self {
        self.inner = self.inner.with_cancellation_port(port);
        self
    }

    pub fn with_interaction_response_port(
        mut self,
        port: Arc<dyn AgentInteractionResponsePort>,
    ) -> Self {
        self.inner = self.inner.with_interaction_response_port(port);
        self
    }

    pub fn with_permission_request_manager(
        mut self,
        manager: Arc<PermissionRequestManager>,
    ) -> Self {
        self.inner = self.inner.with_permission_request_manager(manager);
        self
    }

    pub fn with_services(mut self, services: RuntimeServices) -> Self {
        self.inner = self.inner.with_services(services);
        self
    }

    pub fn with_event_stream(mut self, events: AgentEventStream) -> Self {
        self.inner = self.inner.with_event_stream(events);
        self
    }

    pub fn with_event_source(mut self, source: AgentEventSource) -> Self {
        self.inner = self.inner.with_event_source(source);
        self
    }

    pub fn with_tool_registry(mut self, registry: Arc<dyn RuntimeToolRegistry>) -> Self {
        self.inner = self.inner.with_tool_registry(registry);
        self
    }

    pub fn with_harness_registry(mut self, registry: Arc<HarnessRegistry>) -> Self {
        self.inner = self.inner.with_harness_registry(registry);
        self
    }

    pub fn with_hook_registry(mut self, registry: RuntimeHookRegistry) -> Self {
        self.inner = self.inner.with_hook_registry(registry);
        self
    }

    pub fn with_agent_registry(mut self, registry: Arc<dyn RuntimeAgentRegistry>) -> Self {
        self.inner = self.inner.with_agent_registry(registry);
        self
    }

    pub fn build(self) -> Result<AgentRuntime, RuntimeBuildError> {
        self.inner.build().map(|inner| AgentRuntime { inner })
    }
}

impl AgentRuntime {
    pub fn subscribe_events(&self) -> Result<AgentEventReceiver, RuntimeError> {
        self.inner.subscribe_events()
    }

    pub fn subscribe_session_events(
        &self,
        session_id: &str,
    ) -> Result<AgentSessionEventReceiver, RuntimeError> {
        self.inner.subscribe_session_events(session_id)
    }

    pub fn pending_permission_requests(&self) -> Result<Vec<PermissionRequest>, RuntimeError> {
        self.inner.pending_permission_requests()
    }

    pub fn permission_request_dialog_turn_id(
        &self,
        request_id: &str,
    ) -> Result<Option<String>, RuntimeError> {
        self.inner.permission_request_dialog_turn_id(request_id)
    }

    pub fn subscribe_permission_requests(
        &self,
    ) -> Result<PermissionRequestEventReceiver, RuntimeError> {
        self.inner.subscribe_permission_requests()
    }

    pub async fn respond_permission(
        &self,
        request_id: &str,
        reply: PermissionReply,
    ) -> Result<(), RuntimeError> {
        self.inner
            .respond_permission(request_id, reply, PermissionReplySource::User)
            .await
    }

    pub async fn respond_permission_with_source(
        &self,
        request_id: &str,
        reply: PermissionReply,
        source: PermissionReplySource,
    ) -> Result<(), RuntimeError> {
        self.inner
            .respond_permission(request_id, reply, source)
            .await
    }

    pub async fn respond_permission_batch(
        &self,
        request_id: &str,
        reply: PermissionReply,
    ) -> Result<Vec<String>, RuntimeError> {
        self.inner
            .respond_permission_batch(request_id, reply, PermissionReplySource::User)
            .await
    }

    pub async fn list_project_permission_grants(
        &self,
        project_id: &str,
    ) -> Result<Vec<PermissionGrant>, RuntimeError> {
        self.inner.list_project_permission_grants(project_id).await
    }

    pub async fn remove_project_permission_grant(
        &self,
        key: PermissionGrantKey,
    ) -> Result<bool, RuntimeError> {
        self.inner.remove_project_permission_grant(key).await
    }

    pub async fn clear_project_permission_grants(
        &self,
        project_id: &str,
    ) -> Result<usize, RuntimeError> {
        self.inner.clear_project_permission_grants(project_id).await
    }

    pub async fn list_project_permission_audit(
        &self,
        project_id: &str,
    ) -> Result<Vec<PermissionAuditRecord>, RuntimeError> {
        self.inner.list_project_permission_audit(project_id).await
    }

    pub fn services(&self) -> Option<&RuntimeServices> {
        self.inner.services()
    }

    pub fn registered_tool_names(&self) -> Vec<String> {
        self.inner.registered_tool_names()
    }

    pub fn harness_provider_ids(&self) -> Vec<&str> {
        self.inner.harness_provider_ids()
    }

    pub fn hook_registry(&self) -> &RuntimeHookRegistry {
        self.inner.hook_registry()
    }

    pub fn registered_agent_ids(&self, query: RuntimeAgentRegistryQuery<'_>) -> Vec<String> {
        self.inner.registered_agent_ids(query)
    }

    pub async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> Result<AgentSessionCreateResult, RuntimeError> {
        self.inner.create_session(request).await
    }

    pub async fn create_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> Result<AgentSessionCreateResult, RuntimeError> {
        self.inner.create_session_with_id(session_id, request).await
    }

    /// Creates one connection-scoped Session through the same Runtime owners.
    /// It is intentionally separate from durable Session creation so process
    /// adapters cannot silently weaken persistence semantics.
    pub async fn create_transient_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> Result<AgentSessionCreateResult, RuntimeError> {
        self.inner
            .create_transient_session_with_id(session_id, request)
            .await
    }

    pub async fn discard_transient_session(
        &self,
        request: AgentTransientSessionDiscardRequest,
    ) -> Result<bool, RuntimeError> {
        self.inner.discard_transient_session(request).await
    }

    pub async fn list_sessions(
        &self,
        request: AgentSessionListRequest,
    ) -> Result<Vec<AgentSessionSummary>, RuntimeError> {
        self.inner.list_sessions(request).await
    }

    pub async fn delete_session(
        &self,
        request: AgentSessionDeleteRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.delete_session(request).await
    }

    pub async fn rename_session(
        &self,
        request: AgentSessionRenameRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.rename_session(request).await
    }

    pub async fn archive_session(
        &self,
        request: AgentSessionArchiveRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.archive_session(request).await
    }

    pub async fn set_session_archived(
        &self,
        request: AgentSessionArchiveStateRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.set_session_archived(request).await
    }

    pub async fn record_completed_local_command_turn(
        &self,
        request: AgentLocalCommandTurnRecordRequest,
    ) -> Result<(), RuntimeError> {
        self.inner
            .record_completed_local_command_turn(request)
            .await
    }

    pub async fn update_session_model(
        &self,
        request: AgentSessionModelUpdateRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.update_session_model(request).await
    }

    pub async fn update_session_mode(
        &self,
        request: AgentSessionModeUpdateRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.update_session_mode(request).await
    }

    pub async fn fork_session(
        &self,
        request: AgentSessionForkRequest,
    ) -> Result<AgentSessionForkResult, RuntimeError> {
        self.inner.fork_session(request).await
    }

    pub async fn fork_session_at_turn(
        &self,
        request: AgentSessionForkAtTurnRequest,
    ) -> Result<AgentSessionForkResult, RuntimeError> {
        self.inner.fork_session_at_turn(request).await
    }

    pub async fn generate_session_usage(
        &self,
        request: AgentSessionUsageRequest,
    ) -> Result<SessionUsageReport, RuntimeError> {
        self.inner.generate_session_usage(request).await
    }

    pub async fn wait_for_turn_settlement(
        &self,
        request: AgentTurnSettlementRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.wait_for_turn_settlement(request).await
    }

    pub async fn restore_session(
        &self,
        request: AgentSessionRestoreRequest,
    ) -> Result<AgentSessionRestoreResult, RuntimeError> {
        self.inner.restore_session(request).await
    }

    pub async fn read_session_transcript(
        &self,
        request: SessionTranscriptRequest,
    ) -> Result<SessionTranscript, RuntimeError> {
        self.inner.read_session_transcript(request).await
    }

    pub async fn resolve_session_workspace_binding(
        &self,
        request: AgentSessionWorkspaceRequest,
    ) -> Result<Option<AgentSessionWorkspaceBinding>, RuntimeError> {
        self.inner.resolve_session_workspace_binding(request).await
    }

    pub async fn submit_turn(
        &self,
        request: AgentSubmissionRequest,
    ) -> Result<AgentSubmissionResult, RuntimeError> {
        self.inner.submit_turn(request).await
    }

    pub async fn submit_dialog_turn(
        &self,
        request: AgentDialogTurnRequest,
    ) -> Result<DialogSubmitOutcome, RuntimeError> {
        self.inner.submit_dialog_turn(request).await
    }

    pub async fn deliver_background_result(
        &self,
        request: AgentBackgroundResultRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.deliver_background_result(request).await
    }

    pub async fn deliver_thread_goal(
        &self,
        request: AgentThreadGoalDeliveryRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.deliver_thread_goal(request).await
    }

    pub async fn get_thread_goal(
        &self,
        request: AgentThreadGoalGetRequest,
    ) -> Result<Option<ThreadGoal>, RuntimeError> {
        self.inner.get_thread_goal(request).await
    }

    pub async fn create_thread_goal(
        &self,
        request: AgentThreadGoalCreateRequest,
    ) -> Result<ThreadGoal, RuntimeError> {
        self.inner.create_thread_goal(request).await
    }

    pub async fn update_thread_goal_status(
        &self,
        request: AgentThreadGoalUpdateStatusRequest,
    ) -> Result<ThreadGoal, RuntimeError> {
        self.inner.update_thread_goal_status(request).await
    }

    pub async fn resolve_session_agent_type(
        &self,
        session_id: &str,
    ) -> Result<Option<String>, RuntimeError> {
        self.inner.resolve_session_agent_type(session_id).await
    }

    pub async fn cancel_turn(
        &self,
        request: AgentTurnCancellationRequest,
    ) -> Result<AgentTurnCancellationResult, RuntimeError> {
        self.inner.cancel_turn(request).await
    }

    pub async fn submit_user_answers(
        &self,
        request: AgentUserAnswersRequest,
    ) -> Result<(), RuntimeError> {
        self.inner.submit_user_answers(request).await
    }

    pub async fn publish_event(&self, event: RuntimeEventEnvelope) -> Result<(), RuntimeError> {
        self.inner.publish_event(event).await
    }

    pub async fn run(&self, request: AgentRunRequest) -> Result<AgentRunHandle, RuntimeError> {
        self.inner.run(request).await
    }
}
