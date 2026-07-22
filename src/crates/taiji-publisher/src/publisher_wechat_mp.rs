use crate::{PlatformPublisher, PublishResult, PublishStatus, VideoAsset};
use chrono::{DateTime, Utc};
use reqwest::redirect::Policy;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Mutex;
use tokio::time::Duration;

/// WeChat Official Account access_token response.
#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: String,
    expires_in: i64,
    #[allow(dead_code)]
    errcode: Option<i32>,
    #[allow(dead_code)]
    errmsg: Option<String>,
}

/// Material upload response.
#[derive(Debug, Deserialize)]
struct MaterialUploadResponse {
    media_id: Option<String>,
    #[allow(dead_code)]
    url: Option<String>,
    #[allow(dead_code)]
    errcode: Option<i32>,
    #[allow(dead_code)]
    errmsg: Option<String>,
}

/// Article content within a draft creation request.
#[derive(Debug, serde::Serialize)]
struct DraftArticle {
    title: String,
    content: String,
    thumb_media_id: String,
    need_open_comment: u8,
}

/// Draft creation request body.
#[derive(Debug, serde::Serialize)]
struct DraftAddRequest {
    articles: Vec<DraftArticle>,
}

/// Draft creation response.
#[derive(Debug, Deserialize)]
struct DraftAddResponse {
    media_id: Option<String>,
    #[allow(dead_code)]
    errcode: Option<i32>,
    #[allow(dead_code)]
    errmsg: Option<String>,
}

/// Publish request body.
#[derive(Debug, serde::Serialize)]
struct FreePublishRequest {
    media_id: String,
}

/// Publish response.
#[derive(Debug, Deserialize)]
struct FreePublishResponse {
    publish_id: Option<String>,
    #[allow(dead_code)]
    errcode: Option<i32>,
    #[allow(dead_code)]
    errmsg: Option<String>,
}

/// Publish status polling response.
#[derive(Debug, Deserialize)]
struct PublishStatusResponse {
    #[allow(dead_code)]
    publish_id: Option<String>,
    publish_status: Option<i32>,
    #[allow(dead_code)]
    article_id: Option<String>,
    article_detail: Option<ArticleDetail>,
    #[allow(dead_code)]
    errcode: Option<i32>,
    #[allow(dead_code)]
    errmsg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArticleDetail {
    #[allow(dead_code)]
    idx: Option<i32>,
    article_url: Option<String>,
}

/// Token cache.
struct TokenCache {
    access_token: String,
    expires_at: DateTime<Utc>,
}

/// WeChat Official Account publisher.
///
/// Publishes via WeChat Official Account API (draft + publish mode).
/// Requires a verified WeChat service account (app_id + app_secret).
///
/// Publishing flow:
/// 1. Obtain access_token
/// 2. Upload video material → media_id (optional; if no video, pure article with images/text)
/// 3. Create draft (article message, body = VideoAsset.description)
/// 4. Publish draft (limited to 1 per day for service accounts)
///
/// TODO(tool-audit): This struct owns an independent `reqwest::Client`
/// (`redirect = Policy::none()`). Its builder config is identical to
/// `TwitterPublisher` — both crates could share a single no-redirect
/// `reqwest::Client` instance injected via constructor.
pub struct WechatMpPublisher {
    app_id: String,
    app_secret: String,
    token_cache: Mutex<Option<TokenCache>>,
    http: Client,
}

impl WechatMpPublisher {
    /// TODO(tool-audit): Replace inline `Client::builder()...build()` with
    /// an injected shared `reqwest::Client`. Identical no-redirect config as
    /// `TwitterPublisher` — these two can share a single client instance.
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            token_cache: Mutex::new(None),
            http: Client::builder()
                .redirect(Policy::none())
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Obtain/refresh access_token.
    async fn get_access_token(&self) -> Result<String, String> {
        // Check if cached token is still valid
        {
            let cache = self.token_cache.lock().map_err(|e| e.to_string())?;
            if let Some(ref tc) = *cache {
                if tc.expires_at > Utc::now() {
                    return Ok(tc.access_token.clone());
                }
            }
        }

        // SECURITY NOTE: The WeChat Official Account API `/cgi-bin/token` endpoint only
        // supports GET with query parameters. The `app_secret` is transmitted in the URL
        // query string, which means it may appear in server access logs, proxy logs, and
        // browser/bridge history. This is a known limitation of the WeChat API design.
        // Mitigation: HTTPS (TLS) encrypts the full URL in transit, but the query string
        // remains visible in server-side logs. If WeChat later adds a POST-based token
        // endpoint, this should be migrated immediately.
        let resp = self
            .http
            .get("https://api.weixin.qq.com/cgi-bin/token")
            .query(&[
                ("grant_type", "client_credential"),
                ("appid", &self.app_id),
                ("secret", &self.app_secret),
            ])
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("WeChat access_token request failed: {}", e))?;

