use crate::error::{MarketError, MarketResult};
use image::GenericImageView;
use openbitfun_product_domains::miniapp::market::{
    MarketPackageMeta, MARKET_MAX_PACKAGE_BYTES, MARKET_MAX_SCREENSHOT_BYTES,
    MARKET_MAX_UNCOMPRESSED_BYTES,
};
use openbitfun_product_domains::product_release::OPENBITFUN_INITIAL_RELEASE_VERSION;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read};
use zip::ZipArchive;

const REQUIRED_ENTRIES: &[&str] = &[
    "meta.json",
    "source/index.html",
    "source/style.css",
    "source/ui.js",
    "source/worker.js",
    "source/esm_dependencies.json",
];

#[derive(Debug, Clone)]
pub struct ValidatedMarketPackage {
    pub sha256: String,
    pub size: u64,
    pub meta: MarketPackageMeta,
    pub source_files: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ValidatedScreenshot {
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn validate_market_package(bytes: &[u8]) -> MarketResult<ValidatedMarketPackage> {
    if bytes.is_empty() {
        return Err(MarketError::bad_request(
            "empty_package",
            "The MiniApp package is empty.",
        ));
    }
    if bytes.len() as u64 > MARKET_MAX_PACKAGE_BYTES {
        return Err(MarketError::bad_request(
            "package_too_large",
            "The compressed MiniApp package exceeds 20 MiB.",
        ));
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        MarketError::bad_request("invalid_package", format!("Invalid ZIP archive: {error}"))
    })?;
    let mut seen = HashSet::new();
    let mut files = BTreeMap::new();
    let mut total_uncompressed = 0_u64;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            MarketError::bad_request("invalid_package", format!("Invalid ZIP entry: {error}"))
        })?;
        if entry.is_dir() {
            return Err(MarketError::bad_request(
                "forbidden_package_entry",
                "Marketplace packages cannot contain directory entries.",
            ));
        }
        let Some(enclosed_name) = entry.enclosed_name() else {
            return Err(MarketError::bad_request(
                "unsafe_package_path",
                "The package contains an unsafe path.",
            ));
        };
        let name = enclosed_name
            .to_str()
            .ok_or_else(|| {
                MarketError::bad_request("invalid_package_path", "Package paths must use UTF-8.")
            })?
            .replace('\\', "/");
        let normalized = name.to_ascii_lowercase();
        if !seen.insert(normalized) {
            return Err(MarketError::bad_request(
                "duplicate_package_path",
                format!("The package contains a duplicate path: {name}"),
            ));
        }
        if !REQUIRED_ENTRIES.contains(&name.as_str()) {
            return Err(MarketError::bad_request(
                "forbidden_package_entry",
                format!("The package contains a forbidden entry: {name}"),
            ));
        }
        if entry.unix_mode().is_some_and(|mode| {
            let file_type = mode & 0o170000;
            file_type != 0 && file_type != 0o100000
        }) {
            return Err(MarketError::bad_request(
                "package_link_forbidden",
                "Links and non-regular files are not allowed in MiniApp packages.",
            ));
        }
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        if total_uncompressed > MARKET_MAX_UNCOMPRESSED_BYTES {
            return Err(MarketError::bad_request(
                "package_expansion_too_large",
                "The uncompressed MiniApp package exceeds 64 MiB.",
            ));
        }
        let mut content = String::new();
        entry.read_to_string(&mut content).map_err(|_| {
            MarketError::bad_request(
                "invalid_package_text",
                format!("{name} must be valid UTF-8 text."),
            )
        })?;
        files.insert(name, content);
    }

    for required in REQUIRED_ENTRIES {
        if !files.contains_key(*required) {
            return Err(MarketError::bad_request(
                "missing_package_entry",
                format!("The package is missing {required}."),
            ));
        }
    }

    validate_worker(
        files
            .get("source/worker.js")
            .map(String::as_str)
            .unwrap_or(""),
    )?;
    validate_esm_dependencies(
        files
            .get("source/esm_dependencies.json")
            .map(String::as_str)
            .unwrap_or(""),
    )?;
    validate_no_remote_executable_content(&files)?;

    let raw_meta: Value = serde_json::from_str(
        files
            .get("meta.json")
            .map(String::as_str)
            .unwrap_or_default(),
    )
    .map_err(|error| {
        MarketError::bad_request("invalid_meta", format!("Invalid meta.json: {error}"))
    })?;
    if raw_meta.pointer("/permissions/node/enabled") != Some(&Value::Bool(false)) {
        return Err(MarketError::bad_request(
            "node_forbidden",
            "Marketplace packages must explicitly set permissions.node.enabled to false.",
        ));
    }
    let meta: MarketPackageMeta = serde_json::from_value(raw_meta).map_err(|error| {
        MarketError::bad_request(
            "invalid_meta",
            format!("Invalid marketplace metadata: {error}"),
        )
    })?;
    validate_package_meta(&meta)?;

    Ok(ValidatedMarketPackage {
        sha256: hex::encode(Sha256::digest(bytes)),
        size: bytes.len() as u64,
        meta,
        source_files: files,
    })
}

