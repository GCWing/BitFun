use serde::{Deserialize, Serialize};

pub const USER_QUERY_TAG: &str = "user_query";
pub const SYSTEM_REMINDER_TAG: &str = "system_reminder";
const LEGACY_SYSTEM_REMINDER_TAG: &str = "system-reminder";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptBlockKind {
    UserQuery,
    SystemReminder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptBlock {
    pub kind: PromptBlockKind,
    pub text: String,
}

impl PromptBlock {
    pub fn user_query(text: impl Into<String>) -> Self {
        Self {
            kind: PromptBlockKind::UserQuery,
            text: text.into(),
        }
    }

    pub fn system_reminder(text: impl Into<String>) -> Self {
        Self {
            kind: PromptBlockKind::SystemReminder,
            text: text.into(),
        }
    }

    pub fn render(&self) -> String {
        match self.kind {
            PromptBlockKind::UserQuery => wrap_tag(USER_QUERY_TAG, &self.text),
            PromptBlockKind::SystemReminder => wrap_tag(SYSTEM_REMINDER_TAG, &self.text),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptEnvelope {
    pub blocks: Vec<PromptBlock>,
}

impl PromptEnvelope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_block(&mut self, block: PromptBlock) {
        self.blocks.push(block);
    }

    pub fn push_user_query(&mut self, text: impl Into<String>) {
        self.push_block(PromptBlock::user_query(text));
    }

    pub fn push_system_reminder(&mut self, text: impl Into<String>) {
        self.push_block(PromptBlock::system_reminder(text));
    }

    pub fn render(&self) -> String {
        self.blocks
            .iter()
            .map(PromptBlock::render)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn render_user_query(text: &str) -> String {
    PromptBlock::user_query(text).render()
}

pub fn render_system_reminder(text: &str) -> String {
    PromptBlock::system_reminder(text).render()
}

pub fn has_prompt_markup(raw: &str) -> bool {
    let trimmed = raw.trim_start();
    trimmed.starts_with(&opening_tag(USER_QUERY_TAG))
        || trimmed.starts_with(&opening_tag(SYSTEM_REMINDER_TAG))
        || trimmed.starts_with(&opening_tag(LEGACY_SYSTEM_REMINDER_TAG))
}

pub fn is_system_reminder_only(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with(&opening_tag(SYSTEM_REMINDER_TAG))
        || trimmed.starts_with(&opening_tag(LEGACY_SYSTEM_REMINDER_TAG))
}

pub fn strip_prompt_markup(raw: &str) -> String {
    let text = raw.trim();
    let inner = extract_tag_content(text, USER_QUERY_TAG)
        .map(|content| content.trim().to_string())
        .unwrap_or_else(|| strip_after_first_system_reminder(text).trim().to_string());
    strip_after_first_system_reminder(&inner).trim().to_string()
}

fn wrap_tag(tag: &str, text: &str) -> String {
    format!("<{tag}>\n{text}\n</{tag}>")
}

fn opening_tag(tag: &str) -> String {
    format!("<{tag}>")
}

fn closing_tag(tag: &str) -> String {
    format!("</{tag}>")
}

fn extract_tag_content<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = opening_tag(tag);
    let close = closing_tag(tag);
    let start = text.find(&open)?;
    let after_open = &text[start + open.len()..];
    let end = after_open.find(&close)?;
    Some(&after_open[..end])
}

fn strip_after_first_system_reminder(text: &str) -> &str {
    let underscore = text.find(&opening_tag(SYSTEM_REMINDER_TAG));
    let legacy = text.find(&opening_tag(LEGACY_SYSTEM_REMINDER_TAG));
    match (underscore, legacy) {
        (Some(a), Some(b)) => &text[..a.min(b)],
        (Some(a), None) => &text[..a],
        (None, Some(b)) => &text[..b],
        (None, None) => text,
    }
}
