use crate::infrastructure::{FileSearchOutcome, FileSearchResult, SearchMatchType};
use crate::service::bootstrap::ensure_workspace_gitignore_ignores_bitfun;
use crate::service::config::{get_global_config_service, types::WorkspaceConfig};
use crate::service::search::flashgrep::{
    ConsistencyMode, GlobRequest, ManagedClient, OpenRepoParams, PathScope, QuerySpec,
    RefreshPolicyConfig, RepoConfig, RepoSession, SearchRequest, SearchResults,
};
use crate::util::errors::{BitFunError, BitFunResult};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, LazyLock, Mutex as StdMutex, Weak,
};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

use super::types::{
    ContentSearchOutputMode, ContentSearchRequest, ContentSearchResult, GlobSearchRequest,
    GlobSearchResult, IndexTaskHandle, WorkspaceIndexStatus, WorkspaceSearchFileCount,
    WorkspaceSearchHit,
};

static GLOBAL_WORKSPACE_SEARCH_SERVICE: LazyLock<StdMutex<Weak<WorkspaceSearchService>>> =
    LazyLock::new(|| StdMutex::new(Weak::new()));

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
    session_idle_grace: Duration,
}

impl WorkspaceSearchService {
    pub fn new() -> Self {
        let mut client = ManagedClient::new()
            .with_start_timeout(Duration::from_secs(10))
            .with_retry_interval(Duration::from_millis(100));
        let program = resolve_daemon_program();
        if let Some(program) = program {
            log::info!(
                "WorkspaceSearchService daemon configured: program={}",
                PathBuf::from(&program).display()
            );
            client = client.with_daemon_program(program);
        } else {
            log::info!("WorkspaceSearchService daemon configured: program=flashgrep");
        }

        Self {
            client,
            sessions: RwLock::new(HashMap::new()),
            open_guards: Mutex::new(HashMap::new()),
            session_idle_grace: DEFAULT_SESSION_IDLE_GRACE,
        }
    }

    pub async fn open_repo(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> BitFunResult<WorkspaceIndexStatus> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        self.index_status_for_session(session).await
    }

    pub async fn get_index_status(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> BitFunResult<WorkspaceIndexStatus> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        self.index_status_for_session(session).await
    }

