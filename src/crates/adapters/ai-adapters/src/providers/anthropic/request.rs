use super::AnthropicMessageConverter;
use crate::client::quirks::{
    is_deepseek_reasoning_effort_model, is_deepseek_url, normalize_deepseek_reasoning_effort,
    should_append_tool_stream,
};
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_anthropic_stream;
use crate::trace::ModelExchangeTraceConfig;
use crate::types::ReasoningMode;
use crate::types::{Message, ToolDefinition};
use anyhow::Result;
use log::{debug, warn};
use reqwest::RequestBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnthropicThinkingCapability {
    ManualOnly,
    AdaptivePreferred,
    AdaptiveOnly,
    AdaptiveDefaultNoDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClaudeModelVersion {
    major: u32,
    minor: u32,
}

/// Anthropic-compatible endpoints whose vendors document `Authorization: Bearer`
/// (ANTHROPIC_AUTH_TOKEN) instead of `x-api-key`: Zhipu bigmodel.cn / Z.AI,
/// Moonshot's /anthropic gateway, and Kimi For Coding.
fn wants_bearer_auth(url: &str) -> bool {
    url.contains("bigmodel.cn")
        || url.contains("api.z.ai")
        || url.contains("api.kimi.com/coding")
        || ((url.contains("api.moonshot.cn") || url.contains("api.moonshot.ai"))
            && url.contains("/anthropic"))
}

pub(crate) fn apply_headers(
    client: &AIClient,
    builder: RequestBuilder,
    url: &str,
) -> RequestBuilder {
    shared::apply_header_policy(client, builder, |mut builder| {
        builder = builder.header("Content-Type", "application/json");

        if wants_bearer_auth(url) {
            builder = builder.header("Authorization", format!("Bearer {}", client.config.api_key));
            // Keep bigmodel.cn exactly as it shipped (no version header); the other
            // bearer gateways follow the Anthropic protocol, which requires it.
            if !url.contains("bigmodel.cn") {
                builder = builder.header("anthropic-version", "2023-06-01");
            }
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

fn parse_claude_model_version(model_name: &str, family: &str) -> Option<ClaudeModelVersion> {
    let prefix = format!("claude-{family}-");
    let rest = model_name.strip_prefix(&prefix)?;
    let mut parts = rest.split('-');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    Some(ClaudeModelVersion { major, minor })
}

fn anthropic_thinking_capability(model_name: &str) -> AnthropicThinkingCapability {
    if model_name.starts_with("claude-mythos") || model_name.starts_with("claude-fable") {
        return AnthropicThinkingCapability::AdaptiveDefaultNoDisabled;
    }

    if let Some(version) = parse_claude_model_version(model_name, "opus") {
        if version.major == 4 && version.minor >= 7 {
            return AnthropicThinkingCapability::AdaptiveOnly;
        }
        if version.major > 4 || (version.major == 4 && version.minor >= 6) {
            return AnthropicThinkingCapability::AdaptivePreferred;
        }
    }

    if let Some(version) = parse_claude_model_version(model_name, "sonnet") {
        if version.major > 4 || (version.major == 4 && version.minor >= 6) {
            return AnthropicThinkingCapability::AdaptivePreferred;
        }
    }

    AnthropicThinkingCapability::ManualOnly
}

fn anthropic_supports_adaptive_reasoning(capability: AnthropicThinkingCapability) -> bool {
    !matches!(capability, AnthropicThinkingCapability::ManualOnly)
}

fn default_anthropic_budget_tokens(max_tokens: Option<u32>) -> Option<u32> {
    max_tokens.map(|value| 10_000u32.min(value.saturating_mul(3) / 4))
}

fn anthropic_adaptive_effort(reasoning_effort: Option<&str>) -> &str {
    reasoning_effort
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("medium")
}

fn apply_anthropic_adaptive_reasoning(
    request_body: &mut serde_json::Value,
    reasoning_effort: Option<&str>,
) {
    request_body["thinking"] = serde_json::json!({ "type": "adaptive" });
    request_body["output_config"] = serde_json::json!({
        "effort": anthropic_adaptive_effort(reasoning_effort)
    });
}

fn apply_deepseek_anthropic_reasoning(
    request_body: &mut serde_json::Value,
    mode: ReasoningMode,
    model_name: &str,
    reasoning_effort: Option<&str>,
) {
    match mode {
        ReasoningMode::Default => {}
        ReasoningMode::Disabled => {
            request_body["thinking"] = serde_json::json!({ "type": "disabled" });
            if reasoning_effort.is_some_and(|value| !value.trim().is_empty()) {
                warn!(
                    target: "ai::anthropic_stream_request",
                    "Omitting output_config.effort for DeepSeek Anthropic model {} because thinking is disabled",
                    model_name
                );
            }
        }
        ReasoningMode::Enabled => {
            request_body["thinking"] = serde_json::json!({ "type": "enabled" });
            if let Some(effort) = reasoning_effort.and_then(normalize_deepseek_reasoning_effort) {
                request_body["output_config"] = serde_json::json!({
                    "effort": effort
                });
            }
        }
        ReasoningMode::Adaptive => {
            warn!(
                target: "ai::anthropic_stream_request",
                "DeepSeek Anthropic model {} does not support adaptive reasoning; falling back to thinking.type=enabled",
                model_name
            );
            apply_deepseek_anthropic_reasoning(
                request_body,
                ReasoningMode::Enabled,
                model_name,
                reasoning_effort,
            );
        }
    }
}

fn apply_reasoning_fields(
    request_body: &mut serde_json::Value,
    mode: ReasoningMode,
    url: &str,
    model_name: &str,
    max_tokens: Option<u32>,
    reasoning_effort: Option<&str>,
    thinking_budget_tokens: Option<u32>,
) {
    let is_deepseek_reasoning_target =
        is_deepseek_url(url) || is_deepseek_reasoning_effort_model(model_name);

    if is_deepseek_reasoning_target {
        apply_deepseek_anthropic_reasoning(request_body, mode, model_name, reasoning_effort);
        return;
    }

    let capability = anthropic_thinking_capability(model_name);

    match mode {
        ReasoningMode::Default => {}
        ReasoningMode::Disabled => {
            if capability == AnthropicThinkingCapability::AdaptiveDefaultNoDisabled {
                warn!(
                    target: "ai::anthropic_stream_request",
                    "Model {} does not support thinking.type=disabled; omitting the field and relying on provider defaults",
                    model_name
                );
            } else {
                request_body["thinking"] = serde_json::json!({ "type": "disabled" });
            }
        }
        ReasoningMode::Enabled => {
            if anthropic_supports_adaptive_reasoning(capability) {
                apply_anthropic_adaptive_reasoning(request_body, reasoning_effort);
                return;
            }

            let mut thinking = serde_json::json!({ "type": "enabled" });
            if let Some(budget_tokens) =
                thinking_budget_tokens.or_else(|| default_anthropic_budget_tokens(max_tokens))
            {
                thinking["budget_tokens"] = serde_json::json!(budget_tokens);
            }
            request_body["thinking"] = thinking;
        }
        ReasoningMode::Adaptive => {
            if anthropic_supports_adaptive_reasoning(capability) {
                apply_anthropic_adaptive_reasoning(request_body, reasoning_effort);
            } else {
                warn!(
                    target: "ai::anthropic_stream_request",
                    "Model {} does not advertise Anthropic adaptive reasoning support; falling back to manual thinking",
                    model_name
                );
                apply_reasoning_fields(
                    request_body,
                    ReasoningMode::Enabled,
                    url,
                    model_name,
                    max_tokens,
                    None,
                    thinking_budget_tokens,
                );
            }
        }
    }

    if mode != ReasoningMode::Adaptive
        && !anthropic_supports_adaptive_reasoning(capability)
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
    let max_tokens = client.config.max_tokens.unwrap_or(32000);

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
        url,
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
    trace: Option<ModelExchangeTraceConfig>,
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
    let ttft_timeout = client.stream_options.ttft_timeout;

    execute_sse_request(
        "Anthropic Streaming API",
        &url,
        &request_body,
        max_tries,
        ttft_timeout,
        trace,
        || apply_headers(client, client.client.post(&url), &url),
        move |response, tx, tx_raw, remaining_ttft_timeout| {
            handle_anthropic_stream(
                response,
                tx,
                tx_raw,
                inline_think_in_text,
                remaining_ttft_timeout,
                idle_timeout,
            )
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_auth_matches_each_gateway_documented_scheme() {
        // Vendors that document ANTHROPIC_AUTH_TOKEN for their Claude-compatible gateway.
        for url in [
            "https://open.bigmodel.cn/api/anthropic/v1/messages",
            "https://api.z.ai/api/anthropic/v1/messages",
            "https://api.kimi.com/coding/v1/messages",
            "https://api.moonshot.cn/anthropic/v1/messages",
            "https://api.moonshot.ai/anthropic/v1/messages",
        ] {
            assert!(
                wants_bearer_auth(url),
                "{url} should authenticate with a bearer token"
            );
        }

        // These document ANTHROPIC_API_KEY (x-api-key) instead, so they must stay on the
        // default branch even though their paths look similar.
        for url in [
            "https://api.deepseek.com/anthropic/v1/messages",
            "https://api.minimaxi.com/anthropic/v1/messages",
            "https://api.minimax.io/anthropic/v1/messages",
            "https://api.siliconflow.cn/v1/messages",
            "https://api.anthropic.com/v1/messages",
        ] {
            assert!(
                !wants_bearer_auth(url),
                "{url} should authenticate with x-api-key"
            );
        }
    }

    #[test]
    fn bearer_auth_holds_for_model_discovery_urls() {
        // Discovery hits /v1/models on the same base, so both paths must agree.
        assert!(wants_bearer_auth("https://api.kimi.com/coding/v1/models"));
        assert!(wants_bearer_auth(
            "https://api.moonshot.cn/anthropic/v1/models"
        ));
        assert!(!wants_bearer_auth("https://api.moonshot.cn/v1/models"));
    }

    #[test]
    fn moonshot_bearer_auth_is_limited_to_the_anthropic_gateway() {
        // The OpenAI-compatible Moonshot base never reaches this module, but keep the
        // qualifier honest so a future caller cannot pick up the wrong scheme.
        assert!(!wants_bearer_auth(
            "https://api.moonshot.cn/v1/chat/completions"
        ));
        assert!(!wants_bearer_auth(
            "https://api.moonshot.ai/v1/chat/completions"
        ));
    }

    #[test]
    fn claude_5_family_thinking_capabilities() {
        use AnthropicThinkingCapability::*;

        // Fable rejects an explicit thinking.type=disabled, so it must omit the field.
        assert_eq!(
            anthropic_thinking_capability("claude-fable-5"),
            AdaptiveDefaultNoDisabled
        );
        assert_eq!(
            anthropic_thinking_capability("claude-mythos-preview"),
            AdaptiveDefaultNoDisabled
        );
        assert_eq!(
            anthropic_thinking_capability("claude-opus-5"),
            AdaptivePreferred
        );
        assert_eq!(
            anthropic_thinking_capability("claude-sonnet-5"),
            AdaptivePreferred
        );
        // Haiku 4.5 takes budget_tokens and errors on effort.
        assert_eq!(
            anthropic_thinking_capability("claude-haiku-4-5"),
            ManualOnly
        );
    }

    #[test]
    fn non_claude_models_never_take_the_claude_thinking_paths() {
        use AnthropicThinkingCapability::*;

        // These reach the Anthropic builder through vendor gateways; emitting Claude's
        // adaptive fields to them would be rejected.
        for model in ["minimax-m3", "glm-5.2", "kimi-k3", "kimi-k2.7-code"] {
            assert_eq!(anthropic_thinking_capability(model), ManualOnly, "{model}");
        }
    }
}
