//! MiniApp runtime/storage ports for future owner migration.
//!
//! These traits intentionally describe IO/runtime boundaries without providing
//! implementations. Core keeps the current PathManager, process, and storage
//! execution until equivalence tests cover a concrete adapter.

use crate::miniapp::lifecycle::{
    apply_recompile_result, apply_sync_from_fs_result, clear_worker_restart_required_state,
    mark_deps_installed_state, prepare_rollback_app,
};
use crate::miniapp::runtime::DetectedRuntime;
use crate::miniapp::types::{MiniApp, MiniAppMeta, MiniAppSource, NpmDep};
use crate::miniapp::worker::InstallResult;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub type MiniAppPortFuture<'a, T> = Pin<Box<dyn Future<Output = MiniAppPortResult<T>> + Send + 'a>>;
pub type MiniAppPortResult<T> = Result<T, MiniAppPortError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniAppPortErrorKind {
    NotFound,
    InvalidInput,
    PermissionDenied,
    RuntimeUnavailable,
    Io,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppPortError {
    pub kind: MiniAppPortErrorKind,
    pub message: String,
}

impl MiniAppPortError {
    pub fn new(kind: MiniAppPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for MiniAppPortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for MiniAppPortError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppInstallDepsRequest {
    pub app_id: String,
    #[serde(default)]
    pub dependencies: Vec<NpmDep>,
}

pub trait MiniAppStoragePort: Send + Sync {
    fn list_app_ids(&self) -> MiniAppPortFuture<'_, Vec<String>>;
    fn load(&self, app_id: String) -> MiniAppPortFuture<'_, MiniApp>;
    fn load_meta(&self, app_id: String) -> MiniAppPortFuture<'_, MiniAppMeta>;
    fn load_source(&self, app_id: String) -> MiniAppPortFuture<'_, MiniAppSource>;
    fn save(&self, app: MiniApp) -> MiniAppPortFuture<'_, ()>;
    fn save_version(&self, app_id: String, version: u32, app: MiniApp)
        -> MiniAppPortFuture<'_, ()>;
    fn load_app_storage(&self, app_id: String) -> MiniAppPortFuture<'_, serde_json::Value>;
    fn save_app_storage(
        &self,
        app_id: String,
        key: String,
        value: serde_json::Value,
    ) -> MiniAppPortFuture<'_, ()>;
    fn delete(&self, app_id: String) -> MiniAppPortFuture<'_, ()>;
    fn list_versions(&self, app_id: String) -> MiniAppPortFuture<'_, Vec<u32>>;
    fn load_version(&self, app_id: String, version: u32) -> MiniAppPortFuture<'_, MiniApp>;
}

pub trait MiniAppRuntimePort: Send + Sync {
    fn detect_runtime(&self) -> MiniAppPortFuture<'_, Option<DetectedRuntime>>;
    fn install_deps(
        &self,
        request: MiniAppInstallDepsRequest,
    ) -> MiniAppPortFuture<'_, InstallResult>;
}

/// Storage-backed facade for MiniApp runtime-state lifecycle transitions.
///
/// This keeps only portable state persistence in product-domains. Core still
/// owns compilation, filesystem reads, worker processes, host dispatch, and
/// built-in app runtime policy.
pub struct MiniAppRuntimeFacade<'a> {
    storage: &'a dyn MiniAppStoragePort,
}

impl<'a> MiniAppRuntimeFacade<'a> {
    pub fn new(storage: &'a dyn MiniAppStoragePort) -> Self {
        Self { storage }
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
}
