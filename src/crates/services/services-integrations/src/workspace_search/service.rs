use super::flashgrep::{
    FlashgrepRepoSession, GlobRequest, ManagedClient, OpenRepoParams, PathScope, QuerySpec,
    RefreshPolicyConfig, RepoConfig, RepoSession, SearchRequest, FLASHGREP_LOG_TARGET,
};
use async_trait::async_trait;
use bitfun_services_core::filesystem::{ContentMatchPreviewBuilder, FileSearchOutcome};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

use super::line_hydration::{hydrate_grouped_line_matches, HydratedLineMatches};
use super::result_mapping::convert_search_results;
use super::types::{
    ContentSearchOutputMode, ContentSearchRequest, ContentSearchResult, GlobSearchRequest,
    GlobSearchResult, IndexTaskHandle, WorkspaceIndexStatus, WorkspaceSearchAutoIndexDecision,
    WorkspaceSearchAutoIndexStatus, WorkspaceSearchFileCount,
};
use super::{
    auto_index::{self, AutoIndexDecision, AutoIndexPolicy},
    index_budget,
    index_queue::{AutoIndexQueue, WorkspaceSearchAutoIndexPriority},
};

pub type WorkspaceSearchResult<T> = Result<T, String>;

#[derive(Debug, Clone)]
pub struct WorkspaceSearchRepoConfig {
    pub max_file_size: u64,
}

impl Default for WorkspaceSearchRepoConfig {
    fn default() -> Self {
        let default = RepoConfig::default();
        Self {
            max_file_size: default.max_file_size,
        }
    }
}

impl From<WorkspaceSearchRepoConfig> for RepoConfig {
    fn from(value: WorkspaceSearchRepoConfig) -> Self {
        let default = RepoConfig::default();
        RepoConfig {
            max_file_size: value.max_file_size,
            ..default
        }
    }
}

#[async_trait]
pub trait WorkspaceSearchRuntimeHooks: Send + Sync {
    async fn repo_config(&self) -> WorkspaceSearchRepoConfig;

    async fn ensure_workspace_ready(&self, _repo_root: &Path) -> WorkspaceSearchResult<()> {
        Ok(())
    }
}

struct DefaultWorkspaceSearchRuntimeHooks;

#[async_trait]
impl WorkspaceSearchRuntimeHooks for DefaultWorkspaceSearchRuntimeHooks {
    async fn repo_config(&self) -> WorkspaceSearchRepoConfig {
        WorkspaceSearchRepoConfig::default()
    }
}

const DEFAULT_TOP_K_TOKENS: usize = 6;
const DEFAULT_SESSION_IDLE_GRACE: Duration = Duration::from_secs(45);

#[derive(Debug, Clone)]
struct SessionEntry {
    session: Arc<RepoSession>,
    activity_epoch: Arc<AtomicU64>,
}

pub struct WorkspaceSearchService {
    client: ManagedClient,
    sessions: RwLock<HashMap<PathBuf, SessionEntry>>,
    open_guards: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
    auto_index_queue: Arc<AutoIndexQueue>,
    /// Last auto-index policy outcome per workspace root. The daemon reports `needs_index`
    /// whether the policy is still evaluating or has declined, so the reason is recorded here
    /// and surfaced with the status instead of only being logged.
    auto_index_decisions: RwLock<HashMap<PathBuf, WorkspaceSearchAutoIndexStatus>>,
    /// Workspace roots last handed to budget maintenance, kept so that a build
    /// finishing later can re-enforce the budget against the same set.
    index_budget_roots: RwLock<Vec<PathBuf>>,
    session_idle_grace: Duration,
    hooks: Arc<dyn WorkspaceSearchRuntimeHooks>,
}

impl WorkspaceSearchService {
    pub fn new() -> Self {
        Self::new_with_hooks(Arc::new(DefaultWorkspaceSearchRuntimeHooks))
    }

    pub fn new_with_hooks(hooks: Arc<dyn WorkspaceSearchRuntimeHooks>) -> Self {
        let mut client = ManagedClient::new()
            .with_start_timeout(Duration::from_secs(10))
            .with_retry_interval(Duration::from_millis(100));
        let program = resolve_daemon_program();
        if let Some(program) = program {
            log::info!(
                target: FLASHGREP_LOG_TARGET,
                "WorkspaceSearchService daemon configured: program={}",
                PathBuf::from(&program).display()
            );
            client = client.with_daemon_program(program);
        } else {
            log::info!(
                target: FLASHGREP_LOG_TARGET,
                "WorkspaceSearchService daemon configured: program=flashgrep"
            );
        }

        Self {
            client,
            sessions: RwLock::new(HashMap::new()),
            open_guards: Mutex::new(HashMap::new()),
            auto_index_queue: Arc::new(AutoIndexQueue::default()),
            auto_index_decisions: RwLock::new(HashMap::new()),
            index_budget_roots: RwLock::new(Vec::new()),
            session_idle_grace: DEFAULT_SESSION_IDLE_GRACE,
            hooks,
        }
    }

    pub async fn open_repo(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> WorkspaceSearchResult<WorkspaceIndexStatus> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        let mut status = self.index_status_for_session(session).await?;
        status.auto_index = Some(self.auto_index_status(repo_root.as_ref()).await);
        Ok(status)
    }

    pub async fn get_index_status(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> WorkspaceSearchResult<WorkspaceIndexStatus> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        let mut status = self.index_status_for_session(session).await?;
        status.auto_index = Some(self.auto_index_status(repo_root.as_ref()).await);
        Ok(status)
    }

    /// Reports the recorded auto-index decision, defaulting to `Pending` for a workspace the
    /// policy has not reached yet (the queue runs asynchronously behind workspace activation).
    async fn auto_index_status(&self, repo_root: &Path) -> WorkspaceSearchAutoIndexStatus {
        let pending = WorkspaceSearchAutoIndexStatus {
            decision: WorkspaceSearchAutoIndexDecision::Pending,
            threshold: auto_index::DEFAULT_AUTO_INDEX_MIN_FILES,
            indexable_files: None,
            reason: None,
        };
        let Ok(repo_root) = normalize_repo_root(repo_root) else {
            return pending;
        };
        self.auto_index_decisions
            .read()
            .await
            .get(&repo_root)
            .cloned()
            .unwrap_or(pending)
    }

    async fn record_auto_index_decision(
        &self,
        repo_root: &Path,
        status: WorkspaceSearchAutoIndexStatus,
    ) {
        self.auto_index_decisions
            .write()
            .await
            .insert(repo_root.to_path_buf(), status);
    }

