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
    ProviderCandidate,
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
    ProviderCandidate {
        provider_id: String,
        tool_contract_id: String,
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
        #[serde(default)]
        project_domain_id: String,
        #[serde(default)]
        workspace_id: String,
        plugin_id: String,
    },
    Capability {
        #[serde(default)]
        project_domain_id: String,
        #[serde(default)]
        workspace_id: String,
        plugin_id: String,
        capability_id: String,
    },
    Target {
        #[serde(default)]
        project_domain_id: String,
        #[serde(default)]
        workspace_id: String,
        plugin_id: String,
        target_kind: String,
        target_id: String,
    },
    ProjectPlugin {
        #[serde(default)]
        project_domain_id: String,
        #[serde(default)]
        workspace_id: String,
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
    HostRestarted,
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
    pub workspace_id: String,
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
    pub workspace_id: String,
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
}

pub fn validate_plugin_runtime_read_response(
    request: &PluginRuntimeReadRequest,
    response: &PluginRuntimeReadResponse,
) -> PortResult<()> {
    if response.request_id != request.request_id {
        return Err(invalid_plugin_runtime_response(
            "read response request_id mismatch",
        ));
    }
    if response.project_domain_id != request.project_domain_id {
        return Err(invalid_plugin_runtime_response(
            "read response project_domain_id mismatch",
        ));
    }
    if response.workspace_id != request.workspace_id {
        return Err(invalid_plugin_runtime_response(
            "read response workspace_id mismatch",
        ));
    }
    if response.observed_epochs != request.epochs {
        return Err(invalid_plugin_runtime_response(
            "read response epoch mismatch",
        ));
    }

    for status in &response.plugin_statuses {
        validate_read_status_snapshot(request, status)?;
    }
    for source in &response.sources {
        if !request.plugin_ids.is_empty() && !request.plugin_ids.contains(&source.plugin_id) {
            return Err(invalid_plugin_runtime_response(
                "read response source plugin_id outside request",
            ));
        }
    }
    for diagnostic in &response.diagnostics {
        validate_read_diagnostic(request, response, diagnostic)?;
    }

    Ok(())
}

pub fn validate_plugin_dispatch_response(
    envelope: &PluginDispatchEnvelope,
    response: &PluginResponseEnvelope,
    expected_adapter_id: Option<&str>,
) -> PortResult<()> {
    validate_plugin_dispatch_response_contract(envelope, response, expected_adapter_id)
        .map_err(invalid_plugin_runtime_response)
}

fn validate_read_status_snapshot(
    request: &PluginRuntimeReadRequest,
    status: &PluginStatusSnapshot,
) -> PortResult<()> {
    if !request.plugin_ids.is_empty() && !request.plugin_ids.contains(&status.source.plugin_id) {
        return Err(invalid_plugin_runtime_response(
            "read response status plugin_id outside request",
        ));
    }
    if let Some(quarantine) = &status.quarantine {
        if status.status != PluginStatusKind::Quarantined {
            return Err(invalid_plugin_runtime_response(
                "read response quarantine status mismatch",
            ));
        }
        if status.availability.is_executable() {
            return Err(invalid_plugin_runtime_response(
                "read response quarantined plugin must not be executable",
            ));
        }
        validate_quarantine_against_read_request(request, &status.source, quarantine)?;
    }
    Ok(())
}

fn validate_read_diagnostic(
    request: &PluginRuntimeReadRequest,
    response: &PluginRuntimeReadResponse,
    diagnostic: &PluginDiagnostic,
) -> PortResult<()> {
    if !request.plugin_ids.is_empty() && !request.plugin_ids.contains(&diagnostic.source.plugin_id)
    {
        return Err(invalid_plugin_runtime_response(
            "read response diagnostic plugin_id outside request",
        ));
    }
    let diagnostic_source_is_projected = response
        .sources
        .iter()
        .any(|source| source == &diagnostic.source)
        || response
            .plugin_statuses
            .iter()
            .any(|status| status.source == diagnostic.source);
    if !diagnostic_source_is_projected {
        return Err(invalid_plugin_runtime_response(
            "read response diagnostic source is not projected",
        ));
    }
    Ok(())
}

