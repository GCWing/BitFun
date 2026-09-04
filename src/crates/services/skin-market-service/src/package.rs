use crate::error::{SkinMarketError, SkinMarketResult};
use image::GenericImageView;
use openbitfun_product_domains::appearance_market::{
    AppearanceMarketPackageMeta, AppearancePackageMode, APPEARANCE_MARKET_MAX_ENTRIES,
    APPEARANCE_MARKET_MAX_MANIFEST_BYTES, APPEARANCE_MARKET_MAX_PACKAGE_BYTES,
    APPEARANCE_MARKET_MAX_PREVIEW_BYTES, APPEARANCE_MARKET_MAX_PREVIEW_PIXELS,
    APPEARANCE_MARKET_MAX_UNCOMPRESSED_BYTES,
};
use semver::Version;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use zip::ZipArchive;

const MANIFEST_PATH: &str = "appearance.json";
const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VIDEO_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_IMAGE_PIXELS: u64 = 50_000_000;
const MAX_DIRECTORY_ENTRIES: usize = 64;
const SUPPORTED_CAPABILITIES: &[&str] = &[
    "components.v1",
    "scenes.v1",
    "renderers.v1",
    "assets.v1",
    "background-media.v1",
];

#[derive(Debug, Clone)]
pub struct ValidatedAppearancePackage {
    pub sha256: String,
    pub size: u64,
    pub meta: AppearanceMarketPackageMeta,
    pub canonical_manifest_json: String,
    pub preview: ValidatedPreview,
}

