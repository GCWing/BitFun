//! AI client implementation - refactored version
//!
//! Uses a modular architecture to separate provider-specific logic into the providers module

use crate::infrastructure::ai::providers::anthropic::AnthropicMessageConverter;
use crate::infrastructure::ai::providers::gemini::GeminiMessageConverter;
use crate::infrastructure::ai::providers::openai::OpenAIMessageConverter;
use crate::infrastructure::ai::tool_call_accumulator::{PendingToolCall, ToolCallBoundary};
use crate::service::config::ProxyConfig;
use crate::util::types::*;
use ai_stream_handlers::{
    handle_anthropic_stream, handle_gemini_stream, handle_openai_stream, handle_responses_stream,
    UnifiedResponse,
};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use log::{debug, error, info, warn};
use reqwest::{Client, Proxy};
use serde::Deserialize;
use tokio::sync::mpsc;

/// Streamed response result with the parsed stream and optional raw SSE receiver
pub struct StreamResponse {
    /// Parsed response stream
    pub stream: std::pin::Pin<Box<dyn futures::Stream<Item = Result<UnifiedResponse>> + Send>>,
    /// Raw SSE receiver (for error diagnostics)
    pub raw_sse_rx: Option<mpsc::UnboundedReceiver<String>>,
}

#[derive(Debug, Clone)]
pub struct AIClient {
    client: Client,
    pub config: AIConfig,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelEntry {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModelEntry>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelEntry {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    #[serde(default)]
    models: Vec<GeminiModelEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModelEntry {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_as_default")]
    supported_generation_methods: Vec<String>,
}

fn deserialize_null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|v| v.unwrap_or_default())
}

impl AIClient {
    const TEST_IMAGE_EXPECTED_CODE: &'static str = "BYGR";
    const TEST_IMAGE_PNG_BASE64: &'static str =
        "iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAIAAADTED8xAAACBklEQVR42u3ZsREAIAwDMYf9dw4txwJupI7Wua+YZEPBfO91h4ZjAgQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABIAAQAAgABAACAAEAAIAAYAAQAAgABAACAAEAAIAAYAAQAAgABAAAAAAAEDRZI3QGf7jDvEPAAIAAYAAQAAgABAACAAEAAIAAYAAQAAgABAACAAEAAIAAYAAQAAgABAACAABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAAAjABAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQALwuLkoG8OSfau4AAAAASUVORK5CYII=";
    const STREAM_CONNECT_TIMEOUT_SECS: u64 = 10;
    const HTTP_POOL_IDLE_TIMEOUT_SECS: u64 = 30;
    const HTTP_TCP_KEEPALIVE_SECS: u64 = 60;

    fn image_test_response_matches_expected(response: &str) -> bool {
        let upper = response.to_ascii_uppercase();

        // Accept contiguous letters even when separated by spaces/punctuation.
        let letters_only: String = upper.chars().filter(|c| c.is_ascii_alphabetic()).collect();
        if letters_only.contains(Self::TEST_IMAGE_EXPECTED_CODE) {
            return true;
        }

        let tokens: Vec<&str> = upper
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|s| !s.is_empty())
            .collect();

        if tokens.contains(&Self::TEST_IMAGE_EXPECTED_CODE) {
            return true;
        }

        // Accept outputs like: "B Y G R".
        let single_letter_stream: String = tokens
            .iter()
            .filter_map(|token| {
                if token.len() == 1 {
                    let ch = token.chars().next()?;
                    if matches!(ch, 'R' | 'G' | 'B' | 'Y') {
                        return Some(ch);
                    }
                }
                None
            })
            .collect();
        if single_letter_stream.contains(Self::TEST_IMAGE_EXPECTED_CODE) {
            return true;
        }

        // Accept outputs like: "Blue, Yellow, Green, Red".
        let color_word_stream: String = tokens
            .iter()
            .filter_map(|token| match *token {
                "RED" => Some('R'),
                "GREEN" => Some('G'),
                "BLUE" => Some('B'),
                "YELLOW" => Some('Y'),
                _ => None,
            })
            .collect();
        if color_word_stream.contains(Self::TEST_IMAGE_EXPECTED_CODE) {
            return true;
        }

        // Last fallback: keep only RGBY letters and search code.
        let color_letter_stream: String = upper
            .chars()
            .filter(|c| matches!(*c, 'R' | 'G' | 'B' | 'Y'))
            .collect();
        color_letter_stream.contains(Self::TEST_IMAGE_EXPECTED_CODE)
    }

    fn is_responses_api_format(api_format: &str) -> bool {
        matches!(
            api_format.to_ascii_lowercase().as_str(),
            "response" | "responses"
        )
    }

    fn is_gemini_api_format(api_format: &str) -> bool {
        matches!(
            api_format.to_ascii_lowercase().as_str(),
            "gemini" | "google"
        )
    }

    fn normalize_base_url_for_discovery(base_url: &str) -> String {
        base_url
            .trim()
            .trim_end_matches('#')
            .trim_end_matches('/')
            .to_string()
    }

    fn resolve_openai_models_url(&self) -> String {
        let mut base = Self::normalize_base_url_for_discovery(&self.config.base_url);

        for suffix in ["/chat/completions", "/responses", "/models"] {
            if base.ends_with(suffix) {
                base.truncate(base.len() - suffix.len());
                break;
            }
        }

        if base.is_empty() {
            return "models".to_string();
        }

        format!("{}/models", base)
    }

    fn resolve_anthropic_models_url(&self) -> String {
        let mut base = Self::normalize_base_url_for_discovery(&self.config.base_url);

        if base.ends_with("/v1/messages") {
            base.truncate(base.len() - "/v1/messages".len());
            return format!("{}/v1/models", base);
        }

        if base.ends_with("/v1/models") {
            return base;
        }

        if base.ends_with("/v1") {
            return format!("{}/models", base);
        }

        if base.is_empty() {
            return "v1/models".to_string();
        }

        format!("{}/v1/models", base)
    }

    fn dedupe_remote_models(models: Vec<RemoteModelInfo>) -> Vec<RemoteModelInfo> {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();

        for model in models {
            if seen.insert(model.id.clone()) {
                deduped.push(model);
            }
        }

        deduped
    }

    async fn list_openai_models(&self) -> Result<Vec<RemoteModelInfo>> {
        let url = self.resolve_openai_models_url();
        let response = self
            .apply_openai_headers(self.client.get(&url))
            .send()
            .await?
            .error_for_status()?;

        let payload: OpenAIModelsResponse = response.json().await?;
        Ok(Self::dedupe_remote_models(
            payload
                .data
                .into_iter()
                .map(|model| RemoteModelInfo {
                    id: model.id,
                    display_name: None,
                })
                .collect(),
        ))
    }

    async fn list_anthropic_models(&self) -> Result<Vec<RemoteModelInfo>> {
        let url = self.resolve_anthropic_models_url();
        let response = self
            .apply_anthropic_headers(self.client.get(&url), &url)
            .send()
            .await?
            .error_for_status()?;

        let payload: AnthropicModelsResponse = response.json().await?;
        Ok(Self::dedupe_remote_models(
            payload
                .data
                .into_iter()
                .map(|model| RemoteModelInfo {
                    id: model.id,
                    display_name: model.display_name,
                })
                .collect(),
        ))
    }

    fn resolve_gemini_models_url(&self) -> String {
        let base = Self::normalize_base_url_for_discovery(&self.config.base_url);
        let base = Self::gemini_base_url(&base);
        format!("{}/v1beta/models", base)
    }

