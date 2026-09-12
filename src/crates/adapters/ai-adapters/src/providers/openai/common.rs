use crate::client::quirks::{
    apply_openai_compatible_toggle, is_deepseek_reasoning_effort_model, is_deepseek_url,
    is_glm_52_reasoning_effort_model, is_zhipuai_url, normalize_deepseek_reasoning_effort,
    normalize_glm_52_reasoning_effort,
};
use crate::client::utils::{dedupe_remote_models, normalize_base_url_for_discovery};
use crate::client::AIClient;
use crate::providers::shared;
use crate::types::{
    ReasoningPresetAction, ReasoningPresetDescriptor, RemoteModelInfo, ToolDefinition,
};
use anyhow::{anyhow, Context, Result};
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
            builder = builder.header("X-Verification-Code", "from_openbitfun");
        }

        builder
    })
}

pub(crate) fn compile_chat_reasoning_action(
    preset: &ReasoningPresetDescriptor,
    action: &ReasoningPresetAction,
    request_body: &mut serde_json::Value,
    url: &str,
    configured_model: &str,
) -> Result<bool> {
    let execution_provider = preset.execution_provider.as_deref().unwrap_or("openai");
    let execution_model = preset
        .execution_model
        .as_deref()
        .unwrap_or(configured_model)
        .trim()
        .to_ascii_lowercase();
    let is_deepseek_reasoning_target = execution_provider.eq_ignore_ascii_case("deepseek")
        || is_deepseek_url(url)
        || is_deepseek_reasoning_effort_model(&execution_model);
    let is_glm_52_reasoning_target = is_glm_52_reasoning_effort_model(&execution_model)
        && (execution_provider.eq_ignore_ascii_case("zhipuai") || is_zhipuai_url(url));
    let is_generic_reasoning = shared::is_generic_reasoning_preset(preset);

    match action {
        ReasoningPresetAction::Toggle { enabled } if is_deepseek_reasoning_target => {
            request_body["thinking"] = serde_json::json!({
                "type": if *enabled { "enabled" } else { "disabled" }
            });
            if !enabled {
                request_body
                    .as_object_mut()
                    .map(|body| body.remove("reasoning_effort"));
            }
            Ok(true)
        }
        ReasoningPresetAction::Effort { value } if is_deepseek_reasoning_target => {
            let normalized = normalize_deepseek_reasoning_effort(&execution_model, value)
                .ok_or_else(|| {
                    anyhow!(
                        "DeepSeek reasoning effort '{}' is unsupported for model '{}'",
                        value,
                        execution_model
                    )
                })?;
            request_body["thinking"] = serde_json::json!({ "type": "enabled" });
            request_body["reasoning_effort"] = serde_json::json!(normalized);
            Ok(true)
        }
        ReasoningPresetAction::Toggle { enabled } if is_glm_52_reasoning_target => {
            request_body["thinking"] = serde_json::json!({
                "type": if *enabled { "enabled" } else { "disabled" }
            });
            if !enabled {
                request_body
                    .as_object_mut()
                    .map(|body| body.remove("reasoning_effort"));
            }
            Ok(true)
        }
        ReasoningPresetAction::Effort { value } if is_glm_52_reasoning_target => {
            let normalized = normalize_glm_52_reasoning_effort(value)
                .ok_or_else(|| anyhow!("GLM-5.2 reasoning effort '{}' is unsupported", value))?;
            request_body["thinking"] = serde_json::json!({ "type": "enabled" });
            request_body["reasoning_effort"] = serde_json::json!(normalized);
            Ok(true)
        }
        ReasoningPresetAction::Toggle { enabled } if is_generic_reasoning => {
            if !apply_openai_compatible_toggle(request_body, *enabled, url) {
                request_body["thinking"] = serde_json::json!({
                    "type": if *enabled { "enabled" } else { "disabled" }
                });
            }
            if !enabled {
                request_body
                    .as_object_mut()
                    .map(|body| body.remove("reasoning_effort"));
            }
            Ok(true)
        }
        ReasoningPresetAction::Effort { value } if is_generic_reasoning => {
            let normalized = shared::normalize_generic_reasoning_effort(value)
                .ok_or_else(|| anyhow!("Generic reasoning effort '{}' is unsupported", value))?;
            request_body["reasoning_effort"] = serde_json::json!(normalized);
            Ok(true)
        }
        ReasoningPresetAction::Toggle { enabled } => {
            Ok(apply_openai_compatible_toggle(request_body, *enabled, url))
        }
        ReasoningPresetAction::Effort { .. } | ReasoningPresetAction::BudgetTokens { .. } => {
            Ok(false)
        }
        ReasoningPresetAction::RequestPatch { .. } => {
            unreachable!("patches are compiled by shared code")
        }
    }
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
                routing: None,
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
    #[serde(default)]
    display_name: Option<String>,
    /// Codex backend marks deprecated/internal slugs with `visibility = "hide"`.
    /// We only surface entries the CLI itself shows (`list`).
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    priority: Option<i64>,
}