#[derive(Debug, Clone)]
pub struct ValidatedPreview {
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppearanceManifest {
    schema: String,
    schema_version: u32,
    id: String,
    name: String,
    author: Option<String>,
    #[serde(default)]
    description: String,
    version: String,
    mode: AppearancePackageMode,
    preview: Option<AssetReference>,
    background_media: Option<BackgroundMedia>,
    #[serde(default)]
    required_capabilities: Vec<String>,
    globals: Option<Value>,
    materials: Option<Value>,
    components: Option<Value>,
    scenes: Option<Value>,
    renderers: Option<Value>,
    #[serde(default)]
    assets: BTreeMap<String, AssetDefinition>,
    integrity: Option<Integrity>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetReference {
    kind: String,
    #[serde(rename = "assetId")]
    asset_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackgroundMedia {
    kind: String,
    asset_id: String,
    poster_asset_id: String,
    fit: Option<String>,
    position: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum AssetDefinition {
    Image {
        #[serde(rename = "mimeType")]
        mime_type: String,
        source: PackageSource,
    },
    Video {
        #[serde(rename = "mimeType")]
        mime_type: String,
        source: PackageSource,
    },
}

impl AssetDefinition {
    fn mime_type(&self) -> &str {
        match self {
            Self::Image { mime_type, .. } | Self::Video { mime_type, .. } => mime_type,
        }
    }

    fn source(&self) -> &PackageSource {
        match self {
            Self::Image { source, .. } | Self::Video { source, .. } => source,
        }
    }

    fn is_image(&self) -> bool {
        matches!(self, Self::Image { .. })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageSource {
    kind: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Integrity {
    sha256: BTreeMap<String, String>,
}

pub fn validate_appearance_package(bytes: &[u8]) -> SkinMarketResult<ValidatedAppearancePackage> {
    if bytes.is_empty() {
        return Err(SkinMarketError::bad_request(
            "empty_package",
            "The appearance package is empty.",
        ));
    }
    if bytes.len() as u64 > APPEARANCE_MARKET_MAX_PACKAGE_BYTES {
        return Err(SkinMarketError::bad_request(
            "package_too_large",
            "The compressed appearance package exceeds 96 MiB.",
        ));
    }
    assert_safe_central_directory(bytes)?;

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        SkinMarketError::bad_request("invalid_package", format!("Invalid ZIP archive: {error}"))
    })?;
    if archive.is_empty() || archive.len() > APPEARANCE_MARKET_MAX_ENTRIES + MAX_DIRECTORY_ENTRIES {
        return Err(SkinMarketError::bad_request(
            "invalid_package_entry_count",
            "The Appearance package contains too many archive entries.",
        ));
    }
    let mut file_names = BTreeSet::new();
    let mut normalized_names = BTreeSet::new();
    let mut directory_names = Vec::new();
    let mut expanded_bytes = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            SkinMarketError::bad_request("invalid_package", format!("Invalid ZIP entry: {error}"))
        })?;
        let name = entry.name().to_string();
        validate_archive_path(&name, entry.is_dir())?;
        if entry.encrypted() {
            return Err(SkinMarketError::bad_request(
                "encrypted_package_entry",
                "Encrypted appearance package entries are not allowed.",
            ));
        }
        if entry.unix_mode().is_some_and(|mode| {
            let kind = mode & 0o170000;
            kind != 0 && kind != 0o100000 && kind != 0o040000
        }) {
            return Err(SkinMarketError::bad_request(
                "non_regular_package_entry",
                "Links and non-regular archive entries are not allowed.",
            ));
        }
        if entry.is_dir() {
            if entry.size() != 0 || directory_names.len() >= MAX_DIRECTORY_ENTRIES {
                return Err(SkinMarketError::bad_request(
                    "invalid_package_entry_count",
                    "Appearance package directories must be empty entries and are limited to 64.",
                ));
            }
            directory_names.push(name);
            continue;
        }
        if !entry.is_file() {
            return Err(SkinMarketError::bad_request(
                "non_regular_package_entry",
                "Appearance packages may contain files and directory markers only.",
            ));
        }
        if !normalized_names.insert(name.to_ascii_lowercase()) {
            return Err(SkinMarketError::bad_request(
                "duplicate_package_path",
                format!("The package contains a duplicate path: {name}"),
            ));
        }
        file_names.insert(name);
        expanded_bytes = expanded_bytes.saturating_add(entry.size());
        if expanded_bytes > APPEARANCE_MARKET_MAX_UNCOMPRESSED_BYTES {
            return Err(SkinMarketError::bad_request(
                "package_expansion_too_large",
                "The appearance package expands beyond 128 MiB.",
            ));
        }
    }
    if file_names.is_empty() || file_names.len() > APPEARANCE_MARKET_MAX_ENTRIES {
        return Err(SkinMarketError::bad_request(
            "invalid_package_entry_count",
            format!(
                "Appearance packages must contain between 1 and {} files.",
                APPEARANCE_MARKET_MAX_ENTRIES
            ),
        ));
    }
    if !file_names.contains(MANIFEST_PATH) {
        return Err(SkinMarketError::bad_request(
            "missing_manifest",
            "The appearance package must contain appearance.json at its root.",
        ));
    }

    let mut manifest_bytes = Vec::new();
    archive
        .by_name(MANIFEST_PATH)
        .map_err(|error| SkinMarketError::bad_request("invalid_manifest", error.to_string()))?
        .take(APPEARANCE_MARKET_MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut manifest_bytes)
        .map_err(|error| SkinMarketError::bad_request("invalid_manifest", error.to_string()))?;
    if manifest_bytes.len() as u64 > APPEARANCE_MARKET_MAX_MANIFEST_BYTES {
        return Err(SkinMarketError::bad_request(
            "manifest_too_large",
            "appearance.json exceeds 256 KiB.",
        ));
    }
    let raw_manifest: Value = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        SkinMarketError::bad_request(
            "invalid_manifest",
            format!("appearance.json is not valid JSON: {error}"),
        )
    })?;
    let manifest: AppearanceManifest =
        serde_json::from_value(raw_manifest.clone()).map_err(|error| {
            SkinMarketError::bad_request(
                "invalid_manifest_schema",
                format!("appearance.json does not match the marketplace schema: {error}"),
            )
        })?;
    validate_manifest(&manifest)?;

