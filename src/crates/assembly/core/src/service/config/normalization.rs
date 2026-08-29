use super::manager::{
    normalize_legacy_agent_model_defaults_config_value,
    normalize_legacy_tool_permissions_config_value, strip_removed_model_reasoning_fields,
};
use super::providers::AIConfigProvider;
use super::types::{
    ConfigDiagnostic, ConfigDiagnosticRecoverability, ConfigDiagnosticSeverity, ConfigProvider,
    GlobalConfig, ModelCapability, SubagentModelSelection, CURRENT_CONFIG_SCHEMA_VERSION,
};
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ConfigNormalizationResult {
    pub value: Value,
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub changed: bool,
}

/// Applies deterministic, credential-preserving compatibility normalization
/// before typed deserialization and strict semantic validation.
pub fn normalize_config_value(config: Value) -> ConfigNormalizationResult {
    let original = config.clone();
    let mut diagnostics = Vec::new();
    let mut value =
        strip_removed_model_reasoning_fields(normalize_legacy_tool_permissions_config_value(
            normalize_legacy_agent_model_defaults_config_value(config),
        ));

    let previous_schema = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if previous_schema > u64::from(CURRENT_CONFIG_SCHEMA_VERSION) {
        diagnostics.push(ConfigDiagnostic {
            path: "schema_version".to_string(),
            message: format!(
                "Configuration schema {previous_schema} is newer than supported schema {CURRENT_CONFIG_SCHEMA_VERSION}"
            ),
            code: "CONFIG_SCHEMA_TOO_NEW".to_string(),
            severity: ConfigDiagnosticSeverity::Error,
            recoverability: ConfigDiagnosticRecoverability::None,
        });
        return ConfigNormalizationResult {
            changed: value != original,
            value,
            diagnostics,
        };
    }

    normalize_incompatible_telemetry_value(&mut value, &mut diagnostics);

    if previous_schema < u64::from(CURRENT_CONFIG_SCHEMA_VERSION) {
        if let Some(root) = value.as_object_mut() {
            root.insert(
                "schema_version".to_string(),
                Value::from(CURRENT_CONFIG_SCHEMA_VERSION),
            );
        }
        diagnostics.push(ConfigDiagnostic {
            path: "schema_version".to_string(),
            message: format!(
                "Configuration schema upgraded from {previous_schema} to {CURRENT_CONFIG_SCHEMA_VERSION}"
            ),
            code: "CONFIG_SCHEMA_UPGRADED".to_string(),
            severity: ConfigDiagnosticSeverity::Warning,
            recoverability: ConfigDiagnosticRecoverability::AutoFix,
        });
    }

    ConfigNormalizationResult {
        changed: value != original,
        value,
        diagnostics,
    }
}

fn normalize_incompatible_telemetry_value(
    config: &mut Value,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let Some(telemetry) = config
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .and_then(|app| app.get_mut("telemetry"))
    else {
        return;
    };

    if telemetry.is_boolean() {
        return;
    }

    *telemetry = Value::Bool(false);
    diagnostics.push(ConfigDiagnostic {
        path: "app.telemetry".to_string(),
        message: "Disabled an unsupported telemetry configuration during compatibility recovery"
            .to_string(),
        code: "CONFIG_TELEMETRY_DOWNGRADED".to_string(),
        severity: ConfigDiagnosticSeverity::Warning,
        recoverability: ConfigDiagnosticRecoverability::AutoFix,
    });
}

pub fn reject_unsupported_schema(diagnostics: &[ConfigDiagnostic]) -> BitFunResult<()> {
    if let Some(diagnostic) = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "CONFIG_SCHEMA_TOO_NEW")
    {
        return Err(BitFunError::validation(diagnostic.message.clone()));
    }
    Ok(())
}

/// Canonicalizes typed model fields whose meaning is capability-dependent.
pub fn normalize_typed_config(config: &mut GlobalConfig) -> Vec<ConfigDiagnostic> {
    let mut diagnostics = Vec::new();
    config.schema_version = CURRENT_CONFIG_SCHEMA_VERSION;

    for (index, model) in config.ai.models.iter_mut().enumerate() {
        model.ensure_category_and_capabilities();
        let model_id = model.id.clone();
        for field in model.normalize_inapplicable_generation_fields() {
            diagnostics.push(ConfigDiagnostic {
                path: format!("ai.models[{index}].{field}"),
                message: format!(
                    "Cleared text-generation-only field from model '{}' because it does not support text_chat",
                    model_id
                ),
                code: "MODEL_FIELD_NOT_APPLICABLE".to_string(),
                severity: ConfigDiagnosticSeverity::Warning,
                recoverability: ConfigDiagnosticRecoverability::AutoFix,
            });
        }
    }

    diagnostics
}

