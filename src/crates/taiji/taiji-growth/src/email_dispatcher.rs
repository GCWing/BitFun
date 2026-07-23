//! Email dispatch module.
//!
//! 通过 lettre SMTP + tera 模板实现邮件发送。
//! 支持：交易信号推送、日/周报推送、批量发送、double opt-in 确认、自动退订链接注入。

use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use tera::{Context, Tera};

use crate::types::{
    BatchResult, ContentAsset, EmailBatch, EmailLog, EmailStatus, EmailType, SignalSummary,
    SmtpConfig, Subscriber, SubscriberStatus,
};

// ── 编译期嵌入模板 ──

const DAILY_REPORT_TEMPLATE: &str = include_str!("../templates/email_daily_report.tera");
const SIGNAL_ALERT_TEMPLATE: &str = include_str!("../templates/email_signal_alert.tera");
const CONFIRMATION_TEMPLATE: &str = include_str!("../templates/email_confirmation.tera");

// ── EmailDispatcher ──

pub struct EmailDispatcher {
    smtp_config: SmtpConfig,
    tera: Tera,
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    /// 用于构造退订/确认链接的基础 URL，如 "https://taiji.example.com"
    base_url: String,
}

impl EmailDispatcher {
    /// 创建 EmailDispatcher。
    ///
    /// 嵌入三个 Tera 模板：`daily_report`、`signal_alert`、`confirmation`。
    pub fn new(smtp_config: SmtpConfig, base_url: String) -> Result<Self, String> {
        let mut tera = Tera::default();
        tera.add_raw_template("daily_report", DAILY_REPORT_TEMPLATE)
            .map_err(|e| format!("加载 daily_report 模板失败: {e}"))?;
        tera.add_raw_template("signal_alert", SIGNAL_ALERT_TEMPLATE)
            .map_err(|e| format!("加载 signal_alert 模板失败: {e}"))?;
        tera.add_raw_template("confirmation", CONFIRMATION_TEMPLATE)
            .map_err(|e| format!("加载 confirmation 模板失败: {e}"))?;

        let creds = Credentials::new(smtp_config.username.clone(), smtp_config.password.clone());

        let builder = if smtp_config.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_config.host)
                .map_err(|e| format!("SMTP relay 连接失败: {e}"))?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp_config.host)
                .port(smtp_config.port)
        };
        let mailer = builder.credentials(creds).build();

        Ok(Self {
            smtp_config,
            tera,
            mailer,
            base_url,
        })
    }

    // ── 公开 API ──

    /// 发送交易信号提醒邮件。
    pub async fn send_signal_alert(
        &self,
        to: &Subscriber,
        signal: &SignalSummary,
    ) -> Result<(), String> {
        let mut ctx = Context::new();
        ctx.insert("instrument", &signal.instrument);
        ctx.insert("signal_type", &signal.signal_type);
        ctx.insert("price", &signal.price);
        ctx.insert(
            "timestamp",
            &signal.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        );
        ctx.insert("reason", &signal.reason);
        ctx.insert("strategy", &signal.strategy);
        ctx.insert("freq", &signal.freq);
        ctx.insert(
            "unsubscribe_url",
            &self.unsubscribe_url(&to.id, to.opt_in_token.as_deref().unwrap_or("")),
        );

        let body = self
            .tera
            .render("signal_alert", &ctx)
            .map_err(|e| format!("信号模板渲染失败: {e}"))?;

        let subject = format!(
            "[太极] {} {}信号 @ {}",
            signal.instrument, signal.signal_type, signal.price
        );

        self.send_message(&to.email, &subject, &body).await
    }

    /// 发送每日报告邮件。
    pub async fn send_daily_report(
        &self,
        to: &Subscriber,
        report: &ContentAsset,
    ) -> Result<(), String> {
        let mut ctx = Context::new();
        ctx.insert("title", &report.title);
        ctx.insert("body", &report.markdown_body);
        ctx.insert("date", &report.created_at.format("%Y-%m-%d").to_string());

        // 标签列表传给模板
        let tags: Vec<&str> = report.tags.iter().map(|s| s.as_str()).collect();
        ctx.insert("tags", &tags);

        if let Some(seo_desc) = &report.seo_description {
            ctx.insert("summary", &seo_desc.as_str());
        }

        ctx.insert(
            "unsubscribe_url",
            &self.unsubscribe_url(&to.id, to.opt_in_token.as_deref().unwrap_or("")),
        );

        let body = self
            .tera
            .render("daily_report", &ctx)
            .map_err(|e| format!("日报模板渲染失败: {e}"))?;

        let subject = format!(
            "[太极] 日报: {} — {}",
            report.title,
            report.created_at.format("%Y-%m-%d")
        );

        self.send_message(&to.email, &subject, &body).await
    }

    /// 批量发送邮件。
    ///
    /// 仅向 `Active` 状态订阅者发送，返回每条结果。
    pub async fn send_batch(
        &self,
        subscribers: &[Subscriber],
        content: &EmailBatch,
    ) -> Result<Vec<BatchResult>, String> {
        let mut results = Vec::with_capacity(subscribers.len());

        for sub in subscribers {
            if sub.status != SubscriberStatus::Active {
                results.push(BatchResult {
                    subscriber_id: sub.id.clone(),
                    success: false,
                    error: Some(format!("跳过非 Active 状态订阅者（{:?}）", sub.status)),
                });
                continue;
            }

            let result = match content {
                EmailBatch::Signal(signal) => self.send_signal_alert(sub, signal).await,
                EmailBatch::Report(report) => self.send_daily_report(sub, report).await,
            };

            results.push(BatchResult {
                subscriber_id: sub.id.clone(),
                success: result.is_ok(),
                error: result.err(),
            });
        }

        Ok(results)
    }

    /// 发送 double opt-in 确认邮件。
    pub async fn send_confirmation(&self, to: &Subscriber) -> Result<(), String> {
        let token = to
            .opt_in_token
            .as_deref()
            .ok_or_else(|| "订阅者缺少 opt_in_token".to_string())?;

        let mut ctx = Context::new();
        ctx.insert("email", &to.email);
        ctx.insert(
            "confirm_url",
            &format!("{}/subscribe/confirm?token={}", self.base_url, token),
        );
        ctx.insert("unsubscribe_url", &self.unsubscribe_url(&to.id, token));

        let body = self
            .tera
            .render("confirmation", &ctx)
            .map_err(|e| format!("确认模板渲染失败: {e}"))?;

        let subject = "[太极] 请确认您的邮件订阅";

        self.send_message(&to.email, subject, &body).await
    }

    // ── 内部方法 ──

    /// 构造退订链接。
    fn unsubscribe_url(&self, subscriber_id: &str, token: &str) -> String {
        format!(
            "{}/unsubscribe?id={}&token={}",
            self.base_url, subscriber_id, token
        )
    }

    /// 通过 SMTP 发送邮件。
    async fn send_message(
        &self,
        to_email: &str,
        subject: &str,
        html_body: &str,
    ) -> Result<(), String> {
        let from_addr = format!(
            "{} <{}>",
            self.smtp_config.from_name, self.smtp_config.from_email
        )
        .parse()
        .map_err(|e| format!("发件人地址解析失败: {e}"))?;

        let to_addr: lettre::message::Mailbox = to_email
            .parse()
            .map_err(|e| format!("收件人地址解析失败: {e}"))?;

        let email = Message::builder()
            .from(from_addr)
            .to(to_addr)
            .subject(subject)
            .header(lettre::message::header::ContentType::TEXT_HTML)
            .body(html_body.to_string())
            .map_err(|e| format!("邮件构建失败: {e}"))?;

        self.mailer
            .send(email)
            .await
            .map_err(|e| format!("邮件发送失败: {e}"))?;

        Ok(())
    }

    /// 创建一条 EmailLog（调用方在发送后写入持久化存储）。
    #[allow(dead_code)]
    pub fn build_log(
        subscriber_id: &str,
        email_type: EmailType,
        subject: &str,
        status: EmailStatus,
        error: Option<String>,
    ) -> EmailLog {
        EmailLog {
            id: uuid_fast(),
            subscriber_id: subscriber_id.to_string(),
            email_type,
            subject: subject.to_string(),
            status,
            error,
            sent_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
        }
    }
}

