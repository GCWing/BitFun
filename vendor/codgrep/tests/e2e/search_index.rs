use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::Command,
    thread,
    time::{Duration, SystemTime},
};

use codgrep::{
    advanced::{repo_relative_path, IndexSearcher},
    build_index,
    experimental::index_format::{
        read_doc_terms_file, read_docs_file, write_docs_file, IndexLayout,
    },
    rebuild_index, BuildConfig, QueryConfig, SearchMode, TokenizerMode,
};
use tempfile::tempdir;

use crate::common::{count_matches_query, count_query, query, TestRepo};

fn rewrite_with_fresh_mtime(path: &Path, contents: &str) {
    let before = fs::metadata(path)
        .expect("test should succeed")
        .modified()
        .expect("test should succeed");

    for _ in 0..20 {
        thread::sleep(Duration::from_millis(10));
        fs::write(path, contents).expect("test should succeed");

        let after = fs::metadata(path)
            .expect("test should succeed")
            .modified()
            .expect("test should succeed");
        if after > before {
            return;
        }
    }

    panic!(
        "mtime did not advance after rewriting {} from {:?}",
        path.display(),
        SystemTime::now()
    );
}

fn run_git_in(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed in {}: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_config_defaults_to_sparse_ngram() {
    assert_eq!(BuildConfig::default().tokenizer, TokenizerMode::SparseNgram);
}

#[test]
fn build_and_search_repo() {
    let repo = TestRepo::new();
    let service = repo.write(
        "service.rs",
        "fn load() {\n    let value = getUserById(id);\n}\n",
    );
    repo.write("other.rs", "fn noop() {\n    let value = 42;\n}\n");

    repo.build();

    let results = repo
        .searcher()
        .search(&query("get(User|Account)ById"))
        .expect("test should succeed");

    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].path, service.to_string_lossy());
    assert_eq!(results.hits[0].matches[0].location.line, 2);
    assert_eq!(results.hits[0].matches[0].matched_text, "getUserById");
}

#[test]
fn incremental_build_detects_same_size_rewrite_and_reuses_unchanged_doc_terms() {
    let repo = TestRepo::new();
    let stable = repo.write("stable.rs", "const NAME: &str = \"BETA\";\n");
    let rewritten = repo.write("rewritten.rs", "const NAME: &str = \"ALPHA\";\n");

    let build = repo.trigram_build_config();
    repo.build_with(build.clone());

    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let (_, docs_before) = read_docs_file(&layout.docs_path).expect("test should succeed");
    let terms_before = read_doc_terms_file(&layout.doc_terms_path).expect("test should succeed");
    let before = docs_before
        .into_iter()
        .zip(terms_before)
        .map(|(doc, token_hashes)| (doc.path, token_hashes))
        .collect::<BTreeMap<_, _>>();

    rewrite_with_fresh_mtime(&rewritten, "const NAME: &str = \"OMEGA\";\n");
    repo.build_with(build);

    let searcher = repo.searcher();
    let alpha = searcher
        .search(&count_query("ALPHA"))
        .expect("test should succeed");
    assert_eq!(alpha.candidate_docs, 0);
    assert_eq!(alpha.matched_lines, 0);

    let omega = searcher
        .search(&count_query("OMEGA"))
        .expect("test should succeed");
    assert_eq!(omega.candidate_docs, 1);
    assert_eq!(omega.matched_lines, 1);

    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let (_, docs_after) = read_docs_file(&layout.docs_path).expect("test should succeed");
    let terms_after = read_doc_terms_file(&layout.doc_terms_path).expect("test should succeed");
    let after = docs_after
        .into_iter()
        .zip(terms_after)
        .map(|(doc, token_hashes)| (doc.path, token_hashes))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        before[repo_relative_path(&stable, &repo.repo).as_str()],
        after[repo_relative_path(&stable, &repo.repo).as_str()]
    );
    assert_ne!(
        before[repo_relative_path(&rewritten, &repo.repo).as_str()],
        after[repo_relative_path(&rewritten, &repo.repo).as_str()]
    );
    assert_eq!(after.len(), 2);
}

