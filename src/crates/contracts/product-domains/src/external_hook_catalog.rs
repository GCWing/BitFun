//! Stable, runtime-free contracts for inspecting external AI application Hooks.
//!
//! Catalog entries intentionally contain summaries only. Handler source,
//! command arguments, request bodies, environment variables, and credentials
//! are not part of this contract and must remain inside the source adapter.

use crate::external_hook_contributions::ExternalHookPoint;
use crate::external_sources::{
    validate_id, EcosystemId, ExternalSourceAssetKind, ExternalSourceContext,
    ExternalSourceContractError, ExternalSourceDiagnostic, ExternalSourceHealth,
    ExternalSourceProviderError, ExternalSourceScope, ProviderId, SourceKey,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const EXTERNAL_HOOK_CATALOG_SCHEMA_V1: u32 = 1;
const MAX_SUMMARY_LENGTH: usize = 512;
const MAX_PROVIDER_SOURCES: usize = 2048;
const MAX_PROVIDER_ENTRIES: usize = 2048;
const MAX_PROVIDER_DIAGNOSTICS: usize = 4096;
const MAX_SOURCE_DIAGNOSTICS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalHookSourceKind {
    Settings,
    PluginFile,
    PackageDeclaration,
    HooksFile,
    InlineConfiguration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalHookHandlerKind {
    Function,
    Command,
    Http,
    McpTool,
    Prompt,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalHookProjectionStatus {
    Mapped,
    NativeOnly,
    Opaque,
}

/// Best-effort activation state reported by the native product's static
/// configuration. `Unknown` is intentional: static inspection must not claim
/// that a handler is enabled, trusted, or executable when that requires the
/// native runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalHookNativeActivation {
    Disabled,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ExternalHookMatcherSummary {
    Any,
    Pattern { display: String },
    Dynamic,
    Unavailable,
}

impl ExternalHookMatcherSummary {
    fn validate(&self) -> Result<(), ExternalSourceContractError> {
        if let Self::Pattern { display } = self {
            validate_summary(display, "Hook matcher summary")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookMapping {
    pub hook_point: ExternalHookPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookProviderIdentity {
    pub provider_id: ProviderId,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
}

impl ExternalHookProviderIdentity {
    pub fn new(
        provider_id: impl Into<String>,
        ecosystem_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        let identity = Self {
            provider_id: ProviderId::new(provider_id)?,
            ecosystem_id: EcosystemId::new(ecosystem_id)?,
            display_name: display_name.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_summary(&self.display_name, "Hook provider display name")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookSource {
    pub key: SourceKey,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
    pub source_kind: ExternalHookSourceKind,
    pub scope: ExternalSourceScope,
    /// A user-recognizable, secret-free location. It may be workspace-relative.
    pub location_hint: String,
    pub health: ExternalSourceHealth,
    pub content_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl ExternalHookSource {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_summary(&self.display_name, "Hook source display name")?;
        validate_summary(&self.location_hint, "Hook source location")?;
        validate_id(&self.content_version, "Hook source content version")?;
        if self.diagnostics.len() > MAX_SOURCE_DIAGNOSTICS {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "Hook source diagnostic count",
            ));
        }
        for diagnostic in &self.diagnostics {
            validate_hook_diagnostic(diagnostic)?;
            if diagnostic
                .source
                .as_ref()
                .is_some_and(|source| source != &self.key)
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "Hook source diagnostic identity",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookCatalogEntry {
    pub stable_key: String,
    pub source: SourceKey,
    pub native_event: String,
    pub matcher: ExternalHookMatcherSummary,
    pub handler_kind: ExternalHookHandlerKind,
    pub projection_status: ExternalHookProjectionStatus,
    pub native_activation: ExternalHookNativeActivation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<ExternalHookMapping>,
    pub content_version: String,
}

impl ExternalHookCatalogEntry {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.stable_key, "Hook catalog entry")?;
        validate_id(&self.native_event, "native Hook event")?;
        validate_id(&self.content_version, "Hook entry content version")?;
        self.matcher.validate()?;
        match (self.projection_status, &self.mapping) {
            (ExternalHookProjectionStatus::Mapped, Some(_))
            | (ExternalHookProjectionStatus::NativeOnly, None)
            | (ExternalHookProjectionStatus::Opaque, None) => Ok(()),
            _ => Err(ExternalSourceContractError::InvalidIdentifier(
                "Hook projection mapping",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookProviderSnapshot {
    pub provider: ExternalHookProviderIdentity,
    /// Deterministic adapter order. Product surfaces preserve this order.
    pub sources: Vec<ExternalHookSource>,
    /// Deterministic adapter order across the provider's sources.
    pub entries: Vec<ExternalHookCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl ExternalHookProviderSnapshot {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        self.provider.validate()?;
        if self.sources.len() > MAX_PROVIDER_SOURCES
            || self.entries.len() > MAX_PROVIDER_ENTRIES
            || self.diagnostics.len() > MAX_PROVIDER_DIAGNOSTICS
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "Hook provider snapshot size",
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
                    "provider-qualified Hook source",
                ));
            }
        }

        let mut entry_keys = BTreeSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if entry.source.provider_id != self.provider.provider_id
                || !source_keys.contains(&entry.source)
                || !entry_keys.insert(entry.stable_key.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "provider-qualified Hook entry",
                ));
            }
        }
        for diagnostic in &self.diagnostics {
            validate_hook_diagnostic(diagnostic)?;
            if diagnostic
                .source
                .as_ref()
                .is_some_and(|source| !source_keys.contains(source))
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "Hook provider diagnostic identity",
                ));
            }
        }
        Ok(())
    }
}

/// Capability-specific, runtime-free source provider implemented by an
/// ecosystem adapter. Discovery may read configuration files but must never
/// import, install, initialize, or invoke Hook handlers.
pub trait ExternalHookSourceProvider: Send + Sync {
    fn identity(&self) -> ExternalHookProviderIdentity;

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalHookProviderSnapshot, ExternalSourceProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookCatalogSnapshotV1 {
    pub schema_version: u32,
    pub discovery_pending: bool,
    /// Provider registration order. Product surfaces use these shared display
    /// facts instead of branching on ecosystem ids.
    pub providers: Vec<ExternalHookProviderIdentity>,
    /// Provider order followed by deterministic adapter source order.
    pub sources: Vec<ExternalHookSource>,
    /// Provider order followed by deterministic adapter Hook order.
    pub entries: Vec<ExternalHookCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale_provider_ids: Vec<ProviderId>,
    /// Providers whose latest discovery failed before any valid snapshot was
    /// available. Error details remain in the shared diagnostic collection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_provider_ids: Vec<ProviderId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl Default for ExternalHookCatalogSnapshotV1 {
    fn default() -> Self {
        Self {
            schema_version: EXTERNAL_HOOK_CATALOG_SCHEMA_V1,
            discovery_pending: true,
            providers: Vec::new(),
            sources: Vec::new(),
            entries: Vec::new(),
            stale_provider_ids: Vec::new(),
            failed_provider_ids: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

fn validate_summary(value: &str, label: &'static str) -> Result<(), ExternalSourceContractError> {
    if value.is_empty() || value.len() > MAX_SUMMARY_LENGTH || value.chars().any(char::is_control) {
        return Err(ExternalSourceContractError::InvalidText(label));
    }
    Ok(())
}

fn validate_hook_diagnostic(
    diagnostic: &ExternalSourceDiagnostic,
) -> Result<(), ExternalSourceContractError> {
    if diagnostic.asset_kind != ExternalSourceAssetKind::Hook {
        return Err(ExternalSourceContractError::InvalidIdentifier(
            "Hook diagnostic asset kind",
        ));
    }
    validate_id(&diagnostic.code, "Hook diagnostic code")?;
    validate_summary(&diagnostic.message, "Hook diagnostic message")
}
