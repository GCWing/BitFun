//! Backend locale registry.
//!
//! Add backend-supported locales here first. The i18n service, locale metadata
//! APIs, and model-facing language instructions all derive from this table.

use super::types::LocaleId;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy)]
pub struct LocaleRegistryEntry {
    pub id: LocaleId,
    pub code: &'static str,
    pub name: &'static str,
    pub english_name: &'static str,
    pub native_name: &'static str,
    pub rtl: bool,
    pub model_language_name: &'static str,
    pub short_model_instruction: &'static str,
    pub aliases: &'static [&'static str],
    pub fluent_source: &'static str,
}

pub const LOCALE_REGISTRY: &[LocaleRegistryEntry] = &[
    LocaleRegistryEntry {
        id: LocaleId::ZhCN,
        code: "zh-CN",
        name: "简体中文",
        english_name: "Simplified Chinese",
        native_name: "简体中文",
        rtl: false,
        model_language_name: "Simplified Chinese",
        short_model_instruction: "使用简体中文",
        aliases: &["zh", "zh-Hans", "zh-CN"],
        fluent_source: include_str!("../../../locales/zh-CN.ftl"),
    },
    LocaleRegistryEntry {
        id: LocaleId::ZhTW,
        code: "zh-TW",
        name: "繁體中文",
        english_name: "Traditional Chinese",
        native_name: "繁體中文",
        rtl: false,
        model_language_name: "Traditional Chinese",
        short_model_instruction: "使用繁體中文",
        aliases: &["zh-TW", "zh-Hant", "zh-HK", "zh-MO"],
        fluent_source: include_str!("../../../locales/zh-TW.ftl"),
    },
    LocaleRegistryEntry {
        id: LocaleId::EnUS,
        code: "en-US",
        name: "English",
        english_name: "English (US)",
        native_name: "English",
        rtl: false,
        model_language_name: "English",
        short_model_instruction: "Use English",
        aliases: &["en", "en-US"],
        fluent_source: include_str!("../../../locales/en-US.ftl"),
    },
];

static SORTED_LOCALE_ALIASES: LazyLock<Vec<(&'static LocaleRegistryEntry, &'static str)>> =
    LazyLock::new(|| {
        let mut aliases = LOCALE_REGISTRY
            .iter()
            .flat_map(|entry| entry.aliases.iter().map(move |alias| (entry, *alias)))
            .collect::<Vec<_>>();
        aliases.sort_by(|(_, a), (_, b)| b.len().cmp(&a.len()));
        aliases
    });

pub fn locale_entry(id: LocaleId) -> &'static LocaleRegistryEntry {
    LOCALE_REGISTRY
        .iter()
        .find(|entry| entry.id == id)
        .expect("LocaleId missing from locale registry")
}

pub fn locale_entry_from_code(code: &str) -> Option<&'static LocaleRegistryEntry> {
    let normalized = code.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    LOCALE_REGISTRY
        .iter()
        .find(|entry| entry.code.eq_ignore_ascii_case(&normalized))
        .or_else(|| {
            // Match the longest alias first so script aliases like `zh-Hant`
            // win over broad prefixes like `zh`, and compute that priority once.
            SORTED_LOCALE_ALIASES.iter().find_map(|(entry, alias)| {
                let alias = alias.to_ascii_lowercase();
                (normalized == alias || normalized.starts_with(&format!("{alias}-")))
                    .then_some(*entry)
            })
        })
}