    pub async fn build_index(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> WorkspaceSearchResult<IndexTaskHandle> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        let task = FlashgrepRepoSession::build_index(session.as_ref())
            .await
            .map_err(map_flashgrep_error("Failed to start index build"))?;
        let repo_status = session
            .status()
            .await
            .map_err(map_flashgrep_error("Failed to fetch repository status"))?;
        log::info!(
            target: FLASHGREP_LOG_TARGET,
            "Workspace search build index requested: repo_root={}, task_id={}, phase={:?}",
            repo_root.as_ref().display(),
            task.task_id,
            repo_status.phase
        );
        Ok(IndexTaskHandle {
            task: task.into(),
            repo_status: repo_status.into(),
        })
    }

    pub async fn rebuild_index(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> WorkspaceSearchResult<IndexTaskHandle> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        let task = FlashgrepRepoSession::rebuild_index(session.as_ref())
            .await
            .map_err(map_flashgrep_error("Failed to start index rebuild"))?;
        let repo_status = session
            .status()
            .await
            .map_err(map_flashgrep_error("Failed to fetch repository status"))?;
        log::info!(
            target: FLASHGREP_LOG_TARGET,
            "Workspace search rebuild index requested: repo_root={}, task_id={}, phase={:?}",
            repo_root.as_ref().display(),
            task.task_id,
            repo_status.phase
        );
        Ok(IndexTaskHandle {
            task: task.into(),
            repo_status: repo_status.into(),
        })
    }

    pub async fn search_content(
        &self,
        request: ContentSearchRequest,
    ) -> WorkspaceSearchResult<ContentSearchResult> {
        let started_at = Instant::now();
        let pattern_for_log = abbreviate_pattern_for_log(&request.pattern);
        let repo_root = normalize_repo_root(&request.repo_root)?;
        let normalized_at = Instant::now();
        let scope = build_scope(
            &repo_root,
            request.search_path.as_deref(),
            request.globs,
            request.file_types,
            request.exclude_file_types,
        )?;
        let scope_built_at = Instant::now();
        let scope_roots_count = scope.roots.len();
        let scope_globs_count = scope.globs.len();
        let scope_types_count = scope.types.len();
        let max_results = request.max_results.filter(|limit| *limit > 0);
        let preview_spec = ContentPreviewSpec {
            pattern: request.pattern.clone(),
            case_sensitive: request.case_sensitive,
            use_regex: request.use_regex,
            whole_word: request.whole_word,
        };
        let query = QuerySpec {
            pattern: request.pattern,
            patterns: Vec::new(),
            case_insensitive: !request.case_sensitive,
            multiline: request.multiline,
            dot_matches_new_line: request.multiline,
            fixed_strings: !request.use_regex,
            word_regexp: request.whole_word,
            line_regexp: false,
            top_k_tokens: DEFAULT_TOP_K_TOKENS,
            max_count: None,
            global_max_results: max_results,
            search_mode: request.output_mode.search_mode(),
        };

        let session = self.get_or_open_session(&repo_root).await?;
        let session_ready_at = Instant::now();
        let search_request = SearchRequest::new(query).with_scope(scope);

        let (result, search_completed_at, converted_at) = match request.output_mode {
            // Content output needs the line text, which no daemon search mode
            // returns, so it takes the grouped variant and hydrates from disk.
            ContentSearchOutputMode::Content => {
                let grouped = session
                    .search_grouped_line_matches(search_request)
                    .await
                    .map_err(map_flashgrep_error("Content search failed"))?;
                let search_completed_at = Instant::now();

                let hydrated = hydrate_grouped_content_matches(
                    grouped.results.files,
                    max_results,
                    preview_spec,
                )
                .await?;
                let converted_at = Instant::now();

                if hydrated.unreadable_files > 0 {
                    log::debug!(
                        target: FLASHGREP_LOG_TARGET,
                        "Workspace content search could not read {} matched file(s); reporting their matches without line text: repo_root={}",
                        hydrated.unreadable_files,
                        repo_root.display()
                    );
                }

                let truncated = grouped.results.limit_reached || hydrated.dropped_lines > 0;
                let result = ContentSearchResult {
                    outcome: FileSearchOutcome {
                        results: hydrated.results,
                        truncated,
                    },
                    // `search/grouped_line_matches` reports per-file counts only
                    // for its top-10 summary, which is not the full list the
                    // count output mode promises, so it is left to that mode.
                    file_counts: Vec::new(),
                    hits: Vec::new(),
                    backend: grouped.backend.into(),
                    repo_status: grouped.status.into(),
                    candidate_docs: grouped.results.candidate_docs,
                    matched_lines: grouped.results.matched_lines,
                    matched_occurrences: grouped.results.matched_occurrences,
                };
                (result, search_completed_at, converted_at)
            }
            ContentSearchOutputMode::Count | ContentSearchOutputMode::FilesWithMatches => {
                let search = FlashgrepRepoSession::search(session.as_ref(), search_request)
                    .await
                    .map_err(map_flashgrep_error("Content search failed"))?;
                let search_completed_at = Instant::now();

                let mut results = convert_search_results(&search.results, request.output_mode);
                let converted_at = Instant::now();
                let truncated = max_results
                    .map(|limit| results.len() >= limit)
                    .unwrap_or(false);
                if let Some(limit) = max_results {
                    results.truncate(limit);
                }

                let result = ContentSearchResult {
                    outcome: FileSearchOutcome { results, truncated },
                    file_counts: search
                        .results
                        .file_counts
                        .clone()
                        .into_iter()
                        .map(WorkspaceSearchFileCount::from)
                        .collect(),
                    hits: Vec::new(),
                    backend: search.backend.into(),
                    repo_status: search.status.into(),
                    candidate_docs: search.results.candidate_docs,
                    matched_lines: search.results.matched_lines,
                    matched_occurrences: search.results.matched_occurrences,
                };
                (result, search_completed_at, converted_at)
            }
        };

        log::debug!(
            target: FLASHGREP_LOG_TARGET,
            "Workspace content search completed: repo_root={}, pattern={}, output_mode={:?}, search_mode={:?}, scope_roots={}, globs={}, file_types={}, max_results={:?}, backend={:?}, repo_phase={:?}, base_advance_in_progress={}, workspace_probe_pending={}, dirty_modified={}, dirty_deleted={}, dirty_new={}, candidate_docs={}, matched_lines={}, matched_occurrences={}, returned_results={}, truncated={}, normalize_ms={}, build_scope_ms={}, session_ms={}, search_ms={}, convert_ms={}, total_ms={}",
            repo_root.display(),
            pattern_for_log,
            request.output_mode,
            request.output_mode.search_mode(),
            scope_roots_count,
            scope_globs_count,
            scope_types_count,
            max_results,
            result.backend,
            result.repo_status.phase,
            result.repo_status.base_advance_in_progress,
            result.repo_status.workspace_probe_pending,
            result.repo_status.dirty_files.modified,
            result.repo_status.dirty_files.deleted,
            result.repo_status.dirty_files.new,
            result.candidate_docs,
            result.matched_lines,
            result.matched_occurrences,
            result.outcome.results.len(),
            result.outcome.truncated,
            normalized_at.duration_since(started_at).as_millis(),
            scope_built_at.duration_since(normalized_at).as_millis(),
            session_ready_at.duration_since(scope_built_at).as_millis(),
            search_completed_at.duration_since(session_ready_at).as_millis(),
            converted_at.duration_since(search_completed_at).as_millis(),
            converted_at.duration_since(started_at).as_millis(),
        );

        Ok(result)
    }

