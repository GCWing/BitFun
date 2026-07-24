//! Default Deep Review team and strategy definitions.

use super::constants::{
    DEEP_REVIEW_AGENT_TYPE, DEFAULT_MAX_RETRIES_PER_ROLE, DEFAULT_MAX_SAME_ROLE_INSTANCES,
    DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD, LEGACY_REVIEW_WORKER_AGENT_TYPES,
    REVIEW_FIXER_AGENT_TYPE, REVIEW_JUDGE_AGENT_TYPE, REVIEW_WORKER_AGENT_TYPE,
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

fn role(
    key: &str,
    subagent_id: &str,
    fun_name: &str,
    role_name: &str,
    description: &str,
    responsibilities: &[&str],
    accent_color: &str,
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
        conditional: false,
    }
}

fn strategy_profile(
    level: &str,
    label: &str,
    summary: &str,
    token_impact: &str,
    runtime_impact: &str,
    default_model_slot: &str,
    prompt_directive: &str,
    worker_directive: &str,
    judge_directive: &str,
) -> ReviewStrategyManifestProfile {
    ReviewStrategyManifestProfile {
        level: level.to_string(),
        label: label.to_string(),
        summary: summary.to_string(),
        token_impact: token_impact.to_string(),
        runtime_impact: runtime_impact.to_string(),
        default_model_slot: default_model_slot.to_string(),
        prompt_directive: prompt_directive.to_string(),
        role_directives: BTreeMap::from([
            (
                REVIEW_WORKER_AGENT_TYPE.to_string(),
                worker_directive.to_string(),
            ),
            (
                REVIEW_JUDGE_AGENT_TYPE.to_string(),
                judge_directive.to_string(),
            ),
        ]),
    }
}

