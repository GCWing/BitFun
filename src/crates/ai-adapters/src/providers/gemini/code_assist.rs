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
use log::debug;
use reqwest::RequestBuilder;
use serde::Deserialize;
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

/// Code Assist (`cloudcode-pa.googleapis.com`) does not expose a list-models
/// endpoint; the upstream `gemini-cli` ships a hard-coded `VALID_GEMINI_MODELS`
/// set in `packages/core/src/config/models.ts`. We mirror its stable entries so
/// the BitFun model picker shows exactly what the CLI itself allows.
pub(crate) async fn list_models(_client: &AIClient) -> Result<Vec<RemoteModelInfo>> {
    Ok(vec![
        RemoteModelInfo {
            id: "gemini-2.5-pro".to_string(),
            display_name: Some("Gemini 2.5 Pro".to_string()),
        },
        RemoteModelInfo {
            id: "gemini-2.5-flash".to_string(),
            display_name: Some("Gemini 2.5 Flash".to_string()),
        },
        RemoteModelInfo {
            id: "gemini-2.5-flash-lite".to_string(),
            display_name: Some("Gemini 2.5 Flash-Lite".to_string()),
        },
    ])
}
