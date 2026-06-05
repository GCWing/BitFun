//! MiniApp runtime/storage ports for future owner migration.
//!
//! These traits intentionally describe IO/runtime boundaries without providing
//! implementations. Core keeps the current PathManager, process, and storage
//! execution until equivalence tests cover a concrete adapter.

use crate::miniapp::lifecycle::{
    apply_import_runtime_state, apply_recompile_result, apply_sync_from_fs_result,
    clear_worker_restart_required_state, mark_deps_installed_state, prepare_rollback_app,
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

    pub async fn persist_import_runtime_state(
        &self,
        mut app: MiniApp,
    ) -> MiniAppPortResult<MiniApp> {
        apply_import_runtime_state(&mut app);
        self.storage.save(app.clone()).await?;
        Ok(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        fn load_meta(
            &self,
            _app_id: String,
        ) -> MiniAppPortFuture<'_, crate::miniapp::types::MiniAppMeta> {
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