    let mut declared_paths = BTreeMap::new();
    let mut normalized_declared_paths = BTreeSet::new();
    for (asset_id, definition) in &manifest.assets {
        validate_identifier(asset_id, "asset id")?;
        let source = definition.source();
        if source.kind != "package" {
            return Err(SkinMarketError::bad_request(
                "external_asset_forbidden",
                "Appearance assets must use package-local sources.",
            ));
        }
        validate_archive_path(&source.path, false)?;
        if !source.path.starts_with("assets/") {
            return Err(SkinMarketError::bad_request(
                "invalid_asset_path",
                "Appearance assets must be stored under assets/.",
            ));
        }
        if !normalized_declared_paths.insert(source.path.to_ascii_lowercase()) {
            return Err(SkinMarketError::bad_request(
                "duplicate_asset_path",
                format!(
                    "Multiple assets declare the same package path: {}",
                    source.path
                ),
            ));
        }
        declared_paths.insert(source.path.clone(), (asset_id, definition));
    }

    let allowed_paths = std::iter::once(MANIFEST_PATH.to_string())
        .chain(declared_paths.keys().cloned())
        .collect::<BTreeSet<_>>();
    if let Some(undeclared) = file_names.difference(&allowed_paths).next() {
        return Err(SkinMarketError::bad_request(
            "undeclared_package_entry",
            format!("The package contains an undeclared file: {undeclared}"),
        ));
    }
    if let Some(missing) = allowed_paths.difference(&file_names).next() {
        return Err(SkinMarketError::bad_request(
            "missing_asset",
            format!("The package is missing a declared asset: {missing}"),
        ));
    }
    for directory in directory_names {
        if directory != "assets/"
            && !declared_paths
                .keys()
                .any(|path| path.starts_with(&directory))
        {
            return Err(SkinMarketError::bad_request(
                "undeclared_package_entry",
                format!("The package contains an undeclared directory: {directory}"),
            ));
        }
    }
    let preview_id = select_preview_asset(&manifest)?;
    let mut preview_source = None;
    for (path, (asset_id, definition)) in &declared_paths {
        let mut content = Vec::new();
        let limit = if definition.is_image() {
            MAX_IMAGE_BYTES
        } else {
            MAX_VIDEO_BYTES
        };
        archive
            .by_name(path)
            .map_err(|error| SkinMarketError::bad_request("invalid_asset", error.to_string()))?
            .take(limit + 1)
            .read_to_end(&mut content)
            .map_err(|error| SkinMarketError::bad_request("invalid_asset", error.to_string()))?;
        if content.is_empty() || content.len() as u64 > limit {
            return Err(SkinMarketError::bad_request(
                "invalid_asset_size",
                format!("Appearance asset has an invalid size: {path}"),
            ));
        }
        validate_asset_mime(path, definition, &content)?;
        if let Some(expected) = manifest
            .integrity
            .as_ref()
            .and_then(|integrity| integrity.sha256.get(*asset_id))
        {
            let actual = hex::encode(Sha256::digest(&content));
            if actual != expected.to_ascii_lowercase() {
                return Err(SkinMarketError::bad_request(
                    "asset_digest_mismatch",
                    format!("Appearance asset digest does not match: {path}"),
                ));
            }
        }
        if asset_id.as_str() == preview_id {
            if content.len() as u64 > APPEARANCE_MARKET_MAX_PREVIEW_BYTES {
                return Err(SkinMarketError::bad_request(
                    "preview_too_large",
                    "The selected appearance preview exceeds 4 MiB.",
                ));
            }
            preview_source = Some((content, definition.mime_type().to_string()));
        }
    }
    let (preview_bytes, preview_mime) = preview_source.ok_or_else(|| {
        SkinMarketError::bad_request(
            "preview_required",
            "Marketplace appearance packages must contain an image preview.",
        )
    })?;
    let preview = normalize_preview(&preview_bytes, &preview_mime)?;
    let canonical_manifest_json = serde_json::to_string(&canonicalize_json(raw_manifest))
        .map_err(SkinMarketError::internal)?;

    Ok(ValidatedAppearancePackage {
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
        canonical_manifest_json,
        preview,
    })
}

