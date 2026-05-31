use crate::client::utils::{
    build_request_body_subset, is_trim_custom_request_body_mode, merge_json_value,
};
use crate::client::AIClient;
use crate::types::ReasoningMode;
use reqwest::RequestBuilder;
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;

pub(crate) fn apply_header_policy<F>(
    client: &AIClient,
    builder: RequestBuilder,
    apply_defaults: F,
) -> RequestBuilder
where
    F: FnOnce(RequestBuilder) -> RequestBuilder,
{
    let has_custom_headers = client
        .config
        .custom_headers
        .as_ref()
        .is_some_and(|headers| !headers.is_empty());
    let is_merge_mode = client.config.custom_headers_mode.as_deref() != Some("replace");

    if has_custom_headers && !is_merge_mode {
        return apply_custom_headers(client, builder);
    }

    let mut builder = apply_defaults(builder);

    if has_custom_headers && is_merge_mode {
        builder = apply_custom_headers(client, builder);
    }

    builder
}

pub(crate) fn apply_custom_headers(
    client: &AIClient,
    mut builder: RequestBuilder,
) -> RequestBuilder {
    if let Some(custom_headers) = &client.config.custom_headers {
        if !custom_headers.is_empty() {
            for (key, value) in custom_headers {
                builder = builder.header(key.as_str(), value.as_str());
            }
        }
    }

    builder
}

pub(crate) fn protect_request_body(
    client: &AIClient,
    request_body: &mut serde_json::Value,
    top_level_keys: &[&str],
    nested_fields: &[(&str, &str)],
) -> Option<serde_json::Value> {
    let protected_body = is_trim_custom_request_body_mode(&client.config)
        .then(|| build_request_body_subset(request_body, top_level_keys, nested_fields));

    if let Some(protected_body) = &protected_body {
        *request_body = protected_body.clone();
    }

    protected_body
}

pub(crate) fn restore_protected_body(
    request_body: &mut serde_json::Value,
    protected_body: Option<serde_json::Value>,
) {
    if let Some(protected_body) = protected_body {
        merge_json_value(request_body, protected_body);
    }
}

pub(crate) fn merge_extra_body(
    request_body: &mut serde_json::Value,
    extra_obj: &serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in extra_obj {
        request_body[key] = value.clone();
    }
}

pub(crate) fn merge_extra_body_recursively(
    request_body: &mut serde_json::Value,
    extra_obj: serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in extra_obj {
        if let Some(request_obj) = request_body.as_object_mut() {
            let target = request_obj.entry(key).or_insert(serde_json::Value::Null);
            merge_json_value(target, value);
        }
    }
}

pub(crate) fn log_extra_body_keys(
    target: &str,
    extra_obj: &serde_json::Map<String, serde_json::Value>,
) {
    log::debug!(
        target: target,
        "Applied extra_body overrides: {:?}",
        extra_obj.keys().collect::<Vec<_>>()
    );
}

