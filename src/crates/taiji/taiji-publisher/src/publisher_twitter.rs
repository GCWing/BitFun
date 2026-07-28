use crate::{PlatformPublisher, PublishResult, PublishStatus, VideoAsset};
use reqwest::redirect::Policy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tokio::time::Duration;

/// Twitter tweet creation request body.
#[derive(Debug, Serialize)]
struct CreateTweetRequest {
    text: String,
}

/// Twitter tweet creation response.
#[derive(Debug, Deserialize)]
struct CreateTweetResponse {
    data: Option<TweetData>,
}

#[derive(Debug, Deserialize)]
struct TweetData {
    id: String,
    #[allow(dead_code)]
    text: String,
}

/// Twitter API v2 publisher.
///
/// Authenticates using OAuth 2.0 Bearer Token or PKCE access_token.
/// Posts via POST /2/tweets.
///
/// Token acquisition methods (choose one):
/// - Twitter Developer Portal → App → Keys & Tokens → Bearer Token (directly usable)
/// - OAuth 2.0 PKCE flow → obtain access_token (requires external tool to complete auth flow)
///
/// TODO(tool-audit): This struct owns an independent `reqwest::Client`
/// (`redirect = Policy::none()`). Its builder config is identical to
/// `WechatMpPublisher` — both crates could share a single no-redirect
/// `reqwest::Client` instance injected via constructor.
pub struct TwitterPublisher {
    access_token: Mutex<String>,
    http: Client,
}

