//! Internationalization (i18n) type definitions

use serde::{Deserialize, Serialize};

/// Locale identifier.
/// Add new variants here when a backend-supported locale is introduced.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LocaleId {
    #[serde(rename = "zh-CN")]
    #[default]
    ZhCN,
    #[serde(rename = "en-US")]
    EnUS,
}

impl LocaleId {
    /// Returns the locale identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            LocaleId::ZhCN => "zh-CN",
            LocaleId::EnUS => "en-US",
        }
    }

    /// Parses a locale identifier from a string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "zh-CN" => Some(LocaleId::ZhCN),
            "en-US" => Some(LocaleId::EnUS),
            _ => None,
        }
    }

    /// Returns all supported locales.
    pub fn all() -> Vec<LocaleId> {
        vec![LocaleId::ZhCN, LocaleId::EnUS]
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
        vec![
            LocaleMetadata {
                id: LocaleId::ZhCN,
                name: "简体中文".to_string(),
                english_name: "Simplified Chinese".to_string(),
                native_name: "简体中文".to_string(),
                rtl: false,
            },
            LocaleMetadata {
                id: LocaleId::EnUS,
                name: "English".to_string(),
                english_name: "English (US)".to_string(),
                native_name: "English".to_string(),
                rtl: false,
            },
        ]
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
