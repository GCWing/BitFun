//! Ecosystem-neutral contracts for external AI application sources.
//!
//! Ecosystem adapters implement capability-specific provider traits. Product
//! surfaces and lifecycle coordination consume these types without branching on
//! a concrete ecosystem or carrying arbitrary extension payloads.

use crate::external_integration_policy::ExternalIntegrationPolicySnapshot;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

const MAX_ID_LENGTH: usize = 160;
const MAX_TOOL_NAME_LENGTH: usize = 64;
const MAX_TEXT_LENGTH: usize = 4096;

pub(crate) fn validate_id(
    value: &str,
    label: &'static str,
) -> Result<(), ExternalSourceContractError> {
    if value.is_empty()
        || value.len() > MAX_ID_LENGTH
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(ExternalSourceContractError::InvalidIdentifier(label));
    }
    Ok(())
}

fn validate_text(value: &str, label: &'static str) -> Result<(), ExternalSourceContractError> {
    if value.is_empty() || value.len() > MAX_TEXT_LENGTH || value.chars().any(char::is_control) {
        return Err(ExternalSourceContractError::InvalidText(label));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSourceContractError {
    InvalidIdentifier(&'static str),
    InvalidText(&'static str),
    InvalidPolicyDescriptor(&'static str),
    UnsupportedPolicySchemaMajor(u32),
}

impl fmt::Display for ExternalSourceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(label) => write!(formatter, "invalid {label} identifier"),
            Self::InvalidText(label) => write!(formatter, "invalid {label} text"),
            Self::InvalidPolicyDescriptor(reason) => {
                write!(
                    formatter,
                    "invalid external integration descriptor: {reason}"
                )
            }
            Self::UnsupportedPolicySchemaMajor(major) => {
                write!(
                    formatter,
                    "unsupported external integration policy schema major: {major}"
                )
            }
        }
    }
}

impl Error for ExternalSourceContractError {}

/// Stable product error codes shared by Desktop, Server, CLI and remote hosts.
/// User-facing copy is owned by each surface; `detail` is bounded diagnostic
/// context and must not be used for control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceOperationErrorCode {
    InvalidRequest,
    HostUnavailable,
    HostCapabilityUnavailable,
    TrustRequired,
    PolicyIncompatible,
    PolicyLimited,
    StaleRevision,
    Conflict,
    NotFound,
    Unavailable,
    RuntimeUnavailable,
    Unsupported,
    IncompatibleVersion,
    DependencyFailed,
    Timeout,
    Cancelled,
    Overloaded,
    ProcessLost,
    InvalidResponse,
    TemporarilyUnavailable,
    Internal,
}

impl ExternalSourceOperationErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::HostUnavailable => "host_unavailable",
            Self::HostCapabilityUnavailable => "host_capability_unavailable",
            Self::TrustRequired => "trust_required",
            Self::PolicyIncompatible => "policy_incompatible",
            Self::PolicyLimited => "policy_limited",
            Self::StaleRevision => "stale_revision",
            Self::Conflict => "conflict",
            Self::NotFound => "not_found",
            Self::Unavailable => "unavailable",
            Self::RuntimeUnavailable => "runtime_unavailable",
            Self::Unsupported => "unsupported",
            Self::IncompatibleVersion => "incompatible_version",
            Self::DependencyFailed => "dependency_failed",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Overloaded => "overloaded",
            Self::ProcessLost => "process_lost",
            Self::InvalidResponse => "invalid_response",
            Self::TemporarilyUnavailable => "temporarily_unavailable",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceOperationError {
    pub code: ExternalSourceOperationErrorCode,
    pub detail: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<crate::external_source_control::ExternalSourceOperationStage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery_actions: Vec<crate::external_source_control::ExternalSourceRecoveryActionV1>,
}

impl ExternalSourceOperationError {
    pub fn new(
        code: ExternalSourceOperationErrorCode,
        detail: impl Into<String>,
        retryable: bool,
    ) -> Self {
        let detail = bounded_error_detail(detail.into());
        Self {
            code,
            detail,
            retryable,
            correlation_id: None,
            causation_id: None,
            stage: None,
            recovery_actions: Vec::new(),
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = valid_error_id(correlation_id.into());
        self
    }

    pub fn with_causation_id(mut self, causation_id: impl Into<String>) -> Self {
        self.causation_id = valid_error_id(causation_id.into());
        self
    }

    pub fn with_stage(
        mut self,
        stage: crate::external_source_control::ExternalSourceOperationStage,
    ) -> Self {
        self.stage = Some(stage);
        self
    }

    pub fn with_recovery_action(
        mut self,
        action: crate::external_source_control::ExternalSourceRecoveryActionV1,
    ) -> Self {
        if self.recovery_actions.len() < 8 && !self.recovery_actions.contains(&action) {
            self.recovery_actions.push(action);
        }
        self
    }

    pub fn with_default_recovery_actions(mut self) -> Self {
        use crate::external_source_control::ExternalSourceRecoveryActionV1 as Recovery;

        let actions: &[Recovery] = match self.code {
            ExternalSourceOperationErrorCode::TrustRequired
            | ExternalSourceOperationErrorCode::PolicyLimited => &[Recovery::Review],
            ExternalSourceOperationErrorCode::StaleRevision
            | ExternalSourceOperationErrorCode::NotFound => &[Recovery::Refresh],
            ExternalSourceOperationErrorCode::Conflict => {
                &[Recovery::Refresh, Recovery::ResolveConflict]
            }
            ExternalSourceOperationErrorCode::RuntimeUnavailable
            | ExternalSourceOperationErrorCode::DependencyFailed => {
                &[Recovery::InstallRuntime, Recovery::Retry]
            }
            ExternalSourceOperationErrorCode::HostUnavailable
            | ExternalSourceOperationErrorCode::ProcessLost
            | ExternalSourceOperationErrorCode::InvalidResponse => {
                &[Recovery::ReconnectHost, Recovery::Retry]
            }
            ExternalSourceOperationErrorCode::Unavailable
            | ExternalSourceOperationErrorCode::Timeout
            | ExternalSourceOperationErrorCode::Overloaded
            | ExternalSourceOperationErrorCode::TemporarilyUnavailable
            | ExternalSourceOperationErrorCode::Internal => &[Recovery::Retry, Recovery::Refresh],
            ExternalSourceOperationErrorCode::InvalidRequest
            | ExternalSourceOperationErrorCode::HostCapabilityUnavailable
            | ExternalSourceOperationErrorCode::PolicyIncompatible
            | ExternalSourceOperationErrorCode::Unsupported
            | ExternalSourceOperationErrorCode::IncompatibleVersion
            | ExternalSourceOperationErrorCode::Cancelled => &[],
        };
        for action in actions {
            if self.recovery_actions.len() < 8 && !self.recovery_actions.contains(action) {
                self.recovery_actions.push(action.clone());
            }
        }
        self
    }

    /// Encode a typed failure while legacy internal call paths are migrated
    /// away from `Result<_, String>`. Decoding is exact JSON parsing; callers
    /// must never infer error categories from message text.
    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"code":"internal","detail":"External source operation failed","retryable":false}"#
                .to_string()
        })
    }

    pub fn decode(encoded: &str) -> Option<Self> {
        let mut decoded: Self = serde_json::from_str(encoded).ok()?;
        decoded.detail = bounded_error_detail(decoded.detail);
        decoded.correlation_id = decoded.correlation_id.and_then(valid_error_id);
        decoded.causation_id = decoded.causation_id.and_then(valid_error_id);
        decoded.recovery_actions.truncate(8);
        let mut unique = Vec::with_capacity(decoded.recovery_actions.len());
        for action in decoded.recovery_actions {
            if !unique.contains(&action) {
                unique.push(action);
            }
        }
        decoded.recovery_actions = unique;
        Some(decoded)
    }

    pub fn host_capability_unavailable(detail: impl Into<String>) -> Self {
        Self::new(
            ExternalSourceOperationErrorCode::HostCapabilityUnavailable,
            detail,
            false,
        )
    }

    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::new(
            ExternalSourceOperationErrorCode::InvalidRequest,
            detail,
            false,
        )
    }
}

