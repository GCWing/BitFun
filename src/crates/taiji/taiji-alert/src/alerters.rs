//! Channel-specific alerter implementations.

use crate::{AlertLevel, AlertMessage, SmtpConfig};

use lettre::message::{header, Mailbox, Message, MultiPart, SinglePart};
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, warn};

/// Sends alert messages to a Feishu custom bot via webhook.
///
/// Uses the Feishu custom bot webhook API (`POST /open-apis/bot/v2/hook/{token}`)
/// with `interactive` card messages for rich alert formatting.
///
/// TODO(tool-audit): This struct owns an independent `reqwest::Client` (default config).
/// Consider injecting a shared `reqwest::Client` via the constructor instead of
/// calling `Client::new()` here. Consolidating HTTP clients across taiji crates
/// would reduce socket/file-descriptor usage and enable uniform timeout/retry policy.
#[derive(Debug, Clone)]
pub struct FeishuWebhookAlerter {
    webhook_url: String,
    client: Client,
}

impl FeishuWebhookAlerter {
    /// TODO(tool-audit): Replace `Client::new()` with an injected shared
    /// `reqwest::Client`. See struct-level TODO for rationale.
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: Client::new(),
        }
    }

    /// Send an alert message as an interactive card to the Feishu webhook.
    ///
    /// Returns `Ok(())` on successful delivery, or an error string on failure.
    /// Non-2xx responses from Feishu are treated as errors.
    pub async fn send(&self, msg: &AlertMessage) -> Result<(), String> {
        let card = build_alert_card(msg);
        let payload = json!({
            "msg_type": "interactive",
            "card": card,
        });

        let resp = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("feishu webhook request failed: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            warn!(
                "Feishu webhook returned HTTP {}: body={}",
                status.as_u16(),
                body
            );
            return Err(format!("feishu webhook HTTP {}: {}", status.as_u16(), body));
        }

        // Feishu webhook returns {"StatusCode":0,"StatusMessage":"success"} on success.
        // Non-zero StatusCode indicates an application-level error.
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(code) = parsed.get("StatusCode").and_then(|v| v.as_i64()) {
                if code != 0 {
                    let msg_text = parsed
                        .get("StatusMessage")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    warn!("Feishu webhook API error: StatusCode={code}, StatusMessage={msg_text}");
                    return Err(format!(
                        "feishu webhook API error: StatusCode={code}, StatusMessage={msg_text}"
                    ));
                }
            }
        }

        debug!(
            "Feishu alert sent: level={:?}, title={}",
            msg.level, msg.title
        );
        Ok(())
    }
}

/// Callback type for desktop notifications.
///
/// The consumer (desktop app) provides a function that delivers a native OS
/// notification (e.g. via `send_system_notification` Tauri command).
//
// TODO(P2-6): Unify callback interfaces into a single `Alerter` trait.
// Currently DesktopNotifyFn, HeartbeatAlertFn, and ad-hoc closures serve as
// three separate callback shapes. BitFun's `EventSubscriber` trait provides a
// standard pattern — each channel (desktop/feishu/email/heartbeat) should
// implement a common trait rather than passing raw function pointers.
pub type DesktopNotifyFn = Arc<dyn Fn(String, String) + Send + Sync + 'static>;

/// Sends alert messages as native desktop notifications.
///
/// Stateless — simply invokes the provided callback with title + body.
/// The callback is expected to call the platform notification API.
#[derive(Clone)]
pub struct DesktopAlerter {
    notifier: DesktopNotifyFn,
}

impl DesktopAlerter {
    pub fn new(notifier: DesktopNotifyFn) -> Self {
        Self { notifier }
    }

    /// Deliver a desktop notification for the given alert message.
    pub fn send(&self, msg: &AlertMessage) {
        let title = format!("[太极·{}] {}", msg.level.label_cn(), msg.title);
        let body = if msg.count > 1 {
            format!("{}（累计 {} 次）", msg.body, msg.count)
        } else {
            msg.body.clone()
        };
        (self.notifier)(title, body);
    }
}