        let body = resp.text().await.unwrap_or_default();

        let token_resp: AccessTokenResponse = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse access_token response: {}", e))?;

        if token_resp.errcode.is_some_and(|c| c != 0) {
            return Err(format!(
                "WeChat access_token error (errcode={}): {}",
                token_resp.errcode.unwrap_or(0),
                token_resp.errmsg.as_deref().unwrap_or("unknown"),
            ));
        }

        let expires_at =
            Utc::now() + chrono::Duration::seconds(token_resp.expires_in.saturating_sub(300)); // Refresh 5 minutes early
        let access_token = token_resp.access_token.clone();

        let mut cache = self.token_cache.lock().map_err(|e| e.to_string())?;
        *cache = Some(TokenCache {
            access_token: access_token.clone(),
            expires_at,
        });

        Ok(access_token)
    }

    /// Build article content (HTML format) from VideoAsset.
    fn build_article_content(video: &VideoAsset) -> String {
        let mut content = String::new();

        // Video section: if video path exists, insert video placeholder text
        content.push_str(&format!(
            "<section><p><strong>{}</strong></p>",
            escape_html(&video.title)
        ));

        // Main description
        for para in video
            .description
            .split('\n')
            .filter(|p| !p.trim().is_empty())
        {
            content.push_str(&format!("<p>{}</p>", escape_html(para.trim())));
        }

        // Instrument and frequency info
        content.push_str(&format!(
            "<p style=\"color:#888;font-size:14px;\">Instrument: {} | Frequency: {} | Duration: {}s</p>",
            escape_html(&video.instrument),
            escape_html(&video.freq),
            video.duration_secs as u32,
        ));

        // Tags
        if !video.tags.is_empty() {
            content.push_str(&format!(
                "<p style=\"color:#888;font-size:14px;\">Tags: {}</p>",
                escape_html(&video.tags.join(", "))
            ));
        }

        content.push_str("</section>");
        content
    }
}

/// HTML entity escaping.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[async_trait::async_trait]
impl PlatformPublisher for WechatMpPublisher {
    fn platform_name(&self) -> &str {
        "wechat_mp"
    }

    async fn check_auth(&self) -> Result<bool, String> {
        match self.get_access_token().await {
            Ok(_) => Ok(true),
            Err(e) => {
                // access_token failure is usually an app_id/secret configuration issue
                Err(format!("WeChat Official Account auth failed: {}", e))
            }
        }
    }