fn valid_error_id(value: String) -> Option<String> {
    validate_id(&value, "external source operation reference")
        .is_ok()
        .then_some(value)
}

fn bounded_error_detail(value: String) -> String {
    let bounded = value
        .chars()
        .take(MAX_TEXT_LENGTH)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    if bounded.trim().is_empty() {
        "External source operation failed".to_string()
    } else {
        bounded
    }
}

impl fmt::Display for ExternalSourceOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl Error for ExternalSourceOperationError {}

pub type ExternalSourceOperationResult<T> = Result<T, ExternalSourceOperationError>;

macro_rules! open_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ExternalSourceContractError> {
                let value = value.into();
                validate_id(&value, $label)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

open_id!(EcosystemId, "ecosystem");
open_id!(ExecutionDomainId, "execution domain");
open_id!(ProviderId, "provider");
open_id!(SourceId, "source");
open_id!(CommandLocalId, "command");
open_id!(ToolTargetLocalId, "tool target");
open_id!(ToolExportLocalId, "tool export");
open_id!(McpServerLocalId, "MCP server");
open_id!(
    ExternalIntegrationCapabilityId,
    "external integration capability"
);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceKey {
    pub provider_id: ProviderId,
    pub source_id: SourceId,
}

impl SourceKey {
    pub fn new(
        provider_id: impl Into<String>,
        source_id: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        Ok(Self {
            provider_id: ProviderId::new(provider_id)?,
            source_id: SourceId::new(source_id)?,
        })
    }

    pub fn stable_key(&self) -> String {
        format!(
            "{}:{}{}:{}",
            self.provider_id.as_str().len(),
            self.provider_id,
            self.source_id.as_str().len(),
            self.source_id
        )
    }

    pub fn from_stable_key(value: &str) -> Option<Self> {
        let (provider, remainder) = take_length_prefixed(value)?;
        let (source, remainder) = take_length_prefixed(remainder)?;
        if !remainder.is_empty() {
            return None;
        }
        Self::new(provider, source).ok()
    }
}

fn take_length_prefixed(value: &str) -> Option<(&str, &str)> {
    let separator = value.find(':')?;
    let length = value[..separator].parse::<usize>().ok()?;
    let start = separator + 1;
    let end = start.checked_add(length)?;
    Some((value.get(start..end)?, value.get(end..)?))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceQualifiedCommandId {
    pub source: SourceKey,
    pub local_id: CommandLocalId,
}

impl SourceQualifiedCommandId {
    pub fn new(
        source: SourceKey,
        local_id: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        Ok(Self {
            source,
            local_id: CommandLocalId::new(local_id)?,
        })
    }

    pub fn stable_key(&self) -> String {
        format!(
            "{}{}:{}",
            self.source.stable_key(),
            self.local_id.as_str().len(),
            self.local_id
        )
    }
}

fn validate_tool_name(value: &str) -> Result<(), ExternalSourceContractError> {
    if value.is_empty()
        || value.len() > MAX_TOOL_NAME_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(ExternalSourceContractError::InvalidIdentifier("tool name"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceQualifiedToolTargetId {
    pub source: SourceKey,
    pub local_id: ToolTargetLocalId,
}

impl SourceQualifiedToolTargetId {
    pub fn new(
        source: SourceKey,
        local_id: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        Ok(Self {
            source,
            local_id: ToolTargetLocalId::new(local_id)?,
        })
    }

    pub fn stable_key(&self) -> String {
        format!(
            "{}{}:{}",
            self.source.stable_key(),
            self.local_id.as_str().len(),
            self.local_id
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceQualifiedToolId {
    pub target: SourceQualifiedToolTargetId,
    pub export_id: ToolExportLocalId,
}

impl SourceQualifiedToolId {
    pub fn new(
        target: SourceQualifiedToolTargetId,
        export_id: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        Ok(Self {
            target,
            export_id: ToolExportLocalId::new(export_id)?,
        })
    }

    pub fn stable_key(&self) -> String {
        format!(
            "{}{}:{}",
            self.target.stable_key(),
            self.export_id.as_str().len(),
            self.export_id
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceQualifiedMcpServerId {
    pub source: SourceKey,
    pub local_id: McpServerLocalId,
}

impl SourceQualifiedMcpServerId {
    pub fn new(
        source: SourceKey,
        local_id: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        Ok(Self {
            source,
            local_id: McpServerLocalId::new(local_id)?,
        })
    }

    pub fn stable_key(&self) -> String {
        format!(
            "{}{}:{}",
            self.source.stable_key(),
            self.local_id.as_str().len(),
            self.local_id
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalMcpTransportKind {
    LocalStdio,
    StreamableHttp,
}

impl ExternalMcpTransportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalStdio => "local_stdio",
            Self::StreamableHttp => "streamable_http",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalMcpStaticStatus {
    Ready,
    DisabledBySource,
    Unsupported { reason: String },
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMcpServerDefinition {
    pub id: SourceQualifiedMcpServerId,
    /// Ordered low-to-high source contributions used to materialize this server.
    pub provenance: Vec<SourceKey>,
    /// Logical MCP name used for conflict detection and the public tool namespace.
    pub name: String,
    pub transport: ExternalMcpTransportKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_preview: Option<String>,
    pub argument_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_keys: Vec<String>,
    /// Names read from BitFun's parent environment while resolving `{env:NAME}`
    /// references. Values never enter the static catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_reference_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_names: Vec<String>,
    pub source_enabled: bool,
    pub behavior_version: String,
    pub static_status: ExternalMcpStaticStatus,
}

impl ExternalMcpServerDefinition {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.name, "MCP server name")?;
        validate_id(&self.behavior_version, "MCP behavior version")?;
        if self.provenance.is_empty()
            || self.provenance.len() > 256
            || self.provenance.last() != Some(&self.id.source)
            || self.argument_count > 256
            || self.environment_keys.len() > 128
            || self.environment_reference_names.len() > 128
            || self.header_names.len() > 128
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "MCP configuration size",
            ));
        }
        let mut provenance = BTreeSet::new();
        if self
            .provenance
            .iter()
            .any(|source| !provenance.insert(source))
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "MCP provenance",
            ));
        }
        if let Some(command) = &self.command_preview {
            validate_text(command, "MCP command preview")?;
        }
        if let Some(directory) = &self.working_directory {
            validate_text(directory, "MCP working directory")?;
        }
        if let Some(url) = &self.remote_url_preview {
            validate_text(url, "MCP remote URL preview")?;
        }
        let mut environment_keys = BTreeSet::new();
        for key in &self.environment_keys {
            validate_id(key, "MCP environment key")?;
            if !environment_keys.insert(key) {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "MCP environment key",
                ));
            }
        }
        let mut environment_reference_names = BTreeSet::new();
        for name in &self.environment_reference_names {
            validate_id(name, "MCP environment reference")?;
            if !environment_reference_names.insert(name) {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "MCP environment reference",
                ));
            }
        }
        let mut header_names = BTreeSet::new();
        for name in &self.header_names {
            validate_id(name, "MCP header name")?;
            if !header_names.insert(name.to_ascii_lowercase()) {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "MCP header name",
                ));
            }
        }
        match self.transport {
            ExternalMcpTransportKind::LocalStdio
                if self.command_preview.is_none() || self.remote_url_preview.is_some() =>
            {
                Err(ExternalSourceContractError::InvalidIdentifier(
                    "local MCP transport",
                ))
            }
            ExternalMcpTransportKind::StreamableHttp
                if self.remote_url_preview.is_none() || self.command_preview.is_some() =>
            {
                Err(ExternalSourceContractError::InvalidIdentifier(
                    "remote MCP transport",
                ))
            }
            _ => match &self.static_status {
                ExternalMcpStaticStatus::Unsupported { reason }
                | ExternalMcpStaticStatus::Invalid { reason } => {
                    validate_text(reason, "MCP static status reason")
                }
                ExternalMcpStaticStatus::Ready | ExternalMcpStaticStatus::DisabledBySource => {
                    Ok(())
                }
            },
        }
    }

    pub fn candidate_id(&self) -> String {
        format!("external_mcp:{}", self.id.stable_key())
    }
}

/// Product-owned state for one discovered external MCP candidate. Ecosystem
/// adapters never set this value; they only provide static definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalMcpActivationState {
    ApprovalRequired,
    Starting,
    Active,
    Declined,
    Conflict,
    Covered { selected_candidate_id: String },
    SourceDisabled,
    ConfigurationChanged,
    Unsupported { reason: String },
    RuntimeUnavailable { reason: String },
    Removed,
}

