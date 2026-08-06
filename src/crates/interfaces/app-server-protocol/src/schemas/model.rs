//! Model-domain App Server wire schemas.
//!
//! Read projections intentionally omit credentials and arbitrary secret
//! payloads. Secret-bearing values are accepted only by mutation requests and
//! are never returned by the server.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use bitfun_core_types::{ProviderCatalog, ReasoningConfig};
use serde::{Deserialize, Serialize};

macro_rules! unit_response {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
        pub struct $name {}
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "model/list", response = ListModelsResponse)]
pub struct ListModelsRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ListModelsResponse {
    pub models: Vec<ModelSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_default_model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "model/get", response = GetModelResponse)]
pub struct GetModelRequest {
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GetModelResponse {
    pub model: ModelEditProjection,
}

/// Provider and reasoning facts needed by model configuration surfaces.
/// API keys and provider-specific execution metadata remain host-owned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "config/getTuiModelCatalog", response = TuiModelCatalogResponse)]
pub struct TuiModelCatalogRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct TuiModelCatalogResponse {
    pub provider_catalog: ProviderCatalog,
    pub reasoning_presets_by_model: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub api_key_configured: bool,
    #[serde(default)]
    pub custom_header_names: Vec<String>,
    #[serde(default)]
    pub custom_request_body_configured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_source: Option<String>,
}

/// Editable model fields. This projection is still secret-safe: it exposes
/// only whether write-only values exist, never their contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEditProjection {
    pub summary: ModelSummary,
    pub reasoning_preset_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,
    pub inline_think_in_text: bool,
    pub skip_ssl_verify: bool,
    pub custom_headers_mode: String,
}

#[derive(Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "model/add", response = AddModelResponse)]
pub struct AddModelRequest {
    pub model: ModelMutation,
    #[serde(default)]
    pub make_primary_if_empty: bool,
}

impl std::fmt::Debug for AddModelRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AddModelRequest")
            .field("model", &self.model)
            .field("make_primary_if_empty", &self.make_primary_if_empty)
            .finish()
    }
}

unit_response!(AddModelResponse);

#[derive(Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "model/update", response = UpdateModelResponse)]
pub struct UpdateModelRequest {
    pub model_id: String,
    pub model: ModelMutation,
}

impl std::fmt::Debug for UpdateModelRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UpdateModelRequest")
            .field("model_id", &self.model_id)
            .field("model", &self.model)
            .finish()
    }
}

unit_response!(UpdateModelResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "model/delete", response = DeleteModelResponse)]
pub struct DeleteModelRequest {
    pub model_id: String,
}

unit_response!(DeleteModelResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "model/setDefault", response = SetModelDefaultResponse)]
pub struct SetModelDefaultRequest {
    pub slot: ModelDefaultSlot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

unit_response!(SetModelDefaultResponse);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelDefaultSlot {
    Primary,
    Mode,
}

/// A write-only secret update. `Preserve` is used by edit forms that leave a
/// secret blank; `Clear` is explicit and is never emitted in read responses.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum SecretUpdate {
    Preserve,
    Replace(String),
    Clear,
}

impl std::fmt::Debug for SecretUpdate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Preserve => "Preserve",
            Self::Replace(_) => "Replace(<redacted>)",
            Self::Clear => "Clear",
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMutation {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<SecretUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_headers: Option<SecretUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_request_body: Option<SecretUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub reasoning: Option<ReasoningConfig>,
    #[serde(default)]
    pub inline_think_in_text: bool,
    #[serde(default)]
    pub skip_ssl_verify: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_headers_mode: Option<String>,
}

impl std::fmt::Debug for ModelMutation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelMutation")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("provider", &self.provider)
            .field("model_name", &self.model_name)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field(
                "custom_headers",
                &self.custom_headers.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "custom_request_body",
                &self.custom_request_body.as_ref().map(|_| "<redacted>"),
            )
            .field("context_window", &self.context_window)
            .field("max_tokens", &self.max_tokens)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_summary() -> ModelSummary {
        ModelSummary {
            id: "model-1".to_string(),
            name: "Model".to_string(),
            provider: "openai".to_string(),
            model_name: "gpt-test".to_string(),
            base_url: "https://example.com/v1".to_string(),
            enabled: true,
            context_window: Some(128_000),
            max_tokens: Some(8_192),
            api_key_configured: true,
            custom_header_names: vec!["Authorization".to_string()],
            custom_request_body_configured: true,
            auth_source: Some("api_key".to_string()),
        }
    }

    #[test]
    fn model_read_projection_contains_only_secret_metadata() {
        let projection = ModelEditProjection {
            summary: model_summary(),
            reasoning_preset_options: vec!["high".to_string()],
            reasoning: Some(ReasoningConfig {
                default_preset: Some("high".to_string()),
                ..Default::default()
            }),
            inline_think_in_text: true,
            skip_ssl_verify: false,
            custom_headers_mode: "merge".to_string(),
        };
        let json = serde_json::to_string(&projection).expect("serialize model projection");

        assert!(json.contains("apiKeyConfigured"));
        assert!(json.contains("customHeaderNames"));
        assert!(json.contains("customRequestBodyConfigured"));
        assert!(!json.contains("sk-secret"));
        assert!(!json.contains("Bearer secret"));
        assert!(!json.contains("secret-body-value"));
    }

    #[test]
    fn model_mutation_debug_redacts_all_write_only_values() {
        let mutation = ModelMutation {
            id: "model-1".to_string(),
            name: "Model".to_string(),
            provider: "openai".to_string(),
            model_name: "gpt-test".to_string(),
            base_url: "https://example.com/v1".to_string(),
            api_key: Some(SecretUpdate::Replace("sk-secret".to_string())),
            custom_headers: Some(SecretUpdate::Replace(
                r#"{"Authorization":"Bearer secret"}"#.to_string(),
            )),
            custom_request_body: Some(SecretUpdate::Replace(
                r#"{"secret":"secret-body-value"}"#.to_string(),
            )),
            context_window: Some(128_000),
            max_tokens: Some(8_192),
            enabled: true,
            reasoning: None,
            inline_think_in_text: true,
            skip_ssl_verify: false,
            custom_headers_mode: Some("merge".to_string()),
        };

        let debug = format!("{mutation:?}");
        assert!(!debug.contains("sk-secret"));
        assert!(!debug.contains("Bearer secret"));
        assert!(!debug.contains("secret-body-value"));
        assert!(debug.contains("redacted"));
    }
}
