use super::common::{
    backup_domain_dir, backup_file_once, read_bounded_json, read_optional_bounded_json,
    restore_unverified_file, stage_domain_dir,
};
use openbitfun_config_contracts::validate_current_config_value;
use openbitfun_config_contracts::{AIModelConfig, GlobalConfig};
use openbitfun_legacy_migration::{
    atomic_write_json, DomainContext, DomainScan, LegacyDomainAdapter, LegacyMigrationError,
    LegacyMigrationResult, MigrationRoots,
};
use openbitfun_product_domains::legacy_migration::{
    ConflictResolution, FindingSeverity, MigrationConflict, MigrationDiagnostic, MigrationDomainId,
    MigrationDomainResult, MigrationDomainState, ScanFinding,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const SOURCE_SCHEMA: &str = "bitfun.config.v1";
const TARGET_SCHEMA: &str = "openbitfun.config.current";

pub(crate) struct SettingsAdapter;
pub(crate) struct CredentialsAdapter;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagedSettings {
    target_existed: bool,
    imported: u64,
    skipped: u64,
    conflicts: u64,
    config: GlobalConfig,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CredentialManifest {
    target_existed: bool,
    model_ids: Vec<String>,
    voice_call: bool,
    mcp_servers: bool,
    unsupported_secret_fields: Vec<String>,
}

#[derive(Default)]
struct MergeOutcome {
    imported: u64,
    skipped: u64,
    conflicts: Vec<MigrationConflict>,
    rejected: Vec<String>,
}

impl LegacyDomainAdapter for SettingsAdapter {
    fn domain(&self) -> MigrationDomainId {
        MigrationDomainId::Settings
    }

    fn scan(&self, roots: &MigrationRoots) -> LegacyMigrationResult<DomainScan> {
        let source = read_source_config(roots)?;
        let target = read_target_config(roots)?;
        let (_, outcome) = merge_settings(&source, target)?;
        let bytes = serde_json::to_vec(&source)
            .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
        Ok(DomainScan {
            finding: ScanFinding {
                domain: self.domain(),
                code: if outcome.rejected.is_empty() { "legacy_settings_supported" } else { "settings_items_skipped" }.to_string(),
                severity: if outcome.rejected.is_empty() { FindingSeverity::Info } else { FindingSeverity::Warning },
                entity_count: outcome.imported + outcome.skipped,
                logical_bytes: bytes.len() as u64,
                source_schema: Some(SOURCE_SCHEMA.to_string()),
                migratable: true,
                detail: "Supported legacy settings will be converted through the current configuration contract."
                    .to_string(),
            },
            conflicts: outcome.conflicts,
            target_schema: Some(TARGET_SCHEMA.to_string()),
            dependencies: Vec::new(),
        })
    }

    fn stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<MigrationDomainResult> {
        let source = read_source_config(context.roots)?;
        let target_path = target_config_path(context.roots);
        let target_existed = target_path.exists();
        let target = read_target_config(context.roots)?;
        let (config, outcome) = merge_settings(&source, target)?;
        validate_current_config(&config, "staged legacy configuration")?;
        let staged = StagedSettings {
            target_existed,
            imported: outcome.imported,
            skipped: outcome.skipped,
            conflicts: outcome.conflicts.len() as u64,
            config,
        };
        atomic_write_json(&settings_stage_path(context), &staged)?;
        Ok(MigrationDomainResult {
            domain: self.domain(),
            state: MigrationDomainState::Staged,
            imported: staged.imported,
            skipped: staged.skipped,
            conflicts: staged.conflicts,
            warnings: outcome
                .rejected
                .iter()
                .map(|path| MigrationDiagnostic {
                    code: "setting_field_skipped".to_string(),
                    severity: FindingSeverity::Warning,
                    domain: Some(self.domain()),
                    relative_path: Some(path.clone()),
                    message: "An incompatible setting or model was preserved in the source."
                        .to_string(),
                    ..Default::default()
                })
                .collect(),
            ..MigrationDomainResult::default()
        })
    }

    fn validate_stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let staged: StagedSettings =
            read_bounded_json(&context.layout.stage_root(), &settings_stage_path(context))?;
        validate_current_config(&staged.config, "staged legacy configuration")
    }

    fn commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let staged: StagedSettings =
            read_bounded_json(&context.layout.stage_root(), &settings_stage_path(context))?;
        let target = target_config_path(context.roots);
        backup_file_once(&target, &settings_backup_path(context))?;
        atomic_write_json(&target, &staged.config)
    }

    fn validate_commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let expected: StagedSettings =
            read_bounded_json(&context.layout.stage_root(), &settings_stage_path(context))?;
        let actual = read_target_config(context.roots)?;
        validate_current_config(&actual, "committed legacy configuration")?;
        let expected_value = serde_json::to_value(expected.config)
            .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
        let actual_value = serde_json::to_value(actual)
            .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
        if actual_value != expected_value {
            return Err(LegacyMigrationError::InvalidRequest(
                "committed configuration does not match the staged owner model".to_string(),
            ));
        }
        Ok(())
    }

    fn rollback_unverified(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let staged = read_optional_bounded_json::<StagedSettings>(
            &context.layout.stage_root(),
            &settings_stage_path(context),
        )?;
        if let Some(staged) = staged {
            restore_unverified_file(
                &target_config_path(context.roots),
                &settings_backup_path(context),
                staged.target_existed,
            )?;
        }
        Ok(())
    }
}

