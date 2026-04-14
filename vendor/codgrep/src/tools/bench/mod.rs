mod cache;
mod custom;
mod fixtures;
mod report;
mod standard;
#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    config::{BuildConfig, CorpusMode, TokenizerMode},
    daemon::{
        protocol::{
            ConsistencyMode, EnsureRepoParams, PathScope, QuerySpec, RepoConfig, SearchModeConfig,
            SearchParams,
        },
        DaemonClient, ManagedDaemonClient,
    },
    error::{AppError, Result},
    files::{repo_relative_path, scan_repository},
    index::{build_index, IndexSearcher},
};

use self::{
    cache::collect_cache_paths,
    custom::run_custom,
    fixtures::prepare_dirty_worktree_fixture,
    report::{print_report as print_report_impl, RawWriter},
    standard::{run_codgrep_benchmark, run_rg_benchmark, run_worktree_benchmark},
};

#[derive(Debug, Clone)]
pub(crate) struct BenchConfig {
    pub(crate) suite_dir: PathBuf,
    pub(crate) filter: Option<String>,
    pub(crate) custom_repo: Option<PathBuf>,
    pub(crate) custom_patterns: Vec<String>,
    pub(crate) custom_name: Option<String>,
    pub(crate) custom_case_insensitive: bool,
    pub(crate) tokenizer: TokenizerMode,
    pub(crate) corpus_mode: CorpusMode,
    pub(crate) cache_mode: BenchCacheMode,
    pub(crate) query_mode: BenchQueryMode,
    pub(crate) warmup_iter: usize,
    pub(crate) bench_iter: usize,
    pub(crate) top_k_tokens: usize,
    pub(crate) compare_rg: bool,
    pub(crate) compare_worktree: bool,
    pub(crate) worktree_sample_files: Option<usize>,
    pub(crate) rebuild: bool,
    pub(crate) raw_output: Option<PathBuf>,
    pub(crate) cold_hook: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BenchCacheMode {
    Warm,
    Cold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BenchQueryMode {
    Same,
    Trace,
}

impl BenchCacheMode {
    fn is_cold(self) -> bool {
        matches!(self, Self::Cold)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BenchReport {
    corpus_builds: Vec<CorpusBuildRecord>,
    summaries: Vec<BenchSummary>,
}

#[derive(Debug, Clone)]
struct CorpusBuildRecord {
    corpus: String,
    tokenizer: TokenizerMode,
    docs_indexed: usize,
    duration_secs: f64,
}

#[derive(Debug, Clone)]
struct BenchSummary {
    name: String,
    pattern: String,
    target: String,
    runners: Vec<RunnerSummary>,
}

#[derive(Debug, Clone)]
struct RunnerSummary {
    runner: String,
    mean_secs: f64,
    stddev_secs: f64,
    min_secs: f64,
    sample_count: usize,
    candidate_docs: Option<usize>,
    match_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Corpus {
    Linux,
    SubtitlesEn,
    SubtitlesRu,
}

pub(super) const BENCH_MAX_FILE_SIZE: u64 = 512 * 1024 * 1024;

impl Corpus {
    fn label(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::SubtitlesEn => "subtitles-en",
            Self::SubtitlesRu => "subtitles-ru",
        }
    }

    fn repo_path(self, suite_dir: &Path) -> PathBuf {
        match self {
            Self::Linux => suite_dir.join("linux"),
            Self::SubtitlesEn => suite_dir.join("subtitles").join("en.sample.txt"),
            Self::SubtitlesRu => suite_dir.join("subtitles").join("ru.txt"),
        }
    }

    fn search_target(self, suite_dir: &Path) -> PathBuf {
        self.repo_path(suite_dir)
    }
}

pub(super) fn bench_build_config(
    repo_path: &Path,
    index_path: &Path,
    config: &BenchConfig,
) -> BuildConfig {
    BuildConfig {
        repo_path: repo_path.to_path_buf(),
        index_path: index_path.to_path_buf(),
        tokenizer: config.tokenizer,
        corpus_mode: config.corpus_mode,
        include_hidden: false,
        max_file_size: BENCH_MAX_FILE_SIZE,
        min_sparse_len: 3,
        max_sparse_len: 8,
    }
}

pub(super) fn configure_rg_command(command: &mut Command, config: &BenchConfig) {
    command.arg("--count");
    command.arg("--with-filename");
    command.arg("--max-filesize");
    command.arg(BENCH_MAX_FILE_SIZE.to_string());
    if matches!(config.corpus_mode, CorpusMode::NoIgnore) {
        command.arg("--no-ignore");
    }
}

pub(super) fn bench_index_is_usable(index_path: &Path, expected: &BuildConfig) -> bool {
    let Ok(searcher) = IndexSearcher::open(index_path.to_path_buf()) else {
        return false;
    };
    if searcher.tokenizer_mode() != expected.tokenizer {
        return false;
    }
    let tokenizer_options = searcher.tokenizer_options();
    if tokenizer_options.min_sparse_len != expected.min_sparse_len
        || tokenizer_options.max_sparse_len != expected.max_sparse_len
    {
        return false;
    }
    let Some(build) = searcher.build_settings() else {
        return false;
    };
    build.corpus_mode == expected.corpus_mode
        && build.include_hidden == expected.include_hidden
        && build.max_file_size == expected.max_file_size
}

#[derive(Debug, Clone, Copy)]
struct BenchmarkCase {
    name: &'static str,
    corpus: Corpus,
    pattern: &'static str,
    case_insensitive: bool,
}

#[derive(Debug, Clone)]
struct CustomBenchmarkCase {
    name: String,
    target: String,
    repo_path: PathBuf,
    pattern: String,
    case_insensitive: bool,
}

#[derive(Debug, Clone)]
struct CustomBenchmarkTrace {
    name: String,
    target: String,
    repo_path: PathBuf,
    patterns: Vec<String>,
    case_insensitive: bool,
}

#[derive(Debug, Clone)]
struct DirtyPattern {
    regex_pattern: String,
    case_insensitive: bool,
}

impl CustomBenchmarkTrace {
    fn pattern_at(&self, iteration: usize) -> &str {
        &self.patterns[iteration % self.patterns.len()]
    }
}

#[derive(Debug, Clone)]
struct BenchCachePaths {
    corpus_files: Vec<PathBuf>,
    index_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct DirtyWorktreeFixture {
    repo_path: PathBuf,
    index_path: PathBuf,
    cache_paths: Option<BenchCachePaths>,
    build_cache_paths: Option<BenchCachePaths>,
}

#[derive(Debug, Clone)]
pub(super) struct RgSearchScope {
    current_dir: Option<PathBuf>,
    target: PathBuf,
    ignore_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum CacheRunner {
    Bitfun,
    Rg,
}

pub(super) struct BitfunOutcome {
    candidate_docs: usize,
    match_count: usize,
}

pub(super) struct BenchDaemon {
    client: DaemonClient,
    addr: String,
    repo_id: String,
}

const BENCHMARKS: &[BenchmarkCase] = &[
    BenchmarkCase {
        name: "linux_literal",
        corpus: Corpus::Linux,
        pattern: "PM_RESUME",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "linux_literal_casei",
        corpus: Corpus::Linux,
        pattern: "PM_RESUME",
        case_insensitive: true,
    },
    BenchmarkCase {
        name: "linux_re_literal_suffix",
        corpus: Corpus::Linux,
        pattern: "[A-Z]+_RESUME",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "linux_word",
        corpus: Corpus::Linux,
        pattern: "\\bPM_RESUME\\b",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "linux_unicode_greek",
        corpus: Corpus::Linux,
        pattern: "\\p{Greek}",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "linux_unicode_greek_casei",
        corpus: Corpus::Linux,
        pattern: "\\p{Greek}",
        case_insensitive: true,
    },
    BenchmarkCase {
        name: "linux_unicode_word",
        corpus: Corpus::Linux,
        pattern: "\\wAh",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "linux_no_literal",
        corpus: Corpus::Linux,
        pattern: "\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "linux_alternates",
        corpus: Corpus::Linux,
        pattern: "ERR_SYS|PME_TURN_OFF|LINK_REQ_RST|CFG_BME_EVT",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "linux_alternates_casei",
        corpus: Corpus::Linux,
        pattern: "ERR_SYS|PME_TURN_OFF|LINK_REQ_RST|CFG_BME_EVT",
        case_insensitive: true,
    },
    BenchmarkCase {
        name: "subtitles_en_literal",
        corpus: Corpus::SubtitlesEn,
        pattern: "Sherlock Holmes",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_en_literal_casei",
        corpus: Corpus::SubtitlesEn,
        pattern: "Sherlock Holmes",
        case_insensitive: true,
    },
    BenchmarkCase {
        name: "subtitles_en_literal_word",
        corpus: Corpus::SubtitlesEn,
        pattern: "\\bSherlock Holmes\\b",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_en_alternate",
        corpus: Corpus::SubtitlesEn,
        pattern: "Sherlock Holmes|John Watson|Irene Adler|Inspector Lestrade|Professor Moriarty",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_en_alternate_casei",
        corpus: Corpus::SubtitlesEn,
        pattern: "Sherlock Holmes|John Watson|Irene Adler|Inspector Lestrade|Professor Moriarty",
        case_insensitive: true,
    },
    BenchmarkCase {
        name: "subtitles_en_surrounding_words",
        corpus: Corpus::SubtitlesEn,
        pattern: "\\w+\\s+Holmes\\s+\\w+",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_en_no_literal",
        corpus: Corpus::SubtitlesEn,
        pattern: "\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_ru_literal",
        corpus: Corpus::SubtitlesRu,
        pattern: "Шерлок Холмс",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_ru_literal_casei",
        corpus: Corpus::SubtitlesRu,
        pattern: "Шерлок Холмс",
        case_insensitive: true,
    },
    BenchmarkCase {
        name: "subtitles_ru_literal_word",
        corpus: Corpus::SubtitlesRu,
        pattern: "\\bШерлок Холмс\\b",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_ru_alternate",
        corpus: Corpus::SubtitlesRu,
        pattern: "Шерлок Холмс|Джон Уотсон|Ирен Адлер|инспектор Лестрейд|профессор Мориарти",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_ru_alternate_casei",
        corpus: Corpus::SubtitlesRu,
        pattern: "Шерлок Холмс|Джон Уотсон|Ирен Адлер|инспектор Лестрейд|профессор Мориарти",
        case_insensitive: true,
    },
    BenchmarkCase {
        name: "subtitles_ru_surrounding_words",
        corpus: Corpus::SubtitlesRu,
        pattern: "\\w+\\s+Холмс\\s+\\w+",
        case_insensitive: false,
    },
    BenchmarkCase {
        name: "subtitles_ru_no_literal",
        corpus: Corpus::SubtitlesRu,
        pattern: "\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}\\s+\\w{5}",
        case_insensitive: false,
    },
];

pub(crate) fn run(config: &BenchConfig) -> Result<BenchReport> {
    validate_config(config)?;
    let mut raw_writer = match &config.raw_output {
        Some(path) => Some(RawWriter::create(path)?),
        None => None,
    };
    if config.custom_repo.is_some() {
        return run_custom(config, raw_writer.as_mut());
    }

    let selected = select_benchmarks(config);
    if selected.is_empty() {
        return Err(AppError::InvalidPattern(
            "no benchmarks matched the provided filter".into(),
        ));
    }
    let dirty_patterns_by_corpus = collect_dirty_patterns_by_corpus(&selected);

    let corpora = selected
        .iter()
        .map(|case| case.corpus)
        .collect::<std::collections::BTreeSet<_>>();
    let mut index_paths = BTreeMap::new();
    let mut rg_scopes = BTreeMap::new();
    let mut cache_paths = BTreeMap::new();
    let mut dirty_worktree_fixtures = BTreeMap::new();
    let mut corpus_builds = Vec::new();

    for corpus in corpora {
        ensure_corpus_exists(corpus, &config.suite_dir)?;
        let repo_path = corpus.repo_path(&config.suite_dir);
        let index_path = config.suite_dir.join(".codgrep-bench").join(format!(
            "{}-{}-{}",
            corpus.label(),
            config.corpus_mode.as_str(),
            config.tokenizer.as_str()
        ));
        let build_config = bench_build_config(&repo_path, &index_path, config);

        if config.rebuild && index_path.exists() {
            fs::remove_dir_all(&index_path)?;
        }
        if config.rebuild || !bench_index_is_usable(&index_path, &build_config) {
            let started = std::time::Instant::now();
            let docs_indexed = build_index(&build_config)?;
            corpus_builds.push(CorpusBuildRecord {
                corpus: corpus.label().to_string(),
                tokenizer: config.tokenizer,
                docs_indexed,
                duration_secs: started.elapsed().as_secs_f64(),
            });
        }
        rg_scopes.insert(
            corpus,
            prepare_rg_search_scope(&repo_path, &index_path, config)?,
        );
        let cache_entry = if config.cache_mode.is_cold() {
            Some(collect_cache_paths(
                corpus,
                &repo_path,
                &index_path,
                config,
            )?)
        } else {
            None
        };
        index_paths.insert(corpus, index_path.clone());
        if let Some(cache_entry) = cache_entry {
            cache_paths.insert(corpus, cache_entry);
        }

        if config.compare_worktree {
            let fixture_label = format!(
                "{}-{}-{}",
                corpus.label(),
                config.corpus_mode.as_str(),
                config.tokenizer.as_str()
            );
            let patterns = dirty_patterns_by_corpus
                .get(&corpus)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            dirty_worktree_fixtures.insert(
                corpus,
                prepare_dirty_worktree_fixture(
                    &repo_path,
                    &config.suite_dir,
                    &fixture_label,
                    config,
                    patterns,
                )?,
            );
        }
    }

    let mut summaries = Vec::new();
    for benchmark in selected {
        let index_path = index_paths.get(&benchmark.corpus).ok_or_else(|| {
            AppError::InvalidIndex(format!(
                "missing benchmark index for corpus {}",
                benchmark.corpus.label()
            ))
        })?;
        let cache_paths = cache_paths.get(&benchmark.corpus);
        let mut runners = vec![run_codgrep_benchmark(
            benchmark,
            index_path,
            &config.suite_dir,
            cache_paths,
            config,
            raw_writer.as_mut(),
        )?];

        if config.compare_rg {
            let rg_scope = rg_scopes.get(&benchmark.corpus).ok_or_else(|| {
                AppError::InvalidIndex(format!(
                    "missing rg scope for corpus {}",
                    benchmark.corpus.label()
                ))
            })?;
            runners.push(run_rg_benchmark(
                benchmark,
                rg_scope,
                cache_paths,
                config,
                raw_writer.as_mut(),
            )?);
        }

        if config.compare_worktree {
            let fixture = dirty_worktree_fixtures
                .get(&benchmark.corpus)
                .ok_or_else(|| {
                    AppError::InvalidIndex(format!(
                        "missing dirty worktree fixture for corpus {}",
                        benchmark.corpus.label()
                    ))
                })?;
            runners.push(run_worktree_benchmark(
                benchmark,
                &fixture.repo_path,
                &fixture.index_path,
                fixture.build_cache_paths.as_ref(),
                true,
                config,
                raw_writer.as_mut(),
            )?);
            runners.push(run_worktree_benchmark(
                benchmark,
                &fixture.repo_path,
                &fixture.index_path,
                fixture.cache_paths.as_ref(),
                false,
                config,
                raw_writer.as_mut(),
            )?);
        }

        summaries.push(BenchSummary {
            name: benchmark.name.to_string(),
            pattern: benchmark.pattern.to_string(),
            target: benchmark.corpus.label().to_string(),
            runners,
        });
    }

    Ok(BenchReport {
        corpus_builds,
        summaries,
    })
}

pub(crate) fn print_report(report: &BenchReport) {
    print_report_impl(report);
}

fn validate_config(config: &BenchConfig) -> Result<()> {
    if !config.cache_mode.is_cold() && config.cold_hook.is_some() {
        return Err(AppError::InvalidPattern(
            "--cold-hook requires --cache-mode cold".into(),
        ));
    }
    if config.custom_repo.is_some() && config.custom_patterns.is_empty() {
        return Err(AppError::InvalidPattern(
            "--repo requires at least one --pattern".into(),
        ));
    }
    if config.custom_repo.is_none() && !config.custom_patterns.is_empty() {
        return Err(AppError::InvalidPattern(
            "--pattern requires --repo for ad-hoc benchmarks".into(),
        ));
    }
    if matches!(config.worktree_sample_files, Some(0)) {
        return Err(AppError::InvalidPattern(
            "--worktree-sample-files must be greater than zero".into(),
        ));
    }
    Ok(())
}

fn collect_dirty_patterns_by_corpus(
    selected: &[BenchmarkCase],
) -> BTreeMap<Corpus, Vec<DirtyPattern>> {
    let mut patterns = BTreeMap::<Corpus, Vec<DirtyPattern>>::new();
    for benchmark in selected {
        patterns
            .entry(benchmark.corpus)
            .or_default()
            .push(DirtyPattern {
                regex_pattern: benchmark.pattern.to_string(),
                case_insensitive: benchmark.case_insensitive,
            });
    }
    patterns
}

fn select_benchmarks(config: &BenchConfig) -> Vec<BenchmarkCase> {
    BENCHMARKS
        .iter()
        .copied()
        .filter(|case| match &config.filter {
            Some(filter) => case.name.contains(filter),
            None => true,
        })
        .collect()
}

fn ensure_corpus_exists(corpus: Corpus, suite_dir: &Path) -> Result<()> {
    let path = corpus.repo_path(suite_dir);
    if path.exists() {
        return Ok(());
    }
    Err(AppError::InvalidIndex(format!(
        "missing benchmark corpus for {} at {}",
        corpus.label(),
        path.display()
    )))
}

fn parse_rg_count_output(stdout: &[u8]) -> Result<usize> {
    let mut total = 0usize;
    for line in stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let line = std::str::from_utf8(line).map_err(|error| {
            AppError::InvalidIndex(format!("rg count output was not utf-8: {error}"))
        })?;
        let count = line
            .rsplit_once(':')
            .map_or(line, |(_, count)| count)
            .parse::<usize>()
            .map_err(|error| {
                AppError::InvalidIndex(format!("failed to parse rg count output '{line}': {error}"))
            })?;
        total += count;
    }
    Ok(total)
}

pub(super) fn prepare_rg_search_scope(
    repo_path: &Path,
    index_path: &Path,
    config: &BenchConfig,
) -> Result<RgSearchScope> {
    if repo_path.is_dir() {
        return Ok(RgSearchScope {
            current_dir: Some(repo_path.to_path_buf()),
            target: PathBuf::from("."),
            ignore_file: prepare_rg_ignore_file(repo_path, index_path, config)?,
        });
    }

    Ok(RgSearchScope {
        current_dir: None,
        target: repo_path.to_path_buf(),
        ignore_file: None,
    })
}

fn prepare_rg_ignore_file(
    repo_path: &Path,
    index_path: &Path,
    config: &BenchConfig,
) -> Result<Option<PathBuf>> {
    let repo_root = fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());
    let indexed = IndexSearcher::open(index_path.to_path_buf())?
        .indexed_paths(None)
        .into_iter()
        .map(|path| canonical_repo_relative_path(Path::new(&path), &repo_root))
        .collect::<HashSet<_>>();
    let excluded = scan_repository(&bench_build_config(repo_path, index_path, config))?
        .into_iter()
        .filter_map(|file| {
            let rel = canonical_repo_relative_path(&file.path, &repo_root);
            (!indexed.contains(&rel)).then_some(rel)
        })
        .collect::<Vec<_>>();
    if excluded.is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(index_path)?;
    let ignore_path = index_path.join("bench-rg-exclude.ignore");
    let mut contents = String::new();
    for path in excluded {
        contents.push('/');
        contents.push_str(&escape_ignore_pattern(&path));
        contents.push('\n');
    }
    fs::write(&ignore_path, contents)?;
    Ok(Some(ignore_path))
}

fn canonical_repo_relative_path(path: &Path, repo_root: &Path) -> String {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    repo_relative_path(&canonical, repo_root)
}

fn escape_ignore_pattern(path: &str) -> String {
    let mut escaped = String::with_capacity(path.len());
    for ch in path.chars() {
        match ch {
            '\\' | '*' | '?' | '[' | ']' | '{' | '}' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn codgrep_executable() -> Result<PathBuf> {
    if let Ok(path) = env::var("CARGO_BIN_EXE_cg") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let current = env::current_exe()?;
    if current
        .file_stem()
        .is_some_and(|stem| stem == std::ffi::OsStr::new("cg"))
    {
        return Ok(current);
    }

    let candidates = [
        current
            .parent()
            .map(|parent| parent.join("cg"))
            .unwrap_or_else(|| PathBuf::from("cg")),
        current
            .parent()
            .and_then(Path::parent)
            .map(|parent| parent.join("cg"))
            .unwrap_or_else(|| PathBuf::from("cg")),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Ok(PathBuf::from("cg"))
}

pub(super) fn start_bench_daemon(
    repo_path: &Path,
    index_path: &Path,
    config: &BenchConfig,
) -> Result<BenchDaemon> {
    let managed = ManagedDaemonClient::new().with_daemon_program(codgrep_executable()?);
    let ensured = managed.ensure_repo(EnsureRepoParams {
        repo_path: repo_path.to_path_buf(),
        index_path: Some(index_path.to_path_buf()),
        config: RepoConfig {
            tokenizer: match config.tokenizer {
                TokenizerMode::Trigram => crate::daemon::protocol::TokenizerModeConfig::Trigram,
                TokenizerMode::SparseNgram => {
                    crate::daemon::protocol::TokenizerModeConfig::SparseNgram
                }
            },
            corpus_mode: match config.corpus_mode {
                CorpusMode::RespectIgnore => {
                    crate::daemon::protocol::CorpusModeConfig::RespectIgnore
                }
                CorpusMode::NoIgnore => crate::daemon::protocol::CorpusModeConfig::NoIgnore,
            },
            include_hidden: false,
            max_file_size: 512 * 1024 * 1024,
            min_sparse_len: 3,
            max_sparse_len: 8,
        },
        refresh: Default::default(),
    })?;
    Ok(BenchDaemon {
        client: DaemonClient::new(ensured.addr.clone()),
        addr: ensured.addr,
        repo_id: ensured.repo_id,
    })
}

pub(super) fn run_daemon_search_count(
    daemon: &BenchDaemon,
    repo_path: &Path,
    pattern: &str,
    case_insensitive: bool,
    config: &BenchConfig,
    roots: Vec<PathBuf>,
) -> Result<BitfunOutcome> {
    let roots = benchmark_scope_roots(repo_path, roots);
    let response = daemon.client.search(SearchParams {
        repo_id: daemon.repo_id.clone(),
        query: QuerySpec {
            pattern: pattern.to_string(),
            patterns: vec![pattern.to_string()],
            case_insensitive,
            multiline: false,
            dot_matches_new_line: false,
            fixed_strings: false,
            word_regexp: false,
            line_regexp: false,
            before_context: 0,
            after_context: 0,
            top_k_tokens: config.top_k_tokens,
            max_count: None,
            global_max_results: None,
            search_mode: SearchModeConfig::CountOnly,
        },
        scope: PathScope {
            roots,
            globs: Vec::new(),
            iglobs: Vec::new(),
            type_add: Vec::new(),
            type_clear: Vec::new(),
            types: Vec::new(),
            type_not: Vec::new(),
        },
        consistency: ConsistencyMode::WorkspaceEventual,
        allow_scan_fallback: false,
    })?;

    match response {
        crate::daemon::protocol::Response::SearchCompleted { results, .. } => Ok(BitfunOutcome {
            candidate_docs: results.candidate_docs,
            match_count: results.matched_lines,
        }),
        other => Err(AppError::Protocol(format!(
            "unexpected daemon bench response: {other:?}"
        ))),
    }
}

pub(super) fn benchmark_scope_roots(repo_path: &Path, roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let canonical_repo = std::fs::canonicalize(repo_path).ok();
    roots
        .into_iter()
        .filter_map(|root| normalize_scope_root(repo_path, canonical_repo.as_deref(), root))
        .collect()
}

fn normalize_scope_root(
    repo_path: &Path,
    canonical_repo: Option<&Path>,
    root: PathBuf,
) -> Option<PathBuf> {
    if root == repo_path {
        return None;
    }
    if let Ok(relative) = root.strip_prefix(repo_path) {
        return (!relative.as_os_str().is_empty()).then(|| relative.to_path_buf());
    }

    let canonical_root = std::fs::canonicalize(&root).ok();
    if let (Some(canonical_repo), Some(canonical_root)) =
        (canonical_repo, canonical_root.as_deref())
    {
        if canonical_root == canonical_repo {
            return None;
        }
        if let Ok(relative) = canonical_root.strip_prefix(canonical_repo) {
            return (!relative.as_os_str().is_empty()).then(|| relative.to_path_buf());
        }
    }

    Some(root)
}

impl Drop for BenchDaemon {
    fn drop(&mut self) {
        let _ = self.client.send(crate::daemon::protocol::Request::Shutdown);
        let _ = &self.addr;
    }
}

#[cfg(test)]
use self::fixtures::copy_tree;