    async fn list_gemini_models(&self) -> Result<Vec<RemoteModelInfo>> {
        let url = self.resolve_gemini_models_url();
        debug!("Gemini models list URL: {}", url);

        let response = self
            .apply_gemini_headers(self.client.get(&url))
            .send()
            .await?
            .error_for_status()?;

        let payload: GeminiModelsResponse = response.json().await?;
        Ok(Self::dedupe_remote_models(
            payload
                .models
                .into_iter()
                .filter(|m| {
                    m.supported_generation_methods.is_empty()
                        || m.supported_generation_methods
                            .iter()
                            .any(|method| method == "generateContent")
                })
                .map(|model| {
                    let id = model
                        .name
                        .strip_prefix("models/")
                        .unwrap_or(&model.name)
                        .to_string();
                    RemoteModelInfo {
                        id,
                        display_name: model.display_name,
                    }
                })
                .collect(),
        ))
    }

    /// Create an AIClient without proxy (backward compatible)
    pub fn new(config: AIConfig) -> Self {
        let skip_ssl_verify = config.skip_ssl_verify;
        let client = Self::create_http_client(None, skip_ssl_verify);
        Self { client, config }
    }

    /// Create an AIClient with proxy configuration
    pub fn new_with_proxy(config: AIConfig, proxy_config: Option<ProxyConfig>) -> Self {
        let skip_ssl_verify = config.skip_ssl_verify;
        let client = Self::create_http_client(proxy_config, skip_ssl_verify);
        Self { client, config }
    }

    /// Create an HTTP client (supports proxy config and SSL verification control)
    fn create_http_client(proxy_config: Option<ProxyConfig>, skip_ssl_verify: bool) -> Client {
        let mut builder = Client::builder()
            // SSE requests can legitimately stay open for a long time while the model
            // thinks or executes tools. Keep only connect timeout here and let the
            // stream handlers enforce idle timeouts between chunks.
            .connect_timeout(std::time::Duration::from_secs(
                Self::STREAM_CONNECT_TIMEOUT_SECS,
            ))
            .user_agent("BitFun/1.0")
            .pool_idle_timeout(std::time::Duration::from_secs(
                Self::HTTP_POOL_IDLE_TIMEOUT_SECS,
            ))
            .pool_max_idle_per_host(4)
            .tcp_keepalive(Some(std::time::Duration::from_secs(
                Self::HTTP_TCP_KEEPALIVE_SECS,
            )))
            .danger_accept_invalid_certs(skip_ssl_verify);

        if skip_ssl_verify {
            warn!("SSL certificate verification disabled - security risk, use only in test environments");
        }

        // rustls mode does not support http2_keep_alive_interval/http2_keep_alive_timeout.
        if let Some(proxy_cfg) = proxy_config {
            if proxy_cfg.enabled && !proxy_cfg.url.is_empty() {
                match Self::build_proxy(&proxy_cfg) {
                    Ok(proxy) => {
                        info!("Using proxy: {}", proxy_cfg.url);
                        builder = builder.proxy(proxy);
                    }
                    Err(e) => {
                        error!(
                            "Proxy configuration failed: {}, proceeding without proxy",
                            e
                        );
                        builder = builder.no_proxy();
                    }
                }
            } else {
                builder = builder.no_proxy();
            }
        } else {
            builder = builder.no_proxy();
        }

        match builder.build() {
            Ok(client) => client,
            Err(e) => {
                error!(
                    "HTTP client initialization failed: {}, using default client",
                    e
                );
                Client::new()
            }
        }
    }

    fn build_proxy(config: &ProxyConfig) -> Result<Proxy> {
        let mut proxy =
            Proxy::all(&config.url).map_err(|e| anyhow!("Failed to create proxy: {}", e))?;

        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            if !username.is_empty() && !password.is_empty() {
                proxy = proxy.basic_auth(username, password);
                debug!("Proxy authentication configured for user: {}", username);
            }
        }

