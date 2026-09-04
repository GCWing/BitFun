//! Prompt-free encrypted credential storage backed by local application files.
//!
//! The AES key and ciphertext map are separate `0600` files. This avoids OS
//! credential-store authorization prompts while keeping secrets out of plain
//! JSON. It protects against accidental disclosure, but a process running as
//! the same OS user can read both files and decrypt the values.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const VAULT_VERSION: u8 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct VaultFile {
    version: u8,
    #[serde(default)]
    entries: HashMap<String, String>,
}

impl Default for VaultFile {
    fn default() -> Self {
        Self {
            version: VAULT_VERSION,
            entries: HashMap::new(),
        }
    }
}

/// A small encrypted key/value vault stored entirely in the application data directory.
#[derive(Debug, Clone)]
pub struct CredentialVault {
    key_path: PathBuf,
    vault_path: PathBuf,
    lock_path: PathBuf,
}

impl CredentialVault {
    pub fn new(key_path: impl Into<PathBuf>, vault_path: impl Into<PathBuf>) -> Self {
        let vault_path = vault_path.into();
        let lock_path = vault_path.with_extension("lock");
        Self {
            key_path: key_path.into(),
            vault_path,
            lock_path,
        }
    }

    pub async fn get(&self, entry_name: &str) -> Result<Option<Vec<u8>>> {
        let _lock = self.acquire_lock().await?;
        let Some(file) = self.read_vault().await? else {
            return Ok(None);
        };
        let Some(encoded) = file.entries.get(entry_name) else {
            return Ok(None);
        };
        let key = self.read_key().await?;
        decrypt(&key, encoded)
            .map(Some)
            .context("decrypt local credential vault entry")
    }

    pub async fn set(&self, entry_name: &str, value: &[u8]) -> Result<()> {
        let _lock = self.acquire_lock().await?;
        let key = self.load_or_create_key().await?;
        let mut file = self.read_vault().await?.unwrap_or_default();
        file.entries
            .insert(entry_name.to_string(), encrypt(&key, value)?);
        self.write_vault(&file).await
    }

    pub async fn remove(&self, entry_name: &str) -> Result<()> {
        let _lock = self.acquire_lock().await?;
        let Some(mut file) = self.read_vault().await? else {
            return Ok(());
        };
        if file.entries.remove(entry_name).is_none() {
            return Ok(());
        }
        if file.entries.is_empty() {
            match tokio::fs::remove_file(&self.vault_path).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error).with_context(|| {
                    format!(
                        "remove local credential vault {}",
                        self.vault_path.display()
                    )
                }),
            }
        } else {
            self.write_vault(&file).await
        }
    }

    async fn acquire_lock(&self) -> Result<VaultLock> {
        if let Some(parent) = self.lock_path.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "create local credential vault directory {}",
                    parent.display()
                )
            })?;
        }
        let path = self.lock_path.clone();
        tokio::task::spawn_blocking(move || {
            let mut options = OpenOptions::new();
            options.create(true).truncate(false).read(true).write(true);
            configure_private_open(&mut options);
            let file = options
                .open(&path)
                .with_context(|| format!("open local credential vault lock {}", path.display()))?;
            fs2::FileExt::lock_exclusive(&file).with_context(|| {
                format!("acquire local credential vault lock {}", path.display())
            })?;
            restrict_file_permissions(&path)?;
            Ok(VaultLock { file, path })
        })
        .await
        .context("join local credential vault lock task")?
    }

    async fn read_key(&self) -> Result<[u8; KEY_BYTES]> {
        let bytes = read_private_file(&self.key_path)
            .await
            .with_context(|| format!("read local credential key {}", self.key_path.display()))?;
        let bytes = bytes
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "local credential key {} is missing",
                        self.key_path.display()
                    ),
                )
            })
            .with_context(|| format!("read local credential key {}", self.key_path.display()))?;
        bytes.try_into().map_err(|bytes: Vec<u8>| {
            anyhow!(
                "invalid local credential key length at {}: expected {KEY_BYTES}, got {}",
                self.key_path.display(),
                bytes.len()
            )
        })
    }

    async fn load_or_create_key(&self) -> Result<[u8; KEY_BYTES]> {
        match self.read_key().await {
            Ok(key) => return Ok(key),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) => {}
            Err(error) => return Err(error),
        }
        if let Some(parent) = self.key_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut key = [0u8; KEY_BYTES];
        rand::rngs::OsRng.fill_bytes(&mut key);
        let path = self.key_path.clone();
        let key_for_write = key;
        tokio::task::spawn_blocking(move || {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            configure_private_open(&mut options);
            match options.open(&path) {
                Ok(mut file) => {
                    use std::io::Write;
                    file.write_all(&key_for_write)
                        .with_context(|| format!("write local credential key {}", path.display()))?;
                    file.sync_all()
                        .with_context(|| format!("sync local credential key {}", path.display()))?;
                    restrict_file_permissions(&path)?;
                    Ok(key_for_write)
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let mut read_options = OpenOptions::new();
                    read_options.read(true);
                    configure_private_open(&mut read_options);
                    let mut file = read_options.open(&path).with_context(|| {
                        format!("open concurrently-created credential key {}", path.display())
                    })?;
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes).with_context(|| {
                        format!("read concurrently-created credential key {}", path.display())
                    })?;
                    bytes.try_into().map_err(|bytes: Vec<u8>| {
                        anyhow!(
                            "invalid local credential key length at {}: expected {KEY_BYTES}, got {}",
                            path.display(),
                            bytes.len()
                        )
                    })
                }
                Err(error) => Err(error)
                    .with_context(|| format!("create local credential key {}", path.display())),
            }
        })
        .await
        .context("join local credential key creation task")?
    }

    async fn read_vault(&self) -> Result<Option<VaultFile>> {
        let Some(bytes) = read_private_file(&self.vault_path).await.with_context(|| {
            format!("read local credential vault {}", self.vault_path.display())
        })?
        else {
            return Ok(None);
        };
        let file: VaultFile = serde_json::from_slice(&bytes).with_context(|| {
            format!("parse local credential vault {}", self.vault_path.display())
        })?;
        if file.version != VAULT_VERSION {
            return Err(anyhow!(
                "unsupported local credential vault version {} at {}",
                file.version,
                self.vault_path.display()
            ));
        }
        Ok(Some(file))
    }

    async fn write_vault(&self, file: &VaultFile) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        if let Some(parent) = self.vault_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let bytes = serde_json::to_vec_pretty(file)?;
        let mut suffix = [0u8; 8];
        rand::rngs::OsRng.fill_bytes(&mut suffix);
        let tmp = self
            .vault_path
            .with_extension(format!("tmp-{}", u64::from_le_bytes(suffix)));
        let mut options = tokio::fs::OpenOptions::new();
        options.create_new(true).write(true);
        configure_private_tokio_open(&mut options);
        let mut output = options.open(&tmp).await.with_context(|| {
            format!("create local credential vault temp file {}", tmp.display())
        })?;
        if let Err(error) = async {
            output.write_all(&bytes).await?;
            output.sync_all().await?;
            drop(output);
            tokio::fs::rename(&tmp, &self.vault_path).await
        }
        .await
        {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(error).with_context(|| {
                format!(
                    "commit local credential vault {}",
                    self.vault_path.display()
                )
            });
        }
        restrict_file_permissions(&self.vault_path)
    }
}

