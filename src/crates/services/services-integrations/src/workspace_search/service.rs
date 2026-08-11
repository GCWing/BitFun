use super::flashgrep::{
    FlashgrepRepoSession, GlobRequest, ManagedClient, OpenRepoParams, PathScope, QuerySpec,
    RefreshPolicyConfig, RepoConfig, RepoSession, SearchRequest, FLASHGREP_LOG_TARGET,
};
use async_trait::async_trait;
use bitfun_services_core::filesystem::FileSearchOutcome;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

use super::result_mapping::convert_search_results;
use super::rg_fallback;
use super::types::{
    ContentSearchRequest, ContentSearchResult, GlobSearchRequest, GlobSearchResult,
    IndexTaskHandle, WorkspaceIndexStatus, WorkspaceSearchFileCount,
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
const SESSION_LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const SESSION_STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const SESSION_OPEN_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_INDEX_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_SEARCH_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_GLOB_TIMEOUT: Duration = Duration::from_secs(30);

/// Wait for a tokio mutex guard with a bounded timeout, returning an error
/// string on timeout instead of blocking the caller indefinitely.
async fn try_lock_or_timeout<'a, T>(
    lock: &'a tokio::sync::Mutex<T>,
    what: &'a str,
) -> Result<tokio::sync::MutexGuard<'a, T>, String> {
    tokio::time::timeout(SESSION_LOCK_TIMEOUT, lock.lock())
        .await
        .map_err(|_| format!("workspace search timed out waiting for {what} lock"))
}

#[derive(Debug, Clone)]
struct SessionEntry {
    session: Arc<RepoSession>,
    activity_epoch: Arc<AtomicU64>,
}