/// Sanitized product view for one external MCP server. Runtime credentials are
/// intentionally absent and remain available only through `prepare_server`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMcpCatalogEntry {
    pub candidate_id: String,
    pub definition: ExternalMcpServerDefinition,
    pub approval_key: String,
    pub decision_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
    pub activation_state: ExternalMcpActivationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMcpApprovalRequest {
    pub candidate_id: String,
    pub approval_key: String,
    pub decision_key: String,
    pub definition: ExternalMcpServerDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMcpConflictCandidate {
    pub candidate_id: String,
    pub display_name: String,
    pub external: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceKey>,
    pub behavior_version: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMcpConflict {
    pub conflict_key: String,
    pub server_name: String,
    pub candidates: Vec<ExternalMcpConflictCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_candidate_id: Option<String>,
}

/// Secret runtime value whose Debug representation never includes the value.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum PreparedExternalMcpTransport {
    Local {
        command: String,
        args: Vec<String>,
        environment: BTreeMap<String, SecretValue>,
        working_directory: Option<PathBuf>,
    },
    Remote {
        url: String,
        headers: BTreeMap<String, SecretValue>,
        oauth_enabled: bool,
    },
}

impl PreparedExternalMcpTransport {
    pub fn remote_headers(&self) -> Option<&BTreeMap<String, SecretValue>> {
        match self {
            Self::Remote { headers, .. } => Some(headers),
            Self::Local { .. } => None,
        }
    }
}

impl fmt::Debug for PreparedExternalMcpTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local {
                command,
                args,
                environment,
                working_directory,
            } => formatter
                .debug_struct("Local")
                .field("command", command)
                .field("args", args)
                .field("environment_keys", &environment.keys().collect::<Vec<_>>())
                .field("working_directory", working_directory)
                .finish(),
            Self::Remote {
                url: _,
                headers,
                oauth_enabled,
            } => formatter
                .debug_struct("Remote")
                // Query strings and path segments can carry credentials. The
                // sanitized product snapshot already owns the safe preview.
                .field("url", &"[REDACTED]")
                .field("header_names", &headers.keys().collect::<Vec<_>>())
                .field("oauth_enabled", oauth_enabled)
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExternalMcpServer {
    pub id: SourceQualifiedMcpServerId,
    pub behavior_version: String,
    pub transport: PreparedExternalMcpTransport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMcpProviderIdentity {
    pub provider_id: ProviderId,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
}

impl ExternalMcpProviderIdentity {
    pub fn new(
        provider_id: impl Into<String>,
        ecosystem_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        let display_name = display_name.into();
        validate_text(&display_name, "MCP provider display name")?;
        Ok(Self {
            provider_id: ProviderId::new(provider_id)?,
            ecosystem_id: EcosystemId::new(ecosystem_id)?,
            display_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMcpProviderSnapshot {
    pub provider: ExternalMcpProviderIdentity,
    pub sources: Vec<ExternalSourceRecord>,
    pub servers: Vec<ExternalMcpServerDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl ExternalMcpProviderSnapshot {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        if self.sources.len() > 1024 || self.servers.len() > 1024 {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "MCP provider snapshot size",
            ));
        }
        let mut source_keys = BTreeSet::new();
        for source in &self.sources {
            source.validate()?;
            if source.key.provider_id != self.provider.provider_id
                || source.ecosystem_id != self.provider.ecosystem_id
                || !source_keys.insert(source.key.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "MCP provider-qualified source",
                ));
            }
        }
        let mut server_ids = BTreeSet::new();
        for server in &self.servers {
            server.validate()?;
            if server.id.source.provider_id != self.provider.provider_id
                || !source_keys.contains(&server.id.source)
                || server
                    .provenance
                    .iter()
                    .any(|source| !source_keys.contains(source))
                || !server_ids.insert(server.id.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "provider-qualified MCP server",
                ));
            }
        }
        Ok(())
    }
}

