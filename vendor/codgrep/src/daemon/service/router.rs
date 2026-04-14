use std::{
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    config::QueryConfig,
    error::{AppError, Result},
    path_filter::PathFilterArgs,
    search::SearchMode,
};

use super::DaemonService;
use crate::daemon::{
    convert::{
        build_config_from_open, convert_consistency, convert_query_backend, convert_repo_status,
        convert_search_results, convert_task_status,
    },
    protocol::{
        ClientCapabilities, ConsistencyMode, EnsureRepoParams, ErrorResponse, QuerySpec, Request,
        RequestEnvelope, Response, SearchProtocolCapabilities, ServerCapabilities, ServerInfo,
    },
    repo::{
        GlobRequest, OpenRepoRequest, RepoManager, RepoRefreshPolicy, SearchConsistency,
        SearchRequest,
    },
};

impl DaemonService {
    pub(super) fn handle_envelope(
        &self,
        envelope: RequestEnvelope,
    ) -> Result<(Response, Option<ClientCapabilities>, bool)> {
        if envelope.jsonrpc != "2.0" {
            return Err(AppError::Protocol(format!(
                "unsupported jsonrpc version: {}",
                envelope.jsonrpc
            )));
        }
        self.handle_request(envelope.request)
    }

    #[allow(dead_code)]
    pub(super) fn handle_request_only(&self, request: Request) -> Result<Response> {
        self.handle_request(request)
            .map(|(response, _, _)| response)
    }