#[test]
fn build_writes_current_generation_pointer() {
    let repo = TestRepo::new();
    repo.write("service.rs", "const NAME: &str = \"PM_RESUME\";\n");

    repo.build();

    let current_path = IndexLayout::current_path(&repo.index);
    let generation = std::fs::read_to_string(&current_path).expect("test should succeed");
    let generation = generation.trim();
    assert!(generation.starts_with("base-repo-"));

    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    assert_ne!(layout.data_path, repo.index);
    assert!(layout.docs_path.exists());
    assert_eq!(
        layout.data_path,
        IndexLayout::generations_dir(&repo.index).join(generation)
    );
}

#[test]
fn build_reuses_same_generation_for_same_git_head_and_config() {
    let repo = TestRepo::new();
    repo.init_git();
    repo.write("service.rs", "const NAME: &str = \"PM_RESUME\";\n");
    repo.commit_all("initial");

    let build = repo.trigram_build_config();
    repo.build_with(build.clone());
    let head = repo.git_head();
    let current_path = IndexLayout::current_path(&repo.index);
    let first_generation = std::fs::read_to_string(&current_path).expect("test should succeed");
    let first_generation = first_generation.trim().to_string();
    assert!(first_generation.starts_with(&format!("base-git-{head}-")));

    repo.build_with(build);
    let second_generation = std::fs::read_to_string(&current_path).expect("test should succeed");
    let second_generation = second_generation.trim().to_string();
    assert_eq!(first_generation, second_generation);

    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let (metadata, _) = read_docs_file(&layout.docs_path).expect("test should succeed");
    let build_settings = metadata.build.expect("build metadata should exist");
    assert_eq!(build_settings.head_commit.as_deref(), Some(head.as_str()));
    assert_eq!(
        build_settings.config_fingerprint.as_deref(),
        Some(
            first_generation
                .rsplit('-')
                .next()
                .expect("fingerprint should exist")
        )
    );
}

#[test]
fn build_keeps_commit_snapshot_when_git_worktree_is_dirty() {
    let repo = TestRepo::new();
    repo.init_git();
    repo.write("service.rs", "const NAME: &str = \"BASE_NAME\";\n");
    repo.commit_all("initial");

    repo.build();
    let generation = std::fs::read_to_string(IndexLayout::current_path(&repo.index))
        .expect("test should succeed");
    let generation = generation.trim().to_string();
    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let docs_mtime_before = std::fs::metadata(&layout.docs_path)
        .expect("test should succeed")
        .modified()
        .expect("test should succeed");

    rewrite_with_fresh_mtime(
        &repo.path("service.rs"),
        "const NAME: &str = \"DIRTY_NAME\";\n",
    );
    repo.build();

    let current_generation = std::fs::read_to_string(IndexLayout::current_path(&repo.index))
        .expect("test should succeed");
    assert_eq!(current_generation.trim(), generation);
    let docs_mtime_after = std::fs::metadata(&layout.docs_path)
        .expect("test should succeed")
        .modified()
        .expect("test should succeed");
    assert_eq!(docs_mtime_before, docs_mtime_after);
}

