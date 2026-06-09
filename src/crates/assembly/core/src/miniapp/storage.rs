//! Compatibility facade for MiniApp storage.

use crate::miniapp::types::{MiniApp, MiniAppMeta, MiniAppSource};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_product_domains::miniapp::customization::MiniAppCustomizationMetadata;
use bitfun_product_domains::miniapp::ports::{
    MiniAppPortError, MiniAppPortErrorKind, MiniAppPortFuture, MiniAppStoragePort,
};
pub use bitfun_services_integrations::miniapp::storage::MiniAppImportBundleRequest;
use bitfun_services_integrations::miniapp::storage::{
    MiniAppStorage as ServiceMiniAppStorage, MiniAppStorageError, MiniAppStorageErrorKind,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// MiniApp storage service facade that preserves the historical core API.
pub struct MiniAppStorage {
    inner: ServiceMiniAppStorage,
}

impl MiniAppStorage {
    pub fn new(path_manager: Arc<crate::infrastructure::PathManager>) -> Self {
        Self {
            inner: ServiceMiniAppStorage::new(path_manager.miniapps_dir()),
        }
    }

    pub async fn ensure_app_dir(&self, app_id: &str) -> BitFunResult<()> {
        self.inner
            .ensure_app_dir(app_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn list_app_ids(&self) -> BitFunResult<Vec<String>> {
        self.inner.list_app_ids().await.map_err(map_storage_error)
    }

    pub async fn load(&self, app_id: &str) -> BitFunResult<MiniApp> {
        self.inner.load(app_id).await.map_err(map_storage_error)
    }

    pub async fn load_meta(&self, app_id: &str) -> BitFunResult<MiniAppMeta> {
        self.inner
            .load_meta(app_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn load_source_only(&self, app_id: &str) -> BitFunResult<MiniAppSource> {
        self.inner
            .load_source_only(app_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn save(&self, app: &MiniApp) -> BitFunResult<()> {
        self.inner.save(app).await.map_err(map_storage_error)
    }

    pub fn drafts_root(&self) -> PathBuf {
        self.inner.drafts_root()
    }

    pub fn app_drafts_dir(&self, app_id: &str) -> PathBuf {
        self.inner.app_drafts_dir(app_id)
    }

    pub fn draft_dir(&self, app_id: &str, draft_id: &str) -> PathBuf {
        self.inner.draft_dir(app_id, draft_id)
    }

    pub async fn save_draft(
        &self,
        app_id: &str,
        draft_id: &str,
        app: &MiniApp,
        manifest: &serde_json::Value,
    ) -> BitFunResult<()> {
        self.inner
            .save_draft(app_id, draft_id, app, manifest)
            .await
            .map_err(map_storage_error)
    }

    pub async fn load_draft_app(&self, app_id: &str, draft_id: &str) -> BitFunResult<MiniApp> {
        self.inner
            .load_draft_app(app_id, draft_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn load_draft_manifest(
        &self,
        app_id: &str,
        draft_id: &str,
    ) -> BitFunResult<serde_json::Value> {
        self.inner
            .load_draft_manifest(app_id, draft_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn delete_draft(&self, app_id: &str, draft_id: &str) -> BitFunResult<()> {
        self.inner
            .delete_draft(app_id, draft_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn mark_stale_drafts_for_cleanup(&self) -> BitFunResult<Vec<PathBuf>> {
        self.inner
            .mark_stale_drafts_for_cleanup()
            .await
            .map_err(map_storage_error)
    }

    pub async fn cleanup_marked_drafts(&self, targets: Vec<PathBuf>) -> BitFunResult<()> {
        self.inner
            .cleanup_marked_drafts(targets)
            .await
            .map_err(map_storage_error)
    }

    pub async fn save_version(
        &self,
        app_id: &str,
        version: u32,
        app: &MiniApp,
    ) -> BitFunResult<()> {
        self.inner
            .save_version(app_id, version, app)
            .await
            .map_err(map_storage_error)
    }

    pub async fn load_app_storage(&self, app_id: &str) -> BitFunResult<serde_json::Value> {
        self.inner
            .load_app_storage(app_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn load_draft_storage(
        &self,
        app_id: &str,
        draft_id: &str,
    ) -> BitFunResult<serde_json::Value> {
        self.inner
            .load_draft_storage(app_id, draft_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn save_app_storage(
        &self,
        app_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> BitFunResult<()> {
        self.inner
            .save_app_storage(app_id, key, value)
            .await
            .map_err(map_storage_error)
    }

    pub async fn save_draft_storage(
        &self,
        app_id: &str,
        draft_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> BitFunResult<()> {
        self.inner
            .save_draft_storage(app_id, draft_id, key, value)
            .await
            .map_err(map_storage_error)
    }

    pub async fn load_customization_metadata(
        &self,
        app_id: &str,
    ) -> BitFunResult<Option<MiniAppCustomizationMetadata>> {
        self.inner
            .load_customization_metadata(app_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn save_customization_metadata(
        &self,
        app_id: &str,
        metadata: &MiniAppCustomizationMetadata,
    ) -> BitFunResult<()> {
        self.inner
            .save_customization_metadata(app_id, metadata)
            .await
            .map_err(map_storage_error)
    }

    pub async fn delete(&self, app_id: &str) -> BitFunResult<()> {
        self.inner.delete(app_id).await.map_err(map_storage_error)
    }

    pub async fn list_versions(&self, app_id: &str) -> BitFunResult<Vec<u32>> {
        self.inner
            .list_versions(app_id)
            .await
            .map_err(map_storage_error)
    }

    pub async fn load_version(&self, app_id: &str, version: u32) -> BitFunResult<MiniApp> {
        self.inner
            .load_version(app_id, version)
            .await
            .map_err(map_storage_error)
    }

    pub async fn read_import_meta_json(&self, source_path: &Path) -> BitFunResult<String> {
        self.inner
            .read_import_meta_json(source_path)
            .await
            .map_err(map_storage_error)
    }

    pub async fn write_import_bundle(
        &self,
        request: MiniAppImportBundleRequest,
    ) -> BitFunResult<()> {
        self.inner
            .write_import_bundle(request)
            .await
            .map_err(map_storage_error)
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

fn map_storage_error(error: MiniAppStorageError) -> BitFunError {
    match error.kind() {
        MiniAppStorageErrorKind::NotFound => BitFunError::NotFound(error.message().to_string()),
        MiniAppStorageErrorKind::Validation => BitFunError::validation(error.message().to_string()),
        MiniAppStorageErrorKind::Deserialization => BitFunError::parse(error.message().to_string()),
        MiniAppStorageErrorKind::Io => BitFunError::io(error.message().to_string()),
        MiniAppStorageErrorKind::Backend => BitFunError::service(error.message().to_string()),
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
