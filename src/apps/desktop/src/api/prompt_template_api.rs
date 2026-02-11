//! Prompt Template Management API

use crate::api::app_state::AppState;
use log::{error, warn};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub category: Option<String>,
    pub shortcut: Option<String>,
    pub is_favorite: bool,
    pub order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub usage_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateConfig {
    pub templates: Vec<PromptTemplate>,
    pub global_shortcut: String,
    pub enable_auto_complete: bool,
    pub recent_templates: Vec<String>,
    pub last_sync_time: Option<i64>,
}

impl Default for PromptTemplateConfig {
    fn default() -> Self {
        Self {
            templates: Vec::new(),
            global_shortcut: "Ctrl+Shift+P".to_string(),
            enable_auto_complete: true,
            recent_templates: Vec::new(),
            last_sync_time: None,
        }
    }
}

#[tauri::command]
pub async fn get_prompt_template_config(
    state: State<'_, AppState>,
) -> Result<PromptTemplateConfig, String> {
    let config_service = &state.config_service;

    match config_service
        .get_config::<Option<PromptTemplateConfig>>(Some("prompt_templates"))
        .await
    {
        Ok(Some(config)) => Ok(config),
        Ok(None) => {
            let default_config = create_default_config();
            if let Err(e) = config_service
                .set_config("prompt_templates", &default_config)
                .await
            {
                warn!("Failed to save default config: error={}", e);
            }
            Ok(default_config)
        }
        Err(e) => {
            error!("Failed to get prompt template config: error={}", e);
            Ok(create_default_config())
        }
    }
}

#[tauri::command]
pub async fn save_prompt_template_config(
    state: State<'_, AppState>,
    config: PromptTemplateConfig,
) -> Result<(), String> {
    let config_service = &state.config_service;

    config_service
        .set_config("prompt_templates", config)
        .await
        .map_err(|e| {
            error!("Failed to save prompt template config: error={}", e);
            format!("Failed to save config: {}", e)
        })
}

#[tauri::command]
pub async fn export_prompt_templates(state: State<'_, AppState>) -> Result<String, String> {
    let config = get_prompt_template_config(state).await?;

    serde_json::to_string_pretty(&config).map_err(|e| {
        error!("Failed to export prompt templates: error={}", e);
        format!("Export failed: {}", e)
    })
}

#[tauri::command]
pub async fn import_prompt_templates(
    state: State<'_, AppState>,
    json: String,
) -> Result<(), String> {
    let config: PromptTemplateConfig =
        serde_json::from_str(&json).map_err(|e| format!("Invalid config format: {}", e))?;

    save_prompt_template_config(state, config).await
}

#[tauri::command]
pub async fn reset_prompt_templates(state: State<'_, AppState>) -> Result<(), String> {
    let default_config = create_default_config();
    save_prompt_template_config(state, default_config).await
}

fn create_default_config() -> PromptTemplateConfig {
    let now = chrono::Utc::now().timestamp_millis();

    PromptTemplateConfig {
        templates: Vec::new(),
        global_shortcut: "Ctrl+Shift+P".to_string(),
        enable_auto_complete: true,
        recent_templates: Vec::new(),
        last_sync_time: Some(now),
    }
}