fn validate_manifest(manifest: &AppearanceManifest) -> SkinMarketResult<()> {
    if manifest.schema != "openbitfun.appearance" || manifest.schema_version != 1 {
        return Err(SkinMarketError::bad_request(
            "unsupported_manifest_schema",
            "Appearance packages must use openbitfun.appearance schema version 1.",
        ));
    }
    validate_identifier(&manifest.id, "package id")?;
    if manifest.name.trim().is_empty() || manifest.name.chars().count() > 100 {
        return Err(SkinMarketError::bad_request(
            "invalid_name",
            "Appearance names must contain between 1 and 100 characters.",
        ));
    }
    if manifest
        .author
        .as_ref()
        .is_some_and(|value| value.chars().count() > 100)
        || manifest.description.chars().count() > 500
    {
        return Err(SkinMarketError::bad_request(
            "invalid_manifest_text",
            "Appearance author and description fields exceed their allowed length.",
        ));
    }
    Version::parse(&manifest.version).map_err(|_| {
        SkinMarketError::bad_request(
            "invalid_package_version",
            "Appearance version must use semantic version syntax.",
        )
    })?;
    let mut capabilities = BTreeSet::new();
    for capability in &manifest.required_capabilities {
        if !SUPPORTED_CAPABILITIES.contains(&capability.as_str())
            || !capabilities.insert(capability)
        {
            return Err(SkinMarketError::bad_request(
                "invalid_required_capability",
                format!("Unsupported or duplicate appearance capability: {capability}"),
            ));
        }
    }
    for (name, value) in [
        ("globals", manifest.globals.as_ref()),
        ("materials", manifest.materials.as_ref()),
        ("components", manifest.components.as_ref()),
        ("scenes", manifest.scenes.as_ref()),
        ("renderers", manifest.renderers.as_ref()),
    ] {
        if let Some(value) = value {
            if !value.is_object() {
                return Err(SkinMarketError::bad_request(
                    "invalid_manifest_section",
                    format!("Appearance manifest section {name} must be an object."),
                ));
            }
            validate_declarative_value(value, name)?;
        }
    }
    if let Some(reference) = &manifest.preview {
        if reference.kind != "asset" {
            return Err(SkinMarketError::bad_request(
                "invalid_preview",
                "Appearance preview must reference a declared image asset.",
            ));
        }
        let asset = manifest.assets.get(&reference.asset_id).ok_or_else(|| {
            SkinMarketError::bad_request(
                "unknown_preview_asset",
                "Appearance preview references an unknown asset.",
            )
        })?;
        if !asset.is_image() {
            return Err(SkinMarketError::bad_request(
                "invalid_preview",
                "Appearance preview must reference an image asset.",
            ));
        }
    }
    if let Some(background) = &manifest.background_media {
        if background.kind != "video"
            || background
                .fit
                .as_deref()
                .is_some_and(|value| !matches!(value, "cover" | "contain"))
            || background.position.as_deref().is_some_and(|value| {
                !matches!(value, "center" | "top" | "right" | "bottom" | "left")
            })
        {
            return Err(SkinMarketError::bad_request(
                "invalid_background_media",
                "Appearance background media is invalid.",
            ));
        }
        if !manifest
            .required_capabilities
            .iter()
            .any(|value| value == "background-media.v1")
        {
            return Err(SkinMarketError::bad_request(
                "missing_background_media_capability",
                "Video backgrounds require background-media.v1.",
            ));
        }
        if !matches!(
            manifest.assets.get(&background.asset_id),
            Some(AssetDefinition::Video { .. })
        ) || !matches!(
            manifest.assets.get(&background.poster_asset_id),
            Some(AssetDefinition::Image { .. })
        ) {
            return Err(SkinMarketError::bad_request(
                "invalid_background_media",
                "Video backgrounds require declared video and image poster assets.",
            ));
        }
    }
    for (asset_id, asset) in &manifest.assets {
        if matches!(asset, AssetDefinition::Video { .. })
            && manifest
                .background_media
                .as_ref()
                .is_none_or(|background| background.asset_id != *asset_id)
        {
            return Err(SkinMarketError::bad_request(
                "unused_video_asset",
                "Video assets are allowed only as the selected top-level background media.",
            ));
        }
    }
    if let Some(integrity) = &manifest.integrity {
        for (asset_id, digest) in &integrity.sha256 {
            if !manifest.assets.contains_key(asset_id)
                || digest.len() != 64
                || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(SkinMarketError::bad_request(
                    "invalid_asset_digest",
                    "Appearance integrity entries must reference assets with SHA-256 digests.",
                ));
            }
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &str) -> SkinMarketResult<()> {
    let bytes = value.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 100
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'.' | b'-')
        })
        && !value.contains("..")
        && !value.contains("--")
        && !value.ends_with(['.', '-']);
    if valid {
        Ok(())
    } else {
        Err(SkinMarketError::bad_request(
            "invalid_identifier",
            format!("Appearance {field} must be a lowercase dotted or dashed identifier."),
        ))
    }
}

