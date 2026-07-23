//! Git Agent API - Provides Tauri command interface for Git Function Agent

use crate::api::app_state::AppState;
use bitfun_core::function_agents::{CommitMessage, CommitMessageOptions, GitFunctionAgent};
use log::error;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateCommitMessageRequest {
    pub repo_path: String,
    pub options: Option<CommitMessageOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickCommitMessageRequest {
    pub repo_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCommitMessageRequest {
    pub repo_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCommitMessageResponse {
    pub title: String,
    pub commit_type: String,
    pub scope: Option<String>,
    pub confidence: f32,
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
}

#[tauri::command]
pub async fn generate_commit_message(
    app_state: State<'_, AppState>,
    request: GenerateCommitMessageRequest,
) -> Result<CommitMessage, String> {
    let factory = app_state.ai_client_factory.clone();
    let agent = GitFunctionAgent::new(factory);
    let opts = request.options.unwrap_or_default();

    agent
        .generate_commit_message(Path::new(&request.repo_path), opts)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn quick_commit_message(
    app_state: State<'_, AppState>,
    request: QuickCommitMessageRequest,
) -> Result<CommitMessage, String> {
    let factory = app_state.ai_client_factory.clone();
    let agent = GitFunctionAgent::new(factory);

    agent
        .quick_commit_message(Path::new(&request.repo_path))
        .await
        .map_err(|e| {
            error!(
                "Failed to generate quick commit message: repo_path={}, error={}",
                request.repo_path, e
            );
            e.to_string()
        })
}

#[tauri::command]
pub async fn preview_commit_message(
    app_state: State<'_, AppState>,
    request: PreviewCommitMessageRequest,
) -> Result<PreviewCommitMessageResponse, String> {
    let factory = app_state.ai_client_factory.clone();
    let agent = GitFunctionAgent::new(factory);

    let message = agent
        .quick_commit_message(Path::new(&request.repo_path))
        .await
        .map_err(|e| e.to_string())?;

    Ok(PreviewCommitMessageResponse {
        title: message.title,
        commit_type: format!("{:?}", message.commit_type),
        scope: message.scope,
        confidence: message.confidence,
        files_changed: message.changes_summary.files_changed,
        additions: message.changes_summary.total_additions,
        deletions: message.changes_summary.total_deletions,
    })
}
