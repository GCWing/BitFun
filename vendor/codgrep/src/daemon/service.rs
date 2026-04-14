use std::{sync::mpsc, sync::Arc};

use super::{
    protocol::{RequestEnvelope, ResponseEnvelope, ServerMessage},
    repo::RepoManager,
};

#[path = "service/connections.rs"]
mod connections;
#[path = "service/notifications.rs"]
mod notifications;
#[path = "service/router.rs"]
mod router;

use connections::ConnectionRegistry;
pub use notifications::ServiceNotificationEvent;
use router::protocol_error;

#[derive(Clone)]
pub struct DaemonService {
    repos: Arc<RepoManager>,
    connections: Arc<ConnectionRegistry>,
}

impl DaemonService {
    pub fn new() -> Self {
        let connections = Arc::new(ConnectionRegistry::default());
        let notify_connections = Arc::clone(&connections);
        Self {
            repos: Arc::new(RepoManager::with_notifier(Arc::new(move |event| {
                notify_connections.broadcast(event);
            }))),
            connections,
        }
    }

    pub fn shutdown_all(&self) {
        self.repos.shutdown_all();
    }

    #[allow(dead_code)]
    pub fn handle(&self, envelope: RequestEnvelope) -> ResponseEnvelope {
        self.handle_for_connection(None, envelope)
            .expect("direct service handle should always have a response")
    }

    pub fn register_connection(&self, sender: mpsc::Sender<ServerMessage>) -> u64 {
        self.connections.register(sender)
    }

    pub fn unregister_connection(&self, connection_id: u64) {
        self.connections.unregister(connection_id);
    }

    pub fn handle_for_connection(
        &self,
        connection_id: Option<u64>,
        envelope: RequestEnvelope,
    ) -> Option<ResponseEnvelope> {
        let id = envelope.id;
        match self.handle_envelope(envelope) {
            Ok((result, initialize_caps, initialized)) => {
                if let (Some(connection_id), Some(capabilities)) = (connection_id, initialize_caps)
                {
                    self.connections
                        .set_capabilities(connection_id, capabilities);
                }
                if let (Some(connection_id), true) = (connection_id, initialized) {
                    self.connections.mark_initialized(connection_id);
                }
                id.map(|id| ResponseEnvelope::success(Some(id), result))
            }
            Err(error) => id.map(|id| ResponseEnvelope::error(Some(id), protocol_error(error))),
        }
    }
}