/// 快速生成 UUID v4（简单版，不引入 uuid crate 作为 pub dep）。
fn uuid_fast() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u32;

    // 用时间戳的低位拼一个近似 UUID v4 格式的字符串
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:04x}{:08x}",
        nanos,
        (micros >> 16) as u16,
        (micros >> 4) & 0x0FFF,
        (micros & 0xFFFF) as u16,
        (nanos >> 16) as u16,
        nanos
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SubscriberPreferences;

    #[test]
    fn test_template_renders_with_unsubscribe_url() {
        let mut tera = Tera::default();
        tera.add_raw_template("signal_alert", SIGNAL_ALERT_TEMPLATE)
            .unwrap();
        tera.add_raw_template("daily_report", DAILY_REPORT_TEMPLATE)
            .unwrap();
        tera.add_raw_template("confirmation", CONFIRMATION_TEMPLATE)
            .unwrap();

        // 信号提醒模板渲染
        let mut ctx = Context::new();
        ctx.insert("instrument", "ag2506");
        ctx.insert("signal_type", "open_long");
        ctx.insert("price", &5432.0);
        ctx.insert("timestamp", "2026-07-22 14:30:00 UTC");
        ctx.insert("reason", "量价背离 + 支撑位反弹");
        ctx.insert("strategy", "trend_following_v3");
        ctx.insert("freq", "5min");
        ctx.insert(
            "unsubscribe_url",
            "https://taiji.example.com/unsubscribe?id=sub-001&token=abc",
        );
        let rendered = tera.render("signal_alert", &ctx).unwrap();
        assert!(rendered.contains("ag2506"));
        assert!(rendered.contains("open_long"));
        assert!(rendered.contains("unsubscribe?id=sub-001"));
    }

    #[test]
    fn test_unsubscribe_url_in_all_templates() {
        let mut tera = Tera::default();
        tera.add_raw_template("signal_alert", SIGNAL_ALERT_TEMPLATE)
            .unwrap();
        tera.add_raw_template("daily_report", DAILY_REPORT_TEMPLATE)
            .unwrap();
        tera.add_raw_template("confirmation", CONFIRMATION_TEMPLATE)
            .unwrap();

        let test_url = "https://taiji.example.com/unsubscribe?id=test&token=xyz";
        let templates = ["signal_alert", "daily_report", "confirmation"];

        for t_name in &templates {
            let mut ctx = Context::new();
            // 填充模板所需的通用变量
            ctx.insert("instrument", "rb2510");
            ctx.insert("signal_type", "close_long");
            ctx.insert("price", &3200.0);
            ctx.insert("timestamp", "2026-07-22 15:00:00 UTC");
            ctx.insert("reason", "test");
            ctx.insert("strategy", "test");
            ctx.insert("freq", "1min");
            ctx.insert("title", "Test Report");
            ctx.insert("body", "Test body content");
            ctx.insert("date", "2026-07-22");
            ctx.insert("tags", &Vec::<String>::new());
            ctx.insert("summary", "Test summary");
            ctx.insert("email", "test@example.com");
            ctx.insert(
                "confirm_url",
                "https://taiji.example.com/subscribe/confirm?token=xyz",
            );
            ctx.insert("unsubscribe_url", test_url);

            let rendered = tera.render(t_name, &ctx).unwrap();
            assert!(
                rendered.contains("unsubscribe"),
                "模板 {} 缺少退订链接",
                t_name
            );
        }
    }

    #[test]
    fn test_subscriber_preferences_default() {
        let prefs = SubscriberPreferences::default();
        assert!(prefs.signal_alert);
        assert!(prefs.daily_report);
        assert!(!prefs.weekly_report);
    }

    #[test]
    fn test_content_type_serde_email_types() {
        let signal = EmailType::Signal;
        let json = serde_json::to_string(&signal).unwrap();
        assert_eq!(json, r#""signal""#);

        let deserialized: EmailType = serde_json::from_str(r#""confirmation""#).unwrap();
        assert_eq!(deserialized, EmailType::Confirmation);
    }

    #[test]
    fn test_email_status_serde() {
        let sent = EmailStatus::Sent;
        let json = serde_json::to_string(&sent).unwrap();
        assert_eq!(json, r#""sent""#);

        let deserialized: EmailStatus = serde_json::from_str(r#""failed""#).unwrap();
        assert_eq!(deserialized, EmailStatus::Failed);
    }

    #[test]
    fn test_subscriber_status_serde() {
        let pending = SubscriberStatus::Pending;
        let json = serde_json::to_string(&pending).unwrap();
        assert_eq!(json, r#""pending""#);

        let deserialized: SubscriberStatus = serde_json::from_str(r#""active""#).unwrap();
        assert_eq!(deserialized, SubscriberStatus::Active);
    }

    #[test]
    fn test_uuid_fast_format() {
        let id = uuid_fast();
        assert_eq!(id.len(), 36);
        // UUID v4 格式：第 15 位应为 '4'
        assert_eq!(id.chars().nth(14), Some('4'));
        assert_eq!(id.chars().nth(8), Some('-'));
        assert_eq!(id.chars().nth(13), Some('-'));
        assert_eq!(id.chars().nth(18), Some('-'));
        assert_eq!(id.chars().nth(23), Some('-'));
    }

    #[test]
    fn test_build_log() {
        let log = EmailDispatcher::build_log(
            "sub-001",
            EmailType::Signal,
            "[太极] ag2506 open_long信号 @ 5432",
            EmailStatus::Sent,
            None,
        );
        assert_eq!(log.subscriber_id, "sub-001");
        assert_eq!(log.email_type, EmailType::Signal);
        assert_eq!(log.status, EmailStatus::Sent);
        assert!(log.error.is_none());
        assert!(log.sent_at.is_some());
    }
}
