use anyhow::{anyhow, Context};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Clone)]
pub struct SkinMarketConfig {
    pub bind: SocketAddr,
    pub public_base_url: String,
    pub database_path: PathBuf,
    pub artifact_dir: PathBuf,
    pub web_dir: PathBuf,
    pub identity_me_url: Url,
    pub download_hash_secret: String,
    pub public_browse: bool,
}

impl SkinMarketConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind = env::var("SKIN_MARKET_BIND")
            .unwrap_or_else(|_| "127.0.0.1:9720".to_string())
            .parse()
            .context("SKIN_MARKET_BIND must be a socket address")?;
        let public_base_url = env::var("SKIN_MARKET_PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9720/skin".to_string())
            .trim_end_matches('/')
            .to_string();
        let public_url = Url::parse(&public_base_url)
            .context("SKIN_MARKET_PUBLIC_BASE_URL must be an absolute URL")?;
        if !matches!(public_url.scheme(), "http" | "https") {
            return Err(anyhow!(
                "SKIN_MARKET_PUBLIC_BASE_URL must use http or https"
            ));
        }
        if !public_url.username().is_empty() || public_url.password().is_some() {
            return Err(anyhow!(
                "SKIN_MARKET_PUBLIC_BASE_URL must not contain credentials"
            ));
        }
        if !cfg!(debug_assertions) && public_url.scheme() != "https" {
            return Err(anyhow!(
                "SKIN_MARKET_PUBLIC_BASE_URL must use https in release builds"
            ));
        }
        let identity_me_url = Url::parse(
            &env::var("SKIN_MARKET_IDENTITY_ME_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:9710/miniapp/api/v1/me".to_string()),
        )
        .context("SKIN_MARKET_IDENTITY_ME_URL must be an absolute URL")?;
        if !matches!(identity_me_url.scheme(), "http" | "https") {
            return Err(anyhow!(
                "SKIN_MARKET_IDENTITY_ME_URL must use http or https"
            ));
        }
        if !identity_me_url.username().is_empty() || identity_me_url.password().is_some() {
            return Err(anyhow!(
                "SKIN_MARKET_IDENTITY_ME_URL must not contain credentials"
            ));
        }
        if !cfg!(debug_assertions) && identity_me_url.scheme() != "https" {
            return Err(anyhow!(
                "SKIN_MARKET_IDENTITY_ME_URL must use https in release builds"
            ));
        }
        let data_dir = PathBuf::from(
            env::var("SKIN_MARKET_DATA_DIR")
                .unwrap_or_else(|_| "./var/skin-market/data".to_string()),
        );
        const DEVELOPMENT_SECRET: &str = "development-only-change-me";
        const EXAMPLE_SECRET: &str = "replace-with-at-least-32-random-characters";
        let download_hash_secret = match env::var("SKIN_MARKET_DOWNLOAD_HASH_SECRET") {
            Ok(value) => value,
            Err(_) if cfg!(debug_assertions) => DEVELOPMENT_SECRET.to_string(),
            Err(_) => {
                return Err(anyhow!(
                    "SKIN_MARKET_DOWNLOAD_HASH_SECRET is required in release builds"
                ))
            }
        };
        if !cfg!(debug_assertions)
            && (download_hash_secret.len() < 32
                || download_hash_secret.trim() != download_hash_secret
                || matches!(
                    download_hash_secret.as_str(),
                    DEVELOPMENT_SECRET | EXAMPLE_SECRET
                ))
        {
            return Err(anyhow!(
                "SKIN_MARKET_DOWNLOAD_HASH_SECRET must contain at least 32 non-default characters"
            ));
        }
        Ok(Self {
            bind,
            public_base_url,
            database_path: env::var("SKIN_MARKET_DATABASE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| data_dir.join("market.sqlite")),
            artifact_dir: PathBuf::from(
                env::var("SKIN_MARKET_ARTIFACT_DIR")
                    .unwrap_or_else(|_| "./var/skin-market/artifacts".to_string()),
            ),
            web_dir: PathBuf::from(
                env::var("SKIN_MARKET_WEB_DIR")
                    .unwrap_or_else(|_| "./src/skin-market-web/dist".to_string()),
            ),
            identity_me_url,
            download_hash_secret,
            public_browse: env::var("SKIN_MARKET_PUBLIC_BROWSE")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes"))
                .unwrap_or(true),
        })
    }
}