#[test]
fn rebuild_refreshes_dirty_git_snapshot_even_when_generation_exists() {
    let repo = TestRepo::new();
    repo.init_git();
    let tracked = repo.write("service.rs", "const NAME: &str = \"BASE_NAME\";\n");
    repo.commit_all("initial");

    let build = repo.trigram_build_config();
    build_index(&build).expect("test should succeed");

    rewrite_with_fresh_mtime(&tracked, "const NAME: &str = \"DIRTY_ONE\";\n");
    build_index(&build).expect("test should succeed");

    let generation = std::fs::read_to_string(IndexLayout::current_path(&repo.index))
        .expect("test should succeed");
    let generation = generation.trim().to_string();
    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let docs_mtime_before = std::fs::metadata(&layout.docs_path)
        .expect("test should succeed")
        .modified()
        .expect("test should succeed");

    rewrite_with_fresh_mtime(&tracked, "const NAME: &str = \"DIRTY_TWO\";\n");
    rebuild_index(&build).expect("test should succeed");

    let current_generation = std::fs::read_to_string(IndexLayout::current_path(&repo.index))
        .expect("test should succeed");
    let current_generation = current_generation.trim().to_string();
    assert_ne!(current_generation, generation);
    assert!(current_generation.starts_with(&generation));

    let rebuilt_layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let docs_mtime_after = std::fs::metadata(&rebuilt_layout.docs_path)
        .expect("test should succeed")
        .modified()
        .expect("test should succeed");
    assert!(docs_mtime_after > docs_mtime_before);

    let searcher = IndexSearcher::open(repo.index.clone()).expect("test should succeed");
    let stale_reason = searcher
        .stale_reason()
        .expect("test should succeed")
        .expect("dirty tracked file should keep snapshot stale");
    assert!(stale_reason.contains("indexed file changed"));
}

#[test]
fn build_rebuilds_instead_of_reusing_corrupted_current_generation() {
    let repo = TestRepo::new();
    repo.init_git();
    let tracked = repo.write("service.rs", "const NAME: &str = \"BASE_NAME\";\n");
    repo.commit_all("initial");

    let build = repo.trigram_build_config();
    build_index(&build).expect("test should succeed");

    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let (metadata, _) = read_docs_file(&layout.docs_path).expect("test should succeed");
    write_docs_file(&layout.docs_path, metadata, &[]).expect("test should succeed");

    rewrite_with_fresh_mtime(&tracked, "const NAME: &str = \"DIRTY_NAME\";\n");
    build_index(&build).expect("test should succeed");

    let searcher = IndexSearcher::open(repo.index.clone()).expect("test should succeed");
    assert_eq!(searcher.doc_count(), 1);
    let stale_reason = searcher
        .stale_reason()
        .expect("test should succeed")
        .expect("dirty tracked file should keep snapshot stale");
    assert!(stale_reason.contains("indexed file changed"));
}

#[test]
fn build_ignores_untracked_codgrep_artifacts_when_detecting_dirty_worktree() {
    let repo = TestRepo::new();
    repo.init_git();
    repo.write("root.txt", "const ROOT: &str = \"ROOT\";\n");
    repo.commit_all("initial");

    let nested_source = tempdir().expect("test should succeed");
    run_git_in(nested_source.path(), &["init"]);
    run_git_in(
        nested_source.path(),
        &["config", "user.name", "Nested User"],
    );
    run_git_in(
        nested_source.path(),
        &["config", "user.email", "nested@example.com"],
    );
    fs::write(
        nested_source.path().join("needle.txt"),
        "const NEEDLE: &str = \"MAX_FILE_SIZE\";\n",
    )
    .expect("test should succeed");
    run_git_in(nested_source.path(), &["add", "-A"]);
    run_git_in(nested_source.path(), &["commit", "-m", "nested initial"]);

    let nested_source_path = nested_source.path().to_string_lossy().into_owned();
    let add_submodule = Command::new("git")
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            nested_source_path.as_str(),
            "vendor/nested",
        ])
        .current_dir(&repo.repo)
        .output()
        .expect("git command should run");
    assert!(
        add_submodule.status.success(),
        "git submodule add failed: {}",
        String::from_utf8_lossy(&add_submodule.stderr)
    );
    repo.commit_all("add nested submodule");

    repo.write(".codgrep-index/CURRENT", "stale\n");

    let external_index = tempdir().expect("test should succeed");
    let index_path = external_index.path().join("index");
    let build = BuildConfig {
        repo_path: repo.repo.clone(),
        index_path: index_path.clone(),
        tokenizer: TokenizerMode::SparseNgram,
        corpus_mode: repo.sparse_build_config().corpus_mode,
        include_hidden: false,
        max_file_size: 1024 * 1024,
        min_sparse_len: 3,
        max_sparse_len: 8,
    };
    build_index(&build).expect("test should succeed");

    let results = IndexSearcher::open(index_path)
        .expect("test should succeed")
        .search(&count_query("MAX_FILE_SIZE"))
        .expect("test should succeed");
    assert_eq!(results.candidate_docs, 1);
    assert_eq!(results.matched_lines, 1);
}