    async fn upload(&self, video: &VideoAsset) -> Result<PublishResult, String> {
        let access_token = self
            .get_access_token()
            .await
            .map_err(|e| format!("WeChat Official Account not authenticated: {}", e))?;

        // Step 1: Try uploading video material (if video_path exists and is valid).
        // Video upload is optional; failure does not block article publishing.
        let video_media_id: Option<String> = if video.video_path.exists() {
            match upload_video_material(&self.http, &access_token, video).await {
                Ok(media_id) => Some(media_id),
                Err(e) => {
                    // Video upload failure is non-blocking; log error and continue with article-only publishing
                    eprintln!(
                        "[wechat_mp] Video material upload failed, will publish article-only: {}",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Step 2: Create draft.
        let mut content = Self::build_article_content(video);
        if let Some(ref media_id) = video_media_id {
            // Embed video player placeholder in body
            content.push_str(&format!(
                "<p><br></p><section><p>Video uploaded (media_id: {}),</p></section>",
                media_id,
            ));
        }

        let draft_req = DraftAddRequest {
            articles: vec![DraftArticle {
                title: if video.title.is_empty() {
                    format!("{} {} Review", video.instrument, video.freq)
                } else {
                    video.title.clone()
                },
                content,
                thumb_media_id: String::new(), // Empty when no cover image
                need_open_comment: 0,
            }],
        };

        let draft_resp = self
            .http
            .post(format!(
                "https://api.weixin.qq.com/cgi-bin/draft/add?access_token={}",
                access_token
            ))
            .json(&draft_req)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("WeChat draft creation failed: {}", e))?;

        let draft_body = draft_resp.text().await.unwrap_or_default();
        let draft: DraftAddResponse = serde_json::from_str(&draft_body).map_err(|e| {
            format!(
                "Failed to parse draft response: {} — body: {}",
                e, draft_body
            )
        })?;

        if draft.errcode.is_some_and(|c| c != 0) {
            return Ok(PublishResult {
                platform: "wechat_mp".into(),
                publish_id: String::new(),
                url: None,
                status: PublishStatus::Failed {
                    error: format!(
                        "WeChat draft creation error (errcode={}): {}",
                        draft.errcode.unwrap_or(0),
                        draft.errmsg.as_deref().unwrap_or("unknown"),
                    ),
                },
            });
        }

        let draft_media_id = draft.media_id.ok_or_else(|| {
            "WeChat draft created successfully but no media_id returned".to_string()
        })?;

        // Step 3: Publish draft.
        let publish_req = FreePublishRequest {
            media_id: draft_media_id.clone(),
        };

        let publish_resp = self
            .http
            .post(format!(
                "https://api.weixin.qq.com/cgi-bin/freepublish/submit?access_token={}",
                access_token
            ))
            .json(&publish_req)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("WeChat publish request failed: {}", e))?;

        let publish_body = publish_resp.text().await.unwrap_or_default();
        let publish: FreePublishResponse = serde_json::from_str(&publish_body).map_err(|e| {
            format!(
                "Failed to parse publish response: {} — body: {}",
                e, publish_body
            )
        })?;

        if publish.errcode.is_some_and(|c| c != 0) {
            let errcode = publish.errcode.unwrap_or(0);
            let hint = if errcode == 48004 {
                " (daily publish limit reached)"
            } else {
                ""
            };
            return Ok(PublishResult {
                platform: "wechat_mp".into(),
                publish_id: draft_media_id,
                url: None,
                status: PublishStatus::Failed {
                    error: format!(
                        "WeChat publish error (errcode={}): {}{}",
                        errcode,
                        publish.errmsg.as_deref().unwrap_or("unknown"),
                        hint,
                    ),
                },
            });
        }

        let publish_id = publish.publish_id.unwrap_or(draft_media_id);

        Ok(PublishResult {
            platform: "wechat_mp".into(),
            publish_id: publish_id.clone(),
            url: None, // Need to poll for URL after successful publishing
            status: PublishStatus::Processing, // WeChat publishing is asynchronous
        })
    }

    async fn status(&self, publish_id: &str) -> Result<PublishStatus, String> {
        let access_token = self.get_access_token().await?;

        let resp = self
            .http
            .post(format!(
                "https://api.weixin.qq.com/cgi-bin/freepublish/get?access_token={}",
                access_token
            ))
            .json(&serde_json::json!({ "publish_id": publish_id }))
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("WeChat publish status query failed: {}", e))?;

        let body = resp.text().await.unwrap_or_default();
        let status_resp: PublishStatusResponse = serde_json::from_str(&body).map_err(|e| {
            format!(
                "Failed to parse publish status response: {} — body: {}",
                e, body
            )
        })?;

        if status_resp.errcode.is_some_and(|c| c != 0) {
            return Ok(PublishStatus::Failed {
                error: format!(
                    "WeChat publish status query error (errcode={}): {}",
                    status_resp.errcode.unwrap_or(0),
                    status_resp.errmsg.as_deref().unwrap_or("unknown"),
                ),
            });
        }

        // publish_status: 0-publish succeeded, others-processing
        match status_resp.publish_status {
            Some(0) => {
                let url = status_resp.article_detail.and_then(|d| d.article_url);
                Ok(PublishStatus::Published {
                    url: url.unwrap_or_default(),
                })
            }
            Some(_) => Ok(PublishStatus::Processing),
            None => Ok(PublishStatus::Processing),
        }
    }
}

/// Upload video material to WeChat, returning media_id.
async fn upload_video_material(
    http: &Client,
    access_token: &str,
    video: &VideoAsset,
) -> Result<String, String> {
    let video_path = std::fs::canonicalize(&video.video_path).map_err(|e| {
        format!(
            "Failed to resolve video path {}: {}",
            video.video_path.display(),
            e
        )
    })?;
    let file_bytes = tokio::fs::read(&video_path)
        .await
        .map_err(|e| format!("Failed to read video file: {}", e))?;

    let filename = video
        .video_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video.mp4");

    let part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name(filename.to_string())
        .mime_str("video/mp4")
        .map_err(|e| format!("Failed to build multipart: {}", e))?;

    let form = reqwest::multipart::Form::new()
        .part("media", part)
        .text("description", video.title.clone());

    let resp = http
        .post(format!(
            "https://api.weixin.qq.com/cgi-bin/material/add_material?access_token={}&type=video",
            access_token
        ))
        .multipart(form)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("Video material upload request failed: {}", e))?;