impl LegacyDomainAdapter for CredentialsAdapter {
    fn domain(&self) -> MigrationDomainId {
        MigrationDomainId::Credentials
    }

    fn scan(&self, roots: &MigrationRoots) -> LegacyMigrationResult<DomainScan> {
        let source = read_source_config(roots)?;
        let manifest = credential_manifest(&source, target_config_path(roots).exists());
        let count = manifest.model_ids.len() as u64
            + u64::from(manifest.voice_call)
            + u64::from(manifest.mcp_servers);
        Ok(DomainScan {
            finding: ScanFinding {
                domain: self.domain(),
                code: if count == 0 {
                    "legacy_credentials_absent"
                } else {
                    "legacy_credentials_supported"
                }
                .to_string(),
                severity: if manifest.unsupported_secret_fields.is_empty() {
                    FindingSeverity::Info
                } else {
                    FindingSeverity::Warning
                },
                entity_count: count,
                logical_bytes: 0,
                source_schema: Some(SOURCE_SCHEMA.to_string()),
                migratable: true,
                detail: "Credentials are read only during commit and are never written to migration staging or reports."
                    .to_string(),
            },
            conflicts: Vec::new(),
            target_schema: Some(TARGET_SCHEMA.to_string()),
            dependencies: vec![MigrationDomainId::Settings],
        })
    }

    fn stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<MigrationDomainResult> {
        let source = read_source_config(context.roots)?;
        let manifest = credential_manifest(&source, target_config_path(context.roots).exists());
        atomic_write_json(&credentials_stage_path(context), &manifest)?;
        let warnings = manifest
            .unsupported_secret_fields
            .iter()
            .map(|field| MigrationDiagnostic {
                code: "credential_requires_reauthentication".to_string(),
                severity: FindingSeverity::Warning,
                domain: Some(self.domain()),
                relative_path: None,
                message: format!("Credential metadata at {field} is not safely portable."),
                action: Some("Enter the credential again in OpenBitFun.".to_string()),
            })
            .collect::<Vec<_>>();
        Ok(MigrationDomainResult {
            domain: self.domain(),
            state: MigrationDomainState::Staged,
            imported: manifest.model_ids.len() as u64
                + u64::from(manifest.voice_call)
                + u64::from(manifest.mcp_servers),
            skipped: manifest.unsupported_secret_fields.len() as u64,
            warnings,
            requires_reauthentication: manifest.unsupported_secret_fields.clone(),
            ..MigrationDomainResult::default()
        })
    }

    fn validate_stage(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let bytes = fs::read(credentials_stage_path(context)).map_err(|error| {
            LegacyMigrationError::InvalidRequest(format!("credential stage is unreadable: {error}"))
        })?;
        let manifest: CredentialManifest = serde_json::from_slice(&bytes).map_err(|error| {
            LegacyMigrationError::InvalidRequest(format!("credential stage is invalid: {error}"))
        })?;
        let source = read_source_config(context.roots)?;
        let expected = credential_manifest(&source, manifest.target_existed);
        if serde_json::to_value(&manifest).ok() != serde_json::to_value(expected).ok() {
            return Err(LegacyMigrationError::InvalidRequest(
                "credential manifest differs from source selection".to_string(),
            ));
        }
        if manifest.model_ids.iter().any(|id| id.trim().is_empty()) {
            return Err(LegacyMigrationError::InvalidRequest(
                "credential staging contains an empty logical model id".to_string(),
            ));
        }
        Ok(())
    }

    fn commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let source = read_source_config(context.roots)?;
        let target_path = target_config_path(context.roots);
        let mut target = read_target_config(context.roots)?;
        let manifest: CredentialManifest = read_bounded_json(
            &context.layout.stage_root(),
            &credentials_stage_path(context),
        )?;
        backup_file_once(&target_path, &credentials_backup_path(context))?;

