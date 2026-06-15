//! Storage-backed MiniApp runtime-state facade.
//!
//! The facade owns portable state transitions and persistence sequencing for
//! MiniApp manager workflows. Host-specific compile, filesystem, process, and
//! dispatch IO stay outside product-domains.

use crate::miniapp::customization::{
    apply_draft_customization_metadata, decline_builtin_update_metadata,
    declined_builtin_update_needs_local_snapshot, is_current_declined_builtin_update,
    mark_builtin_update_available_metadata, MiniAppCustomizationBaseline,
    MiniAppCustomizationLocalSnapshot, MiniAppCustomizationMetadata,
};
use crate::miniapp::draft::{
    build_draft_manifest, build_draft_response, MiniAppDraft, MiniAppDraftManifest,
};
use crate::miniapp::lifecycle::{
    apply_draft_permission_update_result, apply_draft_source_sync_result, apply_draft_to_active,
    apply_import_runtime_state, apply_recompile_result, apply_sync_from_fs_result,
    apply_update_patch, build_created_app, clear_worker_restart_required_state,
    ensure_runtime_state, mark_deps_installed_state, prepare_draft_app, prepare_rollback_app,
    MiniAppCreateInput, MiniAppUpdatePatch,
};
use crate::miniapp::ports::{
    MiniAppCompilePort, MiniAppImportFromPathRequest, MiniAppImportPort, MiniAppPortError,
    MiniAppPortErrorKind, MiniAppPortResult, MiniAppStoragePort,
};
use crate::miniapp::storage::{
    build_import_bundle_plan, MiniAppImportBundlePlanError, MiniAppImportBundleWriteRequest,
};
use crate::miniapp::types::{MiniApp, MiniAppMeta, MiniAppSource};

/// Storage-backed facade for MiniApp runtime-state lifecycle transitions.
pub struct MiniAppRuntimeFacade<'a> {
    storage: &'a dyn MiniAppStoragePort,
}

impl<'a> MiniAppRuntimeFacade<'a> {
    pub fn new(storage: &'a dyn MiniAppStoragePort) -> Self {
        Self { storage }
    }

