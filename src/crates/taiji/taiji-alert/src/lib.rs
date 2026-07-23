//! Taiji alert module — multi-channel alarm notification.
//!
//! Provides alert level classification, configuration, message types,
//! and channel-specific alerters (Feishu webhook, email, desktop).
//!
//! # Relationship to BitFun AgenticEvent
//!
//! `taiji-alert` is a **self-contained alert infrastructure** designed for
//! trading-system operational monitoring (cron job failures, heartbeat timeouts,
//! pipeline errors). It is independent of BitFun's agentic event system and
//! focuses on *human-operator notification* rather than *agent-to-agent
//! messaging*.
//!
//! ## AlertLevel → AgenticEventPriority mapping
//!
//! When an alert needs to be bridged into BitFun's event bus (e.g. surfaced in
//! the desktop UI or relayed to a remote session), the severity levels map as
//! follows:
//!
//! | `AlertLevel`   | `AgenticEventPriority` | Rationale |
//! |----------------|------------------------|-----------|
//! | `Heartbeat`    | `Low`                  | Informational liveness check; non-urgent. |
//! | `Warn`         | `Normal`               | Degradation warning; does not block trading. |
//! | `Error`        | `High`                 | Job/pipeline failure requiring operator attention. |
//! | `Critical`     | `Critical`             | System-wide failure or data-loss risk; immediate action needed. |
//!
//! ## AlertMessage → AgenticEvent mapping
//!
//! An [`AlertMessage`] can be projected into a BitFun [`AgenticEvent::SystemError`]
//! variant for consumption by the desktop UI or remote relay:
//!
//! ```ignore
//! // Conceptual bridge (not compiled — bitfun-events is not a dependency):
//! fn to_system_error(msg: &AlertMessage) -> bitfun_events::AgenticEvent {
//!     bitfun_events::AgenticEvent::SystemError {
//!         session_id: None,
//!         error: format!("[{}] {}: {}", msg.level.label_cn(), msg.title, msg.body),
//!         recoverable: msg.level < AlertLevel::Critical,
//!     }
//! }
//! ```
//!
//! The `SystemError` variant is the natural target because:
//! - It carries an error string (maps to alert title + body)
//! - Its `recoverable` flag distinguishes Critical (unrecoverable) from lower levels
//! - It does not require a session context (alerts are system-wide)
//!
//! ## Cross-reference
//!
//! - [`bitfun_events::AgenticEvent`] — 35+ variants covering session lifecycle,
//!   dialog turns, tool execution, token usage, context compression, and Deep Review.
//! - [`bitfun_events::AgenticEventPriority`] — 4-tier priority (Critical/High/Normal/Low)
//!   used for event ordering and UI badge severity.
//! - [`bitfun_events::AgenticEventEnvelope`] — wraps an event with a unique id,
//!   priority, and timestamp for ordered delivery through the event bus.
//!
//! For trading-system alerts that *must* flow through the BitFun event bus
//! (e.g. to show a desktop notification or to be relayed to a remote workspace),
//! bridge code should construct an `AgenticEvent::SystemError` with the
//! appropriate priority from the mapping table above.

pub mod alerters;
pub mod heartbeat;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::warn;

/// Alert severity level, ordered from lowest to highest urgency.
///
/// # BitFun event priority mapping
///
/// Each level corresponds to a [`bitfun_events::AgenticEventPriority`]:
///
/// - `Heartbeat` → `AgenticEventPriority::Low` — informational liveness check.
/// - `Warn` → `AgenticEventPriority::Normal` — degradation warning.
/// - `Error` → `AgenticEventPriority::High` — job/pipeline failure.
/// - `Critical` → `AgenticEventPriority::Critical` — system-wide failure.
///
/// See the [module-level documentation](self) for the full mapping table and
/// bridge guidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    Heartbeat,
    Warn,
    Error,
    Critical,
}

/// SMTP configuration for email alerts.
//
// TODO(P2-1): Deduplicate with `taiji-growth::types::SmtpConfig`.
// Both crates define nearly identical SMTP configs; the growth version
// adds `from_name` + `from_email` fields. Extract a shared `SmtpConfig`
// into `taiji-engine` or a new `taiji-shared` crate so alert and growth
// can both depend on a single definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// SMTP server hostname.
    pub host: String,
    /// SMTP server port.
    pub port: u16,
    /// SMTP authentication username.
    pub username: String,
    /// SMTP authentication password — excluded from serialization.
    #[serde(skip_serializing)]
    pub password: String,
    /// Whether to use STARTTLS.
    pub use_tls: bool,
}

