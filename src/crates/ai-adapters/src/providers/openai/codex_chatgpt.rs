//! Adapter for the Codex CLI ChatGPT-login backend
//! (`https://chatgpt.com/backend-api/codex/responses`).
//!
//! This endpoint speaks a constrained dialect of the OpenAI Responses API
//! used internally by the official `codex` CLI. It is *not* the public
//! `https://api.openai.com/v1/responses` surface — sending a vanilla
//! Responses-shaped body to it produces 400 errors such as:
//!
//! - `Instructions are required`
//! - `Store must be set to false`
//! - `Unsupported parameter: max_output_tokens`
//! - `Missing required parameter: 'tools[0].name'`  (it requires the *flat*
//!   Responses tool schema, not the Chat Completions `{type, function:{...}}`
//!   wrapper)
//!
//! Rather than scattering URL-conditional patches throughout the generic
//! Responses adapter, all backend-specific quirks live in this module.
//! Dispatch happens in `super::responses::send_stream` via
//! [`is_codex_chatgpt_endpoint`].

use super::{common, OpenAIMessageConverter};
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_responses_stream;
use crate::types::{Message, ReasoningMode, ToolDefinition};
use anyhow::Result;
use log::debug;
use serde_json::{json, Value};

const TARGET: &str = "ai::codex_chatgpt_request";
const DEFAULT_INSTRUCTIONS: &str = "You are a helpful AI assistant.";

/// Returns true when `request_url` points at Codex CLI's ChatGPT backend.
pub(crate) fn is_codex_chatgpt_endpoint(request_url: &str) -> bool {
    request_url.contains("chatgpt.com/backend-api/codex")
}

/// Convert a `ToolDefinition` list to the *flat* Responses tool schema that
/// Codex backend (and OpenAI's public Responses API) expects:
/// `{ "type": "function", "name": ..., "description": ..., "parameters": ..., "strict": false }`.
fn convert_tools_flat(tools: Option<Vec<ToolDefinition>>) -> Option<Vec<Value>> {
    tools.map(|defs| {
        defs.into_iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                    "strict": false,
                })
            })
            .collect()
    })
}

fn attach_tools(request_body: &mut Value, tools: Option<Vec<Value>>) {
    if let Some(tools) = tools {
        let names: Vec<String> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(str::to_string))
            .collect();
        shared::log_tool_names(TARGET, names);
        if !tools.is_empty() {
            request_body["tools"] = Value::Array(tools);
            if request_body.get("tool_choice").is_none() {
                request_body["tool_choice"] = Value::String("auto".to_string());
            }
            // Mirror hermes-agent / codex CLI: parallel tool calls allowed.
            if request_body.get("parallel_tool_calls").is_none() {
                request_body["parallel_tool_calls"] = Value::Bool(true);
            }
        }
    }
}

/// Clamp reasoning effort to values accepted by the Codex backend models.
/// `minimal` is rejected by GPT-5.2 / GPT-5.4 family — fall back to `low`,
/// matching hermes-agent's clamp table.
fn clamp_reasoning_effort(effort: &str) -> String {
    match effort {
        "minimal" => "low".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn build_request_body(
    client: &AIClient,
    instructions: Option<String>,
    response_input: Vec<Value>,
    tools_flat: Option<Vec<Value>>,
    extra_body: Option<Value>,
) -> Value {
    let mut body = json!({
        "model": client.config.model,
        "input": response_input,
        "stream": true,
        // Codex backend mandates `store: false`.
        "store": false,
    });

    let resolved_instructions = instructions
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_INSTRUCTIONS.to_string());
    body["instructions"] = Value::String(resolved_instructions);

    // Reasoning — mirror hermes-agent: default effort `medium` when enabled,
    // clamp `minimal -> low`, request encrypted reasoning trace for chain
    // continuity. When explicitly disabled, send `include: []` (empty array)
    // so the backend doesn't attach reasoning items it expects to be replayed.
    if client.config.reasoning_mode != ReasoningMode::Disabled {
        let effort = client
            .config
            .reasoning_effort
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(clamp_reasoning_effort)
            .unwrap_or_else(|| "medium".to_string());
        body["reasoning"] = json!({ "effort": effort, "summary": "auto" });
        body["include"] = json!(["reasoning.encrypted_content"]);
    } else {
        body["include"] = json!([]);
    }

    let protected = shared::protect_request_body(
        client,
        &mut body,
        &[
            "model",
            "input",
            "instructions",
            "stream",
            "store",
            "include",
        ],
        &[],
    );

    if let Some(extra) = extra_body {
        if let Some(extra_obj) = extra.as_object() {
            shared::merge_extra_body(&mut body, extra_obj);
            shared::log_extra_body_keys(TARGET, extra_obj);
        }
    }

    shared::restore_protected_body(&mut body, protected);

    shared::log_request_body(
        TARGET,
        "Codex ChatGPT request body (excluding tools):",
        &body,
    );

    attach_tools(&mut body, tools_flat);

    body
}

pub(crate) async fn send_stream(
    client: &AIClient,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    extra_body: Option<Value>,
    max_tries: usize,
) -> Result<StreamResponse> {
    let url = client.config.request_url.clone();
    debug!(
        "CodexChatGPT config: model={}, request_url={}, max_tries={}",
        client.config.model, url, max_tries
    );

    let (instructions, response_input) =
        OpenAIMessageConverter::convert_messages_to_responses_input(messages);
    let tools_flat = convert_tools_flat(tools);
    let request_body =
        build_request_body(client, instructions, response_input, tools_flat, extra_body);
    let idle_timeout = client.stream_options.idle_timeout;

    execute_sse_request(
        "Codex ChatGPT Responses API",
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
