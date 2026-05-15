//! MiniApp manager — CRUD, version management, compile on save (V2: no permission guard, policy for Worker).

use crate::miniapp::compiler::compile;
use crate::miniapp::permission_policy::resolve_policy;
use crate::miniapp::storage::MiniAppStorage;
use crate::miniapp::types::{
    MiniApp, MiniAppAiContext, MiniAppMeta, MiniAppPermissions, MiniAppSource,
};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_product_domains::miniapp::customization::{
    diff_permissions, MiniAppAvailableBuiltinUpdate, MiniAppCustomizationMetadata,
    MiniAppCustomizationOrigin, MiniAppCustomizationOriginKind, MiniAppPermissionDiff,
};
use bitfun_product_domains::miniapp::lifecycle::{
    build_deps_revision, build_runtime_state, build_source_revision, build_worker_revision,
    ensure_runtime_state, workspace_dir_string,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppDraftManifest {
    pub app_id: String,
    pub draft_id: String,
    pub source_version: u32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppDraft {
    pub app_id: String,
    pub draft_id: String,
    pub source_version: u32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub draft_root: String,
    pub app: MiniApp,
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

    pub fn build_worker_revision(&self, app: &MiniApp, policy_json: &str) -> String {
        build_worker_revision(app, policy_json)
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
        let workspace_dir = workspace_dir_string(workspace_root);

        compile(
            source,
            permissions,
            app_id,
            &app_data_dir_str,
            &workspace_dir,
            theme,
        )
    }

    fn compile_source_with_app_data_dir(
        &self,
        app_id: &str,
        app_data_dir: &Path,
        source: &MiniAppSource,
        permissions: &MiniAppPermissions,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<String> {
        let app_data_dir_str = app_data_dir.to_string_lossy().to_string();
        let workspace_dir = workspace_dir_string(workspace_root);

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
        if ensure_runtime_state(&mut app) {
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
            build_runtime_state(1, now, &source, !source.npm_dependencies.is_empty(), true);

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
            i18n: None,
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
            app.runtime.source_revision = build_source_revision(app.version, app.updated_at);
            app.runtime.worker_restart_required = true;
        }
        if deps_changed {
            app.runtime.deps_revision = build_deps_revision(&app.source);
            app.runtime.deps_dirty = !app.source.npm_dependencies.is_empty();
            app.runtime.worker_restart_required = true;
        }
        app.runtime.ui_recompile_required = false;
        ensure_runtime_state(&mut app);

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

    pub async fn resolve_policy_for_draft(
        &self,
        app_id: &str,
        draft_id: &str,
        permissions: &MiniAppPermissions,
        workspace_root: Option<&Path>,
    ) -> serde_json::Value {
        let app_data_dir = self.storage.draft_dir(app_id, draft_id);
        let gp = self.granted_paths.read().await;
        let granted = gp.get(app_id).map(|v| v.as_slice()).unwrap_or(&[]);
        resolve_policy(permissions, app_id, &app_data_dir, workspace_root, granted)
    }

    /// Snapshot of user-granted extra paths for an app (used by the host-side dispatch
    /// to mirror what `resolve_policy_for_app` would inject into the worker policy).
    pub async fn granted_paths_for_app(&self, app_id: &str) -> Vec<PathBuf> {
        let gp = self.granted_paths.read().await;
        gp.get(app_id).cloned().unwrap_or_default()
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

    pub async fn get_draft_storage(
        &self,
        app_id: &str,
        draft_id: &str,
        key: &str,
    ) -> BitFunResult<serde_json::Value> {
        let storage = self.storage.load_draft_storage(app_id, draft_id).await?;
        Ok(storage.get(key).cloned().unwrap_or(serde_json::Value::Null))
    }

    pub async fn set_draft_storage(
        &self,
        app_id: &str,
        draft_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> BitFunResult<()> {
        self.storage
            .save_draft_storage(app_id, draft_id, key, value)
            .await
    }

    pub async fn create_draft(
        &self,
        app_id: &str,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniAppDraft> {
        let mut app = self.get(app_id).await?;
        let now = Utc::now().timestamp_millis();
        let draft_id = Uuid::new_v4().to_string();
        app.updated_at = now;
        app.compiled_html = self.compile_source_with_app_data_dir(
            app_id,
            &self.storage.draft_dir(app_id, &draft_id),
            &app.source,
            &app.permissions,
            theme,
            workspace_root,
        )?;
        ensure_runtime_state(&mut app);

        let manifest = MiniAppDraftManifest {
            app_id: app_id.to_string(),
            draft_id,
            source_version: app.version,
            status: "draft".to_string(),
            created_at: now,
            updated_at: now,
        };
        self.save_draft_with_manifest(app_id, app, manifest).await
    }

    pub async fn get_draft(&self, app_id: &str, draft_id: &str) -> BitFunResult<MiniAppDraft> {
        let app = self.storage.load_draft_app(app_id, draft_id).await?;
        let manifest = self.load_draft_manifest(app_id, draft_id).await?;
        Ok(self.build_draft_response(app_id, app, manifest))
    }

    pub async fn sync_draft_from_fs(
        &self,
        app_id: &str,
        draft_id: &str,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniAppDraft> {
        let mut app = self.storage.load_draft_app(app_id, draft_id).await?;
        let mut manifest = self.load_draft_manifest(app_id, draft_id).await?;
        app.updated_at = Utc::now().timestamp_millis();
        app.compiled_html = self.compile_source_with_app_data_dir(
            app_id,
            &self.storage.draft_dir(app_id, draft_id),
            &app.source,
            &app.permissions,
            theme,
            workspace_root,
        )?;
        app.runtime = build_runtime_state(
            app.version,
            app.updated_at,
            &app.source,
            !app.source.npm_dependencies.is_empty(),
            true,
        );
        manifest.updated_at = app.updated_at;
        self.save_draft_with_manifest(app_id, app, manifest).await
    }

    pub async fn set_draft_permissions(
        &self,
        app_id: &str,
        draft_id: &str,
        permissions: MiniAppPermissions,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniAppDraft> {
        let mut app = self.storage.load_draft_app(app_id, draft_id).await?;
        let mut manifest = self.load_draft_manifest(app_id, draft_id).await?;
        app.permissions = permissions;
        app.updated_at = Utc::now().timestamp_millis();
        app.compiled_html = self.compile_source_with_app_data_dir(
            app_id,
            &self.storage.draft_dir(app_id, draft_id),
            &app.source,
            &app.permissions,
            theme,
            workspace_root,
        )?;
        app.runtime = build_runtime_state(
            app.version,
            app.updated_at,
            &app.source,
            !app.source.npm_dependencies.is_empty(),
            true,
        );
        manifest.updated_at = app.updated_at;
        self.save_draft_with_manifest(app_id, app, manifest).await
    }

    pub async fn permission_diff_for_draft(
        &self,
        app_id: &str,
        draft_id: &str,
    ) -> BitFunResult<MiniAppPermissionDiff> {
        let active = self.get(app_id).await?;
        let draft = self.storage.load_draft_app(app_id, draft_id).await?;
        Ok(diff_permissions(&active.permissions, &draft.permissions))
    }

    pub async fn apply_draft(
        &self,
        app_id: &str,
        draft_id: &str,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        let current = self.get(app_id).await?;
        let draft = self.storage.load_draft_app(app_id, draft_id).await?;
        let mut app = current.clone();
        let now = Utc::now().timestamp_millis();

        app.name = draft.name;
        app.description = draft.description;
        app.icon = draft.icon;
        app.category = draft.category;
        app.tags = draft.tags;
        app.source = draft.source;
        app.permissions = draft.permissions;
        app.ai_context = draft.ai_context;
        app.i18n = draft.i18n;
        app.version = current.version + 1;
        app.updated_at = now;
        app.compiled_html =
            self.compile_source(app_id, &app.source, &app.permissions, theme, workspace_root)?;
        app.runtime = build_runtime_state(
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
        self.record_draft_applied(app_id, draft_id, now).await?;
        Ok(app)
    }

    pub async fn discard_draft(&self, app_id: &str, draft_id: &str) -> BitFunResult<()> {
        self.storage.delete_draft(app_id, draft_id).await
    }

    pub async fn load_customization_metadata(
        &self,
        app_id: &str,
    ) -> BitFunResult<Option<MiniAppCustomizationMetadata>> {
        self.storage.load_customization_metadata(app_id).await
    }

    pub async fn save_customization_metadata(
        &self,
        app_id: &str,
        metadata: &MiniAppCustomizationMetadata,
    ) -> BitFunResult<()> {
        self.storage
            .save_customization_metadata(app_id, metadata)
            .await
    }

    pub fn draft_dir(&self, app_id: &str, draft_id: &str) -> PathBuf {
        self.storage.draft_dir(app_id, draft_id)
    }

    fn build_draft_response(
        &self,
        app_id: &str,
        app: MiniApp,
        manifest: MiniAppDraftManifest,
    ) -> MiniAppDraft {
        let draft_id = manifest.draft_id;
        let draft_root = self
            .storage
            .draft_dir(app_id, &draft_id)
            .to_string_lossy()
            .to_string();
        MiniAppDraft {
            app_id: manifest.app_id,
            draft_id,
            source_version: manifest.source_version,
            status: manifest.status,
            created_at: manifest.created_at,
            updated_at: manifest.updated_at,
            draft_root,
            app,
        }
    }

    async fn load_draft_manifest(
        &self,
        app_id: &str,
        draft_id: &str,
    ) -> BitFunResult<MiniAppDraftManifest> {
        let value = self.storage.load_draft_manifest(app_id, draft_id).await?;
        serde_json::from_value(value)
            .map_err(|e| BitFunError::parse(format!("Invalid draft manifest: {}", e)))
    }

    async fn save_draft_with_manifest(
        &self,
        app_id: &str,
        app: MiniApp,
        manifest: MiniAppDraftManifest,
    ) -> BitFunResult<MiniAppDraft> {
        let manifest_value = serde_json::to_value(&manifest).map_err(BitFunError::from)?;
        self.storage
            .save_draft(app_id, &manifest.draft_id, &app, &manifest_value)
            .await?;
        Ok(self.build_draft_response(app_id, app, manifest))
    }

    async fn record_draft_applied(
        &self,
        app_id: &str,
        draft_id: &str,
        now: i64,
    ) -> BitFunResult<()> {
        let mut metadata =
            if let Some(existing) = self.storage.load_customization_metadata(app_id).await? {
                existing
            } else if let Some(builtin) = crate::miniapp::BUILTIN_APPS
                .iter()
                .find(|builtin| builtin.id == app_id)
            {
                MiniAppCustomizationMetadata {
                    origin: MiniAppCustomizationOrigin {
                        kind: MiniAppCustomizationOriginKind::Builtin,
                        builtin_id: Some(builtin.id.to_string()),
                        builtin_version: Some(builtin.version),
                    },
                    local_override: true,
                    last_applied_draft_id: None,
                    available_builtin_update: None,
                    updated_at: now,
                }
            } else {
                MiniAppCustomizationMetadata {
                    origin: MiniAppCustomizationOrigin {
                        kind: MiniAppCustomizationOriginKind::UserCreated,
                        builtin_id: None,
                        builtin_version: None,
                    },
                    local_override: false,
                    last_applied_draft_id: None,
                    available_builtin_update: None,
                    updated_at: now,
                }
            };

        if matches!(
            metadata.origin.kind,
            MiniAppCustomizationOriginKind::Builtin
        ) {
            metadata.local_override = true;
            if let Some(builtin) = crate::miniapp::BUILTIN_APPS
                .iter()
                .find(|builtin| builtin.id == app_id)
            {
                metadata.origin.builtin_version = Some(builtin.version);
                metadata.available_builtin_update = None;
            }
        }
        metadata.last_applied_draft_id = Some(draft_id.to_string());
        metadata.updated_at = now;
        self.storage
            .save_customization_metadata(app_id, &metadata)
            .await
    }

    pub async fn mark_builtin_update_available(
        &self,
        app_id: &str,
        builtin_version: u32,
        detected_at: i64,
    ) -> BitFunResult<()> {
        if let Some(mut metadata) = self.storage.load_customization_metadata(app_id).await? {
            metadata.available_builtin_update = Some(MiniAppAvailableBuiltinUpdate {
                builtin_version,
                detected_at,
            });
            metadata.updated_at = detected_at;
            self.storage
                .save_customization_metadata(app_id, &metadata)
                .await?;
        }
        Ok(())
    }

    pub async fn mark_deps_installed(&self, app_id: &str) -> BitFunResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        ensure_runtime_state(&mut app);
        app.runtime.deps_dirty = false;
        app.runtime.worker_restart_required = true;
        self.storage.save(&app).await?;
        Ok(app)
    }

    pub async fn clear_worker_restart_required(&self, app_id: &str) -> BitFunResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        ensure_runtime_state(&mut app);
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
        app.runtime = build_runtime_state(
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
        ensure_runtime_state(&mut app);
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
        app.runtime = build_runtime_state(
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

    /// Import a MiniApp from a directory (e.g. miniapps/git-graph). Copies meta, source, package.json, storage into a new app id and recompiles.
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
        app.runtime = build_runtime_state(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miniapp::types::{FsPermissions, MiniAppPermissions, MiniAppSource};

    fn test_manager() -> MiniAppManager {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-manager-draft-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        MiniAppManager::new(path_manager)
    }

    fn sample_source(css: &str) -> MiniAppSource {
        MiniAppSource {
            html: "<!DOCTYPE html><html><head></head><body><div id=\"app\"></div></body></html>"
                .to_string(),
            css: css.to_string(),
            ui_js: "document.getElementById('app').textContent = 'demo';".to_string(),
            esm_dependencies: Vec::new(),
            worker_js: String::new(),
            npm_dependencies: Vec::new(),
        }
    }

    async fn create_sample_app(manager: &MiniAppManager) -> MiniApp {
        manager
            .create(
                "Demo".to_string(),
                "Demo app".to_string(),
                "box".to_string(),
                "utility".to_string(),
                vec!["demo".to_string()],
                sample_source("body { color: black; }"),
                MiniAppPermissions::default(),
                None,
                None,
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn draft_lifecycle_keeps_active_storage_and_source_isolated_until_apply() {
        let manager = test_manager();
        let app = create_sample_app(&manager).await;
        manager
            .set_storage(&app.id, "score", serde_json::json!(3))
            .await
            .unwrap();

        let draft = manager.create_draft(&app.id, "dark", None).await.unwrap();
        assert_eq!(draft.source_version, app.version);
        assert_eq!(draft.app.source.css, "body { color: black; }");

        let draft_css = manager
            .storage
            .draft_dir(&app.id, &draft.draft_id)
            .join("source")
            .join("style.css");
        tokio::fs::write(&draft_css, "body { background: white; }")
            .await
            .unwrap();

        let draft = manager
            .sync_draft_from_fs(&app.id, &draft.draft_id, "dark", None)
            .await
            .unwrap();
        assert_eq!(draft.app.source.css, "body { background: white; }");

        let active_before_apply = manager.get(&app.id).await.unwrap();
        assert_eq!(active_before_apply.source.css, "body { color: black; }");
        assert_eq!(
            manager.get_storage(&app.id, "score").await.unwrap(),
            serde_json::json!(3)
        );

        let applied = manager
            .apply_draft(&app.id, &draft.draft_id, "dark", None)
            .await
            .unwrap();

        assert_eq!(applied.version, app.version + 1);
        assert_eq!(applied.source.css, "body { background: white; }");
        assert_eq!(manager.list_versions(&app.id).await.unwrap(), vec![1]);
        assert_eq!(
            manager.get_storage(&app.id, "score").await.unwrap(),
            serde_json::json!(3)
        );
    }

    #[tokio::test]
    async fn draft_permission_diff_flags_high_risk_changes_before_apply() {
        let manager = test_manager();
        let app = create_sample_app(&manager).await;
        let draft = manager.create_draft(&app.id, "dark", None).await.unwrap();

        let draft_permissions = MiniAppPermissions {
            fs: Some(FsPermissions {
                read: None,
                write: Some(vec!["{workspace}".to_string()]),
            }),
            ..Default::default()
        };
        manager
            .set_draft_permissions(&app.id, &draft.draft_id, draft_permissions, "dark", None)
            .await
            .unwrap();

        let diff = manager
            .permission_diff_for_draft(&app.id, &draft.draft_id)
            .await
            .unwrap();

        assert!(diff.high_risk);
        assert_eq!(diff.added, vec!["fs.write:{workspace}".to_string()]);
        assert!(manager.get(&app.id).await.unwrap().permissions.fs.is_none());
    }
}
