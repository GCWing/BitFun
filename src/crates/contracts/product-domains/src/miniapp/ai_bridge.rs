//! MiniApp AI bridge domain rules.
//!
//! This module owns provider-neutral permission, rate-limit, model selection,
//! and message normalization rules. Concrete AI clients and streaming transport
//! stay in the product host.

use crate::miniapp::types::AiPermissions;
use serde::{Deserialize, Serialize};
pub const AI_ACCESS_DISABLED_MESSAGE: &str = "AI access is not enabled for this MiniApp";

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
    pub name: String,
    pub provider: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppAiModelDescriptor {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub enabled: bool,
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
        .filter(|model| {
            if allowed_models.is_empty() {
                return true;
            }
            allowed_models.iter().any(|allowed| match allowed.as_str() {
                "primary" => model.id == primary_id,
                "fast" => model.id == fast_id,
                other => model.id == other || model.name == other,
            })
        })
        .map(|model| MiniAppAiModelInfo {
            is_default: model.id == primary_id,
            id: model.id,
            name: model.name,
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
                provider: "openai".to_string(),
                enabled: true,
            },
            MiniAppAiModelDescriptor {
                id: "m-fast".to_string(),
                name: "Fast Model".to_string(),
                provider: "openai".to_string(),
                enabled: true,
            },
            MiniAppAiModelDescriptor {
                id: "disabled".to_string(),
                name: "Disabled".to_string(),
                provider: "openai".to_string(),
                enabled: false,
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
}
