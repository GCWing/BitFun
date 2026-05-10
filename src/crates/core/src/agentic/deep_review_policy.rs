use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::warn;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::Duration;

pub use crate::agentic::deep_review::budget::{
    DeepReviewActiveReviewerGuard, DeepReviewBudgetTracker,
};
pub use crate::agentic::deep_review::concurrency_policy::{
    DeepReviewConcurrencyPolicy, DeepReviewEffectiveConcurrencySnapshot,
};
use crate::agentic::deep_review::constants::DEFAULT_MAX_RETRIES_PER_ROLE;
pub use crate::agentic::deep_review::diagnostics::DeepReviewRuntimeDiagnostics;
pub(crate) use crate::agentic::deep_review::queue::DeepReviewQueueControlTracker;
pub use crate::agentic::deep_review::queue::{
    classify_deep_review_capacity_error, DeepReviewCapacityFailFastReason,
    DeepReviewCapacityQueueDecision, DeepReviewCapacityQueueReason, DeepReviewQueueControlAction,
    DeepReviewQueueControlSnapshot, DeepReviewReviewerQueueState, DeepReviewReviewerQueueStatus,
};
pub use crate::agentic::deep_review::shared_context::{
    DeepReviewSharedContextDuplicate, DeepReviewSharedContextMeasurementSnapshot,
};

pub use crate::agentic::deep_review::constants::{
    CONDITIONAL_REVIEWER_AGENT_TYPES, CORE_REVIEWER_AGENT_TYPES, DEEP_REVIEW_AGENT_TYPE,
    REVIEWER_ARCHITECTURE_AGENT_TYPE, REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_FRONTEND_AGENT_TYPE, REVIEWER_PERFORMANCE_AGENT_TYPE, REVIEWER_SECURITY_AGENT_TYPE,
    REVIEW_FIXER_AGENT_TYPE, REVIEW_JUDGE_AGENT_TYPE,
};
pub use crate::agentic::deep_review::execution_policy::{
    ChangeRiskFactors, DeepReviewExecutionPolicy, DeepReviewPolicyViolation,
    DeepReviewStrategyLevel, DeepReviewSubagentRole,
};
pub use crate::agentic::deep_review::incremental_cache::DeepReviewIncrementalCache;
pub use crate::agentic::deep_review::manifest::DeepReviewRunManifestGate;
pub use crate::agentic::deep_review::team_definition::{
    default_review_team_definition, ReviewStrategyManifestProfile, ReviewTeamDefinition,
    ReviewTeamExecutionPolicyDefinition, ReviewTeamRoleDefinition,
};

const DEFAULT_REVIEW_TEAM_CONFIG_PATH: &str = "ai.review_teams.default";

static GLOBAL_DEEP_REVIEW_BUDGET_TRACKER: LazyLock<DeepReviewBudgetTracker> =
    LazyLock::new(DeepReviewBudgetTracker::default);
static GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER: LazyLock<DeepReviewQueueControlTracker> =
    LazyLock::new(DeepReviewQueueControlTracker::default);

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

pub fn record_deep_review_task_budget(
    parent_dialog_turn_id: &str,
    policy: &DeepReviewExecutionPolicy,
    role: DeepReviewSubagentRole,
    subagent_type: &str,
    is_retry: bool,
) -> Result<(), DeepReviewPolicyViolation> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_task(
        parent_dialog_turn_id,
        policy,
        role,
        subagent_type,
        is_retry,
    )
}

pub fn record_deep_review_concurrency_cap_rejection(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_concurrency_cap_rejection(parent_dialog_turn_id)
}

pub fn record_deep_review_capacity_skip(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_capacity_skip(parent_dialog_turn_id)
}

