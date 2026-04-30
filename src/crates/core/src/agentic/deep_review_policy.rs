use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use dashmap::DashMap;
use log::warn;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

pub const DEEP_REVIEW_AGENT_TYPE: &str = "DeepReview";
pub const REVIEW_JUDGE_AGENT_TYPE: &str = "ReviewJudge";
pub const REVIEW_FIXER_AGENT_TYPE: &str = "ReviewFixer";
pub const REVIEWER_BUSINESS_LOGIC_AGENT_TYPE: &str = "ReviewBusinessLogic";
pub const REVIEWER_PERFORMANCE_AGENT_TYPE: &str = "ReviewPerformance";
pub const REVIEWER_SECURITY_AGENT_TYPE: &str = "ReviewSecurity";
pub const REVIEWER_ARCHITECTURE_AGENT_TYPE: &str = "ReviewArchitecture";
pub const REVIEWER_FRONTEND_AGENT_TYPE: &str = "ReviewFrontend";
pub const CORE_REVIEWER_AGENT_TYPES: [&str; 4] = [
    REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_PERFORMANCE_AGENT_TYPE,
    REVIEWER_SECURITY_AGENT_TYPE,
    REVIEWER_ARCHITECTURE_AGENT_TYPE,
];
pub const CONDITIONAL_REVIEWER_AGENT_TYPES: [&str; 1] = [REVIEWER_FRONTEND_AGENT_TYPE];
const DEFAULT_REVIEW_TEAM_CONFIG_PATH: &str = "ai.review_teams.default";

