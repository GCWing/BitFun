use super::GeminiMessageConverter;
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_gemini_stream;
use crate::types::ReasoningMode;
use crate::types::{Message, ToolDefinition};
use anyhow::Result;
use log::debug;
use reqwest::RequestBuilder;

pub(crate) fn apply_headers(client: &AIClient, builder: RequestBuilder) -> RequestBuilder {
    shared::apply_header_policy(client, builder, |mut builder| {
        builder = builder
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &client.config.api_key)
            .header("Authorization", format!("Bearer {}", client.config.api_key));

        if client.config.base_url.contains("openbitfun.com") {
            builder = builder.header("X-Verification-Code", "from_bitfun");
        }

        builder
    })
}

pub(crate) fn gemini_base_url(url: &str) -> &str {
    let mut value = url.trim().trim_end_matches('/');
    if let Some(pos) = value.find("/v1beta") {
        value = &value[..pos];
    }
    if let Some(pos) = value.find("/models/") {
        value = &value[..pos];
    }
    value.trim_end_matches('/')
}

pub(crate) fn resolve_request_url(base_url: &str, model_name: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }

    let base = gemini_base_url(trimmed);
    let encoded_model = urlencoding::encode(model_name.trim());
    format!(
        "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
        base, encoded_model
    )
}

fn apply_reasoning_fields(request_body: &mut serde_json::Value, mode: ReasoningMode) {
    if matches!(mode, ReasoningMode::Enabled | ReasoningMode::Adaptive) {
        insert_generation_field(
            request_body,
            "thinkingConfig",
            serde_json::json!({
                "includeThoughts": true,
            }),
        );
    }
}

fn ensure_generation_config(
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

fn insert_generation_field(
    request_body: &mut serde_json::Value,
    key: &str,
    value: serde_json::Value,
) {
    ensure_generation_config(request_body).insert(key.to_string(), value);
}

fn normalize_stop_sequences(value: &serde_json::Value) -> Option<serde_json::Value> {
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

fn apply_response_format_translation(
    request_body: &mut serde_json::Value,
    response_format: &serde_json::Value,
) -> bool {
    match response_format {
        serde_json::Value::String(kind) if matches!(kind.as_str(), "json" | "json_object") => {
            insert_generation_field(
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
                    insert_generation_field(
                        request_body,
                        "responseMimeType",
                        serde_json::Value::String("application/json".to_string()),
                    );
                    true
                }
                "json_schema" => {
                    insert_generation_field(
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
                        insert_generation_field(
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

fn translate_extra_body(
    request_body: &mut serde_json::Value,
    extra_obj: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Some(max_tokens) = extra_obj.remove("max_tokens") {
        insert_generation_field(request_body, "maxOutputTokens", max_tokens);
    }

    if let Some(temperature) = extra_obj.remove("temperature") {
        insert_generation_field(request_body, "temperature", temperature);
    }

    let top_p = extra_obj
        .remove("top_p")
        .or_else(|| extra_obj.remove("topP"));
    if let Some(top_p) = top_p {
        insert_generation_field(request_body, "topP", top_p);
    }

    if let Some(stop_sequences) = extra_obj.get("stop").and_then(normalize_stop_sequences) {
        extra_obj.remove("stop");
        insert_generation_field(request_body, "stopSequences", stop_sequences);
    }

    if let Some(response_mime_type) = extra_obj
        .remove("responseMimeType")
        .or_else(|| extra_obj.remove("response_mime_type"))
    {
        insert_generation_field(request_body, "responseMimeType", response_mime_type);
    }

    if let Some(response_schema) = extra_obj
        .remove("responseJsonSchema")
        .or_else(|| extra_obj.remove("responseSchema"))
        .or_else(|| extra_obj.remove("response_schema"))
    {
        insert_generation_field(
            request_body,
            "responseJsonSchema",
            GeminiMessageConverter::sanitize_schema(response_schema),
        );
    }

    if let Some(response_format) = extra_obj.get("response_format").cloned() {
        if apply_response_format_translation(request_body, &response_format) {
            extra_obj.remove("response_format");
        }
    }
}

pub(crate) fn build_request_body(
    client: &AIClient,
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

    if let Some(max_tokens) = client.config.max_tokens {
        insert_generation_field(
            &mut request_body,
            "maxOutputTokens",
            serde_json::json!(max_tokens),
        );
    }

    if let Some(temperature) = client.config.temperature {
        insert_generation_field(
            &mut request_body,
            "temperature",
            serde_json::json!(temperature),
        );
    }

    if let Some(top_p) = client.config.top_p {
        insert_generation_field(&mut request_body, "topP", serde_json::json!(top_p));
    }

    apply_reasoning_fields(&mut request_body, client.config.reasoning_mode);

    if let Some(tools) = gemini_tools {
        let tool_names = tools
            .iter()
            .flat_map(shared::collect_function_declaration_names_or_object_keys)
            .collect::<Vec<_>>();
        shared::log_tool_names("ai::gemini_stream_request", tool_names);

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

    let protected_body = shared::protect_request_body(
        client,
        &mut request_body,
        &["contents", "systemInstruction", "tools", "toolConfig"],
        &[("generationConfig", "maxOutputTokens")],
    );

    if let Some(extra) = extra_body {
        if let Some(mut extra_obj) = extra.as_object().cloned() {
            translate_extra_body(&mut request_body, &mut extra_obj);
            let override_keys = extra_obj.keys().cloned().collect::<Vec<_>>();
            shared::merge_extra_body_recursively(&mut request_body, extra_obj);
            debug!(
                target: "ai::gemini_stream_request",
                "Applied extra_body overrides: {:?}",
                override_keys
            );
        }
    }

    shared::restore_protected_body(&mut request_body, protected_body);

    shared::log_request_body(
        "ai::gemini_stream_request",
        "Gemini stream request body:",
        &request_body,
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
    let url = resolve_request_url(&client.config.request_url, &client.config.model);
    debug!(
        "Gemini config: model={}, request_url={}, max_tries={}",
        client.config.model, url, max_tries
    );

    let (system_instruction, contents) =
        GeminiMessageConverter::convert_messages(messages, &client.config.model);
    let gemini_tools = GeminiMessageConverter::convert_tools(tools);
    let request_body = build_request_body(
        client,
        system_instruction,
        contents,
        gemini_tools,
        extra_body,
    );
    let idle_timeout = client.stream_options.idle_timeout;

    execute_sse_request(
        "Gemini Streaming API",
        &url,
        &request_body,
        max_tries,
        || apply_headers(client, client.client.post(&url)),
        move |response, tx, tx_raw| {
            tokio::spawn(handle_gemini_stream(response, tx, tx_raw, idle_timeout));
        },
    )
    .await
}
