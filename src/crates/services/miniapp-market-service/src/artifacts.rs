use crate::error::{MarketError, MarketResult};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    pub(crate) async fn open(root: PathBuf) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(root.join("packages")).await?;
        tokio::fs::create_dir_all(root.join("screenshots")).await?;
        tokio::fs::create_dir_all(root.join(".tmp")).await?;
        Ok(Self { root })
    }

    pub(crate) fn package_path(&self, sha256: &str) -> PathBuf {
        content_path(&self.root.join("packages"), sha256, "bfminiapp")
    }

    pub(crate) fn screenshot_path(&self, sha256: &str) -> PathBuf {
        content_path(&self.root.join("screenshots"), sha256, "webp")
    }

    pub(crate) async fn put_package(&self, sha256: &str, bytes: &[u8]) -> MarketResult<PathBuf> {
        let path = self.package_path(sha256);
        self.put_atomic(&path, bytes).await?;
        Ok(path)
    }

    pub(crate) async fn put_screenshot(&self, sha256: &str, bytes: &[u8]) -> MarketResult<PathBuf> {
        let path = self.screenshot_path(sha256);
        self.put_atomic(&path, bytes).await?;
        Ok(path)
    }

    pub(crate) async fn read_package(&self, sha256: &str) -> MarketResult<Vec<u8>> {
        tokio::fs::read(self.package_path(sha256))
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    MarketError::not_found("Package artifact was not found.")
                } else {
                    MarketError::internal(error)
                }
            })
    }

    pub(crate) async fn read_screenshot(&self, sha256: &str) -> MarketResult<Vec<u8>> {
        tokio::fs::read(self.screenshot_path(sha256))
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    MarketError::not_found("Screenshot artifact was not found.")
                } else {
                    MarketError::internal(error)
                }
            })
    }

    async fn put_atomic(&self, path: &Path, bytes: &[u8]) -> MarketResult<()> {
        if path.exists() {
            return Ok(());
        }
        let parent = path
            .parent()
            .ok_or_else(|| MarketError::internal("Artifact path has no parent"))?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(MarketError::internal)?;
        let temporary = self
            .root
            .join(".tmp")
            .join(format!("{}.part", Uuid::new_v4()));
        tokio::fs::write(&temporary, bytes)
            .await
            .map_err(MarketError::internal)?;
        match tokio::fs::rename(&temporary, path).await {
            Ok(()) => Ok(()),
            Err(_error) if path.exists() => {
                let _ = tokio::fs::remove_file(&temporary).await;
                Ok(())
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&temporary).await;
                Err(MarketError::internal(error))
            }
        }
    }

    pub(crate) async fn remove_package_if_exists(&self, sha256: &str) -> anyhow::Result<bool> {
        remove_if_exists(&self.package_path(sha256)).await
    }

    pub(crate) async fn remove_screenshot_if_exists(&self, sha256: &str) -> anyhow::Result<bool> {
        remove_if_exists(&self.screenshot_path(sha256)).await
    }

    pub(crate) async fn package_hashes_older_than(
        &self,
        cutoff: SystemTime,
    ) -> anyhow::Result<Vec<String>> {
        content_hashes_older_than(&self.root.join("packages"), "bfminiapp", cutoff).await
    }

    pub(crate) async fn screenshot_hashes_older_than(
        &self,
        cutoff: SystemTime,
    ) -> anyhow::Result<Vec<String>> {
        content_hashes_older_than(&self.root.join("screenshots"), "webp", cutoff).await
    }
}

fn content_path(root: &Path, sha256: &str, extension: &str) -> PathBuf {
    let prefix = sha256.get(..2).unwrap_or("00");
    root.join(prefix).join(format!("{sha256}.{extension}"))
}

async fn remove_if_exists(path: &Path) -> anyhow::Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

async fn content_hashes_older_than(
    root: &Path,
    extension: &str,
    cutoff: SystemTime,
) -> anyhow::Result<Vec<String>> {
    let mut hashes = Vec::new();
    let mut prefixes = tokio::fs::read_dir(root).await?;
    while let Some(prefix) = prefixes.next_entry().await? {
        if !prefix.file_type().await?.is_dir() {
            continue;
        }
        let mut entries = tokio::fs::read_dir(prefix.path()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            let Some(hash) = file_name.strip_suffix(&format!(".{extension}")) else {
                continue;
            };
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                continue;
            }
            let modified = entry.metadata().await?.modified()?;
            if modified <= cutoff {
                hashes.push(hash.to_string());
            }
        }
    }
    Ok(hashes)
}
