mod cli;
mod config;
pub mod daemon;
pub mod error;
mod files;
mod index;
mod json_output;
mod path_filter;
mod path_utils;
mod planner;
mod progress;
mod query_preflight;
pub mod sdk;
mod search;
mod search_engine;
mod tokenizer;
mod tools;
mod workspace;

pub use error::{AppError, Result};

#[deprecated(
    note = "Use the daemon process API (`codgrep::daemon::protocol`) or `codgrep::sdk`; library exports are not the stable external interface."
)]
pub use config::{BuildConfig, CorpusMode, QueryConfig, TokenizerMode};
#[deprecated(
    note = "Use daemon protocol requests or `codgrep::sdk` for index lifecycle; direct library build functions are not the stable external interface."
)]
pub use index::{
    build_index, build_index_with_options, rebuild_index, rebuild_index_with_options,
    IndexBuildOptions, RebuildMode,
};
#[deprecated(
    note = "Use daemon protocol search responses/results or `codgrep::sdk`; library search models are not the stable external interface."
)]
pub use search::{SearchMode, SearchResults};
#[deprecated(
    note = "Use the daemon process API or `codgrep::sdk` instead of direct library search facades."
)]
pub use search_engine::{SearchBackend, SearchEngine, SearchPolicy, SearchResponse};
#[deprecated(
    note = "Use the daemon process API or `codgrep::sdk` instead of direct workspace library facades."
)]
pub use workspace::{
    WorkspaceFreshness, WorkspaceFreshnessState, WorkspaceIndex, WorkspaceIndexOptions,
    WorkspaceSnapshot,
};

/// Expert-facing API surface for advanced integration scenarios.
#[deprecated(
    note = "Advanced library APIs are internal/experimental integration helpers, not the stable external contract; prefer the daemon process API or `codgrep::sdk`."
)]
pub mod advanced {
    pub use crate::files::{
        read_text_file, repo_relative_path, resolve_repo_path, scan_paths, scan_repository,
        RepositoryFile, ScanOptions,
    };
    pub use crate::index::{IndexSearcher, IndexWorktreeDiff};
    pub use crate::path_filter::{normalize_path, PathFilter, PathFilterArgs};
    pub use crate::progress::{IndexProgress, IndexProgressPhase};
    pub use crate::search::{
        FileContext, FileCount, FileMatch, FileMatchCount, MatchLocation, SearchHit, SearchLine,
    };
    pub use crate::workspace::{
        BaseSnapshotInfo, BaseSnapshotKind, IndexStatus, WorkspaceSnapshot,
    };

    /// JSON event/output model types.
    ///
    /// Primarily intended for CLI/integration output adapters; schema may
    /// evolve over time.
    pub mod json {
        pub use crate::json_output::{
            JsonBegin, JsonContext, JsonData, JsonDuration, JsonEnd, JsonMatch, JsonMessage,
            JsonSearchFile, JsonSearchReport, JsonStats, JsonSubmatch, JsonSummary,
        };
    }
}

/// Unstable or evolving API surface intended for experimentation.
#[deprecated(
    note = "Experimental library APIs are not the stable external contract; prefer the daemon process API or `codgrep::sdk`."
)]
pub mod experimental {
    pub use crate::planner::{plan, PureLiteralAlternation, QueryBranch, QueryPlan};
    pub use crate::tokenizer::{create, hash_token, unique_sorted, Tokenizer, TokenizerOptions};

    /// Low-level on-disk index format helpers.
    pub mod index_format {
        pub use crate::index::format::*;
    }
}

#[doc(hidden)]
pub fn run_cli() -> Result<bool> {
    cli::run()
}
