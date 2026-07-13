//! Product-owned plugin package and trust contracts.
//!
//! These contracts identify BitFun-managed packages before an ecosystem
//! adapter or Plugin Runtime Host is selected. Filesystem discovery and trust
//! persistence are concrete service integration responsibilities.

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

const PLUGIN_PACKAGE_MANIFEST_SCHEMA_VERSION: u16 = 1;
const PLUGIN_TRUST_STORE_SCHEMA_VERSION: u16 = 2;
const LEGACY_PLUGIN_TRUST_STORE_SCHEMA_VERSION: u16 = 1;

const MAX_PACKAGE_ID_LEN: usize = 128;
const MAX_ADAPTER_ID_LEN: usize = 64;
const MAX_PACKAGE_VERSION_LEN: usize = 128;
const MAX_PACKAGE_PATH_LEN: usize = 1024;
const MAX_SOURCE_PATH_LEN: usize = 256;
const MAX_SCOPE_ID_LEN: usize = 256;
const MAX_PACKAGE_FILES: usize = 64;
const MAX_PACKAGE_FILE_BYTES: usize = 1024 * 1024;
const MAX_PACKAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TRUST_RECORDS: usize = 1024;
const MAX_ACTIVATION_RECORDS: usize = 1024;
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

    pub fn validate(&self) -> Result<(), PluginSourceContractError> {
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

    pub fn content_hash(&self) -> Result<String, PluginSourceContractError> {
        self.validate()?;
        let mut files = self.files.iter().collect::<Vec<_>>();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let mut hasher = Sha256::new();
        hasher.update(self.schema_version.to_le_bytes());
        hasher.update([0]);
        hasher.update(self.id.as_bytes());
        hasher.update([0]);
        hasher.update(self.version.as_bytes());
        hasher.update([0]);
        hasher.update(self.adapter.as_bytes());
        for file in files {
            hasher.update([0]);
            hasher.update(file.path.as_bytes());
            hasher.update([0]);
            hasher.update(file.sha256.as_bytes());
        }
        Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
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

/// Fixed package content passed from managed source IO to an ecosystem adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPackageInput {
    manifest: PluginPackageManifest,
    source: PluginPackageSourceIdentity,
    files: BTreeMap<String, Vec<u8>>,
}

impl PluginPackageInput {
    pub fn new(
        manifest: PluginPackageManifest,
        source: PluginPackageSourceIdentity,
        files: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, PluginSourceContractError> {
        manifest.validate()?;
        source.validate()?;
        if source.package_id != manifest.id
            || source.version != manifest.version
            || source.adapter != manifest.adapter
            || source.content_hash != manifest.content_hash()?
        {
            return Err(PluginSourceContractError::PackageIdentityMismatch);
        }
        let declared_paths = manifest
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<BTreeSet<_>>();
        let provided_paths = files.keys().map(String::as_str).collect::<BTreeSet<_>>();
        if declared_paths != provided_paths {
            return Err(PluginSourceContractError::PackageFileSetMismatch);
        }
        let mut package_bytes = 0_usize;
        for file in &manifest.files {
            let bytes = &files[&file.path];
            if bytes.len() > MAX_PACKAGE_FILE_BYTES {
                return Err(PluginSourceContractError::PackageFileTooLarge(
                    file.path.clone(),
                ));
            }
            package_bytes = package_bytes
                .checked_add(bytes.len())
                .ok_or(PluginSourceContractError::PackageContentTooLarge)?;
            if package_bytes > MAX_PACKAGE_BYTES {
                return Err(PluginSourceContractError::PackageContentTooLarge);
            }
            let actual_hash = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
            if actual_hash != file.sha256 {
                return Err(PluginSourceContractError::PackageFileHashMismatch(
                    file.path.clone(),
                ));
            }
        }
        Ok(Self {
            manifest,
            source,
            files,
        })
    }

    pub fn into_parts(
        self,
    ) -> (
        PluginPackageManifest,
        PluginPackageSourceIdentity,
        BTreeMap<String, Vec<u8>>,
    ) {
        (self.manifest, self.source, self.files)
    }
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
struct PluginActivationRecord {
    project_domain_id: String,
    workspace_id: String,
    source: PluginPackageSourceIdentity,
    activation_epoch: u64,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginTrustStore {
    schema_version: u16,
    epoch: u64,
    records: Vec<PluginTrustRecord>,
    activation_epoch: u64,
    activation_records: Vec<PluginActivationRecord>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PersistedPluginTrustStore {
    V2(PersistedPluginTrustStoreV2),
    V1(PersistedPluginTrustStoreV1),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedPluginTrustStoreV1 {
    schema_version: u16,
    epoch: u64,
    records: Vec<PluginTrustRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedPluginTrustStoreV2 {
    schema_version: u16,
    epoch: u64,
    records: Vec<PluginTrustRecord>,
    activation_epoch: u64,
    activation_records: Vec<PluginActivationRecord>,
}

impl<'de> Deserialize<'de> for PluginTrustStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match PersistedPluginTrustStore::deserialize(deserializer)? {
            PersistedPluginTrustStore::V1(persisted)
                if persisted.schema_version == LEGACY_PLUGIN_TRUST_STORE_SCHEMA_VERSION =>
            {
                Ok(Self {
                    schema_version: PLUGIN_TRUST_STORE_SCHEMA_VERSION,
                    epoch: persisted.epoch,
                    records: persisted.records,
                    activation_epoch: persisted.epoch,
                    activation_records: Vec::new(),
                })
            }
            PersistedPluginTrustStore::V1(persisted)
                if persisted.schema_version == PLUGIN_TRUST_STORE_SCHEMA_VERSION =>
            {
                Err(serde::de::Error::custom(
                    "schema-v2 plugin trust store requires activation fields",
                ))
            }
            PersistedPluginTrustStore::V1(persisted) => Ok(Self {
                schema_version: persisted.schema_version,
                epoch: persisted.epoch,
                records: persisted.records,
                activation_epoch: 0,
                activation_records: Vec::new(),
            }),
            PersistedPluginTrustStore::V2(persisted)
                if persisted.schema_version == LEGACY_PLUGIN_TRUST_STORE_SCHEMA_VERSION =>
            {
                Err(serde::de::Error::custom(
                    "schema-v1 plugin trust store cannot contain activation fields",
                ))
            }
            PersistedPluginTrustStore::V2(persisted) => Ok(Self {
                schema_version: persisted.schema_version,
                epoch: persisted.epoch,
                records: persisted.records,
                activation_epoch: persisted.activation_epoch,
                activation_records: persisted.activation_records,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginActivationAuthority {
    project_domain_id: String,
    workspace_id: String,
    source: PluginPackageSourceIdentity,
    activation_epoch: u64,
}

impl PluginActivationAuthority {
    pub const fn activation_epoch(&self) -> u64 {
        self.activation_epoch
    }

    pub fn into_parts(self) -> (String, String, PluginPackageSourceIdentity, u64) {
        (
            self.project_domain_id,
            self.workspace_id,
            self.source,
            self.activation_epoch,
        )
    }
}

impl PluginTrustStore {
    pub fn new(initial_epoch: u64) -> Self {
        Self {
            schema_version: PLUGIN_TRUST_STORE_SCHEMA_VERSION,
            epoch: initial_epoch,
            records: Vec::new(),
            activation_epoch: initial_epoch,
            activation_records: Vec::new(),
        }
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub const fn activation_epoch(&self) -> u64 {
        self.activation_epoch
    }

    pub fn activation_sources(
        &self,
        project_domain_id: &str,
        workspace_id: &str,
    ) -> Vec<PluginPackageSourceIdentity> {
        let mut sources = self
            .activation_records
            .iter()
            .filter(|record| {
                record.project_domain_id == project_domain_id && record.workspace_id == workspace_id
            })
            .map(|record| record.source.clone())
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| {
            left.package_id
                .cmp(&right.package_id)
                .then_with(|| left.version.cmp(&right.version))
                .then_with(|| left.adapter.cmp(&right.adapter))
                .then_with(|| left.source_path.cmp(&right.source_path))
                .then_with(|| left.content_hash.cmp(&right.content_hash))
        });
        sources.dedup();
        sources
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
        if self.activation_epoch == 0 {
            return Err(PluginSourceContractError::InvalidActivationEpoch);
        }
        if self.records.len() > MAX_TRUST_RECORDS {
            return Err(PluginSourceContractError::TooManyTrustRecords);
        }
        if self.activation_records.len() > MAX_ACTIVATION_RECORDS {
            return Err(PluginSourceContractError::TooManyActivationRecords);
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

        let mut activation_scopes = HashSet::new();
        for record in &self.activation_records {
            validate_scope(&record.project_domain_id, &record.workspace_id)?;
            record.source.validate()?;
            if record.activation_epoch == 0 || record.activation_epoch > self.activation_epoch {
                return Err(PluginSourceContractError::InvalidActivationEpoch);
            }
            let key = (
                record.project_domain_id.as_str(),
                record.workspace_id.as_str(),
                &record.source,
            );
            if !activation_scopes.insert(key) {
                return Err(PluginSourceContractError::DuplicateActivationRecord);
            }
            let Some(trust_record) = self.records.iter().find(|trust_record| {
                trust_record.project_domain_id == record.project_domain_id
                    && trust_record.workspace_id == record.workspace_id
                    && trust_record.source.package_id == record.source.package_id
            }) else {
                return Err(PluginSourceContractError::UnknownActivationRecord);
            };
            if trust_record.source != record.source
                || trust_record.trust_level != PluginPackageTrustLevel::SourceApproved
            {
                return Err(PluginSourceContractError::StaleActivationRecord);
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

    pub fn is_activated(
        &self,
        project_domain_id: &str,
        workspace_id: &str,
        source: &PluginPackageSourceIdentity,
    ) -> bool {
        self.trust_level_for(project_domain_id, workspace_id, source)
            == PluginPackageTrustLevel::SourceApproved
            && self
                .activation_record(project_domain_id, workspace_id, source)
                .is_some()
    }

    pub fn activation_authority(
        &self,
        project_domain_id: &str,
        workspace_id: &str,
        source: &PluginPackageSourceIdentity,
    ) -> Result<PluginActivationAuthority, PluginSourceContractError> {
        validate_scope(project_domain_id, workspace_id)?;
        if self.trust_level_for(project_domain_id, workspace_id, source)
            != PluginPackageTrustLevel::SourceApproved
        {
            return Err(PluginSourceContractError::PluginPackageNotActivated);
        }
        let record = self
            .activation_record(project_domain_id, workspace_id, source)
            .ok_or(PluginSourceContractError::PluginPackageNotActivated)?;
        Ok(PluginActivationAuthority {
            project_domain_id: project_domain_id.to_string(),
            workspace_id: workspace_id.to_string(),
            source: source.clone(),
            activation_epoch: record.activation_epoch,
        })
    }

    pub fn is_activation_current(&self, authority: &PluginActivationAuthority) -> bool {
        self.trust_level_for(
            &authority.project_domain_id,
            &authority.workspace_id,
            &authority.source,
        ) == PluginPackageTrustLevel::SourceApproved
            && self
                .activation_record(
                    &authority.project_domain_id,
                    &authority.workspace_id,
                    &authority.source,
                )
                .is_some_and(|record| record.activation_epoch == authority.activation_epoch)
    }

    pub fn activate(
        &mut self,
        project_domain_id: &str,
        workspace_id: &str,
        source: PluginPackageSourceIdentity,
        updated_at_ms: u64,
    ) -> Result<bool, PluginSourceContractError> {
        validate_scope(project_domain_id, workspace_id)?;
        source.validate()?;
        if self.trust_level_for(project_domain_id, workspace_id, &source)
            != PluginPackageTrustLevel::SourceApproved
        {
            return Err(PluginSourceContractError::ActivationRequiresSourceApproval);
        }

        if self.activation_records.iter().any(|record| {
            record.project_domain_id == project_domain_id
                && record.workspace_id == workspace_id
                && record.source == source
        }) {
            return Ok(false);
        }

        let mut next = self.clone();
        if next.activation_records.len() >= MAX_ACTIVATION_RECORDS {
            return Err(PluginSourceContractError::TooManyActivationRecords);
        }
        next.advance_activation_epoch()?;
        next.activation_records.push(PluginActivationRecord {
            project_domain_id: project_domain_id.to_string(),
            workspace_id: workspace_id.to_string(),
            source,
            activation_epoch: next.activation_epoch,
            updated_at_ms,
        });
        *self = next;
        Ok(true)
    }

    pub fn clear_activation_record(
        &mut self,
        project_domain_id: &str,
        workspace_id: &str,
        package_id: &str,
        expected_activation_epoch: Option<u64>,
    ) -> Result<Option<PluginPackageSourceIdentity>, PluginSourceContractError> {
        validate_scope(project_domain_id, workspace_id)?;
        validate_package_id(package_id)?;
        let Some(index) = self.activation_records.iter().position(|record| {
            record.project_domain_id == project_domain_id
                && record.workspace_id == workspace_id
                && record.source.package_id == package_id
        }) else {
            return Ok(None);
        };
        if expected_activation_epoch
            .is_some_and(|expected| self.activation_records[index].activation_epoch != expected)
        {
            return Ok(None);
        }

        let mut next = self.clone();
        let removed_source = next.activation_records[index].source.clone();
        next.activation_records.remove(index);
        next.advance_activation_epoch()?;
        *self = next;
        Ok(Some(removed_source))
    }

    fn activation_record(
        &self,
        project_domain_id: &str,
        workspace_id: &str,
        source: &PluginPackageSourceIdentity,
    ) -> Option<&PluginActivationRecord> {
        self.activation_records.iter().find(|record| {
            record.project_domain_id == project_domain_id
                && record.workspace_id == workspace_id
                && record.source == *source
        })
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
                        source: source.clone(),
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
                    source: source.clone(),
                    trust_level,
                    updated_at_ms,
                });
                true
            }
        };

        if !changed {
            return Ok(false);
        }
        let previous_activation_len = next.activation_records.len();
        next.activation_records.retain(|record| {
            record.project_domain_id != project_domain_id
                || record.workspace_id != workspace_id
                || record.source.package_id != source.package_id
                || (record.source == source
                    && trust_level == PluginPackageTrustLevel::SourceApproved)
        });
        let activation_changed = next.activation_records.len() != previous_activation_len;
        next.advance_epoch()?;
        if activation_changed {
            next.advance_activation_epoch()?;
        }
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
        let current_package_ids = current_sources
            .iter()
            .map(|source| source.package_id.as_str())
            .collect::<HashSet<_>>();
        let mut next = self.clone();
        let previous_len = next.records.len();
        next.records.retain(|record| {
            record.project_domain_id != project_domain_id
                || record.workspace_id != workspace_id
                || !current_package_ids.contains(record.source.package_id.as_str())
                || current.contains(&record.source)
        });
        let changed = next.records.len() != previous_len;

        if !changed {
            return Ok(false);
        }
        let previous_activation_len = next.activation_records.len();
        next.activation_records.retain(|record| {
            record.project_domain_id != project_domain_id
                || record.workspace_id != workspace_id
                || !current_package_ids.contains(record.source.package_id.as_str())
                || current.contains(&record.source)
        });
        let activation_changed = next.activation_records.len() != previous_activation_len;
        next.advance_epoch()?;
        if activation_changed {
            next.advance_activation_epoch()?;
        }
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

    fn advance_activation_epoch(&mut self) -> Result<(), PluginSourceContractError> {
        self.activation_epoch = self
            .activation_epoch
            .checked_add(1)
            .ok_or(PluginSourceContractError::ActivationEpochExhausted)?;
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
    PackageIdentityMismatch,
    PackageFileSetMismatch,
    PackageFileTooLarge(String),
    PackageContentTooLarge,
    PackageFileHashMismatch(String),
    InvalidSourcePath,
    EmptyScope,
    InvalidScope,
    InvalidTrustEpoch,
    InvalidActivationEpoch,
    TooManyTrustRecords,
    TooManyActivationRecords,
    UnknownTrustRecord,
    UnknownActivationRecord,
    StaleActivationRecord,
    DuplicateTrustRecord,
    DuplicateActivationRecord,
    InvalidTrustTransition,
    ActivationRequiresSourceApproval,
    PluginPackageNotActivated,
    TrustEpochExhausted,
    ActivationEpochExhausted,
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
            Self::PackageIdentityMismatch => {
                write!(
                    formatter,
                    "plugin package input identity does not match its manifest"
                )
            }
            Self::PackageFileSetMismatch => {
                write!(
                    formatter,
                    "plugin package input files do not match its manifest"
                )
            }
            Self::PackageFileTooLarge(path) => {
                write!(formatter, "plugin package input file is too large: {path}")
            }
            Self::PackageContentTooLarge => {
                write!(formatter, "plugin package input content is too large")
            }
            Self::PackageFileHashMismatch(path) => {
                write!(
                    formatter,
                    "plugin package input file hash does not match: {path}"
                )
            }
            Self::InvalidSourcePath => write!(formatter, "invalid plugin package source path"),
            Self::EmptyScope => write!(formatter, "plugin trust scope is empty"),
            Self::InvalidScope => write!(formatter, "invalid plugin trust scope"),
            Self::InvalidTrustEpoch => write!(formatter, "plugin trust epoch must be positive"),
            Self::InvalidActivationEpoch => {
                write!(formatter, "plugin activation epoch must be positive")
            }
            Self::TooManyTrustRecords => write!(formatter, "too many plugin trust records"),
            Self::TooManyActivationRecords => {
                write!(formatter, "too many plugin activation records")
            }
            Self::UnknownTrustRecord => {
                write!(formatter, "persisted plugin trust record cannot be unknown")
            }
            Self::UnknownActivationRecord => {
                write!(
                    formatter,
                    "persisted plugin activation record has no trust record"
                )
            }
            Self::StaleActivationRecord => {
                write!(
                    formatter,
                    "persisted plugin activation record is not source-approved"
                )
            }
            Self::DuplicateTrustRecord => write!(formatter, "duplicate plugin trust record"),
            Self::DuplicateActivationRecord => {
                write!(formatter, "duplicate plugin activation record")
            }
            Self::InvalidTrustTransition => {
                write!(
                    formatter,
                    "only a source-approved plugin package can be revoked"
                )
            }
            Self::ActivationRequiresSourceApproval => {
                write!(
                    formatter,
                    "only a source-approved plugin package can be activated"
                )
            }
            Self::PluginPackageNotActivated => {
                write!(
                    formatter,
                    "plugin package input is not activated in this scope"
                )
            }
            Self::TrustEpochExhausted => write!(formatter, "plugin trust epoch exhausted"),
            Self::ActivationEpochExhausted => {
                write!(formatter, "plugin activation epoch exhausted")
            }
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
