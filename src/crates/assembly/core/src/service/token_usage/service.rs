//! Compatibility wrapper for token usage persistence.

use super::types::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageSummary,
};
use crate::infrastructure::PathManager;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TOKEN_USAGE_DIR: &str = "token_usage";

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
        model_id: String,
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
                model_id,
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