/// Disables only individually invalid model entries so a local model mistake
/// cannot prevent the rest of the product from starting. Cross-model/default
/// integrity is repaired separately by the model reconciliation pass.
pub async fn isolate_invalid_ai_models(
    config: &mut GlobalConfig,
) -> BitFunResult<Vec<ConfigDiagnostic>> {
    let mut diagnostics = Vec::new();

    for index in 0..config.ai.models.len() {
        if !config.ai.models[index].enabled {
            continue;
        }

        let mut isolated_ai = super::types::AIConfig::default();
        isolated_ai.models = vec![config.ai.models[index].clone()];

        let validation = AIConfigProvider
            .validate_config(&serde_json::to_value(isolated_ai)?)
            .await;
        if let Err(error) = validation {
            let error_message = error.to_string();
            // Reasoning schemas are cross-cutting runtime contracts. Keep these
            // as hard failures so a malformed preset is not silently hidden.
            if error_message.to_ascii_lowercase().contains("reasoning") {
                return Err(error);
            }
            let model_id = config.ai.models[index].id.clone();
            config.ai.models[index].enabled = false;
            diagnostics.push(ConfigDiagnostic {
                path: format!("ai.models[{index}]"),
                message: format!(
                    "Disabled invalid model '{}' during configuration recovery",
                    model_id
                ),
                code: "INVALID_MODEL_DISABLED".to_string(),
                severity: ConfigDiagnosticSeverity::Warning,
                recoverability: ConfigDiagnosticRecoverability::ModelDisabled,
            });
            log::warn!(
                "Disabled invalid model during configuration recovery: model_id={}, error={}",
                model_id,
                error_message
            );
        }
    }

    Ok(diagnostics)
}