    pub(super) fn handle_request(
        &self,
        request: Request,
    ) -> Result<(Response, Option<ClientCapabilities>, bool)> {
        match request {
            Request::Initialize { params } => {
                let caps = params.capabilities.clone();
                Ok((
                    Response::InitializeResult {
                        protocol_version: 1,
                        server_info: ServerInfo {
                            name: "codgrep".into(),
                            version: env!("CARGO_PKG_VERSION").into(),
                        },
                        capabilities: build_server_capabilities(params.capabilities),
                        search: SearchProtocolCapabilities {
                            consistency_modes: vec![
                                ConsistencyMode::SnapshotOnly,
                                ConsistencyMode::WorkspaceEventual,
                                ConsistencyMode::WorkspaceStrict,
                            ],
                            search_modes: vec![
                                super::super::protocol::SearchModeConfig::CountOnly,
                                super::super::protocol::SearchModeConfig::CountMatches,
                                super::super::protocol::SearchModeConfig::FirstHitOnly,
                                super::super::protocol::SearchModeConfig::MaterializeMatches,
                            ],
                        },
                    },
                    Some(caps),
                    false,
                ))
            }
            Request::Initialized => Ok((Response::InitializedAck, None, true)),
            Request::Ping => Ok((
                Response::Pong {
                    now_unix_secs: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                },
                None,
                false,
            )),
            Request::IndexBuild { params } => Ok((
                Response::TaskStarted {
                    task: convert_task_status(self.repos.start_build_index(&params.repo_id)?),
                },
                None,
                false,
            )),
            Request::IndexRebuild { params } => Ok((
                Response::TaskStarted {
                    task: convert_task_status(self.repos.start_rebuild_index(&params.repo_id)?),
                },
                None,
                false,
            )),
            Request::TaskStatus { params } => Ok((
                Response::TaskStatus {
                    task: convert_task_status(self.repos.get_task_status(&params.task_id)?),
                },
                None,
                false,
            )),
            Request::TaskCancel { params } => Ok((
                Response::TaskCancelled {
                    task_id: params.task_id.clone(),
                    accepted: self.repos.cancel_task(&params.task_id)?,
                },
                None,
                false,
            )),
            Request::OpenRepo { params } => {
                let build_config =
                    build_config_from_request(params.repo_path, params.index_path, params.config);
                let status = self.repos.open_repo(OpenRepoRequest {
                    build_config,
                    refresh_policy: RepoRefreshPolicy {
                        rebuild_dirty_threshold: params.refresh.rebuild_dirty_threshold,
                    },
                })?;
                Ok((
                    Response::RepoOpened {
                        repo_id: status.repo_id.clone(),
                        status: convert_repo_status(status),
                    },
                    None,
                    false,
                ))
            }
            Request::EnsureRepo { params } => self.ensure_repo(params),
            Request::GetRepoStatus { params } => Ok((
                Response::RepoStatus {
                    status: convert_repo_status(self.repos.get_status(&params.repo_id)?),
                },
                None,
                false,
            )),
            Request::RefreshRepo { params } => Ok((
                Response::RepoStatus {
                    status: convert_repo_status(
                        self.repos.refresh_repo(&params.repo_id, params.force)?,
                    ),
                },
                None,
                false,
            )),
            Request::BuildIndex { params } => {
                let (indexed_docs, status) = self.repos.build_index(&params.repo_id)?;
                Ok((
                    Response::RepoBuilt {
                        indexed_docs,
                        status: convert_repo_status(status),
                    },
                    None,
                    false,
                ))
            }
            Request::RebuildIndex { params } => {
                let (indexed_docs, status) = self.repos.rebuild_index(&params.repo_id)?;
                Ok((
                    Response::RepoRebuilt {
                        indexed_docs,
                        status: convert_repo_status(status),
                    },
                    None,
                    false,
                ))
            }
            Request::Search { params } => {
                let query = convert_query_spec(params.query)?;
                let scope = PathFilterArgs {
                    roots: normalize_scope_roots(params.scope.roots),
                    globs: params.scope.globs,
                    iglobs: params.scope.iglobs,
                    type_add: params.scope.type_add,
                    type_clear: params.scope.type_clear,
                    types: params.scope.types,
                    type_not: params.scope.type_not,
                };
                let response = self.repos.search(SearchRequest {
                    repo_id: params.repo_id,
                    query,
                    path_filter_args: scope,
                    consistency: convert_consistency_mode(params.consistency),
                    allow_scan_fallback: params.allow_scan_fallback,
                })?;
                Ok((
                    Response::SearchCompleted {
                        repo_id: response.repo_id,
                        backend: convert_query_backend(response.backend),
                        consistency_applied: convert_consistency(response.consistency_applied),
                        status: convert_repo_status(response.status),
                        results: convert_search_results(response.results),
                    },
                    None,
                    false,
                ))
            }
            Request::Glob { params } => {
                let scope = PathFilterArgs {
                    roots: normalize_scope_roots(params.scope.roots),
                    globs: params.scope.globs,
                    iglobs: params.scope.iglobs,
                    type_add: params.scope.type_add,
                    type_clear: params.scope.type_clear,
                    types: params.scope.types,
                    type_not: params.scope.type_not,
                };
                let response = self.repos.glob(GlobRequest {
                    repo_id: params.repo_id,
                    path_filter_args: scope,
                })?;
                Ok((
                    Response::GlobCompleted {
                        repo_id: response.repo_id,
                        status: convert_repo_status(response.status),
                        paths: response.paths,
                    },
                    None,
                    false,
                ))
            }
            Request::CloseRepo { params } => {
                self.repos.close_repo(&params.repo_id)?;
                Ok((
                    Response::RepoClosed {
                        repo_id: params.repo_id,
                    },
                    None,
                    false,
                ))
            }
            Request::Shutdown => Ok((Response::ShutdownAck, None, false)),
            Request::Exit => Ok((Response::ExitAck, None, false)),
        }
    }

