//! Token usage tracking service
//!
//! Tracks and persists token consumption statistics per model, session, and turn.

mod service;
mod subscriber;

pub use bitfun_services_core::token_usage::types;
pub use bitfun_services_core::token_usage::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageSummary,
};
pub use service::TokenUsageService;
pub use subscriber::TokenUsageSubscriber;