/// Sends CRITICAL alert messages via SMTP email.
///
/// Uses the `lettre` crate with tokio async transport and STARTTLS.
/// Only CRITICAL-level alerts trigger email delivery.
pub struct EmailAlerter {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    to: Mailbox,
}

impl EmailAlerter {
    /// Create a new EmailAlerter from SMTP config and recipient address.
    ///
    /// `smtp_config` provides host/port/credentials; `from_name` is the
    /// sender display name; `to_email` is the recipient address.
    pub fn new(smtp_config: &SmtpConfig, from_name: &str, to_email: &str) -> Result<Self, String> {
        let from: Mailbox = format!("{from_name} <{}>", smtp_config.username)
            .parse()
            .map_err(|e| format!("invalid from mailbox: {e}"))?;
        let to: Mailbox = to_email
            .parse()
            .map_err(|e| format!("invalid to mailbox: {e}"))?;

        let mailer = if smtp_config.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_config.host)
                .map_err(|e| format!("failed to create STARTTLS transport: {e}"))?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_config.host)
                .map_err(|e| format!("failed to create SMTP transport: {e}"))?
        }
        .port(smtp_config.port)
        .credentials((smtp_config.username.clone(), smtp_config.password.clone()).into())
        .build();

        Ok(Self { mailer, from, to })
    }

    /// Send an alert as an HTML email.
    ///
    /// Returns `Ok(())` on successful delivery, or an error string on failure.
    pub async fn send(&self, msg: &AlertMessage) -> Result<(), String> {
        let level_color = msg.level.color();
        let timestamp_str = msg.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

        let html_body = format!(
            r#"<html>
<body style="font-family: sans-serif;">
  <h2 style="color: {level_color};">[太极告警·{level_label}] {title}</h2>
  <p>{body}</p>
  <hr>
  <p style="color: #888; font-size: 12px;">
    来源：{source}<br>
    时间：{timestamp}<br>
    累计：{count} 次
  </p>
</body>
</html>"#,
            level_label = msg.level.label_cn(),
            title = msg.title,
            body = msg.body.replace('\n', "<br>"),
            source = msg.source,
            timestamp = timestamp_str,
            count = msg.count,
        );

        let email = Message::builder()
            .from(self.from.clone())
            .to(self.to.clone())
            .subject(format!("[太极·{}] {}", msg.level.label_cn(), msg.title))
            .header(header::ContentType::TEXT_HTML)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_PLAIN)
                            .body(format!(
                                "{}\n\n---\n来源：{}\n时间：{}\n累计：{} 次",
                                msg.body, msg.source, timestamp_str, msg.count
                            )),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(header::ContentType::TEXT_HTML)
                            .body(html_body),
                    ),
            )
            .map_err(|e| format!("failed to build email: {e}"))?;

        self.mailer
            .send(email)
            .await
            .map_err(|e| format!("SMTP send failed: {e}"))?;

        debug!(
            "Email alert sent: level={:?}, title={}",
            msg.level, msg.title
        );
        Ok(())
    }
}

/// Build a Feishu interactive card from an AlertMessage.
///
/// Card structure:
/// - `header` with level-color and [太极告警] title
/// - `elements` containing markdown body, source tag, timestamp, and occurrence count
fn build_alert_card(msg: &AlertMessage) -> serde_json::Value {
    let header_color = alert_level_feishu_color(msg.level);
    let level_label = msg.level.label_cn();

    let timestamp_str = msg.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();

    let markdown = if msg.count > 1 {
        format!(
            "{}  \n\n---\n**来源**：{}  \n**时间**：{}  \n**累计**：{} 次",
            msg.body, msg.source, timestamp_str, msg.count
        )
    } else {
        format!(
            "{}  \n\n---\n**来源**：{}  \n**时间**：{}",
            msg.body, msg.source, timestamp_str
        )
    };

    json!({
        "header": {
            "title": {
                "tag": "plain_text",
                "content": format!("[太极告警·{}] {}", level_label, msg.title),
            },
            "template": header_color,
        },
        "elements": [
            {
                "tag": "markdown",
                "content": markdown,
            },
        ],
    })
}

