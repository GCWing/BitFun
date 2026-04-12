//! Announcement content loader.
//!
//! Loads tip and feature-card content from Markdown files that were embedded
//! into the binary at compile time by `build.rs`.
//!
//! Each file uses YAML front matter (delimited by `---`) followed by Markdown
//! body text:
//!
//! - **Tip files** (`tips/{locale}/NNN_id.md`): front matter carries `id`,
//!   `nth_open`, `auto_dismiss_secs`; the first `# Heading` becomes the toast
//!   title and the remaining text becomes the toast description.
//!
//! - **Feature files** (`features/{locale}/id.md`): front matter carries toast
//!   metadata and modal settings; `<!-- page -->` comments split the body into
//!   separate modal pages, each with its own `# Heading` title.

use super::types::{
    AnnouncementCard, CardSource, CardType, CompletionAction, ModalConfig, ModalPage, ModalSize,
    PageLayout, ToastConfig, TriggerCondition, TriggerRule,
};

include!(concat!(env!("OUT_DIR"), "/embedded_announcements.rs"));

const FALLBACK_LOCALE: &str = "en-US";

// ─── Front matter ────────────────────────────────────────────────────────────

/// Minimal front matter for a tip card.
#[derive(Debug)]
struct TipFrontMatter {
    id: String,
    nth_open: u64,
    auto_dismiss_secs: u64,
}

/// Minimal front matter for a feature card.
#[derive(Debug)]
struct FeatureFrontMatter {
    id: String,
    /// `version_first_open` | `always` | `manual` (default: version_first_open)
    trigger: String,
    once_per_version: bool,
    delay_ms: u64,
    toast_title: String,
    toast_desc: String,
    modal_size: String,
    completion_action: String,
    auto_dismiss_ms: Option<u64>,
    priority: i32,
}

/// Split raw file content into (front_matter_text, body_text).
/// Returns `None` if the file does not start with `---`.
fn split_front_matter(src: &str) -> Option<(&str, &str)> {
    let src = src.trim_start();
    if !src.starts_with("---") {
        return None;
    }
    let after_open = &src[3..];
    // Skip optional newline immediately after opening `---`
    let after_open = after_open
        .trim_start_matches('\n')
        .trim_start_matches("\r\n");
    let close = after_open.find("\n---")?;
    let fm = &after_open[..close];
    let body = &after_open[close + 4..]; // skip "\n---"
    Some((fm, body))
}

/// Parse a simple `key: value` YAML line.
fn parse_kv(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    let key = line[..idx].trim();
    let val = line[idx + 1..].trim().trim_matches('"');
    Some((key, val))
}

fn parse_tip_front_matter(fm: &str) -> Option<TipFrontMatter> {
    let mut id = String::new();
    let mut nth_open: u64 = 1;
    let mut auto_dismiss_secs: u64 = 10;

    for line in fm.lines() {
        if let Some((k, v)) = parse_kv(line) {
            match k {
                "id" => id = v.to_string(),
                "nth_open" => nth_open = v.parse().unwrap_or(1),
                "auto_dismiss_secs" => auto_dismiss_secs = v.parse().unwrap_or(10),
                _ => {}
            }
        }
    }

    if id.is_empty() {
        return None;
    }
    Some(TipFrontMatter {
        id,
        nth_open,
        auto_dismiss_secs,
    })
}

fn parse_feature_front_matter(fm: &str) -> Option<FeatureFrontMatter> {
    let mut id = String::new();
    let mut trigger = "version_first_open".to_string();
    let mut once_per_version = true;
    let mut delay_ms: u64 = 2000;
    let mut toast_title = String::new();
    let mut toast_desc = String::new();
    let mut modal_size = "lg".to_string();
    let mut completion_action = "never_show_again".to_string();
    let mut auto_dismiss_ms: Option<u64> = None;
    let mut priority: i32 = 0;

    for line in fm.lines() {
        if let Some((k, v)) = parse_kv(line) {
            match k {
                "id" => id = v.to_string(),
                "trigger" => trigger = v.to_string(),
                "once_per_version" => once_per_version = v == "true",
                "delay_ms" => delay_ms = v.parse().unwrap_or(2000),
                "toast_title" => toast_title = v.to_string(),
                "toast_desc" => toast_desc = v.to_string(),
                "modal_size" => modal_size = v.to_string(),
                "completion_action" => completion_action = v.to_string(),
                "auto_dismiss_ms" => auto_dismiss_ms = v.parse().ok(),
                "priority" => priority = v.parse().unwrap_or(0),
                _ => {}
            }
        }
    }

    if id.is_empty() || toast_title.is_empty() {
        return None;
    }
    Some(FeatureFrontMatter {
        id,
        trigger,
        once_per_version,
        delay_ms,
        toast_title,
        toast_desc,
        modal_size,
        completion_action,
        auto_dismiss_ms,
        priority,
    })
}

// ─── Body parsing ─────────────────────────────────────────────────────────────

/// Parse a page body: extract the first `# Heading` as title, the rest as body.
/// Returns (title, body_markdown).
fn parse_page_body(text: &str) -> (String, String) {
    let text = text.trim();
    let mut lines = text.lines();
    let mut title = String::new();
    let mut body_lines: Vec<&str> = Vec::new();
    let mut found_title = false;

    for line in &mut lines {
        if !found_title && line.starts_with("# ") {
            title = line[2..].trim().to_string();
            found_title = true;
        } else {
            body_lines.push(line);
        }
    }

    let body = body_lines.join("\n").trim().to_string();
    (title, body)
}

