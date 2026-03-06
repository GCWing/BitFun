//! Token usage tracking API

use crate::api::app_state::AppState;
use bitfun_core::service::token_usage::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageSummary,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct RecordTokenUsageRequest {
    pub model_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cached_tokens: u32,
    #[serde(default)]
    pub is_subagent: bool,
}

#[derive(Debug, Deserialize)]
pub struct GetModelStatsRequest {
    pub model_id: String,
    pub time_range: Option<TimeRange>,
    #[serde(default)]
    pub include_subagent: bool,
}

#[derive(Debug, Deserialize)]
pub struct GetSessionStatsRequest {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryTokenUsageRequest {
    pub model_id: Option<String>,
    pub session_id: Option<String>,
    pub time_range: TimeRange,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    #[serde(default)]
    pub include_subagent: bool,
}

#[derive(Debug, Deserialize)]
pub struct ClearModelStatsRequest {
    pub model_id: String,
}

#[derive(Debug, Serialize)]
pub struct GetAllModelStatsResponse {
    pub stats: HashMap<String, ModelTokenStats>,
}

/// Record token usage for a specific turn
#[tauri::command]
pub async fn record_token_usage(
    state: State<'_, AppState>,
    request: RecordTokenUsageRequest,
) -> Result<(), String> {
    debug!(
        "Recording token usage: model={}, session={}, input={}, output={}",
        request.model_id, request.session_id, request.input_tokens, request.output_tokens
    );

    state
        .token_usage_service
        .record_usage(
            request.model_id,
            request.session_id,
            request.turn_id,
            request.input_tokens,
            request.output_tokens,
            request.cached_tokens,
            request.is_subagent,
        )
        .await
        .map_err(|e| {
            error!("Failed to record token usage: {}", e);
            format!("Failed to record token usage: {}", e)
        })
}

/// Get token statistics for a specific model
#[tauri::command]
pub async fn get_model_token_stats(
    state: State<'_, AppState>,
    request: GetModelStatsRequest,
) -> Result<Option<ModelTokenStats>, String> {
    debug!("Getting token stats for model: {}", request.model_id);

    match request.time_range {
        Some(time_range) => {
            state
                .token_usage_service
                .get_model_stats_filtered(&request.model_id, time_range, request.include_subagent)
                .await
                .map_err(|e| {
                    error!("Failed to get filtered model stats: {}", e);
                    format!("Failed to get filtered model stats: {}", e)
                })
        }
        None => {
            Ok(state
                .token_usage_service
                .get_model_stats(&request.model_id)
                .await)
        }
    }
}

/// Get token statistics for all models
#[tauri::command]
pub async fn get_all_model_token_stats(
    state: State<'_, AppState>,
) -> Result<GetAllModelStatsResponse, String> {
    debug!("Getting token stats for all models");

    let stats = state.token_usage_service.get_all_model_stats().await;

    Ok(GetAllModelStatsResponse { stats })
}

/// Get token statistics for a specific session
#[tauri::command]
pub async fn get_session_token_stats(
    state: State<'_, AppState>,
    request: GetSessionStatsRequest,
) -> Result<Option<SessionTokenStats>, String> {
    debug!("Getting token stats for session: {}", request.session_id);

    Ok(state
        .token_usage_service
        .get_session_stats(&request.session_id)
        .await)
}

/// Query token usage records with filters
#[tauri::command]
pub async fn query_token_usage(
    state: State<'_, AppState>,
    request: QueryTokenUsageRequest,
) -> Result<TokenUsageSummary, String> {
    debug!("Querying token usage with filters: {:?}", request);

    let query = TokenUsageQuery {
        model_id: request.model_id,
        session_id: request.session_id,
        time_range: request.time_range,
        limit: request.limit,
        offset: request.offset,
        include_subagent: request.include_subagent,
    };

    state
        .token_usage_service
        .get_summary(query)
        .await
        .map_err(|e| {
            error!("Failed to query token usage: {}", e);
            format!("Failed to query token usage: {}", e)
        })
}

/// Clear token statistics for a specific model
#[tauri::command]
pub async fn clear_model_token_stats(
    state: State<'_, AppState>,
    request: ClearModelStatsRequest,
) -> Result<(), String> {
    info!("Clearing token stats for model: {}", request.model_id);

    state
        .token_usage_service
        .clear_model_stats(&request.model_id)
        .await
        .map_err(|e| {
            error!("Failed to clear model stats: {}", e);
            format!("Failed to clear model stats: {}", e)
        })
}

/// Clear all token statistics
#[tauri::command]
pub async fn clear_all_token_stats(state: State<'_, AppState>) -> Result<(), String> {
    info!("Clearing all token statistics");

    state
        .token_usage_service
        .clear_all_stats()
        .await
        .map_err(|e| {
            error!("Failed to clear all stats: {}", e);
            format!("Failed to clear all stats: {}", e)
        })
}

