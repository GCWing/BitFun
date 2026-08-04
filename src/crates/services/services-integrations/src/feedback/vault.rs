use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use async_trait::async_trait;
use rand::RngCore;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

const NONCE_LEN: usize = 12;

#[async_trait]
pub trait FeedbackCredentialStore: Send + Sync {
    async fn load(&self) -> Result<Option<String>>;
    async fn store(&self, value: &str) -> Result<()>;
}

pub struct FileFeedbackCredentialStore {
    key_path: PathBuf,
    data_path: PathBuf,
    lock: Mutex<()>,
}

impl FileFeedbackCredentialStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let directory = data_dir.join("feedback");
        Self {
            key_path: directory.join(".credentials.key"),
            data_path: directory.join("credentials.vault"),
            lock: Mutex::new(()),
        }
    }

    async fn ensure_key(&self) -> Result<[u8; 32]> {
        if self.key_path.exists() {
            return self.read_key().await;
        }
        if let Some(parent) = self.key_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("create feedback credential directory")?;
        }
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        let temporary_path = self.key_path.with_extension("key.tmp");
        tokio::fs::write(&temporary_path, key)
            .await
            .context("write feedback credential key")?;
        set_owner_only(&temporary_path);
        tokio::fs::rename(&temporary_path, &self.key_path)
            .await
            .context("replace feedback credential key")?;
        set_owner_only(&self.key_path);
        Ok(key)
    }

    async fn read_key(&self) -> Result<[u8; 32]> {
        let bytes = tokio::fs::read(&self.key_path)
            .await
            .context("read feedback credential key")?;
        if bytes.len() != 32 {
            anyhow::bail!("feedback credential key has invalid length");
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(key)
    }
}

#[async_trait]
impl FeedbackCredentialStore for FileFeedbackCredentialStore {
    async fn load(&self) -> Result<Option<String>> {
        let _guard = self.lock.lock().await;
        if !self.key_path.exists() || !self.data_path.exists() {
            return Ok(None);
        }
        let key = self.read_key().await?;
        let blob = tokio::fs::read(&self.data_path)
            .await
            .context("read feedback credential vault")?;
        if blob.len() <= NONCE_LEN {
            anyhow::bail!("feedback credential vault is truncated");
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|error| anyhow::anyhow!("initialize feedback vault cipher: {error}"))?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|error| anyhow::anyhow!("decrypt feedback credential vault: {error}"))?;
        String::from_utf8(plaintext)
            .context("decode feedback credential vault")
            .map(Some)
    }

    async fn store(&self, value: &str) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = self.ensure_key().await?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|error| anyhow::anyhow!("initialize feedback vault cipher: {error}"))?;
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), value.as_bytes())
            .map_err(|error| anyhow::anyhow!("encrypt feedback credential vault: {error}"))?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);

        if let Some(parent) = self.data_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("create feedback credential directory")?;
        }
        let temporary_path = self.data_path.with_extension("vault.tmp");
        tokio::fs::write(&temporary_path, blob)
            .await
            .context("write feedback credential vault")?;
        set_owner_only(&temporary_path);
        tokio::fs::rename(&temporary_path, &self.data_path)
            .await
            .context("replace feedback credential vault")?;
        set_owner_only(&self.data_path);
        Ok(())
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        log::warn!("Failed to restrict feedback credential file permissions: {error}");
    }
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::{FeedbackCredentialStore, FileFeedbackCredentialStore};

    #[tokio::test]
    async fn round_trips_encrypted_credentials() {
        let directory = std::env::temp_dir().join(format!(
            "bitfun-feedback-vault-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let vault = FileFeedbackCredentialStore::new(directory.clone());
        vault.store("capability-secret").await.unwrap();
        assert_eq!(
            vault.load().await.unwrap().as_deref(),
            Some("capability-secret")
        );
        let _ = tokio::fs::remove_dir_all(directory).await;
    }
}