    fn ensure_repo(
        &self,
        params: EnsureRepoParams,
    ) -> Result<(Response, Option<ClientCapabilities>, bool)> {
        let build_config =
            build_config_from_request(params.repo_path, params.index_path, params.config);
        let repo_id = build_config
            .normalized()?
            .repo_path
            .to_string_lossy()
            .into_owned();
        let mut status = self.repos.open_repo(OpenRepoRequest {
            build_config,
            refresh_policy: RepoRefreshPolicy {
                rebuild_dirty_threshold: params.refresh.rebuild_dirty_threshold,
            },
        })?;
        if matches!(status.phase, crate::daemon::repo::RepoPhase::Opening) {
            status = wait_for_repo_open(&self.repos, &repo_id)?;
        }

        let (indexed_docs, status) = match status.phase {
            crate::daemon::repo::RepoPhase::MissingIndex => {
                let (indexed_docs, status) = self.repos.build_index(&repo_id)?;
                (Some(indexed_docs), status)
            }
            _ => (None, status),
        };

        Ok((
            Response::RepoEnsured {
                repo_id,
                status: convert_repo_status(status),
                indexed_docs,
            },
            None,
            false,
        ))
    }
}

fn wait_for_repo_open(
    repos: &RepoManager,
    repo_id: &str,
) -> Result<crate::daemon::repo::RepoStatus> {
    for _ in 0..400 {
        let status = repos.get_status(repo_id)?;
        if !matches!(status.phase, crate::daemon::repo::RepoPhase::Opening) {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(AppError::Protocol(format!(
        "timed out waiting for repo {repo_id} to finish opening"
    )))
}

fn build_config_from_request(
    repo_path: PathBuf,
    index_path: Option<PathBuf>,
    config: super::super::protocol::RepoConfig,
) -> crate::config::BuildConfig {
    build_config_from_open(
        repo_path,
        index_path,
        config.tokenizer,
        config.corpus_mode,
        config.include_hidden,
        config.max_file_size,
        config.min_sparse_len,
        config.max_sparse_len,
    )
}

fn convert_query_spec(query: QuerySpec) -> Result<QueryConfig> {
    if query.pattern.trim().is_empty() {
        return Err(AppError::InvalidPattern(
            "query pattern cannot be empty".into(),
        ));
    }
    let patterns = if query.patterns.is_empty() {
        vec![query.pattern.clone()]
    } else {
        query.patterns
    };
    Ok(QueryConfig {
        regex_pattern: query.pattern,
        patterns,
        case_insensitive: query.case_insensitive,
        multiline: query.multiline,
        dot_matches_new_line: query.dot_matches_new_line,
        fixed_strings: query.fixed_strings,
        word_regexp: query.word_regexp,
        line_regexp: query.line_regexp,
        before_context: query.before_context,
        after_context: query.after_context,
        top_k_tokens: query.top_k_tokens,
        max_count: query.max_count,
        global_max_results: query.global_max_results,
        search_mode: match query.search_mode {
            super::super::protocol::SearchModeConfig::CountOnly => SearchMode::CountOnly,
            super::super::protocol::SearchModeConfig::CountMatches => SearchMode::CountMatches,
            super::super::protocol::SearchModeConfig::FirstHitOnly => SearchMode::FirstHitOnly,
            super::super::protocol::SearchModeConfig::MaterializeMatches => {
                SearchMode::MaterializeMatches
            }
        },
    })
}

fn normalize_scope_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots
}

fn build_server_capabilities(_client: ClientCapabilities) -> ServerCapabilities {
    ServerCapabilities {
        workspace_open: true,
        workspace_ensure: true,
        workspace_list: false,
        workspace_refresh: true,
        index_build: true,
        index_rebuild: true,
        task_status: true,
        task_cancel: true,
        search_query: true,
        glob_query: true,
        progress_notifications: true,
        status_notifications: true,
    }
}

pub(super) fn protocol_error(error: AppError) -> ErrorResponse {
    ErrorResponse {
        code: -32001,
        message: error.to_string(),
        data: None,
    }
}

fn convert_consistency_mode(mode: ConsistencyMode) -> SearchConsistency {
    match mode {
        ConsistencyMode::SnapshotOnly => SearchConsistency::SnapshotOnly,
        ConsistencyMode::WorkspaceEventual => SearchConsistency::WorkspaceEventual,
        ConsistencyMode::WorkspaceStrict => SearchConsistency::WorkspaceStrict,
    }
}
