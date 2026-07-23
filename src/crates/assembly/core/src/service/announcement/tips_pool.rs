//! Built-in tips pool.
//!
//! Tips are lightweight cards (no modal) that teach users about features they
//! may have missed.  Content is loaded from Markdown files embedded at compile
//! time via `content_loader`.

use super::content_loader;
use super::types::AnnouncementCard;

/// Returns the full list of built-in tip cards for the given locale.
///
/// Content is sourced from `content/tips/{locale}/*.md` files.
/// Falls back to `en-US` if the requested locale has no tip files.
pub fn builtin_tips(locale: &str) -> Vec<AnnouncementCard> {
    content_loader::load_tips(locale)
}