    pub async fn list_metadata(&self) -> MiniAppPortResult<Vec<MiniAppMeta>> {
        let ids = self.storage.list_app_ids().await?;
        let mut metas = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(meta) = self.storage.load_meta(id).await {
                metas.push(meta);
            }
        }
        metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(metas)
    }

    pub async fn load_app_ensuring_runtime_state(
        &self,
        app_id: String,
    ) -> MiniAppPortResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        if ensure_runtime_state(&mut app) {
            self.storage.save(app.clone()).await?;
        }
        Ok(app)
    }

    pub async fn create_app(
        &self,
        id: String,
        input: MiniAppCreateInput,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        let app = build_created_app(id, input, compiled_html, now);
        self.storage.save(app.clone()).await?;
        Ok(app)
    }

    pub async fn persist_update_result_for_app(
        &self,
        app_id: String,
        previous_app: MiniApp,
        patch: MiniAppUpdatePatch,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        let app = apply_update_patch(&previous_app, patch, compiled_html, now);
        self.storage
            .save_version(app_id, previous_app.version, previous_app)
            .await?;
        self.storage.save(app.clone()).await?;
        Ok(app)
    }

    pub async fn persist_draft_for_app(
        &self,
        app_id: String,
        draft_id: String,
        draft_root: String,
        app: MiniApp,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniAppDraft> {
        let app = prepare_draft_app(app, compiled_html, now);
        let manifest = build_draft_manifest(app_id.clone(), draft_id, app.version, now);
        self.save_draft_with_manifest(app_id, draft_root, app, manifest)
            .await
    }

    pub async fn get_draft(
        &self,
        app_id: String,
        draft_id: String,
        draft_root: String,
    ) -> MiniAppPortResult<MiniAppDraft> {
        let app = self
            .storage
            .load_draft_app(app_id.clone(), draft_id.clone())
            .await?;
        let manifest = self.load_draft_manifest(app_id, draft_id).await?;
        Ok(build_draft_response(draft_root, app, manifest))
    }

    pub async fn persist_draft_source_sync_result(
        &self,
        draft: MiniAppDraft,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniAppDraft> {
        let app_id = draft.app_id.clone();
        let draft_root = draft.draft_root.clone();
        let mut manifest = manifest_from_draft(&draft);
        let app = apply_draft_source_sync_result(draft.app, compiled_html, now);
        manifest.updated_at = app.updated_at;
        self.save_draft_with_manifest(app_id, draft_root, app, manifest)
            .await
    }

    pub async fn persist_draft_permission_update_result(
        &self,
        draft: MiniAppDraft,
        permissions: crate::miniapp::types::MiniAppPermissions,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniAppDraft> {
        let app_id = draft.app_id.clone();
        let draft_root = draft.draft_root.clone();
        let mut manifest = manifest_from_draft(&draft);
        let app = apply_draft_permission_update_result(draft.app, permissions, compiled_html, now);
        manifest.updated_at = app.updated_at;
        self.save_draft_with_manifest(app_id, draft_root, app, manifest)
            .await
    }

    pub async fn permission_diff_for_draft(
        &self,
        app_id: String,
        draft_id: String,
    ) -> MiniAppPortResult<crate::miniapp::customization::MiniAppPermissionDiff> {
        let active = self.load_app_ensuring_runtime_state(app_id.clone()).await?;
        let draft = self.storage.load_draft_app(app_id, draft_id).await?;
        Ok(crate::miniapp::customization::diff_permissions(
            &active.permissions,
            &draft.permissions,
        ))
    }

    pub async fn apply_loaded_draft(
        &self,
        current: MiniApp,
        draft: MiniAppDraft,
        compiled_html: String,
        baseline: MiniAppCustomizationBaseline,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        self.apply_draft_app(
            current,
            draft.draft_id,
            draft.app,
            compiled_html,
            baseline,
            now,
        )
        .await
    }

    pub async fn apply_draft_app(
        &self,
        current: MiniApp,
        draft_id: String,
        draft_app: MiniApp,
        compiled_html: String,
        baseline: MiniAppCustomizationBaseline,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        let app_id = current.id.clone();
        let app = apply_draft_to_active(&current, draft_app, compiled_html, now);
        self.storage
            .save_version(app_id.clone(), current.version, current)
            .await?;
        self.storage.save(app.clone()).await?;
        self.record_draft_applied(app_id, draft_id, baseline, now)
            .await?;
        Ok(app)
    }

    pub async fn discard_draft(&self, app_id: String, draft_id: String) -> MiniAppPortResult<()> {
        self.storage.delete_draft(app_id, draft_id).await
    }

    pub async fn mark_builtin_update_available(
        &self,
        app_id: String,
        builtin_version: u32,
        source_hash: String,
        detected_at: i64,
    ) -> MiniAppPortResult<bool> {
        let Some(metadata) = self
            .storage
            .load_customization_metadata(app_id.clone())
            .await?
        else {
            return Ok(false);
        };
        let declined_update_current = self
            .has_matching_declined_builtin_update(&app_id, &metadata, &source_hash)
            .await?;
        let decision = mark_builtin_update_available_metadata(
            metadata,
            builtin_version,
            &source_hash,
            detected_at,
            declined_update_current,
        );
        if decision.metadata_changed {
            self.storage
                .save_customization_metadata(app_id, decision.metadata)
                .await?;
        }
        Ok(decision.should_surface_update)
    }

    pub async fn decline_builtin_update(
        &self,
        app_id: String,
        builtin_version: u32,
        source_hash: String,
        declined_at: i64,
    ) -> MiniAppPortResult<Option<MiniAppCustomizationMetadata>> {
        let Some(metadata) = self
            .storage
            .load_customization_metadata(app_id.clone())
            .await?
        else {
            return Ok(None);
        };
        let local_snapshot = self.storage.load(app_id.clone()).await.ok().map(|app| {
            MiniAppCustomizationLocalSnapshot {
                version: app.version,
                updated_at: app.updated_at,
            }
        });
        let metadata = decline_builtin_update_metadata(
            metadata,
            builtin_version,
            &source_hash,
            declined_at,
            local_snapshot,
        );
        self.storage
            .save_customization_metadata(app_id, metadata.clone())
            .await?;
        Ok(Some(metadata))
    }

    pub async fn mark_deps_installed(&self, app_id: String) -> MiniAppPortResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        mark_deps_installed_state(&mut app);
        self.storage.save(app.clone()).await?;
        Ok(app)
    }

    pub async fn clear_worker_restart_required(
        &self,
        app_id: String,
    ) -> MiniAppPortResult<MiniApp> {
        let mut app = self.storage.load(app_id).await?;
        if clear_worker_restart_required_state(&mut app) {
            self.storage.save(app.clone()).await?;
        }
        Ok(app)
    }

    pub async fn rollback(
        &self,
        app_id: String,
        version: u32,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        let current = self.storage.load(app_id.clone()).await?;
        let target = self.storage.load_version(app_id.clone(), version).await?;
        let app = prepare_rollback_app(&current, target, now);
        self.storage
            .save_version(app_id, current.version, current)
            .await?;
        self.storage.save(app.clone()).await?;
        Ok(app)
    }

    pub async fn persist_recompile_result(
        &self,
        app_id: String,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        let app = self.storage.load(app_id).await?;
        self.persist_recompile_result_for_app(app, compiled_html, now)
            .await
    }

    pub async fn persist_recompile_result_for_app(
        &self,
        mut app: MiniApp,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        apply_recompile_result(&mut app, compiled_html, now);
        self.storage.save(app.clone()).await?;
        Ok(app)
    }

    pub async fn persist_sync_from_fs_result(
        &self,
        app_id: String,
        source: MiniAppSource,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        let previous = self.storage.load(app_id.clone()).await?;
        self.persist_sync_from_fs_result_for_app(app_id, previous, source, compiled_html, now)
            .await
    }

    pub async fn persist_sync_from_fs_result_for_app(
        &self,
        app_id: String,
        previous: MiniApp,
        source: MiniAppSource,
        compiled_html: String,
        now: i64,
    ) -> MiniAppPortResult<MiniApp> {
        let app = apply_sync_from_fs_result(&previous, source, compiled_html, now);
        self.storage
            .save_version(app_id, previous.version, previous)
            .await?;
        self.storage.save(app.clone()).await?;
        Ok(app)
    }

    pub async fn persist_import_runtime_state(
        &self,
        mut app: MiniApp,
    ) -> MiniAppPortResult<MiniApp> {
        apply_import_runtime_state(&mut app);
        self.storage.save(app.clone()).await?;
        Ok(app)
    }

    pub async fn import_from_path(
        &self,
        import_port: &dyn MiniAppImportPort,
        compile_port: &dyn MiniAppCompilePort,
        request: MiniAppImportFromPathRequest,
    ) -> MiniAppPortResult<MiniApp> {
        let meta_content = import_port
            .read_import_meta_json(request.source_path.clone())
            .await?;
        let plan = build_import_bundle_plan(&request.app_id, &meta_content, request.imported_at)
            .map_err(map_import_bundle_plan_error)?;
        import_port
            .write_import_bundle(MiniAppImportBundleWriteRequest {
                source_path: request.source_path,
                app_id: request.app_id.clone(),
                meta_json: plan.meta_json,
                esm_dependencies_json: plan.esm_dependencies_json,
                package_json: plan.package_json,
                storage_json: plan.storage_json,
                compiled_html: plan.compiled_html,
            })
            .await?;

        let app = self.storage.load(request.app_id.clone()).await?;
        let compiled_html = compile_port
            .compile_app(
                app.id.clone(),
                app.source.clone(),
                app.permissions.clone(),
                request.theme,
                request.workspace_root,
            )
            .await?;
        let app = self
            .persist_recompile_result_for_app(app, compiled_html, request.recompiled_at)
            .await?;
        self.persist_import_runtime_state(app).await
    }

    async fn load_draft_manifest(
        &self,
        app_id: String,
        draft_id: String,
    ) -> MiniAppPortResult<MiniAppDraftManifest> {
        let value = self.storage.load_draft_manifest(app_id, draft_id).await?;
        serde_json::from_value(value).map_err(|error| {
            MiniAppPortError::new(
                MiniAppPortErrorKind::Deserialization,
                format!("Invalid draft manifest: {error}"),
            )
        })
    }

    async fn save_draft_with_manifest(
        &self,
        app_id: String,
        draft_root: String,
        app: MiniApp,
        manifest: MiniAppDraftManifest,
    ) -> MiniAppPortResult<MiniAppDraft> {
        let manifest_value = serde_json::to_value(&manifest).map_err(|error| {
            MiniAppPortError::new(
                MiniAppPortErrorKind::Deserialization,
                format!("Invalid draft manifest: {error}"),
            )
        })?;
        self.storage
            .save_draft(
                app_id,
                manifest.draft_id.clone(),
                app.clone(),
                manifest_value,
            )
            .await?;
        Ok(build_draft_response(draft_root, app, manifest))
    }

    async fn record_draft_applied(
        &self,
        app_id: String,
        draft_id: String,
        baseline: MiniAppCustomizationBaseline,
        now: i64,
    ) -> MiniAppPortResult<()> {
        let existing = self
            .storage
            .load_customization_metadata(app_id.clone())
            .await?;
        let metadata = apply_draft_customization_metadata(existing, baseline, &draft_id, now);
        self.storage
            .save_customization_metadata(app_id, metadata)
            .await
    }

    async fn has_matching_declined_builtin_update(
        &self,
        app_id: &str,
        metadata: &MiniAppCustomizationMetadata,
        source_hash: &str,
    ) -> MiniAppPortResult<bool> {
        let local_snapshot = if declined_builtin_update_needs_local_snapshot(metadata, source_hash)
        {
            self.storage.load(app_id.to_string()).await.ok().map(|app| {
                MiniAppCustomizationLocalSnapshot {
                    version: app.version,
                    updated_at: app.updated_at,
                }
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
}

fn map_import_bundle_plan_error(error: MiniAppImportBundlePlanError) -> MiniAppPortError {
    match error {
        MiniAppImportBundlePlanError::InvalidMeta(source) => MiniAppPortError::new(
            MiniAppPortErrorKind::Deserialization,
            format!("Invalid meta.json: {source}"),
        ),
        MiniAppImportBundlePlanError::MetaSerialization(source)
        | MiniAppImportBundlePlanError::PackageSerialization(source) => {
            MiniAppPortError::new(MiniAppPortErrorKind::Deserialization, source.to_string())
        }
    }
}

fn manifest_from_draft(draft: &MiniAppDraft) -> MiniAppDraftManifest {
    MiniAppDraftManifest {
        app_id: draft.app_id.clone(),
        draft_id: draft.draft_id.clone(),
        source_version: draft.source_version,
        status: draft.status.clone(),
        created_at: draft.created_at,
        updated_at: draft.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miniapp::ports::MiniAppPortFuture;
    use crate::miniapp::types::{MiniAppPermissions, MiniAppRuntimeState};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MemoryStorage {
        saved: Arc<Mutex<Vec<MiniApp>>>,
    }

    impl MiniAppStoragePort for MemoryStorage {
        fn list_app_ids(&self) -> MiniAppPortFuture<'_, Vec<String>> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load(&self, _app_id: String) -> MiniAppPortFuture<'_, MiniApp> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load_meta(&self, _app_id: String) -> MiniAppPortFuture<'_, MiniAppMeta> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load_source(&self, _app_id: String) -> MiniAppPortFuture<'_, MiniAppSource> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn save(&self, app: MiniApp) -> MiniAppPortFuture<'_, ()> {
            let saved = self.saved.clone();
            Box::pin(async move {
                saved.lock().unwrap().push(app);
                Ok(())
            })
        }

        fn save_version(
            &self,
            _app_id: String,
            _version: u32,
            _app: MiniApp,
        ) -> MiniAppPortFuture<'_, ()> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load_app_storage(&self, _app_id: String) -> MiniAppPortFuture<'_, serde_json::Value> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn save_app_storage(
            &self,
            _app_id: String,
            _key: String,
            _value: serde_json::Value,
        ) -> MiniAppPortFuture<'_, ()> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load_draft_app(
            &self,
            _app_id: String,
            _draft_id: String,
        ) -> MiniAppPortFuture<'_, MiniApp> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load_draft_manifest(
            &self,
            _app_id: String,
            _draft_id: String,
        ) -> MiniAppPortFuture<'_, serde_json::Value> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn save_draft(
            &self,
            _app_id: String,
            _draft_id: String,
            _app: MiniApp,
            _manifest: serde_json::Value,
        ) -> MiniAppPortFuture<'_, ()> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn delete_draft(&self, _app_id: String, _draft_id: String) -> MiniAppPortFuture<'_, ()> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load_customization_metadata(
            &self,
            _app_id: String,
        ) -> MiniAppPortFuture<'_, Option<MiniAppCustomizationMetadata>> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn save_customization_metadata(
            &self,
            _app_id: String,
            _metadata: MiniAppCustomizationMetadata,
        ) -> MiniAppPortFuture<'_, ()> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn delete(&self, _app_id: String) -> MiniAppPortFuture<'_, ()> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn list_versions(&self, _app_id: String) -> MiniAppPortFuture<'_, Vec<u32>> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }

        fn load_version(&self, _app_id: String, _version: u32) -> MiniAppPortFuture<'_, MiniApp> {
            Box::pin(async { unreachable!("not needed for import runtime-state facade test") })
        }
    }

    fn imported_app() -> MiniApp {
        MiniApp {
            id: "imported".to_string(),
            name: "Imported".to_string(),
            description: "Imported app".to_string(),
            icon: "box".to_string(),
            category: "utility".to_string(),
            tags: Vec::new(),
            version: 7,
            created_at: 11,
            updated_at: 12,
            source: MiniAppSource::default(),
            compiled_html: "<html></html>".to_string(),
            permissions: MiniAppPermissions::default(),
            ai_context: None,
            runtime: MiniAppRuntimeState::default(),
            i18n: None,
        }
    }

    #[test]
    fn import_runtime_state_facade_applies_state_and_persists_once() {
        let storage = MemoryStorage::default();
        let saved = storage.saved.clone();
        let facade = MiniAppRuntimeFacade::new(&storage);

        let app = block_on(facade.persist_import_runtime_state(imported_app())).unwrap();

        assert_eq!(app.runtime.source_revision, "src:7:12");
        assert_eq!(app.runtime.deps_revision, "");
        assert!(!app.runtime.deps_dirty);
        assert!(app.runtime.worker_restart_required);
        assert!(!app.runtime.ui_recompile_required);
        let saved = saved.lock().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, app.id);
        assert_eq!(
            saved[0].runtime.source_revision,
            app.runtime.source_revision
        );
        assert_eq!(saved[0].runtime.deps_revision, app.runtime.deps_revision);
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }
}