    pub async fn glob(
        &self,
        request: GlobSearchRequest,
    ) -> WorkspaceSearchResult<GlobSearchResult> {
        let repo_root = normalize_repo_root(&request.repo_root)?;
        let search_path = request.search_path.as_deref().unwrap_or(&repo_root);
        let normalized_search_path = normalize_scope_path(&repo_root, search_path)?;
        let (walk_root, pattern) = derive_glob_walk_root(&normalized_search_path, &request.pattern);
        if !walk_root.is_dir() {
            let session = self.get_or_open_session(&repo_root).await?;
            let repo_status = session
                .status()
                .await
                .map_err(map_flashgrep_error("Glob status failed"))?;
            return Ok(GlobSearchResult {
                paths: Vec::new(),
                matches_relative_to: path_to_string(&walk_root),
                total_matches: Some(0),
                truncated: false,
                repo_status: repo_status.into(),
            });
        }
        let scope = build_scope(&repo_root, Some(&walk_root), vec![pattern], vec![], vec![])?;
        let session = self.get_or_open_session(&repo_root).await?;
        let outcome =
            FlashgrepRepoSession::glob(session.as_ref(), GlobRequest::new().with_scope(scope))
                .await
                .map_err(map_flashgrep_error("Glob search failed"))?;
        let mut paths = outcome
            .paths
            .into_iter()
            .map(|path| relativize_glob_result_path(&repo_root, &walk_root, &path))
            .collect::<Vec<_>>();
        paths.sort();
        let total_matches = paths.len();
        if request.limit > 0 {
            paths.truncate(request.limit);
        } else {
            paths.clear();
        }

        Ok(GlobSearchResult {
            paths,
            matches_relative_to: path_to_string(&walk_root),
            total_matches: Some(total_matches),
            truncated: total_matches > request.limit,
            repo_status: outcome.status.into(),
        })
    }

    pub fn schedule_repo_release(self: &Arc<Self>, repo_root: impl AsRef<Path>) {
        let Ok(repo_root) = normalize_repo_root(repo_root.as_ref()) else {
            return;
        };
        let delay = self.session_idle_grace;
        let service = Arc::downgrade(self);
        tokio::spawn(async move {
            let Some(current_service) = service.upgrade() else {
                return;
            };
            let Some(expected_epoch) = current_service
                .sessions
                .read()
                .await
                .get(&repo_root)
                .map(|entry| entry.activity_epoch.load(Ordering::Relaxed))
            else {
                return;
            };
            drop(current_service);
            release_repo_after_delay(service, repo_root, expected_epoch, delay).await;
        });
    }

    /// Queue an automatic base-index build without blocking workspace activation or search.
    pub async fn schedule_auto_index(
        self: &Arc<Self>,
        repo_root: impl AsRef<Path>,
        priority: WorkspaceSearchAutoIndexPriority,
    ) {
        let Ok(repo_root) = normalize_repo_root(repo_root.as_ref()) else {
            return;
        };
        if self.auto_index_queue.enqueue(repo_root, priority).await {
            let service = Arc::clone(self);
            tokio::spawn(async move {
                service.run_auto_index_queue().await;
            });
        }
    }

    /// Reclaim old local workspace indexes without touching active or queued workspaces.
    pub async fn enforce_index_disk_budget(
        &self,
        workspace_roots: Vec<PathBuf>,
        protected_roots: Vec<PathBuf>,
    ) {
        let workspace_roots = workspace_roots
            .into_iter()
            .filter_map(|path| normalize_repo_root(&path).ok())
            .collect::<Vec<_>>();
        let protected = protected_roots
            .into_iter()
            .filter_map(|path| normalize_repo_root(&path).ok())
            .collect::<HashSet<_>>();
        *self.index_budget_roots.write().await = workspace_roots.clone();
        self.enforce_normalized_index_disk_budget(workspace_roots, protected)
            .await;
    }

    /// Re-run budget maintenance against the roots cached by the last
    /// `enforce_index_disk_budget` call.
    ///
    /// Automatic index builds are the only thing that grows on-disk index size,
    /// so they are the trigger; a workspace switch on its own changes nothing
    /// and would only pay for a recursive size walk of every known index.
    async fn enforce_cached_index_disk_budget(&self) {
        let workspace_roots = self.index_budget_roots.read().await.clone();
        if workspace_roots.is_empty() {
            return;
        }
        // Caller-supplied protection (the focused workspace) is not cached: it
        // already holds an open session, and live sessions are unioned below.
        self.enforce_normalized_index_disk_budget(workspace_roots, HashSet::new())
            .await;
    }

