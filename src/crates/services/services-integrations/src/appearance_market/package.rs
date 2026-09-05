use image::ImageReader;
use openbitfun_product_domains::appearance_market::{
    AppearanceMarketPackageMeta, AppearancePackageMode, APPEARANCE_MARKET_MAX_ENTRIES,
    APPEARANCE_MARKET_MAX_MANIFEST_BYTES, APPEARANCE_MARKET_MAX_PACKAGE_BYTES,
    APPEARANCE_MARKET_MAX_PREVIEW_BYTES, APPEARANCE_MARKET_MAX_PREVIEW_PIXELS,
    APPEARANCE_MARKET_MAX_UNCOMPRESSED_BYTES,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read};
use zip::ZipArchive;

const MANIFEST_PATH: &str = "appearance.json";
const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VIDEO_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DIRECTORY_ENTRIES: usize = 64;
const SUPPORTED_CAPABILITIES: &[&str] = &[
    "components.v1",
    "scenes.v1",
    "renderers.v1",
    "assets.v1",
    "background-media.v1",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AppearanceMarketPackageError {
    pub code: &'static str,
    pub message: String,
}

impl AppearanceMarketPackageError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedAppearancePreview {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct ValidatedAppearanceMarketPackage {
    pub sha256: String,
    pub size: u64,
    pub meta: AppearanceMarketPackageMeta,
    pub preview: ValidatedAppearancePreview,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppearanceManifest {
    schema: String,
    schema_version: u32,
    id: String,
    name: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    description: String,
    version: String,
    mode: AppearancePackageMode,
    preview: AssetReference,
    #[serde(default)]
    background_media: Option<BackgroundMedia>,
    #[serde(default)]
    required_capabilities: Vec<String>,
    #[serde(default)]
    assets: BTreeMap<String, PackageAsset>,
    #[serde(default)]
    integrity: Option<IntegrityManifest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackgroundMedia {
    kind: String,
    asset_id: String,
    poster_asset_id: String,
    #[serde(default)]
    fit: Option<String>,
    #[serde(default)]
    position: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetReference {
    kind: String,
    asset_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageAsset {
    kind: String,
    mime_type: String,
    source: PackageSource,
}

#[derive(Debug, Deserialize)]
struct PackageSource {
    kind: String,
    path: String,
}

#[derive(Debug, Default, Deserialize)]
struct IntegrityManifest {
    #[serde(default)]
    sha256: BTreeMap<String, String>,
}

pub fn validate_appearance_market_package(
    bytes: &[u8],
) -> Result<ValidatedAppearanceMarketPackage, AppearanceMarketPackageError> {
    if bytes.is_empty() {
        return Err(error("empty_package", "The Appearance package is empty."));
    }
    if bytes.len() as u64 > APPEARANCE_MARKET_MAX_PACKAGE_BYTES {
        return Err(error(
            "package_too_large",
            "The compressed Appearance package exceeds 96 MiB.",
        ));
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|source| {
        error(
            "invalid_package",
            format!("The Appearance package is not a valid ZIP archive: {source}"),
        )
    })?;
    if archive.is_empty() || archive.len() > APPEARANCE_MARKET_MAX_ENTRIES + MAX_DIRECTORY_ENTRIES {
        return Err(error(
            "invalid_entry_count",
            "The Appearance package contains too many archive entries.",
        ));
    }

    let mut seen = HashSet::new();
    let mut entry_sizes = HashMap::new();
    let mut directory_count = 0;
    let mut total_uncompressed = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|source| {
            error(
                "invalid_package",
                format!("The Appearance package contains an invalid ZIP entry: {source}"),
            )
        })?;
        if entry.encrypted() {
            return Err(error(
                "encrypted_entry_forbidden",
                "Encrypted Appearance package entries are not allowed.",
            ));
        }
        if entry.unix_mode().is_some_and(|mode| {
            let file_type = mode & 0o170000;
            file_type != 0 && file_type != 0o100000 && file_type != 0o040000
        }) {
            return Err(error(
                "non_regular_entry_forbidden",
                "Appearance packages may contain regular files only.",
            ));
        }
        let name = safe_entry_name(&entry)?;
        if entry.is_dir() {
            directory_count += 1;
            if entry.size() != 0 || directory_count > MAX_DIRECTORY_ENTRIES {
                return Err(error(
                    "invalid_entry_count",
                    "Appearance package directories must be empty entries and are limited to 64.",
                ));
            }
            continue;
        }
        if !entry.is_file() {
            return Err(error(
                "non_regular_entry_forbidden",
                "Appearance packages may contain files and directory markers only.",
            ));
        }
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(error(
                "duplicate_package_path",
                format!("The Appearance package contains a duplicate path: {name}"),
            ));
        }
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        if total_uncompressed > APPEARANCE_MARKET_MAX_UNCOMPRESSED_BYTES {
            return Err(error(
                "package_expansion_too_large",
                "The Appearance package expands beyond 128 MiB.",
            ));
        }
        entry_sizes.insert(name, entry.size());
    }
    if entry_sizes.is_empty() || entry_sizes.len() > APPEARANCE_MARKET_MAX_ENTRIES {
        return Err(error(
            "invalid_entry_count",
            format!(
                "Appearance packages must contain between 1 and {} files.",
                APPEARANCE_MARKET_MAX_ENTRIES
            ),
        ));
    }

    let manifest_size = entry_sizes.get(MANIFEST_PATH).copied().ok_or_else(|| {
        error(
            "missing_manifest",
            format!("The Appearance package is missing {MANIFEST_PATH}."),
        )
    })?;
    if manifest_size > APPEARANCE_MARKET_MAX_MANIFEST_BYTES {
        return Err(error(
            "manifest_too_large",
            "The Appearance manifest exceeds 256 KiB.",
        ));
    }
    let manifest_bytes = read_entry(&mut archive, MANIFEST_PATH)?;
    let raw_manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).map_err(|source| {
            error(
                "invalid_manifest",
                format!("appearance.json is not valid JSON: {source}"),
            )
        })?;
    for field in [
        "globals",
        "materials",
        "components",
        "scenes",
        "renderers",
        "backgroundMedia",
    ] {
        if let Some(value) = raw_manifest.get(field) {
            reject_remote_or_executable_values(value, &format!("$.{field}"))?;
        }
    }
    let manifest: AppearanceManifest = serde_json::from_value(raw_manifest).map_err(|source| {
        error(
            "invalid_manifest",
            format!("appearance.json does not match the Appearance package contract: {source}"),
        )
    })?;
    validate_manifest_metadata(&manifest)?;

    let mut declared_paths = HashMap::new();
    for (asset_id, asset) in &manifest.assets {
        validate_identifier(asset_id, "asset id")?;
        if asset.source.kind != "package" {
            return Err(error(
                "external_asset_forbidden",
                format!("Appearance asset {asset_id} must use a package source."),
            ));
        }
        validate_package_path(&asset.source.path)?;
        if declared_paths
            .insert(asset.source.path.clone(), asset_id.as_str())
            .is_some()
        {
            return Err(error(
                "duplicate_asset_path",
                format!(
                    "Multiple Appearance assets declare the package path {}.",
                    asset.source.path
                ),
            ));
        }
        if !entry_sizes.contains_key(&asset.source.path) {
            return Err(error(
                "missing_asset",
                format!(
                    "Declared Appearance asset is missing: {}",
                    asset.source.path
                ),
            ));
        }
    }
    for path in entry_sizes.keys() {
        if path != MANIFEST_PATH && !declared_paths.contains_key(path) {
            return Err(error(
                "undeclared_package_entry",
                format!("The Appearance package contains an undeclared file: {path}"),
            ));
        }
    }

    for (asset_id, asset) in &manifest.assets {
        let size = entry_sizes[&asset.source.path];
        let max_size = match asset.kind.as_str() {
            "image" => MAX_IMAGE_BYTES,
            "video" => MAX_VIDEO_BYTES,
            _ => {
                return Err(error(
                    "invalid_asset_kind",
                    format!("Appearance asset {asset_id} has an unsupported kind."),
                ));
            }
        };
        if size == 0 || size > max_size {
            return Err(error(
                "invalid_asset_size",
                format!(
                    "Appearance asset {} has an invalid size.",
                    asset.source.path
                ),
            ));
        }
    }

    if manifest.preview.kind != "asset" {
        return Err(error(
            "invalid_preview",
            "The marketplace preview must reference a packaged image asset.",
        ));
    }
    let preview_asset = manifest
        .assets
        .get(&manifest.preview.asset_id)
        .ok_or_else(|| {
            error(
                "invalid_preview",
                "The marketplace preview asset is missing.",
            )
        })?;
    if preview_asset.kind != "image" {
        return Err(error(
            "invalid_preview",
            "The marketplace preview must reference an image asset.",
        ));
    }
    if entry_sizes[&preview_asset.source.path] > APPEARANCE_MARKET_MAX_PREVIEW_BYTES {
        return Err(error(
            "preview_too_large",
            "The marketplace preview exceeds 4 MiB.",
        ));
    }

    let mut preview = None;
    for (asset_id, asset) in &manifest.assets {
        let asset_bytes = read_entry(&mut archive, &asset.source.path)?;
        validate_asset_magic(asset_id, asset, &asset_bytes)?;
        let digest = hex::encode(Sha256::digest(&asset_bytes));
        if let Some(expected) = manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.sha256.get(asset_id))
        {
            if !is_sha256(expected) || digest != expected.to_ascii_lowercase() {
                return Err(error(
                    "asset_integrity_mismatch",
                    format!("Appearance asset integrity mismatch: {}", asset.source.path),
                ));
            }
        }
        if asset_id == &manifest.preview.asset_id {
            let (width, height) = image_dimensions(&asset_bytes, &asset.mime_type)?;
            preview = Some(ValidatedAppearancePreview {
                bytes: asset_bytes,
                mime_type: asset.mime_type.clone(),
                sha256: digest,
                width,
                height,
            });
        }
    }
    if let Some(integrity) = &manifest.integrity {
        for asset_id in integrity.sha256.keys() {
            if !manifest.assets.contains_key(asset_id) {
                return Err(error(
                    "unknown_integrity_asset",
                    format!("Appearance integrity references unknown asset {asset_id}."),
                ));
            }
        }
    }

    Ok(ValidatedAppearanceMarketPackage {
        sha256: hex::encode(Sha256::digest(bytes)),
        size: bytes.len() as u64,
        meta: AppearanceMarketPackageMeta {
            package_id: manifest.id,
            name: manifest.name,
            description: manifest.description,
            author: manifest.author,
            mode: manifest.mode,
            package_version: manifest.version,
            required_capabilities: manifest.required_capabilities,
        },
        preview: preview.expect("validated preview asset must be loaded"),
    })
}

fn validate_manifest_metadata(
    manifest: &AppearanceManifest,
) -> Result<(), AppearanceMarketPackageError> {
    if manifest.schema != "openbitfun.appearance" || manifest.schema_version != 1 {
        return Err(error(
            "unsupported_manifest",
            "The package must use openbitfun.appearance schema version 1.",
        ));
    }
    validate_identifier(&manifest.id, "package id")?;
    if manifest.name.trim().is_empty() || manifest.name.len() > 100 {
        return Err(error(
            "invalid_name",
            "Appearance names must contain between 1 and 100 bytes.",
        ));
    }
    if manifest
        .author
        .as_ref()
        .is_some_and(|value| value.len() > 100)
        || manifest.description.len() > 500
    {
        return Err(error(
            "invalid_metadata",
            "Appearance author or description metadata is too long.",
        ));
    }
    semver::Version::parse(&manifest.version).map_err(|_| {
        error(
            "invalid_version",
            "The Appearance version must use semantic version syntax.",
        )
    })?;
    let mut seen_capabilities = HashSet::new();
    for capability in &manifest.required_capabilities {
        if !SUPPORTED_CAPABILITIES.contains(&capability.as_str())
            || !seen_capabilities.insert(capability)
        {
            return Err(error(
                "unsupported_capability",
                format!("Unsupported or duplicate Appearance capability: {capability}"),
            ));
        }
    }
    if let Some(background) = &manifest.background_media {
        let video = manifest.assets.get(&background.asset_id);
        let poster = manifest.assets.get(&background.poster_asset_id);
        if background.kind != "video"
            || !matches!(video, Some(asset) if asset.kind == "video")
            || !matches!(poster, Some(asset) if asset.kind == "image")
            || !manifest
                .required_capabilities
                .iter()
                .any(|value| value == "background-media.v1")
            || background
                .fit
                .as_deref()
                .is_some_and(|value| !matches!(value, "cover" | "contain"))
            || background.position.as_deref().is_some_and(|value| {
                !matches!(value, "center" | "top" | "right" | "bottom" | "left")
            })
        {
            return Err(error(
                "invalid_background_media",
                "Appearance background media must reference one video and one image poster.",
            ));
        }
    }
    for (asset_id, asset) in &manifest.assets {
        if asset.kind == "video"
            && manifest
                .background_media
                .as_ref()
                .is_none_or(|background| background.asset_id != *asset_id)
        {
            return Err(error(
                "unused_video_asset",
                "Video assets are allowed only as the selected top-level background media.",
            ));
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), AppearanceMarketPackageError> {
    let bytes = value.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 100
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'.' | b'-')
        })
        && !matches!(bytes.last(), Some(b'.' | b'-'))
        && !value.contains("..")
        && !value.contains("--")
        && !value.contains(".-")
        && !value.contains("-.");
    if valid {
        Ok(())
    } else {
        Err(error(
            "invalid_identifier",
            format!("The Appearance {label} is not a lowercase dotted or dashed identifier."),
        ))
    }
}

