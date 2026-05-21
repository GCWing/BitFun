mod client;
pub mod error;
mod protocol;
mod repo_session;
mod rpc_client;
mod types;

pub(crate) const FLASHGREP_LOG_TARGET: &str = "flashgrep";

pub(crate) fn log_flashgrep_stderr_line(line: &str) {
    log_flashgrep_stderr_line_with_context(None, line);
}

pub(crate) fn log_flashgrep_stderr_line_with_context(context: Option<&str>, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }

    if let Some(rest) = trimmed.strip_prefix("flashgrep[") {
        if let Some((area, rest)) = rest.split_once("][") {
            if let Some((level, message)) = rest.split_once("] ") {
                let formatted = match context {
                    Some(context) => format!("flashgrep[{area}] {context} {message}"),
                    None => format!("flashgrep[{area}] {message}"),
                };
                match level {
                    "error" => log::error!(target: FLASHGREP_LOG_TARGET, "{formatted}"),
                    "warn" => log::warn!(target: FLASHGREP_LOG_TARGET, "{formatted}"),
                    "info" => log::info!(target: FLASHGREP_LOG_TARGET, "{formatted}"),
                    "debug" => log::debug!(target: FLASHGREP_LOG_TARGET, "{formatted}"),
                    "trace" => log::trace!(target: FLASHGREP_LOG_TARGET, "{formatted}"),
                    _ => log::debug!(target: FLASHGREP_LOG_TARGET, "{trimmed}"),
                }
                return;
            }
        }
    }

    match context {
        Some(context) => log::debug!(target: FLASHGREP_LOG_TARGET, "{context} {trimmed}"),
        None => log::debug!(target: FLASHGREP_LOG_TARGET, "{trimmed}"),
    }
}

pub(crate) use client::{ManagedClient, RepoSession};
pub(crate) use protocol::{
    ClientCapabilities, ClientInfo, FileMatch, GlobParams, InitializeParams, MatchLocation,
    RepoRef, Request, Response, SearchHit, SearchLine, SearchParams, TaskRef,
};
pub(crate) use repo_session::FlashgrepRepoSession;
pub(crate) use rpc_client::{drain_content_length_messages, ProtocolClient};
pub(crate) use types::{
    ConsistencyMode, DirtyFileStats, FileCount, GlobOutcome, GlobRequest, OpenRepoParams,
    PathScope, QuerySpec, RefreshPolicyConfig, RepoConfig, RepoPhase, RepoStatus, SearchBackend,
    SearchModeConfig, SearchOutcome, SearchRequest, SearchResults, TaskKind, TaskPhase, TaskState,
    TaskStatus, WorkspaceOverlayStatus,
};