fn validate_declarative_value(value: &Value, field: &str) -> SkinMarketResult<()> {
    match value {
        Value::Object(values) => {
            for (key, nested) in values {
                let key_lower = key.to_ascii_lowercase();
                if matches!(
                    key_lower.as_str(),
                    "css" | "html" | "script" | "javascript" | "selector" | "stylesheet"
                ) {
                    return Err(SkinMarketError::bad_request(
                        "executable_appearance_content",
                        format!(
                            "Appearance section {field} contains forbidden executable styling."
                        ),
                    ));
                }
                validate_declarative_value(nested, field)?;
            }
        }
        Value::Array(values) => {
            for nested in values {
                validate_declarative_value(nested, field)?;
            }
        }
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            if lower.contains("http://")
                || lower.contains("https://")
                || lower.contains("javascript:")
                || lower.contains("data:")
                || lower.contains("url(")
                || lower.contains('<')
            {
                return Err(SkinMarketError::bad_request(
                    "external_appearance_content",
                    format!("Appearance section {field} contains forbidden external content."),
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn select_preview_asset(manifest: &AppearanceManifest) -> SkinMarketResult<String> {
    manifest
        .preview
        .as_ref()
        .map(|reference| reference.asset_id.clone())
        .ok_or_else(|| {
            SkinMarketError::bad_request(
                "preview_required",
                "Marketplace appearance packages must explicitly declare an image preview.",
            )
        })
}

fn validate_asset_mime(
    path: &str,
    definition: &AssetDefinition,
    bytes: &[u8],
) -> SkinMarketResult<()> {
    match definition {
        AssetDefinition::Image { mime_type, .. } => {
            let format = image::guess_format(bytes).map_err(|_| {
                SkinMarketError::bad_request(
                    "invalid_asset_mime",
                    format!("Appearance image MIME could not be identified: {path}"),
                )
            })?;
            let actual = match format {
                image::ImageFormat::Png => "image/png",
                image::ImageFormat::Jpeg => "image/jpeg",
                image::ImageFormat::WebP => "image/webp",
                image::ImageFormat::Gif => "image/gif",
                _ => {
                    return Err(SkinMarketError::bad_request(
                        "unsupported_asset_mime",
                        format!("Appearance image format is not allowed: {path}"),
                    ))
                }
            };
            if actual != mime_type {
                return Err(SkinMarketError::bad_request(
                    "asset_mime_mismatch",
                    format!("Appearance image MIME does not match its declaration: {path}"),
                ));
            }
            let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), format)
                .into_dimensions()
                .map_err(|error| {
                    SkinMarketError::bad_request(
                        "invalid_image",
                        format!("Appearance image dimensions are invalid: {error}"),
                    )
                })?;
            if width == 0
                || height == 0
                || width > MAX_IMAGE_DIMENSION
                || height > MAX_IMAGE_DIMENSION
                || u64::from(width).saturating_mul(u64::from(height)) > MAX_IMAGE_PIXELS
            {
                return Err(SkinMarketError::bad_request(
                    "invalid_image_dimensions",
                    format!("Appearance image dimensions exceed the limit: {path}"),
                ));
            }
        }
        AssetDefinition::Video { mime_type, .. } => {
            let actual = if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
                "video/mp4"
            } else if bytes.len() >= 4
                && bytes[..4] == [0x1a, 0x45, 0xdf, 0xa3]
                && bytes
                    .windows(4)
                    .take(4_096)
                    .any(|window| window.eq_ignore_ascii_case(b"webm"))
            {
                "video/webm"
            } else {
                return Err(SkinMarketError::bad_request(
                    "invalid_asset_mime",
                    format!("Appearance video MIME could not be identified: {path}"),
                ));
            };
            if actual != mime_type || !matches!(mime_type.as_str(), "video/mp4" | "video/webm") {
                return Err(SkinMarketError::bad_request(
                    "asset_mime_mismatch",
                    format!("Appearance video MIME does not match its declaration: {path}"),
                ));
            }
        }
    }
    Ok(())
}

fn normalize_preview(bytes: &[u8], mime_type: &str) -> SkinMarketResult<ValidatedPreview> {
    let format = match mime_type {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        "image/gif" => image::ImageFormat::Gif,
        _ => {
            return Err(SkinMarketError::bad_request(
                "invalid_preview",
                "The selected appearance preview is not an image.",
            ))
        }
    };
    let (source_width, source_height) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|error| {
            SkinMarketError::bad_request(
                "invalid_preview",
                format!("The appearance preview dimensions are invalid: {error}"),
            )
        })?;
    if u64::from(source_width).saturating_mul(u64::from(source_height))
        > APPEARANCE_MARKET_MAX_PREVIEW_PIXELS
    {
        return Err(SkinMarketError::bad_request(
            "preview_dimensions_too_large",
            "The marketplace preview exceeds the 16 megapixel decode budget.",
        ));
    }
    let decoded = image::load_from_memory_with_format(bytes, format).map_err(|error| {
        SkinMarketError::bad_request(
            "invalid_preview",
            format!("The appearance preview could not be decoded: {error}"),
        )
    })?;
    let normalized = if decoded.width() > 1_600 || decoded.height() > 1_600 {
        decoded.thumbnail(1_600, 1_600)
    } else {
        decoded
    };
    let (width, height) = normalized.dimensions();
    let mut cursor = Cursor::new(Vec::new());
    normalized
        .write_to(&mut cursor, image::ImageFormat::WebP)
        .map_err(SkinMarketError::internal)?;
    let normalized_bytes = cursor.into_inner();
    Ok(ValidatedPreview {
        sha256: hex::encode(Sha256::digest(&normalized_bytes)),
        bytes: normalized_bytes,
        width,
        height,
    })
}

fn validate_archive_path(path: &str, directory: bool) -> SkinMarketResult<()> {
    let candidate = if directory {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        path
    };
    let safe = !candidate.is_empty()
        && candidate.len() <= 200
        && !candidate.starts_with('/')
        && !candidate.contains("..")
        && !candidate.contains('\\')
        && !candidate.contains(':')
        && !candidate.contains("//")
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'/' | b'-'));
    if safe {
        Ok(())
    } else {
        Err(SkinMarketError::bad_request(
            "unsafe_package_path",
            format!("The appearance package contains an unsafe path: {path}"),
        ))
    }
}

