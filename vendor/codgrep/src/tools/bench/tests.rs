use std::fs;

use tempfile::tempdir;

use super::{
    bench_build_config, benchmark_scope_roots, copy_tree, parse_rg_count_output,
    prepare_rg_search_scope, run, BenchCacheMode, BenchConfig,
};
use super::{fixtures::prepare_dirty_worktree_fixture, DirtyPattern};
use crate::config::{CorpusMode, TokenizerMode};
use crate::index::build_index;
use crate::tools::bench::report::render_report;

#[test]
fn bench_runner_smoke_test() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path();
    fs::create_dir_all(suite_dir.join("linux")).expect("test should succeed");
    fs::create_dir_all(suite_dir.join("subtitles")).expect("test should succeed");

    fs::write(
        suite_dir.join("linux").join("kernel.c"),
        "PM_RESUME\nERR_SYS\nabc Ah\nΩHolmesΩ\n",
    )
    .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("en.sample.txt"),
        "Sherlock Holmes met John Watson.\n",
    )
    .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("ru.txt"),
        "Шерлок Холмс встретил Джон Уотсон.\n",
    )
    .expect("test should succeed");

    let report = run(&BenchConfig {
        suite_dir: suite_dir.to_path_buf(),
        filter: Some("linux_literal".into()),
        custom_repo: None,
        custom_patterns: Vec::new(),
        custom_name: None,
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: false,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    })
    .expect("test should succeed");

    assert_eq!(report.summaries.len(), 2);
    assert!(report
        .summaries
        .iter()
        .all(|summary| summary.runners[0].sample_count == 1));
}

#[test]
fn parse_rg_count_output_supports_single_and_multi_file_formats() {
    assert_eq!(
        parse_rg_count_output(b"3\n").expect("test should succeed"),
        3
    );
    assert_eq!(
        parse_rg_count_output(b"/tmp/a.rs:2\n/tmp/b.rs:5\n").expect("test should succeed"),
        7
    );
}

#[test]
fn benchmark_scope_roots_uses_repo_relative_paths_for_daemon_queries() {
    let temp = tempdir().expect("test should succeed");
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join("src")).expect("test should succeed");
    fs::write(repo.join("src").join("lib.rs"), "PM_RESUME\n").expect("test should succeed");

    let repo_roots = benchmark_scope_roots(&repo, vec![repo.clone()]);
    assert!(repo_roots.is_empty());

    let nested = benchmark_scope_roots(&repo, vec![repo.join("src").join("lib.rs")]);
    assert_eq!(nested, vec![std::path::PathBuf::from("src/lib.rs")]);
}

#[cfg(target_os = "linux")]
#[test]
fn cold_bench_runs_hook_before_each_iteration() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path();
    fs::create_dir_all(suite_dir.join("linux")).expect("test should succeed");
    fs::create_dir_all(suite_dir.join("subtitles")).expect("test should succeed");

    fs::write(
        suite_dir.join("linux").join("kernel.c"),
        "PM_RESUME\npm_resume\n",
    )
    .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("en.sample.txt"),
        "Sherlock Holmes met John Watson.\n",
    )
    .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("ru.txt"),
        "Шерлок Холмс встретил Джон Уотсон.\n",
    )
    .expect("test should succeed");

    let hook_path = suite_dir.join("cold-hook-count.txt");
    let hook = format!("printf x >> {}", hook_path.display());

    let report = run(&BenchConfig {
        suite_dir: suite_dir.to_path_buf(),
        filter: Some("linux_literal_casei".into()),
        custom_repo: None,
        custom_patterns: Vec::new(),
        custom_name: None,
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Cold,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 1,
        bench_iter: 2,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: false,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: Some(hook),
    })
    .expect("test should succeed");

    assert_eq!(report.summaries.len(), 1);
    let hook_count = fs::read_to_string(hook_path).expect("test should succeed");
    assert_eq!(hook_count, "xxx");
}

