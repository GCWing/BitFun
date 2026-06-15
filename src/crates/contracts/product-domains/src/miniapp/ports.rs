//! MiniApp runtime/storage ports for future owner migration.
//!
//! These traits intentionally describe IO/runtime boundaries without providing
//! implementations. Core keeps the current PathManager, process, and storage
//! execution until equivalence tests cover a concrete adapter.

pub use crate::miniapp::runtime_facade::MiniAppRuntimeFacade;

use crate::miniapp::customization::MiniAppCustomizationMetadata;
use crate::miniapp::runtime::DetectedRuntime;
use crate::miniapp::storage::MiniAppImportBundleWriteRequest;
use crate::miniapp::types::{MiniApp, MiniAppMeta, MiniAppPermissions, MiniAppSource, NpmDep};
use crate::miniapp::worker::InstallResult;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

pub type MiniAppPortFuture<'a, T> = Pin<Box<dyn Future<Output = MiniAppPortResult<T>> + Send + 'a>>;
pub type MiniAppPortResult<T> = Result<T, MiniAppPortError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniAppPortErrorKind {
    NotFound,
    InvalidInput,
    Deserialization,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppImportFromPathRequest {
    pub source_path: PathBuf,
    pub app_id: String,
    pub theme: String,
    pub workspace_root: Option<PathBuf>,
    pub imported_at: i64,
    pub recompiled_at: i64,
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
    fn load_draft_app(&self, app_id: String, draft_id: String) -> MiniAppPortFuture<'_, MiniApp>;
    fn load_draft_manifest(
        &self,
        app_id: String,
        draft_id: String,
    ) -> MiniAppPortFuture<'_, serde_json::Value>;
    fn save_draft(
        &self,
        app_id: String,
        draft_id: String,
        app: MiniApp,
        manifest: serde_json::Value,
    ) -> MiniAppPortFuture<'_, ()>;
    fn delete_draft(&self, app_id: String, draft_id: String) -> MiniAppPortFuture<'_, ()>;
    fn load_customization_metadata(
        &self,
        app_id: String,
    ) -> MiniAppPortFuture<'_, Option<MiniAppCustomizationMetadata>>;
    fn save_customization_metadata(
        &self,
        app_id: String,
        metadata: MiniAppCustomizationMetadata,
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

pub trait MiniAppImportPort: Send + Sync {
    fn read_import_meta_json(&self, source_path: PathBuf) -> MiniAppPortFuture<'_, String>;
    fn write_import_bundle(
        &self,
        request: MiniAppImportBundleWriteRequest,
    ) -> MiniAppPortFuture<'_, ()>;
}

pub trait MiniAppCompilePort: Send + Sync {
    fn compile_app(
        &self,
        app_id: String,
        source: MiniAppSource,
        permissions: MiniAppPermissions,
        theme: String,
        workspace_root: Option<PathBuf>,
    ) -> MiniAppPortFuture<'_, String>;
}
