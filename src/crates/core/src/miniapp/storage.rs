//! MiniApp storage — persist and load MiniApp data under user data dir (V2: ui.js, worker.js, package.json).

use crate::miniapp::types::{MiniApp, MiniAppMeta, MiniAppSource, NpmDep};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_product_domains::miniapp::customization::MiniAppCustomizationMetadata;
use bitfun_product_domains::miniapp::ports::{
    MiniAppPortError, MiniAppPortErrorKind, MiniAppPortFuture, MiniAppStoragePort,
};
use bitfun_product_domains::miniapp::storage::{
    build_package_json, parse_npm_dependencies, MiniAppStorageLayout, COMPILED_HTML,
    DRAFTS_CLEANUP_MARKER, DRAFTS_CLEANUP_PREFIX, DRAFTS_DIR, DRAFT_JSON, ESM_DEPS_JSON,
    INDEX_HTML, META_JSON, PACKAGE_JSON, STORAGE_JSON, STYLE_CSS, UI_JS, WORKER_JS,
};
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// MiniApp storage service (file-based under path_manager.miniapps_dir).
pub struct MiniAppStorage {
    path_manager: Arc<crate::infrastructure::PathManager>,
}

impl MiniAppStorage {
    pub fn new(path_manager: Arc<crate::infrastructure::PathManager>) -> Self {
        Self { path_manager }
    }

    fn layout(&self, app_id: &str) -> MiniAppStorageLayout {
        MiniAppStorageLayout::new(self.path_manager.miniapps_dir(), app_id)
    }

