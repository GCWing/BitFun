//! Live App storage — persist and load under user data dir (V2: ui.js, worker.js, package.json).

use crate::live_app::types::{LiveApp, LiveAppMeta, LiveAppSource, NpmDep};
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json;
use std::path::PathBuf;
use std::sync::Arc;

const META_JSON: &str = "meta.json";
const SOURCE_DIR: &str = "source";
const INDEX_HTML: &str = "index.html";
const STYLE_CSS: &str = "style.css";
const UI_JS: &str = "ui.js";
const WORKER_JS: &str = "worker.js";
const PACKAGE_JSON: &str = "package.json";
const ESM_DEPS_JSON: &str = "esm_dependencies.json";
const COMPILED_HTML: &str = "compiled.html";
const STORAGE_JSON: &str = "storage.json";
const VERSIONS_DIR: &str = "versions";

/// Live App storage service (file-based under `path_manager.live_apps_dir()`).
pub struct LiveAppStorage {
    path_manager: Arc<crate::infrastructure::PathManager>,
}

impl LiveAppStorage {
    pub fn new(path_manager: Arc<crate::infrastructure::PathManager>) -> Self {
        Self { path_manager }
    }

    fn app_dir(&self, app_id: &str) -> PathBuf {
        self.path_manager.live_app_dir(app_id)
    }

    fn meta_path(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join(META_JSON)
    }

    fn source_dir(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join(SOURCE_DIR)
    }

    fn compiled_path(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join(COMPILED_HTML)
    }

    fn storage_path(&self, app_id: &str) -> PathBuf {
        self.app_dir(app_id).join(STORAGE_JSON)
    }

    fn version_path(&self, app_id: &str, version: u32) -> PathBuf {
        self.app_dir(app_id)
            .join(VERSIONS_DIR)
            .join(format!("v{}.json", version))
    }

