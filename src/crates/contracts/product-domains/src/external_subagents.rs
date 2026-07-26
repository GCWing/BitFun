//! Provider-neutral contracts for external subagent definitions.
//!
//! Ecosystem adapters own parsing and same-ecosystem composition. Product
//! assembly consumes only these typed definitions and never receives raw
//! configuration payloads.

use crate::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceContractError,
    ExternalSourceDiagnostic, ExternalSourceProviderError, ExternalSourceRecord,
    ExternalSourceScope, ExternalWatchRoot, ProviderId, SourceKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

const MAX_ID_LENGTH: usize = 160;
const MAX_LABEL_LENGTH: usize = 4096;
const MAX_PROMPT_BYTES: usize = 256 * 1024;
const MAX_TOOL_SELECTORS: usize = 256;
const MAX_DIAGNOSTIC_CODES: usize = 256;
const MAX_PROVENANCE_REFS: usize = 256;
const MAX_PROVIDER_SOURCES: usize = 1024;
const MAX_PROVIDER_DEFINITIONS: usize = 1024;
const MAX_PROVIDER_DIAGNOSTICS: usize = 1024;

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

fn is_stable_diagnostic_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_ID_LENGTH
        && code.trim() == code
        && !code.chars().any(char::is_control)
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn validate_provider_diagnostic(
    diagnostic: &ExternalSourceDiagnostic,
    provider_id: &ProviderId,
    sources: &BTreeSet<SourceKey>,
) -> Result<(), ExternalSourceContractError> {
    if !is_stable_diagnostic_code(&diagnostic.code) {
        return Err(ExternalSourceContractError::InvalidIdentifier(
            "external subagent provider diagnostic code",
        ));
    }
    if diagnostic.message.trim().is_empty()
        || diagnostic.message.len() > MAX_LABEL_LENGTH
        || diagnostic.message.chars().any(char::is_control)
    {
        return Err(ExternalSourceContractError::InvalidText(
            "external subagent provider diagnostic message",
        ));
    }
    if diagnostic.asset_kind != ExternalSourceAssetKind::Subagent
        || diagnostic
            .source
            .as_ref()
            .is_some_and(|source| &source.provider_id != provider_id || !sources.contains(source))
    {
        return Err(ExternalSourceContractError::InvalidIdentifier(
            "external subagent provider diagnostic source",
        ));
    }
    Ok(())
}