/// Capability-specific MCP provider. Discovery is static; preparation may read
/// runtime values only after the product owner has approved a behavior version.
pub trait ExternalMcpSourceProvider: Send + Sync {
    fn identity(&self) -> ExternalMcpProviderIdentity;

    fn discover(
        &self,
        input: &ExternalMcpDiscoveryInput,
    ) -> Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError>;

    fn prepare_server(
        &self,
        input: &ExternalMcpDiscoveryInput,
        server_id: &SourceQualifiedMcpServerId,
        expected_behavior_version: &str,
    ) -> Result<PreparedExternalMcpServer, ExternalSourceProviderError>;

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot>;
}

pub fn external_mcp_approval_key(
    execution_domain_id: &str,
    workspace_key: &str,
    server_id: &SourceQualifiedMcpServerId,
    behavior_version: &str,
) -> String {
    let stable_id = server_id.stable_key();
    let workspace_fingerprint = stable_fingerprint([workspace_key.as_bytes()]);
    let server_fingerprint = stable_fingerprint([stable_id.as_bytes()]);
    format!(
        "external_mcp_approval:{}:{}:{}:{}",
        execution_domain_id,
        &workspace_fingerprint[..24],
        &server_fingerprint[..24],
        stable_fingerprint([behavior_version.as_bytes()])
    )
}

pub fn external_mcp_conflict_key<'a>(
    execution_domain_id: &str,
    workspace_key: &str,
    server_name: &str,
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let normalized_name = server_name.to_ascii_lowercase();
    let lineage = external_mcp_conflict_lineage(execution_domain_id, workspace_key, server_name);
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_unstable();
    let encoded = candidates
        .into_iter()
        .map(|(id, version)| format!("{}:{id}{}:{version}", id.len(), version.len()))
        .collect::<Vec<_>>();
    let mut parts = vec![
        execution_domain_id.as_bytes(),
        workspace_key.as_bytes(),
        normalized_name.as_bytes(),
    ];
    parts.extend(encoded.iter().map(|value| value.as_bytes()));
    format!("{}:{}", lineage, stable_fingerprint(parts))
}

fn external_mcp_conflict_lineage(
    execution_domain_id: &str,
    workspace_key: &str,
    server_name: &str,
) -> String {
    let workspace_fingerprint = stable_fingerprint([workspace_key.as_bytes()]);
    format!(
        "external_mcp:{}:{}:{}",
        execution_domain_id,
        &workspace_fingerprint[..24],
        server_name.to_ascii_lowercase()
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalToolRuntimeKind {
    JavaScript,
    TypeScript,
}

impl ExternalToolRuntimeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::JavaScript => "java_script",
            Self::TypeScript => "type_script",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalToolCapability {
    FileSystem,
    Network,
    Process,
    Environment,
}

impl ExternalToolCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileSystem => "file_system",
            Self::Network => "network",
            Self::Process => "process",
            Self::Environment => "environment",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalToolStaticStatus {
    Ready,
    Unsupported { reason: String },
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolDefinition {
    pub id: SourceQualifiedToolId,
    pub name: String,
    pub description_preview: String,
    pub module_path: String,
    pub working_directory: String,
    pub runtime_kind: ExternalToolRuntimeKind,
    pub capabilities: Vec<ExternalToolCapability>,
    pub content_version: String,
    pub static_status: ExternalToolStaticStatus,
}

impl ExternalToolDefinition {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_tool_name(&self.name)?;
        if !self.description_preview.is_empty() {
            validate_text(&self.description_preview, "tool description preview")?;
        }
        validate_text(&self.module_path, "tool module path")?;
        validate_text(&self.working_directory, "tool working directory")?;
        validate_id(&self.content_version, "tool content version")?;
        let mut capabilities = BTreeSet::new();
        if self
            .capabilities
            .iter()
            .any(|capability| !capabilities.insert(*capability))
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "tool capability",
            ));
        }
        Ok(())
    }

    pub fn candidate_id(&self) -> String {
        format!("external:{}", self.id.stable_key())
    }
}

fn stable_fingerprint<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut first = 0xcbf29ce484222325_u64;
    let mut second = 0x84222325cbf29ce4_u64;
    for part in parts {
        for byte in part.iter().copied().chain([0]) {
            first ^= u64::from(byte);
            first = first.wrapping_mul(0x100000001b3);
            second ^= u64::from(byte);
            second = second.wrapping_mul(0x9e3779b185ebca87);
        }
    }
    format!("{first:016x}{second:016x}")
}

