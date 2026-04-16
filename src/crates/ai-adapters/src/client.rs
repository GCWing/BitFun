//! AI client implementation.
//!
//! The client module now acts as a small facade:
//! - `client/*` holds shared transport and aggregation utilities
//! - `providers/*` owns provider-specific request/response adaptation

pub(crate) mod format;
pub(crate) mod healthcheck;
pub(crate) mod http;
pub(crate) mod quirks;
pub(crate) mod response_aggregator;
pub(crate) mod sse;
pub(crate) mod utils;

use crate::providers::{anthropic, gemini, openai};
use crate::types::ProxyConfig;
use crate::types::*;
use anyhow::Result;
use format::ApiFormat;
use reqwest::Client;
use std::time::Duration;
use tokio::sync::mpsc;

/// Streamed response result with the parsed stream and optional raw SSE receiver.
pub struct StreamResponse {
    pub stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<crate::stream::UnifiedResponse>> + Send>,
    >,
    pub raw_sse_rx: Option<mpsc::UnboundedReceiver<String>>,
}

/// Runtime stream behavior shared across provider implementations.
#[derive(Debug, Clone, Default)]
pub struct StreamOptions {
    /// Maximum idle time between streamed chunks. `None` means wait indefinitely.
    pub idle_timeout: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct AIClient {
    pub(crate) client: Client,
    pub config: AIConfig,
    pub(crate) stream_options: StreamOptions,
}

impl AIClient {
    pub(crate) const TEST_IMAGE_EXPECTED_CODE: &'static str = "BYGR";
    pub(crate) const TEST_IMAGE_PNG_BASE64: &'static str =
        "iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAIAAADTED8xAAACBklEQVR42u3ZsREAIAwDMYf9dw4txwJupI7Wua+YZEPBfO91h4ZjAgQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABIAAQAAgABAACAAEAAIAAYAAQAAgABAACAAEAAIAAYAAQAAgABAAAAAAAEDRZI3QGf7jDvEPAAIAAYAAQAAgABAACAAEAAIAAYAAQAAgABAACAAEAAIAAYAAQAAgABAACAABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAACAAEAAIAAQAAgABgABAAAjABAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQAAgABAACAAGAAEAAIAAQALwuLkoG8OSfau4AAAAASUVORK5CYII=";
    pub(crate) const STREAM_CONNECT_TIMEOUT_SECS: u64 = 10;
    pub(crate) const HTTP_POOL_IDLE_TIMEOUT_SECS: u64 = 30;
    pub(crate) const HTTP_TCP_KEEPALIVE_SECS: u64 = 60;

    /// Create an AIClient without proxy.
    pub fn new(config: AIConfig) -> Self {
        Self::new_with_runtime_options(config, None, StreamOptions::default())
    }

    /// Create an AIClient with proxy configuration.
    pub fn new_with_proxy(config: AIConfig, proxy_config: Option<ProxyConfig>) -> Self {
        Self::new_with_runtime_options(config, proxy_config, StreamOptions::default())
    }

    /// Create an AIClient with proxy and runtime stream options.
    pub fn new_with_runtime_options(
        config: AIConfig,
        proxy_config: Option<ProxyConfig>,
        stream_options: StreamOptions,
    ) -> Self {
        let client = http::create_http_client(proxy_config, config.skip_ssl_verify);
        Self {
            client,
            config,
            stream_options,
        }
    }

    pub async fn send_message_stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<StreamResponse> {
        let custom_body = self.config.custom_request_body.clone();
        self.send_message_stream_with_extra_body(messages, tools, custom_body)
            .await
    }

    pub async fn send_message_stream_with_extra_body(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
    ) -> Result<StreamResponse> {
        let max_tries = 3;
        match ApiFormat::parse(&self.config.format)? {
            ApiFormat::OpenAIChat => {
                openai::chat::send_stream(self, messages, tools, extra_body, max_tries).await
            }
            ApiFormat::OpenAIResponses => {
                openai::responses::send_stream(self, messages, tools, extra_body, max_tries).await
            }
            ApiFormat::Anthropic => {
                anthropic::request::send_stream(self, messages, tools, extra_body, max_tries).await
            }
            ApiFormat::Gemini => {
                gemini::request::send_stream(self, messages, tools, extra_body, max_tries).await
            }
        }
    }

