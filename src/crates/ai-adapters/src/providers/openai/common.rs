use crate::client::quirks::apply_openai_compatible_reasoning_fields;
use crate::client::utils::{dedupe_remote_models, normalize_base_url_for_discovery};
use crate::client::AIClient;
use crate::providers::shared;
use crate::types::RemoteModelInfo;
use anyhow::Result;
use log::warn;
use reqwest::RequestBuilder;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
    apply_openai_compatible_reasoning_fields(
        request_body,
        client.config.reasoning_mode,
        client.config.reasoning_effort.as_deref(),
        url,
        &client.config.model,
    );
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
    #[serde(default)]
    supported_in_api: Option<bool>,
    #[serde(default)]
    priority: Option<i64>,
}

const DEFAULT_CODEX_MODELS: &[&str] = &[
    "gpt-5.5",
    "gpt-5.4-mini",
    "gpt-5.4",
    "gpt-5.3-codex",
    "gpt-5.2-codex",
    "gpt-5.1-codex-max",
    "gpt-5.1-codex-mini",
];

const FORWARD_COMPAT_CODEX_MODELS: &[(&str, &[&str])] = &[
    ("gpt-5.5", &["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex"]),
    ("gpt-5.4-mini", &["gpt-5.3-codex", "gpt-5.2-codex"]),
    ("gpt-5.4", &["gpt-5.3-codex", "gpt-5.2-codex"]),
    ("gpt-5.3-codex", &["gpt-5.2-codex"]),
];

fn codex_home_dir() -> PathBuf {
    std::env::var("CODEX_HOME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn add_unique_model_id(ordered: &mut Vec<String>, id: String) {
    if !id.trim().is_empty() && !ordered.iter().any(|existing| existing == &id) {
        ordered.push(id);
    }
}

fn add_forward_compat_codex_models(ordered: &mut Vec<String>) {
    for (synthetic, templates) in FORWARD_COMPAT_CODEX_MODELS {
        if ordered.iter().any(|model| model == synthetic) {
            continue;
        }
        if templates
            .iter()
            .any(|template| ordered.iter().any(|model| model == template))
        {
            ordered.push((*synthetic).to_string());
        }
    }
}

fn read_codex_config_model(codex_home: &Path) -> Option<String> {
    let config_path = codex_home.join("config.toml");
    let text = match std::fs::read_to_string(&config_path) {
        Ok(t) => t,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Failed to read Codex config from {}: {}",
                    config_path.display(),
                    e
                );
            }
            return None;
        }
    };
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim() != "model" {
            return None;
        }
        let model = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
        (!model.is_empty()).then(|| model.to_string())
    })
}

fn read_codex_cached_models(codex_home: &Path) -> Vec<String> {
    let cache_path = codex_home.join("models_cache.json");
    let bytes = match std::fs::read(&cache_path) {
        Ok(b) => b,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Failed to read Codex models cache from {}: {}",
                    cache_path.display(),
                    e
                );
            }
            return Vec::new();
        }
    };
    let payload: CodexBackendModelsResponse = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(e) => {
            warn!(
                "Failed to parse Codex models cache JSON from {}: {}",
                cache_path.display(),
                e
            );
            return Vec::new();
        }
    };
    codex_models_from_entries(payload.models)
}

fn codex_models_from_entries(entries: Vec<CodexBackendModelEntry>) -> Vec<String> {
    let mut sortable = Vec::new();
    for model in entries {
        if model.supported_in_api == Some(false) {
            continue;
        }
        if model
            .visibility
            .as_deref()
            .map(|v| {
                let normalized = v.trim().to_ascii_lowercase();
                normalized == "hide" || normalized == "hidden"
            })
            .unwrap_or(false)
        {
            continue;
        }
        sortable.push((model.priority.unwrap_or(10_000), model.slug));
    }
    sortable.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut ordered = Vec::new();
    for (_, slug) in sortable {
        add_unique_model_id(&mut ordered, slug);
    }
    ordered
}

fn codex_fallback_model_ids() -> Vec<String> {
    let codex_home = codex_home_dir();
    let mut ordered = Vec::new();
    if let Some(model) = read_codex_config_model(&codex_home) {
        add_unique_model_id(&mut ordered, model);
    }
    for model in read_codex_cached_models(&codex_home) {
        add_unique_model_id(&mut ordered, model);
    }
    for model in DEFAULT_CODEX_MODELS {
        add_unique_model_id(&mut ordered, (*model).to_string());
    }
    add_forward_compat_codex_models(&mut ordered);
    ordered
}

fn codex_model_infos(model_ids: Vec<String>) -> Vec<RemoteModelInfo> {
    dedupe_remote_models(
        model_ids
            .into_iter()
            .map(|id| RemoteModelInfo {
                id,
                display_name: None,
            })
            .collect(),
    )
}

/// `chatgpt.com/backend-api/codex/models` returns each model's
/// `minimal_client_version`, and only emits entries whose minimum is satisfied
/// by the `client_version` query param. Hermes-agent uses `client_version=1.0.0`
/// for discovery, which avoids accidentally hiding newer models when the local
/// CLI binary is old or unavailable.
fn codex_models_url(base_models_url: &str) -> String {
    let separator = if base_models_url.contains('?') {
        '&'
    } else {
        '?'
    };
    format!("{base_models_url}{separator}client_version=1.0.0")
}

async fn list_codex_chatgpt_models(
    client: &AIClient,
    base_models_url: &str,
) -> Result<Vec<RemoteModelInfo>> {
    let url = codex_models_url(base_models_url);

    let live_models = async {
        let response = apply_headers(client, client.client.get(&url))
            .send()
            .await?
            .error_for_status()?;

        let payload: CodexBackendModelsResponse = response.json().await?;
        Ok::<Vec<String>, anyhow::Error>(codex_models_from_entries(payload.models))
    }
    .await;

    let mut model_ids = match live_models {
        Ok(models) if !models.is_empty() => models,
        Ok(_) => {
            log::warn!(
                "Codex backend model discovery returned no models; using local fallback catalog"
            );
            codex_fallback_model_ids()
        }
        Err(error) => {
            log::warn!(
                "Codex backend model discovery failed: {}; using local fallback catalog",
                error
            );
            codex_fallback_model_ids()
        }
    };

    add_forward_compat_codex_models(&mut model_ids);
    Ok(codex_model_infos(model_ids))
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
