use codgrep::{advanced::PathFilter, advanced::PathFilterArgs, QueryConfig, SearchMode};

use crate::common::{count_matches_query, count_query, query, TestRepo};

#[test]
fn candidate_paths_exports_prefiltered_docs() {
    let repo = TestRepo::new();
    let source = repo.write(
        "service.rs",
        "fn load() { let value = fuzzy_file_search(); }\n",
    );
    repo.write("other.rs", "fn noop() { let value = 42; }\n");

    repo.build_sparse();

    let candidates = repo
        .searcher()
        .candidate_paths(&query("fuzzy_file_search"))
        .expect("test should succeed");

    assert_eq!(candidates, vec![source.to_string_lossy().to_string()]);
}

#[test]
fn build_respects_gitignore_by_default() {
    let repo = TestRepo::new();
    repo.create_dir(".git");
    repo.write(".gitignore", "ignored.rs\n");
    repo.write("ignored.rs", "const NAME: &str = \"PM_RESUME\";\n");
    repo.write("tracked.rs", "const NAME: &str = \"OTHER\";\n");

    repo.build();

    let results = repo
        .searcher()
        .search(&count_query("PM_RESUME"))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 0);
    assert_eq!(results.matched_lines, 0);
}

#[test]
fn build_can_disable_ignore_rules() {
    let repo = TestRepo::new();
    repo.create_dir(".git");
    repo.write(".gitignore", "ignored.rs\n");
    let ignored = repo.write("ignored.rs", "const NAME: &str = \"PM_RESUME\";\n");

    repo.build_with(codgrep::BuildConfig {
        corpus_mode: codgrep::CorpusMode::NoIgnore,
        ..repo.trigram_build_config()
    });

    let searcher = repo.searcher();
    let query = count_query("PM_RESUME");
    let results = searcher.search(&query).expect("test should succeed");

    assert_eq!(results.candidate_docs, 1);
    assert_eq!(results.matched_lines, 1);
    assert!(results.hits.is_empty());
    assert_eq!(
        searcher
            .candidate_paths(&query)
            .expect("test should succeed"),
        vec![ignored.to_string_lossy().to_string()]
    );
}

#[test]
fn build_skips_hidden_files_by_default() {
    let repo = TestRepo::new();
    repo.write(".secret.rs", "const NAME: &str = \"PM_RESUME\";\n");

    repo.build();

    let counts = repo
        .searcher()
        .count_matches_by_file(&count_query("PM_RESUME"), None)
        .expect("test should succeed");

    assert!(counts.is_empty());
}

#[test]
fn build_can_include_hidden_files() {
    let repo = TestRepo::new();
    let hidden = repo.write(".secret.rs", "const NAME: &str = \"PM_RESUME\";\n");

    repo.build_with(codgrep::BuildConfig {
        include_hidden: true,
        ..repo.trigram_build_config()
    });

    let counts = repo
        .searcher()
        .count_matches_by_file(&count_query("PM_RESUME"), None)
        .expect("test should succeed");

    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].path, hidden.to_string_lossy());
    assert_eq!(counts[0].matched_lines, 1);
}

#[test]
fn search_can_be_restricted_to_selected_paths() {
    let repo = TestRepo::new();
    let src = repo.write("src/lib.rs", "const NAME: &str = \"PM_RESUME\";\n");
    let tests = repo.write("tests/lib.rs", "const NAME: &str = \"PM_RESUME\";\n");

    repo.build();

    let searcher = repo.searcher();
    let query = query("PM_RESUME");
    let cwd = std::env::current_dir().expect("test should succeed");

    let src_filter = PathFilter::new(
        PathFilterArgs {
            roots: vec![repo.path("src")],
            ..PathFilterArgs::default()
        },
        &cwd,
    )
    .expect("test should succeed");
    let results = searcher
        .search_with_filter(&query, Some(&src_filter))
        .expect("test should succeed");
    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].path, src.to_string_lossy());

    let tests_filter = PathFilter::new(
        PathFilterArgs {
            roots: vec![repo.path("tests")],
            ..PathFilterArgs::default()
        },
        &cwd,
    )
    .expect("test should succeed");
    let counts = searcher
        .count_matches_by_file(
            &QueryConfig {
                search_mode: SearchMode::CountOnly,
                ..query.clone()
            },
            Some(&tests_filter),
        )
        .expect("test should succeed");
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].path, tests.to_string_lossy());
    assert_eq!(counts[0].matched_lines, 1);
}

