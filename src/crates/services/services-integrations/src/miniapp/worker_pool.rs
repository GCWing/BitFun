//! JS worker pool: LRU pool, get_or_spawn, call, stop_all, install_deps.

use crate::miniapp::worker::{JsWorker, SharedMiniAppWorkerEventSink};
use bitfun_product_domains::miniapp::ports::{
    MiniAppInstallDepsRequest, MiniAppPortError, MiniAppPortErrorKind, MiniAppPortFuture,
    MiniAppRuntimePort,
};
use bitfun_product_domains::miniapp::runtime::{detect_runtime, DetectedRuntime};
use bitfun_product_domains::miniapp::types::{NodePermissions, NpmDep};
pub use bitfun_product_domains::miniapp::worker::InstallResult;
use bitfun_product_domains::miniapp::worker::{
    plan_install_deps, select_lru_worker, worker_is_idle, worker_pool_at_capacity, InstallDepsPlan,
};
use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniAppWorkerPoolErrorKind {
    NotFound,
    Validation,
    Io,
    RuntimeUnavailable,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppWorkerPoolError {
    kind: MiniAppWorkerPoolErrorKind,
    message: String,
}

impl MiniAppWorkerPoolError {
    pub fn new(kind: MiniAppWorkerPoolErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(MiniAppWorkerPoolErrorKind::Validation, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(MiniAppWorkerPoolErrorKind::NotFound, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(MiniAppWorkerPoolErrorKind::Io, message)
    }

    pub fn kind(&self) -> MiniAppWorkerPoolErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for MiniAppWorkerPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MiniAppWorkerPoolError {}

pub type MiniAppWorkerPoolResult<T> = Result<T, MiniAppWorkerPoolError>;

struct WorkerEntry {
    revision: String,
    worker: Arc<Mutex<JsWorker>>,
}

fn spawn_worker_reaper() -> Arc<Mutex<std::collections::HashMap<String, WorkerEntry>>> {
    let workers = Arc::new(Mutex::new(
        std::collections::HashMap::<String, WorkerEntry>::new(),
    ));

    // Background task: evict idle workers every 60s without waiting for a new spawn.
    let workers_bg = Arc::clone(&workers);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await; // skip first immediate tick
        loop {
            interval.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let mut guard = workers_bg.lock().await;
            let to_remove: Vec<String> = guard
                .iter()
                .filter(|(_, entry)| {
                    if let Ok(worker) = entry.worker.try_lock() {
                        worker_is_idle(now, worker.last_activity_ms())
                    } else {
                        false
                    }
                })
                .map(|(k, _)| k.clone())
                .collect();
            for id in to_remove {
                if let Some(entry) = guard.remove(&id) {
                    let mut w = entry.worker.lock().await;
                    w.kill().await;
                }
            }
        }
    });

    workers
}

pub struct JsWorkerPool {
    workers: Arc<Mutex<std::collections::HashMap<String, WorkerEntry>>>,
    runtime: DetectedRuntime,
    worker_host_path: PathBuf,
    miniapps_dir: PathBuf,
    event_sink: Option<SharedMiniAppWorkerEventSink>,
}

impl JsWorkerPool {
    pub fn new(
        miniapps_dir: PathBuf,
        worker_host_path: PathBuf,
        event_sink: Option<SharedMiniAppWorkerEventSink>,
    ) -> MiniAppWorkerPoolResult<Self> {
        let runtime = detect_runtime().ok_or_else(|| {
            MiniAppWorkerPoolError::validation(
                "No JS runtime found (install Bun or Node.js)".to_string(),
            )
        })?;
        Ok(Self::from_runtime(
            miniapps_dir,
            worker_host_path,
            runtime,
            event_sink,
        ))
    }

    pub fn from_runtime(
        miniapps_dir: PathBuf,
        worker_host_path: PathBuf,
        runtime: DetectedRuntime,
        event_sink: Option<SharedMiniAppWorkerEventSink>,
    ) -> Self {
        let workers = spawn_worker_reaper();

        Self {
            workers,
            runtime,
            worker_host_path,
            miniapps_dir,
            event_sink,
        }
    }

    fn miniapp_dir(&self, app_id: &str) -> PathBuf {
        self.miniapps_dir.join(app_id)
    }

    pub fn runtime_info(&self) -> &DetectedRuntime {
        &self.runtime
    }

    /// Get or spawn a Worker for the app. policy_json is the resolved permission policy JSON string.
    pub async fn get_or_spawn(
        &self,
        app_id: &str,
        worker_revision: &str,
        policy_json: &str,
        node_perms: Option<&NodePermissions>,
    ) -> MiniAppWorkerPoolResult<Arc<Mutex<JsWorker>>> {
        let app_dir = self.miniapp_dir(app_id);
        self.get_or_spawn_with_app_dir(
            app_id,
            app_id,
            &app_dir,
            worker_revision,
            policy_json,
            node_perms,
        )
        .await
    }

    pub async fn get_or_spawn_with_app_dir(
        &self,
        worker_key: &str,
        app_id: &str,
        app_dir: &Path,
        worker_revision: &str,
        policy_json: &str,
        node_perms: Option<&NodePermissions>,
    ) -> MiniAppWorkerPoolResult<Arc<Mutex<JsWorker>>> {
        let mut guard = self.workers.lock().await;
        self.evict_idle(&mut guard).await;

        if let Some(entry) = guard.remove(worker_key) {
            if entry.revision == worker_revision {
                let worker = Arc::clone(&entry.worker);
                guard.insert(worker_key.to_string(), entry);
                return Ok(worker);
            }
            let mut stale = entry.worker.lock().await;
            stale.kill().await;
        }

        if worker_pool_at_capacity(guard.len()) {
            self.evict_lru(&mut guard).await;
        }

        if !app_dir.exists() {
            return Err(MiniAppWorkerPoolError::not_found(format!(
                "MiniApp worker dir not found: {}",
                app_dir.display()
            )));
        }

        let worker = JsWorker::spawn(
            &self.runtime,
            &self.worker_host_path,
            &app_dir,
            policy_json,
            app_id.to_string(),
            self.event_sink.clone(),
        )
        .await
        .map_err(MiniAppWorkerPoolError::validation)?;

        let _timeout_ms = node_perms.and_then(|n| n.timeout_ms).unwrap_or(30_000);
        let worker = Arc::new(Mutex::new(worker));
        guard.insert(
            worker_key.to_string(),
            WorkerEntry {
                revision: worker_revision.to_string(),
                worker: Arc::clone(&worker),
            },
        );
        Ok(worker)
    }

    async fn evict_idle(&self, guard: &mut std::collections::HashMap<String, WorkerEntry>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let to_remove: Vec<String> = guard
            .iter()
            .filter(|(_, entry)| {
                let w = entry.worker.try_lock();
                if let Ok(worker) = w {
                    worker_is_idle(now, worker.last_activity_ms())
                } else {
                    false
                }
            })
            .map(|(k, _)| k.clone())
            .collect();
        for id in to_remove {
            if let Some(entry) = guard.remove(&id) {
                let mut w = entry.worker.lock().await;
                w.kill().await;
            }
        }
    }

    async fn evict_lru(&self, guard: &mut std::collections::HashMap<String, WorkerEntry>) {
        let oldest_id = select_lru_worker(guard.iter().map(|(id, entry)| {
            let activity = entry
                .worker
                .try_lock()
                .map(|worker| worker.last_activity_ms())
                .unwrap_or(0);
            (id.as_str(), activity)
        }))
        .unwrap_or_default();
        if !oldest_id.is_empty() {
            if let Some(entry) = guard.remove(&oldest_id) {
                let mut w = entry.worker.lock().await;
                w.kill().await;
            }
        }
    }

    /// Call a method on the app's Worker. Spawns the worker if needed; caller must provide policy_json.
    pub async fn call(
        &self,
        app_id: &str,
        worker_revision: &str,
        policy_json: &str,
        permissions: Option<&NodePermissions>,
        method: &str,
        params: Value,
    ) -> MiniAppWorkerPoolResult<Value> {
        let worker = self
            .get_or_spawn(app_id, worker_revision, policy_json, permissions)
            .await?;
        let timeout_ms = permissions.and_then(|n| n.timeout_ms).unwrap_or(30_000);
        let guard = worker.lock().await;
        guard
            .call(method, params, timeout_ms)
            .await
            .map_err(MiniAppWorkerPoolError::validation)
    }

    pub async fn call_with_app_dir(
        &self,
        worker_key: &str,
        app_id: &str,
        app_dir: &Path,
        worker_revision: &str,
        policy_json: &str,
        permissions: Option<&NodePermissions>,
        method: &str,
        params: Value,
    ) -> MiniAppWorkerPoolResult<Value> {
        let worker = self
            .get_or_spawn_with_app_dir(
                worker_key,
                app_id,
                app_dir,
                worker_revision,
                policy_json,
                permissions,
            )
            .await?;
        let timeout_ms = permissions.and_then(|n| n.timeout_ms).unwrap_or(30_000);
        let guard = worker.lock().await;
        guard
            .call(method, params, timeout_ms)
            .await
            .map_err(MiniAppWorkerPoolError::validation)
    }

    /// Stop and remove the Worker for the app.
    pub async fn stop(&self, app_id: &str) {
        let mut guard = self.workers.lock().await;
        if let Some(entry) = guard.remove(app_id) {
            let mut w = entry.worker.lock().await;
            w.kill().await;
        }
    }

    /// Return app IDs of currently running Workers.
    pub async fn list_running(&self) -> Vec<String> {
        let guard = self.workers.lock().await;
        guard.keys().cloned().collect()
    }

    pub async fn is_running(&self, app_id: &str) -> bool {
        let guard = self.workers.lock().await;
        guard.contains_key(app_id)
    }

    /// Stop all Workers.
    pub async fn stop_all(&self) {
        let mut guard = self.workers.lock().await;
        for (_, entry) in guard.drain() {
            let mut w = entry.worker.lock().await;
            w.kill().await;
        }
    }

    pub fn has_installed_deps(&self, app_id: &str) -> bool {
        self.miniapp_dir(app_id).join("node_modules").exists()
    }

    pub fn has_installed_deps_in_dir(&self, app_dir: &Path) -> bool {
        app_dir.join("node_modules").exists()
    }

    /// Install npm dependencies for the app (bun install or npm/pnpm install).
    pub async fn install_deps(
        &self,
        app_id: &str,
        _deps: &[NpmDep],
    ) -> MiniAppWorkerPoolResult<InstallResult> {
        let app_dir = self.miniapp_dir(app_id);
        self.install_deps_in_dir(&app_dir, _deps).await
    }

    pub async fn install_deps_in_dir(
        &self,
        app_dir: &Path,
        _deps: &[NpmDep],
    ) -> MiniAppWorkerPoolResult<InstallResult> {
        let package_json = app_dir.join("package.json");
        let command = match plan_install_deps(
            package_json.exists(),
            &self.runtime.kind,
            which::which("pnpm").is_ok(),
        ) {
            InstallDepsPlan::SkipMissingPackageJson => {
                return Ok(InstallResult {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                });
            }
            InstallDepsPlan::Run(command) => command,
        };

        let output = bitfun_services_core::process_manager::create_tokio_command(command.program)
            .args(command.args)
            .current_dir(&app_dir)
            .output()
            .await
            .map_err(|e| MiniAppWorkerPoolError::io(format!("install_deps failed: {}", e)))?;

        Ok(InstallResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

impl MiniAppRuntimePort for JsWorkerPool {
    fn detect_runtime(&self) -> MiniAppPortFuture<'_, Option<DetectedRuntime>> {
        Box::pin(async move { Ok(Some(self.runtime.clone())) })
    }

    fn install_deps(
        &self,
        request: MiniAppInstallDepsRequest,
    ) -> MiniAppPortFuture<'_, InstallResult> {
        Box::pin(async move {
            self.install_deps(&request.app_id, &request.dependencies)
                .await
                .map_err(map_miniapp_runtime_port_error)
        })
    }
}

fn map_miniapp_runtime_port_error(error: MiniAppWorkerPoolError) -> MiniAppPortError {
    let kind = match error.kind() {
        MiniAppWorkerPoolErrorKind::NotFound => MiniAppPortErrorKind::NotFound,
        MiniAppWorkerPoolErrorKind::Validation => MiniAppPortErrorKind::InvalidInput,
        MiniAppWorkerPoolErrorKind::Io => MiniAppPortErrorKind::Io,
        MiniAppWorkerPoolErrorKind::RuntimeUnavailable => MiniAppPortErrorKind::RuntimeUnavailable,
        MiniAppWorkerPoolErrorKind::Backend => MiniAppPortErrorKind::Backend,
    };
    MiniAppPortError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::miniapp::runtime::RuntimeKind;
    use std::fs;
    use std::path::{Path, PathBuf};

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
    async fn runtime_port_adapter_preserves_existing_runtime_and_noop_install() {
        let root = TestTempDir::new("bitfun-miniapp-runtime-port");
        let miniapps_dir = root.path().join("miniapps");
        let app_id = "demo_app";
        tokio::fs::create_dir_all(miniapps_dir.join(app_id))
            .await
            .unwrap();
        let pool = JsWorkerPool::from_runtime(
            miniapps_dir,
            PathBuf::from("worker-host.js"),
            DetectedRuntime {
                kind: RuntimeKind::Node,
                path: PathBuf::from("node"),
                version: "v20.0.0".to_string(),
            },
            None,
        );
        let port: &dyn MiniAppRuntimePort = &pool;

        let runtime = port.detect_runtime().await.unwrap().unwrap();
        assert_eq!(runtime.kind, RuntimeKind::Node);
        assert_eq!(runtime.version, "v20.0.0");

        let result = port
            .install_deps(MiniAppInstallDepsRequest {
                app_id: app_id.to_string(),
                dependencies: vec![NpmDep {
                    name: "lodash".to_string(),
                    version: "^4.17.21".to_string(),
                }],
            })
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn install_deps_in_dir_noops_without_package_json() {
        let root = TestTempDir::new("bitfun-miniapp-runtime-draft-port");
        let miniapps_dir = root.path().join("miniapps");
        let draft_dir = miniapps_dir
            .join(".drafts")
            .join("demo_app")
            .join("draft_1");
        tokio::fs::create_dir_all(&draft_dir).await.unwrap();
        let pool = JsWorkerPool::from_runtime(
            miniapps_dir,
            PathBuf::from("worker-host.js"),
            DetectedRuntime {
                kind: RuntimeKind::Node,
                path: PathBuf::from("node"),
                version: "v20.0.0".to_string(),
            },
            None,
        );

        let result = pool
            .install_deps_in_dir(
                &draft_dir,
                &[NpmDep {
                    name: "lodash".to_string(),
                    version: "^4.17.21".to_string(),
                }],
            )
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }
}