fn safe_entry_name<R: Read>(
    entry: &zip::read::ZipFile<'_, R>,
) -> Result<String, AppearanceMarketPackageError> {
    let name = entry.name();
    validate_package_path(name)?;
    entry
        .enclosed_name()
        .and_then(|path| path.to_str().map(str::to_owned))
        .ok_or_else(|| {
            error(
                "unsafe_package_path",
                "Appearance package paths must use UTF-8.",
            )
        })
}

fn validate_package_path(path: &str) -> Result<(), AppearanceMarketPackageError> {
    let bytes = path.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 240
        && bytes[0].is_ascii_alphanumeric()
        && !path.starts_with('/')
        && !path.contains("..")
        && !path.contains('\\')
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'.' | b'/' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(error(
            "unsafe_package_path",
            format!("Unsafe Appearance package path: {path}"),
        ))
    }
}

fn read_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>, AppearanceMarketPackageError> {
    let mut entry = archive.by_name(path).map_err(|_| {
        error(
            "missing_package_entry",
            format!("The Appearance package is missing {path}."),
        )
    })?;
    let mut bytes = Vec::with_capacity(entry.size().min(4 * 1024 * 1024) as usize);
    entry.read_to_end(&mut bytes).map_err(|source| {
        error(
            "invalid_package_entry",
            format!("Could not read Appearance package entry {path}: {source}"),
        )
    })?;
    Ok(bytes)
}

