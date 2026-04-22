//! Internationalization (i18n) type definitions

use super::locale_registry::{locale_entry, locale_entry_from_code, LOCALE_REGISTRY};
use serde::{Deserialize, Serialize};

/// Locale identifier.
/// Add new variants here when a backend-supported locale is introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LocaleId {
    #[serde(rename = "zh-CN")]
    #[default]
    ZhCN,
    #[serde(rename = "zh-TW")]
    ZhTW,
    #[serde(rename = "en-US")]
    EnUS,
}

impl LocaleId {
    /// Returns the locale identifier string.
    pub fn as_str(&self) -> &'static str {
        locale_entry(*self).code
    }

    /// Parses a locale identifier from a string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        locale_entry_from_code(s).map(|entry| entry.id)
    }

    /// Returns all supported locales.
    pub fn all() -> Vec<LocaleId> {
        LOCALE_REGISTRY.iter().map(|entry| entry.id).collect()
    }

    /// Returns the English language name used in model-facing instructions.
    pub fn model_language_name(&self) -> &'static str {
        locale_entry(*self).model_language_name
    }

    /// Returns the short imperative language instruction for small model prompts.
    pub fn short_model_instruction(&self) -> &'static str {
        locale_entry(*self).short_model_instruction
    }
}

impl std::fmt::Display for LocaleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Locale metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleMetadata {
    /// Locale identifier
    pub id: LocaleId,
    /// Localized language name
    pub name: String,
    /// English language name
    pub english_name: String,
    /// Native language name
    pub native_name: String,
    /// Whether this is an RTL language
    pub rtl: bool,
}

impl LocaleMetadata {
    /// Returns metadata for all locales.
    pub fn all() -> Vec<LocaleMetadata> {
        LOCALE_REGISTRY
            .iter()
            .map(|entry| LocaleMetadata {
                id: entry.id,
                name: entry.name.to_string(),
                english_name: entry.english_name.to_string(),
                native_name: entry.native_name.to_string(),
                rtl: entry.rtl,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_parser_accepts_registered_locales_only() {
        for locale in LocaleId::all() {
            assert_eq!(LocaleId::from_str(locale.as_str()), Some(locale));
        }

        assert_eq!(LocaleId::from_str("zh-Hant-TW"), Some(LocaleId::ZhTW));
        assert_eq!(LocaleId::from_str("  ZH-hans-CN  "), Some(LocaleId::ZhCN));
        assert_eq!(LocaleId::from_str("en"), Some(LocaleId::EnUS));
        assert_eq!(LocaleId::from_str("fr-FR"), None);
    }

    #[test]
    fn locale_metadata_matches_supported_locale_ids() {
        let ids: Vec<_> = LocaleId::all();
        let metadata_ids: Vec<_> = LocaleMetadata::all()
            .into_iter()
            .map(|metadata| metadata.id)
            .collect();

        assert_eq!(metadata_ids, ids);
    }
}

/// I18n configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nConfig {
    /// Current locale
    #[serde(rename = "currentLanguage")]
    pub current_language: LocaleId,
    /// Fallback locale
    #[serde(rename = "fallbackLanguage")]
    pub fallback_language: LocaleId,
    /// Whether to auto-detect locale
    #[serde(rename = "autoDetect")]
    pub auto_detect: bool,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            current_language: LocaleId::ZhCN,
            fallback_language: LocaleId::EnUS,
            auto_detect: false,
        }
    }
}

/// Translation arguments
#[derive(Debug, Clone, Default)]
pub struct TranslationArgs {
    args: std::collections::HashMap<String, FluentValue>,
}

/// Fluent value type
#[derive(Debug, Clone)]
pub enum FluentValue {
    String(String),
    Number(f64),
}

impl TranslationArgs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_string(mut self, key: &str, value: impl Into<String>) -> Self {
        self.args
            .insert(key.to_string(), FluentValue::String(value.into()));
        self
    }

    pub fn with_number(mut self, key: &str, value: f64) -> Self {
        self.args
            .insert(key.to_string(), FluentValue::Number(value));
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &FluentValue)> {
        self.args.iter()
    }
}
