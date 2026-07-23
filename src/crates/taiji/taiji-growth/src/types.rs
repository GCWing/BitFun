use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Re-exported from [`taiji_content::DateRange`], the canonical definition.
pub use taiji_content::DateRange;

/// 内容类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    DailyReport,
    WeeklyReport,
    BlogPost,
    CoursePage,
}

/// 内容发布状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentStatus {
    Draft,
    Ready,
    Published,
    Failed,
}

/// 网站发布内容资产。
///
/// 与 VideoAsset 互补：VideoAsset 承载视频发布流程，
/// ContentAsset 承载网站（图文）发布流程，两者可共享同一个分析来源。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAsset {
    /// UUID
    pub id: String,
    /// 文章标题
    pub title: String,
    /// 内容类型
    pub content_type: ContentType,
    /// Markdown 正文
    pub markdown_body: String,
    /// 前置元数据（YAML front matter 键值对）
    pub front_matter: HashMap<String, serde_json::Value>,
    /// 标签列表
    pub tags: Vec<String>,
    /// SEO 标题（独立于 title，用于 `<title>` 标签）
    pub seo_title: Option<String>,
    /// SEO 描述（用于 `<meta name="description">`）
    pub seo_description: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 发布状态
    pub status: ContentStatus,
}

/// 报告生成配置。
///
/// 指定从哪支品种/周期的分析数据生成网站报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// 品种代码，如 "ag2506"
    pub instrument: String,
    /// 周期，如 "5min"
    pub freq: String,
    /// 日期范围
    pub date_range: DateRange,
    /// 报告模板名称（对应 Tera 模板文件 stem）
    pub template: String,
    /// 输出目录（Markdown 文件写入位置）
    pub output_dir: PathBuf,
}

/// 网站站点配置。
///
/// 映射到 Zola `config.toml` / Hugo `config.yaml` 的关键字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsiteConfig {
    /// 站点标题
    pub site_title: String,
    /// 站点根 URL
    pub base_url: String,
    /// Google Analytics 测量 ID（可选）
    pub google_analytics_id: Option<String>,
    /// Plausible 自定义域名（可选）
    pub plausible_domain: Option<String>,
    /// 支持的语言列表，如 ["zh", "en"]
    pub languages: Vec<String>,
    /// 默认语言
    pub default_language: String,
}

// ── 邮件与订阅类型 ──

/// SMTP 连接配置。
//
// TODO(P2-1): Deduplicate with `taiji-alert::SmtpConfig`.
// This version adds `from_name` + `from_email` that the alert version
// lacks. Extract a shared `SmtpConfig` with all fields into
// `taiji-engine` or a new `taiji-shared` crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// SMTP 服务器地址
    pub host: String,
    /// SMTP 端口
    pub port: u16,
    /// 认证用户名
    pub username: String,
    /// 认证密码
    #[serde(skip_serializing)]
    pub password: String,
    /// 发件人名称
    pub from_name: String,
    /// 发件人邮箱
    pub from_email: String,
    /// 是否使用 TLS（true: 直接 TLS, false: 明文/STARTTLS）
    pub use_tls: bool,
}

/// 订阅者状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriberStatus {
    /// 待确认（double opt-in 确认邮件已发）
    Pending,
    /// 已激活
    Active,
    /// 已退订
    Unsubscribed,
}

/// 订阅偏好。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberPreferences {
    /// 交易信号提醒
    pub signal_alert: bool,
    /// 每日报告
    pub daily_report: bool,
    /// 每周报告
    pub weekly_report: bool,
}

impl Default for SubscriberPreferences {
    fn default() -> Self {
        Self {
            signal_alert: true,
            daily_report: true,
            weekly_report: false,
        }
    }
}

/// 邮件订阅者。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscriber {
    /// UUID
    pub id: String,
    /// 邮箱地址
    pub email: String,
    /// 订阅状态
    pub status: SubscriberStatus,
    /// Double opt-in 验证令牌
    pub opt_in_token: Option<String>,
    /// 确认时间
    pub opt_in_at: Option<DateTime<Utc>>,
    /// 订阅偏好
    pub preferences: SubscriberPreferences,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 邮件类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmailType {
    Signal,
    DailyReport,
    WeeklyReport,
    Confirmation,
}

/// 邮件发送状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmailStatus {
    Queued,
    Sent,
    Bounced,
    Failed,
}

