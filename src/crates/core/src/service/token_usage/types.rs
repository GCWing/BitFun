//! Token usage data types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Single token usage record for a specific API call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageRecord {
    pub model_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub timestamp: DateTime<Utc>,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cached_tokens: u32,
    pub total_tokens: u32,
    /// Whether this record is from a subagent call
    #[serde(default)]
    pub is_subagent: bool,
}

/// Aggregated token statistics for a model
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelTokenStats {
    pub model_id: String,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cached: u64,
    pub total_tokens: u64,
    /// Number of distinct sessions that used this model
    pub session_count: u32,
    /// Number of API requests made with this model
    pub request_count: u32,
    /// Set of session IDs that used this model (for dedup counting)
    #[serde(default)]
    pub session_ids: HashSet<String>,
    pub first_used: Option<DateTime<Utc>>,
    pub last_used: Option<DateTime<Utc>>,
}

/// Token statistics for a specific session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokenStats {
    pub session_id: String,
    pub model_id: String,
    pub total_input: u32,
    pub total_output: u32,
    pub total_cached: u32,
    pub total_tokens: u32,
    pub request_count: u32,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

/// Time range for querying statistics
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimeRange {
    Today,
    ThisWeek,
    ThisMonth,
    All,
    Custom {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}

/// Query parameters for token usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageQuery {
    pub model_id: Option<String>,
    pub session_id: Option<String>,
    pub time_range: TimeRange,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Whether to include subagent token usage in results (default: false)
    #[serde(default)]
    pub include_subagent: bool,
}

/// Summary of token usage with breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageSummary {
    pub total_input: u64,
    pub total_output: u64,
    pub total_cached: u64,
    pub total_tokens: u64,
    pub by_model: HashMap<String, ModelTokenStats>,
    pub by_session: HashMap<String, SessionTokenStats>,
    pub record_count: usize,
}