#[test]
fn bench_runner_supports_worktree_runners() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path();
    fs::create_dir_all(suite_dir.join("linux")).expect("test should succeed");
    fs::create_dir_all(suite_dir.join("subtitles")).expect("test should succeed");

    fs::write(
        suite_dir.join("linux").join("kernel.c"),
        "PM_RESUME\nERR_SYS\nabc Ah\nΩHolmesΩ\n",
    )
    .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("en.sample.txt"),
        "Sherlock Holmes met John Watson.\n",
    )
    .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("ru.txt"),
        "Шерлок Холмс встретил Джон Уотсон.\n",
    )
    .expect("test should succeed");

    let report = run(&BenchConfig {
        suite_dir: suite_dir.to_path_buf(),
        filter: Some("linux_literal".into()),
        custom_repo: None,
        custom_patterns: Vec::new(),
        custom_name: None,
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: true,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    })
    .expect("test should succeed");

    assert_eq!(report.summaries.len(), 2);
    let summary = report
        .summaries
        .iter()
        .find(|summary| summary.name == "linux_literal")
        .expect("test should succeed");
    assert_eq!(summary.runners.len(), 3);
    assert!(summary
        .runners
        .iter()
        .any(|runner| runner.runner == "codgrep_worktree_build"));
    assert!(summary
        .runners
        .iter()
        .any(|runner| runner.runner == "codgrep_worktree"));
}

#[cfg(unix)]
#[test]
fn copy_tree_dereferences_symlinked_files_for_worktree_fixtures() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("test should succeed");
    let source = temp.path().join("source");
    let target = temp.path().join("target");

    fs::create_dir_all(source.join("nested")).expect("test should succeed");
    fs::write(source.join("nested").join("real.txt"), "PM_RESUME\n").expect("test should succeed");
    symlink("nested/real.txt", source.join("linked.txt")).expect("test should succeed");

    copy_tree(&source, &target).expect("test should succeed");

    let copied = target.join("linked.txt");
    assert!(copied.is_file());
    assert_eq!(
        fs::read_to_string(copied).expect("test should succeed"),
        "PM_RESUME\n"
    );
}

#[test]
fn sampled_dirty_worktree_fixture_limits_files_but_keeps_required_matches() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path().join("suite");
    let repo_dir = temp.path().join("repo");

    fs::create_dir_all(&suite_dir).expect("test should succeed");
    fs::create_dir_all(repo_dir.join("drivers")).expect("test should succeed");
    fs::create_dir_all(repo_dir.join("kernel")).expect("test should succeed");
    fs::write(repo_dir.join("drivers").join("apm.c"), "PM_RESUME\n").expect("test should succeed");
    fs::write(repo_dir.join("kernel").join("ata.c"), "ERR_SYS\n").expect("test should succeed");
    fs::write(repo_dir.join("misc.txt"), "unrelated\n").expect("test should succeed");

    let fixture = prepare_dirty_worktree_fixture(
        &repo_dir,
        &suite_dir,
        "linux-small",
        &BenchConfig {
            suite_dir: suite_dir.clone(),
            filter: None,
            custom_repo: None,
            custom_patterns: Vec::new(),
            custom_name: None,
            custom_case_insensitive: false,
            tokenizer: TokenizerMode::Trigram,
            corpus_mode: CorpusMode::RespectIgnore,
            cache_mode: BenchCacheMode::Warm,
            query_mode: super::BenchQueryMode::Trace,
            warmup_iter: 0,
            bench_iter: 1,
            top_k_tokens: 4,
            compare_rg: false,
            compare_worktree: true,
            worktree_sample_files: Some(2),
            rebuild: true,
            raw_output: None,
            cold_hook: None,
        },
        &[
            DirtyPattern {
                regex_pattern: "PM_RESUME".into(),
                case_insensitive: false,
            },
            DirtyPattern {
                regex_pattern: "ERR_SYS".into(),
                case_insensitive: false,
            },
        ],
    )
    .expect("test should succeed");

    let copied = walkdir::WalkDir::new(&fixture.repo_path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            entry
                .path()
                .strip_prefix(&fixture.repo_path)
                .expect("test should succeed")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();

    assert_eq!(copied.len(), 2);
    assert!(copied.iter().any(|path| path == "drivers/apm.c"));
    assert!(copied.iter().any(|path| path == "kernel/ata.c"));
}

#[test]
fn bench_runner_supports_custom_repo_patterns() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path().join("suite");
    let repo_dir = temp.path().join("repo");

    fs::create_dir_all(&suite_dir).expect("test should succeed");
    fs::create_dir_all(&repo_dir).expect("test should succeed");
    fs::write(repo_dir.join("main.ts"), "const APP = 'BitFun';\n").expect("test should succeed");

    let report = run(&BenchConfig {
        suite_dir,
        filter: None,
        custom_repo: Some(repo_dir),
        custom_patterns: vec!["BitFun".into()],
        custom_name: Some("codgrep-custom".into()),
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: false,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    })
    .expect("test should succeed");

    assert_eq!(report.summaries.len(), 1);
    let summary = &report.summaries[0];
    assert_eq!(summary.name, "codgrep-custom");
    assert_eq!(summary.pattern, "BitFun");
    assert_eq!(summary.target, "codgrep-custom");
    assert!(summary
        .runners
        .iter()
        .any(|runner| runner.runner == "codgrep"));
}

