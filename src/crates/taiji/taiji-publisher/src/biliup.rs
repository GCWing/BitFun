use crate::process_util::{create_command, run_with_timeout, sanitize_cli_arg};
use crate::{PlatformPublisher, PublishResult, PublishStatus, VideoAsset};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Default timeout for subprocess invocations (60 seconds — biliup uploads
/// may take longer than a simple probe).
const DEFAULT_CMD_TIMEOUT: Duration = Duration::from_secs(60);

/// Bilibili publishing adapter — wraps the biliup CLI.
pub struct BiliupPublisher {
    /// Path to biliup CLI executable
    biliup_bin: PathBuf,
    /// Path to Bilibili cookie file
    cookie_path: PathBuf,
}

impl BiliupPublisher {
    pub fn new(biliup_bin: PathBuf, cookie_path: PathBuf) -> Self {
        Self {
            biliup_bin,
            cookie_path,
        }
    }

    /// Check if biliup CLI is available.
    ///
    /// Uses [`create_command`] (which adds `CREATE_NO_WINDOW` on Windows) and
    /// [`run_with_timeout`] so a hung `biliup --version` cannot block the caller.
    /// TODO: migrate to `bitfun_services_core::process_manager::create_command`
    ///       once taiji-publisher depends on services-core.
    pub fn check_cli(&self) -> Result<String, String> {
        let mut cmd = create_command(&self.biliup_bin);
        cmd.arg("--version");
        let output = run_with_timeout(cmd, DEFAULT_CMD_TIMEOUT)
            .map_err(|e| format!("biliup CLI unavailable: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err("biliup --version returned non-zero exit code".into())
        }
    }

    /// Check if cookie file exists and is non-empty
    pub fn check_cookie(&self) -> Result<bool, String> {
        if !self.cookie_path.exists() {
            return Ok(false);
        }
        let meta = std::fs::metadata(&self.cookie_path)
            .map_err(|e| format!("Cannot read cookie file: {}", e))?;
        Ok(meta.len() > 0)
    }

    /// Build biliup upload command arguments
    fn build_upload_args(&self, video: &VideoAsset) -> Vec<String> {
        // Canonicalize to prevent path traversal
        let resolved_path =
            fs::canonicalize(&video.video_path).unwrap_or_else(|_| video.video_path.clone());
        let mut args = vec![
            "upload".to_string(),
            "--cookie".to_string(),
            self.cookie_path.to_string_lossy().to_string(),
            resolved_path.to_string_lossy().to_string(),
            "--title".to_string(),
            sanitize_cli_arg(&video.title),
        ];
        if !video.tags.is_empty() {
            args.push("--tag".to_string());
            args.push(sanitize_cli_arg(&video.tags.join(",")));
        }
        if !video.description.is_empty() {
            args.push("--desc".to_string());
            args.push(sanitize_cli_arg(&video.description));
        }
        args
    }
}

#[async_trait::async_trait]
impl PlatformPublisher for BiliupPublisher {
    fn platform_name(&self) -> &str {
        "bilibili"
    }

    async fn check_auth(&self) -> Result<bool, String> {
        self.check_cookie()
    }

    async fn upload(&self, video: &VideoAsset) -> Result<PublishResult, String> {
        // 1. Check authentication
        if !self.check_cookie()? {
            return Ok(PublishResult {
                platform: "bilibili".into(),
                publish_id: String::new(),
                url: None,
                status: PublishStatus::Failed {
                    error: "Cookie expired, please re-export and place in biliup cookie path"
                        .into(),
                },
            });
        }

        // 2. Execute biliup upload
        let args = self.build_upload_args(video);
        let mut cmd = create_command(&self.biliup_bin);
        cmd.args(&args);
        let output = run_with_timeout(cmd, DEFAULT_CMD_TIMEOUT)
            .map_err(|e| format!("biliup execution failed: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Ok(PublishResult {
                platform: "bilibili".into(),
                publish_id: String::new(),
                url: None,
                status: PublishStatus::Failed {
                    error: format!("biliup upload failed: {}", stderr.trim()),
                },
            });
        }

        // 3. Extract BV number from stdout (biliup output format: "https://www.bilibili.com/video/BVxxxx")
        let bv = extract_bv(&stdout);

        Ok(PublishResult {
            platform: "bilibili".into(),
            publish_id: bv.clone(),
            url: if bv.is_empty() {
                None
            } else {
                Some(format!("https://www.bilibili.com/video/{}", bv))
            },
            status: PublishStatus::Uploading {
                progress_pct: 100.0,
            },
        })
    }

    async fn status(&self, _publish_id: &str) -> Result<PublishStatus, String> {
        // biliup does not support real-time status queries; return Processing
        Ok(PublishStatus::Processing)
    }
}

/// Extract BV number from biliup stdout
fn extract_bv(stdout: &str) -> String {
    for line in stdout.lines() {
        if let Some(pos) = line.find("BV") {
            let bv_part = &line[pos..];
            let bv: String = bv_part
                .chars()
                .take_while(|c| c.is_alphanumeric())
                .collect();
            if bv.starts_with("BV") && bv.len() >= 12 {
                return bv;
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bv_from_url() {
        let stdout = "Upload success!\nhttps://www.bilibili.com/video/BV1xx411c7mD\n";
        assert_eq!(extract_bv(stdout), "BV1xx411c7mD");
    }

    #[test]
    fn test_extract_bv_not_found() {
        assert_eq!(extract_bv("no bv here"), "");
    }

    #[test]
    fn test_biliup_publisher_new() {
        let publisher =
            BiliupPublisher::new(PathBuf::from("biliup"), PathBuf::from("cookies.json"));
        assert_eq!(publisher.platform_name(), "bilibili");
    }
}
