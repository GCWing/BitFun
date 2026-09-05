//! Compatibility wrapper for token usage persistence.

use super::statistics::UsageAttributionResolver;
use super::types::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageSummary,
};
use crate::infrastructure::PathManager;
use crate::service::config::types::AIModelConfig;
use anyhow::Result;
use openbitfun_services_core::token_usage::{
    TokenUsageStatisticsRequest, UsageGranularity, UsageStatistics, UsageStatisticsFilter,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TOKEN_USAGE_DIR: &str = "token_usage";

pub struct TokenUsageService {
    inner: openbitfun_services_core::token_usage::TokenUsageService,
}

impl TokenUsageService {
    pub async fn new(path_manager: Arc<PathManager>) -> Result<Self> {
        Self::new_in_base_dir(path_manager.user_data_dir().join(TOKEN_USAGE_DIR)).await
    }

    pub async fn new_in_base_dir(base_dir: PathBuf) -> Result<Self> {
        let inner = openbitfun_services_core::token_usage::TokenUsageService::new(base_dir)
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
            openbitfun_services_core::token_usage::aggregate_statistics_with_time_zone(
                &records,
                granularity,
                time_zone.as_deref(),
                |record| resolver.attribute(record),
            ),
        )
    }

    /// Resolve a surface request and aggregate this OpenBitFun host's persisted
    /// usage. The request is intentionally workspace-agnostic: Peer transport
    /// selects the host, while SSH workspace routing does not change it.
    pub async fn get_statistics_for_request(
        &self,
        request: TokenUsageStatisticsRequest,
    ) -> Result<UsageStatistics> {
        let resolved = request.resolve().map_err(anyhow::Error::msg)?;
        self.get_statistics(resolved.query, resolved.granularity, resolved.filter)
            .await
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

/// Install the process-wide token usage service. The canonical Agent Runtime
/// initializer calls this after constructing the service; tools that call the
/// model outside the round executor (e.g. `analyze_image`) use it to persist
/// usage that would otherwise never reach the token usage store.
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