        if let Some(models) = source.pointer("/ai/models").and_then(Value::as_array) {
            for source_model in models {
                let Some(id) = source_model.get("id").and_then(Value::as_str) else {
                    continue;
                };
                if !manifest.model_ids.iter().any(|candidate| candidate == id) {
                    continue;
                }
                let Ok(source_model) =
                    serde_json::from_value::<AIModelConfig>(source_model.clone())
                else {
                    continue;
                };
                if let Some(target_model) = target.ai.models.iter_mut().find(|model| model.id == id)
                {
                    if target_model.api_key.is_empty() {
                        target_model.api_key = source_model.api_key;
                    }
                    if target_model
                        .custom_headers
                        .as_ref()
                        .is_none_or(|headers| headers.is_empty())
                    {
                        target_model.custom_headers = source_model.custom_headers;
                    }
                    if target_model
                        .custom_request_body
                        .as_deref()
                        .is_none_or(str::is_empty)
                    {
                        target_model.custom_request_body = source_model.custom_request_body;
                    }
                }
            }
        }
        if manifest.voice_call && target.app.voice_call.api_key.is_empty() {
            if let Some(secret) = source
                .pointer("/app/voice_call/api_key")
                .and_then(Value::as_str)
            {
                target.app.voice_call.api_key = secret.to_string();
            }
        }
        if manifest.mcp_servers && target.mcp_servers.is_none() {
            target.mcp_servers = source
                .get("mcp_servers")
                .filter(|value| !value.is_null())
                .cloned();
        }
        validate_current_config(&target, "credential migration target")?;
        atomic_write_json(&target_path, &target)
    }

    fn validate_commit(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let source = read_source_config(context.roots)?;
        let target = read_target_config(context.roots)?;
        let manifest: CredentialManifest = read_bounded_json(
            &context.layout.stage_root(),
            &credentials_stage_path(context),
        )?;
        validate_current_config(&target, "committed credential configuration")?;
        for id in &manifest.model_ids {
            let source_model = source
                .pointer("/ai/models")
                .and_then(Value::as_array)
                .and_then(|models| {
                    models
                        .iter()
                        .find(|model| model.get("id").and_then(Value::as_str) == Some(id.as_str()))
                })
                .cloned()
                .and_then(|value| serde_json::from_value::<AIModelConfig>(value).ok());
            let target_model = target.ai.models.iter().find(|model| model.id == *id);
            let Some(source_model) = source_model else {
                continue;
            };
            let Some(target_model) = target_model else {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "credential owner model {id} was not committed"
                )));
            };
            if !source_model.api_key.is_empty() && target_model.api_key.is_empty() {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "credential for model {id} was not committed"
                )));
            }
            if source_model
                .custom_headers
                .as_ref()
                .is_some_and(|headers| !headers.is_empty())
                && target_model
                    .custom_headers
                    .as_ref()
                    .is_none_or(|headers| headers.is_empty())
            {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "custom headers for model {id} were not committed"
                )));
            }
            if source_model
                .custom_request_body
                .as_deref()
                .is_some_and(|body| !body.is_empty())
                && target_model
                    .custom_request_body
                    .as_deref()
                    .is_none_or(str::is_empty)
            {
                return Err(LegacyMigrationError::InvalidRequest(format!(
                    "custom request body for model {id} was not committed"
                )));
            }
        }
        if manifest.mcp_servers && target.mcp_servers.is_none() {
            return Err(LegacyMigrationError::InvalidRequest(
                "MCP server configuration was not committed".to_string(),
            ));
        }
        Ok(())
    }

    fn rollback_unverified(&self, context: &DomainContext<'_>) -> LegacyMigrationResult<()> {
        let manifest = read_optional_bounded_json::<CredentialManifest>(
            &context.layout.stage_root(),
            &credentials_stage_path(context),
        )?;
        if let Some(manifest) = manifest {
            restore_unverified_file(
                &target_config_path(context.roots),
                &credentials_backup_path(context),
                manifest.target_existed,
            )?;
        }
        Ok(())
    }
}

fn read_source_config(roots: &MigrationRoots) -> LegacyMigrationResult<Value> {
    read_bounded_json(&roots.legacy_user_root, &source_config_path(roots))
}

fn read_target_config(roots: &MigrationRoots) -> LegacyMigrationResult<GlobalConfig> {
    let path = target_config_path(roots);
    let value = read_optional_bounded_json::<Value>(&roots.target_user_root, &path)?;
    match value {
        Some(value) => {
            validate_current_config_value(&value, "legacy migration target configuration")
                .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
            serde_json::from_value(value).map_err(|error| {
                LegacyMigrationError::InvalidRequest(format!(
                    "target configuration does not match the current owner model: {error}"
                ))
            })
        }
        None => Ok(GlobalConfig::default()),
    }
}

fn merge_settings(
    source: &Value,
    target: GlobalConfig,
) -> LegacyMigrationResult<(GlobalConfig, MergeOutcome)> {
    let (mut source_config, mut source_value, rejected) = convert_source_config(source)?;
    let defaults = GlobalConfig::default();
    let mut outcome = MergeOutcome {
        skipped: rejected.len() as u64,
        rejected,
        ..Default::default()
    };
    let source_models = std::mem::take(&mut source_config.ai.models);
    let mut target_value = serde_json::to_value(&target)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    let default_value = serde_json::to_value(&defaults)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    if let Some(ai) = source_value.get_mut("ai").and_then(Value::as_object_mut) {
        ai.remove("models");
    }
    merge_compatible_value(
        "",
        &source_value,
        &mut target_value,
        Some(&default_value),
        &mut outcome,
    );
    let mut merged: GlobalConfig = serde_json::from_value(target_value).map_err(|error| {
        LegacyMigrationError::InvalidRequest(format!(
            "merged configuration does not match the current owner model: {error}"
        ))
    })?;
    merge_models(source_models, &mut merged.ai.models, &mut outcome)?;
    merged.product_id = defaults.product_id;
    merged.schema_version = defaults.schema_version;
    merged.version = defaults.version;
    merged.last_modified = chrono::Utc::now();
    Ok((merged, outcome))
}