macro_rules! external_subagent_id {
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

external_subagent_id!(ExternalSubagentLocalId, "external subagent local");
external_subagent_id!(ExternalSubagentCandidateId, "external subagent candidate");
external_subagent_id!(
    ExternalSubagentBehaviorVersion,
    "external subagent behavior version"
);

/// Sensitive prompt text that deliberately implements neither `Serialize` nor
/// a content-bearing `Debug` representation.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretText(String);

impl SecretText {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretText([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentContributionId {
    pub source: SourceKey,
    pub local_id: ExternalSubagentLocalId,
}

impl ExternalSubagentContributionId {
    pub fn new(source: SourceKey, local_id: ExternalSubagentLocalId) -> Self {
        Self { source, local_id }
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
pub enum ExternalSubagentContributionRole {
    Base,
    Overlay,
    Definition,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentProvenanceRef {
    pub contribution_id: ExternalSubagentContributionId,
    pub role: ExternalSubagentContributionRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentProviderIdentity {
    pub provider_id: ProviderId,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
}

impl ExternalSubagentProviderIdentity {
    pub fn new(
        provider_id: impl Into<String>,
        ecosystem_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        let display_name = display_name.into();
        if display_name.is_empty()
            || display_name.len() > MAX_LABEL_LENGTH
            || display_name.chars().any(char::is_control)
        {
            return Err(ExternalSourceContractError::InvalidText(
                "external subagent provider display name",
            ));
        }
        Ok(Self {
            provider_id: ProviderId::new(provider_id)?,
            ecosystem_id: EcosystemId::new(ecosystem_id)?,
            display_name,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSubagentMode {
    Subagent,
    All,
    Primary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExternalSubagentModelRequest {
    Default,
    Exact {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_hint: Option<String>,
        model_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentToolSelector {
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_host_name: Option<String>,
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentToolRequest {
    pub selectors: Vec<ExternalSubagentToolSelector>,
    pub uses_conservative_default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSubagentCompatibilityState {
    Ready,
    ReadyWithDegradation,
    Blocked,
    Invalid,
}

/// Complete backend-only definition. Prompt content is redacted from `Debug`
/// and the type is intentionally not serializable.
#[derive(Clone, PartialEq, Eq)]
pub struct ExternalSubagentDefinition {
    pub candidate_id: ExternalSubagentCandidateId,
    pub logical_id: String,
    pub provenance: Vec<ExternalSubagentProvenanceRef>,
    pub display_name: String,
    pub description: String,
    pub prompt: SecretText,
    pub mode: ExternalSubagentMode,
    pub disabled: bool,
    pub hidden: bool,
    pub requested_model: ExternalSubagentModelRequest,
    pub requested_tools: ExternalSubagentToolRequest,
    pub compatibility: ExternalSubagentCompatibilityState,
    pub diagnostic_codes: Vec<String>,
    pub behavior_version: ExternalSubagentBehaviorVersion,
}

impl fmt::Debug for ExternalSubagentDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalSubagentDefinition")
            .field("candidate_id", &self.candidate_id)
            .field("logical_id", &self.logical_id)
            .field("provenance", &self.provenance)
            .field("display_name", &self.display_name)
            .field("description", &self.description)
            .field("prompt", &self.prompt)
            .field("mode", &self.mode)
            .field("disabled", &self.disabled)
            .field("hidden", &self.hidden)
            .field("requested_model", &self.requested_model)
            .field("requested_tools", &self.requested_tools)
            .field("compatibility", &self.compatibility)
            .field("diagnostic_codes", &self.diagnostic_codes)
            .field("behavior_version", &self.behavior_version)
            .finish()
    }
}

impl ExternalSubagentDefinition {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.logical_id, "external subagent logical")?;
        if self.provenance.is_empty()
            || self.provenance.len() > MAX_PROVENANCE_REFS
            || self
                .provenance
                .iter()
                .any(|item| item.contribution_id.local_id.as_str() != self.logical_id)
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "external subagent provenance",
            ));
        }
        let mut provenance = BTreeSet::new();
        if self.provenance.iter().any(|item| !provenance.insert(item)) {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "external subagent provenance",
            ));
        }
        for (value, label) in [
            (&self.display_name, "external subagent display name"),
            (&self.description, "external subagent description"),
        ] {
            if value.len() > MAX_LABEL_LENGTH || value.chars().any(char::is_control) {
                return Err(ExternalSourceContractError::InvalidText(label));
            }
        }
        if self.prompt.expose().len() > MAX_PROMPT_BYTES
            || (self.prompt.expose().trim().is_empty()
                && !matches!(
                    self.compatibility,
                    ExternalSubagentCompatibilityState::Blocked
                        | ExternalSubagentCompatibilityState::Invalid
                ))
        {
            return Err(ExternalSourceContractError::InvalidText(
                "external subagent prompt",
            ));
        }
        if let ExternalSubagentModelRequest::Exact {
            provider_hint,
            model_name,
        } = &self.requested_model
        {
            if provider_hint.as_ref().is_some_and(|provider| {
                provider.is_empty()
                    || provider.len() > MAX_LABEL_LENGTH
                    || provider.trim() != provider
                    || provider.chars().any(char::is_control)
            }) || model_name.is_empty()
                || model_name.len() > MAX_LABEL_LENGTH
                || model_name.trim() != model_name
                || model_name.chars().any(char::is_control)
            {
                return Err(ExternalSourceContractError::InvalidText(
                    "external subagent model request",
                ));
            }
        }
        if self.requested_tools.selectors.len() > MAX_TOOL_SELECTORS {
            return Err(ExternalSourceContractError::InvalidText(
                "external subagent tool selectors",
            ));
        }
        let mut tool_selectors = BTreeSet::new();
        for selector in &self.requested_tools.selectors {
            let valid_name = |value: &str| {
                !value.is_empty()
                    && value.len() <= MAX_LABEL_LENGTH
                    && value.trim() == value
                    && !value.chars().any(char::is_control)
            };
            if !valid_name(&selector.source_name)
                || selector
                    .canonical_host_name
                    .as_deref()
                    .is_some_and(|name| !valid_name(name))
                || !tool_selectors.insert((
                    selector.source_name.as_str(),
                    selector.canonical_host_name.as_deref(),
                ))
            {
                return Err(ExternalSourceContractError::InvalidText(
                    "external subagent tool selector",
                ));
            }
        }
        if self.diagnostic_codes.len() > MAX_DIAGNOSTIC_CODES {
            return Err(ExternalSourceContractError::InvalidText(
                "external subagent diagnostic codes",
            ));
        }
        let mut diagnostic_codes = BTreeSet::new();
        if self
            .diagnostic_codes
            .iter()
            .any(|code| !is_stable_diagnostic_code(code) || !diagnostic_codes.insert(code.as_str()))
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "external subagent diagnostic code",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalSubagentDiscoveryInput {
    pub context: ExternalSourceContext,
    pub suppressed_sources: BTreeSet<SourceKey>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ExternalSubagentProviderSnapshot {
    pub provider: ExternalSubagentProviderIdentity,
    pub sources: Vec<ExternalSourceRecord>,
    pub definitions: Vec<ExternalSubagentDefinition>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl fmt::Debug for ExternalSubagentProviderSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalSubagentProviderSnapshot")
            .field("provider", &self.provider)
            .field("sources", &self.sources)
            .field("definitions", &self.definitions)
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

impl ExternalSubagentProviderSnapshot {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        let diagnostic_count = self
            .sources
            .iter()
            .fold(self.diagnostics.len(), |total, source| {
                total.saturating_add(source.diagnostics.len())
            });
        if self.sources.len() > MAX_PROVIDER_SOURCES
            || self.definitions.len() > MAX_PROVIDER_DEFINITIONS
            || diagnostic_count > MAX_PROVIDER_DIAGNOSTICS
        {
            return Err(ExternalSourceContractError::InvalidText(
                "external subagent provider snapshot size",
            ));
        }
        let mut sources = BTreeSet::new();
        for source in &self.sources {
            source.validate()?;
            if source.key.provider_id != self.provider.provider_id
                || source.ecosystem_id != self.provider.ecosystem_id
                || !sources.insert(source.key.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "external subagent provider-qualified source",
                ));
            }
        }
        for source in &self.sources {
            for diagnostic in &source.diagnostics {
                validate_provider_diagnostic(diagnostic, &self.provider.provider_id, &sources)?;
            }
        }
        for diagnostic in &self.diagnostics {
            validate_provider_diagnostic(diagnostic, &self.provider.provider_id, &sources)?;
        }
        let mut candidates = BTreeSet::new();
        for definition in &self.definitions {
            definition.validate()?;
            if !candidates.insert(definition.candidate_id.clone())
                || definition.provenance.iter().any(|item| {
                    item.contribution_id.source.provider_id != self.provider.provider_id
                        || !sources.contains(&item.contribution_id.source)
                })
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "provider-qualified external subagent",
                ));
            }
        }
        Ok(())
    }
}

pub trait ExternalSubagentSourceProvider: Send + Sync {
    fn identity(&self) -> ExternalSubagentProviderIdentity;

    fn discover(
        &self,
        input: &ExternalSubagentDiscoveryInput,
    ) -> Result<ExternalSubagentProviderSnapshot, ExternalSourceProviderError>;

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ExternalSubagentActivationState {
    ApprovalRequired,
    Declined,
    Disabled,
    Active,
    Conflict,
    Blocked,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentDiagnosticSummary {
    pub code: String,
    pub blocks_activation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentSummary {
    pub candidate_id: String,
    pub logical_id: String,
    pub display_name: String,
    pub description: String,
    pub provider_label: String,
    pub scope: ExternalSourceScope,
    pub source_keys: Vec<SourceKey>,
    pub source_location_labels: Vec<String>,
    pub source_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_model_label: Option<String>,
    pub effective_tool_labels: Vec<String>,
    pub supports_follow_up: bool,
    pub compatibility_state: ExternalSubagentCompatibilityState,
    pub diagnostics: Vec<ExternalSubagentDiagnosticSummary>,
    pub activation_state: ExternalSubagentActivationState,
    pub decision_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentConflictCandidate {
    pub candidate_id: String,
    pub display_name: String,
    pub source_label: String,
    pub external: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSubagentConflict {
    pub conflict_key: String,
    pub logical_id: String,
    pub candidates: Vec<ExternalSubagentConflictCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_candidate_id: Option<String>,
}

pub fn external_subagent_candidate_id(
    provider_id: &ProviderId,
    logical_id: &str,
    provenance: &[ExternalSubagentProvenanceRef],
) -> ExternalSubagentCandidateId {
    let parts = provenance
        .iter()
        .map(|item| format!("{}:{:?}", item.contribution_id.stable_key(), item.role));
    ExternalSubagentCandidateId::new(format!(
        "external_subagent:{}:{}:{}",
        provider_id,
        logical_id.to_ascii_lowercase(),
        stable_digest(parts)
    ))
    .expect("bounded digest-based candidate id")
}

pub fn external_subagent_approval_key(
    candidate_id: &ExternalSubagentCandidateId,
    behavior_version: &ExternalSubagentBehaviorVersion,
    activation_envelope: &str,
) -> String {
    format!(
        "external_subagent_approval:{}",
        stable_digest([
            candidate_id.as_str(),
            behavior_version.as_str(),
            activation_envelope,
        ])
    )
}

pub fn external_subagent_conflict_key<'a>(
    execution_domain_id: &str,
    workspace_scope: &str,
    logical_id: &str,
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> String {
    let mut candidates = candidates
        .into_iter()
        .map(|(id, version)| format!("{}:{id}{}:{version}", id.len(), version.len()))
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    format!(
        "external_subagent_conflict:{}",
        stable_digest(
            [
                execution_domain_id.to_string(),
                workspace_scope.to_string(),
                logical_id.to_ascii_lowercase(),
            ]
            .into_iter()
            .chain(candidates)
        )
    )
}

fn stable_digest(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        let part = part.as_ref().as_bytes();
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hex::encode(hasher.finalize())
}
