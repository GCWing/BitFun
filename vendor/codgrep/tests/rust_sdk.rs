use std::fs;

use codgrep::sdk::{
    count_only_query, EnsureRepoParams, GlobRequest, ManagedClient, OpenRepoParams, PathScope,
    RepoConfig, RepoEvent, RepoPhase, SearchBackend, SearchRequest, TaskState,
};
use tempfile::tempdir;

#[test]
fn rust_sdk_manages_repo_and_queries_daemon() {
    let temp = tempdir().expect("temp dir should succeed");
    let repo_path = temp.path().join("repo");
    fs::create_dir_all(repo_path.join("src")).expect("repo dir should succeed");
    fs::create_dir_all(repo_path.join("tests")).expect("repo dir should succeed");
    fs::write(
        repo_path.join("src/lib.rs"),
        "const NAME: &str = \"HELLO\";\n",
    )
    .expect("write should succeed");
    fs::write(
        repo_path.join("tests/lib.rs"),
        "const NAME: &str = \"TEST\";\n",
    )
    .expect("write should succeed");

    let client = ManagedClient::new().with_daemon_program(env!("CARGO_BIN_EXE_cg"));
    let repo = client
        .ensure_repo(EnsureRepoParams {
            repo_path: repo_path.clone(),
            index_path: None,
            config: RepoConfig::default(),
            refresh: Default::default(),
        })
        .expect("ensure should succeed");

    let ensured = repo
        .ensured_repo()
        .expect("ensure should populate metadata");
    assert_eq!(ensured.repo_id, repo.repo_id());
    assert!(matches!(
        ensured.status.phase,
        RepoPhase::ReadyClean | RepoPhase::ReadyDirty
    ));

    let search = repo
        .search(SearchRequest::new(count_only_query("HELLO")))
        .expect("search should succeed");
    assert!(matches!(
        search.backend,
        SearchBackend::IndexedClean | SearchBackend::IndexedWorkspaceRepair
    ));
    assert_eq!(search.results.matched_lines, 1);

    let glob = repo
        .glob(GlobRequest {
            scope: PathScope {
                roots: vec![repo_path.join("src")],
                globs: vec!["*.rs".into()],
                ..PathScope::default()
            },
        })
        .expect("glob should succeed");
    assert_eq!(
        glob.paths,
        vec![fs::canonicalize(repo_path.join("src/lib.rs"))
            .expect("canonicalize should succeed")
            .to_string_lossy()
            .into_owned()]
    );

    repo.shutdown_daemon().expect("shutdown should succeed");
}

#[test]
fn rust_sdk_supports_open_then_manual_index_build() {
    let temp = tempdir().expect("temp dir should succeed");
    let repo_path = temp.path().join("repo");
    fs::create_dir_all(&repo_path).expect("repo dir should succeed");
    fs::write(repo_path.join("main.rs"), "const NAME: &str = \"HELLO\";\n")
        .expect("write should succeed");

    let client = ManagedClient::new().with_daemon_program(env!("CARGO_BIN_EXE_cg"));
    let repo = client
        .open_repo(OpenRepoParams {
            repo_path: repo_path.clone(),
            index_path: None,
            config: RepoConfig::default(),
            refresh: Default::default(),
        })
        .expect("open should succeed");

    let opened = repo.opened_repo().expect("open should populate metadata");
    assert_eq!(opened.repo_id, repo.repo_id());
    assert!(repo.ensured_repo().is_none());

    let task = repo.index_build().expect("async build should start");
    let task = repo
        .wait_task(task.task_id.clone(), std::time::Duration::from_secs(10))
        .expect("task should finish");
    assert!(matches!(task.state, TaskState::Completed));

    let search = repo
        .search(SearchRequest::new(count_only_query("HELLO")))
        .expect("search should succeed after build");
    assert_eq!(search.results.matched_lines, 1);

    repo.shutdown_daemon().expect("shutdown should succeed");
}

#[test]
fn rust_sdk_can_subscribe_to_progress_notifications() {
    let temp = tempdir().expect("temp dir should succeed");
    let repo_path = temp.path().join("repo");
    fs::create_dir_all(repo_path.join("src")).expect("repo dir should succeed");
    fs::write(
        repo_path.join("src/lib.rs"),
        "const NAME: &str = \"HELLO\";\n",
    )
    .expect("write should succeed");

    let client = ManagedClient::new().with_daemon_program(env!("CARGO_BIN_EXE_cg"));
    let repo = client
        .open_repo(OpenRepoParams {
            repo_path,
            index_path: None,
            config: RepoConfig::default(),
            refresh: Default::default(),
        })
        .expect("open should succeed");

    let mut events = repo
        .subscribe_events()
        .expect("event subscription should succeed");
    let task = repo.index_build().expect("async build should start");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut saw_progress = false;
    let mut saw_status_changed = false;
    let mut saw_task_finished = false;
    while std::time::Instant::now() < deadline {
        let Some(event) = events
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("recv should succeed")
        else {
            continue;
        };
        match event {
            RepoEvent::Progress(params) => {
                if params.task_id == task.task_id {
                    saw_progress = true;
                }
            }
            RepoEvent::WorkspaceStatusChanged(_) => {
                saw_status_changed = true;
            }
            RepoEvent::TaskFinished(params) => {
                if params.task.task_id == task.task_id {
                    saw_task_finished = true;
                    break;
                }
            }
        }
    }

    assert!(saw_progress, "expected progress notification");
    assert!(
        saw_status_changed,
        "expected workspace/statusChanged notification"
    );
    assert!(saw_task_finished, "expected task/finished notification");

    repo.shutdown_daemon().expect("shutdown should succeed");
}
