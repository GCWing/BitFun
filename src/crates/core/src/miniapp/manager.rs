//! MiniApp manager — CRUD, version management, compile on save (V2: no permission guard, policy for Worker).

use crate::miniapp::compiler::compile;
use crate::miniapp::permission_policy::resolve_policy;
use crate::miniapp::storage::MiniAppStorage;
use crate::miniapp::types::{
    MiniApp, MiniAppAiContext, MiniAppMeta, MiniAppPermissions, MiniAppRuntimeState, MiniAppSource,
};
use crate::util::errors::BitFunResult;
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use uuid::Uuid;

static GLOBAL_MINIAPP_MANAGER: OnceLock<Arc<MiniAppManager>> = OnceLock::new();

/// Initialize the global MiniAppManager (called once at startup from Tauri app_state).
pub fn initialize_global_miniapp_manager(manager: Arc<MiniAppManager>) {
    let _ = GLOBAL_MINIAPP_MANAGER.set(manager);
}

/// Get the global MiniAppManager, returning None if not initialized.
pub fn try_get_global_miniapp_manager() -> Option<Arc<MiniAppManager>> {
    GLOBAL_MINIAPP_MANAGER.get().cloned()
}

/// MiniApp manager: create, read, update, delete, list, compile, rollback.
pub struct MiniAppManager {
    storage: MiniAppStorage,
    path_manager: Arc<crate::infrastructure::PathManager>,
    /// User-granted paths per app (for resolve_policy).
    granted_paths: RwLock<HashMap<String, Vec<PathBuf>>>,
}

impl MiniAppManager {
    pub fn new(path_manager: Arc<crate::infrastructure::PathManager>) -> Self {
        let storage = MiniAppStorage::new(path_manager.clone());
        Self {
            storage,
            path_manager,
            granted_paths: RwLock::new(HashMap::new()),
        }
    }

    fn build_source_revision(version: u32, updated_at: i64) -> String {
        format!("src:{version}:{updated_at}")
    }

    fn build_deps_revision(source: &MiniAppSource) -> String {
        let mut deps: Vec<String> = source
            .npm_dependencies
            .iter()
            .map(|dep| format!("{}@{}", dep.name, dep.version))
            .collect();
        deps.sort();
        deps.join("|")
    }

    fn build_runtime_state(
        version: u32,
        updated_at: i64,
        source: &MiniAppSource,
        deps_dirty: bool,
        worker_restart_required: bool,
    ) -> MiniAppRuntimeState {
        MiniAppRuntimeState {
            source_revision: Self::build_source_revision(version, updated_at),
            deps_revision: Self::build_deps_revision(source),
            deps_dirty,
            worker_restart_required,
            ui_recompile_required: false,
        }
    }

    fn ensure_runtime_state(app: &mut MiniApp) -> bool {
        let mut changed = false;
        if app.runtime.source_revision.is_empty() {
            app.runtime.source_revision = Self::build_source_revision(app.version, app.updated_at);
            changed = true;
        }
        let deps_revision = Self::build_deps_revision(&app.source);
        if app.runtime.deps_revision != deps_revision {
            app.runtime.deps_revision = deps_revision;
            changed = true;
        }
        changed
    }

    pub fn build_worker_revision(&self, app: &MiniApp, policy_json: &str) -> String {
        format!(
            "{}::{}::{}",
            app.runtime.source_revision, app.runtime.deps_revision, policy_json
        )
    }

    fn workspace_dir_string(workspace_root: Option<&Path>) -> String {
        workspace_root
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn compile_source(
        &self,
        app_id: &str,
        source: &MiniAppSource,
        permissions: &MiniAppPermissions,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<String> {
        let app_data_dir = self.path_manager.miniapp_dir(app_id);
        let app_data_dir_str = app_data_dir.to_string_lossy().to_string();
        let workspace_dir = Self::workspace_dir_string(workspace_root);

        compile(
            source,
            permissions,
            app_id,
            &app_data_dir_str,
            &workspace_dir,
            theme,
        )
    }

    /// List all MiniApp metadata.
    pub async fn list(&self) -> BitFunResult<Vec<MiniAppMeta>> {
        let ids = self.storage.list_app_ids().await?;
        let mut metas = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(meta) = self.storage.load_meta(&id).await {
                metas.push(meta);
            }
        }
        metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(metas)
    }

    /// Get full MiniApp by id.
    pub async fn get(&self, app_id: &str) -> BitFunResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        if Self::ensure_runtime_state(&mut app) {
            self.storage.save(&app).await?;
        }
        Ok(app)
    }

