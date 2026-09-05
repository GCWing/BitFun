//! Local-only contracts for explicitly importing compatible external command Hooks.
//!
//! The public DTOs are consumed by local CLI/Desktop surfaces. Prepared types
//! cross only the adapter-to-assembly port and deliberately redact commands and
//! asset bytes from `Debug` output.

use crate::external_hook_catalog::{ExternalHookCatalogSnapshotV1, ExternalHookSource};
use crate::external_sources::{
    validate_id, ExternalSourceContractError, ExternalSourceDiagnostic, ExternalSourceScope,
    SourceKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};

pub const EXTERNAL_HOOK_IMPORT_SCHEMA_V1: u32 = 1;
pub const MAX_EXTERNAL_HOOK_IMPORT_HANDLERS: usize = 2048;
pub const MAX_EXTERNAL_HOOK_IMPORT_SKIPPED_REASONS: usize = 256;
pub const MAX_EXTERNAL_HOOK_IMPORT_ASSETS: usize = 256;
pub const MAX_EXTERNAL_HOOK_IMPORT_ASSET_BYTES: usize = 1024 * 1024;
pub const MAX_EXTERNAL_HOOK_IMPORT_TOTAL_ASSET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EXTERNAL_HOOK_IMPORT_ASSET_DEPTH: usize = 8;
pub const MANAGED_HOOK_ROOT_PLACEHOLDER: &str = "__OPENBITFUN_MANAGED_HOOK_ROOT__";

const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_MATCHER_BYTES: usize = 512;
const MAX_TEXT_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalHookImportDispositionV1 {
    Import,
    Update,
    Unchanged,
    Unavailable,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExternalHookImportDependencyV1 {
    Managed { relative_path: String },
    External { location: String },
}

impl fmt::Debug for ExternalHookImportDependencyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Managed { .. } => formatter.write_str("Managed { path: <redacted> }"),
            Self::External { .. } => formatter.write_str("External { location: <redacted> }"),
        }
    }
}

impl ExternalHookImportDependencyV1 {
    fn validate(&self) -> Result<(), ExternalSourceContractError> {
        let value = match self {
            Self::Managed { relative_path } => relative_path,
            Self::External { location } => location,
        };
        validate_bounded_text(value, MAX_TEXT_BYTES, "Hook import dependency")
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookImportHandlerV1 {
    pub stable_key: String,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_windows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<ExternalHookImportDependencyV1>,
}

impl fmt::Debug for ExternalHookImportHandlerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalHookImportHandlerV1")
            .field("stable_key", &self.stable_key)
            .field("event", &self.event)
            .field("matcher", &self.matcher)
            .field("command", &"<redacted>")
            .field(
                "command_windows",
                &self.command_windows.as_ref().map(|_| "<redacted>"),
            )
            .field("timeout_seconds", &self.timeout_seconds)
            .field(
                "status_message",
                &self.status_message.as_ref().map(|_| "<redacted>"),
            )
            .field("dependency_count", &self.dependencies.len())
            .finish()
    }
}

