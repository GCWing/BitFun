use crate::{PortError, PortErrorKind, PortResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginRuntimeUnavailableReason {
    NotBuilt,
    UnsupportedProfile,
    DisabledByPolicy,
    HostUnavailable,
}

impl PluginRuntimeUnavailableReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotBuilt => "not_built",
            Self::UnsupportedProfile => "unsupported_profile",
            Self::DisabledByPolicy => "disabled_by_policy",
            Self::HostUnavailable => "host_unavailable",
        }
    }
}

impl std::fmt::Display for PluginRuntimeUnavailableReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
#[non_exhaustive]
pub enum ExtensionCapabilityAvailability {
    Disabled {
        reason: PluginRuntimeUnavailableReason,
    },
    ProjectionOnly {
        reason: PluginRuntimeUnavailableReason,
    },
    Available,
    Unavailable {
        reason: PluginRuntimeUnavailableReason,
    },
}

impl ExtensionCapabilityAvailability {
    pub const fn disabled(reason: PluginRuntimeUnavailableReason) -> Self {
        Self::Disabled { reason }
    }

    pub const fn projection_only(reason: PluginRuntimeUnavailableReason) -> Self {
        Self::ProjectionOnly { reason }
    }

    pub const fn is_executable(self) -> bool {
        matches!(self, Self::Available)
    }
}

