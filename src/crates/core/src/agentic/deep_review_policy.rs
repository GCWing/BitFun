use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use dashmap::DashMap;
use log::warn;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

pub const DEEP_REVIEW_AGENT_TYPE: &str = "DeepReview";
pub const REVIEW_JUDGE_AGENT_TYPE: &str = "ReviewJudge";
pub const REVIEW_FIXER_AGENT_TYPE: &str = "ReviewFixer";
pub const REVIEWER_BUSINESS_LOGIC_AGENT_TYPE: &str = "ReviewBusinessLogic";
pub const REVIEWER_PERFORMANCE_AGENT_TYPE: &str = "ReviewPerformance";
pub const REVIEWER_SECURITY_AGENT_TYPE: &str = "ReviewSecurity";
pub const CORE_REVIEWER_AGENT_TYPES: [&str; 3] = [
    REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_PERFORMANCE_AGENT_TYPE,
    REVIEWER_SECURITY_AGENT_TYPE,
];
const DEFAULT_REVIEW_TEAM_CONFIG_PATH: &str = "ai.review_teams.default";

const DEFAULT_REVIEWER_TIMEOUT_SECONDS: u64 = 600;
const DEFAULT_JUDGE_TIMEOUT_SECONDS: u64 = 600;
const MAX_TIMEOUT_SECONDS: u64 = 3600;
const DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD: usize = 20;
const DEFAULT_MAX_SAME_ROLE_INSTANCES: usize = 3;
const MAX_SAME_ROLE_INSTANCES: usize = 8;
const BUDGET_TTL: Duration = Duration::from_secs(60 * 60);
const PRUNE_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewSubagentRole {
    Reviewer,
    Judge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewStrategyLevel {
    Quick,
    Normal,
    Deep,
}

impl Default for DeepReviewStrategyLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl DeepReviewStrategyLevel {
    fn from_value(value: Option<&Value>) -> Option<Self> {
        match value.and_then(Value::as_str) {
            Some("quick") => Some(Self::Quick),
            Some("normal") => Some(Self::Normal),
            Some("deep") => Some(Self::Deep),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewExecutionPolicy {
    pub extra_subagent_ids: Vec<String>,
    pub strategy_level: DeepReviewStrategyLevel,
    pub member_strategy_overrides: HashMap<String, DeepReviewStrategyLevel>,
    pub reviewer_timeout_seconds: u64,
    pub judge_timeout_seconds: u64,
    /// When the number of target files exceeds this threshold, the DeepReview
    /// orchestrator should split files across multiple same-role reviewer
    /// instances to reduce per-instance workload and timeout risk.
    /// Set to 0 to disable file splitting.
    pub reviewer_file_split_threshold: usize,
    /// Maximum number of same-role reviewer instances allowed per review turn.
    /// Clamped to [1, MAX_SAME_ROLE_INSTANCES].
    pub max_same_role_instances: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewPolicyViolation {
    pub code: &'static str,
    pub message: String,
}

impl DeepReviewPolicyViolation {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn to_tool_error_message(&self) -> String {
        json!({
            "code": self.code,
            "message": self.message,
        })
        .to_string()
    }
}

impl Default for DeepReviewExecutionPolicy {
    fn default() -> Self {
        Self {
            extra_subagent_ids: Vec::new(),
            strategy_level: DeepReviewStrategyLevel::default(),
            member_strategy_overrides: HashMap::new(),
            reviewer_timeout_seconds: DEFAULT_REVIEWER_TIMEOUT_SECONDS,
            judge_timeout_seconds: DEFAULT_JUDGE_TIMEOUT_SECONDS,
            reviewer_file_split_threshold: DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD,
            max_same_role_instances: DEFAULT_MAX_SAME_ROLE_INSTANCES,
        }
    }
}

impl DeepReviewExecutionPolicy {
    pub fn from_config_value(raw: Option<&Value>) -> Self {
        let Some(config) = raw.and_then(Value::as_object) else {
            return Self::default();
        };

        Self {
            extra_subagent_ids: normalize_extra_subagent_ids(config.get("extra_subagent_ids")),
            strategy_level: DeepReviewStrategyLevel::from_value(config.get("strategy_level"))
                .unwrap_or_default(),
            member_strategy_overrides: normalize_member_strategy_overrides(
                config.get("member_strategy_overrides"),
            ),
            reviewer_timeout_seconds: clamp_u64(
                config.get("reviewer_timeout_seconds"),
                0,
                MAX_TIMEOUT_SECONDS,
                DEFAULT_REVIEWER_TIMEOUT_SECONDS,
            ),
            judge_timeout_seconds: clamp_u64(
                config.get("judge_timeout_seconds"),
                0,
                MAX_TIMEOUT_SECONDS,
                DEFAULT_JUDGE_TIMEOUT_SECONDS,
            ),
            reviewer_file_split_threshold: clamp_usize(
                config.get("reviewer_file_split_threshold"),
                0,
                usize::MAX,
                DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD,
            ),
            max_same_role_instances: clamp_usize(
                config.get("max_same_role_instances"),
                1,
                MAX_SAME_ROLE_INSTANCES,
                DEFAULT_MAX_SAME_ROLE_INSTANCES,
            ),
        }
    }

    pub fn classify_subagent(
        &self,
        subagent_type: &str,
    ) -> Result<DeepReviewSubagentRole, DeepReviewPolicyViolation> {
        if CORE_REVIEWER_AGENT_TYPES.contains(&subagent_type)
            || self
                .extra_subagent_ids
                .iter()
                .any(|configured| configured == subagent_type)
        {
            return Ok(DeepReviewSubagentRole::Reviewer);
        }

        match subagent_type {
            REVIEW_JUDGE_AGENT_TYPE => Ok(DeepReviewSubagentRole::Judge),
            REVIEW_FIXER_AGENT_TYPE => Err(DeepReviewPolicyViolation::new(
                "deep_review_fixer_not_allowed",
                "ReviewFixer is not allowed during DeepReview execution; remediation must wait for explicit user approval",
            )),
            DEEP_REVIEW_AGENT_TYPE => Err(DeepReviewPolicyViolation::new(
                "deep_review_nested_task_disallowed",
                "DeepReview cannot launch another DeepReview task",
            )),
            _ => Err(DeepReviewPolicyViolation::new(
                "deep_review_subagent_not_allowed",
                format!(
                    "DeepReview may only launch configured review-team agents or ReviewJudge; '{}' is not allowed",
                    subagent_type
                ),
            )),
        }
    }

    pub fn effective_timeout_seconds(
        &self,
        role: DeepReviewSubagentRole,
        requested_timeout_seconds: Option<u64>,
    ) -> Option<u64> {
        let cap = match role {
            DeepReviewSubagentRole::Reviewer => self.reviewer_timeout_seconds,
            DeepReviewSubagentRole::Judge => self.judge_timeout_seconds,
        };

        if cap == 0 {
            return requested_timeout_seconds;
        }

        Some(
            requested_timeout_seconds
                .map(|requested| requested.min(cap))
                .unwrap_or(cap),
        )
    }

    /// Returns true when the file count exceeds the split threshold and
    /// `max_same_role_instances > 1`, meaning the orchestrator should
    /// partition the file list across multiple same-role reviewer instances.
    pub fn should_split_files(&self, file_count: usize) -> bool {
        self.max_same_role_instances > 1
            && self.reviewer_file_split_threshold > 0
            && file_count > self.reviewer_file_split_threshold
    }

    /// Given a file count that exceeds the split threshold, compute how many
    /// same-role instances to launch. Capped by `max_same_role_instances`.
    pub fn same_role_instance_count(&self, file_count: usize) -> usize {
        if !self.should_split_files(file_count) {
            return 1;
        }
        // Split into chunks of roughly `reviewer_file_split_threshold` files
        // each, but never exceed `max_same_role_instances`.
        let needed = (file_count + self.reviewer_file_split_threshold - 1)
            / self.reviewer_file_split_threshold;
        needed.clamp(1, self.max_same_role_instances)
    }
}

#[derive(Debug)]
struct DeepReviewTurnBudget {
    judge_calls: usize,
    /// Tracks total reviewer calls (across all roles) per turn.
    /// Capped by `max_same_role_instances * CORE_REVIEWER_AGENT_TYPES.len() +
    /// extra_subagent_ids.len()` so the orchestrator cannot spawn an
    /// unbounded number of same-role instances.
    reviewer_calls: usize,
    updated_at: Instant,
}

impl DeepReviewTurnBudget {
    fn new(now: Instant) -> Self {
        Self {
            judge_calls: 0,
            reviewer_calls: 0,
            updated_at: now,
        }
    }
}

pub struct DeepReviewBudgetTracker {
    turns: DashMap<String, DeepReviewTurnBudget>,
    last_pruned_at: std::sync::Mutex<Instant>,
}

impl Default for DeepReviewBudgetTracker {
    fn default() -> Self {
        Self {
            turns: DashMap::new(),
            last_pruned_at: std::sync::Mutex::new(Instant::now()),
        }
    }
}

impl DeepReviewBudgetTracker {
    pub fn record_task(
        &self,
        parent_dialog_turn_id: &str,
        policy: &DeepReviewExecutionPolicy,
        role: DeepReviewSubagentRole,
    ) -> Result<(), DeepReviewPolicyViolation> {
        let now = Instant::now();
        if let Ok(last_pruned) = self.last_pruned_at.lock() {
            if now.saturating_duration_since(*last_pruned) >= PRUNE_INTERVAL {
                drop(last_pruned);
                self.prune_stale(now);
            }
        }

        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));

        match role {
            DeepReviewSubagentRole::Reviewer => {
                let max_reviewer_calls = policy.max_same_role_instances
                    * (CORE_REVIEWER_AGENT_TYPES.len() + policy.extra_subagent_ids.len());
                if budget.reviewer_calls >= max_reviewer_calls {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_reviewer_budget_exhausted",
                        format!(
                            "Reviewer launch budget exhausted for this DeepReview turn (max calls: {})",
                            max_reviewer_calls
                        ),
                    ));
                }
                budget.reviewer_calls += 1;
            }
            DeepReviewSubagentRole::Judge => {
                let max_judge_calls = 1;
                if budget.judge_calls >= max_judge_calls {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_budget_exhausted",
                        format!(
                            "ReviewJudge launch budget exhausted for this DeepReview turn (max calls: {})",
                            max_judge_calls
                        ),
                    ));
                }

                budget.judge_calls += 1;
            }
        }

        budget.updated_at = now;
        Ok(())
    }

    fn prune_stale(&self, now: Instant) {
        self.turns
            .retain(|_, budget| now.saturating_duration_since(budget.updated_at) <= BUDGET_TTL);
        if let Ok(mut last_pruned) = self.last_pruned_at.lock() {
            *last_pruned = now;
        }
    }

    /// Explicitly clean up all budget tracking data.
    /// Call this when the application is shutting down or when the review session ends.
    pub fn cleanup(&self) {
        self.turns.clear();
        if let Ok(mut last_pruned) = self.last_pruned_at.lock() {
            *last_pruned = Instant::now();
        }
    }
}