fn validate_asset_magic(
    asset_id: &str,
    asset: &PackageAsset,
    bytes: &[u8],
) -> Result<(), AppearanceMarketPackageError> {
    let actual = sniff_media_type(bytes).ok_or_else(|| {
        error(
            "unsupported_asset_format",
            format!("Appearance asset {asset_id} has an unsupported file format."),
        )
    })?;
    if actual != asset.mime_type {
        return Err(error(
            "asset_mime_mismatch",
            format!("Appearance asset {asset_id} does not match its declared MIME type."),
        ));
    }
    let expected_kind = if actual.starts_with("image/") {
        "image"
    } else {
        "video"
    };
    if asset.kind != expected_kind {
        return Err(error(
            "asset_kind_mismatch",
            format!("Appearance asset {asset_id} does not match its declared kind."),
        ));
    }
    Ok(())
}

fn sniff_media_type(bytes: &[u8]) -> Option<String> {
    if bytes.len() >= 24 && bytes[0] == 0x89 && &bytes[1..4] == b"PNG" {
        Some("image/png".to_string())
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif".to_string())
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp".to_string())
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        Some("image/jpeg".to_string())
    } else if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        Some("video/mp4".to_string())
    } else if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
        && bytes
            .windows(4)
            .take(4096)
            .any(|window| window.eq_ignore_ascii_case(b"webm"))
    {
        Some("video/webm".to_string())
    } else {
        None
    }
}

