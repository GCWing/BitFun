//! Deep Review core compatibility facade.
//!
//! Runtime-owned policy, budget, queue, manifest, and shared-context state live
//! in `bitfun-agent-runtime::deep_review`. Core only keeps product config
//! loading here so existing callers keep their paths while ownership moves down.

use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::warn;
use serde_json::Value;

pub use bitfun_agent_runtime::deep_review::{
    apply_deep_review_queue_control, classify_deep_review_capacity_error,
    clear_deep_review_queue_control_for_tool, deep_review_active_reviewer_count,
    deep_review_capacity_skip_count, deep_review_concurrency_cap_rejection_count,
    deep_review_effective_concurrency_snapshot, deep_review_effective_parallel_instances,
    deep_review_has_judge_been_launched, deep_review_max_retries_per_role,
    deep_review_queue_control_snapshot, deep_review_retries_used,
    deep_review_runtime_diagnostics_snapshot, deep_review_shared_context_measurement_snapshot,
    deep_review_turn_elapsed_seconds, default_review_team_definition,
    record_deep_review_capacity_skip, record_deep_review_capacity_skip_for_reason,
    record_deep_review_concurrency_cap_rejection,
    record_deep_review_effective_concurrency_capacity_error,
    record_deep_review_effective_concurrency_success, record_deep_review_runtime_auto_retry,
    record_deep_review_runtime_auto_retry_suppressed, record_deep_review_runtime_capacity_skip,
    record_deep_review_runtime_manual_queue_action, record_deep_review_runtime_manual_retry,
    record_deep_review_runtime_provider_capacity_queue,
    record_deep_review_runtime_provider_capacity_retry,
    record_deep_review_runtime_provider_capacity_retry_success,
    record_deep_review_runtime_queue_wait, record_deep_review_shared_context_tool_use,
    record_deep_review_task_budget, set_deep_review_effective_concurrency_user_override,
    try_begin_deep_review_active_reviewer, try_begin_deep_review_active_reviewer_for_launch_batch,
    ChangeRiskFactors, DeepReviewActiveReviewerGuard, DeepReviewBudgetTracker,
    DeepReviewCapacityFailFastReason, DeepReviewCapacityQueueDecision,
    DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
    DeepReviewEffectiveConcurrencySnapshot, DeepReviewExecutionPolicy, DeepReviewIncrementalCache,
    DeepReviewPolicyViolation, DeepReviewQueueControlAction, DeepReviewQueueControlSnapshot,
    DeepReviewReviewerQueueState, DeepReviewReviewerQueueStatus, DeepReviewRunManifestGate,
    DeepReviewRuntimeDiagnostics, DeepReviewSharedContextDuplicate,
    DeepReviewSharedContextMeasurementSnapshot, DeepReviewStrategyLevel, DeepReviewSubagentRole,
    ReviewStrategyManifestProfile, ReviewTeamDefinition, ReviewTeamExecutionPolicyDefinition,
    ReviewTeamRoleDefinition, CONDITIONAL_REVIEWER_AGENT_TYPES, CORE_REVIEWER_AGENT_TYPES,
    DEEP_REVIEW_AGENT_TYPE, REVIEWER_ARCHITECTURE_AGENT_TYPE, REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_FRONTEND_AGENT_TYPE, REVIEWER_PERFORMANCE_AGENT_TYPE, REVIEWER_SECURITY_AGENT_TYPE,
    REVIEW_FIXER_AGENT_TYPE, REVIEW_JUDGE_AGENT_TYPE,
};

const DEFAULT_REVIEW_TEAM_CONFIG_PATH: &str = "ai.review_teams.default";

pub async fn load_default_deep_review_policy() -> BitFunResult<DeepReviewExecutionPolicy> {
    let config_service = GlobalConfigManager::get_service().await.map_err(|error| {
        BitFunError::config(format!(
            "Failed to load DeepReview execution policy because config service is unavailable: {}",
            error
        ))
    })?;

    let raw_config = match config_service
        .get_config::<Value>(Some(DEFAULT_REVIEW_TEAM_CONFIG_PATH))
        .await
    {
        Ok(config) => Some(config),
        Err(error) if is_missing_default_review_team_config_error(&error) => {
            warn!(
                "DeepReview policy config missing at {}, using defaults",
                DEFAULT_REVIEW_TEAM_CONFIG_PATH
            );
            None
        }
        Err(error) => {
            return Err(BitFunError::config(format!(
                "Failed to load DeepReview execution policy from {}: {}",
                DEFAULT_REVIEW_TEAM_CONFIG_PATH, error
            )));
        }
    };

    Ok(DeepReviewExecutionPolicy::from_config_value(
        raw_config.as_ref(),
    ))
}

pub fn is_missing_default_review_team_config_error(error: &BitFunError) -> bool {
    matches!(error, BitFunError::NotFound(message)
        if message == &format!("Config path '{}' not found", DEFAULT_REVIEW_TEAM_CONFIG_PATH))
}

#[cfg(test)]
mod tests {
    use super::{
        default_review_team_definition, is_missing_default_review_team_config_error,
        DeepReviewBudgetTracker, DeepReviewExecutionPolicy, DeepReviewRunManifestGate,
        DeepReviewStrategyLevel, DeepReviewSubagentRole, REVIEWER_SECURITY_AGENT_TYPE,
    };
    use crate::util::errors::BitFunError;
    use serde_json::json;

    #[test]
    fn only_missing_default_review_team_path_can_fallback_to_defaults() {
        let matching =
            BitFunError::NotFound("Config path 'ai.review_teams.default' not found".to_string());
        assert!(is_missing_default_review_team_config_error(&matching));

        let different_path =
            BitFunError::NotFound("Config path 'ai.review_teams.other' not found".to_string());
        assert!(!is_missing_default_review_team_config_error(
            &different_path
        ));

        let other_error =
            BitFunError::config("Config path 'ai.review_teams.default' not found".to_string());
        assert!(!is_missing_default_review_team_config_error(&other_error));
    }

    #[test]
    fn compatibility_facade_preserves_deep_review_runtime_exports() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "strategy_level": "deep",
            "member_strategy_overrides": {
                "ReviewSecurity": "quick"
            }
        })));
        assert_eq!(policy.strategy_level, DeepReviewStrategyLevel::Deep);
        assert_eq!(
            policy
                .member_strategy_overrides
                .get(REVIEWER_SECURITY_AGENT_TYPE),
            Some(&DeepReviewStrategyLevel::Quick)
        );

        let tracker = DeepReviewBudgetTracker::default();
        tracker
            .record_task(
                "facade-turn",
                &DeepReviewExecutionPolicy::default(),
                DeepReviewSubagentRole::Reviewer,
                REVIEWER_SECURITY_AGENT_TYPE,
                false,
            )
            .expect("facade runtime budget export");

        let manifest = json!({
            "reviewMode": "deep",
            "workPackets": [{ "subagentId": "ReviewSecurity" }]
        });
        let gate = DeepReviewRunManifestGate::from_value(&manifest).expect("manifest gate");
        assert!(gate.ensure_active("ReviewSecurity").is_ok());

        let team = default_review_team_definition();
        assert!(team
            .core_roles
            .iter()
            .any(|reviewer| reviewer.subagent_id == REVIEWER_SECURITY_AGENT_TYPE));
    }
}
