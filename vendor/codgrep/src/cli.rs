use std::io::Write;
use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

use crate::{
    config::{CorpusMode, TokenizerMode},
    daemon::{
        protocol::{
            ConsistencyMode, EnsureRepoParams, GlobParams, OpenRepoParams, PathScope, QuerySpec,
            RefreshRepoParams, RepoConfig, RepoRef, Request, SearchModeConfig, SearchParams,
        },
        serve, DaemonClient, ManagedDaemonClient, ServerOptions,
    },
    error::{AppError, Result},
    tools::bench::BenchQueryMode,
    tools::bench::{self, BenchCacheMode, BenchConfig},
};

#[derive(Debug, Parser)]
#[command(name = "cg")]
#[command(about = "Repository search server and daemon tooling", version)]
#[command(subcommand_required = true, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Build(BuildArgs),
    Bench(BenchArgs),
    Serve(ServeArgs),
    Daemon(DaemonArgs),
}

#[derive(Debug, Args)]
struct BuildArgs {
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    index: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "sparse-ngram")]
    tokenizer: TokenizerArg,
    #[arg(long, value_enum)]
    corpus_mode: Option<CorpusArg>,
    #[arg(long, default_value_t = false)]
    no_ignore: bool,
    #[arg(short = '.', long, default_value_t = false)]
    hidden: bool,
    #[arg(long, default_value_t = 2 * 1024 * 1024)]
    max_file_size: u64,
    #[arg(long, default_value_t = 3)]
    min_sparse_len: usize,
    #[arg(long, default_value_t = 8)]
    max_sparse_len: usize,
}

#[derive(Debug, Args)]
struct BenchArgs {
    #[arg(long)]
    suite_dir: PathBuf,
    #[arg(long)]
    filter: Option<String>,
    #[arg(long)]
    repo: Option<PathBuf>,
    #[arg(long = "pattern")]
    patterns: Vec<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(short = 'i', long, default_value_t = false)]
    case_insensitive: bool,
    #[arg(long, value_enum, default_value = "sparse-ngram")]
    tokenizer: TokenizerArg,
    #[arg(long, value_enum, default_value = "ignore")]
    corpus_mode: CorpusArg,
    #[arg(long, value_enum, default_value = "warm")]
    cache_mode: BenchCacheArg,
    #[arg(long, value_enum, default_value = "trace")]
    query_mode: BenchQueryArg,
    #[arg(long, default_value_t = 1)]
    warmup_iter: usize,
    #[arg(long, default_value_t = 3)]
    bench_iter: usize,
    #[arg(long, default_value_t = 6)]
    top_k_tokens: usize,
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    compare_rg: bool,
    #[arg(long, default_value_t = false)]
    compare_worktree: bool,
    #[arg(long)]
    worktree_sample_files: Option<usize>,
    #[arg(long, default_value_t = false)]
    rebuild: bool,
    #[arg(long)]
    raw_output: Option<PathBuf>,
    #[arg(long)]
    cold_hook: Option<String>,
}

#[derive(Debug, Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:4597")]
    bind: String,
    #[arg(long)]
    state_file: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    stdio: bool,
}

#[derive(Debug, Args)]
struct DaemonArgs {
    #[command(subcommand)]
    command: DaemonCommands,
}

#[derive(Debug, Subcommand)]
enum DaemonCommands {
    Open(DaemonOpenArgs),
    Ensure(DaemonOpenArgs),
    Status(DaemonRepoArgs),
    Refresh(DaemonRefreshArgs),
    Build(DaemonRepoArgs),
    Rebuild(DaemonRepoArgs),
    Search(DaemonSearchArgs),
    Glob(DaemonGlobArgs),
    Close(DaemonRepoArgs),
    Shutdown(DaemonConnectArgs),
}

#[derive(Debug, Args, Clone)]
struct DaemonConnectArgs {
    #[arg(long, default_value = "127.0.0.1:4597")]
    addr: String,
}

#[derive(Debug, Args)]
struct DaemonRepoArgs {
    #[command(flatten)]
    connect: DaemonConnectArgs,
    #[arg(long)]
    repo_id: String,
}

#[derive(Debug, Args)]
struct DaemonRefreshArgs {
    #[command(flatten)]
    repo: DaemonRepoArgs,
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Debug, Args)]
struct DaemonOpenArgs {
    #[command(flatten)]
    connect: DaemonConnectArgs,
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    index: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "sparse-ngram")]
    tokenizer: TokenizerArg,
    #[arg(long, value_enum, default_value = "ignore")]
    corpus_mode: CorpusArg,
    #[arg(short = '.', long, default_value_t = false)]
    hidden: bool,
    #[arg(long, default_value_t = 2 * 1024 * 1024)]
    max_file_size: u64,
    #[arg(long, default_value_t = 3)]
    min_sparse_len: usize,
    #[arg(long, default_value_t = 8)]
    max_sparse_len: usize,
    #[arg(long, default_value_t = 256)]
    rebuild_dirty_threshold: usize,
}