impl ExternalHookImportHandlerV1 {
    fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.stable_key, "Hook import handler")?;
        validate_id(&self.event, "Hook import event")?;
        if let Some(matcher) = &self.matcher {
            validate_bounded_text(matcher, MAX_MATCHER_BYTES, "Hook import matcher")?;
        }
        validate_bounded_text(&self.command, MAX_COMMAND_BYTES, "Hook import command")?;
        if let Some(command) = &self.command_windows {
            validate_bounded_text(command, MAX_COMMAND_BYTES, "Hook import Windows command")?;
        }
        if let Some(status) = &self.status_message {
            validate_bounded_text(status, MAX_TEXT_BYTES, "Hook import status")?;
        }
        if self.dependencies.len() > MAX_EXTERNAL_HOOK_IMPORT_ASSETS {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "Hook import dependency count",
            ));
        }
        for dependency in &self.dependencies {
            dependency.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookImportSkippedV1 {
    pub reason_code: String,
    pub count: u32,
}

impl ExternalHookImportSkippedV1 {
    fn validate(&self) -> Result<(), ExternalSourceContractError> {
        validate_id(&self.reason_code, "Hook import skip reason")?;
        if self.count == 0 {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "Hook import skip count",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookImportPlanV1 {
    pub schema_version: u32,
    pub source: ExternalHookSource,
    pub disposition: ExternalHookImportDispositionV1,
    pub behavior_version: String,
    pub handlers: Vec<ExternalHookImportHandlerV1>,
    pub skipped: Vec<ExternalHookImportSkippedV1>,
    pub plan_fingerprint: String,
}

impl fmt::Debug for ExternalHookImportPlanV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalHookImportPlanV1")
            .field("schema_version", &self.schema_version)
            .field("source", &self.source.key)
            .field("disposition", &self.disposition)
            .field("behavior_version", &self.behavior_version)
            .field("handler_count", &self.handlers.len())
            .field("skipped", &self.skipped)
            .field("plan_fingerprint", &self.plan_fingerprint)
            .finish()
    }
}

impl ExternalHookImportPlanV1 {
    pub fn validate(&self) -> Result<(), ExternalSourceContractError> {
        if self.schema_version != EXTERNAL_HOOK_IMPORT_SCHEMA_V1 {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "Hook import schema",
            ));
        }
        self.source.validate()?;
        validate_id(&self.behavior_version, "Hook import behavior version")?;
        validate_id(&self.plan_fingerprint, "Hook import plan fingerprint")?;
        if self.handlers.len() > MAX_EXTERNAL_HOOK_IMPORT_HANDLERS
            || self.skipped.len() > MAX_EXTERNAL_HOOK_IMPORT_SKIPPED_REASONS
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "Hook import plan size",
            ));
        }
        let mut keys = BTreeSet::new();
        for handler in &self.handlers {
            handler.validate()?;
            if !keys.insert(&handler.stable_key) {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "Hook import handler identity",
                ));
            }
        }
        for skipped in &self.skipped {
            skipped.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookImportApplyRequestV1 {
    pub schema_version: u32,
    pub source: SourceKey,
    pub plan_fingerprint: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExternalHookImportApplyOutcomeV1 {
    Applied {
        snapshot: ExternalHookImportSnapshotV1,
    },
    Unchanged {
        snapshot: ExternalHookImportSnapshotV1,
    },
    Stale {
        refreshed_plan: ExternalHookImportPlanV1,
    },
}

impl fmt::Debug for ExternalHookImportApplyOutcomeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Applied { snapshot } => formatter
                .debug_struct("Applied")
                .field("revision", &snapshot.revision)
                .finish(),
            Self::Unchanged { snapshot } => formatter
                .debug_struct("Unchanged")
                .field("revision", &snapshot.revision)
                .finish(),
            Self::Stale { refreshed_plan } => formatter
                .debug_struct("Stale")
                .field("plan", refreshed_plan)
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookImportApplyResultV1 {
    pub schema_version: u32,
    pub outcome: ExternalHookImportApplyOutcomeV1,
}

impl fmt::Debug for ExternalHookImportApplyResultV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalHookImportApplyResultV1")
            .field("schema_version", &self.schema_version)
            .field("outcome", &self.outcome)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportedHookSourceStateV1 {
    Current,
    UpdateAvailable,
    SourceMissing,
    UpdateCheckFailed,
    BundleMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportedHookSourceSnapshotV1 {
    pub import_id: String,
    pub source: ExternalHookSource,
    pub enabled: bool,
    pub behavior_version: String,
    pub state: ImportedHookSourceStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookImportSnapshotV1 {
    pub schema_version: u32,
    pub revision: String,
    pub catalog: ExternalHookCatalogSnapshotV1,
    pub imports: Vec<ImportedHookSourceSnapshotV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExternalHookImportMutationV1 {
    SetEnabled { import_id: String, enabled: bool },
    Remove { import_id: String },
    ResetCorruptStore { scope: ExternalSourceScope },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalHookImportMutationRequestV1 {
    pub schema_version: u32,
    pub expected_revision: String,
    pub action: ExternalHookImportMutationV1,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreparedExternalHookHandler {
    pub stable_key: String,
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    pub command_windows: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub status_message: Option<String>,
    pub dependencies: Vec<ExternalHookImportDependencyV1>,
}

impl fmt::Debug for PreparedExternalHookHandler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExternalHookHandler")
            .field("stable_key", &self.stable_key)
            .field("event", &self.event)
            .field("command", &"<redacted>")
            .field("dependency_count", &self.dependencies.len())
            .finish()
    }
}

impl PreparedExternalHookHandler {
    pub fn public_review(&self) -> ExternalHookImportHandlerV1 {
        ExternalHookImportHandlerV1 {
            stable_key: self.stable_key.clone(),
            event: self.event.clone(),
            matcher: self.matcher.clone(),
            command: self.command.clone(),
            command_windows: self.command_windows.clone(),
            timeout_seconds: self.timeout_seconds,
            status_message: self.status_message.clone(),
            dependencies: self.dependencies.clone(),
        }
    }

    pub fn public_review_at(
        &self,
        managed_root: &Path,
    ) -> Result<ExternalHookImportHandlerV1, ExternalSourceContractError> {
        let managed_root = managed_root.to_string_lossy().replace('\\', "/");
        if managed_root.is_empty()
            || managed_root
                .chars()
                .any(|value| value.is_control() || matches!(value, '"' | '$' | '`' | '%' | '!'))
        {
            return Err(ExternalSourceContractError::InvalidText(
                "managed Hook root",
            ));
        }
        let mut review = self.public_review();
        review.command = review
            .command
            .replace(MANAGED_HOOK_ROOT_PLACEHOLDER, &managed_root);
        review.command_windows = review
            .command_windows
            .map(|command| command.replace(MANAGED_HOOK_ROOT_PLACEHOLDER, &managed_root));
        review.validate()?;
        Ok(review)
    }

    fn validate(&self) -> Result<(), ExternalSourceContractError> {
        self.public_review().validate()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreparedExternalHookAsset {
    pub relative_path: PathBuf,
    pub bytes: Vec<u8>,
}

impl fmt::Debug for PreparedExternalHookAsset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExternalHookAsset")
            .field("relative_path", &self.relative_path)
            .field("byte_count", &self.bytes.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreparedExternalHookImport {
    pub source: ExternalHookSource,
    pub behavior_version: String,
    pub handlers: Vec<PreparedExternalHookHandler>,
    pub skipped: Vec<ExternalHookImportSkippedV1>,
    pub assets: Vec<PreparedExternalHookAsset>,
}

impl fmt::Debug for PreparedExternalHookImport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExternalHookImport")
            .field("source", &self.source.key)
            .field("behavior_version", &self.behavior_version)
            .field("handler_count", &self.handlers.len())
            .field("skipped", &self.skipped)
            .field("asset_count", &self.assets.len())
            .finish()
    }
}

impl PreparedExternalHookImport {
    pub fn new(
        source: ExternalHookSource,
        handlers: Vec<PreparedExternalHookHandler>,
        skipped: Vec<ExternalHookImportSkippedV1>,
        mut assets: Vec<PreparedExternalHookAsset>,
    ) -> Result<Self, ExternalSourceContractError> {
        source.validate()?;
        if handlers.len() > MAX_EXTERNAL_HOOK_IMPORT_HANDLERS
            || (handlers.is_empty() && skipped.is_empty())
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "prepared Hook import handler count",
            ));
        }
        if skipped.len() > MAX_EXTERNAL_HOOK_IMPORT_SKIPPED_REASONS
            || assets.len() > MAX_EXTERNAL_HOOK_IMPORT_ASSETS
        {
            return Err(ExternalSourceContractError::InvalidIdentifier(
                "prepared Hook import size",
            ));
        }
        let mut handler_keys = BTreeSet::new();
        for handler in &handlers {
            handler.validate()?;
            if !handler_keys.insert(&handler.stable_key) {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "prepared Hook import handler identity",
                ));
            }
        }
        for item in &skipped {
            item.validate()?;
        }
        assets.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let mut asset_paths = BTreeSet::new();
        let mut total_bytes = 0usize;
        for asset in &assets {
            validate_asset_path(&asset.relative_path)?;
            if asset.bytes.len() > MAX_EXTERNAL_HOOK_IMPORT_ASSET_BYTES
                || !asset_paths.insert(asset.relative_path.clone())
            {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "prepared Hook import asset",
                ));
            }
            total_bytes = total_bytes.checked_add(asset.bytes.len()).ok_or(
                ExternalSourceContractError::InvalidIdentifier("prepared Hook import asset bytes"),
            )?;
            if total_bytes > MAX_EXTERNAL_HOOK_IMPORT_TOTAL_ASSET_BYTES {
                return Err(ExternalSourceContractError::InvalidIdentifier(
                    "prepared Hook import asset bytes",
                ));
            }
        }
        let behavior_version = behavior_version(&source, &handlers, &assets);
        Ok(Self {
            source,
            behavior_version,
            handlers,
            skipped,
            assets,
        })
    }
}

