//! I18n API

use crate::api::app_state::AppState;
use bitfun_core::service::i18n::{sync_global_i18n_service_locale, LocaleId, LocaleMetadata};
use log::{error, info, warn};
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
        Ok(language) => Ok(LocaleId::from_str(&language)
            .unwrap_or_default()
            .as_str()
            .to_string()),
        Err(_) => Ok("zh-CN".to_string()),
    }
}

#[tauri::command]
pub async fn i18n_set_language(
    state: State<'_, AppState>,
    _app: tauri::AppHandle,
    request: SetLanguageRequest,
) -> Result<String, String> {
    let Some(locale_id) = LocaleId::from_str(&request.language) else {
        return Err(format!("Unsupported language: {}", request.language));
    };
    let language = locale_id.as_str();

    let config_service = &state.config_service;

    match config_service.set_config("app.language", language).await {
        Ok(_) => {
            info!("Language set to: {}", language);

            // Sync the in-memory I18nService so bot/remote-connect responses
            // use the newly selected language without requiring an app restart.
            match sync_global_i18n_service_locale(locale_id).await {
                Ok(true) => {}
                Ok(false) => {
                    warn!(
                        "Global I18nService not initialized after language change: language={}",
                        language
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to sync I18nService locale after language change: language={}, error={}",
                        language, e
                    );
                }
            }

            #[cfg(target_os = "macos")]
            {
                let has_workspace = state.workspace_path.read().await.is_some();
                let mode = if has_workspace {
                    crate::macos_menubar::MenubarMode::Workspace
                } else {
                    crate::macos_menubar::MenubarMode::Startup
                };
                let edit_mode = *state.macos_edit_menu_mode.read().await;
                let _ = crate::macos_menubar::set_macos_menubar_with_mode(
                    &_app, language, mode, edit_mode,
                );
            }
            Ok(format!("Language switched to: {}", language))
        }
        Err(e) => {
            error!("Failed to set language: language={}, error={}", language, e);
            Err(format!("Failed to set language: {}", e))
        }
    }
}

#[tauri::command]
pub async fn i18n_get_supported_languages() -> Result<Vec<LocaleMetadataResponse>, String> {
    Ok(LocaleMetadata::all()
        .into_iter()
        .map(|locale| LocaleMetadataResponse {
            id: locale.id.as_str().to_string(),
            name: locale.name,
            english_name: locale.english_name,
            native_name: locale.native_name,
            rtl: locale.rtl,
        })
        .collect())
}

#[tauri::command]
pub async fn i18n_get_config(state: State<'_, AppState>) -> Result<Value, String> {
    let config_service = &state.config_service;

    let current_language = match config_service
        .get_config::<String>(Some("app.language"))
        .await
    {
        Ok(language) => LocaleId::from_str(&language)
            .unwrap_or_default()
            .as_str()
            .to_string(),
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
        let Some(locale_id) = LocaleId::from_str(language) else {
            return Err(format!("Unsupported language: {}", language));
        };

        match config_service
            .set_config("app.language", locale_id.as_str())
            .await
        {
            Ok(_) => {
                match sync_global_i18n_service_locale(locale_id).await {
                    Ok(true) => {}
                    Ok(false) => {
                        warn!(
                            "Global I18nService not initialized after i18n config save: language={}",
                            locale_id.as_str()
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to sync I18nService locale after i18n config save: language={}, error={}",
                            locale_id.as_str(),
                            e
                        );
                    }
                }
                Ok("i18n config saved".to_string())
            }
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