#[test]
fn bench_rg_runner_uses_indexed_text_scope_for_custom_repo() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path().join("suite");
    let repo_dir = temp.path().join("repo");

    fs::create_dir_all(&suite_dir).expect("test should succeed");
    fs::create_dir_all(&repo_dir).expect("test should succeed");
    fs::write(repo_dir.join("main.ts"), "const APP = 'BitFun';\n").expect("test should succeed");
    fs::write(repo_dir.join("latin1.txt"), b"BitFun\xff\n").expect("test should succeed");

    let report = run(&BenchConfig {
        suite_dir,
        filter: None,
        custom_repo: Some(repo_dir),
        custom_patterns: vec!["BitFun".into()],
        custom_name: Some("codgrep-custom-rg-scope".into()),
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: true,
        compare_worktree: false,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    })
    .expect("test should succeed");

    let summary = &report.summaries[0];
    let codgrep = summary
        .runners
        .iter()
        .find(|runner| runner.runner == "codgrep")
        .expect("test should succeed");
    let rg = summary
        .runners
        .iter()
        .find(|runner| runner.runner == "rg")
        .expect("test should succeed");
    assert_eq!(codgrep.match_count, 1);
    assert_eq!(rg.match_count, 1);
}

#[test]
fn prepare_rg_search_scope_excludes_non_indexed_files() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path().join("suite");
    let repo_dir = temp.path().join("repo");
    let index_path = temp.path().join("index");

    fs::create_dir_all(&suite_dir).expect("test should succeed");
    fs::create_dir_all(&repo_dir).expect("test should succeed");
    fs::write(repo_dir.join("main.ts"), "const APP = 'BitFun';\n").expect("test should succeed");
    fs::write(repo_dir.join("latin1.txt"), b"BitFun\xff\n").expect("test should succeed");

    let config = BenchConfig {
        suite_dir,
        filter: None,
        custom_repo: Some(repo_dir.clone()),
        custom_patterns: vec!["BitFun".into()],
        custom_name: Some("codgrep-custom-rg-scope".into()),
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: true,
        compare_worktree: false,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    };
    build_index(&bench_build_config(&repo_dir, &index_path, &config)).expect("test should succeed");

    let scope =
        prepare_rg_search_scope(&repo_dir, &index_path, &config).expect("test should succeed");
    let ignore_file = scope.ignore_file.expect("test should succeed");
    let ignore_contents = fs::read_to_string(ignore_file).expect("test should succeed");
    assert!(ignore_contents.contains("/latin1.txt\n"));
}

#[test]
fn bench_runner_supports_custom_worktree_build_runner() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path().join("suite");
    let repo_dir = temp.path().join("repo");

    fs::create_dir_all(&suite_dir).expect("test should succeed");
    fs::create_dir_all(&repo_dir).expect("test should succeed");
    fs::write(repo_dir.join("main.ts"), "const APP = 'BitFun';\n").expect("test should succeed");

    let report = run(&BenchConfig {
        suite_dir,
        filter: None,
        custom_repo: Some(repo_dir),
        custom_patterns: vec!["BitFun".into()],
        custom_name: Some("codgrep-worktree".into()),
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: true,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    })
    .expect("test should succeed");

    assert_eq!(report.summaries.len(), 1);
    let summary = &report.summaries[0];
    assert!(summary
        .runners
        .iter()
        .any(|runner| runner.runner == "codgrep_worktree_build"));
    assert!(summary
        .runners
        .iter()
        .any(|runner| runner.runner == "codgrep_worktree"));
}

