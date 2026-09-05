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
use crate::trace::ModelExchangeTraceConfig;
use crate::types::{Message, RemoteModelInfo, ToolDefinition};
use anyhow::{anyhow, Result};
use log::{debug, warn};
use openbitfun_core_types::errors::AiProviderError;
use reqwest::RequestBuilder;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::sync::Mutex;

const CODE_ASSIST_BASE: &str = "https://cloudcode-pa.googleapis.com";
const ANTIGRAVITY_DAILY_BASE: &str = "https://daily-cloudcode-pa.sandbox.googleapis.com";
const ANTIGRAVITY_AUTOPUSH_BASE: &str = "https://autopush-cloudcode-pa.sandbox.googleapis.com";
const ANTIGRAVITY_DEFAULT_PROJECT: &str = "rising-fact-p41fc";
const STREAM_ENDPOINT: &str = "/v1internal:streamGenerateContent?alt=sse";
const LOAD_CODE_ASSIST_ENDPOINT: &str = "/v1internal:loadCodeAssist";
const ONBOARD_USER_ENDPOINT: &str = "/v1internal:onboardUser";

fn cached_project() -> &'static Mutex<Option<(String, String)>> {
    static CACHE: OnceLock<Mutex<Option<(String, String)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn apply_headers(client: &AIClient, builder: RequestBuilder) -> RequestBuilder {
    let has_custom_user_agent = client
        .config
        .custom_headers
        .as_ref()
        .is_some_and(|headers| {
            headers
                .keys()
                .any(|key| key.eq_ignore_ascii_case("user-agent"))
        });
    shared::apply_header_policy(client, builder, |builder| {
        let builder = builder
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", client.config.api_key));
        if has_custom_user_agent {
            builder
        } else {
            builder.header("User-Agent", "OpenBitFun-CodeAssist/1.0")
        }
    })
}

#[derive(Debug, Deserialize)]
struct LoadCodeAssistResponse {
    #[serde(default, rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<serde_json::Value>,
    #[serde(default, rename = "allowedTiers")]
    allowed_tiers: Vec<CodeAssistTier>,
}

#[derive(Debug, Deserialize)]
struct CodeAssistTier {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "isDefault")]
    is_default: Option<bool>,
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

fn is_antigravity(client: &AIClient) -> bool {
    client
        .config
        .custom_headers
        .as_ref()
        .and_then(|headers| headers.get("Client-Metadata"))
        .is_some_and(|value| value.contains("ANTIGRAVITY"))
}

#[derive(Debug, PartialEq, Eq)]
struct AntigravityModelRoute {
    model: String,
    thinking_level: Option<String>,
    thinking_budget: Option<i64>,
}

