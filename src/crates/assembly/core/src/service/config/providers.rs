//! Configuration provider implementations
//!
//! Providers for different configuration sections, responsible for defaults, validation,
//! and change handling.

use super::types::*;
#[cfg(feature = "ai-adapter-runtime")]
use crate::infrastructure::ai::reasoning_catalog::{
    load_models_dev_reasoning_catalog_without_refresh, project_model_reasoning_catalog,
};
#[cfg(feature = "ai-adapter-runtime")]
use crate::infrastructure::ai::AIClient;
use crate::util::errors::*;
use async_trait::async_trait;
use bitfun_core_types::ReasoningCatalogBinding;
#[cfg(test)]
use bitfun_core_types::{ReasoningConfig, ReasoningPreset, ReasoningPresetAction};
use log::{error, info};
use std::collections::HashMap;

fn serialize_default_config(section: &str, value: impl serde::Serialize) -> serde_json::Value {
    match serde_json::to_value(value) {
        Ok(serialized) => serialized,
        Err(err) => {
            error!(
                "Failed to serialize default config section: section={}, error={}",
                section, err
            );
            serde_json::Value::Object(serde_json::Map::new())
        }
    }
}

/// AI configuration provider.
pub struct AIConfigProvider;

#[async_trait]
impl ConfigProvider for AIConfigProvider {
    fn name(&self) -> &str {
        "ai"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("ai", AIConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let mut warnings = Vec::new();

        if let Ok(ai_config) = serde_json::from_value::<AIConfig>(config.clone()) {
            #[cfg(feature = "ai-adapter-runtime")]
            let models_dev = if ai_config.models.iter().any(|model| {
                model.reasoning.as_ref().is_some_and(|reasoning| {
                    !matches!(reasoning.catalog, ReasoningCatalogBinding::Disabled)
                })
            }) {
                Some(load_models_dev_reasoning_catalog_without_refresh().await)
            } else {
                None
            };

            if let Some(stream_idle_timeout_secs) = ai_config.stream_idle_timeout_secs {
                if stream_idle_timeout_secs == 0 {
                    return Err(BitFunError::validation(
                        "AI stream_idle_timeout_secs must be greater than 0".to_string(),
                    ));
                }
            }

            if let Some(stream_ttft_timeout_secs) = ai_config.stream_ttft_timeout_secs {
                if stream_ttft_timeout_secs == 0 {
                    return Err(BitFunError::validation(
                        "AI stream_ttft_timeout_secs must be greater than 0".to_string(),
                    ));
                }
            }

            for (index, model) in ai_config.models.iter().enumerate() {
                if model.name.trim().is_empty() {
                    return Err(BitFunError::validation(format!(
                        "Model name is required at index {}",
                        index
                    )));
                }
                if model.provider.trim().is_empty() {
                    return Err(BitFunError::validation(format!(
                        "Model provider is required at index {}",
                        index
                    )));
                }
                if model.api_key.trim().is_empty() {
                    warnings.push(format!("Model '{}' has empty API key", model.name));
                }
                if let Some(context_window) = model.context_window {
                    if context_window < MIN_MODEL_CONTEXT_WINDOW_TOKENS {
                        return Err(BitFunError::validation(format!(
                            "Model '{}' context_window must be at least {}",
                            model.name, MIN_MODEL_CONTEXT_WINDOW_TOKENS
                        )));
                    }
                }
                if let Some(max_tokens) = model.max_tokens {
                    if max_tokens == 0 {
                        return Err(BitFunError::validation(format!(
                            "Model '{}' max_tokens must be greater than 0",
                            model.name
                        )));
                    }
                }
                if let Some(temperature) = model.temperature {
                    if !temperature.is_nan() && !(0.0..=2.0).contains(&temperature) {
                        warnings.push(format!(
                            "Model '{}' temperature should be between 0 and 2",
                            model.name
                        ));
                    }
                }

                if let Some(reasoning) = model.reasoning.as_ref() {
                    reasoning.validate_schema().map_err(|message| {
                        BitFunError::validation(format!(
                            "Model '{}' reasoning config is invalid at index {}: {}",
                            model.name, index, message
                        ))
                    })?;

                    if let Some(default_preset) = reasoning.default_preset.as_deref() {
                        #[cfg(feature = "ai-adapter-runtime")]
                        {
                            let projection = project_model_reasoning_catalog(
                                model,
                                models_dev
                                    .as_ref()
                                    .and_then(|snapshot| snapshot.catalog.as_deref()),
                            );
                            if projection.default_preset.as_deref() != Some(default_preset) {
                                return Err(BitFunError::validation(format!(
                                    "Model '{}' reasoning default preset '{}' is not available at index {}",
                                    model.name, default_preset, index
                                )));
                            }
                        }

                        #[cfg(not(feature = "ai-adapter-runtime"))]
                        if reasoning.preset(default_preset).is_none()
                            && matches!(reasoning.catalog, ReasoningCatalogBinding::Disabled)
                        {
                            return Err(BitFunError::validation(format!(
                                "Model '{}' reasoning default preset '{}' is not available at index {}",
                                model.name, default_preset, index
                            )));
                        }
                    }

                    #[cfg(feature = "ai-adapter-runtime")]
                    {
                        let runtime_config = <crate::util::types::AIConfig as TryFrom<
                            AIModelConfig,
                        >>::try_from(model.clone())
                        .map_err(|message| {
                            BitFunError::validation(format!(
                                "Model '{}' reasoning target is invalid at index {}: {}",
                                model.name, index, message
                            ))
                        })?;
                        let client = AIClient::new(runtime_config);
                        let projection = project_model_reasoning_catalog(
                            model,
                            models_dev
                                .as_ref()
                                .and_then(|snapshot| snapshot.catalog.as_deref()),
                        );
                        for preset in reasoning
                            .presets
                            .iter()
                            .filter(|preset| !preset.disabled && !preset.actions.is_empty())
                        {
                            let preset_id = preset.id.trim();
                            let descriptor = projection
                                .presets
                                .iter()
                                .find(|descriptor| descriptor.id == preset_id)
                                .ok_or_else(|| {
                                    BitFunError::validation(format!(
                                        "Model '{}' reasoning preset '{}' is not available at index {}",
                                        model.name, preset_id, index
                                    ))
                                })?;
                            client.validate_reasoning_preset(descriptor).map_err(|error| {
                                BitFunError::validation(format!(
                                    "Model '{}' reasoning preset '{}' is unsupported at index {}: {}",
                                    model.name, preset_id, index, error
                                ))
                            })?;
                        }
                    }
                }
            }

            for (func_agent_name, model_id) in &ai_config.func_agent_models {
                if !ai_config.models.iter().any(|m| m.id == *model_id)
                    && model_id != "primary"
                    && model_id != "fast"
                {
                    return Err(BitFunError::validation(format!(
                        "Function Agent '{}' configured model '{}' does not exist",
                        func_agent_name, model_id
                    )));
                }
            }
        } else {
            return Err(BitFunError::validation(
                "Invalid AI config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(ai_config) = serde_json::from_value::<AIConfig>(new_config.clone()) {
            info!(
                "AI config changed: {} models configured",
                ai_config.models.len()
            );
            if let Some(text_chat_model) = ai_config.default_models.primary {
                info!("Primary model: {}", text_chat_model);
            }
            if let Some(fast_model) = ai_config.default_models.fast {
                info!("Fast model: {}", fast_model);
            }
        }
        Ok(())
    }

    async fn migrate_config(
        &self,
        version: &str,
        config: serde_json::Value,
    ) -> BitFunResult<serde_json::Value> {
        match version {
            "0.1.0" => {
                if let Ok(mut ai_config) = serde_json::from_value::<AIConfig>(config.clone()) {
                    for model in &mut ai_config.models {
                        if config.get("enabled").is_none() {
                            model.enabled = true;
                        }
                    }
                    Ok(serde_json::to_value(ai_config)?)
                } else {
                    Ok(config)
                }
            }
            _ => Ok(config),
        }
    }
}

/// Web UI appearance selection provider.
pub struct AppearanceConfigProvider;

#[async_trait]
impl ConfigProvider for AppearanceConfigProvider {
    fn name(&self) -> &str {
        "appearance"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("appearance", AppearanceConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let warnings = Vec::new();

        if let Ok(appearance_config) = serde_json::from_value::<AppearanceConfig>(config.clone()) {
            if appearance_config.selection.trim().is_empty() {
                return Err(BitFunError::validation(
                    "Appearance selection must not be empty".to_string(),
                ));
            }
        } else {
            return Err(BitFunError::validation(
                "Invalid appearance config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(appearance_config) =
            serde_json::from_value::<AppearanceConfig>(new_config.clone())
        {
            info!(
                "Appearance config changed: selection = {}",
                appearance_config.selection
            );
        }
        Ok(())
    }

    async fn migrate_config(
        &self,
        _version: &str,
        config: serde_json::Value,
    ) -> BitFunResult<serde_json::Value> {
        Ok(config)
    }
}

/// Editor configuration provider.
pub struct EditorConfigProvider;

#[async_trait]
impl ConfigProvider for EditorConfigProvider {
    fn name(&self) -> &str {
        "editor"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("editor", EditorConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let mut warnings = Vec::new();

        if let Ok(editor_config) = serde_json::from_value::<EditorConfig>(config.clone()) {
            if editor_config.font_size < 8 || editor_config.font_size > 72 {
                warnings.push("Font size should be between 8 and 72".to_string());
            }

            if editor_config.tab_size < 1 || editor_config.tab_size > 8 {
                warnings.push("Tab size should be between 1 and 8".to_string());
            }

            if editor_config.line_height < 1.0 || editor_config.line_height > 3.0 {
                warnings.push("Line height should be between 1.0 and 3.0".to_string());
            }
        } else {
            return Err(BitFunError::validation(
                "Invalid editor config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(editor_config) = serde_json::from_value::<EditorConfig>(new_config.clone()) {
            info!(
                "Editor config changed: font_size={}",
                editor_config.font_size
            );
        }
        Ok(())
    }

    async fn migrate_config(
        &self,
        _version: &str,
        config: serde_json::Value,
    ) -> BitFunResult<serde_json::Value> {
        Ok(config)
    }
}

/// Terminal configuration provider.
pub struct TerminalConfigProvider;

#[async_trait]
impl ConfigProvider for TerminalConfigProvider {
    fn name(&self) -> &str {
        "terminal"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("terminal", TerminalConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let mut warnings = Vec::new();

        if let Ok(terminal_config) = serde_json::from_value::<TerminalConfig>(config.clone()) {
            if terminal_config.font_size < 8 || terminal_config.font_size > 72 {
                warnings.push("Terminal font size should be between 8 and 72".to_string());
            }

            if terminal_config.scrollback > 100000 {
                warnings.push("Large scrollback buffer may impact performance".to_string());
            }

            if terminal_config.terminal_panel_position != "right"
                && terminal_config.terminal_panel_position != "bottom"
            {
                warnings.push(
                    "Terminal panel position should be either 'right' or 'bottom'".to_string(),
                );
            }
        } else {
            return Err(BitFunError::validation(
                "Invalid terminal config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(terminal_config) = serde_json::from_value::<TerminalConfig>(new_config.clone()) {
            info!(
                "Terminal config changed: shell={}, font_size={}",
                terminal_config.default_shell, terminal_config.font_size
            );
        }
        Ok(())
    }

    async fn migrate_config(
        &self,
        _version: &str,
        config: serde_json::Value,
    ) -> BitFunResult<serde_json::Value> {
        Ok(config)
    }
}

/// Workspace configuration provider.
pub struct WorkspaceConfigProvider;

#[async_trait]
impl ConfigProvider for WorkspaceConfigProvider {
    fn name(&self) -> &str {
        "workspace"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("workspace", WorkspaceConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let mut warnings = Vec::new();

        if let Ok(workspace_config) = serde_json::from_value::<WorkspaceConfig>(config.clone()) {
            if workspace_config.max_file_size > 1024 * 1024 * 1024 {
                warnings.push("Very large max file size may impact performance".to_string());
            }

            if workspace_config.exclude_patterns.is_empty() {
                warnings
                    .push("No exclude patterns defined, may scan unnecessary files".to_string());
            }
        } else {
            return Err(BitFunError::validation(
                "Invalid workspace config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(workspace_config) = serde_json::from_value::<WorkspaceConfig>(new_config.clone())
        {
            info!(
                "Workspace config changed: {} exclude patterns",
                workspace_config.exclude_patterns.len()
            );
        }
        Ok(())
    }

    async fn migrate_config(
        &self,
        _version: &str,
        config: serde_json::Value,
    ) -> BitFunResult<serde_json::Value> {
        Ok(config)
    }
}

/// App configuration provider.
pub struct AppConfigProvider;

#[async_trait]
impl ConfigProvider for AppConfigProvider {
    fn name(&self) -> &str {
        "app"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("app", AppConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let mut warnings = Vec::new();

        if let Ok(app_config) = serde_json::from_value::<AppConfig>(config.clone()) {
            if app_config.zoom_level < 0.5 || app_config.zoom_level > 3.0 {
                warnings.push("Zoom level should be between 0.5 and 3.0".to_string());
            }

            if app_config.sidebar.width < 200 || app_config.sidebar.width > 800 {
                warnings.push("Sidebar width should be between 200 and 800 pixels".to_string());
            }

            let valid_log_level = matches!(
                app_config.logging.level.to_lowercase().as_str(),
                "trace" | "debug" | "info" | "warn" | "error" | "off"
            );
            if !valid_log_level {
                return Err(BitFunError::validation(format!(
                    "Invalid app.logging.level '{}': expected one of trace/debug/info/warn/error/off",
                    app_config.logging.level
                )));
            }
        } else {
            return Err(BitFunError::validation(
                "Invalid app config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(app_config) = serde_json::from_value::<AppConfig>(new_config.clone()) {
            info!(
                "App config changed: language={}, zoom_level={}, log_level={}",
                app_config.language, app_config.zoom_level, app_config.logging.level
            );
        }
        Ok(())
    }

    async fn migrate_config(
        &self,
        _version: &str,
        config: serde_json::Value,
    ) -> BitFunResult<serde_json::Value> {
        Ok(config)
    }
}

/// Configuration provider registry.
pub struct ConfigProviderRegistry {
    providers: HashMap<String, Box<dyn ConfigProvider>>,
}

impl ConfigProviderRegistry {
    /// Creates the default provider registry.
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };

        registry.register(Box::new(AIConfigProvider));
        registry.register(Box::new(AppearanceConfigProvider));
        registry.register(Box::new(EditorConfigProvider));
        registry.register(Box::new(TerminalConfigProvider));
        registry.register(Box::new(WorkspaceConfigProvider));
        registry.register(Box::new(AppConfigProvider));

        registry
    }

    /// Registers a configuration provider.
    pub fn register(&mut self, provider: Box<dyn ConfigProvider>) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    /// Gets a provider by name.
    pub fn get_provider(&self, name: &str) -> Option<&dyn ConfigProvider> {
        self.providers.get(name).map(Box::as_ref)
    }

    /// Returns all provider names.
    pub fn get_provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Builds the default configuration.
    pub fn get_default_config(&self) -> GlobalConfig {
        GlobalConfig::default()
    }

    /// Validates the full configuration.
    pub async fn validate_config(
        &self,
        config: &GlobalConfig,
    ) -> BitFunResult<ConfigValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for provider_name in ["ai", "appearance", "editor", "terminal", "workspace", "app"] {
            let Some(provider) = self.get_provider(provider_name) else {
                continue;
            };
            let section_value = self.get_config_section(provider_name, config)?;
            match provider.validate_config(&section_value).await {
                Ok(provider_warnings) => {
                    warnings.extend(provider_warnings.into_iter().map(|msg| {
                        ConfigValidationWarning {
                            path: provider_name.to_string(),
                            message: msg,
                            code: "VALIDATION_WARNING".to_string(),
                            severity: "warning".to_string(),
                        }
                    }))
                }
                Err(e) => errors.push(ConfigValidationError {
                    path: provider_name.to_string(),
                    message: e.to_string(),
                    code: "VALIDATION_ERROR".to_string(),
                    severity: "error".to_string(),
                }),
            }
        }

        Ok(ConfigValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    /// Notifies providers of a configuration change.
    pub async fn notify_config_changed(
        &self,
        path: &str,
        old_config: &GlobalConfig,
        new_config: &GlobalConfig,
    ) -> BitFunResult<()> {
        let provider_name = path.split('.').next().unwrap_or(path);

        if let Some(provider) = self.get_provider(provider_name) {
            let old_value = self.get_config_section(provider_name, old_config)?;
            let new_value = self.get_config_section(provider_name, new_config)?;

            provider.on_config_changed(&old_value, &new_value).await?;
        }

        Ok(())
    }

    /// Gets a specific configuration section.
    fn get_config_section(
        &self,
        section: &str,
        config: &GlobalConfig,
    ) -> BitFunResult<serde_json::Value> {
        match section {
            "app" => Ok(serde_json::to_value(&config.app)?),
            "appearance" => Ok(serde_json::to_value(&config.appearance)?),
            "editor" => Ok(serde_json::to_value(&config.editor)?),
            "terminal" => Ok(serde_json::to_value(&config.terminal)?),
            "workspace" => Ok(serde_json::to_value(&config.workspace)?),
            "ai" => Ok(serde_json::to_value(&config.ai)?),
            _ => Err(BitFunError::validation(format!(
                "Unknown config section: {}",
                section
            ))),
        }
    }
}

impl Default for ConfigProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_with_reasoning(reasoning: ReasoningConfig) -> AIModelConfig {
        AIModelConfig {
            id: "reasoning-model".to_string(),
            name: "Reasoning model".to_string(),
            provider: "responses".to_string(),
            model_name: "gpt-5.4".to_string(),
            base_url: "https://api.openai.com/v1/responses".to_string(),
            enabled: true,
            reasoning: Some(reasoning),
            ..AIModelConfig::default()
        }
    }

    async fn validate_reasoning(reasoning: ReasoningConfig) -> BitFunResult<Vec<String>> {
        let mut config = AIConfig::default();
        config.models.push(model_with_reasoning(reasoning));
        AIConfigProvider
            .validate_config(&serde_json::to_value(config)?)
            .await
    }

    #[tokio::test]
    async fn rejects_a_model_context_window_smaller_than_32k() {
        let mut config = AIConfig::default();
        config.models.push(AIModelConfig {
            name: "Test model".to_string(),
            provider: "openai".to_string(),
            context_window: Some(MIN_MODEL_CONTEXT_WINDOW_TOKENS - 1),
            ..AIModelConfig::default()
        });
        let value = serde_json::to_value(config).expect("AI config should serialize");

        let error = AIConfigProvider
            .validate_config(&value)
            .await
            .expect_err("small context windows must be rejected");

        assert!(error
            .to_string()
            .contains("context_window must be at least 32000"));
    }

    #[tokio::test]
    async fn rejects_invalid_canonical_reasoning_actions() {
        for (action, expected) in [
            (
                ReasoningPresetAction::BudgetTokens { value: 0 },
                "budget_tokens value must be greater than 0",
            ),
            (
                ReasoningPresetAction::RequestPatch {
                    body: serde_json::json!(["invalid"]),
                },
                "request_patch body must be a JSON object",
            ),
        ] {
            let error = validate_reasoning(ReasoningConfig {
                default_preset: Some("custom".to_string()),
                presets: vec![ReasoningPreset {
                    id: "custom".to_string(),
                    actions: vec![action],
                    ..ReasoningPreset::default()
                }],
                ..ReasoningConfig::default()
            })
            .await
            .expect_err("invalid reasoning schema must be rejected");

            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[cfg(feature = "ai-adapter-runtime")]
    #[tokio::test]
    async fn rejects_reasoning_actions_unsupported_by_the_configured_target() {
        let error = validate_reasoning(ReasoningConfig {
            catalog: ReasoningCatalogBinding::Disabled,
            default_preset: Some("on".to_string()),
            presets: vec![ReasoningPreset {
                id: "on".to_string(),
                actions: vec![ReasoningPresetAction::Toggle { enabled: true }],
                ..ReasoningPreset::default()
            }],
        })
        .await
        .expect_err("Responses must reject a generic toggle action");

        assert!(error.to_string().contains("unsupported"), "{error}");
    }

    #[tokio::test]
    async fn rejects_duplicate_and_unavailable_default_presets() {
        let duplicate = validate_reasoning(ReasoningConfig {
            presets: vec![
                ReasoningPreset {
                    id: "same".to_string(),
                    actions: vec![ReasoningPresetAction::Toggle { enabled: true }],
                    ..ReasoningPreset::default()
                },
                ReasoningPreset {
                    id: "same".to_string(),
                    actions: vec![ReasoningPresetAction::Toggle { enabled: false }],
                    ..ReasoningPreset::default()
                },
            ],
            ..ReasoningConfig::default()
        })
        .await
        .expect_err("duplicate preset IDs must be rejected");
        assert!(duplicate.to_string().contains("duplicate preset ID 'same'"));

        let unavailable = validate_reasoning(ReasoningConfig {
            catalog: ReasoningCatalogBinding::Disabled,
            default_preset: Some("missing".to_string()),
            ..ReasoningConfig::default()
        })
        .await
        .expect_err("missing default preset must be rejected");
        assert!(unavailable
            .to_string()
            .contains("default preset 'missing' is not available"));
    }

    #[cfg(feature = "ai-adapter-runtime")]
    #[tokio::test]
    async fn accepts_generated_models_dev_default_preset() {
        validate_reasoning(ReasoningConfig {
            catalog: ReasoningCatalogBinding::Auto,
            default_preset: Some("high".to_string()),
            ..ReasoningConfig::default()
        })
        .await
        .expect("bundled models.dev preset should be valid");
    }

    #[tokio::test]
    async fn registry_runs_ai_validation() {
        let mut config = GlobalConfig::default();
        config.ai.models.push(model_with_reasoning(ReasoningConfig {
            presets: vec![ReasoningPreset {
                id: "bad".to_string(),
                actions: vec![ReasoningPresetAction::BudgetTokens { value: 0 }],
                ..ReasoningPreset::default()
            }],
            ..ReasoningConfig::default()
        }));

        let validation = ConfigProviderRegistry::new()
            .validate_config(&config)
            .await
            .expect("registry validation result");

        assert!(!validation.valid);
        assert_eq!(validation.errors[0].path, "ai");
        assert!(validation.errors[0]
            .message
            .contains("budget_tokens value must be greater than 0"));
    }
}