    pub async fn build_index(&self, repo_root: impl AsRef<Path>) -> BitFunResult<IndexTaskHandle> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        let task = session
            .index_build()
            .await
            .map_err(map_flashgrep_error("Failed to start index build"))?;
        let repo_status = session
            .status()
            .await
            .map_err(map_flashgrep_error("Failed to fetch repository status"))?;
        Ok(IndexTaskHandle {
            task: task.into(),
            repo_status: repo_status.into(),
        })
    }

    pub async fn rebuild_index(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> BitFunResult<IndexTaskHandle> {
        let session = self.get_or_open_session(repo_root.as_ref()).await?;
        let task = session
            .index_rebuild()
            .await
            .map_err(map_flashgrep_error("Failed to start index rebuild"))?;
        let repo_status = session
            .status()
            .await
            .map_err(map_flashgrep_error("Failed to fetch repository status"))?;
        Ok(IndexTaskHandle {
            task: task.into(),
            repo_status: repo_status.into(),
        })
    }

    pub async fn search_content(
        &self,
        request: ContentSearchRequest,
    ) -> BitFunResult<ContentSearchResult> {
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
        let search = session
            .search(
                SearchRequest::new(query)
                    .with_scope(scope)
                    .with_consistency(ConsistencyMode::WorkspaceEventual)
                    .with_scan_fallback(true),
            )
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
            hits: search
                .results
                .hits
                .clone()
                .into_iter()
                .map(WorkspaceSearchHit::from)
                .collect(),
            backend: search.backend.into(),
            repo_status: search.status.into(),
            candidate_docs: search.results.candidate_docs,
            matched_lines: search.results.matched_lines,
            matched_occurrences: search.results.matched_occurrences,
        };

        log::info!(
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

    pub async fn glob(&self, request: GlobSearchRequest) -> BitFunResult<GlobSearchResult> {
        let repo_root = normalize_repo_root(&request.repo_root)?;
        let scope = build_scope(
            &repo_root,
            request.search_path.as_deref(),
            vec![request.pattern],
            vec![],
            vec![],
        )?;
        let session = self.get_or_open_session(&repo_root).await?;
        let mut outcome = session
            .glob(GlobRequest::new().with_scope(scope))
            .await
            .map_err(map_flashgrep_error("Glob search failed"))?;
        outcome.paths.sort();
        if request.limit > 0 {
            outcome.paths.truncate(request.limit);
        } else {
            outcome.paths.clear();
        }

        Ok(GlobSearchResult {
            paths: outcome.paths,
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
                "Workspace search shutdown releasing sessions via daemon shutdown: count={}",
                released_sessions
            );
        }
        if let Err(error) = self.client.shutdown_daemon().await {
            log::debug!("Workspace search daemon shutdown skipped: {}", error);
        }
    }

    pub async fn stop_all_daemons(&self) {
        let released_sessions = self.sessions.write().await.drain().count();
        self.open_guards.lock().await.clear();
        if released_sessions > 0 {
            log::info!(
                "Workspace search stop releasing sessions via daemon stop: count={}",
                released_sessions
            );
        }
        if let Err(error) = self.client.stop_daemon().await {
            log::debug!("Workspace search daemon stop skipped: {}", error);
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
                            "Failed to create runtime for workspace search shutdown: {}",
                            error
                        );
                    }
                }
            }) {
            Ok(handle) => {
                if handle.join().is_err() {
                    log::warn!(
                        "Workspace search shutdown thread panicked during blocking shutdown"
                    );
                }
            }
            Err(error) => {
                log::warn!(
                    "Failed to spawn workspace search shutdown thread: {}",
                    error
                );
            }
        }
    }

    async fn get_or_open_session(&self, repo_root: &Path) -> BitFunResult<Arc<RepoSession>> {
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
                "Workspace search session became unhealthy, reopening repository session: path={}",
                repo_root.display()
            );
            self.sessions.write().await.remove(&repo_root);
            if let Err(error) = existing.session.close().await {
                log::debug!(
                    "Workspace search repo close after unhealthy session failed: path={}, error={}",
                    repo_root.display(),
                    error
                );
            }
        }

        let repo_config = repo_config_for_workspace_search().await;
        if let Err(error) = ensure_workspace_gitignore_ignores_bitfun(&repo_root).await {
            log::warn!(
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

        let entry =
            SessionEntry {
                session: Arc::new(self.client.open_repo(params).await.map_err(
                    map_flashgrep_error("Failed to open flashgrep repository session"),
                )?),
                activity_epoch: Arc::new(AtomicU64::new(1)),
            };

        let mut sessions = self.sessions.write().await;
        Ok(sessions
            .entry(repo_root)
            .or_insert_with(|| entry.clone())
            .session
            .clone())
    }

    async fn index_status_for_session(
        &self,
        session: Arc<RepoSession>,
    ) -> BitFunResult<WorkspaceIndexStatus> {
        let repo_status = session
            .status()
            .await
            .map_err(map_flashgrep_error("Failed to fetch repository status"))?;
        let active_task = match repo_status.active_task_id.clone() {
            Some(task_id) => match session.task_status(task_id).await {
                Ok(task) => Some(task),
                Err(error) => {
                    log::warn!("Failed to fetch active flashgrep task status: {}", error);
                    None
                }
            },
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
            log::info!(
                "Releasing idle workspace search repository session: path={}",
                repo_root.display()
            );
            if let Err(error) = entry.session.close().await {
                log::warn!(
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

pub fn set_global_workspace_search_service(service: Arc<WorkspaceSearchService>) {
    let mut global = match GLOBAL_WORKSPACE_SEARCH_SERVICE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *global = Arc::downgrade(&service);
}

pub fn get_global_workspace_search_service() -> Option<Arc<WorkspaceSearchService>> {
    let global = match GLOBAL_WORKSPACE_SEARCH_SERVICE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    global.upgrade()
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
        &["flashgrep-x86_64-unknown-linux-gnu"]
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        &["flashgrep-aarch64-unknown-linux-gnu"]
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

pub async fn workspace_search_feature_enabled() -> bool {
    match get_global_config_service().await {
        Ok(config_service) => config_service
            .get_config::<bool>(Some("app.ai_experience.enable_workspace_search"))
            .await
            .unwrap_or(false),
        Err(_) => false,
    }
}

pub async fn workspace_search_runtime_available() -> bool {
    workspace_search_feature_enabled().await && workspace_search_daemon_available()
}

pub fn resolve_workspace_search_daemon_program_path() -> Option<PathBuf> {
    if let Some(program) = std::env::var_os("FLASHGREP_DAEMON_BIN") {
        let path = PathBuf::from(program);
        if path.exists() {
            return Some(path);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../../..");
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

async fn repo_config_for_workspace_search() -> RepoConfig {
    let max_file_size = match get_global_config_service().await {
        Ok(config_service) => match config_service
            .get_config::<WorkspaceConfig>(Some("workspace"))
            .await
        {
            Ok(workspace_config) => workspace_config.max_file_size,
            Err(error) => {
                log::warn!(
                    "Failed to read workspace config for flashgrep repo open, using default max_file_size: {}",
                    error
                );
                WorkspaceConfig::default().max_file_size
            }
        },
        Err(error) => {
            log::warn!(
                "Global config service unavailable for flashgrep repo open, using default max_file_size: {}",
                error
            );
            WorkspaceConfig::default().max_file_size
        }
    };

    RepoConfig {
        max_file_size,
        ..RepoConfig::default()
    }
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

fn normalize_repo_root(repo_root: &Path) -> BitFunResult<PathBuf> {
    if !repo_root.exists() {
        return Err(BitFunError::service(format!(
            "Search root does not exist: {}",
            repo_root.display()
        )));
    }
    if !repo_root.is_dir() {
        return Err(BitFunError::service(format!(
            "Search root is not a directory: {}",
            repo_root.display()
        )));
    }

    dunce::canonicalize(repo_root).map_err(|error| {
        BitFunError::service(format!(
            "Failed to normalize search root {}: {}",
            repo_root.display(),
            error
        ))
    })
}

fn build_scope(
    repo_root: &Path,
    search_path: Option<&Path>,
    globs: Vec<String>,
    file_types: Vec<String>,
    exclude_file_types: Vec<String>,
) -> BitFunResult<PathScope> {
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

fn normalize_scope_path(repo_root: &Path, search_path: &Path) -> BitFunResult<PathBuf> {
    let normalized = dunce::canonicalize(search_path).map_err(|error| {
        BitFunError::service(format!(
            "Failed to normalize search path {}: {}",
            search_path.display(),
            error
        ))
    })?;
    if !normalized.starts_with(repo_root) {
        return Err(BitFunError::service(format!(
            "Search path is outside workspace root: {}",
            normalized.display()
        )));
    }
    Ok(normalized)
}

fn convert_search_results(
    search_results: &SearchResults,
    output_mode: ContentSearchOutputMode,
) -> Vec<FileSearchResult> {
    match output_mode {
        ContentSearchOutputMode::Content => convert_hits_to_file_search_results(search_results),
        ContentSearchOutputMode::Count => convert_file_counts_to_search_results(search_results),
        ContentSearchOutputMode::FilesWithMatches => {
            convert_hits_to_file_only_results(search_results)
        }
    }
}

fn convert_file_counts_to_search_results(search_results: &SearchResults) -> Vec<FileSearchResult> {
    search_results
        .file_counts
        .iter()
        .map(|count| FileSearchResult {
            path: count.path.clone(),
            name: Path::new(&count.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&count.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: Some(count.matched_lines.to_string()),
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn convert_hits_to_file_search_results(search_results: &SearchResults) -> Vec<FileSearchResult> {
    let mut file_results = Vec::new();
    for hit in &search_results.hits {
        let name = Path::new(&hit.path)
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or(&hit.path)
            .to_string();

        let mut lines = BTreeMap::new();
        for file_match in &hit.matches {
            lines
                .entry(file_match.location.line)
                .or_insert_with(|| file_match.clone());
        }

        for (_, file_match) in lines {
            let (preview_before, preview_inside, preview_after) =
                split_preview(&file_match.snippet, &file_match.matched_text);
            file_results.push(FileSearchResult {
                path: hit.path.clone(),
                name: name.clone(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(file_match.location.line),
                matched_content: Some(file_match.snippet),
                preview_before,
                preview_inside,
                preview_after,
            });
        }
    }
    file_results
}

fn convert_hits_to_file_only_results(search_results: &SearchResults) -> Vec<FileSearchResult> {
    search_results
        .hits
        .iter()
        .map(|hit| FileSearchResult {
            path: hit.path.clone(),
            name: Path::new(&hit.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&hit.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: None,
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn split_preview(
    snippet: &str,
    matched_text: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    if matched_text.is_empty() {
        return (None, Some(snippet.to_string()), None);
    }

    if let Some(offset) = snippet.find(matched_text) {
        let before = snippet[..offset].to_string();
        let inside = matched_text.to_string();
        let after = snippet[offset + matched_text.len()..].to_string();
        return (
            (!before.is_empty()).then_some(before),
            Some(inside),
            (!after.is_empty()).then_some(after),
        );
    }

    (None, Some(snippet.to_string()), None)
}

fn map_flashgrep_error(
    prefix: &'static str,
) -> impl Fn(crate::service::search::flashgrep::error::AppError) -> BitFunError {
    move |error| {
        let detail = match &error {
            crate::service::search::flashgrep::error::AppError::Io(io_error)
                if io_error.kind() == std::io::ErrorKind::NotFound =>
            {
                format!("{error}. {}", workspace_search_daemon_missing_hint())
            }
            _ => error.to_string(),
        };
        BitFunError::service(format!("{prefix}: {detail}"))
    }
}
