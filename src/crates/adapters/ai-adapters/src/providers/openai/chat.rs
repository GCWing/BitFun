use super::{common, OpenAIMessageConverter};
use crate::client::quirks::should_append_tool_stream;
use crate::client::sse::execute_sse_request;
#[cfg(feature = "subscription-auth")]
use crate::client::sse::execute_sse_request_with_raw_body;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_openai_stream;
#[cfg(feature = "subscription-auth")]
use crate::stream::handle_qoder_stream;
use crate::trace::ModelExchangeTraceConfig;
use crate::types::{Message, ModelRequestContext, ToolDefinition};
#[cfg(not(feature = "subscription-auth"))]
use anyhow::anyhow;
use anyhow::Result;
use log::{debug, warn};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub(crate) fn try_build_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let mut request_body = serde_json::json!({
        "model": client.config.model,
        "messages": openai_messages,
        "stream": true
    });

    let model_name = client.config.model.to_lowercase();

    if should_append_tool_stream(url, &model_name) {
        request_body["tool_stream"] = serde_json::Value::Bool(true);
    }

    let base_reasoning_fields = shared::capture_reasoning_fields(
        &request_body,
        &["thinking", "enable_thinking", "reasoning_effort"],
        &[],
    );

    if let Some(max_tokens) = client.config.max_tokens {
        request_body["max_tokens"] = serde_json::json!(max_tokens);
    }

    let protected_keys = &[
        "model",
        "messages",
        "stream",
        "max_tokens",
        "tool_stream",
        "tools",
    ];
    if let Some(preset) = client.model_reasoning_preset.as_ref() {
        shared::apply_reasoning_actions(
            preset,
            &mut request_body,
            protected_keys,
            &[],
            |action, body| {
                common::compile_chat_reasoning_action(
                    preset,
                    action,
                    body,
                    url,
                    &client.config.model,
                )
            },
        )?;
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
    if let Some(preset) = client.selected_reasoning_preset.as_ref() {
        shared::reset_reasoning_fields(
            &mut request_body,
            base_reasoning_fields.as_ref(),
            &["thinking", "enable_thinking", "reasoning_effort"],
            &[],
        );
        shared::apply_reasoning_actions(
            preset,
            &mut request_body,
            protected_keys,
            &[],
            |action, body| {
                common::compile_chat_reasoning_action(
                    preset,
                    action,
                    body,
                    url,
                    &client.config.model,
                )
            },
        )?;
    }

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

    Ok(request_body)
}

#[cfg(test)]
pub(crate) fn build_request_body(
    client: &AIClient,
    url: &str,
    openai_messages: Vec<serde_json::Value>,
    openai_tools: Option<Vec<serde_json::Value>>,
    extra_body: Option<serde_json::Value>,
) -> serde_json::Value {
    try_build_request_body(client, url, openai_messages, openai_tools, extra_body)
        .expect("request body should compile")
}

/// Generates a 32-character hex string suitable for CodeBuddy request IDs.
/// Uses a monotonic counter + timestamp hashed with SHA-256 for uniqueness.
static HEX32_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_hex32() -> String {
    let count = HEX32_COUNTER.fetch_add(1, Ordering::Relaxed);
    let time_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = Sha256::new();
    hasher.update(count.to_le_bytes());
    hasher.update(time_nanos.to_le_bytes());
    let hash = hasher.finalize();
    hex::encode(&hash[..16])
}