#[test]
fn build_changes_generation_when_config_changes() {
    let repo = TestRepo::new();
    repo.init_git();
    repo.write("service.rs", "const NAME: &str = \"PM_RESUME\";\n");
    repo.commit_all("initial");

    repo.build_with(repo.trigram_build_config());
    let first_generation = std::fs::read_to_string(IndexLayout::current_path(&repo.index))
        .expect("test should succeed");
    let first_generation = first_generation.trim().to_string();

    repo.build_with(BuildConfig {
        include_hidden: true,
        ..repo.trigram_build_config()
    });
    let second_generation = std::fs::read_to_string(IndexLayout::current_path(&repo.index))
        .expect("test should succeed");
    let second_generation = second_generation.trim().to_string();

    assert_ne!(first_generation, second_generation);
}

#[test]
fn build_materializes_head_snapshot_when_dirty_git_repo_has_no_cached_base() {
    let repo = TestRepo::new();
    let tracked = repo.write("tracked.rs", "const NAME: &str = \"BASE_NAME\";\n");
    repo.init_git();
    repo.commit_all("initial");

    rewrite_with_fresh_mtime(&tracked, "const NAME: &str = \"DIRTY_NAME\";\n");
    repo.write("new.rs", "const NAME: &str = \"NEW_NAME\";\n");

    repo.build();

    let searcher = repo.searcher();
    let base_only = searcher
        .search(&count_query("BASE_NAME"))
        .expect("test should succeed");
    assert_eq!(base_only.candidate_docs, 1);
    assert_eq!(base_only.matched_lines, 0);

    let dirty_only = searcher
        .search(&count_query("DIRTY_NAME"))
        .expect("test should succeed");
    assert_eq!(dirty_only.candidate_docs, 0);
    assert_eq!(dirty_only.matched_lines, 0);

    let new_only = searcher
        .search(&count_query("NEW_NAME"))
        .expect("test should succeed");
    assert_eq!(new_only.candidate_docs, 0);
    assert_eq!(new_only.matched_lines, 0);

    let diff = searcher
        .diff_against_worktree()
        .expect("test should succeed");
    assert_eq!(
        diff.modified_files,
        vec![tracked.to_string_lossy().into_owned()]
    );
    assert_eq!(diff.deleted_files, Vec::<String>::new());
    assert_eq!(
        diff.new_files,
        vec![repo.path("new.rs").to_string_lossy().into_owned()]
    );
}

#[test]
fn case_insensitive_search_uses_folded_tokens() {
    let repo = TestRepo::new();
    let source = repo.write("service.rs", "const NAME: &str = \"Pm_Resume\";\n");

    repo.build();

    let results = repo
        .searcher()
        .search(&QueryConfig {
            case_insensitive: true,
            ..query("pm_resume")
        })
        .expect("test should succeed");

    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].path, source.to_string_lossy());
    assert_eq!(results.hits[0].matches[0].matched_text, "Pm_Resume");
}

