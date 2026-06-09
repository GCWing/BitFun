//! Built-in MiniApps — bundled, seeded into miniapps_dir on first launch / upgrade.
//!
//! Each built-in app has a fixed id (so it can be located across runs). On startup
//! we compare `.builtin-manifest.json` with the bundled asset hash and only rewrite
//! source files when newer code is available.
//! The user's `storage.json` is preserved across upgrades.

use crate::miniapp::manager::MiniAppManager;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_product_domains::miniapp::builtin::{
    build_builtin_package_json, build_builtin_seed_meta, builtin_source_files,
    parse_builtin_install_marker, preserved_builtin_created_at, resolve_builtin_seed_action,
    resolve_builtin_seed_check, serialize_builtin_install_marker, BuiltinInstallMarker,
    BuiltinMiniAppBundle, BuiltinSeedAction, BuiltinSeedCheck, BUILTIN_INSTALL_MARKER,
    BUILTIN_PLACEHOLDER_COMPILED_HTML, LEGACY_BUILTIN_VERSION_MARKER,
};
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;

/// A built-in MiniApp bundled with the application binary.
pub type BuiltinApp = BuiltinMiniAppBundle;

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
    let marker_path = app_dir.join(BUILTIN_INSTALL_MARKER);
    let installed_marker = read_builtin_install_marker(&marker_path).await?;
    let seed_artifacts = match resolve_builtin_seed_check(app, installed_marker.as_ref()) {
        BuiltinSeedCheck::Skip => return Ok(()),
        BuiltinSeedCheck::NeedsSeed(artifacts) => artifacts,
    };

    let now = Utc::now().timestamp_millis();
    let has_local_override = match manager.load_customization_metadata(app.id).await {
        Ok(Some(metadata)) => metadata.local_override,
        Ok(None) => false,
        Err(e) => {
            log::warn!(
                "read customization metadata for builtin miniapp '{}' failed: {}",
                app.id,
                e
            );
            false
        }
    };

    match resolve_builtin_seed_action(seed_artifacts, has_local_override) {
        BuiltinSeedAction::PreserveLocalOverride(artifacts) => {
            let recorded = manager
                .mark_builtin_update_available(app.id, app.version, &artifacts.content_hash, now)
                .await?;
            write_builtin_install_marker(&marker_path, &artifacts.marker).await?;
            write_file(
                app_dir.join(LEGACY_BUILTIN_VERSION_MARKER),
                &artifacts.legacy_version,
            )
            .await?;
            if recorded {
                log::info!(
                    "preserved customized builtin miniapp '{}' and recorded bundled update v{}",
                    app.id,
                    app.version
                );
            } else {
                log::info!(
                    "preserved customized builtin miniapp '{}' and skipped previously declined bundled update v{}",
                    app.id,
                    app.version
                );
            }
            return Ok(());
        }
        BuiltinSeedAction::SeedBundle(artifacts) => {
            seed_builtin_bundle(manager, app, artifacts, now).await
        }
    }
}

async fn seed_builtin_bundle(
    manager: &Arc<MiniAppManager>,
    app: &BuiltinApp,
    artifacts: bitfun_product_domains::miniapp::builtin::BuiltinSeedArtifacts,
    now: i64,
) -> BitFunResult<()> {
    let app_dir = manager.path_manager().miniapp_dir(app.id);
    let source_dir = app_dir.join("source");
    tokio::fs::create_dir_all(&source_dir)
        .await
        .map_err(|e| BitFunError::io(format!("create dir failed: {}", e)))?;

    // meta.json — parse bundled meta, then set id/timestamps. Preserve created_at if present.
    let meta_path = app_dir.join("meta.json");
    let existing_meta_json = tokio::fs::read_to_string(&meta_path).await.ok();
    let meta = build_builtin_seed_meta(
        app,
        preserved_builtin_created_at(existing_meta_json.as_deref()),
        now,
    )
    .map_err(|e| BitFunError::parse(format!("invalid bundled meta.json: {}", e)))?;

    let meta_json = serde_json::to_string_pretty(&meta).map_err(BitFunError::from)?;
    tokio::fs::write(&meta_path, meta_json)
        .await
        .map_err(|e| BitFunError::io(format!("write meta.json failed: {}", e)))?;

    // Source files (always overwrite).
    for (file_name, content) in builtin_source_files(app) {
        write_file(source_dir.join(file_name), content).await?;
    }

    // package.json — overwrite with empty deps; built-in apps must not require npm install.
    let pkg = build_builtin_package_json(app.id);
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
        BUILTIN_PLACEHOLDER_COMPILED_HTML,
    )
    .await?;

    // Recompile to assemble the final compiled.html with bridge + theme + import map.
    manager.recompile(app.id, "dark", None).await?;

    let marker_path = app_dir.join(BUILTIN_INSTALL_MARKER);
    write_builtin_install_marker(&marker_path, &artifacts.marker).await?;
    write_file(
        app_dir.join(LEGACY_BUILTIN_VERSION_MARKER),
        &artifacts.legacy_version,
    )
    .await?;
    log::info!(
        "seeded builtin miniapp '{}' (v{}, {})",
        app.id,
        app.version,
        artifacts.marker.hash
    );
    Ok(())
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