#[derive(Debug, Clone, Default)]
pub struct ModelReferenceReconcileResult {
    pub invalidated_model_ids: Vec<String>,
    pub default_models_changed: bool,
    pub task_models_changed: bool,
    pub agent_model_defaults_changed: bool,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

impl ModelReferenceReconcileResult {
    pub fn is_noop(&self) -> bool {
        !self.default_models_changed
            && !self.task_models_changed
            && !self.agent_model_defaults_changed
    }
}

fn enabled_model_with_capability(
    config: &GlobalConfig,
    model_id: &str,
    capability: ModelCapability,
) -> bool {
    config.ai.models.iter().any(|model| {
        model.enabled && model.id == model_id && model.supports_capability(capability.clone())
    })
}

fn first_enabled_model_with_capability(
    config: &GlobalConfig,
    capability: ModelCapability,
) -> Option<String> {
    config
        .ai
        .models
        .iter()
        .find(|model| model.enabled && model.supports_capability(capability.clone()))
        .map(|model| model.id.clone())
}

fn diagnose_reference_repair(
    diagnostics: &mut Vec<ConfigDiagnostic>,
    path: &str,
    previous: Option<&str>,
    replacement: Option<&str>,
) {
    diagnostics.push(ConfigDiagnostic {
        path: path.to_string(),
        message: format!(
            "Repaired model reference from {:?} to {:?} to match the slot capability",
            previous, replacement
        ),
        code: "MODEL_REFERENCE_REPAIRED".to_string(),
        severity: ConfigDiagnosticSeverity::Warning,
        recoverability: ConfigDiagnosticRecoverability::AutoFix,
    });
}

#[derive(Clone, Copy)]
enum MissingSlotPolicy {
    FillFromFirstCapableModel,
    Preserve,
}

/// Reconciles every product model reference against both enablement and the
/// capability required by its consumer.
pub fn reconcile_model_references(config: &mut GlobalConfig) -> ModelReferenceReconcileResult {
    let snapshot = config.clone();
    let mut result = ModelReferenceReconcileResult::default();
    let mut invalidated = HashSet::new();

    let direct_text_reference_is_valid = |reference: &str| {
        matches!(reference, "primary" | "fast")
            || enabled_model_with_capability(&snapshot, reference, ModelCapability::TextChat)
    };

    let mut reconcile_task_model =
        |selection: &mut crate::service::config::types::TaskModelSelection,
         path: &str,
         allow_inherit: bool| {
            let previous = selection.fixed_model_id().map(str::to_string);
            let valid = match selection {
                crate::service::config::types::TaskModelSelection::Inherit => allow_inherit,
                crate::service::config::types::TaskModelSelection::Fixed { model_id } => {
                    direct_text_reference_is_valid(model_id)
                }
            };
            if !valid {
                if let Some(model_id) = previous.as_ref() {
                    invalidated.insert(model_id.clone());
                }
                *selection = crate::service::config::types::TaskModelSelection::Fixed {
                    model_id: "fast".to_string(),
                };
                result.task_models_changed = true;
                diagnose_reference_repair(
                    &mut result.diagnostics,
                    path,
                    previous.as_deref().or(Some("inherit")),
                    Some("fast"),
                );
            }
        };
    reconcile_task_model(
        &mut config.ai.task_models.session_title,
        "ai.task_models.session_title",
        true,
    );
    reconcile_task_model(
        &mut config.ai.task_models.git_commit,
        "ai.task_models.git_commit",
        false,
    );

    if !direct_text_reference_is_valid(&config.ai.agent_model_defaults.mode) {
        invalidated.insert(config.ai.agent_model_defaults.mode.clone());
        let previous = std::mem::replace(
            &mut config.ai.agent_model_defaults.mode,
            "primary".to_string(),
        );
        result.agent_model_defaults_changed = true;
        diagnose_reference_repair(
            &mut result.diagnostics,
            "ai.agent_model_defaults.mode",
            Some(&previous),
            Some("primary"),
        );
    }

    if config
        .ai
        .agent_model_defaults
        .subagents
        .default_selection
        .fixed_model_id()
        .is_some_and(|model_id| !direct_text_reference_is_valid(model_id))
    {
        let previous = config
            .ai
            .agent_model_defaults
            .subagents
            .default_selection
            .fixed_model_id()
            .map(str::to_string);
        if let Some(previous) = previous.as_ref() {
            invalidated.insert(previous.clone());
        }
        config.ai.agent_model_defaults.subagents.default_selection =
            SubagentModelSelection::fixed("fast");
        result.agent_model_defaults_changed = true;
        diagnose_reference_repair(
            &mut result.diagnostics,
            "ai.agent_model_defaults.subagents.default",
            previous.as_deref(),
            Some("fast"),
        );
    }

    if let std::collections::hash_map::Entry::Vacant(entry) = config
        .ai
        .agent_model_defaults
        .subagents
        .builtin
        .entry("ResearchSpecialist".to_string())
    {
        entry.insert(SubagentModelSelection::Inherit);
        result.agent_model_defaults_changed = true;
        result.diagnostics.push(ConfigDiagnostic {
            path: "ai.agent_model_defaults.subagents.builtin.ResearchSpecialist".to_string(),
            message: "Configured ResearchSpecialist to inherit the parent model so DeepResearch does not depend on an unrelated fast-model endpoint".to_string(),
            code: "RESEARCH_SPECIALIST_MODEL_DEFAULT_MIGRATED".to_string(),
            severity: ConfigDiagnosticSeverity::Warning,
            recoverability: ConfigDiagnosticRecoverability::AutoFix,
        });
    }

    config
        .ai
        .agent_model_defaults
        .subagents
        .builtin
        .retain(|subagent_id, selection| {
            let invalid = selection
                .fixed_model_id()
                .is_some_and(|model_id| !direct_text_reference_is_valid(model_id));
            if invalid {
                if let Some(model_id) = selection.fixed_model_id() {
                    invalidated.insert(model_id.to_string());
                    diagnose_reference_repair(
                        &mut result.diagnostics,
                        &format!("ai.agent_model_defaults.subagents.builtin.{subagent_id}"),
                        Some(model_id),
                        None,
                    );
                }
                result.agent_model_defaults_changed = true;
            }
            !invalid
        });

    if config
        .ai
        .agent_model_defaults
        .subagents
        .fork
        .fixed_model_id()
        .is_some_and(|model_id| !direct_text_reference_is_valid(model_id))
    {
        let previous = config
            .ai
            .agent_model_defaults
            .subagents
            .fork
            .fixed_model_id()
            .map(str::to_string);
        if let Some(previous) = previous.as_ref() {
            invalidated.insert(previous.clone());
        }
        config.ai.agent_model_defaults.subagents.fork = SubagentModelSelection::Inherit;
        result.agent_model_defaults_changed = true;
        diagnose_reference_repair(
            &mut result.diagnostics,
            "ai.agent_model_defaults.subagents.fork",
            previous.as_deref(),
            Some("inherit"),
        );
    }

    let mut reconcile_slot = |slot: &mut Option<String>,
                              path: &str,
                              capability: ModelCapability,
                              missing_policy: MissingSlotPolicy| {
        let previous = slot.clone();
        let valid = previous
            .as_deref()
            .is_some_and(|id| enabled_model_with_capability(&snapshot, id, capability.clone()));
        if valid || (previous.is_none() && matches!(missing_policy, MissingSlotPolicy::Preserve)) {
            return;
        }
        let replacement = first_enabled_model_with_capability(&snapshot, capability);
        if replacement == previous {
            return;
        }
        if let Some(previous) = previous.as_ref().filter(|id| !id.is_empty()) {
            invalidated.insert(previous.clone());
        }
        *slot = replacement;
        result.default_models_changed = true;
        diagnose_reference_repair(
            &mut result.diagnostics,
            path,
            previous.as_deref(),
            slot.as_deref(),
        );
    };

    reconcile_slot(
        &mut config.ai.default_models.primary,
        "ai.default_models.primary",
        ModelCapability::TextChat,
        MissingSlotPolicy::FillFromFirstCapableModel,
    );
    reconcile_slot(
        &mut config.ai.default_models.fast,
        "ai.default_models.fast",
        ModelCapability::TextChat,
        MissingSlotPolicy::Preserve,
    );
    reconcile_slot(
        &mut config.ai.default_models.image_understanding,
        "ai.default_models.image_understanding",
        ModelCapability::ImageUnderstanding,
        MissingSlotPolicy::Preserve,
    );
    reconcile_slot(
        &mut config.ai.default_models.image_generation,
        "ai.default_models.image_generation",
        ModelCapability::ImageGeneration,
        MissingSlotPolicy::Preserve,
    );
    reconcile_slot(
        &mut config.ai.default_models.search,
        "ai.default_models.search",
        ModelCapability::Search,
        MissingSlotPolicy::Preserve,
    );
    reconcile_slot(
        &mut config.ai.default_models.speech_recognition,
        "ai.default_models.speech_recognition",
        ModelCapability::SpeechRecognition,
        MissingSlotPolicy::Preserve,
    );

    result.invalidated_model_ids = invalidated.into_iter().collect();
    result.invalidated_model_ids.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::config::types::{AIModelConfig, ModelCapability, ModelCategory};

    #[test]
    fn pure_speech_models_drop_text_generation_sentinels() {
        let mut config = GlobalConfig::default();
        config.ai.models.push(AIModelConfig {
            id: "speech-cloud".to_string(),
            name: "Qwen ASR".to_string(),
            category: ModelCategory::SpeechRecognition,
            capabilities: vec![ModelCapability::SpeechRecognition],
            context_window: Some(0),
            max_tokens: Some(0),
            enabled: true,
            ..AIModelConfig::default()
        });

        let diagnostics = normalize_typed_config(&mut config);

        assert_eq!(config.ai.models[0].context_window, None);
        assert_eq!(config.ai.models[0].max_tokens, None);
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "MODEL_FIELD_NOT_APPLICABLE"));
    }

    #[test]
    fn mixed_text_and_speech_models_keep_generation_fields() {
        let mut config = GlobalConfig::default();
        config.ai.models.push(AIModelConfig {
            id: "mixed".to_string(),
            category: ModelCategory::GeneralChat,
            capabilities: vec![
                ModelCapability::TextChat,
                ModelCapability::SpeechRecognition,
            ],
            context_window: Some(64_000),
            max_tokens: Some(8_000),
            ..AIModelConfig::default()
        });

        assert!(normalize_typed_config(&mut config).is_empty());
        assert_eq!(config.ai.models[0].context_window, Some(64_000));
        assert_eq!(config.ai.models[0].max_tokens, Some(8_000));
    }

    #[test]
    fn default_slots_reconcile_by_capability() {
        let mut config = GlobalConfig::default();
        config.ai.models = vec![
            AIModelConfig {
                id: "speech".to_string(),
                enabled: true,
                category: ModelCategory::SpeechRecognition,
                capabilities: vec![ModelCapability::SpeechRecognition],
                ..AIModelConfig::default()
            },
            AIModelConfig {
                id: "text".to_string(),
                enabled: true,
                category: ModelCategory::GeneralChat,
                capabilities: vec![ModelCapability::TextChat],
                ..AIModelConfig::default()
            },
        ];
        config.ai.default_models.primary = Some("speech".to_string());
        config.ai.default_models.fast = Some("speech".to_string());
        config.ai.default_models.speech_recognition = Some("text".to_string());

        let result = reconcile_model_references(&mut config);

        assert_eq!(config.ai.default_models.primary.as_deref(), Some("text"));
        assert_eq!(config.ai.default_models.fast.as_deref(), Some("text"));
        assert_eq!(
            config.ai.default_models.speech_recognition.as_deref(),
            Some("speech")
        );
        assert!(result.default_models_changed);
    }

    #[test]
    fn missing_fast_slot_is_preserved_and_resolves_to_primary() {
        let mut config = GlobalConfig::default();
        config.ai.models = vec![
            AIModelConfig {
                id: "first-text".to_string(),
                enabled: true,
                category: ModelCategory::GeneralChat,
                capabilities: vec![ModelCapability::TextChat],
                ..AIModelConfig::default()
            },
            AIModelConfig {
                id: "primary-text".to_string(),
                enabled: true,
                category: ModelCategory::GeneralChat,
                capabilities: vec![ModelCapability::TextChat],
                ..AIModelConfig::default()
            },
        ];
        config.ai.default_models.primary = Some("primary-text".to_string());
        config.ai.default_models.fast = None;

        let result = reconcile_model_references(&mut config);

        assert_eq!(config.ai.default_models.fast, None);
        assert_eq!(
            config.ai.resolve_model_selection("fast").as_deref(),
            Some("primary-text")
        );
        assert!(!result.default_models_changed);
    }

    #[test]
    fn task_models_reconcile_invalid_fixed_and_git_inherit_to_fast() {
        let mut config = GlobalConfig::default();
        config.ai.task_models.session_title =
            crate::service::config::types::TaskModelSelection::Fixed {
                model_id: "missing".to_string(),
            };
        config.ai.task_models.git_commit =
            crate::service::config::types::TaskModelSelection::Inherit;

        let result = reconcile_model_references(&mut config);

        assert!(result.task_models_changed);
        assert_eq!(
            config.ai.task_models.session_title.fixed_model_id(),
            Some("fast")
        );
        assert_eq!(
            config.ai.task_models.git_commit.fixed_model_id(),
            Some("fast")
        );
        assert!(result
            .invalidated_model_ids
            .contains(&"missing".to_string()));
    }

    #[test]
    fn legacy_research_specialist_default_migrates_to_parent_inheritance() {
        let mut config = GlobalConfig::default();
        config
            .ai
            .agent_model_defaults
            .subagents
            .builtin
            .remove("ResearchSpecialist");

        let result = reconcile_model_references(&mut config);

        assert!(result.agent_model_defaults_changed);
        assert_eq!(
            config
                .ai
                .agent_model_defaults
                .subagents
                .builtin
                .get("ResearchSpecialist"),
            Some(&SubagentModelSelection::Inherit)
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "RESEARCH_SPECIALIST_MODEL_DEFAULT_MIGRATED" }));
    }

    #[test]
    fn explicit_research_specialist_model_override_is_preserved() {
        let mut config = GlobalConfig::default();
        config.ai.agent_model_defaults.subagents.builtin.insert(
            "ResearchSpecialist".to_string(),
            SubagentModelSelection::fixed("fast"),
        );

        let result = reconcile_model_references(&mut config);

        assert!(!result.agent_model_defaults_changed);
        assert_eq!(
            config
                .ai
                .agent_model_defaults
                .subagents
                .builtin
                .get("ResearchSpecialist"),
            Some(&SubagentModelSelection::fixed("fast"))
        );
    }

    #[tokio::test]
    async fn global_ai_errors_do_not_disable_individually_valid_models() {
        let mut config = GlobalConfig::default();
        config.ai.stream_idle_timeout_secs = Some(0);
        config.ai.models.push(AIModelConfig {
            id: "valid-text".to_string(),
            name: "Valid text model".to_string(),
            provider: "openai".to_string(),
            model_name: "text-model".to_string(),
            base_url: "https://example.com/v1".to_string(),
            enabled: true,
            capabilities: vec![ModelCapability::TextChat],
            context_window: Some(64_000),
            ..AIModelConfig::default()
        });

        let diagnostics = isolate_invalid_ai_models(&mut config)
            .await
            .expect("model isolation should succeed");

        assert!(diagnostics.is_empty());
        assert!(config.ai.models[0].enabled);
    }
}
