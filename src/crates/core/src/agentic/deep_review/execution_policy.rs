//! Deep Review execution policy parsing and strategy helpers.
//!
//! This module translates launch strategy metadata into runtime guardrails such
//! as reviewer timeouts, file-splitting thresholds, same-role caps, and retry
//! limits. Strategy scoring remains advisory unless a separate product decision
//! approves backend-owned strategy selection.

use super::constants::{
    CONDITIONAL_REVIEWER_AGENT_TYPES, CORE_REVIEWER_AGENT_TYPES, DEEP_REVIEW_AGENT_TYPE,
    DEFAULT_MAX_RETRIES_PER_ROLE, DEFAULT_MAX_SAME_ROLE_INSTANCES,
    DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD, REVIEW_FIXER_AGENT_TYPE, REVIEW_JUDGE_AGENT_TYPE,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const DEFAULT_REVIEWER_TIMEOUT_SECONDS: u64 = 600;
const DEFAULT_JUDGE_TIMEOUT_SECONDS: u64 = 600;
const MAX_TIMEOUT_SECONDS: u64 = 3600;
const BASE_TIMEOUT_QUICK_SECONDS: u64 = 180;
const BASE_TIMEOUT_NORMAL_SECONDS: u64 = 300;
const BASE_TIMEOUT_DEEP_SECONDS: u64 = 600;
const TIMEOUT_PER_FILE_SECONDS: u64 = 15;
const TIMEOUT_PER_100_LINES_SECONDS: u64 = 30;
const MAX_SAME_ROLE_INSTANCES: usize = 8;
const MAX_RETRIES_PER_ROLE: usize = 3;

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

/// Risk factors used for automatic strategy selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRiskFactors {
    pub file_count: usize,
    pub total_lines_changed: usize,
    pub files_in_security_paths: usize,
    pub max_cyclomatic_complexity_delta: usize,
    pub cross_crate_changes: usize,
}

impl Default for ChangeRiskFactors {
    fn default() -> Self {
        Self {
            file_count: 0,
            total_lines_changed: 0,
            files_in_security_paths: 0,
            max_cyclomatic_complexity_delta: 0,
            cross_crate_changes: 0,
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
    /// Maximum retry launches allowed per reviewer role in one DeepReview turn.
    /// Set to 0 to disable automatic reviewer retries.
    pub max_retries_per_role: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewPolicyViolation {
    pub code: &'static str,
    pub message: String,
}

impl DeepReviewPolicyViolation {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
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
            max_retries_per_role: DEFAULT_MAX_RETRIES_PER_ROLE,
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
                usize::MAX,
                DEFAULT_MAX_SAME_ROLE_INSTANCES,
            ),
            max_retries_per_role: clamp_usize(
                config.get("max_retries_per_role"),
                0,
                MAX_RETRIES_PER_ROLE,
                DEFAULT_MAX_RETRIES_PER_ROLE,
            ),
        }
    }

