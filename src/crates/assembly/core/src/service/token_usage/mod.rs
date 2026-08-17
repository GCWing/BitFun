//! Token usage tracking service
//!
//! Tracks and persists token consumption statistics per model, session, and turn.

mod service;
mod statistics;
mod subscriber;

pub use bitfun_services_core::token_usage::types;
pub use bitfun_services_core::token_usage::{
    aggregate_statistics, ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery,
    TokenUsageRecord, TokenUsageSummary, UsageAttribution, UsageAttributionStatus,
    UsageDimensionAttribution, UsageGranularity, UsageStatistics, UsageStatisticsEntry,
    UsageTrendPoint,
};
pub use service::{
    get_global_token_usage_service, set_global_token_usage_service, TokenUsageService,
};
pub use statistics::UsageAttributionResolver;
pub use subscriber::TokenUsageSubscriber;
