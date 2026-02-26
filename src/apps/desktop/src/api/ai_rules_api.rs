//! AI Rules Management API

use crate::api::AppState;
use bitfun_core::service::ai_rules::*;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiRuleLevel {
    User,
    Project,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRulesRequest {
    pub level: ApiRuleLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRuleRequest {
    pub level: ApiRuleLevel,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRuleApiRequest {
    pub level: ApiRuleLevel,
    pub rule: CreateRuleRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRuleApiRequest {
    pub level: ApiRuleLevel,
    pub name: String,
    pub rule: UpdateRuleRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRuleApiRequest {
    pub level: ApiRuleLevel,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRulesStatsRequest {
    pub level: ApiRuleLevel,
}

#[tauri::command]
pub async fn get_ai_rules(
    state: State<'_, AppState>,
    request: GetRulesRequest,
) -> Result<Vec<AIRule>, String> {
    let rules_service = &state.ai_rules_service;

    match request.level {
        ApiRuleLevel::User => rules_service
            .get_user_rules()
            .await
            .map_err(|e| format!("Failed to get user rules: {}", e)),
        ApiRuleLevel::Project => rules_service
            .get_project_rules()
            .await
            .map_err(|e| format!("Failed to get project rules: {}", e)),
        ApiRuleLevel::All => {
            let mut all_rules = Vec::new();

            let user_rules = rules_service
                .get_user_rules()
                .await
                .map_err(|e| format!("Failed to get user rules: {}", e))?;
            all_rules.extend(user_rules);

            let project_rules = rules_service
                .get_project_rules()
                .await
                .map_err(|e| format!("Failed to get project rules: {}", e))?;
            all_rules.extend(project_rules);
            all_rules.sort_by(|a, b| a.name.cmp(&b.name));

            Ok(all_rules)
        }
    }
}

#[tauri::command]
pub async fn get_ai_rule(
    state: State<'_, AppState>,
    request: GetRuleRequest,
) -> Result<Option<AIRule>, String> {
    let rules_service = &state.ai_rules_service;

    match request.level {
        ApiRuleLevel::User => rules_service
            .get_user_rule(&request.name)
            .await
            .map_err(|e| format!("Failed to get user rule: {}", e)),
        ApiRuleLevel::Project => rules_service
            .get_project_rule(&request.name)
            .await
            .map_err(|e| format!("Failed to get project rule: {}", e)),
        ApiRuleLevel::All => {
            if let Some(rule) = rules_service
                .get_user_rule(&request.name)
                .await
                .map_err(|e| format!("Failed to get user rule: {}", e))?
            {
                Ok(Some(rule))
            } else {
                rules_service
                    .get_project_rule(&request.name)
                    .await
                    .map_err(|e| format!("Failed to get project rule: {}", e))
            }
        }
    }
}

#[tauri::command]
pub async fn create_ai_rule(
    state: State<'_, AppState>,
    request: CreateRuleApiRequest,
) -> Result<AIRule, String> {
    let rules_service = &state.ai_rules_service;

    match request.level {
        ApiRuleLevel::User => rules_service
            .create_user_rule(request.rule)
            .await
            .map_err(|e| format!("Failed to create user rule: {}", e)),
        ApiRuleLevel::Project => rules_service
            .create_project_rule(request.rule)
            .await
            .map_err(|e| format!("Failed to create project rule: {}", e)),
        ApiRuleLevel::All => Err(
            "Cannot create rule with 'all' level. Please specify 'user' or 'project'.".to_string(),
        ),
    }
}

#[tauri::command]
pub async fn update_ai_rule(
    state: State<'_, AppState>,
    request: UpdateRuleApiRequest,
) -> Result<AIRule, String> {
    let rules_service = &state.ai_rules_service;

    match request.level {
        ApiRuleLevel::User => rules_service
            .update_user_rule(&request.name, request.rule)
            .await
            .map_err(|e| format!("Failed to update user rule: {}", e)),
        ApiRuleLevel::Project => rules_service
            .update_project_rule(&request.name, request.rule)
            .await
            .map_err(|e| format!("Failed to update project rule: {}", e)),
        ApiRuleLevel::All => Err(
            "Cannot update rule with 'all' level. Please specify 'user' or 'project'.".to_string(),
        ),
    }
}

#[tauri::command]
pub async fn delete_ai_rule(
    state: State<'_, AppState>,
    request: DeleteRuleApiRequest,
) -> Result<bool, String> {
    let rules_service = &state.ai_rules_service;

    match request.level {
        ApiRuleLevel::User => rules_service
            .delete_user_rule(&request.name)
            .await
            .map_err(|e| format!("Failed to delete user rule: {}", e)),
        ApiRuleLevel::Project => rules_service
            .delete_project_rule(&request.name)
            .await
            .map_err(|e| format!("Failed to delete project rule: {}", e)),
        ApiRuleLevel::All => Err(
            "Cannot delete rule with 'all' level. Please specify 'user' or 'project'.".to_string(),
        ),
    }
}

#[tauri::command]
pub async fn get_ai_rules_stats(
    state: State<'_, AppState>,
    request: GetRulesStatsRequest,
) -> Result<RuleStats, String> {
    let rules_service = &state.ai_rules_service;

    match request.level {
        ApiRuleLevel::User => rules_service
            .get_user_rules_stats()
            .await
            .map_err(|e| format!("Failed to get user rules stats: {}", e)),
        ApiRuleLevel::Project => rules_service
            .get_project_rules_stats()
            .await
            .map_err(|e| format!("Failed to get project rules stats: {}", e)),
        ApiRuleLevel::All => {
            let user_stats = rules_service
                .get_user_rules_stats()
                .await
                .map_err(|e| format!("Failed to get user rules stats: {}", e))?;
            let project_stats = rules_service
                .get_project_rules_stats()
                .await
                .map_err(|e| format!("Failed to get project rules stats: {}", e))?;

            let mut by_apply_type = user_stats.by_apply_type.clone();
            for (key, value) in project_stats.by_apply_type {
                *by_apply_type.entry(key).or_insert(0) += value;
            }

            Ok(RuleStats {
                total_rules: user_stats.total_rules + project_stats.total_rules,
                enabled_rules: user_stats.enabled_rules + project_stats.enabled_rules,
                disabled_rules: user_stats.disabled_rules + project_stats.disabled_rules,
                by_apply_type,
            })
        }
    }
}

#[tauri::command]
pub async fn build_ai_rules_system_prompt(state: State<'_, AppState>) -> Result<String, String> {
    let rules_service = &state.ai_rules_service;

    rules_service
        .build_system_prompt()
        .await
        .map_err(|e| format!("Failed to build system prompt: {}", e))
}

#[tauri::command]
pub async fn reload_ai_rules(
    state: State<'_, AppState>,
    level: ApiRuleLevel,
) -> Result<(), String> {
    let rules_service = &state.ai_rules_service;

    match level {
        ApiRuleLevel::User => rules_service
            .reload_user_rules()
            .await
            .map_err(|e| format!("Failed to reload user rules: {}", e)),
        ApiRuleLevel::Project => rules_service
            .reload_project_rules()
            .await
            .map_err(|e| format!("Failed to reload project rules: {}", e)),
        ApiRuleLevel::All => {
            rules_service
                .reload_user_rules()
                .await
                .map_err(|e| format!("Failed to reload user rules: {}", e))?;
            rules_service
                .reload_project_rules()
                .await
                .map_err(|e| format!("Failed to reload project rules: {}", e))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleRuleApiRequest {
    pub level: ApiRuleLevel,
    pub name: String,
}

#[tauri::command]
pub async fn toggle_ai_rule(
    state: State<'_, AppState>,
    request: ToggleRuleApiRequest,
) -> Result<AIRule, String> {
    let rules_service = &state.ai_rules_service;

    match request.level {
        ApiRuleLevel::User => rules_service
            .toggle_user_rule(&request.name)
            .await
            .map_err(|e| format!("Failed to toggle user rule: {}", e)),
        ApiRuleLevel::Project => rules_service
            .toggle_project_rule(&request.name)
            .await
            .map_err(|e| format!("Failed to toggle project rule: {}", e)),
        ApiRuleLevel::All => Err(
            "Cannot toggle rule with 'all' level. Please specify 'user' or 'project'.".to_string(),
        ),
    }
}
