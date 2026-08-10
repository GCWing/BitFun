//! Ecosystem-neutral contracts for named workspace reference directories.
//!
//! Providers translate external configuration into declared local directory
//! paths. A declared path may not exist yet.
//! Product Assembly remains responsible for composing those facts with native
//! workspace related paths and for deciding which product surfaces consume
//! them. A reference never grants filesystem permissions by itself.

use crate::external_sources::{
    validate_id, EcosystemId, ExternalSourceContext, ExternalSourceContractError,
    ExternalSourceDiagnostic, ExternalSourceProviderError, ExternalSourceRecord,
    ExternalSourceScope, ExternalWatchRoot, ProviderId, SourceKey,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

const MAX_REFERENCE_DESCRIPTION_LENGTH: usize = 4096;
const MAX_REFERENCE_COUNT: usize = 1024;
const MAX_PROVIDER_DIAGNOSTICS: usize = 256;
const MAX_SNAPSHOT_DIAGNOSTIC_ENTRIES: usize = MAX_PROVIDER_DIAGNOSTICS * 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalWorkspaceReferenceProviderIdentity {
    pub provider_id: ProviderId,
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
}

impl ExternalWorkspaceReferenceProviderIdentity {
    pub fn new(
        provider_id: impl Into<String>,
        ecosystem_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, ExternalSourceContractError> {
        let display_name = display_name.into();
        validate_reference_text(&display_name, "workspace reference provider display name")?;
        Ok(Self {
            provider_id: ProviderId::new(provider_id)?,
            ecosystem_id: EcosystemId::new(ecosystem_id)?,
            display_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalWorkspaceReferenceDefinition {
    pub source: SourceKey,
    pub alias: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    pub content_version: String,
}

impl ExternalWorkspaceReferenceDefinition {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.alias, "workspace reference alias")?;
        validate_id(&self.content_version, "workspace reference content version")?;
        if !self.path.is_absolute() {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "workspace reference path",
            ));
        }
        if let Some(description) = &self.description {
            validate_reference_text(description, "workspace reference description")?;
        }
        Ok(())
    }

    pub fn stable_key(&self) -> String {
        format!(
            "{}{}:{}",
            self.source.stable_key(),
            self.alias.len(),
            self.alias
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalWorkspaceReferenceProviderSnapshot {
    pub provider: ExternalWorkspaceReferenceProviderIdentity,
    pub sources: Vec<ExternalSourceRecord>,
    pub references: Vec<ExternalWorkspaceReferenceDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

impl ExternalWorkspaceReferenceProviderSnapshot {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        let diagnostic_entries = self
            .sources
            .iter()
            .fold(self.diagnostics.len(), |count, source| {
                count.saturating_add(source.diagnostics.len())
            });
        if self.sources.len() > MAX_REFERENCE_COUNT
            || self.references.len() > MAX_REFERENCE_COUNT
            || self.diagnostics.len() > MAX_PROVIDER_DIAGNOSTICS
            || diagnostic_entries > MAX_SNAPSHOT_DIAGNOSTIC_ENTRIES
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "workspace reference provider snapshot size",
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
                    "workspace reference provider-qualified source",
                ));
            }
        }

        let mut aliases = BTreeSet::new();
        for reference in &self.references {
            reference.validate()?;
            if reference.source.provider_id != self.provider.provider_id
                || !source_keys.contains(&reference.source)
                || !aliases.insert(reference.alias.as_str())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "workspace reference provider-qualified definition",
                ));
            }
        }
        Ok(())
    }
}

pub trait ExternalWorkspaceReferenceSourceProvider: Send + Sync {
    fn identity(&self) -> ExternalWorkspaceReferenceProviderIdentity;

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalWorkspaceReferenceProviderSnapshot, ExternalSourceProviderError>;

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceReferenceOrigin {
    Native,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceReferenceCatalogEntry {
    pub stable_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    pub origin: WorkspaceReferenceOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem_id: Option<EcosystemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scope: Option<ExternalSourceScope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceReferenceSnapshot {
    pub generation: u64,
    pub discovery_pending: bool,
    pub references: Vec<WorkspaceReferenceCatalogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

fn validate_reference_text(
    value: &str,
    label: &'static str,
) -> Result<(), ExternalSourceContractError> {
    if value.is_empty()
        || value.len() > MAX_REFERENCE_DESCRIPTION_LENGTH
        || value.chars().any(char::is_control)
    {
        return Err(ExternalSourceContractError::InvalidText(label));
    }
    Ok(())
}