/// Approval follows a target, execution domain, runtime, and disclosed
/// capability set. A content-only update therefore keeps prior approval while
/// capability expansion requires a new decision.
pub fn external_tool_approval_key(
    execution_domain_id: &str,
    target: &SourceQualifiedToolTargetId,
    runtime_kind: ExternalToolRuntimeKind,
    capabilities: impl IntoIterator<Item = ExternalToolCapability>,
) -> String {
    let mut capabilities = capabilities.into_iter().collect::<Vec<_>>();
    capabilities.sort_unstable();
    capabilities.dedup();
    let target_key = target.stable_key();
    let runtime = runtime_kind.as_str();
    let capability_names = capabilities
        .into_iter()
        .map(ExternalToolCapability::as_str)
        .collect::<Vec<_>>();
    let mut parts = vec![
        execution_domain_id.as_bytes(),
        target_key.as_bytes(),
        runtime.as_bytes(),
    ];
    parts.extend(capability_names.iter().map(|value| value.as_bytes()));
    format!(
        "external_tool_approval:{}:{}",
        execution_domain_id,
        stable_fingerprint(parts)
    )
}

/// Builds a version-sensitive fingerprint for local, MCP, and external tool
/// candidates. Candidate ordering does not affect the result.
pub fn external_tool_conflict_key<'a>(
    execution_domain_id: &str,
    tool_name: &str,
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_unstable();
    // Registry tool names are case-sensitive; case-only variants must not
    // share a preference lineage or overwrite one another's decision.
    let normalized_name = tool_name;
    let encoded = candidates
        .into_iter()
        .map(|(id, version)| format!("{}:{id}{}:{version}", id.len(), version.len()))
        .collect::<Vec<_>>();
    let mut parts = vec![execution_domain_id.as_bytes(), normalized_name.as_bytes()];
    parts.extend(encoded.iter().map(|value| value.as_bytes()));
    format!(
        "external_tool:{}:{}:{}",
        execution_domain_id,
        normalized_name,
        stable_fingerprint(parts)
    )
}