/// Map AlertLevel to Feishu card header template color.
fn alert_level_feishu_color(level: AlertLevel) -> &'static str {
    match level {
        AlertLevel::Heartbeat => "blue",
        AlertLevel::Warn => "yellow",
        AlertLevel::Error => "orange",
        AlertLevel::Critical => "red",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn build_alert_card_contains_level_info() {
        let msg = AlertMessage {
            level: AlertLevel::Error,
            title: "ag2506 视频生成失败".into(),
            body: "FFmpeg 合成超时，请检查 GPU 状态。".into(),
            source: "cron:video_ag2506_daily".into(),
            timestamp: Utc::now(),
            count: 1,
        };
        let card = build_alert_card(&msg);
        let card_str = card.to_string();

        assert!(card_str.contains("太极告警"));
        assert!(card_str.contains("错误"));
        assert!(card_str.contains("ag2506 视频生成失败"));
        assert!(card_str.contains("FFmpeg 合成超时"));
        assert!(card_str.contains("cron:video_ag2506_daily"));
        assert!(!card_str.contains("累计"));
    }

    #[test]
    fn build_alert_card_shows_count_when_aggregated() {
        let msg = AlertMessage {
            level: AlertLevel::Warn,
            title: "聚合测试".into(),
            body: "多次失败已合并。".into(),
            source: "cron:test".into(),
            timestamp: Utc::now(),
            count: 5,
        };
        let card = build_alert_card(&msg);
        let card_str = card.to_string();

        assert!(card_str.contains("累计"));
        assert!(card_str.contains("5 次"));
    }

    #[test]
    fn alert_level_feishu_color_mapping() {
        assert_eq!(alert_level_feishu_color(AlertLevel::Heartbeat), "blue");
        assert_eq!(alert_level_feishu_color(AlertLevel::Warn), "yellow");
        assert_eq!(alert_level_feishu_color(AlertLevel::Error), "orange");
        assert_eq!(alert_level_feishu_color(AlertLevel::Critical), "red");
    }

    #[test]
    fn feishu_webhook_alerter_new() {
        let alerter = FeishuWebhookAlerter::new(
            "https://open.feishu.cn/open-apis/bot/v2/hook/test-token".into(),
        );
        assert!(alerter.webhook_url.contains("test-token"));
    }

    #[test]
    fn desktop_alerter_invokes_callback() {
        use std::sync::Mutex;
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let alerter = DesktopAlerter::new(Arc::new(move |title, body| {
            received_clone.lock().unwrap().push((title, body));
        }));

        let msg = AlertMessage {
            level: AlertLevel::Warn,
            title: "测试桌面通知".into(),
            body: "这是一条测试消息。".into(),
            source: "test".into(),
            timestamp: Utc::now(),
            count: 1,
        };
        alerter.send(&msg);

        let entries = received.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].0.contains("太极"));
        assert!(entries[0].0.contains("警告"));
        assert!(entries[0].0.contains("测试桌面通知"));
        assert!(entries[0].1.contains("测试消息"));
    }

    #[test]
    fn desktop_alerter_shows_count() {
        use std::sync::Mutex;
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let alerter = DesktopAlerter::new(Arc::new(move |title, body| {
            received_clone.lock().unwrap().push((title, body));
        }));

        let msg = AlertMessage {
            level: AlertLevel::Error,
            title: "聚合测试".into(),
            body: "多次失败已合并。".into(),
            source: "cron:test".into(),
            timestamp: Utc::now(),
            count: 3,
        };
        alerter.send(&msg);

        let entries = received.lock().unwrap();
        assert!(entries[0].1.contains("累计 3 次"));
    }

    #[test]
    fn email_alerter_construction_error_on_bad_mailbox() {
        let smtp_config = SmtpConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "user@example.com".into(),
            password: "secret".into(),
            use_tls: true,
        };
        // Empty to_email should fail to parse as a Mailbox.
        let result = EmailAlerter::new(&smtp_config, "太极告警", "");
        assert!(result.is_err());
    }
}