    async fn enforce_normalized_index_disk_budget(
        &self,
        workspace_roots: Vec<PathBuf>,
        protected_roots: HashSet<PathBuf>,
    ) {
        let mut protected = protected_roots;
        protected.extend(self.sessions.read().await.keys().cloned());
        protected.extend(self.auto_index_queue.protected_roots().await);

        let result =
            tokio::task::spawn_blocking(move || index_budget::enforce(workspace_roots, protected))
                .await;
        match result {
            Ok(Ok(report)) => {
                if report.removed.is_empty() && !report.over_budget {
                    return;
                }
                log::info!(
                    target: FLASHGREP_LOG_TARGET,
                    "Workspace search index budget maintenance completed: total_before_bytes={}, total_after_bytes={}, removed={}, over_budget={}",
                    report.total_before,
                    report.total_after,
                    report.removed.len(),
                    report.over_budget
                );
            }
            Ok(Err(error)) => log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search index budget maintenance failed: {}",
                error
            ),
            Err(error) => log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search index budget worker failed: {}",
                error
            ),
        }
    }

    pub async fn remove_workspace_index(&self, repo_root: impl AsRef<Path>) {
        let Ok(repo_root) = normalize_repo_root(repo_root.as_ref()) else {
            return;
        };
        if self
            .auto_index_queue
            .protected_roots()
            .await
            .into_iter()
            .any(|path| path == repo_root)
        {
            log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Keeping workspace search index because automatic indexing is still active: path={}",
                repo_root.display()
            );
            return;
        }

        let session = self.sessions.write().await.remove(&repo_root);
        self.open_guards.lock().await.remove(&repo_root);
        if let Some(entry) = session {
            if let Err(error) = entry.session.close().await {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Failed to close workspace search session before index removal: path={}, error={}",
                    repo_root.display(),
                    error
                );
            }
        }

        let removal_root = repo_root.clone();
        match tokio::task::spawn_blocking(move || index_budget::remove_for_repo(removal_root)).await
        {
            Ok(Ok(true)) => log::info!(
                target: FLASHGREP_LOG_TARGET,
                "Removed workspace search index: path={}",
                repo_root.display()
            ),
            Ok(Ok(false)) => {}
            Ok(Err(error)) => log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Failed to remove workspace search index: {}",
                error
            ),
            Err(error) => log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search index removal worker failed: {}",
                error
            ),
        }
    }

    pub async fn shutdown_all_daemons(&self) {
        let released_sessions = self.sessions.write().await.drain().count();
        self.open_guards.lock().await.clear();
        if released_sessions > 0 {
            log::info!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search shutdown releasing sessions via daemon shutdown: count={}",
                released_sessions
            );
        }
        if let Err(error) = self.client.shutdown_daemon().await {
            log::debug!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search daemon shutdown skipped: {}",
                error
            );
        }
    }

    pub async fn stop_all_daemons(&self) {
        let released_sessions = self.sessions.write().await.drain().count();
        self.open_guards.lock().await.clear();
        if released_sessions > 0 {
            log::info!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search stop releasing sessions via daemon stop: count={}",
                released_sessions
            );
        }
        if let Err(error) = self.client.stop_daemon().await {
            log::debug!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search daemon stop skipped: {}",
                error
            );
        }
    }

    pub fn shutdown_blocking(self: &Arc<Self>) {
        let service = Arc::clone(self);
        match std::thread::Builder::new()
            .name("workspace-search-shutdown".to_string())
            .spawn(move || {
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => {
                        runtime.block_on(async move {
                            service.shutdown_all_daemons().await;
                        });
                    }
                    Err(error) => {
                        log::warn!(
                            target: FLASHGREP_LOG_TARGET,
                            "Failed to create runtime for workspace search shutdown: {}",
                            error
                        );
                    }
                }
            }) {
            Ok(handle) => {
                if handle.join().is_err() {
                    log::warn!(
                        target: FLASHGREP_LOG_TARGET,
                        "Workspace search shutdown thread panicked during blocking shutdown"
                    );
                }
            }
            Err(error) => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Failed to spawn workspace search shutdown thread: {}",
                    error
                );
            }
        }
    }

    async fn get_or_open_session(
        &self,
        repo_root: &Path,
    ) -> WorkspaceSearchResult<Arc<RepoSession>> {
        let repo_root = normalize_repo_root(repo_root)?;
        let repo_guard = {
            let mut guards = self.open_guards.lock().await;
            guards
                .entry(repo_root.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _repo_guard = repo_guard.lock().await;

        if let Some(existing) = self.sessions.read().await.get(&repo_root).cloned() {
            existing.activity_epoch.fetch_add(1, Ordering::Relaxed);
            if existing.session.status().await.is_ok() {
                return Ok(existing.session);
            }
            log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search session became unhealthy, reopening repository session: path={}",
                repo_root.display()
            );
            self.sessions.write().await.remove(&repo_root);
            if let Err(error) = existing.session.close().await {
                log::debug!(
                    target: FLASHGREP_LOG_TARGET,
                    "Workspace search repo close after unhealthy session failed: path={}, error={}",
                    repo_root.display(),
                    error
                );
            }
        }

        let repo_config: RepoConfig = self.hooks.repo_config().await.into();
        if let Err(error) = self.hooks.ensure_workspace_ready(&repo_root).await {
            log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Failed to ensure workspace .gitignore ignores .bitfun before search warmup: path={}, error={}",
                repo_root.display(),
                error
            );
        }
        let params = OpenRepoParams {
            repo_path: repo_root.clone(),
            storage_root: Some(default_storage_root(&repo_root)),
            config: repo_config,
            refresh: RefreshPolicyConfig::default(),
        };
        let storage_root = params
            .storage_root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string());

        let entry =
            SessionEntry {
                session: Arc::new(self.client.open_repo(params).await.map_err(
                    map_flashgrep_error("Failed to open flashgrep repository session"),
                )?),
                activity_epoch: Arc::new(AtomicU64::new(1)),
            };
        log::info!(
            target: FLASHGREP_LOG_TARGET,
            "Opened workspace search repository session: path={}, storage_root={}",
            repo_root.display(),
            storage_root
        );

        let mut sessions = self.sessions.write().await;
        Ok(sessions
            .entry(repo_root)
            .or_insert_with(|| entry.clone())
            .session
            .clone())
    }

    async fn run_auto_index_queue(self: Arc<Self>) {
        let mut built_any = false;
        while let Some(repo_root) = self.auto_index_queue.next().await {
            match self.auto_index_repo(&repo_root).await {
                Ok(built) => built_any |= built,
                Err(error) => log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Automatic workspace search indexing failed: path={}, error={}",
                    repo_root.display(),
                    error
                ),
            }
            // Snapshot the session generation before removing the queue entry. A focused request
            // that races with completion either promotes the in-flight entry or increments the
            // generation after this snapshot, so the delayed background release cannot close it.
            let release_epoch = self
                .sessions
                .read()
                .await
                .get(&repo_root)
                .map(|entry| entry.activity_epoch.load(Ordering::Relaxed));
            let final_priority = self.auto_index_queue.complete(&repo_root).await;
            if final_priority == Some(WorkspaceSearchAutoIndexPriority::Background) {
                // Restored workspaces are indexed serially in the background. Keeping every
                // completed repository open makes the single flashgrep daemon retain all of
                // their watchers, segment readers, and helper processes indefinitely.
                if let Some(expected_epoch) = release_epoch {
                    let service = Arc::downgrade(&self);
                    let delay = self.session_idle_grace;
                    tokio::spawn(release_repo_after_delay(
                        service,
                        repo_root.clone(),
                        expected_epoch,
                        delay,
                    ));
                }
            }
        }

        // Enforce only after the queue drains: the roots still queued would be
        // protected anyway, and one walk covers every build in this drain.
        if built_any {
            self.enforce_cached_index_disk_budget().await;
        }
    }

    /// Returns whether this call actually produced index data on disk.
    async fn auto_index_repo(&self, repo_root: &Path) -> WorkspaceSearchResult<bool> {
        let repo_config = self.hooks.repo_config().await;
        let decision = auto_index::evaluate(
            repo_root.to_path_buf(),
            AutoIndexPolicy {
                min_indexable_files: auto_index::DEFAULT_AUTO_INDEX_MIN_FILES,
                max_file_size: repo_config.max_file_size,
            },
        )
        .await;
        match decision {
            AutoIndexDecision::BelowThreshold { indexable_files } => {
                log::debug!(
                    target: FLASHGREP_LOG_TARGET,
                    "Skipping automatic workspace search indexing below file threshold: path={}, indexable_files={}, threshold={}",
                    repo_root.display(),
                    indexable_files,
                    auto_index::DEFAULT_AUTO_INDEX_MIN_FILES
                );
                self.record_auto_index_decision(
                    repo_root,
                    WorkspaceSearchAutoIndexStatus {
                        decision: WorkspaceSearchAutoIndexDecision::BelowThreshold,
                        threshold: auto_index::DEFAULT_AUTO_INDEX_MIN_FILES,
                        indexable_files: Some(indexable_files),
                        reason: None,
                    },
                )
                .await;
                return Ok(false);
            }
            AutoIndexDecision::Unsupported { reason } => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Skipping automatic workspace search indexing because flashgrep requires a Git worktree: path={}, reason={}",
                    repo_root.display(),
                    reason
                );
                self.record_auto_index_decision(
                    repo_root,
                    WorkspaceSearchAutoIndexStatus {
                        decision: WorkspaceSearchAutoIndexDecision::Unsupported,
                        threshold: auto_index::DEFAULT_AUTO_INDEX_MIN_FILES,
                        indexable_files: None,
                        reason: Some(reason),
                    },
                )
                .await;
                return Ok(false);
            }
            AutoIndexDecision::Eligible {
                indexable_files_at_least,
            } => {
                log::info!(
                    target: FLASHGREP_LOG_TARGET,
                    "Starting automatic workspace search indexing: path={}, indexable_files_at_least={}, threshold={}",
                    repo_root.display(),
                    indexable_files_at_least,
                    auto_index::DEFAULT_AUTO_INDEX_MIN_FILES
                );
                self.record_auto_index_decision(
                    repo_root,
                    WorkspaceSearchAutoIndexStatus {
                        decision: WorkspaceSearchAutoIndexDecision::Eligible,
                        threshold: auto_index::DEFAULT_AUTO_INDEX_MIN_FILES,
                        indexable_files: Some(indexable_files_at_least),
                        reason: None,
                    },
                )
                .await;
            }
        }

        let session = self.get_or_open_session(repo_root).await?;
        let status = wait_for_auto_index_repo_status(session.as_ref()).await?;
        if !repo_needs_auto_index_build(status.phase, status.active_task_id.as_deref()) {
            return Ok(false);
        }

        let task = FlashgrepRepoSession::build_index(session.as_ref())
            .await
            .map_err(map_flashgrep_error(
                "Failed to start automatic workspace search indexing",
            ))?;
        wait_for_index_task(session.as_ref(), task.task_id).await?;
        Ok(true)
    }

    async fn index_status_for_session<S>(
        &self,
        session: Arc<S>,
    ) -> WorkspaceSearchResult<WorkspaceIndexStatus>
    where
        S: FlashgrepRepoSession + ?Sized,
    {
        let repo_status = session
            .status()
            .await
            .map_err(map_flashgrep_error("Failed to fetch repository status"))?;
        let active_task = match repo_status.active_task_id.clone() {
            Some(task_id) => match session.task_status(task_id).await {
                Ok(task) => Some(task),
                Err(error) => {
                    log::warn!(
                        target: FLASHGREP_LOG_TARGET,
                        "Failed to fetch active flashgrep task status: {}",
                        error
                    );
                    None
                }
            },
            None => None,
        };

        Ok(WorkspaceIndexStatus {
            repo_status: repo_status.into(),
            active_task: active_task.map(Into::into),
            auto_index: None,
        })
    }

    async fn release_repo_if_idle(self: &Arc<Self>, repo_root: PathBuf, expected_epoch: u64) {
        let active_session = self
            .sessions
            .read()
            .await
            .get(&repo_root)
            .map(|entry| entry.session.clone());
        if let Some(session) = active_session {
            if session
                .status()
                .await
                .map(|status| status.active_task_id.is_some())
                .unwrap_or(false)
            {
                self.schedule_repo_release(repo_root);
                return;
            }
        }

        let entry = {
            let mut sessions = self.sessions.write().await;
            let Some(entry) = sessions.get(&repo_root) else {
                return;
            };
            if entry.activity_epoch.load(Ordering::Relaxed) != expected_epoch {
                return;
            }
            sessions.remove(&repo_root)
        };

        if let Some(entry) = entry {
            log::debug!(
                target: FLASHGREP_LOG_TARGET,
                "Releasing idle workspace search repository session: path={}",
                repo_root.display()
            );
            if let Err(error) = FlashgrepRepoSession::close(entry.session.as_ref()).await {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Failed to release idle workspace search repository session: path={}, error={}",
                    repo_root.display(),
                    error
                );
            }
            self.open_guards.lock().await.remove(&repo_root);
        }
    }
}

