use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

use crate::{
    error::{AppError, Result},
    index::build_index,
};

use super::{
    bench_build_config, bench_index_is_usable,
    cache::{collect_cache_paths_for_target, prepare_sample, sanitize_benchmark_name},
    configure_rg_command,
    fixtures::prepare_dirty_worktree_fixture,
    prepare_rg_search_scope,
    report::{build_summary, mean_usize, RawSample, RawWriter},
    run_daemon_search_count, start_bench_daemon, BenchCachePaths, BenchConfig, BenchQueryMode,
    BenchReport, CorpusBuildRecord, CustomBenchmarkCase, CustomBenchmarkTrace, DirtyPattern,
    DirtyWorktreeFixture, RgSearchScope,
};

pub(super) fn run_custom(
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<BenchReport> {
    let repo_path = config
        .custom_repo
        .as_ref()
        .ok_or_else(|| AppError::InvalidPattern("custom benchmark repo path is missing".into()))?;
    if !repo_path.exists() {
        return Err(AppError::InvalidIndex(format!(
            "missing custom benchmark repo at {}",
            repo_path.display()
        )));
    }

    let target_label = config
        .custom_name
        .clone()
        .or_else(|| {
            repo_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "custom".into());
    let index_path = custom_index_path(repo_path, &target_label, config);
    let build_config = bench_build_config(repo_path, &index_path, config);
    if config.rebuild && index_path.exists() {
        fs::remove_dir_all(&index_path)?;
    }

    let mut corpus_builds = Vec::new();
    if config.rebuild || !bench_index_is_usable(&index_path, &build_config) {
        let started = Instant::now();
        let docs_indexed = build_index(&build_config)?;
        corpus_builds.push(CorpusBuildRecord {
            corpus: target_label.clone(),
            tokenizer: config.tokenizer,
            docs_indexed,
            duration_secs: started.elapsed().as_secs_f64(),
        });
    }

    let cache_paths = if config.cache_mode.is_cold() {
        Some(collect_cache_paths_for_target(
            repo_path,
            &index_path,
            config,
            &target_label,
        )?)
    } else {
        None
    };
    let rg_scope = prepare_rg_search_scope(repo_path, &index_path, config)?;
    let dirty_fixture = if config.compare_worktree {
        let patterns = config
            .custom_patterns
            .iter()
            .map(|pattern| DirtyPattern {
                regex_pattern: pattern.clone(),
                case_insensitive: config.custom_case_insensitive,
            })
            .collect::<Vec<_>>();
        Some(prepare_dirty_worktree_fixture(
            repo_path,
            &config.suite_dir,
            &target_label,
            config,
            &patterns,
        )?)
    } else {
        None
    };
    let mut summaries = Vec::new();
    if matches!(config.query_mode, BenchQueryMode::Trace) && config.custom_patterns.len() > 1 {
        let benchmark = CustomBenchmarkTrace {
            name: target_label.clone(),
            target: target_label.clone(),
            repo_path: repo_path.clone(),
            patterns: config.custom_patterns.clone(),
            case_insensitive: config.custom_case_insensitive,
        };

        let mut runners = vec![run_codgrep_custom_trace_benchmark(
            &benchmark,
            &index_path,
            cache_paths.as_ref(),
            config,
            raw_writer.as_deref_mut(),
        )?];

        if config.compare_rg {
            runners.push(run_rg_custom_trace_benchmark(
                &benchmark,
                &rg_scope,
                cache_paths.as_ref(),
                config,
                raw_writer.as_deref_mut(),
            )?);
        }

        if let Some(fixture) = dirty_fixture.as_ref() {
            runners.push(run_worktree_custom_trace_benchmark(
                &benchmark,
                fixture,
                true,
                config,
                raw_writer.as_deref_mut(),
            )?);
            runners.push(run_worktree_custom_trace_benchmark(
                &benchmark,
                fixture,
                false,
                config,
                raw_writer.as_deref_mut(),
            )?);
        }

        summaries.push(super::BenchSummary {
            name: benchmark.name,
            pattern: format!("trace[{}]", benchmark.patterns.len()),
            target: benchmark.target,
            runners,
        });
    } else {
        for (index, pattern) in config.custom_patterns.iter().enumerate() {
            let benchmark = CustomBenchmarkCase {
                name: if config.custom_patterns.len() == 1 {
                    target_label.clone()
                } else {
                    format!("{}_{}", target_label, index + 1)
                },
                target: target_label.clone(),
                repo_path: repo_path.clone(),
                pattern: pattern.clone(),
                case_insensitive: config.custom_case_insensitive,
            };

            let mut runners = vec![run_codgrep_custom_benchmark(
                &benchmark,
                &index_path,
                cache_paths.as_ref(),
                config,
                raw_writer.as_deref_mut(),
            )?];

            if config.compare_rg {
                runners.push(run_rg_custom_benchmark(
                    &benchmark,
                    &rg_scope,
                    cache_paths.as_ref(),
                    config,
                    raw_writer.as_deref_mut(),
                )?);
            }

            if let Some(fixture) = dirty_fixture.as_ref() {
                runners.push(run_worktree_custom_benchmark(
                    &benchmark,
                    fixture,
                    true,
                    config,
                    raw_writer.as_deref_mut(),
                )?);
                runners.push(run_worktree_custom_benchmark(
                    &benchmark,
                    fixture,
                    false,
                    config,
                    raw_writer.as_deref_mut(),
                )?);
            }

            summaries.push(super::BenchSummary {
                name: benchmark.name,
                pattern: benchmark.pattern,
                target: benchmark.target,
                runners,
            });
        }
    }

    Ok(BenchReport {
        corpus_builds,
        summaries,
    })
}
fn custom_index_path(repo_path: &Path, target_label: &str, config: &BenchConfig) -> PathBuf {
    let default_index = repo_path.join(".codgrep-index");
    if default_index.exists() {
        return default_index;
    }
    let suffix = format!(
        ".codgrep-index-{}-{}-{}",
        sanitize_benchmark_name(target_label),
        config.corpus_mode.as_str(),
        config.tokenizer.as_str()
    );
    repo_path.join(suffix)
}

fn run_codgrep_custom_benchmark(
    benchmark: &CustomBenchmarkCase,
    index_path: &Path,
    cache_paths: Option<&BenchCachePaths>,
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    let daemon = start_bench_daemon(&benchmark.repo_path, index_path, config)?;
    let mut durations = Vec::with_capacity(config.bench_iter);
    let mut candidate_docs = None;
    let mut match_count = 0;

    for _ in 0..config.warmup_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let _ = run_codgrep_custom_daemon_once(&daemon, benchmark, config)?;
    }

    for iter in 0..config.bench_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let started = Instant::now();
        let outcome = run_codgrep_custom_daemon_once(&daemon, benchmark, config)?;
        let duration_secs = started.elapsed().as_secs_f64();
        candidate_docs = Some(outcome.candidate_docs);
        match_count = outcome.match_count;
        durations.push(duration_secs);

        if let Some(writer) = raw_writer.as_deref_mut() {
            writer.write_sample(RawSample {
                benchmark: &benchmark.name,
                corpus: &benchmark.target,
                runner: "codgrep",
                iteration: iter,
                duration_secs,
                candidate_docs,
                match_count,
            })?;
        }
    }

    Ok(build_summary(
        "codgrep".into(),
        durations,
        candidate_docs,
        match_count,
    ))
}

