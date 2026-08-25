#![cfg(feature = "workspace-search")]

use std::path::Path;
use std::time::{Duration, Instant};

use bitfun_services_integrations::workspace_search::{
    resolve_workspace_search_daemon_program_path, workspace_search_daemon_binary_name,
    workspace_search_daemon_binary_names, workspace_search_daemon_missing_hint,
    ContentSearchOutputMode, ContentSearchRequest, WorkspaceSearchService,
};

#[test]
fn daemon_binary_contract_lists_current_platform_candidate() {
    let primary = workspace_search_daemon_binary_name();

    assert!(!primary.is_empty());
    assert!(workspace_search_daemon_binary_names().contains(&primary));
}

#[test]
fn daemon_missing_hint_preserves_env_override_guidance() {
    let hint = workspace_search_daemon_missing_hint();

    assert!(hint.contains("FLASHGREP_DAEMON_BIN"));
    assert!(hint.contains("flashgrep/"));
    assert!(hint.contains(workspace_search_daemon_binary_name()));
}

#[test]
fn service_constructs_without_core_runtime_dependencies() {
    let _service = WorkspaceSearchService::new();
}

/// Live end-to-end check that content matches carry real line text.
///
/// The flashgrep daemon returns match positions only, so content output is
/// hydrated from disk. This test spawns the real daemon, so it is ignored by
/// default; run it with
/// `cargo test -p bitfun-services-integrations --features workspace-search
///  --test workspace_search_contracts -- --ignored`.
#[tokio::test]
#[ignore = "spawns the real flashgrep daemon and indexes a temporary repository"]
async fn content_search_hydrates_real_line_text_and_previews() {
    let Some(daemon) = resolve_workspace_search_daemon_program_path() else {
        panic!(
            "flashgrep daemon binary not found: {}",
            workspace_search_daemon_missing_hint()
        );
    };
    println!("using daemon: {}", daemon.display());

    let repo = tempfile::tempdir().expect("temp repo should be created");
    let repo_root = repo.path().canonicalize().expect("repo root canonicalizes");
    git(&repo_root, &["init"]);
    git(&repo_root, &["config", "user.email", "test@example.com"]);
    git(&repo_root, &["config", "user.name", "test"]);
    std::fs::write(
        repo_root.join("needle.rs"),
        "fn main() {}\nlet answer = compute_needle_value(41 + 1);\n",
    )
    .expect("fixture file should be written");
    git(&repo_root, &["add", "."]);
    git(&repo_root, &["commit", "-m", "fixture"]);

    let service = WorkspaceSearchService::new();
    let handle = service
        .build_index(&repo_root)
        .await
        .expect("index build should start");
    println!("index task: {:?}", handle.task.state);

    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        let status = service
            .get_index_status(&repo_root)
            .await
            .expect("index status should be readable");
        if status.active_task.is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let result = service
        .search_content(ContentSearchRequest {
            repo_root: repo_root.clone(),
            search_path: None,
            pattern: "compute_needle_value".to_string(),
            output_mode: ContentSearchOutputMode::Content,
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            multiline: false,
            max_results: None,
            globs: Vec::new(),
            file_types: Vec::new(),
            exclude_file_types: Vec::new(),
        })
        .await
        .expect("content search should succeed");

    println!(
        "backend={:?} results={:?}",
        result.backend, result.outcome.results
    );
    let matched = result
        .outcome
        .results
        .first()
        .expect("content search should return the fixture match");
    assert_eq!(matched.line_number, Some(2));
    assert_eq!(
        matched.matched_content.as_deref(),
        Some("let answer = compute_needle_value(41 + 1);")
    );
    assert_eq!(
        matched.preview_inside.as_deref(),
        Some("compute_needle_value")
    );
    assert_eq!(matched.preview_before.as_deref(), Some("let answer = "));
    assert_eq!(matched.preview_after.as_deref(), Some("(41 + 1);"));

    service.remove_workspace_index(&repo_root).await;
    service.shutdown_all_daemons().await;
}

#[cfg(test)]
fn git(repo_root: &Path, args: &[&str]) {
    let output = bitfun_services_core::process_manager::create_command("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .expect("git should be available");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