        Ok(proxy)
    }

    fn get_api_format(&self) -> &str {
        &self.config.format
    }

    /// Whether the URL is Alibaba DashScope API.
    /// Alibaba DashScope uses `enable_thinking`=true/false for thinking, not the `thinking` object.
    fn is_dashscope_url(url: &str) -> bool {
        url.contains("dashscope.aliyuncs.com")
    }

    /// Whether the URL is MiniMax API.
    /// MiniMax (api.minimaxi.com) uses `reasoning_split=true` to enable streamed thinking content
    /// delivered via `delta.reasoning_details` rather than the standard `reasoning_content` field.
    fn is_minimax_url(url: &str) -> bool {
        url.contains("api.minimaxi.com")
    }

    /// Apply thinking-related fields onto the request body (mutates `request_body`).
    ///
    /// * `enable` - whether thinking process is enabled
    /// * `url` - request URL
    /// * `model_name` - model name (e.g. for Claude budget_tokens in Anthropic format)
    /// * `api_format` - "openai" or "anthropic"
    /// * `max_tokens` - optional max_tokens (for Anthropic Claude budget_tokens)
    fn apply_thinking_fields(
        request_body: &mut serde_json::Value,
        enable: bool,
        url: &str,
        model_name: &str,
        api_format: &str,
        max_tokens: Option<u32>,
    ) {
        if Self::is_dashscope_url(url) && api_format.eq_ignore_ascii_case("openai") {
            request_body["enable_thinking"] = serde_json::json!(enable);
            return;
        }
        if Self::is_minimax_url(url) && api_format.eq_ignore_ascii_case("openai") {
            if enable {
                request_body["reasoning_split"] = serde_json::json!(true);
            }
            return;
        }
        let thinking_value = if enable {
            if api_format.eq_ignore_ascii_case("anthropic") && model_name.starts_with("claude") {
                let mut obj = serde_json::map::Map::new();
                obj.insert(
                    "type".to_string(),
                    serde_json::Value::String("enabled".to_string()),
                );
                if let Some(m) = max_tokens {
                    obj.insert(
                        "budget_tokens".to_string(),
                        serde_json::json!(10000u32.min(m * 3 / 4)),
                    );
                }
                serde_json::Value::Object(obj)
            } else {
                serde_json::json!({ "type": "enabled" })
            }
        } else {
            serde_json::json!({ "type": "disabled" })
        };
        request_body["thinking"] = thinking_value;
    }

    /// Whether to append the `tool_stream` request field.
    ///
    /// Only Zhipu (https://open.bigmodel.cn) uses this field; and only for GLM models (pure version >= 4.6).
    /// Adding this parameter for non-Zhipu APIs may cause abnormal behavior:
    /// 1) incomplete output; (Aliyun Coding Plan, 2026-02-28)
    /// 2) extra `<tool_call>` prefix on some tool names. (Aliyun Coding Plan, 2026-02-28)
    fn should_append_tool_stream(url: &str, model_name: &str) -> bool {
        if !url.contains("open.bigmodel.cn") {
            return false;
        }
        Self::parse_glm_major_minor(model_name)
            .map(|(major, minor)| major > 4 || (major == 4 && minor >= 6))
            .unwrap_or(false)
    }

    /// Parse strict `glm-<major>[.<minor>]` from model names like:
    /// - glm-4.6
    /// - glm-5
    ///
    /// Models with non-numeric suffixes are treated as not requiring this GLM-specific field, e.g.:
    /// - glm-4.6-flash
    /// - glm-4.5v
    fn parse_glm_major_minor(model_name: &str) -> Option<(u32, u32)> {
        let version_part = model_name.strip_prefix("glm-")?;

        if version_part.is_empty() {
            return None;
        }

        let mut parts = version_part.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor: u32 = match parts.next() {
            Some(v) => v.parse().ok()?,
            None => 0,
        };

        // Only allow one numeric segment after the decimal point.
        if parts.next().is_some() {
            return None;
        }

        Some((major, minor))
    }

    /// Determine whether to use merge mode
    ///
    /// true: apply default headers first, then custom headers (custom can override)
    /// false: if custom headers exist, replace defaults entirely
    /// Default is merge mode
    fn is_merge_headers_mode(&self) -> bool {
        // Default to merge mode; use replace mode only when explicitly set to "replace"
        self.config.custom_headers_mode.as_deref() != Some("replace")
    }

    /// Apply custom headers to the builder
    fn apply_custom_headers(
        &self,
        mut builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let Some(custom_headers) = &self.config.custom_headers {
            if !custom_headers.is_empty() {
                for (key, value) in custom_headers {
                    builder = builder.header(key.as_str(), value.as_str());
                }
            }
        }
        builder
    }

    /// Apply OpenAI-style request headers (merge/replace).
    fn apply_openai_headers(
        &self,
        mut builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        let has_custom_headers = self
            .config
            .custom_headers
            .as_ref()
            .is_some_and(|h| !h.is_empty());
        let is_merge_mode = self.is_merge_headers_mode();

        if has_custom_headers && !is_merge_mode {
            return self.apply_custom_headers(builder);
        }

        builder = builder
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key));

        if self.config.base_url.contains("openbitfun.com") {
            builder = builder.header("X-Verification-Code", "from_bitfun");
        }

        if has_custom_headers && is_merge_mode {
            builder = self.apply_custom_headers(builder);
        }

        builder
    }

    /// Apply Anthropic-style request headers (merge/replace).
    fn apply_anthropic_headers(
        &self,
        mut builder: reqwest::RequestBuilder,
        url: &str,
    ) -> reqwest::RequestBuilder {
        let has_custom_headers = self
            .config
            .custom_headers
            .as_ref()
            .is_some_and(|h| !h.is_empty());
        let is_merge_mode = self.is_merge_headers_mode();

        if has_custom_headers && !is_merge_mode {
            return self.apply_custom_headers(builder);
        }

        builder = builder.header("Content-Type", "application/json");

        if url.contains("bigmodel.cn") {
            builder = builder.header("Authorization", format!("Bearer {}", self.config.api_key));
        } else {
            builder = builder
                .header("x-api-key", &self.config.api_key)
                .header("anthropic-version", "2023-06-01");
        }

        if url.contains("openbitfun.com") {
            builder = builder.header("X-Verification-Code", "from_bitfun");
        }

        if has_custom_headers && is_merge_mode {
            builder = self.apply_custom_headers(builder);
        }

        builder
    }

    /// Apply Gemini-style request headers (merge/replace).
    fn apply_gemini_headers(
        &self,
        mut builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        let has_custom_headers = self
            .config
            .custom_headers
            .as_ref()
            .is_some_and(|h| !h.is_empty());
        let is_merge_mode = self.is_merge_headers_mode();

        if has_custom_headers && !is_merge_mode {
            return self.apply_custom_headers(builder);
        }

        builder = builder
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.config.api_key)
            .header("Authorization", format!("Bearer {}", self.config.api_key));

        if self.config.base_url.contains("openbitfun.com") {
            builder = builder.header("X-Verification-Code", "from_bitfun");
        }

        if has_custom_headers && is_merge_mode {
            builder = self.apply_custom_headers(builder);
        }

        builder
    }

    fn merge_json_value(target: &mut serde_json::Value, overlay: serde_json::Value) {
        match (target, overlay) {
            (serde_json::Value::Object(target_map), serde_json::Value::Object(overlay_map)) => {
                for (key, value) in overlay_map {
                    let entry = target_map.entry(key).or_insert(serde_json::Value::Null);
                    Self::merge_json_value(entry, value);
                }
            }
            (target_slot, overlay_value) => {
                *target_slot = overlay_value;
            }
        }
    }

    fn ensure_gemini_generation_config(
        request_body: &mut serde_json::Value,
    ) -> &mut serde_json::Map<String, serde_json::Value> {
        if !request_body
            .get("generationConfig")
            .is_some_and(serde_json::Value::is_object)
        {
            request_body["generationConfig"] = serde_json::json!({});
        }

        request_body["generationConfig"]
            .as_object_mut()
            .expect("generationConfig must be an object")
    }

    fn insert_gemini_generation_field(
        request_body: &mut serde_json::Value,
        key: &str,
        value: serde_json::Value,
    ) {
        Self::ensure_gemini_generation_config(request_body).insert(key.to_string(), value);
    }

    fn normalize_gemini_stop_sequences(value: &serde_json::Value) -> Option<serde_json::Value> {
        match value {
            serde_json::Value::String(sequence) => {
                Some(serde_json::Value::Array(vec![serde_json::Value::String(
                    sequence.clone(),
                )]))
            }
            serde_json::Value::Array(items) => {
                let sequences = items
                    .iter()
                    .filter_map(|item| item.as_str().map(|sequence| sequence.to_string()))
                    .map(serde_json::Value::String)
                    .collect::<Vec<_>>();

                if sequences.is_empty() {
                    None
                } else {
                    Some(serde_json::Value::Array(sequences))
                }
            }
            _ => None,
        }
    }

    fn apply_gemini_response_format_translation(
        request_body: &mut serde_json::Value,
        response_format: &serde_json::Value,
    ) -> bool {
        match response_format {
            serde_json::Value::String(kind) if matches!(kind.as_str(), "json" | "json_object") => {
                Self::insert_gemini_generation_field(
                    request_body,
                    "responseMimeType",
                    serde_json::Value::String("application/json".to_string()),
                );
                true
            }
            serde_json::Value::Object(map) => {
                let Some(kind) = map.get("type").and_then(serde_json::Value::as_str) else {
                    return false;
                };

                match kind {
                    "json" | "json_object" => {
                        Self::insert_gemini_generation_field(
                            request_body,
                            "responseMimeType",
                            serde_json::Value::String("application/json".to_string()),
                        );
                        true
                    }
                    "json_schema" => {
                        Self::insert_gemini_generation_field(
                            request_body,
                            "responseMimeType",
                            serde_json::Value::String("application/json".to_string()),
                        );

                        if let Some(schema) = map
                            .get("json_schema")
                            .and_then(serde_json::Value::as_object)
                            .and_then(|json_schema| json_schema.get("schema"))
                            .or_else(|| map.get("schema"))
                        {
                            Self::insert_gemini_generation_field(
                                request_body,
                                "responseJsonSchema",
                                GeminiMessageConverter::sanitize_schema(schema.clone()),
                            );
                        }

                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn translate_gemini_extra_body(
        request_body: &mut serde_json::Value,
        extra_obj: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        if let Some(max_tokens) = extra_obj.remove("max_tokens") {
            Self::insert_gemini_generation_field(request_body, "maxOutputTokens", max_tokens);
        }

        if let Some(temperature) = extra_obj.remove("temperature") {
            Self::insert_gemini_generation_field(request_body, "temperature", temperature);
        }

        let top_p = extra_obj
            .remove("top_p")
            .or_else(|| extra_obj.remove("topP"));
        if let Some(top_p) = top_p {
            Self::insert_gemini_generation_field(request_body, "topP", top_p);
        }

        if let Some(stop_sequences) = extra_obj
            .get("stop")
            .and_then(Self::normalize_gemini_stop_sequences)
        {
            extra_obj.remove("stop");
            Self::insert_gemini_generation_field(request_body, "stopSequences", stop_sequences);
        }

        if let Some(response_mime_type) = extra_obj
            .remove("responseMimeType")
            .or_else(|| extra_obj.remove("response_mime_type"))
        {
            Self::insert_gemini_generation_field(
                request_body,
                "responseMimeType",
                response_mime_type,
            );
        }

        if let Some(response_schema) = extra_obj
            .remove("responseJsonSchema")
            .or_else(|| extra_obj.remove("responseSchema"))
            .or_else(|| extra_obj.remove("response_schema"))
        {
            Self::insert_gemini_generation_field(
                request_body,
                "responseJsonSchema",
                GeminiMessageConverter::sanitize_schema(response_schema),
            );
        }

        if let Some(response_format) = extra_obj.get("response_format").cloned() {
            if Self::apply_gemini_response_format_translation(request_body, &response_format) {
                extra_obj.remove("response_format");
            }
        }
    }

    fn unified_usage_to_gemini_usage(usage: ai_stream_handlers::UnifiedTokenUsage) -> GeminiUsage {
        GeminiUsage {
            prompt_token_count: usage.prompt_token_count,
            candidates_token_count: usage.candidates_token_count,
            total_token_count: usage.total_token_count,
            reasoning_token_count: usage.reasoning_token_count,
            cached_content_token_count: usage.cached_content_token_count,
        }
    }

    /// Build an OpenAI-format request body
    fn build_openai_request_body(
        &self,
        url: &str,
        openai_messages: Vec<serde_json::Value>,
        openai_tools: Option<Vec<serde_json::Value>>,
        extra_body: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut request_body = serde_json::json!({
            "model": self.config.model,
            "messages": openai_messages,
            "stream": true
        });

        let model_name = self.config.model.to_lowercase();

        if Self::should_append_tool_stream(url, &model_name) {
            request_body["tool_stream"] = serde_json::Value::Bool(true);
        }

        Self::apply_thinking_fields(
            &mut request_body,
            self.config.enable_thinking_process,
            url,
            &model_name,
            "openai",
            self.config.max_tokens,
        );

        if let Some(max_tokens) = self.config.max_tokens {
            request_body["max_tokens"] = serde_json::json!(max_tokens);
        }

        if let Some(extra) = extra_body {
            if let Some(extra_obj) = extra.as_object() {
                for (key, value) in extra_obj {
                    request_body[key] = value.clone();
                }
                debug!(target: "ai::openai_stream_request", "Applied extra_body overrides: {:?}", extra_obj.keys().collect::<Vec<_>>());
            }
        }

        // This client currently consumes only the first choice in stream handling.
        // Remove custom n override and keep provider defaults.
        if let Some(request_obj) = request_body.as_object_mut() {
            if let Some(existing_n) = request_obj.remove("n") {
                warn!(
                    target: "ai::openai_stream_request",
                    "Removed custom request field n={} because the stream processor only handles the first choice",
                    existing_n
                );
            }
        }

        debug!(target: "ai::openai_stream_request",
            "OpenAI stream request body (excluding tools):\n{}",
            serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| "serialization failed".to_string())
        );

        if let Some(tools) = openai_tools {
            let tool_names = tools
                .iter()
                .map(Self::extract_openai_tool_name)
                .collect::<Vec<_>>();
            debug!(target: "ai::openai_stream_request", "\ntools: {:?}", tool_names);
            if !tools.is_empty() {
                request_body["tools"] = serde_json::Value::Array(tools);
                // Respect `extra_body` overrides (e.g. tool_choice="required") when present.
                let has_tool_choice = request_body
                    .get("tool_choice")
                    .is_some_and(|v| !v.is_null());
                if !has_tool_choice {
                    request_body["tool_choice"] = serde_json::Value::String("auto".to_string());
                }
            }
        }

        request_body
    }

    /// Build a Responses API request body.
    fn build_responses_request_body(
        &self,
        instructions: Option<String>,
        response_input: Vec<serde_json::Value>,
        openai_tools: Option<Vec<serde_json::Value>>,
        extra_body: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut request_body = serde_json::json!({
            "model": self.config.model,
            "input": response_input,
            "stream": true
        });

        if let Some(instructions) = instructions.filter(|value| !value.trim().is_empty()) {
            request_body["instructions"] = serde_json::Value::String(instructions);
        }

        if let Some(max_tokens) = self.config.max_tokens {
            request_body["max_output_tokens"] = serde_json::json!(max_tokens);
        }

        if let Some(ref effort) = self.config.reasoning_effort {
            request_body["reasoning"] = serde_json::json!({
                "effort": effort,
                "summary": "auto"
            });
        }

        if let Some(extra) = extra_body {
            if let Some(extra_obj) = extra.as_object() {
                for (key, value) in extra_obj {
                    request_body[key] = value.clone();
                }
                debug!(
                    target: "ai::responses_stream_request",
                    "Applied extra_body overrides: {:?}",
                    extra_obj.keys().collect::<Vec<_>>()
                );
            }
        }

        debug!(
            target: "ai::responses_stream_request",
            "Responses stream request body (excluding tools):\n{}",
            serde_json::to_string_pretty(&request_body)
                .unwrap_or_else(|_| "serialization failed".to_string())
        );

        if let Some(tools) = openai_tools {
            let tool_names = tools
                .iter()
                .map(Self::extract_openai_tool_name)
                .collect::<Vec<_>>();
            debug!(target: "ai::responses_stream_request", "\ntools: {:?}", tool_names);
            if !tools.is_empty() {
                request_body["tools"] = serde_json::Value::Array(tools);
                // Respect `extra_body` overrides (e.g. tool_choice="required") when present.
                let has_tool_choice = request_body
                    .get("tool_choice")
                    .is_some_and(|v| !v.is_null());
                if !has_tool_choice {
                    request_body["tool_choice"] = serde_json::Value::String("auto".to_string());
                }
            }
        }

        request_body
    }

    /// Build an Anthropic-format request body
    fn build_anthropic_request_body(
        &self,
        url: &str,
        system_message: Option<String>,
        anthropic_messages: Vec<serde_json::Value>,
        anthropic_tools: Option<Vec<serde_json::Value>>,
        extra_body: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let max_tokens = self.config.max_tokens.unwrap_or(8192);

        let mut request_body = serde_json::json!({
            "model": self.config.model,
            "messages": anthropic_messages,
            "max_tokens": max_tokens,
            "stream": true
        });

        let model_name = self.config.model.to_lowercase();

        // Zhipu extension: only set `tool_stream` for open.bigmodel.cn.
        if Self::should_append_tool_stream(url, &model_name) {
            request_body["tool_stream"] = serde_json::Value::Bool(true);
        }

        Self::apply_thinking_fields(
            &mut request_body,
            self.config.enable_thinking_process,
            url,
            &model_name,
            "anthropic",
            Some(max_tokens),
        );

        if let Some(system) = system_message {
            request_body["system"] = serde_json::Value::String(system);
        }

        if let Some(extra) = extra_body {
            if let Some(extra_obj) = extra.as_object() {
                for (key, value) in extra_obj {
                    request_body[key] = value.clone();
                }
                debug!(target: "ai::anthropic_stream_request", "Applied extra_body overrides: {:?}", extra_obj.keys().collect::<Vec<_>>());
            }
        }

        debug!(target: "ai::anthropic_stream_request",
            "Anthropic stream request body (excluding tools):\n{}",
            serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| "serialization failed".to_string())
        );

        if let Some(tools) = anthropic_tools {
            let tool_names = tools
                .iter()
                .map(Self::extract_anthropic_tool_name)
                .collect::<Vec<_>>();
            debug!(target: "ai::anthropic_stream_request", "\ntools: {:?}", tool_names);
            if !tools.is_empty() {
                request_body["tools"] = serde_json::Value::Array(tools);
            }
        }

        request_body
    }

    /// Build a Gemini-format request body.
    fn build_gemini_request_body(
        &self,
        system_instruction: Option<serde_json::Value>,
        contents: Vec<serde_json::Value>,
        gemini_tools: Option<Vec<serde_json::Value>>,
        extra_body: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut request_body = serde_json::json!({
            "contents": contents,
        });

        if let Some(system_instruction) = system_instruction {
            request_body["systemInstruction"] = system_instruction;
        }

        if let Some(max_tokens) = self.config.max_tokens {
            Self::insert_gemini_generation_field(
                &mut request_body,
                "maxOutputTokens",
                serde_json::json!(max_tokens),
            );
        }

        if let Some(temperature) = self.config.temperature {
            Self::insert_gemini_generation_field(
                &mut request_body,
                "temperature",
                serde_json::json!(temperature),
            );
        }

        if let Some(top_p) = self.config.top_p {
            Self::insert_gemini_generation_field(
                &mut request_body,
                "topP",
                serde_json::json!(top_p),
            );
        }

        if self.config.enable_thinking_process {
            Self::insert_gemini_generation_field(
                &mut request_body,
                "thinkingConfig",
                serde_json::json!({
                    "includeThoughts": true,
                }),
            );
        }

        if let Some(tools) = gemini_tools {
            let tool_names = tools
                .iter()
                .flat_map(|tool| {
                    if let Some(declarations) = tool
                        .get("functionDeclarations")
                        .and_then(|value| value.as_array())
                    {
                        declarations
                            .iter()
                            .filter_map(|declaration| {
                                declaration
                                    .get("name")
                                    .and_then(|value| value.as_str())
                                    .map(str::to_string)
                            })
                            .collect::<Vec<_>>()
                    } else {
                        tool.as_object()
                            .into_iter()
                            .flat_map(|map| map.keys().cloned())
                            .collect::<Vec<_>>()
                    }
                })
                .collect::<Vec<_>>();
            debug!(target: "ai::gemini_stream_request", "\ntools: {:?}", tool_names);

            if !tools.is_empty() {
                request_body["tools"] = serde_json::Value::Array(tools);
                let has_function_declarations = request_body["tools"]
                    .as_array()
                    .map(|tools| {
                        tools
                            .iter()
                            .any(|tool| tool.get("functionDeclarations").is_some())
                    })
                    .unwrap_or(false);

                if has_function_declarations {
                    request_body["toolConfig"] = serde_json::json!({
                        "functionCallingConfig": {
                            "mode": "AUTO"
                        }
                    });
                }
            }
        }

        if let Some(extra) = extra_body {
            if let Some(mut extra_obj) = extra.as_object().cloned() {
                Self::translate_gemini_extra_body(&mut request_body, &mut extra_obj);
                let override_keys = extra_obj.keys().cloned().collect::<Vec<_>>();

                for (key, value) in extra_obj {
                    if let Some(request_obj) = request_body.as_object_mut() {
                        let target = request_obj.entry(key).or_insert(serde_json::Value::Null);
                        Self::merge_json_value(target, value);
                    }
                }
                debug!(
                    target: "ai::gemini_stream_request",
                    "Applied extra_body overrides: {:?}",
                    override_keys
                );
            }
        }

        debug!(
            target: "ai::gemini_stream_request",
            "Gemini stream request body:\n{}",
            serde_json::to_string_pretty(&request_body)
                .unwrap_or_else(|_| "serialization failed".to_string())
        );

        request_body
    }

    fn resolve_gemini_request_url(base_url: &str, model_name: &str) -> String {
        let trimmed = base_url.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            return String::new();
        }

        let base = Self::gemini_base_url(trimmed);
        let encoded_model = urlencoding::encode(model_name.trim());
        format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
            base, encoded_model
        )
    }

    /// Strip /v1beta, /models/... and similar suffixes from a gemini URL,
    /// returning only the bare host root (e.g. https://generativelanguage.googleapis.com).
    fn gemini_base_url(url: &str) -> &str {
        let mut u = url;
        if let Some(pos) = u.find("/v1beta") {
            u = &u[..pos];
        }
        if let Some(pos) = u.find("/models/") {
            u = &u[..pos];
        }
        u.trim_end_matches('/')
    }

    fn extract_openai_tool_name(tool: &serde_json::Value) -> String {
        tool.get("function")
            .and_then(|f| f.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn extract_anthropic_tool_name(tool: &serde_json::Value) -> String {
        tool.get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Send a streaming message request
    ///
    /// Returns `StreamResponse` with:
    /// - `stream`: parsed response stream
    /// - `raw_sse_rx`: raw SSE receiver (for collecting data during error diagnostics)
    pub async fn send_message_stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<StreamResponse> {
        let custom_body = self.config.custom_request_body.clone();
        self.send_message_stream_with_extra_body(messages, tools, custom_body)
            .await
    }

    /// Send a streaming message request with extra request body overrides
    ///
    /// Returns `StreamResponse` with:
    /// - `stream`: parsed response stream
    /// - `raw_sse_rx`: raw SSE receiver (for collecting data during error diagnostics)
    pub async fn send_message_stream_with_extra_body(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
    ) -> Result<StreamResponse> {
        let max_tries = 3;
        match self.get_api_format().to_lowercase().as_str() {
            "openai" => {
                self.send_openai_stream(messages, tools, extra_body, max_tries)
                    .await
            }
            format if Self::is_gemini_api_format(format) => {
                self.send_gemini_stream(messages, tools, extra_body, max_tries)
                    .await
            }
            format if Self::is_responses_api_format(format) => {
                self.send_responses_stream(messages, tools, extra_body, max_tries)
                    .await
            }
            "anthropic" => {
                self.send_anthropic_stream(messages, tools, extra_body, max_tries)
                    .await
            }
            _ => Err(anyhow!("Unknown API format: {}", self.get_api_format())),
        }
    }

    /// Send an OpenAI streaming request with retries
    ///
    /// # Parameters
    /// - `messages`: message list
    /// - `tools`: tool definitions
    /// - `extra_body`: extra request body parameters
    /// - `max_tries`: max attempts (including the first)
    async fn send_openai_stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
        max_tries: usize,
    ) -> Result<StreamResponse> {
        let url = self.config.request_url.clone();
        debug!(
            "OpenAI config: model={}, request_url={}, max_tries={}",
            self.config.model, self.config.request_url, max_tries
        );

        // Use OpenAI message converter
        let openai_messages = OpenAIMessageConverter::convert_messages(messages);
        let openai_tools = OpenAIMessageConverter::convert_tools(tools);

        // Build request body
        let request_body =
            self.build_openai_request_body(&url, openai_messages, openai_tools, extra_body);

        let mut last_error = None;
        let base_wait_time_ms = 500;

        for attempt in 0..max_tries {
            let request_start_time = std::time::Instant::now();

            // Send request - apply request headers
            let request_builder = self.apply_openai_headers(self.client.post(&url));
            let response_result = request_builder.json(&request_body).send().await;

            let response = match response_result {
                Ok(resp) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let status = resp.status();

                    if status.is_client_error() {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        error!(
                            "OpenAI Streaming API client error {}: {}",
                            status, error_text
                        );
                        return Err(anyhow!(
                            "OpenAI Streaming API client error {}: {}",
                            status,
                            error_text
                        ));
                    }

                    if status.is_success() {
                        debug!(
                            "Stream request connected: {}ms, status: {}, attempt: {}/{}",
                            connect_time,
                            status,
                            attempt + 1,
                            max_tries
                        );
                        resp
                    } else {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        let error =
                            anyhow!("OpenAI Streaming API error {}: {}", status, error_text);
                        warn!(
                            "Stream request failed (attempt {}/{}): {}",
                            attempt + 1,
                            max_tries,
                            error
                        );
                        last_error = Some(error);

                        if attempt < max_tries - 1 {
                            let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                            debug!("Retrying after {}ms (attempt {})", delay_ms, attempt + 2);
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        }
                        continue;
                    }
                }
                Err(e) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let error = anyhow!("Stream request connection failed: {}", e);
                    warn!(
                        "Stream request connection failed: {}ms, attempt {}/{}, error: {}",
                        connect_time,
                        attempt + 1,
                        max_tries,
                        e
                    );
                    last_error = Some(error);

                    if attempt < max_tries - 1 {
                        let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                        debug!("Retrying after {}ms (attempt {})", delay_ms, attempt + 2);
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                    continue;
                }
            };

            // Success: create channels and return
            let (tx, rx) = mpsc::unbounded_channel();
            let (tx_raw, rx_raw) = mpsc::unbounded_channel();

            tokio::spawn(handle_openai_stream(
                response,
                tx,
                Some(tx_raw),
                self.config.inline_think_in_text,
            ));

            return Ok(StreamResponse {
                stream: Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)),
                raw_sse_rx: Some(rx_raw),
            });
        }

        let error_msg = format!(
            "Stream request failed after {} attempts: {}",
            max_tries,
            last_error.unwrap_or_else(|| anyhow!("Unknown error"))
        );
        error!("{}", error_msg);
        Err(anyhow!(error_msg))
    }

    /// Send a Gemini streaming request with retries.
    async fn send_gemini_stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
        max_tries: usize,
    ) -> Result<StreamResponse> {
        let url = Self::resolve_gemini_request_url(&self.config.request_url, &self.config.model);
        debug!(
            "Gemini config: model={}, request_url={}, max_tries={}",
            self.config.model, url, max_tries
        );

        let (system_instruction, contents) =
            GeminiMessageConverter::convert_messages(messages, &self.config.model);
        let gemini_tools = GeminiMessageConverter::convert_tools(tools);
        let request_body =
            self.build_gemini_request_body(system_instruction, contents, gemini_tools, extra_body);

        let mut last_error = None;
        let base_wait_time_ms = 500;

        for attempt in 0..max_tries {
            let request_start_time = std::time::Instant::now();
            let request_builder = self.apply_gemini_headers(self.client.post(&url));
            let response_result = request_builder.json(&request_body).send().await;

            let response = match response_result {
                Ok(resp) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let status = resp.status();

                    if status.is_client_error() {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        error!(
                            "Gemini Streaming API client error {}: {}",
                            status, error_text
                        );
                        return Err(anyhow!(
                            "Gemini Streaming API client error {}: {}",
                            status,
                            error_text
                        ));
                    }

                    if status.is_success() {
                        debug!(
                            "Gemini stream request connected: {}ms, status: {}, attempt: {}/{}",
                            connect_time,
                            status,
                            attempt + 1,
                            max_tries
                        );
                        resp
                    } else {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        let error =
                            anyhow!("Gemini Streaming API error {}: {}", status, error_text);
                        warn!(
                            "Gemini stream request failed: {}ms, attempt {}/{}, error: {}",
                            connect_time,
                            attempt + 1,
                            max_tries,
                            error
                        );
                        last_error = Some(error);

                        if attempt < max_tries - 1 {
                            let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                            debug!(
                                "Retrying Gemini after {}ms (attempt {})",
                                delay_ms,
                                attempt + 2
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        }
                        continue;
                    }
                }
                Err(e) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let error = anyhow!("Gemini stream request connection failed: {}", e);
                    warn!(
                        "Gemini stream request connection failed: {}ms, attempt {}/{}, error: {}",
                        connect_time,
                        attempt + 1,
                        max_tries,
                        e
                    );
                    last_error = Some(error);

                    if attempt < max_tries - 1 {
                        let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                        debug!(
                            "Retrying Gemini after {}ms (attempt {})",
                            delay_ms,
                            attempt + 2
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                    continue;
                }
            };

            let (tx, rx) = mpsc::unbounded_channel();
            let (tx_raw, rx_raw) = mpsc::unbounded_channel();

            tokio::spawn(handle_gemini_stream(response, tx, Some(tx_raw)));

            return Ok(StreamResponse {
                stream: Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)),
                raw_sse_rx: Some(rx_raw),
            });
        }

        let error_msg = format!(
            "Gemini stream request failed after {} attempts: {}",
            max_tries,
            last_error.unwrap_or_else(|| anyhow!("Unknown error"))
        );
        error!("{}", error_msg);
        Err(anyhow!(error_msg))
    }

    /// Send a Responses API streaming request with retries.
    async fn send_responses_stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
        max_tries: usize,
    ) -> Result<StreamResponse> {
        let url = self.config.request_url.clone();
        debug!(
            "Responses config: model={}, request_url={}, max_tries={}",
            self.config.model, self.config.request_url, max_tries
        );

        let (instructions, response_input) =
            OpenAIMessageConverter::convert_messages_to_responses_input(messages);
        let openai_tools = OpenAIMessageConverter::convert_tools(tools);
        let request_body = self.build_responses_request_body(
            instructions,
            response_input,
            openai_tools,
            extra_body,
        );

        let mut last_error = None;
        let base_wait_time_ms = 500;

        for attempt in 0..max_tries {
            let request_start_time = std::time::Instant::now();
            let request_builder = self.apply_openai_headers(self.client.post(&url));
            let response_result = request_builder.json(&request_body).send().await;

            let response = match response_result {
                Ok(resp) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let status = resp.status();

                    if status.is_client_error() {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        error!("Responses API client error {}: {}", status, error_text);
                        return Err(anyhow!(
                            "Responses API client error {}: {}",
                            status,
                            error_text
                        ));
                    }

                    if status.is_success() {
                        debug!(
                            "Responses request connected: {}ms, status: {}, attempt: {}/{}",
                            connect_time,
                            status,
                            attempt + 1,
                            max_tries
                        );
                        resp
                    } else {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        let error = anyhow!("Responses API error {}: {}", status, error_text);
                        warn!(
                            "Responses request failed (attempt {}/{}): {}",
                            attempt + 1,
                            max_tries,
                            error
                        );
                        last_error = Some(error);

                        if attempt < max_tries - 1 {
                            let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                            debug!("Retrying after {}ms (attempt {})", delay_ms, attempt + 2);
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        }
                        continue;
                    }
                }
                Err(e) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let error = anyhow!("Responses request connection failed: {}", e);
                    warn!(
                        "Responses request connection failed: {}ms, attempt {}/{}, error: {}",
                        connect_time,
                        attempt + 1,
                        max_tries,
                        e
                    );
                    last_error = Some(error);

                    if attempt < max_tries - 1 {
                        let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                        debug!("Retrying after {}ms (attempt {})", delay_ms, attempt + 2);
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                    continue;
                }
            };

            let (tx, rx) = mpsc::unbounded_channel();
            let (tx_raw, rx_raw) = mpsc::unbounded_channel();

            tokio::spawn(handle_responses_stream(response, tx, Some(tx_raw)));

            return Ok(StreamResponse {
                stream: Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)),
                raw_sse_rx: Some(rx_raw),
            });
        }

        let error_msg = format!(
            "Responses request failed after {} attempts: {}",
            max_tries,
            last_error.unwrap_or_else(|| anyhow!("Unknown error"))
        );
        error!("{}", error_msg);
        Err(anyhow!(error_msg))
    }

    /// Send an Anthropic streaming request with retries
    ///
    /// # Parameters
    /// - `messages`: message list
    /// - `tools`: tool definitions
    /// - `extra_body`: extra request body parameters
    /// - `max_tries`: max attempts (including the first)
    async fn send_anthropic_stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
        max_tries: usize,
    ) -> Result<StreamResponse> {
        let url = self.config.request_url.clone();
        debug!(
            "Anthropic config: model={}, request_url={}, max_tries={}",
            self.config.model, self.config.request_url, max_tries
        );

        // Use Anthropic message converter
        let (system_message, anthropic_messages) =
            AnthropicMessageConverter::convert_messages(messages);
        let anthropic_tools = AnthropicMessageConverter::convert_tools(tools);

        // Build request body
        let request_body = self.build_anthropic_request_body(
            &url,
            system_message,
            anthropic_messages,
            anthropic_tools,
            extra_body,
        );

        let mut last_error = None;
        let base_wait_time_ms = 500;

        for attempt in 0..max_tries {
            let request_start_time = std::time::Instant::now();

            // Send request - apply Anthropic-style request headers
            let request_builder = self.apply_anthropic_headers(self.client.post(&url), &url);
            let response_result = request_builder.json(&request_body).send().await;

            let response = match response_result {
                Ok(resp) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let status = resp.status();

                    if status.is_client_error() {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        error!(
                            "Anthropic Streaming API client error {}: {}",
                            status, error_text
                        );
                        return Err(anyhow!(
                            "Anthropic Streaming API client error {}: {}",
                            status,
                            error_text
                        ));
                    }

                    if status.is_success() {
                        debug!(
                            "Stream request connected: {}ms, status: {}, attempt: {}/{}",
                            connect_time,
                            status,
                            attempt + 1,
                            max_tries
                        );
                        resp
                    } else {
                        let error_text = resp
                            .text()
                            .await
                            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                        let error =
                            anyhow!("Anthropic Streaming API error {}: {}", status, error_text);
                        warn!(
                            "Stream request failed (attempt {}/{}): {}",
                            attempt + 1,
                            max_tries,
                            error
                        );
                        last_error = Some(error);

                        if attempt < max_tries - 1 {
                            let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                            debug!("Retrying after {}ms (attempt {})", delay_ms, attempt + 2);
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        }
                        continue;
                    }
                }
                Err(e) => {
                    let connect_time = request_start_time.elapsed().as_millis();
                    let error = anyhow!("Stream request connection failed: {}", e);
                    warn!(
                        "Stream request connection failed: {}ms, attempt {}/{}, error: {}",
                        connect_time,
                        attempt + 1,
                        max_tries,
                        e
                    );
                    last_error = Some(error);

                    if attempt < max_tries - 1 {
                        let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                        debug!("Retrying after {}ms (attempt {})", delay_ms, attempt + 2);
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                    continue;
                }
            };

            // Success: create channels and return
            let (tx, rx) = mpsc::unbounded_channel();
            let (tx_raw, rx_raw) = mpsc::unbounded_channel();

            tokio::spawn(handle_anthropic_stream(response, tx, Some(tx_raw)));

            return Ok(StreamResponse {
                stream: Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)),
                raw_sse_rx: Some(rx_raw),
            });
        }

        let error_msg = format!(
            "Stream request failed after {} attempts: {}",
            max_tries,
            last_error.unwrap_or_else(|| anyhow!("Unknown error"))
        );
        error!("{}", error_msg);
        Err(anyhow!(error_msg))
    }

    /// Send a message and wait for the full response (non-streaming)
    pub async fn send_message(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<GeminiResponse> {
        let custom_body = self.config.custom_request_body.clone();
        self.send_message_with_extra_body(messages, tools, custom_body)
            .await
    }

    /// Send a message and wait for the full response (non-streaming, with extra body overrides)
    pub async fn send_message_with_extra_body(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
    ) -> Result<GeminiResponse> {
        let stream_response = self
            .send_message_stream_with_extra_body(messages, tools, extra_body)
            .await?;
        let mut stream = stream_response.stream;

        let mut full_text = String::new();
        let mut full_reasoning = String::new();
        let mut finish_reason = None;
        let mut usage = None;
        let mut provider_metadata: Option<serde_json::Value> = None;

        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut pending_tool_call = PendingToolCall::default();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let UnifiedResponse {
                        text,
                        reasoning_content,
                        thinking_signature: _,
                        tool_call,
                        usage: chunk_usage,
                        finish_reason: chunk_finish_reason,
                        provider_metadata: chunk_provider_metadata,
                    } = chunk;

                    if let Some(text) = text {
                        full_text.push_str(&text);
                    }

                    if let Some(reasoning_content) = reasoning_content {
                        full_reasoning.push_str(&reasoning_content);
                    }

                    if let Some(tool_call) = tool_call {
                        let ai_stream_handlers::UnifiedToolCall {
                            id,
                            name,
                            arguments,
                        } = tool_call;

                        if let Some(tool_call_id) = id {
                            if !tool_call_id.is_empty() {
                                // Some providers repeat the tool id on every delta. Only switch when the id changes.
                                let is_new_tool = pending_tool_call.tool_id() != tool_call_id;
                                if is_new_tool {
                                    if let Some(finalized) =
                                        pending_tool_call.finalize(ToolCallBoundary::NewTool)
                                    {
                                        if finalized.is_error {
                                            warn!(
                                                "[send_message] Dropping invalid tool call at boundary=new_tool: tool_id={}, tool_name={}, raw_len={}",
                                                finalized.tool_id,
                                                finalized.tool_name,
                                                finalized.raw_arguments.len()
                                            );
                                        } else {
                                            let arguments = finalized.arguments_as_object_map();
                                            tool_calls.push(ToolCall {
                                                id: finalized.tool_id,
                                                name: finalized.tool_name,
                                                arguments,
                                            });
                                        }
                                    }
                                    pending_tool_call.start_new(tool_call_id, name.clone());
                                    debug!(
                                        "[send_message] Detected tool call: {}",
                                        pending_tool_call.tool_name()
                                    );
                                } else {
                                    pending_tool_call.update_tool_name_if_missing(name.clone());
                                }
                            }
                        }

                        if let Some(tool_call_arguments) = arguments {
                            if pending_tool_call.has_pending() {
                                pending_tool_call.append_arguments(&tool_call_arguments);
                            }
                        }
                    }

                    if let Some(finish_reason_) = chunk_finish_reason {
                        if let Some(finalized) =
                            pending_tool_call.finalize(ToolCallBoundary::FinishReason)
                        {
                            if finalized.is_error {
                                warn!(
                                    "[send_message] Dropping invalid tool call at boundary=finish_reason: tool_id={}, tool_name={}, raw_len={}",
                                    finalized.tool_id,
                                    finalized.tool_name,
                                    finalized.raw_arguments.len()
                                );
                            } else {
                                let arguments = finalized.arguments_as_object_map();
                                tool_calls.push(ToolCall {
                                    id: finalized.tool_id,
                                    name: finalized.tool_name,
                                    arguments,
                                });
                            }
                        }
                        finish_reason = Some(finish_reason_);
                    }

                    if let Some(chunk_usage) = chunk_usage {
                        usage = Some(Self::unified_usage_to_gemini_usage(chunk_usage));
                    }

                    if let Some(chunk_provider_metadata) = chunk_provider_metadata {
                        match provider_metadata.as_mut() {
                            Some(existing) => {
                                Self::merge_json_value(existing, chunk_provider_metadata);
                            }
                            None => provider_metadata = Some(chunk_provider_metadata),
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }

        if let Some(finalized) = pending_tool_call.finalize(ToolCallBoundary::EndOfAggregation) {
            if finalized.is_error {
                warn!(
                    "[send_message] Dropping invalid tool call at boundary=end_of_aggregation: tool_id={}, tool_name={}, raw_len={}",
                    finalized.tool_id,
                    finalized.tool_name,
                    finalized.raw_arguments.len()
                );
            } else {
                let arguments = finalized.arguments_as_object_map();
                tool_calls.push(ToolCall {
                    id: finalized.tool_id,
                    name: finalized.tool_name,
                    arguments,
                });
            }
        }

        let reasoning_content = if full_reasoning.is_empty() {
            None
        } else {
            Some(full_reasoning)
        };

        let tool_calls_result = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        let response = GeminiResponse {
            text: full_text,
            reasoning_content,
            tool_calls: tool_calls_result,
            usage,
            finish_reason,
            provider_metadata,
        };

        Ok(response)
    }

    pub async fn test_connection(&self) -> Result<ConnectionTestResult> {
        let start_time = std::time::Instant::now();

        // Reuse the normal chat request path so the test matches real conversations, even when
        // a provider rejects stricter tool_choice settings such as "required".
        let test_messages = vec![Message::user(
            "Call the get_weather tool for city=Beijing. Do not answer with plain text."
                .to_string(),
        )];
        let tools = Some(vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the weather of a city".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string", "description": "The city to get the weather for" }
                },
                "required": ["city"],
                "additionalProperties": false
            }),
        }]);

        let result = self.send_message(test_messages, tools).await;

        match result {
            Ok(response) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;
                if response.tool_calls.is_some() {
                    Ok(ConnectionTestResult {
                        success: true,
                        response_time_ms,
                        model_response: Some(response.text),
                        message_code: None,
                        error_details: None,
                    })
                } else {
                    Ok(ConnectionTestResult {
                        success: true,
                        response_time_ms,
                        model_response: Some(response.text),
                        message_code: Some(ConnectionTestMessageCode::ToolCallsNotDetected),
                        error_details: None,
                    })
                }
            }
            Err(e) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;
                let error_msg = format!("{}", e);
                debug!("test connection failed: {}", error_msg);
                Ok(ConnectionTestResult {
                    success: false,
                    response_time_ms,
                    model_response: None,
                    message_code: None,
                    error_details: Some(error_msg),
                })
            }
        }
    }

    pub async fn test_image_input_connection(&self) -> Result<ConnectionTestResult> {
        let start_time = std::time::Instant::now();
        let provider = self.config.format.to_ascii_lowercase();
        let prompt = "Inspect the attached image and reply with exactly one 4-letter code for quadrant colors in TL,TR,BL,BR order using letters R,G,B,Y (R=red, G=green, B=blue, Y=yellow).";

        let content = if provider == "anthropic" {
            serde_json::json!([
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": Self::TEST_IMAGE_PNG_BASE64
                    }
                },
                {
                    "type": "text",
                    "text": prompt
                }
            ])
        } else {
            serde_json::json!([
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/png;base64,{}", Self::TEST_IMAGE_PNG_BASE64)
                    }
                },
                {
                    "type": "text",
                    "text": prompt
                }
            ])
        };

        let test_messages = vec![Message {
            role: "user".to_string(),
            content: Some(content.to_string()),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }];

        match self.send_message(test_messages, None).await {
            Ok(response) => {
                let matched = Self::image_test_response_matches_expected(&response.text);

                if matched {
                    Ok(ConnectionTestResult {
                        success: true,
                        response_time_ms: start_time.elapsed().as_millis() as u64,
                        model_response: Some(response.text),
                        message_code: None,
                        error_details: None,
                    })
                } else {
                    let detail = format!(
                        "Image understanding verification failed: expected code '{}', got response '{}'",
                        Self::TEST_IMAGE_EXPECTED_CODE, response.text
                    );
                    debug!("test image input connection failed: {}", detail);
                    Ok(ConnectionTestResult {
                        success: false,
                        response_time_ms: start_time.elapsed().as_millis() as u64,
                        model_response: Some(response.text),
                        message_code: Some(ConnectionTestMessageCode::ImageInputCheckFailed),
                        error_details: Some(detail),
                    })
                }
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                debug!("test image input connection failed: {}", error_msg);
                Ok(ConnectionTestResult {
                    success: false,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    model_response: None,
                    message_code: None,
                    error_details: Some(error_msg),
                })
            }
        }
    }

    pub async fn list_models(&self) -> Result<Vec<RemoteModelInfo>> {
        match self.get_api_format().to_ascii_lowercase().as_str() {
            "openai" | "response" | "responses" => self.list_openai_models().await,
            "anthropic" => self.list_anthropic_models().await,
            format if Self::is_gemini_api_format(format) => self.list_gemini_models().await,
            unsupported => Err(anyhow!(
                "Listing models is not supported for API format: {}",
                unsupported
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AIClient;
    use crate::infrastructure::ai::providers::gemini::GeminiMessageConverter;
    use crate::util::types::{AIConfig, ToolDefinition};
    use serde_json::json;

    fn make_test_client(format: &str, custom_request_body: Option<serde_json::Value>) -> AIClient {
        AIClient::new(AIConfig {
            name: "test".to_string(),
            base_url: "https://example.com/v1".to_string(),
            request_url: "https://example.com/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            format: format.to_string(),
            context_window: 128000,
            max_tokens: Some(8192),
            temperature: None,
            top_p: None,
            enable_thinking_process: false,
            support_preserved_thinking: false,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            custom_request_body,
        })
    }

    #[test]
    fn resolves_openai_models_url_from_completion_endpoint() {
        let client = AIClient::new(AIConfig {
            name: "test".to_string(),
            base_url: "https://api.openai.com/v1/chat/completions".to_string(),
            request_url: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4.1".to_string(),
            format: "openai".to_string(),
            context_window: 128000,
            max_tokens: Some(8192),
            temperature: None,
            top_p: None,
            enable_thinking_process: false,
            support_preserved_thinking: false,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            custom_request_body: None,
        });

        assert_eq!(
            client.resolve_openai_models_url(),
            "https://api.openai.com/v1/models"
        );
    }

    #[test]
    fn resolves_anthropic_models_url_from_messages_endpoint() {
        let client = AIClient::new(AIConfig {
            name: "test".to_string(),
            base_url: "https://api.anthropic.com/v1/messages".to_string(),
            request_url: "https://api.anthropic.com/v1/messages".to_string(),
            api_key: "test-key".to_string(),
            model: "claude-sonnet-4-5".to_string(),
            format: "anthropic".to_string(),
            context_window: 200000,
            max_tokens: Some(8192),
            temperature: None,
            top_p: None,
            enable_thinking_process: false,
            support_preserved_thinking: false,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            custom_request_body: None,
        });

        assert_eq!(
            client.resolve_anthropic_models_url(),
            "https://api.anthropic.com/v1/models"
        );
    }

    #[test]
    fn build_gemini_request_body_translates_response_format_and_merges_generation_config() {
        let client = AIClient::new(AIConfig {
            name: "gemini".to_string(),
            base_url: "https://example.com".to_string(),
            request_url: "https://example.com/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
                .to_string(),
            api_key: "test-key".to_string(),
            model: "gemini-2.5-pro".to_string(),
            format: "gemini".to_string(),
            context_window: 128000,
            max_tokens: Some(4096),
            temperature: Some(0.2),
            top_p: Some(0.8),
            enable_thinking_process: true,
            support_preserved_thinking: true,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            custom_request_body: None,
        });

        let request_body = client.build_gemini_request_body(
            None,
            vec![json!({
                "role": "user",
                "parts": [{ "text": "hello" }]
            })],
            None,
            Some(json!({
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "schema": {
                            "type": "object",
                            "properties": {
                                "answer": { "type": "string" }
                            },
                            "required": ["answer"],
                            "additionalProperties": false
                        }
                    }
                },
                "stop": ["END"],
                "generationConfig": {
                    "candidateCount": 1
                }
            })),
        );

        assert_eq!(request_body["generationConfig"]["maxOutputTokens"], 4096);
        assert_eq!(request_body["generationConfig"]["temperature"], 0.2);
        assert_eq!(request_body["generationConfig"]["topP"], 0.8);
        assert_eq!(
            request_body["generationConfig"]["thinkingConfig"]["includeThoughts"],
            true
        );
        assert_eq!(
            request_body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert_eq!(request_body["generationConfig"]["candidateCount"], 1);
        assert_eq!(
            request_body["generationConfig"]["stopSequences"],
            json!(["END"])
        );
        assert_eq!(
            request_body["generationConfig"]["responseJsonSchema"]["required"],
            json!(["answer"])
        );
        assert!(request_body["generationConfig"]["responseJsonSchema"]
            .get("additionalProperties")
            .is_none());
        assert!(request_body.get("response_format").is_none());
        assert!(request_body.get("stop").is_none());
    }

    #[test]
    fn build_gemini_request_body_omits_function_calling_config_for_native_only_tools() {
        let client = AIClient::new(AIConfig {
            name: "gemini".to_string(),
            base_url: "https://example.com".to_string(),
            request_url: "https://example.com/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
                .to_string(),
            api_key: "test-key".to_string(),
            model: "gemini-2.5-pro".to_string(),
            format: "gemini".to_string(),
            context_window: 128000,
            max_tokens: Some(4096),
            temperature: None,
            top_p: None,
            enable_thinking_process: false,
            support_preserved_thinking: true,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            custom_request_body: None,
        });

        let gemini_tools = GeminiMessageConverter::convert_tools(Some(vec![ToolDefinition {
            name: "WebSearch".to_string(),
            description: "Search the web".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }),
        }]));

        let request_body = client.build_gemini_request_body(
            None,
            vec![json!({
                "role": "user",
                "parts": [{ "text": "hello" }]
            })],
            gemini_tools,
            None,
        );

        assert_eq!(request_body["tools"][0]["googleSearch"], json!({}));
        assert!(request_body.get("toolConfig").is_none());
    }

    #[test]
    fn streaming_http_client_does_not_apply_global_request_timeout() {
        let client = make_test_client("openai", None);
        let request = client
            .client
            .get("https://example.com/stream")
            .build()
            .expect("request should build");

        assert_eq!(request.timeout(), None);
    }
}