#[test]
fn sparse_search_uses_indexed_tokenizer_options() {
    let repo = TestRepo::new();
    let source = repo.write("service.rs", "const NAME: &str = \"abcdef\";\n");

    repo.build_with(BuildConfig {
        min_sparse_len: 4,
        max_sparse_len: 4,
        ..repo.sparse_build_config()
    });

    let results = repo
        .searcher()
        .search(&query("abcdef"))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 1);
    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].path, source.to_string_lossy());
    assert_eq!(results.hits[0].matches[0].matched_text, "abcdef");
}

#[test]
fn impossible_literal_returns_no_candidates() {
    let repo = TestRepo::new();
    repo.write("service.rs", "const NAME: &str = \"abcdef\";\n");

    repo.build();

    let results = repo
        .searcher()
        .search(&query("zzzz"))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 0);
    assert!(results.hits.is_empty());
}

#[test]
fn sparse_literal_query_avoids_full_scan_when_fallback_tokens_exist() {
    let repo = TestRepo::new();
    repo.write("match.rs", "const NAME: &str = \"PM_RESUME\";\n");
    repo.write("other1.rs", "const NAME: &str = \"PM_RESET\";\n");
    repo.write("other2.rs", "const NAME: &str = \"PM_RESTORE\";\n");
    repo.write("other3.rs", "const NAME: &str = \"HELLO_WORLD\";\n");

    repo.build_sparse();

    let results = repo
        .searcher()
        .search(&query("PM_RESUME"))
        .expect("test should succeed");

    assert_eq!(results.hits.len(), 1);
    assert!(results.candidate_docs < 4);
}

#[test]
fn sparse_literal_query_matches_substrings_inside_longer_identifiers() {
    let repo = TestRepo::new();
    repo.write("match.rs", "const NAME: &str = \"AC_ERR_SYSTEM\";\n");
    for idx in 0..20 {
        repo.write(
            format!("noise_{idx}.rs"),
            &format!("const NAME: &str = \"AC_ERR_STATUS_{idx:02}\";\n"),
        );
    }

    repo.build_sparse();

    let results = repo
        .searcher()
        .search(&query("ERR_SYS"))
        .expect("test should succeed");

    assert_eq!(results.searches_with_match, 1);
    assert_eq!(results.hits.len(), 1);
    assert!(results.hits[0].path.ends_with("match.rs"));
}

#[test]
fn sparse_build_writes_trigram_fallback_sidecar() {
    let repo = TestRepo::new();
    repo.write("match.rs", "const NAME: &str = \"AC_ERR_SYSTEM\";\n");

    repo.build_sparse();

    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    let (metadata, _) = read_docs_file(&layout.docs_path).expect("test should succeed");
    let fallback = metadata
        .fallback_trigram
        .as_ref()
        .expect("sparse index should record trigram fallback metadata");
    assert_eq!(fallback.doc_count, 1);
    assert!(fallback.key_count > 0);
    assert!(layout.trigram_fallback_lookup_path.exists());
    assert!(layout.trigram_fallback_postings_path.exists());
    assert!(layout.trigram_fallback_doc_terms_path.exists());
}

#[test]
fn sparse_search_finds_identifier_substrings_without_trigram_sidecar() {
    let repo = TestRepo::new();
    repo.write("match.rs", "const NAME: &str = \"AC_ERR_SYSTEM\";\n");

    repo.build_sparse();
    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    std::fs::remove_file(&layout.trigram_fallback_lookup_path).expect("test should succeed");
    std::fs::remove_file(&layout.trigram_fallback_postings_path).expect("test should succeed");

    let results = repo
        .searcher()
        .search(&query("ERR_SYS"))
        .expect("test should succeed");

    assert_eq!(results.searches_with_match, 1);
    assert_eq!(results.hits.len(), 1);
}

