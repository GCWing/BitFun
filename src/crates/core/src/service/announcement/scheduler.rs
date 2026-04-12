//! Announcement scheduling engine.
//!
//! On every application start, `AnnouncementScheduler::run` is called once.
//! It updates the persistent state, merges all card sources and returns the
//! ordered list of cards that should be presented in this session.

use super::registry::local_cards;
use super::remote::RemoteFetcher;
use super::state_store::AnnouncementStateStore;
use super::tips_pool::builtin_tips;
use super::types::{AnnouncementCard, AnnouncementState, TriggerCondition};
use crate::infrastructure::app_paths::PathManager;
use crate::util::errors::BitFunResult;
use log::{debug, info};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared handle returned to `AppState`.
pub type AnnouncementSchedulerRef = Arc<AnnouncementScheduler>;

pub struct AnnouncementScheduler {
    store: AnnouncementStateStore,
    remote_fetcher: RemoteFetcher,
    state: RwLock<AnnouncementState>,
    current_version: String,
}

impl AnnouncementScheduler {
    /// Create a new scheduler and load persisted state from disk.
    pub async fn new(path_manager: &Arc<PathManager>) -> BitFunResult<Self> {
        let store = AnnouncementStateStore::new(path_manager);
        let remote_fetcher = RemoteFetcher::new(path_manager);
        let state = store.load().await?;
        let current_version = env!("CARGO_PKG_VERSION").to_string();
        Ok(Self {
            store,
            remote_fetcher,
            state: RwLock::new(state),
            current_version,
        })
    }

    /// Run scheduling: update version / open-count, collect and filter cards.
    ///
    /// `locale` is used to load the correct language variant of announcement
    /// content (e.g. `"zh-CN"` or `"en-US"`).  Falls back to `en-US` when the
    /// requested locale has no files.
    ///
    /// Should be called once during application startup (non-blocking – awaited
    /// inside the Tauri `setup` callback or from `announcement_api`).
    pub async fn run(&self, locale: &str) -> BitFunResult<Vec<AnnouncementCard>> {
        let mut state = self.state.write().await;
        let is_version_first_open = state.last_seen_version != self.current_version;

        if is_version_first_open {
            info!(
                "Announcement scheduler: version upgrade detected ({} → {}); resetting dismissed_ids",
                state.last_seen_version, self.current_version
            );
            state.last_seen_version = self.current_version.clone();
            // On version upgrade the user should see version-specific cards again
            // even if they were dismissed in a previous sub-release.
            state.dismissed_ids.clear();
        }

        state.app_open_count += 1;
        let open_count = state.app_open_count;
        self.store.save(&state).await?;
        drop(state);

        // Kick off remote fetch in the background (does not block card delivery).
        let remote_fetcher = self.remote_fetcher.clone();
        tokio::spawn(async move {
            remote_fetcher.fetch_if_stale().await;
        });

        // Merge all card sources.
        let mut all_cards: Vec<AnnouncementCard> = Vec::new();
        all_cards.extend(local_cards(locale));
        all_cards.extend(builtin_tips(locale));
        all_cards.extend(self.remote_fetcher.cached_cards().await);

        debug!(
            "Announcement scheduler: {} total cards before filtering",
            all_cards.len()
        );

        let state = self.state.read().await;
        let now = chrono::Utc::now().timestamp();

        let mut eligible: Vec<AnnouncementCard> = all_cards
            .into_iter()
            .filter(|card| self.is_eligible(card, &state, open_count, is_version_first_open, now))
            .collect();

        // Highest priority first; stable sort preserves source order for equal priorities.
        eligible.sort_by(|a, b| b.priority.cmp(&a.priority));

        debug!(
            "Announcement scheduler: {} cards eligible for display",
            eligible.len()
        );

        Ok(eligible)
    }

    /// Immediately mark a card as eligible for manual triggering (bypasses normal filters).
    ///
    /// `locale` is forwarded to the content loader for language selection.
    pub async fn trigger_card(&self, id: &str, locale: &str) -> Option<AnnouncementCard> {
        let all: Vec<AnnouncementCard> = local_cards(locale)
            .into_iter()
            .chain(builtin_tips(locale))
            .chain(self.remote_fetcher.cached_cards().await)
            .collect();
        all.into_iter().find(|c| c.id == id)
    }

    /// Record that the user has seen (opened the modal for) a card.
    pub async fn mark_seen(&self, id: &str) -> BitFunResult<()> {
        let mut state = self.state.write().await;
        state.seen_ids.insert(id.to_string());
        self.store.save(&state).await
    }

    /// Dismiss a card for the current version cycle.
    pub async fn dismiss(&self, id: &str) -> BitFunResult<()> {
        let mut state = self.state.write().await;
        state.dismissed_ids.insert(id.to_string());
        self.store.save(&state).await
    }

    /// Permanently suppress a card.
    pub async fn never_show(&self, id: &str) -> BitFunResult<()> {
        let mut state = self.state.write().await;
        state.never_show_ids.insert(id.to_string());
        self.store.save(&state).await
    }

    // ──────────────────────────────────────────────────────────────────────
    // Private helpers
    // ──────────────────────────────────────────────────────────────────────

    fn is_eligible(
        &self,
        card: &AnnouncementCard,
        state: &AnnouncementState,
        open_count: u64,
        is_version_first_open: bool,
        now: i64,
    ) -> bool {
        // Never-show list takes absolute precedence.
        if state.never_show_ids.contains(&card.id) {
            return false;
        }

        // Dismissed within the current version cycle.
        if state.dismissed_ids.contains(&card.id) {
            return false;
        }

        // Expired remote card.
        if let Some(expires) = card.expires_at {
            if now >= expires {
                return false;
            }
        }

        // Version restriction.
        if let Some(required_version) = &card.app_version {
            if required_version != &self.current_version {
                return false;
            }
        }

        // once_per_version: skip if already seen in this version.
        if card.trigger.once_per_version && state.seen_ids.contains(&card.id) {
            return false;
        }

        // Trigger condition check.
        match &card.trigger.condition {
            TriggerCondition::VersionFirstOpen => is_version_first_open,
            TriggerCondition::AppNthOpen { n } => open_count == *n,
            TriggerCondition::Always => true,
            TriggerCondition::Manual => false, // only via explicit `trigger_announcement`
            TriggerCondition::FeatureUsed { .. } => false, // handled programmatically
        }
    }
}
