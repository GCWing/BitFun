use crate::error::{MarketError, MarketResult};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Semaphore;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarketImageVariant {
    CompactV1,
    LargeV1,
}

impl MarketImageVariant {
    const ALL: [Self; 2] = [Self::CompactV1, Self::LargeV1];

    pub(crate) const fn cache_key(self) -> &'static str {
        match self {
            Self::CompactV1 => "compact-v1",
            Self::LargeV1 => "large-v1",
        }
    }

    const fn max_dimension(self) -> u32 {
        match self {
            Self::CompactV1 => 640,
            Self::LargeV1 => 1_280,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ArtifactStore {
    root: PathBuf,
    variant_generation_permits: Arc<Semaphore>,
}

impl ArtifactStore {
    pub(crate) async fn open(root: PathBuf) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(root.join("packages")).await?;
        tokio::fs::create_dir_all(root.join("screenshots")).await?;
        tokio::fs::create_dir_all(root.join(".tmp")).await?;
        Ok(Self {
            root,
            variant_generation_permits: Arc::new(Semaphore::new(4)),
        })
    }

    pub(crate) fn package_path(&self, sha256: &str) -> PathBuf {
        content_path(&self.root.join("packages"), sha256, "bfminiapp")
    }

    pub(crate) fn screenshot_path(&self, sha256: &str) -> PathBuf {
        content_path(&self.root.join("screenshots"), sha256, "webp")
    }

    fn screenshot_variant_path(&self, sha256: &str, variant: MarketImageVariant) -> PathBuf {
        content_path(
            &self.root.join("screenshots"),
            sha256,
            &format!("{}.webp", variant.cache_key()),
        )
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
                    MarketError::not_found("Marketplace image artifact was not found.")
                } else {
                    MarketError::internal(error)
                }
            })
    }

    pub(crate) async fn read_screenshot_variant(
        &self,
        sha256: &str,
        variant: MarketImageVariant,
    ) -> MarketResult<Vec<u8>> {
        let variant_path = self.screenshot_variant_path(sha256, variant);
        match tokio::fs::read(&variant_path).await {
            Ok(bytes) => return Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(MarketError::internal(error)),
        }

        let _permit = self
            .variant_generation_permits
            .acquire()
            .await
            .map_err(MarketError::internal)?;
        match tokio::fs::read(&variant_path).await {
            Ok(bytes) => return Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(MarketError::internal(error)),
        }

        let source = self.read_screenshot(sha256).await?;
        let max_dimension = variant.max_dimension();
        let bytes = tokio::task::spawn_blocking(move || render_webp_variant(source, max_dimension))
            .await
            .map_err(MarketError::internal)?
            .map_err(MarketError::internal)?;
        self.put_atomic(&variant_path, &bytes).await?;
        Ok(bytes)
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
        let mut removed = remove_if_exists(&self.screenshot_path(sha256)).await?;
        for variant in MarketImageVariant::ALL {
            removed |= remove_if_exists(&self.screenshot_variant_path(sha256, variant)).await?;
        }
        Ok(removed)
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

fn render_webp_variant(bytes: Vec<u8>, max_dimension: u32) -> image::ImageResult<Vec<u8>> {
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::WebP)?;
    if decoded.width() <= max_dimension && decoded.height() <= max_dimension {
        return Ok(bytes);
    }
    let resized = decoded.thumbnail(max_dimension, max_dimension);
    let mut cursor = Cursor::new(Vec::new());
    resized.write_to(&mut cursor, image::ImageFormat::WebP)?;
    Ok(cursor.into_inner())
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};

    fn test_webp(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
            Rgba([(x % 255) as u8, (y % 255) as u8, 120, 255])
        }));
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::WebP)
            .unwrap();
        output.into_inner()
    }

    #[tokio::test]
    async fn caches_and_removes_resized_screenshot_variants() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::open(temporary.path().to_path_buf())
            .await
            .unwrap();
        let sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        store
            .put_screenshot(sha256, &test_webp(800, 400))
            .await
            .unwrap();

        let compact = store
            .read_screenshot_variant(sha256, MarketImageVariant::CompactV1)
            .await
            .unwrap();
        let decoded =
            image::load_from_memory_with_format(&compact, image::ImageFormat::WebP).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (640, 320));
        let variant_path = store.screenshot_variant_path(sha256, MarketImageVariant::CompactV1);
        assert!(variant_path.exists());
        assert_eq!(
            store
                .read_screenshot_variant(sha256, MarketImageVariant::CompactV1)
                .await
                .unwrap(),
            compact
        );

        assert!(store.remove_screenshot_if_exists(sha256).await.unwrap());
        assert!(!store.screenshot_path(sha256).exists());
        assert!(!variant_path.exists());
    }
}