    pub async fn send_message(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<GeminiResponse> {
        let custom_body = self.config.custom_request_body.clone();
        self.send_message_with_extra_body(messages, tools, custom_body)
            .await
    }

    pub async fn send_message_with_extra_body(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        extra_body: Option<serde_json::Value>,
    ) -> Result<GeminiResponse> {
        let stream_response = self
            .send_message_stream_with_extra_body(messages, tools, extra_body)
            .await?;
        response_aggregator::aggregate_stream_response(stream_response).await
    }

    pub async fn test_connection(&self) -> Result<ConnectionTestResult> {
        healthcheck::test_connection(self).await
    }

    pub async fn test_image_input_connection(&self) -> Result<ConnectionTestResult> {
        healthcheck::test_image_input_connection(self).await
    }

    pub async fn list_models(&self) -> Result<Vec<RemoteModelInfo>> {
        match ApiFormat::parse(&self.config.format)? {
            ApiFormat::OpenAIChat | ApiFormat::OpenAIResponses => {
                openai::common::list_models(self).await
            }
            ApiFormat::Anthropic => anthropic::discovery::list_models(self).await,
            ApiFormat::Gemini => gemini::discovery::list_models(self).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AIClient;
    use crate::providers::{anthropic, gemini, gemini::GeminiMessageConverter, openai};
    use crate::types::ReasoningMode;
    use crate::types::{AIConfig, ToolDefinition};
    use serde_json::{json, Value};

    fn make_test_client(format: &str, custom_request_body: Option<Value>) -> AIClient {
        AIClient::new(AIConfig {
            name: format!("{}-test", format),
            base_url: "https://example.com/v1".to_string(),
            request_url: "https://example.com/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            format: format.to_string(),
            context_window: 128000,
            max_tokens: Some(8192),
            temperature: None,
            top_p: None,
            reasoning_mode: ReasoningMode::Default,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body,
            custom_request_body_mode: None,
        })
    }

    fn make_trim_test_client(format: &str) -> AIClient {
        let mut client = make_test_client(format, None);
        client.config.custom_request_body_mode = Some("trim".to_string());
        client
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
            reasoning_mode: ReasoningMode::Default,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        });

        assert_eq!(
            openai::common::resolve_models_url(&client),
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
            reasoning_mode: ReasoningMode::Default,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        });

        assert_eq!(
            anthropic::discovery::resolve_models_url(&client),
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
            reasoning_mode: ReasoningMode::Enabled,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        });