/// 邮件发送日志。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailLog {
    /// UUID
    pub id: String,
    /// 关联订阅者 ID
    pub subscriber_id: String,
    /// 邮件类型
    pub email_type: EmailType,
    /// 邮件主题
    pub subject: String,
    /// 发送状态
    pub status: EmailStatus,
    /// 错误信息
    pub error: Option<String>,
    /// 发送时间
    pub sent_at: Option<DateTime<Utc>>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 交易信号摘要（从 taiji-engine Signal 转换，用于邮件模板渲染）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSummary {
    /// 合约代码，如 "ag2506"
    pub instrument: String,
    /// 信号类型："open_long" | "close_long" | "open_short" | "close_short" | "stop_loss" | "take_profit"
    pub signal_type: String,
    /// 信号价格
    pub price: f64,
    /// 信号产生时间
    pub timestamp: DateTime<Utc>,
    /// 信号理由
    pub reason: String,
    /// 策略名称
    pub strategy: String,
    /// 周期
    pub freq: String,
}

/// 批量发送内容枚举。
#[derive(Debug, Clone)]
pub enum EmailBatch {
    Signal(SignalSummary),
    Report(ContentAsset),
}

/// 批量发送单条结果。
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub subscriber_id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_content_asset_roundtrip() {
        let mut front_matter = HashMap::new();
        front_matter.insert("author".into(), serde_json::Value::String("taiji".into()));
        front_matter.insert("draft".into(), serde_json::Value::Bool(false));

        let asset = ContentAsset {
            id: "report-001".into(),
            title: "ag2506 每日复盘".into(),
            content_type: ContentType::DailyReport,
            markdown_body: "## 量价分析\n\n今日收盘价...".into(),
            front_matter,
            tags: vec!["期货".into(), "白银".into(), "技术分析".into()],
            seo_title: Some("ag2506 2026-07-22 复盘报告".into()),
            seo_description: Some("基于量价时空理论的 ag2506 每日技术分析".into()),
            created_at: Utc::now(),
            status: ContentStatus::Ready,
        };

        let json = serde_json::to_string(&asset).unwrap();
        let roundtrip: ContentAsset = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.id, "report-001");
        assert_eq!(roundtrip.content_type, ContentType::DailyReport);
        assert_eq!(roundtrip.status, ContentStatus::Ready);
        assert!(roundtrip.markdown_body.contains("量价分析"));
        assert_eq!(roundtrip.tags.len(), 3);
        assert_eq!(
            roundtrip.seo_title.as_deref(),
            Some("ag2506 2026-07-22 复盘报告")
        );
    }

    #[test]
    fn test_content_type_serde() {
        let daily = ContentType::DailyReport;
        let json = serde_json::to_string(&daily).unwrap();
        assert_eq!(json, r#""daily_report""#);

        let deserialized: ContentType = serde_json::from_str(r#""weekly_report""#).unwrap();
        assert_eq!(deserialized, ContentType::WeeklyReport);

        let deserialized: ContentType = serde_json::from_str(r#""blog_post""#).unwrap();
        assert_eq!(deserialized, ContentType::BlogPost);

        let deserialized: ContentType = serde_json::from_str(r#""course_page""#).unwrap();
        assert_eq!(deserialized, ContentType::CoursePage);
    }

    #[test]
    fn test_content_status_serde() {
        let published = ContentStatus::Published;
        let json = serde_json::to_string(&published).unwrap();
        assert_eq!(json, r#""published""#);

        let deserialized: ContentStatus = serde_json::from_str(r#""failed""#).unwrap();
        assert_eq!(deserialized, ContentStatus::Failed);
    }

    #[test]
    fn test_report_config_roundtrip() {
        let config = ReportConfig {
            instrument: "rb2510".into(),
            freq: "15min".into(),
            date_range: DateRange {
                start: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 7, 22).unwrap(),
            },
            template: "daily_review".into(),
            output_dir: PathBuf::from("content/reports"),
        };

        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: ReportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.instrument, "rb2510");
        assert_eq!(roundtrip.freq, "15min");
        assert_eq!(roundtrip.template, "daily_review");
    }

    #[test]
    fn test_website_config_roundtrip() {
        let config = WebsiteConfig {
            site_title: "太极量化报告".into(),
            base_url: "https://taiji.example.com".into(),
            google_analytics_id: Some("G-XXXXXXXXXX".into()),
            plausible_domain: None,
            languages: vec!["zh".into(), "en".into()],
            default_language: "zh".into(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: WebsiteConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.site_title, "太极量化报告");
        assert_eq!(roundtrip.base_url, "https://taiji.example.com");
        assert_eq!(
            roundtrip.google_analytics_id.as_deref(),
            Some("G-XXXXXXXXXX")
        );
        assert!(roundtrip.plausible_domain.is_none());
        assert_eq!(roundtrip.languages, vec!["zh", "en"]);
        assert_eq!(roundtrip.default_language, "zh");
    }
}
