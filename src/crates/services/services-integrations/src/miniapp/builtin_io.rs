//! Built-in MiniApp seed and marker filesystem IO.

use bitfun_product_domains::miniapp::builtin::{
    build_builtin_package_json, build_builtin_seed_meta, builtin_source_files,
    parse_builtin_install_marker, preserved_builtin_created_at, serialize_builtin_install_marker,
    BuiltinInstallMarker, BuiltinMiniAppBundle, BUILTIN_INSTALL_MARKER,
    BUILTIN_PLACEHOLDER_COMPILED_HTML, LEGACY_BUILTIN_VERSION_MARKER,
};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum MiniAppBuiltinIoError {
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    MarkerSerialization(serde_json::Error),
    MetaSerialization(serde_json::Error),
    PackageSerialization(serde_json::Error),
    InvalidBundledMeta(serde_json::Error),
}

impl fmt::Display for MiniAppBuiltinIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "{action} {} failed: {source}", path.display()),
            Self::MarkerSerialization(source) => {
                write!(f, "serialize builtin marker failed: {source}")
            }
            Self::MetaSerialization(source) => write!(f, "serialize meta.json failed: {source}"),
            Self::PackageSerialization(source) => {
                write!(f, "serialize package.json failed: {source}")
            }
            Self::InvalidBundledMeta(source) => write!(f, "invalid bundled meta.json: {source}"),
        }
    }
}

impl std::error::Error for MiniAppBuiltinIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::MarkerSerialization(source)
            | Self::MetaSerialization(source)
            | Self::PackageSerialization(source)
            | Self::InvalidBundledMeta(source) => Some(source),
        }
    }
}

pub type MiniAppBuiltinIoResult<T> = Result<T, MiniAppBuiltinIoError>;

pub async fn read_builtin_install_marker(
    path: &Path,
) -> MiniAppBuiltinIoResult<Option<BuiltinInstallMarker>> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(MiniAppBuiltinIoError::Io {
                action: "read builtin marker",
                path: path.to_path_buf(),
                source,
            });
        }
    };

    match parse_builtin_install_marker(&content) {
        Ok(marker) => Ok(Some(marker)),
        Err(error) => {
            log::warn!(
                "ignore invalid builtin miniapp marker {}: {}",
                path.display(),
                error
            );
            Ok(None)
        }
    }
}

pub async fn write_builtin_install_marker(
    path: &Path,
    marker: &BuiltinInstallMarker,
) -> MiniAppBuiltinIoResult<()> {
    let content = serialize_builtin_install_marker(marker)
        .map_err(MiniAppBuiltinIoError::MarkerSerialization)?;
    write_text_file(path, &content).await
}

pub async fn write_legacy_builtin_version_marker(
    app_dir: &Path,
    legacy_version: &str,
) -> MiniAppBuiltinIoResult<()> {
    write_text_file(&app_dir.join(LEGACY_BUILTIN_VERSION_MARKER), legacy_version).await
}

pub async fn prepare_builtin_seed_bundle_files(
    app_dir: &Path,
    app: &BuiltinMiniAppBundle,
    now: i64,
) -> MiniAppBuiltinIoResult<()> {
    let source_dir = app_dir.join("source");
    tokio::fs::create_dir_all(&source_dir)
        .await
        .map_err(|source| MiniAppBuiltinIoError::Io {
            action: "create dir",
            path: source_dir.clone(),
            source,
        })?;

    let meta_path = app_dir.join("meta.json");
    let existing_meta_json = tokio::fs::read_to_string(&meta_path).await.ok();
    let meta = build_builtin_seed_meta(
        app,
        preserved_builtin_created_at(existing_meta_json.as_deref()),
        now,
    )
    .map_err(MiniAppBuiltinIoError::InvalidBundledMeta)?;
    let meta_json =
        serde_json::to_string_pretty(&meta).map_err(MiniAppBuiltinIoError::MetaSerialization)?;
    write_text_file(&meta_path, &meta_json).await?;

    for (file_name, content) in builtin_source_files(app) {
        write_text_file(&source_dir.join(file_name), content).await?;
    }

    let pkg = build_builtin_package_json(app.id);
    let pkg_json =
        serde_json::to_string_pretty(&pkg).map_err(MiniAppBuiltinIoError::PackageSerialization)?;
    write_text_file(&app_dir.join("package.json"), &pkg_json).await?;

    let storage_path = app_dir.join("storage.json");
    if !storage_path.exists() {
        write_text_file(&storage_path, "{}").await?;
    }

    write_text_file(
        &app_dir.join("compiled.html"),
        BUILTIN_PLACEHOLDER_COMPILED_HTML,
    )
    .await
}

pub async fn write_text_file(path: &Path, content: &str) -> MiniAppBuiltinIoResult<()> {
    tokio::fs::write(path, content)
        .await
        .map_err(|source| MiniAppBuiltinIoError::Io {
            action: "write",
            path: path.to_path_buf(),
            source,
        })
}

pub fn builtin_marker_path(app_dir: &Path) -> PathBuf {
    app_dir.join(BUILTIN_INSTALL_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::miniapp::builtin::BUILTIN_APPS;

    fn scratch_dir(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("bitfun-miniapp-builtin-io-{label}-{unique}"));
        std::fs::create_dir_all(&path).expect("create scratch dir");
        path
    }

    #[tokio::test]
    async fn marker_io_ignores_invalid_marker_and_round_trips_valid_marker() {
        let dir = scratch_dir("marker");
        let path = builtin_marker_path(&dir);
        write_text_file(&path, "not-json").await.unwrap();
        assert!(read_builtin_install_marker(&path).await.unwrap().is_none());

        let marker = BuiltinInstallMarker {
            version: 7,
            hash: "sha256:test".to_string(),
        };
        write_builtin_install_marker(&path, &marker).await.unwrap();
        assert_eq!(
            read_builtin_install_marker(&path).await.unwrap(),
            Some(marker)
        );

        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn prepare_builtin_seed_bundle_files_preserves_existing_storage() {
        let dir = scratch_dir("bundle");
        write_text_file(&dir.join("storage.json"), r#"{"kept":true}"#)
            .await
            .unwrap();

        prepare_builtin_seed_bundle_files(&dir, &BUILTIN_APPS[0], 1234)
            .await
            .unwrap();

        assert!(dir.join("meta.json").exists());
        assert!(dir.join("source").join("index.html").exists());
        assert!(dir.join("package.json").exists());
        assert!(dir.join("compiled.html").exists());
        assert_eq!(
            tokio::fs::read_to_string(dir.join("storage.json"))
                .await
                .unwrap(),
            r#"{"kept":true}"#
        );

        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
