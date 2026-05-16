//! MiniApp runtime/storage ports for future owner migration.
//!
//! These traits intentionally describe IO/runtime boundaries without providing
//! implementations. Core keeps the current PathManager, process, and storage
//! execution until equivalence tests cover a concrete adapter.

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