fn validate_plugin_dispatch_response_contract(
    envelope: &PluginDispatchEnvelope,
    response: &PluginResponseEnvelope,
    expected_adapter_id: Option<&str>,
) -> Result<(), String> {
    if response.envelope_version != envelope.envelope_version {
        return Err("envelope_version mismatch".to_string());
    }
    if response.request_event_id != envelope.event_id {
        return Err("request_event_id mismatch".to_string());
    }
    if response.project_domain_id != envelope.project_domain_id {
        return Err("project_domain_id mismatch".to_string());
    }
    if response.workspace_id != envelope.workspace_id {
        return Err("workspace_id mismatch".to_string());
    }
    if response.adapter_id.is_empty() {
        return Err("adapter_id is empty".to_string());
    }
    if let Some(expected_adapter_id) = expected_adapter_id {
        if response.adapter_id != expected_adapter_id {
            return Err("adapter_id mismatch".to_string());
        }
    }
    if response.plugin_id.as_deref() != Some(envelope.source.plugin_id.as_str()) {
        return Err("plugin_id mismatch".to_string());
    }
    if response.observed_epochs != envelope.epochs {
        return Err("observed epoch mismatch".to_string());
    }
    let has_nested_quarantine = response
        .plugin_statuses
        .iter()
        .any(|status| status.quarantine.is_some());
    if (response.quarantine.is_some() || has_nested_quarantine) && !response.effects.is_empty() {
        return Err("quarantine response must not carry success effects".to_string());
    }

    for (index, effect) in response.effects.iter().enumerate() {
        validate_effect_candidate(envelope, effect)
            .map_err(|message| format!("effect {index}: {message}"))?;
    }
    for diagnostic in &response.diagnostics {
        if diagnostic.source != envelope.source {
            return Err(format!(
                "diagnostic {} source mismatch",
                diagnostic.diagnostic_id
            ));
        }
        validate_audit_ref(envelope, &diagnostic.audit)
            .map_err(|message| format!("diagnostic {}: {message}", diagnostic.diagnostic_id))?;
    }
    if let Some(quarantine) = &response.quarantine {
        validate_quarantine_against_envelope(envelope, quarantine)?;
    }
    for status in &response.plugin_statuses {
        if status.source != envelope.source {
            return Err(format!(
                "status source mismatch for plugin {}",
                status.source.plugin_id
            ));
        }
        if let Some(quarantine) = &status.quarantine {
            validate_quarantine_against_envelope(envelope, quarantine)?;
        }
    }

    Ok(())
}

fn validate_effect_candidate(
    envelope: &PluginDispatchEnvelope,
    effect: &PluginEffectCandidate,
) -> Result<(), String> {
    if effect.source_ref != envelope.source {
        return Err("source_ref mismatch".to_string());
    }
    if effect.declared_capability != envelope.declared_capability {
        return Err("declared_capability mismatch".to_string());
    }
    match &effect.permission {
        PluginPermissionGate::PolicyAllowed { .. } => {
            Err("plugin host responses must not carry final policy_allowed decisions".to_string())
        }
        PluginPermissionGate::PermissionRequired { prompt } => {
            validate_permission_prompt(envelope, effect, prompt)
        }
        PluginPermissionGate::PolicyDenied { .. } => {
            Err("plugin host responses must not carry final policy_denied decisions".to_string())
        }
    }
}

