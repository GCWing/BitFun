use super::service::{WorkspaceIdentityChangedEvent, WorkspaceService};
use crate::infrastructure::events::EventEmitter;
use crate::util::errors::*;
use log::{debug, error, info, warn};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

const IDENTITY_FILE_NAME: &str = "IDENTITY.md";
const IDENTITY_EVENT_NAME: &str = "workspace-identity-changed";
const IDENTITY_DEBOUNCE_MS: u64 = 350;

pub struct WorkspaceIdentityWatchService {
    workspace_service: Arc<WorkspaceService>,
    emitter: Arc<Mutex<Option<Arc<dyn EventEmitter>>>>,
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    watched_paths: Arc<RwLock<HashMap<PathBuf, String>>>,
    pending_refreshes: Arc<Mutex<HashMap<PathBuf, JoinHandle<()>>>>,
}

impl WorkspaceIdentityWatchService {
    pub fn new(workspace_service: Arc<WorkspaceService>) -> Self {
        Self {
            workspace_service,
            emitter: Arc::new(Mutex::new(None)),
            watcher: Arc::new(Mutex::new(None)),
            watched_paths: Arc::new(RwLock::new(HashMap::new())),
            pending_refreshes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn set_event_emitter(&self, emitter: Arc<dyn EventEmitter>) -> BitFunResult<()> {
        {
            let mut emitter_guard = self.emitter.lock().await;
            *emitter_guard = Some(emitter);
        }

        self.sync_watched_workspaces().await
    }

    pub async fn sync_watched_workspaces(&self) -> BitFunResult<()> {
        let assistant_workspaces = self.workspace_service.get_assistant_workspaces().await;
        let next_paths: HashMap<PathBuf, String> = assistant_workspaces
            .into_iter()
            .map(|workspace| (workspace.root_path, workspace.id))
            .collect();

        let next_root_set: HashSet<PathBuf> = next_paths.keys().cloned().collect();

        {
            let mut watched_paths = self.watched_paths.write().await;
            *watched_paths = next_paths;
        }

        {
            let mut pending_refreshes = self.pending_refreshes.lock().await;
            let stale_paths: Vec<PathBuf> = pending_refreshes
                .keys()
                .filter(|path| !next_root_set.contains(*path))
                .cloned()
                .collect();

            for path in stale_paths {
                if let Some(handle) = pending_refreshes.remove(&path) {
                    handle.abort();
                }
            }
        }

        self.create_watcher().await?;
        info!(
            "Workspace identity watcher synced: watched_workspace_count={}",
            next_root_set.len()
        );

        Ok(())
    }

    async fn create_watcher(&self) -> BitFunResult<()> {
        let watched_paths = self.watched_paths.read().await;

        if watched_paths.is_empty() {
            let mut watcher_guard = self.watcher.lock().await;
            *watcher_guard = None;
            return Ok(());
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).map_err(|e| {
            BitFunError::service(format!("Failed to create identity watcher: {}", e))
        })?;

        let mut watched_count = 0usize;
        for root_path in watched_paths.keys() {
            match watcher.watch(root_path, RecursiveMode::NonRecursive) {
                Ok(_) => {
                    watched_count += 1;
                }
                Err(e) => {
                    error!(
                        "Failed to watch identity directory, skipping path='{}' error={}",
                        root_path.display(),
                        e
                    );
                }
            }
        }

        if watched_count == 0 {
            return Err(BitFunError::service(
                "Failed to watch any identity directories".to_string(),
            ));
        }

        {
            let mut watcher_guard = self.watcher.lock().await;
            *watcher_guard = Some(watcher);
        }

        let workspace_service = self.workspace_service.clone();
        let emitter = self.emitter.clone();
        let watched_paths = self.watched_paths.clone();
        let pending_refreshes = self.pending_refreshes.clone();
        let runtime = tokio::runtime::Handle::current();

        tokio::task::spawn_blocking(move || loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    let affected_roots =
                        runtime.block_on(Self::extract_affected_roots(&event, &watched_paths));
                    for root_path in affected_roots {
                        runtime.block_on(Self::schedule_refresh(
                            root_path,
                            workspace_service.clone(),
                            emitter.clone(),
                            watched_paths.clone(),
                            pending_refreshes.clone(),
                        ));
                    }
                }
                Ok(Err(error)) => {
                    error!("Workspace identity watcher error: {}", error);
                }
                Err(_) => break,
            }
        });

        Ok(())
    }

    async fn extract_affected_roots(
        event: &Event,
        watched_paths: &Arc<RwLock<HashMap<PathBuf, String>>>,
    ) -> Vec<PathBuf> {
        let watched_roots = watched_paths.read().await;
        let mut affected_roots = HashSet::new();

        for path in &event.paths {
            if !Self::is_identity_path(path) {
                continue;
            }

            if let Some(parent) = path.parent() {
                if watched_roots.contains_key(parent) {
                    affected_roots.insert(parent.to_path_buf());
                }
            }
        }

        affected_roots.into_iter().collect()
    }

    fn is_identity_path(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case(IDENTITY_FILE_NAME))
            .unwrap_or(false)
    }

    async fn schedule_refresh(
        root_path: PathBuf,
        workspace_service: Arc<WorkspaceService>,
        emitter: Arc<Mutex<Option<Arc<dyn EventEmitter>>>>,
        watched_paths: Arc<RwLock<HashMap<PathBuf, String>>>,
        pending_refreshes: Arc<Mutex<HashMap<PathBuf, JoinHandle<()>>>>,
    ) {
        {
            let mut refreshes = pending_refreshes.lock().await;
            if let Some(existing_task) = refreshes.remove(&root_path) {
                existing_task.abort();
            }
        }

        let root_path_for_task = root_path.clone();
        let pending_refreshes_for_task = pending_refreshes.clone();
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(IDENTITY_DEBOUNCE_MS)).await;

            let workspace_id = {
                let watched_paths = watched_paths.read().await;
                watched_paths.get(&root_path_for_task).cloned()
            };

            let Some(workspace_id) = workspace_id else {
                return;
            };

            match workspace_service
                .refresh_workspace_identity(&workspace_id)
                .await
            {
                Ok(Some(event_payload)) => {
                    let emitter = emitter.lock().await.clone();
                    if let Some(emitter) = emitter {
                        if let Err(error) = emitter
                            .emit(
                                IDENTITY_EVENT_NAME,
                                serde_json::to_value(&event_payload).unwrap_or_default(),
                            )
                            .await
                        {
                            error!(
                                "Failed to emit workspace identity update: workspace_id={} error={}",
                                workspace_id, error
                            );
                        } else {
                            debug!(
                                "Emitted workspace identity update: workspace_id={} changed_fields={:?}",
                                workspace_id, event_payload.changed_fields
                            );
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    warn!(
                        "Failed to refresh workspace identity after file change: workspace_id={} error={}",
                        workspace_id, error
                    );
                }
            }

            let mut refreshes = pending_refreshes_for_task.lock().await;
            refreshes.remove(&root_path_for_task);
        });

        let mut refreshes = pending_refreshes.lock().await;
        refreshes.insert(root_path, handle);
    }
}

impl std::fmt::Debug for WorkspaceIdentityWatchService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceIdentityWatchService").finish()
    }
}

#[allow(dead_code)]
fn _assert_event_serializable(_event: &WorkspaceIdentityChangedEvent) {}