    /// Ensure app directory and source subdir exist.
    pub async fn ensure_app_dir(&self, app_id: &str) -> BitFunResult<()> {
        let dir = self.app_dir(app_id);
        let source = self.source_dir(app_id);
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create live app dir {}: {}",
                dir.display(),
                e
            ))
        })?;
        tokio::fs::create_dir_all(&source).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create source dir {}: {}",
                source.display(),
                e
            ))
        })?;
        Ok(())
    }

    /// List all app IDs (directories under `live_apps_dir`).
    pub async fn list_app_ids(&self) -> BitFunResult<Vec<String>> {
        let root = self.path_manager.live_apps_dir();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&root)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read liveapps dir: {}", e)))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read liveapps entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with('.') {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        Ok(ids)
    }

    /// Load full Live App by id (meta + source + compiled_html).
    pub async fn load(&self, app_id: &str) -> BitFunResult<LiveApp> {
        let meta_path = self.meta_path(app_id);
        let meta_content = tokio::fs::read_to_string(&meta_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("Live App not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read meta: {}", e))
            }
        })?;
        let meta: LiveAppMeta = serde_json::from_str(&meta_content)
            .map_err(|e| BitFunError::parse(format!("Invalid meta.json: {}", e)))?;

        let source = self.load_source(app_id).await?;
        let compiled_html = self.load_compiled_html(app_id).await?;

        Ok(LiveApp {
            id: meta.id,
            name: meta.name,
            description: meta.description,
            icon: meta.icon,
            category: meta.category,
            tags: meta.tags,
            version: meta.version,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            source,
            compiled_html,
            permissions: meta.permissions,
            ai_context: meta.ai_context,
            runtime: meta.runtime,
        })
    }

    /// Load only metadata (for list views).
    pub async fn load_meta(&self, app_id: &str) -> BitFunResult<LiveAppMeta> {
        let meta_path = self.meta_path(app_id);
        let content = tokio::fs::read_to_string(&meta_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("Live App not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read meta: {}", e))
            }
        })?;
        serde_json::from_str(&content)
            .map_err(|e| BitFunError::parse(format!("Invalid meta.json: {}", e)))
    }

    async fn load_source(&self, app_id: &str) -> BitFunResult<LiveAppSource> {
        let sd = self.source_dir(app_id);
        let html = tokio::fs::read_to_string(sd.join(INDEX_HTML))
            .await
            .unwrap_or_default();
        let css = tokio::fs::read_to_string(sd.join(STYLE_CSS))
            .await
            .unwrap_or_default();
        let ui_js = tokio::fs::read_to_string(sd.join(UI_JS))
            .await
            .unwrap_or_default();
        let worker_js = tokio::fs::read_to_string(sd.join(WORKER_JS))
            .await
            .unwrap_or_default();

        let esm_dependencies = if sd.join(ESM_DEPS_JSON).exists() {
            let c = tokio::fs::read_to_string(sd.join(ESM_DEPS_JSON))
                .await
                .unwrap_or_default();
            serde_json::from_str(&c).unwrap_or_default()
        } else {
            Vec::new()
        };

        let npm_dependencies = self.load_npm_dependencies(app_id).await?;

        Ok(LiveAppSource {
            html,
            css,
            ui_js,
            esm_dependencies,
            worker_js,
            npm_dependencies,
        })
    }

    /// Load only source files and package dependencies from disk.
    pub async fn load_source_only(&self, app_id: &str) -> BitFunResult<LiveAppSource> {
        self.load_source(app_id).await
    }

    async fn load_npm_dependencies(&self, app_id: &str) -> BitFunResult<Vec<NpmDep>> {
        let p = self.app_dir(app_id).join(PACKAGE_JSON);
        if !p.exists() {
            return Ok(Vec::new());
        }
        let c = tokio::fs::read_to_string(&p)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read package.json: {}", e)))?;
        let pkg: serde_json::Value = serde_json::from_str(&c)
            .map_err(|e| BitFunError::parse(format!("Invalid package.json: {}", e)))?;
        let empty = serde_json::Map::new();
        let deps = pkg
            .get("dependencies")
            .and_then(|d| d.as_object())
            .unwrap_or(&empty);
        let npm_dependencies: Vec<NpmDep> = deps
            .iter()
            .map(|(name, v)| NpmDep {
                name: name.clone(),
                version: v.as_str().unwrap_or("*").to_string(),
            })
            .collect();
        Ok(npm_dependencies)
    }

    async fn load_compiled_html(&self, app_id: &str) -> BitFunResult<String> {
        let p = self.compiled_path(app_id);
        tokio::fs::read_to_string(&p).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("Compiled HTML not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read compiled.html: {}", e))
            }
        })
    }

    /// Save full Live App (meta, source files, compiled.html).
    pub async fn save(&self, app: &LiveApp) -> BitFunResult<()> {
        self.ensure_app_dir(&app.id).await?;

        let meta = LiveAppMeta::from(app);
        let meta_path = self.meta_path(&app.id);
        let meta_json = serde_json::to_string_pretty(&meta).map_err(BitFunError::from)?;
        tokio::fs::write(&meta_path, meta_json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write meta: {}", e)))?;

        let sd = self.source_dir(&app.id);
        tokio::fs::write(sd.join(INDEX_HTML), &app.source.html)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write index.html: {}", e)))?;
        tokio::fs::write(sd.join(STYLE_CSS), &app.source.css)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write style.css: {}", e)))?;
        tokio::fs::write(sd.join(UI_JS), &app.source.ui_js)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write ui.js: {}", e)))?;
        tokio::fs::write(sd.join(WORKER_JS), &app.source.worker_js)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write worker.js: {}", e)))?;

        let esm_json = serde_json::to_string_pretty(&app.source.esm_dependencies)
            .map_err(BitFunError::from)?;
        tokio::fs::write(sd.join(ESM_DEPS_JSON), esm_json)
            .await
            .map_err(|e| {
                BitFunError::io(format!("Failed to write esm_dependencies.json: {}", e))
            })?;

        self.write_package_json(&app.id, &app.source.npm_dependencies)
            .await?;

        tokio::fs::write(self.compiled_path(&app.id), &app.compiled_html)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write compiled.html: {}", e)))?;

        Ok(())
    }

    async fn write_package_json(&self, app_id: &str, deps: &[NpmDep]) -> BitFunResult<()> {
        let mut dependencies = serde_json::Map::new();
        for d in deps {
            dependencies.insert(d.name.clone(), serde_json::Value::String(d.version.clone()));
        }
        let pkg = serde_json::json!({
            "name": format!("liveapp-{}", app_id),
            "private": true,
            "dependencies": dependencies
        });
        let p = self.app_dir(app_id).join(PACKAGE_JSON);
        let json = serde_json::to_string_pretty(&pkg).map_err(BitFunError::from)?;
        tokio::fs::write(&p, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write package.json: {}", e)))?;
        Ok(())
    }

    /// Save a version snapshot (for rollback).
    pub async fn save_version(
        &self,
        app_id: &str,
        version: u32,
        app: &LiveApp,
    ) -> BitFunResult<()> {
        let versions_dir = self.app_dir(app_id).join(VERSIONS_DIR);
        tokio::fs::create_dir_all(&versions_dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create versions dir: {}", e)))?;
        let path = self.version_path(app_id, version);
        let json = serde_json::to_string_pretty(app).map_err(BitFunError::from)?;
        tokio::fs::write(&path, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write version file: {}", e)))?;
        Ok(())
    }

    /// Load app storage (KV JSON). Returns empty object if missing.
    pub async fn load_app_storage(&self, app_id: &str) -> BitFunResult<serde_json::Value> {
        let p = self.storage_path(app_id);
        if !p.exists() {
            return Ok(serde_json::json!({}));
        }
        let c = tokio::fs::read_to_string(&p)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read storage: {}", e)))?;
        Ok(serde_json::from_str(&c).unwrap_or_else(|_| serde_json::json!({})))
    }

    /// Save app storage (merge with existing or replace).
    pub async fn save_app_storage(
        &self,
        app_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> BitFunResult<()> {
        self.ensure_app_dir(app_id).await?;
        let mut current = self.load_app_storage(app_id).await?;
        let obj = current
            .as_object_mut()
            .ok_or_else(|| BitFunError::validation("App storage is not an object".to_string()))?;
        obj.insert(key.to_string(), value);
        let p = self.storage_path(app_id);
        let json = serde_json::to_string_pretty(&current).map_err(BitFunError::from)?;
        tokio::fs::write(&p, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write storage: {}", e)))?;
        Ok(())
    }

    /// Delete Live App directory entirely.
    pub async fn delete(&self, app_id: &str) -> BitFunResult<()> {
        let dir = self.app_dir(app_id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| BitFunError::io(format!("Failed to delete live app dir: {}", e)))?;
        }
        Ok(())
    }

    /// List version numbers that have snapshots.
    pub async fn list_versions(&self, app_id: &str) -> BitFunResult<Vec<u32>> {
        let vdir = self.app_dir(app_id).join(VERSIONS_DIR);
        if !vdir.exists() {
            return Ok(Vec::new());
        }
        let mut versions = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&vdir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read versions dir: {}", e)))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read versions entry: {}", e)))?
        {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('v') && name.ends_with(".json") {
                if let Ok(n) = name[1..name.len() - 5].parse::<u32>() {
                    versions.push(n);
                }
            }
        }
        versions.sort();
        Ok(versions)
    }

    /// Load a specific version snapshot.
    pub async fn load_version(&self, app_id: &str, version: u32) -> BitFunResult<LiveApp> {
        let p = self.version_path(app_id, version);
        let c = tokio::fs::read_to_string(&p).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("Version v{} not found", version))
            } else {
                BitFunError::io(format!("Failed to read version: {}", e))
            }
        })?;
        serde_json::from_str(&c)
            .map_err(|e| BitFunError::parse(format!("Invalid version file: {}", e)))
    }
}