static GLOBAL_DEEP_REVIEW_BUDGET_TRACKER: LazyLock<DeepReviewBudgetTracker> =
    LazyLock::new(DeepReviewBudgetTracker::default);

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
) -> Result<(), DeepReviewPolicyViolation> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_task(parent_dialog_turn_id, policy, role)
}

fn normalize_extra_subagent_ids(raw: Option<&Value>) -> Vec<String> {
    let Some(values) = raw.and_then(Value::as_array) else {
        return Vec::new();
    };

    let disallowed = disallowed_extra_subagent_ids();
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for value in values {
        let Some(id) = value_to_id(value) else {
            continue;
        };
        if id.is_empty() || disallowed.contains(id.as_str()) || !seen.insert(id.clone()) {
            continue;
        }
        normalized.push(id);
    }

    normalized
}

fn normalize_member_strategy_overrides(
    raw: Option<&Value>,
) -> HashMap<String, DeepReviewStrategyLevel> {
    let Some(values) = raw.and_then(Value::as_object) else {
        return HashMap::new();
    };

    let mut normalized = HashMap::new();
    for (subagent_id, value) in values {
        let id = subagent_id.trim();
        let Some(strategy_level) = DeepReviewStrategyLevel::from_value(Some(value)) else {
            continue;
        };
        if !id.is_empty() {
            normalized.insert(id.to_string(), strategy_level);
        }
    }

    normalized
}