    pub fn classify_subagent(
        &self,
        subagent_type: &str,
    ) -> Result<DeepReviewSubagentRole, DeepReviewPolicyViolation> {
        if CORE_REVIEWER_AGENT_TYPES.contains(&subagent_type)
            || CONDITIONAL_REVIEWER_AGENT_TYPES.contains(&subagent_type)
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

    pub fn predictive_timeout(
        &self,
        role: DeepReviewSubagentRole,
        strategy: DeepReviewStrategyLevel,
        file_count: usize,
        line_count: usize,
        reviewer_count: usize,
    ) -> u64 {
        let base = match strategy {
            DeepReviewStrategyLevel::Quick => BASE_TIMEOUT_QUICK_SECONDS,
            DeepReviewStrategyLevel::Normal => BASE_TIMEOUT_NORMAL_SECONDS,
            DeepReviewStrategyLevel::Deep => BASE_TIMEOUT_DEEP_SECONDS,
        };
        let file_overhead = u64::try_from(file_count)
            .unwrap_or(u64::MAX)
            .saturating_mul(TIMEOUT_PER_FILE_SECONDS);
        let line_overhead = u64::try_from(line_count / 100)
            .unwrap_or(u64::MAX)
            .saturating_mul(TIMEOUT_PER_100_LINES_SECONDS);
        let raw = base
            .saturating_add(file_overhead)
            .saturating_add(line_overhead);
        let multiplier = match role {
            DeepReviewSubagentRole::Reviewer => 1,
            DeepReviewSubagentRole::Judge => {
                let reviewer_count = u64::try_from(reviewer_count.max(1)).unwrap_or(u64::MAX);
                1 + reviewer_count.saturating_sub(1) / 3
            }
        };

        raw.saturating_mul(multiplier).min(MAX_TIMEOUT_SECONDS)
    }

    pub fn with_run_manifest_execution_policy(&self, raw_manifest: &Value) -> Self {
        let Some(manifest) = raw_manifest.as_object() else {
            return self.clone();
        };
        if manifest.get("reviewMode").and_then(Value::as_str) != Some("deep") {
            return self.clone();
        }

        let mut policy = self.clone();
        if let Some(strategy_level) =
            DeepReviewStrategyLevel::from_value(manifest.get("strategyLevel"))
        {
            policy.strategy_level = strategy_level;
        }

        let Some(execution_policy) = manifest.get("executionPolicy").and_then(Value::as_object)
        else {
            return policy;
        };

        policy.reviewer_timeout_seconds = clamp_u64(
            execution_policy.get("reviewerTimeoutSeconds"),
            0,
            MAX_TIMEOUT_SECONDS,
            policy.reviewer_timeout_seconds,
        );
        policy.judge_timeout_seconds = clamp_u64(
            execution_policy.get("judgeTimeoutSeconds"),
            0,
            MAX_TIMEOUT_SECONDS,
            policy.judge_timeout_seconds,
        );
        policy.reviewer_file_split_threshold = clamp_usize(
            execution_policy.get("reviewerFileSplitThreshold"),
            0,
            usize::MAX,
            policy.reviewer_file_split_threshold,
        );
        policy.max_same_role_instances = clamp_usize(
            execution_policy.get("maxSameRoleInstances"),
            1,
            MAX_SAME_ROLE_INSTANCES,
            policy.max_same_role_instances,
        );
        policy.max_retries_per_role = clamp_usize(
            execution_policy.get("maxRetriesPerRole"),
            0,
            MAX_RETRIES_PER_ROLE,
            policy.max_retries_per_role,
        );

        policy
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

    /// Auto-select strategy level based on change risk factors.
    /// Returns the recommended level and a human-readable rationale.
    pub fn auto_select_strategy(
        &self,
        risk: &ChangeRiskFactors,
    ) -> (DeepReviewStrategyLevel, String) {
        let score = risk.file_count
            + risk.total_lines_changed / 100
            + risk.files_in_security_paths * 3
            + risk.cross_crate_changes * 2;

        match score {
            0..=5 => (
                DeepReviewStrategyLevel::Quick,
                format!(
                    "Small change ({} files, {} lines). Quick scan sufficient.",
                    risk.file_count, risk.total_lines_changed
                ),
            ),
            6..=20 => (
                DeepReviewStrategyLevel::Normal,
                format!(
                    "Medium change ({} files, {} lines). Standard review recommended.",
                    risk.file_count, risk.total_lines_changed
                ),
            ),
            _ => (
                DeepReviewStrategyLevel::Deep,
                format!(
                    "Large/high-risk change ({} files, {} lines, {} security files). Deep review recommended.",
                    risk.file_count, risk.total_lines_changed, risk.files_in_security_paths
                ),
            ),
        }
    }
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
        .chain(CONDITIONAL_REVIEWER_AGENT_TYPES)
        .chain([
            REVIEW_JUDGE_AGENT_TYPE,
            DEEP_REVIEW_AGENT_TYPE,
            REVIEW_FIXER_AGENT_TYPE,
        ])
        .collect()
}

pub(crate) fn reviewer_agent_type_count() -> usize {
    CORE_REVIEWER_AGENT_TYPES.len() + CONDITIONAL_REVIEWER_AGENT_TYPES.len()
}

fn value_to_id(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.trim().to_string()),
        _ => None,
    }
}

pub(crate) fn clamp_u64(raw: Option<&Value>, min: u64, max: u64, fallback: u64) -> u64 {
    let Some(value) = raw.and_then(number_as_i64) else {
        return fallback;
    };

    let min_i64 = i64::try_from(min).unwrap_or(i64::MAX);
    let max_i64 = i64::try_from(max).unwrap_or(i64::MAX);
    value.clamp(min_i64, max_i64) as u64
}

pub(crate) fn clamp_usize(raw: Option<&Value>, min: usize, max: usize, fallback: usize) -> usize {
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