async fn release_repo_after_delay(
    service: std::sync::Weak<WorkspaceSearchService>,
    repo_root: PathBuf,
    expected_epoch: u64,
    delay: Duration,
) {
    tokio::time::sleep(delay).await;
    let Some(service) = service.upgrade() else {
        return;
    };
    service
        .release_repo_if_idle(repo_root, expected_epoch)
        .await;
}

/// The pattern and flags needed to recompute match highlighting locally.
///
/// The daemon reports neither the matched text nor its offsets, so previews are
/// rebuilt client-side from the same pattern that was searched.
#[derive(Debug, Clone)]
struct ContentPreviewSpec {
    pattern: String,
    case_sensitive: bool,
    use_regex: bool,
    whole_word: bool,
}

/// Reads matched line text off disk for the grouped daemon result.
///
/// `max_results` is applied inside the hydration pass so files whose matches are
/// about to be dropped are never opened.
async fn hydrate_grouped_content_matches(
    files: Vec<(String, Vec<usize>)>,
    max_results: Option<usize>,
    preview_spec: ContentPreviewSpec,
) -> WorkspaceSearchResult<HydratedLineMatches> {
    tokio::task::spawn_blocking(move || {
        // A pattern the daemon accepts can still fail to compile here (different
        // regex dialect, or a multiline query that no single line matches), in
        // which case results keep their line text and lose only the highlight.
        let preview = match ContentMatchPreviewBuilder::new(
            &preview_spec.pattern,
            preview_spec.case_sensitive,
            preview_spec.use_regex,
            preview_spec.whole_word,
        ) {
            Ok(preview) => Some(preview),
            Err(error) => {
                log::debug!(
                    target: FLASHGREP_LOG_TARGET,
                    "Workspace content search preview highlighting disabled: {error}"
                );
                None
            }
        };
        hydrate_grouped_line_matches(&files, max_results, preview.as_ref())
    })
    .await
    .map_err(|error| format!("Content search line hydration failed: {error}"))
}