/// Global alert configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Feishu custom bot webhook URL (e.g. https://open.feishu.cn/open-apis/bot/v2/hook/...).
    pub feishu_webhook_url: String,
    /// Optional SMTP configuration for email alerts.
    pub smtp_config: Option<SmtpConfig>,
    /// Minimum alert level to actually send. Levels below this threshold are suppressed.
    pub alert_level: AlertLevel,
    /// Heartbeat interval in minutes. If no job executes within this window, a heartbeat
    /// alert is emitted.
    pub heartbeat_interval_min: u32,
    /// Aggregation window in seconds. Alerts of the same type within this window are
    /// merged into a single message with an incrementing count.
    pub aggregation_window_secs: u32,
}

/// A single alert message ready for delivery.
///
/// # BitFun event envelope mapping
///
/// [`AlertMessage`] is a self-contained alert payload. To bridge into BitFun's
/// event bus, project it into an [`AgenticEvent::SystemError`](bitfun_events::AgenticEvent::SystemError)
/// wrapped in an [`AgenticEventEnvelope`](bitfun_events::AgenticEventEnvelope):
///
/// | Field         | Maps to                                      |
/// |---------------|----------------------------------------------|
/// | `level`       | `AgenticEventPriority` (see [`AlertLevel`] mapping) |
/// | `title`       | First line of the `SystemError.error` string |
/// | `body`        | Remaining detail in `SystemError.error`      |
/// | `source`      | Prepended to the error string as context     |
/// | `timestamp`   | `AgenticEventEnvelope.timestamp` (`SystemTime`) |
/// | `count`       | Included in the error string (e.g. "(×N)")   |
///
/// The `SystemError.recoverable` flag is `true` for levels below `Critical`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMessage {
    /// Severity level of this alert.
    pub level: AlertLevel,
    /// Short title for the alert (e.g. instrument + failure summary).
    pub title: String,
    /// Detailed body in Markdown format.
    pub body: String,
    /// Source component that generated the alert (e.g. "cron:job_id", "heartbeat").
    pub source: String,
    /// Timestamp when the alert was created.
    pub timestamp: DateTime<Utc>,
    /// Number of aggregated occurrences represented by this message.
    pub count: u32,
}

impl AlertLevel {
    /// Color token for visual differentiation in alert cards and UI.
    pub fn color(&self) -> &'static str {
        match self {
            AlertLevel::Heartbeat => "blue",
            AlertLevel::Warn => "yellow",
            AlertLevel::Error => "orange",
            AlertLevel::Critical => "red",
        }
    }

    /// Human-readable label in Chinese.
    pub fn label_cn(&self) -> &'static str {
        match self {
            AlertLevel::Heartbeat => "心跳",
            AlertLevel::Warn => "警告",
            AlertLevel::Error => "错误",
            AlertLevel::Critical => "严重",
        }
    }
}

/// Tracks recent alerts for aggregation.
#[derive(Debug, Clone)]
struct RecentAlert {
    source: String,
    level: AlertLevel,
    first_at: DateTime<Utc>,
    count: u32,
}

/// Central alert dispatcher with three-channel routing and aggregation.
///
/// Routes alerts by severity:
/// - `Warn` → Desktop only
/// - `Error` → Desktop + Feishu
/// - `Critical` → Desktop + Feishu + Email
/// - `Heartbeat` → Feishu only
///
/// Alerts of the same (`source`, `level`) within the configured aggregation
/// window are merged into a single message with an incrementing count.
pub struct AlertManager {
    config: AlertConfig,
    min_level: Mutex<AlertLevel>,
    feishu: alerters::FeishuWebhookAlerter,
    desktop: alerters::DesktopAlerter,
    email: Option<Arc<alerters::EmailAlerter>>,
    /// Aggregation buffer: recent alerts keyed by (source, level).
    recent: Mutex<Vec<RecentAlert>>,
}

impl AlertManager {
    /// Create a new AlertManager.
    ///
    /// `desktop_notifier` is a callback that delivers native desktop notifications.
    /// `email_alerter` is optional; when `None`, email alerts are silently skipped.
    pub fn new(
        config: AlertConfig,
        desktop_notifier: alerters::DesktopNotifyFn,
        email_alerter: Option<alerters::EmailAlerter>,
    ) -> Self {
        let min_level = config.alert_level;
        Self {
            feishu: alerters::FeishuWebhookAlerter::new(config.feishu_webhook_url.clone()),
            desktop: alerters::DesktopAlerter::new(desktop_notifier),
            email: email_alerter.map(Arc::new),
            config,
            min_level: Mutex::new(min_level),
            recent: Mutex::new(Vec::new()),
        }
    }

