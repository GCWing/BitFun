//! Built-in MiniApp bundle contracts and pure seed policy.
//!
//! Seed skip requires both a matching content hash and an installed marker version
//! that is at least the bundled version. Do not hardcode bundle version numbers in
//! tests — bumping a MiniApp version should not require shotgun edits across tests.

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
/// how bundles are seeded, compiled, and persisted.
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

/// Built-in MiniApps that ship with the product-domain package.
///
/// The concrete seeding runtime still lives in the app/core integration layer;
/// this list owns only the stable bundle identity and embedded source assets.
pub const BUILTIN_APPS: &[BuiltinMiniAppBundle] = &[
    BuiltinMiniAppBundle {
        id: "builtin-gomoku",
        version: 11,
        meta_json: include_str!("builtin/assets/gomoku/meta.json"),
        html: include_str!("builtin/assets/gomoku/index.html"),
        css: include_str!("builtin/assets/gomoku/style.css"),
        ui_js: include_str!("builtin/assets/gomoku/ui.js"),
        worker_js: include_str!("builtin/assets/gomoku/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinMiniAppBundle {
        id: "builtin-daily-divination",
        version: 21,
        meta_json: include_str!("builtin/assets/divination/meta.json"),
        html: include_str!("builtin/assets/divination/index.html"),
        css: include_str!("builtin/assets/divination/style.css"),
        ui_js: include_str!("builtin/assets/divination/ui.js"),
        worker_js: include_str!("builtin/assets/divination/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinMiniAppBundle {
        id: "builtin-regex-playground",
        version: 16,
        meta_json: include_str!("builtin/assets/regex-playground/meta.json"),
        html: include_str!("builtin/assets/regex-playground/index.html"),
        css: include_str!("builtin/assets/regex-playground/style.css"),
        ui_js: include_str!("builtin/assets/regex-playground/ui.js"),
        worker_js: include_str!("builtin/assets/regex-playground/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinMiniAppBundle {
        id: "builtin-coding-selfie",
        version: 28,
        meta_json: include_str!("builtin/assets/coding-selfie/meta.json"),
        html: include_str!("builtin/assets/coding-selfie/index.html"),
        css: include_str!("builtin/assets/coding-selfie/style.css"),
        ui_js: include_str!("builtin/assets/coding-selfie/ui.js"),
        worker_js: include_str!("builtin/assets/coding-selfie/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinMiniAppBundle {
        id: "builtin-pr-review",
        version: 3,
        meta_json: include_str!("builtin/assets/pr-review/meta.json"),
        html: include_str!("builtin/assets/pr-review/index.html"),
        css: include_str!("builtin/assets/pr-review/style.css"),
        ui_js: include_str!("builtin/assets/pr-review/ui.js"),
        worker_js: include_str!("builtin/assets/pr-review/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinMiniAppBundle {
        id: "builtin-ppt-live",
        version: 167,
        meta_json: include_str!("builtin/assets/ppt-live/meta.json"),
        html: include_str!("builtin/assets/ppt-live/index.html"),
        css: include_str!("builtin/assets/ppt-live/style.css"),
        ui_js: include_str!("builtin/assets/ppt-live/dist/ui.bundle.js"),
        worker_js: include_str!("builtin/assets/ppt-live/worker.js"),
        esm_dependencies_json: include_str!("builtin/assets/ppt-live/esm_dependencies.json"),
    },
];

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

#[cfg(test)]
mod tests {
    // Do not assert hardcoded BUILTIN_APPS[i].version or meta["version"] values here.
    // Version bumps should only touch bundle registration and seed runtime, not tests.

    use super::{builtin_content_hash, BUILTIN_APPS};

    #[test]
    fn builtin_miniapp_bundles_keep_product_domain_asset_owner_contract() {
        let ids = BUILTIN_APPS.iter().map(|app| app.id).collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "builtin-gomoku",
                "builtin-daily-divination",
                "builtin-regex-playground",
                "builtin-coding-selfie",
                "builtin-pr-review",
                "builtin-ppt-live",
            ]
        );

        for app in BUILTIN_APPS {
            assert!(!app.meta_json.trim().is_empty());
            assert!(!app.html.trim().is_empty());
            assert!(!app.css.trim().is_empty());
            assert!(!app.ui_js.trim().is_empty());
            assert!(!app.worker_js.trim().is_empty());
            assert!(builtin_content_hash(app).starts_with("sha256:"));
        }
    }

    #[test]
    fn ppt_live_bundle_uses_bitfun_host_capabilities() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-ppt-live")
            .expect("PPT Live should be registered");
        let meta: serde_json::Value =
            serde_json::from_str(app.meta_json).expect("PPT Live metadata should be valid");
        let bundle: serde_json::Value =
            serde_json::from_str(include_str!("builtin/assets/ppt-live/bundle.json"))
                .expect("PPT Live bundle metadata should be valid");

        assert_eq!(meta["version"].as_u64(), Some(u64::from(app.version)));
        assert_eq!(bundle["version"].as_u64(), Some(u64::from(app.version)));
        assert_eq!(meta["permissions"]["node"]["enabled"], false);
        assert_eq!(meta["permissions"]["ai"]["enabled"], false);
        assert_eq!(meta["permissions"]["agent"]["enabled"], true);
        assert_eq!(meta["permissions"]["agent"]["rate_limit_per_minute"], 120);
        // Research happens inside hidden agent turns (WebSearch/WebFetch via
        // the agent permission); the app itself no longer fetches URLs.
        assert_eq!(
            meta["permissions"]["net"]["allow"].as_array().map(Vec::len),
            Some(0)
        );
        assert!(app.ui_js.contains("Unsupported PPT Live action"));
        // Render, audit, and continuation turns reuse the hidden planning
        // session and its pinned skill-derived project contract. The
        // bundle is minified, so structural checks read the source.
        let adapter_source = include_str!("builtin/assets/ppt-live/src/bitfun-backend-adapter.js");
        assert!(adapter_source.contains("sessionId: options.sessionId"));
        assert!(adapter_source.contains("user::bitfun-system::ppt-design"));
        assert!(adapter_source.contains("references/editable-pptx.md"));
        assert!(adapter_source.contains("references/slide-decks.md"));
        assert!(adapter_source.contains("buildRepairPrompt"));
        assert!(adapter_source.contains("buildAuditPrompt"));
        assert!(adapter_source.contains("AUTOMATIC COMPLETION CONTINUATION"));
        assert!(adapter_source.contains("buildAuditVerificationFeedback"));
        assert!(adapter_source.contains("do not merely explain what remains"));
        assert!(adapter_source.contains("quality-report.json"));
        assert!(!adapter_source.contains("app.ai"));
        assert!(!adapter_source.contains("installFallbackBackend"));
        assert!(app.ui_js.contains("SAME deck Agent Session"));
        assert!(app.ui_js.contains("interrupted before it finished"));
        assert!(app.ui_js.contains("Unknown MiniApp agent session"));
        // Staged generation follows the ppt-design skill's native file
        // protocol: the agent works inside a deck project directory under the
        // app's appdata storage, writes project.json and slides/slide-NN.html,
        // and ui.js reads the files back instead of parsing giant JSON text.
        assert!(adapter_source.contains("protocol: 'files'"));
        assert!(adapter_source.contains("appDataWorkspace: options.appDataWorkspace"));
        assert!(app.ui_js.contains("project.json"));
        assert!(app.ui_js.contains("slides/slide-"));
        let ui_source = include_str!("builtin/assets/ppt-live/ui.js");
        assert!(ui_source.contains("backendUsesFileProtocol"));
        assert!(ui_source.contains("tryReadDeckSlideFile"));
        assert!(meta["permissions"]["fs"]["read"]
            .as_array()
            .is_some_and(|scopes| scopes.iter().any(|scope| scope == "{appdata}")));
        assert!(meta["permissions"]["fs"]["write"]
            .as_array()
            .is_some_and(|scopes| scopes.iter().any(|scope| scope == "{appdata}")));
        assert!(!app.ui_js.contains("Sparo"));
        assert!(
            include_str!("builtin/assets/ppt-live/ui.js").contains("installBitFunBackendAdapter")
        );
        assert!(!meta["permissions"]["ai"]["enabled"]
            .as_bool()
            .unwrap_or(true));
        assert!(adapter_source.contains("data-information-visualization.md"));
        assert!(adapter_source.contains("references/content-guidelines.md"));
        assert!(adapter_source.contains("comparisons -> tables/matrices"));
        // The standalone fallback render prompt (used when the planning
        // session is lost) must reload the design skill itself.
        assert!(app.ui_js.contains("user::bitfun-system::ppt-design"));
        let ppt_live_source = include_str!("builtin/assets/ppt-live/ui.js");
        assert!(ppt_live_source.contains("for (const slidePlan of renderPlans)"));
        assert!(ppt_live_source.contains("validateSlideForPptxGeneration"));
        assert!(ppt_live_source.contains("planningEvidenceIssues"));
        assert!(ppt_live_source.contains("runFinalDeckAudit"));
        assert!(ppt_live_source.contains("completedById"));
        assert!(ppt_live_source.contains("quality-report.json"));
        assert!(ppt_live_source.contains("PPT_BACKEND_CONTINUATION_MAX_ATTEMPTS"));
        assert!(ppt_live_source.contains("completionRecoveryInput"));
        assert!(ppt_live_source.contains("inspectDeckJsonFile"));
        assert!(ppt_live_source.contains("auditWriteEvidenceIssues"));
        assert!(ppt_live_source.contains("recoveryExhaustedError"));
        assert!(!ppt_live_source.contains("PPT_PARALLEL_SLIDE_WORKERS"));
        assert!(!ppt_live_source.contains("runWithConcurrencyLimit"));
        assert!(!ppt_live_source.contains("enrichSources(state)"));
        assert!(app.html.contains("exportPptx"));
        assert!(!app.html.contains("src=\"./ui.js\""));
        assert!(!app.html.contains("href=\"./style.css\""));
        assert!(app.css.contains("--bitfun-bg"));
    }
}