fn configured_thinking_level(request: &serde_json::Value) -> Option<String> {
    request
        .get("generationConfig")
        .and_then(|value| value.get("thinkingConfig"))
        .and_then(|value| value.get("thinkingLevel"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn configured_thinking_budget(request: &serde_json::Value) -> Option<i64> {
    request
        .get("generationConfig")
        .and_then(|value| value.get("thinkingConfig"))
        .and_then(|value| {
            value
                .get("thinking_budget")
                .or_else(|| value.get("thinkingBudget"))
        })
        .and_then(serde_json::Value::as_i64)
        .filter(|value| *value > 0)
}

fn strip_thinking_tier(model: &str) -> (&str, Option<&str>) {
    for tier in ["minimal", "low", "medium", "high", "max"] {
        let suffix = format!("-{tier}");
        if let Some(base) = model.strip_suffix(suffix.as_str()) {
            return (base, Some(tier));
        }
    }
    (model, None)
}

/// Resolves current Antigravity wire ids and keeps old Gemini CLI preview ids
/// working for model configurations persisted by earlier OpenBitFun releases.
fn resolve_antigravity_model(
    configured_model: &str,
    request: &serde_json::Value,
) -> AntigravityModelRoute {
    let normalized = configured_model.trim().to_ascii_lowercase();
    let normalized = normalized
        .strip_prefix("antigravity-")
        .unwrap_or(&normalized)
        .to_string();
    let normalized = normalized
        .strip_suffix("-preview-customtools")
        .or_else(|| normalized.strip_suffix("-preview"))
        .unwrap_or(&normalized)
        .to_string();
    let (base, requested_tier) = strip_thinking_tier(&normalized);
    let configured_level = configured_thinking_level(request);

    if base.starts_with("gemini-3") && base.contains("-pro") && !base.contains("image") {
        let level = match requested_tier.or(configured_level.as_deref()) {
            Some("high") => "high",
            _ => "low",
        };
        return AntigravityModelRoute {
            model: format!("{base}-{level}"),
            thinking_level: Some(level.to_string()),
            thinking_budget: None,
        };
    }

    if base.starts_with("gemini-3") && base.contains("-flash") {
        let level = match requested_tier.or(configured_level.as_deref()) {
            Some(level @ ("minimal" | "low" | "medium" | "high")) => level,
            _ => "low",
        };
        return AntigravityModelRoute {
            model: base.to_string(),
            thinking_level: Some(level.to_string()),
            thinking_budget: None,
        };
    }

    if base.starts_with("claude-") && base.contains("-thinking") {
        let thinking_budget = match requested_tier {
            Some("low") => 8_192,
            Some("medium") => 16_384,
            Some("high" | "max") => 32_768,
            _ => configured_thinking_budget(request).unwrap_or(32_768),
        };
        return AntigravityModelRoute {
            model: base.to_string(),
            thinking_level: None,
            thinking_budget: Some(thinking_budget),
        };
    }

    AntigravityModelRoute {
        model: normalized,
        thinking_level: None,
        thinking_budget: None,
    }
}

fn configure_antigravity_claude_tools(request: &mut serde_json::Value) {
    let request = request
        .as_object_mut()
        .expect("Gemini request body must be an object");
    let tool_config = request
        .entry("toolConfig")
        .or_insert_with(|| serde_json::json!({}));
    if !tool_config.is_object() {
        *tool_config = serde_json::json!({});
    }
    let tool_config = tool_config
        .as_object_mut()
        .expect("toolConfig must be an object");
    let function_calling = tool_config
        .entry("functionCallingConfig")
        .or_insert_with(|| serde_json::json!({}));
    if !function_calling.is_object() {
        *function_calling = serde_json::json!({});
    }
    function_calling
        .as_object_mut()
        .expect("functionCallingConfig must be an object")
        .insert(
            "mode".to_string(),
            serde_json::Value::String("VALIDATED".to_string()),
        );

    let Some(tools) = request
        .get_mut("tools")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for declarations in tools.iter_mut().filter_map(|tool| {
        tool.get_mut("functionDeclarations")
            .and_then(serde_json::Value::as_array_mut)
    }) {
        for declaration in declarations {
            let Some(declaration) = declaration.as_object_mut() else {
                continue;
            };
            let parameters = declaration
                .entry("parameters")
                .or_insert_with(|| serde_json::json!({ "type": "object" }));
            if !parameters.is_object() {
                *parameters = serde_json::json!({ "type": "object" });
            }
            let parameters = parameters
                .as_object_mut()
                .expect("tool parameters must be an object");
            let has_properties = parameters
                .get("properties")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|properties| !properties.is_empty());
            if has_properties {
                continue;
            }
            parameters.insert("type".to_string(), serde_json::json!("object"));
            parameters.insert(
                "properties".to_string(),
                serde_json::json!({
                    "_placeholder": {
                        "type": "boolean",
                        "description": "Placeholder. Always pass true."
                    }
                }),
            );
            parameters.insert("required".to_string(), serde_json::json!(["_placeholder"]));
        }
    }
}

fn apply_antigravity_thinking(request: &mut serde_json::Value, route: &AntigravityModelRoute) {
    if route.model.starts_with("claude-") {
        configure_antigravity_claude_tools(request);
    }
    if route.thinking_level.is_none() && route.thinking_budget.is_none() {
        return;
    }
    if !request
        .get("generationConfig")
        .is_some_and(serde_json::Value::is_object)
    {
        request["generationConfig"] = serde_json::json!({});
    }
    let generation = request["generationConfig"]
        .as_object_mut()
        .expect("generationConfig must be an object");
    let thinking = generation
        .entry("thinkingConfig")
        .or_insert_with(|| serde_json::json!({}));
    if !thinking.is_object() {
        *thinking = serde_json::json!({});
    }
    let thinking = thinking
        .as_object_mut()
        .expect("thinkingConfig must be an object");

    if let Some(level) = &route.thinking_level {
        thinking.remove("thinkingBudget");
        thinking.remove("thinking_budget");
        thinking.insert("includeThoughts".to_string(), serde_json::Value::Bool(true));
        thinking.insert(
            "thinkingLevel".to_string(),
            serde_json::Value::String(level.clone()),
        );
    } else if let Some(budget) = route.thinking_budget {
        let include_thoughts = thinking
            .remove("includeThoughts")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        thinking.remove("thinkingLevel");
        thinking.remove("thinkingBudget");
        thinking.insert(
            "include_thoughts".to_string(),
            serde_json::Value::Bool(include_thoughts),
        );
        thinking.insert(
            "thinking_budget".to_string(),
            serde_json::Value::Number(budget.into()),
        );
        let max_output_tokens = generation
            .get("maxOutputTokens")
            .and_then(serde_json::Value::as_i64);
        if max_output_tokens.is_none_or(|limit| limit <= budget) {
            generation.insert(
                "maxOutputTokens".to_string(),
                serde_json::Value::Number(64_000.into()),
            );
        }
    }
}

fn antigravity_platform(client: &AIClient) -> &'static str {
    let metadata = client
        .config
        .custom_headers
        .as_ref()
        .and_then(|headers| headers.get("Client-Metadata"))
        .map(String::as_str)
        .unwrap_or_default();
    if metadata.contains("WINDOWS") {
        "WINDOWS"
    } else {
        // The Antigravity desktop client exposes only Windows/macOS
        // fingerprints; its OpenCode plugin maps Linux/headless hosts to one
        // of those supported platforms too.
        "MACOS"
    }
}

fn antigravity_metadata(platform: &str, duet_project: Option<&str>) -> serde_json::Value {
    let mut metadata = serde_json::json!({
        "ideType": "ANTIGRAVITY",
        "platform": platform,
        "pluginType": "GEMINI",
    });
    if let Some(project) = duet_project {
        metadata
            .as_object_mut()
            .expect("Antigravity metadata must be an object")
            .insert(
                "duetProject".to_string(),
                serde_json::Value::String(project.to_string()),
            );
    }
    metadata
}

fn extract_project(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| {
            value.as_str().map(str::to_string).or_else(|| {
                value
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
        })
        .filter(|project| !project.trim().is_empty())
}

fn default_tier(load: &LoadCodeAssistResponse, antigravity: bool) -> String {
    load.allowed_tiers
        .iter()
        .find(|tier| tier.is_default.unwrap_or(false))
        .or_else(|| load.allowed_tiers.first())
        .and_then(|tier| tier.id.clone())
        .filter(|tier| !tier.trim().is_empty())
        .unwrap_or_else(|| if antigravity { "FREE" } else { "free-tier" }.to_string())
}

async fn remember_project(client: &AIClient, project: String) -> String {
    *cached_project().lock().await = Some((client.config.api_key.clone(), project.clone()));
    project
}

async fn discover_project(client: &AIClient) -> Result<String> {
    {
        let guard = cached_project().lock().await;
        if let Some((credential, project)) = guard.as_ref() {
            if credential == &client.config.api_key {
                return Ok(project.clone());
            }
        }
    }

    if let Ok(env_project) = std::env::var("GOOGLE_CLOUD_PROJECT") {
        if !env_project.is_empty() {
            return Ok(remember_project(client, env_project).await);
        }
    }

    let antigravity = is_antigravity(client);
    let metadata = if antigravity {
        antigravity_metadata(antigravity_platform(client), None)
    } else {
        serde_json::json!({
            "ideType": "IDE_UNSPECIFIED",
            "platform": "PLATFORM_UNSPECIFIED",
            "pluginType": "GEMINI",
        })
    };

    // OpenCode's Antigravity adapter uses the compatibility project only for
    // discovery. Onboarding omits it unless OAuth supplied an actual project,
    // allowing Code Assist to provision the account's managed project.
    let load_metadata = if antigravity {
        antigravity_metadata(
            antigravity_platform(client),
            Some(ANTIGRAVITY_DEFAULT_PROJECT),
        )
    } else {
        metadata.clone()
    };
    let load_body = serde_json::json!({ "metadata": load_metadata });
    let load_endpoints: &[&str] = if antigravity {
        &[
            CODE_ASSIST_BASE,
            ANTIGRAVITY_DAILY_BASE,
            ANTIGRAVITY_AUTOPUSH_BASE,
        ]
    } else {
        &[CODE_ASSIST_BASE]
    };
    let mut loaded = None;
    let mut last_load_error = None;
    for endpoint in load_endpoints {
        let load_url = format!("{endpoint}{LOAD_CODE_ASSIST_ENDPOINT}");
        let mut request = apply_headers(client, client.client.post(&load_url));
        if antigravity {
            request = request.header("User-Agent", "google-api-nodejs-client/9.15.1");
        }
        match request.json(&load_body).send().await {
            Ok(response) if response.status().is_success() => {
                loaded = Some(response.json::<LoadCodeAssistResponse>().await?);
                break;
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                last_load_error = Some(format!("HTTP {status}: {body}"));
            }
            Err(error) => last_load_error = Some(error.to_string()),
        }
    }
    let Some(load_parsed) = loaded else {
        if antigravity {
            warn!(
                "Antigravity project discovery failed across all endpoints; using the compatibility project: {}",
                last_load_error.unwrap_or_else(|| "unknown error".to_string())
            );
            return Ok(remember_project(client, ANTIGRAVITY_DEFAULT_PROJECT.to_string()).await);
        }
        return Err(anyhow!(
            "loadCodeAssist failed: {}",
            last_load_error.unwrap_or_else(|| "unknown error".to_string())
        ));
    };
    if let Some(project) = extract_project(load_parsed.cloudaicompanion_project.as_ref()) {
        return Ok(remember_project(client, project).await);
    }

    // Need to onboard a managed Code Assist project. Antigravity can return an
    // asynchronous operation, so match its OpenCode plugin's bounded polling
    // instead of assuming the first response is complete.
    let tier_id = default_tier(&load_parsed, antigravity);
    let onboard_body = serde_json::json!({
        "tierId": tier_id,
        "metadata": metadata,
    });
    let onboard_endpoints: &[&str] = if antigravity {
        &[
            ANTIGRAVITY_DAILY_BASE,
            ANTIGRAVITY_AUTOPUSH_BASE,
            CODE_ASSIST_BASE,
        ]
    } else {
        &[CODE_ASSIST_BASE]
    };
    for endpoint in onboard_endpoints {
        for _ in 0..10 {
            let onboard_url = format!("{endpoint}{ONBOARD_USER_ENDPOINT}");
            let response = match apply_headers(client, client.client.post(&onboard_url))
                .json(&onboard_body)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => response,
                _ => break,
            };
            let parsed: OnboardOperation = response.json().await?;
            if parsed.done.unwrap_or(false) {
                if let Some(project) = parsed
                    .response
                    .and_then(|response| response.cloudaicompanion_project)
                    .and_then(|project| project.id)
                    .filter(|project| !project.trim().is_empty())
                {
                    return Ok(remember_project(client, project).await);
                }
                if antigravity {
                    return Ok(
                        remember_project(client, ANTIGRAVITY_DEFAULT_PROJECT.to_string()).await,
                    );
                }
                return Err(anyhow!("onboardUser response missing project id"));
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
    if antigravity {
        warn!("Antigravity managed-project onboarding did not complete; using the compatibility project");
        return Ok(remember_project(client, ANTIGRAVITY_DEFAULT_PROJECT.to_string()).await);
    }
    Err(anyhow!("onboardUser did not complete"))
}

pub(crate) async fn send_stream(
    client: &AIClient,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    extra_body: Option<serde_json::Value>,
    max_tries: usize,
    trace: Option<ModelExchangeTraceConfig>,
) -> Result<StreamResponse> {
    let project = discover_project(client).await?;

    let (system_instruction, contents) =
        GeminiMessageConverter::convert_messages(messages, &client.config.model);
    let gemini_tools = GeminiMessageConverter::convert_tools(tools);
    let mut inner = gemini_request::try_build_request_body(
        client,
        system_instruction,
        contents,
        gemini_tools,
        extra_body,
    )?;

    let antigravity = is_antigravity(client);
    let model = if antigravity {
        let route = resolve_antigravity_model(&client.config.model, &inner);
        apply_antigravity_thinking(&mut inner, &route);
        route.model
    } else {
        client.config.model.clone()
    };
    let mut request_body = serde_json::json!({
        "model": model,
        "project": project,
        "request": inner,
    });
    if antigravity {
        if let Some(obj) = request_body.as_object_mut() {
            obj.insert(
                "userAgent".to_string(),
                serde_json::Value::String("antigravity".to_string()),
            );
            obj.insert(
                "requestType".to_string(),
                serde_json::Value::String("agent".to_string()),
            );
            #[cfg(feature = "subscription-auth")]
            obj.insert(
                "requestId".to_string(),
                serde_json::Value::String(format!("agent-{}", uuid::Uuid::new_v4())),
            );
        }
    }

    let configured_url = if client.config.request_url.is_empty() {
        format!("{}{}", CODE_ASSIST_BASE, STREAM_ENDPOINT)
    } else {
        client.config.request_url.clone()
    };
    let urls = if antigravity {
        vec![
            format!("{ANTIGRAVITY_DAILY_BASE}{STREAM_ENDPOINT}"),
            format!("{ANTIGRAVITY_AUTOPUSH_BASE}{STREAM_ENDPOINT}"),
            format!("{CODE_ASSIST_BASE}{STREAM_ENDPOINT}"),
        ]
    } else {
        vec![configured_url]
    };

    debug!(
        "Gemini Code Assist config: model={}, configured_model={}, request_url={}, project={}, max_tries={}",
        model, client.config.model, urls[0], project, max_tries
    );

    let idle_timeout = client.stream_options.idle_timeout;
    let ttft_timeout = client.stream_options.ttft_timeout;
    let mut last_error = None;
    for (index, url) in urls.iter().enumerate() {
        match execute_sse_request(
            "Gemini Code Assist Streaming API",
            url,
            &request_body,
            max_tries,
            ttft_timeout,
            trace.clone(),
            || apply_headers(client, client.client.post(url)),
            move |response, tx, tx_raw, remaining_ttft_timeout| {
                handle_gemini_stream(response, tx, tx_raw, remaining_ttft_timeout, idle_timeout)
            },
        )
        .await
        {
            Ok(response) => return Ok(response),
            Err(error)
                if index + 1 < urls.len() && should_try_next_antigravity_endpoint(&error) =>
            {
                warn!(
                    "Antigravity request failed at {}; trying the next OpenCode-compatible endpoint: {error:#}",
                    url
                );
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("no Gemini Code Assist endpoint was available")))
}

fn should_try_next_antigravity_endpoint(error: &anyhow::Error) -> bool {
    match error
        .downcast_ref::<AiProviderError>()
        .and_then(|error| error.http_status)
    {
        Some(403 | 404) => true,
        Some(status) if status >= 500 => true,
        Some(_) => false,
        // Transport and timeout errors do not have structured HTTP status.
        None => true,
    }
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

const DEFAULT_ANTIGRAVITY_MODELS: &[(&str, &str)] = &[
    ("gemini-3.1-pro-high", "Gemini 3.1 Pro (High)"),
    ("gemini-3.1-pro-low", "Gemini 3.1 Pro (Low)"),
    ("gemini-3-pro-high", "Gemini 3 Pro (High)"),
    ("gemini-3-pro-low", "Gemini 3 Pro (Low)"),
    ("gemini-3-flash", "Gemini 3 Flash"),
    ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
    ("claude-opus-4-6-thinking", "Claude Opus 4.6 Thinking"),
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
pub(crate) async fn list_models(client: &AIClient) -> Result<Vec<RemoteModelInfo>> {
    if is_antigravity(client) {
        return Ok(DEFAULT_ANTIGRAVITY_MODELS
            .iter()
            .map(|(id, display_name)| RemoteModelInfo {
                id: (*id).to_string(),
                display_name: Some((*display_name).to_string()),
            })
            .collect());
    }

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

#[cfg(test)]
mod tests {
    use super::{
        antigravity_metadata, apply_antigravity_thinking, default_tier, extract_project,
        resolve_antigravity_model, should_try_next_antigravity_endpoint, AiProviderError,
        CodeAssistTier, LoadCodeAssistResponse, ANTIGRAVITY_DEFAULT_PROJECT,
    };

    #[test]
    fn accepts_string_and_object_project_shapes() {
        assert_eq!(
            extract_project(Some(&serde_json::json!("project-string"))).as_deref(),
            Some("project-string")
        );
        assert_eq!(
            extract_project(Some(&serde_json::json!({ "id": "project-object" }))).as_deref(),
            Some("project-object")
        );
    }

    #[test]
    fn selects_the_provider_default_tier() {
        let load = LoadCodeAssistResponse {
            cloudaicompanion_project: None,
            allowed_tiers: vec![
                CodeAssistTier {
                    id: Some("FIRST".to_string()),
                    is_default: Some(false),
                },
                CodeAssistTier {
                    id: Some("DEFAULT".to_string()),
                    is_default: Some(true),
                },
            ],
        };
        assert_eq!(default_tier(&load, true), "DEFAULT");
    }

    #[test]
    fn scopes_the_compatibility_project_to_antigravity_discovery() {
        let load = antigravity_metadata("MACOS", Some(ANTIGRAVITY_DEFAULT_PROJECT));
        let onboard = antigravity_metadata("MACOS", None);

        assert_eq!(load["duetProject"], ANTIGRAVITY_DEFAULT_PROJECT);
        assert!(onboard.get("duetProject").is_none());
        assert_eq!(onboard["ideType"], "ANTIGRAVITY");
    }

    #[test]
    fn antigravity_endpoint_fallback_only_handles_compatible_failures() {
        let error = |status| {
            anyhow::Error::new(AiProviderError::from_parts(
                format!("HTTP {status}"),
                Some("Antigravity".to_string()),
                None,
                Some(status),
            ))
            .context("request failed")
        };

        assert!(!should_try_next_antigravity_endpoint(&error(400)));
        assert!(!should_try_next_antigravity_endpoint(&error(429)));
        assert!(should_try_next_antigravity_endpoint(&error(403)));
        assert!(should_try_next_antigravity_endpoint(&error(404)));
        assert!(should_try_next_antigravity_endpoint(&error(503)));
        assert!(should_try_next_antigravity_endpoint(&anyhow::anyhow!(
            "transport error"
        )));
    }

    #[test]
    fn maps_legacy_and_current_antigravity_models_to_subscription_wire_ids() {
        let empty_request = serde_json::json!({});
        let cases = [
            ("gemini-3-pro-preview", "gemini-3-pro-low"),
            ("gemini-3.1-pro-preview-customtools", "gemini-3.1-pro-low"),
            ("antigravity-gemini-3-pro-high", "gemini-3-pro-high"),
            ("gemini-3-flash-preview", "gemini-3-flash"),
            ("claude-opus-4-6-thinking-high", "claude-opus-4-6-thinking"),
            ("claude-sonnet-4-6", "claude-sonnet-4-6"),
        ];

        for (configured, expected) in cases {
            assert_eq!(
                resolve_antigravity_model(configured, &empty_request).model,
                expected
            );
        }
    }

    #[test]
    fn writes_provider_specific_antigravity_thinking_fields() {
        let mut gemini_request = serde_json::json!({});
        let gemini_route = resolve_antigravity_model("gemini-3.1-pro-high", &gemini_request);
        apply_antigravity_thinking(&mut gemini_request, &gemini_route);
        assert_eq!(
            gemini_request["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            "high"
        );

        let mut claude_request = serde_json::json!({});
        let claude_route =
            resolve_antigravity_model("claude-opus-4-6-thinking-low", &claude_request);
        apply_antigravity_thinking(&mut claude_request, &claude_route);
        assert_eq!(
            claude_request["generationConfig"]["thinkingConfig"]["thinking_budget"],
            8_192
        );
        assert_eq!(
            claude_request["generationConfig"]["maxOutputTokens"],
            64_000
        );
    }

    #[test]
    fn configures_validated_claude_tool_calls_and_nonempty_schemas() {
        let mut request = serde_json::json!({
            "tools": [{
                "functionDeclarations": [{
                    "name": "empty_tool",
                    "parameters": { "type": "object", "properties": {} }
                }]
            }]
        });
        let route = resolve_antigravity_model("claude-sonnet-4-6", &request);
        apply_antigravity_thinking(&mut request, &route);

        assert_eq!(
            request["toolConfig"]["functionCallingConfig"]["mode"],
            "VALIDATED"
        );
        assert_eq!(
            request["tools"][0]["functionDeclarations"][0]["parameters"]["required"],
            serde_json::json!(["_placeholder"])
        );
    }
}
