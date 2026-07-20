//! Ecosystem-neutral contracts for external AI application sources.
//!
//! Ecosystem adapters implement capability-specific provider traits. Product
//! surfaces and lifecycle coordination consume these types without branching on
//! a concrete ecosystem or carrying arbitrary extension payloads.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

const MAX_ID_LENGTH: usize = 160;
const MAX_TOOL_NAME_LENGTH: usize = 64;
const MAX_TEXT_LENGTH: usize = 4096;

fn validate_id(value: &str, label: &'static str) -> Result<(), ExternalSourceContractError> {
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
}

impl fmt::Display for ExternalSourceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(label) => write!(formatter, "invalid {label} identifier"),
            Self::InvalidText(label) => write!(formatter, "invalid {label} text"),
        }
    }
}

impl Error for ExternalSourceContractError {}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceDiagnostic {
    pub severity: ExternalSourceDiagnosticSeverity,
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
            code: code.into(),
            message: message.into(),
            source,
        }
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
    /// Providers with no internal duplicate names may use this default.
    fn resolve_commands(
        &self,
        commands: &[PromptCommandDefinition],
        enabled_sources: &BTreeSet<SourceKey>,
    ) -> Result<Vec<PromptCommandDefinition>, ExternalSourceProviderError> {
        let mut names = BTreeSet::new();
        let mut resolved = Vec::new();
        for command in commands
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}
