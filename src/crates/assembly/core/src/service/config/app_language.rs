//! Canonical UI language for user-facing AI output.
//!
//! Desktop and server store the active locale in `app.language` (see `i18n_set_language` in the
//! desktop crate). Agent prompts read this via `PromptBuilder::get_language_preference`. Any
//! other AI calls that should match the UI (e.g. session titles) must use the same source — not
//! `I18nService::get_current_locale`, which historically synced from `i18n.currentLanguage` only.

use super::GlobalConfigManager;
use crate::service::i18n::LocaleId;
use log::debug;

const DEFAULT_APP_LANGUAGE: LocaleId = LocaleId::ZhCN;

/// Returns a supported `app.language` from global config; otherwise `zh-CN`
/// (matches [`crate::service::config::AppConfig::default`]).
pub async fn get_app_language() -> LocaleId {
    let Ok(svc) = GlobalConfigManager::get_service().await else {
        return DEFAULT_APP_LANGUAGE;
    };
    match svc.get_config::<String>(Some("app.language")).await {
        Ok(code) => {
            if let Some(locale) = LocaleId::from_str(&code) {
                locale
            } else {
                debug!("Unknown app.language {}, defaulting to zh-CN", code);
                DEFAULT_APP_LANGUAGE
            }
        }
        Err(_) => DEFAULT_APP_LANGUAGE,
    }
}

/// Returns a supported `app.language` code from global config; otherwise `zh-CN`
/// (matches [`crate::service::config::AppConfig::default`]).
pub async fn get_app_language_code() -> String {
    get_app_language().await.as_str().to_string()
}

/// Short instruction for models to answer in the app UI language (session titles, etc.).
pub fn short_model_user_language_instruction(lang_code: &str) -> &'static str {
    LocaleId::from_str(lang_code)
        .unwrap_or(DEFAULT_APP_LANGUAGE)
        .short_model_instruction()
}
