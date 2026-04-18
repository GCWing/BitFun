//! Announcement system types.
//!
//! Defines all data structures for the announcement / feature-demo / tips mechanism.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Categories of announcement cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    /// New version feature showcase.
    Feature,
    /// Operational news or blog post.
    News,
    /// Lightweight usage tip (toast only, no modal).
    Tip,
    /// Important system announcement (shown as modal without prior toast).
    Announcement,
}

/// Origin of an announcement card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardSource {
    /// Statically registered in the local binary.
    Local,
    /// Downloaded from a remote endpoint.
    Remote,
    /// Built-in tips pool.
    BuiltinTip,
}

/// Conditions that must be met before a card is shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerCondition {
    /// First launch after a version upgrade.
    VersionFirstOpen,
    /// The N-th time the application has been opened (1-indexed).
    AppNthOpen { n: u64 },
    /// A named application feature was used (supplied programmatically).
    FeatureUsed { feature: String },
    /// Must be triggered manually via `trigger_announcement`.
    Manual,
    /// Always eligible (used for announcements that should appear on every start until dismissed).
    Always,
}

/// When and how a card should be presented.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRule {
    pub condition: TriggerCondition,
    /// Milliseconds to wait after application start before displaying.
    #[serde(default)]
    pub delay_ms: u64,
    /// When true, a card is only shown once per application version.
    #[serde(default = "default_true")]
    pub once_per_version: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TriggerRule {
    fn default() -> Self {
        Self {
            condition: TriggerCondition::VersionFirstOpen,
            delay_ms: 2000,
            once_per_version: true,
        }
    }
}

/// Configuration for the bottom-left toast entry point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToastConfig {
    /// Icon identifier or emoji string (rendered by the frontend).
    pub icon: String,
    /// Toast title (i18n key or literal text).
    pub title: String,
    /// Short description shown below the title (i18n key or literal text).
    pub description: String,
    /// Label for the primary action button (i18n key or literal text).
    #[serde(default)]
    pub action_label: String,
    /// Whether the user can close the toast without acting.
    #[serde(default = "default_true")]
    pub dismissible: bool,
    /// Auto-dismiss after this many milliseconds; `None` means no auto-dismiss.
    #[serde(default)]
    pub auto_dismiss_ms: Option<u64>,
}

/// Preferred modal size.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalSize {
    Sm,
    Md,
    Lg,
    Xl,
}

impl Default for ModalSize {
    fn default() -> Self {
        ModalSize::Lg
    }
}

/// What happens when the user finishes or closes the modal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionAction {
    /// Only dismiss for this session; may reappear next launch if conditions match.
    Dismiss,
    /// Permanently suppress via `never_show_ids`.
    NeverShowAgain,
}

impl Default for CompletionAction {
    fn default() -> Self {
        CompletionAction::Dismiss
    }
}

/// Layout template for a single modal page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageLayout {
    TextOnly,
    MediaLeft,
    MediaRight,
    MediaTop,
    FullscreenMedia,
}

impl Default for PageLayout {
    fn default() -> Self {
        PageLayout::MediaTop
    }
}

/// Media asset type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Lottie,
    Video,
    Image,
    Gif,
}

/// A media asset attached to a modal page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    pub media_type: MediaType,
    /// Relative path under `public/announcements/` or an HTTPS URL.
    pub src: String,
}

/// A single page inside a feature modal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalPage {
    #[serde(default)]
    pub layout: PageLayout,
    /// Page title (i18n key or literal text).
    pub title: String,
    /// Body copy in Markdown (i18n key or literal text).
    pub body: String,
    #[serde(default)]
    pub media: Option<MediaConfig>,
}

/// Full configuration for the centre modal overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalConfig {
    #[serde(default)]
    pub size: ModalSize,
    /// Allow the user to close the modal with the × button.
    #[serde(default = "default_true")]
    pub closable: bool,
    pub pages: Vec<ModalPage>,
    #[serde(default)]
    pub completion_action: CompletionAction,
}

/// A single announcement / feature-demo card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementCard {
    /// Globally unique identifier, e.g. `feature_v1_3_0_live_app`.
    pub id: String,
    pub card_type: CardType,
    pub source: CardSource,
    /// Application version this card is associated with. `None` = any version.
    #[serde(default)]
    pub app_version: Option<String>,
    /// Higher priority cards are shown first.
    #[serde(default)]
    pub priority: i32,
    pub trigger: TriggerRule,
    pub toast: ToastConfig,
    /// If `None`, no modal is opened when the user clicks the toast action.
    #[serde(default)]
    pub modal: Option<ModalConfig>,
    /// Unix timestamp (seconds) after which the card is ignored. Remote cards only.
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// Persisted state for the announcement system.
///
/// Stored at `~/.config/bitfun/config/announcement-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnouncementState {
    /// Version string recorded when the state was last saved.
    #[serde(default)]
    pub last_seen_version: String,
    /// How many times the application has been opened.
    #[serde(default)]
    pub app_open_count: u64,
    /// IDs of cards the user has seen (action button clicked or modal opened).
    #[serde(default)]
    pub seen_ids: HashSet<String>,
    /// IDs dismissed for the current version cycle; reset on version upgrade.
    #[serde(default)]
    pub dismissed_ids: HashSet<String>,
    /// IDs the user has permanently suppressed.
    #[serde(default)]
    pub never_show_ids: HashSet<String>,
    /// Unix timestamp (seconds) of the last successful remote fetch.
    #[serde(default)]
    pub last_remote_fetch_at: Option<i64>,
}