pub(crate) fn summarize_request_body_for_log(
    request_body: &serde_json::Value,
) -> serde_json::Value {
    let mut summary = serde_json::Map::new();

    if let Some(model) = request_body
        .get("model")
        .and_then(serde_json::Value::as_str)
    {
        summary.insert(
            "model".to_string(),
            serde_json::Value::String(model.to_string()),
        );
    }
    if let Some(stream) = request_body
        .get("stream")
        .and_then(serde_json::Value::as_bool)
    {
        summary.insert("stream".to_string(), serde_json::Value::Bool(stream));
    }
    if let Some(max_tokens) = request_body
        .get("max_tokens")
        .and_then(|value| value.as_u64())
    {
        summary.insert(
            "max_tokens".to_string(),
            serde_json::Value::Number(max_tokens.into()),
        );
    }
    if let Some(tool_stream) = request_body
        .get("tool_stream")
        .and_then(serde_json::Value::as_bool)
    {
        summary.insert(
            "tool_stream".to_string(),
            serde_json::Value::Bool(tool_stream),
        );
    }
    if let Some(system) = request_body
        .get("system")
        .and_then(serde_json::Value::as_str)
    {
        summary.insert(
            "system_chars".to_string(),
            serde_json::Value::Number((system.chars().count() as u64).into()),
        );
    }
    if let Some(messages) = request_body
        .get("messages")
        .and_then(serde_json::Value::as_array)
    {
        summary.insert(
            "message_count".to_string(),
            serde_json::Value::Number((messages.len() as u64).into()),
        );
        summary.insert(
            "messages".to_string(),
            serde_json::Value::Array(messages.iter().map(summarize_message_for_log).collect()),
        );
    }
    if let Some(tools) = request_body
        .get("tools")
        .and_then(serde_json::Value::as_array)
    {
        summary.insert(
            "tool_count".to_string(),
            serde_json::Value::Number((tools.len() as u64).into()),
        );
    }
    if let Some(object) = request_body.as_object() {
        let mut top_level_keys = object.keys().cloned().collect::<Vec<_>>();
        top_level_keys.sort();
        summary.insert(
            "top_level_keys".to_string(),
            serde_json::Value::Array(
                top_level_keys
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    serde_json::Value::Object(summary)
}

fn summarize_message_for_log(message: &serde_json::Value) -> serde_json::Value {
    let mut summary = serde_json::Map::new();
    let content = message.get("content");

    if let Some(role) = message.get("role").and_then(serde_json::Value::as_str) {
        summary.insert(
            "role".to_string(),
            serde_json::Value::String(role.to_string()),
        );
    }
    if let Some(content) = content {
        summary.insert(
            "content_chars".to_string(),
            serde_json::Value::Number((content_text_chars(content) as u64).into()),
        );
        if let Some(items) = content.as_array() {
            summary.insert(
                "content_items".to_string(),
                serde_json::Value::Number((items.len() as u64).into()),
            );
            let mut content_types = items
                .iter()
                .filter_map(|item| item.get("type").and_then(serde_json::Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>();
            content_types.sort();
            content_types.dedup();
            if !content_types.is_empty() {
                summary.insert(
                    "content_types".to_string(),
                    serde_json::Value::Array(
                        content_types
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
        }
    }

    serde_json::Value::Object(summary)
}

fn content_text_chars(content: &serde_json::Value) -> usize {
    if let Some(text) = content.as_str() {
        return text.chars().count();
    }

    content
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
                .map(|text| text.chars().count())
                .sum()
        })
        .unwrap_or(0)
}

fn should_log_full_request_body(include_sensitive_diagnostics: bool) -> bool {
    include_sensitive_diagnostics
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AIRequestAuditMode {
    Disabled,
    Both,
    JsonlOnly,
    LogOnly,
}

#[derive(Debug, Clone, Serialize)]
struct AIRequestEffectiveOptionsAudit {
    event: &'static str,
    provider: String,
    model: Option<String>,
    request_url_host: Option<String>,
    reasoning_mode_config: String,
    thinking_effective: Option<String>,
    reasoning_effort_effective: Option<String>,
    max_tokens: Option<u64>,
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_budget_tokens: Option<u64>,
    top_level_keys: Vec<String>,
}

fn audit_mode_from_value(value: Option<&str>) -> AIRequestAuditMode {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("0" | "false" | "off" | "disabled") => AIRequestAuditMode::Disabled,
        Some("jsonl-only") => AIRequestAuditMode::JsonlOnly,
        Some("log-only") => AIRequestAuditMode::LogOnly,
        _ => AIRequestAuditMode::Both,
    }
}

fn audit_mode_from_env() -> AIRequestAuditMode {
    audit_mode_from_value(std::env::var("BITFUN_AI_REQUEST_AUDIT").ok().as_deref())
}

fn reasoning_mode_name(mode: ReasoningMode) -> &'static str {
    match mode {
        ReasoningMode::Default => "default",
        ReasoningMode::Enabled => "enabled",
        ReasoningMode::Disabled => "disabled",
        ReasoningMode::Adaptive => "adaptive",
    }
}

fn top_level_keys(request_body: &serde_json::Value) -> Vec<String> {
    let mut keys = request_body
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    keys.into_iter()
        .filter(|key| {
            !matches!(
                key.as_str(),
                "messages"
                    | "input"
                    | "contents"
                    | "system"
                    | "systemInstruction"
                    | "tools"
                    | "toolConfig"
            )
        })
        .collect()
}

fn request_url_host(request_url: &str) -> Option<String> {
    reqwest::Url::parse(request_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
}

fn string_at<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str()
}

fn bool_at(value: &serde_json::Value, path: &[&str]) -> Option<bool> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_bool()
}

fn u64_at(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_u64()
}

fn effective_thinking(request_body: &serde_json::Value) -> Option<String> {
    if let Some(thinking_type) = string_at(request_body, &["thinking", "type"]) {
        return Some(thinking_type.to_string());
    }

    if let Some(enable_thinking) = bool_at(request_body, &["enable_thinking"]) {
        return Some(
            if enable_thinking {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
        );
    }

    if request_body.get("reasoning").is_some() {
        return Some("reasoning".to_string());
    }

    if let Some(include_thoughts) = bool_at(
        request_body,
        &["generationConfig", "thinkingConfig", "includeThoughts"],
    ) {
        return Some(
            if include_thoughts {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
        );
    }

    if request_body
        .get("generationConfig")
        .and_then(|config| config.get("thinkingConfig"))
        .is_some()
    {
        return Some("configured".to_string());
    }

    None
}

fn effective_reasoning_effort(request_body: &serde_json::Value) -> Option<String> {
    string_at(request_body, &["reasoning_effort"])
        .or_else(|| string_at(request_body, &["reasoning", "effort"]))
        .or_else(|| string_at(request_body, &["output_config", "effort"]))
        .or_else(|| string_at(request_body, &["thinking", "effort"]))
        .map(str::to_string)
}

fn effective_max_tokens(request_body: &serde_json::Value) -> Option<u64> {
    u64_at(request_body, &["max_tokens"])
        .or_else(|| u64_at(request_body, &["max_output_tokens"]))
        .or_else(|| u64_at(request_body, &["generationConfig", "maxOutputTokens"]))
}

fn effective_thinking_budget_tokens(request_body: &serde_json::Value) -> Option<u64> {
    u64_at(request_body, &["thinking", "budget_tokens"])
        .or_else(|| {
            u64_at(
                request_body,
                &["generationConfig", "thinkingConfig", "thinkingBudget"],
            )
        })
        .or_else(|| {
            u64_at(
                request_body,
                &["generationConfig", "thinkingConfig", "thinkingBudgetTokens"],
            )
        })
}

fn build_ai_request_effective_options_audit(
    provider: &str,
    request_url: &str,
    reasoning_mode: ReasoningMode,
    request_body: &serde_json::Value,
) -> AIRequestEffectiveOptionsAudit {
    AIRequestEffectiveOptionsAudit {
        event: "ai_request_effective_options",
        provider: provider.to_string(),
        model: request_body
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        request_url_host: request_url_host(request_url),
        reasoning_mode_config: reasoning_mode_name(reasoning_mode).to_string(),
        thinking_effective: effective_thinking(request_body),
        reasoning_effort_effective: effective_reasoning_effort(request_body),
        max_tokens: effective_max_tokens(request_body),
        stream: request_body
            .get("stream")
            .and_then(serde_json::Value::as_bool),
        thinking_budget_tokens: effective_thinking_budget_tokens(request_body),
        top_level_keys: top_level_keys(request_body),
    }
}

fn default_ai_request_audit_jsonl_path() -> PathBuf {
    std::env::var_os("BITFUN_AI_REQUEST_AUDIT_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".config")
                .join("bitfun")
                .join("logs")
                .join("ai-request-audit.jsonl")
        })
}

fn append_ai_request_audit_jsonl(
    audit: &AIRequestEffectiveOptionsAudit,
    path: PathBuf,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut line = serde_json::to_string(audit)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    line.push('\n');

    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())
}

pub(crate) fn audit_ai_request_effective_options(
    target: &str,
    provider: &str,
    client: &AIClient,
    request_url: &str,
    request_body: &serde_json::Value,
) {
    let mode = audit_mode_from_env();
    if mode == AIRequestAuditMode::Disabled {
        return;
    }

    let audit = build_ai_request_effective_options_audit(
        provider,
        request_url,
        client.config.reasoning_mode,
        request_body,
    );

    if matches!(mode, AIRequestAuditMode::Both | AIRequestAuditMode::LogOnly) {
        log::info!(
            target: target,
            "AI request effective options: provider={} model={} thinking={} reasoning_effort={} max_tokens={} stream={}",
            audit.provider,
            audit.model.as_deref().unwrap_or("none"),
            audit.thinking_effective.as_deref().unwrap_or("none"),
            audit.reasoning_effort_effective.as_deref().unwrap_or("none"),
            audit
                .max_tokens
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            audit
                .stream
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        );
    }

    if matches!(
        mode,
        AIRequestAuditMode::Both | AIRequestAuditMode::JsonlOnly
    ) {
        if let Err(error) =
            append_ai_request_audit_jsonl(&audit, default_ai_request_audit_jsonl_path())
        {
            log::warn!(
                target: target,
                "Failed to append AI request audit JSONL: {}",
                error
            );
        }
    }
}

pub(crate) fn log_request_body(target: &str, label: &str, request_body: &serde_json::Value) {
    if should_log_full_request_body(crate::diagnostics::include_sensitive_diagnostics()) {
        log::debug!(
            target: target,
            "{}\n{}",
            label,
            serde_json::to_string_pretty(request_body)
                .unwrap_or_else(|_| "serialization failed".to_string())
        );
        return;
    }

    let summary_label = label.trim_end_matches(':');
    log::debug!(
        target: target,
        "{} summary:\n{}",
        summary_label,
        serde_json::to_string_pretty(&summarize_request_body_for_log(request_body))
            .unwrap_or_else(|_| "serialization failed".to_string())
    );
}

pub(crate) fn log_tool_names(target: &str, tool_names: Vec<String>) {
    log::debug!(target: target, "\ntools: {:?}", tool_names);
}

pub(crate) fn extract_top_level_string_field(
    value: &serde_json::Value,
    key: &str,
) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

pub(crate) fn collect_function_declaration_names_or_object_keys(
    tool: &serde_json::Value,
) -> Vec<String> {
    if let Some(declarations) = tool
        .get("functionDeclarations")
        .and_then(serde_json::Value::as_array)
    {
        declarations
            .iter()
            .filter_map(|declaration| extract_top_level_string_field(declaration, "name"))
            .collect()
    } else {
        tool.as_object()
            .into_iter()
            .flat_map(|map| map.keys().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::should_log_full_request_body;
    use super::summarize_request_body_for_log;

    #[test]
    fn request_body_log_summary_keeps_shape_without_message_contents() {
        let request_body = serde_json::json!({
            "model": "kimi-k2.6",
            "stream": true,
            "max_tokens": 32000,
            "system": "secret system context",
            "messages": [
                { "role": "user", "content": "secret user message" },
                {
                    "role": "assistant",
                    "content": [
                        { "type": "text", "text": "secret assistant message" },
                        { "type": "tool_use", "id": "tool-1", "name": "Read" }
                    ]
                }
            ]
        });

        let summary = summarize_request_body_for_log(&request_body);
        let summary_text = serde_json::to_string(&summary).unwrap();

        assert!(!summary_text.contains("secret system context"));
        assert!(!summary_text.contains("secret user message"));
        assert!(!summary_text.contains("secret assistant message"));
        assert_eq!(summary["model"], "kimi-k2.6");
        assert_eq!(summary["stream"], true);
        assert_eq!(summary["max_tokens"], 32000);
        assert_eq!(summary["system_chars"], 21);
        assert_eq!(summary["message_count"], 2);
        assert_eq!(summary["messages"][0]["role"], "user");
        assert_eq!(summary["messages"][0]["content_chars"], 19);
        assert_eq!(summary["messages"][1]["content_items"], 2);
    }

    #[test]
    fn request_body_logging_keeps_full_payload_when_sensitive_diagnostics_are_enabled() {
        assert!(should_log_full_request_body(true));
        assert!(!should_log_full_request_body(false));
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    use crate::types::ReasoningMode;
    use serde_json::json;

    #[test]
    fn audit_summary_extracts_openai_qwen_reasoning_options() {
        let request_body = json!({
            "model": "qwen3.7-max",
            "messages": [
                {"role": "system", "content": "secret system prompt"},
                {"role": "user", "content": "secret user prompt"}
            ],
            "max_tokens": 8192,
            "reasoning_effort": "max",
            "stream": true,
            "thinking": {"type": "enabled"}
        });

        let summary = build_ai_request_effective_options_audit(
            "openai",
            "https://api.openbitfun.com/v1/chat/completions",
            ReasoningMode::Enabled,
            &request_body,
        );

        assert_eq!(summary.event, "ai_request_effective_options");
        assert_eq!(summary.provider, "openai");
        assert_eq!(summary.model.as_deref(), Some("qwen3.7-max"));
        assert_eq!(
            summary.request_url_host.as_deref(),
            Some("api.openbitfun.com")
        );
        assert_eq!(summary.reasoning_mode_config, "enabled");
        assert_eq!(summary.thinking_effective.as_deref(), Some("enabled"));
        assert_eq!(summary.reasoning_effort_effective.as_deref(), Some("max"));
        assert_eq!(summary.max_tokens, Some(8192));
        assert_eq!(summary.stream, Some(true));
        assert!(summary.top_level_keys.contains(&"thinking".to_string()));
        assert!(summary
            .top_level_keys
            .contains(&"reasoning_effort".to_string()));

        let serialized = serde_json::to_string(&summary).expect("summary serializes");
        assert!(!serialized.contains("secret system prompt"));
        assert!(!serialized.contains("secret user prompt"));
        assert!(!serialized.contains("\"messages\""));
    }

    #[test]
    fn audit_summary_extracts_enable_thinking_boolean() {
        let request_body = json!({
            "model": "Qwen/Qwen3-Coder-480B-A35B-Instruct",
            "messages": [],
            "enable_thinking": true,
            "stream": true
        });

        let summary = build_ai_request_effective_options_audit(
            "openai",
            "https://api.siliconflow.cn/v1/chat/completions",
            ReasoningMode::Enabled,
            &request_body,
        );

        assert_eq!(summary.thinking_effective.as_deref(), Some("enabled"));
        assert_eq!(summary.reasoning_effort_effective, None);
    }

    #[test]
    fn audit_summary_extracts_responses_reasoning_effort() {
        let request_body = json!({
            "model": "gpt-5.1",
            "input": [{"role": "user", "content": "secret input"}],
            "max_output_tokens": 4096,
            "reasoning": {"effort": "high"},
            "stream": true
        });

        let summary = build_ai_request_effective_options_audit(
            "responses",
            "https://api.openai.com/v1/responses",
            ReasoningMode::Adaptive,
            &request_body,
        );

        assert_eq!(summary.provider, "responses");
        assert_eq!(summary.thinking_effective.as_deref(), Some("reasoning"));
        assert_eq!(summary.reasoning_effort_effective.as_deref(), Some("high"));
        assert_eq!(summary.max_tokens, Some(4096));

        let serialized = serde_json::to_string(&summary).expect("summary serializes");
        assert!(!serialized.contains("secret input"));
        assert!(!serialized.contains("\"input\""));
    }

    #[test]
    fn audit_summary_extracts_anthropic_and_deepseek_output_config() {
        let request_body = json!({
            "model": "deepseek-v4-pro",
            "messages": [{"role": "user", "content": "secret"}],
            "max_tokens": 6144,
            "stream": true,
            "thinking": {"type": "enabled", "budget_tokens": 6144},
            "output_config": {"effort": "max"}
        });

        let summary = build_ai_request_effective_options_audit(
            "anthropic",
            "https://api.deepseek.com/anthropic/v1/messages",
            ReasoningMode::Enabled,
            &request_body,
        );

        assert_eq!(summary.provider, "anthropic");
        assert_eq!(summary.thinking_effective.as_deref(), Some("enabled"));
        assert_eq!(summary.reasoning_effort_effective.as_deref(), Some("max"));
        assert_eq!(summary.max_tokens, Some(6144));
        assert_eq!(summary.thinking_budget_tokens, Some(6144));
    }

    #[test]
    fn audit_summary_extracts_gemini_thinking_config() {
        let request_body = json!({
            "contents": [{"role": "user", "parts": [{"text": "secret"}]}],
            "generationConfig": {
                "maxOutputTokens": 8192,
                "thinkingConfig": {
                    "includeThoughts": true,
                    "thinkingBudget": 2048
                }
            }
        });

        let summary = build_ai_request_effective_options_audit(
            "gemini",
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse",
            ReasoningMode::Enabled,
            &request_body,
        );

        assert_eq!(summary.provider, "gemini");
        assert_eq!(summary.thinking_effective.as_deref(), Some("enabled"));
        assert_eq!(summary.reasoning_effort_effective, None);
        assert_eq!(summary.max_tokens, Some(8192));
        assert_eq!(summary.thinking_budget_tokens, Some(2048));

        let serialized = serde_json::to_string(&summary).expect("summary serializes");
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("\"contents\""));
    }

    #[test]
    fn audit_mode_defaults_to_both_and_honors_overrides() {
        assert_eq!(audit_mode_from_value(None), AIRequestAuditMode::Both);
        assert_eq!(
            audit_mode_from_value(Some("0")),
            AIRequestAuditMode::Disabled
        );
        assert_eq!(
            audit_mode_from_value(Some("false")),
            AIRequestAuditMode::Disabled
        );
        assert_eq!(
            audit_mode_from_value(Some("off")),
            AIRequestAuditMode::Disabled
        );
        assert_eq!(
            audit_mode_from_value(Some("disabled")),
            AIRequestAuditMode::Disabled
        );
        assert_eq!(
            audit_mode_from_value(Some("jsonl-only")),
            AIRequestAuditMode::JsonlOnly
        );
        assert_eq!(
            audit_mode_from_value(Some("log-only")),
            AIRequestAuditMode::LogOnly
        );
        assert_eq!(
            audit_mode_from_value(Some("unexpected")),
            AIRequestAuditMode::Both
        );
    }

    #[test]
    fn append_ai_request_audit_jsonl_writes_single_json_line() {
        let request_body = json!({
            "model": "qwen3.7-max",
            "stream": true,
            "thinking": {"type": "enabled"},
            "reasoning_effort": "xhigh"
        });
        let audit = build_ai_request_effective_options_audit(
            "openai",
            "https://api.openbitfun.com/v1/chat/completions",
            ReasoningMode::Enabled,
            &request_body,
        );
        let path = std::env::temp_dir().join(format!(
            "bitfun-ai-request-audit-test-{}.jsonl",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        append_ai_request_audit_jsonl(&audit, path.clone()).expect("append succeeds");

        let contents = std::fs::read_to_string(&path).expect("audit file exists");
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).expect("valid json");
        assert_eq!(parsed["event"], "ai_request_effective_options");
        assert_eq!(parsed["model"], "qwen3.7-max");
        assert_eq!(parsed["reasoning_effort_effective"], "xhigh");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn append_ai_request_audit_jsonl_reports_write_errors() {
        let request_body = json!({"model": "qwen3.7-max"});
        let audit = build_ai_request_effective_options_audit(
            "openai",
            "https://api.openbitfun.com/v1/chat/completions",
            ReasoningMode::Enabled,
            &request_body,
        );
        let directory_path = std::env::temp_dir();

        let error = append_ai_request_audit_jsonl(&audit, directory_path)
            .expect_err("opening a directory as file fails");
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::IsADirectory
                | std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::Other
        ));
    }
}
