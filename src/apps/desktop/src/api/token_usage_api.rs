//! Token usage statistics API (settings "usage statistics" page).

use crate::api::app_state::AppState;
use bitfun_core::service::token_usage::{
    types::{TimeRange, TokenUsageQuery},
    UsageGranularity, UsageStatistics,
};
use chrono::{DateTime, Duration, Utc};
use log::error;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageStatisticsRequest {
    /// One of "last24Hours" | "today" | "thisWeek" | "thisMonth" | "all" | "custom".
    pub time_range: String,
    /// One of "hour" | "day".
    pub granularity: String,
    #[serde(default)]
    pub start: Option<DateTime<Utc>>,
    #[serde(default)]
    pub end: Option<DateTime<Utc>>,
    #[serde(default)]
    pub include_subagent: bool,
}

fn resolve_time_range(request: &TokenUsageStatisticsRequest) -> Result<TimeRange, String> {
    match request.time_range.as_str() {
        "today" => Ok(TimeRange::Today),
        "thisWeek" => Ok(TimeRange::ThisWeek),
        "thisMonth" => Ok(TimeRange::ThisMonth),
        "all" => Ok(TimeRange::All),
        "custom" => {
            let start = request
                .start
                .ok_or_else(|| "custom time range requires a start timestamp".to_string())?;
            let end = request.end.unwrap_or_else(Utc::now);
            if end <= start {
                return Err("custom time range end must be after start".to_string());
            }
            Ok(TimeRange::Custom { start, end })
        }
        _ => {
            // Default and "last24Hours": the trailing 24 hours.
            let end = Utc::now();
            Ok(TimeRange::Custom {
                start: end - Duration::hours(24),
                end,
            })
        }
    }
}

#[tauri::command]
pub async fn get_token_usage_statistics(
    request: TokenUsageStatisticsRequest,
    state: State<'_, AppState>,
) -> Result<UsageStatistics, String> {
    let time_range = resolve_time_range(&request)?;
    let granularity = match request.granularity.as_str() {
        "day" => UsageGranularity::Day,
        _ => UsageGranularity::Hour,
    };
    let query = TokenUsageQuery {
        model_id: None,
        session_id: None,
        time_range,
        limit: None,
        offset: None,
        include_subagent: request.include_subagent,
    };

    state
        .token_usage_service
        .get_statistics(query, granularity)
        .await
        .map_err(|e| {
            error!("Failed to load token usage statistics: {}", e);
            format!("Failed to load token usage statistics: {}", e)
        })
}
