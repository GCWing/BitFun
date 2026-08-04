use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use bitfun_product_domains::feedback::FeedbackRecordSummary;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const CACHE_VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const STATE_CACHE_AAD: &[u8] = b"bitfun-feedback-state-cache-v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct FeedbackStateCacheData {
    pub(super) version: u8,
    #[serde(default)]
    pub(super) enroll_idempotency_key: Option<String>,
    #[serde(default)]
    pub(super) refresh_idempotency_key: Option<String>,
    #[serde(default)]
    pub(super) anonymous_id: Option<String>,
    #[serde(default)]
    pub(super) pending_create_fingerprint: Option<String>,
    #[serde(default)]
    pub(super) pending_create_idempotency_key: Option<String>,
    #[serde(default)]
    pub(super) inbox_items: Vec<FeedbackRecordSummary>,
    #[serde(default)]
    pub(super) inbox_next_cursor: Option<String>,
    #[serde(default)]
    pub(super) inbox_has_more: bool,
    #[serde(default)]
    pub(super) read_cursors: HashMap<String, String>,
    #[serde(default)]
    pub(super) pending_reply_fingerprints: HashMap<String, String>,
    #[serde(default)]
    pub(super) pending_reply_idempotency_keys: HashMap<String, String>,
}

impl FeedbackStateCacheData {
    pub(super) fn with_current_version(mut self) -> Self {
        self.version = CACHE_VERSION;
        self
    }
}

pub(super) struct FeedbackStateCache {
    directory: PathBuf,
    key: [u8; 32],
}

impl FeedbackStateCache {
    pub(super) fn new(directory: PathBuf, key: [u8; 32]) -> Self {
        Self { directory, key }
    }

    pub(super) async fn load(&self) -> Result<Option<FeedbackStateCacheData>> {
        let path = self.path();
        if !path.exists() {
            return Ok(None);
        }
        let blob = tokio::fs::read(&path)
            .await
            .context("read feedback state cache")?;
        if blob.len() <= NONCE_LEN {
            anyhow::bail!("feedback state cache is truncated");
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|error| anyhow::anyhow!("initialize feedback state cache cipher: {error}"))?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: STATE_CACHE_AAD,
                },
            )
            .map_err(|error| anyhow::anyhow!("decrypt feedback state cache: {error}"))?;
        let data: FeedbackStateCacheData =
            serde_json::from_slice(&plaintext).context("decode feedback state cache")?;
        if data.version != CACHE_VERSION {
            anyhow::bail!("feedback state cache version is invalid");
        }
        Ok(Some(data))
    }

    pub(super) async fn store(&self, data: &FeedbackStateCacheData) -> Result<()> {
        tokio::fs::create_dir_all(&self.directory)
            .await
            .context("create feedback state cache directory")?;
        let plaintext = serde_json::to_vec(data).context("encode feedback state cache")?;
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|error| anyhow::anyhow!("initialize feedback state cache cipher: {error}"))?;
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: STATE_CACHE_AAD,
                },
            )
            .map_err(|error| anyhow::anyhow!("encrypt feedback state cache: {error}"))?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);

        let path = self.path();
        let temporary_path = path.with_extension("cache.tmp");
        tokio::fs::write(&temporary_path, blob)
            .await
            .context("write feedback state cache")?;
        set_owner_only(&temporary_path);
        tokio::fs::rename(&temporary_path, &path)
            .await
            .context("replace feedback state cache")?;
        set_owner_only(&path);
        Ok(())
    }

    pub(super) async fn remove(&self) -> Result<()> {
        match tokio::fs::remove_file(self.path()).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("remove feedback state cache"),
        }
    }

    fn path(&self) -> PathBuf {
        self.directory.join("state.cache")
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        log::warn!("Failed to restrict feedback state cache permissions: {error}");
    }
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::{FeedbackStateCache, FeedbackStateCacheData};
    use bitfun_product_domains::feedback::{
        FeedbackCategory, FeedbackRecordSummary, FeedbackStatus,
    };

    #[tokio::test]
    async fn encrypts_state_and_treats_corruption_as_invalid_cache() {
        let directory = std::env::temp_dir().join(format!(
            "bitfun-feedback-state-cache-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let cache = FeedbackStateCache::new(directory.clone(), [9u8; 32]);
        let data = FeedbackStateCacheData {
            inbox_items: vec![FeedbackRecordSummary {
                feedback_id: "feedback-1".to_string(),
                category: FeedbackCategory::Other,
                status: FeedbackStatus::Submitted,
                has_new_reply: false,
                created_at: "2026-07-30T01:00:00Z".to_string(),
                updated_at: "2026-07-30T01:00:00Z".to_string(),
                can_open: true,
            }],
            ..FeedbackStateCacheData::default()
        }
        .with_current_version();

        cache.store(&data).await.unwrap();
        let encrypted = tokio::fs::read(cache.path()).await.unwrap();
        assert!(!String::from_utf8_lossy(&encrypted).contains("feedback-1"));
        let restored = cache.load().await.unwrap().unwrap();
        assert_eq!(restored.inbox_items, data.inbox_items);

        tokio::fs::write(cache.path(), b"corrupt").await.unwrap();
        assert!(cache.load().await.is_err());
        let _ = tokio::fs::remove_dir_all(directory).await;
    }
}