/// Split feature body into pages using `<!-- page -->` as the delimiter.
fn split_pages(body: &str) -> Vec<&str> {
    body.split("<!-- page -->").collect()
}

// ─── Card builders ────────────────────────────────────────────────────────────

fn build_tip_card(fm: TipFrontMatter, body: &str) -> AnnouncementCard {
    let (title, desc) = parse_page_body(body);
    AnnouncementCard {
        id: format!("tip_{}", fm.id),
        card_type: CardType::Tip,
        source: CardSource::BuiltinTip,
        app_version: None,
        priority: -10,
        trigger: TriggerRule {
            condition: TriggerCondition::AppNthOpen { n: fm.nth_open },
            delay_ms: 5000,
            once_per_version: false,
        },
        toast: ToastConfig {
            icon: String::new(),
            title,
            description: desc,
            action_label: "announcements.common.got_it".to_string(),
            dismissible: true,
            auto_dismiss_ms: Some(fm.auto_dismiss_secs * 1000),
        },
        modal: None,
        expires_at: None,
    }
}

fn build_feature_card(fm: FeatureFrontMatter, body: &str) -> AnnouncementCard {
    let trigger_condition = match fm.trigger.as_str() {
        "always" => TriggerCondition::Always,
        "manual" => TriggerCondition::Manual,
        _ => TriggerCondition::VersionFirstOpen,
    };

    let modal_size = match fm.modal_size.as_str() {
        "sm" => ModalSize::Sm,
        "md" => ModalSize::Md,
        "xl" => ModalSize::Xl,
        _ => ModalSize::Lg,
    };

    let completion_action = match fm.completion_action.as_str() {
        "dismiss" => CompletionAction::Dismiss,
        _ => CompletionAction::NeverShowAgain,
    };

    let pages: Vec<ModalPage> = split_pages(body)
        .into_iter()
        .map(|page_text| {
            let (title, body_md) = parse_page_body(page_text);
            ModalPage {
                layout: PageLayout::TextOnly,
                title,
                body: body_md,
                media: None,
            }
        })
        .filter(|p| !p.title.is_empty())
        .collect();

    AnnouncementCard {
        id: fm.id,
        card_type: CardType::Feature,
        source: CardSource::Local,
        app_version: None,
        priority: fm.priority,
        trigger: TriggerRule {
            condition: trigger_condition,
            delay_ms: fm.delay_ms,
            once_per_version: fm.once_per_version,
        },
        toast: ToastConfig {
            icon: String::new(),
            title: fm.toast_title,
            description: fm.toast_desc,
            action_label: "announcements.common.learn_more".to_string(),
            dismissible: true,
            auto_dismiss_ms: fm.auto_dismiss_ms,
        },
        modal: if pages.is_empty() {
            None
        } else {
            Some(ModalConfig {
                size: modal_size,
                closable: true,
                completion_action,
                pages,
            })
        },
        expires_at: None,
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Resolve the best available locale key prefix: tries `locale` first, then
/// falls back to `FALLBACK_LOCALE`.
fn resolve_locale<'a>(category: &str, locale: &'a str) -> &'a str {
    let probe = format!("{}/{}/", category, locale);
    let has_any = EMBEDDED_ANNOUNCEMENTS
        .keys()
        .any(|k| k.starts_with(probe.as_str()));
    if has_any {
        locale
    } else {
        FALLBACK_LOCALE
    }
}

/// Load all built-in tip cards for the given locale.
///
/// Falls back to `en-US` if the requested locale has no tip files.
pub fn load_tips(locale: &str) -> Vec<AnnouncementCard> {
    let effective = resolve_locale("tips", locale);
    let prefix = format!("tips/{}/", effective);

    let mut cards: Vec<AnnouncementCard> = EMBEDDED_ANNOUNCEMENTS
        .iter()
        .filter(|(k, _)| k.starts_with(prefix.as_str()))
        .filter_map(|(_, content)| {
            let (fm_text, body) = split_front_matter(content)?;
            let fm = parse_tip_front_matter(fm_text)?;
            Some(build_tip_card(fm, body))
        })
        .collect();

    // Stable sort by nth_open so tips fire in the intended order.
    cards.sort_by_key(|c| {
        if let TriggerCondition::AppNthOpen { n } = c.trigger.condition {
            n
        } else {
            u64::MAX
        }
    });
    cards
}

/// Load all locally registered feature cards for the given locale.
///
/// Falls back to `en-US` if the requested locale has no feature files.
pub fn load_features(locale: &str) -> Vec<AnnouncementCard> {
    let effective = resolve_locale("features", locale);
    let prefix = format!("features/{}/", effective);

    EMBEDDED_ANNOUNCEMENTS
        .iter()
        .filter(|(k, _)| k.starts_with(prefix.as_str()))
        .filter_map(|(_, content)| {
            let (fm_text, body) = split_front_matter(content)?;
            let fm = parse_feature_front_matter(fm_text)?;
            Some(build_feature_card(fm, body))
        })
        .collect()
}
