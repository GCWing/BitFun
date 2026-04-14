use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

use notify::{
    event::{ModifyKind, RemoveKind},
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};

use crate::{
    config::{BuildConfig, CorpusMode, QueryConfig},
    error::{AppError, Result},
    files::{is_workspace_internal_path, scan_paths, ScanOptions},
    index::{DirtyPathKind, IndexBuildOptions, IndexWorktreeDiff},
    path_filter::{normalize_path, PathFilter, PathFilterArgs},
    progress::{IndexProgress, IndexProgressPhase},
    search::SearchResults,
    search_engine::{SearchBackend, SearchDocument, SearchDocumentIndex, SearchEngine},
    tokenizer::TokenizerOptions,
    workspace::{IndexStatus, WorkspaceIndex, WorkspaceIndexOptions},
};

use super::{
    rg_backend::{run_rg_glob, run_rg_search},
    service::ServiceNotificationEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoPhase {
    Opening,
    MissingIndex,
    Indexing,
    ReadyClean,
    ReadyDirty,
    Rebuilding,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchConsistency {
    SnapshotOnly,
    WorkspaceEventual,
    WorkspaceStrict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryBackend {
    IndexedSnapshot,
    IndexedClean,
    IndexedWorkspaceRepair,
    RgFallback,
    ScanFallback,
}

#[derive(Debug, Clone, Default)]
pub struct DirtyStats {
    pub modified: usize,
    pub deleted: usize,
    pub new: usize,
}

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub repo_id: String,
    pub repo_path: PathBuf,
    pub index_path: PathBuf,
    pub phase: RepoPhase,
    pub snapshot_key: Option<String>,
    pub last_probe_at: Option<SystemTime>,
    pub last_rebuild_at: Option<SystemTime>,
    pub dirty_files: DirtyStats,
    pub rebuild_recommended: bool,
    pub active_task_id: Option<String>,
    pub watcher_healthy: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RepoTaskKind {
    BuildIndex,
    RebuildIndex,
    RefreshWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoTaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct RepoTaskStatus {
    pub task_id: String,
    pub repo_id: String,
    pub kind: RepoTaskKind,
    pub state: RepoTaskState,
    pub phase: Option<IndexProgressPhase>,
    pub message: String,
    pub processed: usize,
    pub total: Option<usize>,
    pub started_at: SystemTime,
    pub updated_at: SystemTime,
    pub finished_at: Option<SystemTime>,
    pub cancellable: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RepoRefreshPolicy {
    pub rebuild_dirty_threshold: usize,
}

impl Default for RepoRefreshPolicy {
    fn default() -> Self {
        Self {
            rebuild_dirty_threshold: 256,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenRepoRequest {
    pub build_config: BuildConfig,
    pub refresh_policy: RepoRefreshPolicy,
}

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub repo_id: String,
    pub query: QueryConfig,
    pub path_filter_args: PathFilterArgs,
    pub consistency: SearchConsistency,
    pub allow_scan_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub repo_id: String,
    pub backend: QueryBackend,
    pub consistency_applied: SearchConsistency,
    pub status: RepoStatus,
    pub results: SearchResults,
}

#[derive(Debug, Clone)]
pub struct GlobRequest {
    pub repo_id: String,
    pub path_filter_args: PathFilterArgs,
}

#[derive(Debug, Clone)]
pub struct GlobResponse {
    pub repo_id: String,
    pub status: RepoStatus,
    pub paths: Vec<String>,
}

pub struct RepoManager {
    repos: Mutex<HashMap<String, Arc<RepoRuntime>>>,
    next_task_id: AtomicU64,
    notifier: Arc<dyn Fn(ServiceNotificationEvent) + Send + Sync>,
}

impl RepoManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::with_notifier(Arc::new(|_event| {}))
    }

    pub fn with_notifier(notifier: Arc<dyn Fn(ServiceNotificationEvent) + Send + Sync>) -> Self {
        Self {
            repos: Mutex::new(HashMap::new()),
            next_task_id: AtomicU64::new(0),
            notifier,
        }
    }

    pub fn open_repo(&self, request: OpenRepoRequest) -> Result<RepoStatus> {
        let build_config = request.build_config.normalized()?;
        let repo_id = build_config.repo_path.to_string_lossy().into_owned();

        let runtime = {
            let mut repos = match self.repos.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(existing) = repos.get(&repo_id) {
                return existing.status_snapshot();
            }
            let runtime = Arc::new(RepoRuntime::new(
                repo_id.clone(),
                build_config,
                request.refresh_policy,
                Arc::clone(&self.notifier),
            )?);
            repos.insert(repo_id, Arc::clone(&runtime));
            runtime
        };

        RepoRuntime::start_watcher(&runtime)?;
        let task_id = self.allocate_task_id();
        let _ = runtime.start_task(task_id, RepoTaskKind::RefreshWorkspace)?;
        runtime.status_snapshot()
    }

    pub fn get_status(&self, repo_id: &str) -> Result<RepoStatus> {
        self.repo(repo_id)?.status_snapshot()
    }

    pub fn refresh_repo(&self, repo_id: &str, force: bool) -> Result<RepoStatus> {
        self.repo(repo_id)?.refresh(force)
    }

    pub fn build_index(&self, repo_id: &str) -> Result<(usize, RepoStatus)> {
        self.repo(repo_id)?.build_index()
    }

    pub fn rebuild_index(&self, repo_id: &str) -> Result<(usize, RepoStatus)> {
        self.repo(repo_id)?.rebuild_index()
    }

    pub fn start_build_index(&self, repo_id: &str) -> Result<RepoTaskStatus> {
        let runtime = self.repo(repo_id)?;
        let task_id = self.allocate_task_id();
        runtime.start_task(task_id, RepoTaskKind::BuildIndex)
    }

    pub fn start_rebuild_index(&self, repo_id: &str) -> Result<RepoTaskStatus> {
        let runtime = self.repo(repo_id)?;
        let task_id = self.allocate_task_id();
        runtime.start_task(task_id, RepoTaskKind::RebuildIndex)
    }

    pub fn get_task_status(&self, task_id: &str) -> Result<RepoTaskStatus> {
        let repos = match self.repos.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        for runtime in repos.values() {
            if let Some(task) = runtime.task_status(task_id) {
                return Ok(task);
            }
        }
        Err(AppError::Protocol(format!("unknown task_id: {task_id}")))
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<bool> {
        let repos = match self.repos.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        for runtime in repos.values() {
            if let Some(accepted) = runtime.cancel_task(task_id) {
                return Ok(accepted);
            }
        }
        Err(AppError::Protocol(format!("unknown task_id: {task_id}")))
    }

    pub fn search(&self, request: SearchRequest) -> Result<SearchResponse> {
        self.repo(&request.repo_id)?.search(request)
    }

    pub fn glob(&self, request: GlobRequest) -> Result<GlobResponse> {
        self.repo(&request.repo_id)?.glob(request)
    }

    pub fn close_repo(&self, repo_id: &str) -> Result<()> {
        let runtime = {
            let mut repos = match self.repos.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            repos.remove(repo_id)
        };
        match runtime {
            Some(runtime) => {
                runtime.shutdown();
                Ok(())
            }
            None => Err(AppError::Protocol(format!("unknown repo_id: {repo_id}"))),
        }
    }

    pub fn shutdown_all(&self) {
        let runtimes = {
            let mut repos = match self.repos.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            repos
                .drain()
                .map(|(_, runtime)| runtime)
                .collect::<Vec<_>>()
        };
        for runtime in runtimes {
            runtime.shutdown();
        }
    }

    fn repo(&self, repo_id: &str) -> Result<Arc<RepoRuntime>> {
        let repos = match self.repos.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        repos
            .get(repo_id)
            .cloned()
            .ok_or_else(|| AppError::Protocol(format!("unknown repo_id: {repo_id}")))
    }

    fn allocate_task_id(&self) -> String {
        let id = self.next_task_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("task-{id}")
    }
}

impl Drop for RepoManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}

struct RepoRuntime {
    repo_id: String,
    build_config: BuildConfig,
    search_engine: SearchEngine,
    workspace: WorkspaceIndex,
    rebuild_dirty_threshold: usize,
    notifier: Arc<dyn Fn(ServiceNotificationEvent) + Send + Sync>,
    state: Mutex<RepoState>,
    dirty_overlay: Mutex<DirtyOverlayState>,
    glob_cache: Mutex<GlobCacheState>,
    pending_directory_rescans: Mutex<BTreeMap<String, Instant>>,
    scheduled_reconcile_at: Mutex<Option<Instant>>,
    watch_event_burst: Mutex<WatchEventBurst>,
    glob_cache_epoch: AtomicU64,
    resync_required: AtomicBool,
    watcher_stop: Mutex<Option<mpsc::Sender<()>>>,
    watcher_handle: Mutex<Option<JoinHandle<()>>>,
    task_handle: Mutex<Option<ActiveTaskHandle>>,
    tasks: Mutex<HashMap<String, Arc<RepoTaskRecord>>>,
    shutting_down: AtomicBool,
}

struct ActiveTaskHandle {
    handle: JoinHandle<()>,
}

#[derive(Debug, Clone)]
struct RepoState {
    phase: RepoPhase,
    snapshot_key: Option<String>,
    last_probe_at: Option<SystemTime>,
    last_rebuild_at: Option<SystemTime>,
    dirty_files: DirtyStats,
    rebuild_recommended: bool,
    active_task_id: Option<String>,
    watcher_healthy: bool,
    last_error: Option<String>,
}

impl Default for RepoState {
    fn default() -> Self {
        Self {
            phase: RepoPhase::Opening,
            snapshot_key: None,
            last_probe_at: None,
            last_rebuild_at: None,
            dirty_files: DirtyStats::default(),
            rebuild_recommended: false,
            active_task_id: None,
            watcher_healthy: true,
            last_error: None,
        }
    }
}

struct RepoTaskRecord {
    kind: RepoTaskKind,
    cancel_requested: AtomicBool,
    state: Mutex<RepoTaskStatus>,
}

#[derive(Debug, Clone, Default)]
struct DirtyPathSet {
    modified: std::collections::HashSet<String>,
    deleted: std::collections::HashSet<String>,
    new: std::collections::HashSet<String>,
}

#[derive(Debug, Clone, Default)]
struct DirtyOverlayState {
    paths: DirtyPathSet,
    documents: HashMap<String, SearchDocument>,
    search_index: Option<Arc<SearchDocumentIndex>>,
}

#[derive(Debug, Clone)]
struct DirtyOverlaySnapshot {
    paths: DirtyPathSet,
    shadowed_base_paths: std::collections::HashSet<String>,
    search_index: Option<Arc<SearchDocumentIndex>>,
}

#[derive(Debug, Default)]
struct GlobCacheState {
    epoch: u64,
    entries: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
struct WatchEventBurst {
    window_started_at: Instant,
    event_count: usize,
}

impl Default for WatchEventBurst {
    fn default() -> Self {
        Self {
            window_started_at: Instant::now(),
            event_count: 0,
        }
    }
}

impl RepoTaskRecord {
    fn new(task_id: String, repo_id: String, kind: RepoTaskKind) -> Self {
        let now = SystemTime::now();
        Self {
            kind,
            cancel_requested: AtomicBool::new(false),
            state: Mutex::new(RepoTaskStatus {
                task_id,
                repo_id,
                kind,
                state: RepoTaskState::Queued,
                phase: None,
                message: String::new(),
                processed: 0,
                total: None,
                started_at: now,
                updated_at: now,
                finished_at: None,
                cancellable: true,
                error: None,
            }),
        }
    }

    fn snapshot(&self) -> RepoTaskStatus {
        match self.state.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn mark_running(&self) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.state = RepoTaskState::Running;
        state.updated_at = SystemTime::now();
    }

    fn update_progress(&self, progress: IndexProgress) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.state = RepoTaskState::Running;
        state.phase = Some(progress.phase);
        state.message = progress.message;
        state.processed = progress.processed;
        state.total = progress.total;
        state.updated_at = SystemTime::now();
    }

    fn finish_completed(&self, message: String) {
        let now = SystemTime::now();
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.state = RepoTaskState::Completed;
        state.message = message;
        state.updated_at = now;
        state.finished_at = Some(now);
        state.cancellable = false;
        state.error = None;
    }

    fn finish_cancelled(&self, message: String) {
        let now = SystemTime::now();
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.state = RepoTaskState::Cancelled;
        state.message = message;
        state.updated_at = now;
        state.finished_at = Some(now);
        state.cancellable = false;
        state.error = None;
    }

    fn finish_failed(&self, message: String) {
        let now = SystemTime::now();
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.state = RepoTaskState::Failed;
        state.message = message.clone();
        state.updated_at = now;
        state.finished_at = Some(now);
        state.cancellable = false;
        state.error = Some(message);
    }

    fn request_cancel(&self) -> bool {
        let was = self.cancel_requested.swap(true, Ordering::Relaxed);
        !was && self.snapshot().cancellable
    }

    fn cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::Relaxed)
    }
}

impl DirtyPathSet {
    fn from_diff(diff: IndexWorktreeDiff) -> Self {
        Self {
            modified: diff.modified_files.into_iter().collect(),
            deleted: diff.deleted_files.into_iter().collect(),
            new: diff.new_files.into_iter().collect(),
        }
    }

    fn to_diff(&self) -> IndexWorktreeDiff {
        let mut modified_files = self.modified.iter().cloned().collect::<Vec<_>>();
        modified_files.sort_unstable();
        let mut deleted_files = self.deleted.iter().cloned().collect::<Vec<_>>();
        deleted_files.sort_unstable();
        let mut new_files = self.new.iter().cloned().collect::<Vec<_>>();
        new_files.sort_unstable();
        IndexWorktreeDiff {
            modified_files,
            deleted_files,
            new_files,
        }
    }

    fn stats(&self) -> DirtyStats {
        DirtyStats {
            modified: self.modified.len(),
            deleted: self.deleted.len(),
            new: self.new.len(),
        }
    }

    fn len(&self) -> usize {
        self.modified.len() + self.deleted.len() + self.new.len()
    }

    fn is_empty(&self) -> bool {
        self.modified.is_empty() && self.deleted.is_empty() && self.new.is_empty()
    }

    fn upsert(&mut self, path: String, kind: DirtyPathKind) -> bool {
        let changed = self.remove(&path);
        match kind {
            DirtyPathKind::Modified => changed || self.modified.insert(path),
            DirtyPathKind::Deleted => changed || self.deleted.insert(path),
            DirtyPathKind::New => changed || self.new.insert(path),
        }
    }

    fn remove(&mut self, path: &str) -> bool {
        let removed_modified = self.modified.remove(path);
        let removed_deleted = self.deleted.remove(path);
        let removed_new = self.new.remove(path);
        removed_modified || removed_deleted || removed_new
    }

    fn merge(&mut self, other: DirtyPathSet) -> bool {
        let mut changed = false;
        for path in other.modified {
            changed |= self.upsert(path, DirtyPathKind::Modified);
        }
        for path in other.deleted {
            changed |= self.upsert(path, DirtyPathKind::Deleted);
        }
        for path in other.new {
            changed |= self.upsert(path, DirtyPathKind::New);
        }
        changed
    }
}

impl DirtyOverlayState {
    fn snapshot(&self) -> DirtyOverlaySnapshot {
        DirtyOverlaySnapshot {
            paths: self.paths.clone(),
            shadowed_base_paths: self
                .paths
                .modified
                .iter()
                .chain(self.paths.deleted.iter())
                .cloned()
                .collect(),
            search_index: self.search_index.clone(),
        }
    }
}

impl RepoRuntime {
    fn new(
        repo_id: String,
        build_config: BuildConfig,
        refresh_policy: RepoRefreshPolicy,
        notifier: Arc<dyn Fn(ServiceNotificationEvent) + Send + Sync>,
    ) -> Result<Self> {
        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: build_config.clone(),
        })?;
        let search_engine = SearchEngine::new(build_config.clone());
        Ok(Self {
            repo_id,
            build_config,
            search_engine,
            workspace,
            rebuild_dirty_threshold: refresh_policy.rebuild_dirty_threshold,
            notifier,
            state: Mutex::new(RepoState::default()),
            dirty_overlay: Mutex::new(DirtyOverlayState::default()),
            glob_cache: Mutex::new(GlobCacheState::default()),
            pending_directory_rescans: Mutex::new(BTreeMap::new()),
            scheduled_reconcile_at: Mutex::new(None),
            watch_event_burst: Mutex::new(WatchEventBurst::default()),
            glob_cache_epoch: AtomicU64::new(0),
            resync_required: AtomicBool::new(false),
            watcher_stop: Mutex::new(None),
            watcher_handle: Mutex::new(None),
            task_handle: Mutex::new(None),
            tasks: Mutex::new(HashMap::new()),
            shutting_down: AtomicBool::new(false),
        })
    }

    fn status_snapshot(&self) -> Result<RepoStatus> {
        let state = match self.state.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        Ok(RepoStatus {
            repo_id: self.repo_id.clone(),
            repo_path: self.build_config.repo_path.clone(),
            index_path: self.build_config.index_path.clone(),
            phase: state.phase,
            snapshot_key: state.snapshot_key,
            last_probe_at: state.last_probe_at,
            last_rebuild_at: state.last_rebuild_at,
            dirty_files: state.dirty_files,
            rebuild_recommended: state.rebuild_recommended,
            active_task_id: state.active_task_id,
            watcher_healthy: state.watcher_healthy,
            last_error: state.last_error,
        })
    }

    fn emit_workspace_status_changed(&self) {
        if let Ok(status) = self.status_snapshot() {
            (self.notifier)(ServiceNotificationEvent::WorkspaceStatus(
                super::convert::convert_repo_status(status),
            ));
        }
    }

    fn emit_task_progress(&self, task: RepoTaskStatus) {
        (self.notifier)(ServiceNotificationEvent::TaskProgress(task));
    }

    fn emit_task_finished(&self, task: RepoTaskStatus) {
        (self.notifier)(ServiceNotificationEvent::TaskFinished(task));
    }

    fn refresh(&self, force: bool) -> Result<RepoStatus> {
        if force || self.resync_required.load(Ordering::Relaxed) {
            self.refresh_overlay_from_worktree()?;
        }
        self.status_snapshot()
    }

    fn start_task(self: &Arc<Self>, task_id: String, kind: RepoTaskKind) -> Result<RepoTaskStatus> {
        self.cleanup_finished_task_handle();
        self.ensure_no_active_task()?;

        let task = Arc::new(RepoTaskRecord::new(
            task_id.clone(),
            self.repo_id.clone(),
            kind,
        ));
        {
            let mut tasks = match self.tasks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            tasks.insert(task_id.clone(), Arc::clone(&task));
        }
        self.mark_task_started(kind, Some(task_id.clone()));

        let runtime = Arc::clone(self);
        let thread_task = Arc::clone(&task);
        let handle = thread::spawn(move || {
            runtime.run_task(thread_task);
        });
        {
            let mut slot = match self.task_handle.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *slot = Some(ActiveTaskHandle { handle });
        }

        Ok(task.snapshot())
    }

    fn task_status(&self, task_id: &str) -> Option<RepoTaskStatus> {
        let tasks = match self.tasks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        tasks.get(task_id).map(|task| task.snapshot())
    }

    fn cancel_task(&self, task_id: &str) -> Option<bool> {
        let tasks = match self.tasks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        tasks.get(task_id).map(|task| task.request_cancel())
    }

    fn build_index(&self) -> Result<(usize, RepoStatus)> {
        self.ensure_no_active_task()?;
        self.set_phase(RepoPhase::Indexing, None);
        let indexed_docs = self.workspace.ensure_base_snapshot()?.doc_count;
        self.refresh_overlay_from_worktree()?;
        {
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.last_rebuild_at = Some(SystemTime::now());
            state.last_error = None;
        }
        Ok((indexed_docs, self.status_snapshot()?))
    }

    fn rebuild_index(&self) -> Result<(usize, RepoStatus)> {
        self.ensure_no_active_task()?;
        self.set_phase(RepoPhase::Rebuilding, None);
        let indexed_docs = self.search_engine.rebuild_index()?;
        self.refresh_overlay_from_worktree()?;
        {
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.last_rebuild_at = Some(SystemTime::now());
            state.last_error = None;
        }
        Ok((indexed_docs, self.status_snapshot()?))
    }

    fn search(&self, request: SearchRequest) -> Result<SearchResponse> {
        let scope =
            normalize_repo_scope_args(&self.build_config.repo_path, &request.path_filter_args)?;
        let filter = build_path_filter(&self.build_config.repo_path, &scope)?;
        match request.consistency {
            SearchConsistency::SnapshotOnly => {
                let response = self.workspace.search_snapshot_with_filter(
                    &request.query,
                    filter.as_ref(),
                    request.allow_scan_fallback,
                )?;
                let backend = match response.backend {
                    SearchBackend::Index => QueryBackend::IndexedSnapshot,
                    SearchBackend::Scan => QueryBackend::ScanFallback,
                };
                Ok(SearchResponse {
                    repo_id: self.repo_id.clone(),
                    backend,
                    consistency_applied: request.consistency,
                    status: self.status_snapshot()?,
                    results: response.results,
                })
            }
            SearchConsistency::WorkspaceEventual | SearchConsistency::WorkspaceStrict => {
                self.rescan_pending_directories()?;
                if matches!(request.consistency, SearchConsistency::WorkspaceStrict) {
                    self.refresh(true)?;
                } else if self.resync_required.load(Ordering::Relaxed) {
                    // Keep eventual searches responsive under watcher churn by
                    // reconciling in the background instead of forcing a
                    // synchronous full refresh on every query.
                    self.schedule_background_reconcile(Duration::from_millis(0));
                }

                let status = self.status_snapshot()?;
                match status.phase {
                    RepoPhase::Opening
                    | RepoPhase::MissingIndex
                    | RepoPhase::Indexing
                    | RepoPhase::Rebuilding => {
                        self.search_during_transition(&request, &scope, filter.as_ref(), status)
                    }
                    RepoPhase::ReadyClean | RepoPhase::ReadyDirty => {
                        let overlay = self.current_overlay_snapshot();
                        let backend = if overlay.paths.is_empty() {
                            QueryBackend::IndexedClean
                        } else {
                            QueryBackend::IndexedWorkspaceRepair
                        };
                        let results = if overlay.paths.is_empty() {
                            self.workspace.search_with_dirty_diff(
                                &request.query,
                                &IndexWorktreeDiff::default(),
                                filter.as_ref(),
                            )?
                        } else {
                            self.workspace.search_with_dirty_overlay(
                                &request.query,
                                &overlay.shadowed_base_paths,
                                overlay
                                    .search_index
                                    .as_deref()
                                    .expect("dirty overlay index should exist"),
                                filter.as_ref(),
                            )?
                        };
                        if !overlay.paths.is_empty() {
                            self.reconcile_dirty_paths()?;
                        }
                        Ok(SearchResponse {
                            repo_id: self.repo_id.clone(),
                            backend,
                            consistency_applied: request.consistency,
                            status: self.status_snapshot()?,
                            results,
                        })
                    }
                    RepoPhase::Degraded => Err(AppError::Protocol(
                        status
                            .last_error
                            .unwrap_or_else(|| "repo runtime is degraded".into()),
                    )),
                }
            }
        }
    }

    fn glob(&self, request: GlobRequest) -> Result<GlobResponse> {
        let scope =
            normalize_repo_scope_args(&self.build_config.repo_path, &request.path_filter_args)?;
        let filter = build_path_filter(&self.build_config.repo_path, &scope)?;
        let cache_key = glob_scope_cache_key(&scope);
        let cache_epoch = self.glob_cache_epoch.load(Ordering::Relaxed);
        if let Some(paths) = self.lookup_glob_cache(cache_epoch, &cache_key) {
            return Ok(GlobResponse {
                repo_id: self.repo_id.clone(),
                status: self.status_snapshot()?,
                paths,
            });
        }
        let paths = match run_rg_glob(&self.build_config, &scope, filter.as_ref()) {
            Ok(paths) => paths,
            Err(_) => {
                let roots = scan_roots(&self.build_config.repo_path, &scope);
                let mut paths = scan_paths(
                    &roots,
                    Some(&self.build_config.index_path),
                    ScanOptions {
                        respect_ignore: matches!(
                            self.build_config.corpus_mode,
                            CorpusMode::RespectIgnore
                        ),
                        include_hidden: self.build_config.include_hidden,
                        max_file_size: self.build_config.max_file_size,
                        max_depth: None,
                        ignore_files: Vec::new(),
                    },
                    filter.as_ref(),
                )?
                .into_iter()
                .map(|file| file.path.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
                paths.sort_unstable();
                paths
            }
        };
        if self.glob_cache_epoch.load(Ordering::Relaxed) == cache_epoch {
            self.store_glob_cache(cache_epoch, cache_key, &paths);
        }
        Ok(GlobResponse {
            repo_id: self.repo_id.clone(),
            status: self.status_snapshot()?,
            paths,
        })
    }

    fn run_task(&self, task: Arc<RepoTaskRecord>) {
        task.mark_running();

        let task_for_progress = Arc::clone(&task);
        let mut progress = move |event: IndexProgress| {
            task_for_progress.update_progress(event);
            self.emit_task_progress(task_for_progress.snapshot());
        };
        let task_for_cancel = Arc::clone(&task);
        let mut should_cancel = move || {
            task_for_cancel.cancel_requested() || self.shutting_down.load(Ordering::Relaxed)
        };

        let result = match task.kind {
            RepoTaskKind::BuildIndex => self
                .workspace
                .ensure_base_snapshot_with_options(
                    IndexBuildOptions::new()
                        .with_progress(&mut progress)
                        .with_cancel(&mut should_cancel),
                )
                .map(|info| info.doc_count),
            RepoTaskKind::RebuildIndex => self.search_engine.rebuild_index_with_options(
                IndexBuildOptions::new()
                    .with_progress(&mut progress)
                    .with_cancel(&mut should_cancel),
            ),
            RepoTaskKind::RefreshWorkspace => self.refresh_overlay_from_worktree().map(|_| 0),
        };

        match result.and_then(|indexed_docs| self.finish_task_success(task.kind, indexed_docs)) {
            Ok(message) => {
                task.finish_completed(message);
                self.set_active_task(None);
                self.emit_task_finished(task.snapshot());
                self.emit_workspace_status_changed();
            }
            Err(AppError::Cancelled) => {
                task.finish_cancelled("operation cancelled".into());
                self.set_active_task(None);
                self.emit_task_finished(task.snapshot());
                self.emit_workspace_status_changed();
            }
            Err(error) => {
                let message = error.to_string();
                task.finish_failed(message.clone());
                self.set_last_error(Some(message));
                self.set_active_task(None);
                self.emit_task_finished(task.snapshot());
                self.emit_workspace_status_changed();
            }
        }
    }

    fn finish_task_success(&self, kind: RepoTaskKind, indexed_docs: usize) -> Result<String> {
        if !matches!(kind, RepoTaskKind::RefreshWorkspace) {
            self.refresh_overlay_from_worktree()?;
        }
        {
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.last_rebuild_at = Some(SystemTime::now());
            state.last_error = None;
        }
        let message = match kind {
            RepoTaskKind::BuildIndex => format!("indexed {indexed_docs} documents"),
            RepoTaskKind::RebuildIndex => format!("rebuilt index with {indexed_docs} documents"),
            RepoTaskKind::RefreshWorkspace => "refreshed workspace".into(),
        };
        Ok(message)
    }

    fn refresh_overlay_from_worktree(&self) -> Result<()> {
        match self.workspace.status() {
            Ok(IndexStatus { base, dirty_files }) => {
                let overlay = DirtyPathSet::from_diff(dirty_files.unwrap_or_default());
                self.resync_required.store(false, Ordering::Relaxed);
                let mut state = match self.state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                state.snapshot_key = base.map(|base| base.snapshot_key);
                state.last_probe_at = Some(SystemTime::now());
                state.last_error = None;
                drop(state);
                self.replace_dirty_paths(overlay)?;
                Ok(())
            }
            Err(AppError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                self.clear_overlay();
                let mut state = match self.state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                state.snapshot_key = None;
                state.last_probe_at = Some(SystemTime::now());
                state.last_error = None;
                state.dirty_files = DirtyStats::default();
                state.rebuild_recommended = false;
                state.phase = RepoPhase::MissingIndex;
                drop(state);
                self.emit_workspace_status_changed();
                Ok(())
            }
            Err(AppError::InvalidIndex(error)) => {
                self.clear_overlay();
                let mut state = match self.state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                state.snapshot_key = None;
                state.last_probe_at = Some(SystemTime::now());
                state.last_error = Some(error);
                state.dirty_files = DirtyStats::default();
                state.rebuild_recommended = false;
                state.phase = RepoPhase::MissingIndex;
                drop(state);
                self.emit_workspace_status_changed();
                Ok(())
            }
            Err(error) => {
                self.set_phase(RepoPhase::Degraded, Some(error.to_string()));
                Err(error)
            }
        }
    }

    fn reconcile_dirty_paths(&self) -> Result<()> {
        let current = self.current_dirty_paths();
        let remaining = self.workspace.reconcile_dirty_diff(&current)?;
        self.replace_dirty_paths(DirtyPathSet::from_diff(remaining))?;
        Ok(())
    }

    fn current_dirty_paths(&self) -> IndexWorktreeDiff {
        match self.dirty_overlay.lock() {
            Ok(guard) => guard.paths.to_diff(),
            Err(poisoned) => poisoned.into_inner().paths.to_diff(),
        }
    }

    fn current_overlay_snapshot(&self) -> DirtyOverlaySnapshot {
        match self.dirty_overlay.lock() {
            Ok(guard) => guard.snapshot(),
            Err(poisoned) => poisoned.into_inner().snapshot(),
        }
    }

    fn replace_dirty_paths(&self, dirty: DirtyPathSet) -> Result<()> {
        let documents = self.load_overlay_documents(&dirty)?;
        let search_index =
            self.build_overlay_search_index(documents.values().cloned().collect::<Vec<_>>());
        {
            let mut overlay = match self.dirty_overlay.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *overlay = DirtyOverlayState {
                paths: dirty.clone(),
                documents,
                search_index: Some(search_index),
            };
        }
        self.update_overlay_state(&dirty);
        Ok(())
    }

    fn add_dirty_paths(&self, delta: DirtyPathSet) {
        let dirty = {
            let mut overlay = match self.dirty_overlay.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if !overlay.paths.merge(delta.clone()) {
                return;
            }
            if let Err(error) = self.apply_overlay_delta(&mut overlay, &delta) {
                drop(overlay);
                self.mark_resync_required();
                self.set_last_error(Some(format!("failed to refresh dirty overlay: {error}")));
                return;
            }
            self.sync_overlay_search_index_delta(&mut overlay, &delta);
            overlay.paths.clone()
        };
        self.update_overlay_state(&dirty);
    }

    fn load_overlay_documents(
        &self,
        dirty: &DirtyPathSet,
    ) -> Result<HashMap<String, SearchDocument>> {
        let mut documents = HashMap::new();
        for path in dirty.modified.iter().chain(dirty.new.iter()) {
            if let Some(document) = self.workspace.load_dirty_document(path)? {
                documents.insert(path.clone(), document);
            }
        }
        Ok(documents)
    }

    fn apply_overlay_delta(
        &self,
        overlay: &mut DirtyOverlayState,
        delta: &DirtyPathSet,
    ) -> Result<()> {
        for path in &delta.deleted {
            overlay.documents.remove(path);
        }
        for path in delta.modified.iter().chain(delta.new.iter()) {
            match self.workspace.load_dirty_document(path)? {
                Some(document) => {
                    overlay.documents.insert(path.clone(), document);
                }
                None => {
                    overlay.documents.remove(path);
                }
            }
        }
        Ok(())
    }

    fn update_overlay_state(&self, dirty: &DirtyPathSet) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let dirty_len = dirty.len();
        state.dirty_files = dirty.stats();
        state.rebuild_recommended = dirty_len >= self.rebuild_dirty_threshold;
        if state.snapshot_key.is_none() {
            state.phase = RepoPhase::MissingIndex;
        } else if dirty.is_empty() && !self.resync_required.load(Ordering::Relaxed) {
            state.phase = RepoPhase::ReadyClean;
        } else {
            state.phase = RepoPhase::ReadyDirty;
        }
        drop(state);
        self.invalidate_glob_cache();
        self.emit_workspace_status_changed();
    }

    fn clear_overlay(&self) {
        {
            let mut overlay = match self.dirty_overlay.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *overlay = DirtyOverlayState::default();
        }
        {
            let mut pending = match self.pending_directory_rescans.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            pending.clear();
        }
        {
            let mut scheduled = match self.scheduled_reconcile_at.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *scheduled = None;
        }
        self.invalidate_glob_cache();
        self.resync_required.store(false, Ordering::Relaxed);
    }

    fn build_overlay_search_index(
        &self,
        mut documents: Vec<SearchDocument>,
    ) -> Arc<SearchDocumentIndex> {
        documents.sort_unstable_by(|left, right| left.logical_path.cmp(&right.logical_path));
        Arc::new(SearchDocumentIndex::build(
            self.build_config.tokenizer,
            TokenizerOptions {
                min_sparse_len: self.build_config.min_sparse_len,
                max_sparse_len: self.build_config.max_sparse_len,
            },
            documents,
        ))
    }

    fn sync_overlay_search_index_delta(
        &self,
        overlay: &mut DirtyOverlayState,
        delta: &DirtyPathSet,
    ) {
        let search_index = overlay
            .search_index
            .get_or_insert_with(|| self.build_overlay_search_index(Vec::new()));
        let index = Arc::make_mut(search_index);

        for path in &delta.deleted {
            index.remove_document(path);
        }
        for path in delta.modified.iter().chain(delta.new.iter()) {
            match overlay.documents.get(path).cloned() {
                Some(document) => index.upsert_document(document),
                None => index.remove_document(path),
            }
        }
    }

    fn start_watcher(runtime: &Arc<Self>) -> Result<()> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        {
            let mut stop = match runtime.watcher_stop.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *stop = Some(stop_tx);
        }

        let thread_runtime = Arc::clone(runtime);
        let handle = thread::spawn(move || {
            let (event_tx, event_rx) = mpsc::channel();
            let mut watcher = match RecommendedWatcher::new(
                move |result| {
                    let _ = event_tx.send(result);
                },
                Config::default(),
            ) {
                Ok(watcher) => watcher,
                Err(error) => {
                    let _ = ready_tx.send(Err(format!("failed to start file watcher: {error}")));
                    thread_runtime.set_watcher_healthy(false);
                    thread_runtime.set_phase(
                        RepoPhase::Degraded,
                        Some(format!("failed to start file watcher: {error}")),
                    );
                    return;
                }
            };

            if let Err(error) = watcher.watch(
                &thread_runtime.build_config.repo_path,
                RecursiveMode::Recursive,
            ) {
                let _ = ready_tx.send(Err(format!("failed to watch repository: {error}")));
                thread_runtime.set_watcher_healthy(false);
                thread_runtime.set_phase(
                    RepoPhase::Degraded,
                    Some(format!("failed to watch repository: {error}")),
                );
                return;
            }
            let _ = ready_tx.send(Ok(()));

            while !thread_runtime.shutting_down.load(Ordering::Relaxed) {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match event_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(Ok(event)) => {
                        thread_runtime.handle_watch_event(event);
                        thread_runtime.run_scheduled_reconcile_if_due();
                    }
                    Ok(Err(error)) => {
                        thread_runtime.set_watcher_healthy(false);
                        thread_runtime.mark_resync_required();
                        thread_runtime.set_last_error(Some(format!("watch error: {error}")));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        thread_runtime.run_scheduled_reconcile_if_due();
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        let mut worker = match runtime.watcher_handle.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *worker = Some(handle);
        drop(worker);

        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(AppError::Protocol(message)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(AppError::Protocol(
                "timed out waiting for file watcher to start".into(),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(AppError::Protocol(
                "file watcher stopped before reporting startup status".into(),
            )),
        }
    }

    fn handle_watch_event(&self, event: Event) {
        let event_path_count = event.paths.len().max(1);
        let mut dirty = DirtyPathSet::default();
        let is_remove_file = matches!(event.kind, EventKind::Remove(RemoveKind::File));
        let is_remove_folder = matches!(event.kind, EventKind::Remove(RemoveKind::Folder));
        let mut needs_resync = matches!(
            event.kind,
            EventKind::Any
                | EventKind::Other
                | EventKind::Access(_)
                | EventKind::Modify(ModifyKind::Name(_))
                | EventKind::Remove(RemoveKind::Folder)
        );

        for path in event.paths {
            let candidate = if path.is_absolute() {
                path
            } else {
                self.build_config.repo_path.join(path)
            };
            if is_workspace_internal_path(&candidate) {
                continue;
            }
            if self.is_index_path(&candidate) {
                continue;
            }
            match self.normalize_watch_path(&candidate) {
                Some(normalized) => {
                    if !candidate.exists()
                        && (self.discard_overlay_new_path(&normalized)
                            || self.discard_overlay_new_paths_under_directory(&normalized))
                    {
                        needs_resync = false;
                        self.clear_pending_directory_rescan(&normalized);
                        continue;
                    }
                    if is_remove_file && self.discard_overlay_new_path(&normalized) {
                        needs_resync = false;
                        self.clear_pending_directory_rescan(&normalized);
                        continue;
                    }
                    if is_remove_folder
                        && self.discard_overlay_new_paths_under_directory(&normalized)
                    {
                        needs_resync = false;
                        self.clear_pending_directory_rescan(&normalized);
                        continue;
                    }
                    if is_remove_folder
                        && !candidate.exists()
                        && matches!(
                            self.workspace.has_indexed_path_under(&normalized),
                            Ok(false)
                        )
                    {
                        needs_resync = false;
                        self.clear_pending_directory_rescan(&normalized);
                        continue;
                    }
                    if candidate.exists() && candidate.is_dir() {
                        match self.collect_directory_dirty_paths(&candidate) {
                            Ok(directory_dirty) => {
                                if !directory_dirty.is_empty() {
                                    self.clear_pending_directory_rescan(&normalized);
                                    dirty.merge(directory_dirty);
                                } else {
                                    self.note_pending_directory_rescan(normalized.clone());
                                }
                            }
                            Err(_) => needs_resync = true,
                        }
                        continue;
                    }
                    match self.classify_watch_paths([normalized]) {
                        Ok(classified) => {
                            dirty.merge(classified);
                        }
                        Err(_) => needs_resync = true,
                    }
                }
                None => {
                    needs_resync = true;
                }
            }
        }

        if self.note_watch_event_burst(event_path_count) {
            needs_resync = true;
        }

        if !dirty.is_empty() {
            self.add_dirty_paths(dirty);
        }
        if needs_resync {
            self.mark_resync_required();
        }
    }

    fn note_pending_directory_rescan(&self, directory: String) {
        let mut pending = match self.pending_directory_rescans.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        pending.entry(directory).or_insert_with(Instant::now);
        drop(pending);
        self.schedule_background_reconcile(Duration::from_millis(750));
    }

    fn clear_pending_directory_rescan(&self, path: &str) {
        let mut pending = match self.pending_directory_rescans.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        pending.retain(|directory, _| {
            directory != path
                && !directory.starts_with(&format!("{path}{}", std::path::MAIN_SEPARATOR))
        });
    }

    fn rescan_pending_directories(&self) -> Result<()> {
        const PENDING_RESCAN_TTL: Duration = Duration::from_secs(5);

        let directories = {
            let pending = match self.pending_directory_rescans.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            pending
                .iter()
                .map(|(directory, first_seen)| (directory.clone(), *first_seen))
                .collect::<Vec<_>>()
        };
        if directories.is_empty() {
            return Ok(());
        }

        let mut resolved = Vec::new();
        let mut should_resync = false;
        for (directory, first_seen) in directories {
            let candidate = PathBuf::from(&directory);
            if !candidate.exists() || !candidate.is_dir() {
                resolved.push(directory);
                continue;
            }
            match self.collect_directory_dirty_paths(&candidate) {
                Ok(directory_dirty) => {
                    if !directory_dirty.is_empty() {
                        self.add_dirty_paths(directory_dirty);
                        resolved.push(directory);
                    } else if first_seen.elapsed() >= PENDING_RESCAN_TTL {
                        resolved.push(directory);
                    }
                }
                Err(_) => {
                    resolved.push(directory);
                    should_resync = true;
                }
            }
        }

        if !resolved.is_empty() {
            let mut pending = match self.pending_directory_rescans.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            for directory in resolved {
                pending.remove(&directory);
            }
        }
        if should_resync {
            self.mark_resync_required();
        }
        Ok(())
    }

    fn note_watch_event_burst(&self, event_count: usize) -> bool {
        const WATCH_EVENT_BURST_WINDOW: Duration = Duration::from_millis(750);
        const WATCH_EVENT_BURST_THRESHOLD: usize = 64;

        let now = Instant::now();
        let mut burst = match self.watch_event_burst.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if now.duration_since(burst.window_started_at) > WATCH_EVENT_BURST_WINDOW {
            burst.window_started_at = now;
            burst.event_count = event_count;
        } else {
            burst.event_count = burst.event_count.saturating_add(event_count);
        }

        let exceeded = burst.event_count >= WATCH_EVENT_BURST_THRESHOLD;
        if exceeded {
            burst.window_started_at = now;
            burst.event_count = 0;
        }
        drop(burst);

        if exceeded {
            self.schedule_background_reconcile(Duration::from_millis(750));
        }
        exceeded
    }

    fn schedule_background_reconcile(&self, delay: Duration) {
        let due_at = Instant::now() + delay;
        let mut scheduled = match self.scheduled_reconcile_at.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match *scheduled {
            Some(existing) if existing <= due_at => {}
            _ => *scheduled = Some(due_at),
        }
    }

    fn run_scheduled_reconcile_if_due(&self) {
        let due_at = {
            let scheduled = match self.scheduled_reconcile_at.lock() {
                Ok(guard) => *guard,
                Err(poisoned) => *poisoned.into_inner(),
            };
            scheduled
        };
        let Some(due_at) = due_at else {
            return;
        };
        if Instant::now() < due_at {
            return;
        }
        if self.has_active_task() {
            self.schedule_background_reconcile(Duration::from_secs(1));
            return;
        }

        {
            let mut scheduled = match self.scheduled_reconcile_at.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *scheduled = None;
        }

        let result = self.rescan_pending_directories().and_then(|_| {
            if self.resync_required.load(Ordering::Relaxed) {
                self.refresh_overlay_from_worktree()
            } else {
                Ok(())
            }
        });
        if let Err(error) = result {
            self.set_last_error(Some(format!(
                "background workspace reconcile failed: {error}"
            )));
            self.schedule_background_reconcile(Duration::from_secs(1));
        }
    }

    fn has_active_task(&self) -> bool {
        match self.state.lock() {
            Ok(guard) => guard.active_task_id.is_some(),
            Err(poisoned) => poisoned.into_inner().active_task_id.is_some(),
        }
    }

    fn discard_overlay_new_path(&self, path: &str) -> bool {
        self.discard_overlay_new_paths_matching(|candidate| candidate == path)
    }

    fn discard_overlay_new_paths_under_directory(&self, directory: &str) -> bool {
        let prefix = format!("{directory}{}", std::path::MAIN_SEPARATOR);
        self.discard_overlay_new_paths_matching(|candidate| {
            candidate == directory || candidate.starts_with(&prefix)
        })
    }

    fn discard_overlay_new_paths_matching<F>(&self, mut matches: F) -> bool
    where
        F: FnMut(&str) -> bool,
    {
        let dirty = {
            let mut overlay = match self.dirty_overlay.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let removed = overlay
                .paths
                .new
                .iter()
                .filter(|path| matches(path))
                .cloned()
                .collect::<Vec<_>>();
            if removed.is_empty() {
                return false;
            }

            if let Some(search_index) = &mut overlay.search_index {
                let index = Arc::make_mut(search_index);
                for path in &removed {
                    index.remove_document(path);
                }
            }
            for path in &removed {
                overlay.paths.remove(path);
                overlay.documents.remove(path);
            }
            overlay.paths.clone()
        };
        self.update_overlay_state(&dirty);
        true
    }

    fn collect_directory_dirty_paths(&self, directory: &Path) -> Result<DirtyPathSet> {
        let scanned = scan_paths(
            &[directory.to_path_buf()],
            Some(&self.build_config.index_path),
            ScanOptions {
                respect_ignore: matches!(self.build_config.corpus_mode, CorpusMode::RespectIgnore),
                include_hidden: self.build_config.include_hidden,
                max_file_size: self.build_config.max_file_size,
                max_depth: None,
                ignore_files: Vec::new(),
            },
            None,
        )?;

        let mut paths = Vec::new();
        for file in scanned {
            if is_workspace_internal_path(&file.path) {
                continue;
            }
            if self.is_index_path(&file.path) {
                continue;
            }
            let Some(normalized) = self.normalize_watch_path(&file.path) else {
                continue;
            };
            paths.push(normalized);
        }
        self.classify_watch_paths(paths)
    }

    fn classify_watch_paths<I>(&self, paths: I) -> Result<DirtyPathSet>
    where
        I: IntoIterator<Item = String>,
    {
        let diff = self.workspace.classify_dirty_paths(paths)?;
        Ok(DirtyPathSet::from_diff(diff))
    }

    fn normalize_watch_path(&self, path: &Path) -> Option<String> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.build_config.repo_path.join(path)
        };
        let candidate = self.normalize_repo_watch_path(absolute)?;
        if !candidate.starts_with(&self.build_config.repo_path) {
            return None;
        }
        Some(candidate.to_string_lossy().into_owned())
    }

    fn normalize_repo_watch_path(&self, absolute: PathBuf) -> Option<PathBuf> {
        if absolute.starts_with(&self.build_config.repo_path) {
            return Some(absolute);
        }
        if let Ok(canonical) = std::fs::canonicalize(&absolute) {
            if canonical.starts_with(&self.build_config.repo_path) {
                return Some(canonical);
            }
        }
        let parent = absolute.parent()?;
        let file_name = absolute.file_name()?;
        let canonical_parent = std::fs::canonicalize(parent).ok()?;
        let adjusted = canonical_parent.join(file_name);
        adjusted
            .starts_with(&self.build_config.repo_path)
            .then_some(adjusted)
    }

    fn is_index_path(&self, path: &Path) -> bool {
        self.normalize_repo_watch_path(path.to_path_buf())
            .is_some_and(|candidate| candidate.starts_with(self.normalized_index_path()))
    }

    fn normalized_index_path(&self) -> PathBuf {
        if let Ok(canonical) = std::fs::canonicalize(&self.build_config.index_path) {
            return canonical;
        }
        let Some(parent) = self.build_config.index_path.parent() else {
            return self.build_config.index_path.clone();
        };
        let Some(file_name) = self.build_config.index_path.file_name() else {
            return self.build_config.index_path.clone();
        };
        match std::fs::canonicalize(parent) {
            Ok(canonical_parent) => canonical_parent.join(file_name),
            Err(_) => self.build_config.index_path.clone(),
        }
    }

    fn ensure_no_active_task(&self) -> Result<()> {
        self.cleanup_finished_task_handle();
        let active_task_id = match self.state.lock() {
            Ok(guard) => guard.active_task_id.clone(),
            Err(poisoned) => poisoned.into_inner().active_task_id.clone(),
        };
        if let Some(task_id) = active_task_id {
            if let Some(task) = self.task_status(&task_id) {
                if matches!(task.state, RepoTaskState::Queued | RepoTaskState::Running) {
                    return Err(AppError::Protocol(format!(
                        "repo {} already has a running task: {task_id}",
                        self.repo_id
                    )));
                }
            }
        }
        Ok(())
    }

    fn cleanup_finished_task_handle(&self) {
        let maybe_handle = {
            let mut slot = match self.task_handle.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let Some(active) = slot.as_ref() else {
                return;
            };
            if !active.handle.is_finished() {
                return;
            }
            slot.take()
        };
        if let Some(active) = maybe_handle {
            let _ = active.handle.join();
        }
    }

    fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::Relaxed);
        if let Some(task_id) = match self.state.lock() {
            Ok(guard) => guard.active_task_id.clone(),
            Err(poisoned) => poisoned.into_inner().active_task_id.clone(),
        } {
            let _ = self.cancel_task(&task_id);
        }
        if let Some(stop) = match self.watcher_stop.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        } {
            let _ = stop.send(());
        }
        if let Some(handle) = match self.watcher_handle.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        } {
            let _ = handle.join();
        }
        if let Some(active) = match self.task_handle.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        } {
            let _ = active.handle.join();
        }
    }

    fn set_phase(&self, phase: RepoPhase, last_error: Option<String>) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.phase = phase;
        state.last_error = last_error;
        drop(state);
        self.emit_workspace_status_changed();
    }

    fn set_last_error(&self, last_error: Option<String>) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.last_error = last_error;
        drop(state);
        self.emit_workspace_status_changed();
    }

    fn set_active_task(&self, active_task_id: Option<String>) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.active_task_id = active_task_id;
    }

    fn set_watcher_healthy(&self, watcher_healthy: bool) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.watcher_healthy = watcher_healthy;
        drop(state);
        self.emit_workspace_status_changed();
    }

    fn mark_resync_required(&self) {
        self.resync_required.store(true, Ordering::Relaxed);
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if state.snapshot_key.is_some() {
            state.phase = RepoPhase::ReadyDirty;
        }
        drop(state);
        self.schedule_background_reconcile(Duration::from_millis(750));
        self.invalidate_glob_cache();
        self.emit_workspace_status_changed();
    }

    fn invalidate_glob_cache(&self) {
        self.glob_cache_epoch.fetch_add(1, Ordering::Relaxed);
    }

    fn lookup_glob_cache(&self, epoch: u64, key: &str) -> Option<Vec<String>> {
        let mut cache = match self.glob_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.epoch != epoch {
            cache.epoch = epoch;
            cache.entries.clear();
            return None;
        }
        cache.entries.get(key).cloned()
    }

    fn store_glob_cache(&self, epoch: u64, key: String, paths: &[String]) {
        const MAX_GLOB_CACHE_ENTRIES: usize = 32;

        let mut cache = match self.glob_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.epoch != epoch {
            cache.epoch = epoch;
            cache.entries.clear();
        }
        if !cache.entries.contains_key(&key) && cache.entries.len() >= MAX_GLOB_CACHE_ENTRIES {
            cache.entries.clear();
        }
        cache.entries.insert(key, paths.to_vec());
    }

    fn mark_task_started(&self, kind: RepoTaskKind, active_task_id: Option<String>) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.active_task_id = active_task_id;
        state.last_error = None;
        state.phase = match kind {
            RepoTaskKind::BuildIndex => RepoPhase::Indexing,
            RepoTaskKind::RebuildIndex => RepoPhase::Rebuilding,
            RepoTaskKind::RefreshWorkspace => RepoPhase::Opening,
        };
        drop(state);
        self.emit_workspace_status_changed();
    }

    fn search_during_transition(
        &self,
        request: &SearchRequest,
        scope: &PathFilterArgs,
        filter: Option<&PathFilter>,
        status: RepoStatus,
    ) -> Result<SearchResponse> {
        if matches!(request.consistency, SearchConsistency::WorkspaceStrict) {
            return Err(AppError::Protocol(format!(
                "repo {} is not ready yet ({})",
                self.repo_id,
                repo_phase_label(status.phase)
            )));
        }

        match run_rg_search(&self.build_config, &request.query, scope) {
            Ok(results) => Ok(SearchResponse {
                repo_id: self.repo_id.clone(),
                backend: QueryBackend::RgFallback,
                consistency_applied: request.consistency,
                status: self.status_snapshot()?,
                results,
            }),
            Err(_) => Ok(SearchResponse {
                repo_id: self.repo_id.clone(),
                backend: QueryBackend::ScanFallback,
                consistency_applied: request.consistency,
                status: self.status_snapshot()?,
                results: self
                    .workspace
                    .scan_search_with_filter(&request.query, filter)?,
            }),
        }
    }
}