    let body = resp.text().await.unwrap_or_default();
    let upload: MaterialUploadResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse material upload response: {}", e))?;

    if upload.errcode.is_some_and(|c| c != 0) {
        return Err(format!(
            "Video material upload error (errcode={}): {}",
            upload.errcode.unwrap_or(0),
            upload.errmsg.as_deref().unwrap_or("unknown"),
        ));
    }

    upload
        .media_id
        .ok_or_else(|| "Video material uploaded successfully but no media_id returned".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("A & B"), "A &amp; B");
        assert_eq!(escape_html("hello"), "hello");
    }

    #[test]
    fn test_build_article_content() {
        let asset = VideoAsset {
            id: "w1".into(),
            instrument: "ag2506".into(),
            freq: "5min".into(),
            date_range: crate::DateRange {
                start: chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                end: chrono::NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
            },
            duration_secs: 60.0,
            video_path: std::path::PathBuf::from("test.mp4"),
            video_size_bytes: 1024,
            title: "Silver Review".into(),
            description: "B2→S3 structure confirmed.\nVWAP crossed above.".into(),
            tags: vec!["futures".into(), "silver".into()],
            created_at: Utc::now(),
            thumbnail_path: None,
            category: None,
            content_body: None,
            seo_title: None,
            seo_description: None,
        };

        let content = WechatMpPublisher::build_article_content(&asset);
        assert!(content.contains("Silver Review"));
        assert!(content.contains("B2→S3"));
        assert!(content.contains("ag2506"));
        assert!(content.contains("5min"));
        assert!(content.contains("futures, silver"));
    }

    #[test]
    fn test_build_article_content_escapes_html() {
        let asset = VideoAsset {
            id: "w2".into(),
            instrument: "rb<i>".into(),
            freq: "5min".into(),
            date_range: crate::DateRange {
                start: chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                end: chrono::NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
            },
            duration_secs: 30.0,
            video_path: std::path::PathBuf::from("test.mp4"),
            video_size_bytes: 512,
            title: "H<1>ello".into(),
            description: "B<2>".into(),
            tags: vec![],
            created_at: Utc::now(),
            thumbnail_path: None,
            category: None,
            content_body: None,
            seo_title: None,
            seo_description: None,
        };

        let content = WechatMpPublisher::build_article_content(&asset);
        assert!(!content.contains("<i>"));
        assert!(!content.contains("<1>"));
        assert!(!content.contains("<2>"));
        assert!(content.contains("&lt;i&gt;"));
        assert!(content.contains("&lt;1&gt;"));
    }

    #[test]
    fn test_wechat_mp_publisher_new() {
        let p = WechatMpPublisher::new("app_id".into(), "secret".into());
        assert_eq!(p.platform_name(), "wechat_mp");
    }
}
