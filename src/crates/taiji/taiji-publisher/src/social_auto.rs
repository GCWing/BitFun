use crate::process_util::{create_command, run_with_timeout, sanitize_cli_arg};
use crate::{PlatformPublisher, PublishResult, PublishStatus, VideoAsset};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Default timeout for subprocess invocations (30 seconds).
const DEFAULT_CMD_TIMEOUT: Duration = Duration::from_secs(30);

/// Supported social media platforms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SocialPlatform {
    Douyin,
    Xiaohongshu,
}

impl SocialPlatform {
    pub fn as_str(&self) -> &str {
        match self {
            SocialPlatform::Douyin => "douyin",
            SocialPlatform::Xiaohongshu => "xiaohongshu",
        }
    }

    /// Parse platform from string (non-trait method to avoid conflict with std::str::FromStr).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "douyin" => Some(SocialPlatform::Douyin),
            "xiaohongshu" => Some(SocialPlatform::Xiaohongshu),
            _ => None,
        }
    }
}

/// social-auto-upload multi-platform publishing adapter.
/// Wraps social-auto-upload CLI (Playwright automation).
pub struct SocialPublisher {
    platform: SocialPlatform,
    python_bin: PathBuf,
    script_path: PathBuf,
    cookie_dir: PathBuf,
}

impl SocialPublisher {
    pub fn new(
        platform: SocialPlatform,
        python_bin: PathBuf,
        script_path: PathBuf,
        cookie_dir: PathBuf,
    ) -> Self {
        Self {
            platform,
            python_bin,
            script_path,
            cookie_dir,
        }
    }

    /// Probe whether the specified platform is available (check Python + social-auto-upload script existence).
    ///
    /// Uses [`create_command`] (which adds `CREATE_NO_WINDOW` on Windows) and
    /// [`run_with_timeout`] so a hung `python --version` cannot block the caller.
    /// TODO: migrate to `bitfun_services_core::process_manager::create_command`
    ///       once taiji-publisher depends on services-core.
    pub fn probe(&self) -> Result<bool, String> {
        // Check Python availability
        let mut cmd = create_command(&self.python_bin);
        cmd.arg("--version");
        run_with_timeout(cmd, DEFAULT_CMD_TIMEOUT)
            .map_err(|e| format!("Python unavailable: {}", e))?;

        // Check upload script exists
        if !self.script_path.exists() {
            return Ok(false);
        }
        Ok(true)
    }

    /// Build upload command arguments
    fn build_upload_args(&self, video: &VideoAsset) -> Vec<String> {
        // Canonicalize to prevent path traversal
        let resolved_path =
            fs::canonicalize(&video.video_path).unwrap_or_else(|_| video.video_path.clone());
        let mut args = vec![
            self.script_path.to_string_lossy().to_string(),
            self.platform.as_str().to_string(),
            resolved_path.to_string_lossy().to_string(),
            "--cookie-dir".to_string(),
            self.cookie_dir.to_string_lossy().to_string(),
        ];
        if !video.title.is_empty() {
            args.push("--title".to_string());
            args.push(sanitize_cli_arg(&video.title));
        }
        if !video.description.is_empty() {
            args.push("--desc".to_string());
            args.push(sanitize_cli_arg(&video.description));
        }
        if !video.tags.is_empty() {
            args.push("--tags".to_string());
            args.push(sanitize_cli_arg(&video.tags.join(",")));
        }
        args
    }
}

#[async_trait::async_trait]
impl PlatformPublisher for SocialPublisher {
    fn platform_name(&self) -> &str {
        self.platform.as_str()
    }

    async fn check_auth(&self) -> Result<bool, String> {
        // Check if cookie directory has a valid Playwright session
        if !self.cookie_dir.exists() {
            return Ok(false);
        }
        // Simple check: non-empty directory implies valid session
        let entries = std::fs::read_dir(&self.cookie_dir)
            .map_err(|e| format!("Cannot read cookie directory: {}", e))?;
        Ok(entries.count() > 0)
    }

