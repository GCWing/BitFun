//! Google Cloud Code Assist transport (`cloudcode-pa.googleapis.com`).
//!
//! Used by `gemini-cli` after a personal Google login. The endpoint accepts the
//! regular Gemini request body but wrapped in
//! `{ "model": "...", "project": "...", "request": { ... } }` and authenticated
//! with a Bearer access_token (we don't pass `x-goog-api-key`).

use super::{request as gemini_request, GeminiMessageConverter};
use crate::client::sse::execute_sse_request;
use crate::client::{AIClient, StreamResponse};
use crate::providers::shared;
use crate::stream::handle_gemini_stream;
use crate::types::{Message, RemoteModelInfo, ToolDefinition};
use anyhow::{anyhow, Result};
use log::{debug, warn};
use reqwest::RequestBuilder;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::sync::Mutex;

const CODE_ASSIST_BASE: &str = "https://cloudcode-pa.googleapis.com";
const STREAM_ENDPOINT: &str = "/v1internal:streamGenerateContent?alt=sse";
const LOAD_CODE_ASSIST_ENDPOINT: &str = "/v1internal:loadCodeAssist";
const ONBOARD_USER_ENDPOINT: &str = "/v1internal:onboardUser";

fn cached_project() -> &'static Mutex<Option<String>> {
    static CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn apply_headers(client: &AIClient, builder: RequestBuilder) -> RequestBuilder {
    shared::apply_header_policy(client, builder, |builder| {
        builder
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", client.config.api_key))
            .header("User-Agent", "BitFun-CodeAssist/1.0")
    })
}

#[derive(Debug, Deserialize)]
struct LoadCodeAssistResponse {
    #[serde(default, rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OnboardOperation {
    #[serde(default)]
    done: Option<bool>,
    #[serde(default)]
    response: Option<OnboardResponse>,
}

#[derive(Debug, Deserialize)]
struct OnboardResponse {
    #[serde(default, rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<OnboardProject>,
}

#[derive(Debug, Deserialize)]
struct OnboardProject {
    #[serde(default)]
    id: Option<String>,
}

async fn discover_project(client: &AIClient) -> Result<String> {
    {
        let guard = cached_project().lock().await;
        if let Some(p) = guard.clone() {
            return Ok(p);
        }
    }

    if let Ok(env_project) = std::env::var("GOOGLE_CLOUD_PROJECT") {
        if !env_project.is_empty() {
            *cached_project().lock().await = Some(env_project.clone());
            return Ok(env_project);
        }
    }

    let metadata = serde_json::json!({
        "ideType": "IDE_UNSPECIFIED",
        "platform": "PLATFORM_UNSPECIFIED",
        "pluginType": "GEMINI",
    });

    let load_url = format!("{}{}", CODE_ASSIST_BASE, LOAD_CODE_ASSIST_ENDPOINT);
    let load_body = serde_json::json!({ "metadata": metadata });
    let load_resp = apply_headers(client, client.client.post(&load_url))
        .json(&load_body)
        .send()
        .await?;
    let load_status = load_resp.status();
    if !load_status.is_success() {
        let body = load_resp.text().await.unwrap_or_default();
        return Err(anyhow!("loadCodeAssist failed: HTTP {load_status}: {body}"));
    }
    let load_parsed: LoadCodeAssistResponse = load_resp.json().await?;
    if let Some(project) = load_parsed
        .cloudaicompanion_project
        .filter(|s| !s.is_empty())
    {
        *cached_project().lock().await = Some(project.clone());
        return Ok(project);
    }

    // Need to onboard – create a free-tier Code Assist project.
    let onboard_url = format!("{}{}", CODE_ASSIST_BASE, ONBOARD_USER_ENDPOINT);
    let onboard_body = serde_json::json!({
        "tierId": "free-tier",
        "metadata": metadata,
    });
    let onboard_resp = apply_headers(client, client.client.post(&onboard_url))
        .json(&onboard_body)
        .send()
        .await?;
    let onboard_status = onboard_resp.status();
    if !onboard_status.is_success() {
        let body = onboard_resp.text().await.unwrap_or_default();
        return Err(anyhow!("onboardUser failed: HTTP {onboard_status}: {body}"));
    }
    let parsed: OnboardOperation = onboard_resp.json().await?;
    if !parsed.done.unwrap_or(false) {
        return Err(anyhow!("onboardUser did not complete in a single call"));
    }
    let project = parsed
        .response
        .and_then(|r| r.cloudaicompanion_project)
        .and_then(|p| p.id)
        .ok_or_else(|| anyhow!("onboardUser response missing project id"))?;
    *cached_project().lock().await = Some(project.clone());
    Ok(project)
}

