//! Compatibility wrapper for token usage persistence.

use super::statistics::UsageAttributionResolver;
use super::types::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageSummary,
};
use crate::infrastructure::PathManager;
use crate::service::config::types::AIModelConfig;
use anyhow::Result;
use bitfun_services_core::token_usage::{UsageGranularity, UsageStatistics, UsageStatisticsFilter};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TOKEN_USAGE_DIR: &str = "token_usage";

/// Surface-neutral request for the settings usage dashboard.
///
/// Desktop Tauri, Server RPC, and Peer Host adapters deserialize the same
/// request and delegate here so range and filter semantics stay identical.
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    /// IANA time zone used for local-calendar ranges and trend buckets.
    #[serde(default)]
    pub time_zone: Option<String>,
    #[serde(default)]
    pub include_subagent: bool,
    #[serde(default)]
    pub filter_kind: bitfun_services_core::token_usage::UsageStatisticsFilterKind,
    #[serde(default)]
    pub filter_query: Option<String>,
}

fn resolve_statistics_time_range(
    request: &TokenUsageStatisticsRequest,
) -> std::result::Result<TimeRange, String> {
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

fn resolve_statistics_filter(
    request: &TokenUsageStatisticsRequest,
) -> Option<UsageStatisticsFilter> {
    request
        .filter_query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(|query| UsageStatisticsFilter {
            kind: request.filter_kind,
            query: query.to_string(),
        })
}

pub struct TokenUsageService {
    inner: bitfun_services_core::token_usage::TokenUsageService,
}

impl TokenUsageService {
    pub async fn new(path_manager: Arc<PathManager>) -> Result<Self> {
        Self::new_in_base_dir(path_manager.user_data_dir().join(TOKEN_USAGE_DIR)).await
    }

    pub async fn new_in_base_dir(base_dir: PathBuf) -> Result<Self> {
        let inner = bitfun_services_core::token_usage::TokenUsageService::new(base_dir)
            .await
            .map_err(anyhow::Error::msg)?;
        Ok(Self { inner })
    }

    pub fn base_dir(&self) -> &Path {
        self.inner.base_dir()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_usage(
        &self,
        model_config_id: String,
        effective_model_name: String,
        session_id: String,
        turn_id: String,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: Option<u32>,
        token_details: Option<serde_json::Value>,
        is_subagent: bool,
    ) -> Result<()> {
        self.inner
            .record_usage(
                model_config_id,
                effective_model_name,
                session_id,
                turn_id,
                input_tokens,
                output_tokens,
                cached_tokens,
                token_details,
                is_subagent,
            )
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn get_model_stats(&self, model_id: &str) -> Option<ModelTokenStats> {
        self.inner.get_model_stats(model_id).await
    }

    pub async fn get_model_stats_filtered(
        &self,
        model_id: &str,
        time_range: TimeRange,
        include_subagent: bool,
    ) -> Result<Option<ModelTokenStats>> {
        self.inner
            .get_model_stats_filtered(model_id, time_range, include_subagent)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn get_all_model_stats(&self) -> HashMap<String, ModelTokenStats> {
        self.inner.get_all_model_stats().await
    }

    pub async fn get_session_stats(&self, session_id: &str) -> Option<SessionTokenStats> {
        self.inner.get_session_stats(session_id).await
    }

    pub async fn query_records(&self, query: TokenUsageQuery) -> Result<Vec<TokenUsageRecord>> {
        self.inner
            .query_records(query)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub(crate) async fn query_records_for_sessions(
        &self,
        query: TokenUsageQuery,
        session_ids: &HashSet<String>,
    ) -> Result<Vec<TokenUsageRecord>> {
        self.inner
            .query_records_for_sessions(query, session_ids)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn get_summary(&self, query: TokenUsageQuery) -> Result<TokenUsageSummary> {
        self.inner
            .get_summary(query)
            .await
            .map_err(anyhow::Error::msg)
    }

    /// Aggregate persisted records into dashboard statistics.
    ///
    /// Attribution resolves the current model configuration for each record's
    /// `model_config_id`. Records whose configuration was deleted remain in
    /// isolated unresolved buckets rather than being guessed by model name.
    pub async fn get_statistics(
        &self,
        query: TokenUsageQuery,
        granularity: UsageGranularity,
        filter: Option<UsageStatisticsFilter>,
    ) -> Result<UsageStatistics> {
        let time_zone = query.time_zone.clone();
        let mut records = self
            .inner
            .query_records(query)
            .await
            .map_err(anyhow::Error::msg)?;

        let configs = crate::service::config::get_global_config_service()
            .await
            .map_err(anyhow::Error::msg)?
            .get_config::<Vec<AIModelConfig>>(Some("ai.models"))
            .await
            .unwrap_or_default();
        let resolver = UsageAttributionResolver::new(&configs);
        if let Some(filter) = filter {
            let normalized_query = filter.query.trim().to_lowercase();
            if !normalized_query.is_empty() {
                records.retain(|record| {
                    resolver.matches_filter(record, filter.kind, &normalized_query)
                });
            }
        }

        Ok(
            bitfun_services_core::token_usage::aggregate_statistics_with_time_zone(
                &records,
                granularity,
                time_zone.as_deref(),
                |record| resolver.attribute(record),
            ),
        )
    }

    /// Resolve a surface request and aggregate this BitFun host's persisted
    /// usage. The request is intentionally workspace-agnostic: Peer transport
    /// selects the host, while SSH workspace routing does not change it.
    pub async fn get_statistics_for_request(
        &self,
        request: TokenUsageStatisticsRequest,
    ) -> Result<UsageStatistics> {
        let time_range = resolve_statistics_time_range(&request).map_err(anyhow::Error::msg)?;
        let granularity = match request.granularity.as_str() {
            "day" => UsageGranularity::Day,
            _ => UsageGranularity::Hour,
        };
        let filter = resolve_statistics_filter(&request);
        let query = TokenUsageQuery {
            model_id: None,
            session_id: None,
            time_range,
            time_zone: request.time_zone,
            limit: None,
            offset: None,
            include_subagent: request.include_subagent,
        };

        self.get_statistics(query, granularity, filter).await
    }

    pub async fn clear_model_stats(&self, model_id: &str) -> Result<()> {
        self.inner
            .clear_model_stats(model_id)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn clear_all_stats(&self) -> Result<()> {
        self.inner
            .clear_all_stats()
            .await
            .map_err(anyhow::Error::msg)
    }
}

static GLOBAL_TOKEN_USAGE_SERVICE: std::sync::OnceLock<Arc<TokenUsageService>> =
    std::sync::OnceLock::new();

/// Install the process-wide token usage service. Called once by the desktop
/// runtime after the service is constructed; tools that call the model outside
/// the round executor (e.g. `analyze_image`) use it to persist usage that would
/// otherwise never reach the token usage store.
pub fn set_global_token_usage_service(service: Arc<TokenUsageService>) {
    match GLOBAL_TOKEN_USAGE_SERVICE.set(service) {
        Ok(_) => log::info!("Global token usage service set"),
        Err(_) => log::info!("Global token usage service already exists, skipping set"),
    }
}

/// Access the process-wide token usage service, if installed.
pub fn get_global_token_usage_service() -> Option<Arc<TokenUsageService>> {
    GLOBAL_TOKEN_USAGE_SERVICE.get().cloned()
}

#[cfg(test)]
mod request_tests {
    use super::*;
    use bitfun_services_core::token_usage::UsageStatisticsFilterKind;

    #[test]
    fn legacy_request_without_filter_fields_defaults_to_unfiltered() {
        let request: TokenUsageStatisticsRequest = serde_json::from_value(serde_json::json!({
            "timeRange": "today",
            "granularity": "hour"
        }))
        .expect("request");

        assert_eq!(request.filter_kind, UsageStatisticsFilterKind::All);
        assert_eq!(resolve_statistics_filter(&request), None);
    }

    #[test]
    fn filter_fields_deserialize_and_trim_query() {
        let request: TokenUsageStatisticsRequest = serde_json::from_value(serde_json::json!({
            "timeRange": "today",
            "granularity": "hour",
            "filterKind": "provider",
            "filterQuery": "  DeepSeek  "
        }))
        .expect("request");

        assert_eq!(
            resolve_statistics_filter(&request),
            Some(UsageStatisticsFilter {
                kind: UsageStatisticsFilterKind::Provider,
                query: "DeepSeek".to_string(),
            })
        );
    }

    #[test]
    fn custom_range_rejects_non_increasing_bounds() {
        let request: TokenUsageStatisticsRequest = serde_json::from_value(serde_json::json!({
            "timeRange": "custom",
            "granularity": "day",
            "start": "2026-08-17T12:00:00Z",
            "end": "2026-08-17T12:00:00Z"
        }))
        .expect("request");

        assert_eq!(
            resolve_statistics_time_range(&request).unwrap_err(),
            "custom time range end must be after start"
        );
    }
}