    /// Change the minimum alert level at runtime.
    pub fn set_level(&self, level: AlertLevel) {
        if let Ok(mut min) = self.min_level.lock() {
            *min = level;
        }
    }

    /// Submit an alert for delivery.
    ///
    /// The alert is first checked against the minimum level threshold, then
    /// aggregated with recent alerts of the same source and level, and finally
    /// routed to the appropriate notification channels.
    pub fn alert(&self, mut msg: AlertMessage) {
        let min_level = *self.min_level.lock().unwrap_or_else(|e| {
            warn!("AlertManager min_level lock poisoned: {}", e);
            e.into_inner()
        });

        if msg.level < min_level {
            return;
        }

        // Aggregation: merge with existing alert of same (source, level) if within the window.
        if let Ok(mut recent) = self.recent.lock() {
            let window = Duration::seconds(self.config.aggregation_window_secs as i64);
            let cutoff = msg.timestamp - window;

            // Prune expired entries.
            recent.retain(|r| r.first_at >= cutoff);

            // Try to merge with an existing entry.
            if let Some(existing) = recent
                .iter_mut()
                .find(|r| r.source == msg.source && r.level == msg.level)
            {
                existing.count += 1;
                msg.count = existing.count;
                msg.timestamp = existing.first_at; // Use first occurrence timestamp.
            } else {
                let entry = RecentAlert {
                    source: msg.source.clone(),
                    level: msg.level,
                    first_at: msg.timestamp,
                    count: 1,
                };
                recent.push(entry);
            }
        }

        // Route to channels based on severity.
        self.dispatch(&msg);
    }

    /// Route an alert to the appropriate channels.
    fn dispatch(&self, msg: &AlertMessage) {
        match msg.level {
            AlertLevel::Warn => {
                self.desktop.send(msg);
            }
            AlertLevel::Error => {
                self.desktop.send(msg);
                self.send_feishu(msg);
            }
            AlertLevel::Critical => {
                self.desktop.send(msg);
                self.send_feishu(msg);
                self.send_email(msg);
            }
            AlertLevel::Heartbeat => {
                self.send_feishu(msg);
            }
        }
    }

    fn send_feishu(&self, msg: &AlertMessage) {
        let feishu = self.feishu.clone();
        let msg = msg.clone();
        // TODO(P2-6): Replace `Handle::try_current()` + `spawn` with an injected
        // async runtime handle or channel. The current pattern silently drops
        // delivery when no tokio runtime is active (e.g. sync tests), which can
        // mask real failures. BitFun's `EventEmitter` / `EventRouter` provide
        // runtime-aware dispatch without manual Handle probing.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    if let Err(e) = feishu.send(&msg).await {
                        warn!("Failed to send Feishu alert: {}", e);
                    }
                });
            }
            Err(_) => {
                // No tokio runtime available (e.g. in sync tests) — skip async delivery.
            }
        }
    }

    fn send_email(&self, msg: &AlertMessage) {
        if let Some(ref email) = self.email {
            let email = Arc::clone(email);
            let msg = msg.clone();
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    handle.spawn(async move {
                        if let Err(e) = email.send(&msg).await {
                            warn!("Failed to send email alert: {}", e);
                        }
                    });
                }
                Err(_) => {
                    // No tokio runtime available — skip async delivery.
                }
            }
        }
    }

    /// Returns a reference to the Feishu webhook alerter (for heartbeat use).
    pub fn feishu_url(&self) -> &str {
        &self.config.feishu_webhook_url
    }

    /// Returns the heartbeat interval in minutes.
    pub fn heartbeat_interval_min(&self) -> u32 {
        self.config.heartbeat_interval_min
    }
}

