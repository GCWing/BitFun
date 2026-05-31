//! Built-in MiniApp bundle contracts and pure seed policy.

use crate::miniapp::storage::{
    build_package_json, ESM_DEPS_JSON, INDEX_HTML, STYLE_CSS, UI_JS, WORKER_JS,
};
use crate::miniapp::types::MiniAppMeta;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BUILTIN_INSTALL_MARKER: &str = ".builtin-manifest.json";
pub const LEGACY_BUILTIN_VERSION_MARKER: &str = ".builtin-version";
pub const BUILTIN_PLACEHOLDER_COMPILED_HTML: &str =
    "<!DOCTYPE html><html><body>Loading...</body></html>";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuiltinInstallMarker {
    pub version: u32,
    pub hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltinSeedArtifacts {
    pub content_hash: String,
    pub marker: BuiltinInstallMarker,
    pub legacy_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltinSeedCheck {
    Skip,
    NeedsSeed(BuiltinSeedArtifacts),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltinSeedAction {
    PreserveLocalOverride(BuiltinSeedArtifacts),
    SeedBundle(BuiltinSeedArtifacts),
}

/// Pure built-in MiniApp asset bundle shape. The owning runtime still decides
/// how assets are embedded, seeded, compiled, and persisted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinMiniAppBundle {
    pub id: &'static str,
    pub version: u32,
    pub meta_json: &'static str,
    pub html: &'static str,
    pub css: &'static str,
    pub ui_js: &'static str,
    pub worker_js: &'static str,
    pub esm_dependencies_json: &'static str,
}

pub fn builtin_content_hash(app: &BuiltinMiniAppBundle) -> String {
    let mut hasher = Sha256::new();
    hash_builtin_asset(&mut hasher, "meta.json", app.meta_json);
    hash_builtin_asset(&mut hasher, "index.html", app.html);
    hash_builtin_asset(&mut hasher, "style.css", app.css);
    hash_builtin_asset(&mut hasher, "ui.js", app.ui_js);
    hash_builtin_asset(&mut hasher, "worker.js", app.worker_js);
    hash_builtin_asset(
        &mut hasher,
        "esm_dependencies.json",
        app.esm_dependencies_json,
    );
    format!("sha256:{}", hex_encode(&hasher.finalize()))
}

pub fn build_builtin_install_marker(
    app: &BuiltinMiniAppBundle,
    content_hash: &str,
) -> BuiltinInstallMarker {
    BuiltinInstallMarker {
        version: app.version,
        hash: content_hash.to_string(),
    }
}

pub fn legacy_builtin_version_marker_content(app: &BuiltinMiniAppBundle) -> String {
    app.version.to_string()
}

pub fn build_builtin_seed_artifacts(app: &BuiltinMiniAppBundle) -> BuiltinSeedArtifacts {
    let content_hash = builtin_content_hash(app);
    BuiltinSeedArtifacts {
        marker: build_builtin_install_marker(app, &content_hash),
        legacy_version: legacy_builtin_version_marker_content(app),
        content_hash,
    }
}

pub fn preserved_builtin_created_at(existing_meta_json: Option<&str>) -> Option<i64> {
    existing_meta_json
        .and_then(|existing| serde_json::from_str::<MiniAppMeta>(existing).ok())
        .map(|meta| meta.created_at)
}

pub fn build_builtin_seed_meta(
    app: &BuiltinMiniAppBundle,
    preserved_created_at: Option<i64>,
    now: i64,
) -> serde_json::Result<MiniAppMeta> {
    let mut meta: MiniAppMeta = serde_json::from_str(app.meta_json)?;
    meta.id = app.id.to_string();
    meta.created_at = preserved_created_at.unwrap_or(now);
    meta.updated_at = now;
    Ok(meta)
}

pub fn resolve_builtin_seed_check(
    app: &BuiltinMiniAppBundle,
    installed: Option<&BuiltinInstallMarker>,
) -> BuiltinSeedCheck {
    let artifacts = build_builtin_seed_artifacts(app);
    if should_seed_builtin_app(app, &artifacts.content_hash, installed) {
        BuiltinSeedCheck::NeedsSeed(artifacts)
    } else {
        BuiltinSeedCheck::Skip
    }
}

pub fn resolve_builtin_seed_action(
    artifacts: BuiltinSeedArtifacts,
    has_local_override: bool,
) -> BuiltinSeedAction {
    if has_local_override {
        BuiltinSeedAction::PreserveLocalOverride(artifacts)
    } else {
        BuiltinSeedAction::SeedBundle(artifacts)
    }
}

pub fn serialize_builtin_install_marker(
    marker: &BuiltinInstallMarker,
) -> serde_json::Result<String> {
    serde_json::to_string_pretty(marker)
}

pub fn parse_builtin_install_marker(content: &str) -> serde_json::Result<BuiltinInstallMarker> {
    serde_json::from_str(content)
}

pub fn should_seed_builtin_app(
    app: &BuiltinMiniAppBundle,
    content_hash: &str,
    installed: Option<&BuiltinInstallMarker>,
) -> bool {
    !matches!(
        installed,
        Some(marker) if marker.version >= app.version && marker.hash == content_hash
    )
}

pub fn build_builtin_package_json(app_id: &str) -> serde_json::Value {
    build_package_json(app_id, &[])
}

pub fn builtin_source_files(app: &BuiltinMiniAppBundle) -> [(&'static str, &'static str); 5] {
    [
        (INDEX_HTML, app.html),
        (STYLE_CSS, app.css),
        (UI_JS, app.ui_js),
        (WORKER_JS, app.worker_js),
        (ESM_DEPS_JSON, app.esm_dependencies_json),
    ]
}

fn hash_builtin_asset(hasher: &mut Sha256, name: &str, content: &str) {
    hasher.update(name.as_bytes());
    hasher.update([0u8]);
    hasher.update(content.len().to_le_bytes());
    hasher.update([0u8]);
    hasher.update(content.as_bytes());
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