/// Collects the official CodeBuddy (`copilot.tencent.com`) conversation
/// fingerprint headers (mirrors the official CLI request assembly, see recon
/// report R-CB-CONVID 2026-08-21):
/// - `X-Conversation-ID`: session-stable (BitFun `session_id`).
/// - `X-Conversation-Request-ID`: turn-stable (one value per user prompt,
///   shared by every request/retry of that turn); falls back to a per-request
///   value only when the turn-level ID is unavailable.
/// - `X-Agent-Intent`/`X-Agent-Purpose`/`X-Product`/`X-IDE-*`/
///   `X-Product-Version`/`X-Requested-With`: official client fingerprint.
///
/// Request-unique IDs (`X-Request-ID`/`X-Conversation-Message-ID`) are
/// appended by the caller so this function stays deterministic for tests.
fn codebuddy_fingerprint_headers(
    request_context: Option<&ModelRequestContext>,
) -> Vec<(&'static str, String)> {
    let mut headers: Vec<(&'static str, String)> = Vec::new();
    if let Some(ctx) = request_context {
        if let Some(sid) = &ctx.session_id {
            headers.push(("X-Conversation-ID", sid.clone()));
        }
        headers.push((
            "X-Conversation-Request-ID",
            ctx.conversation_request_id
                .clone()
                .unwrap_or_else(generate_hex32),
        ));
    } else {
        headers.push(("X-Conversation-Request-ID", generate_hex32()));
    }
    headers.push(("X-Agent-Intent", "craft".to_string()));
    headers.push(("X-Agent-Purpose", "conversation".to_string()));
    headers.push(("X-Product", "SaaS".to_string()));
    // Official CLI client info (codebuddy.js module 33387 + clientInfoProvider):
    // PRODUCT_TYPE="CLI"; platform defaults to PRODUCT_TYPE; ideType/ideName
    // both fall back to platform; version = CLI package version 2.137.1.
    headers.push(("X-IDE-Type", "CLI".to_string()));
    headers.push(("X-IDE-Name", "CLI".to_string()));
    headers.push(("X-IDE-Version", "2.137.1".to_string()));
    headers.push(("X-Product-Version", "2.137.1".to_string()));
    headers.push(("X-Requested-With", "XMLHttpRequest".to_string()));
    headers
}

pub(crate) async fn send_stream(
    client: &AIClient,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    extra_body: Option<serde_json::Value>,
    max_tries: usize,
    trace: Option<ModelExchangeTraceConfig>,
    request_context: Option<ModelRequestContext>,
) -> Result<StreamResponse> {
    let url = client.config.request_url.clone();
    debug!(
        "OpenAI config: model={}, request_url={}, max_tries={}",
        client.config.model, client.config.request_url, max_tries
    );

    let openai_messages = OpenAIMessageConverter::convert_messages(messages);
    let openai_tools = OpenAIMessageConverter::convert_tools(tools);
    let request_body =
        try_build_request_body(client, &url, openai_messages, openai_tools, extra_body)?;
    let inline_think_in_text = client.config.inline_think_in_text;
    let idle_timeout = client.stream_options.idle_timeout;
    let ttft_timeout = client.stream_options.ttft_timeout;

    // Qoder's CN gateway rejects plain-Bearer requests (ALB 503); every
    // inference request must be signed by the embedded wasm
    // (`prepareInferRequest`), which returns a rewritten URL, COSY signature
    // headers, and an encrypted body. The response is a gateway envelope
    // (`data:{"body":"<OpenAI chunk>"}`) that `handle_qoder_stream` unwraps.
    if is_qoder_gateway(&url) {
        return send_qoder_signed_stream(
            client,
            url,
            request_body,
            max_tries,
            ttft_timeout,
            trace,
            inline_think_in_text,
            idle_timeout,
        )
        .await;
    }

    let header_url = url.clone();
    execute_sse_request(
        "OpenAI Streaming API",
        &url,
        &request_body,
        max_tries,
        ttft_timeout,
        trace,
        move || {
            let mut builder = common::apply_headers(client, client.client.post(&header_url));
            if header_url.contains("copilot.tencent.com") {
                for (name, value) in codebuddy_fingerprint_headers(request_context.as_ref()) {
                    builder = builder.header(name, value);
                }
                let req_id = generate_hex32();
                builder = builder.header("X-Request-ID", req_id.clone());
                builder = builder.header("X-Conversation-Message-ID", req_id);
            }
            builder
        },
        move |response, tx, tx_raw, remaining_ttft_timeout| {
            handle_openai_stream(
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

/// True when the request URL targets the Qoder CN or international gateway,
/// which requires wasm-signed inference requests.
fn is_qoder_gateway(url: &str) -> bool {
    url.contains("gateway.qoder.com.cn") || url.contains("api2-v2.qoder.sh")
}

/// Sends a Qoder inference request through the wasm-signed channel.
#[cfg(feature = "subscription-auth")]
#[allow(clippy::too_many_arguments)]
async fn send_qoder_signed_stream(
    client: &AIClient,
    _url: String,
    request_body: serde_json::Value,
    max_tries: usize,
    ttft_timeout: Option<Duration>,
    trace: Option<ModelExchangeTraceConfig>,
    inline_think_in_text: bool,
    idle_timeout: Option<Duration>,
) -> Result<StreamResponse> {
    let options = crate::subscription_auth::SubscriptionHttpOptions::default();
    let model_key = &client.config.model;
    let (signed_url, signed_headers, signed_body) =
        crate::subscription_auth::sign_qoder_infer_request(&options, &request_body, model_key)
            .await?;
    debug!("Qoder signed infer url: {}", signed_url);

    let url = signed_url;
    let trace_url = url.clone();
    execute_sse_request_with_raw_body(
        "Qoder Streaming API",
        &trace_url,
        &request_body,
        Some(signed_body),
        max_tries,
        ttft_timeout,
        trace,
        move || {
            let mut builder = client.client.post(&url);
            for (name, value) in &signed_headers {
                builder = builder.header(name, value);
            }
            builder
        },
        move |response, tx, tx_raw, remaining_ttft_timeout| {
            handle_qoder_stream(
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

#[cfg(not(feature = "subscription-auth"))]
#[allow(clippy::too_many_arguments)]
async fn send_qoder_signed_stream(
    _client: &AIClient,
    _url: String,
    _request_body: serde_json::Value,
    _max_tries: usize,
    _ttft_timeout: Option<Duration>,
    _trace: Option<ModelExchangeTraceConfig>,
    _inline_think_in_text: bool,
    _idle_timeout: Option<Duration>,
) -> Result<StreamResponse> {
    Err(anyhow!(
        "Qoder inference requires the subscription-auth feature"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_hex32_produces_32_char_hex() {
        let id = generate_hex32();
        assert_eq!(id.len(), 32, "hex32 must be exactly 32 characters");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "hex32 must contain only hex digits, got: {id}"
        );
    }

    #[test]
    fn generate_hex32_unique_per_call() {
        let a = generate_hex32();
        let b = generate_hex32();
        assert_ne!(a, b, "consecutive hex32 calls must produce distinct values");
    }

    #[test]
    fn codebuddy_url_detection() {
        assert!("https://copilot.tencent.com/v1/chat/completions".contains("copilot.tencent.com"));
        assert!(!"https://api.openai.com/v1/chat/completions".contains("copilot.tencent.com"));
        assert!(!"https://gateway.qoder.com.cn/v1".contains("copilot.tencent.com"));
    }

    fn ctx_with_ids() -> ModelRequestContext {
        ModelRequestContext {
            prompt_cache_route_key: Some("route-1".to_string()),
            session_id: Some("sess-abc".to_string()),
            conversation_request_id: Some("turn-xyz".to_string()),
        }
    }

    #[test]
    fn codebuddy_fingerprint_headers_conversation_request_id_is_turn_stable() {
        let ctx = ctx_with_ids();
        let first = codebuddy_fingerprint_headers(Some(&ctx));
        let second = codebuddy_fingerprint_headers(Some(&ctx));
        let get = |headers: &[(&'static str, String)], name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        // Same turn -> identical X-Conversation-Request-ID across requests.
        assert_eq!(
            get(&first, "X-Conversation-Request-ID"),
            get(&second, "X-Conversation-Request-ID")
        );
        assert_eq!(get(&first, "X-Conversation-Request-ID"), "turn-xyz");
        // Different turns -> different values.
        let other = ModelRequestContext {
            conversation_request_id: Some("turn-other".to_string()),
            ..ctx
        };
        assert_ne!(
            get(&first, "X-Conversation-Request-ID"),
            get(
                &codebuddy_fingerprint_headers(Some(&other)),
                "X-Conversation-Request-ID"
            )
        );
    }

    #[test]
    fn codebuddy_fingerprint_headers_includes_full_official_set() {
        let headers = codebuddy_fingerprint_headers(Some(&ctx_with_ids()));
        let names: Vec<&str> = headers.iter().map(|(n, _)| *n).collect();
        for expected in [
            "X-Conversation-ID",
            "X-Conversation-Request-ID",
            "X-Agent-Intent",
            "X-Agent-Purpose",
            "X-Product",
            "X-IDE-Type",
            "X-IDE-Name",
            "X-IDE-Version",
            "X-Product-Version",
            "X-Requested-With",
        ] {
            assert!(
                names.contains(&expected),
                "missing fingerprint header: {expected}"
            );
        }
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(get("X-Conversation-ID"), "sess-abc");
        assert_eq!(get("X-Agent-Intent"), "craft");
        assert_eq!(get("X-Agent-Purpose"), "conversation");
        assert_eq!(get("X-Product"), "SaaS");
        assert_eq!(get("X-IDE-Type"), "CLI");
        assert_eq!(get("X-IDE-Name"), "CLI");
        // Official CodeBuddy CLI version (package.json of
        // @tencent-ai/codebuddy-code 2.137.1), NOT BitFun's own version.
        assert_eq!(get("X-IDE-Version"), "2.137.1");
        assert_eq!(get("X-Product-Version"), "2.137.1");
        assert_eq!(get("X-Requested-With"), "XMLHttpRequest");
        // No duplicate header names.
        let mut sorted = names.clone();
        sorted.sort_unstable();
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(sorted, deduped, "header names must not repeat");
    }

    #[test]
    fn codebuddy_fingerprint_headers_falls_back_when_context_missing() {
        let headers = codebuddy_fingerprint_headers(None);
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        // Request-ID fallback still produced; fingerprint headers still present.
        assert_eq!(get("X-Conversation-Request-ID").len(), 32);
        assert!(headers.iter().all(|(n, _)| *n != "X-Conversation-ID"));
        assert_eq!(get("X-Agent-Intent"), "craft");
    }
}
