use super::{common, OpenAIMessageConverter};
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_responses_stream;
use crate::trace::ModelExchangeTraceConfig;
use crate::types::ReasoningMode;
use crate::types::{Message, ToolDefinition};
use anyhow::Result;
use log::debug;

pub(crate) fn build_request_body(
    client: &AIClient,
    instructions: Option<String>,
    response_input: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut request_body = serde_json::json!({
        "model": client.config.model,
        "input": response_input,
        "stream": true
    });

    if let Some(instructions) = instructions.filter(|value| !value.trim().is_empty()) {
        request_body["instructions"] = serde_json::Value::String(instructions);
    }

    if let Some(max_tokens) = client.config.max_tokens {
        request_body["max_output_tokens"] = serde_json::json!(max_tokens);
    }

    let responses_effort = client
        .config
        .reasoning_effort
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            if client.config.reasoning_mode == ReasoningMode::Disabled {
                Some("none".to_string())
            } else {
                None
            }
        });

    if let Some(effort) = responses_effort {
        request_body["reasoning"] = serde_json::json!({
            "effort": effort
        });
    }

    let protected_body = shared::protect_request_body(
        client,
        &mut request_body,
        &[
            "model",
            "input",
            "instructions",
            "stream",
            "max_output_tokens",
        ],
        &[],
    );

    if let Some(extra) = extra_body {
        if let Some(extra_obj) = extra.as_object() {
            shared::merge_extra_body(&mut request_body, extra_obj);
            shared::log_extra_body_keys("ai::responses_stream_request", extra_obj);
        }
    }

    shared::restore_protected_body(&mut request_body, protected_body);

    shared::log_request_body(
        "ai::responses_stream_request",
        "Responses stream request body (excluding tools):",
        &request_body,
    );

    common::attach_tools(
        &mut request_body,
        openai_tools,
        "ai::responses_stream_request",
    );

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
    // Codex CLI's ChatGPT-login backend (`chatgpt.com/backend-api/codex`)
    // speaks a constrained Responses dialect with several extra
    // requirements (flat tool schema, mandatory `instructions`,
    // `store: false`, no `max_output_tokens`, etc.). Keep that adapter
    // self-contained so the standard Responses path stays untouched.
    if super::codex_chatgpt::is_codex_chatgpt_endpoint(&client.config.request_url) {
        return super::codex_chatgpt::send_stream(
            client, messages, tools, extra_body, max_tries, trace,
        )
        .await;
    }

    let url = client.config.request_url.clone();
    debug!(
        "Responses config: model={}, request_url={}, max_tries={}",
        client.config.model, client.config.request_url, max_tries
    );

    let (instructions, response_input) =
        OpenAIMessageConverter::convert_messages_to_responses_input(messages);
    let openai_tools = common::convert_tools_flat(tools);
    let request_body = build_request_body(
        client,
        instructions,
        response_input,
        openai_tools,
        extra_body,
    );
    let idle_timeout = client.stream_options.idle_timeout;
    let ttft_timeout = client.stream_options.ttft_timeout;

    execute_sse_request(
        "Responses API",
        &url,
        &request_body,
        max_tries,
        ttft_timeout,
        trace,
        || common::apply_headers(client, client.client.post(&url)),
        move |response, tx, tx_raw, remaining_ttft_timeout| {
            handle_responses_stream(
                response,
                tx,
                tx_raw,
                remaining_ttft_timeout,
                idle_timeout,
            )
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::build_request_body;
    use crate::types::{ReasoningMode, ToolDefinition};
    use crate::{client::AIClient, types::AIConfig};
    use serde_json::json;

    fn test_client() -> AIClient {
        AIClient::new(AIConfig {
            name: "test".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            request_url: "https://api.openai.com/v1/responses".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.4".to_string(),
            format: "responses".to_string(),
            context_window: 128_000,
            max_tokens: None,
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
        })
    }

    #[test]
    fn attaches_flat_tool_schema_for_responses_api() {
        let client = test_client();
        let request_body = build_request_body(
            &client,
            None,
            vec![json!({
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": "hello" }]
            })],
            crate::providers::openai::common::convert_tools_flat(Some(vec![ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get weather".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    }
                }),
            }])),
            None,
        );

        assert_eq!(request_body["tools"][0]["name"], json!("get_weather"));
        assert_eq!(request_body["tools"][0]["type"], json!("function"));
        assert!(request_body["tools"][0].get("function").is_none());
    }
}
