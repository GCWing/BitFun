use crate::infrastructure::ai::client::utils::{
    build_request_body_subset, is_trim_custom_request_body_mode, merge_json_value,
};
use crate::infrastructure::ai::client::AIClient;
use reqwest::RequestBuilder;

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

pub(crate) fn log_request_body(target: &str, label: &str, request_body: &serde_json::Value) {
    log::debug!(
        target: target,
        "{}\n{}",
        label,
        serde_json::to_string_pretty(request_body)
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