fn run_codgrep_custom_trace_benchmark(
    benchmark: &CustomBenchmarkTrace,
    index_path: &Path,
    cache_paths: Option<&BenchCachePaths>,
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    let daemon = start_bench_daemon(&benchmark.repo_path, index_path, config)?;
    let mut durations = Vec::with_capacity(config.bench_iter);
    let mut total_candidate_docs = 0usize;
    let mut total_match_count = 0usize;

    for iter in 0..config.warmup_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let pattern = benchmark.pattern_at(iter);
        let _ = run_daemon_search_count(
            &daemon,
            &benchmark.repo_path,
            pattern,
            benchmark.case_insensitive,
            config,
            vec![benchmark.repo_path.clone()],
        )?;
    }

    for iter in 0..config.bench_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let pattern = benchmark.pattern_at(iter);
        let started = Instant::now();
        let outcome = run_daemon_search_count(
            &daemon,
            &benchmark.repo_path,
            pattern,
            benchmark.case_insensitive,
            config,
            vec![benchmark.repo_path.clone()],
        )?;
        let duration_secs = started.elapsed().as_secs_f64();
        total_candidate_docs += outcome.candidate_docs;
        total_match_count += outcome.match_count;
        durations.push(duration_secs);
        if let Some(writer) = raw_writer.as_deref_mut() {
            writer.write_sample(RawSample {
                benchmark: &benchmark.name,
                corpus: &benchmark.target,
                runner: "codgrep",
                iteration: iter,
                duration_secs,
                candidate_docs: Some(outcome.candidate_docs),
                match_count: outcome.match_count,
            })?;
        }
    }

    Ok(build_summary(
        "codgrep".into(),
        durations,
        Some(mean_usize(total_candidate_docs, config.bench_iter)),
        mean_usize(total_match_count, config.bench_iter),
    ))
}