fn behavior_version(
    source: &ExternalHookSource,
    handlers: &[PreparedExternalHookHandler],
    assets: &[PreparedExternalHookAsset],
) -> String {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, source.key.stable_key().as_bytes());
    for handler in handlers {
        hash_part(&mut hasher, handler.stable_key.as_bytes());
        hash_part(&mut hasher, handler.event.as_bytes());
        hash_optional(&mut hasher, handler.matcher.as_deref());
        hash_part(&mut hasher, handler.command.as_bytes());
        hash_optional(&mut hasher, handler.command_windows.as_deref());
        hash_part(
            &mut hasher,
            handler
                .timeout_seconds
                .unwrap_or_default()
                .to_string()
                .as_bytes(),
        );
        hash_optional(&mut hasher, handler.status_message.as_deref());
        for dependency in &handler.dependencies {
            match dependency {
                ExternalHookImportDependencyV1::Managed { relative_path } => {
                    hash_part(&mut hasher, b"managed");
                    hash_part(&mut hasher, relative_path.as_bytes());
                }
                ExternalHookImportDependencyV1::External { location } => {
                    hash_part(&mut hasher, b"external");
                    hash_part(&mut hasher, location.as_bytes());
                }
            }
        }
    }
    for asset in assets {
        hash_part(
            &mut hasher,
            asset.relative_path.to_string_lossy().as_bytes(),
        );
        hash_part(&mut hasher, &asset.bytes);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn hash_optional(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_part(hasher, b"some");
            hash_part(hasher, value.as_bytes());
        }
        None => hash_part(hasher, b"none"),
    }
}

fn hash_part(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn validate_asset_path(path: &PathBuf) -> Result<(), ExternalSourceContractError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().count() > MAX_EXTERNAL_HOOK_IMPORT_ASSET_DEPTH
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ExternalSourceContractError::InvalidIdentifier(
            "prepared Hook import asset path",
        ));
    }
    Ok(())
}

fn validate_bounded_text(
    value: &str,
    max_bytes: usize,
    label: &'static str,
) -> Result<(), ExternalSourceContractError> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(ExternalSourceContractError::InvalidText(label));
    }
    Ok(())
}