#[test]
fn sparse_search_finds_punctuated_literals_without_trigram_sidecar() {
    let repo = TestRepo::new();
    repo.write(
        "match.rs",
        "const NAME: &str = \"base::ScopedAllowBlocking\";\n",
    );
    for idx in 0..32 {
        repo.write(
            format!("noise_{idx}.rs"),
            &format!("const NAME: &str = \"scopedallowblocking_noise_{idx}\";\n"),
        );
    }

    repo.build_sparse();
    let layout = IndexLayout::resolve(&repo.index).expect("test should succeed");
    std::fs::remove_file(&layout.trigram_fallback_lookup_path).expect("test should succeed");
    std::fs::remove_file(&layout.trigram_fallback_postings_path).expect("test should succeed");

    let results = repo
        .searcher()
        .search(&count_query("base::ScopedAllowBlocking"))
        .expect("test should succeed");

    assert_eq!(results.searches_with_match, 1);
    assert_eq!(results.matched_lines, 1);
    assert!(results.candidate_docs <= 8);
}

#[test]
fn pure_literal_alternation_uses_exact_multi_literal_scan() {
    let repo = TestRepo::new();
    let other = repo.write("other.rs", "const NAME: &str = \"UNRELATED\";\n");
    repo.write("err.rs", "const NAME: &str = \"ERR_SYS\";\n");
    repo.write("turn_off.rs", "const NAME: &str = \"PME_TURN_OFF\";\n");
    repo.write("link.rs", "const NAME: &str = \"LINK_REQ_RST\";\n");
    repo.write("cfg.rs", "const NAME: &str = \"CFG_BME_EVT\";\n");

    repo.build_sparse();

    let results = repo
        .searcher()
        .search(&query("ERR_SYS|PME_TURN_OFF|LINK_REQ_RST|CFG_BME_EVT"))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 4);
    assert_eq!(results.searches_with_match, 4);
    assert_eq!(results.hits.len(), 4);
    assert!(results
        .hits
        .iter()
        .all(|hit| hit.path != other.to_string_lossy()));
}

#[test]
fn pure_literal_alternation_count_only_uses_exact_multi_literal_scan() {
    let repo = TestRepo::new();
    repo.write("other.rs", "const NAME: &str = \"UNRELATED\";\n");
    repo.write("err.rs", "const NAME: &str = \"ERR_SYS\";\n");
    repo.write("turn_off.rs", "const NAME: &str = \"PME_TURN_OFF\";\n");
    repo.write("link.rs", "const NAME: &str = \"LINK_REQ_RST\";\n");
    repo.write("cfg.rs", "const NAME: &str = \"CFG_BME_EVT\";\n");

    repo.build_sparse();

    let results = repo
        .searcher()
        .search(&count_query(
            "ERR_SYS|PME_TURN_OFF|LINK_REQ_RST|CFG_BME_EVT",
        ))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 4);
    assert_eq!(results.searches_with_match, 4);
    assert_eq!(results.matched_lines, 4);
}

#[test]
fn sparse_pure_literal_alternation_matches_substrings_inside_longer_identifiers() {
    let repo = TestRepo::new();
    repo.write("err.rs", "const NAME: &str = \"AC_ERR_SYSTEM\";\n");
    repo.write(
        "turn_off.rs",
        "const NAME: &str = \"PME_TURN_OFF_TIMEOUT\";\n",
    );
    repo.write("link.rs", "const NAME: &str = \"LINK_REQ_RST_ERR\";\n");
    repo.write("cfg.rs", "const NAME: &str = \"CFG_BME_EVT_STATUS\";\n");
    for idx in 0..20 {
        repo.write(
            format!("noise_err_{idx}.rs"),
            &format!("const NAME: &str = \"AC_ERR_STATUS_{idx:02}\";\n"),
        );
        repo.write(
            format!("noise_off_{idx}.rs"),
            &format!("const NAME: &str = \"PME_TURN_ON_{idx:02}\";\n"),
        );
        repo.write(
            format!("noise_link_{idx}.rs"),
            &format!("const NAME: &str = \"LINK_REQ_ACK_{idx:02}\";\n"),
        );
        repo.write(
            format!("noise_cfg_{idx}.rs"),
            &format!("const NAME: &str = \"CFG_BME_ACK_{idx:02}\";\n"),
        );
    }

    repo.build_sparse();

    let results = repo
        .searcher()
        .search(&query("ERR_SYS|PME_TURN_OFF|LINK_REQ_RST|CFG_BME_EVT"))
        .expect("test should succeed");

    assert_eq!(results.searches_with_match, 4);
    assert_eq!(results.hits.len(), 4);
}