fn run_codgrep_custom_daemon_once(
    daemon: &super::BenchDaemon,
    benchmark: &CustomBenchmarkCase,
    config: &BenchConfig,
) -> Result<super::BitfunOutcome> {
    run_daemon_search_count(
        daemon,
        &benchmark.repo_path,
        &benchmark.pattern,
        benchmark.case_insensitive,
        config,
        vec![benchmark.repo_path.clone()],
    )
}

fn run_rg_custom_benchmark(
    benchmark: &CustomBenchmarkCase,
    scope: &RgSearchScope,
    cache_paths: Option<&BenchCachePaths>,
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    let mut durations = Vec::with_capacity(config.bench_iter);
    let mut match_count = 0;
    for _ in 0..config.warmup_iter {
        prepare_sample(super::CacheRunner::Rg, cache_paths, config)?;
        let _ = run_rg_custom_once(
            scope,
            &benchmark.pattern,
            benchmark.case_insensitive,
            config,
        )?;
    }
    for iter in 0..config.bench_iter {
        prepare_sample(super::CacheRunner::Rg, cache_paths, config)?;
        let started = Instant::now();
        match_count = run_rg_custom_once(
            scope,
            &benchmark.pattern,
            benchmark.case_insensitive,
            config,
        )?;
        let duration_secs = started.elapsed().as_secs_f64();
        durations.push(duration_secs);
        if let Some(writer) = raw_writer.as_deref_mut() {
            writer.write_sample(RawSample {
                benchmark: &benchmark.name,
                corpus: &benchmark.target,
                runner: "rg",
                iteration: iter,
                duration_secs,
                candidate_docs: None,
                match_count,
            })?;
        }
    }
    Ok(build_summary("rg".into(), durations, None, match_count))
}

fn run_rg_custom_trace_benchmark(
    benchmark: &CustomBenchmarkTrace,
    scope: &RgSearchScope,
    cache_paths: Option<&BenchCachePaths>,
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    let mut durations = Vec::with_capacity(config.bench_iter);
    let mut total_match_count = 0usize;
    for iter in 0..config.warmup_iter {
        prepare_sample(super::CacheRunner::Rg, cache_paths, config)?;
        let pattern = benchmark.pattern_at(iter);
        let _ = run_rg_custom_once(scope, pattern, benchmark.case_insensitive, config)?;
    }
    for iter in 0..config.bench_iter {
        prepare_sample(super::CacheRunner::Rg, cache_paths, config)?;
        let pattern = benchmark.pattern_at(iter);
        let started = Instant::now();
        let match_count = run_rg_custom_once(scope, pattern, benchmark.case_insensitive, config)?;
        let duration_secs = started.elapsed().as_secs_f64();
        total_match_count += match_count;
        durations.push(duration_secs);
        if let Some(writer) = raw_writer.as_deref_mut() {
            writer.write_sample(RawSample {
                benchmark: &benchmark.name,
                corpus: &benchmark.target,
                runner: "rg",
                iteration: iter,
                duration_secs,
                candidate_docs: None,
                match_count,
            })?;
        }
    }
    Ok(build_summary(
        "rg".into(),
        durations,
        None,
        mean_usize(total_match_count, config.bench_iter),
    ))
}

