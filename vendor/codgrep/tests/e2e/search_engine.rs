use std::process::Command;

use codgrep::{BuildConfig, SearchBackend, SearchEngine, SearchPolicy, TokenizerMode};

use crate::common::{count_matches_query, query, TestRepo};

#[test]
fn search_engine_uses_index_when_fresh() {
    let repo = TestRepo::new();
    let source = repo.write("service.rs", "const NAME: &str = \"PM_RESUME\";\n");
    repo.build();

    let engine = SearchEngine::new(repo.trigram_build_config());
    let outcome = engine
        .search(&query("PM_RESUME"), SearchPolicy::FallbackToScan)
        .expect("test should succeed");

    assert_eq!(outcome.backend, SearchBackend::Index);
    assert_eq!(outcome.results.hits.len(), 1);
    assert_eq!(outcome.results.hits[0].path, source.to_string_lossy());
}

#[test]
fn search_engine_uses_index_without_checking_freshness() {
    let repo = TestRepo::new();
    let tracked = repo.write("tracked.rs", "const NAME: &str = \"PM_RESUME\";\n");
    repo.build();
    let new_file = repo.write("new.rs", "const NAME: &str = \"PM_RESUME\";\n");

    let engine = SearchEngine::new(repo.trigram_build_config());
    let outcome = engine
        .search(&query("PM_RESUME"), SearchPolicy::FallbackToScan)
        .expect("test should succeed");

    assert_eq!(outcome.backend, SearchBackend::Index);
    assert_eq!(outcome.results.hits.len(), 1);
    assert_eq!(outcome.results.hits[0].path, tracked.to_string_lossy());
    assert_ne!(outcome.results.hits[0].path, new_file.to_string_lossy());
}

#[test]
fn search_engine_scans_when_index_is_missing() {
    let repo = TestRepo::new();
    let source = repo.write("service.rs", "const NAME: &str = \"PM_RESUME\";\n");

    let engine = SearchEngine::new(repo.trigram_build_config());
    let outcome = engine
        .search(&query("PM_RESUME"), SearchPolicy::FallbackToScan)
        .expect("test should succeed");

    assert_eq!(outcome.backend, SearchBackend::Scan);
    assert_eq!(outcome.results.hits.len(), 1);
    assert_eq!(outcome.results.hits[0].path, source.to_string_lossy());
}

#[test]
fn search_engine_scans_count_matches_when_index_is_missing() {
    let repo = TestRepo::new();
    repo.write("service.rs", "PM_RESUME PM_RESUME\nPM_RESUME\n");

    let engine = SearchEngine::new(repo.trigram_build_config());
    let outcome = engine
        .search(
            &count_matches_query("PM_RESUME"),
            SearchPolicy::FallbackToScan,
        )
        .expect("test should succeed");

    assert_eq!(outcome.backend, SearchBackend::Scan);
    assert_eq!(outcome.results.matched_lines, 2);
    assert_eq!(outcome.results.matched_occurrences, 3);
    assert!(outcome.results.hits.is_empty());
}

#[test]
fn search_engine_scan_count_matches_honors_max_count_by_matching_line() {
    let repo = TestRepo::new();
    repo.write("service.rs", "PM_RESUME PM_RESUME\nPM_RESUME\n");

    let engine = SearchEngine::new(repo.trigram_build_config());
    let mut query = count_matches_query("PM_RESUME");
    query.max_count = Some(1);

    let outcome = engine
        .search(&query, SearchPolicy::FallbackToScan)
        .expect("test should succeed");

    assert_eq!(outcome.backend, SearchBackend::Scan);
    assert_eq!(outcome.results.matched_lines, 1);
    assert_eq!(outcome.results.matched_occurrences, 2);
    assert!(outcome.results.hits.is_empty());
}

#[test]
fn search_engine_reports_worktree_diff_separately() {
    let repo = TestRepo::new();
    repo.write("tracked.rs", "const NAME: &str = \"PM_RESUME\";\n");
    repo.build();
    let new_file = repo.write("new.rs", "const NAME: &str = \"PM_RESUME\";\n");

    let engine = SearchEngine::new(repo.trigram_build_config());
    let diff = engine.diff_against_worktree().expect("test should succeed");

    assert!(diff
        .new_files
        .contains(&new_file.to_string_lossy().into_owned()));
    assert!(engine
        .stale_reason()
        .expect("test should succeed")
        .is_some());
}

#[test]
fn search_engine_uses_index_by_default_when_preflight_is_disabled() {
    let repo = TestRepo::new();
    repo.write("words.txt", "alpha beta gamma\n");
    repo.build();

    let engine = SearchEngine::new(repo.trigram_build_config());
    let outcome = engine
        .search(&query("\\w+\\s+\\w+"), SearchPolicy::FallbackToScan)
        .expect("test should succeed");

    assert_eq!(outcome.backend, SearchBackend::Index);
    assert_eq!(outcome.results.hits.len(), 1);
}

#[test]
fn search_engine_keeps_relative_build_paths_fresh_after_reopen() {
    let repo = TestRepo::new();
    let source = repo.write("service.rs", "const NAME: &str = \"PM_RESUME\";\n");
    let repo_root = repo
        .repo
        .parent()
        .expect("temp repo should have a parent")
        .to_path_buf();
    let index_path = repo_root.join("idx");

    let status = Command::new(env!("CARGO_BIN_EXE_cg"))
        .current_dir(&repo_root)
        .arg("build")
        .arg("--repo")
        .arg("repo")
        .arg("--index")
        .arg("idx")
        .arg("--tokenizer")
        .arg("trigram")
        .status()
        .expect("test should succeed");
    assert!(status.success());

    let engine = SearchEngine::new(BuildConfig {
        repo_path: repo.repo.clone(),
        index_path,
        tokenizer: TokenizerMode::Trigram,
        ..repo.trigram_build_config()
    });
    let diff = engine.diff_against_worktree().expect("test should succeed");
    assert!(diff.is_empty());

    let outcome = engine
        .search(&query("PM_RESUME"), SearchPolicy::FallbackToScan)
        .expect("test should succeed");
    assert_eq!(outcome.backend, SearchBackend::Index);
    assert_eq!(outcome.results.hits.len(), 1);
    assert_eq!(outcome.results.hits[0].path, source.to_string_lossy());
}
