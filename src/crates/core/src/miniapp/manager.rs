//! MiniApp manager — CRUD, version management, compile on save (V2: no permission guard, policy for Worker).

use crate::miniapp::compiler::compile;
use crate::miniapp::permission_policy::resolve_policy;
use crate::miniapp::storage::MiniAppStorage;
use crate::miniapp::types::{
    MiniApp, MiniAppAiContext, MiniAppMeta, MiniAppPermissions, MiniAppSource,
};
use crate::product_domain_runtime::CoreProductDomainRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_product_domains::miniapp::customization::{
    apply_draft_customization_metadata, decline_builtin_update_metadata,
    declined_builtin_update_needs_local_snapshot, diff_permissions,
    is_current_declined_builtin_update, mark_builtin_update_available_metadata,
    MiniAppCustomizationBaseline, MiniAppCustomizationLocalSnapshot, MiniAppCustomizationMetadata,
    MiniAppPermissionDiff,
};
use bitfun_product_domains::miniapp::draft::{
    build_draft_manifest, build_draft_response, MiniAppDraft, MiniAppDraftManifest,
};
use bitfun_product_domains::miniapp::lifecycle::{
    apply_draft_permission_update_result, apply_draft_source_sync_result, apply_draft_to_active,
    apply_update_patch, build_created_app, build_worker_revision, ensure_runtime_state,
    prepare_draft_app, prepare_imported_meta, workspace_dir_string, MiniAppCreateInput,
    MiniAppUpdatePatch,
};
use bitfun_product_domains::miniapp::ports::{
    MiniAppPortError, MiniAppPortErrorKind, MiniAppRuntimeFacade,
};
use bitfun_product_domains::miniapp::storage::{
    build_import_fallbacks, MiniAppImportLayout, COMPILED_HTML, ESM_DEPS_JSON, META_JSON,
    PACKAGE_JSON, REQUIRED_SOURCE_FILES, SOURCE_DIR, STORAGE_JSON,
};
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

    pub fn build_worker_revision(&self, app: &MiniApp, policy_json: &str) -> String {
        build_worker_revision(app, policy_json)
    }

    fn runtime_facade(&self) -> MiniAppRuntimeFacade<'_> {
        CoreProductDomainRuntime::miniapp_runtime_facade(&self.storage)
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

        let app = build_created_app(
            id.clone(),
            MiniAppCreateInput {
                name,
                description,
                icon,
                category,
                tags,
                source,
                permissions,
                ai_context,
            },
            compiled_html,
            now,
        );

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
        let previous_app = self.storage.load(app_id).await?;
        let patch = MiniAppUpdatePatch {
            name,
            description,
            icon,
            category,
            tags,
            source,
            permissions,
            ai_context,
        };
        let now = Utc::now().timestamp_millis();
        let compiled_html = self.compile_source(
            app_id,
            patch.source_for_compile(&previous_app),
            patch.permissions_for_compile(&previous_app),
            "dark",
            workspace_root,
        )?;
        let app = apply_update_patch(&previous_app, patch, compiled_html, now);

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
        let app = self.get(app_id).await?;
        let now = Utc::now().timestamp_millis();
        let draft_id = Uuid::new_v4().to_string();
        let compiled_html = self.compile_source_with_app_data_dir(
            app_id,
            &self.storage.draft_dir(app_id, &draft_id),
            &app.source,
            &app.permissions,
            theme,
            workspace_root,
        )?;
        let app = prepare_draft_app(app, compiled_html, now);

        let manifest = build_draft_manifest(app_id, draft_id, app.version, now);
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
        let app = self.storage.load_draft_app(app_id, draft_id).await?;
        let mut manifest = self.load_draft_manifest(app_id, draft_id).await?;
        let now = Utc::now().timestamp_millis();
        let compiled_html = self.compile_source_with_app_data_dir(
            app_id,
            &self.storage.draft_dir(app_id, draft_id),
            &app.source,
            &app.permissions,
            theme,
            workspace_root,
        )?;
        let app = apply_draft_source_sync_result(app, compiled_html, now);
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
        let app = self.storage.load_draft_app(app_id, draft_id).await?;
        let mut manifest = self.load_draft_manifest(app_id, draft_id).await?;
        let now = Utc::now().timestamp_millis();
        let compiled_html = self.compile_source_with_app_data_dir(
            app_id,
            &self.storage.draft_dir(app_id, draft_id),
            &app.source,
            &permissions,
            theme,
            workspace_root,
        )?;
        let app = apply_draft_permission_update_result(app, permissions, compiled_html, now);
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
        let now = Utc::now().timestamp_millis();
        let compiled_html = self.compile_source(
            app_id,
            &draft.source,
            &draft.permissions,
            theme,
            workspace_root,
        )?;
        let app = apply_draft_to_active(&current, draft, compiled_html, now);

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

    pub async fn mark_stale_drafts_for_cleanup(&self) -> BitFunResult<Vec<PathBuf>> {
        self.storage.mark_stale_drafts_for_cleanup().await
    }

    pub async fn cleanup_marked_drafts(&self, targets: Vec<PathBuf>) -> BitFunResult<()> {
        self.storage.cleanup_marked_drafts(targets).await
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
        let draft_root = self
            .storage
            .draft_dir(app_id, &manifest.draft_id)
            .to_string_lossy()
            .to_string();
        build_draft_response(draft_root, app, manifest)
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
        let existing = self.storage.load_customization_metadata(app_id).await?;
        let baseline = if let Some(builtin) = crate::miniapp::BUILTIN_APPS
            .iter()
            .find(|builtin| builtin.id == app_id)
        {
            MiniAppCustomizationBaseline::Builtin {
                builtin_id: builtin.id.to_string(),
                builtin_version: builtin.version,
            }
        } else {
            MiniAppCustomizationBaseline::UserCreated
        };
        let metadata = apply_draft_customization_metadata(existing, baseline, draft_id, now);
        self.storage
            .save_customization_metadata(app_id, &metadata)
            .await
    }

    pub async fn mark_builtin_update_available(
        &self,
        app_id: &str,
        builtin_version: u32,
        source_hash: &str,
        detected_at: i64,
    ) -> BitFunResult<bool> {
        if let Some(metadata) = self.storage.load_customization_metadata(app_id).await? {
            let declined_update_current = self
                .has_matching_declined_builtin_update(app_id, &metadata, source_hash)
                .await?;
            let decision = mark_builtin_update_available_metadata(
                metadata,
                builtin_version,
                source_hash,
                detected_at,
                declined_update_current,
            );
            if decision.metadata_changed {
                self.storage
                    .save_customization_metadata(app_id, &decision.metadata)
                    .await?;
            }
            return Ok(decision.should_surface_update);
        }
        Ok(false)
    }

    async fn has_matching_declined_builtin_update(
        &self,
        app_id: &str,
        metadata: &MiniAppCustomizationMetadata,
        source_hash: &str,
    ) -> BitFunResult<bool> {
        let local_snapshot = if declined_builtin_update_needs_local_snapshot(metadata, source_hash)
        {
            self.storage
                .load(app_id)
                .await
                .ok()
                .map(|app| MiniAppCustomizationLocalSnapshot {
                    version: app.version,
                    updated_at: app.updated_at,
                })
        } else {
            None
        };

        Ok(is_current_declined_builtin_update(
            metadata,
            source_hash,
            local_snapshot,
        ))
    }

    pub async fn decline_builtin_update(
        &self,
        app_id: &str,
        builtin_version: u32,
        source_hash: &str,
        declined_at: i64,
    ) -> BitFunResult<Option<MiniAppCustomizationMetadata>> {
        let Some(metadata) = self.storage.load_customization_metadata(app_id).await? else {
            return Ok(None);
        };

        let local_snapshot =
            self.storage
                .load(app_id)
                .await
                .ok()
                .map(|app| MiniAppCustomizationLocalSnapshot {
                    version: app.version,
                    updated_at: app.updated_at,
                });
        let metadata = decline_builtin_update_metadata(
            metadata,
            builtin_version,
            source_hash,
            declined_at,
            local_snapshot,
        );
        self.storage
            .save_customization_metadata(app_id, &metadata)
            .await?;
        Ok(Some(metadata))
    }

    pub async fn mark_deps_installed(&self, app_id: &str) -> BitFunResult<MiniApp> {
        self.runtime_facade()
            .mark_deps_installed(app_id.to_string())
            .await
            .map_err(map_miniapp_port_error)
    }

    pub async fn clear_worker_restart_required(&self, app_id: &str) -> BitFunResult<MiniApp> {
        self.runtime_facade()
            .clear_worker_restart_required(app_id.to_string())
            .await
            .map_err(map_miniapp_port_error)
    }

    /// List version numbers for an app.
    pub async fn list_versions(&self, app_id: &str) -> BitFunResult<Vec<u32>> {
        self.storage.list_versions(app_id).await
    }

    /// Rollback app to a previous version (loads version snapshot, saves as current).
    pub async fn rollback(&self, app_id: &str, version: u32) -> BitFunResult<MiniApp> {
        let now = Utc::now().timestamp_millis();
        self.runtime_facade()
            .rollback(app_id.to_string(), version, now)
            .await
            .map_err(map_miniapp_port_error)
    }

    /// Recompile app (e.g. after workspace or theme change). Updates compiled_html and saves.
    pub async fn recompile(
        &self,
        app_id: &str,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        let app = self.storage.load(app_id).await?;
        let compiled_html =
            self.compile_source(app_id, &app.source, &app.permissions, theme, workspace_root)?;
        self.runtime_facade()
            .persist_recompile_result_for_app(app, compiled_html, Utc::now().timestamp_millis())
            .await
            .map_err(map_miniapp_port_error)
    }

    pub async fn sync_from_fs(
        &self,
        app_id: &str,
        theme: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<MiniApp> {
        let previous_app = self.storage.load(app_id).await?;
        let source = self.storage.load_source_only(app_id).await?;
        let compiled_html = self.compile_source(
            app_id,
            &source,
            &previous_app.permissions,
            theme,
            workspace_root,
        )?;
        self.runtime_facade()
            .persist_sync_from_fs_result_for_app(
                app_id.to_string(),
                previous_app,
                source,
                compiled_html,
                Utc::now().timestamp_millis(),
            )
            .await
            .map_err(map_miniapp_port_error)
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

        let import_layout = MiniAppImportLayout::new(src);
        let meta_path = import_layout.meta_path();
        let source_dir = import_layout.source_dir();
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
        for (required, path) in import_layout.required_source_file_paths() {
            if !path.exists() {
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
        prepare_imported_meta(&mut meta, &id, now);

        let fallbacks = build_import_fallbacks(&id);
        let dest_dir = self.path_manager.miniapp_dir(&id);
        let dest_source = dest_dir.join(SOURCE_DIR);
        tokio::fs::create_dir_all(&dest_source)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create app dir: {}", e)))?;

        let meta_json = serde_json::to_string_pretty(&meta).map_err(BitFunError::from)?;
        tokio::fs::write(dest_dir.join(META_JSON), meta_json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write meta.json: {}", e)))?;

        for name in REQUIRED_SOURCE_FILES {
            let from = source_dir.join(name);
            let to = dest_source.join(name);
            if from.exists() {
                tokio::fs::copy(&from, &to)
                    .await
                    .map_err(|e| BitFunError::io(format!("Failed to copy {}: {}", name, e)))?;
            }
        }
        let esm_path = import_layout.esm_dependencies_path();
        if esm_path.exists() {
            tokio::fs::copy(&esm_path, dest_source.join(ESM_DEPS_JSON))
                .await
                .map_err(|e| {
                    BitFunError::io(format!("Failed to copy esm_dependencies.json: {}", e))
                })?;
        } else {
            tokio::fs::write(
                dest_source.join(ESM_DEPS_JSON),
                fallbacks.esm_dependencies_json,
            )
            .await
            .map_err(|_e| BitFunError::io("Failed to write esm_dependencies.json"))?;
        }

        let pkg_src = import_layout.package_json_path();
        if pkg_src.exists() {
            tokio::fs::copy(&pkg_src, dest_dir.join(PACKAGE_JSON))
                .await
                .map_err(|e| BitFunError::io(format!("Failed to copy package.json: {}", e)))?;
        } else {
            tokio::fs::write(
                dest_dir.join(PACKAGE_JSON),
                serde_json::to_string_pretty(&fallbacks.package_json).map_err(BitFunError::from)?,
            )
            .await
            .map_err(|_e| BitFunError::io("Failed to write package.json"))?;
        }

        let storage_src = import_layout.storage_json_path();
        if storage_src.exists() {
            tokio::fs::copy(&storage_src, dest_dir.join(STORAGE_JSON))
                .await
                .map_err(|e| BitFunError::io(format!("Failed to copy storage.json: {}", e)))?;
        } else {
            tokio::fs::write(dest_dir.join(STORAGE_JSON), fallbacks.storage_json)
                .await
                .map_err(|_e| BitFunError::io("Failed to write storage.json"))?;
        }

        tokio::fs::write(dest_dir.join(COMPILED_HTML), fallbacks.compiled_html)
            .await
            .map_err(|_e| BitFunError::io("Failed to write placeholder compiled.html"))?;

        let app = self.recompile(&id, "dark", workspace_root).await?;
        self.runtime_facade()
            .persist_import_runtime_state(app)
            .await
            .map_err(map_miniapp_port_error)
    }
}

fn map_miniapp_port_error(error: MiniAppPortError) -> BitFunError {
    let message = strip_bitfun_error_prefix(error.message);
    match error.kind {
        MiniAppPortErrorKind::NotFound => BitFunError::NotFound(message),
        MiniAppPortErrorKind::InvalidInput => BitFunError::validation(message),
        MiniAppPortErrorKind::PermissionDenied => BitFunError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            message,
        )),
        MiniAppPortErrorKind::RuntimeUnavailable => BitFunError::ProcessError(message),
        MiniAppPortErrorKind::Io => BitFunError::io(message),
        MiniAppPortErrorKind::Backend => BitFunError::service(message),
    }
}

fn strip_bitfun_error_prefix(message: String) -> String {
    const PREFIXES: &[&str] = &[
        "Not found: ",
        "Validation error: ",
        "Deserialization error: ",
        "IO error: ",
        "Process error: ",
        "Service error: ",
    ];

    for prefix in PREFIXES {
        if let Some(stripped) = message.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miniapp::types::{
        FsPermissions, MiniAppMeta, MiniAppPermissions, MiniAppSource, NpmDep,
    };
    use bitfun_product_domains::miniapp::storage::{
        COMPILED_HTML, ESM_DEPS_JSON, INDEX_HTML, PACKAGE_JSON, SOURCE_DIR, STORAGE_JSON,
        STYLE_CSS, UI_JS, WORKER_JS,
    };

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

    #[test]
    fn miniapp_port_error_mapping_preserves_manager_error_shape() {
        let not_found = map_miniapp_port_error(MiniAppPortError::new(
            MiniAppPortErrorKind::NotFound,
            "Not found: MiniApp not found: missing",
        ));
        assert_eq!(
            not_found.to_string(),
            "Not found: MiniApp not found: missing"
        );

        let permission_denied = map_miniapp_port_error(MiniAppPortError::new(
            MiniAppPortErrorKind::PermissionDenied,
            "IO error: access denied",
        ));
        match permission_denied {
            BitFunError::Io(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
                assert_eq!(error.to_string(), "access denied");
            }
            other => panic!("expected permission denied IO error, got {other:?}"),
        }
    }

    async fn write_import_source(root: &std::path::Path) {
        let source_dir = root.join(SOURCE_DIR);
        tokio::fs::create_dir_all(&source_dir).await.unwrap();
        let meta = MiniAppMeta {
            id: "template-id".to_string(),
            name: "Imported".to_string(),
            description: "Imported app".to_string(),
            icon: "box".to_string(),
            category: "utility".to_string(),
            tags: vec!["imported".to_string()],
            version: 7,
            created_at: 11,
            updated_at: 12,
            permissions: MiniAppPermissions::default(),
            ai_context: None,
            runtime: Default::default(),
            i18n: None,
        };
        tokio::fs::write(
            root.join(META_JSON),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(
            source_dir.join(INDEX_HTML),
            "<!DOCTYPE html><html><head></head><body><div id=\"app\"></div></body></html>",
        )
        .await
        .unwrap();
        tokio::fs::write(source_dir.join(STYLE_CSS), "body { color: blue; }")
            .await
            .unwrap();
        tokio::fs::write(
            source_dir.join(UI_JS),
            "document.getElementById('app').textContent = 'imported';",
        )
        .await
        .unwrap();
        tokio::fs::write(source_dir.join(WORKER_JS), "")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn runtime_preflight_preserves_recompile_sync_rollback_and_deps_state() {
        let manager = test_manager();
        let mut app = create_sample_app(&manager).await;
        app.source.npm_dependencies = vec![NpmDep {
            name: "lodash".to_string(),
            version: "^4.17.21".to_string(),
        }];
        manager.storage.save(&app).await.unwrap();

        let installed = manager.mark_deps_installed(&app.id).await.unwrap();
        assert!(!installed.runtime.deps_dirty);
        assert!(installed.runtime.worker_restart_required);
        let cleared = manager
            .clear_worker_restart_required(&app.id)
            .await
            .unwrap();
        assert!(!cleared.runtime.worker_restart_required);

        let style_path = manager
            .path_manager()
            .miniapp_dir(&app.id)
            .join(SOURCE_DIR)
            .join(STYLE_CSS);
        tokio::fs::write(&style_path, "body { color: red; }")
            .await
            .unwrap();
        let synced = manager.sync_from_fs(&app.id, "dark", None).await.unwrap();
        assert_eq!(synced.version, app.version + 1);
        assert_eq!(synced.source.css, "body { color: red; }");
        assert!(synced.runtime.deps_dirty);
        assert!(synced.runtime.worker_restart_required);
        assert_eq!(manager.list_versions(&app.id).await.unwrap(), vec![1]);

        let recompiled = manager.recompile(&app.id, "dark", None).await.unwrap();
        assert_eq!(recompiled.version, synced.version);
        assert_eq!(recompiled.source.css, synced.source.css);
        assert!(recompiled.compiled_html.contains("body { color: red; }"));
        assert!(!recompiled.runtime.ui_recompile_required);

        let rolled_back = manager.rollback(&app.id, app.version).await.unwrap();
        assert_eq!(rolled_back.version, recompiled.version + 1);
        // sync_from_fs snapshots the source already loaded from disk; keep this
        // boundary explicit before moving manager/runtime ownership.
        assert_eq!(rolled_back.source.css, "body { color: red; }");
        assert!(rolled_back.runtime.deps_dirty);
        assert!(rolled_back.runtime.worker_restart_required);
        assert_eq!(manager.list_versions(&app.id).await.unwrap(), vec![1, 2]);
    }

    #[tokio::test]
    async fn import_from_path_preserves_fallback_files_recompile_and_runtime_state() {
        let manager = test_manager();
        let import_root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-import-source-{}",
            uuid::Uuid::new_v4()
        ));
        write_import_source(&import_root).await;

        let imported = manager
            .import_from_path(import_root.clone(), None)
            .await
            .unwrap();
        let app_dir = manager.path_manager().miniapp_dir(&imported.id);
        let source_dir = app_dir.join(SOURCE_DIR);

        assert_ne!(imported.id, "template-id");
        assert_eq!(imported.name, "Imported");
        assert_eq!(imported.version, 7);
        assert_eq!(imported.source.css, "body { color: blue; }");
        assert!(imported.compiled_html.contains("textContent = 'imported'"));
        assert!(!imported.runtime.deps_dirty);
        assert!(imported.runtime.worker_restart_required);
        assert!(!imported.runtime.ui_recompile_required);

        assert_eq!(
            tokio::fs::read_to_string(source_dir.join(ESM_DEPS_JSON))
                .await
                .unwrap(),
            "[]"
        );
        assert_eq!(
            tokio::fs::read_to_string(app_dir.join(STORAGE_JSON))
                .await
                .unwrap(),
            "{}"
        );
        let package_json: serde_json::Value = serde_json::from_str(
            &tokio::fs::read_to_string(app_dir.join(PACKAGE_JSON))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(package_json["name"], format!("miniapp-{}", imported.id));
        assert_eq!(package_json["dependencies"], serde_json::json!({}));
        assert!(tokio::fs::read_to_string(app_dir.join(COMPILED_HTML))
            .await
            .unwrap()
            .contains("textContent = 'imported'"));

        let _ = tokio::fs::remove_dir_all(import_root).await;
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
