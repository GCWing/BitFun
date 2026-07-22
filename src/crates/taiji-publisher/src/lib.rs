use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Re-exported from [`taiji_content::DateRange`], the canonical definition.
pub use taiji_content::DateRange;

/// Video publishing asset — only contains fields required for the publishing stage.
/// Render intermediates (frame sequences, echarts options, TTS scripts, audio) are managed
/// in a separate intermediate directory and not included in VideoAsset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoAsset {
    /// UUID
    pub id: String,
    /// Instrument code, e.g. "ag2506"
    pub instrument: String,
    /// Period/frequency, e.g. "5min"
    pub freq: String,
    /// Date range
    pub date_range: DateRange,
    /// Video duration (seconds)
    pub duration_secs: f64,
    /// Final MP4 file path
    pub video_path: PathBuf,
    /// File size (bytes)
    pub video_size_bytes: u64,
    /// Video title
    pub title: String,
    /// Video description
    pub description: String,
    /// Tag list
    pub tags: Vec<String>,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Cover image path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_path: Option<PathBuf>,
    /// Content category (daily_review / weekly / ad_hoc)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Accompanying article body (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_body: Option<String>,
    /// SEO title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seo_title: Option<String>,
    /// SEO description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seo_description: Option<String>,
}

/// Unified multi-platform publishing trait.
/// Each platform (Bilibili/Douyin/Xiaohongshu/YouTube) is independently implemented.
#[async_trait::async_trait]
pub trait PlatformPublisher: Send + Sync {
    /// Returns the platform name, e.g. "bilibili"
    fn platform_name(&self) -> &str;

    /// Check if authentication is valid
    async fn check_auth(&self) -> Result<bool, String>;

    /// Upload video and publish
    async fn upload(&self, video: &VideoAsset) -> Result<PublishResult, String>;

    /// Query publish status
    async fn status(&self, publish_id: &str) -> Result<PublishStatus, String>;

    /// Update published content (not supported by default)
    async fn update(&self, _video: &VideoAsset) -> Result<PublishResult, String> {
        Err("update not supported".into())
    }

    /// Unpublish published content (not supported by default)
    async fn unpublish(&self, _publish_id: &str) -> Result<PublishStatus, String> {
        Err("unpublish not supported".into())
    }
}

/// Single-platform publish result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    /// Platform name
    pub platform: String,
    /// Platform-side publish ID
    pub publish_id: String,
    /// Public URL (after successful publishing)
    pub url: Option<String>,
    /// Current status
    pub status: PublishStatus,
}

/// Publish status enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PublishStatus {
    /// Uploading, progress_pct is 0.0-100.0
    Uploading { progress_pct: f64 },
    /// Platform transcoding/processing
    Processing,
    /// Published, url is the public link
    Published { url: String },
    /// Publish failed, error is the error description
    Failed { error: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_video_asset_roundtrip() {
        let asset = VideoAsset {
            id: "test-uuid".into(),
            instrument: "ag2506".into(),
            freq: "5min".into(),
            date_range: DateRange {
                start: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
            },
            duration_secs: 60.0,
            video_path: PathBuf::from("output/test.mp4"),
            video_size_bytes: 1024000,
            title: "Test Video".into(),
            description: "AI-generated technical analysis".into(),
            tags: vec!["futures".into(), "technical analysis".into()],
            created_at: Utc::now(),
            thumbnail_path: None,
            category: None,
            content_body: None,
            seo_title: None,
            seo_description: None,
        };
        let json = serde_json::to_string(&asset).unwrap();
        let roundtrip: VideoAsset = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.instrument, "ag2506");
        assert_eq!(roundtrip.freq, "5min");
    }

    #[test]
    fn test_publish_status_roundtrip() {
        let published = PublishStatus::Published {
            url: "https://example.com".into(),
        };
        let json = serde_json::to_string(&published).unwrap();
        let roundtrip: PublishStatus = serde_json::from_str(&json).unwrap();
        match roundtrip {
            PublishStatus::Published { url } => assert!(url.contains("example.com")),
            _ => panic!("expected Published variant"),
        }
    }
}

pub mod biliup;
pub mod process_util;
pub mod publish_scheduler;
pub mod publisher_twitter;
pub mod publisher_wechat_mp;
pub mod social_auto;
