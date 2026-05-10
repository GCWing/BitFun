//! Default Deep Review team and reviewer strategy definitions.

use super::constants::{
    CONDITIONAL_REVIEWER_AGENT_TYPES, CORE_REVIEWER_AGENT_TYPES, DEEP_REVIEW_AGENT_TYPE,
    DEFAULT_MAX_RETRIES_PER_ROLE, DEFAULT_MAX_SAME_ROLE_INSTANCES,
    DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD, REVIEWER_ARCHITECTURE_AGENT_TYPE,
    REVIEWER_BUSINESS_LOGIC_AGENT_TYPE, REVIEWER_FRONTEND_AGENT_TYPE,
    REVIEWER_PERFORMANCE_AGENT_TYPE, REVIEWER_SECURITY_AGENT_TYPE, REVIEW_FIXER_AGENT_TYPE,
    REVIEW_JUDGE_AGENT_TYPE,
};
use serde::Serialize;
use std::collections::BTreeMap;

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
