use super::{common, OpenAIMessageConverter};
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_responses_stream;
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
) -> Result<StreamResponse> {
    let url = client.config.request_url.clone();
    debug!(
        "Responses config: model={}, request_url={}, max_tries={}",
        client.config.model, client.config.request_url, max_tries
    );

    let (instructions, response_input) =
        OpenAIMessageConverter::convert_messages_to_responses_input(messages);
    let openai_tools = OpenAIMessageConverter::convert_tools(tools);
    let request_body = build_request_body(
        client,
        instructions,
        response_input,
        openai_tools,
        extra_body,
    );
    let idle_timeout = client.stream_options.idle_timeout;

    execute_sse_request(
        "Responses API",
        &url,
        &request_body,
        max_tries,
        || common::apply_headers(client, client.client.post(&url)),
        move |response, tx, tx_raw| {
            tokio::spawn(handle_responses_stream(response, tx, tx_raw, idle_timeout));
        },
    )
    .await
}
