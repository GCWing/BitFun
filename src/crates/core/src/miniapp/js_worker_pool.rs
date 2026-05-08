//! JS Worker pool — LRU pool, get_or_spawn, call, stop_all, install_deps.

use crate::miniapp::js_worker::JsWorker;
use crate::miniapp::runtime_detect::{detect_runtime, DetectedRuntime};
use crate::miniapp::types::{NodePermissions, NpmDep};
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_WORKERS: usize = 5;
const IDLE_TIMEOUT_MS: i64 = 3 * 60 * 1000; // 3 minutes

/// Result of npm/bun install.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

struct WorkerEntry {
    revision: String,
    worker: Arc<Mutex<JsWorker>>,
}

pub struct JsWorkerPool {
    workers: Arc<Mutex<std::collections::HashMap<String, WorkerEntry>>>,
    runtime: DetectedRuntime,
    worker_host_path: PathBuf,
    path_manager: Arc<crate::infrastructure::PathManager>,
}

impl JsWorkerPool {
    pub fn new(
        path_manager: Arc<crate::infrastructure::PathManager>,
        worker_host_path: PathBuf,
    ) -> BitFunResult<Self> {
        let runtime = detect_runtime().ok_or_else(|| {
            BitFunError::validation("No JS runtime found (install Bun or Node.js)".to_string())
        })?;
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
                            now - worker.last_activity_ms() > IDLE_TIMEOUT_MS
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

        Ok(Self {
            workers,
            runtime,
            worker_host_path,
            path_manager,
        })
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
    ) -> BitFunResult<Arc<Mutex<JsWorker>>> {
        let mut guard = self.workers.lock().await;
        self.evict_idle(&mut guard).await;

        if let Some(entry) = guard.remove(app_id) {
            if entry.revision == worker_revision {
                let worker = Arc::clone(&entry.worker);
                guard.insert(app_id.to_string(), entry);
                return Ok(worker);
            }
            let mut stale = entry.worker.lock().await;
            stale.kill().await;
        }

        if guard.len() >= MAX_WORKERS {
            self.evict_lru(&mut guard).await;
        }

        let app_dir = self.path_manager.miniapp_dir(app_id);
        if !app_dir.exists() {
            return Err(BitFunError::NotFound(format!(
                "MiniApp dir not found: {}",
                app_id
            )));
        }

        let worker = JsWorker::spawn(
            &self.runtime,
            &self.worker_host_path,
            &app_dir,
            policy_json,
            app_id.to_string(),
        )
        .await
        .map_err(BitFunError::validation)?;

        let _timeout_ms = node_perms.and_then(|n| n.timeout_ms).unwrap_or(30_000);
        let worker = Arc::new(Mutex::new(worker));
        guard.insert(
            app_id.to_string(),
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
                    now - worker.last_activity_ms() > IDLE_TIMEOUT_MS
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
        let (oldest_id, _) = guard
            .iter()
            .map(|(id, entry)| {
                let activity = entry
                    .worker
                    .try_lock()
                    .map(|worker| worker.last_activity_ms())
                    .unwrap_or(0);
                (id.clone(), activity)
            })
            .min_by_key(|(_, a)| *a)
            .unwrap_or((String::new(), 0));
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
    ) -> BitFunResult<Value> {
        let worker = self
            .get_or_spawn(app_id, worker_revision, policy_json, permissions)
            .await?;
        let timeout_ms = permissions.and_then(|n| n.timeout_ms).unwrap_or(30_000);
        let guard = worker.lock().await;
        guard
            .call(method, params, timeout_ms)
            .await
            .map_err(BitFunError::validation)
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
        self.path_manager
            .miniapp_dir(app_id)
            .join("node_modules")
            .exists()
    }

    /// Install npm dependencies for the app (bun install or npm/pnpm install).
    pub async fn install_deps(
        &self,
        app_id: &str,
        _deps: &[NpmDep],
    ) -> BitFunResult<InstallResult> {
        let app_dir = self.path_manager.miniapp_dir(app_id);
        let package_json = app_dir.join("package.json");
        if !package_json.exists() {
            return Ok(InstallResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            });
        }

        let (cmd, args): (&str, &[&str]) = match self.runtime.kind {
            crate::miniapp::runtime_detect::RuntimeKind::Bun => {
                ("bun", &["install", "--production"][..])
            }
            crate::miniapp::runtime_detect::RuntimeKind::Node => {
                if which::which("pnpm").is_ok() {
                    ("pnpm", &["install", "--prod"][..])
                } else {
                    ("npm", &["install", "--production"][..])
                }
            }
        };

        let output = crate::util::process_manager::create_tokio_command(cmd)
            .args(args)
            .current_dir(&app_dir)
            .output()
            .await
            .map_err(|e| BitFunError::io(format!("install_deps failed: {}", e)))?;

        Ok(InstallResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