fn repo_phase_label(phase: RepoPhase) -> &'static str {
    match phase {
        RepoPhase::Opening => "opening",
        RepoPhase::MissingIndex => "missing_index",
        RepoPhase::Indexing => "indexing",
        RepoPhase::ReadyClean => "ready_clean",
        RepoPhase::ReadyDirty => "ready_dirty",
        RepoPhase::Rebuilding => "rebuilding",
        RepoPhase::Degraded => "degraded",
    }
}

fn build_path_filter(repo_root: &PathBuf, args: &PathFilterArgs) -> Result<Option<PathFilter>> {
    let has_scope = !args.roots.is_empty()
        || !args.globs.is_empty()
        || !args.iglobs.is_empty()
        || !args.type_add.is_empty()
        || !args.type_clear.is_empty()
        || !args.types.is_empty()
        || !args.type_not.is_empty();
    if !has_scope {
        return Ok(None);
    }
    Ok(Some(PathFilter::new(args.clone(), repo_root)?))
}

fn normalize_repo_scope_args(repo_root: &Path, args: &PathFilterArgs) -> Result<PathFilterArgs> {
    let mut normalized = args.clone();
    normalized.roots = args
        .roots
        .iter()
        .map(|root| {
            let normalized = normalize_path(root, repo_root);
            if normalized == *repo_root || normalized.starts_with(repo_root) {
                Ok(normalized)
            } else {
                Err(AppError::Protocol(format!(
                    "scope root escapes repo boundary: {}",
                    normalized.to_string_lossy()
                )))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(normalized)
}

fn glob_scope_cache_key(args: &PathFilterArgs) -> String {
    fn append_path_list(key: &mut String, values: &[PathBuf]) {
        for value in values {
            key.push('\u{1f}');
            key.push_str(&value.to_string_lossy());
        }
    }

    fn append_string_list(key: &mut String, values: &[String]) {
        for value in values {
            key.push('\u{1f}');
            key.push_str(value);
        }
    }

    let mut key = String::new();
    append_path_list(&mut key, &args.roots);
    key.push('\u{1e}');
    append_string_list(&mut key, &args.globs);
    key.push('\u{1e}');
    append_string_list(&mut key, &args.iglobs);
    key.push('\u{1e}');
    append_string_list(&mut key, &args.type_add);
    key.push('\u{1e}');
    append_string_list(&mut key, &args.type_clear);
    key.push('\u{1e}');
    append_string_list(&mut key, &args.types);
    key.push('\u{1e}');
    append_string_list(&mut key, &args.type_not);
    key
}

fn scan_roots(repo_root: &Path, args: &PathFilterArgs) -> Vec<PathBuf> {
    if args.roots.is_empty() {
        return vec![repo_root.to_path_buf()];
    }
    args.roots
        .iter()
        .map(|root| normalize_path(root, repo_root))
        .collect()
}