fn validate_permission_prompt(
    envelope: &PluginDispatchEnvelope,
    effect: &PluginEffectCandidate,
    prompt: &PermissionPromptDescriptor,
) -> Result<(), String> {
    if prompt.plugin != envelope.source {
        return Err("permission prompt plugin mismatch".to_string());
    }
    if prompt.requested_capability != envelope.declared_capability {
        return Err("permission prompt capability mismatch".to_string());
    }
    if prompt.requested_effect != permission_effect_kind_for_payload(&effect.payload) {
        return Err("permission prompt requested_effect mismatch".to_string());
    }
    if prompt.target != effect.target_ref {
        return Err("permission prompt target mismatch".to_string());
    }
    if prompt.risk_level != effect.risk_level {
        return Err("permission prompt risk_level mismatch".to_string());
    }
    if prompt.owner != envelope.declared_capability.owner {
        return Err("permission prompt owner mismatch".to_string());
    }
    if prompt.rollback.reason_ref.as_deref() != Some(audit_reason_ref(envelope).as_str()) {
        return Err("permission prompt rollback reason_ref mismatch".to_string());
    }
    if prompt.deny_state != PermissionPromptDenyState::CandidateDiscarded {
        return Err("permission prompt deny_state mismatch".to_string());
    }
    validate_audit_ref(envelope, &prompt.audit)
}

fn validate_quarantine_against_read_request(
    request: &PluginRuntimeReadRequest,
    source: &PluginSourceRef,
    quarantine: &PluginQuarantineState,
) -> PortResult<()> {
    if quarantine.source != *source {
        return Err(invalid_plugin_runtime_response(
            "read response quarantine source mismatch",
        ));
    }
    validate_quarantine_scope(
        &request.project_domain_id,
        &request.workspace_id,
        source,
        &quarantine.scope,
    )
    .map_err(invalid_plugin_runtime_response)?;
    validate_quarantine_clear_condition(quarantine).map_err(invalid_plugin_runtime_response)?;
    Ok(())
}

fn validate_quarantine_against_envelope(
    envelope: &PluginDispatchEnvelope,
    quarantine: &PluginQuarantineState,
) -> Result<(), String> {
    if quarantine.source != envelope.source {
        return Err("quarantine source mismatch".to_string());
    }
    validate_quarantine_scope(
        &envelope.project_domain_id,
        &envelope.workspace_id,
        &envelope.source,
        &quarantine.scope,
    )?;
    validate_audit_ref(envelope, &quarantine.audit)
        .map_err(|message| format!("quarantine {message}"))?;
    validate_quarantine_clear_condition(quarantine)
        .map_err(|message| format!("quarantine {message}"))?;
    Ok(())
}