async fn wait_for_index_task(session: &RepoSession, task_id: String) -> WorkspaceSearchResult<()> {
    loop {
        let task = session
            .task_status(task_id.clone())
            .await
            .map_err(map_flashgrep_error(
                "Failed to poll automatic workspace search indexing",
            ))?;
        match task.state {
            super::flashgrep::TaskState::Queued | super::flashgrep::TaskState::Running => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            super::flashgrep::TaskState::Completed => return Ok(()),
            super::flashgrep::TaskState::Failed => {
                return Err(task.error.unwrap_or(task.message));
            }
            super::flashgrep::TaskState::Cancelled => {
                return Err("Automatic workspace search indexing was cancelled".to_string());
            }
        }
    }
}

async fn wait_for_auto_index_repo_status(
    session: &RepoSession,
) -> WorkspaceSearchResult<super::flashgrep::RepoStatus> {
    loop {
        let status = session.status().await.map_err(map_flashgrep_error(
            "Failed to fetch repository status for automatic indexing",
        ))?;
        if !matches!(status.phase, super::flashgrep::RepoPhase::Opening) {
            return Ok(status);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn repo_needs_auto_index_build(
    phase: super::flashgrep::RepoPhase,
    active_task_id: Option<&str>,
) -> bool {
    active_task_id.is_none()
        && matches!(
            phase,
            super::flashgrep::RepoPhase::MissingBaseSnapshot
                | super::flashgrep::RepoPhase::BuildingBaseSnapshot
                | super::flashgrep::RepoPhase::RebuildingBaseSnapshot
        )
}

impl Default for WorkspaceSearchService {
    fn default() -> Self {
        Self::new()
    }
}

pub fn workspace_search_daemon_binary_names() -> &'static [&'static str] {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        &["flashgrep-x86_64-pc-windows-msvc.exe"]
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        &["flashgrep-aarch64-pc-windows-msvc.exe"]
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        &["flashgrep-x86_64-apple-darwin"]
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        &["flashgrep-aarch64-apple-darwin"]
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        &[
            "flashgrep-x86_64-unknown-linux-musl",
            "flashgrep-x86_64-unknown-linux-gnu",
        ]
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        &[
            "flashgrep-aarch64-unknown-linux-musl",
            "flashgrep-aarch64-unknown-linux-gnu",
        ]
    } else if cfg!(windows) {
        &["flashgrep.exe"]
    } else {
        &["flashgrep"]
    }
}

pub fn workspace_search_daemon_binary_name() -> &'static str {
    workspace_search_daemon_binary_names()
        .first()
        .copied()
        .unwrap_or("flashgrep")
}

pub fn workspace_search_daemon_missing_hint() -> String {
    let bundled_paths = workspace_search_daemon_binary_names()
        .iter()
        .map(|name| format!("flashgrep/{name}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "workspace search daemon binary is missing; expected one of bundled resources [{}] or a valid FLASHGREP_DAEMON_BIN override",
        bundled_paths
    )
}

pub fn workspace_search_daemon_available() -> bool {
    resolve_workspace_search_daemon_program_path().is_some()
}

pub fn resolve_workspace_search_daemon_program_path() -> Option<PathBuf> {
    if let Some(program) = std::env::var_os("FLASHGREP_DAEMON_BIN") {
        let path = PathBuf::from(program);
        if path.exists() {
            return Some(path);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../../../..");
    let binary_names = workspace_search_daemon_binary_names();
    let profile = std::env::var("PROFILE").ok();

    for candidate in daemon_binary_candidates(&workspace_root, binary_names, profile.as_deref()) {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    which::which("flashgrep").ok()
}

fn resolve_daemon_program() -> Option<OsString> {
    resolve_workspace_search_daemon_program_path().map(PathBuf::into_os_string)
}

fn daemon_binary_candidates(
    workspace_root: &Path,
    binary_names: &[&str],
    current_profile: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    let mut push_candidate = |path: PathBuf| {
        if seen.insert(path.clone()) {
            candidates.push(path);
        }
    };

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            for binary_name in binary_names {
                push_candidate(parent.join(binary_name));
            }
            push_exe_relative_bundle_candidates(&mut push_candidate, parent, binary_names);
        }
    }

    for profile in current_profile
        .into_iter()
        .chain(["debug", "release", "release-fast"])
    {
        for binary_name in binary_names {
            push_candidate(
                workspace_root
                    .join("target")
                    .join(profile)
                    .join(binary_name),
            );
        }
    }

    candidates
}

fn push_exe_relative_bundle_candidates(
    push_candidate: &mut impl FnMut(PathBuf),
    exe_dir: &Path,
    binary_names: &[&str],
) {
    if cfg!(target_os = "macos") {
        for binary_name in binary_names {
            push_candidate(exe_dir.join("../Resources/flashgrep").join(binary_name));
        }
    }

    for binary_name in binary_names {
        push_candidate(exe_dir.join("flashgrep").join(binary_name));
        push_candidate(exe_dir.join("resources/flashgrep").join(binary_name));
    }

    if cfg!(target_os = "linux") {
        for binary_name in binary_names {
            push_candidate(exe_dir.join("../lib/bitfun/flashgrep").join(binary_name));
            push_candidate(exe_dir.join("../share/bitfun/flashgrep").join(binary_name));
            push_candidate(
                exe_dir
                    .join("../share/com.bitfun.desktop/flashgrep")
                    .join(binary_name),
            );
        }
    }
}

fn default_storage_root(repo_root: &Path) -> PathBuf {
    index_budget::storage_root(repo_root)
}

fn abbreviate_pattern_for_log(pattern: &str) -> String {
    const MAX_CHARS: usize = 120;
    let mut chars = pattern.chars();
    let abbreviated: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{}...", abbreviated)
    } else {
        abbreviated
    }
}

fn normalize_repo_root(repo_root: &Path) -> WorkspaceSearchResult<PathBuf> {
    if !repo_root.exists() {
        return Err(format!(
            "Search root does not exist: {}",
            repo_root.display()
        ));
    }
    if !repo_root.is_dir() {
        return Err(format!(
            "Search root is not a directory: {}",
            repo_root.display()
        ));
    }

    dunce::canonicalize(repo_root).map_err(|error| {
        format!(
            "Failed to normalize search root {}: {}",
            repo_root.display(),
            error
        )
    })
}

fn build_scope(
    repo_root: &Path,
    search_path: Option<&Path>,
    globs: Vec<String>,
    file_types: Vec<String>,
    exclude_file_types: Vec<String>,
) -> WorkspaceSearchResult<PathScope> {
    let roots = match search_path {
        Some(path) => {
            let normalized = normalize_scope_path(repo_root, path)?;
            if normalized == repo_root {
                Vec::new()
            } else {
                vec![normalized]
            }
        }
        None => Vec::new(),
    };

    Ok(PathScope {
        roots,
        globs,
        iglobs: Vec::new(),
        type_add: Vec::new(),
        type_clear: Vec::new(),
        types: file_types,
        type_not: exclude_file_types,
    })
}

fn extract_glob_base_directory(pattern: &str) -> (String, String) {
    let glob_start = pattern.find(['*', '?', '[', '{']);

    match glob_start {
        Some(index) => {
            let static_prefix = &pattern[..index];
            let last_separator = static_prefix
                .char_indices()
                .rev()
                .find(|(_, ch)| *ch == '/' || *ch == '\\')
                .map(|(idx, _)| idx);

            if let Some(separator_index) = last_separator {
                (
                    static_prefix[..separator_index].to_string(),
                    pattern[separator_index + 1..].to_string(),
                )
            } else {
                (String::new(), pattern.to_string())
            }
        }
        None => {
            let trimmed = pattern.trim_end_matches(['/', '\\']);
            let literal_path = Path::new(trimmed);
            let base_dir = literal_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty() && *parent != Path::new("."))
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_default();
            let file_name = literal_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| trimmed.to_string());

            let relative_pattern = if pattern.ends_with('/') || pattern.ends_with('\\') {
                format!("{}/", file_name)
            } else {
                file_name
            };

            (base_dir, relative_pattern)
        }
    }
}

