use anyhow::{anyhow, Context};
use std::collections::HashSet;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MarketConfig {
    pub bind: SocketAddr,
    pub public_base_url: String,
    pub database_path: PathBuf,
    pub artifact_dir: PathBuf,
    pub web_dir: PathBuf,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub session_secret: String,
    pub admin_github_ids: HashSet<i64>,
    pub public_browse: bool,
    pub web_submissions_enabled: bool,
}

impl MarketConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind = env::var("MARKET_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9710".to_string())
            .parse()
            .context("MARKET_BIND must be a socket address")?;
        let public_base_url = env::var("MARKET_PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9710/miniapp".to_string())
            .trim_end_matches('/')
            .to_string();
        let data_dir = PathBuf::from(
            env::var("MARKET_DATA_DIR").unwrap_or_else(|_| "./var/miniapp-market/data".to_string()),
        );
        let database_path = env::var("MARKET_DATABASE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("market.sqlite"));
        let artifact_dir = PathBuf::from(
            env::var("MARKET_ARTIFACT_DIR")
                .unwrap_or_else(|_| "./var/miniapp-market/artifacts".to_string()),
        );
        let web_dir = PathBuf::from(
            env::var("MARKET_WEB_DIR")
                .unwrap_or_else(|_| "./src/miniapp-market-web/dist".to_string()),
        );
        let session_secret = env::var("MARKET_SESSION_SECRET")
            .unwrap_or_else(|_| "development-only-change-me".to_string());
        if session_secret.len() < 24 && !cfg!(debug_assertions) {
            return Err(anyhow!(
                "MARKET_SESSION_SECRET must contain at least 24 characters"
            ));
        }
        let admin_github_ids = env::var("MARKET_ADMIN_GITHUB_IDS")
            .unwrap_or_else(|_| "24753352".to_string())
            .split(',')
            .filter_map(|value| value.trim().parse::<i64>().ok())
            .collect();
        let public_browse = env::var("MARKET_PUBLIC_BROWSE")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes"))
            .unwrap_or(true);
        let web_submissions_enabled = env::var("MARKET_WEB_SUBMISSIONS_ENABLED")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes"))
            .unwrap_or(false);

        Ok(Self {
            bind,
            public_base_url,
            database_path,
            artifact_dir,
            web_dir,
            github_client_id: non_empty_env("MARKET_GITHUB_CLIENT_ID"),
            github_client_secret: non_empty_env("MARKET_GITHUB_CLIENT_SECRET"),
            session_secret,
            admin_github_ids,
            public_browse,
            web_submissions_enabled,
        })
    }

    pub fn github_configured(&self) -> bool {
        self.github_client_id.is_some() && self.github_client_secret.is_some()
    }

    pub fn github_callback_url(&self) -> String {
        format!("{}/api/v1/auth/github/callback", self.public_base_url)
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