impl std::fmt::Debug for AlertManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlertManager")
            .field("min_level", &self.min_level)
            .field("has_email", &self.email.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_level_ordering() {
        assert!(AlertLevel::Heartbeat < AlertLevel::Warn);
        assert!(AlertLevel::Warn < AlertLevel::Error);
        assert!(AlertLevel::Error < AlertLevel::Critical);
    }

    #[test]
    fn alert_level_serde_roundtrip() {
        let levels = vec![
            (AlertLevel::Heartbeat, "heartbeat"),
            (AlertLevel::Warn, "warn"),
            (AlertLevel::Error, "error"),
            (AlertLevel::Critical, "critical"),
        ];
        for (level, expected) in levels {
            let json = serde_json::to_string(&level).unwrap();
            assert_eq!(json, format!("\"{expected}\""));
            let roundtrip: AlertLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtrip, level);
        }
    }

    #[test]
    fn smtp_config_password_not_serialized() {
        let config = SmtpConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "user".into(),
            password: "secret".into(),
            use_tls: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("secret"));
        assert!(json.contains("smtp.example.com"));
    }

    #[test]
    fn alert_config_serialize_deserialize() {
        let config = AlertConfig {
            feishu_webhook_url: "https://open.feishu.cn/open-apis/bot/v2/hook/test".into(),
            smtp_config: None,
            alert_level: AlertLevel::Warn,
            heartbeat_interval_min: 30,
            aggregation_window_secs: 300,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AlertConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.feishu_webhook_url, config.feishu_webhook_url);
        assert_eq!(parsed.alert_level, AlertLevel::Warn);
        assert_eq!(parsed.heartbeat_interval_min, 30);
        assert_eq!(parsed.aggregation_window_secs, 300);
        assert!(parsed.smtp_config.is_none());
    }

    #[test]
    fn alert_message_fields() {
        let msg = AlertMessage {
            level: AlertLevel::Error,
            title: "测试告警".into(),
            body: "管道执行失败，请检查。".into(),
            source: "cron:job_001".into(),
            timestamp: Utc::now(),
            count: 3,
        };
        assert_eq!(msg.level, AlertLevel::Error);
        assert_eq!(msg.count, 3);
        assert!(msg.source.starts_with("cron:"));
    }

    // ── AlertManager tests ──

    fn make_test_msg(level: AlertLevel, source: &str) -> AlertMessage {
        AlertMessage {
            level,
            title: "测试标题".into(),
            body: "测试内容。".into(),
            source: source.into(),
            timestamp: Utc::now(),
            count: 1,
        }
    }

    fn make_test_manager() -> (
        AlertManager,
        std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    ) {
        let desktop_calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let dc = desktop_calls.clone();
        let desktop_notifier: alerters::DesktopNotifyFn =
            std::sync::Arc::new(move |title, body| {
                dc.lock().unwrap().push((title, body));
            });

        let config = AlertConfig {
            feishu_webhook_url: "https://example.com/hook/test".into(),
            smtp_config: None,
            alert_level: AlertLevel::Warn,
            heartbeat_interval_min: 30,
            aggregation_window_secs: 300,
        };

        let mgr = AlertManager::new(config, desktop_notifier, None);
        (mgr, desktop_calls)
    }

    #[test]
    fn alert_manager_routes_warn_to_desktop_only() {
        let (mgr, desktop_calls) = make_test_manager();
        mgr.alert(make_test_msg(AlertLevel::Warn, "cron:test"));

        let calls = desktop_calls.lock().unwrap();
        assert!(
            !calls.is_empty(),
            "Warn should trigger desktop notification"
        );
    }

    #[test]
    fn alert_manager_suppresses_below_min_level() {
        let (mgr, desktop_calls) = make_test_manager();
        // Heartbeat is below the min level (Warn), should be suppressed.
        mgr.alert(make_test_msg(AlertLevel::Heartbeat, "heartbeat"));

        let calls = desktop_calls.lock().unwrap();
        assert!(
            calls.is_empty(),
            "Heartbeat below min_level should be suppressed"
        );
    }

    #[test]
    fn alert_manager_aggregates_same_source_level() {
        let (mgr, desktop_calls) = make_test_manager();
        let msg = make_test_msg(AlertLevel::Error, "cron:job_001");
        mgr.alert(msg.clone());
        mgr.alert(msg);

        let calls = desktop_calls.lock().unwrap();
        // Two alerts, second should have count=2 in the body.
        let second = &calls[1];
        assert!(
            second.1.contains("累计 2 次"),
            "Second alert should show aggregated count=2, got: {}",
            second.1
        );
    }

    #[test]
    fn alert_manager_no_aggregation_different_source() {
        let (mgr, desktop_calls) = make_test_manager();
        mgr.alert(make_test_msg(AlertLevel::Error, "cron:job_001"));
        mgr.alert(make_test_msg(AlertLevel::Error, "cron:job_002"));

        let calls = desktop_calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert!(!calls[0].1.contains("累计"));
        assert!(!calls[1].1.contains("累计"));
    }

    #[test]
    fn alert_manager_set_level_updates_min() {
        let (mgr, desktop_calls) = make_test_manager();

        // Raise min level to Critical — Warn should be suppressed.
        mgr.set_level(AlertLevel::Critical);
        mgr.alert(make_test_msg(AlertLevel::Warn, "cron:test"));

        let calls = desktop_calls.lock().unwrap();
        assert!(
            calls.is_empty(),
            "Warn should be suppressed after raising min to Critical"
        );
    }
}
