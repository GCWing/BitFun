use crate::client::utils::{
    build_request_body_subset, is_trim_custom_request_body_mode, merge_json_value,
};
use crate::client::AIClient;
use crate::types::{ReasoningPresetAction, ReasoningPresetDescriptor};
use anyhow::{anyhow, Result};
use reqwest::RequestBuilder;

/// Internal execution identity for best-effort reasoning controls offered when
/// neither models.dev nor a model-specific adapter projection has a preset.
/// The field is host-only and is never serialized to Web or remote clients.
pub(crate) const GENERIC_REASONING_PROVIDER_ID: &str = "openbitfun-generic";

pub(crate) fn is_generic_reasoning_preset(preset: &ReasoningPresetDescriptor) -> bool {
    preset.execution_provider.as_deref() == Some(GENERIC_REASONING_PROVIDER_ID)
}

pub(crate) fn normalize_generic_reasoning_effort(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        _ => None,
    }
}

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

pub(crate) fn capture_reasoning_fields(
    request_body: &serde_json::Value,
    top_level_keys: &[&str],
    nested_fields: &[(&str, &str)],
) -> Option<serde_json::Value> {
    Some(build_request_body_subset(
        request_body,
        top_level_keys,
        nested_fields,
    ))
}

pub(crate) fn reset_reasoning_fields(
    request_body: &mut serde_json::Value,
    captured_reasoning_fields: Option<&serde_json::Value>,
    reasoning_top_level_keys: &[&str],
    reasoning_nested_fields: &[(&str, &str)],
) {
    if let Some(request_object) = request_body.as_object_mut() {
        for key in reasoning_top_level_keys {
            request_object.remove(*key);
        }
        for (parent, child) in reasoning_nested_fields {
            if let Some(parent_object) = request_object
                .get_mut(*parent)
                .and_then(serde_json::Value::as_object_mut)
            {
                parent_object.remove(*child);
            }
        }
    }
    if let Some(captured_reasoning_fields) = captured_reasoning_fields {
        merge_json_value(request_body, captured_reasoning_fields.clone());
    }
}

pub(crate) fn apply_reasoning_request_patch(
    action: &ReasoningPresetAction,
    request_body: &mut serde_json::Value,
    protected_top_level_keys: &[&str],
    protected_nested_fields: &[(&str, &str)],
) -> Result<()> {
    let ReasoningPresetAction::RequestPatch { body } = action else {
        return Ok(());
    };
    let mut patch = body.clone();
    let Some(patch_object) = patch.as_object_mut() else {
        return Err(anyhow!(
            "Reasoning preset request_patch body must be a JSON object"
        ));
    };
    let protected_body = build_request_body_subset(
        request_body,
        protected_top_level_keys,
        protected_nested_fields,
    );
    for key in protected_top_level_keys {
        patch_object.remove(*key);
    }
    for (parent, child) in protected_nested_fields {
        if let Some(parent_object) = patch_object
            .get_mut(*parent)
            .and_then(serde_json::Value::as_object_mut)
        {
            parent_object.remove(*child);
        }
    }
    apply_json_merge_patch(request_body, &patch);
    // A merge patch can replace or delete a protected field's parent with
    // null, a scalar, or an array. Restore the captured paths after every
    // patch so nested runtime fields survive those parent-level changes.
    merge_json_value(request_body, protected_body);
    Ok(())
}

pub(crate) fn apply_reasoning_actions<F>(
    preset: &ReasoningPresetDescriptor,
    request_body: &mut serde_json::Value,
    protected_top_level_keys: &[&str],
    protected_nested_fields: &[(&str, &str)],
    mut compile_typed_action: F,
) -> Result<()>
where
    F: FnMut(&ReasoningPresetAction, &mut serde_json::Value) -> Result<bool>,
{
    for action in &preset.actions {
        if matches!(action, ReasoningPresetAction::RequestPatch { .. }) {
            apply_reasoning_request_patch(
                action,
                request_body,
                protected_top_level_keys,
                protected_nested_fields,
            )?;
        } else if !compile_typed_action(action, request_body)? {
            return Err(anyhow!(
                "Reasoning preset '{}' contains an action unsupported by this provider: {:?}",
                preset.id,
                action
            ));
        }
    }
    Ok(())
}

fn apply_json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    let serde_json::Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = serde_json::json!({});
    }
    let target = target
        .as_object_mut()
        .expect("merge patch target should be an object");
    for (key, patch_value) in patch {
        if patch_value.is_null() {
            target.remove(key);
            continue;
        }
        let target_value = target.entry(key.clone()).or_insert(serde_json::Value::Null);
        apply_json_merge_patch(target_value, patch_value);
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