fn disallowed_extra_subagent_ids() -> HashSet<&'static str> {
    CORE_REVIEWER_AGENT_TYPES
        .into_iter()
        .chain([
            REVIEW_JUDGE_AGENT_TYPE,
            DEEP_REVIEW_AGENT_TYPE,
            REVIEW_FIXER_AGENT_TYPE,
        ])
        .collect()
}

fn value_to_id(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.trim().to_string()),
        _ => None,
    }
}

fn clamp_u64(raw: Option<&Value>, min: u64, max: u64, fallback: u64) -> u64 {
    let Some(value) = raw.and_then(number_as_i64) else {
        return fallback;
    };

    let min_i64 = i64::try_from(min).unwrap_or(i64::MAX);
    let max_i64 = i64::try_from(max).unwrap_or(i64::MAX);
    value.clamp(min_i64, max_i64) as u64
}

fn clamp_usize(raw: Option<&Value>, min: usize, max: usize, fallback: usize) -> usize {
    let Some(value) = raw.and_then(number_as_i64) else {
        return fallback;
    };

    let min_i64 = i64::try_from(min).unwrap_or(i64::MAX);
    let max_i64 = i64::try_from(max).unwrap_or(i64::MAX);
    value.clamp(min_i64, max_i64) as usize
}

fn number_as_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_u64()
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        is_missing_default_review_team_config_error, DeepReviewBudgetTracker,
        DeepReviewExecutionPolicy, DeepReviewStrategyLevel, DeepReviewSubagentRole,
        REVIEW_FIXER_AGENT_TYPE,
    };
    use crate::util::errors::BitFunError;
    use serde_json::json;

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
    fn classify_always_rejects_review_fixer() {
        let policy = DeepReviewExecutionPolicy::default();
        let result = policy.classify_subagent(REVIEW_FIXER_AGENT_TYPE);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "deep_review_fixer_not_allowed");

        let policy_with_legacy_config =
            DeepReviewExecutionPolicy::from_config_value(Some(&json!({
                "auto_fix_enabled": true,
                "auto_fix_max_rounds": 2
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
            .record_task("turn-1", &policy, DeepReviewSubagentRole::Judge)
            .unwrap();
        assert!(tracker
            .record_task("turn-1", &policy, DeepReviewSubagentRole::Judge)
            .is_err());

        // turn-2: fresh budget, should succeed
        tracker
            .record_task("turn-2", &policy, DeepReviewSubagentRole::Judge)
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

        // Default policy: 3 core reviewers * 2 max instances = 6 reviewer calls allowed
        for _ in 0..6 {
            tracker
                .record_task("turn-1", &policy, DeepReviewSubagentRole::Reviewer)
                .unwrap();
        }
        // 7th reviewer call should be rejected
        assert!(tracker
            .record_task("turn-1", &policy, DeepReviewSubagentRole::Reviewer)
            .is_err());
    }

    #[test]
    fn max_same_role_instances_clamped_to_range() {
        // Value 0 should be clamped to 1
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 0
        })));
        assert_eq!(policy.max_same_role_instances, 1);

        // Value above max (8) should be clamped to 8
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 100
        })));
        assert_eq!(policy.max_same_role_instances, 8);
    }
}
