//! I18n API

use crate::api::app_state::AppState;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleMetadataResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "englishName")]
    pub english_name: String,
    #[serde(rename = "nativeName")]
    pub native_name: String,
    pub rtl: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetLanguageRequest {
    pub language: String,
}

#[derive(Debug, Deserialize)]
pub struct TranslateRequest {
    pub key: String,
    pub args: Option<Value>,
}

#[tauri::command]
pub async fn i18n_get_current_language(state: State<'_, AppState>) -> Result<String, String> {
    let config_service = &state.config_service;

    match config_service
        .get_config::<String>(Some("app.language"))
        .await
    {
        Ok(language) => Ok(language),
        Err(_) => Ok("zh-CN".to_string()),
    }
}

#[tauri::command]
pub async fn i18n_set_language(
    state: State<'_, AppState>,
    _app: tauri::AppHandle,
    request: SetLanguageRequest,
) -> Result<String, String> {
    let supported = vec!["zh-CN", "en-US"];
    if !supported.contains(&request.language.as_str()) {
        return Err(format!("Unsupported language: {}", request.language));
    }

    let config_service = &state.config_service;

    match config_service
        .set_config("app.language", &request.language)
        .await
    {
        Ok(_) => {
            info!("Language set to: {}", request.language);
            #[cfg(target_os = "macos")]
            {
                let has_workspace = state.workspace_path.read().await.is_some();
                let mode = if has_workspace {
                    crate::macos_menubar::MenubarMode::Workspace
                } else {
                    crate::macos_menubar::MenubarMode::Startup
                };
                let _ = crate::macos_menubar::set_macos_menubar_with_mode(
                    &_app,
                    &request.language,
                    mode,
                );
            }
            Ok(format!("Language switched to: {}", request.language))
        }
        Err(e) => {
            error!(
                "Failed to set language: language={}, error={}",
                request.language, e
            );
            Err(format!("Failed to set language: {}", e))
        }
    }
}

#[tauri::command]
pub async fn i18n_get_supported_languages() -> Result<Vec<LocaleMetadataResponse>, String> {
    let locales = vec![
        LocaleMetadataResponse {
            id: "zh-CN".to_string(),
            name: "简体中文".to_string(),
            english_name: "Simplified Chinese".to_string(),
            native_name: "简体中文".to_string(),
            rtl: false,
        },
        LocaleMetadataResponse {
            id: "en-US".to_string(),
            name: "English".to_string(),
            english_name: "English (US)".to_string(),
            native_name: "English".to_string(),
            rtl: false,
        },
    ];

    Ok(locales)
}

#[tauri::command]
pub async fn i18n_get_config(state: State<'_, AppState>) -> Result<Value, String> {
    let config_service = &state.config_service;

    let current_language = match config_service
        .get_config::<String>(Some("app.language"))
        .await
    {
        Ok(language) => language,
        Err(_) => "zh-CN".to_string(),
    };

    Ok(serde_json::json!({
        "currentLanguage": current_language,
        "fallbackLanguage": "en-US",
        "autoDetect": false
    }))
}

#[tauri::command]
pub async fn i18n_set_config(state: State<'_, AppState>, config: Value) -> Result<String, String> {
    let config_service = &state.config_service;

    if let Some(language) = config.get("currentLanguage").and_then(|v| v.as_str()) {
        match config_service.set_config("app.language", language).await {
            Ok(_) => Ok("i18n config saved".to_string()),
            Err(e) => {
                error!(
                    "Failed to save i18n config: language={}, error={}",
                    language, e
                );
                Err(format!("Failed to save i18n config: {}", e))
            }
        }
    } else {
        Ok("i18n config saved (no language change)".to_string())
    }
}
