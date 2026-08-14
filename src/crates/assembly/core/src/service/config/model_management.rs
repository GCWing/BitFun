//! Secret-safe model management use cases owned by the configuration domain.

use std::collections::HashMap;

use super::{AIConfig, AIModelConfig, AuthConfig, ConfigService, GlobalConfig, ReasoningConfig};
use crate::{BitFunError, BitFunResult};

#[derive(Clone)]
pub enum SecretUpdate<T> {
    Preserve,
    Replace(T),
    Clear,
}

impl<T> std::fmt::Debug for SecretUpdate<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Preserve => "Preserve",
            Self::Replace(_) => "Replace(<redacted>)",
            Self::Clear => "Clear",
        })
    }
}

#[derive(Clone, Debug)]
pub struct AIModelManagementMutation {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,
    pub api_key: Option<SecretUpdate<String>>,
    pub custom_headers: Option<SecretUpdate<HashMap<String, String>>>,
    pub custom_request_body: Option<SecretUpdate<String>>,
    pub context_window: Option<u32>,
    pub max_tokens: Option<u32>,
    pub enabled: bool,
    pub reasoning: Option<ReasoningConfig>,
    pub inline_think_in_text: bool,
    pub skip_ssl_verify: bool,
    pub custom_headers_mode: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AIModelManagementSummary {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,
    pub enabled: bool,
    pub context_window: Option<u32>,
    pub max_tokens: Option<u32>,
    pub api_key_configured: bool,
    pub custom_header_names: Vec<String>,
    pub custom_request_body_configured: bool,
    pub auth_source: String,
}

#[derive(Clone, Debug)]
pub struct AIModelManagementProjection {
    pub summary: AIModelManagementSummary,
    pub reasoning_preset_options: Vec<String>,
    pub reasoning: Option<ReasoningConfig>,
    pub inline_think_in_text: bool,
    pub skip_ssl_verify: bool,
    pub custom_headers_mode: String,
}

#[derive(Clone, Debug)]
pub struct AIModelManagementCatalog {
    pub models: Vec<AIModelManagementSummary>,
    pub primary_model_id: Option<String>,
    pub fast_model_id: Option<String>,
    pub mode_default_model_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AIModelDefaultSlot {
    Primary,
    Mode,
}

impl AIModelManagementSummary {
    fn from_model(model: &AIModelConfig) -> Self {
        let mut custom_header_names = model
            .custom_headers
            .as_ref()
            .map(|headers| headers.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        custom_header_names.sort();
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            provider: model.provider.clone(),
            model_name: model.model_name.clone(),
            base_url: model.base_url.clone(),
            enabled: model.enabled,
            context_window: model.context_window,
            max_tokens: model.max_tokens,
            api_key_configured: !model.api_key.is_empty(),
            custom_header_names,
            custom_request_body_configured: model.custom_request_body.is_some(),
            auth_source: match &model.auth {
                AuthConfig::ApiKey => "api_key".to_string(),
                AuthConfig::Subscription { provider, .. } => {
                    format!("subscription:{provider:?}").to_ascii_lowercase()
                }
            },
        }
    }
}

impl AIModelManagementProjection {
    fn from_model(model: &AIModelConfig) -> Self {
        Self {
            summary: AIModelManagementSummary::from_model(model),
            reasoning_preset_options: model
                .reasoning
                .as_ref()
                .map(|reasoning| {
                    reasoning
                        .presets
                        .iter()
                        .map(|preset| preset.id.clone())
                        .collect()
                })
                .unwrap_or_default(),
            reasoning: model.reasoning.clone(),
            inline_think_in_text: model.inline_think_in_text,
            skip_ssl_verify: model.skip_ssl_verify,
            custom_headers_mode: model
                .custom_headers_mode
                .clone()
                .unwrap_or_else(|| "merge".to_string()),
        }
    }
}

impl AIModelManagementMutation {
    fn into_model(self, existing: Option<AIModelConfig>) -> AIModelConfig {
        let current = existing.unwrap_or_default();
        AIModelConfig {
            id: self.id,
            name: self.name,
            provider: self.provider,
            model_name: self.model_name,
            base_url: self.base_url,
            request_url: current.request_url,
            api_key: apply_required_secret(self.api_key, current.api_key),
            context_window: self.context_window,
            max_tokens: self.max_tokens,
            temperature: current.temperature,
            top_p: current.top_p,
            enabled: self.enabled,
            category: current.category,
            capabilities: current.capabilities,
            recommended_for: current.recommended_for,
            metadata: current.metadata,
            reasoning: self.reasoning,
            inline_think_in_text: self.inline_think_in_text,
            custom_headers: apply_optional_secret(self.custom_headers, current.custom_headers),
            custom_headers_mode: self.custom_headers_mode.or(current.custom_headers_mode),
            skip_ssl_verify: self.skip_ssl_verify,
            custom_request_body: apply_optional_string_secret(
                self.custom_request_body,
                current.custom_request_body,
            ),
            custom_request_body_mode: current.custom_request_body_mode,
            auth: current.auth,
        }
    }
}

fn apply_required_secret(update: Option<SecretUpdate<String>>, existing: String) -> String {
    match update.unwrap_or(SecretUpdate::Preserve) {
        SecretUpdate::Preserve => existing,
        SecretUpdate::Replace(value) => value,
        SecretUpdate::Clear => String::new(),
    }
}

fn apply_optional_secret<T>(update: Option<SecretUpdate<T>>, existing: Option<T>) -> Option<T> {
    match update.unwrap_or(SecretUpdate::Preserve) {
        SecretUpdate::Preserve => existing,
        SecretUpdate::Replace(value) => Some(value),
        SecretUpdate::Clear => None,
    }
}

fn apply_optional_string_secret(
    update: Option<SecretUpdate<String>>,
    existing: Option<String>,
) -> Option<String> {
    match update.unwrap_or(SecretUpdate::Preserve) {
        SecretUpdate::Preserve => existing,
        SecretUpdate::Replace(value) => (!value.is_empty()).then_some(value),
        SecretUpdate::Clear => None,
    }
}

fn resolve_selector(ai: &AIConfig, selector: &Option<String>) -> Option<String> {
    selector
        .as_deref()
        .and_then(|selector| ai.resolve_model_selection(selector))
}

fn resolve_mode_selector(ai: &AIConfig, selector: &str) -> Option<String> {
    match selector.trim() {
        "" | "auto" | "default" => ai.resolve_model_selection("primary"),
        selector => ai.resolve_model_selection(selector),
    }
}

fn selector_is_unset(selector: &Option<String>) -> bool {
    selector
        .as_deref()
        .is_none_or(|selector| selector.trim().is_empty())
}

impl ConfigService {
    pub async fn list_ai_models_for_management(&self) -> BitFunResult<AIModelManagementCatalog> {
        let config: GlobalConfig = self.get_config(None).await?;
        Ok(AIModelManagementCatalog {
            models: config
                .ai
                .models
                .iter()
                .map(AIModelManagementSummary::from_model)
                .collect(),
            primary_model_id: resolve_selector(&config.ai, &config.ai.default_models.primary),
            fast_model_id: resolve_selector(&config.ai, &config.ai.default_models.fast),
            mode_default_model_id: resolve_mode_selector(
                &config.ai,
                &config.ai.agent_model_defaults.mode,
            ),
        })
    }