pub fn validate_screenshot(bytes: &[u8]) -> MarketResult<ValidatedScreenshot> {
    if bytes.is_empty() || bytes.len() as u64 > MARKET_MAX_SCREENSHOT_BYTES {
        return Err(MarketError::bad_request(
            "invalid_screenshot_size",
            "Marketplace images must be between 1 byte and 5 MiB.",
        ));
    }
    let format = image::guess_format(bytes).map_err(|error| {
        MarketError::bad_request(
            "invalid_screenshot",
            format!("The marketplace image format could not be identified: {error}"),
        )
    })?;
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP
    ) {
        return Err(MarketError::bad_request(
            "unsupported_screenshot_format",
            "Marketplace images must be PNG, JPEG, or WebP.",
        ));
    }
    let (original_width, original_height) =
        image::ImageReader::with_format(Cursor::new(bytes), format)
            .into_dimensions()
            .map_err(|error| {
                MarketError::bad_request(
                    "invalid_screenshot",
                    format!("The marketplace image dimensions could not be read: {error}"),
                )
            })?;
    let pixels = u64::from(original_width).saturating_mul(u64::from(original_height));
    if original_width == 0
        || original_height == 0
        || original_width > 16_384
        || original_height > 16_384
        || pixels > 40_000_000
    {
        return Err(MarketError::bad_request(
            "invalid_screenshot_dimensions",
            "Marketplace image dimensions are invalid or exceed 40 megapixels.",
        ));
    }
    let image = image::load_from_memory_with_format(bytes, format).map_err(|error| {
        MarketError::bad_request(
            "invalid_screenshot",
            format!("The marketplace image could not be decoded: {error}"),
        )
    })?;
    let normalized = if original_width > 2560 || original_height > 2560 {
        image.thumbnail(2560, 2560)
    } else {
        image
    };
    let (width, height) = normalized.dimensions();
    let mut cursor = Cursor::new(Vec::new());
    normalized
        .write_to(&mut cursor, image::ImageFormat::WebP)
        .map_err(MarketError::internal)?;
    let normalized_bytes = cursor.into_inner();
    Ok(ValidatedScreenshot {
        sha256: hex::encode(Sha256::digest(&normalized_bytes)),
        bytes: normalized_bytes,
        width,
        height,
    })
}

fn validate_worker(worker: &str) -> MarketResult<()> {
    let compact = worker
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<String>()
        .replace(char::is_whitespace, "");
    if compact.is_empty()
        || matches!(
            compact.as_str(),
            "module.exports={};" | "module.exports={}" | "export{};" | "export{}"
        )
    {
        return Ok(());
    }
    Err(MarketError::bad_request(
        "worker_forbidden",
        "Marketplace v1 only accepts an empty or no-op worker.js.",
    ))
}