    fn app_dir(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).app_dir()
    }

    fn meta_path(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).meta_path()
    }

    fn source_dir(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).source_dir()
    }

    fn compiled_path(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).compiled_path()
    }

    fn storage_path(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).storage_path()
    }

    fn version_path(&self, app_id: &str, version: u32) -> PathBuf {
        self.layout(app_id).version_path(version)
    }

    pub fn drafts_root(&self) -> PathBuf {
        MiniAppStorageLayout::drafts_root(self.path_manager.miniapps_dir())
    }

    pub fn app_drafts_dir(&self, app_id: &str) -> PathBuf {
        MiniAppStorageLayout::app_drafts_dir(self.path_manager.miniapps_dir(), app_id)
    }

    pub fn draft_dir(&self, app_id: &str, draft_id: &str) -> PathBuf {
        MiniAppStorageLayout::draft_dir(self.path_manager.miniapps_dir(), app_id, draft_id)
    }

    fn cleanup_drafts_root(&self) -> PathBuf {
        MiniAppStorageLayout::cleanup_drafts_root(
            self.path_manager.miniapps_dir(),
            &uuid::Uuid::new_v4().to_string(),
        )
    }

    fn cleanup_marker_path(&self, drafts_root: &Path) -> PathBuf {
        drafts_root.join(DRAFTS_CLEANUP_MARKER)
    }

    fn draft_not_found(app_id: &str, draft_id: &str) -> BitFunError {
        BitFunError::NotFound(format!("MiniApp draft not found: {}/{}", app_id, draft_id))
    }

    fn ensure_active_drafts_root_readable(&self, app_id: &str, draft_id: &str) -> BitFunResult<()> {
        if self.cleanup_marker_path(&self.drafts_root()).exists() {
            return Err(Self::draft_not_found(app_id, draft_id));
        }
        Ok(())
    }

    fn draft_source_dir(&self, app_id: &str, draft_id: &str) -> PathBuf {
        MiniAppStorageLayout::draft_source_dir(self.path_manager.miniapps_dir(), app_id, draft_id)
    }

    fn customization_path(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).customization_path()
    }

    /// Ensure app directory and source subdir exist.
    pub async fn ensure_app_dir(&self, app_id: &str) -> BitFunResult<()> {
        let dir = self.app_dir(app_id);
        let source = self.source_dir(app_id);
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create miniapp dir {}: {}",
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

    /// List all app IDs (directories under miniapps_dir).
    pub async fn list_app_ids(&self) -> BitFunResult<Vec<String>> {
        let root = self.path_manager.miniapps_dir();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&root)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read miniapps dir: {}", e)))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read miniapps entry: {}", e)))?
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

    /// Load full MiniApp by id (meta + source + compiled_html).
    pub async fn load(&self, app_id: &str) -> BitFunResult<MiniApp> {
        let meta_path = self.meta_path(app_id);
        let meta_content = tokio::fs::read_to_string(&meta_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("MiniApp not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read meta: {}", e))
            }
        })?;
        let meta: MiniAppMeta = serde_json::from_str(&meta_content)
            .map_err(|e| BitFunError::parse(format!("Invalid meta.json: {}", e)))?;

        let source = self.load_source(app_id).await?;
        let compiled_html = self.load_compiled_html(app_id).await?;

        Ok(MiniApp {
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
            i18n: meta.i18n,
        })
    }

    /// Load only metadata (for list views).
    pub async fn load_meta(&self, app_id: &str) -> BitFunResult<MiniAppMeta> {
        let meta_path = self.meta_path(app_id);
        let content = tokio::fs::read_to_string(&meta_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("MiniApp not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read meta: {}", e))
            }
        })?;
        serde_json::from_str(&content)
            .map_err(|e| BitFunError::parse(format!("Invalid meta.json: {}", e)))
    }

    async fn load_source(&self, app_id: &str) -> BitFunResult<MiniAppSource> {
        self.load_source_from_dirs(self.source_dir(app_id), self.app_dir(app_id))
            .await
    }

    async fn load_source_from_dirs(
        &self,
        source_dir: PathBuf,
        package_dir: PathBuf,
    ) -> BitFunResult<MiniAppSource> {
        let sd = source_dir;
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

        let npm_dependencies = self
            .load_npm_dependencies_from_package(package_dir.join(PACKAGE_JSON))
            .await?;

        Ok(MiniAppSource {
            html,
            css,
            ui_js,
            esm_dependencies,
            worker_js,
            npm_dependencies,
        })
    }

    /// Load only source files and package dependencies from disk.
    pub async fn load_source_only(&self, app_id: &str) -> BitFunResult<MiniAppSource> {
        self.load_source(app_id).await
    }

    async fn load_npm_dependencies_from_package(&self, p: PathBuf) -> BitFunResult<Vec<NpmDep>> {
        if !p.exists() {
            return Ok(Vec::new());
        }
        let c = tokio::fs::read_to_string(&p)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read package.json: {}", e)))?;
        parse_npm_dependencies(&c)
            .map_err(|e| BitFunError::parse(format!("Invalid package.json: {}", e)))
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

    /// Save full MiniApp (meta, source files, compiled.html).
    pub async fn save(&self, app: &MiniApp) -> BitFunResult<()> {
        self.save_app_files(&self.app_dir(&app.id), &self.source_dir(&app.id), app)
            .await
    }

    async fn save_app_files(
        &self,
        app_dir: &std::path::Path,
        source_dir: &std::path::Path,
        app: &MiniApp,
    ) -> BitFunResult<()> {
        tokio::fs::create_dir_all(app_dir).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create miniapp dir {}: {}",
                app_dir.display(),
                e
            ))
        })?;
        tokio::fs::create_dir_all(source_dir).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create source dir {}: {}",
                source_dir.display(),
                e
            ))
        })?;
        let meta = MiniAppMeta::from(app);
        let meta_path = app_dir.join(META_JSON);
        let meta_json = serde_json::to_string_pretty(&meta).map_err(BitFunError::from)?;
        tokio::fs::write(&meta_path, meta_json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write meta: {}", e)))?;

        let sd = source_dir;
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

        self.write_package_json_to_dir(app_dir, &app.id, &app.source.npm_dependencies)
            .await?;

        let storage_path = app_dir.join(STORAGE_JSON);
        if !storage_path.exists() {
            tokio::fs::write(&storage_path, "{}")
                .await
                .map_err(|e| BitFunError::io(format!("Failed to write storage.json: {}", e)))?;
        }

        tokio::fs::write(app_dir.join(COMPILED_HTML), &app.compiled_html)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write compiled.html: {}", e)))?;

        Ok(())
    }

    async fn write_package_json_to_dir(
        &self,
        app_dir: &std::path::Path,
        app_id: &str,
        deps: &[NpmDep],
    ) -> BitFunResult<()> {
        let pkg = build_package_json(app_id, deps);
        let json = serde_json::to_string_pretty(&pkg).map_err(BitFunError::from)?;
        tokio::fs::write(app_dir.join(PACKAGE_JSON), json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write package.json: {}", e)))?;
        Ok(())
    }

    pub async fn save_draft(
        &self,
        app_id: &str,
        draft_id: &str,
        app: &MiniApp,
        manifest: &serde_json::Value,
    ) -> BitFunResult<()> {
        self.ensure_active_drafts_root_writable().await?;
        let draft_dir = self.draft_dir(app_id, draft_id);
        let source_dir = self.draft_source_dir(app_id, draft_id);
        self.save_app_files(&draft_dir, &source_dir, app).await?;
        let manifest_json = serde_json::to_string_pretty(manifest).map_err(BitFunError::from)?;
        tokio::fs::write(draft_dir.join(DRAFT_JSON), manifest_json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write draft.json: {}", e)))?;
        let storage_path = draft_dir.join(STORAGE_JSON);
        if !storage_path.exists() {
            tokio::fs::write(storage_path, "{}")
                .await
                .map_err(|e| BitFunError::io(format!("Failed to write draft storage: {}", e)))?;
        }
        Ok(())
    }

    pub async fn load_draft_app(&self, app_id: &str, draft_id: &str) -> BitFunResult<MiniApp> {
        self.ensure_active_drafts_root_readable(app_id, draft_id)?;
        let draft_dir = self.draft_dir(app_id, draft_id);
        let meta_content = tokio::fs::read_to_string(draft_dir.join(META_JSON))
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Self::draft_not_found(app_id, draft_id)
                } else {
                    BitFunError::io(format!("Failed to read draft meta: {}", e))
                }
            })?;
        let meta: MiniAppMeta = serde_json::from_str(&meta_content)
            .map_err(|e| BitFunError::parse(format!("Invalid draft meta.json: {}", e)))?;
        let source = self
            .load_source_from_dirs(self.draft_source_dir(app_id, draft_id), draft_dir.clone())
            .await?;
        let compiled_html = tokio::fs::read_to_string(draft_dir.join(COMPILED_HTML))
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BitFunError::NotFound(format!(
                        "MiniApp draft compiled HTML not found: {}/{}",
                        app_id, draft_id
                    ))
                } else {
                    BitFunError::io(format!("Failed to read draft compiled.html: {}", e))
                }
            })?;
        Ok(MiniApp {
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
            i18n: meta.i18n,
        })
    }

    pub async fn load_draft_manifest(
        &self,
        app_id: &str,
        draft_id: &str,
    ) -> BitFunResult<serde_json::Value> {
        self.ensure_active_drafts_root_readable(app_id, draft_id)?;
        let path = self.draft_dir(app_id, draft_id).join(DRAFT_JSON);
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Self::draft_not_found(app_id, draft_id)
            } else {
                BitFunError::io(format!("Failed to read draft.json: {}", e))
            }
        })?;
        serde_json::from_str(&content)
            .map_err(|e| BitFunError::parse(format!("Invalid draft.json: {}", e)))
    }

    pub async fn delete_draft(&self, app_id: &str, draft_id: &str) -> BitFunResult<()> {
        let dir = self.draft_dir(app_id, draft_id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| BitFunError::io(format!("Failed to delete miniapp draft: {}", e)))?;
        }
        Ok(())
    }

    pub async fn mark_stale_drafts_for_cleanup(&self) -> BitFunResult<Vec<PathBuf>> {
        let mut targets = self.collect_marked_drafts_roots().await?;
        if let Some(target) = self.isolate_active_drafts_root().await? {
            targets.push(target);
        }
        targets.sort();
        targets.dedup();
        Ok(targets)
    }

    pub async fn cleanup_marked_drafts(&self, targets: Vec<PathBuf>) -> BitFunResult<()> {
        for target in targets {
            if !self.is_cleanup_safe_drafts_root(&target) {
                continue;
            }
            if !self.cleanup_marker_path(&target).exists() {
                continue;
            }
            if target.exists() {
                tokio::fs::remove_dir_all(&target).await.map_err(|e| {
                    BitFunError::io(format!(
                        "Failed to clean marked miniapp drafts {}: {}",
                        target.display(),
                        e
                    ))
                })?;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        Ok(())
    }

    async fn ensure_active_drafts_root_writable(&self) -> BitFunResult<()> {
        if self.cleanup_marker_path(&self.drafts_root()).exists() {
            let _ = self.isolate_active_drafts_root().await?;
        }
        Ok(())
    }

    async fn collect_marked_drafts_roots(&self) -> BitFunResult<Vec<PathBuf>> {
        let root = self.path_manager.miniapps_dir();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut targets = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&root)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read miniapps dir: {}", e)))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read miniapps entry: {}", e)))?
        {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if name.starts_with(DRAFTS_CLEANUP_PREFIX)
                && path.is_dir()
                && self.cleanup_marker_path(&path).exists()
            {
                targets.push(path);
            }
        }
        Ok(targets)
    }

    async fn isolate_active_drafts_root(&self) -> BitFunResult<Option<PathBuf>> {
        let active = self.drafts_root();
        if !active.exists() {
            return Ok(None);
        }
        self.write_cleanup_marker(&active).await?;
        let target = self.cleanup_drafts_root();
        tokio::fs::rename(&active, &target).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to mark miniapp drafts for cleanup {} -> {}: {}",
                active.display(),
                target.display(),
                e
            ))
        })?;
        Ok(Some(target))
    }

    async fn write_cleanup_marker(&self, drafts_root: &Path) -> BitFunResult<()> {
        tokio::fs::create_dir_all(drafts_root).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create miniapp drafts dir {}: {}",
                drafts_root.display(),
                e
            ))
        })?;
        tokio::fs::write(
            self.cleanup_marker_path(drafts_root),
            "pending miniapp draft cleanup\n",
        )
        .await
        .map_err(|e| BitFunError::io(format!("Failed to mark miniapp drafts: {}", e)))?;
        Ok(())
    }

    fn is_cleanup_safe_drafts_root(&self, path: &Path) -> bool {
        let root = self.path_manager.miniapps_dir();
        if !path.starts_with(&root) {
            return false;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            return false;
        };
        name == DRAFTS_DIR || name.starts_with(DRAFTS_CLEANUP_PREFIX)
    }

    /// Save a version snapshot (for rollback).
    pub async fn save_version(
        &self,
        app_id: &str,
        version: u32,
        app: &MiniApp,
    ) -> BitFunResult<()> {
        let versions_dir = self.layout(app_id).versions_dir();
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

    pub async fn load_draft_storage(
        &self,
        app_id: &str,
        draft_id: &str,
    ) -> BitFunResult<serde_json::Value> {
        self.ensure_active_drafts_root_readable(app_id, draft_id)?;
        let p = self.draft_dir(app_id, draft_id).join(STORAGE_JSON);
        if !p.exists() {
            return Ok(serde_json::json!({}));
        }
        let c = tokio::fs::read_to_string(&p)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read draft storage: {}", e)))?;
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

    pub async fn save_draft_storage(
        &self,
        app_id: &str,
        draft_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> BitFunResult<()> {
        self.ensure_active_drafts_root_writable().await?;
        let dir = self.draft_dir(app_id, draft_id);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create draft dir: {}", e)))?;
        let mut current = self.load_draft_storage(app_id, draft_id).await?;
        let obj = current
            .as_object_mut()
            .ok_or_else(|| BitFunError::validation("Draft storage is not an object".to_string()))?;
        obj.insert(key.to_string(), value);
        let json = serde_json::to_string_pretty(&current).map_err(BitFunError::from)?;
        tokio::fs::write(dir.join(STORAGE_JSON), json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write draft storage: {}", e)))?;
        Ok(())
    }

    pub async fn load_customization_metadata(
        &self,
        app_id: &str,
    ) -> BitFunResult<Option<MiniAppCustomizationMetadata>> {
        let path = self.customization_path(app_id);
        if !path.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            BitFunError::io(format!("Failed to read customization metadata: {}", e))
        })?;
        serde_json::from_str(&content)
            .map(Some)
            .map_err(|e| BitFunError::parse(format!("Invalid customization metadata: {}", e)))
    }

    pub async fn save_customization_metadata(
        &self,
        app_id: &str,
        metadata: &MiniAppCustomizationMetadata,
    ) -> BitFunResult<()> {
        self.ensure_app_dir(app_id).await?;
        let json = serde_json::to_string_pretty(metadata).map_err(BitFunError::from)?;
        tokio::fs::write(self.customization_path(app_id), json)
            .await
            .map_err(|e| {
                BitFunError::io(format!("Failed to write customization metadata: {}", e))
            })?;
        Ok(())
    }

    /// Delete MiniApp directory entirely.
    pub async fn delete(&self, app_id: &str) -> BitFunResult<()> {
        let dir = self.app_dir(app_id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| BitFunError::io(format!("Failed to delete miniapp dir: {}", e)))?;
        }
        let drafts_dir = self.app_drafts_dir(app_id);
        if drafts_dir.exists() {
            tokio::fs::remove_dir_all(&drafts_dir)
                .await
                .map_err(|e| BitFunError::io(format!("Failed to delete miniapp drafts: {}", e)))?;
        }
        Ok(())
    }

    /// List version numbers that have snapshots.
    pub async fn list_versions(&self, app_id: &str) -> BitFunResult<Vec<u32>> {
        let vdir = self.layout(app_id).versions_dir();
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
    pub async fn load_version(&self, app_id: &str, version: u32) -> BitFunResult<MiniApp> {
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

impl MiniAppStoragePort for MiniAppStorage {
    fn list_app_ids(&self) -> MiniAppPortFuture<'_, Vec<String>> {
        Box::pin(async move { self.list_app_ids().await.map_err(map_miniapp_port_error) })
    }

    fn load(&self, app_id: String) -> MiniAppPortFuture<'_, MiniApp> {
        Box::pin(async move { self.load(&app_id).await.map_err(map_miniapp_port_error) })
    }

    fn load_meta(&self, app_id: String) -> MiniAppPortFuture<'_, MiniAppMeta> {
        Box::pin(async move {
            self.load_meta(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn load_source(&self, app_id: String) -> MiniAppPortFuture<'_, MiniAppSource> {
        Box::pin(async move {
            self.load_source_only(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn save(&self, app: MiniApp) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move { self.save(&app).await.map_err(map_miniapp_port_error) })
    }

    fn save_version(
        &self,
        app_id: String,
        version: u32,
        app: MiniApp,
    ) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move {
            self.save_version(&app_id, version, &app)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn load_app_storage(&self, app_id: String) -> MiniAppPortFuture<'_, serde_json::Value> {
        Box::pin(async move {
            self.load_app_storage(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn save_app_storage(
        &self,
        app_id: String,
        key: String,
        value: serde_json::Value,
    ) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move {
            self.save_app_storage(&app_id, &key, value)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn delete(&self, app_id: String) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move { self.delete(&app_id).await.map_err(map_miniapp_port_error) })
    }

    fn list_versions(&self, app_id: String) -> MiniAppPortFuture<'_, Vec<u32>> {
        Box::pin(async move {
            self.list_versions(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn load_version(&self, app_id: String, version: u32) -> MiniAppPortFuture<'_, MiniApp> {
        Box::pin(async move {
            self.load_version(&app_id, version)
                .await
                .map_err(map_miniapp_port_error)
        })
    }
}

fn map_miniapp_port_error(error: BitFunError) -> MiniAppPortError {
    let kind = match &error {
        BitFunError::NotFound(_) => MiniAppPortErrorKind::NotFound,
        BitFunError::Validation(_) | BitFunError::Deserialization(_) => {
            MiniAppPortErrorKind::InvalidInput
        }
        BitFunError::Io(io_error) if io_error.kind() == std::io::ErrorKind::PermissionDenied => {
            MiniAppPortErrorKind::PermissionDenied
        }
        BitFunError::Io(_) => MiniAppPortErrorKind::Io,
        _ => MiniAppPortErrorKind::Backend,
    };
    MiniAppPortError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::miniapp::customization::{
        MiniAppCustomizationMetadata, MiniAppCustomizationOrigin, MiniAppCustomizationOriginKind,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{}-{}", prefix, uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).expect("test root should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[tokio::test]
    async fn storage_port_adapter_preserves_existing_file_lifecycle() {
        let root = TestTempDir::new("bitfun-miniapp-storage-port");
        let path_manager = Arc::new(
            crate::infrastructure::PathManager::with_user_root_for_tests(root.path().to_path_buf()),
        );
        let storage = MiniAppStorage::new(path_manager);
        let port: &dyn MiniAppStoragePort = &storage;
        let app = sample_app("demo_app");

        port.save(app.clone()).await.unwrap();

        let ids = port.list_app_ids().await.unwrap();
        assert_eq!(ids, vec!["demo_app".to_string()]);

        let meta = port.load_meta("demo_app".to_string()).await.unwrap();
        assert_eq!(meta.name, "Demo");

        let source = port.load_source("demo_app".to_string()).await.unwrap();
        assert_eq!(source.ui_js, "console.log('ui');");

        let loaded = port.load("demo_app".to_string()).await.unwrap();
        assert_eq!(loaded.compiled_html, "<html></html>");

        port.save_app_storage(
            "demo_app".to_string(),
            "answer".to_string(),
            serde_json::json!(42),
        )
        .await
        .unwrap();
        let app_storage = port.load_app_storage("demo_app".to_string()).await.unwrap();
        assert_eq!(app_storage["answer"], 42);

        port.save_version("demo_app".to_string(), 1, app)
            .await
            .unwrap();
        assert_eq!(
            port.list_versions("demo_app".to_string()).await.unwrap(),
            vec![1]
        );
        assert_eq!(
            port.load_version("demo_app".to_string(), 1)
                .await
                .unwrap()
                .id,
            "demo_app"
        );

        port.delete("demo_app".to_string()).await.unwrap();
        assert!(port.list_app_ids().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn storage_adapter_uses_product_domain_layout_contract() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-layout-port-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager.clone());
        let app = sample_app("layout_app");
        let layout = MiniAppStorageLayout::new(path_manager.miniapps_dir(), "layout_app");

        storage.save(&app).await.unwrap();
        assert!(layout.storage_path().is_file());
        assert_eq!(
            fs::read_to_string(layout.storage_path()).unwrap(),
            "{}".to_string()
        );
        storage
            .save_app_storage("layout_app", "answer", serde_json::json!(42))
            .await
            .unwrap();
        storage.save_version("layout_app", 7, &app).await.unwrap();

        assert!(layout.app_dir().is_dir());
        assert!(layout.meta_path().is_file());
        assert!(layout.compiled_path().is_file());
        assert!(layout.package_json_path().is_file());
        assert!(layout.source_file_path(INDEX_HTML).is_file());
        assert!(layout.source_file_path(STYLE_CSS).is_file());
        assert!(layout.source_file_path(UI_JS).is_file());
        assert!(layout.source_file_path(WORKER_JS).is_file());
        assert!(layout.source_file_path(ESM_DEPS_JSON).is_file());
        assert!(layout.version_path(7).is_file());
    }

    #[tokio::test]
    async fn saving_app_files_preserves_existing_storage_json() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-storage-preserve-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);
        let app = sample_app("storage_app");

        storage.save(&app).await.unwrap();
        storage
            .save_app_storage("storage_app", "answer", serde_json::json!(42))
            .await
            .unwrap();
        storage.save(&app).await.unwrap();

        assert_eq!(
            storage
                .load_app_storage("storage_app")
                .await
                .unwrap()
                .get("answer"),
            Some(&serde_json::json!(42))
        );
    }

    #[tokio::test]
    async fn draft_storage_is_hidden_and_isolated_from_active_storage() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-draft-storage-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);
        let app = sample_app("demo_app");

        storage.save(&app).await.unwrap();
        storage
            .save_app_storage("demo_app", "answer", serde_json::json!(42))
            .await
            .unwrap();
        storage
            .save_draft_storage("demo_app", "draft_one", "answer", serde_json::json!(7))
            .await
            .unwrap();

        assert_eq!(
            storage
                .load_app_storage("demo_app")
                .await
                .unwrap()
                .get("answer"),
            Some(&serde_json::json!(42))
        );
        assert_eq!(
            storage
                .load_draft_storage("demo_app", "draft_one")
                .await
                .unwrap()
                .get("answer"),
            Some(&serde_json::json!(7))
        );
        assert_eq!(storage.list_app_ids().await.unwrap(), vec!["demo_app"]);

        let draft_dir = storage.app_drafts_dir("demo_app");
        assert!(draft_dir.exists());
        storage.delete("demo_app").await.unwrap();
        assert!(!draft_dir.exists());
    }

    #[tokio::test]
    async fn mark_stale_drafts_moves_sandboxes_off_the_active_read_path() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-stale-drafts-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);
        let app = sample_app("demo_app");

        storage.save(&app).await.unwrap();
        storage
            .save_draft_storage("demo_app", "stale_draft", "answer", serde_json::json!(7))
            .await
            .unwrap();

        assert!(storage.drafts_root().exists());
        let cleanup_targets = storage.mark_stale_drafts_for_cleanup().await.unwrap();

        assert_eq!(cleanup_targets.len(), 1);
        assert!(cleanup_targets[0].exists());
        assert!(storage.cleanup_marker_path(&cleanup_targets[0]).exists());
        assert!(!storage.drafts_root().exists());
        assert!(storage.load("demo_app").await.is_ok());
        assert_eq!(
            storage
                .load_draft_storage("demo_app", "stale_draft")
                .await
                .unwrap(),
            serde_json::json!({})
        );
    }

    #[tokio::test]
    async fn draft_reads_skip_marked_active_root() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-marked-draft-read-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);

        storage
            .save_draft_storage("demo_app", "stale_draft", "answer", serde_json::json!(7))
            .await
            .unwrap();
        storage
            .write_cleanup_marker(&storage.drafts_root())
            .await
            .unwrap();

        let error = storage
            .load_draft_storage("demo_app", "stale_draft")
            .await
            .unwrap_err();
        assert!(matches!(error, BitFunError::NotFound(_)));
    }

    #[tokio::test]
    async fn cleanup_marked_drafts_removes_quarantined_sandboxes_later() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-clean-marked-drafts-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);

        storage
            .save_draft_storage("demo_app", "stale_draft", "answer", serde_json::json!(7))
            .await
            .unwrap();
        let cleanup_targets = storage.mark_stale_drafts_for_cleanup().await.unwrap();
        let cleanup_root = cleanup_targets[0].clone();

        storage
            .cleanup_marked_drafts(cleanup_targets)
            .await
            .unwrap();

        assert!(!cleanup_root.exists());
        assert!(!storage.drafts_root().exists());
    }

    #[tokio::test]
    async fn saving_new_draft_isolates_marked_active_root_first() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-marked-draft-write-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);

        storage
            .save_draft_storage("demo_app", "stale_draft", "answer", serde_json::json!(7))
            .await
            .unwrap();
        storage
            .write_cleanup_marker(&storage.drafts_root())
            .await
            .unwrap();

        storage
            .save_draft_storage("demo_app", "fresh_draft", "answer", serde_json::json!(9))
            .await
            .unwrap();

        assert_eq!(
            storage
                .load_draft_storage("demo_app", "fresh_draft")
                .await
                .unwrap()
                .get("answer"),
            Some(&serde_json::json!(9))
        );
        assert!(!storage.cleanup_marker_path(&storage.drafts_root()).exists());
    }

    #[tokio::test]
    async fn customization_metadata_roundtrips() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-customization-meta-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);
        let app = sample_app("builtin-demo");
        storage.save(&app).await.unwrap();

        let metadata = MiniAppCustomizationMetadata {
            origin: MiniAppCustomizationOrigin {
                kind: MiniAppCustomizationOriginKind::Builtin,
                builtin_id: Some("builtin-demo".to_string()),
                builtin_version: Some(3),
            },
            local_override: true,
            last_applied_draft_id: Some("draft_one".to_string()),
            available_builtin_update: None,
            declined_builtin_updates: Vec::new(),
            updated_at: 123,
        };

        storage
            .save_customization_metadata("builtin-demo", &metadata)
            .await
            .unwrap();

        assert_eq!(
            storage
                .load_customization_metadata("builtin-demo")
                .await
                .unwrap(),
            Some(metadata)
        );
    }

    fn sample_app(id: &str) -> MiniApp {
        MiniApp {
            id: id.to_string(),
            name: "Demo".to_string(),
            description: "Demo app".to_string(),
            icon: "sparkles".to_string(),
            category: "tools".to_string(),
            tags: vec!["demo".to_string()],
            version: 1,
            created_at: 1,
            updated_at: 2,
            source: MiniAppSource {
                html: "<div id=\"app\"></div>".to_string(),
                css: "body {}".to_string(),
                ui_js: "console.log('ui');".to_string(),
                esm_dependencies: Vec::new(),
                worker_js: "export default {};".to_string(),
                npm_dependencies: vec![NpmDep {
                    name: "lodash".to_string(),
                    version: "^4.17.21".to_string(),
                }],
            },
            compiled_html: "<html></html>".to_string(),
            permissions: Default::default(),
            ai_context: None,
            runtime: Default::default(),
            i18n: None,
        }
    }
}
