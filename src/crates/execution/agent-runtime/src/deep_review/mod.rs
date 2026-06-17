//! Deep Review provider-neutral runtime policy and state owners.
//!
//! Concrete workflow execution, AI calls, report delivery, and host-side tool
//! side effects remain in product assembly/core adapters until equivalence is
//! separately protected.

pub mod budget;
pub mod concurrency_policy;
pub mod constants;
pub mod diagnostics;
pub mod execution_policy;
pub mod incremental_cache;
pub mod manifest;
pub mod queue;
pub mod report;
mod runtime_state;
pub mod shared_context;
pub mod task_execution;
pub mod team_definition;
pub mod tool_context;

pub use budget::{DeepReviewActiveReviewerGuard, DeepReviewBudgetTracker};
pub use concurrency_policy::{DeepReviewConcurrencyPolicy, DeepReviewEffectiveConcurrencySnapshot};
pub use constants::{
    CONDITIONAL_REVIEWER_AGENT_TYPES, CORE_REVIEWER_AGENT_TYPES, DEEP_REVIEW_AGENT_TYPE,
    REVIEWER_ARCHITECTURE_AGENT_TYPE, REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_FRONTEND_AGENT_TYPE, REVIEWER_PERFORMANCE_AGENT_TYPE, REVIEWER_SECURITY_AGENT_TYPE,
    REVIEW_FIXER_AGENT_TYPE, REVIEW_JUDGE_AGENT_TYPE,
};
pub use diagnostics::DeepReviewRuntimeDiagnostics;
pub use execution_policy::{
    ChangeRiskFactors, DeepReviewExecutionPolicy, DeepReviewPolicyViolation,
    DeepReviewStrategyLevel, DeepReviewSubagentRole,
};
pub use incremental_cache::DeepReviewIncrementalCache;
pub use manifest::DeepReviewRunManifestGate;
pub use queue::{
    classify_deep_review_capacity_error, DeepReviewCapacityFailFastReason,
    DeepReviewCapacityQueueDecision, DeepReviewCapacityQueueReason, DeepReviewQueueControlAction,
    DeepReviewQueueControlSnapshot, DeepReviewReviewerQueueState, DeepReviewReviewerQueueStatus,
};
pub use report::DeepReviewCacheUpdate;
pub use runtime_state::*;
pub use shared_context::{
    DeepReviewSharedContextDuplicate, DeepReviewSharedContextMeasurementSnapshot,
};
pub use task_execution::{DeepReviewLaunchBatchInfo, DeepReviewQueueWaitSkipReason};
pub use team_definition::{
    default_review_team_definition, ReviewStrategyManifestProfile, ReviewTeamDefinition,
    ReviewTeamExecutionPolicyDefinition, ReviewTeamRoleDefinition,
};
pub use tool_context::{append_tool_use_context_data, DeepReviewToolParentContext};