fn convert_source_config(
    source: &Value,
) -> LegacyMigrationResult<(GlobalConfig, Value, Vec<String>)> {
    validate_source_version(source)?;
    let defaults = GlobalConfig::default();
    let mut normalized_source = source.clone();
    normalize_legacy_config_value(&mut normalized_source);
    strip_staged_credentials(&mut normalized_source);
    let normalized_root = normalized_source.as_object_mut().ok_or_else(|| {
        LegacyMigrationError::InvalidRequest(
            "legacy configuration root is not an object".to_string(),
        )
    })?;
    for field in ["product_id", "schema_version", "version", "last_modified"] {
        normalized_root.remove(field);
    }

    let mut converted = serde_json::to_value(&defaults)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    let mut rejected = Vec::new();
    let accepted = accept_config_patch(&mut converted, "", &normalized_source, &mut rejected)
        .unwrap_or_else(|| serde_json::json!({}));
    let config: GlobalConfig = serde_json::from_value(converted)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    let mut compatible_source = serde_json::to_value(&config)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    retain_source_fields(&mut compatible_source, &accepted);
    Ok((config, compatible_source, rejected))
}

// Validate candidate patches against the real owner type. Invalid leaves keep
// the default; arrays are admitted one complete entry at a time.
fn accept_config_patch(
    root: &mut Value,
    pointer: &str,
    source: &Value,
    rejected: &mut Vec<String>,
) -> Option<Value> {
    let before = root.clone();
    let mut candidate = before.clone();
    let mut applied = false;
    if pointer.is_empty() {
        if let (Some(target), Some(fields)) = (candidate.as_object_mut(), source.as_object()) {
            for (key, value) in fields {
                target.insert(key.clone(), value.clone());
            }
            applied = true;
        }
    } else if let Some((parent, key)) = pointer.rsplit_once('/') {
        if let Some(fields) = candidate.pointer_mut(parent).and_then(Value::as_object_mut) {
            fields.insert(key.replace("~1", "/").replace("~0", "~"), source.clone());
            applied = true;
        }
    }
    if applied
        && serde_json::from_value::<GlobalConfig>(candidate.clone()).is_ok()
        && (!pointer.is_empty() || source.is_object())
    {
        *root = candidate;
        return Some(source.clone());
    }
    if let Some(fields) = source.as_object() {
        let mut accepted = serde_json::Map::new();
        for (key, value) in fields {
            let child = format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"));
            if let Some(value) = accept_config_patch(root, &child, value, rejected) {
                accepted.insert(key.clone(), value);
            }
        }
        return Some(Value::Object(accepted));
    }
    if let Some(values) = source.as_array() {
        let mut accepted = Vec::new();
        let mut empty = root.clone();
        let Some(slot) = empty.pointer_mut(pointer) else {
            rejected.push(pointer.to_string());
            return None;
        };
        *slot = Value::Array(Vec::new());
        if serde_json::from_value::<GlobalConfig>(empty.clone()).is_err() {
            rejected.push(pointer.to_string());
            return None;
        }
        *root = empty;
        for (index, value) in values.iter().enumerate() {
            let mut next = accepted.clone();
            next.push(value.clone());
            let mut test = root.clone();
            let Some(slot) = test.pointer_mut(pointer) else {
                rejected.push(format!("{pointer}/{index}"));
                continue;
            };
            *slot = Value::Array(next.clone());
            if serde_json::from_value::<GlobalConfig>(test.clone()).is_ok() {
                *root = test;
                accepted = next;
            } else {
                rejected.push(format!("{pointer}/{index}"));
            }
        }
        return Some(Value::Array(accepted));
    }
    rejected.push(pointer.to_string());
    None
}

fn retain_source_fields(value: &mut Value, source: &Value) {
    let (Value::Object(fields), Value::Object(source_fields)) = (value, source) else {
        return;
    };
    fields.retain(|name, value| {
        let Some(source_value) = source_fields.get(name) else {
            return false;
        };
        retain_source_fields(value, source_value);
        true
    });
}

fn normalize_legacy_config_value(value: &mut Value) {
    if let Some(appearance) = value.get_mut("appearance").and_then(Value::as_object_mut) {
        let selection = appearance
            .get("selection")
            .or_else(|| appearance.get("theme_id"))
            .and_then(Value::as_str)
            .map(canonical_product_id);
        if let Some(selection) = selection {
            appearance.insert("selection".to_string(), Value::String(selection));
        }
        appearance.remove("theme_id");
    }

    let Some(ai) = value.get_mut("ai").and_then(Value::as_object_mut) else {
        return;
    };
    if let Some(profiles) = ai.get_mut("agent_profiles").and_then(Value::as_object_mut) {
        for (profile_id, profile) in profiles {
            let Some(profile) = profile.as_object_mut() else {
                continue;
            };
            if profile
                .get("profile_id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                profile.insert("profile_id".to_string(), Value::String(profile_id.clone()));
            }
            if let Some(enabled) = profile.remove("enabled_skills") {
                profile
                    .entry("enabled_user_skills".to_string())
                    .or_insert(enabled);
            }
            canonicalize_string_list(profile.get_mut("enabled_user_skills"));
            canonicalize_string_list(profile.get_mut("disabled_user_skills"));
        }
    }
    if let Some(skill_settings) = ai.get_mut("skill_settings").and_then(Value::as_object_mut) {
        canonicalize_string_list(skill_settings.get_mut("globally_disabled_user_skills"));
    }
}

fn canonicalize_string_list(value: Option<&mut Value>) {
    let Some(values) = value.and_then(Value::as_array_mut) else {
        return;
    };
    for value in values {
        if let Some(text) = value.as_str() {
            *value = Value::String(canonical_product_id(text));
        }
    }
}