#[derive(Debug, Args)]
struct DaemonSearchArgs {
    #[command(flatten)]
    repo: DaemonRepoArgs,
    #[arg(index = 1, value_name = "PATTERN")]
    pattern: String,
    #[arg(
        short = 'e',
        long = "regexp",
        value_name = "PATTERN",
        allow_hyphen_values = true
    )]
    regexp: Vec<String>,
    #[arg(short = 'F', long = "fixed-strings", default_value_t = false)]
    fixed_strings: bool,
    #[arg(short = 'i', long = "ignore-case", default_value_t = false)]
    ignore_case: bool,
    #[arg(short = 'U', long = "multiline", default_value_t = false)]
    multiline: bool,
    #[arg(long = "multiline-dotall", default_value_t = false)]
    multiline_dotall: bool,
    #[arg(short = 'w', long = "word-regexp", default_value_t = false)]
    word_regexp: bool,
    #[arg(short = 'x', long = "line-regexp", default_value_t = false)]
    line_regexp: bool,
    #[arg(short = 'A', long = "after-context")]
    after_context: Option<usize>,
    #[arg(short = 'B', long = "before-context")]
    before_context: Option<usize>,
    #[arg(short = 'm', long = "max-count")]
    max_count: Option<usize>,
    #[arg(short = 'q', long = "quiet", default_value_t = false)]
    quiet: bool,
    #[arg(short = 'c', long = "count", default_value_t = false)]
    count: bool,
    #[arg(long = "count-matches", default_value_t = false)]
    count_matches: bool,
    #[arg(long, value_enum, default_value = "workspace-eventual")]
    consistency: DaemonConsistencyArg,
    #[arg(long, default_value_t = false)]
    allow_scan_fallback: bool,
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    glob: Vec<String>,
    #[arg(long = "iglob", value_name = "GLOB")]
    iglob: Vec<String>,
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    file_type: Vec<String>,
    #[arg(short = 'T', long = "type-not", value_name = "TYPE")]
    file_type_not: Vec<String>,
    #[arg(long = "type-add", value_name = "TYPE_SPEC")]
    type_add: Vec<String>,
    #[arg(long = "type-clear", value_name = "TYPE")]
    type_clear: Vec<String>,
    #[arg(index = 2, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct DaemonGlobArgs {
    #[command(flatten)]
    repo: DaemonRepoArgs,
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    glob: Vec<String>,
    #[arg(long = "iglob", value_name = "GLOB")]
    iglob: Vec<String>,
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    file_type: Vec<String>,
    #[arg(short = 'T', long = "type-not", value_name = "TYPE")]
    file_type_not: Vec<String>,
    #[arg(long = "type-add", value_name = "TYPE_SPEC")]
    type_add: Vec<String>,
    #[arg(long = "type-clear", value_name = "TYPE")]
    type_clear: Vec<String>,
    #[arg(index = 1, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BenchCacheArg {
    Warm,
    Cold,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BenchQueryArg {
    Same,
    Trace,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TokenizerArg {
    Trigram,
    SparseNgram,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CorpusArg {
    Ignore,
    NoIgnore,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DaemonConsistencyArg {
    SnapshotOnly,
    WorkspaceEventual,
    WorkspaceStrict,
}

impl From<TokenizerArg> for TokenizerMode {
    fn from(value: TokenizerArg) -> Self {
        match value {
            TokenizerArg::Trigram => TokenizerMode::Trigram,
            TokenizerArg::SparseNgram => TokenizerMode::SparseNgram,
        }
    }
}

impl From<CorpusArg> for CorpusMode {
    fn from(value: CorpusArg) -> Self {
        match value {
            CorpusArg::Ignore => CorpusMode::RespectIgnore,
            CorpusArg::NoIgnore => CorpusMode::NoIgnore,
        }
    }
}

impl From<BenchCacheArg> for BenchCacheMode {
    fn from(value: BenchCacheArg) -> Self {
        match value {
            BenchCacheArg::Warm => BenchCacheMode::Warm,
            BenchCacheArg::Cold => BenchCacheMode::Cold,
        }
    }
}

impl From<BenchQueryArg> for BenchQueryMode {
    fn from(value: BenchQueryArg) -> Self {
        match value {
            BenchQueryArg::Same => BenchQueryMode::Same,
            BenchQueryArg::Trace => BenchQueryMode::Trace,
        }
    }
}

pub(crate) fn run() -> Result<bool> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build(args) => run_build(args),
        Commands::Bench(args) => run_bench(BenchConfig {
            suite_dir: args.suite_dir,
            filter: args.filter,
            custom_repo: args.repo,
            custom_patterns: args.patterns,
            custom_name: args.name,
            custom_case_insensitive: args.case_insensitive,
            tokenizer: args.tokenizer.into(),
            corpus_mode: args.corpus_mode.into(),
            cache_mode: args.cache_mode.into(),
            query_mode: args.query_mode.into(),
            warmup_iter: args.warmup_iter,
            bench_iter: args.bench_iter,
            top_k_tokens: args.top_k_tokens,
            compare_rg: args.compare_rg,
            compare_worktree: args.compare_worktree,
            worktree_sample_files: args.worktree_sample_files,
            rebuild: args.rebuild,
            raw_output: args.raw_output,
            cold_hook: args.cold_hook,
        }),
        Commands::Serve(args) => run_serve(args),
        Commands::Daemon(args) => run_daemon(args),
    }
}

fn run_build(args: BuildArgs) -> Result<bool> {
    let corpus_mode = if args.no_ignore {
        CorpusMode::NoIgnore
    } else {
        args.corpus_mode.unwrap_or(CorpusArg::Ignore).into()
    };
    let params = EnsureRepoParams {
        repo_path: args.repo,
        index_path: args.index,
        config: RepoConfig {
            tokenizer: match args.tokenizer {
                TokenizerArg::Trigram => crate::daemon::protocol::TokenizerModeConfig::Trigram,
                TokenizerArg::SparseNgram => {
                    crate::daemon::protocol::TokenizerModeConfig::SparseNgram
                }
            },
            corpus_mode: match corpus_mode {
                CorpusMode::RespectIgnore => {
                    crate::daemon::protocol::CorpusModeConfig::RespectIgnore
                }
                CorpusMode::NoIgnore => crate::daemon::protocol::CorpusModeConfig::NoIgnore,
            },
            include_hidden: args.hidden,
            max_file_size: args.max_file_size,
            min_sparse_len: args.min_sparse_len,
            max_sparse_len: args.max_sparse_len,
        },
        refresh: crate::daemon::protocol::RefreshPolicyConfig::default(),
    };
    let ensured = ManagedDaemonClient::new().ensure_repo(params)?;
    match ensured.indexed_docs {
        Some(indexed_docs) => {
            println!(
                "ensured repo via daemon at {}: indexed {indexed_docs} docs",
                ensured.addr
            );
        }
        None => {
            println!(
                "ensured repo via daemon at {}: base snapshot already available",
                ensured.addr
            );
        }
    }
    Ok(true)
}

fn run_serve(args: ServeArgs) -> Result<bool> {
    serve(ServerOptions {
        bind_addr: args.bind,
        state_file: args.state_file,
        stdio: args.stdio,
    })?;
    Ok(true)
}

fn run_daemon(args: DaemonArgs) -> Result<bool> {
    match args.command {
        DaemonCommands::Open(args) => {
            let addr = args.connect.addr.clone();
            let params = build_daemon_open_params(args);
            let client = DaemonClient::new(addr);
            let response = client.open_repo(params)?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Ensure(args) => {
            let addr = args.connect.addr.clone();
            let open = build_daemon_open_params(args);
            let client = DaemonClient::new(addr);
            let response = client.ensure_repo(crate::daemon::protocol::EnsureRepoParams {
                repo_path: open.repo_path,
                index_path: open.index_path,
                config: open.config,
                refresh: open.refresh,
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Status(args) => {
            let client = DaemonClient::new(args.connect.addr);
            let response = client.send(Request::GetRepoStatus {
                params: RepoRef {
                    repo_id: args.repo_id,
                },
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Refresh(args) => {
            let client = DaemonClient::new(args.repo.connect.addr);
            let response = client.send(Request::RefreshRepo {
                params: RefreshRepoParams {
                    repo_id: args.repo.repo_id,
                    force: args.force,
                },
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Build(args) => {
            let client = DaemonClient::new(args.connect.addr);
            let response = client.send(Request::BuildIndex {
                params: RepoRef {
                    repo_id: args.repo_id,
                },
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Rebuild(args) => {
            let client = DaemonClient::new(args.connect.addr);
            let response = client.send(Request::RebuildIndex {
                params: RepoRef {
                    repo_id: args.repo_id,
                },
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Search(args) => {
            let client = DaemonClient::new(args.repo.connect.addr);
            let mut patterns = vec![args.pattern.clone()];
            patterns.extend(args.regexp);
            let combined_pattern = combine_daemon_patterns(&patterns, args.fixed_strings);
            let response = client.send(Request::Search {
                params: SearchParams {
                    repo_id: args.repo.repo_id,
                    query: QuerySpec {
                        pattern: combined_pattern,
                        patterns,
                        case_insensitive: args.ignore_case,
                        multiline: args.multiline,
                        dot_matches_new_line: args.multiline_dotall,
                        fixed_strings: args.fixed_strings,
                        word_regexp: args.word_regexp,
                        line_regexp: args.line_regexp,
                        before_context: args.before_context.unwrap_or(0),
                        after_context: args.after_context.unwrap_or(0),
                        top_k_tokens: 6,
                        max_count: args.max_count,
                        global_max_results: None,
                        search_mode: if args.quiet {
                            SearchModeConfig::FirstHitOnly
                        } else if args.count_matches {
                            SearchModeConfig::CountMatches
                        } else if args.count {
                            SearchModeConfig::CountOnly
                        } else {
                            SearchModeConfig::MaterializeMatches
                        },
                    },
                    scope: PathScope {
                        roots: args.paths,
                        globs: args.glob,
                        iglobs: args.iglob,
                        type_add: args.type_add,
                        type_clear: args.type_clear,
                        types: args.file_type,
                        type_not: args.file_type_not,
                    },
                    consistency: match args.consistency {
                        DaemonConsistencyArg::SnapshotOnly => ConsistencyMode::SnapshotOnly,
                        DaemonConsistencyArg::WorkspaceEventual => {
                            ConsistencyMode::WorkspaceEventual
                        }
                        DaemonConsistencyArg::WorkspaceStrict => ConsistencyMode::WorkspaceStrict,
                    },
                    allow_scan_fallback: args.allow_scan_fallback,
                },
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Glob(args) => {
            let client = DaemonClient::new(args.repo.connect.addr);
            let response = client.glob(GlobParams {
                repo_id: args.repo.repo_id,
                scope: PathScope {
                    roots: args.paths,
                    globs: args.glob,
                    iglobs: args.iglob,
                    type_add: args.type_add,
                    type_clear: args.type_clear,
                    types: args.file_type,
                    type_not: args.file_type_not,
                },
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Close(args) => {
            let client = DaemonClient::new(args.connect.addr);
            let response = client.send(Request::CloseRepo {
                params: RepoRef {
                    repo_id: args.repo_id,
                },
            })?;
            print_daemon_response(&response)?;
            Ok(true)
        }
        DaemonCommands::Shutdown(args) => {
            let client = DaemonClient::new(args.addr);
            let response = client.send(Request::Shutdown)?;
            print_daemon_response(&response)?;
            Ok(true)
        }
    }
}

fn build_daemon_open_params(args: DaemonOpenArgs) -> OpenRepoParams {
    OpenRepoParams {
        repo_path: args.repo,
        index_path: args.index,
        config: RepoConfig {
            tokenizer: match args.tokenizer {
                TokenizerArg::Trigram => crate::daemon::protocol::TokenizerModeConfig::Trigram,
                TokenizerArg::SparseNgram => {
                    crate::daemon::protocol::TokenizerModeConfig::SparseNgram
                }
            },
            corpus_mode: match args.corpus_mode {
                CorpusArg::Ignore => crate::daemon::protocol::CorpusModeConfig::RespectIgnore,
                CorpusArg::NoIgnore => crate::daemon::protocol::CorpusModeConfig::NoIgnore,
            },
            include_hidden: args.hidden,
            max_file_size: args.max_file_size,
            min_sparse_len: args.min_sparse_len,
            max_sparse_len: args.max_sparse_len,
        },
        refresh: crate::daemon::protocol::RefreshPolicyConfig {
            rebuild_dirty_threshold: args.rebuild_dirty_threshold,
        },
    }
}

fn print_daemon_response(response: &crate::daemon::protocol::Response) -> Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer_pretty(&mut stdout, response).map_err(|error| {
        AppError::Protocol(format!("failed to encode daemon response: {error}"))
    })?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn combine_daemon_patterns(patterns: &[String], fixed_strings: bool) -> String {
    if patterns.len() == 1 {
        return if fixed_strings {
            regex::escape(&patterns[0])
        } else {
            patterns[0].clone()
        };
    }

    let branches = patterns
        .iter()
        .map(|pattern| {
            if fixed_strings {
                regex::escape(pattern)
            } else {
                pattern.clone()
            }
        })
        .collect::<Vec<_>>();
    format!("(?:{})", branches.join("|"))
}

fn run_bench(config: BenchConfig) -> Result<bool> {
    if cfg!(debug_assertions) {
        eprintln!(
            "warning: cg bench is running from a debug build; performance results will be pessimistic. use `cargo run --release -- bench ...` or `target/release/cg bench ...` for representative numbers."
        );
    }
    let report = bench::run(&config)?;
    bench::print_report(&report);
    Ok(true)
}