const DEFAULT_REVIEWER_TIMEOUT_SECONDS: u64 = 600;
const DEFAULT_JUDGE_TIMEOUT_SECONDS: u64 = 600;
const MAX_TIMEOUT_SECONDS: u64 = 3600;
const BASE_TIMEOUT_QUICK_SECONDS: u64 = 180;
const BASE_TIMEOUT_NORMAL_SECONDS: u64 = 300;
const BASE_TIMEOUT_DEEP_SECONDS: u64 = 600;
const TIMEOUT_PER_FILE_SECONDS: u64 = 15;
const TIMEOUT_PER_100_LINES_SECONDS: u64 = 30;
const DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD: usize = 20;
const DEFAULT_MAX_SAME_ROLE_INSTANCES: usize = 3;
const MAX_SAME_ROLE_INSTANCES: usize = 8;
const DEFAULT_MAX_RETRIES_PER_ROLE: usize = 1;
const MAX_RETRIES_PER_ROLE: usize = 3;
const DEFAULT_MAX_PARALLEL_INSTANCES: usize = 4;
const BUDGET_TTL: Duration = Duration::from_secs(60 * 60);
const PRUNE_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTeamRoleDefinition {
    pub key: String,
    pub subagent_id: String,
    pub fun_name: String,
    pub role_name: String,
    pub description: String,
    pub responsibilities: Vec<String>,
    pub accent_color: String,
    pub conditional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStrategyManifestProfile {
    pub level: String,
    pub label: String,
    pub summary: String,
    pub token_impact: String,
    pub runtime_impact: String,
    pub default_model_slot: String,
    pub prompt_directive: String,
    pub role_directives: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTeamExecutionPolicyDefinition {
    pub reviewer_timeout_seconds: u64,
    pub judge_timeout_seconds: u64,
    pub reviewer_file_split_threshold: usize,
    pub max_same_role_instances: usize,
    pub max_retries_per_role: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTeamDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub warning: String,
    pub default_model: String,
    pub default_strategy_level: String,
    pub default_execution_policy: ReviewTeamExecutionPolicyDefinition,
    pub core_roles: Vec<ReviewTeamRoleDefinition>,
    pub strategy_profiles: BTreeMap<String, ReviewStrategyManifestProfile>,
    pub disallowed_extra_subagent_ids: Vec<String>,
    pub hidden_agent_ids: Vec<String>,
}

fn review_role(
    key: &str,
    subagent_id: &str,
    fun_name: &str,
    role_name: &str,
    description: &str,
    responsibilities: &[&str],
    accent_color: &str,
    conditional: bool,
) -> ReviewTeamRoleDefinition {
    ReviewTeamRoleDefinition {
        key: key.to_string(),
        subagent_id: subagent_id.to_string(),
        fun_name: fun_name.to_string(),
        role_name: role_name.to_string(),
        description: description.to_string(),
        responsibilities: responsibilities
            .iter()
            .map(|item| item.to_string())
            .collect(),
        accent_color: accent_color.to_string(),
        conditional,
    }
}

fn role_directives(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(role, directive)| (role.to_string(), directive.to_string()))
        .collect()
}

fn strategy_profile(
    level: &str,
    label: &str,
    summary: &str,
    token_impact: &str,
    runtime_impact: &str,
    default_model_slot: &str,
    prompt_directive: &str,
    directives: &[(&str, &str)],
) -> ReviewStrategyManifestProfile {
    ReviewStrategyManifestProfile {
        level: level.to_string(),
        label: label.to_string(),
        summary: summary.to_string(),
        token_impact: token_impact.to_string(),
        runtime_impact: runtime_impact.to_string(),
        default_model_slot: default_model_slot.to_string(),
        prompt_directive: prompt_directive.to_string(),
        role_directives: role_directives(directives),
    }
}

pub fn default_review_team_definition() -> ReviewTeamDefinition {
    let core_roles = vec![
        review_role(
            "businessLogic",
            REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
            "Logic Reviewer",
            "Business Logic Reviewer",
            "A workflow sleuth that inspects business rules, state transitions, recovery paths, and real-user correctness.",
            &[
                "Verify workflows, state transitions, and domain rules still behave correctly.",
                "Check boundary cases, rollback paths, and data integrity assumptions.",
                "Focus on issues that can break user outcomes or product intent.",
            ],
            "#2563eb",
            false,
        ),
        review_role(
            "performance",
            REVIEWER_PERFORMANCE_AGENT_TYPE,
            "Performance Reviewer",
            "Performance Reviewer",
            "A speed-focused profiler that hunts hot paths, unnecessary work, blocking calls, and scale-sensitive regressions.",
            &[
                "Inspect hot paths, large loops, and unnecessary allocations or recomputation.",
                "Flag blocking work, N+1 patterns, and wasteful data movement.",
                "Keep performance advice practical and aligned with the existing architecture.",
            ],
            "#d97706",
            false,
        ),
        review_role(
            "security",
            REVIEWER_SECURITY_AGENT_TYPE,
            "Security Reviewer",
            "Security Reviewer",
            "A boundary guardian that scans for injection risks, trust leaks, privilege mistakes, and unsafe file or command handling.",
            &[
                "Review trust boundaries, auth assumptions, and sensitive data handling.",
                "Look for injection, unsafe command execution, and exposure risks.",
                "Highlight concrete fixes that reduce risk without broad rewrites.",
            ],
            "#dc2626",
            false,
        ),
        review_role(
            "architecture",
            REVIEWER_ARCHITECTURE_AGENT_TYPE,
            "Architecture Reviewer",
            "Architecture Reviewer",
            "A structural watchdog that checks module boundaries, dependency direction, API contract design, and abstraction integrity.",
            &[
                "Detect layer boundary violations and wrong-direction imports.",
                "Verify API contracts, tool schemas, and transport messages stay consistent.",
                "Ensure platform-agnostic code does not leak platform-specific details.",
            ],
            "#0891b2",
            false,
        ),
        review_role(
            "frontend",
            REVIEWER_FRONTEND_AGENT_TYPE,
            "Frontend Reviewer",
            "Frontend Reviewer",
            "A UI specialist that checks i18n synchronization, React performance patterns, accessibility, and frontend-backend contract alignment.",
            &[
                "Verify i18n key completeness across all locales.",
                "Check React performance patterns (memoization, virtualization, effect dependencies).",
                "Flag accessibility violations and frontend-backend API contract drift.",
            ],
            "#059669",
            true,
        ),
        review_role(
            "judge",
            REVIEW_JUDGE_AGENT_TYPE,
            "Review Arbiter",
            "Review Quality Inspector",
            "An independent third-party arbiter that validates reviewer reports for logical consistency and evidence quality. It spot-checks specific code locations only when a claim needs verification, rather than re-reviewing the codebase from scratch.",
            &[
                "Validate, merge, downgrade, or reject reviewer findings based on logical consistency and evidence quality.",
                "Filter out false positives and directionally-wrong optimization advice by examining reviewer reasoning.",
                "Spot-check specific code locations only when a reviewer claim needs verification.",
                "Ensure every surviving issue has an actionable fix or follow-up plan.",
            ],
            "#7c3aed",
            false,
        ),
    ];

    let strategy_profiles = BTreeMap::from([
        (
            "quick".to_string(),
            strategy_profile(
                "quick",
                "Quick",
                "Fast screening for high-confidence issues in the requested diff or scope.",
                "0.4-0.6x",
                "0.5-0.7x",
                "fast",
                "Prefer a concise diff-focused pass. Report only high-confidence correctness, security, or regression risks and avoid speculative design rewrites.",
                &[
                    (
                        REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
                        "Only trace logic paths directly changed by the diff. Do not follow call chains beyond one hop. Report only issues where the diff introduces a provably wrong behavior.",
                    ),
                    (
                        REVIEWER_PERFORMANCE_AGENT_TYPE,
                        "Scan the diff for known anti-patterns only: nested loops, repeated fetches, blocking calls on hot paths, unnecessary re-renders. Do not trace call chains or estimate impact beyond what the diff shows.",
                    ),
                    (
                        REVIEWER_SECURITY_AGENT_TYPE,
                        "Scan the diff for direct security risks only: injection, secret exposure, unsafe commands, missing auth. Do not trace data flows beyond one hop.",
                    ),
                    (
                        REVIEWER_ARCHITECTURE_AGENT_TYPE,
                        "Only check imports directly changed by the diff. Flag violations of documented layer boundaries.",
                    ),
                    (
                        REVIEWER_FRONTEND_AGENT_TYPE,
                        "Only check i18n key completeness and direct platform boundary violations in changed frontend files.",
                    ),
                    (
                        REVIEW_JUDGE_AGENT_TYPE,
                        "This was a quick review. Focus on confirming or rejecting each finding efficiently. If a finding's evidence is thin, reject it rather than spending time verifying.",
                    ),
                ],
            ),
        ),
        (
            "normal".to_string(),
            strategy_profile(
                "normal",
                "Normal",
                "Balanced review depth for day-to-day code review with practical evidence.",
                "1x",
                "1x",
                "fast",
                "Perform the standard role-specific review. Balance coverage with precision and include concrete evidence for each issue.",
                &[
                    (
                        REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
                        "Trace each changed function's direct callers and callees to verify business rules and state transitions. Stop investigating a path once you have enough evidence to confirm or dismiss it.",
                    ),
                    (
                        REVIEWER_PERFORMANCE_AGENT_TYPE,
                        "Inspect the diff for anti-patterns, then read surrounding code to confirm impact on hot paths. Report only issues likely to matter at realistic scale.",
                    ),
                    (
                        REVIEWER_SECURITY_AGENT_TYPE,
                        "Trace each changed input path from entry point to usage. Check trust boundaries, auth assumptions, and data sanitization. Report only issues with a realistic threat narrative.",
                    ),
                    (
                        REVIEWER_ARCHITECTURE_AGENT_TYPE,
                        "Check the diff's imports plus one level of dependency direction. Verify API contract consistency.",
                    ),
                    (
                        REVIEWER_FRONTEND_AGENT_TYPE,
                        "Check i18n, React performance patterns, and accessibility in changed components. Verify frontend-backend API contract alignment.",
                    ),
                    (
                        REVIEW_JUDGE_AGENT_TYPE,
                        "Validate each finding's logical consistency and evidence quality. Spot-check code only when a claim needs verification.",
                    ),
                ],
            ),
        ),
        (
            "deep".to_string(),
            strategy_profile(
                "deep",
                "Deep",
                "Thorough multi-pass review for risky, broad, or release-sensitive changes.",
                "1.8-2.5x",
                "1.5-2.5x",
                "primary",
                "Run a thorough role-specific pass. Inspect edge cases, cross-file interactions, failure modes, and remediation tradeoffs before finalizing findings.",
                &[
                    (
                        REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
                        "Map full call chains for changed functions. Verify state transitions end-to-end, check rollback and error-recovery paths, and test edge cases in data shape and lifecycle assumptions. Prioritize findings by user-facing impact.",
                    ),
                    (
                        REVIEWER_PERFORMANCE_AGENT_TYPE,
                        "In addition to the normal pass, check for latent scaling risks - data structures that degrade at volume, or algorithms that are correct but unnecessarily expensive. Only report if you can estimate the impact. Do not speculate about edge cases or failure modes unrelated to performance.",
                    ),
                    (
                        REVIEWER_SECURITY_AGENT_TYPE,
                        "In addition to the normal pass, trace data flows across trust boundaries end-to-end. Check for privilege escalation chains, indirect injection vectors, and failure modes that expose sensitive data. Report only issues with a complete threat narrative.",
                    ),
                    (
                        REVIEWER_ARCHITECTURE_AGENT_TYPE,
                        "Map the full dependency graph for changed modules. Check for structural anti-patterns, circular dependencies, and cross-cutting concerns.",
                    ),
                    (
                        REVIEWER_FRONTEND_AGENT_TYPE,
                        "Thorough React analysis: effect dependencies, memoization, virtualization. Full accessibility audit. State management pattern review. Cross-layer contract verification.",
                    ),
                    (
                        REVIEW_JUDGE_AGENT_TYPE,
                        "This was a deep review with potentially complex findings. Cross-validate findings across reviewers for consistency. For each finding, verify the evidence supports the conclusion and the suggested fix is safe. Pay extra attention to overlapping findings across reviewers or same-role instances.",
                    ),
                ],
            ),
        ),
    ]);

    let mut hidden_agent_ids = vec![
        DEEP_REVIEW_AGENT_TYPE.to_string(),
        REVIEW_JUDGE_AGENT_TYPE.to_string(),
    ];
    hidden_agent_ids.extend(CORE_REVIEWER_AGENT_TYPES.iter().map(|id| id.to_string()));
    hidden_agent_ids.extend(
        CONDITIONAL_REVIEWER_AGENT_TYPES
            .iter()
            .map(|id| id.to_string()),
    );
    hidden_agent_ids.sort();
    hidden_agent_ids.dedup();

    let mut disallowed_extra_subagent_ids = hidden_agent_ids.clone();
    disallowed_extra_subagent_ids.push(REVIEW_FIXER_AGENT_TYPE.to_string());
    disallowed_extra_subagent_ids.sort();
    disallowed_extra_subagent_ids.dedup();

    ReviewTeamDefinition {
        id: "default-review-team".to_string(),
        name: "Code Review Team".to_string(),
        description: "A multi-reviewer team for deep code review with mandatory logic, performance, security, architecture, conditional frontend, and quality-gate roles.".to_string(),
        warning: "Deep review may take longer and usually consumes more tokens than a standard review.".to_string(),
        default_model: "fast".to_string(),
        default_strategy_level: "normal".to_string(),
        default_execution_policy: ReviewTeamExecutionPolicyDefinition {
            reviewer_timeout_seconds: 300,
            judge_timeout_seconds: 240,
            reviewer_file_split_threshold: DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD,
            max_same_role_instances: DEFAULT_MAX_SAME_ROLE_INSTANCES,
            max_retries_per_role: DEFAULT_MAX_RETRIES_PER_ROLE,
        },
        core_roles,
        strategy_profiles,
        disallowed_extra_subagent_ids,
        hidden_agent_ids,
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewRunManifestGate {
    active_subagent_ids: HashSet<String>,
    skipped_subagent_reasons: HashMap<String, String>,
}

impl DeepReviewRunManifestGate {
    pub fn from_value(raw: &Value) -> Option<Self> {
        let manifest = raw.as_object()?;
        if manifest.get("reviewMode").and_then(Value::as_str) != Some("deep") {
            return None;
        }

        let mut active_subagent_ids = HashSet::new();
        collect_manifest_members(manifest.get("workPackets"), &mut active_subagent_ids);
        collect_manifest_members(manifest.get("coreReviewers"), &mut active_subagent_ids);
        collect_manifest_members(
            manifest.get("enabledExtraReviewers"),
            &mut active_subagent_ids,
        );
        if let Some(id) = manifest
            .get("qualityGateReviewer")
            .and_then(manifest_member_subagent_id)
        {
            active_subagent_ids.insert(id);
        }

        if active_subagent_ids.is_empty() {
            return None;
        }

        let mut skipped_subagent_reasons = HashMap::new();
        if let Some(skipped) = manifest.get("skippedReviewers").and_then(Value::as_array) {
            for member in skipped {
                let Some(id) = manifest_member_subagent_id(member) else {
                    continue;
                };
                let reason = member
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("skipped")
                    .trim();
                skipped_subagent_reasons.insert(
                    id,
                    if reason.is_empty() {
                        "skipped".to_string()
                    } else {
                        reason.to_string()
                    },
                );
            }
        }

        Some(Self {
            active_subagent_ids,
            skipped_subagent_reasons,
        })
    }

    pub fn ensure_active(&self, subagent_type: &str) -> Result<(), DeepReviewPolicyViolation> {
        if self.active_subagent_ids.contains(subagent_type) {
            return Ok(());
        }

        let reason = self
            .skipped_subagent_reasons
            .get(subagent_type)
            .map(String::as_str)
            .unwrap_or("missing_from_manifest");

        Err(DeepReviewPolicyViolation::new(
            "deep_review_subagent_not_active_for_target",
            format!(
                "DeepReview subagent '{}' is not active for this review target (reason: {})",
                subagent_type, reason
            ),
        ))
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
                MAX_SAME_ROLE_INSTANCES,
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

    /// Extract the concurrency policy from a run manifest, if present.
    pub fn concurrency_policy_from_manifest(&self, raw_manifest: &Value) -> DeepReviewConcurrencyPolicy {
        raw_manifest
            .get("concurrencyPolicy")
            .map(DeepReviewConcurrencyPolicy::from_manifest)
            .unwrap_or_default()
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
                    risk.file_count,
                    risk.total_lines_changed,
                    risk.files_in_security_paths
                ),
            ),
        }
    }
}

/// Dynamic concurrency control for deep review reviewer launches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewConcurrencyPolicy {
    /// Maximum parallel reviewer instances at once.
    pub max_parallel_instances: usize,
    /// Whether to stagger launches (wait N seconds between batches).
    pub stagger_seconds: u64,
    /// Whether to batch extras separately from core reviewers.
    pub batch_extras_separately: bool,
}