struct VaultLock {
    file: File,
    path: PathBuf,
}

impl Drop for VaultLock {
    fn drop(&mut self) {
        if let Err(error) = fs2::FileExt::unlock(&self.file) {
            log::warn!(
                "release local credential vault lock {} failed: {error}",
                self.path.display()
            );
        }
    }
}

fn encrypt(key: &[u8; KEY_BYTES], plaintext: &[u8]) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| anyhow!("cipher init: {error}"))?;
    let mut nonce = [0u8; NONCE_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|error| anyhow!("encrypt local credential: {error}"))?;
    let mut blob = Vec::with_capacity(NONCE_BYTES + ciphertext.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);
    Ok(B64.encode(blob))
}

fn decrypt(key: &[u8; KEY_BYTES], encoded: &str) -> Result<Vec<u8>> {
    let blob = B64
        .decode(encoded)
        .context("decode local credential entry")?;
    if blob.len() <= NONCE_BYTES {
        return Err(anyhow!("local credential entry is too short"));
    }
    let (nonce, ciphertext) = blob.split_at(NONCE_BYTES);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| anyhow!("cipher init: {error}"))?;
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|error| anyhow!("decrypt local credential: {error}"))
}

#[cfg(unix)]
fn configure_private_open(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn configure_private_open(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn configure_private_tokio_open(options: &mut tokio::fs::OpenOptions) {
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn configure_private_tokio_open(_options: &mut tokio::fs::OpenOptions) {}

#[cfg(unix)]
fn restrict_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).with_context(|| {
        format!(
            "restrict local credential file permissions {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

async fn read_private_file(path: &Path) -> Result<Option<Vec<u8>>> {
    use tokio::io::AsyncReadExt;

    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    configure_private_tokio_open(&mut options);
    let mut file = match options.open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("open local credential file {}", path.display()));
        }
    };
    restrict_file_permissions(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .await
        .with_context(|| format!("read local credential file {}", path.display()))?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vault(label: &str) -> (PathBuf, CredentialVault) {
        let dir = std::env::temp_dir().join(format!(
            "openbitfun-credential-vault-{label}-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let vault = CredentialVault::new(dir.join(".key"), dir.join("vault.json"));
        (dir, vault)
    }

    #[tokio::test]
    async fn round_trip_is_encrypted_and_removable() {
        let (dir, vault) = test_vault("round-trip");
        vault.set("account", b"secret-token").await.unwrap();
        assert_eq!(
            vault.get("account").await.unwrap(),
            Some(b"secret-token".to_vec())
        );
        let stored = tokio::fs::read_to_string(dir.join("vault.json"))
            .await
            .unwrap();
        assert!(!stored.contains("secret-token"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            for path in [dir.join(".key"), dir.join("vault.json")] {
                let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600);
            }
        }
        vault.remove("account").await.unwrap();
        assert_eq!(vault.get("account").await.unwrap(), None);
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn existing_vault_without_key_is_reported_not_reset() {
        let (dir, vault) = test_vault("missing-key");
        vault.set("account", b"secret-token").await.unwrap();
        tokio::fs::remove_file(dir.join(".key")).await.unwrap();
        let error = vault.get("account").await.unwrap_err();
        assert!(error.to_string().contains("read local credential key"));
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinked_key_is_rejected() {
        use std::os::unix::fs::symlink;

        let (dir, vault) = test_vault("symlink-key");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let target = dir.join("elsewhere");
        tokio::fs::write(&target, [7u8; KEY_BYTES]).await.unwrap();
        symlink(&target, dir.join(".key")).unwrap();
        let error = vault.set("account", b"secret-token").await.unwrap_err();
        assert!(error.to_string().contains("credential key"));
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
