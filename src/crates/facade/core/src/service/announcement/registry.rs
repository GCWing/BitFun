//! Local static card registry.
//!
//! Each application release can add a new Markdown file under
//! `content/features/{locale}/` to register feature announcement cards.
//! Cards are loaded at startup and matched against the running version at
//! scheduling time, so old cards are automatically ignored once the user has
//! seen them.

use super::content_loader;
use super::types::AnnouncementCard;

/// Returns all locally registered announcement cards for the given locale.
///
/// Content is sourced from `content/features/{locale}/*.md` files.
/// Falls back to `en-US` if the requested locale has no feature files.
pub fn local_cards(locale: &str) -> Vec<AnnouncementCard> {
    content_loader::load_features(locale)
}