pub fn record_deep_review_capacity_skip_for_reason(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_capacity_skip_for_reason(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_queue_wait(parent_dialog_turn_id: &str, queue_elapsed_ms: u64) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_queue_wait(parent_dialog_turn_id, queue_elapsed_ms)
}

pub fn record_deep_review_runtime_provider_capacity_queue(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_provider_capacity_queue(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_provider_capacity_retry(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_provider_capacity_retry(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_provider_capacity_retry_success(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_provider_capacity_retry_success(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_capacity_skip(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_capacity_skip(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_manual_queue_action(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_manual_queue_action(parent_dialog_turn_id)
}

pub fn record_deep_review_runtime_manual_retry(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_manual_retry(parent_dialog_turn_id)
}

pub fn record_deep_review_runtime_auto_retry(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_auto_retry(parent_dialog_turn_id)
}

pub fn record_deep_review_runtime_auto_retry_suppressed(parent_dialog_turn_id: &str, reason: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_auto_retry_suppressed(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_shared_context_tool_use(
    parent_dialog_turn_id: &str,
    subagent_type: &str,
    tool_name: &str,
    file_path: &str,
) -> DeepReviewSharedContextMeasurementSnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_shared_context_tool_use(
        parent_dialog_turn_id,
        subagent_type,
        tool_name,
        file_path,
    )
}

pub fn deep_review_shared_context_measurement_snapshot(
    parent_dialog_turn_id: &str,
) -> DeepReviewSharedContextMeasurementSnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.shared_context_measurement_snapshot(parent_dialog_turn_id)
}

pub fn deep_review_runtime_diagnostics_snapshot(
    parent_dialog_turn_id: &str,
) -> Option<DeepReviewRuntimeDiagnostics> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.runtime_diagnostics_snapshot(parent_dialog_turn_id)
}

pub fn try_begin_deep_review_active_reviewer(
    parent_dialog_turn_id: &str,
    max_active_reviewers: usize,
) -> Option<DeepReviewActiveReviewerGuard<'static>> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .try_begin_active_reviewer(parent_dialog_turn_id, max_active_reviewers)
}

pub fn try_begin_deep_review_active_reviewer_for_launch_batch(
    parent_dialog_turn_id: &str,
    max_active_reviewers: usize,
    launch_batch: u64,
    packet_id: Option<&str>,
) -> Result<Option<DeepReviewActiveReviewerGuard<'static>>, DeepReviewPolicyViolation> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.try_begin_active_reviewer_for_launch_batch(
        parent_dialog_turn_id,
        max_active_reviewers,
        launch_batch,
        packet_id,
    )
}

pub fn deep_review_effective_concurrency_snapshot(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .effective_concurrency_snapshot(parent_dialog_turn_id, configured_max_parallel_instances)
}

pub fn deep_review_effective_parallel_instances(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .effective_parallel_instances(parent_dialog_turn_id, configured_max_parallel_instances)
}

pub fn record_deep_review_effective_concurrency_capacity_error(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
    reason: DeepReviewCapacityQueueReason,
    retry_after: Option<Duration>,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_effective_concurrency_capacity_error(
        parent_dialog_turn_id,
        configured_max_parallel_instances,
        reason,
        retry_after,
    )
}

pub fn record_deep_review_effective_concurrency_success(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_effective_concurrency_success(
        parent_dialog_turn_id,
        configured_max_parallel_instances,
    )
}

pub fn set_deep_review_effective_concurrency_user_override(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
    user_override_parallel_instances: Option<usize>,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.set_effective_concurrency_user_override(
        parent_dialog_turn_id,
        configured_max_parallel_instances,
        user_override_parallel_instances,
    )
}

/// Returns the number of active reviewer calls for a given turn.
pub fn deep_review_active_reviewer_count(parent_dialog_turn_id: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.active_reviewer_count(parent_dialog_turn_id)
}

/// Returns true if a judge has been launched for a given turn.
pub fn deep_review_has_judge_been_launched(parent_dialog_turn_id: &str) -> bool {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.has_judge_been_launched(parent_dialog_turn_id)
}

pub fn deep_review_concurrency_cap_rejection_count(parent_dialog_turn_id: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.concurrency_cap_rejection_count(parent_dialog_turn_id)
}

pub fn deep_review_capacity_skip_count(parent_dialog_turn_id: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.capacity_skip_count(parent_dialog_turn_id)
}

pub fn apply_deep_review_queue_control(
    parent_dialog_turn_id: &str,
    tool_id: &str,
    action: DeepReviewQueueControlAction,
) -> DeepReviewQueueControlSnapshot {
    GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER.apply(parent_dialog_turn_id, tool_id, action)
}

pub fn deep_review_queue_control_snapshot(
    parent_dialog_turn_id: &str,
    tool_id: &str,
) -> DeepReviewQueueControlSnapshot {
    GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER.snapshot(parent_dialog_turn_id, tool_id)
}

pub fn clear_deep_review_queue_control_for_tool(parent_dialog_turn_id: &str, tool_id: &str) {
    GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER.clear_tool(parent_dialog_turn_id, tool_id)
}

/// Returns the number of retries used for a specific subagent type in a given turn.
pub fn deep_review_retries_used(parent_dialog_turn_id: &str, subagent_type: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.retries_used(parent_dialog_turn_id, subagent_type)
}

pub fn deep_review_turn_elapsed_seconds(parent_dialog_turn_id: &str) -> Option<u64> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.turn_elapsed_seconds(parent_dialog_turn_id)
}

/// Returns the fallback max retries per role when an effective run policy is unavailable.
pub fn deep_review_max_retries_per_role(_parent_dialog_turn_id: &str) -> usize {
    DEFAULT_MAX_RETRIES_PER_ROLE
}

#[cfg(test)]
mod tests {
    use super::{
        is_missing_default_review_team_config_error, DeepReviewBudgetTracker,
        DeepReviewExecutionPolicy, DeepReviewIncrementalCache, DeepReviewRunManifestGate,
        DeepReviewStrategyLevel, DeepReviewSubagentRole, REVIEWER_ARCHITECTURE_AGENT_TYPE,
        REVIEWER_PERFORMANCE_AGENT_TYPE, REVIEWER_SECURITY_AGENT_TYPE, REVIEW_FIXER_AGENT_TYPE,
        REVIEW_JUDGE_AGENT_TYPE,
    };
    use crate::util::errors::BitFunError;
    use serde_json::json;
    use serde_json::Value;
    use std::time::Duration;

    #[test]
    fn only_missing_default_review_team_path_can_fallback_to_defaults() {
        assert!(is_missing_default_review_team_config_error(
            &BitFunError::NotFound("Config path 'ai.review_teams.default' not found".to_string())
        ));
        assert!(!is_missing_default_review_team_config_error(
            &BitFunError::config("Config service unavailable")
        ));
        assert!(!is_missing_default_review_team_config_error(
            &BitFunError::config("Config path 'ai.review_teams.default.extra' not found")
        ));
    }

    #[test]
    fn default_policy_is_read_only_with_normal_strategy() {
        let policy = DeepReviewExecutionPolicy::default();

        assert_eq!(policy.strategy_level, DeepReviewStrategyLevel::Normal);
        assert!(policy.member_strategy_overrides.is_empty());
        assert_eq!(
            policy
                .classify_subagent(REVIEW_FIXER_AGENT_TYPE)
                .unwrap_err()
                .code,
            "deep_review_fixer_not_allowed"
        );
    }

    #[test]
    fn frontend_reviewer_is_conditional_not_core() {
        let policy = DeepReviewExecutionPolicy::default();

        assert!(!super::CORE_REVIEWER_AGENT_TYPES.contains(&super::REVIEWER_FRONTEND_AGENT_TYPE));
        assert!(
            super::CONDITIONAL_REVIEWER_AGENT_TYPES.contains(&super::REVIEWER_FRONTEND_AGENT_TYPE)
        );
        assert_eq!(
            policy
                .classify_subagent(super::REVIEWER_FRONTEND_AGENT_TYPE)
                .unwrap(),
            DeepReviewSubagentRole::Reviewer
        );
    }

    #[test]
    fn default_review_team_definition_exposes_role_manifest() {
        let definition = super::default_review_team_definition();
        let role_ids: Vec<&str> = definition
            .core_roles
            .iter()
            .map(|role| role.subagent_id.as_str())
            .collect();

        assert_eq!(definition.default_strategy_level, "normal");
        assert!(role_ids.contains(&super::REVIEWER_BUSINESS_LOGIC_AGENT_TYPE));
        assert!(role_ids.contains(&super::REVIEWER_ARCHITECTURE_AGENT_TYPE));
        assert!(role_ids.contains(&super::REVIEWER_FRONTEND_AGENT_TYPE));
        assert!(role_ids.contains(&super::REVIEW_JUDGE_AGENT_TYPE));
        assert!(definition.core_roles.iter().any(|role| {
            role.subagent_id == super::REVIEWER_FRONTEND_AGENT_TYPE && role.conditional
        }));
        assert!(definition
            .hidden_agent_ids
            .contains(&super::REVIEWER_FRONTEND_AGENT_TYPE.to_string()));
        assert!(definition
            .disallowed_extra_subagent_ids
            .contains(&super::REVIEWER_FRONTEND_AGENT_TYPE.to_string()));
        assert!(definition
            .strategy_profiles
            .get("quick")
            .expect("quick strategy")
            .role_directives
            .contains_key(super::REVIEWER_FRONTEND_AGENT_TYPE));
    }

    #[test]
    fn deep_review_team_definition_module_matches_policy_facade() {
        let module_definition =
            crate::agentic::deep_review::team_definition::default_review_team_definition();
        let facade_definition = super::default_review_team_definition();

        assert_eq!(
            serde_json::to_value(module_definition).unwrap(),
            serde_json::to_value(facade_definition).unwrap()
        );
    }

    #[test]
    fn parses_review_strategy_and_member_overrides_from_config() {
        let raw = json!({
            "extra_subagent_ids": ["ExtraOne"],
            "strategy_level": "deep",
            "member_strategy_overrides": {
                "ReviewSecurity": "quick",
                "ReviewJudge": "deep",
                "ExtraOne": "normal",
                "ExtraInvalid": "invalid"
            }
        });

        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&raw));

        assert_eq!(policy.strategy_level, DeepReviewStrategyLevel::Deep);
        assert_eq!(
            policy.member_strategy_overrides.get("ReviewSecurity"),
            Some(&DeepReviewStrategyLevel::Quick)
        );
        assert_eq!(
            policy.member_strategy_overrides.get("ReviewJudge"),
            Some(&DeepReviewStrategyLevel::Deep)
        );
        assert_eq!(
            policy.member_strategy_overrides.get("ExtraOne"),
            Some(&DeepReviewStrategyLevel::Normal)
        );
        assert!(!policy
            .member_strategy_overrides
            .contains_key("ExtraInvalid"));
    }

    #[test]
    fn deep_review_execution_policy_module_matches_policy_facade() {
        let raw = json!({
            "extra_subagent_ids": ["ExtraOne"],
            "strategy_level": "deep",
            "member_strategy_overrides": {
                "ReviewSecurity": "quick",
                "ExtraOne": "normal"
            },
            "reviewer_timeout_seconds": 480,
            "judge_timeout_seconds": 420,
            "reviewer_file_split_threshold": 16,
            "max_same_role_instances": 2,
            "max_retries_per_role": 1
        });

        let module_policy =
            crate::agentic::deep_review::execution_policy::DeepReviewExecutionPolicy::from_config_value(
                Some(&raw),
            );
        let facade_policy = super::DeepReviewExecutionPolicy::from_config_value(Some(&raw));

        assert_eq!(module_policy, facade_policy);
    }

    #[test]
    fn deep_review_manifest_gate_module_matches_policy_facade() {
        let manifest = json!({
            "reviewMode": "deep",
            "workPackets": [
                {"subagentId": "ReviewBusinessLogic"},
                {"subagent_id": "ReviewSecurity"}
            ],
            "qualityGateReviewer": {"subagentId": "ReviewJudge"},
            "skippedReviewers": [
                {"subagentId": "ReviewFrontend", "reason": "not_frontend"}
            ]
        });

        let module_gate =
            crate::agentic::deep_review::manifest::DeepReviewRunManifestGate::from_value(&manifest)
                .expect("module manifest gate");
        let facade_gate =
            super::DeepReviewRunManifestGate::from_value(&manifest).expect("facade manifest gate");

        assert_eq!(
            module_gate.ensure_active("ReviewBusinessLogic"),
            facade_gate.ensure_active("ReviewBusinessLogic")
        );
        assert_eq!(
            module_gate.ensure_active("ReviewFrontend"),
            facade_gate.ensure_active("ReviewFrontend")
        );
    }

    #[test]
    fn deep_review_diagnostics_module_matches_policy_facade() {
        let mut suppressed = std::collections::BTreeMap::new();
        suppressed.insert("scope_not_reduced".to_string(), 2);
        let mut provider_queue_reasons = std::collections::BTreeMap::new();
        provider_queue_reasons.insert("provider_rate_limit".to_string(), 1);
        let mut provider_retry_reasons = std::collections::BTreeMap::new();
        provider_retry_reasons.insert("retry_after".to_string(), 1);
        let mut provider_retry_success_reasons = std::collections::BTreeMap::new();
        provider_retry_success_reasons.insert("retry_after".to_string(), 1);
        let mut capacity_skip_reasons = std::collections::BTreeMap::new();
        capacity_skip_reasons.insert("provider_concurrency_limit".to_string(), 1);
        let module_diagnostics =
            crate::agentic::deep_review::diagnostics::DeepReviewRuntimeDiagnostics {
                queue_wait_count: 1,
                queue_wait_total_ms: 1250,
                queue_wait_max_ms: 1250,
                provider_capacity_queue_count: 1,
                provider_capacity_retry_count: 1,
                provider_capacity_retry_success_count: 1,
                capacity_skip_count: 1,
                provider_capacity_queue_reason_counts: provider_queue_reasons,
                provider_capacity_retry_reason_counts: provider_retry_reasons,
                provider_capacity_retry_success_reason_counts: provider_retry_success_reasons,
                capacity_skip_reason_counts: capacity_skip_reasons,
                effective_parallel_min: Some(1),
                effective_parallel_final: Some(2),
                manual_queue_action_count: 1,
                manual_retry_count: 1,
                auto_retry_count: 1,
                auto_retry_suppressed_reason_counts: suppressed,
                shared_context_total_calls: 3,
                shared_context_duplicate_calls: 1,
                shared_context_duplicate_context_count: 1,
                shared_context_duplicate_savings_candidate_count: 1,
            };
        let facade_diagnostics: super::DeepReviewRuntimeDiagnostics = module_diagnostics.clone();

        assert_eq!(
            serde_json::to_value(module_diagnostics).unwrap(),
            serde_json::to_value(facade_diagnostics).unwrap()
        );
    }

    #[test]
    fn classify_rejects_deep_review_nested_task() {
        let policy = DeepReviewExecutionPolicy::default();
        let result = policy.classify_subagent("DeepReview");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code,
            "deep_review_nested_task_disallowed"
        );
    }

    #[test]
    fn classify_rejects_unknown_subagent() {
        let policy = DeepReviewExecutionPolicy::default();
        let result = policy.classify_subagent("UnknownAgent");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "deep_review_subagent_not_allowed");
    }

    #[test]
    fn run_manifest_gate_allows_only_active_reviewers() {
        let manifest = json!({
            "reviewMode": "deep",
            "coreReviewers": [
                { "subagentId": "ReviewBusinessLogic" }
            ],
            "enabledExtraReviewers": [
                { "subagentId": "ExtraReviewer" }
            ],
            "qualityGateReviewer": { "subagentId": "ReviewJudge" },
            "skippedReviewers": [
                { "subagentId": "ReviewFrontend", "reason": "not_applicable" }
            ]
        });

        let gate = DeepReviewRunManifestGate::from_value(&manifest)
            .expect("valid run manifest should produce a gate");

        gate.ensure_active("ReviewBusinessLogic").unwrap();
        gate.ensure_active("ExtraReviewer").unwrap();
        gate.ensure_active("ReviewJudge").unwrap();

        let violation = gate.ensure_active("ReviewFrontend").unwrap_err();
        assert_eq!(violation.code, "deep_review_subagent_not_active_for_target");
        assert!(violation.message.contains("ReviewFrontend"));
        assert!(violation.message.contains("not_applicable"));
    }

    #[test]
    fn run_manifest_gate_is_absent_without_review_team_shape() {
        let manifest = json!({
            "reviewMode": "deep",
            "skippedReviewers": [
                { "subagentId": "ReviewFrontend", "reason": "not_applicable" }
            ]
        });

        assert!(DeepReviewRunManifestGate::from_value(&manifest).is_none());
    }

    #[test]
    fn run_manifest_gate_accepts_work_packet_roster() {
        let manifest = json!({
            "reviewMode": "deep",
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewBusinessLogic",
                    "subagentId": "ReviewBusinessLogic"
                },
                {
                    "packet_id": "judge:ReviewJudge",
                    "subagent_id": "ReviewJudge"
                }
            ],
            "skippedReviewers": [
                { "subagentId": "ReviewFrontend", "reason": "not_applicable" }
            ]
        });

        let gate = DeepReviewRunManifestGate::from_value(&manifest)
            .expect("work packet manifest should produce a gate");

        gate.ensure_active("ReviewBusinessLogic").unwrap();
        gate.ensure_active("ReviewJudge").unwrap();

        let violation = gate.ensure_active("ReviewFrontend").unwrap_err();
        assert_eq!(violation.code, "deep_review_subagent_not_active_for_target");
        assert!(violation.message.contains("not_applicable"));
    }

    #[test]
    fn classify_always_rejects_review_fixer() {
        let policy = DeepReviewExecutionPolicy::default();
        let result = policy.classify_subagent(REVIEW_FIXER_AGENT_TYPE);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "deep_review_fixer_not_allowed");

        let policy_with_legacy_config =
            DeepReviewExecutionPolicy::from_config_value(Some(&json!({
                "auto_fix_enabled": true
            })));
        let result2 = policy_with_legacy_config.classify_subagent(REVIEW_FIXER_AGENT_TYPE);
        assert!(result2.is_err());
        assert_eq!(result2.unwrap_err().code, "deep_review_fixer_not_allowed");
    }

    #[test]
    fn extra_subagent_ids_deduplicates_and_filters_disallowed() {
        let raw = json!({
            "extra_subagent_ids": [
                "ExtraOne",
                "ExtraOne",
                "ReviewBusinessLogic",
                "DeepReview",
                "ReviewFixer",
                "ReviewJudge",
                "",
                123
            ]
        });
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&raw));

        assert_eq!(policy.extra_subagent_ids.len(), 1);
        assert_eq!(policy.extra_subagent_ids[0], "ExtraOne");
        assert!(!policy
            .extra_subagent_ids
            .contains(&"ReviewBusinessLogic".to_string()));
        assert!(!policy
            .extra_subagent_ids
            .contains(&"DeepReview".to_string()));
    }

    #[test]
    fn budget_tracker_caps_judge_calls_per_turn() {
        let policy = DeepReviewExecutionPolicy::default();
        let tracker = DeepReviewBudgetTracker::default();

        // turn-1: one judge call allowed
        tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Judge,
                REVIEW_JUDGE_AGENT_TYPE,
                false,
            )
            .unwrap();
        assert!(tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Judge,
                REVIEW_JUDGE_AGENT_TYPE,
                false,
            )
            .is_err());

        // turn-2: fresh budget, should succeed
        tracker
            .record_task(
                "turn-2",
                &policy,
                DeepReviewSubagentRole::Judge,
                REVIEW_JUDGE_AGENT_TYPE,
                false,
            )
            .unwrap();
    }

    #[test]
    fn effective_timeout_zero_cap_allows_any_requested() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "reviewer_timeout_seconds": 0,
            "judge_timeout_seconds": 0
        })));

        // When cap is 0, any requested timeout should pass through
        assert_eq!(
            policy.effective_timeout_seconds(DeepReviewSubagentRole::Reviewer, Some(900)),
            Some(900)
        );
        assert_eq!(
            policy.effective_timeout_seconds(DeepReviewSubagentRole::Reviewer, None),
            None
        );
    }

    #[test]
    fn predictive_timeout_scales_with_target_size_and_reviewer_count() {
        let policy = DeepReviewExecutionPolicy::default();

        assert_eq!(
            policy.predictive_timeout(
                DeepReviewSubagentRole::Reviewer,
                DeepReviewStrategyLevel::Normal,
                25,
                0,
                5,
            ),
            675
        );
        assert_eq!(
            policy.predictive_timeout(
                DeepReviewSubagentRole::Judge,
                DeepReviewStrategyLevel::Normal,
                25,
                0,
                5,
            ),
            1350
        );
    }

    #[test]
    fn run_manifest_execution_policy_overrides_static_timeouts() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "reviewer_timeout_seconds": 300,
            "judge_timeout_seconds": 240,
            "reviewer_file_split_threshold": 20,
            "max_same_role_instances": 3
        })));
        let manifest = json!({
            "reviewMode": "deep",
            "strategyLevel": "normal",
            "executionPolicy": {
                "reviewerTimeoutSeconds": 675,
                "judgeTimeoutSeconds": 1350,
                "reviewerFileSplitThreshold": 10,
                "maxSameRoleInstances": 4
            },
            "coreReviewers": [
                { "subagentId": "ReviewBusinessLogic" }
            ],
            "qualityGateReviewer": { "subagentId": "ReviewJudge" }
        });

        let effective = policy.with_run_manifest_execution_policy(&manifest);

        assert_eq!(effective.reviewer_timeout_seconds, 675);
        assert_eq!(effective.judge_timeout_seconds, 1350);
        assert_eq!(effective.reviewer_file_split_threshold, 10);
        assert_eq!(effective.max_same_role_instances, 4);
    }

    #[test]
    fn default_file_split_threshold_and_max_instances() {
        let policy = DeepReviewExecutionPolicy::default();
        assert_eq!(policy.reviewer_file_split_threshold, 20);
        assert_eq!(policy.max_same_role_instances, 3);
    }

    #[test]
    fn should_split_files_below_threshold() {
        let policy = DeepReviewExecutionPolicy::default();
        // 20 files, threshold is 20, should NOT split (needs > threshold)
        assert!(!policy.should_split_files(20));
        // 21 files, threshold is 20, should split
        assert!(policy.should_split_files(21));
    }

    #[test]
    fn should_split_disabled_when_threshold_zero() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "reviewer_file_split_threshold": 0
        })));
        assert!(!policy.should_split_files(100));
    }

    #[test]
    fn should_split_disabled_when_max_instances_one() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 1
        })));
        assert!(!policy.should_split_files(100));
    }

    #[test]
    fn same_role_instance_count_capped_by_max() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "reviewer_file_split_threshold": 5,
            "max_same_role_instances": 3
        })));
        // 50 files / 5 threshold = 10 groups, but capped at 3
        assert_eq!(policy.same_role_instance_count(50), 3);
    }

    #[test]
    fn same_role_instance_count_exact_groups() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "reviewer_file_split_threshold": 10,
            "max_same_role_instances": 5
        })));
        // 25 files / 10 threshold = 3 groups
        assert_eq!(policy.same_role_instance_count(25), 3);
    }

    #[test]
    fn same_role_instance_count_no_split() {
        let policy = DeepReviewExecutionPolicy::default();
        // Below threshold, always 1
        assert_eq!(policy.same_role_instance_count(10), 1);
    }

    #[test]
    fn budget_tracker_caps_reviewer_calls_by_max_same_role_instances() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 2
        })));
        let tracker = DeepReviewBudgetTracker::default();

        // Default policy: 5 core reviewers * 2 max instances = 10 reviewer calls allowed
        for _ in 0..10 {
            tracker
                .record_task(
                    "turn-1",
                    &policy,
                    DeepReviewSubagentRole::Reviewer,
                    "ReviewBusinessLogic",
                    false,
                )
                .unwrap();
        }
        // 11th reviewer call should be rejected
        assert!(tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Reviewer,
                "ReviewSecurity",
                false,
            )
            .is_err());
    }

    #[test]
    fn budget_tracker_allows_one_retry_after_initial_reviewer_budget() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 1,
            "max_retries_per_role": 1
        })));
        let tracker = DeepReviewBudgetTracker::default();

        for reviewer in [
            "ReviewBusinessLogic",
            "ReviewPerformance",
            "ReviewSecurity",
            "ReviewArchitecture",
            "ReviewFrontend",
        ] {
            tracker
                .record_task(
                    "turn-1",
                    &policy,
                    DeepReviewSubagentRole::Reviewer,
                    reviewer,
                    false,
                )
                .unwrap();
        }

        assert!(tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Reviewer,
                "ReviewSecurity",
                false,
            )
            .is_err());
        tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Reviewer,
                "ReviewSecurity",
                true,
            )
            .unwrap();

        let violation = tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Reviewer,
                "ReviewSecurity",
                true,
            )
            .unwrap_err();
        assert_eq!(violation.code, "deep_review_retry_budget_exhausted");
    }

    #[test]
    fn budget_tracker_rejects_retry_without_initial_reviewer_call() {
        let policy = DeepReviewExecutionPolicy::default();
        let tracker = DeepReviewBudgetTracker::default();

        let violation = tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Reviewer,
                "ReviewSecurity",
                true,
            )
            .unwrap_err();

        assert_eq!(violation.code, "deep_review_retry_without_initial_attempt");
    }

    #[test]
    fn max_same_role_instances_clamped_to_range() {
        // Value 0 should be clamped to 1
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 0
        })));
        assert_eq!(policy.max_same_role_instances, 1);

        // Large values are preserved so the config does not impose a hidden cap.
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 100
        })));
        assert_eq!(policy.max_same_role_instances, 100);
    }

    #[test]
    fn auto_select_strategy_quick_for_small_changes() {
        let policy = DeepReviewExecutionPolicy::default();
        let risk = super::ChangeRiskFactors {
            file_count: 2,
            total_lines_changed: 80,
            files_in_security_paths: 0,
            max_cyclomatic_complexity_delta: 0,
            cross_crate_changes: 0,
        };
        let (level, rationale) = policy.auto_select_strategy(&risk);
        assert_eq!(level, DeepReviewStrategyLevel::Quick);
        assert!(rationale.contains("2 files"));
        assert!(rationale.contains("80 lines"));
    }

    #[test]
    fn auto_select_strategy_normal_for_medium_changes() {
        let policy = DeepReviewExecutionPolicy::default();
        let risk = super::ChangeRiskFactors {
            file_count: 8,
            total_lines_changed: 400,
            files_in_security_paths: 0,
            max_cyclomatic_complexity_delta: 0,
            cross_crate_changes: 0,
        };
        let (level, rationale) = policy.auto_select_strategy(&risk);
        assert_eq!(level, DeepReviewStrategyLevel::Normal);
        assert!(rationale.contains("8 files"));
    }

    #[test]
    fn auto_select_strategy_deep_for_large_or_risky_changes() {
        let policy = DeepReviewExecutionPolicy::default();
        let risk = super::ChangeRiskFactors {
            file_count: 30,
            total_lines_changed: 2000,
            files_in_security_paths: 3,
            max_cyclomatic_complexity_delta: 0,
            cross_crate_changes: 2,
        };
        let (level, rationale) = policy.auto_select_strategy(&risk);
        assert_eq!(level, DeepReviewStrategyLevel::Deep);
        assert!(rationale.contains("30 files"));
        assert!(rationale.contains("3 security files"));
    }

    #[test]
    fn auto_select_strategy_security_paths_boost_score() {
        let policy = super::DeepReviewExecutionPolicy::default();
        // 4 files + 0 lines/100 + 2 security * 3 = 10 -> Normal
        let risk = super::ChangeRiskFactors {
            file_count: 4,
            total_lines_changed: 0,
            files_in_security_paths: 2,
            max_cyclomatic_complexity_delta: 0,
            cross_crate_changes: 0,
        };
        let (level, _) = policy.auto_select_strategy(&risk);
        assert_eq!(level, DeepReviewStrategyLevel::Normal);
    }

    #[test]
    fn concurrency_policy_default_values() {
        let policy = super::DeepReviewConcurrencyPolicy::default();
        assert_eq!(policy.max_parallel_instances, 4);
        assert_eq!(policy.stagger_seconds, 0);
        assert!(policy.batch_extras_separately);
    }

    #[test]
    fn concurrency_policy_from_manifest() {
        let raw = json!({
            "maxParallelInstances": 6,
            "staggerSeconds": 5,
            "batchExtrasSeparately": false,
            "allowBoundedAutoRetry": true,
            "autoRetryElapsedGuardSeconds": 240
        });
        let policy = super::DeepReviewConcurrencyPolicy::from_manifest(&raw);
        assert_eq!(policy.max_parallel_instances, 6);
        assert_eq!(policy.stagger_seconds, 5);
        assert!(!policy.batch_extras_separately);
        assert!(policy.allow_bounded_auto_retry);
        assert_eq!(policy.auto_retry_elapsed_guard_seconds, 240);
    }

    #[test]
    fn concurrency_policy_clamps_auto_retry_elapsed_guard() {
        let policy = super::DeepReviewConcurrencyPolicy::from_manifest(&json!({
            "allowBoundedAutoRetry": true,
            "autoRetryElapsedGuardSeconds": 1
        }));
        assert!(policy.allow_bounded_auto_retry);
        assert_eq!(policy.auto_retry_elapsed_guard_seconds, 30);

        let policy = super::DeepReviewConcurrencyPolicy::from_manifest(&json!({
            "allowBoundedAutoRetry": true,
            "autoRetryElapsedGuardSeconds": 9999
        }));
        assert_eq!(policy.auto_retry_elapsed_guard_seconds, 900);
    }

    #[test]
    fn concurrency_effective_max_same_role_instances() {
        let exec_policy = DeepReviewExecutionPolicy::default();
        let conc_policy = super::DeepReviewConcurrencyPolicy {
            max_parallel_instances: 4,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        // 5 reviewer types (4 core + 1 conditional), 4 / 5 = 0 -> clamped to 1
        assert_eq!(
            conc_policy.effective_max_same_role_instances(&exec_policy),
            1
        );

        let conc_policy_12 = super::DeepReviewConcurrencyPolicy {
            max_parallel_instances: 12,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        // 12 / 5 = 2, capped by default max_same_role_instances (3) -> 2
        assert_eq!(
            conc_policy_12.effective_max_same_role_instances(&exec_policy),
            2
        );
    }

    #[test]
    fn concurrency_check_launch_allowed() {
        let policy = super::DeepReviewConcurrencyPolicy::default();
        // 0 active reviewers -> reviewer allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Reviewer, false)
            .is_ok());
        // 4 active reviewers (at cap) -> reviewer blocked
        let err = policy
            .check_launch_allowed(4, DeepReviewSubagentRole::Reviewer, false)
            .unwrap_err();
        assert_eq!(err.code, "deep_review_concurrency_cap_reached");
        // 1 active reviewer -> judge blocked
        let err = policy
            .check_launch_allowed(1, DeepReviewSubagentRole::Judge, false)
            .unwrap_err();
        assert_eq!(err.code, "deep_review_judge_launch_blocked_by_reviewers");
        // 0 active reviewers, judge not pending -> judge allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Judge, false)
            .is_ok());
        // 0 active reviewers, judge pending -> blocked
        let err = policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Judge, true)
            .unwrap_err();
        assert_eq!(err.code, "deep_review_judge_already_pending");
    }

    #[test]
    fn concurrency_policy_from_run_manifest() {
        let policy = DeepReviewExecutionPolicy::default();
        let manifest = json!({
            "reviewMode": "deep",
            "concurrencyPolicy": {
                "maxParallelInstances": 3,
                "staggerSeconds": 10,
                "maxQueueWaitSeconds": 45
            }
        });
        let conc = policy.concurrency_policy_from_manifest(&manifest);
        assert_eq!(conc.max_parallel_instances, 3);
        assert_eq!(conc.stagger_seconds, 10);
        assert_eq!(conc.max_queue_wait_seconds, 45);
        assert!(conc.batch_extras_separately);
    }

    #[test]
    fn active_reviewer_guard_tracks_running_reviewers_only() {
        let tracker = DeepReviewBudgetTracker::default();
        let policy = DeepReviewExecutionPolicy::default();

        tracker
            .record_task(
                "turn-active",
                &policy,
                DeepReviewSubagentRole::Reviewer,
                REVIEWER_SECURITY_AGENT_TYPE,
                false,
            )
            .unwrap();
        assert_eq!(tracker.active_reviewer_count("turn-active"), 0);

        {
            let _guard = tracker.begin_active_reviewer("turn-active");
            assert_eq!(tracker.active_reviewer_count("turn-active"), 1);
        }

        assert_eq!(tracker.active_reviewer_count("turn-active"), 0);
    }

    #[test]
    fn active_reviewer_try_begin_respects_capacity_atomically() {
        let tracker = DeepReviewBudgetTracker::default();
        let first = tracker
            .try_begin_active_reviewer("turn-atomic", 1)
            .expect("first reviewer should acquire capacity");

        assert!(tracker
            .try_begin_active_reviewer("turn-atomic", 1)
            .is_none());
        assert_eq!(tracker.active_reviewer_count("turn-atomic"), 1);

        drop(first);

        assert!(tracker
            .try_begin_active_reviewer("turn-atomic", 1)
            .is_some());
    }

    #[test]
    fn capacity_skip_count_is_tracked_separately_from_hard_rejections() {
        let tracker = DeepReviewBudgetTracker::default();

        tracker.record_capacity_skip("turn-skip");
        tracker.record_capacity_skip("turn-skip");
        tracker.record_concurrency_cap_rejection("turn-skip");

        assert_eq!(tracker.capacity_skip_count("turn-skip"), 2);
        assert_eq!(tracker.concurrency_cap_rejection_count("turn-skip"), 1);
    }

    #[test]
    fn shared_context_measurement_tracks_duplicate_readonly_file_context_without_content() {
        let tracker = DeepReviewBudgetTracker::default();

        tracker.record_shared_context_tool_use(
            "turn-shared-context",
            REVIEWER_SECURITY_AGENT_TYPE,
            "Read",
            ".\\src\\lib.rs",
        );
        tracker.record_shared_context_tool_use(
            "turn-shared-context",
            REVIEWER_PERFORMANCE_AGENT_TYPE,
            "Read",
            "src/lib.rs",
        );
        tracker.record_shared_context_tool_use(
            "turn-shared-context",
            REVIEWER_SECURITY_AGENT_TYPE,
            "GetFileDiff",
            "src/lib.rs",
        );
        tracker.record_shared_context_tool_use(
            "turn-shared-context",
            REVIEWER_ARCHITECTURE_AGENT_TYPE,
            "Read",
            "src/other.rs",
        );

        let snapshot = tracker.shared_context_measurement_snapshot("turn-shared-context");

        assert_eq!(snapshot.total_calls, 4);
        assert_eq!(snapshot.duplicate_calls, 1);
        assert_eq!(snapshot.duplicate_context_count, 1);
        assert_eq!(snapshot.repeated_contexts.len(), 1);
        assert_eq!(snapshot.repeated_contexts[0].tool_name, "Read");
        assert_eq!(snapshot.repeated_contexts[0].file_path, "src/lib.rs");
        assert_eq!(snapshot.repeated_contexts[0].call_count, 2);
        assert_eq!(snapshot.repeated_contexts[0].reviewer_count, 2);
    }

    #[test]
    fn runtime_diagnostics_records_queue_and_capacity_transitions_as_counts() {
        let tracker = DeepReviewBudgetTracker::default();

        tracker.record_runtime_queue_wait("turn-runtime", 1_250);
        tracker.record_runtime_queue_wait("turn-runtime", 2_500);
        tracker.record_runtime_capacity_skip(
            "turn-runtime",
            super::DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
        );

        let diagnostics = tracker
            .runtime_diagnostics_snapshot("turn-runtime")
            .expect("runtime diagnostics should exist");

        assert_eq!(diagnostics.queue_wait_count, 2);
        assert_eq!(diagnostics.queue_wait_total_ms, 3_750);
        assert_eq!(diagnostics.queue_wait_max_ms, 2_500);
        assert_eq!(diagnostics.capacity_skip_count, 1);
        assert_eq!(
            diagnostics
                .capacity_skip_reason_counts
                .get("provider_concurrency_limit"),
            Some(&1)
        );
        assert_eq!(diagnostics.provider_capacity_queue_count, 0);
    }

    #[test]
    fn runtime_diagnostics_merges_shared_context_without_content() {
        let tracker = DeepReviewBudgetTracker::default();

        tracker.record_shared_context_tool_use(
            "turn-runtime-shared",
            REVIEWER_SECURITY_AGENT_TYPE,
            "Read",
            "src/lib.rs",
        );
        tracker.record_shared_context_tool_use(
            "turn-runtime-shared",
            REVIEWER_ARCHITECTURE_AGENT_TYPE,
            "Read",
            "src/lib.rs",
        );

        let diagnostics = tracker
            .runtime_diagnostics_snapshot("turn-runtime-shared")
            .expect("runtime diagnostics should exist");

        assert_eq!(diagnostics.shared_context_total_calls, 2);
        assert_eq!(diagnostics.shared_context_duplicate_context_count, 1);
        assert!(!format!("{diagnostics:?}").contains("fn "));
    }

    #[test]
    fn effective_concurrency_lowers_after_capacity_errors_without_exceeding_hard_cap() {
        let tracker = DeepReviewBudgetTracker::default();

        assert_eq!(tracker.effective_parallel_instances("turn-effective", 4), 4);

        tracker.record_effective_concurrency_capacity_error(
            "turn-effective",
            4,
            super::DeepReviewCapacityQueueReason::LocalConcurrencyCap,
            None,
        );
        assert_eq!(tracker.effective_parallel_instances("turn-effective", 4), 3);

        for _ in 0..8 {
            tracker.record_effective_concurrency_capacity_error(
                "turn-effective",
                4,
                super::DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                None,
            );
        }
        assert_eq!(tracker.effective_parallel_instances("turn-effective", 4), 1);
    }

    #[test]
    fn effective_concurrency_recovers_after_success_observation_window() {
        let tracker = DeepReviewBudgetTracker::default();

        tracker.record_effective_concurrency_capacity_error(
            "turn-recover",
            4,
            super::DeepReviewCapacityQueueReason::LocalConcurrencyCap,
            None,
        );
        assert_eq!(tracker.effective_parallel_instances("turn-recover", 4), 3);

        tracker.record_effective_concurrency_success("turn-recover", 4);
        tracker.record_effective_concurrency_success("turn-recover", 4);
        assert_eq!(tracker.effective_parallel_instances("turn-recover", 4), 3);

        tracker.record_effective_concurrency_success("turn-recover", 4);
        assert_eq!(tracker.effective_parallel_instances("turn-recover", 4), 4);
    }

    #[test]
    fn effective_concurrency_respects_retry_after_before_recovery() {
        let tracker = DeepReviewBudgetTracker::default();

        let snapshot = tracker.record_effective_concurrency_capacity_error(
            "turn-retry-after",
            4,
            super::DeepReviewCapacityQueueReason::RetryAfter,
            Some(Duration::from_secs(60)),
        );
        assert_eq!(snapshot.learned_parallel_instances, 3);
        assert_eq!(snapshot.effective_parallel_instances, 1);
        assert!(snapshot.retry_after_remaining_ms.unwrap_or_default() > 0);

        for _ in 0..3 {
            tracker.record_effective_concurrency_success("turn-retry-after", 4);
        }
        assert_eq!(
            tracker.effective_parallel_instances("turn-retry-after", 4),
            1
        );
    }

    #[test]
    fn effective_concurrency_user_override_is_bounded_and_visible() {
        let tracker = DeepReviewBudgetTracker::default();

        tracker.record_effective_concurrency_capacity_error(
            "turn-override",
            4,
            super::DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            None,
        );
        tracker.set_effective_concurrency_user_override("turn-override", 4, Some(9));

        let snapshot = tracker.effective_concurrency_snapshot("turn-override", 4);
        assert_eq!(snapshot.configured_max_parallel_instances, 4);
        assert_eq!(snapshot.learned_parallel_instances, 3);
        assert_eq!(snapshot.user_override_parallel_instances, Some(4));
        assert_eq!(snapshot.effective_parallel_instances, 4);

        tracker.set_effective_concurrency_user_override("turn-override", 4, Some(0));
        let snapshot = tracker.effective_concurrency_snapshot("turn-override", 4);
        assert_eq!(snapshot.user_override_parallel_instances, Some(1));
        assert_eq!(snapshot.effective_parallel_instances, 1);
    }

    #[test]
    fn capacity_error_classifier_queues_only_transient_capacity_failures() {
        let queueable_cases = [
            (
                "provider_rate_limit",
                "Provider rate limit exceeded",
                None,
                super::DeepReviewCapacityQueueReason::ProviderRateLimit,
            ),
            (
                "provider_error",
                "Too many concurrent requests for this account",
                None,
                super::DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            ),
            (
                "provider_unavailable",
                "Model is temporarily overloaded",
                None,
                super::DeepReviewCapacityQueueReason::TemporaryOverload,
            ),
            (
                "provider_error",
                "Retry later",
                Some(30),
                super::DeepReviewCapacityQueueReason::RetryAfter,
            ),
            (
                "deep_review_concurrency_cap_reached",
                "Maximum parallel reviewer instances reached",
                None,
                super::DeepReviewCapacityQueueReason::LocalConcurrencyCap,
            ),
        ];

        for (code, message, retry_after_seconds, expected_reason) in queueable_cases {
            let decision =
                super::classify_deep_review_capacity_error(code, message, retry_after_seconds);
            assert!(decision.queueable, "{code} should be queueable");
            assert_eq!(decision.reason, Some(expected_reason));
        }

        let retry_after_decision = super::classify_deep_review_capacity_error(
            "provider_error",
            "Provider returned Retry-After: 45",
            None,
        );
        assert_eq!(
            retry_after_decision.reason,
            Some(super::DeepReviewCapacityQueueReason::RetryAfter)
        );
        assert_eq!(retry_after_decision.retry_after_seconds, Some(45));
    }

    #[test]
    fn capacity_error_classifier_fails_fast_for_non_capacity_failures() {
        let non_queueable_cases = [
            (
                "authentication_failed",
                "API key is invalid",
                super::DeepReviewCapacityFailFastReason::Authentication,
            ),
            (
                "provider_quota_exhausted",
                "Quota exhausted for this billing period",
                super::DeepReviewCapacityFailFastReason::BillingOrQuota,
            ),
            (
                "billing_required",
                "Billing is not configured",
                super::DeepReviewCapacityFailFastReason::BillingOrQuota,
            ),
            (
                "invalid_model",
                "The requested model does not exist",
                super::DeepReviewCapacityFailFastReason::InvalidModel,
            ),
            (
                "user_cancelled",
                "User cancelled the operation",
                super::DeepReviewCapacityFailFastReason::UserCancellation,
            ),
            (
                "deep_review_subagent_not_allowed",
                "Subagent is not allowed",
                super::DeepReviewCapacityFailFastReason::InvalidReviewerTooling,
            ),
            (
                "invalid_tooling",
                "Review agent is missing GetFileDiff",
                super::DeepReviewCapacityFailFastReason::InvalidReviewerTooling,
            ),
        ];

        for (code, message, expected_reason) in non_queueable_cases {
            let decision = super::classify_deep_review_capacity_error(code, message, None);
            assert!(!decision.queueable, "{code} should fail fast");
            assert_eq!(decision.reason, None);
            assert_eq!(decision.fail_fast_reason, Some(expected_reason));
        }
    }

    #[test]
    fn queue_state_keeps_queue_wait_out_of_reviewer_timeout() {
        let queued = super::DeepReviewReviewerQueueState::queued_for_capacity(
            super::DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            45_000,
        );
        assert_eq!(
            queued.status,
            super::DeepReviewReviewerQueueStatus::QueuedForCapacity
        );
        assert_eq!(queued.queue_elapsed_ms, 45_000);
        assert_eq!(queued.run_elapsed_ms, 0);
        assert_eq!(queued.timeout_elapsed_ms(), 0);

        let running = super::DeepReviewReviewerQueueState::running(45_000, 8_000);
        assert_eq!(
            running.status,
            super::DeepReviewReviewerQueueStatus::Running
        );
        assert_eq!(running.queue_elapsed_ms, 45_000);
        assert_eq!(running.run_elapsed_ms, 8_000);
        assert_eq!(running.timeout_elapsed_ms(), 8_000);
    }

    #[test]
    fn paused_queue_state_does_not_consume_reviewer_timeout() {
        let paused = super::DeepReviewReviewerQueueState::paused_by_user(120_000);

        assert_eq!(
            paused.status,
            super::DeepReviewReviewerQueueStatus::PausedByUser
        );
        assert_eq!(paused.queue_elapsed_ms, 120_000);
        assert_eq!(paused.run_elapsed_ms, 0);
        assert_eq!(paused.timeout_elapsed_ms(), 0);
        assert_eq!(paused.reason, None);
    }

    #[test]
    fn queue_control_pause_continue_cancel_are_tool_scoped() {
        let turn_id = "turn-queue-control-tool";
        let primary_tool_id = "tool-queue-control-a";
        let other_tool_id = "tool-queue-control-b";

        let paused = super::apply_deep_review_queue_control(
            turn_id,
            primary_tool_id,
            super::DeepReviewQueueControlAction::Pause,
        );
        assert!(paused.paused);
        assert!(!paused.cancelled);

        let other = super::deep_review_queue_control_snapshot(turn_id, other_tool_id);
        assert!(!other.paused);
        assert!(!other.cancelled);

        let continued = super::apply_deep_review_queue_control(
            turn_id,
            primary_tool_id,
            super::DeepReviewQueueControlAction::Continue,
        );
        assert!(!continued.paused);
        assert!(!continued.cancelled);

        let cancelled = super::apply_deep_review_queue_control(
            turn_id,
            primary_tool_id,
            super::DeepReviewQueueControlAction::Cancel,
        );
        assert!(!cancelled.paused);
        assert!(cancelled.cancelled);

        super::clear_deep_review_queue_control_for_tool(turn_id, primary_tool_id);
        let cleared = super::deep_review_queue_control_snapshot(turn_id, primary_tool_id);
        assert!(!cleared.paused);
        assert!(!cleared.cancelled);
    }

    #[test]
    fn queue_control_skip_optional_is_turn_scoped() {
        let turn_id = "turn-queue-control-optional";
        let primary_tool_id = "tool-queue-control-primary";
        let other_tool_id = "tool-queue-control-other";

        let snapshot = super::apply_deep_review_queue_control(
            turn_id,
            primary_tool_id,
            super::DeepReviewQueueControlAction::SkipOptional,
        );
        assert!(snapshot.skip_optional);

        let other = super::deep_review_queue_control_snapshot(turn_id, other_tool_id);
        assert!(other.skip_optional);

        super::clear_deep_review_queue_control_for_tool(turn_id, primary_tool_id);
        let after_tool_clear = super::deep_review_queue_control_snapshot(turn_id, other_tool_id);
        assert!(after_tool_clear.skip_optional);
    }

    // --- Incremental review cache tests ---

    #[test]
    fn incremental_cache_builds_and_reads() {
        let mut cache = DeepReviewIncrementalCache::new("fp-abc123");
        assert_eq!(cache.fingerprint(), "fp-abc123");
        assert!(cache.is_empty());

        cache.store_packet("reviewer:ReviewSecurity", "Found 2 security issues");
        cache.store_packet("reviewer:ReviewBusinessLogic", "All good");
        assert_eq!(cache.len(), 2);
        assert!(!cache.is_empty());

        assert_eq!(
            cache.get_packet("reviewer:ReviewSecurity"),
            Some("Found 2 security issues")
        );
        assert_eq!(cache.get_packet("reviewer:ReviewArchitecture"), None);
    }

    #[test]
    fn incremental_cache_matches_fingerprint() {
        let cache = DeepReviewIncrementalCache::new("fp-abc123");
        let manifest = json!({
            "incrementalReviewCache": {
                "fingerprint": "fp-abc123"
            }
        });
        assert!(cache.matches_manifest(&manifest));

        let wrong_manifest = json!({
            "incrementalReviewCache": {
                "fingerprint": "fp-other"
            }
        });
        assert!(!cache.matches_manifest(&wrong_manifest));
    }

    #[test]
    fn incremental_cache_to_and_from_value() {
        let mut cache = DeepReviewIncrementalCache::new("fp-test");
        cache.store_packet("reviewer:ReviewSecurity", "sec result");
        cache.store_packet("reviewer:ReviewBusinessLogic", "logic result");

        let value = cache.to_value();
        let restored = DeepReviewIncrementalCache::from_value(&value);
        assert_eq!(restored.fingerprint(), "fp-test");
        assert_eq!(restored.len(), 2);
        assert_eq!(
            restored.get_packet("reviewer:ReviewSecurity"),
            Some("sec result")
        );
    }

    #[test]
    fn incremental_cache_preserves_split_packet_keys() {
        let mut cache = DeepReviewIncrementalCache::new("fp-split");
        cache.store_packet("reviewer:ReviewSecurity:group-1-of-2", "sec group 1");
        cache.store_packet("reviewer:ReviewSecurity:group-2-of-2", "sec group 2");

        let restored = DeepReviewIncrementalCache::from_value(&cache.to_value());

        assert_eq!(
            restored.get_packet("reviewer:ReviewSecurity:group-1-of-2"),
            Some("sec group 1")
        );
        assert_eq!(
            restored.get_packet("reviewer:ReviewSecurity:group-2-of-2"),
            Some("sec group 2")
        );
        assert_eq!(restored.get_packet("ReviewSecurity"), None);
    }

    #[test]
    fn incremental_cache_from_null_value() {
        let cache = DeepReviewIncrementalCache::from_value(&Value::Null);
        assert!(cache.is_empty());
        assert_eq!(cache.fingerprint(), "");
    }
}
