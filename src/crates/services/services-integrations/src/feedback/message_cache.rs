use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use bitfun_product_domains::feedback::FeedbackMessage;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const CACHE_VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct MessageCacheData {
    version: u8,
    pub(super) feedback_id: String,
    pub(super) messages: Vec<FeedbackMessage>,
    #[serde(default)]
    pub(super) sync_cursor: Option<String>,
    #[serde(default)]
    pub(super) sync_complete: bool,
}

impl MessageCacheData {
    pub(super) fn empty(feedback_id: &str) -> Self {
        Self {
            version: CACHE_VERSION,
            feedback_id: feedback_id.to_string(),
            messages: Vec::new(),
            sync_cursor: None,
            sync_complete: false,
        }
    }
}

pub(super) struct MessageCache {
    directory: PathBuf,
    key: [u8; 32],
}

impl MessageCache {
    pub(super) fn new(directory: PathBuf, key: [u8; 32]) -> Self {
        Self { directory, key }
    }

    pub(super) async fn load(&self, feedback_id: &str) -> Result<Option<MessageCacheData>> {
        let path = self.path(feedback_id);
        if !path.exists() {
            return Ok(None);
        }
        let blob = tokio::fs::read(&path)
            .await
            .context("read feedback message cache")?;
        if blob.len() <= NONCE_LEN {
            anyhow::bail!("feedback message cache is truncated");
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|error| anyhow::anyhow!("initialize feedback cache cipher: {error}"))?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: feedback_id.as_bytes(),
                },
            )
            .map_err(|error| anyhow::anyhow!("decrypt feedback message cache: {error}"))?;
        let data: MessageCacheData =
            serde_json::from_slice(&plaintext).context("decode feedback message cache")?;
        if data.version != CACHE_VERSION || data.feedback_id != feedback_id {
            anyhow::bail!("feedback message cache identity is invalid");
        }
        Ok(Some(data))
    }

    pub(super) async fn store(&self, data: &MessageCacheData) -> Result<()> {
        tokio::fs::create_dir_all(&self.directory)
            .await
            .context("create feedback message cache directory")?;
        let plaintext = serde_json::to_vec(data).context("encode feedback message cache")?;
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|error| anyhow::anyhow!("initialize feedback cache cipher: {error}"))?;
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: data.feedback_id.as_bytes(),
                },
            )
            .map_err(|error| anyhow::anyhow!("encrypt feedback message cache: {error}"))?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);

        let path = self.path(&data.feedback_id);
        let temporary_path = path.with_extension("cache.tmp");
        tokio::fs::write(&temporary_path, blob)
            .await
            .context("write feedback message cache")?;
        set_owner_only(&temporary_path);
        tokio::fs::rename(&temporary_path, &path)
            .await
            .context("replace feedback message cache")?;
        set_owner_only(&path);
        Ok(())
    }

    pub(super) async fn remove(&self, feedback_id: &str) -> Result<()> {
        let path = self.path(feedback_id);
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("remove feedback message cache"),
        }
    }

    fn path(&self, feedback_id: &str) -> PathBuf {
        let digest = Sha256::digest(feedback_id.as_bytes());
        self.directory.join(format!("{:x}.cache", digest))
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        log::warn!("Failed to restrict feedback message cache permissions: {error}");
    }
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::{MessageCache, MessageCacheData};
    use bitfun_product_domains::feedback::{FeedbackMessage, FeedbackSender};

    #[tokio::test]
    async fn encrypts_message_content_and_rejects_corruption() {
        let directory = std::env::temp_dir().join(format!(
            "bitfun-feedback-message-cache-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let cache = MessageCache::new(directory.clone(), [7u8; 32]);
        let mut data = MessageCacheData::empty("feedback-1");
        data.messages.push(FeedbackMessage {
            message_id: "message-1".to_string(),
            sender: FeedbackSender::Admin,
            content: "private message body".to_string(),
            content_deleted: true,
            created_at: "2026-07-28T01:00:00Z".to_string(),
        });
        data.sync_cursor = Some("cursor-1".to_string());
        data.sync_complete = true;

        cache.store(&data).await.unwrap();
        let path = cache.path("feedback-1");
        let encrypted = tokio::fs::read(&path).await.unwrap();
        assert!(!String::from_utf8_lossy(&encrypted).contains("private message body"));
        let restored = cache.load("feedback-1").await.unwrap().unwrap();
        assert_eq!(restored.messages, data.messages);

        tokio::fs::write(&path, b"corrupt").await.unwrap();
        assert!(cache.load("feedback-1").await.is_err());
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[test]
    fn defaults_deleted_marker_for_legacy_cached_messages() {
        let message: FeedbackMessage = serde_json::from_value(serde_json::json!({
            "messageId": "message-legacy",
            "sender": "admin",
            "content": "legacy content",
            "createdAt": "2026-07-28T01:00:00Z"
        }))
        .unwrap();

        assert!(!message.content_deleted);
    }
}
