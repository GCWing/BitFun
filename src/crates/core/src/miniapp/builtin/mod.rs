//! Built-in MiniApps — bundled, seeded into miniapps_dir on first launch / upgrade.
//!
//! Each built-in app has a fixed id (so it can be located across runs). On startup
//! we compare `.builtin-manifest.json` with the bundled asset hash and only rewrite
//! source files when newer code is available.
//! The user's `storage.json` is preserved across upgrades.

use crate::miniapp::manager::MiniAppManager;
use crate::miniapp::types::MiniAppMeta;
use crate::util::errors::{BitFunError, BitFunResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

const BUILTIN_MARKER: &str = ".builtin-manifest.json";
const LEGACY_BUILTIN_VERSION_MARKER: &str = ".builtin-version";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct BuiltinInstallMarker {
    version: u32,
    hash: String,
}

/// A built-in MiniApp bundled with the application binary.
#[derive(Clone, Copy)]
pub struct BuiltinApp {
    /// Stable id used as on-disk directory name (also exposed in the gallery).
    pub id: &'static str,
    /// Schema version for migration-sensitive changes. Asset changes are detected by hash.
    pub version: u32,
    pub meta_json: &'static str,
    pub html: &'static str,
    pub css: &'static str,
    pub ui_js: &'static str,
    pub worker_js: &'static str,
    pub esm_dependencies_json: &'static str,
}

/// All built-in apps that ship with BitFun.
pub const BUILTIN_APPS: &[BuiltinApp] = &[
    BuiltinApp {
        id: "builtin-gomoku",
        version: 11,
        meta_json: include_str!("assets/gomoku/meta.json"),
        html: include_str!("assets/gomoku/index.html"),
        css: include_str!("assets/gomoku/style.css"),
        ui_js: include_str!("assets/gomoku/ui.js"),
        worker_js: include_str!("assets/gomoku/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinApp {
        id: "builtin-daily-divination",
        version: 21,
        meta_json: include_str!("assets/divination/meta.json"),
        html: include_str!("assets/divination/index.html"),
        css: include_str!("assets/divination/style.css"),
        ui_js: include_str!("assets/divination/ui.js"),
        worker_js: include_str!("assets/divination/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinApp {
        id: "builtin-regex-playground",
        version: 16,
        meta_json: include_str!("assets/regex-playground/meta.json"),
        html: include_str!("assets/regex-playground/index.html"),
        css: include_str!("assets/regex-playground/style.css"),
        ui_js: include_str!("assets/regex-playground/ui.js"),
        worker_js: include_str!("assets/regex-playground/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinApp {
        id: "builtin-coding-selfie",
        version: 28,
        meta_json: include_str!("assets/coding-selfie/meta.json"),
        html: include_str!("assets/coding-selfie/index.html"),
        css: include_str!("assets/coding-selfie/style.css"),
        ui_js: include_str!("assets/coding-selfie/ui.js"),
        worker_js: include_str!("assets/coding-selfie/worker.js"),
        esm_dependencies_json: "[]",
    },
    BuiltinApp {
        id: "builtin-pr-review",
        version: 3,
        meta_json: include_str!("assets/pr-review/meta.json"),
        html: include_str!("assets/pr-review/index.html"),
        css: include_str!("assets/pr-review/style.css"),
        ui_js: include_str!("assets/pr-review/ui.js"),
        worker_js: include_str!("assets/pr-review/worker.js"),
        esm_dependencies_json: "[]",
    },
];

/// Seed all built-in MiniApps into the user data directory. Idempotent: skips apps
/// whose on-disk marker hash matches the bundled content. User's `storage.json`
/// is preserved across reseeds; source files & meta.json (without timestamps) are
/// overwritten.
pub async fn seed_builtin_miniapps(manager: &Arc<MiniAppManager>) -> BitFunResult<()> {
    for app in BUILTIN_APPS {
        if let Err(e) = seed_one(manager, app).await {
            log::warn!("seed builtin miniapp '{}' failed: {}", app.id, e);
        }
    }
    Ok(())
}

async fn seed_one(manager: &Arc<MiniAppManager>, app: &BuiltinApp) -> BitFunResult<()> {
    let app_dir = manager.path_manager().miniapp_dir(app.id);
    let marker_path = app_dir.join(BUILTIN_MARKER);
    let content_hash = builtin_content_hash(app);

    if let Some(marker) = read_builtin_install_marker(&marker_path).await? {
        if !should_seed_builtin_app_with_hash(app, &content_hash, Some(&marker)) {
            return Ok(());
        }
    }

    let now = Utc::now().timestamp_millis();
    match manager.load_customization_metadata(app.id).await {
        Ok(Some(metadata)) if metadata.local_override => {
            manager
                .mark_builtin_update_available(app.id, app.version, now)
                .await?;
            let marker = BuiltinInstallMarker {
                version: app.version,
                hash: content_hash,
            };
            write_builtin_install_marker(&marker_path, &marker).await?;
            write_file(
                app_dir.join(LEGACY_BUILTIN_VERSION_MARKER),
                &app.version.to_string(),
            )
            .await?;
            log::info!(
                "preserved customized builtin miniapp '{}' and recorded bundled update v{}",
                app.id,
                app.version
            );
            return Ok(());
        }
        Ok(_) => {}
        Err(e) => {
            log::warn!(
                "read customization metadata for builtin miniapp '{}' failed: {}",
                app.id,
                e
            );
        }
    }

    let source_dir = app_dir.join("source");
    tokio::fs::create_dir_all(&source_dir)
        .await
        .map_err(|e| BitFunError::io(format!("create dir failed: {}", e)))?;

    // meta.json — parse bundled meta, then set id/timestamps. Preserve created_at if present.
    let mut meta: MiniAppMeta = serde_json::from_str(app.meta_json)
        .map_err(|e| BitFunError::parse(format!("invalid bundled meta.json: {}", e)))?;
    meta.id = app.id.to_string();

    let meta_path = app_dir.join("meta.json");
    let preserved_created_at = match tokio::fs::read_to_string(&meta_path).await {
        Ok(existing) => serde_json::from_str::<MiniAppMeta>(&existing)
            .ok()
            .map(|m| m.created_at)
            .unwrap_or(now),
        Err(_) => now,
    };
    meta.created_at = preserved_created_at;
    meta.updated_at = now;

    let meta_json = serde_json::to_string_pretty(&meta).map_err(BitFunError::from)?;
    tokio::fs::write(&meta_path, meta_json)
        .await
        .map_err(|e| BitFunError::io(format!("write meta.json failed: {}", e)))?;

    // Source files (always overwrite).
    write_file(source_dir.join("index.html"), app.html).await?;
    write_file(source_dir.join("style.css"), app.css).await?;
    write_file(source_dir.join("ui.js"), app.ui_js).await?;
    write_file(source_dir.join("worker.js"), app.worker_js).await?;
    write_file(
        source_dir.join("esm_dependencies.json"),
        app.esm_dependencies_json,
    )
    .await?;

    // package.json — overwrite with empty deps; built-in apps must not require npm install.
    let pkg = serde_json::json!({
        "name": format!("miniapp-{}", app.id),
        "private": true,
        "dependencies": {}
    });
    let pkg_json = serde_json::to_string_pretty(&pkg).map_err(BitFunError::from)?;
    write_file(app_dir.join("package.json"), &pkg_json).await?;

    // Preserve user's storage.json if present, otherwise initialize to "{}".
    let storage_path = app_dir.join("storage.json");
    if !storage_path.exists() {
        write_file(storage_path, "{}").await?;
    }

    // Placeholder compiled.html so storage::load() doesn't fail before recompile.
    write_file(
        app_dir.join("compiled.html"),
        "<!DOCTYPE html><html><body>Loading...</body></html>",
    )
    .await?;

    // Recompile to assemble the final compiled.html with bridge + theme + import map.
    manager.recompile(app.id, "dark", None).await?;

    let marker = BuiltinInstallMarker {
        version: app.version,
        hash: content_hash,
    };
    write_builtin_install_marker(&marker_path, &marker).await?;
    write_file(
        app_dir.join(LEGACY_BUILTIN_VERSION_MARKER),
        &app.version.to_string(),
    )
    .await?;
    log::info!(
        "seeded builtin miniapp '{}' (v{}, {})",
        app.id,
        app.version,
        marker.hash
    );
    Ok(())
}

fn builtin_content_hash(app: &BuiltinApp) -> String {
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
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn hash_builtin_asset(hasher: &mut Sha256, name: &str, content: &str) {
    hasher.update(name.as_bytes());
    hasher.update([0u8]);
    hasher.update(content.len().to_le_bytes());
    hasher.update([0u8]);
    hasher.update(content.as_bytes());
}

#[cfg(test)]
fn should_seed_builtin_app(app: &BuiltinApp, installed: Option<&BuiltinInstallMarker>) -> bool {
    let content_hash = builtin_content_hash(app);
    should_seed_builtin_app_with_hash(app, &content_hash, installed)
}

fn should_seed_builtin_app_with_hash(
    app: &BuiltinApp,
    content_hash: &str,
    installed: Option<&BuiltinInstallMarker>,
) -> bool {
    !matches!(
        installed,
        Some(marker) if marker.version >= app.version && marker.hash == content_hash
    )
}

async fn read_builtin_install_marker(path: &Path) -> BitFunResult<Option<BuiltinInstallMarker>> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(BitFunError::io(format!(
                "read builtin marker {} failed: {}",
                path.display(),
                error
            )));
        }
    };

    match serde_json::from_str::<BuiltinInstallMarker>(&content) {
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

async fn write_builtin_install_marker(
    path: &Path,
    marker: &BuiltinInstallMarker,
) -> BitFunResult<()> {
    let content = serde_json::to_string_pretty(marker).map_err(BitFunError::from)?;
    write_file(path, &content).await
}

async fn write_file<P: AsRef<std::path::Path>>(path: P, content: &str) -> BitFunResult<()> {
    tokio::fs::write(path.as_ref(), content)
        .await
        .map_err(|e| BitFunError::io(format!("write {} failed: {}", path.as_ref().display(), e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::miniapp::customization::{
        MiniAppCustomizationMetadata, MiniAppCustomizationOrigin, MiniAppCustomizationOriginKind,
    };

    fn test_manager() -> Arc<MiniAppManager> {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-builtin-customization-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        Arc::new(MiniAppManager::new(path_manager))
    }

    #[tokio::test]
    async fn builtin_reseed_preserves_local_override_and_records_available_update() {
        let manager = test_manager();
        let builtin = &BUILTIN_APPS[0];
        seed_builtin_miniapps(&manager).await.unwrap();

        let custom_css = "body { background: #f7f7f7; }";
        let app_dir = manager.path_manager().miniapp_dir(builtin.id);
        tokio::fs::write(app_dir.join("source").join("style.css"), custom_css)
            .await
            .unwrap();
        manager
            .save_customization_metadata(
                builtin.id,
                &MiniAppCustomizationMetadata {
                    origin: MiniAppCustomizationOrigin {
                        kind: MiniAppCustomizationOriginKind::Builtin,
                        builtin_id: Some(builtin.id.to_string()),
                        builtin_version: Some(builtin.version),
                    },
                    local_override: true,
                    last_applied_draft_id: Some("draft-1".to_string()),
                    available_builtin_update: None,
                    updated_at: Utc::now().timestamp_millis(),
                },
            )
            .await
            .unwrap();
        tokio::fs::write(app_dir.join(BUILTIN_MARKER), "0")
            .await
            .unwrap();

        seed_builtin_miniapps(&manager).await.unwrap();

        let css = tokio::fs::read_to_string(app_dir.join("source").join("style.css"))
            .await
            .unwrap();
        assert_eq!(css, custom_css);

        let metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        assert!(metadata.local_override);
        assert_eq!(
            metadata
                .available_builtin_update
                .map(|update| update.builtin_version),
            Some(builtin.version)
        );
    }

    #[test]
    fn bundled_pr_review_app_is_seeded_as_a_builtin_miniapp() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        let meta: serde_json::Value = serde_json::from_str(app.meta_json).unwrap();
        assert_eq!(meta["permissions"]["node"]["enabled"], false);
        assert_eq!(meta["permissions"]["notifications"]["system"], true);
        assert!(meta["permissions"]["fs"]["read"]
            .as_array()
            .is_some_and(|read| read
                .iter()
                .any(|value| value.as_str() == Some("{workspace}"))));
        assert!(meta["permissions"]["shell"]["allow"]
            .as_array()
            .is_some_and(|allow| allow.iter().any(|value| value.as_str() == Some("gh"))));
        assert!(meta["permissions"]["shell"]["allow"]
            .as_array()
            .is_some_and(|allow| allow.iter().any(|value| value.as_str() == Some("git"))));
        assert!(meta["i18n"]["locales"].get("en-US").is_some());
        assert!(meta["i18n"]["locales"].get("zh-CN").is_some());
        assert!(meta["i18n"]["locales"].get("zh-TW").is_some());
    }

    #[test]
    fn bundled_pr_review_app_exposes_a_guided_review_workspace() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("queueModeAll"));
        assert!(app.ui_js.contains("queueModeMine"));
        assert!(app.ui_js.contains("data-action=\"start-review\""));
        assert!(app.ui_js.contains("data-action=\"delete-subscription\""));
        assert!(app.ui_js.contains("subscription.enabled !== false"));
        assert!(app.ui_js.contains("data-action=\"toggle-subscription\""));
        assert!(app.ui_js.contains("normalizeSubscription"));
        assert!(app.ui_js.contains("activeSubscriptions"));
        assert!(app.ui_js.contains("refreshQueueOnOpen"));
        assert!(app.ui_js.contains("formatDate(item.updatedAt"));
        assert!(app.ui_js.contains("pr-queue-actor"));
        assert!(app.ui_js.contains("modeLabel(mode)"));
        assert!(app.ui_js.contains("progressPct"));
        assert!(app.ui_js.contains("openSelectedPrExternal"));
        assert!(app.ui_js.contains("data-action=\"sync-current\""));
        assert!(app.ui_js.contains("renderComposerStatus"));
        assert!(app.ui_js.contains("data-action=\"delete-operation\""));
        assert!(app.ui_js.contains("data-action=\"jump-file-target\""));
        assert!(app.ui_js.contains("compactPath"));
        assert!(app.css.contains("pr-file-link"));
        assert!(!app.ui_js.contains("Please double-check this change"));
        assert!(app.ui_js.contains("data-action=\"delete-provider\""));
        assert!(app.ui_js.contains("manualComment"));
        assert!(app.ui_js.contains("renderFilesExplorer"));
        assert!(app.ui_js.contains("authorizeGitHubCli"));
        assert!(app.ui_js.contains("ensureProfileToken"));
        assert!(app.ui_js.contains("persistableState"));
        assert!(app.ui_js.contains("data-action=\"cancel-review\""));
        assert!(app.ui_js.contains("parseRepositoryRef"));
        assert!(app.ui_js.contains("discoverWorkspaceRepositories"));
        assert!(app.ui_js.contains("applyWorkspaceDiscoveredRepositories"));
        assert!(app.ui_js.contains("dismissedWorkspaceRepos"));
        assert!(app.ui_js.contains("MAX_WORKSPACE_SCAN_DIRS"));
        assert!(app.ui_js.contains("renderHighlightedDiff"));
        assert!(app.ui_js.contains("reviewProgress"));
        assert!(app.ui_js.contains("pr-repo-first"));
        assert!(app.css.contains("pr-command-bar"));
        assert!(app.css.contains("pr-review-workspace"));
        assert!(app.css.contains("--bitfun-bg"));
        assert!(app.css.contains("data-theme-type=\"light\""));
        assert!(app.css.contains("pr-url-card"));
        assert!(app.css.contains("background-size: 240% 240%"));
        assert!(app.css.contains("pr-btn--compact"));
        assert!(app.css.contains("pr-listen-switch"));
        assert!(app.css.contains("pr-token-details"));
        assert!(app.css.contains("pr-text-btn"));
        assert!(!app
            .ui_js
            .contains("value=\"${esc(state.volatile.sessionTokens"));
        assert!(!app.css.contains("background: #0f1114"));
        assert!(!app.css.contains("background: rgba(23, 25, 28"));
    }

    #[test]
    fn bundled_pr_review_file_switch_preserves_detail_scroll() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("function readReviewWorkspaceScroll"));
        assert!(app.ui_js.contains("function restoreReviewWorkspaceScroll"));
        assert!(app.ui_js.contains("function render(options = {})"));
        assert!(app.ui_js.contains("options.preserveReviewWorkspaceScroll"));
        assert!(app
            .ui_js
            .contains("render({ preserveReviewWorkspaceScroll: true })"));
    }

    #[test]
    fn bundled_pr_review_keeps_review_output_actionable_and_detail_compact() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("function buildReviewOperations"));
        assert!(app
            .ui_js
            .contains("Do not create a general summary comment"));
        assert!(app.ui_js.contains("summaryComment only when"));
        assert!(!app
            .ui_js
            .contains("id: `summary-${snapshot.headSha || Date.now()}`"));
        assert!(app.ui_js.contains("function renderDraftStateChip"));
        assert!(app.ui_js.contains("pr-overview-fold"));
        assert!(app.css.contains(".pr-chip.is-draft"));
        assert!(app.css.contains(".pr-chip.is-ready"));
        assert!(app.css.contains("max-height: min(320px, 45vh)"));
    }

    #[test]
    fn builtin_app_content_hash_changes_when_assets_change() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        let changed = super::BuiltinApp {
            ui_js: "changed ui",
            ..*app
        };

        assert_ne!(builtin_content_hash(app), builtin_content_hash(&changed));
    }

    #[test]
    fn builtin_seed_decision_uses_content_hash_before_version_marker() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");
        let current_marker = BuiltinInstallMarker {
            version: app.version,
            hash: builtin_content_hash(app),
        };
        let stale_hash_marker = BuiltinInstallMarker {
            version: app.version,
            hash: "sha256:stale".to_string(),
        };
        let older_version_marker = BuiltinInstallMarker {
            version: app.version.saturating_sub(1),
            hash: builtin_content_hash(app),
        };

        assert!(!should_seed_builtin_app(app, Some(&current_marker)));
        assert!(should_seed_builtin_app(app, Some(&stale_hash_marker)));
        assert!(should_seed_builtin_app(app, Some(&older_version_marker)));
        assert!(should_seed_builtin_app(app, None));
    }
}