fn validate_esm_dependencies(content: &str) -> MarketResult<()> {
    let dependencies: Vec<Value> = serde_json::from_str(content).map_err(|error| {
        MarketError::bad_request(
            "invalid_esm_dependencies",
            format!("Invalid esm_dependencies.json: {error}"),
        )
    })?;
    if !dependencies.is_empty() {
        return Err(MarketError::bad_request(
            "esm_dependencies_forbidden",
            "Marketplace v1 does not accept runtime ESM dependencies.",
        ));
    }
    Ok(())
}

fn validate_no_remote_executable_content(files: &BTreeMap<String, String>) -> MarketResult<()> {
    let html = files
        .get("source/index.html")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let forbidden_html = [
        "<script src=",
        "<iframe",
        "<object",
        "<embed",
        "javascript:",
        "http-equiv=\"refresh\"",
        "http-equiv='refresh'",
    ];
    if forbidden_html.iter().any(|needle| html.contains(needle)) {
        return Err(MarketError::bad_request(
            "remote_executable_content_forbidden",
            "The MiniApp HTML contains a forbidden executable or navigation element.",
        ));
    }
    let ui_js = files
        .get("source/ui.js")
        .map(String::as_str)
        .unwrap_or_default();
    let compact = ui_js.replace(char::is_whitespace, "");
    if compact.contains("import(\"http")
        || compact.contains("import('http")
        || compact.contains("import(`http")
        || compact.contains("eval(")
        || compact.contains("newFunction(")
    {
        return Err(MarketError::bad_request(
            "dynamic_code_forbidden",
            "Remote imports and dynamic code evaluation are not allowed.",
        ));
    }
    Ok(())
}

fn validate_package_meta(meta: &MarketPackageMeta) -> MarketResult<()> {
    if meta.name.trim().is_empty() || meta.name.chars().count() > 80 {
        return Err(MarketError::bad_request(
            "invalid_name",
            "MiniApp names must contain between 1 and 80 characters.",
        ));
    }
    if meta.description.trim().is_empty() || meta.description.chars().count() > 500 {
        return Err(MarketError::bad_request(
            "invalid_description",
            "MiniApp descriptions must contain between 1 and 500 characters.",
        ));
    }
    if !openbitfun_product_domains::miniapp::market::validate_market_category(&meta.category) {
        return Err(MarketError::bad_request(
            "invalid_category",
            "The MiniApp category is not supported.",
        ));
    }
    if meta.tags.len() > 10 || meta.tags.iter().any(|tag| tag.chars().count() > 32) {
        return Err(MarketError::bad_request(
            "invalid_tags",
            "MiniApps may declare at most 10 tags of up to 32 characters.",
        ));
    }
    if meta
        .permissions
        .node
        .as_ref()
        .is_none_or(|node| node.enabled)
    {
        return Err(MarketError::bad_request(
            "node_forbidden",
            "Marketplace packages cannot enable Node.",
        ));
    }
    if meta
        .permissions
        .fs
        .as_ref()
        .into_iter()
        .flat_map(|fs| {
            fs.read
                .iter()
                .chain(fs.write.iter())
                .flat_map(|values| values.iter())
        })
        .any(|scope| scope == "{home}" || scope.starts_with('/') || scope.contains(":\\"))
    {
        return Err(MarketError::bad_request(
            "broad_filesystem_scope_forbidden",
            "Marketplace packages may use appdata, workspace, or user-selected paths only.",
        ));
    }
    if meta
        .permissions
        .shell
        .as_ref()
        .and_then(|shell| shell.allow.as_ref())
        .is_some_and(|commands| {
            commands
                .iter()
                .any(|command| is_forbidden_interpreter(command))
        })
    {
        return Err(MarketError::bad_request(
            "shell_interpreter_forbidden",
            "Shells and general-purpose interpreters cannot be allowlisted.",
        ));
    }
    Ok(())
}

