use super::AnthropicMessageConverter;
use crate::client::quirks::should_append_tool_stream;
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_anthropic_stream;
use crate::types::ReasoningMode;
use crate::types::{Message, ToolDefinition};
use anyhow::Result;
use log::{debug, warn};
use reqwest::RequestBuilder;

pub(crate) fn apply_headers(
    client: &AIClient,
    builder: RequestBuilder,
    url: &str,
) -> RequestBuilder {
    shared::apply_header_policy(client, builder, |mut builder| {
        builder = builder.header("Content-Type", "application/json");

        if url.contains("bigmodel.cn") {
            builder = builder.header("Authorization", format!("Bearer {}", client.config.api_key));
        } else {
            builder = builder
                .header("x-api-key", &client.config.api_key)
                .header("anthropic-version", "2023-06-01");
        }

        if url.contains("openbitfun.com") {
            builder = builder.header("X-Verification-Code", "from_bitfun");
        }

        builder
    })
}

fn anthropic_supports_adaptive_reasoning(model_name: &str) -> bool {
    matches!(
        model_name,
        name if name.starts_with("claude-opus-4-6")
            || name.starts_with("claude-sonnet-4-6")
            || name.starts_with("claude-mythos")
    )
}

fn anthropic_supports_thinking_budget(model_name: &str) -> bool {
    model_name.starts_with("claude")
}

fn default_anthropic_budget_tokens(max_tokens: Option<u32>) -> Option<u32> {
    max_tokens.map(|value| 10_000u32.min(value.saturating_mul(3) / 4))
}

fn apply_reasoning_fields(
    request_body: &mut serde_json::Value,
    mode: ReasoningMode,
    model_name: &str,
    max_tokens: Option<u32>,
    reasoning_effort: Option<&str>,
    thinking_budget_tokens: Option<u32>,
) {
    match mode {
        ReasoningMode::Default => {}
        ReasoningMode::Disabled => {
            request_body["thinking"] = serde_json::json!({ "type": "disabled" });
        }
        ReasoningMode::Enabled => {
            let mut thinking = serde_json::json!({ "type": "enabled" });
            if anthropic_supports_thinking_budget(model_name) {
                if let Some(budget_tokens) =
                    thinking_budget_tokens.or_else(|| default_anthropic_budget_tokens(max_tokens))
                {
                    thinking["budget_tokens"] = serde_json::json!(budget_tokens);
                }
            }
            request_body["thinking"] = thinking;
        }
        ReasoningMode::Adaptive => {
            if anthropic_supports_adaptive_reasoning(model_name) {
                request_body["thinking"] = serde_json::json!({ "type": "adaptive" });
                if let Some(effort) = reasoning_effort.filter(|value| !value.trim().is_empty()) {
                    request_body["output_config"] = serde_json::json!({
                        "effort": effort
                    });
                }
            } else {
                warn!(
                    target: "ai::anthropic_stream_request",
                    "Model {} does not advertise Anthropic adaptive reasoning support; falling back to manual thinking",
                    model_name
                );
                apply_reasoning_fields(
                    request_body,
                    ReasoningMode::Enabled,
                    model_name,
                    max_tokens,
                    None,
                    thinking_budget_tokens,
                );
            }
        }
    }

    if mode != ReasoningMode::Adaptive
        && reasoning_effort.is_some_and(|value| !value.trim().is_empty())
    {
        warn!(
            target: "ai::anthropic_stream_request",
            "Ignoring reasoning_effort for Anthropic model {} because effort currently applies only to adaptive reasoning mode",
            model_name
        );
    }
}

pub(crate) fn build_request_body(
    client: &AIClient,
    url: &str,
    system_message: Option<String>,
    anthropic_messages: Vec<serde_json::Value>,
    anthropic_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> serde_json::Value {
    let max_tokens = client.config.max_tokens.unwrap_or(8192);

    let mut request_body = serde_json::json!({
        "model": client.config.model,
        "messages": anthropic_messages,
        "max_tokens": max_tokens,
        "stream": true
    });

    let model_name = client.config.model.to_lowercase();

    if should_append_tool_stream(url, &model_name) {
        request_body["tool_stream"] = serde_json::Value::Bool(true);
    }

    apply_reasoning_fields(
        &mut request_body,
        client.config.reasoning_mode,
        &model_name,
        Some(max_tokens),
        client.config.reasoning_effort.as_deref(),
        client.config.thinking_budget_tokens,
    );

    if let Some(system) = system_message {
        request_body["system"] = serde_json::Value::String(system);
    }

    let protected_body = shared::protect_request_body(
        client,
        &mut request_body,
        &[
            "model",
            "messages",
            "max_tokens",
            "stream",
            "system",
            "tool_stream",
        ],
        &[],
    );

    if let Some(extra) = extra_body {
        if let Some(extra_obj) = extra.as_object() {
            shared::merge_extra_body(&mut request_body, extra_obj);
            shared::log_extra_body_keys("ai::anthropic_stream_request", extra_obj);
        }
    }

    shared::restore_protected_body(&mut request_body, protected_body);

    shared::log_request_body(
        "ai::anthropic_stream_request",
        "Anthropic stream request body (excluding tools):",
        &request_body,
    );

    if let Some(tools) = anthropic_tools {
        let tool_names = tools
            .iter()
            .map(|tool| {
                shared::extract_top_level_string_field(tool, "name")
                    .unwrap_or_else(|| "unknown".to_string())
            })
            .collect::<Vec<_>>();
        shared::log_tool_names("ai::anthropic_stream_request", tool_names);
        if !tools.is_empty() {
            request_body["tools"] = serde_json::Value::Array(tools);
        }
    }

    request_body
}

pub(crate) async fn send_stream(
    client: &AIClient,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    extra_body: Option<serde_json::Value>,
    max_tries: usize,
) -> Result<StreamResponse> {
    let url = client.config.request_url.clone();
    debug!(
        "Anthropic config: model={}, request_url={}, max_tries={}",
        client.config.model, client.config.request_url, max_tries
    );

    let (system_message, anthropic_messages) =
        AnthropicMessageConverter::convert_messages(messages);
    let anthropic_tools = AnthropicMessageConverter::convert_tools(tools);
    let request_body = build_request_body(
        client,
        &url,
        system_message,
        anthropic_messages,
        anthropic_tools,
        extra_body,
    );
    let inline_think_in_text = client.config.inline_think_in_text;
    let idle_timeout = client.stream_options.idle_timeout;

    execute_sse_request(
        "Anthropic Streaming API",
        &url,
        &request_body,
        max_tries,
        || apply_headers(client, client.client.post(&url), &url),
        move |response, tx, tx_raw| {
            tokio::spawn(handle_anthropic_stream(
                response,
                tx,
                tx_raw,
                inline_think_in_text,
                idle_timeout,
            ));
        },
    )
    .await
}