fn image_dimensions(
    bytes: &[u8],
    mime_type: &str,
) -> Result<(u32, u32), AppearanceMarketPackageError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|source| error("invalid_preview", source.to_string()))?;
    let guessed = reader
        .format()
        .and_then(|format| format.to_mime_type().split('/').nth(1).map(str::to_string));
    if guessed.is_none() {
        return Err(error(
            "invalid_preview",
            "The Appearance preview image format could not be detected.",
        ));
    }
    let (width, height) = reader
        .into_dimensions()
        .map_err(|source| error("invalid_preview", source.to_string()))?;
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || pixels > APPEARANCE_MARKET_MAX_PREVIEW_PIXELS
    {
        return Err(error(
            "preview_dimensions_too_large",
            "The Appearance preview dimensions exceed the marketplace limit.",
        ));
    }
    if !mime_type.starts_with("image/") {
        return Err(error(
            "invalid_preview",
            "The Appearance preview must use an image MIME type.",
        ));
    }
    Ok((width, height))
}

fn reject_remote_or_executable_values(
    value: &serde_json::Value,
    path: &str,
) -> Result<(), AppearanceMarketPackageError> {
    match value {
        serde_json::Value::String(text) => {
            let normalized = text.to_ascii_lowercase();
            if normalized.contains("http://")
                || normalized.contains("https://")
                || normalized.contains("javascript:")
                || normalized.contains("data:")
                || normalized.contains("url(")
                || normalized.contains("<script")
                || normalized.contains("<svg")
                || normalized.contains("<html")
            {
                return Err(error(
                    "remote_or_executable_content_forbidden",
                    format!("Appearance manifest value {path} contains forbidden content."),
                ));
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                reject_remote_or_executable_values(item, &format!("{path}[{index}]"))?;
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, item) in fields {
                reject_remote_or_executable_values(item, &format!("{path}.{key}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn error(code: &'static str, message: impl Into<String>) -> AppearanceMarketPackageError {
    AppearanceMarketPackageError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn package(manifest: serde_json::Value, files: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file(MANIFEST_PATH, options).unwrap();
        std::io::Write::write_all(
            &mut writer,
            serde_json::to_string(&manifest).unwrap().as_bytes(),
        )
        .unwrap();
        writer.add_directory("assets/", options).unwrap();
        for (path, bytes) in files {
            writer.start_file(*path, options).unwrap();
            std::io::Write::write_all(&mut writer, bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 120, 220, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn manifest() -> serde_json::Value {
        serde_json::json!({
            "schema": "openbitfun.appearance",
            "schemaVersion": 1,
            "id": "market.test",
            "name": "Market Test",
            "description": "A safe test appearance",
            "version": "1.2.3",
            "mode": "dark",
            "preview": { "kind": "asset", "assetId": "hero" },
            "requiredCapabilities": ["assets.v1"],
            "assets": {
                "hero": {
                    "kind": "image",
                    "mimeType": "image/png",
                    "source": { "kind": "package", "path": "assets/hero.png" }
                }
            }
        })
    }

    #[test]
    fn validates_a_safe_marketplace_package() {
        let png = png();
        let bytes = package(manifest(), &[("assets/hero.png", &png)]);
        let validated = validate_appearance_market_package(&bytes).unwrap();
        assert_eq!(validated.meta.package_id, "market.test");
        assert_eq!(validated.preview.width, 2);
        assert_eq!(validated.preview.mime_type, "image/png");
    }

    #[test]
    fn requires_a_packaged_preview() {
        let mut manifest = manifest();
        manifest.as_object_mut().unwrap().remove("preview");
        let png = png();
        let bytes = package(manifest, &[("assets/hero.png", &png)]);
        let error = validate_appearance_market_package(&bytes).unwrap_err();
        assert_eq!(error.code, "invalid_manifest");
    }

    #[test]
    fn rejects_undeclared_files() {
        let png = png();
        let bytes = package(
            manifest(),
            &[("assets/hero.png", &png), ("hidden.js", b"alert(1)")],
        );
        let error = validate_appearance_market_package(&bytes).unwrap_err();
        assert_eq!(error.code, "undeclared_package_entry");
    }

    #[test]
    fn rejects_remote_manifest_content() {
        let mut manifest = manifest();
        manifest["globals"] = serde_json::json!({
            "colors": { "remote": "url(https://example.com/tracker.png)" }
        });
        let png = png();
        let bytes = package(manifest, &[("assets/hero.png", &png)]);
        let error = validate_appearance_market_package(&bytes).unwrap_err();
        assert_eq!(error.code, "remote_or_executable_content_forbidden");
    }
}