pub(crate) async fn send_stream(
    client: &AIClient,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    extra_body: Option<serde_json::Value>,
    max_tries: usize,
) -> Result<StreamResponse> {
    let project = discover_project(client).await?;

    let (system_instruction, contents) =
        GeminiMessageConverter::convert_messages(messages, &client.config.model);
    let gemini_tools = GeminiMessageConverter::convert_tools(tools);
    let inner = gemini_request::build_request_body(
        client,
        system_instruction,
        contents,
        gemini_tools,
        extra_body,
    );

    let request_body = serde_json::json!({
        "model": client.config.model,
        "project": project,
        "request": inner,
    });

    let url = if client.config.request_url.is_empty() {
        format!("{}{}", CODE_ASSIST_BASE, STREAM_ENDPOINT)
    } else {
        client.config.request_url.clone()
    };

    debug!(
        "Gemini Code Assist config: model={}, request_url={}, project={}, max_tries={}",
        client.config.model, url, project, max_tries
    );

    let idle_timeout = client.stream_options.idle_timeout;
    execute_sse_request(
        "Gemini Code Assist Streaming API",
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

const DEFAULT_CODE_ASSIST_MODELS: &[(&str, &str)] = &[
    ("gemini-3.1-pro-preview", "Gemini 3.1 Pro"),
    ("gemini-3-pro-preview", "Gemini 3 Pro"),
    ("gemini-3-flash-preview", "Gemini 3 Flash"),
    ("gemini-3.1-flash-lite-preview", "Gemini 3.1 Flash Lite"),
    ("gemini-2.5-pro", "Gemini 2.5 Pro"),
    ("gemini-2.5-flash", "Gemini 2.5 Flash"),
    ("gemini-2.5-flash-lite", "Gemini 2.5 Flash-Lite"),
];

fn gemini_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".gemini"))
}

fn read_gemini_settings_model(gemini_home: &Path) -> Option<String> {
    let settings_path = gemini_home.join("settings.json");
    let bytes = match std::fs::read(&settings_path) {
        Ok(b) => b,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Failed to read Gemini settings from {}: {}",
                    settings_path.display(),
                    e
                );
            }
            return None;
        }
    };
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                "Failed to parse Gemini settings JSON from {}: {}",
                settings_path.display(),
                e
            );
            return None;
        }
    };
    value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
}

fn read_gemini_env_model(gemini_home: &Path) -> Option<String> {
    let env_path = gemini_home.join(".env");
    let text = match std::fs::read_to_string(&env_path) {
        Ok(t) => t,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Failed to read Gemini .env from {}: {}",
                    env_path.display(),
                    e
                );
            }
            return None;
        }
    };
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim() != "GEMINI_MODEL" {
            return None;
        }
        let model = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
        (!model.is_empty()).then(|| model.to_string())
    })
}

/// Code Assist (`cloudcode-pa.googleapis.com`) does not expose a list-models
/// endpoint; the upstream `gemini-cli` ships a hard-coded `VALID_GEMINI_MODELS`
/// set in `packages/core/src/config/models.ts`. We mirror its stable entries and
/// preserve the user's local configured model when present.
pub(crate) async fn list_models(_client: &AIClient) -> Result<Vec<RemoteModelInfo>> {
    let mut models = Vec::new();

    if let Some(gemini_home) = gemini_home_dir() {
        if let Some(model) =
            read_gemini_settings_model(&gemini_home).or_else(|| read_gemini_env_model(&gemini_home))
        {
            models.push(RemoteModelInfo {
                id: model,
                display_name: None,
            });
        }
    }

    for (id, display_name) in DEFAULT_CODE_ASSIST_MODELS {
        models.push(RemoteModelInfo {
            id: (*id).to_string(),
            display_name: Some((*display_name).to_string()),
        });
    }

    Ok(crate::client::utils::dedupe_remote_models(models))
}