fn validate_quarantine_clear_condition(quarantine: &PluginQuarantineState) -> Result<(), String> {
    if quarantine.clears_when.len() != 1
        || quarantine.clears_when[0] != PluginQuarantineClearCondition::HostRestarted
    {
        return Err(
            "clears_when must contain only host_restarted for the P0-B quarantine contract"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_quarantine_scope(
    project_domain_id: &str,
    workspace_id: &str,
    source: &PluginSourceRef,
    scope: &PluginQuarantineScope,
) -> Result<(), String> {
    match scope {
        PluginQuarantineScope::Plugin {
            project_domain_id: scope_project_domain_id,
            workspace_id: scope_workspace_id,
            plugin_id,
        } if scope_project_domain_id == project_domain_id
            && scope_workspace_id == workspace_id
            && plugin_id == &source.plugin_id =>
        {
            Ok(())
        }
        PluginQuarantineScope::Capability {
            project_domain_id: scope_project_domain_id,
            workspace_id: scope_workspace_id,
            plugin_id,
            capability_id,
        } if scope_project_domain_id == project_domain_id
            && scope_workspace_id == workspace_id
            && plugin_id == &source.plugin_id
            && !capability_id.is_empty() =>
        {
            Ok(())
        }
        PluginQuarantineScope::Target {
            project_domain_id: scope_project_domain_id,
            workspace_id: scope_workspace_id,
            plugin_id,
            target_kind,
            target_id,
        } if scope_project_domain_id == project_domain_id
            && scope_workspace_id == workspace_id
            && plugin_id == &source.plugin_id
            && !target_kind.is_empty()
            && !target_id.is_empty() =>
        {
            Ok(())
        }
        PluginQuarantineScope::ProjectPlugin {
            project_domain_id: scope_project_domain_id,
            workspace_id: scope_workspace_id,
            plugin_id,
        } if scope_project_domain_id == project_domain_id
            && scope_workspace_id == workspace_id
            && plugin_id == &source.plugin_id =>
        {
            Ok(())
        }
        _ => Err("quarantine scope mismatch".to_string()),
    }
}

fn validate_audit_ref(
    envelope: &PluginDispatchEnvelope,
    audit: &PluginAuditRef,
) -> Result<(), String> {
    if audit.correlation_id != envelope.correlation_id {
        return Err("audit correlation_id mismatch".to_string());
    }
    if audit.event_id.as_deref() != Some(envelope.event_id.as_str()) {
        return Err("audit event_id mismatch".to_string());
    }
    Ok(())
}

fn permission_effect_kind_for_payload(
    payload: &PluginEffectCandidatePayload,
) -> PermissionPromptEffectKind {
    match payload {
        PluginEffectCandidatePayload::ProviderCandidate { .. } => {
            PermissionPromptEffectKind::ProviderCandidate
        }
    }
}

fn audit_reason_ref(envelope: &PluginDispatchEnvelope) -> String {
    format!("audit:{}", envelope.event_id)
}

fn invalid_plugin_runtime_response(message: impl Into<String>) -> PortError {
    PortError::new(
        PortErrorKind::Backend,
        format!(
            "plugin runtime returned an invalid response: {}",
            message.into()
        ),
    )
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
struct ContractCheckedPluginRuntimeClient {
    inner: Arc<dyn PluginRuntimeClient>,
}

#[async_trait::async_trait]
impl PluginRuntimeClient for ContractCheckedPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        self.inner.availability()
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        let response = self.inner.read_plugins(request.clone()).await?;
        validate_plugin_runtime_read_response(&request, &response)?;
        Ok(response)
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        let response = self.inner.dispatch(envelope.clone()).await?;
        validate_plugin_dispatch_response(&envelope, &response, None)?;
        Ok(response)
    }
}

#[derive(Clone)]
struct PluginRuntimeClientToken(());

#[derive(Clone)]
// The private token seals the executable variant so callers must use
// PluginRuntimeBinding::client(), which wraps clients with contract validation.
#[allow(private_interfaces)]
#[non_exhaustive]
pub enum PluginRuntimeBinding {
    Disabled(DisabledPluginRuntimeClient),
    ProjectionOnly(ProjectionOnlyPluginRuntimeClient),
    Client(PluginRuntimeClientToken, Arc<dyn PluginRuntimeClient>),
}

impl PluginRuntimeBinding {
    pub const fn disabled(reason: PluginRuntimeUnavailableReason) -> Self {
        Self::Disabled(DisabledPluginRuntimeClient::new(reason))
    }

    pub const fn projection_only(reason: PluginRuntimeUnavailableReason) -> Self {
        Self::ProjectionOnly(ProjectionOnlyPluginRuntimeClient::new(reason))
    }

    pub fn client(client: Arc<dyn PluginRuntimeClient>) -> Self {
        Self::Client(
            PluginRuntimeClientToken(()),
            Arc::new(ContractCheckedPluginRuntimeClient { inner: client }),
        )
    }

    pub fn is_client_binding(&self) -> bool {
        matches!(self, Self::Client(_, _))
    }

    pub fn availability(&self) -> PluginRuntimeAvailability {
        match self {
            Self::Disabled(client) => client.availability(),
            Self::ProjectionOnly(client) => client.availability(),
            Self::Client(_, client) => client.availability(),
        }
    }

    pub fn as_client(&self) -> Arc<dyn PluginRuntimeClient> {
        match self {
            Self::Disabled(client) => Arc::new(client.clone()),
            Self::ProjectionOnly(client) => Arc::new(client.clone()),
            Self::Client(_, client) => Arc::clone(client),
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