impl Default for DaemonService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        fs,
        path::PathBuf,
        sync::{Mutex, MutexGuard, OnceLock},
        thread,
        time::Duration,
    };

    use tempfile::tempdir;

    use super::*;
    use crate::daemon::protocol::{
        ConsistencyMode, EnsureRepoParams, GlobParams, InitializeParams, OpenRepoParams, PathScope,
        QuerySpec, RepoConfig, RepoStatus as RepoStatusPayload, Request, Response, SearchBackend,
        SearchModeConfig, TaskState, TaskStatus as TaskStatusPayload,
    };
    use crate::experimental::index_format::{read_docs_file, write_docs_file, IndexLayout};

    thread_local! {
        static DAEMON_TEST_LOCK_DEPTH: Cell<usize> = const { Cell::new(0) };
    }

    const STATUS_WAIT_ATTEMPTS: usize = 400;
    const SEARCH_WAIT_ATTEMPTS: usize = 600;
    const TASK_WAIT_ATTEMPTS: usize = 400;

    enum DaemonTestGuard {
        Primary(MutexGuard<'static, ()>),
        Nested,
    }

    impl Drop for DaemonTestGuard {
        fn drop(&mut self) {
            DAEMON_TEST_LOCK_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            if let Self::Primary(guard) = self {
                let _ = &guard;
            }
        }
    }

    fn daemon_test_serial() -> DaemonTestGuard {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let nested = DAEMON_TEST_LOCK_DEPTH.with(|depth| {
            let current = depth.get();
            depth.set(current + 1);
            current > 0
        });
        if nested {
            DaemonTestGuard::Nested
        } else {
            let guard = match LOCK.get_or_init(|| Mutex::new(())).lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            DaemonTestGuard::Primary(guard)
        }
    }

    fn open_repo(service: &DaemonService, repo_path: PathBuf) -> (String, RepoStatusPayload) {
        let (repo_id, status) = match service
            .handle_request_only(Request::OpenRepo {
                params: OpenRepoParams {
                    repo_path,
                    index_path: None,
                    config: RepoConfig::default(),
                    refresh: Default::default(),
                },
            })
            .expect("open repo should succeed")
        {
            Response::RepoOpened { repo_id, status } => (repo_id, status),
            other => panic!("unexpected response: {other:?}"),
        };
        let status = if matches!(status.phase, crate::daemon::protocol::RepoPhase::Opening) {
            wait_for_status(service, &repo_id, |status| {
                !matches!(status.phase, crate::daemon::protocol::RepoPhase::Opening)
            })
        } else {
            status
        };
        (repo_id, status)
    }

    fn ensure_repo(service: &DaemonService, repo_path: PathBuf) -> Response {
        service
            .handle_request_only(Request::EnsureRepo {
                params: EnsureRepoParams {
                    repo_path,
                    index_path: None,
                    config: RepoConfig::default(),
                    refresh: Default::default(),
                },
            })
            .expect("ensure repo should succeed")
    }

    fn build_repo(service: &DaemonService, repo_id: &str) {
        service
            .handle_request_only(Request::BuildIndex {
                params: crate::daemon::protocol::RepoRef {
                    repo_id: repo_id.to_string(),
                },
            })
            .expect("build should succeed");
    }

    fn query(pattern: &str) -> QuerySpec {
        QuerySpec {
            pattern: pattern.into(),
            patterns: Vec::new(),
            case_insensitive: false,
            multiline: false,
            dot_matches_new_line: false,
            fixed_strings: false,
            word_regexp: false,
            line_regexp: false,
            before_context: 0,
            after_context: 0,
            top_k_tokens: 6,
            max_count: None,
            global_max_results: None,
            search_mode: SearchModeConfig::CountOnly,
        }
    }

    fn search_repo(
        service: &DaemonService,
        repo_id: &str,
        pattern: &str,
        consistency: ConsistencyMode,
    ) -> Response {
        service
            .handle_request_only(Request::Search {
                params: crate::daemon::protocol::SearchParams {
                    repo_id: repo_id.to_string(),
                    query: query(pattern),
                    scope: PathScope::default(),
                    consistency,
                    allow_scan_fallback: false,
                },
            })
            .expect("search should succeed")
    }

    fn search_repo_with_params(
        service: &DaemonService,
        repo_id: &str,
        query: QuerySpec,
        roots: Vec<PathBuf>,
        consistency: ConsistencyMode,
    ) -> Response {
        service
            .handle_request_only(Request::Search {
                params: crate::daemon::protocol::SearchParams {
                    repo_id: repo_id.to_string(),
                    query,
                    scope: PathScope {
                        roots,
                        ..PathScope::default()
                    },
                    consistency,
                    allow_scan_fallback: false,
                },
            })
            .expect("search should succeed")
    }

    fn refresh_repo(service: &DaemonService, repo_id: &str, force: bool) -> RepoStatusPayload {
        match service
            .handle_request_only(Request::RefreshRepo {
                params: crate::daemon::protocol::RefreshRepoParams {
                    repo_id: repo_id.to_string(),
                    force,
                },
            })
            .expect("refresh should succeed")
        {
            Response::RepoStatus { status } => status,
            other => panic!("unexpected response: {other:?}"),
        }
    }

    fn glob_repo(service: &DaemonService, repo_id: &str, scope: PathScope) -> Response {
        service
            .handle_request_only(Request::Glob {
                params: GlobParams {
                    repo_id: repo_id.to_string(),
                    scope,
                },
            })
            .expect("glob should succeed")
    }

    fn get_status(service: &DaemonService, repo_id: &str) -> RepoStatusPayload {
        match service
            .handle_request_only(Request::GetRepoStatus {
                params: crate::daemon::protocol::RepoRef {
                    repo_id: repo_id.to_string(),
                },
            })
            .expect("status should succeed")
        {
            Response::RepoStatus { status } => status,
            other => panic!("unexpected response: {other:?}"),
        }
    }

    fn wait_for_status<F>(service: &DaemonService, repo_id: &str, predicate: F) -> RepoStatusPayload
    where
        F: Fn(&RepoStatusPayload) -> bool,
    {
        for _ in 0..STATUS_WAIT_ATTEMPTS {
            let status = get_status(service, repo_id);
            if predicate(&status) {
                return status;
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for repo status");
    }

    fn wait_for_search_count(
        service: &DaemonService,
        repo_id: &str,
        pattern: &str,
        consistency: ConsistencyMode,
        expected: usize,
    ) -> (SearchBackend, RepoStatusPayload) {
        for _ in 0..SEARCH_WAIT_ATTEMPTS {
            match search_repo(service, repo_id, pattern, consistency) {
                Response::SearchCompleted {
                    backend,
                    results,
                    status,
                    ..
                } if results.matched_lines == expected => return (backend, status),
                Response::SearchCompleted { .. } => {}
                other => panic!("unexpected response: {other:?}"),
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for search count");
    }

    fn wait_for_glob_paths(
        service: &DaemonService,
        repo_id: &str,
        scope: PathScope,
        expected: &[String],
    ) -> RepoStatusPayload {
        for _ in 0..SEARCH_WAIT_ATTEMPTS {
            match glob_repo(service, repo_id, scope.clone()) {
                Response::GlobCompleted { paths, status, .. } if paths == expected => {
                    return status;
                }
                Response::GlobCompleted { .. } => {}
                other => panic!("unexpected response: {other:?}"),
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for glob paths");
    }

    fn wait_for_task_status<F>(
        service: &DaemonService,
        task_id: &str,
        predicate: F,
    ) -> TaskStatusPayload
    where
        F: Fn(&TaskStatusPayload) -> bool,
    {
        for _ in 0..TASK_WAIT_ATTEMPTS {
            let task = match service
                .handle_request_only(Request::TaskStatus {
                    params: crate::daemon::protocol::TaskRef {
                        task_id: task_id.to_string(),
                    },
                })
                .expect("task status should succeed")
            {
                Response::TaskStatus { task } => task,
                other => panic!("unexpected response: {other:?}"),
            };
            if predicate(&task) {
                return task;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("timed out waiting for task status");
    }

    fn assert_exact_dirty_stats(
        status: &RepoStatusPayload,
        modified: usize,
        deleted: usize,
        new: usize,
    ) {
        let exact = status.dirty_files.modified == modified
            && status.dirty_files.deleted == deleted
            && status.dirty_files.new == new;
        assert!(
            exact,
            "expected exact dirty stats, got modified={}, deleted={}, new={}",
            status.dirty_files.modified, status.dirty_files.deleted, status.dirty_files.new
        );
    }

    #[test]
    fn daemon_reports_missing_index_then_builds_and_searches() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("main.rs"),
            "const NAME: &str = \"WORKTREE\";\n",
        )
        .expect("write should succeed");

        let (repo_id, status) = open_repo(&service, repo_path.clone());
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::MissingIndex
        ));

        let build = service
            .handle_request_only(Request::BuildIndex {
                params: crate::daemon::protocol::RepoRef {
                    repo_id: repo_id.clone(),
                },
            })
            .expect("build should succeed");
        let status = match build {
            Response::RepoBuilt { status, .. } => status,
            other => panic!("unexpected response: {other:?}"),
        };
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyClean
        ));

        let search = search_repo(
            &service,
            &repo_id,
            "WORKTREE",
            ConsistencyMode::WorkspaceEventual,
        );

        match search {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(matches!(
                    backend,
                    SearchBackend::IndexedClean | SearchBackend::IndexedWorkspaceRepair
                ));
                assert_eq!(results.matched_lines, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn daemon_treats_corrupted_current_generation_as_missing_index() {
        let _serial = daemon_test_serial();
        let setup_service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("main.rs"),
            "const NAME: &str = \"WORKTREE\";\n",
        )
        .expect("write should succeed");

        let (repo_id, _) = open_repo(&setup_service, repo_path.clone());
        build_repo(&setup_service, &repo_id);
        let built = wait_for_status(&setup_service, &repo_id, |status| {
            matches!(
                status.phase,
                crate::daemon::protocol::RepoPhase::ReadyClean
                    | crate::daemon::protocol::RepoPhase::ReadyDirty
            )
        });

        let index_path = PathBuf::from(&built.index_path);
        let current = fs::read_to_string(IndexLayout::current_path(&index_path))
            .expect("current generation should exist");
        let layout = IndexLayout::for_generation(&index_path, current.trim());
        let (metadata, _) = read_docs_file(&layout.docs_path).expect("docs should load");
        write_docs_file(&layout.docs_path, metadata, &[]).expect("corrupt docs should succeed");

        setup_service.shutdown_all();

        let service = DaemonService::new();
        let (_, status) = open_repo(&service, repo_path);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::MissingIndex
        ));
    }

    #[test]
    fn glob_lists_paths_without_requiring_index_and_respects_scope_filters() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(repo_path.join("src")).expect("repo dir should succeed");
        fs::create_dir_all(repo_path.join("tests")).expect("repo dir should succeed");
        fs::create_dir_all(repo_path.join(".git")).expect("git dir should succeed");
        fs::write(repo_path.join(".gitignore"), "ignored.rs\n").expect("write should succeed");
        let src = repo_path.join("src/lib.rs");
        let test = repo_path.join("tests/lib.rs");
        fs::write(&src, "pub const NAME: &str = \"SRC\";\n").expect("write should succeed");
        fs::write(&test, "pub const NAME: &str = \"TEST\";\n").expect("write should succeed");
        fs::write(
            repo_path.join("ignored.rs"),
            "pub const NAME: &str = \"IGNORED\";\n",
        )
        .expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());

        let src = fs::canonicalize(src).expect("canonicalize should succeed");
        let test = fs::canonicalize(test).expect("canonicalize should succeed");

        match glob_repo(
            &service,
            &repo_id,
            PathScope {
                globs: vec!["*.rs".into()],
                types: vec!["rust".into()],
                ..PathScope::default()
            },
        ) {
            Response::GlobCompleted { paths, .. } => {
                assert_eq!(
                    paths,
                    vec![
                        src.to_string_lossy().into_owned(),
                        test.to_string_lossy().into_owned()
                    ]
                );
            }
            other => panic!("unexpected response: {other:?}"),
        }

        match glob_repo(
            &service,
            &repo_id,
            PathScope {
                roots: vec![repo_path.join("src")],
                globs: vec!["*.rs".into()],
                ..PathScope::default()
            },
        ) {
            Response::GlobCompleted { paths, .. } => {
                assert_eq!(paths, vec![src.to_string_lossy().into_owned()]);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn scope_roots_must_stay_within_repo_boundary() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(repo_path.join("src")).expect("repo dir should succeed");
        fs::write(
            repo_path.join("src/lib.rs"),
            "pub const NAME: &str = \"SRC\";\n",
        )
        .expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        let outside_root = repo_path
            .parent()
            .expect("repo should have parent")
            .to_path_buf();

        let search_error = service
            .handle_request_only(Request::Search {
                params: crate::daemon::protocol::SearchParams {
                    repo_id: repo_id.clone(),
                    query: query("SRC"),
                    scope: PathScope {
                        roots: vec![outside_root.clone()],
                        ..PathScope::default()
                    },
                    consistency: ConsistencyMode::WorkspaceEventual,
                    allow_scan_fallback: false,
                },
            })
            .expect_err("search should reject roots outside repo");
        assert!(search_error
            .to_string()
            .contains("scope root escapes repo boundary"));

        let glob_error = service
            .handle_request_only(Request::Glob {
                params: GlobParams {
                    repo_id,
                    scope: PathScope {
                        roots: vec![outside_root],
                        ..PathScope::default()
                    },
                },
            })
            .expect_err("glob should reject roots outside repo");
        assert!(glob_error
            .to_string()
            .contains("scope root escapes repo boundary"));
    }

    #[test]
    fn async_index_build_returns_task_and_completes() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(repo_path.join("main.rs"), "const NAME: &str = \"HELLO\";\n")
            .expect("write should succeed");

        let (repo_id, status) = open_repo(&service, repo_path);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::MissingIndex
        ));

        let started = service
            .handle_request_only(Request::IndexBuild {
                params: crate::daemon::protocol::RepoRef {
                    repo_id: repo_id.clone(),
                },
            })
            .expect("async build should succeed");
        let task_id = match started {
            Response::TaskStarted { task } => {
                assert_eq!(task.workspace_id, repo_id);
                assert!(matches!(
                    task.state,
                    TaskState::Queued | TaskState::Running | TaskState::Completed
                ));
                task.task_id
            }
            other => panic!("unexpected response: {other:?}"),
        };

        let task = wait_for_task_status(&service, &task_id, |task| {
            matches!(task.state, TaskState::Completed)
        });
        assert!(matches!(
            task.kind,
            crate::daemon::protocol::TaskKind::BuildIndex
        ));

        let status = wait_for_status(&service, &repo_id, |status| {
            matches!(status.phase, crate::daemon::protocol::RepoPhase::ReadyClean)
                && status.active_task_id.is_none()
        });
        assert!(status.watcher_healthy);

        match search_repo(
            &service,
            &repo_id,
            "HELLO",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted { results, .. } => {
                assert_eq!(results.matched_lines, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn eventual_search_uses_fallback_while_repo_is_still_opening_or_missing_index() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(repo_path.join("main.rs"), "const NAME: &str = \"HELLO\";\n")
            .expect("write should succeed");

        let repo_id = match service
            .handle_request_only(Request::OpenRepo {
                params: OpenRepoParams {
                    repo_path: repo_path.clone(),
                    index_path: None,
                    config: RepoConfig::default(),
                    refresh: Default::default(),
                },
            })
            .expect("open repo should succeed")
        {
            Response::RepoOpened { repo_id, .. } => repo_id,
            other => panic!("unexpected response: {other:?}"),
        };

        match search_repo(
            &service,
            &repo_id,
            "HELLO",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(matches!(
                    backend,
                    SearchBackend::RgFallback | SearchBackend::ScanFallback
                ));
                assert_eq!(results.matched_lines, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn eventual_search_avoids_stale_snapshot_while_repo_is_still_opening() {
        let _serial = daemon_test_serial();
        let setup_service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        for index in 0..256 {
            fs::write(
                repo_path.join(format!("file-{index}.rs")),
                format!("const NAME_{index}: &str = \"HELLO\";\n"),
            )
            .expect("write should succeed");
        }

        match ensure_repo(&setup_service, repo_path.clone()) {
            Response::RepoEnsured { status, .. } => {
                assert!(matches!(
                    status.phase,
                    crate::daemon::protocol::RepoPhase::ReadyClean
                        | crate::daemon::protocol::RepoPhase::ReadyDirty
                ));
                assert!(status.watcher_healthy);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let (repo_id, status) = match service
            .handle_request_only(Request::OpenRepo {
                params: OpenRepoParams {
                    repo_path: repo_path.clone(),
                    index_path: None,
                    config: RepoConfig::default(),
                    refresh: Default::default(),
                },
            })
            .expect("open repo should succeed")
        {
            Response::RepoOpened { repo_id, status } => (repo_id, status),
            other => panic!("unexpected response: {other:?}"),
        };
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::Opening
        ));

        match search_repo(
            &service,
            &repo_id,
            "HELLO",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(
                    matches!(
                        backend,
                        SearchBackend::RgFallback | SearchBackend::ScanFallback
                    ),
                    "transition search should avoid stale snapshot, got {backend:?}"
                );
                assert_eq!(results.matched_lines, 256);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn jsonrpc_initialize_and_ping_use_v2_envelope() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();

        let initialize = service.handle(RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: Some(1),
            request: Request::Initialize {
                params: InitializeParams::default(),
            },
        });
        assert_eq!(initialize.jsonrpc, "2.0");
        assert_eq!(initialize.id, Some(1));
        assert!(initialize.error.is_none());
        match initialize.result.expect("initialize result should exist") {
            Response::InitializeResult {
                protocol_version,
                capabilities,
                ..
            } => {
                assert_eq!(protocol_version, 1);
                assert!(capabilities.workspace_open);
                assert!(capabilities.search_query);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let ping = service.handle(RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: Some(2),
            request: Request::Ping,
        });
        assert_eq!(ping.jsonrpc, "2.0");
        assert_eq!(ping.id, Some(2));
        match ping.result.expect("ping result should exist") {
            Response::Pong { now_unix_secs } => {
                assert!(now_unix_secs > 0);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn ensure_repo_builds_missing_index_once() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("main.rs"),
            "const NAME: &str = \"WORKTREE\";\n",
        )
        .expect("write should succeed");

        match ensure_repo(&service, repo_path.clone()) {
            Response::RepoEnsured {
                repo_id,
                indexed_docs,
                status,
            } => {
                assert_eq!(
                    repo_id,
                    fs::canonicalize(&repo_path)
                        .expect("repo should canonicalize")
                        .to_string_lossy()
                );
                assert_eq!(indexed_docs, Some(1));
                assert!(matches!(
                    status.phase,
                    crate::daemon::protocol::RepoPhase::ReadyClean
                ));
            }
            other => panic!("unexpected response: {other:?}"),
        }

        match ensure_repo(&service, repo_path) {
            Response::RepoEnsured {
                indexed_docs,
                status,
                ..
            } => {
                assert_eq!(indexed_docs, None);
                assert!(matches!(
                    status.phase,
                    crate::daemon::protocol::RepoPhase::ReadyClean
                ));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn workspace_strict_search_repairs_dirty_worktree() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let tracked = repo_path.join("tracked.rs");
        fs::write(&tracked, "const NAME: &str = \"BASE\";\n").expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::write(&tracked, "const NAME: &str = \"DIRTY\";\n").expect("rewrite should succeed");

        let search = search_repo(
            &service,
            &repo_id,
            "DIRTY",
            ConsistencyMode::WorkspaceStrict,
        );

        match search {
            Response::SearchCompleted {
                backend,
                results,
                status,
                ..
            } => {
                assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                assert_eq!(results.matched_lines, 1);
                assert!(matches!(
                    status.phase,
                    crate::daemon::protocol::RepoPhase::ReadyDirty
                ));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn watcher_marks_dirty_paths_for_eventual_workspace_search() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let tracked = repo_path.join("tracked.rs");
        fs::write(&tracked, "const NAME: &str = \"BASE\";\n").expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::write(&tracked, "const NAME: &str = \"DIRTY\";\n").expect("rewrite should succeed");

        let status = wait_for_status(&service, &repo_id, |status| {
            matches!(status.phase, crate::daemon::protocol::RepoPhase::ReadyDirty)
                && status.dirty_files.modified == 1
                && status.dirty_files.deleted == 0
                && status.dirty_files.new == 0
        });
        assert_exact_dirty_stats(&status, 1, 0, 0);

        let search = search_repo(
            &service,
            &repo_id,
            "DIRTY",
            ConsistencyMode::WorkspaceEventual,
        );

        match search {
            Response::SearchCompleted {
                backend,
                results,
                status,
                ..
            } => {
                assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                assert_eq!(results.matched_lines, 1);
                assert_eq!(status.dirty_files.modified, 1);
                assert_eq!(status.dirty_files.deleted, 0);
                assert_eq!(status.dirty_files.new, 0);
                assert!(matches!(
                    status.phase,
                    crate::daemon::protocol::RepoPhase::ReadyDirty
                ));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn watcher_reconciles_batched_tracked_file_updates() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");

        let dirty_count = 96usize;
        for index in 0..dirty_count {
            fs::write(
                repo_path.join(format!("tracked_{index:03}.rs")),
                "const NAME: &str = \"BASE\";\n",
            )
            .expect("write should succeed");
        }

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        for index in 0..dirty_count {
            fs::write(
                repo_path.join(format!("tracked_{index:03}.rs")),
                "const NAME: &str = \"DIRTY\";\n",
            )
            .expect("rewrite should succeed");
        }

        let status = wait_for_status(&service, &repo_id, |status| {
            matches!(status.phase, crate::daemon::protocol::RepoPhase::ReadyDirty)
                && status.dirty_files.modified == dirty_count
                && status.dirty_files.deleted == 0
                && status.dirty_files.new == 0
        });
        assert_exact_dirty_stats(&status, dirty_count, 0, 0);

        let (backend, search_status) = wait_for_search_count(
            &service,
            &repo_id,
            "DIRTY",
            ConsistencyMode::WorkspaceEventual,
            dirty_count,
        );
        assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
        assert_eq!(search_status.dirty_files.modified, dirty_count);
    }

    #[test]
    fn watcher_tracks_new_text_files() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("tracked.rs"),
            "const NAME: &str = \"BASE\";\n",
        )
        .expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::write(
            repo_path.join("added.rs"),
            "const NAME: &str = \"ADDED\";\n",
        )
        .expect("write should succeed");

        let (backend, status) = wait_for_search_count(
            &service,
            &repo_id,
            "ADDED",
            ConsistencyMode::WorkspaceEventual,
            1,
        );
        assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
        assert_exact_dirty_stats(&status, 0, 0, 1);
    }

    #[test]
    fn watcher_tracks_new_text_files_inside_new_directory_without_probe() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("tracked.rs"),
            "const NAME: &str = \"BASE\";\n",
        )
        .expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        let nested = repo_path.join("nested");
        fs::create_dir_all(&nested).expect("nested dir should succeed");
        fs::write(
            nested.join("added.rs"),
            "const NAME: &str = \"NESTED_ADDED\";\n",
        )
        .expect("nested write should succeed");

        let status = wait_for_status(&service, &repo_id, |status| {
            matches!(status.phase, crate::daemon::protocol::RepoPhase::ReadyDirty)
                && status.dirty_files.modified == 0
                && status.dirty_files.deleted == 0
                && status.dirty_files.new == 1
        });
        assert_exact_dirty_stats(&status, 0, 0, 1);

        match search_repo(
            &service,
            &repo_id,
            "NESTED_ADDED",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                assert_eq!(results.matched_lines, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn watcher_tracks_deleted_files() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let deleted = repo_path.join("deleted.rs");
        fs::write(&deleted, "const NAME: &str = \"BASE\";\n").expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::remove_file(&deleted).expect("remove should succeed");

        let status = wait_for_status(&service, &repo_id, |status| {
            matches!(status.phase, crate::daemon::protocol::RepoPhase::ReadyDirty)
                && status.dirty_files.deleted == 1
        });
        assert_eq!(status.dirty_files.modified, 0);
        assert_eq!(status.dirty_files.new, 0);

        match search_repo(
            &service,
            &repo_id,
            "BASE",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend,
                results,
                status,
                ..
            } => {
                assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                assert_eq!(results.matched_lines, 0);
                assert_eq!(status.dirty_files.deleted, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn reopened_daemon_materializes_fixed_string_phrase_from_dirty_overlay() {
        let _serial = daemon_test_serial();
        let setup_service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let cpp = repo_path.join("tracked.cpp");
        let header = repo_path.join("tracked.h");
        fs::write(
            &cpp,
            "/// VersionTuple baseline line.\nconst char *Name = \"VersionTuple\";\n",
        )
        .expect("write should succeed");
        fs::write(
            &header,
            "class VersionTuple {\n  /// VersionTuple baseline line.\n};\n",
        )
        .expect("write should succeed");

        match ensure_repo(&setup_service, repo_path.clone()) {
            Response::RepoEnsured { status, .. } => {
                assert!(matches!(
                    status.phase,
                    crate::daemon::protocol::RepoPhase::ReadyClean
                ));
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let repo_id = match service
            .handle_request_only(Request::OpenRepo {
                params: OpenRepoParams {
                    repo_path: repo_path.clone(),
                    index_path: None,
                    config: RepoConfig::default(),
                    refresh: Default::default(),
                },
            })
            .expect("open repo should succeed")
        {
            Response::RepoOpened { repo_id, .. } => repo_id,
            other => panic!("unexpected response: {other:?}"),
        };

        let phrase = "VersionTuple watcher validation content gamma";
        fs::write(
            &cpp,
            "/// VersionTuple baseline line.\n/// VersionTuple watcher edit baseline.\nconst char *Name = \"VersionTuple\";\n",
        )
        .expect("write should succeed");
        fs::write(
            &header,
            "class VersionTuple {\n  /// VersionTuple watcher validation content gamma.\n  /// VersionTuple baseline line.\n};\n",
        )
        .expect("write should succeed");

        let status = match service
            .handle_request_only(Request::RefreshRepo {
                params: crate::daemon::protocol::RefreshRepoParams {
                    repo_id: repo_id.clone(),
                    force: true,
                },
            })
            .expect("refresh should succeed")
        {
            Response::RepoStatus { status } => status,
            other => panic!("unexpected response: {other:?}"),
        };
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
        ));
        assert!(status.watcher_healthy);

        let phrase_query = QuerySpec {
            pattern: regex::escape(phrase),
            patterns: vec![phrase.into()],
            fixed_strings: true,
            search_mode: SearchModeConfig::MaterializeMatches,
            ..query(phrase)
        };
        match search_repo_with_params(
            &service,
            &repo_id,
            phrase_query,
            vec![header],
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(matches!(
                    backend,
                    SearchBackend::IndexedWorkspaceRepair | SearchBackend::IndexedClean
                ));
                assert!(results.matched_lines >= 1);
                assert!(!results.hits.is_empty());
                assert!(results
                    .hits
                    .iter()
                    .any(|hit| hit.path.ends_with("tracked.h")));
                assert!(results
                    .hits
                    .iter()
                    .flat_map(|hit| hit.matches.iter())
                    .any(|matched| matched
                        .snippet
                        .contains("VersionTuple watcher validation content gamma")));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn dirty_overlay_uses_latest_contents_after_multiple_edits() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let tracked = repo_path.join("tracked.rs");
        fs::write(&tracked, "const VALUE: &str = \"BASE\";\n").expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::write(&tracked, "const VALUE: &str = \"FIRST_EDIT\";\n")
            .expect("first rewrite should succeed");
        let status = refresh_repo(&service, &repo_id, true);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        match search_repo(
            &service,
            &repo_id,
            "FIRST_EDIT",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                assert_eq!(results.matched_lines, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        fs::write(&tracked, "const VALUE: &str = \"SECOND_EDIT\";\n")
            .expect("second rewrite should succeed");
        let status = refresh_repo(&service, &repo_id, true);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        match search_repo(
            &service,
            &repo_id,
            "FIRST_EDIT",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted { results, .. } => {
                assert_eq!(results.matched_lines, 0);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        match search_repo(
            &service,
            &repo_id,
            "SECOND_EDIT",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                assert_eq!(results.matched_lines, 1);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn repeated_dirty_overlay_searches_remain_stable() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let tracked = repo_path.join("tracked.rs");
        fs::write(&tracked, "const VALUE: &str = \"BASE\";\n").expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::write(
            &tracked,
            "const VALUE: &str = \"REPEATED_DIRTY_TOKEN_20260410\";\n",
        )
        .expect("rewrite should succeed");
        let status = refresh_repo(&service, &repo_id, true);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        for _ in 0..3 {
            match search_repo(
                &service,
                &repo_id,
                "REPEATED_DIRTY_TOKEN_20260410",
                ConsistencyMode::WorkspaceEventual,
            ) {
                Response::SearchCompleted {
                    backend, results, ..
                } => {
                    assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                    assert_eq!(results.matched_lines, 1);
                }
                other => panic!("unexpected response: {other:?}"),
            }
        }
    }

    #[test]
    fn mixed_incremental_sequence_remains_searchable() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let tracked = repo_path.join("tracked.rs");
        fs::write(&tracked, "const VALUE: &str = \"BASE\";\n").expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::write(
            &tracked,
            "const VALUE: &str = \"MIXED_EXISTING_TOKEN_20260410\";\n",
        )
        .expect("rewrite should succeed");
        let status = refresh_repo(&service, &repo_id, true);
        assert!(status.dirty_files.modified >= 1);

        let _ = wait_for_search_count(
            &service,
            &repo_id,
            "MIXED_EXISTING_TOKEN_20260410",
            ConsistencyMode::WorkspaceEventual,
            1,
        );

        fs::write(&tracked, "const VALUE: &str = \"BASE\";\n").expect("revert should succeed");
        refresh_repo(&service, &repo_id, true);
        wait_for_search_count(
            &service,
            &repo_id,
            "MIXED_EXISTING_TOKEN_20260410",
            ConsistencyMode::WorkspaceEventual,
            0,
        );

        let nested = repo_path.join("nested");
        fs::create_dir_all(&nested).expect("nested dir should succeed");
        fs::write(
            nested.join("live.rs"),
            "const VALUE: &str = \"MIXED_NEWDIR_TOKEN_20260410\";\n",
        )
        .expect("nested write should succeed");
        refresh_repo(&service, &repo_id, true);

        let (_, status) = wait_for_search_count(
            &service,
            &repo_id,
            "MIXED_NEWDIR_TOKEN_20260410",
            ConsistencyMode::WorkspaceEventual,
            1,
        );
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        for _ in 0..3 {
            let (_, status) = wait_for_search_count(
                &service,
                &repo_id,
                "MIXED_NEWDIR_TOKEN_20260410",
                ConsistencyMode::WorkspaceEventual,
                1,
            );
            assert!(matches!(
                status.phase,
                crate::daemon::protocol::RepoPhase::ReadyDirty
            ));
        }
    }

    #[test]
    fn path_scoped_search_limits_dirty_overlay_results() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        let left = repo_path.join("left.rs");
        let right = repo_path.join("right.rs");
        fs::write(&left, "const VALUE: &str = \"BASE\";\n").expect("left write should succeed");
        fs::write(&right, "const VALUE: &str = \"BASE\";\n").expect("right write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        fs::write(&left, "const VALUE: &str = \"SCOPED_TOKEN\";\n")
            .expect("left rewrite should succeed");
        fs::write(&right, "const VALUE: &str = \"SCOPED_TOKEN\";\n")
            .expect("right rewrite should succeed");
        let status = refresh_repo(&service, &repo_id, true);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        let query = QuerySpec {
            search_mode: SearchModeConfig::MaterializeMatches,
            ..query("SCOPED_TOKEN")
        };
        match search_repo_with_params(
            &service,
            &repo_id,
            query,
            vec![left.clone()],
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted {
                backend, results, ..
            } => {
                assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
                assert_eq!(results.matched_lines, 1);
                assert_eq!(results.hits.len(), 1);
                assert!(results.hits[0].path.ends_with("left.rs"));
                assert!(results.hits[0]
                    .matches
                    .iter()
                    .any(|matched| matched.snippet.contains("SCOPED_TOKEN")));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn transient_new_file_does_not_leave_phantom_match_after_delete() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("tracked.rs"),
            "const VALUE: &str = \"BASE\";\n",
        )
        .expect("tracked write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        let transient = repo_path.join("transient.rs");
        fs::write(&transient, "const VALUE: &str = \"EPHEMERAL_TOKEN\";\n")
            .expect("transient write should succeed");
        fs::remove_file(&transient).expect("transient delete should succeed");

        let status = refresh_repo(&service, &repo_id, true);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyClean
                | crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        match search_repo(
            &service,
            &repo_id,
            "EPHEMERAL_TOKEN",
            ConsistencyMode::WorkspaceEventual,
        ) {
            Response::SearchCompleted { results, .. } => {
                assert_eq!(results.matched_lines, 0);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn watcher_removes_overlay_only_new_file_without_refresh() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("tracked.rs"),
            "const VALUE: &str = \"BASE\";\n",
        )
        .expect("tracked write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        let nested = repo_path.join("nested");
        fs::create_dir_all(&nested).expect("nested dir should succeed");
        let transient = nested.join("transient.rs");
        fs::write(
            &transient,
            "const VALUE: &str = \"WATCHER_DELETE_TOKEN_20260410\";\n",
        )
        .expect("transient write should succeed");

        let (backend, status) = wait_for_search_count(
            &service,
            &repo_id,
            "WATCHER_DELETE_TOKEN_20260410",
            ConsistencyMode::WorkspaceEventual,
            1,
        );
        assert!(matches!(backend, SearchBackend::IndexedWorkspaceRepair));
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        fs::remove_file(&transient).expect("transient delete should succeed");
        fs::remove_dir(&nested).expect("nested dir delete should succeed");

        let (backend, status) = wait_for_search_count(
            &service,
            &repo_id,
            "WATCHER_DELETE_TOKEN_20260410",
            ConsistencyMode::WorkspaceEventual,
            0,
        );
        assert!(matches!(
            backend,
            SearchBackend::IndexedClean | SearchBackend::IndexedWorkspaceRepair
        ));
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyClean
                | crate::daemon::protocol::RepoPhase::ReadyDirty
        ));

        let status = wait_for_status(&service, &repo_id, |status| {
            status.dirty_files.modified == 0
                && status.dirty_files.deleted == 0
                && status.dirty_files.new == 0
        });
        assert_exact_dirty_stats(&status, 0, 0, 0);

        for _ in 0..3 {
            match search_repo(
                &service,
                &repo_id,
                "WATCHER_DELETE_TOKEN_20260410",
                ConsistencyMode::WorkspaceEventual,
            ) {
                Response::SearchCompleted { results, .. } => {
                    assert_eq!(results.matched_lines, 0);
                }
                other => panic!("unexpected response: {other:?}"),
            }
        }
    }

    #[test]
    fn watcher_marks_directory_events_for_resync() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        fs::write(
            repo_path.join("tracked.rs"),
            "const NAME: &str = \"BASE\";\n",
        )
        .expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);

        let baseline_probe = get_status(&service, &repo_id).last_probe_unix_secs;
        thread::sleep(Duration::from_secs(1));

        fs::create_dir_all(repo_path.join("nested")).expect("mkdir should succeed");

        let status = wait_for_status(&service, &repo_id, |status| {
            matches!(status.phase, crate::daemon::protocol::RepoPhase::ReadyClean)
                && status.dirty_files.modified == 0
                && status.dirty_files.deleted == 0
                && status.dirty_files.new == 0
                && status.last_probe_unix_secs != baseline_probe
        });
        assert_eq!(status.dirty_files.modified, 0);
        assert_eq!(status.dirty_files.deleted, 0);
        assert_eq!(status.dirty_files.new, 0);

        let refreshed = service
            .handle_request_only(Request::RefreshRepo {
                params: crate::daemon::protocol::RefreshRepoParams {
                    repo_id,
                    force: false,
                },
            })
            .expect("refresh should succeed");

        match refreshed {
            Response::RepoStatus { status } => {
                assert!(matches!(
                    status.phase,
                    crate::daemon::protocol::RepoPhase::ReadyClean
                ));
                assert_eq!(status.dirty_files.modified, 0);
                assert_eq!(status.dirty_files.deleted, 0);
                assert_eq!(status.dirty_files.new, 0);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn watcher_updates_invalidate_glob_cache() {
        let _serial = daemon_test_serial();
        let service = DaemonService::new();
        let temp = tempdir().expect("temp dir should succeed");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(repo_path.join("src")).expect("repo dir should succeed");
        let stable = repo_path.join("src/lib.rs");
        fs::write(&stable, "pub const NAME: &str = \"STABLE\";\n").expect("write should succeed");

        let (repo_id, _) = open_repo(&service, repo_path.clone());
        build_repo(&service, &repo_id);
        let stable = fs::canonicalize(&stable)
            .expect("canonicalize should succeed")
            .to_string_lossy()
            .into_owned();
        let scope = PathScope {
            globs: vec!["*.rs".into()],
            ..PathScope::default()
        };

        match glob_repo(&service, &repo_id, scope.clone()) {
            Response::GlobCompleted { paths, .. } => {
                assert_eq!(paths, vec![stable.clone()]);
            }
            other => panic!("unexpected response: {other:?}"),
        }

        let fresh = repo_path.join("src/fresh.rs");
        fs::write(&fresh, "pub const NAME: &str = \"FRESH\";\n").expect("write should succeed");
        let fresh = fs::canonicalize(&fresh)
            .expect("canonicalize should succeed")
            .to_string_lossy()
            .into_owned();

        let status = wait_for_glob_paths(
            &service,
            &repo_id,
            scope.clone(),
            &[fresh.clone(), stable.clone()],
        );
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
                | crate::daemon::protocol::RepoPhase::ReadyClean
        ));

        fs::remove_file(repo_path.join("src/fresh.rs")).expect("delete should succeed");
        let status = wait_for_glob_paths(&service, &repo_id, scope, &[stable.clone()]);
        assert!(matches!(
            status.phase,
            crate::daemon::protocol::RepoPhase::ReadyDirty
                | crate::daemon::protocol::RepoPhase::ReadyClean
        ));
    }
}
