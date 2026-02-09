//! Startchat Agent API

use log::error;
use tauri::State;
use bitfun_core::function_agents::{
    StartchatFunctionAgent,
    WorkStateAnalysis,
    WorkStateOptions,
    startchat_func_agent::Language,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::app_state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeWorkStateRequest {
    pub repo_path: String,
    pub options: Option<WorkStateOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAnalyzeRequest {
    pub repo_path: String,
    pub language: Option<Language>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateGreetingRequest {
    pub repo_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkStateSummaryResponse {
    pub greeting_title: String,
    pub current_state_summary: String,
    pub has_git_changes: bool,
    pub unstaged_files: u32,
    pub unpushed_commits: u32,
    pub predicted_actions_count: usize,
}

#[tauri::command]
pub async fn analyze_work_state(
    state: State<'_, AppState>,
    request: AnalyzeWorkStateRequest,
) -> Result<WorkStateAnalysis, String> {
    let agent = StartchatFunctionAgent::new(state.ai_client_factory.clone());
    let opts = request.options.unwrap_or_default();
    
    agent
        .analyze_work_state(Path::new(&request.repo_path), opts)
        .await
        .map_err(|e| {
            error!("Work state analysis failed: repo_path={}, error={}", request.repo_path, e);
            e.to_string()
        })
}

#[tauri::command]
pub async fn quick_analyze_work_state(
    state: State<'_, AppState>,
    request: QuickAnalyzeRequest,
) -> Result<WorkStateAnalysis, String> {
    let agent = StartchatFunctionAgent::new(state.ai_client_factory.clone());
    let language = request.language.unwrap_or(Language::Chinese);
    
    agent
        .quick_analyze(Path::new(&request.repo_path), language)
        .await
        .map_err(|e| {
            error!("Quick work state analysis failed: repo_path={}, error={}", request.repo_path, e);
            e.to_string()
        })
}

#[tauri::command]
pub async fn generate_greeting_only(
    state: State<'_, AppState>,
    request: GenerateGreetingRequest,
) -> Result<WorkStateAnalysis, String> {
    let agent = StartchatFunctionAgent::new(state.ai_client_factory.clone());
    
    agent
        .generate_greeting_only(Path::new(&request.repo_path))
        .await
        .map_err(|e| {
            error!("Generate greeting failed: repo_path={}, error={}", request.repo_path, e);
            e.to_string()
        })
}

#[tauri::command]
pub async fn get_work_state_summary(
    state: State<'_, AppState>,
    request: QuickAnalyzeRequest,
) -> Result<WorkStateSummaryResponse, String> {
    let agent = StartchatFunctionAgent::new(state.ai_client_factory.clone());
    
    let language = request.language.unwrap_or(Language::Chinese);
    
    let analysis = agent
        .quick_analyze(Path::new(&request.repo_path), language)
        .await
        .map_err(|e| {
            error!("Failed to get work state summary: repo_path={}, error={}", request.repo_path, e);
            e.to_string()
        })?;
    
    let (unstaged_files, unpushed_commits, has_git_changes) = if let Some(ref git) = analysis.current_state.git_state {
        (
            git.unstaged_files + git.staged_files,
            git.unpushed_commits,
            git.unstaged_files > 0 || git.staged_files > 0 || git.unpushed_commits > 0
        )
    } else {
        (0, 0, false)
    };
    
    Ok(WorkStateSummaryResponse {
        greeting_title: analysis.greeting.title,
        current_state_summary: analysis.current_state.summary,
        has_git_changes,
        unstaged_files,
        unpushed_commits,
        predicted_actions_count: analysis.predicted_actions.len(),
    })
}
