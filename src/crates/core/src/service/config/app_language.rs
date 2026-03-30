//! Canonical UI language for user-facing AI output.
//!
//! Desktop and server store the active locale in `app.language` (see `i18n_set_language` in the
//! desktop crate). Agent prompts read this via `PromptBuilder::get_language_preference`. Any
//! other AI calls that should match the UI (e.g. session titles) must use the same source — not
//! `I18nService::get_current_locale`, which historically synced from `i18n.currentLanguage` only.

use super::GlobalConfigManager;
use log::debug;

/// Returns `zh-CN` or `en-US` from global config when valid; otherwise `zh-CN` (matches [`crate::service::config::AppConfig::default`]).
pub async fn get_app_language_code() -> String {
    let Ok(svc) = GlobalConfigManager::get_service().await else {
        return "zh-CN".to_string();
    };
    match svc.get_config::<String>(Some("app.language")).await {
        Ok(code) if code == "zh-CN" || code == "en-US" => code,
        Ok(other) => {
            debug!("Unknown app.language {}, defaulting to zh-CN", other);
            "zh-CN".to_string()
        }
        Err(_) => "zh-CN".to_string(),
    }
}

/// Short instruction for models to answer in the app UI language (session titles, etc.).
pub fn short_model_user_language_instruction(lang_code: &str) -> &'static str {
    match lang_code {
        "en-US" => "Use English",
        _ => "使用简体中文",
    }
}