fn run_rg_custom_once(
    scope: &RgSearchScope,
    pattern: &str,
    case_insensitive: bool,
    config: &BenchConfig,
) -> Result<usize> {
    let mut command = Command::new("rg");
    configure_rg_command(&mut command, config);
    if case_insensitive {
        command.arg("-i");
    }
    if let Some(current_dir) = scope.current_dir.as_ref() {
        command.current_dir(current_dir);
    }
    if let Some(ignore_file) = scope.ignore_file.as_ref() {
        command.arg("--ignore-file");
        command.arg(ignore_file);
    }
    command.arg(pattern);
    command.arg(&scope.target);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::null());

    let output = command.output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(AppError::InvalidIndex(format!(
            "rg failed for trace benchmark {} with status {}",
            pattern, output.status
        )));
    }
    super::parse_rg_count_output(&output.stdout)
}

fn run_worktree_custom_benchmark(
    benchmark: &CustomBenchmarkCase,
    fixture: &DirtyWorktreeFixture,
    build_overlay_each_sample: bool,
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    run_workspace_benchmark(
        &fixture.repo_path,
        &fixture.index_path,
        &benchmark.name,
        &benchmark.target,
        if build_overlay_each_sample {
            "codgrep_worktree_build"
        } else {
            "codgrep_worktree"
        },
        Some(&benchmark.pattern),
        benchmark.case_insensitive,
        cache_paths_for_fixture(fixture, build_overlay_each_sample),
        config,
        raw_writer.as_deref_mut(),
    )
}

fn run_worktree_custom_trace_benchmark(
    benchmark: &CustomBenchmarkTrace,
    fixture: &DirtyWorktreeFixture,
    build_overlay_each_sample: bool,
    config: &BenchConfig,
    raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    run_workspace_trace_benchmark(
        &fixture.repo_path,
        &fixture.index_path,
        benchmark,
        if build_overlay_each_sample {
            "codgrep_worktree_build"
        } else {
            "codgrep_worktree"
        },
        cache_paths_for_fixture(fixture, build_overlay_each_sample),
        config,
        raw_writer,
    )
}

fn run_workspace_benchmark(
    repo_path: &Path,
    index_path: &Path,
    benchmark_name: &str,
    corpus: &str,
    runner: &str,
    pattern: Option<&str>,
    case_insensitive: bool,
    cache_paths: Option<&BenchCachePaths>,
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    let pattern = pattern.expect("single-pattern worktree benchmark requires a pattern");
    let mut durations = Vec::with_capacity(config.bench_iter);
    let mut candidate_docs = None;
    let mut match_count = 0;
    let build_overlay_each_sample = runner == "codgrep_worktree_build";
    let daemon = (!build_overlay_each_sample)
        .then(|| start_bench_daemon(repo_path, index_path, config))
        .transpose()?;
    for _ in 0..config.warmup_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let _ = run_workspace_query(
            daemon.as_ref(),
            repo_path,
            index_path,
            pattern,
            case_insensitive,
            build_overlay_each_sample,
            config,
        )?;
    }
    for iter in 0..config.bench_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let started = Instant::now();
        let outcome = run_workspace_query(
            daemon.as_ref(),
            repo_path,
            index_path,
            pattern,
            case_insensitive,
            build_overlay_each_sample,
            config,
        )?;
        let duration_secs = started.elapsed().as_secs_f64();
        candidate_docs = Some(outcome.candidate_docs);
        match_count = outcome.match_count;
        durations.push(duration_secs);
        if let Some(writer) = raw_writer.as_deref_mut() {
            writer.write_sample(RawSample {
                benchmark: benchmark_name,
                corpus,
                runner,
                iteration: iter,
                duration_secs,
                candidate_docs,
                match_count,
            })?;
        }
    }
    Ok(build_summary(
        runner.into(),
        durations,
        candidate_docs,
        match_count,
    ))
}

