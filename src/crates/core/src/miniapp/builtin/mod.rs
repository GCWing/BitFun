//! Built-in MiniApps — bundled, seeded into miniapps_dir on first launch / upgrade.
//!
//! Each built-in app has a fixed id (so it can be located across runs) and a schema
//! `version`. On startup we compare the on-disk marker file `.builtin-version` with
//! the bundled version and only rewrite source files when newer code is available.
//! The user's `storage.json` is preserved across upgrades.

use crate::miniapp::manager::MiniAppManager;
use crate::miniapp::types::MiniAppMeta;
use crate::util::errors::{BitFunError, BitFunResult};
use chrono::Utc;
use std::sync::Arc;

const BUILTIN_MARKER: &str = ".builtin-version";

/// A built-in MiniApp bundled with the application binary.
pub struct BuiltinApp {
    /// Stable id used as on-disk directory name (also exposed in the gallery).
    pub id: &'static str,
    /// Schema version of the bundled assets — bump when sources change to trigger reseed.
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
];

/// Seed all built-in MiniApps into the user data directory. Idempotent: skips apps
/// whose on-disk marker version is >= the bundled version. User's `storage.json`
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

    // Skip if marker indicates same or newer version is already on disk.
    if let Ok(content) = tokio::fs::read_to_string(&marker_path).await {
        if let Ok(installed) = content.trim().parse::<u32>() {
            if installed >= app.version {
                return Ok(());
            }
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
    let now = Utc::now().timestamp_millis();

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

    write_file(marker_path, &app.version.to_string()).await?;
    log::info!("seeded builtin miniapp '{}' (v{})", app.id, app.version);
    Ok(())
}

async fn write_file<P: AsRef<std::path::Path>>(path: P, content: &str) -> BitFunResult<()> {
    tokio::fs::write(path.as_ref(), content)
        .await
        .map_err(|e| BitFunError::io(format!("write {} failed: {}", path.as_ref().display(), e)))
}