pub struct WorkspaceSearchService {
    client: ManagedClient,
    sessions: RwLock<HashMap<PathBuf, SessionEntry>>,
    open_guards: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
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
            session_idle_grace: DEFAULT_SESSION_IDLE_GRACE,
            hooks,
        }
    }

    pub async fn open_repo(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> WorkspaceSearchResult<WorkspaceIndexStatus> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        self.index_status_for_session(session).await
    }

    pub async fn get_index_status(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> WorkspaceSearchResult<WorkspaceIndexStatus> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        self.index_status_for_session(session).await
    }

    pub async fn build_index(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> WorkspaceSearchResult<IndexTaskHandle> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        let task = tokio::time::timeout(
            SESSION_INDEX_TIMEOUT,
            FlashgrepRepoSession::build_index(session.as_ref()),
        )
        .await
        .map_err(|_| format!("workspace search timed out starting index build: path={}", repo_root.as_ref().display()))?
        .map_err(map_flashgrep_error("Failed to start index build"))?;
        let repo_status = match tokio::time::timeout(SESSION_STATUS_TIMEOUT, session.status())
            .await
        {
            Ok(Ok(status)) => status.into(),
            Ok(Err(error)) => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Failed to fetch repository status after index build: path={}, error={}",
                    repo_root.as_ref().display(),
                    error
                );
                unknown_repo_status(
                    repo_root.as_ref(),
                    &format!("failed to fetch repository status after index build: {error}"),
                )
            }
            Err(_) => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Workspace search timed out fetching repository status after index build: path={}",
                    repo_root.as_ref().display()
                );
                unknown_repo_status(
                    repo_root.as_ref(),
                    "timed out fetching repository status after index build",
                )
            }
        };
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
        let task = tokio::time::timeout(
            SESSION_INDEX_TIMEOUT,
            FlashgrepRepoSession::rebuild_index(session.as_ref()),
        )
        .await
        .map_err(|_| format!("workspace search timed out starting index rebuild: path={}", repo_root.as_ref().display()))?
        .map_err(map_flashgrep_error("Failed to start index rebuild"))?;
        let repo_status = match tokio::time::timeout(SESSION_STATUS_TIMEOUT, session.status())
            .await
        {
            Ok(Ok(status)) => status.into(),
            Ok(Err(error)) => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Failed to fetch repository status after index rebuild: path={}, error={}",
                    repo_root.as_ref().display(),
                    error
                );
                unknown_repo_status(
                    repo_root.as_ref(),
                    &format!("failed to fetch repository status after index rebuild: {error}"),
                )
            }
            Err(_) => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Workspace search timed out fetching repository status after index rebuild: path={}",
                    repo_root.as_ref().display()
                );
                unknown_repo_status(
                    repo_root.as_ref(),
                    "timed out fetching repository status after index rebuild",
                )
            }
        };
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
        // rg 交叉校验所需的请求快照（pattern/globs/file_types 等随后被 move 进 query/scope）。
        let validation_request = rg_fallback::RgValidationRequest {
            search_root: request
                .search_path
                .clone()
                .unwrap_or_else(|| repo_root.clone()),
            pattern: request.pattern.clone(),
            case_insensitive: !request.case_sensitive,
            multiline: request.multiline,
            whole_word: request.whole_word,
            fixed_strings: !request.use_regex,
            globs: scope.globs.clone(),
            file_types: scope.types.clone(),
            exclude_file_types: scope.type_not.clone(),
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
            before_context: request.before_context,
            after_context: request.after_context,
            top_k_tokens: DEFAULT_TOP_K_TOKENS,
            max_count: None,
            global_max_results: max_results,
            search_mode: request.output_mode.search_mode(),
        };

        let session = self.get_or_open_session(&repo_root).await?;
        let session_ready_at = Instant::now();
        let search = tokio::time::timeout(
            SESSION_SEARCH_TIMEOUT,
            FlashgrepRepoSession::search(
                session.as_ref(),
                SearchRequest::new(query)
                    .with_scope(scope)
                    .with_scan_fallback(true),
            ),
        )
        .await
        .map_err(|_| {
            format!(
                "workspace search timed out executing content search: repo_root={}, pattern={}",
                repo_root.display(),
                pattern_for_log
            )
        })?
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

        // 根因级假空交叉校验（RECON-卡搜索根因彻查-20260809）：
        // flashgrep daemon（闭源）overlay 路径匹配 bug 可能在任意相位/scope 组合下
        // 返回 Ok(空)，工具层的 phase/scope/candidate_docs 判据只能枚举已见形态。
        // 这里在 service 层对空结果用 rg 库引擎做结果实证：rg 有命中而 flashgrep
        // 为空 = 假空 = 信任 rg 结果；rg 也为空 = 真实无结果，原样返回。
        let result = match cross_validate_empty_result(&validation_request, result) {
            Ok(validated) => validated,
            Err(original) => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Workspace content search rg cross-validation unavailable, keeping daemon result: repo_root={}, pattern={}",
                    repo_root.display(),
                    pattern_for_log,
                );
                *original
            }
        };

        log::debug!(
            target: FLASHGREP_LOG_TARGET,
            "Workspace content search completed: repo_root={}, pattern={}, output_mode={:?}, search_mode={:?}, scope_roots={}, globs={}, file_types={}, max_results={:?}, backend={:?}, repo_phase={:?}, rebuild_recommended={}, dirty_modified={}, dirty_deleted={}, dirty_new={}, candidate_docs={}, matched_lines={}, matched_occurrences={}, returned_results={}, truncated={}, normalize_ms={}, build_scope_ms={}, session_ms={}, search_ms={}, convert_ms={}, total_ms={}",
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
            result.repo_status.rebuild_recommended,
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
            let repo_status = tokio::time::timeout(
                SESSION_STATUS_TIMEOUT,
                session.status(),
            )
            .await
            .map_err(|_| format!("workspace search timed out fetching repository status: path={}", repo_root.display()))?
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
        let outcome = tokio::time::timeout(
            SESSION_GLOB_TIMEOUT,
            FlashgrepRepoSession::glob(session.as_ref(), GlobRequest::new().with_scope(scope)),
        )
        .await
        .map_err(|_| format!("workspace search timed out executing glob search: path={}", repo_root.display()))?
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
            tokio::time::sleep(delay).await;
            let Some(service) = service.upgrade() else {
                return;
            };
            service.release_repo_if_idle(repo_root).await;
        });
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
            let mut guards = try_lock_or_timeout(&self.open_guards, "workspace search")
                .await?;
            guards
                .entry(repo_root.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _repo_guard = try_lock_or_timeout(&repo_guard, "repository").await?;

        if let Some(existing) = self.sessions.read().await.get(&repo_root).cloned() {
            existing.activity_epoch.fetch_add(1, Ordering::Relaxed);
            let status_ok = tokio::time::timeout(
                SESSION_STATUS_TIMEOUT,
                existing.session.status(),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false);
            if status_ok {
                return Ok(existing.session);
            }
            log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Workspace search session became unhealthy or timed out, reopening repository session: path={}",
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

        let session = tokio::time::timeout(SESSION_OPEN_TIMEOUT, self.client.open_repo(params))
            .await
            .map_err(|_| {
                format!(
                    "workspace search timed out opening flashgrep repository session: path={}",
                    repo_root.display()
                )
            })?
            .map_err(map_flashgrep_error("Failed to open flashgrep repository session"))?;
        let entry = SessionEntry {
            session: Arc::new(session),
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

    async fn index_status_for_session<S>(
        &self,
        session: Arc<S>,
    ) -> WorkspaceSearchResult<WorkspaceIndexStatus>
    where
        S: FlashgrepRepoSession + ?Sized,
    {
        let repo_status = tokio::time::timeout(
            SESSION_STATUS_TIMEOUT,
            session.status(),
        )
        .await
        .map_err(|_| format!("workspace search timed out fetching repository status"))?
        .map_err(map_flashgrep_error("Failed to fetch repository status"))?;
        let active_task = match repo_status.active_task_id.clone() {
            Some(task_id) => {
                match tokio::time::timeout(SESSION_STATUS_TIMEOUT, session.task_status(task_id))
                    .await
                {
                    Ok(Ok(task)) => Some(task),
                    Ok(Err(error)) => {
                        log::warn!(
                            target: FLASHGREP_LOG_TARGET,
                            "Failed to fetch active flashgrep task status: {}",
                            error
                        );
                        None
                    }
                    Err(_) => {
                        log::warn!(
                            target: FLASHGREP_LOG_TARGET,
                            "Workspace search timed out fetching active flashgrep task status"
                        );
                        None
                    }
                }
            }
            None => None,
        };

        Ok(WorkspaceIndexStatus {
            repo_status: repo_status.into(),
            active_task: active_task.map(Into::into),
        })
    }

    async fn release_repo_if_idle(&self, repo_root: PathBuf) {
        let Some(expected_epoch) = self
            .sessions
            .read()
            .await
            .get(&repo_root)
            .map(|entry| entry.activity_epoch.load(Ordering::Relaxed))
        else {
            return;
        };

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
    repo_root
        .join(".bitfun")
        .join("search")
        .join("flashgrep-index")
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

fn unknown_repo_status(
    repo_root: &Path,
    reason: &str,
) -> super::types::WorkspaceSearchRepoStatus {
    super::types::WorkspaceSearchRepoStatus {
        repo_id: String::new(),
        repo_path: repo_root.display().to_string(),
        storage_root: String::new(),
        base_snapshot_root: String::new(),
        workspace_overlay_root: String::new(),
        phase: super::types::WorkspaceSearchRepoPhase::Limited,
        snapshot_key: None,
        last_probe_unix_secs: None,
        last_rebuild_unix_secs: None,
        dirty_files: super::types::WorkspaceSearchDirtyFiles {
            modified: 0,
            deleted: 0,
            new: 0,
        },
        rebuild_recommended: false,
        active_task_id: None,
        probe_healthy: false,
        last_error: Some(reason.to_string()),
        overlay: None,
    }
}

/// 对 flashgrep 返回的空结果做 rg 库引擎交叉校验（service 层根因级假空兜底）。
///
/// 返回 `Ok(result)`：若结果为假空（rg 有命中）则替换为 rg 结果，否则原样返回。
/// 返回 `Err(original)`：校验自身不可用/无法判定，原样交还 daemon 结果由调用方保留。
fn cross_validate_empty_result(
    validation_request: &super::rg_fallback::RgValidationRequest,
    result: ContentSearchResult,
) -> Result<ContentSearchResult, Box<ContentSearchResult>> {
    if !super::rg_fallback::search_result_is_empty(&result) {
        return Ok(result);
    }
    let outcome = match super::rg_fallback::rg_search(
        validation_request,
        super::rg_fallback::RG_VALIDATION_FILE_BUDGET,
    ) {
        Ok(Some(outcome)) => outcome,
        // 预算内无法判定（scope 文件数超预算且前段无命中）或校验不可用：
        // 保守保留原结果，交给工具层既有判据兜底，不在 service 层放大不确定性。
        Ok(None) => return Err(Box::new(result)),
        Err(_) => return Err(Box::new(result)),
    };
    if outcome.total_matches() == 0 {
        // rg 也确认无命中 = 真实空结果。
        return Ok(result);
    }
    log::warn!(
        target: FLASHGREP_LOG_TARGET,
        "Workspace search daemon returned empty but rg cross-validation found matches (false-empty); serving rg results: search_root={}, pattern={}, rg_matched_lines={}, rg_files={}, phase={:?}, candidate_docs={}",
        validation_request.search_root.display(),
        abbreviate_pattern_for_log(&validation_request.pattern),
        outcome.total_matches(),
        outcome.files.len(),
        result.repo_status.phase,
        result.candidate_docs,
    );
    Ok(outcome.into_content_search_result(result.repo_status.clone()))
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
    use crate::workspace_search::ContentSearchOutputMode;

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
    fn content_search_converts_line_matches_without_line_text() {
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
        assert_eq!(results[0].matched_content.as_deref(), Some("line 42"));
        assert_eq!(results[0].preview_inside, None);
    }

    #[test]
    fn content_search_converts_line_matches_with_line_text_preview() {
        let mut search_results = empty_search_results();
        search_results.line_matches = serde_json::from_value(serde_json::json!([{
            "path": "src/search.rs",
            "line_number": 42,
            "line_text": "let result = search();"
        }]))
        .expect("line_matches should decode");

        let results = convert_search_results(&search_results, ContentSearchOutputMode::Content);

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].matched_content.as_deref(),
            Some("let result = search();")
        );
        assert_eq!(
            results[0].preview_inside.as_deref(),
            Some("let result = search();")
        );
        assert_eq!(results[0].preview_before, None);
        assert_eq!(results[0].preview_after, None);
    }
}
