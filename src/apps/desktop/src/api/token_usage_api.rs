//! Token usage statistics API (settings "usage statistics" page).

use crate::api::app_state::AppState;
use log::error;
use openbitfun_core::service::token_usage::{TokenUsageStatisticsRequest, UsageStatistics};
use tauri::State;

#[tauri::command]
pub async fn get_token_usage_statistics(
    request: TokenUsageStatisticsRequest,
    state: State<'_, AppState>,
) -> Result<UsageStatistics, String> {
    state
        .token_usage_service
        .get_statistics_for_request(request)
        .await
        .map_err(|e| {
            error!("Failed to load token usage statistics: {}", e);
            format!("Failed to load token usage statistics: {}", e)
        })
}