fn strip_staged_credentials(value: &mut Value) {
    if let Some(root) = value.as_object_mut() {
        root.remove("mcp_servers");
    }
    if let Some(models) = value
        .pointer_mut("/ai/models")
        .and_then(Value::as_array_mut)
    {
        for model in models {
            let Some(model) = model.as_object_mut() else {
                continue;
            };
            model.remove("api_key");
            model.remove("custom_headers");
            model.remove("custom_request_body");
        }
    }
    if let Some(voice_call) = value
        .pointer_mut("/app/voice_call")
        .and_then(Value::as_object_mut)
    {
        voice_call.remove("api_key");
    }
}

fn merge_compatible_value(
    path: &str,
    source: &Value,
    target: &mut Value,
    default: Option<&Value>,
    outcome: &mut MergeOutcome,
) {
    if source == target {
        return;
    }
    if let (Value::Object(source_fields), Value::Object(target_fields)) = (source, &mut *target) {
        let default_fields = default.and_then(Value::as_object);
        for (name, source_value) in source_fields {
            let child_path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            if let Some(target_value) = target_fields.get_mut(name) {
                merge_compatible_value(
                    &child_path,
                    source_value,
                    target_value,
                    default_fields.and_then(|fields| fields.get(name)),
                    outcome,
                );
            } else {
                target_fields.insert(name.clone(), source_value.clone());
                outcome.imported = outcome.imported.saturating_add(1);
            }
        }
        return;
    }

    if default.is_some_and(|default| target == default) {
        *target = source.clone();
        outcome.imported = outcome.imported.saturating_add(1);
    } else {
        record_target_wins(path, outcome);
    }
}

fn merge_models(
    source_models: Vec<AIModelConfig>,
    target_models: &mut Vec<AIModelConfig>,
    outcome: &mut MergeOutcome,
) -> LegacyMigrationResult<()> {
    for model in source_models {
        if model.id.trim().is_empty() {
            outcome.skipped = outcome.skipped.saturating_add(1);
            continue;
        }
        if let Some(existing) = target_models
            .iter()
            .find(|existing| existing.id == model.id)
        {
            let existing = settings_only_model_value(existing)?;
            let source = settings_only_model_value(&model)?;
            if existing != source {
                record_target_wins(&format!("ai.models.{}", model.id), outcome);
            }
        } else {
            target_models.push(model);
            outcome.imported = outcome.imported.saturating_add(1);
        }
    }
    Ok(())
}

fn settings_only_model_value(model: &AIModelConfig) -> LegacyMigrationResult<Value> {
    let mut value = serde_json::to_value(model)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    strip_model_credentials(&mut value);
    Ok(value)
}

fn strip_model_credentials(value: &mut Value) {
    let Some(model) = value.as_object_mut() else {
        return;
    };
    model.remove("api_key");
    model.remove("custom_headers");
    model.remove("custom_request_body");
}

fn credential_manifest(source: &Value, target_existed: bool) -> CredentialManifest {
    let model_ids = source
        .pointer("/ai/models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|model| {
            model_has_portable_credentials(model)
                && serde_json::from_value::<AIModelConfig>((*model).clone()).is_ok()
        })
        .filter_map(|model| model.get("id").and_then(Value::as_str))
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let voice_call = source
        .pointer("/app/voice_call/api_key")
        .and_then(Value::as_str)
        .is_some_and(|secret| !secret.is_empty());
    let mcp_servers = source
        .get("mcp_servers")
        .is_some_and(|value| !value.is_null());
    let mut unsupported_secret_fields = Vec::new();
    collect_unsupported_secret_fields(source, "", &mut unsupported_secret_fields);
    unsupported_secret_fields.retain(|path| !is_portable_credential_path(path));
    if let Some(models) = source.pointer("/ai/models").and_then(Value::as_array) {
        for (index, model) in models.iter().enumerate() {
            if model_has_portable_credentials(model)
                && serde_json::from_value::<AIModelConfig>(model.clone()).is_err()
            {
                unsupported_secret_fields.push(format!("/ai/models/{index}"));
            }
        }
    }
    unsupported_secret_fields.sort();
    unsupported_secret_fields.dedup();
    CredentialManifest {
        target_existed,
        model_ids,
        voice_call,
        mcp_servers,
        unsupported_secret_fields,
    }
}

fn model_has_portable_credentials(model: &Value) -> bool {
    model
        .get("api_key")
        .and_then(Value::as_str)
        .is_some_and(|secret| !secret.is_empty())
        || model
            .get("custom_headers")
            .and_then(Value::as_object)
            .is_some_and(|headers| !headers.is_empty())
        || model
            .get("custom_request_body")
            .and_then(Value::as_str)
            .is_some_and(|body| !body.is_empty())
}

fn is_portable_credential_path(path: &str) -> bool {
    path == "app.voice_call.api_key"
        || path == "mcp_servers"
        || path.starts_with("mcp_servers.")
        || (path.starts_with("ai.models.")
            && (path.ends_with(".api_key")
                || path.contains(".custom_headers.")
                || path.ends_with(".custom_request_body")))
}