pub fn default_review_team_definition() -> ReviewTeamDefinition {
    let core_roles = vec![
        role(
            "worker",
            REVIEW_WORKER_AGENT_TYPE,
            "Focused Review",
            "On-demand Review Check",
            "A read-only check whose focus and scope are chosen for the current change when more evidence would be useful.",
            &[
                "Check only the question assigned by the main review.",
                "Stay within the selected scope and support conclusions with concrete evidence.",
                "Do not modify files or repeat work already completed by the main review.",
            ],
            "#3b82f6",
        ),
        role(
            "judge",
            REVIEW_JUDGE_AGENT_TYPE,
            "Independent Review Check",
            "Review Quality Check",
            "A read-only independent check used only when a serious finding, conflicting evidence, or an uncertain conclusion needs validation.",
            &[
                "Confirm or reject disputed findings using concrete evidence.",
                "Check only the claims that need independent validation.",
                "Make sure each retained issue has a safe, practical next step.",
            ],
            "#8b5cf6",
        ),
    ];

    let strategy_profiles = BTreeMap::from([
        (
            "quick".to_string(),
            strategy_profile(
                "quick",
                "Quick",
                "Quick keeps the main review concise and allows narrowly focused extra checks only when justified.",
                "0.4-0.6x",
                "0.5-0.7x",
                "fast",
                "Prefer a concise diff-focused pass. Report only high-confidence correctness, security, or regression risks and avoid speculative design rewrites.",
                "Answer only the supplied narrow question from direct diff evidence. Do not trace beyond one dependency hop.",
                "Confirm or reject the disputed finding efficiently; reject claims with thin evidence.",
            ),
        ),
        (
            "normal".to_string(),
            strategy_profile(
                "normal",
                "Normal",
                "Normal balances evidence depth with optional independent checks selected for the current change.",
                "1x",
                "1x",
                "fast",
                "Perform a practical evidence-backed review and stop investigating once each suspected issue is confirmed or dismissed.",
                "Apply the supplied lens to the changed path and its direct contracts. Report only realistic impact with concrete evidence.",
                "Validate each disputed finding and spot-check code only where its evidence needs verification.",
            ),
        ),
        (
            "deep".to_string(),
            strategy_profile(
                "deep",
                "Deep",
                "Deep gives the main review and any justified independent checks the longest bounded budget.",
                "1.8-2.5x",
                "1.5-2.5x",
                "primary",
                "Inspect edge cases, cross-file interactions, failure modes, and remediation tradeoffs before finalizing findings.",
                "Apply the supplied lens end-to-end within its exact scope, including relevant failure paths and cross-boundary contracts; do not broaden into unrelated review domains.",
                "Cross-check complex disputed findings and verify that both evidence and suggested remediation are safe.",
            ),
        ),
    ]);

    let hidden_agent_ids = vec![
        DEEP_REVIEW_AGENT_TYPE.to_string(),
        REVIEW_WORKER_AGENT_TYPE.to_string(),
        REVIEW_JUDGE_AGENT_TYPE.to_string(),
    ];
    let mut disallowed_extra_subagent_ids = hidden_agent_ids.clone();
    disallowed_extra_subagent_ids.push(REVIEW_FIXER_AGENT_TYPE.to_string());
    disallowed_extra_subagent_ids.extend(
        LEGACY_REVIEW_WORKER_AGENT_TYPES
            .iter()
            .map(|agent_type| agent_type.to_string()),
    );
    disallowed_extra_subagent_ids.sort();

    ReviewTeamDefinition {
        id: "default-review-team".to_string(),
        name: "Code Review".to_string(),
        description: "One main review that can request focused independent checks when more evidence is needed.".to_string(),
        warning: "Strict review may take longer and usually consumes more tokens than a standard review.".to_string(),
        default_model: "fast".to_string(),
        default_strategy_level: "normal".to_string(),
        default_execution_policy: ReviewTeamExecutionPolicyDefinition {
            reviewer_timeout_seconds: 3600,
            judge_timeout_seconds: 2400,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_team_exposes_one_dynamic_worker_and_the_conditional_judge() {
        let definition = default_review_team_definition();
        assert_eq!(
            definition
                .core_roles
                .iter()
                .map(|role| (role.key.as_str(), role.subagent_id.as_str()))
                .collect::<Vec<_>>(),
            [
                ("worker", REVIEW_WORKER_AGENT_TYPE),
                ("judge", REVIEW_JUDGE_AGENT_TYPE)
            ]
        );
        assert!(definition
            .strategy_profiles
            .values()
            .all(|profile| profile.role_directives.len() == 2));
    }

    #[test]
    fn default_team_uses_readable_user_facing_copy() {
        let definition = default_review_team_definition();
        let worker = &definition.core_roles[0];
        let judge = &definition.core_roles[1];

        assert_eq!(worker.fun_name, "Focused Review");
        assert_eq!(worker.role_name, "On-demand Review Check");
        assert_eq!(judge.fun_name, "Independent Review Check");
        assert_eq!(judge.role_name, "Review Quality Check");
        assert_eq!(
            definition.description,
            "One main review that can request focused independent checks when more evidence is needed."
        );

        let user_facing_copy = definition
            .strategy_profiles
            .values()
            .map(|profile| profile.summary.as_str())
            .chain([worker.description.as_str(), judge.description.as_str()])
            .collect::<Vec<_>>()
            .join("\n")
            .to_ascii_lowercase();
        for implementation_term in ["worker", "lens", "specialist", "inspector"] {
            assert!(
                !user_facing_copy.contains(implementation_term),
                "user-facing copy should not contain {implementation_term}"
            );
        }
        assert!(!user_facing_copy.contains("one optional"));
        assert!(!user_facing_copy.contains("one justified"));
        assert!(!user_facing_copy.contains("one narrowly focused"));
    }

    #[test]
    fn serialized_default_team_keeps_the_frontend_fallback_contract() {
        let value = serde_json::to_value(default_review_team_definition())
            .expect("default team should serialize");

        assert_eq!(value["name"], "Code Review");
        assert_eq!(
            value["description"],
            "One main review that can request focused independent checks when more evidence is needed."
        );
        assert_eq!(value["coreRoles"][0]["subagentId"], "ReviewWorker");
        assert_eq!(value["coreRoles"][0]["accentColor"], "#3b82f6");
        assert_eq!(value["coreRoles"][1]["subagentId"], "ReviewJudge");
        assert_eq!(value["coreRoles"][1]["accentColor"], "#8b5cf6");
        assert_eq!(value["strategyProfiles"]["normal"]["label"], "Normal");
        assert_eq!(value["strategyProfiles"]["deep"]["label"], "Deep");
        assert_eq!(
            value["hiddenAgentIds"],
            serde_json::json!(["DeepReview", "ReviewWorker", "ReviewJudge"])
        );
        assert_eq!(
            value["disallowedExtraSubagentIds"],
            serde_json::json!([
                "DeepReview",
                "ReviewArchitecture",
                "ReviewBusinessLogic",
                "ReviewFixer",
                "ReviewFrontend",
                "ReviewGeneral",
                "ReviewJudge",
                "ReviewPerformance",
                "ReviewSecurity",
                "ReviewWorker"
            ])
        );
    }
}
