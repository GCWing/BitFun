//! Configuration provider implementations
//!
//! Providers for different configuration sections, responsible for defaults, validation,
//! and change handling.

use super::types::*;
use crate::util::errors::*;
use async_trait::async_trait;
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
                    if context_window == 0 {
                        return Err(BitFunError::validation(format!(
                            "Model '{}' context_window must be greater than 0",
                            model.name
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
                    if temperature < 0.0 || temperature > 2.0 {
                        warnings.push(format!(
                            "Model '{}' temperature should be between 0 and 2",
                            model.name
                        ));
                    }
                }
            }

            for (agent_name, model_id) in &ai_config.agent_models {
                if !ai_config.models.iter().any(|m| m.id == *model_id)
                    && model_id != "primary"
                    && model_id != "fast"
                {
                    return Err(BitFunError::validation(format!(
                        "Primary Agent '{}' configured model '{}' does not exist",
                        agent_name, model_id
                    )));
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

/// Theme configuration provider (legacy, for a single theme).
pub struct ThemeConfigProvider;

#[async_trait]
impl ConfigProvider for ThemeConfigProvider {
    fn name(&self) -> &str {
        "theme"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("theme", ThemeConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let warnings = Vec::new();

        if let Ok(theme_config) = serde_json::from_value::<ThemeConfig>(config.clone()) {
            let colors = [
                &theme_config.colors.primary,
                &theme_config.colors.secondary,
                &theme_config.colors.background,
                &theme_config.colors.surface,
                &theme_config.colors.text,
                &theme_config.colors.accent,
                &theme_config.colors.success,
                &theme_config.colors.warning,
                &theme_config.colors.error,
            ];

            for color in colors {
                if !color.starts_with("#") && !color.starts_with("rgb") && !color.starts_with("hsl")
                {
                    return Err(BitFunError::validation(format!(
                        "Invalid color format: {}",
                        color
                    )));
                }
            }
        } else {
            return Err(BitFunError::validation(
                "Invalid theme config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(theme_config) = serde_json::from_value::<ThemeConfig>(new_config.clone()) {
            info!("Theme config changed to: {}", theme_config.display_name);
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

/// Theme system configuration provider (new, supports theme management).
pub struct ThemesConfigProvider;

#[async_trait]
impl ConfigProvider for ThemesConfigProvider {
    fn name(&self) -> &str {
        "themes"
    }

    fn get_default_config(&self) -> serde_json::Value {
        serialize_default_config("themes", ThemesConfig::default())
    }

    async fn validate_config(&self, config: &serde_json::Value) -> BitFunResult<Vec<String>> {
        let warnings = Vec::new();

        if let Ok(_themes_config) = serde_json::from_value::<ThemesConfig>(config.clone()) {
        } else {
            return Err(BitFunError::validation(
                "Invalid themes config format".to_string(),
            ));
        }

        Ok(warnings)
    }

    async fn on_config_changed(
        &self,
        _old_config: &serde_json::Value,
        new_config: &serde_json::Value,
    ) -> BitFunResult<()> {
        if let Ok(themes_config) = serde_json::from_value::<ThemesConfig>(new_config.clone()) {
            info!(
                "Themes config changed: current theme = {}",
                themes_config.current
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
                "Editor config changed: font_size={}, theme={}",
                editor_config.font_size, editor_config.theme
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
        registry.register(Box::new(ThemeConfigProvider));
        registry.register(Box::new(ThemesConfigProvider));
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
    pub fn get_provider(&self, name: &str) -> Option<&Box<dyn ConfigProvider>> {
        self.providers.get(name)
    }

    /// Returns all provider names.
    pub fn get_provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Builds the default configuration.
    pub fn get_default_config(&self) -> GlobalConfig {
        let mut config = GlobalConfig::default();

        if let Some(provider) = self.get_provider("app") {
            if let Ok(app_config) = serde_json::from_value(provider.get_default_config()) {
                config.app = app_config;
            }
        }

        if let Some(provider) = self.get_provider("theme") {
            if let Ok(theme_config) = serde_json::from_value(provider.get_default_config()) {
                config.theme = theme_config;
            }
        }

        if let Some(provider) = self.get_provider("themes") {
            if let Ok(themes_config) = serde_json::from_value(provider.get_default_config()) {
                config.themes = Some(themes_config);
            }
        }

        if let Some(provider) = self.get_provider("editor") {
            if let Ok(editor_config) = serde_json::from_value(provider.get_default_config()) {
                config.editor = editor_config;
            }
        }

        if let Some(provider) = self.get_provider("terminal") {
            if let Ok(terminal_config) = serde_json::from_value(provider.get_default_config()) {
                config.terminal = terminal_config;
            }
        }

        if let Some(provider) = self.get_provider("workspace") {
            if let Ok(workspace_config) = serde_json::from_value(provider.get_default_config()) {
                config.workspace = workspace_config;
            }
        }

        if let Some(provider) = self.get_provider("ai") {
            if let Ok(ai_config) = serde_json::from_value(provider.get_default_config()) {
                config.ai = ai_config;
            }
        }

        config
    }

    /// Validates the full configuration.
    pub async fn validate_config(
        &self,
        config: &GlobalConfig,
    ) -> BitFunResult<ConfigValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if let Some(provider) = self.get_provider("app") {
            let app_value = serde_json::to_value(&config.app)?;
            match provider.validate_config(&app_value).await {
                Ok(provider_warnings) => {
                    warnings.extend(provider_warnings.into_iter().map(|msg| {
                        ConfigValidationWarning {
                            path: "app".to_string(),
                            message: msg,
                            code: "VALIDATION_WARNING".to_string(),
                            severity: "warning".to_string(),
                        }
                    }))
                }
                Err(e) => errors.push(ConfigValidationError {
                    path: "app".to_string(),
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
            "theme" => Ok(serde_json::to_value(&config.theme)?),
            "themes" => Ok(serde_json::to_value(&config.themes)?),
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