fn collect_unsupported_secret_fields(value: &Value, path: &str, output: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                let next = if path.is_empty() {
                    name.to_string()
                } else {
                    format!("{path}.{name}")
                };
                let lower = name.to_ascii_lowercase();
                if matches!(lower.as_str(), "password" | "token" | "secret" | "api_key")
                    && value.as_str().is_some_and(|secret| !secret.is_empty())
                {
                    output.push(next.clone());
                }
                collect_unsupported_secret_fields(value, &next, output);
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                collect_unsupported_secret_fields(value, &format!("{path}.{index}"), output);
            }
        }
        _ => {}
    }
}

fn validate_source_version(source: &Value) -> LegacyMigrationResult<()> {
    let schema = source.get("schema_version").and_then(Value::as_u64);
    let version = source.get("version").and_then(Value::as_str);
    if schema != Some(1) || !version.is_some_and(|version| version.starts_with("0.")) {
        return Err(LegacyMigrationError::UnsupportedSource(format!(
            "expected BitFun config schema 1 from a 0.x release, found schema={schema:?}, version={version:?}"
        )));
    }
    Ok(())
}

fn validate_current_config(config: &GlobalConfig, context: &str) -> LegacyMigrationResult<()> {
    let value = serde_json::to_value(config)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    validate_current_config_value(&value, context)
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))?;
    serde_json::from_value::<GlobalConfig>(value)
        .map(|_| ())
        .map_err(|error| LegacyMigrationError::InvalidRequest(error.to_string()))
}

fn record_target_wins(path: &str, outcome: &mut MergeOutcome) {
    outcome.skipped += 1;
    outcome.conflicts.push(MigrationConflict {
        domain: MigrationDomainId::Settings,
        code: "target_setting_wins".to_string(),
        source_summary: format!("legacy setting {path}"),
        target_summary: format!("existing OpenBitFun setting {path}"),
        resolution: ConflictResolution::TargetWins,
    });
}

fn canonical_product_id(value: &str) -> String {
    value
        .replace("user::bitfun::", "user::openbitfun::")
        .replace("bitfun-", "openbitfun-")
}

fn source_config_path(roots: &MigrationRoots) -> PathBuf {
    roots.legacy_user_root.join("config").join("app.json")
}

fn target_config_path(roots: &MigrationRoots) -> PathBuf {
    roots.target_user_root.join("config").join("app.json")
}

fn settings_stage_path(context: &DomainContext<'_>) -> PathBuf {
    stage_domain_dir(context, "settings").join("app.json")
}

fn credentials_stage_path(context: &DomainContext<'_>) -> PathBuf {
    stage_domain_dir(context, "credentials").join("manifest.json")
}

fn settings_backup_path(context: &DomainContext<'_>) -> PathBuf {
    backup_domain_dir(context, "settings").join("app.json")
}