const DEFAULT_CODEX_MODELS: &[&str] =
    &["gpt-5.5", "gpt-5.3-codex-spark", "gpt-5.4", "gpt-5.4-mini"];

fn parsed_gpt_version(model_id: &str) -> Option<(u32, u32)> {
    let version = model_id.strip_prefix("gpt-")?.split('-').next()?;
    let (major, minor) = version.split_once('.')?;
    if minor.contains('.') {
        return None;
    }
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// Mirrors OpenCode's ChatGPT subscription allowlist while keeping future
/// versioned variants visible. Exact `gpt-5.6` and the Pro entitlement remain
/// excluded because the subscription endpoint does not offer those products.
fn codex_subscription_model_allowed(model_id: &str) -> bool {
    let model_id = model_id.trim().to_ascii_lowercase();
    if DEFAULT_CODEX_MODELS.contains(&model_id.as_str()) {
        return true;
    }
    if matches!(model_id.as_str(), "gpt-5.5-pro" | "gpt-5.6") {
        return false;
    }
    parsed_gpt_version(&model_id).is_some_and(|version| version > (5, 4))
}

pub(crate) fn is_known_codex_reasoning_model(model_id: &str) -> bool {
    let model_id = model_id.trim().to_ascii_lowercase();
    model_id == "gpt-5-codex" || codex_subscription_model_allowed(&model_id)
}

/// The account's live catalog is authoritative, including subscription-only
/// models whose supported_in_api flag is false (that flag is for public API
/// billing). Do not apply the offline reasoning-model heuristic here.
fn codex_models_from_entries(mut entries: Vec<CodexBackendModelEntry>) -> Vec<RemoteModelInfo> {
    entries.retain(|model| {
        !model.slug.trim().is_empty()
            && !model.visibility.as_deref().is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "hide" | "hidden"
                )
            })
    });
    entries.sort_by(|a, b| {
        a.priority
            .unwrap_or(10_000)
            .cmp(&b.priority.unwrap_or(10_000))
            .then_with(|| a.slug.cmp(&b.slug))
    });
    dedupe_remote_models(
        entries
            .into_iter()
            .map(|model| RemoteModelInfo {
                routing: None,
                id: model.slug,
                display_name: model.display_name,
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

    let response = apply_headers(client, client.client.get(&url))
        .send()
        .await
        .context("fetch Codex subscription models")?
        .error_for_status()
        .context("Codex subscription model discovery failed")?;
    let payload: CodexBackendModelsResponse = response
        .json()
        .await
        .context("parse Codex subscription models")?;
    let models = codex_models_from_entries(payload.models);
    if models.is_empty() {
        return Err(anyhow!("Codex returned no visible models for this account"));
    }
    Ok(models)
}

pub(crate) fn extract_tool_name(tool: &serde_json::Value) -> String {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(|name| name.as_str())
        .or_else(|| tool.get("name").and_then(|name| name.as_str()))
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) fn attach_tools(
    request_body: &mut serde_json::Value,
    tools: Option<Vec<serde_json::Value>>,
    target: &str,
) {
    match tools {
        Some(tools) if !tools.is_empty() => {
            let tool_names = tools.iter().map(extract_tool_name).collect::<Vec<_>>();
            shared::log_tool_names(target, tool_names);
            request_body["tools"] = serde_json::Value::Array(tools);
            let has_tool_choice = request_body
                .get("tool_choice")
                .is_some_and(|value| !value.is_null());
            if !has_tool_choice {
                request_body["tool_choice"] = serde_json::Value::String("auto".to_string());
            }
        }
        _ => {
            if request_body
                .as_object_mut()
                .and_then(|object| object.remove("tool_choice"))
                .is_some()
            {
                log::debug!(
                    target: target,
                    "Removed tool_choice from OpenAI request because no tools are attached"
                );
            }
        }
    }
}

pub(crate) fn convert_tools_flat(
    tools: Option<Vec<ToolDefinition>>,
) -> Option<Vec<serde_json::Value>> {
    tools.map(|defs| {
        defs.into_iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                    "strict": false,
                })
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{attach_tools, codex_subscription_model_allowed, is_known_codex_reasoning_model};
    use serde_json::json;

    #[test]
    fn live_codex_catalog_keeps_subscription_only_and_new_models() {
        let payload: super::CodexBackendModelsResponse = serde_json::from_value(json!({
            "models": [
                {"slug": "gpt-5.3-codex-spark", "supported_in_api": false, "priority": 1,
                 "display_name": "GPT-5.3 Codex Spark", "visibility": "list"},
                {"slug": "future-subscription-model", "priority": 2},
                {"slug": "gpt-5.5-pro", "priority": 3},
                {"slug": "internal", "visibility": "hidden"},
                {"slug": "  "},
                {"slug": "gpt-5.3-codex-spark", "priority": 8}
            ]
        }))
        .unwrap();
        let models = super::codex_models_from_entries(payload.models);
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            [
                "gpt-5.3-codex-spark",
                "future-subscription-model",
                "gpt-5.5-pro"
            ]
        );
        assert_eq!(
            models[0].display_name.as_deref(),
            Some("GPT-5.3 Codex Spark")
        );
    }

    #[test]
    fn attach_tools_removes_tool_choice_without_tools() {
        let mut request_body = json!({
            "model": "test-model",
            "messages": [],
            "stream": true,
            "tool_choice": "none"
        });

        attach_tools(&mut request_body, None, "test");

        assert!(request_body.get("tools").is_none());
        assert!(request_body.get("tool_choice").is_none());
    }

    #[test]
    fn codex_subscription_models_match_opencode_and_allow_future_variants() {
        for model in [
            "gpt-5.5",
            "gpt-5.3-codex-spark",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.6-sol",
            "gpt-6.0-codex",
        ] {
            assert!(codex_subscription_model_allowed(model), "{model}");
        }
        for model in ["gpt-5.5-pro", "gpt-5.6", "gpt-5.4-pro", "o3"] {
            assert!(!codex_subscription_model_allowed(model), "{model}");
        }

        assert!(is_known_codex_reasoning_model("GPT-5.5"));
        assert!(is_known_codex_reasoning_model("gpt-5-codex"));
        assert!(is_known_codex_reasoning_model("gpt-5.5-proxy"));
        assert!(!is_known_codex_reasoning_model("gpt-5.5-pro"));
    }

    #[test]
    fn attach_tools_removes_tool_choice_for_empty_tools() {
        let mut request_body = json!({
            "model": "test-model",
            "messages": [],
            "stream": true,
            "tool_choice": "none"
        });

        attach_tools(&mut request_body, Some(vec![]), "test");

        assert!(request_body.get("tools").is_none());
        assert!(request_body.get("tool_choice").is_none());
    }

    #[test]
    fn attach_tools_preserves_explicit_tool_choice_with_tools() {
        let mut request_body = json!({
            "model": "test-model",
            "messages": [],
            "stream": true,
            "tool_choice": "none"
        });

        attach_tools(
            &mut request_body,
            Some(vec![json!({
                "type": "function",
                "function": {
                    "name": "example",
                    "description": "Example tool",
                    "parameters": { "type": "object" }
                }
            })]),
            "test",
        );

        assert_eq!(request_body["tool_choice"], json!("none"));
        assert_eq!(
            request_body["tools"][0]["function"]["name"],
            json!("example")
        );
    }
}