impl TwitterPublisher {
    /// TODO(tool-audit): Replace inline `Client::builder()...build()` with
    /// an injected shared `reqwest::Client`. Identical no-redirect config as
    /// `WechatMpPublisher` — these two can share a single client instance.
    pub fn new(access_token: String) -> Self {
        Self {
            access_token: Mutex::new(access_token),
            http: Client::builder()
                .redirect(Policy::none())
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Update access_token (for manual refresh after token expiry).
    pub fn update_token(&self, new_token: String) {
        if let Ok(mut t) = self.access_token.lock() {
            *t = new_token;
        }
    }

    /// Build tweet text from VideoAsset.
    ///
    /// Format: title + summary (truncated at 280 chars) + video link + #VolumePriceTimeSpace
    fn build_tweet_text(video: &VideoAsset) -> String {
        let hashtag = " #VolumePriceTimeSpace";
        let hashtag_len = hashtag.len();
        let max_len: usize = 280;
        let body_limit = max_len - hashtag_len;

        let mut parts: Vec<String> = Vec::new();

        if !video.title.is_empty() {
            parts.push(video.title.clone());
        }

        if !video.description.is_empty() {
            if parts.is_empty() {
                parts.push(truncate_utf8(&video.description, body_limit));
            } else {
                let title_len = parts[0].chars().count();
                // Reserve 2 chars for "\n\n"
                let remaining = body_limit.saturating_sub(title_len).saturating_sub(2);
                if remaining > 0 {
                    parts.push(
                        video
                            .description
                            .chars()
                            .take(remaining)
                            .collect::<String>()
                            .trim()
                            .to_string(),
                    );
                }
            }
        }

        let mut body = parts.join("\n\n");

        // Final truncation to ensure 280 char limit
        let total_limit = max_len.saturating_sub(hashtag_len);
        if body.chars().count() > total_limit {
            body = body
                .chars()
                .take(total_limit.saturating_sub(1))
                .collect::<String>();
        }

        body.push_str(hashtag);
        body
    }
}

/// Truncate string at UTF-8 character boundary.
fn truncate_utf8(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

#[async_trait::async_trait]
impl PlatformPublisher for TwitterPublisher {
    fn platform_name(&self) -> &str {
        "twitter"
    }

    async fn check_auth(&self) -> Result<bool, String> {
        let token = { self.access_token.lock().map_err(|e| e.to_string())?.clone() };

        // Use GET /2/users/me for lightweight token validity check
        let resp = self
            .http
            .get("https://api.twitter.com/2/users/me")
            .bearer_auth(&token)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Auth check failed: {}", e))?;

        Ok(resp.status().is_success())
    }

    async fn upload(&self, video: &VideoAsset) -> Result<PublishResult, String> {
        let token = { self.access_token.lock().map_err(|e| e.to_string())?.clone() };

        let tweet_text = Self::build_tweet_text(video);

        let body = CreateTweetRequest {
            text: tweet_text.clone(),
        };

        let resp = self
            .http
            .post("https://api.twitter.com/2/tweets")
            .bearer_auth(&token)
            .json(&body)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("Twitter post failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let err_body = resp.text().await.unwrap_or_default();
            return Ok(PublishResult {
                platform: "twitter".into(),
                publish_id: String::new(),
                url: None,
                status: PublishStatus::Failed {
                    error: format!("Twitter API error (HTTP {}): {}", status, err_body),
                },
            });
        }

        let tweet_resp: CreateTweetResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Twitter response: {}", e))?;

        let tweet_data = tweet_resp
            .data
            .ok_or_else(|| "Twitter returned empty data".to_string())?;
        let url = format!("https://twitter.com/i/status/{}", tweet_data.id);

        Ok(PublishResult {
            platform: "twitter".into(),
            publish_id: tweet_data.id,
            url: Some(url.clone()),
            status: PublishStatus::Published { url },
        })
    }

    async fn status(&self, publish_id: &str) -> Result<PublishStatus, String> {
        let token = { self.access_token.lock().map_err(|e| e.to_string())?.clone() };

        let resp = self
            .http
            .get(format!(
                "https://api.twitter.com/2/tweets/{}?tweet.fields=created_at",
                publish_id
            ))
            .bearer_auth(&token)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Twitter status query failed: {}", e))?;

        if resp.status().is_success() {
            let url = format!("https://twitter.com/i/status/{}", publish_id);
            Ok(PublishStatus::Published { url })
        } else if resp.status().as_u16() == 404 {
            Ok(PublishStatus::Failed {
                error: "tweet not found".into(),
            })
        } else {
            Ok(PublishStatus::Processing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_asset(title: &str, desc: &str) -> VideoAsset {
        VideoAsset {
            id: "t1".into(),
            instrument: "ag2506".into(),
            freq: "5min".into(),
            date_range: crate::DateRange {
                start: chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                end: chrono::NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
            },
            duration_secs: 60.0,
            video_path: std::path::PathBuf::from("test.mp4"),
            video_size_bytes: 1024,
            title: title.into(),
            description: desc.into(),
            tags: vec!["futures".into()],
            created_at: Utc::now(),
            thumbnail_path: None,
            category: None,
            content_body: None,
            seo_title: None,
            seo_description: None,
        }
    }

    #[test]
    fn test_build_tweet_text_basic() {
        let asset = test_asset(
            "Silver 5min Volume-Price Analysis",
            "Today's silver main contract ag2506 shows B2→S3 structure.",
        );
        let text = TwitterPublisher::build_tweet_text(&asset);
        assert!(text.contains("Silver"));
        assert!(text.contains("B2→S3"));
        assert!(text.contains("#VolumePriceTimeSpace"));
        assert!(text.chars().count() <= 280);
    }

    #[test]
    fn test_build_tweet_text_truncation() {
        let asset = test_asset(&"X".repeat(300), &"Y".repeat(500));
        let text = TwitterPublisher::build_tweet_text(&asset);
        assert!(text.chars().count() <= 280);
        assert!(text.contains("#VolumePriceTimeSpace"));
    }

    #[test]
    fn test_build_tweet_text_no_title() {
        let asset = test_asset("", "Description only without title");
        let text = TwitterPublisher::build_tweet_text(&asset);
        assert!(text.contains("Description only without title"));
        assert!(text.contains("#VolumePriceTimeSpace"));
    }

    #[test]
    fn test_truncate_utf8_boundary() {
        let s = "Hello World";
        let t = truncate_utf8(s, 2);
        assert_eq!(t, "He");
        assert_eq!(t.chars().count(), 2);
    }

    #[test]
    fn test_twitter_publisher_new() {
        let p = TwitterPublisher::new("test-token".into());
        assert_eq!(p.platform_name(), "twitter");
    }
}
