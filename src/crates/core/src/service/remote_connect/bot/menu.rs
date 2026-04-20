//! Unified menu model shared by all IM bot adapters.
//!
//! The command router builds a [`MenuView`] for every reply.  Each platform
//! adapter renders the view to its native primitive: Telegram inline
//! keyboards, Feishu interactive cards, or WeChat numbered text lines.
//!
//! There is intentionally no per-platform menu state in this module — menu
//! semantics live in `command_router::dispatch_im_bot_command_inner`.

use serde::{Deserialize, Serialize};

use super::locale::{strings_for, BotLanguage, BotStrings};

/// Visual style of a menu item.  Adapters map this to their own primitive
/// (e.g. Telegram has no styling, Feishu uses a `type` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuItemStyle {
    Primary,
    Default,
    Danger,
}

/// One row in a [`MenuView`].
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Short, button-friendly label shown to the user.  Should be ≤ 14 chars.
    pub label: String,
    /// Real command string the bot will execute when the item is selected.
    /// For platforms with native buttons this becomes `callback_data`; for
    /// WeChat it is mapped via [`MenuView::numeric_commands`] for `1` ~ `n`
    /// numeric replies.
    pub command: String,
    pub style: MenuItemStyle,
}

impl MenuItem {
    pub fn primary(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            style: MenuItemStyle::Primary,
        }
    }
    pub fn default(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            style: MenuItemStyle::Default,
        }
    }
    pub fn danger(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            style: MenuItemStyle::Danger,
        }
    }
}

/// Unified, platform-agnostic menu/reply view.
///
/// Always provide a short `title` and at most 5 items.  The body field is
/// optional and used for context like "Current session: …" or last dialog
/// playback.
#[derive(Debug, Clone, Default)]
pub struct MenuView {
    /// One-line context header (≤ 30 chars target).
    pub title: String,
    /// Optional secondary body text.
    pub body: Option<String>,
    pub items: Vec<MenuItem>,
    /// Optional footer hint shown below items (telegram/feishu silently
    /// drop this; weixin shows it as the last text line).
    pub footer_hint: Option<String>,
}

impl MenuView {
    pub fn plain(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            items: Vec::new(),
            footer_hint: None,
        }
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn with_items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    pub fn with_footer(mut self, hint: impl Into<String>) -> Self {
        self.footer_hint = Some(hint.into());
        self
    }

    pub fn push_item(&mut self, item: MenuItem) {
        self.items.push(item);
    }

    /// Commands corresponding to numeric replies `1..=items.len()`.
    pub fn numeric_commands(&self) -> Vec<String> {
        self.items.iter().map(|i| i.command.clone()).collect()
    }

    /// Render the menu as plain text suitable for IM platforms without
    /// native buttons (e.g. WeChat iLink).
    pub fn render_plain_text(&self, language: BotLanguage) -> String {
        let s = strings_for(language);
        let mut out = String::new();
        if !self.title.is_empty() {
            out.push_str(&self.title);
        }
        if let Some(body) = &self.body {
            if !body.is_empty() {
                if !out.is_empty() {
                    out.push_str("\n\n");
                }
                out.push_str(body);
            }
        }
        if !self.items.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            for (i, item) in self.items.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                out.push_str(&format!("{} {}", i + 1, item.label));
            }
        }
        let hint = self
            .footer_hint
            .clone()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| {
                if self.items.is_empty() {
                    String::new()
                } else {
                    s.footer_reply_or_menu.to_string()
                }
            });
        if !hint.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&hint);
        }
        out
    }

    /// Render the title and optional body as a single text block, used by
    /// adapters with native buttons (Telegram / Feishu) where the items are
    /// shown separately as buttons.
    pub fn render_text_block(&self) -> String {
        let mut out = String::new();
        if !self.title.is_empty() {
            out.push_str(&self.title);
        }
        if let Some(body) = &self.body {
            if !body.is_empty() {
                if !out.is_empty() {
                    out.push_str("\n\n");
                }
                out.push_str(body);
            }
        }
        if let Some(hint) = &self.footer_hint {
            if !hint.is_empty() {
                if !out.is_empty() {
                    out.push_str("\n\n");
                }
                out.push_str(hint);
            }
        }
        if out.is_empty() {
            // Telegram refuses empty messages; fall back to a single space.
            " ".to_string()
        } else {
            out
        }
    }
}

/// Common menu builder helpers used by command router and platforms.
pub mod build {
    use super::*;

    /// Append the standard `back to main menu` item if not already present.
    pub fn with_back(view: &mut MenuView, s: &BotStrings) {
        if !view.items.iter().any(|i| i.command == "/menu") {
            view.push_item(MenuItem::default(s.item_back, "/menu"));
        }
    }
}