fn assert_safe_central_directory(bytes: &[u8]) -> SkinMarketResult<()> {
    const END: [u8; 4] = *b"PK\x05\x06";
    const CENTRAL: [u8; 4] = *b"PK\x01\x02";
    const ZIP64_LOCATOR: [u8; 4] = *b"PK\x06\x07";
    let search_start = bytes.len().saturating_sub(65_557);
    let end = (search_start..bytes.len().saturating_sub(3))
        .rev()
        .find(|offset| bytes[*offset..*offset + 4] == END)
        .ok_or_else(|| {
            SkinMarketError::bad_request(
                "invalid_package",
                "Appearance archive central directory is missing.",
            )
        })?;
    if end + 22 > bytes.len() {
        return Err(SkinMarketError::bad_request(
            "invalid_package",
            "Appearance archive central directory is truncated.",
        ));
    }
    if end >= 20 && bytes[end - 20..end - 16] == ZIP64_LOCATOR {
        return Err(SkinMarketError::bad_request(
            "zip64_forbidden",
            "ZIP64 appearance packages are not allowed.",
        ));
    }
    let disk = u16::from_le_bytes([bytes[end + 4], bytes[end + 5]]);
    let central_disk = u16::from_le_bytes([bytes[end + 6], bytes[end + 7]]);
    let entries = u16::from_le_bytes([bytes[end + 10], bytes[end + 11]]);
    let central_size = u32::from_le_bytes(bytes[end + 12..end + 16].try_into().unwrap());
    let central_offset = u32::from_le_bytes(bytes[end + 16..end + 20].try_into().unwrap());
    if usize::from(entries) > APPEARANCE_MARKET_MAX_ENTRIES + MAX_DIRECTORY_ENTRIES {
        return Err(SkinMarketError::bad_request(
            "invalid_package_entry_count",
            "The Appearance package contains too many archive entries.",
        ));
    }
    if disk != 0
        || central_disk != 0
        || entries == u16::MAX
        || central_size == u32::MAX
        || central_offset == u32::MAX
        || usize::try_from(central_offset)
            .ok()
            .and_then(|offset| offset.checked_add(central_size as usize))
            .is_none_or(|limit| limit > end)
    {
        return Err(SkinMarketError::bad_request(
            "zip64_forbidden",
            "Multi-disk and ZIP64 appearance packages are not allowed.",
        ));
    }
    let mut offset = central_offset as usize;
    let mut expanded = 0_u64;
    for _ in 0..entries {
        if offset + 46 > bytes.len() || bytes[offset..offset + 4] != CENTRAL {
            return Err(SkinMarketError::bad_request(
                "invalid_package",
                "Appearance archive central directory entry is invalid.",
            ));
        }
        let flags = u16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]);
        let compressed = u32::from_le_bytes(bytes[offset + 20..offset + 24].try_into().unwrap());
        let uncompressed = u32::from_le_bytes(bytes[offset + 24..offset + 28].try_into().unwrap());
        let name_len = u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[offset + 30], bytes[offset + 31]]) as usize;
        let comment_len = u16::from_le_bytes([bytes[offset + 32], bytes[offset + 33]]) as usize;
        let disk_start = u16::from_le_bytes([bytes[offset + 34], bytes[offset + 35]]);
        let local_header_offset =
            u32::from_le_bytes(bytes[offset + 42..offset + 46].try_into().unwrap());
        if flags & 1 != 0
            || compressed == u32::MAX
            || uncompressed == u32::MAX
            || disk_start != 0
            || local_header_offset == u32::MAX
        {
            return Err(SkinMarketError::bad_request(
                "encrypted_or_zip64_entry",
                "Encrypted and ZIP64 appearance package entries are not allowed.",
            ));
        }
        let entry_end = offset
            .checked_add(46 + name_len + extra_len + comment_len)
            .filter(|value| *value <= bytes.len())
            .ok_or_else(|| {
                SkinMarketError::bad_request(
                    "invalid_package",
                    "Appearance archive central directory entry is truncated.",
                )
            })?;
        let mut extra_offset = offset + 46 + name_len;
        let extra_end = extra_offset + extra_len;
        while extra_offset < extra_end {
            if extra_offset + 4 > extra_end {
                return Err(SkinMarketError::bad_request(
                    "invalid_package",
                    "Appearance archive extra data is truncated.",
                ));
            }
            let field_id = u16::from_le_bytes([bytes[extra_offset], bytes[extra_offset + 1]]);
            let field_size =
                u16::from_le_bytes([bytes[extra_offset + 2], bytes[extra_offset + 3]]) as usize;
            extra_offset = extra_offset
                .checked_add(4 + field_size)
                .filter(|value| *value <= extra_end)
                .ok_or_else(|| {
                    SkinMarketError::bad_request(
                        "invalid_package",
                        "Appearance archive extra data is truncated.",
                    )
                })?;
            if field_id == 0x0001 {
                return Err(SkinMarketError::bad_request(
                    "zip64_forbidden",
                    "ZIP64 appearance package entries are not allowed.",
                ));
            }
        }
        expanded = expanded.saturating_add(u64::from(uncompressed));
        if expanded > APPEARANCE_MARKET_MAX_UNCOMPRESSED_BYTES {
            return Err(SkinMarketError::bad_request(
                "package_expansion_too_large",
                "The appearance package expands beyond 128 MiB.",
            ));
        }
        offset = entry_end;
    }
    Ok(())
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn png() -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(4, 3)
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    fn package_with_digest(
        mut manifest: Value,
        extras: &[(&str, Vec<u8>)],
        digest: Option<String>,
    ) -> Vec<u8> {
        let preview = png();
        manifest["integrity"] = serde_json::json!({
            "sha256": {
                "preview": digest.unwrap_or_else(|| hex::encode(Sha256::digest(&preview)))
            }
        });
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("appearance.json", SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(serde_json::to_vec(&manifest).unwrap().as_slice())
            .unwrap();
        writer
            .add_directory("assets/", SimpleFileOptions::default())
            .unwrap();
        writer
            .start_file("assets/preview.png", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&preview).unwrap();
        for (name, content) in extras {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn package(manifest: Value, extras: &[(&str, Vec<u8>)]) -> Vec<u8> {
        package_with_digest(manifest, extras, None)
    }

    fn manifest() -> Value {
        serde_json::json!({
            "schema": "openbitfun.appearance",
            "schemaVersion": 1,
            "id": "example.aurora",
            "name": "Aurora",
            "description": "A safe appearance",
            "version": "1.2.3",
            "mode": "dark",
            "preview": { "kind": "asset", "assetId": "preview" },
            "requiredCapabilities": ["assets.v1"],
            "assets": {
                "preview": {
                    "kind": "image",
                    "mimeType": "image/png",
                    "source": { "kind": "package", "path": "assets/preview.png" }
                }
            }
        })
    }

    #[test]
    fn valid_package_is_bound_to_a_normalized_webp_preview() {
        let result = validate_appearance_package(&package(manifest(), &[])).unwrap();
        assert_eq!(result.meta.package_id, "example.aurora");
        assert_eq!(result.preview.width, 4);
        assert_eq!(result.preview.height, 3);
        assert_eq!(
            image::guess_format(&result.preview.bytes).unwrap(),
            image::ImageFormat::WebP
        );
    }

    #[test]
    fn package_rejects_unknown_files_schema_and_digest_mismatch() {
        assert_eq!(
            validate_appearance_package(&package(manifest(), &[("extra.txt", b"x".to_vec())]))
                .unwrap_err()
                .code,
            "undeclared_package_entry"
        );
        let mut wrong_schema = manifest();
        wrong_schema["schemaVersion"] = Value::from(2);
        assert_eq!(
            validate_appearance_package(&package(wrong_schema, &[]))
                .unwrap_err()
                .code,
            "unsupported_manifest_schema"
        );
        let wrong_digest = package_with_digest(manifest(), &[], Some("00".repeat(32)));
        assert_eq!(
            validate_appearance_package(&wrong_digest).unwrap_err().code,
            "asset_digest_mismatch"
        );
    }

    #[test]
    fn package_rejects_invalid_semver_mime_and_external_content() {
        let mut invalid = manifest();
        invalid["version"] = Value::from("latest");
        assert_eq!(
            validate_appearance_package(&package(invalid, &[]))
                .unwrap_err()
                .code,
            "invalid_package_version"
        );
        let mut invalid = manifest();
        invalid["assets"]["preview"]["mimeType"] = Value::from("image/jpeg");
        assert_eq!(
            validate_appearance_package(&package(invalid, &[]))
                .unwrap_err()
                .code,
            "asset_mime_mismatch"
        );
        let mut invalid = manifest();
        invalid["globals"] = serde_json::json!({ "background": "url(https://invalid/)" });
        assert_eq!(
            validate_appearance_package(&package(invalid, &[]))
                .unwrap_err()
                .code,
            "external_appearance_content"
        );
    }
}