fn is_safe_relative_subpath(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn derive_glob_walk_root(search_path_abs: &Path, pattern: &str) -> (PathBuf, String) {
    let (base_dir, relative_pattern) = extract_glob_base_directory(pattern);
    let base_path = Path::new(&base_dir);

    if base_dir.is_empty() || !is_safe_relative_subpath(base_path) {
        return (search_path_abs.to_path_buf(), pattern.to_string());
    }

    let walk_root = search_path_abs.join(base_path);
    if walk_root.starts_with(search_path_abs) {
        (walk_root, relative_pattern)
    } else {
        (search_path_abs.to_path_buf(), pattern.to_string())
    }
}

fn path_to_string(path: &Path) -> String {
    dunce::simplified(path).to_string_lossy().replace('\\', "/")
}

fn relativize_glob_result_path(repo_root: &Path, walk_root: &Path, path: &str) -> String {
    let path = path.replace('\\', "/");
    let path_buf = PathBuf::from(&path);
    if path_buf.is_absolute() {
        return path_buf
            .strip_prefix(walk_root)
            .map(path_to_string)
            .unwrap_or(path);
    }

    let walk_root_relative_to_repo = walk_root.strip_prefix(repo_root).ok();
    if let Some(base) = walk_root_relative_to_repo {
        if !base.as_os_str().is_empty() {
            let base = path_to_string(base);
            let base_with_slash = format!("{}/", base.trim_end_matches('/'));
            if path == base {
                return String::new();
            }
            if let Some(relative) = path.strip_prefix(&base_with_slash) {
                return relative.to_string();
            }
        }
    }

    path
}

fn normalize_scope_path(repo_root: &Path, search_path: &Path) -> WorkspaceSearchResult<PathBuf> {
    let normalized = dunce::canonicalize(search_path).map_err(|error| {
        format!(
            "Failed to normalize search path {}: {}",
            search_path.display(),
            error
        )
    })?;
    if !normalized.starts_with(repo_root) {
        return Err(format!(
            "Search path is outside workspace root: {}",
            normalized.display()
        ));
    }
    Ok(normalized)
}

fn map_flashgrep_error(
    prefix: &'static str,
) -> impl Fn(super::flashgrep::error::AppError) -> String {
    move |error| {
        let detail = match &error {
            super::flashgrep::error::AppError::Io(io_error)
                if io_error.kind() == std::io::ErrorKind::NotFound =>
            {
                format!("{error}. {}", workspace_search_daemon_missing_hint())
            }
            _ => error.to_string(),
        };
        format!("{prefix}: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_search::flashgrep::SearchResults;
    use crate::workspace_search::{ContentSearchOutputMode, WorkspaceSearchRepoStatus};

    fn empty_search_results() -> SearchResults {
        serde_json::from_value(serde_json::json!({
            "candidate_docs": 0,
            "searches_with_match": 0,
            "bytes_searched": 0,
            "matched_lines": 0,
            "matched_occurrences": 0
        }))
        .expect("empty search results should decode with defaulted collections")
    }

    fn repo_status_json(extra_status_fields: serde_json::Value) -> serde_json::Value {
        let mut status = serde_json::json!({
            "repo_id": "/repo",
            "repo_path": "/repo",
            "storage_root": "/repo/.bitfun/search/flashgrep-index",
            "base_snapshot_root": "/repo/.bitfun/search/flashgrep-index/base-snapshot",
            "workspace_overlay_root": "/repo/.bitfun/search/flashgrep-index/workspace-overlay",
            "phase": "ready_clean",
            "snapshot_key": "base-git:abc123+cfg:deadbeef",
            "last_probe_unix_secs": null,
            "last_rebuild_unix_secs": null,
            "dirty_files": {"modified": 0, "deleted": 0, "new": 0},
            "active_task_id": null,
            "probe_healthy": true,
            "last_error": null
        });
        let object = status.as_object_mut().expect("status should be an object");
        for (key, value) in extra_status_fields
            .as_object()
            .expect("extra fields should be an object")
        {
            object.insert(key.clone(), value.clone());
        }
        status
    }

    #[test]
    fn repo_status_reports_no_pending_probe_for_daemons_that_omit_the_field() {
        // Daemons older than v0.2.14 predate `workspace_probe_pending` and reconcile the worktree
        // inside `open_repo`, so a status without the field describes the current worktree and must
        // decode as "nothing owed" rather than failing.
        let status: crate::workspace_search::flashgrep::RepoStatus =
            serde_json::from_value(repo_status_json(serde_json::json!({})))
                .expect("a status without the field should decode");
        assert!(!status.workspace_probe_pending);
        let exposed: WorkspaceSearchRepoStatus = status.into();
        assert!(!exposed.workspace_probe_pending);
    }

    #[test]
    fn repo_status_carries_a_pending_probe_through_to_callers() {
        let status: crate::workspace_search::flashgrep::RepoStatus = serde_json::from_value(
            repo_status_json(serde_json::json!({"workspace_probe_pending": true})),
        )
        .expect("a status with the field should decode");
        assert!(status.workspace_probe_pending);
        // Callers decide what to do about staleness, so the flag has to survive the conversion into
        // the type the UI and agent tools actually read.
        let exposed: WorkspaceSearchRepoStatus = status.into();
        assert!(exposed.workspace_probe_pending);
    }

    #[test]
    fn repo_status_reports_no_maintenance_error_for_daemons_that_omit_the_field() {
        // Daemons older than v0.2.15 have no separate maintenance slot at all: a failed compaction
        // lands in `last_error`, which the next successful worktree probe clears within a few
        // seconds. Decoding the absent field as "nothing failed" is honest for those builds — the
        // failure really is unobservable there — and must not fail the decode.
        let status: crate::workspace_search::flashgrep::RepoStatus =
            serde_json::from_value(repo_status_json(serde_json::json!({})))
                .expect("a status without the field should decode");
        assert!(status.last_maintenance_error.is_none());
        let exposed: WorkspaceSearchRepoStatus = status.into();
        assert!(exposed.last_maintenance_error.is_none());
    }

    #[test]
    fn repo_status_carries_a_maintenance_error_through_to_callers() {
        let status: crate::workspace_search::flashgrep::RepoStatus =
            serde_json::from_value(repo_status_json(serde_json::json!({
                "last_maintenance_error": "io error: Permission denied (os error 13)",
            })))
            .expect("a status with the field should decode");
        // The whole point of the separate slot is that it reaches a status poller, so it has to
        // survive the conversion into the type the UI actually renders.
        let exposed: WorkspaceSearchRepoStatus = status.into();
        assert_eq!(
            exposed.last_maintenance_error.as_deref(),
            Some("io error: Permission denied (os error 13)")
        );
    }

    #[test]
    fn content_search_output_modes_use_current_flashgrep_protocol_modes() {
        assert_eq!(
            ContentSearchOutputMode::Content.search_mode(),
            crate::workspace_search::flashgrep::SearchModeConfig::LineMatches
        );
        assert_eq!(
            ContentSearchOutputMode::Count.search_mode(),
            crate::workspace_search::flashgrep::SearchModeConfig::CountOnly
        );
        assert_eq!(
            ContentSearchOutputMode::FilesWithMatches.search_mode(),
            crate::workspace_search::flashgrep::SearchModeConfig::FilesWithMatches
        );
    }

    #[test]
    fn automatic_indexing_only_starts_when_a_base_snapshot_is_missing_and_idle() {
        assert!(repo_needs_auto_index_build(
            crate::workspace_search::flashgrep::RepoPhase::MissingBaseSnapshot,
            None,
        ));
        assert!(repo_needs_auto_index_build(
            crate::workspace_search::flashgrep::RepoPhase::BuildingBaseSnapshot,
            None,
        ));
        assert!(!repo_needs_auto_index_build(
            crate::workspace_search::flashgrep::RepoPhase::MissingBaseSnapshot,
            Some("task-1"),
        ));
        assert!(!repo_needs_auto_index_build(
            crate::workspace_search::flashgrep::RepoPhase::ReadyClean,
            None,
        ));
    }

    #[test]
    fn glob_scope_preprocessing_extracts_static_pattern_prefix() {
        let repo_root = std::env::temp_dir().join("bitfun-workspace-search-test-repo");
        let search_path = repo_root.join("workspace");
        let (walk_root, pattern) = derive_glob_walk_root(&search_path, "src/*.rs");

        assert_eq!(walk_root, search_path.join("src"));
        assert_eq!(pattern, "*.rs");

        let (unsafe_walk_root, unsafe_pattern) = derive_glob_walk_root(&search_path, "../*.rs");
        assert_eq!(unsafe_walk_root, search_path);
        assert_eq!(unsafe_pattern, "../*.rs");
    }

    #[test]
    fn glob_results_are_relative_to_effective_walk_root() {
        let repo_root = std::env::temp_dir().join("bitfun-workspace-search-test-repo");
        let walk_root = repo_root.join("src");

        assert_eq!(
            relativize_glob_result_path(&repo_root, &walk_root, "src/lib.rs"),
            "lib.rs"
        );
        assert_eq!(
            relativize_glob_result_path(
                &repo_root,
                &walk_root,
                &path_to_string(&walk_root.join("deep/mod.rs")),
            ),
            "deep/mod.rs"
        );
        assert!(path_to_string(&walk_root).ends_with("/src"));
    }

    #[test]
    fn plain_line_matches_carry_positions_without_content() {
        // `search` never returns line text in any mode, so this mapping (used by
        // the remote transport) must not invent a placeholder; local searches go
        // through `search/grouped_line_matches` + `line_hydration` instead.
        let mut search_results = empty_search_results();
        search_results.line_matches = serde_json::from_value(serde_json::json!([{
            "path": "src/search.rs",
            "line_number": 42
        }]))
        .expect("line_matches should decode");

        let results = convert_search_results(&search_results, ContentSearchOutputMode::Content);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/search.rs");
        assert_eq!(results[0].name, "search.rs");
        assert_eq!(results[0].line_number, Some(42));
        assert_eq!(results[0].matched_content, None);
        assert_eq!(results[0].preview_inside, None);
    }

    #[test]
    fn grouped_line_match_results_decode_with_backend_and_status() {
        // The compact variant is deliberately not implemented: its response
        // omits repo_id/backend/status, which every caller here reports.
        let response: super::super::flashgrep::Response = serde_json::from_value(
            serde_json::json!({
                "kind": "search_grouped_line_matches_completed",
                "repo_id": "/repo",
                "backend": "indexed_clean",
                "status": {
                    "repo_id": "/repo",
                    "repo_path": "/repo",
                    "storage_root": "/repo/.bitfun/search/flashgrep-index",
                    "base_snapshot_root": "/repo/.bitfun/search/flashgrep-index/base-snapshot",
                    "workspace_overlay_root": "/repo/.bitfun/search/flashgrep-index/workspace-overlay",
                    "phase": "ready_clean",
                    "snapshot_key": null,
                    "last_probe_unix_secs": null,
                    "last_rebuild_unix_secs": null,
                    "dirty_files": {"modified": 0, "deleted": 0, "new": 0},
                    "active_task_id": null,
                    "probe_healthy": true,
                    "last_error": null
                },
                "results": {
                    "candidate_docs": 3,
                    "matched_lines": 2,
                    "matched_occurrences": 2,
                    "files": [["/repo/src/search.rs", [42, 43]]]
                }
            }),
        )
        .expect("grouped response should decode");

        match response {
            super::super::flashgrep::Response::SearchGroupedLineMatchesCompleted {
                results,
                ..
            } => {
                assert_eq!(results.matched_lines, 2);
                assert_eq!(
                    results.files,
                    vec![("/repo/src/search.rs".to_string(), vec![42, 43])]
                );
                assert!(!results.limit_reached);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }
}