fn is_forbidden_interpreter(command: &str) -> bool {
    matches!(
        command.trim().to_ascii_lowercase().as_str(),
        "sh" | "bash"
            | "zsh"
            | "fish"
            | "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "python"
            | "python3"
            | "node"
            | "bun"
            | "deno"
            | "ruby"
            | "perl"
    )
}

pub fn validate_min_openbitfun_version(value: &str) -> MarketResult<()> {
    let version = semver::Version::parse(value).map_err(|_| {
        MarketError::bad_request(
            "invalid_min_openbitfun_version",
            "minOpenBitFunVersion must use semantic version syntax, for example 1.0.0.",
        )
    })?;
    if version < OPENBITFUN_INITIAL_RELEASE_VERSION {
        return Err(MarketError::bad_request(
            "invalid_min_openbitfun_version",
            "minOpenBitFunVersion must be 1.0.0 or newer.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn minimum_openbitfun_version_starts_at_initial_release() {
        assert!(validate_min_openbitfun_version("1.0.0").is_ok());
        assert!(validate_min_openbitfun_version("1.2.0").is_ok());

        let pre_release_identity = [0, 9, 0]
            .into_iter()
            .map(|part| part.to_string())
            .collect::<Vec<_>>()
            .join(".");
        assert_eq!(
            validate_min_openbitfun_version(&pre_release_identity)
                .expect_err("pre-1.0 product versions must be rejected")
                .code,
            "invalid_min_openbitfun_version"
        );
        assert_eq!(
            validate_min_openbitfun_version("1.0.0-rc.1")
                .expect_err("pre-release versions before 1.0.0 must be rejected")
                .code,
            "invalid_min_openbitfun_version"
        );
    }

    fn package(meta: &str, worker: &str, esm: &str) -> Vec<u8> {
        package_with_extra(meta, worker, esm, &[])
    }

    fn package_with_extra(
        meta: &str,
        worker: &str,
        esm: &str,
        extra: &[(&str, &str, Option<u32>)],
    ) -> Vec<u8> {
        package_custom(meta, "<main>Test</main>", worker, esm, extra)
    }

    fn package_custom(
        meta: &str,
        html: &str,
        worker: &str,
        esm: &str,
        extra: &[(&str, &str, Option<u32>)],
    ) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        for (name, content) in [
            ("meta.json", meta),
            ("source/index.html", html),
            ("source/style.css", "body { margin: 0; }"),
            ("source/ui.js", "document.body.dataset.ready = '1';"),
            ("source/worker.js", worker),
            ("source/esm_dependencies.json", esm),
        ] {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        for (name, content, mode) in extra {
            let options = mode.map_or_else(
                || {
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                },
                |mode| {
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .unix_permissions(mode)
                },
            );
            writer.start_file(*name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn set_unix_file_type(bytes: &mut [u8], entry_name: &str, mode: u32) {
        let mut offset = 0;
        while offset + 46 <= bytes.len() {
            let Some(relative) = bytes[offset..]
                .windows(4)
                .position(|window| window == b"PK\x01\x02")
            else {
                panic!("central directory entry not found");
            };
            let header = offset + relative;
            let name_len = u16::from_le_bytes([bytes[header + 28], bytes[header + 29]]) as usize;
            let name_start = header + 46;
            let name_end = name_start + name_len;
            if name_end <= bytes.len() && &bytes[name_start..name_end] == entry_name.as_bytes() {
                bytes[header + 4..header + 6].copy_from_slice(&0x0314_u16.to_le_bytes());
                bytes[header + 38..header + 42].copy_from_slice(&(mode << 16).to_le_bytes());
                return;
            }
            offset = name_end;
        }
        panic!("target central directory entry not found");
    }

    fn valid_meta() -> &'static str {
        r#"{
          "name":"Test App",
          "description":"A safe test MiniApp.",
          "icon":"🧪",
          "category":"developer",
          "tags":["test"],
          "version":1,
          "permissions":{"node":{"enabled":false}}
        }"#
    }

    #[test]
    fn package_accepts_self_contained_node_disabled_bundle() {
        let result = validate_market_package(&package(valid_meta(), "module.exports = {};", "[]"))
            .expect("valid package");
        assert_eq!(result.meta.name, "Test App");
    }

    #[test]
    fn package_rejects_node_and_esm_execution_paths() {
        let node_meta = valid_meta().replace("\"enabled\":false", "\"enabled\":true");
        assert_eq!(
            validate_market_package(&package(&node_meta, "", "[]"))
                .unwrap_err()
                .code,
            "node_forbidden"
        );
        assert_eq!(
            validate_market_package(&package(valid_meta(), "", "[{\"name\":\"react\"}]"))
                .unwrap_err()
                .code,
            "esm_dependencies_forbidden"
        );
    }

    #[test]
    fn package_rejects_worker_logic() {
        assert_eq!(
            validate_market_package(&package(
                valid_meta(),
                "module.exports = { run() { return 1; } };",
                "[]",
            ))
            .unwrap_err()
            .code,
            "worker_forbidden"
        );
    }

    #[test]
    fn package_rejects_zip_slip_forbidden_files_and_case_collisions() {
        let zip_slip = package_with_extra(
            valid_meta(),
            "",
            "[]",
            &[("../outside.txt", "secret", None)],
        );
        assert_eq!(
            validate_market_package(&zip_slip).unwrap_err().code,
            "unsafe_package_path"
        );

        let forbidden = package_with_extra(valid_meta(), "", "[]", &[("package.json", "{}", None)]);
        assert_eq!(
            validate_market_package(&forbidden).unwrap_err().code,
            "forbidden_package_entry"
        );

        let collision = package_with_extra(valid_meta(), "", "[]", &[("META.JSON", "{}", None)]);
        assert_eq!(
            validate_market_package(&collision).unwrap_err().code,
            "duplicate_package_path"
        );
    }

    #[test]
    fn package_rejects_directory_entries() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .add_directory("source/", SimpleFileOptions::default())
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        assert_eq!(
            validate_market_package(&bytes).unwrap_err().code,
            "forbidden_package_entry"
        );
    }

    #[test]
    fn screenshot_rejects_formats_outside_png_jpeg_and_webp() {
        assert_eq!(
            validate_screenshot(b"BM\0\0\0\0\0\0\0\0\0\0")
                .unwrap_err()
                .code,
            "unsupported_screenshot_format"
        );
    }

    #[test]
    fn package_rejects_links_and_remote_executable_content() {
        let mut link = package(valid_meta(), "", "[]");
        set_unix_file_type(&mut link, "source/worker.js", 0o120777);
        assert_eq!(
            validate_market_package(&link).unwrap_err().code,
            "package_link_forbidden"
        );

        let mut files = BTreeMap::new();
        files.insert(
            "source/index.html".to_string(),
            "<script src=\"https://example.com/code.js\"></script>".to_string(),
        );
        files.insert("source/ui.js".to_string(), String::new());
        assert_eq!(
            validate_no_remote_executable_content(&files)
                .unwrap_err()
                .code,
            "remote_executable_content_forbidden"
        );
    }

    #[test]
    fn package_rejects_more_than_64_mib_of_expanded_content() {
        let oversized_html = "x".repeat(MARKET_MAX_UNCOMPRESSED_BYTES as usize + 1);
        let bytes = package_custom(valid_meta(), &oversized_html, "", "[]", &[]);
        assert_eq!(
            validate_market_package(&bytes).unwrap_err().code,
            "package_expansion_too_large"
        );
    }
}