impl Default for DeepReviewConcurrencyPolicy {
    fn default() -> Self {
        Self {
            max_parallel_instances: DEFAULT_MAX_PARALLEL_INSTANCES,
            stagger_seconds: 0,
            batch_extras_separately: true,
        }
    }
}

impl DeepReviewConcurrencyPolicy {
    pub fn from_manifest(raw: &Value) -> Self {
        let Some(obj) = raw.as_object() else {
            return Self::default();
        };

        Self {
            max_parallel_instances: clamp_usize(
                obj.get("maxParallelInstances"),
                1,
                16,
                DEFAULT_MAX_PARALLEL_INSTANCES,
            ),
            stagger_seconds: clamp_u64(
                obj.get("staggerSeconds"),
                0,
                60,
                0,
            ),
            batch_extras_separately: obj
                .get("batchExtrasSeparately")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }

    /// Compute the effective max same-role instances, capped by both
    /// the execution policy's `max_same_role_instances` and the
    /// concurrency policy's `max_parallel_instances / role_count`.
    pub fn effective_max_same_role_instances(
        &self,
        policy: &DeepReviewExecutionPolicy,
    ) -> usize {
        let role_count = reviewer_agent_type_count() + policy.extra_subagent_ids.len();
        let max_per_role = self.max_parallel_instances / role_count.max(1);
        max_per_role.max(1).min(policy.max_same_role_instances)
    }

    /// Check whether the current number of active launches exceeds the cap.
    /// Returns `Ok(())` if the launch is allowed, or an error describing why not.
    pub fn check_launch_allowed(
        &self,
        active_count: usize,
        role: DeepReviewSubagentRole,
        is_judge_pending: bool,
    ) -> Result<(), DeepReviewPolicyViolation> {
        match role {
            DeepReviewSubagentRole::Reviewer => {
                if active_count >= self.max_parallel_instances {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_concurrency_cap_reached",
                        format!(
                            "Maximum parallel reviewer instances reached ({}/{}). Wait for running reviewers to complete before launching more.",
                            active_count, self.max_parallel_instances
                        ),
                    ));
                }
            }
            DeepReviewSubagentRole::Judge => {
                if active_count > 0 {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_launch_blocked_by_reviewers",
                        format!(
                            "ReviewJudge cannot launch while {} reviewer(s) are still active. Wait for reviewers to complete first.",
                            active_count
                        ),
                    ));
                }
                if is_judge_pending {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_already_pending",
                        "ReviewJudge is already pending or running in this turn.",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DeepReviewTurnBudget {
    judge_calls: usize,
    /// Tracks total reviewer calls (across all roles) per turn.
    /// Capped by `max_same_role_instances * reviewer_agent_type_count() +
    /// extra_subagent_ids.len()` so the orchestrator cannot spawn an unbounded
    /// number of same-role instances.
    reviewer_calls: usize,
    reviewer_calls_by_subagent: HashMap<String, usize>,
    retries_used_by_subagent: HashMap<String, usize>,
    updated_at: Instant,
}

impl DeepReviewTurnBudget {
    fn new(now: Instant) -> Self {
        Self {
            judge_calls: 0,
            reviewer_calls: 0,
            reviewer_calls_by_subagent: HashMap::new(),
            retries_used_by_subagent: HashMap::new(),
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
        subagent_type: &str,
        is_retry: bool,
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
                let subagent_type = normalize_budget_subagent_type(subagent_type)?;
                if is_retry {
                    if policy.max_retries_per_role == 0 {
                        return Err(DeepReviewPolicyViolation::new(
                            "deep_review_retry_budget_exhausted",
                            format!(
                                "Retry budget is disabled for DeepReview reviewer '{}'",
                                subagent_type
                            ),
                        ));
                    }
                    if !budget
                        .reviewer_calls_by_subagent
                        .contains_key(subagent_type.as_str())
                    {
                        return Err(DeepReviewPolicyViolation::new(
                            "deep_review_retry_without_initial_attempt",
                            format!(
                                "Cannot retry DeepReview reviewer '{}' before an initial attempt in this turn",
                                subagent_type
                            ),
                        ));
                    }
                    let retry_count = budget
                        .retries_used_by_subagent
                        .entry(subagent_type.clone())
                        .or_insert(0);
                    if *retry_count >= policy.max_retries_per_role {
                        return Err(DeepReviewPolicyViolation::new(
                            "deep_review_retry_budget_exhausted",
                            format!(
                                "Retry budget exhausted for DeepReview reviewer '{}' (max retries: {})",
                                subagent_type, policy.max_retries_per_role
                            ),
                        ));
                    }
                    *retry_count += 1;
                    budget.updated_at = now;
                    return Ok(());
                }

                let max_reviewer_calls = policy.max_same_role_instances
                    * (reviewer_agent_type_count() + policy.extra_subagent_ids.len());
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
                *budget
                    .reviewer_calls_by_subagent
                    .entry(subagent_type)
                    .or_insert(0) += 1;
            }
            DeepReviewSubagentRole::Judge => {
                if is_retry {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_retry_disallowed",
                        "ReviewJudge retry is not covered by the reviewer retry budget",
                    ));
                }
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

    /// Returns the number of reviewer calls recorded for a given turn.
    /// Used by the concurrency enforcement to check if a new launch is allowed.
    pub fn active_reviewer_count(&self, parent_dialog_turn_id: &str) -> usize {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| budget.reviewer_calls)
            .unwrap_or(0)
    }

    /// Returns true if a judge call has been recorded for a given turn.
    pub fn has_judge_been_launched(&self, parent_dialog_turn_id: &str) -> bool {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| budget.judge_calls > 0)
            .unwrap_or(false)
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

/// Returns the number of active reviewer calls for a given turn.
pub fn deep_review_active_reviewer_count(parent_dialog_turn_id: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.active_reviewer_count(parent_dialog_turn_id)
}

/// Returns true if a judge has been launched for a given turn.
pub fn deep_review_has_judge_been_launched(parent_dialog_turn_id: &str) -> bool {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.has_judge_been_launched(parent_dialog_turn_id)
}

/// Returns the number of retries used for a specific subagent type in a given turn.
pub fn deep_review_retries_used(parent_dialog_turn_id: &str, subagent_type: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .turns
        .get(parent_dialog_turn_id)
        .map(|budget| {
            budget
                .retries_used_by_subagent
                .get(subagent_type)
                .copied()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// Returns the configured max retries per role for the given turn.
/// Returns 0 if no budget tracking exists for the turn.
pub fn deep_review_max_retries_per_role(_parent_dialog_turn_id: &str) -> usize {
    // Use the compile-time default. The runtime policy can override via
    // run manifest, but this function is called from a non-async context
    // (task_tool result formatting) where we cannot await the async loader.
    // The default value matches DEFAULT_MAX_RETRIES_PER_ROLE.
    DEFAULT_MAX_RETRIES_PER_ROLE
}

fn collect_manifest_members(raw: Option<&Value>, output: &mut HashSet<String>) {
    let Some(values) = raw.and_then(Value::as_array) else {
        return;
    };

    for member in values {
        if let Some(id) = manifest_member_subagent_id(member) {
            output.insert(id);
        }
    }
}

fn manifest_member_subagent_id(value: &Value) -> Option<String> {
    let id = value
        .get("subagentId")
        .or_else(|| value.get("subagent_id"))
        .and_then(Value::as_str)?
        .trim();
    (!id.is_empty()).then(|| id.to_string())
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

fn reviewer_agent_type_count() -> usize {
    CORE_REVIEWER_AGENT_TYPES.len() + CONDITIONAL_REVIEWER_AGENT_TYPES.len()
}

fn normalize_budget_subagent_type(
    subagent_type: &str,
) -> Result<String, DeepReviewPolicyViolation> {
    let normalized = subagent_type.trim();
    if normalized.is_empty() {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_subagent_type_missing",
            "DeepReview task budget requires a non-empty subagent type",
        ));
    }

    Ok(normalized.to_string())
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

/// Incremental review cache stores completed reviewer outputs keyed by packet_id.
/// When a deep review is re-run with the same target fingerprint, cached outputs
/// are reused instead of re-dispatching reviewers.
pub struct DeepReviewIncrementalCache {
    fingerprint: String,
    packets: HashMap<String, String>,
}

impl DeepReviewIncrementalCache {
    pub fn new(fingerprint: &str) -> Self {
        Self {
            fingerprint: fingerprint.to_string(),
            packets: HashMap::new(),
        }
    }

    pub fn from_value(value: &Value) -> Self {
        let obj = value.as_object();
        let fingerprint = obj
            .and_then(|o| o.get("fingerprint"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let packets = obj
            .and_then(|o| o.get("packets"))
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            fingerprint,
            packets,
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "fingerprint": self.fingerprint,
            "packets": self.packets,
        })
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn store_packet(&mut self, packet_id: &str, output: &str) {
        self.packets
            .insert(packet_id.to_string(), output.to_string());
    }

    pub fn get_packet(&self, packet_id: &str) -> Option<&str> {
        self.packets.get(packet_id).map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Check if the cached fingerprint matches the fingerprint in the run manifest.
    /// Returns false if the manifest has no incrementalReviewCache section.
    pub fn matches_manifest(&self, manifest: &Value) -> bool {
        manifest
            .get("incrementalReviewCache")
            .and_then(|ic| ic.get("fingerprint"))
            .and_then(Value::as_str)
            .map(|fp| fp == self.fingerprint)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_missing_default_review_team_config_error, DeepReviewBudgetTracker,
        DeepReviewExecutionPolicy, DeepReviewIncrementalCache, DeepReviewRunManifestGate,
        DeepReviewStrategyLevel, DeepReviewSubagentRole, REVIEW_FIXER_AGENT_TYPE,
        REVIEW_JUDGE_AGENT_TYPE,
    };
    use serde_json::Value;
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

        // Value above max (8) should be clamped to 8
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "max_same_role_instances": 100
        })));
        assert_eq!(policy.max_same_role_instances, 8);
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
        // 4 files + 0 lines/100 + 2 security * 3 = 10 → Normal
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
            "batchExtrasSeparately": false
        });
        let policy = super::DeepReviewConcurrencyPolicy::from_manifest(&raw);
        assert_eq!(policy.max_parallel_instances, 6);
        assert_eq!(policy.stagger_seconds, 5);
        assert!(!policy.batch_extras_separately);
    }