        let request_body = gemini::request::build_request_body(
            &client,
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
            reasoning_mode: ReasoningMode::Default,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
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

        let request_body = gemini::request::build_request_body(
            &client,
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
    fn build_openai_request_body_uses_generic_thinking_object_when_enabled() {
        let client = AIClient::new(AIConfig {
            name: "openai-compatible".to_string(),
            base_url: "https://example.com/v1".to_string(),
            request_url: "https://example.com/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "test-model".to_string(),
            format: "openai".to_string(),
            context_window: 128000,
            max_tokens: Some(4096),
            temperature: None,
            top_p: None,
            reasoning_mode: ReasoningMode::Enabled,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        });

        let request_body = openai::chat::build_request_body(
            &client,
            &client.config.request_url,
            vec![json!({ "role": "user", "content": "hello" })],
            None,
            None,
        );

        assert_eq!(request_body["thinking"]["type"], "enabled");
        assert!(request_body.get("enable_thinking").is_none());
        assert!(request_body.get("reasoning_split").is_none());
    }

    #[test]
    fn build_openai_request_body_uses_enable_thinking_for_siliconflow() {
        let client = AIClient::new(AIConfig {
            name: "siliconflow".to_string(),
            base_url: "https://api.siliconflow.cn/v1".to_string(),
            request_url: "https://api.siliconflow.cn/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "Qwen/Qwen3-Coder-480B-A35B-Instruct".to_string(),
            format: "openai".to_string(),
            context_window: 128000,
            max_tokens: Some(4096),
            temperature: None,
            top_p: None,
            reasoning_mode: ReasoningMode::Enabled,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        });

        let request_body = openai::chat::build_request_body(
            &client,
            &client.config.request_url,
            vec![json!({ "role": "user", "content": "hello" })],
            None,
            None,
        );

        assert_eq!(request_body["enable_thinking"], true);
        assert!(request_body.get("thinking").is_none());
    }

    #[test]
    fn build_responses_request_body_maps_disabled_mode_to_none_effort() {
        let client = AIClient::new(AIConfig {
            name: "responses".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            request_url: "https://api.openai.com/v1/responses".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5".to_string(),
            format: "responses".to_string(),
            context_window: 128000,
            max_tokens: Some(4096),
            temperature: None,
            top_p: None,
            reasoning_mode: ReasoningMode::Disabled,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        });

        let request_body = openai::responses::build_request_body(
            &client,
            Some("Be concise".to_string()),
            vec![json!({
                "role": "user",
                "content": [{ "type": "input_text", "text": "hello" }]
            })],
            None,
            None,
        );

        assert_eq!(request_body["reasoning"]["effort"], "none");
    }

    #[test]
    fn build_anthropic_request_body_uses_adaptive_reasoning_and_effort() {
        let client = AIClient::new(AIConfig {
            name: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            request_url: "https://api.anthropic.com/v1/messages".to_string(),
            api_key: "test-key".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            format: "anthropic".to_string(),
            context_window: 200000,
            max_tokens: Some(8192),
            temperature: None,
            top_p: None,
            reasoning_mode: ReasoningMode::Adaptive,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: Some("high".to_string()),
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
        });

        let request_body = anthropic::request::build_request_body(
            &client,
            &client.config.request_url,
            None,
            vec![json!({ "role": "user", "content": [{ "type": "text", "text": "hello" }] })],
            None,
            None,
        );

        assert_eq!(request_body["thinking"]["type"], "adaptive");
        assert_eq!(request_body["output_config"]["effort"], "high");
    }

    #[test]
    fn build_openai_request_body_trim_mode_preserves_essential_fields() {
        let mut client = make_trim_test_client("openai");
        client.config.max_tokens = Some(8192);
        let messages = vec![json!({ "role": "user", "content": "hello" })];

        let request_body = openai::chat::build_request_body(
            &client,
            &client.config.request_url,
            messages.clone(),
            None,
            Some(json!({
                "model": "override-model",
                "messages": [{ "role": "user", "content": "override" }],
                "stream": false,
                "max_tokens": 1,
                "temperature": 0.7,
                "response_format": { "type": "json_object" }
            })),
        );

        assert_eq!(request_body["model"], "test-model");
        assert_eq!(request_body["messages"], json!(messages));
        assert_eq!(request_body["stream"], true);
        assert_eq!(request_body["max_tokens"], 8192);
        assert_eq!(request_body["temperature"], 0.7);
        assert_eq!(request_body["response_format"]["type"], "json_object");
        assert!(request_body.get("thinking").is_none());
    }

    #[test]
    fn build_responses_request_body_trim_mode_preserves_essential_fields() {
        let mut client = make_trim_test_client("responses");
        client.config.max_tokens = Some(4096);
        let input = vec![json!({
            "role": "user",
            "content": [{ "type": "input_text", "text": "hello" }]
        })];

        let request_body = openai::responses::build_request_body(
            &client,
            Some("Be concise".to_string()),
            input.clone(),
            None,
            Some(json!({
                "instructions": "override me",
                "input": [{ "role": "user", "content": [{ "type": "input_text", "text": "override" }] }],
                "stream": false,
                "max_output_tokens": 1,
                "temperature": 0.1
            })),
        );

        assert_eq!(request_body["model"], "test-model");
        assert_eq!(request_body["input"], json!(input));
        assert_eq!(request_body["instructions"], "Be concise");
        assert_eq!(request_body["stream"], true);
        assert_eq!(request_body["max_output_tokens"], 4096);
        assert_eq!(request_body["temperature"], 0.1);
        assert!(request_body.get("reasoning").is_none());
    }

    #[test]
    fn build_anthropic_request_body_trim_mode_preserves_essential_fields() {
        let mut client = make_trim_test_client("anthropic");
        client.config.max_tokens = Some(8192);
        let messages = vec![json!({
            "role": "user",
            "content": [{ "type": "text", "text": "hello" }]
        })];

        let request_body = anthropic::request::build_request_body(
            &client,
            &client.config.request_url,
            Some("Use the system prompt".to_string()),
            messages.clone(),
            None,
            Some(json!({
                "system": "override me",
                "messages": [{ "role": "user", "content": [{ "type": "text", "text": "override" }] }],
                "max_tokens": 1,
                "stream": false,
                "metadata": { "tag": "kept" }
            })),
        );

        assert_eq!(request_body["model"], "test-model");
        assert_eq!(request_body["messages"], json!(messages));
        assert_eq!(request_body["system"], "Use the system prompt");
        assert_eq!(request_body["stream"], true);
        assert_eq!(request_body["max_tokens"], 8192);
        assert_eq!(request_body["metadata"]["tag"], "kept");
        assert!(request_body.get("thinking").is_none());
    }

    #[test]
    fn build_gemini_request_body_trim_mode_preserves_essential_fields() {
        let mut client = make_trim_test_client("gemini");
        client.config.model = "gemini-2.5-pro".to_string();
        client.config.max_tokens = Some(4096);

        let contents = vec![json!({
            "role": "user",
            "parts": [{ "text": "hello" }]
        })];
        let system_instruction = json!({
            "parts": [{ "text": "system" }]
        });
        let gemini_tools = GeminiMessageConverter::convert_tools(Some(vec![ToolDefinition {
            name: "lookup".to_string(),
            description: "Look up data".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        }]));

        let request_body = gemini::request::build_request_body(
            &client,
            Some(system_instruction.clone()),
            contents.clone(),
            gemini_tools,
            Some(json!({
                "contents": [{ "role": "user", "parts": [{ "text": "override" }] }],
                "systemInstruction": { "parts": [{ "text": "override system" }] },
                "generationConfig": {
                    "maxOutputTokens": 1,
                    "candidateCount": 2
                },
                "tools": [],
                "toolConfig": {
                    "functionCallingConfig": {
                        "mode": "NONE"
                    }
                },
                "temperature": 0.3
            })),
        );

        assert_eq!(request_body["contents"], json!(contents));
        assert_eq!(request_body["systemInstruction"], system_instruction);
        assert_eq!(request_body["generationConfig"]["maxOutputTokens"], 4096);
        assert_eq!(request_body["generationConfig"]["candidateCount"], 2);
        assert_eq!(request_body["generationConfig"]["temperature"], 0.3);
        assert_eq!(
            request_body["toolConfig"]["functionCallingConfig"]["mode"],
            "AUTO"
        );
        assert_eq!(
            request_body["tools"][0]["functionDeclarations"][0]["name"],
            "lookup"
        );
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