fn run_workspace_trace_benchmark(
    repo_path: &Path,
    index_path: &Path,
    benchmark: &CustomBenchmarkTrace,
    runner: &str,
    cache_paths: Option<&BenchCachePaths>,
    config: &BenchConfig,
    mut raw_writer: Option<&mut RawWriter>,
) -> Result<super::RunnerSummary> {
    let mut durations = Vec::with_capacity(config.bench_iter);
    let mut total_candidate_docs = 0usize;
    let mut total_match_count = 0usize;
    let build_overlay_each_sample = runner == "codgrep_worktree_build";
    let daemon = (!build_overlay_each_sample)
        .then(|| start_bench_daemon(repo_path, index_path, config))
        .transpose()?;
    for iter in 0..config.warmup_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let _ = run_workspace_query(
            daemon.as_ref(),
            repo_path,
            index_path,
            benchmark.pattern_at(iter),
            benchmark.case_insensitive,
            build_overlay_each_sample,
            config,
        )?;
    }
    for iter in 0..config.bench_iter {
        prepare_sample(super::CacheRunner::Bitfun, cache_paths, config)?;
        let started = Instant::now();
        let outcome = run_workspace_query(
            daemon.as_ref(),
            repo_path,
            index_path,
            benchmark.pattern_at(iter),
            benchmark.case_insensitive,
            build_overlay_each_sample,
            config,
        )?;
        let duration_secs = started.elapsed().as_secs_f64();
        total_candidate_docs += outcome.candidate_docs;
        total_match_count += outcome.match_count;
        durations.push(duration_secs);
        if let Some(writer) = raw_writer.as_deref_mut() {
            writer.write_sample(RawSample {
                benchmark: &benchmark.name,
                corpus: &benchmark.target,
                runner,
                iteration: iter,
                duration_secs,
                candidate_docs: Some(outcome.candidate_docs),
                match_count: outcome.match_count,
            })?;
        }
    }
    Ok(build_summary(
        runner.into(),
        durations,
        Some(mean_usize(total_candidate_docs, config.bench_iter)),
        mean_usize(total_match_count, config.bench_iter),
    ))
}

fn run_workspace_query(
    daemon: Option<&super::BenchDaemon>,
    repo_path: &Path,
    index_path: &Path,
    pattern: &str,
    case_insensitive: bool,
    build_overlay_each_sample: bool,
    config: &BenchConfig,
) -> Result<super::BitfunOutcome> {
    if build_overlay_each_sample {
        let daemon = start_bench_daemon(repo_path, index_path, config)?;
        return run_daemon_search_count(
            &daemon,
            repo_path,
            pattern,
            case_insensitive,
            config,
            vec![repo_path.to_path_buf()],
        );
    }
    let daemon = daemon.ok_or_else(|| {
        AppError::Protocol("missing cached daemon for dirty worktree benchmark".into())
    })?;
    run_daemon_search_count(
        daemon,
        repo_path,
        pattern,
        case_insensitive,
        config,
        vec![repo_path.to_path_buf()],
    )
}

fn cache_paths_for_fixture(
    fixture: &DirtyWorktreeFixture,
    build_overlay_each_sample: bool,
) -> Option<&BenchCachePaths> {
    if build_overlay_each_sample {
        fixture.build_cache_paths.as_ref()
    } else {
        fixture.cache_paths.as_ref()
    }
}