#[test]
fn pure_literal_alternation_respects_leftmost_first_order() {
    let repo = TestRepo::new();
    repo.write("service.rs", "const NAME: &str = \"ab\";\n");

    repo.build();

    let results = repo
        .searcher()
        .search(&query("a|ab"))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 1);
    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].matches.len(), 1);
    assert_eq!(results.hits[0].matches[0].matched_text, "a");
}

#[test]
fn trigram_pure_literal_alternation_respects_case_sensitive_index_prefilter() {
    let repo = TestRepo::new();
    repo.write(
        "service.rs",
        "const A: &str = \"ERR_SYS\";\nconst B: &str = \"CFG_BME_EVT\";\n",
    );

    repo.build();

    let results = repo
        .searcher()
        .search(&count_query(
            "ERR_SYS|PME_TURN_OFF|LINK_REQ_RST|CFG_BME_EVT",
        ))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 1);
    assert_eq!(results.matched_lines, 2);
}

#[test]
fn multiline_verifier_handles_cross_line_matches() {
    let repo = TestRepo::new();
    repo.write("story.txt", "Sherlock\nHolmes\n");

    repo.build();

    let results = repo
        .searcher()
        .search(&QueryConfig {
            dot_matches_new_line: true,
            ..query("Sherlock.*Holmes")
        })
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 1);
    assert_eq!(results.matched_lines, 2);
    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].matches.len(), 1);
    assert_eq!(results.hits[0].matches[0].location.line, 1);
    assert_eq!(results.hits[0].matches[0].matched_text, "Sherlock\nHolmes");
}

#[test]
fn default_search_does_not_span_lines_for_whitespace_classes() {
    let repo = TestRepo::new();
    repo.write("story.txt", "Sherlock\nHolmes\n");

    repo.build();

    let results = repo
        .searcher()
        .search(&QueryConfig {
            search_mode: SearchMode::CountOnly,
            ..query("Sherlock\\s+Holmes")
        })
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 1);
    assert_eq!(results.matched_lines, 0);
    assert!(results.hits.is_empty());
}

#[test]
fn sparse_short_unindexed_literal_branch_falls_back_to_broad_candidates() {
    let repo = TestRepo::new();
    repo.write("alpha.rs", "const NAME: &str = \"foo\";\n");
    repo.write("beta.rs", "const NAME: &str = \"bar\";\n");
    repo.write("gamma.rs", "const NAME: &str = \"baz\";\n");

    repo.build_sparse();

    let results = repo
        .searcher()
        .search(&count_query("\\wAh"))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 3);
    assert_eq!(results.matched_lines, 0);
    assert!(results.hits.is_empty());
}

#[test]
fn sparse_short_unindexed_literal_count_matches_reports_occurrences() {
    let repo = TestRepo::new();
    repo.write("alpha.rs", "const NAME: &str = \"xAh xAh xAh\";\n");
    repo.write("beta.rs", "const NAME: &str = \"bar\";\n");
    repo.write("gamma.rs", "const NAME: &str = \"baz\";\n");

    repo.build_sparse();

    let results = repo
        .searcher()
        .search(&count_matches_query("\\wAh"))
        .expect("test should succeed");

    assert_eq!(results.candidate_docs, 3);
    assert_eq!(results.matched_lines, 1);
    assert_eq!(results.matched_occurrences, 3);
    assert!(results.hits.is_empty());
}