#[test]
fn search_can_be_restricted_to_exact_file_path() {
    let repo = TestRepo::new();
    let source = repo.write("input.txt", "abc");
    repo.write("other.txt", "zzz");

    repo.build();

    let searcher = repo.searcher();
    let cwd = std::env::current_dir().expect("test should succeed");
    let file_filter = PathFilter::new(
        PathFilterArgs {
            roots: vec![source.clone()],
            ..PathFilterArgs::default()
        },
        &cwd,
    )
    .expect("test should succeed");

    assert_eq!(
        searcher.indexed_paths(Some(&file_filter)),
        vec![source.to_string_lossy()]
    );

    let literal = searcher
        .search_with_filter(&query("abc"), Some(&file_filter))
        .expect("test should succeed");
    assert_eq!(literal.hits.len(), 1);
    assert_eq!(literal.hits[0].path, source.to_string_lossy());

    let regex = searcher
        .search(&query("a(.*c)"))
        .expect("test should succeed");
    assert_eq!(regex.hits.len(), 1);
    assert_eq!(regex.hits[0].path, source.to_string_lossy());

    let regex = searcher
        .search_with_filter(&query("a(.*c)"), Some(&file_filter))
        .expect("test should succeed");
    assert_eq!(regex.hits.len(), 1);
    assert_eq!(regex.hits[0].path, source.to_string_lossy());
}

#[test]
fn files_without_matches_are_exposed_via_searcher_api() {
    let repo = TestRepo::new();
    repo.write("matched.rs", "const NAME: &str = \"PM_RESUME\";\n");
    let missed = repo.write("missed.rs", "const NAME: &str = \"OTHER\";\n");

    repo.build();

    let without_matches = repo
        .searcher()
        .files_without_matches(&count_query("PM_RESUME"), None)
        .expect("test should succeed");

    assert_eq!(without_matches, vec![missed.to_string_lossy().to_string()]);
}

#[test]
fn count_total_matches_are_exposed_via_searcher_api() {
    let repo = TestRepo::new();
    let source = repo.write("matches.rs", "PM_RESUME PM_RESUME\nPM_RESUME\n");

    repo.build();

    let query = count_matches_query("PM_RESUME");
    let searcher = repo.searcher();
    let results = searcher.search(&query).expect("test should succeed");
    assert_eq!(results.matched_lines, 2);
    assert_eq!(results.matched_occurrences, 3);
    assert!(results.hits.is_empty());

    let counts = searcher
        .count_total_matches_by_file(&query, None)
        .expect("test should succeed");
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].path, source.to_string_lossy());
    assert_eq!(counts[0].matched_occurrences, 3);
}

#[test]
fn include_zero_counts_are_exposed_via_searcher_api() {
    let repo = TestRepo::new();
    let matched = repo.write("matched.rs", "PM_RESUME PM_RESUME\n");
    let missed = repo.write("missed.rs", "OTHER\n");

    repo.build();

    let searcher = repo.searcher();
    let count_query = count_query("PM_RESUME");
    let count_matches_query = count_matches_query("PM_RESUME");

    let line_counts = searcher
        .count_matches_by_file_including_zero(&count_query, None)
        .expect("test should succeed");
    assert_eq!(line_counts.len(), 2);
    assert_eq!(line_counts[0].path, matched.to_string_lossy());
    assert_eq!(line_counts[0].matched_lines, 1);
    assert_eq!(line_counts[1].path, missed.to_string_lossy());
    assert_eq!(line_counts[1].matched_lines, 0);

    let total_counts = searcher
        .count_total_matches_by_file_including_zero(&count_matches_query, None)
        .expect("test should succeed");
    assert_eq!(total_counts.len(), 2);
    assert_eq!(total_counts[0].path, matched.to_string_lossy());
    assert_eq!(total_counts[0].matched_occurrences, 2);
    assert_eq!(total_counts[1].path, missed.to_string_lossy());
    assert_eq!(total_counts[1].matched_occurrences, 0);
}
