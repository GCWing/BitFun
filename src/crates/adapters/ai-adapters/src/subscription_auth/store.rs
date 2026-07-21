//! On-disk persistence for subscription auth credentials.
//!
//! Tokens live in a single JSON document keyed by provider id
//! (`codex` / `antigravity` / `opencode`). The file is written with mode
//! `0600` on Unix so other local users cannot read the stored tokens.
//!
//! Path: `{dirs::config_dir()}/bitfun/data/subscription_auth.json`.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

/// A single stored credential for one provider.
///
/// `oauth` credentials keep the refresh/access token pair plus the millisecond
/// epoch expiry; `api` credentials keep a static API key. Both may carry an
/// opaque `metadata` object (email, org info, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StoredCredential {
    Oauth {
        refresh: String,
        access: String,
        /// Milliseconds since the Unix epoch when `access` expires.
        expires: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
    Api {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
}

/// Provider id -> stored credential.
pub type Store = HashMap<String, StoredCredential>;

fn store_path_override() -> &'static RwLock<Option<PathBuf>> {
    static OVERRIDE: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| RwLock::new(None))
}

/// Overrides the store path for tests. Pass a temp-dir file path.
pub fn set_store_path_for_test(path: PathBuf) {
    if let Ok(mut guard) = store_path_override().write() {
        *guard = Some(path);
    }
}

fn store_path() -> Result<PathBuf> {
    if let Ok(guard) = store_path_override().read() {
        if let Some(path) = guard.as_ref() {
            return Ok(path.clone());
        }
    }
    let base = dirs::config_dir().ok_or_else(|| anyhow!("system config directory unavailable"))?;
    Ok(base
        .join("bitfun")
        .join("data")
        .join("subscription_auth.json"))
}

/// Loads the full credential store. Returns an empty store when the file does
/// not exist yet.
pub async fn load() -> Result<Store> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(Store::new());
    }
    let bytes = tokio::fs::read(&path)
        .await
        .with_context(|| format!("read subscription auth store at {}", path.display()))?;
    if bytes.is_empty() {
        return Ok(Store::new());
    }
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parse subscription auth store at {}", path.display()))
}

/// Persists the full credential store with restrictive permissions.
pub async fn save(store: &Store) -> Result<()> {
    let path = store_path()?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create subscription auth store dir {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(store)?;
    write_atomic(&path, &bytes).await
}

/// Writes `bytes` to `path` atomically (temp file + rename) so a crash
/// mid-write cannot corrupt the token store. On Unix the temp file is created
/// with mode `0600` up front, so tokens are never briefly world-readable.
async fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let tmp = path.with_extension("tmp");
    #[cfg(unix)]
    let open_options = {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        options
    };
    #[cfg(not(unix))]
    let open_options = {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        options
    };

    {
        let mut file = open_options
            .open(&tmp)
            .await
            .with_context(|| format!("create subscription auth temp file {}", tmp.display()))?;
        file.write_all(bytes)
            .await
            .with_context(|| format!("write subscription auth temp file {}", tmp.display()))?;
        file.sync_all()
            .await
            .with_context(|| format!("sync subscription auth temp file {}", tmp.display()))?;
    }
    tokio::fs::rename(&tmp, path).await.with_context(|| {
        format!(
            "rename subscription auth store {} -> {}",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}