    /// Create a new MiniApp (generates id, sets created_at/updated_at, compiles).
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        name: String,
        description: String,
        icon: String,
        category: String,
        tags: Vec<String>,
        source: MiniAppSource,
        permissions: MiniAppPermissions,
        ai_context: Option<MiniAppAiContext>,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();

        let compiled_html =
            self.compile_source(&id, &source, &permissions, "dark", workspace_root)?;
        let runtime =
            Self::build_runtime_state(1, now, &source, !source.npm_dependencies.is_empty(), true);

        let app = MiniApp {
            id: id.clone(),
            name,
            description,
            icon,
            category,
            tags,
            version: 1,
            created_at: now,
            updated_at: now,
            source,
            compiled_html,
            permissions,
            ai_context,
            runtime,
        };

        self.storage.save(&app).await?;
        Ok(app)
    }

    /// Update existing MiniApp (increment version, recompile, save).
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        app_id: &str,
        name: Option<String>,
        description: Option<String>,
        icon: Option<String>,
        category: Option<String>,
        tags: Option<Vec<String>>,
        source: Option<MiniAppSource>,
        permissions: Option<MiniAppPermissions>,
        ai_context: Option<MiniAppAiContext>,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        let previous_app = app.clone();
        let source_changed = source.is_some();
        let permissions_changed = permissions.is_some();

        if let Some(n) = name {
            app.name = n;
        }
        if let Some(d) = description {
            app.description = d;
        }
        if let Some(i) = icon {
            app.icon = i;
        }
        if let Some(c) = category {
            app.category = c;
        }
        if let Some(t) = tags {
            app.tags = t;
        }
        if let Some(s) = source {
            app.source = s;
        }
        if let Some(p) = permissions {
            app.permissions = p;
        }
        if let Some(a) = ai_context {
            app.ai_context = Some(a);
        }

        app.version += 1;
        app.updated_at = Utc::now().timestamp_millis();

        app.compiled_html = self.compile_source(
            app_id,
            &app.source,
            &app.permissions,
            "dark",
            workspace_root,
        )?;
        let deps_changed = previous_app.source.npm_dependencies != app.source.npm_dependencies;
        if source_changed || permissions_changed {
            app.runtime.source_revision = Self::build_source_revision(app.version, app.updated_at);
            app.runtime.worker_restart_required = true;
        }
        if deps_changed {
            app.runtime.deps_revision = Self::build_deps_revision(&app.source);
            app.runtime.deps_dirty = !app.source.npm_dependencies.is_empty();
            app.runtime.worker_restart_required = true;
        }
        app.runtime.ui_recompile_required = false;
        Self::ensure_runtime_state(&mut app);

        self.storage
            .save_version(app_id, previous_app.version, &previous_app)
            .await?;
        self.storage.save(&app).await?;
        Ok(app)
    }

    /// Delete MiniApp and its directory.
    pub async fn delete(&self, app_id: &str) -> BitFunResult<()> {
        self.granted_paths.write().await.remove(app_id);
        self.storage.delete(app_id).await
    }

    /// Get the path manager (for external callers that need paths like miniapp_dir).
    pub fn path_manager(&self) -> &Arc<crate::infrastructure::PathManager> {
        &self.path_manager
    }

    /// Resolve permission policy for the given app (for JS Worker startup).
    pub async fn resolve_policy_for_app(
        &self,
        app_id: &str,
        permissions: &MiniAppPermissions,
        workspace_root: Option<&Path>,
    ) -> serde_json::Value {
        let app_data_dir = self.path_manager.miniapp_dir(app_id);
        let gp = self.granted_paths.read().await;
        let granted = gp.get(app_id).map(|v| v.as_slice()).unwrap_or(&[]);
        resolve_policy(permissions, app_id, &app_data_dir, workspace_root, granted)
    }

    /// Grant workspace access for an app (no-op; workspace context is supplied by caller).
    pub async fn grant_workspace(&self, _app_id: &str) {}

    /// Grant path (user-selected) for an app.
    pub async fn grant_path(&self, app_id: &str, path: PathBuf) {
        let mut guard = self.granted_paths.write().await;
        let list = guard.entry(app_id.to_string()).or_default();
        if !list.contains(&path) {
            list.push(path);
        }
    }

    /// Get app storage (KV) value.
    pub async fn get_storage(&self, app_id: &str, key: &str) -> BitFunResult<serde_json::Value> {
        let storage = self.storage.load_app_storage(app_id).await?;
        Ok(storage.get(key).cloned().unwrap_or(serde_json::Value::Null))
    }

    /// Set app storage (KV) value.
    pub async fn set_storage(
        &self,
        app_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> BitFunResult<()> {
        self.storage.save_app_storage(app_id, key, value).await
    }

    pub async fn mark_deps_installed(&self, app_id: &str) -> BitFunResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        Self::ensure_runtime_state(&mut app);
        app.runtime.deps_dirty = false;
        app.runtime.worker_restart_required = true;
        self.storage.save(&app).await?;
        Ok(app)
    }

    pub async fn clear_worker_restart_required(&self, app_id: &str) -> BitFunResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        Self::ensure_runtime_state(&mut app);
        if app.runtime.worker_restart_required {
            app.runtime.worker_restart_required = false;
            self.storage.save(&app).await?;
        }
        Ok(app)
    }

    /// List version numbers for an app.
    pub async fn list_versions(&self, app_id: &str) -> BitFunResult<Vec<u32>> {
        self.storage.list_versions(app_id).await
    }

    /// Rollback app to a previous version (loads version snapshot, saves as current).
    pub async fn rollback(&self, app_id: &str, version: u32) -> BitFunResult<MiniApp> {
        let current = self.storage.load(app_id).await?;
        let mut app = self.storage.load_version(app_id, version).await?;
        let now = Utc::now().timestamp_millis();
        app.version = current.version + 1;
        app.updated_at = now;
        app.runtime = Self::build_runtime_state(
            app.version,
            app.updated_at,
            &app.source,
            !app.source.npm_dependencies.is_empty(),
            true,
        );
        self.storage
            .save_version(app_id, current.version, &current)
            .await?;
        self.storage.save(&app).await?;
        Ok(app)
    }

    /// Recompile app (e.g. after workspace or theme change). Updates compiled_html and saves.
    pub async fn recompile(
        &self,
        app_id: &str,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        app.compiled_html =
            self.compile_source(app_id, &app.source, &app.permissions, theme, workspace_root)?;
        app.updated_at = Utc::now().timestamp_millis();
        Self::ensure_runtime_state(&mut app);
        app.runtime.ui_recompile_required = false;
        self.storage.save(&app).await?;
        Ok(app)
    }

    pub async fn sync_from_fs(
        &self,
        app_id: &str,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        let previous_app = self.storage.load(app_id).await?;
        let mut app = previous_app.clone();
        app.source = self.storage.load_source_only(app_id).await?;
        app.version += 1;
        app.updated_at = Utc::now().timestamp_millis();

        app.compiled_html =
            self.compile_source(app_id, &app.source, &app.permissions, theme, workspace_root)?;
        app.runtime = Self::build_runtime_state(
            app.version,
            app.updated_at,
            &app.source,
            !app.source.npm_dependencies.is_empty(),
            true,
        );
        self.storage
            .save_version(app_id, previous_app.version, &previous_app)
            .await?;
        self.storage.save(&app).await?;
        Ok(app)
    }

    /// Import a MiniApp from a directory (e.g. miniapps/my-app). Copies meta, source, package.json, storage into a new app id and recompiles.
    pub async fn import_from_path(
        &self,
        source_path: PathBuf,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        use crate::util::errors::BitFunError;

        let src = source_path.as_path();
        if !src.is_dir() {
            return Err(BitFunError::validation(format!(
                "Not a directory: {}",
                src.display()
            )));
        }

        let meta_path = src.join("meta.json");
        let source_dir = src.join("source");
        if !meta_path.exists() {
            return Err(BitFunError::validation(format!(
                "Missing meta.json in {}",
                src.display()
            )));
        }
        if !source_dir.is_dir() {
            return Err(BitFunError::validation(format!(
                "Missing source/ directory in {}",
                src.display()
            )));
        }
        for required in &["index.html", "style.css", "ui.js", "worker.js"] {
            if !source_dir.join(required).exists() {
                return Err(BitFunError::validation(format!(
                    "Missing source/{} in {}",
                    required,
                    src.display()
                )));
            }
        }

        let meta_content = tokio::fs::read_to_string(&meta_path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read meta.json: {}", e)))?;
        let mut meta: MiniAppMeta = serde_json::from_str(&meta_content)
            .map_err(|e| BitFunError::parse(format!("Invalid meta.json: {}", e)))?;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();
        meta.id = id.clone();
        meta.created_at = now;
        meta.updated_at = now;

        let dest_dir = self.path_manager.miniapp_dir(&id);
        let dest_source = dest_dir.join("source");
        tokio::fs::create_dir_all(&dest_source)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create app dir: {}", e)))?;

        let meta_json = serde_json::to_string_pretty(&meta).map_err(BitFunError::from)?;
        tokio::fs::write(dest_dir.join("meta.json"), meta_json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write meta.json: {}", e)))?;

        for name in &["index.html", "style.css", "ui.js", "worker.js"] {
            let from = source_dir.join(name);
            let to = dest_source.join(name);
            if from.exists() {
                tokio::fs::copy(&from, &to)
                    .await
                    .map_err(|e| BitFunError::io(format!("Failed to copy {}: {}", name, e)))?;
            }
        }
        let esm_path = source_dir.join("esm_dependencies.json");
        if esm_path.exists() {
            tokio::fs::copy(&esm_path, dest_source.join("esm_dependencies.json"))
                .await
                .map_err(|e| {
                    BitFunError::io(format!("Failed to copy esm_dependencies.json: {}", e))
                })?;
        } else {
            tokio::fs::write(dest_source.join("esm_dependencies.json"), "[]")
                .await
                .map_err(|_e| BitFunError::io("Failed to write esm_dependencies.json"))?;
        }

        let pkg_src = src.join("package.json");
        if pkg_src.exists() {
            tokio::fs::copy(&pkg_src, dest_dir.join("package.json"))
                .await
                .map_err(|e| BitFunError::io(format!("Failed to copy package.json: {}", e)))?;
        } else {
            let pkg = serde_json::json!({
                "name": format!("miniapp-{}", id),
                "private": true,
                "dependencies": {}
            });
            tokio::fs::write(
                dest_dir.join("package.json"),
                serde_json::to_string_pretty(&pkg).map_err(BitFunError::from)?,
            )
            .await
            .map_err(|_e| BitFunError::io("Failed to write package.json"))?;
        }

        let storage_src = src.join("storage.json");
        if storage_src.exists() {
            tokio::fs::copy(&storage_src, dest_dir.join("storage.json"))
                .await
                .map_err(|e| BitFunError::io(format!("Failed to copy storage.json: {}", e)))?;
        } else {
            tokio::fs::write(dest_dir.join("storage.json"), "{}")
                .await
                .map_err(|_e| BitFunError::io("Failed to write storage.json"))?;
        }

        let placeholder_html = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>Loading...</body></html>";
        tokio::fs::write(dest_dir.join("compiled.html"), placeholder_html)
            .await
            .map_err(|_e| BitFunError::io("Failed to write placeholder compiled.html"))?;

        let mut app = self.recompile(&id, "dark", workspace_root).await?;
        app.runtime = Self::build_runtime_state(
            app.version,
            app.updated_at,
            &app.source,
            !app.source.npm_dependencies.is_empty(),
            true,
        );
        self.storage.save(&app).await?;
        Ok(app)
    }
}