pub fn external_tool_decision_key(approval_key: &str, content_version: &str) -> String {
    format!(
        "external_tool_decision:{}",
        stable_fingerprint([approval_key.as_bytes(), content_version.as_bytes()])
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalSourceScope {
    UserGlobal,
    Project,
    WorkspaceLocal,
    RemoteUser,
    RemoteProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalSourceHealth {
    Available,
    Partial,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalSourceDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalSourceAssetKind {
    #[default]
    Source,
    Command,
    Tool,
    Subagent,
    Mcp,
    Hook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceDiagnostic {
    pub severity: ExternalSourceDiagnosticSeverity,
    #[serde(default)]
    pub asset_kind: ExternalSourceAssetKind,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceKey>,
}

impl ExternalSourceDiagnostic {
    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        source: Option<SourceKey>,
    ) -> Self {
        Self {
            severity: ExternalSourceDiagnosticSeverity::Warning,
            asset_kind: ExternalSourceAssetKind::Source,
            code: code.into(),
            message: message.into(),
            source,
        }
    }

    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        source: Option<SourceKey>,
    ) -> Self {
        Self {
            severity: ExternalSourceDiagnosticSeverity::Error,
            asset_kind: ExternalSourceAssetKind::Source,
            code: code.into(),
            message: message.into(),
            source,
        }
    }

    pub fn with_asset_kind(mut self, asset_kind: ExternalSourceAssetKind) -> Self {
        self.asset_kind = asset_kind;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceRecord {
    pub key: SourceKey,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
    pub source_kind: String,
    pub scope: ExternalSourceScope,
    pub location: String,
    pub execution_domain_id: ExecutionDomainId,
    pub health: ExternalSourceHealth,
    pub content_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl ExternalSourceRecord {
    pub fn preference_key(&self) -> String {
        format!(
            "{}:{}{}",
            self.execution_domain_id.as_str().len(),
            self.execution_domain_id,
            self.key.stable_key()
        )
    }

    pub fn source_key_from_preference_key(value: &str) -> Option<SourceKey> {
        let (_, stable_source_key) = take_length_prefixed(value)?;
        SourceKey::from_stable_key(stable_source_key)
    }

    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.source_kind, "source kind")?;
        validate_text(&self.display_name, "source display name")?;
        validate_text(&self.location, "source location")?;
        validate_id(&self.content_version, "content version")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PromptCommandAvailability {
    Available,
    Restricted {
        reason: String,
        required_capabilities: Vec<String>,
    },
    Invalid {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptCommandDefinition {
    pub id: SourceQualifiedCommandId,
    pub name: String,
    pub description: String,
    pub template: String,
    pub availability: PromptCommandAvailability,
    /// Version of this command only. Unrelated edits in the same source must
    /// not invalidate a remembered conflict choice.
    pub content_version: String,
}

impl PromptCommandDefinition {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.name, "command name")?;
        if !self.description.is_empty() {
            validate_text(&self.description, "command description")?;
        }
        if self.template.is_empty() || self.template.len() > 256 * 1024 {
            return Err(ExternalSourceContractError::InvalidText("command template"));
        }
        validate_id(&self.content_version, "command content version")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpandedPromptCommand {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptCommandProviderIdentity {
    pub provider_id: ProviderId,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
}

impl PromptCommandProviderIdentity {
    pub fn new(
        provider_id: impl Into<String>,
        ecosystem_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        let display_name = display_name.into();
        validate_text(&display_name, "provider display name")?;
        Ok(Self {
            provider_id: ProviderId::new(provider_id)?,
            ecosystem_id: EcosystemId::new(ecosystem_id)?,
            display_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptCommandProviderSnapshot {
    pub provider: PromptCommandProviderIdentity,
    pub sources: Vec<ExternalSourceRecord>,
    pub commands: Vec<PromptCommandDefinition>,
    /// Commands that were discovered by identity but could not be read or
    /// parsed in this generation. The coordinator may retain only these
    /// commands from the previous valid generation; commands absent from both
    /// lists are stable deletions and must be withdrawn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable_command_ids: Vec<SourceQualifiedCommandId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl PromptCommandProviderSnapshot {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        let mut source_keys = BTreeSet::new();
        for source in &self.sources {
            source.validate()?;
            if source.key.provider_id != self.provider.provider_id
                || source.ecosystem_id != self.provider.ecosystem_id
                || !source_keys.insert(source.key.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "provider-qualified source",
                ));
            }
        }
        let mut command_ids = BTreeSet::new();
        for command in &self.commands {
            command.validate()?;
            if command.id.source.provider_id != self.provider.provider_id
                || !source_keys.contains(&command.id.source)
                || !command_ids.insert(command.id.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "provider-qualified command",
                ));
            }
        }
        let mut unavailable_ids = BTreeSet::new();
        for command_id in &self.unavailable_command_ids {
            if command_id.source.provider_id != self.provider.provider_id
                || !source_keys.contains(&command_id.source)
                || command_ids.contains(command_id)
                || !unavailable_ids.insert(command_id.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "unavailable provider-qualified command",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalSourceContext {
    pub workspace_root: Option<PathBuf>,
    pub execution_domain_id: ExecutionDomainId,
}

/// Host-private key used to turn secret-bearing executable configuration into
/// a stable opaque revision. The key is deliberately not serializable and its
/// debug representation never exposes key material.
#[derive(Clone, PartialEq, Eq)]
pub struct ExternalMcpRevisionKey([u8; 32]);

impl ExternalMcpRevisionKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Produces a stable public revision without exposing an offline oracle
    /// for literal credentials contained in executable configuration.
    pub fn opaque_revision<'a>(
        &self,
        domain: &str,
        parts: impl IntoIterator<Item = &'a [u8]>,
    ) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.0).expect("SHA-256 HMAC accepts a 32-byte key");
        update_hmac_framed(&mut mac, domain.as_bytes());
        for part in parts {
            update_hmac_framed(&mut mac, part);
        }
        format!("hmac-sha256:{}", hex::encode(mac.finalize().into_bytes()))
    }
}

fn update_hmac_framed(mac: &mut Hmac<Sha256>, value: &[u8]) {
    mac.update(&(value.len() as u64).to_be_bytes());
    mac.update(value);
}

impl fmt::Debug for ExternalMcpRevisionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExternalMcpRevisionKey([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMcpDiscoveryInput {
    pub context: ExternalSourceContext,
    pub suppressed_sources: BTreeSet<SourceKey>,
    pub revision_key: ExternalMcpRevisionKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWatchRoot {
    pub path: PathBuf,
    pub recursive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceProviderError {
    pub code: String,
    pub message: String,
    pub transient: bool,
}

impl ExternalSourceProviderError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, transient: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            transient,
        }
    }
}

impl fmt::Display for ExternalSourceProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for ExternalSourceProviderError {}

/// Capability-specific provider implemented independently by each ecosystem adapter.
pub trait PromptCommandSourceProvider: Send + Sync {
    fn identity(&self) -> PromptCommandProviderIdentity;

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<PromptCommandProviderSnapshot, ExternalSourceProviderError>;

    fn expand(
        &self,
        command: &PromptCommandDefinition,
        arguments: &str,
    ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError>;

    /// Resolves same-ecosystem overlays after product suppression is applied.
    /// The full snapshot is supplied so a provider can preserve native source
    /// ordering and treat known-but-unavailable higher-layer identities as
    /// tombstones instead of silently reactivating a lower-layer definition.
    /// Providers with no internal duplicate names may use this default.
    fn resolve_commands(
        &self,
        snapshot: &PromptCommandProviderSnapshot,
        enabled_sources: &BTreeSet<SourceKey>,
    ) -> Result<Vec<PromptCommandDefinition>, ExternalSourceProviderError> {
        let mut names = BTreeSet::new();
        let mut resolved = Vec::new();
        for command in snapshot
            .commands
            .iter()
            .filter(|command| enabled_sources.contains(&command.id.source))
        {
            if !names.insert(command.name.to_ascii_lowercase()) {
                return Err(ExternalSourceProviderError::new(
                    "external_source.provider_resolution_required",
                    "provider returned same-name commands without resolving its ecosystem overlays",
                    false,
                ));
            }
            resolved.push(command.clone());
        }
        Ok(resolved)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolProviderIdentity {
    pub provider_id: ProviderId,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
}

impl ExternalToolProviderIdentity {
    pub fn new(
        provider_id: impl Into<String>,
        ecosystem_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        let display_name = display_name.into();
        validate_text(&display_name, "tool provider display name")?;
        Ok(Self {
            provider_id: ProviderId::new(provider_id)?,
            ecosystem_id: EcosystemId::new(ecosystem_id)?,
            display_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolProviderSnapshot {
    pub provider: ExternalToolProviderIdentity,
    pub sources: Vec<ExternalSourceRecord>,
    pub tools: Vec<ExternalToolDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl ExternalToolProviderSnapshot {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        let mut source_keys = BTreeSet::new();
        for source in &self.sources {
            source.validate()?;
            if source.key.provider_id != self.provider.provider_id
                || source.ecosystem_id != self.provider.ecosystem_id
                || !source_keys.insert(source.key.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "tool provider-qualified source",
                ));
            }
        }
        let mut tool_ids = BTreeSet::new();
        for tool in &self.tools {
            tool.validate()?;
            if tool.id.target.source.provider_id != self.provider.provider_id
                || !source_keys.contains(&tool.id.target.source)
                || !tool_ids.insert(tool.id.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "provider-qualified tool",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExternalToolExport {
    pub export_name: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExternalToolTarget {
    pub target_id: SourceQualifiedToolTargetId,
    pub content_version: String,
    pub module_source: String,
    pub module_url: String,
    pub working_directory: String,
    /// Optional VCS/worktree root supplied by the ecosystem adapter. The
    /// generic runtime forwards it without deriving ecosystem-specific paths.
    pub worktree_root: Option<String>,
    pub expected_tools: Vec<PreparedExternalToolExport>,
}

/// Capability-specific tool provider implemented independently by each
/// external ecosystem adapter. Discovery is static; executable preparation is
/// called only after product approval.
pub trait ExternalToolSourceProvider: Send + Sync {
    fn identity(&self) -> ExternalToolProviderIdentity;

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalToolProviderSnapshot, ExternalSourceProviderError>;

    fn prepare_target(
        &self,
        context: &ExternalSourceContext,
        target_id: &SourceQualifiedToolTargetId,
        expected_content_version: &str,
    ) -> Result<PreparedExternalToolTarget, ExternalSourceProviderError>;

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalSourceLifecycleState {
    Available,
    Restricted,
    Degraded,
    Unavailable,
    Removed,
    Suppressed,
    UsingLastValidVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceCatalogEntry {
    pub stable_key: String,
    /// Opaque identity shared by provider records that describe the same
    /// physical source. Product surfaces may coalesce matching entries without
    /// comparing redacted display locations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_group_id: Option<String>,
    pub record: ExternalSourceRecord,
    pub lifecycle: ExternalSourceLifecycleState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptCommandCatalogEntry {
    pub definition: PromptCommandDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalToolActivationState {
    ApprovalRequired,
    Declined,
    Disabled,
    Active,
    Conflict,
    Unsupported { reason: String },
    RuntimeUnavailable { reason: String },
    LoadFailed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolCatalogEntry {
    pub definition: ExternalToolDefinition,
    pub approval_key: String,
    pub decision_key: String,
    pub activation: ExternalToolActivationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolApprovalRequest {
    pub approval_key: String,
    pub decision_key: String,
    pub target_id: SourceQualifiedToolTargetId,
    pub source_display_name: String,
    pub source_scope: ExternalSourceScope,
    pub source_location: String,
    pub working_directory: String,
    pub runtime_kind: ExternalToolRuntimeKind,
    pub capabilities: Vec<ExternalToolCapability>,
    pub content_version: String,
    pub tool_names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalToolConflictCandidateKind {
    BuiltIn,
    Mcp,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolConflictCandidate {
    pub candidate_id: String,
    pub display_name: String,
    pub kind: ExternalToolConflictCandidateKind,
    pub provider_id: String,
    pub content_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolConflict {
    pub conflict_key: String,
    pub tool_name: String,
    pub candidates: Vec<ExternalToolConflictCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_candidate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptCommandConflictCandidate {
    pub candidate_id: String,
    pub source: SourceKey,
    pub source_display_name: String,
    pub ecosystem_id: EcosystemId,
    pub content_version: String,
    pub command_description: String,
    pub source_scope: ExternalSourceScope,
    pub source_location: String,
    pub availability: PromptCommandAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptCommandConflict {
    pub conflict_key: String,
    pub command_name: String,
    pub candidates: Vec<PromptCommandConflictCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_candidate_id: Option<String>,
}

/// Builds a stable conflict fingerprint that changes when a participant or its
/// content version changes. Candidate ordering does not affect the result.
pub fn prompt_command_conflict_key<'a>(
    execution_domain_id: &str,
    command_name: &str,
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_unstable();
    let mut first = 0xcbf29ce484222325_u64;
    let mut second = 0x84222325cbf29ce4_u64;
    for byte in execution_domain_id
        .bytes()
        .chain([0])
        .chain(command_name.to_ascii_lowercase().bytes())
        .chain(candidates.into_iter().flat_map(|(id, version)| {
            format!("{}:{id}{}:{version}", id.len(), version.len()).into_bytes()
        }))
    {
        first ^= u64::from(byte);
        first = first.wrapping_mul(0x100000001b3);
        second ^= u64::from(byte);
        second = second.wrapping_mul(0x9e3779b185ebca87);
    }
    format!(
        "prompt_command:{}:{}:{first:016x}{second:016x}",
        execution_domain_id,
        command_name.to_ascii_lowercase()
    )
}

pub fn native_prompt_command_conflict_key<'a>(
    execution_domain_id: &str,
    command_name: &str,
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_unstable();
    let normalized_name = command_name.to_ascii_lowercase();
    let execution_domain_fingerprint = stable_fingerprint([execution_domain_id.as_bytes()]);
    let native_group_fingerprint = native_prompt_command_group_fingerprint(
        candidates
            .iter()
            .map(|(candidate_id, _)| *candidate_id)
            .filter(|candidate_id| candidate_id.starts_with("bitfun.")),
    );
    let participant_fingerprint = prompt_command_conflict_key(
        execution_domain_id,
        &normalized_name,
        candidates.iter().copied(),
    );
    format!(
        "native:prompt_command:{}:{}:{}:{}:{}",
        &execution_domain_fingerprint[..24],
        normalized_name.len(),
        normalized_name,
        &native_group_fingerprint[..24],
        stable_fingerprint([participant_fingerprint.as_bytes()]),
    )
}

/// Computes the canonical identity of one product surface's native command
/// candidate group. Consumers use the same fingerprint when invalidating only
/// that surface's persisted conflict choices.
pub fn native_prompt_command_group_fingerprint<'a>(
    candidate_ids: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut candidate_ids = candidate_ids.into_iter().collect::<Vec<_>>();
    candidate_ids.sort_unstable();
    candidate_ids.dedup();
    let encoded = candidate_ids
        .into_iter()
        .map(|candidate_id| format!("{}:{candidate_id}", candidate_id.len()))
        .collect::<Vec<_>>();
    stable_fingerprint(encoded.iter().map(|value| value.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Stable identity facts declared by a product surface for its own slash
/// commands. This carries no executable payload: Core validates the facts and
/// remains the sole owner of conflict keys, preferences, reconfirmation, and
/// guarded external expansion.
pub struct NativePromptCommandDescriptor {
    pub command_name: String,
    pub candidate_id: String,
    pub behavior_version: String,
}

impl NativePromptCommandDescriptor {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.command_name, "native prompt command name")?;
        validate_text(&self.candidate_id, "native prompt command candidate")?;
        if !self.candidate_id.starts_with("bitfun.") {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "native prompt command candidate",
            ));
        }
        validate_text(
            &self.behavior_version,
            "native prompt command behavior version",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePromptCommandConflictProjection {
    pub command_name: String,
    pub external_candidate_id: String,
    pub conflict_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_candidate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePromptCommandReconfirmationProjection {
    pub command_name: String,
    pub native_candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePromptCommandConflictSnapshot {
    pub preference_revision: u64,
    pub conflicts: Vec<NativePromptCommandConflictProjection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reconfirmations: Vec<NativePromptCommandReconfirmationProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceCatalogSnapshot {
    pub generation: u64,
    /// True until every registered provider has produced its first result.
    /// Product surfaces must present this as a neutral discovery state rather
    /// than treating the current empty catalog as a confirmed empty result.
    #[serde(default)]
    pub discovery_pending: bool,
    pub sources: Vec<ExternalSourceCatalogEntry>,
    pub commands: Vec<PromptCommandCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_conflicts: Vec<PromptCommandConflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ExternalToolCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_approval_requests: Vec<ExternalToolApprovalRequest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_conflicts: Vec<ExternalToolConflict>,
    /// Independent catalog generation for MCP row/action stability.
    #[serde(default)]
    pub mcp_generation: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<ExternalMcpCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_approval_requests: Vec<ExternalMcpApprovalRequest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_conflicts: Vec<ExternalMcpConflict>,
    /// Independent catalog generation for subagent row/action stability.
    #[serde(default)]
    pub subagent_generation: u64,
    /// Monotonic persisted-preference revision used by subagent decisions.
    #[serde(default)]
    pub preference_revision: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<crate::external_subagents::ExternalSubagentSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagent_conflicts: Vec<crate::external_subagents::ExternalSubagentConflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_subagent_approvals: Vec<String>,
    /// Effective policy is owned by product assembly and projected unchanged
    /// to every product surface.
    #[serde(default)]
    pub integration_policy: ExternalIntegrationPolicySnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalPromptCommandDefinitionSummary {
    pub id: SourceQualifiedCommandId,
    pub name: String,
    pub description: String,
    pub availability: PromptCommandAvailability,
    pub content_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalPromptCommandSummary {
    /// Opaque guarded-execution identity. New product surfaces must pass this
    /// value back unchanged rather than reproducing the stable-key encoding.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub candidate_id: String,
    pub definition: ExternalPromptCommandDefinitionSummary,
}

/// Stable cross-host projection. Executable prompt templates and prepared
/// runtime payloads never cross a product-surface transport boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceHostCapabilities {
    pub can_refresh: bool,
    pub can_mutate_policy: bool,
    pub can_manage_sources: bool,
    pub can_approve_runtime: bool,
    pub can_execute_external_assets: bool,
    pub can_set_safe_mode: bool,
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub can_reveal_source_location: bool,
}

fn bool_is_false(value: &bool) -> bool {
    !*value
}

impl ExternalSourceHostCapabilities {
    pub const fn read_write() -> Self {
        Self {
            can_refresh: true,
            can_mutate_policy: true,
            can_manage_sources: true,
            can_approve_runtime: true,
            can_execute_external_assets: true,
            can_set_safe_mode: true,
            can_reveal_source_location: false,
        }
    }

    /// Desktop-local host actions are intentionally separate from portable
    /// read/write control capabilities. Remote and headless hosts fail closed.
    pub const fn local_desktop() -> Self {
        Self {
            can_reveal_source_location: true,
            ..Self::read_write()
        }
    }

    pub const fn read_only_projection() -> Self {
        Self {
            can_refresh: true,
            can_mutate_policy: false,
            can_manage_sources: false,
            can_approve_runtime: false,
            can_execute_external_assets: false,
            can_set_safe_mode: false,
            can_reveal_source_location: false,
        }
    }
}

impl Default for ExternalSourceHostCapabilities {
    fn default() -> Self {
        Self::read_write()
    }
}

/// Stable cross-host projection. Executable prompt templates and prepared
/// runtime payloads never cross a product-surface transport boundary. Host
/// capability facts are transport-owned and do not alter the authoritative
/// product catalog or persisted policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourcePublicSnapshot {
    #[serde(default)]
    pub host_capabilities: ExternalSourceHostCapabilities,
    pub generation: u64,
    pub discovery_pending: bool,
    pub sources: Vec<ExternalSourceCatalogEntry>,
    pub commands: Vec<ExternalPromptCommandSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_conflicts: Vec<PromptCommandConflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ExternalToolCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_approval_requests: Vec<ExternalToolApprovalRequest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_conflicts: Vec<ExternalToolConflict>,
    #[serde(default)]
    pub mcp_generation: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<ExternalMcpCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_approval_requests: Vec<ExternalMcpApprovalRequest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_conflicts: Vec<ExternalMcpConflict>,
    #[serde(default)]
    pub subagent_generation: u64,
    #[serde(default)]
    pub preference_revision: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<crate::external_subagents::ExternalSubagentSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagent_conflicts: Vec<crate::external_subagents::ExternalSubagentConflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_subagent_approvals: Vec<String>,
    #[serde(default)]
    pub integration_policy: ExternalIntegrationPolicySnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl ExternalSourcePublicSnapshot {
    /// Down-projects values added after the original catalog contract so an
    /// older controller never receives an enum variant it cannot render.
    /// New control clients use `ExternalSourceControlSnapshotV1` for the
    /// orthogonal review state instead.
    pub fn into_legacy_v0_compatible(mut self) -> Self {
        for command in &mut self.commands {
            command.candidate_id.clear();
        }
        for tool in &mut self.tools {
            if matches!(tool.activation, ExternalToolActivationState::Declined) {
                tool.activation = ExternalToolActivationState::Disabled;
            }
        }
        for subagent in &mut self.subagents {
            subagent.unavailable_tool_labels.clear();
        }
        self
    }
}

impl From<ExternalSourceCatalogSnapshot> for ExternalSourcePublicSnapshot {
    fn from(snapshot: ExternalSourceCatalogSnapshot) -> Self {
        Self {
            host_capabilities: ExternalSourceHostCapabilities::read_write(),
            generation: snapshot.generation,
            discovery_pending: snapshot.discovery_pending,
            sources: snapshot.sources,
            commands: snapshot
                .commands
                .into_iter()
                .map(|entry| {
                    let candidate_id = entry.definition.id.stable_key();
                    ExternalPromptCommandSummary {
                        candidate_id,
                        definition: ExternalPromptCommandDefinitionSummary {
                            id: entry.definition.id,
                            name: entry.definition.name,
                            description: entry.definition.description,
                            availability: entry.definition.availability,
                            content_version: entry.definition.content_version,
                        },
                    }
                })
                .collect(),
            command_conflicts: snapshot.command_conflicts,
            tools: snapshot.tools,
            tool_approval_requests: snapshot.tool_approval_requests,
            tool_conflicts: snapshot.tool_conflicts,
            mcp_generation: snapshot.mcp_generation,
            mcp_servers: snapshot.mcp_servers,
            mcp_approval_requests: snapshot.mcp_approval_requests,
            mcp_conflicts: snapshot.mcp_conflicts,
            subagent_generation: snapshot.subagent_generation,
            preference_revision: snapshot.preference_revision,
            subagents: snapshot.subagents,
            subagent_conflicts: snapshot.subagent_conflicts,
            pending_subagent_approvals: snapshot.pending_subagent_approvals,
            integration_policy: snapshot.integration_policy,
            diagnostics: snapshot.diagnostics,
        }
    }
}
