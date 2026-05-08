use crate::api::app_state::AppState;
use bitfun_core::infrastructure::{FileSearchResult, FileSearchResultGroup, SearchMatchType};
use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use bitfun_core::service::search::{
    workspace_search_daemon_available, workspace_search_feature_enabled, ContentSearchResult,
    WorkspaceSearchBackend, WorkspaceSearchRepoPhase,
};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRepoIndexRequest {
    pub root_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMetadataResponse {
    pub backend: WorkspaceSearchBackend,
    pub repo_phase: WorkspaceSearchRepoPhase,
    pub rebuild_recommended: bool,
    pub candidate_docs: usize,
    pub matched_lines: usize,
    pub matched_occurrences: usize,
}

async fn workspace_search_unavailable_message(root_path: &str) -> Option<String> {
    if is_remote_path(root_path.trim()).await {
        return Some(
            "Remote workspace search status is not managed by BitFun workspace search".to_string(),
        );
    }

    if !workspace_search_feature_enabled().await {
        return Some(
            "Workspace search is disabled. Enable it in Settings > Session Config to use accelerated workspace search.".to_string(),
        );
    }

    if !workspace_search_daemon_available() {
        return Some(
            "Workspace search daemon is unavailable. BitFun will continue using legacy search."
                .to_string(),
        );
    }

    None
}

pub(crate) async fn should_use_workspace_search(root_path: &str) -> bool {
    workspace_search_unavailable_message(root_path).await.is_none()
}

pub(crate) async fn search_file_contents_via_workspace_search(
    state: &State<'_, AppState>,
    root_path: &str,
    pattern: &str,
    case_sensitive: bool,
    use_regex: bool,
    whole_word: bool,
    max_results: usize,
) -> Result<bitfun_core::service::search::ContentSearchResult, String> {
    state
        .workspace_search_service
        .search_content(bitfun_core::service::search::ContentSearchRequest {
            repo_root: root_path.into(),
            search_path: None,
            pattern: pattern.to_string(),
            output_mode: bitfun_core::service::search::ContentSearchOutputMode::Content,
            case_sensitive,
            use_regex,
            whole_word,
            multiline: false,
            before_context: 0,
            after_context: 0,
            max_results: Some(max_results),
            globs: Vec::new(),
            file_types: Vec::new(),
            exclude_file_types: Vec::new(),
        })
        .await
        .map_err(|error| {
            format!(
                "Failed to search file contents via workspace search: {}",
                error
            )
        })
}

pub(crate) fn group_search_results(results: Vec<FileSearchResult>) -> Vec<FileSearchResultGroup> {
    let mut grouped = Vec::<FileSearchResultGroup>::new();
    let mut positions = std::collections::HashMap::<String, usize>::new();

    for result in results {
        let path = result.path.clone();
        let position = if let Some(position) = positions.get(&path).copied() {
            position
        } else {
            let position = grouped.len();
            positions.insert(path.clone(), position);
            grouped.push(FileSearchResultGroup {
                path,
                name: result.name.clone(),
                is_directory: result.is_directory,
                file_name_match: None,
                content_matches: Vec::new(),
            });
            position
        };
        let group = &mut grouped[position];

        match result.match_type {
            SearchMatchType::FileName => group.file_name_match = Some(result),
            SearchMatchType::Content => group.content_matches.push(result),
        }
    }

    grouped
}

pub(crate) fn search_metadata_from_content_result(
    result: &ContentSearchResult,
) -> SearchMetadataResponse {
    SearchMetadataResponse {
        backend: result.backend,
        repo_phase: result.repo_status.phase,
        rebuild_recommended: result.repo_status.rebuild_recommended,
        candidate_docs: result.candidate_docs,
        matched_lines: result.matched_lines,
        matched_occurrences: result.matched_occurrences,
    }
}

#[tauri::command]
pub async fn search_get_repo_status(
    state: State<'_, AppState>,
    request: SearchRepoIndexRequest,
) -> Result<serde_json::Value, String> {
    if let Some(message) = workspace_search_unavailable_message(&request.root_path).await {
        return Err(message);
    }

    state
        .workspace_search_service
        .get_index_status(&request.root_path)
        .await
        .map(|status| serde_json::to_value(status).unwrap_or_else(|_| serde_json::json!({})))
        .map_err(|error| format!("Failed to get search repository status: {}", error))
}

#[tauri::command]
pub async fn search_build_index(
    state: State<'_, AppState>,
    request: SearchRepoIndexRequest,
) -> Result<serde_json::Value, String> {
    if let Some(message) = workspace_search_unavailable_message(&request.root_path).await {
        return Err(message);
    }

    state
        .workspace_search_service
        .build_index(&request.root_path)
        .await
        .map(|task| serde_json::to_value(task).unwrap_or_else(|_| serde_json::json!({})))
        .map_err(|error| format!("Failed to build workspace index: {}", error))
}

#[tauri::command]
pub async fn search_rebuild_index(
    state: State<'_, AppState>,
    request: SearchRepoIndexRequest,
) -> Result<serde_json::Value, String> {
    if let Some(message) = workspace_search_unavailable_message(&request.root_path).await {
        return Err(message);
    }

    state
        .workspace_search_service
        .rebuild_index(&request.root_path)
        .await
        .map(|task| serde_json::to_value(task).unwrap_or_else(|_| serde_json::json!({})))
        .map_err(|error| format!("Failed to rebuild workspace index: {}", error))
}