pub type PluginRuntimeAvailability = ExtensionCapabilityAvailability;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginSourceKind {
    LocalPath,
    OpenCodeCompatible,
    RemoteRegistry,
    BitFunNative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginTrustLevel {
    Unknown,
    Trusted,
    Denied,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSourceRef {
    pub plugin_id: String,
    pub source_kind: PluginSourceKind,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub content_hash: String,
    pub trust_level: PluginTrustLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<PluginManifestRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifestRef {
    pub manifest_id: String,
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginConfigValidationStatus {
    Valid,
    Warning,
    Invalid,
    NotValidated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfigValidationIssue {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfigValidationState {
    pub status: PluginConfigValidationStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<PluginConfigValidationIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginStatusKind {
    Enabled,
    ProjectionOnly,
    Disabled,
    InvalidConfig,
    TrustRequired,
    Quarantined,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusSnapshot {
    pub source: PluginSourceRef,
    pub status: PluginStatusKind,
    pub availability: PluginRuntimeAvailability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_validation: Option<PluginConfigValidationState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine: Option<PluginQuarantineState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostic_ids: Vec<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginOwnerKind {
    ProductFeature,
    ExtensionContract,
    AssemblyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginOwnerRef {
    pub kind: PluginOwnerKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapabilityRef {
    pub capability_id: String,
    pub owner: PluginOwnerRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginTargetRef {
    pub target_kind: String,
    pub target_id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<PluginArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAuditRef {
    pub correlation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginArtifactRef {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginDataClassification {
    Public,
    Workspace,
    Sensitive,
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginPayloadRedaction {
    None,
    Partial,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPayloadRef {
    pub payload_id: String,
    pub schema_version: String,
    pub data_classification: PluginDataClassification,
    pub redaction: PluginPayloadRedaction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PermissionPromptEffectKind {
    Unsupported,
    ProviderCandidate,
    Descriptor,
    EvidenceCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginRollbackMode {
    RemoveContribution,
    RestorePrevious,
    DisablePlugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRollbackPolicy {
    pub mode: PluginRollbackMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PermissionPromptDenyState {
    NoStateChange,
    CandidateDiscarded,
    TemporarilyUnavailable,
    PolicyDenied,
    Quarantined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPromptDescriptor {
    pub descriptor_version: u16,
    pub prompt_id: String,
    pub plugin: PluginSourceRef,
    pub requested_capability: PluginCapabilityRef,
    pub requested_effect: PermissionPromptEffectKind,
    pub target: PluginTargetRef,
    pub risk_level: PluginRiskLevel,
    pub owner: PluginOwnerRef,
    pub rollback: PluginRollbackPolicy,
    pub deny_state: PermissionPromptDenyState,
    pub audit: PluginAuditRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "status"
)]
#[non_exhaustive]
pub enum PluginPermissionGate {
    PolicyAllowed {
        audit: PluginAuditRef,
    },
    PermissionRequired {
        prompt: PermissionPromptDescriptor,
    },
    PolicyDenied {
        deny_state: PermissionPromptDenyState,
        audit: PluginAuditRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[non_exhaustive]
pub enum PluginEffectCandidatePayload {
    Unsupported {
        capability: String,
        reason: String,
    },
    ProviderCandidate {
        provider_id: String,
        tool_contract_id: String,
    },
    Descriptor {
        descriptor_id: String,
        descriptor_version: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload_ref: Option<PluginPayloadRef>,
    },
    EvidenceCandidate {
        payload_ref: PluginPayloadRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEffectCandidate {
    pub effect_id: String,
    pub schema_version: String,
    pub declared_capability: PluginCapabilityRef,
    pub target_ref: PluginTargetRef,
    pub data_classification: PluginDataClassification,
    pub risk_level: PluginRiskLevel,
    pub permission: PluginPermissionGate,
    pub source_ref: PluginSourceRef,
    pub payload: PluginEffectCandidatePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[non_exhaustive]
pub enum PluginDiagnosticDetail {
    Manifest {
        manifest: PluginManifestRef,
    },
    ConfigValidation {
        manifest: PluginManifestRef,
        validation: PluginConfigValidationState,
    },
    Trust {
        trust_level: PluginTrustLevel,
    },
    Deadline {
        deadline_ms: u64,
        elapsed_ms: u64,
    },
    Quarantine {
        scope: PluginQuarantineScope,
        reason: PluginQuarantineReason,
    },
    HostLifecycle {
        phase: PluginHostLifecyclePhase,
    },
    Adapter {
        adapter_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDiagnostic {
    pub diagnostic_id: String,
    pub severity: PluginDiagnosticSeverity,
    pub source: PluginSourceRef,
    pub code: String,
    pub message: String,
    pub detail: PluginDiagnosticDetail,
    pub audit: PluginAuditRef,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery_actions: Vec<PluginRecoveryAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[non_exhaustive]
pub enum PluginQuarantineScope {
    Plugin {
        plugin_id: String,
    },
    Capability {
        plugin_id: String,
        capability_id: String,
    },
    Target {
        plugin_id: String,
        target_kind: String,
        target_id: String,
    },
    ProjectPlugin {
        project_domain_id: String,
        plugin_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginQuarantineReason {
    HostFailure,
    PolicyViolation,
    TrustChanged,
    DeadlineExceeded,
    AdapterFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginQuarantineClearCondition {
    UserAction,
    TrustEpochAdvanced,
    PluginUpdated,
    HostRestarted,
    PolicyUpdated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginRecoveryActionKind {
    Retry,
    Disable,
    Retrust,
    OpenLog,
    ClearQuarantine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecoveryAction {
    pub action_id: String,
    pub kind: PluginRecoveryActionKind,
    pub target: PluginTargetRef,
    pub audit: PluginAuditRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<PluginArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecoveryActionRequest {
    pub request_id: String,
    pub source: PluginSourceRef,
    pub action_id: String,
    pub quarantine_id: String,
    pub scope: PluginQuarantineScope,
    pub requested_by: PluginOwnerRef,
    pub authorization: PluginAuditRef,
    pub epochs: PluginRuntimeEpochs,
    pub idempotency_key: String,
    pub requested_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginRecoveryActionStatus {
    Accepted,
    Completed,
    Rejected,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecoveryActionResult {
    pub request_id: String,
    pub action_id: String,
    pub status: PluginRecoveryActionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<PluginDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine: Option<PluginQuarantineState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginQuarantineState {
    pub schema_version: u16,
    pub quarantine_id: String,
    pub scope: PluginQuarantineScope,
    pub reason: PluginQuarantineReason,
    pub source: PluginSourceRef,
    pub audit: PluginAuditRef,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_ref: Option<PluginArtifactRef>,
    pub clears_when: Vec<PluginQuarantineClearCondition>,
    pub recovery_actions: Vec<PluginRecoveryAction>,
    pub diagnostic_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginHostLifecyclePhase {
    Init,
    Manifest,
    Dispatch,
    Deadline,
    Dispose,
    FailureQuarantine,
    Diagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginHostLifecycleEvent {
    pub event_id: String,
    pub phase: PluginHostLifecyclePhase,
    pub project_domain_id: String,
    pub source: PluginSourceRef,
    pub observed_at_ms: u64,
    pub project_epoch: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeEpochs {
    pub project_epoch: u64,
    pub trust_epoch: u64,
    pub policy_epoch: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_registry_epoch: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeReadRequest {
    pub request_id: String,
    pub project_domain_id: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugin_ids: Vec<String>,
    pub include_config_validation: bool,
    pub epochs: PluginRuntimeEpochs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeReadResponse {
    pub request_id: String,
    pub project_domain_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<PluginSourceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugin_statuses: Vec<PluginStatusSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<PluginDiagnostic>,
    pub observed_epochs: PluginRuntimeEpochs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDispatchEnvelope {
    pub envelope_version: u16,
    pub event_id: String,
    pub event_type: String,
    pub event_version: String,
    pub project_domain_id: String,
    pub workspace_id: String,
    pub extension_point_id: String,
    pub source: PluginSourceRef,
    pub declared_capability: PluginCapabilityRef,
    pub correlation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    pub idempotency_key: String,
    pub deadline_ms: u64,
    pub epochs: PluginRuntimeEpochs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_ref: Option<PluginPayloadRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginResponseEnvelope {
    pub envelope_version: u16,
    pub request_event_id: String,
    pub project_domain_id: String,
    pub adapter_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    pub completed_at_ms: u64,
    pub effects: Vec<PluginEffectCandidate>,
    pub diagnostics: Vec<PluginDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine: Option<PluginQuarantineState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugin_statuses: Vec<PluginStatusSnapshot>,
    pub observed_epochs: PluginRuntimeEpochs,
}

#[async_trait::async_trait]
pub trait PluginRuntimeClient: Send + Sync {
    fn availability(&self) -> PluginRuntimeAvailability;

    async fn read_plugins(
        &self,
        _request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        Err(PortError::new(
            PortErrorKind::NotAvailable,
            "plugin runtime read model is not available",
        ))
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope>;

    async fn execute_recovery_action(
        &self,
        _request: PluginRecoveryActionRequest,
    ) -> PortResult<PluginRecoveryActionResult> {
        Err(PortError::new(
            PortErrorKind::NotAvailable,
            "plugin runtime recovery actions are not available",
        ))
    }
}

#[derive(Debug, Clone)]
pub struct DisabledPluginRuntimeClient {
    reason: PluginRuntimeUnavailableReason,
}

impl DisabledPluginRuntimeClient {
    pub const fn new(reason: PluginRuntimeUnavailableReason) -> Self {
        Self { reason }
    }

    fn not_available(&self) -> PortError {
        PortError::new(
            PortErrorKind::NotAvailable,
            format!("plugin runtime is disabled: {}", self.reason),
        )
    }
}

impl Default for DisabledPluginRuntimeClient {
    fn default() -> Self {
        Self::new(PluginRuntimeUnavailableReason::NotBuilt)
    }
}

#[async_trait::async_trait]
impl PluginRuntimeClient for DisabledPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Disabled {
            reason: self.reason,
        }
    }

    async fn read_plugins(
        &self,
        _request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        Err(self.not_available())
    }

    async fn dispatch(
        &self,
        _envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        Err(self.not_available())
    }
}

#[derive(Debug, Clone)]
pub struct ProjectionOnlyPluginRuntimeClient {
    reason: PluginRuntimeUnavailableReason,
}

impl ProjectionOnlyPluginRuntimeClient {
    pub const fn new(reason: PluginRuntimeUnavailableReason) -> Self {
        Self { reason }
    }

    fn not_available(&self) -> PortError {
        PortError::new(
            PortErrorKind::NotAvailable,
            format!("plugin runtime is projection-only: {}", self.reason),
        )
    }
}

#[async_trait::async_trait]
impl PluginRuntimeClient for ProjectionOnlyPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::ProjectionOnly {
            reason: self.reason,
        }
    }

    async fn read_plugins(
        &self,
        _request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        Err(self.not_available())
    }

    async fn dispatch(
        &self,
        _envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        Err(self.not_available())
    }
}

#[derive(Clone)]
#[non_exhaustive]
pub enum PluginRuntimeBinding {
    Disabled(DisabledPluginRuntimeClient),
    ProjectionOnly(ProjectionOnlyPluginRuntimeClient),
    Client(Arc<dyn PluginRuntimeClient>),
}

impl PluginRuntimeBinding {
    pub const fn disabled(reason: PluginRuntimeUnavailableReason) -> Self {
        Self::Disabled(DisabledPluginRuntimeClient::new(reason))
    }

    pub const fn projection_only(reason: PluginRuntimeUnavailableReason) -> Self {
        Self::ProjectionOnly(ProjectionOnlyPluginRuntimeClient::new(reason))
    }

    pub fn client(client: Arc<dyn PluginRuntimeClient>) -> Self {
        Self::Client(client)
    }

    pub fn availability(&self) -> PluginRuntimeAvailability {
        match self {
            Self::Disabled(client) => client.availability(),
            Self::ProjectionOnly(client) => client.availability(),
            Self::Client(client) => client.availability(),
        }
    }

    pub fn as_client(&self) -> Arc<dyn PluginRuntimeClient> {
        match self {
            Self::Disabled(client) => Arc::new(client.clone()),
            Self::ProjectionOnly(client) => Arc::new(client.clone()),
            Self::Client(client) => Arc::clone(client),
        }
    }
}

impl Default for PluginRuntimeBinding {
    fn default() -> Self {
        Self::disabled(PluginRuntimeUnavailableReason::NotBuilt)
    }
}

impl std::fmt::Debug for PluginRuntimeBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRuntimeBinding")
            .field("availability", &self.availability())
            .finish()
    }
}