fn credentials_backup_path(context: &DomainContext<'_>) -> PathBuf {
    backup_domain_dir(context, "credentials").join("app.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters_for_groups;
    use openbitfun_legacy_migration::{
        probe_legacy_source, CancellationToken, MigrationEngine, NoCrashInjection, ProbeLimits,
    };
    use openbitfun_product_domains::legacy_migration::{MigrationGroupId, MigrationSelection};
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn malformed_fields_and_models_do_not_discard_valid_siblings() {
        let source = serde_json::json!({"schema_version":1,"version":"0.2.19","ai":{
            "subagent_max_concurrency": 3, "stream_idle_timeout_secs":"broken", "enable_deferred_tool_loading": [],
            "models":[{"id":"valid","name":"Valid"},{"id":"bad","temperature":[]}]
        }});
        let (config, _, rejected) = convert_source_config(&source).unwrap();
        assert_eq!(config.ai.subagent_max_concurrency, 3);
        assert_eq!(config.ai.models.len(), 1);
        assert_eq!(config.ai.models[0].id, "valid");
        assert!(rejected.iter().any(|p| p == "/ai/stream_idle_timeout_secs"));
        assert!(rejected.iter().any(|p| p == "/ai/models/1"));
        let mut root = serde_json::to_value(GlobalConfig::default()).unwrap();
        assert!(accept_config_patch(
            &mut root,
            "/missing/field",
            &serde_json::json!(true),
            &mut Vec::new()
        )
        .is_none());
    }

    #[test]
    fn settings_and_credentials_convert_without_staging_secrets() {
        let temp = test_tempdir("settings-credentials");
        let roots = test_roots(temp.path());
        seed_source(&roots, true);
        let source_before = sha256(&source_config_path(&roots));
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let selection = MigrationSelection {
            groups: BTreeSet::from([MigrationGroupId::SettingsAndCredentials]),
        };
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        let report = engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();

        let target = read_target_config(&roots).unwrap();
        assert_eq!(target.app.language, "en-US");
        assert!(!target.app.auto_update);
        assert!(target.app.telemetry);
        assert!(!target.app.confirm_on_exit);
        assert!(!target.app.restore_windows);
        assert!(target.app.prevent_sleep);
        assert_eq!(target.app.zoom_level, 1.25);
        assert_eq!(target.app.notifications.duration, 8123);
        assert_eq!(target.editor.font_size, 17);
        assert_eq!(target.editor.word_wrap, "on");
        assert!(!target.editor.format_on_save);
        assert_eq!(target.terminal.default_shell, "pwsh");
        assert_eq!(target.terminal.terminal_panel_position, "bottom");
        assert_eq!(target.terminal.font_size, 16);
        assert_eq!(target.terminal.scrollback, 4321);
        assert_eq!(target.workspace.max_file_size, 123_456);
        assert_eq!(target.workspace.line_ending, "lf");
        assert!(!target.workspace.insert_final_newline);
        assert!(target.tool_permissions.interaction.auto_approve_ask);
        assert_eq!(target.appearance.selection, "openbitfun-dark");
        assert_eq!(
            target.ai.default_models.primary.as_deref(),
            Some("legacy-model")
        );
        assert_eq!(
            target.ai.default_models.fast.as_deref(),
            Some("legacy-model")
        );
        assert_eq!(target.ai.agent_model_defaults.mode, "legacy-model");
        assert_eq!(
            target
                .ai
                .agent_model_defaults
                .subagents
                .default_selection
                .fixed_model_id(),
            Some("legacy-model")
        );
        let profile = &target.ai.agent_profiles["coding_shared"];
        assert_eq!(profile.profile_id, "coding_shared");
        assert_eq!(profile.added_tools, ["LegacyTool"]);
        assert_eq!(profile.removed_tools, ["ReadFile"]);
        assert_eq!(
            profile.disabled_user_skills,
            ["user::openbitfun::disabled-skill"]
        );
        assert_eq!(
            profile.enabled_user_skills,
            ["user::openbitfun::user-skill"]
        );
        assert_eq!(
            target.ai.skill_settings.globally_disabled_user_skills,
            ["user::openbitfun::global-disabled-skill"]
        );
        assert_eq!(
            target.ai.review_teams["default"].reviewer_timeout_seconds,
            77
        );
        assert_eq!(target.ai.subagent_max_concurrency, 3);
        assert_eq!(target.ai.stream_idle_timeout_secs, Some(321));
        assert_eq!(target.ai.stream_ttft_timeout_secs, Some(123));
        assert_eq!(target.ai.tool_execution_timeout_secs, Some(456));
        assert!(!target.ai.enable_deferred_tool_loading);
        assert_eq!(target.ai.browser_control_preferred_browser, "edge");
        let model = target
            .ai
            .models
            .iter()
            .find(|model| model.id == "legacy-model")
            .unwrap();
        assert_eq!(model.api_key, "fixture-api-key");
        assert_eq!(
            model.custom_headers.as_ref().unwrap()["Authorization"],
            "Bearer fixture-header-secret"
        );
        assert_eq!(
            model.custom_request_body.as_deref(),
            Some(r#"{"token":"fixture-body-secret"}"#)
        );
        assert_eq!(target.app.voice_call.api_key, "fixture-voice-secret");
        assert_eq!(
            target.mcp_servers,
            Some(serde_json::json!({
                "fixture": {
                    "command": "fixture-mcp",
                    "env": {"TOKEN": "fixture-mcp-secret"}
                }
            }))
        );
        assert_eq!(target.product_id, GlobalConfig::default().product_id);
        assert_eq!(sha256(&source_config_path(&roots)), source_before);
        assert!(report.requires_reauthentication.is_empty());

        let stage = fs::read_dir(
            roots
                .migration_root()
                .join("runs")
                .join(&plan.run_id)
                .join("stage"),
        )
        .unwrap()
        .flat_map(|entry| walk_files(&entry.unwrap().path()))
        .flat_map(|path| fs::read(path).unwrap())
        .collect::<Vec<_>>();
        let stage = String::from_utf8_lossy(&stage);
        for secret in [
            "fixture-api-key",
            "fixture-header-secret",
            "fixture-body-secret",
            "fixture-voice-secret",
            "fixture-mcp-secret",
        ] {
            assert!(!stage.contains(secret), "staging leaked {secret}");
        }
    }

    #[test]
    fn explicit_target_values_win_and_are_reported() {
        let temp = test_tempdir("settings-target-wins");
        let roots = test_roots(temp.path());
        seed_source(&roots, true);
        let mut target = GlobalConfig::default();
        target.app.language = "fr-FR".to_string();
        target.app.close_button_behavior = "quit".to_string();
        target.ai.default_models.primary = Some("target-model".to_string());
        target.ai.models.extend([
            AIModelConfig {
                id: "target-model".to_string(),
                name: "Target model".to_string(),
                ..AIModelConfig::default()
            },
            AIModelConfig {
                id: "legacy-model".to_string(),
                api_key: "target-secret".to_string(),
                custom_headers: Some(Default::default()),
                custom_request_body: Some(String::new()),
                ..AIModelConfig::default()
            },
        ]);
        atomic_write_json(&target_config_path(&roots), &target).unwrap();
        let source = probe_legacy_source(&roots, ProbeLimits::default())
            .unwrap()
            .unwrap();
        let selection = MigrationSelection {
            groups: BTreeSet::from([MigrationGroupId::SettingsAndCredentials]),
        };
        let engine = MigrationEngine::new(roots.clone(), adapters_for_groups(&selection)).unwrap();
        let plan = engine
            .plan(&source, selection, &CancellationToken::default())
            .unwrap();
        assert!(plan
            .conflicts
            .iter()
            .any(|conflict| conflict.code == "target_setting_wins"));
        engine
            .execute(&plan, &CancellationToken::default(), &NoCrashInjection)
            .unwrap();
        let target = read_target_config(&roots).unwrap();
        assert_eq!(target.app.language, "fr-FR");
        assert_eq!(target.app.close_button_behavior, "quit");
        assert_eq!(
            target.ai.default_models.primary.as_deref(),
            Some("target-model")
        );
        assert_eq!(
            target.ai.default_models.fast.as_deref(),
            Some("legacy-model")
        );
        let legacy_model = target
            .ai
            .models
            .iter()
            .find(|model| model.id == "legacy-model")
            .unwrap();
        assert_eq!(legacy_model.api_key, "target-secret");
        assert_eq!(
            legacy_model.custom_headers.as_ref().unwrap()["Authorization"],
            "Bearer fixture-header-secret"
        );
        assert_eq!(
            legacy_model.custom_request_body.as_deref(),
            Some(r#"{"token":"fixture-body-secret"}"#)
        );
    }

    fn seed_source(roots: &MigrationRoots, with_secret: bool) {
        let source = serde_json::json!({
            "product_id": "bitfun",
            "app": {
                "language": "en-US",
                "auto_update": false,
                "telemetry": true,
                "confirm_on_exit": false,
                "restore_windows": false,
                "prevent_sleep": true,
                "zoom_level": 1.25,
                "notifications": {"duration": 8123},
                "voice_call": {
                    "api_key": if with_secret { "fixture-voice-secret" } else { "" }
                }
            },
            "editor": {
                "font_size": 17,
                "word_wrap": "on",
                "format_on_save": false
            },
            "terminal": {
                "default_shell": "pwsh",
                "terminal_panel_position": "bottom",
                "font_size": 16,
                "scrollback": 4321
            },
            "workspace": {
                "max_file_size": 123456,
                "line_ending": "lf",
                "insert_final_newline": false
            },
            "tool_permissions": {
                "interaction": {"auto_approve_ask": true}
            },
            "appearance": {"theme_id": "bitfun-dark"},
            "ai": {
                "default_models": {
                    "primary": "legacy-model",
                    "fast": "legacy-model"
                },
                "agent_model_defaults": {
                    "mode": "legacy-model",
                    "subagents": {
                        "default": {"kind": "fixed", "model_id": "legacy-model"},
                        "builtin": {},
                        "fork": {"kind": "inherit"}
                    }
                },
                "agent_profiles": {
                    "coding_shared": {
                        "added_tools": ["LegacyTool"],
                        "removed_tools": ["ReadFile"],
                        "disabled_user_skills": ["user::bitfun::disabled-skill"],
                        "enabled_skills": ["user::bitfun::user-skill"]
                    }
                },
                "skill_settings": {
                    "globally_disabled_user_skills": ["user::bitfun::global-disabled-skill"]
                },
                "review_teams": {
                    "default": {"reviewer_timeout_seconds": 77}
                },
                "subagent_max_concurrency": 3,
                "stream_idle_timeout_secs": 321,
                "stream_ttft_timeout_secs": 123,
                "tool_execution_timeout_secs": 456,
                "enable_deferred_tool_loading": false,
                "browser_control_preferred_browser": "edge",
                "models": [{
                    "id": "legacy-model",
                    "name": "Legacy model",
                    "provider": "openai",
                    "model_name": "legacy-model",
                    "base_url": "https://example.invalid/v1",
                    "enabled": true,
                    "category": "general_chat",
                    "capabilities": ["text_chat"],
                    "api_key": if with_secret { "fixture-api-key" } else { "" },
                    "custom_headers": if with_secret {
                        serde_json::json!({"Authorization": "Bearer fixture-header-secret"})
                    } else {
                        serde_json::json!({})
                    },
                    "custom_headers_mode": "merge",
                    "custom_request_body": if with_secret {
                        r#"{"token":"fixture-body-secret"}"#
                    } else {
                        ""
                    },
                    "custom_request_body_mode": "merge"
                }]
            },
            "mcp_servers": {
                "fixture": {
                    "command": "fixture-mcp",
                    "env": {"TOKEN": if with_secret { "fixture-mcp-secret" } else { "" }}
                }
            },
            "schema_version": 1,
            "version": "0.2.19",
            "last_modified": 0
        });
        atomic_write_json(&source_config_path(roots), &source).unwrap();
    }

    fn test_roots(root: &Path) -> MigrationRoots {
        MigrationRoots {
            legacy_user_root: root.join("legacy-user"),
            legacy_home_root: root.join("legacy-home"),
            legacy_skills_root: root.join("legacy-skills"),
            legacy_ssh_root: root.join("legacy-ssh"),
            target_user_root: root.join("target-user"),
            target_home_root: root.join("target-home"),
            target_skills_root: root.join("target-skills"),
            target_ssh_root: root.join("target-ssh"),
        }
    }

    fn test_tempdir(label: &str) -> tempfile::TempDir {
        let root = std::env::var_os("OPENBITFUN_TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        tempfile::Builder::new()
            .prefix(&format!("openbitfun-migration-{label}-"))
            .tempdir_in(root)
            .unwrap()
    }

    fn sha256(path: &Path) -> Vec<u8> {
        Sha256::digest(fs::read(path).unwrap()).to_vec()
    }

    fn walk_files(root: &Path) -> Vec<PathBuf> {
        if root.is_file() {
            return vec![root.to_path_buf()];
        }
        fs::read_dir(root)
            .unwrap()
            .flat_map(|entry| walk_files(&entry.unwrap().path()))
            .collect()
    }
}
