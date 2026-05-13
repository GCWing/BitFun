//! Encrypted file-backed storage for SSH password authentication.
//!
//! A random 32-byte key lives in `data_dir/.ssh_password_vault.key` (0600 on Unix).
//! Ciphertext map is stored in `data_dir/ssh_password_vault.json`.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::Mutex;

const NONCE_LEN: usize = 12;

#[derive(Serialize, Deserialize, Default)]
struct VaultFile {
    entries: HashMap<String, String>,
}

pub struct SSHPasswordVault {
    key_path: PathBuf,
    vault_path: PathBuf,
    lock: Mutex<()>,
}

impl SSHPasswordVault {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            key_path: data_dir.join(".ssh_password_vault.key"),
            vault_path: data_dir.join("ssh_password_vault.json"),
            lock: Mutex::new(()),
        }
    }

    async fn ensure_key(&self) -> Result<[u8; 32]> {
        if self.key_path.exists() {
            let bytes = tokio::fs::read(&self.key_path)
                .await
                .context("read ssh password vault key")?;
            if bytes.len() != 32 {
                anyhow::bail!("invalid ssh password vault key length");
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
        if let Some(p) = self.key_path.parent() {
            tokio::fs::create_dir_all(p).await?;
        }
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        tokio::fs::write(&self.key_path, key.as_slice())
            .await
            .context("write ssh password vault key")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.key_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(key)
    }

    fn encrypt_password(key: &[u8; 32], plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ct = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encrypt: {}", e))?;
        let mut blob = Vec::with_capacity(NONCE_LEN + ct.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ct);
        Ok(B64.encode(blob))
    }

    fn decrypt_password(key: &[u8; 32], blob_b64: &str) -> Result<String> {
        let blob = B64
            .decode(blob_b64)
            .context("base64 decode ssh vault entry")?;
        if blob.len() <= NONCE_LEN {
            anyhow::bail!("ssh vault entry too short");
        }
        let (nonce, ct) = blob.split_at(NONCE_LEN);
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("{}", e))?;
        let pt = cipher
            .decrypt(Nonce::from_slice(nonce), ct)
            .map_err(|e| anyhow::anyhow!("decrypt: {}", e))?;
        String::from_utf8(pt).context("utf8 decode ssh vault password")
    }

    pub async fn store(&self, connection_id: &str, password: &str) -> Result<()> {
        let _g = self.lock.lock().await;
        let key = self.ensure_key().await?;
        let mut file: VaultFile = if self.vault_path.exists() {
            let s = tokio::fs::read_to_string(&self.vault_path)
                .await
                .unwrap_or_default();
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            VaultFile::default()
        };
        let enc = Self::encrypt_password(&key, password)?;
        file.entries.insert(connection_id.to_string(), enc);
        if let Some(p) = self.vault_path.parent() {
            tokio::fs::create_dir_all(p).await?;
        }
        let body = serde_json::to_string_pretty(&file)?;
        tokio::fs::write(&self.vault_path, body)
            .await
            .context("write ssh password vault")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.vault_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub async fn load(&self, connection_id: &str) -> Result<Option<String>> {
        let _g = self.lock.lock().await;
        if !self.vault_path.exists() || !self.key_path.exists() {
            return Ok(None);
        }
        let bytes = tokio::fs::read(&self.key_path)
            .await
            .context("read ssh vault key")?;
        if bytes.len() != 32 {
            anyhow::bail!("invalid ssh password vault key length");
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);

        let s = tokio::fs::read_to_string(&self.vault_path)
            .await
            .unwrap_or_default();
        let file: VaultFile = serde_json::from_str(&s).unwrap_or_default();
        let Some(entry) = file.entries.get(connection_id) else {
            return Ok(None);
        };
        match Self::decrypt_password(&key, entry) {
            Ok(p) => Ok(Some(p)),
            Err(e) => {
                log::warn!(
                    "Failed to decrypt SSH password vault entry for {}: {}",
                    connection_id,
                    e
                );
                Ok(None)
            }
        }
    }

    pub async fn remove(&self, connection_id: &str) -> Result<()> {
        let _g = self.lock.lock().await;
        if !self.vault_path.exists() {
            return Ok(());
        }
        let s = tokio::fs::read_to_string(&self.vault_path)
            .await
            .unwrap_or_default();
        let mut file: VaultFile = serde_json::from_str(&s).unwrap_or_default();
        file.entries.remove(connection_id);
        if file.entries.is_empty() {
            let _ = tokio::fs::remove_file(&self.vault_path).await;
        } else {
            tokio::fs::write(&self.vault_path, serde_json::to_string_pretty(&file)?).await?;
        }
        Ok(())
    }

    pub async fn migrate_entry(
        &self,
        old_connection_id: &str,
        new_connection_id: &str,
    ) -> Result<()> {
        if old_connection_id == new_connection_id {
            return Ok(());
        }
        let _g = self.lock.lock().await;
        if !self.vault_path.exists() {
            return Ok(());
        }
        let s = tokio::fs::read_to_string(&self.vault_path)
            .await
            .unwrap_or_default();
        let mut file: VaultFile = serde_json::from_str(&s).unwrap_or_default();
        let Some(entry) = file.entries.remove(old_connection_id) else {
            return Ok(());
        };
        file.entries
            .entry(new_connection_id.to_string())
            .or_insert(entry);
        tokio::fs::write(&self.vault_path, serde_json::to_string_pretty(&file)?).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.vault_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SSHPasswordVault;

    #[tokio::test]
    async fn migrate_entry_moves_password_to_new_connection_id() {
        let dir =
            std::env::temp_dir().join(format!("bitfun-ssh-vault-test-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&dir).await;
        let vault = SSHPasswordVault::new(dir.clone());

        vault
            .store("ssh-root@example.com:22", "secret")
            .await
            .unwrap();
        vault
            .migrate_entry("ssh-root@example.com:22", "ssh-root@example.com")
            .await
            .unwrap();

        assert_eq!(
            vault.load("ssh-root@example.com").await.unwrap().as_deref(),
            Some("secret")
        );
        assert!(
            vault
                .load("ssh-root@example.com:22")
                .await
                .unwrap()
                .is_none()
        );

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