#[test]
fn bench_report_labels_worktree_runner_modes() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path();
    fs::create_dir_all(suite_dir.join("linux")).expect("test should succeed");
    fs::create_dir_all(suite_dir.join("subtitles")).expect("test should succeed");

    fs::write(suite_dir.join("linux").join("kernel.c"), "PM_RESUME\n")
        .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("en.sample.txt"),
        "Sherlock Holmes\n",
    )
    .expect("test should succeed");
    fs::write(suite_dir.join("subtitles").join("ru.txt"), "Шерлок Холмс\n")
        .expect("test should succeed");

    let report = run(&BenchConfig {
        suite_dir: suite_dir.to_path_buf(),
        filter: Some("linux_literal".into()),
        custom_repo: None,
        custom_patterns: Vec::new(),
        custom_name: None,
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: true,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    })
    .expect("test should succeed");

    let rendered = render_report(&report);
    let base = rendered
        .find("codgrep [daemon-steady-state]")
        .expect("test should succeed");
    let worktree_build = rendered
        .find("codgrep_worktree_build [dirty-first-query]")
        .expect("test should succeed");
    let worktree_cached = rendered
        .find("codgrep_worktree [dirty-cached-query]")
        .expect("test should succeed");

    assert!(base < worktree_build);
    assert!(worktree_build < worktree_cached);
}

#[test]
fn bench_raw_output_includes_runner_family_and_mode() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path();
    fs::create_dir_all(suite_dir.join("linux")).expect("test should succeed");
    fs::create_dir_all(suite_dir.join("subtitles")).expect("test should succeed");

    fs::write(suite_dir.join("linux").join("kernel.c"), "PM_RESUME\n")
        .expect("test should succeed");
    fs::write(
        suite_dir.join("subtitles").join("en.sample.txt"),
        "Sherlock Holmes\n",
    )
    .expect("test should succeed");
    fs::write(suite_dir.join("subtitles").join("ru.txt"), "Шерлок Холмс\n")
        .expect("test should succeed");

    let raw_output = suite_dir.join("bench.csv");
    let _ = run(&BenchConfig {
        suite_dir: suite_dir.to_path_buf(),
        filter: Some("linux_literal".into()),
        custom_repo: None,
        custom_patterns: Vec::new(),
        custom_name: None,
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 1,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: true,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: Some(raw_output.clone()),
        cold_hook: None,
    })
    .expect("test should succeed");

    let csv = fs::read_to_string(raw_output).expect("test should succeed");
    let lines = csv.lines().collect::<Vec<_>>();
    assert_eq!(
        lines[0],
        "benchmark,corpus,runner,runner_family,runner_mode,iteration,duration_secs,candidate_docs,match_count"
    );
    assert!(lines
        .iter()
        .any(|line| line.contains(",codgrep,codgrep,daemon_steady_state,")));
    assert!(lines
        .iter()
        .any(|line| line.contains(",codgrep_worktree_build,codgrep,dirty_first_query,")));
    assert!(lines
        .iter()
        .any(|line| line.contains(",codgrep_worktree,codgrep,dirty_cached_query,")));
}

#[test]
fn bench_runner_uses_trace_mode_for_multiple_custom_patterns() {
    let temp = tempdir().expect("test should succeed");
    let suite_dir = temp.path().join("suite");
    let repo_dir = temp.path().join("repo");

    fs::create_dir_all(&suite_dir).expect("test should succeed");
    fs::create_dir_all(&repo_dir).expect("test should succeed");
    fs::write(
        repo_dir.join("main.ts"),
        "const APP = 'BitFun';\nconst TOOL = 'Search';\n",
    )
    .expect("test should succeed");

    let report = run(&BenchConfig {
        suite_dir,
        filter: None,
        custom_repo: Some(repo_dir),
        custom_patterns: vec!["BitFun".into(), "Search".into()],
        custom_name: Some("codgrep-trace".into()),
        custom_case_insensitive: false,
        tokenizer: TokenizerMode::Trigram,
        corpus_mode: CorpusMode::RespectIgnore,
        cache_mode: BenchCacheMode::Warm,
        query_mode: super::BenchQueryMode::Trace,
        warmup_iter: 0,
        bench_iter: 2,
        top_k_tokens: 4,
        compare_rg: false,
        compare_worktree: false,
        worktree_sample_files: None,
        rebuild: true,
        raw_output: None,
        cold_hook: None,
    })
    .expect("test should succeed");

    assert_eq!(report.summaries.len(), 1);
    let summary = &report.summaries[0];
    assert_eq!(summary.name, "codgrep-trace");
    assert_eq!(summary.pattern, "trace[2]");
    assert_eq!(summary.target, "codgrep-trace");
    assert!(summary
        .runners
        .iter()
        .all(|runner| runner.sample_count == 2));
}