    pub async fn get_ai_model_for_management(
        &self,
        model_id: &str,
    ) -> BitFunResult<AIModelManagementProjection> {
        let model = self
            .get_ai_models()
            .await?
            .into_iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| BitFunError::NotFound(format!("AI model '{model_id}' was not found")))?;
        Ok(AIModelManagementProjection::from_model(&model))
    }

    pub async fn add_ai_model_for_management(
        &self,
        mutation: AIModelManagementMutation,
        make_primary_if_empty: bool,
    ) -> BitFunResult<()> {
        let model = mutation.into_model(None);
        let model_id = model.id.clone();
        self.add_ai_model(model).await?;
        if make_primary_if_empty {
            let config: GlobalConfig = self.get_config(None).await?;
            if selector_is_unset(&config.ai.default_models.primary) {
                self.set_config("ai.default_models.primary", &Some(model_id))
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn update_ai_model_for_management(
        &self,
        model_id: &str,
        mutation: AIModelManagementMutation,
    ) -> BitFunResult<()> {
        if mutation.id != model_id {
            return Err(BitFunError::validation(
                "Model update identity does not match the request target",
            ));
        }
        let existing = self
            .get_ai_models()
            .await?
            .into_iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| BitFunError::NotFound(format!("AI model '{model_id}' was not found")))?;
        self.update_ai_model(model_id, mutation.into_model(Some(existing)))
            .await
    }

    pub async fn delete_ai_model_for_management(&self, model_id: &str) -> BitFunResult<()> {
        self.delete_ai_model(model_id).await
    }

    pub async fn set_ai_model_default_for_management(
        &self,
        slot: AIModelDefaultSlot,
        model_id: Option<String>,
    ) -> BitFunResult<()> {
        match slot {
            AIModelDefaultSlot::Primary => {
                self.set_config("ai.default_models.primary", &model_id)
                    .await
            }
            AIModelDefaultSlot::Mode => {
                self.set_config(
                    "ai.agent_model_defaults.mode",
                    model_id.as_deref().unwrap_or("auto"),
                )
                .await
            }
        }
    }
}