    async fn upload(&self, video: &VideoAsset) -> Result<PublishResult, String> {
        // 1. Probe availability
        if !self.probe()? {
            return Ok(PublishResult {
                platform: self.platform.as_str().into(),
                publish_id: String::new(),
                url: None,
                status: PublishStatus::Failed {
                    error: format!("{} platform unavailable: please install social-auto-upload and configure Playwright", self.platform.as_str()),
                },
            });
        }

        // 2. Check authentication
        if !self.check_auth().await? {
            return Ok(PublishResult {
                platform: self.platform.as_str().into(),
                publish_id: String::new(),
                url: None,
                status: PublishStatus::Failed {
                    error: format!("{} Cookie expired, please re-login", self.platform.as_str()),
                },
            });
        }

        // 3. Execute upload script
        let args = self.build_upload_args(video);
        let mut cmd = create_command(&self.python_bin);
        cmd.args(&args);
        let output = run_with_timeout(cmd, DEFAULT_CMD_TIMEOUT)
            .map_err(|e| format!("social-auto-upload execution failed: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            let err_msg = if stderr.contains("验证码") || stderr.contains("captcha") {
                format!(
                    "{} platform triggered CAPTCHA, please login manually and retry",
                    self.platform.as_str()
                )
            } else {
                format!(
                    "{} upload failed: {}",
                    self.platform.as_str(),
                    stderr.trim()
                )
            };
            return Ok(PublishResult {
                platform: self.platform.as_str().into(),
                publish_id: String::new(),
                url: None,
                status: PublishStatus::Failed { error: err_msg },
            });
        }

        // 4. Extract result URL
        let url = extract_social_url(&stdout);

        Ok(PublishResult {
            platform: self.platform.as_str().into(),
            publish_id: url.clone().unwrap_or_default(),
            url,
            status: PublishStatus::Uploading {
                progress_pct: 100.0,
            },
        })
    }

    async fn status(&self, _publish_id: &str) -> Result<PublishStatus, String> {
        // social-auto-upload does not support real-time status queries
        Ok(PublishStatus::Processing)
    }
}

fn extract_social_url(stdout: &str) -> Option<String> {
    // Try JSON parsing first (more precise)
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(stdout) {
        if let Some(url) = v.get("url").and_then(|u| u.as_str()) {
            return Some(url.to_string());
        }
    }
    // Fall back to line-by-line URL scanning
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find("http") {
            let url_part = &trimmed[pos..];
            let end = url_part
                .find(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == '}' || c == ']')
                .unwrap_or(url_part.len());
            return Some(url_part[..end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_social_platform_as_str() {
        assert_eq!(SocialPlatform::Douyin.as_str(), "douyin");
        assert_eq!(SocialPlatform::Xiaohongshu.as_str(), "xiaohongshu");
    }

    #[test]
    fn test_social_platform_from_str() {
        assert_eq!(
            SocialPlatform::parse("douyin"),
            Some(SocialPlatform::Douyin)
        );
        assert_eq!(
            SocialPlatform::parse("xiaohongshu"),
            Some(SocialPlatform::Xiaohongshu)
        );
        assert_eq!(SocialPlatform::parse("youtube"), None);
    }

    #[test]
    fn test_extract_url_from_stdout() {
        assert_eq!(
            extract_social_url("Uploaded: https://www.douyin.com/video/12345"),
            Some("https://www.douyin.com/video/12345".into())
        );
    }

    #[test]
    fn test_extract_url_from_json() {
        let json = r#"{"url": "https://www.xiaohongshu.com/explore/abc", "status": "ok"}"#;
        assert_eq!(
            extract_social_url(json),
            Some("https://www.xiaohongshu.com/explore/abc".into())
        );
    }

    #[test]
    fn test_extract_url_not_found() {
        assert_eq!(extract_social_url("no url here"), None);
    }

    #[test]
    fn test_social_publisher_new() {
        let publisher = SocialPublisher::new(
            SocialPlatform::Douyin,
            PathBuf::from("python"),
            PathBuf::from("scripts/publish/social_upload.py"),
            PathBuf::from("cookies/"),
        );
        assert_eq!(publisher.platform_name(), "douyin");
    }
}
