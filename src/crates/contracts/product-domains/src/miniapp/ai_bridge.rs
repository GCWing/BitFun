//! MiniApp AI bridge domain rules.
//!
//! This module owns provider-neutral permission, rate-limit, model selection,
//! and message normalization rules. Concrete AI clients and streaming transport
//! stay in the product host.

use crate::miniapp::types::AiPermissions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub const AI_ACCESS_DISABLED_MESSAGE: &str = "AI access is not enabled for this MiniApp";
pub const AI_MESSAGES_REQUIRED_MESSAGE: &str = "messages must not be empty";
pub const AI_STREAM_ID_REQUIRED_MESSAGE: &str = "streamId is required";

pub fn require_enabled_ai_permissions(
    ai_permissions: Option<&AiPermissions>,
) -> Result<&AiPermissions, String> {
    let ai_permissions = ai_permissions.ok_or(AI_ACCESS_DISABLED_MESSAGE)?;
    if !ai_permissions.enabled {
        return Err(AI_ACCESS_DISABLED_MESSAGE.to_string());
    }
    Ok(ai_permissions)
}

pub fn validate_model(
    model: Option<&str>,
    ai_permissions: &AiPermissions,
) -> Result<String, String> {
    let requested = model.unwrap_or("primary");
    if let Some(allowed) = ai_permissions.allowed_models.as_ref() {
        if !allowed.is_empty() && !allowed.iter().any(|model| model == requested) {
            return Err(format!(
                "Model '{}' is not allowed by this MiniApp's AI permissions",
                requested
            ));
        }
    }
    Ok(requested.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAiModelInfo {
    pub id: String,
    /// User-defined configuration name.
    pub name: String,
    /// Actual model identifier shown in the host model picker.
    pub model_name: String,
    pub provider: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppAiModelDescriptor {
    pub id: String,
    pub name: String,
    pub model_name: String,
    pub provider: String,
    pub enabled: bool,
    pub supports_text_chat: bool,
}

pub fn available_models_for_permissions<I>(
    models: I,
    allowed_models: &[String],
    primary_id: &str,
    fast_id: &str,
) -> Vec<MiniAppAiModelInfo>
where
    I: IntoIterator<Item = MiniAppAiModelDescriptor>,
{
    models
        .into_iter()
        .filter(|model| model.enabled)
        // Match the host chat ModelSelector: only chat-capable models.
        .filter(|model| model.supports_text_chat)
        .filter(|model| {
            if allowed_models.is_empty() {
                return true;
            }
            allowed_models.iter().any(|allowed| match allowed.as_str() {
                "primary" => model.id == primary_id,
                "fast" => model.id == fast_id,
                other => model.id == other || model.name == other || model.model_name == other,
            })
        })
        .map(|model| MiniAppAiModelInfo {
            is_default: model.id == primary_id,
            id: model.id,
            name: model.name,
            model_name: model.model_name,
            provider: model.provider,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniAppAiMessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppAiMessagePlan {
    pub role: MiniAppAiMessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppAiRequestPlan {
    pub model_ref: String,
    pub messages: Vec<MiniAppAiMessagePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAiStreamPayload {
    pub app_id: String,
    pub stream_id: String,
    #[serde(rename = "type")]
    pub payload_type: String,
    pub data: Value,
}

pub fn build_ai_message_plan<'a, I>(
    system_prompt: Option<&str>,
    chat_messages: I,
) -> Vec<MiniAppAiMessagePlan>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut messages = Vec::new();
    if let Some(system_prompt) = system_prompt.filter(|value| !value.is_empty()) {
        messages.push(MiniAppAiMessagePlan {
            role: MiniAppAiMessageRole::System,
            content: system_prompt.to_string(),
        });
    }
    for (role, content) in chat_messages {
        let role = if role.eq_ignore_ascii_case("assistant") {
            MiniAppAiMessageRole::Assistant
        } else {
            MiniAppAiMessageRole::User
        };
        messages.push(MiniAppAiMessagePlan {
            role,
            content: content.to_string(),
        });
    }
    messages
}

pub fn plan_ai_complete_request(
    ai_permissions: &AiPermissions,
    model: Option<&str>,
    system_prompt: Option<&str>,
    prompt: &str,
) -> Result<MiniAppAiRequestPlan, String> {
    Ok(MiniAppAiRequestPlan {
        model_ref: validate_model(model, ai_permissions)?,
        messages: build_ai_message_plan(system_prompt, [("user", prompt)]),
    })
}

pub fn plan_ai_chat_request<'a, I>(
    ai_permissions: &AiPermissions,
    model: Option<&str>,
    system_prompt: Option<&str>,
    chat_messages: I,
) -> Result<MiniAppAiRequestPlan, String>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let chat_messages: Vec<(&'a str, &'a str)> = chat_messages.into_iter().collect();
    if chat_messages.is_empty() {
        return Err(AI_MESSAGES_REQUIRED_MESSAGE.to_string());
    }
    Ok(MiniAppAiRequestPlan {
        model_ref: validate_model(model, ai_permissions)?,
        messages: build_ai_message_plan(system_prompt, chat_messages),
    })
}

pub fn require_non_empty_ai_messages(message_count: usize) -> Result<(), String> {
    if message_count == 0 {
        return Err(AI_MESSAGES_REQUIRED_MESSAGE.to_string());
    }
    Ok(())
}

pub fn require_non_empty_stream_id(stream_id: &str) -> Result<String, String> {
    if stream_id.trim().is_empty() {
        return Err(AI_STREAM_ID_REQUIRED_MESSAGE.to_string());
    }
    Ok(stream_id.to_string())
}

pub fn ai_stream_chunk_payload(
    app_id: &str,
    stream_id: &str,
    text: Option<String>,
    reasoning_content: Option<String>,
) -> MiniAppAiStreamPayload {
    MiniAppAiStreamPayload {
        app_id: app_id.to_string(),
        stream_id: stream_id.to_string(),
        payload_type: "chunk".to_string(),
        data: json!({
            "text": text,
            "reasoningContent": reasoning_content,
        }),
    }
}

pub fn ai_stream_error_payload(
    app_id: &str,
    stream_id: &str,
    message: impl Into<String>,
) -> MiniAppAiStreamPayload {
    MiniAppAiStreamPayload {
        app_id: app_id.to_string(),
        stream_id: stream_id.to_string(),
        payload_type: "error".to_string(),
        data: json!({ "message": message.into() }),
    }
}

pub fn ai_stream_done_payload(
    app_id: &str,
    stream_id: &str,
    full_text: impl Into<String>,
    usage: Option<MiniAppAiUsage>,
) -> MiniAppAiStreamPayload {
    MiniAppAiStreamPayload {
        app_id: app_id.to_string(),
        stream_id: stream_id.to_string(),
        payload_type: "done".to_string(),
        data: json!({
            "fullText": full_text.into(),
            "usage": usage,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ai_permissions(allowed_models: Option<Vec<String>>) -> AiPermissions {
        AiPermissions {
            enabled: true,
            allowed_models,
            rate_limit_per_minute: Some(2),
            max_tokens_per_request: None,
        }
    }

    #[test]
    fn model_selection_keeps_alias_and_allowlist_contract() {
        let perms = ai_permissions(Some(vec!["primary".to_string(), "m-fast".to_string()]));
        assert_eq!(validate_model(None, &perms).unwrap(), "primary");
        assert_eq!(validate_model(Some("m-fast"), &perms).unwrap(), "m-fast");
        assert_eq!(
            validate_model(Some("other"), &perms).unwrap_err(),
            "Model 'other' is not allowed by this MiniApp's AI permissions"
        );
    }

    #[test]
    fn model_list_filters_enabled_models_by_alias_or_id_or_name() {
        let models = vec![
            MiniAppAiModelDescriptor {
                id: "m-primary".to_string(),
                name: "Primary Model".to_string(),
                model_name: "gpt-primary".to_string(),
                provider: "openai".to_string(),
                enabled: true,
                supports_text_chat: true,
            },
            MiniAppAiModelDescriptor {
                id: "m-fast".to_string(),
                name: "Fast Model".to_string(),
                model_name: "gpt-fast".to_string(),
                provider: "openai".to_string(),
                enabled: true,
                supports_text_chat: true,
            },
            MiniAppAiModelDescriptor {
                id: "embedding".to_string(),
                name: "Embed".to_string(),
                model_name: "text-embedding".to_string(),
                provider: "openai".to_string(),
                enabled: true,
                supports_text_chat: false,
            },
            MiniAppAiModelDescriptor {
                id: "disabled".to_string(),
                name: "Disabled".to_string(),
                model_name: "disabled-model".to_string(),
                provider: "openai".to_string(),
                enabled: false,
                supports_text_chat: true,
            },
        ];

        let visible = available_models_for_permissions(
            models,
            &["primary".to_string(), "Fast Model".to_string()],
            "m-primary",
            "m-fast",
        );

        assert_eq!(visible.len(), 2);
        assert!(visible[0].is_default);
        assert_eq!(visible[0].model_name, "gpt-primary");
        assert_eq!(visible[1].id, "m-fast");
    }

    #[test]
    fn message_plan_treats_unknown_roles_as_user() {
        let messages = build_ai_message_plan(
            Some("system"),
            [("assistant", "ok"), ("tool", "fallback user")],
        );
        assert_eq!(messages[0].role, MiniAppAiMessageRole::System);
        assert_eq!(messages[1].role, MiniAppAiMessageRole::Assistant);
        assert_eq!(messages[2].role, MiniAppAiMessageRole::User);
    }

    #[test]
    fn ai_request_plans_preserve_model_and_message_contract() {
        let perms = ai_permissions(Some(vec!["primary".to_string(), "m-fast".to_string()]));

        let complete =
            plan_ai_complete_request(&perms, None, Some("system"), "hello").expect("complete plan");
        assert_eq!(complete.model_ref, "primary");
        assert_eq!(complete.messages.len(), 2);
        assert_eq!(complete.messages[0].role, MiniAppAiMessageRole::System);
        assert_eq!(complete.messages[1].role, MiniAppAiMessageRole::User);
        assert_eq!(complete.messages[1].content, "hello");

        let chat = plan_ai_chat_request(
            &perms,
            Some("m-fast"),
            Some("system"),
            [("assistant", "prior"), ("tool", "fallback user")],
        )
        .expect("chat plan");
        assert_eq!(chat.model_ref, "m-fast");
        assert_eq!(chat.messages[1].role, MiniAppAiMessageRole::Assistant);
        assert_eq!(chat.messages[2].role, MiniAppAiMessageRole::User);

        assert_eq!(
            plan_ai_chat_request(&perms, None, None, Vec::<(&str, &str)>::new()).unwrap_err(),
            AI_MESSAGES_REQUIRED_MESSAGE
        );
        assert_eq!(
            require_non_empty_ai_messages(0).unwrap_err(),
            AI_MESSAGES_REQUIRED_MESSAGE
        );
        assert!(require_non_empty_ai_messages(1).is_ok());
        assert_eq!(
            require_non_empty_stream_id("   ").unwrap_err(),
            AI_STREAM_ID_REQUIRED_MESSAGE
        );
        assert_eq!(
            require_non_empty_stream_id(" stream-1 ").unwrap(),
            " stream-1 "
        );
    }

    #[test]
    fn ai_stream_payload_helpers_preserve_wire_shape() {
        assert_eq!(
            serde_json::to_value(ai_stream_chunk_payload(
                "app-1",
                "stream-1",
                Some("text".to_string()),
                Some("reasoning".to_string()),
            ))
            .unwrap(),
            serde_json::json!({
                "appId": "app-1",
                "streamId": "stream-1",
                "type": "chunk",
                "data": {
                    "text": "text",
                    "reasoningContent": "reasoning"
                }
            })
        );

        assert_eq!(
            serde_json::to_value(ai_stream_done_payload(
                "app-1",
                "stream-1",
                "final",
                Some(MiniAppAiUsage {
                    prompt_tokens: 1,
                    completion_tokens: 2,
                    total_tokens: 3,
                }),
            ))
            .unwrap(),
            serde_json::json!({
                "appId": "app-1",
                "streamId": "stream-1",
                "type": "done",
                "data": {
                    "fullText": "final",
                    "usage": {
                        "promptTokens": 1,
                        "completionTokens": 2,
                        "totalTokens": 3
                    }
                }
            })
        );

        assert_eq!(
            serde_json::to_value(ai_stream_error_payload("app-1", "stream-1", "failed")).unwrap(),
            serde_json::json!({
                "appId": "app-1",
                "streamId": "stream-1",
                "type": "error",
                "data": { "message": "failed" }
            })
        );
    }
}
