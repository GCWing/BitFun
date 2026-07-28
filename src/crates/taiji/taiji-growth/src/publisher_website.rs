use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::WebsiteConfig;

/// 构建结果 —— Zola / Hugo `build` 输出摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    /// 构建输出目录（Zola 的 /public 或 Hugo 的 /public）
    pub output_dir: PathBuf,
    /// 生成的页面总数
    pub page_count: u32,
    /// 构建耗时（秒）
    pub build_duration_secs: f64,
    /// 本次构建变更的文件列表（相对路径）
    pub changed_files: Vec<String>,
}

/// 部署状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployStatus {
    /// 排队等待部署
    Queued,
    /// 正在构建
    Building,
    /// 正在部署到目标平台
    Deploying,
    /// 已发布，url 为公开链接
    Published { url: String },
    /// 部署失败，error 为错误描述
    Failed { error: String },
}

/// 单平台部署结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    /// 部署 ID（UUID 或平台侧 job ID）
    pub deploy_id: String,
    /// 部署目标平台，如 "github-pages"
    pub platform: String,
    /// 公开链接（发布成功后）
    pub url: Option<String>,
    /// 当前部署状态
    pub status: DeployStatus,
}

/// 网站发布统一 trait。
///
/// 每个部署目标（GitHub Pages / Vercel / Netlify）独立实现。
/// 与 `PlatformPublisher`（taiji-publisher）职责互补：
/// `PlatformPublisher` 处理视频平台发布（B站/抖音/小红书），
/// `WebsitePublisher` 处理静态网站部署管道。
#[async_trait::async_trait]
pub trait WebsitePublisher: Send + Sync {
    /// 返回部署平台名称，如 "github-pages"
    fn platform_name(&self) -> &str;

    /// 执行 SSG 构建（Zola / Hugo build）。
    async fn build(&self, config: &WebsiteConfig) -> Result<BuildResult, String>;

    /// 将构建产物部署到目标平台。
    async fn deploy(&self) -> Result<DeployResult, String>;

    /// 查询部署状态。
    async fn status(&self, deploy_id: &str) -> Result<DeployStatus, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_result_roundtrip() {
        let result = BuildResult {
            output_dir: PathBuf::from("public"),
            page_count: 42,
            build_duration_secs: 0.85,
            changed_files: vec!["reports/ag2506/index.html".into(), "index.html".into()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: BuildResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.page_count, 42);
        assert_eq!(roundtrip.changed_files.len(), 2);
    }

    #[test]
    fn test_deploy_status_serde() {
        let published = DeployStatus::Published {
            url: "https://example.com".into(),
        };
        let json = serde_json::to_string(&published).unwrap();
        let roundtrip: DeployStatus = serde_json::from_str(&json).unwrap();
        match roundtrip {
            DeployStatus::Published { url } => assert!(url.contains("example.com")),
            _ => panic!("expected Published variant"),
        }

        let failed = DeployStatus::Failed {
            error: "timeout".into(),
        };
        let json = serde_json::to_string(&failed).unwrap();
        let roundtrip: DeployStatus = serde_json::from_str(&json).unwrap();
        match roundtrip {
            DeployStatus::Failed { error } => assert_eq!(error, "timeout"),
            _ => panic!("expected Failed variant"),
        }
    }

    #[test]
    fn test_deploy_result_roundtrip() {
        let result = DeployResult {
            deploy_id: "deploy-001".into(),
            platform: "github-pages".into(),
            url: None,
            status: DeployStatus::Deploying,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: DeployResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.deploy_id, "deploy-001");
        assert_eq!(roundtrip.platform, "github-pages");
        assert!(roundtrip.url.is_none());
    }
}
