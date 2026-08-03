use crate::error::{SkinMarketError, SkinMarketResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{Mutex, OwnedMutexGuard};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct ArtifactStore {
    root: PathBuf,
    mutation_lock: Arc<Mutex<()>>,
}

pub(crate) struct ArtifactMutationGuard {
    _guard: OwnedMutexGuard<()>,
}

impl ArtifactStore {
    pub(crate) async fn open(root: PathBuf) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(root.join("packages")).await?;
        tokio::fs::create_dir_all(root.join("previews")).await?;
        tokio::fs::create_dir_all(root.join(".tmp")).await?;
        Ok(Self {
            root,
            mutation_lock: Arc::new(Mutex::new(())),
        })
    }

    pub(crate) async fn lock_mutations(&self) -> ArtifactMutationGuard {
        ArtifactMutationGuard {
            _guard: self.mutation_lock.clone().lock_owned().await,
        }
    }

    fn package_path(&self, sha256: &str) -> PathBuf {
        content_path(&self.root.join("packages"), sha256, "bitfun-appearance")
    }

    fn preview_path(&self, sha256: &str) -> PathBuf {
        content_path(&self.root.join("previews"), sha256, "webp")
    }

    pub(crate) async fn put_package(
        &self,
        _guard: &ArtifactMutationGuard,
        sha256: &str,
        bytes: &[u8],
    ) -> SkinMarketResult<()> {
        self.put_atomic(&self.package_path(sha256), bytes).await
    }

    pub(crate) async fn put_preview(
        &self,
        _guard: &ArtifactMutationGuard,
        sha256: &str,
        bytes: &[u8],
    ) -> SkinMarketResult<()> {
        self.put_atomic(&self.preview_path(sha256), bytes).await
    }

    pub(crate) async fn open_package(
        &self,
        sha256: &str,
    ) -> SkinMarketResult<(tokio::fs::File, u64)> {
        let path = self.package_path(sha256);
        let file = tokio::fs::File::open(&path).await.map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SkinMarketError::not_found("Appearance package artifact was not found.")
            } else {
                SkinMarketError::internal(error)
            }
        })?;
        let size = file
            .metadata()
            .await
            .map_err(SkinMarketError::internal)?
            .len();
        Ok((file, size))
    }

    pub(crate) async fn read_preview(&self, sha256: &str) -> SkinMarketResult<Vec<u8>> {
        read_artifact(
            self.preview_path(sha256),
            "Appearance preview artifact was not found.",
        )
        .await
    }

    async fn put_atomic(&self, target: &Path, bytes: &[u8]) -> SkinMarketResult<()> {
        if target.exists() {
            return Ok(());
        }
        let parent = target
            .parent()
            .ok_or_else(|| SkinMarketError::internal("artifact path has no parent"))?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(SkinMarketError::internal)?;
        let temporary = self
            .root
            .join(".tmp")
            .join(format!("{}.part", Uuid::new_v4()));
        tokio::fs::write(&temporary, bytes)
            .await
            .map_err(SkinMarketError::internal)?;
        match tokio::fs::rename(&temporary, target).await {
            Ok(()) => Ok(()),
            Err(_) if target.exists() => {
                let _ = tokio::fs::remove_file(&temporary).await;
                Ok(())
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&temporary).await;
                Err(SkinMarketError::internal(error))
            }
        }
    }

    pub(crate) async fn remove_package(
        &self,
        _guard: &ArtifactMutationGuard,
        sha256: &str,
    ) -> anyhow::Result<bool> {
        remove_if_exists(&self.package_path(sha256)).await
    }

    pub(crate) async fn remove_preview(
        &self,
        _guard: &ArtifactMutationGuard,
        sha256: &str,
    ) -> anyhow::Result<bool> {
        remove_if_exists(&self.preview_path(sha256)).await
    }

    pub(crate) async fn package_hashes_older_than(
        &self,
        cutoff: SystemTime,
    ) -> anyhow::Result<Vec<String>> {
        hashes_older_than(&self.root.join("packages"), "bitfun-appearance", cutoff).await
    }

    pub(crate) async fn preview_hashes_older_than(
        &self,
        cutoff: SystemTime,
    ) -> anyhow::Result<Vec<String>> {
        hashes_older_than(&self.root.join("previews"), "webp", cutoff).await
    }

    pub(crate) async fn remove_temporary_older_than(
        &self,
        _guard: &ArtifactMutationGuard,
        cutoff: SystemTime,
    ) -> anyhow::Result<u64> {
        let root = self.root.join(".tmp");
        let mut removed = 0;
        let mut entries = tokio::fs::read_dir(&root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            if !entry.file_type().await?.is_file()
                || !file_name.to_string_lossy().ends_with(".part")
                || entry.metadata().await?.modified()? > cutoff
            {
                continue;
            }
            if remove_if_exists(&entry.path()).await? {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn content_path(root: &Path, sha256: &str, extension: &str) -> PathBuf {
    root.join(sha256.get(..2).unwrap_or("00"))
        .join(format!("{sha256}.{extension}"))
}

async fn read_artifact(path: PathBuf, not_found: &'static str) -> SkinMarketResult<Vec<u8>> {
    tokio::fs::read(path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SkinMarketError::not_found(not_found)
        } else {
            SkinMarketError::internal(error)
        }
    })
}

async fn remove_if_exists(path: &Path) -> anyhow::Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

async fn hashes_older_than(
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
            if !entry.file_type().await?.is_file() || entry.metadata().await?.modified()? > cutoff {
                continue;
            }
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            let Some(hash) = file_name.strip_suffix(&format!(".{extension}")) else {
                continue;
            };
            if hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                hashes.push(hash.to_string());
            }
        }
    }
    Ok(hashes)
}
