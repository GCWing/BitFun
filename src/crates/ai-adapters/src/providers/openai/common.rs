use crate::client::quirks::apply_openai_compatible_reasoning_fields;
use crate::client::utils::{dedupe_remote_models, normalize_base_url_for_discovery};
use crate::client::AIClient;
use crate::providers::shared;
use crate::types::RemoteModelInfo;
use anyhow::Result;
use reqwest::RequestBuilder;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelEntry {
    id: String,
}

pub(crate) fn apply_headers(client: &AIClient, builder: RequestBuilder) -> RequestBuilder {
    shared::apply_header_policy(client, builder, |mut builder| {
        builder = builder
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", client.config.api_key));

        if client.config.base_url.contains("openbitfun.com") {
            builder = builder.header("X-Verification-Code", "from_bitfun");
        }

        builder
    })
}

pub(crate) fn apply_reasoning_fields(
    request_body: &mut serde_json::Value,
    client: &AIClient,
    url: &str,
) {
    apply_openai_compatible_reasoning_fields(request_body, client.config.reasoning_mode, url);
}

pub(crate) fn resolve_models_url(client: &AIClient) -> String {
    let mut base = normalize_base_url_for_discovery(&client.config.base_url);

    for suffix in ["/chat/completions", "/responses", "/models"] {
        if base.ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }

    if base.is_empty() {
        return "models".to_string();
    }

    format!("{}/models", base)
}

pub(crate) async fn list_models(client: &AIClient) -> Result<Vec<RemoteModelInfo>> {
    let url = resolve_models_url(client);

    // Codex CLI's ChatGPT backend (`chatgpt.com/backend-api/codex`) hosts a
    // private, non-OpenAI-shaped `/models` endpoint that returns
    // `{ "models": [{ "slug": "...", "display_name": "..." }, ...] }`. Detect
    // and route it through a dedicated parser instead of the public OpenAI
    // schema (which would yield zero models because of the envelope mismatch).
    if url.contains("chatgpt.com/backend-api/codex") {
        return list_codex_chatgpt_models(client, &url).await;
    }

    let response = apply_headers(client, client.client.get(&url))
        .send()
        .await?
        .error_for_status()?;

    let payload: OpenAIModelsResponse = response.json().await?;
    Ok(dedupe_remote_models(
        payload
            .data
            .into_iter()
            .map(|model| RemoteModelInfo {
                id: model.id,
                display_name: None,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct CodexBackendModelsResponse {
    #[serde(default)]
    models: Vec<CodexBackendModelEntry>,
}

#[derive(Debug, Deserialize)]
struct CodexBackendModelEntry {
    slug: String,
    /// Returned by the backend but unused — see comment in the mapping below
    /// (display_name is dropped to avoid duplicate-looking entries).
    #[allow(dead_code)]
    #[serde(default)]
    display_name: Option<String>,
    /// Codex backend marks deprecated/internal slugs with `visibility = "hide"`.
    /// We only surface entries the CLI itself shows (`list`).
    #[serde(default)]
    visibility: Option<String>,
}

/// `chatgpt.com/backend-api/codex/models` returns each model's
/// `minimal_client_version`, and only emits entries whose minimum is satisfied
/// by the `client_version` query param. Codex CLI credentials inject a
/// Codex-shaped `User-Agent` containing the locally installed CLI version; use
/// that same version here so the model picker matches what the user sees in
/// `codex /model`.
fn codex_client_version(client: &AIClient) -> Option<String> {
    let headers = client.config.custom_headers.as_ref()?;
    let user_agent = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("User-Agent"))?
        .1
        .trim();
    let version = user_agent
        .strip_prefix("codex_cli_rs/")
        .or_else(|| user_agent.strip_prefix("codex/"))?
        .trim();

    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

async fn list_codex_chatgpt_models(
    client: &AIClient,
    base_models_url: &str,
) -> Result<Vec<RemoteModelInfo>> {
    let url = if let Some(version) = codex_client_version(client) {
        let separator = if base_models_url.contains('?') {
            '&'
        } else {
            '?'
        };
        format!("{base_models_url}{separator}client_version={version}")
    } else {
        log::warn!(
            "Codex backend model discovery is missing a codex CLI client version; requesting models without client_version"
        );
        base_models_url.to_string()
    };

    let response = apply_headers(client, client.client.get(&url))
        .send()
        .await?
        .error_for_status()?;

    let payload: CodexBackendModelsResponse = response.json().await?;

    let filtered: Vec<RemoteModelInfo> = payload
        .models
        .into_iter()
        .filter(|model| {
            model
                .visibility
                .as_deref()
                .map(|v| v.eq_ignore_ascii_case("list"))
                .unwrap_or(true)
        })
        .map(|model| RemoteModelInfo {
            id: model.slug,
            // Codex backend's `display_name` is often the same slug with
            // different casing (e.g. `gpt-5.4-mini` vs `GPT-5.4-Mini`). The
            // BitFun model picker renders display_name + slug stacked, which
            // looks like duplicate names. Drop display_name so each entry is a
            // single line keyed only by the canonical slug.
            display_name: None,
        })
        .collect();

    Ok(dedupe_remote_models(filtered))
}

pub(crate) fn extract_tool_name(tool: &serde_json::Value) -> String {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(|name| name.as_str())
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) fn attach_tools(
    request_body: &mut serde_json::Value,
    tools: Option<Vec<serde_json::Value>>,
    target: &str,
) {
    if let Some(tools) = tools {
        let tool_names = tools.iter().map(extract_tool_name).collect::<Vec<_>>();
        shared::log_tool_names(target, tool_names);
        if !tools.is_empty() {
            request_body["tools"] = serde_json::Value::Array(tools);
            let has_tool_choice = request_body
                .get("tool_choice")
                .is_some_and(|value| !value.is_null());
            if !has_tool_choice {
                request_body["tool_choice"] = serde_json::Value::String("auto".to_string());
            }
        }
    }
}
