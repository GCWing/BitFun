//! AI client implementation - refactored version
//!
//! Uses a modular architecture to separate provider-specific logic into the providers module

use crate::infrastructure::ai::providers::anthropic::AnthropicMessageConverter;
use crate::infrastructure::ai::providers::openai::OpenAIMessageConverter;
use crate::service::config::ProxyConfig;
use crate::util::types::*;
use crate::util::JsonChecker;
use ai_stream_handlers::{handle_anthropic_stream, handle_openai_stream, UnifiedResponse};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use log::{debug, error, info, warn};
use reqwest::{Client, Proxy};
use std::collections::HashMap;
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

impl AIClient {
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
            .timeout(std::time::Duration::from_secs(600))
            .connect_timeout(std::time::Duration::from_secs(10))
            .user_agent("BitFun/1.0")
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(4)
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
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
            .map_or(false, |h| !h.is_empty());
        let is_merge_mode = self.is_merge_headers_mode();

        if has_custom_headers && !is_merge_mode {
            return self.apply_custom_headers(builder);
        }

        builder = builder
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key));

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
            .map_or(false, |h| !h.is_empty());
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

        if has_custom_headers && is_merge_mode {
            builder = self.apply_custom_headers(builder);
        }

        builder
    }

    /// Build an OpenAI-format request body
    fn build_openai_request_body(
        &self,
        openai_messages: Vec<serde_json::Value>,
        openai_tools: Option<Vec<serde_json::Value>>,
        extra_body: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut request_body = serde_json::json!({
            "model": self.config.model,
            "messages": openai_messages,
            "temperature": 0.7,
            "top_p": 1.0,
            "stream": true
        });

        let model_name = self.config.model.to_lowercase();

        if model_name == "glm-4.6" || model_name == "glm-4.7" {
            request_body["tool_stream"] = serde_json::Value::Bool(true);
        }

        request_body["thinking"] = serde_json::json!({
            "type": if self.config.enable_thinking_process { "enabled" } else { "disabled" }
        });

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

        debug!(target: "ai::openai_stream_request",
            "OpenAI stream request body (excluding tools):\n{}",
            serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| "serialization failed".to_string())
        );

        if let Some(tools) = openai_tools {
            let tool_names = tools
                .iter()
                .map(|tool| Self::extract_openai_tool_name(tool))
                .collect::<Vec<_>>();
            debug!(target: "ai::openai_stream_request", "\ntools: {:?}", tool_names);
            if !tools.is_empty() {
                request_body["tools"] = serde_json::Value::Array(tools);
                request_body["tool_choice"] = serde_json::Value::String("auto".to_string());
            }
        }

        request_body
    }

    /// Build an Anthropic-format request body
    fn build_anthropic_request_body(
        &self,
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

        // TODO: Zhipu tool streaming currently only supports the OpenAI format
        if model_name == "glm-4.6" || model_name == "glm-4.7" {
            request_body["tool_stream"] = serde_json::Value::Bool(true);
        }

        request_body["thinking"] = if self.config.enable_thinking_process {
            if model_name.starts_with("claude") {
                serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": 10000u32.min(max_tokens * 3 / 4)
                })
            } else {
                serde_json::json!({
                    "type": "enabled"
                })
            }
        } else {
            serde_json::json!({
                "type": "disabled"
            })
        };

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
                .map(|tool| Self::extract_anthropic_tool_name(tool))
                .collect::<Vec<_>>();
            debug!(target: "ai::anthropic_stream_request", "\ntools: {:?}", tool_names);
            if !tools.is_empty() {
                request_body["tools"] = serde_json::Value::Array(tools);
            }
        }

        request_body
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
        let url = self.config.base_url.clone();
        debug!(
            "OpenAI config: model={}, base_url={}, max_tries={}",
            self.config.model, self.config.base_url, max_tries
        );

        // Use OpenAI message converter
        let openai_messages = OpenAIMessageConverter::convert_messages(messages);
        let openai_tools = OpenAIMessageConverter::convert_tools(tools);

        // Build request body
        let request_body =
            self.build_openai_request_body(openai_messages, openai_tools, extra_body);

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

            tokio::spawn(handle_openai_stream(response, tx, Some(tx_raw)));

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
        let url = self.config.base_url.clone();
        debug!(
            "Anthropic config: model={}, base_url={}, max_tries={}",
            self.config.model, self.config.base_url, max_tries
        );

        // Use Anthropic message converter
        let (system_message, anthropic_messages) =
            AnthropicMessageConverter::convert_messages(messages);
        let anthropic_tools = AnthropicMessageConverter::convert_tools(tools);

        // Build request body
        let request_body = self.build_anthropic_request_body(
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

        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut cur_tool_call_id = String::new();
        let mut cur_tool_call_name = String::new();
        let mut json_checker = JsonChecker::new();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if let Some(text) = chunk.text {
                        full_text.push_str(&text);
                    }

                    if let Some(reasoning_content) = chunk.reasoning_content {
                        full_reasoning.push_str(&reasoning_content);
                    }

                    if let Some(finish_reason_) = chunk.finish_reason {
                        finish_reason = Some(finish_reason_);
                    }

                    if let Some(tool_call) = chunk.tool_call {
                        if let Some(tool_call_id) = tool_call.id {
                            if !tool_call_id.is_empty() {
                                cur_tool_call_id = tool_call_id;
                                cur_tool_call_name = tool_call.name.unwrap_or_default();
                                json_checker.reset();
                                debug!("[send_message] Detected tool call: {}", cur_tool_call_name);
                            }
                        }

                        if let Some(ref tool_call_arguments) = tool_call.arguments {
                            json_checker.append(tool_call_arguments);
                        }

                        if json_checker.is_valid() {
                            let arguments_string = json_checker.get_buffer();
                            let arguments: HashMap<String, serde_json::Value> =
                                serde_json::from_str(&arguments_string).unwrap_or_else(|e| {
                                    error!(
                                        "[send_message] Failed to parse tool arguments: {}, arguments: {}",
                                        e,
                                        arguments_string
                                    );
                                    HashMap::new()
                                });
                            tool_calls.push(ToolCall {
                                id: cur_tool_call_id.clone(),
                                name: cur_tool_call_name.clone(),
                                arguments,
                            });
                            debug!(
                                "[send_message] Tool call arguments complete: {}",
                                cur_tool_call_name
                            );
                            json_checker.reset();
                        }
                    }
                }
                Err(e) => return Err(e),
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
            usage: None,
            finish_reason,
        };

        Ok(response)
    }

    pub async fn test_connection(&self) -> Result<ConnectionTestResult> {
        let start_time = std::time::Instant::now();

        let test_messages = vec![Message::user("What's the weather in Beijing?".to_string())];
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

        match self.send_message(test_messages, tools).await {
            Ok(response) => {
                let response_time_ms = start_time.elapsed().as_millis() as u64;
                if response.tool_calls.is_some() {
                    Ok(ConnectionTestResult {
                        success: true,
                        response_time_ms,
                        model_response: Some(response.text),
                        error_details: None,
                    })
                } else {
                    Ok(ConnectionTestResult {
                        success: false,
                        response_time_ms,
                        model_response: Some(response.text),
                        error_details: Some("Model does not support tool calls".to_string()),
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
                    error_details: Some(error_msg),
                })
            }
        }
    }
}
