use super::{common, OpenAIMessageConverter};
use crate::client::quirks::should_append_tool_stream;
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_openai_stream;
use crate::types::{Message, ToolDefinition};
use anyhow::Result;
use log::{debug, warn};

pub(crate) fn build_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut request_body = serde_json::json!({
        "model": client.config.model,
        "messages": openai_messages,
        "stream": true
    });

    let model_name = client.config.model.to_lowercase();

    if should_append_tool_stream(url, &model_name) {
        request_body["tool_stream"] = serde_json::Value::Bool(true);
    }

    common::apply_reasoning_fields(&mut request_body, client, url);

    if let Some(max_tokens) = client.config.max_tokens {
        request_body["max_tokens"] = serde_json::json!(max_tokens);
    }

    let protected_body = shared::protect_request_body(
        client,
        &mut request_body,
        &["model", "messages", "stream", "max_tokens", "tool_stream"],
        &[],
    );

    if let Some(extra) = extra_body {
        if let Some(extra_obj) = extra.as_object() {
            shared::merge_extra_body(&mut request_body, extra_obj);
            shared::log_extra_body_keys("ai::openai_stream_request", extra_obj);
        }
    }

    shared::restore_protected_body(&mut request_body, protected_body);

    if let Some(request_obj) = request_body.as_object_mut() {
        if let Some(existing_n) = request_obj.remove("n") {
            warn!(
                target: "ai::openai_stream_request",
                "Removed custom request field n={} because the stream processor only handles the first choice",
                existing_n
            );
        }
    }

    shared::log_request_body(
        "ai::openai_stream_request",
        "OpenAI stream request body (excluding tools):",
        &request_body,
    );

    common::attach_tools(&mut request_body, openai_tools, "ai::openai_stream_request");

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
        "OpenAI config: model={}, request_url={}, max_tries={}",
        client.config.model, client.config.request_url, max_tries
    );

    let openai_messages = OpenAIMessageConverter::convert_messages(messages);
    let openai_tools = OpenAIMessageConverter::convert_tools(tools);
    let request_body = build_request_body(client, &url, openai_messages, openai_tools, extra_body);
    let inline_think_in_text = client.config.inline_think_in_text;
    let idle_timeout = client.stream_options.idle_timeout;

    execute_sse_request(
        "OpenAI Streaming API",
        &url,
        &request_body,
        max_tries,
        || common::apply_headers(client, client.client.post(&url)),
        move |response, tx, tx_raw| {
            tokio::spawn(handle_openai_stream(
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
