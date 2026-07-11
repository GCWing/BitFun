//! Product-owned plugin package and trust contracts.
//!
//! These contracts identify BitFun-managed packages before an ecosystem
//! adapter or Plugin Runtime Host is selected. Filesystem discovery and trust
//! persistence are concrete service integration responsibilities.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

const PLUGIN_PACKAGE_MANIFEST_SCHEMA_VERSION: u16 = 1;
const PLUGIN_TRUST_STORE_SCHEMA_VERSION: u16 = 1;

const MAX_PACKAGE_ID_LEN: usize = 128;
const MAX_ADAPTER_ID_LEN: usize = 64;
const MAX_PACKAGE_VERSION_LEN: usize = 128;
const MAX_PACKAGE_PATH_LEN: usize = 1024;
const MAX_SOURCE_PATH_LEN: usize = 256;
const MAX_SCOPE_ID_LEN: usize = 256;
const MAX_PACKAGE_FILES: usize = 64;
const MAX_TRUST_RECORDS: usize = 1024;
const SHA256_PREFIX: &str = "sha256:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPackageFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPackageManifest {
    pub schema_version: u16,
    pub id: String,
    pub version: String,
    pub adapter: String,
    pub files: Vec<PluginPackageFile>,
}

impl PluginPackageManifest {
    pub fn parse_json(json: &str) -> Result<Self, PluginSourceContractError> {
        let manifest: Self = serde_json::from_str(json)
            .map_err(|error| PluginSourceContractError::InvalidJson(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), PluginSourceContractError> {
        if self.schema_version != PLUGIN_PACKAGE_MANIFEST_SCHEMA_VERSION {
            return Err(PluginSourceContractError::UnsupportedManifestSchema(
                self.schema_version,
            ));
        }
        validate_package_id(&self.id)?;
        validate_adapter_id(&self.adapter)?;
        if !is_valid_text(&self.version, MAX_PACKAGE_VERSION_LEN) {
            return Err(PluginSourceContractError::InvalidPackageVersion);
        }
        if self.files.is_empty() || self.files.len() > MAX_PACKAGE_FILES {
            return Err(PluginSourceContractError::InvalidPackageFileCount);
        }

        let mut paths = HashSet::new();
        for file in &self.files {
            validate_package_relative_path(&file.path)?;
            validate_sha256(&file.sha256)?;
            if !paths.insert(file.path.as_str()) {
                return Err(PluginSourceContractError::DuplicatePackageFile(
                    file.path.clone(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPackageSourceIdentity {
    pub package_id: String,
    pub version: String,
    pub adapter: String,
    pub source_path: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginPackageTrustLevel {
    Unknown,
    SourceApproved,
    Denied,
    Revoked,
}

impl PluginPackageSourceIdentity {
    pub fn validate(&self) -> Result<(), PluginSourceContractError> {
        validate_package_id(&self.package_id)?;
        validate_adapter_id(&self.adapter)?;
        if !is_valid_text(&self.version, MAX_PACKAGE_VERSION_LEN) {
            return Err(PluginSourceContractError::InvalidPackageVersion);
        }
        if !is_valid_text(&self.source_path, MAX_SOURCE_PATH_LEN) {
            return Err(PluginSourceContractError::InvalidSourcePath);
        }
        validate_sha256(&self.content_hash)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTrustDecision {
    ApproveSource,
    Denied,
    Revoked,
}

impl PluginTrustDecision {
    const fn trust_level(self) -> PluginPackageTrustLevel {
        match self {
            Self::ApproveSource => PluginPackageTrustLevel::SourceApproved,
            Self::Denied => PluginPackageTrustLevel::Denied,
            Self::Revoked => PluginPackageTrustLevel::Revoked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PluginTrustRecord {
    pub project_domain_id: String,
    pub workspace_id: String,
    pub source: PluginPackageSourceIdentity,
    pub trust_level: PluginPackageTrustLevel,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginTrustStore {
    schema_version: u16,
    epoch: u64,
    records: Vec<PluginTrustRecord>,
}

impl PluginTrustStore {
    pub fn new(initial_epoch: u64) -> Self {
        Self {
            schema_version: PLUGIN_TRUST_STORE_SCHEMA_VERSION,
            epoch: initial_epoch,
            records: Vec::new(),
        }
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn validate(&self) -> Result<(), PluginSourceContractError> {
        if self.schema_version != PLUGIN_TRUST_STORE_SCHEMA_VERSION {
            return Err(PluginSourceContractError::UnsupportedTrustStoreSchema(
                self.schema_version,
            ));
        }
        if self.epoch == 0 {
            return Err(PluginSourceContractError::InvalidTrustEpoch);
        }
        if self.records.len() > MAX_TRUST_RECORDS {
            return Err(PluginSourceContractError::TooManyTrustRecords);
        }

        let mut package_scopes = HashSet::new();
        for record in &self.records {
            validate_scope(&record.project_domain_id, &record.workspace_id)?;
            record.source.validate()?;
            if record.trust_level == PluginPackageTrustLevel::Unknown {
                return Err(PluginSourceContractError::UnknownTrustRecord);
            }
            let key = (
                record.project_domain_id.as_str(),
                record.workspace_id.as_str(),
                record.source.package_id.as_str(),
            );
            if !package_scopes.insert(key) {
                return Err(PluginSourceContractError::DuplicateTrustRecord);
            }
        }
        Ok(())
    }

    pub fn trust_level_for(
        &self,
        project_domain_id: &str,
        workspace_id: &str,
        source: &PluginPackageSourceIdentity,
    ) -> PluginPackageTrustLevel {
        self.records
            .iter()
            .find(|record| {
                record.project_domain_id == project_domain_id
                    && record.workspace_id == workspace_id
                    && record.source == *source
            })
            .map(|record| record.trust_level)
            .unwrap_or(PluginPackageTrustLevel::Unknown)
    }

    pub fn apply_decision(
        &mut self,
        project_domain_id: &str,
        workspace_id: &str,
        source: PluginPackageSourceIdentity,
        decision: PluginTrustDecision,
        updated_at_ms: u64,
    ) -> Result<bool, PluginSourceContractError> {
        validate_scope(project_domain_id, workspace_id)?;
        source.validate()?;
        if decision == PluginTrustDecision::Revoked
            && self.trust_level_for(project_domain_id, workspace_id, &source)
                != PluginPackageTrustLevel::SourceApproved
        {
            return Err(PluginSourceContractError::InvalidTrustTransition);
        }

        let mut next = self.clone();
        let trust_level = decision.trust_level();
        let existing_index = next.records.iter().position(|record| {
            record.project_domain_id == project_domain_id
                && record.workspace_id == workspace_id
                && record.source.package_id == source.package_id
        });

        let changed = match existing_index {
            Some(index) => {
                let record = &mut next.records[index];
                if record.source == source && record.trust_level == trust_level {
                    false
                } else {
                    *record = PluginTrustRecord {
                        project_domain_id: project_domain_id.to_string(),
                        workspace_id: workspace_id.to_string(),
                        source,
                        trust_level,
                        updated_at_ms,
                    };
                    true
                }
            }
            None => {
                next.records.push(PluginTrustRecord {
                    project_domain_id: project_domain_id.to_string(),
                    workspace_id: workspace_id.to_string(),
                    source,
                    trust_level,
                    updated_at_ms,
                });
                true
            }
        };

        if !changed {
            return Ok(false);
        }
        next.advance_epoch()?;
        *self = next;
        Ok(true)
    }

    pub fn reconcile_sources(
        &mut self,
        project_domain_id: &str,
        workspace_id: &str,
        current_sources: &[PluginPackageSourceIdentity],
    ) -> Result<bool, PluginSourceContractError> {
        validate_scope(project_domain_id, workspace_id)?;
        for source in current_sources {
            source.validate()?;
        }
        let current = current_sources.iter().collect::<HashSet<_>>();
        let mut next = self.clone();
        let previous_len = next.records.len();
        next.records.retain(|record| {
            record.project_domain_id != project_domain_id
                || record.workspace_id != workspace_id
                || current.contains(&record.source)
        });
        let changed = next.records.len() != previous_len;

        if !changed {
            return Ok(false);
        }
        next.advance_epoch()?;
        *self = next;
        Ok(true)
    }

    fn advance_epoch(&mut self) -> Result<(), PluginSourceContractError> {
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or(PluginSourceContractError::TrustEpochExhausted)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginSourceContractError {
    InvalidJson(String),
    UnsupportedManifestSchema(u16),
    UnsupportedTrustStoreSchema(u16),
    InvalidPackageId,
    InvalidAdapterId,
    InvalidPackageVersion,
    InvalidPackageFileCount,
    InvalidPackagePath(String),
    InvalidSha256(String),
    DuplicatePackageFile(String),
    InvalidSourcePath,
    EmptyScope,
    InvalidScope,
    InvalidTrustEpoch,
    TooManyTrustRecords,
    UnknownTrustRecord,
    DuplicateTrustRecord,
    InvalidTrustTransition,
    TrustEpochExhausted,
}

impl fmt::Display for PluginSourceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid plugin JSON: {message}"),
            Self::UnsupportedManifestSchema(version) => {
                write!(
                    formatter,
                    "unsupported plugin manifest schema version: {version}"
                )
            }
            Self::UnsupportedTrustStoreSchema(version) => {
                write!(
                    formatter,
                    "unsupported plugin trust schema version: {version}"
                )
            }
            Self::InvalidPackageId => write!(formatter, "invalid plugin package id"),
            Self::InvalidAdapterId => write!(formatter, "invalid plugin package adapter id"),
            Self::InvalidPackageVersion => write!(formatter, "invalid plugin package version"),
            Self::InvalidPackageFileCount => write!(formatter, "invalid plugin package file count"),
            Self::InvalidPackagePath(path) => {
                write!(formatter, "invalid plugin package path: {path}")
            }
            Self::InvalidSha256(hash) => write!(formatter, "invalid plugin package sha256: {hash}"),
            Self::DuplicatePackageFile(path) => {
                write!(formatter, "duplicate plugin package file: {path}")
            }
            Self::InvalidSourcePath => write!(formatter, "invalid plugin package source path"),
            Self::EmptyScope => write!(formatter, "plugin trust scope is empty"),
            Self::InvalidScope => write!(formatter, "invalid plugin trust scope"),
            Self::InvalidTrustEpoch => write!(formatter, "plugin trust epoch must be positive"),
            Self::TooManyTrustRecords => write!(formatter, "too many plugin trust records"),
            Self::UnknownTrustRecord => {
                write!(formatter, "persisted plugin trust record cannot be unknown")
            }
            Self::DuplicateTrustRecord => write!(formatter, "duplicate plugin trust record"),
            Self::InvalidTrustTransition => {
                write!(
                    formatter,
                    "only a source-approved plugin package can be revoked"
                )
            }
            Self::TrustEpochExhausted => write!(formatter, "plugin trust epoch exhausted"),
        }
    }
}

impl std::error::Error for PluginSourceContractError {}

fn validate_package_id(id: &str) -> Result<(), PluginSourceContractError> {
    let mut chars = id.chars();
    let starts_valid =
        matches!(chars.next(), Some(ch) if ch.is_ascii_lowercase() || ch.is_ascii_digit());
    let rest_valid = chars
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '-' | '_'));
    if id.len() > MAX_PACKAGE_ID_LEN || !starts_valid || !rest_valid {
        return Err(PluginSourceContractError::InvalidPackageId);
    }
    Ok(())
}

fn validate_adapter_id(id: &str) -> Result<(), PluginSourceContractError> {
    let mut chars = id.chars();
    let starts_valid = matches!(chars.next(), Some(ch) if ch.is_ascii_lowercase());
    let rest_valid = chars
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '-' | '_'));
    if id.len() > MAX_ADAPTER_ID_LEN || !starts_valid || !rest_valid {
        return Err(PluginSourceContractError::InvalidAdapterId);
    }
    Ok(())
}

fn validate_package_relative_path(path: &str) -> Result<(), PluginSourceContractError> {
    let invalid = path.is_empty()
        || path.len() > MAX_PACKAGE_PATH_LEN
        || path
            .chars()
            .any(|ch| ch.is_control() || is_bidi_format_character(ch))
        || path.starts_with('/')
        || path.contains('\\')
        || path.split('/').any(|segment| {
            segment.is_empty() || segment == "." || segment == ".." || segment.contains(':')
        });
    if invalid {
        return Err(PluginSourceContractError::InvalidPackagePath(
            path.to_string(),
        ));
    }
    Ok(())
}

fn validate_sha256(hash: &str) -> Result<(), PluginSourceContractError> {
    let digest = hash.strip_prefix(SHA256_PREFIX).unwrap_or_default();
    if digest.len() != 64
        || !digest
            .chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch))
    {
        return Err(PluginSourceContractError::InvalidSha256(hash.to_string()));
    }
    Ok(())
}

fn validate_scope(
    project_domain_id: &str,
    workspace_id: &str,
) -> Result<(), PluginSourceContractError> {
    if project_domain_id.trim().is_empty() || workspace_id.trim().is_empty() {
        return Err(PluginSourceContractError::EmptyScope);
    }
    if !is_valid_text(project_domain_id, MAX_SCOPE_ID_LEN)
        || !is_valid_text(workspace_id, MAX_SCOPE_ID_LEN)
    {
        return Err(PluginSourceContractError::InvalidScope);
    }
    Ok(())
}

fn is_valid_text(value: &str, max_len: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_len
        && !value
            .chars()
            .any(|ch| ch.is_control() || is_bidi_format_character(ch))
}

fn is_bidi_format_character(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}