    #[test]
    fn concurrency_effective_max_same_role_instances() {
        let exec_policy = DeepReviewExecutionPolicy::default();
        let conc_policy = super::DeepReviewConcurrencyPolicy {
            max_parallel_instances: 4,
            stagger_seconds: 0,
            batch_extras_separately: true,
        };
        // 5 reviewer types (4 core + 1 conditional), 4 / 5 = 0 → clamped to 1
        assert_eq!(conc_policy.effective_max_same_role_instances(&exec_policy), 1);

        let conc_policy_12 = super::DeepReviewConcurrencyPolicy {
            max_parallel_instances: 12,
            stagger_seconds: 0,
            batch_extras_separately: true,
        };
        // 12 / 5 = 2, capped by default max_same_role_instances (3) → 2
        assert_eq!(conc_policy_12.effective_max_same_role_instances(&exec_policy), 2);
    }

    #[test]
    fn concurrency_check_launch_allowed() {
        let policy = super::DeepReviewConcurrencyPolicy::default();
        // 0 active reviewers → reviewer allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Reviewer, false)
            .is_ok());
        // 4 active reviewers (at cap) → reviewer blocked
        let err = policy
            .check_launch_allowed(4, DeepReviewSubagentRole::Reviewer, false)
            .unwrap_err();
        assert_eq!(err.code, "deep_review_concurrency_cap_reached");
        // 1 active reviewer → judge blocked
        let err = policy
            .check_launch_allowed(1, DeepReviewSubagentRole::Judge, false)
            .unwrap_err();
        assert_eq!(err.code, "deep_review_judge_launch_blocked_by_reviewers");
        // 0 active reviewers, judge not pending → judge allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Judge, false)
            .is_ok());
        // 0 active reviewers, judge pending → blocked
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
                "staggerSeconds": 10
            }
        });
        let conc = policy.concurrency_policy_from_manifest(&manifest);
        assert_eq!(conc.max_parallel_instances, 3);
        assert_eq!(conc.stagger_seconds, 10);
        assert!(conc.batch_extras_separately);
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
    fn incremental_cache_from_null_value() {
        let cache = DeepReviewIncrementalCache::from_value(&Value::Null);
        assert!(cache.is_empty());
        assert_eq!(cache.fingerprint(), "");
    }
}