async fn write_builtin_install_marker(
    path: &Path,
    marker: &BuiltinInstallMarker,
) -> BitFunResult<()> {
    let content = serialize_builtin_install_marker(marker).map_err(BitFunError::from)?;
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
    use bitfun_product_domains::miniapp::builtin::{builtin_content_hash, should_seed_builtin_app};
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

    async fn write_outdated_builtin_marker(app_dir: &std::path::Path) {
        write_builtin_install_marker(
            &app_dir.join(BUILTIN_INSTALL_MARKER),
            &BuiltinInstallMarker {
                version: 0,
                hash: "sha256:outdated".to_string(),
            },
        )
        .await
        .unwrap();
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
                    declined_builtin_updates: Vec::new(),
                    updated_at: Utc::now().timestamp_millis(),
                },
            )
            .await
            .unwrap();
        write_outdated_builtin_marker(&app_dir).await;

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
        let update = metadata.available_builtin_update.unwrap();
        assert_eq!(update.builtin_version, builtin.version);
        assert!(!update.source_hash.is_empty());
    }

    #[tokio::test]
    async fn builtin_reseed_skips_declined_update_until_local_override_changes() {
        let manager = test_manager();
        let builtin = &BUILTIN_APPS[0];
        seed_builtin_miniapps(&manager).await.unwrap();

        let custom_css = "body { background: #fafafa; }";
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
                    declined_builtin_updates: Vec::new(),
                    updated_at: Utc::now().timestamp_millis(),
                },
            )
            .await
            .unwrap();

        write_outdated_builtin_marker(&app_dir).await;
        seed_builtin_miniapps(&manager).await.unwrap();
        let first_metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        let first_update = first_metadata.available_builtin_update.unwrap();
        let source_hash = first_update.source_hash.clone();

        manager
            .decline_builtin_update(builtin.id, first_update.builtin_version, &source_hash, 1234)
            .await
            .unwrap();
        write_outdated_builtin_marker(&app_dir).await;
        seed_builtin_miniapps(&manager).await.unwrap();

        let declined_metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        assert!(declined_metadata.available_builtin_update.is_none());
        assert_eq!(declined_metadata.declined_builtin_updates.len(), 1);
        assert_eq!(
            declined_metadata.declined_builtin_updates[0].source_hash,
            source_hash
        );
        let repeated_same_source = manager
            .mark_builtin_update_available(builtin.id, builtin.version + 1, &source_hash, 5678)
            .await
            .unwrap();
        assert!(!repeated_same_source);
        let css = tokio::fs::read_to_string(app_dir.join("source").join("style.css"))
            .await
            .unwrap();
        assert_eq!(css, custom_css);

        tokio::fs::write(
            app_dir.join("source").join("style.css"),
            "body { background: #ffffff; }",
        )
        .await
        .unwrap();
        manager
            .sync_from_fs(builtin.id, "dark", None)
            .await
            .unwrap();
        write_outdated_builtin_marker(&app_dir).await;
        seed_builtin_miniapps(&manager).await.unwrap();

        let updated_metadata = manager
            .load_customization_metadata(builtin.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            updated_metadata
                .available_builtin_update
                .as_ref()
                .map(|update| (update.builtin_version, update.source_hash.as_str())),
            Some((builtin.version, source_hash.as_str()))
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
        assert!(app.ui_js.contains("formatDate(updatedAt"));
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
        assert!(app.ui_js.contains("renderWatchRepositoryCard"));
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
    fn bundled_pr_review_exposes_guarded_github_lifecycle_actions() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("refreshMergeReadiness"));
        assert!(app.ui_js.contains("transitionDraftState"));
        assert!(app.ui_js.contains("mergePullRequest"));
        assert!(app.ui_js.contains("githubGraphql"));
        assert!(app.ui_js.contains("requestLifecycleAction"));
        assert!(app.ui_js.contains("confirmLifecycleAction"));
        assert!(app.ui_js.contains("renderMergeReadinessPanel"));
        assert!(app.ui_js.contains("data-action=\"request-lifecycle\""));
        assert!(app.ui_js.contains("data-action=\"confirm-lifecycle\""));
        assert!(app.ui_js.contains("expectedHeadSha"));
        assert!(app.ui_js.contains("merge_method"));
        assert!(app.ui_js.contains("markPullRequestReadyForReview"));
        assert!(app.ui_js.contains("convertPullRequestToDraft"));
        assert!(app.css.contains("pr-lifecycle-panel"));
        assert!(app.css.contains("pr-btn--merge"));
        assert!(app.css.contains("pr-btn--secondary"));
    }

    #[test]
    fn bundled_pr_review_polishes_queue_notifications_scroll_and_review_focus() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app
            .ui_js
            .contains("function shouldSuppressSystemNotification"));
        assert!(app.ui_js.contains("document.visibilityState"));
        assert!(app.ui_js.contains("function buildNotificationDigest"));
        assert!(app.ui_js.contains("queueOrigin"));
        assert!(app.ui_js.contains("queueOrigin: 'assigned'"));
        assert!(app.ui_js.contains("function sourceAccessError"));
        assert!(app.ui_js.contains("function preferAccessError"));
        assert!(app.ui_js.contains("errorSourceUnavailable"));
        assert!(app.ui_js.contains("statusPartialSync"));
        assert!(app
            .ui_js
            .contains("throw sourceAccessError(identity, error)"));
        assert!(app.ui_js.contains("const sourceErrors = []"));
        assert!(app.ui_js.contains("let refreshedSourceCount = 0"));
        assert!(app
            .ui_js
            .contains("state.ui.status = t('statusPartialSync'"));
        assert!(!app.ui_js.contains("rawMessage"));
        assert!(!app.ui_js.contains("error.body"));
        assert!(app.ui_js.contains("dropMissing: !hadSourceError"));
        assert!(app.ui_js.contains("function readPaneScrolls"));
        assert!(app.ui_js.contains("preservePaneScroll"));
        assert!(app.ui_js.contains("reviewLanguage"));
        assert!(app.ui_js.contains("reviewLanguageZh"));
        assert!(app.ui_js.contains("suggestedFix"));
        assert!(app.ui_js.contains("renderReviewingBanner"));
        assert!(app.ui_js.contains("prDraftPathRow"));
        assert!(app.css.contains("max-height: min(560px, 56vh)"));
        assert!(app.css.contains("pr-btn--danger"));
        assert!(app.css.contains("pr-reviewing-banner"));
        assert!(app.css.contains("pr-draft-path-row"));
    }

    #[test]
    fn bundled_pr_review_keeps_primary_review_surface_minimal() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("function renderSettingsModal"));
        assert!(app.ui_js.contains("settingsOpen"));
        assert!(app.ui_js.contains("data-action=\"open-settings\""));
        assert!(app.ui_js.contains("data-action=\"close-settings\""));
        assert!(app.ui_js.contains("function shouldShowComposer"));
        assert!(app.ui_js.contains("renderComposerPlaceholder"));
        assert!(app.ui_js.contains("pr-files-fold"));
        assert!(app.ui_js.contains("pr-queue-item--compact"));
        assert!(app.css.contains("pr-command-bar--simple"));
        assert!(app.css.contains("pr-settings-modal"));
        assert!(app.css.contains("pr-main-layout--reviewing"));
        assert!(app.css.contains("pr-main-layout--no-composer"));
        assert!(app.css.contains("pr-queue-item--compact"));
    }

    #[test]
    fn bundled_pr_review_keeps_secondary_details_compact_and_actionable() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("function checkStateTone"));
        assert!(app.ui_js.contains("pr-ci-row"));
        assert!(app.ui_js.contains("ciFreshnessHint"));
        assert!(app.ui_js.contains("function lifecycleButtonTitle"));
        assert!(app.ui_js.contains("lifecycleAutoAuthHint"));
        assert!(app.ui_js.contains("ensureProfileToken(profile)"));
        assert!(app.ui_js.contains("decisionBody && !hasInlineFindings"));
        assert!(app.css.contains(".pr-fold summary::before"));
        assert!(app.css.contains(".pr-ci-row"));
        assert!(!app.ui_js.contains("renderLifecycleGuidance(problemKeys)"));
    }

    #[test]
    fn bundled_pr_review_keeps_review_layout_stable_while_generating() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("pr-fold-title"));
        assert!(app.ui_js.contains("pr-fold-meta"));
        assert!(app.ui_js.contains("pr-btn--review"));
        assert!(app.ui_js.contains("render({ preservePaneScroll: true });"));
        assert!(app
            .ui_js
            .contains("finish('statusReady', { preservePaneScroll: true })"));
        assert!(app.css.contains(".pr-fold-title"));
        assert!(app.css.contains(".pr-fold-meta"));
        assert!(app.css.contains(".pr-btn--review"));
        assert!(app.css.contains("flex-wrap: nowrap"));
    }

    #[test]
    fn bundled_pr_review_confirms_draft_replacement_and_reuses_published_context() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("function requestGenerateDraft"));
        assert!(app.ui_js.contains("function unpublishedDraftOperations"));
        assert!(app.ui_js.contains("renderDraftOverwriteConfirm"));
        assert!(app
            .ui_js
            .contains("data-action=\"confirm-overwrite-draft\""));
        assert!(app.ui_js.contains("publishedReviewContext"));
        assert!(app.ui_js.contains("recordPublishedReviewContext"));
        assert!(app.ui_js.contains("previousPublishedFindings"));
        assert!(app
            .ui_js
            .contains("Do not repeat previous published review comments"));
        assert!(app
            .ui_js
            .contains("You may disagree with or refine those earlier comments"));
    }

    #[test]
    fn bundled_pr_review_keeps_sync_jump_and_manual_comment_flows_clear() {
        let app = BUILTIN_APPS
            .iter()
            .find(|app| app.id == "builtin-pr-review")
            .expect("PR Review must be delivered as a built-in MiniApp");

        assert!(app.ui_js.contains("startupSyncing"));
        assert!(app.ui_js.contains("pr-status--busy"));
        assert!(app.ui_js.contains("function shouldShowShellStatus"));
        assert!(app.ui_js.contains("pr-shell--with-status"));
        assert!(app.ui_js.contains("function resetSelectedPrTransientUi"));
        assert!(app.ui_js.contains("state.data.selectedKey !== nextKey"));
        assert!(app.ui_js.contains("statusPublishFailed"));
        assert!(app.ui_js.contains("publishedCount > 0"));
        assert!(app.ui_js.contains("state.ui.focusedDiffPosition = null"));
        assert!(app
            .ui_js
            .contains("shouldShowShellStatus() ? '' : renderStatus()"));
        assert!(app.ui_js.contains("state.ui.filesExpanded = true"));
        assert!(app.ui_js.contains("data-fold=\"files\""));
        assert!(app.ui_js.contains("function directOpenBusyReason"));
        assert!(app.ui_js.contains("function busyActionReason"));
        assert!(app.ui_js.contains("function actionAvailabilityAttrs"));
        assert!(app.ui_js.contains("directOpenBusySync"));
        assert!(app.ui_js.contains("busyActionSync"));
        assert!(app
            .ui_js
            .contains("aria-disabled=\"${disabledReason ? 'true' : 'false'}\""));
        assert!(app.ui_js.contains("data-disabled-reason"));
        assert!(app
            .ui_js
            .contains("const disabledAttrs = busyActionAttrs('open-direct')"));
        assert!(!app.ui_js.contains("state.ui.busy ? 'disabled'"));
        assert!(!app
            .ui_js
            .contains("data-action=\"open-direct\" ${state.ui.busy ? 'disabled' : ''}"));
        assert!(!app.ui_js.contains("data-action=\"mark-reviewed\""));
        assert!(!app.ui_js.contains("markReviewed"));
        assert!(app.ui_js.contains("manualCommentExpanded"));
        assert!(app.ui_js.contains("data-action=\"toggle-manual-comment\""));
        assert!(app
            .ui_js
            .contains("rows=\"${state.ui.manualCommentExpanded ? 15 : 2}\""));
        assert!(app.css.contains(".pr-manual-comment-head"));
        assert!(app.css.contains(".pr-status--busy"));
        assert!(app
            .css
            .contains("grid-template-rows: auto auto minmax(0, 1fr)"));
        assert!(app.css.contains(".pr-shell--with-status .pr-main-layout"));
        assert!(app.css.contains(".pr-btn[aria-disabled=\"true\"]"));
        assert!(app.css.contains(".pr-sync-tile[aria-disabled=\"true\"]"));
        assert!(!app
            .ui_js
            .contains("${renderReviews(snapshot.reviews)}\n        ${renderManualComment()}"));
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
        let content_hash = builtin_content_hash(app);
        let stale_hash_marker = BuiltinInstallMarker {
            version: app.version,
            hash: "sha256:stale".to_string(),
        };
        let older_version_marker = BuiltinInstallMarker {
            version: app.version.saturating_sub(1),
            hash: content_hash.clone(),
        };

        assert!(!should_seed_builtin_app(
            app,
            &content_hash,
            Some(&current_marker)
        ));
        assert!(should_seed_builtin_app(
            app,
            &content_hash,
            Some(&stale_hash_marker)
        ));
        assert!(should_seed_builtin_app(
            app,
            &content_hash,
            Some(&older_version_marker)
        ));
        assert!(should_seed_builtin_app(app, &content_hash, None));
    }
}
