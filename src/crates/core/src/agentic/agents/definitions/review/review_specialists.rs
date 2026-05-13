use crate::agentic::deep_review_policy::{
    REVIEWER_ARCHITECTURE_AGENT_TYPE, REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_FRONTEND_AGENT_TYPE, REVIEWER_PERFORMANCE_AGENT_TYPE, REVIEWER_SECURITY_AGENT_TYPE,
    REVIEW_JUDGE_AGENT_TYPE,
};
use crate::agentic::agents::AgentToolPolicyOverrides;
use crate::agentic::tools::framework::ToolExposure;
use crate::define_readonly_subagent_with_overrides;

fn reviewer_tool_exposure_overrides() -> AgentToolPolicyOverrides {
    let mut overrides = AgentToolPolicyOverrides::default();
    overrides.insert("GetFileDiff".to_string(), ToolExposure::Expanded);
    overrides.insert("Git".to_string(), ToolExposure::Expanded);
    overrides
}

define_readonly_subagent_with_overrides!(
    BusinessLogicReviewerAgent,
    REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    "Business Logic Reviewer",
    r#"Independent read-only reviewer focused on workflow correctness, business rules, state transitions, data integrity, and edge-case handling in the review target. Use this when you need a fresh perspective on whether the change still does the right thing for real users."#,
    "review_business_logic_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff", "Git"],
    reviewer_tool_exposure_overrides()
);

define_readonly_subagent_with_overrides!(
    PerformanceReviewerAgent,
    REVIEWER_PERFORMANCE_AGENT_TYPE,
    "Performance Reviewer",
    r#"Independent read-only reviewer focused on latency, hot-path efficiency, unnecessary allocations, N+1 patterns, blocking calls, over-fetching, and scale-sensitive regressions introduced by the review target."#,
    "review_performance_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff", "Git"],
    reviewer_tool_exposure_overrides()
);

define_readonly_subagent_with_overrides!(
    SecurityReviewerAgent,
    REVIEWER_SECURITY_AGENT_TYPE,
    "Security Reviewer",
    r#"Independent read-only reviewer focused on security risks such as injection, auth gaps, data exposure, unsafe command/file handling, privilege escalation, and trust-boundary mistakes in the review target."#,
    "review_security_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff", "Git"],
    reviewer_tool_exposure_overrides()
);

define_readonly_subagent_with_overrides!(
    ArchitectureReviewerAgent,
    REVIEWER_ARCHITECTURE_AGENT_TYPE,
    "Architecture Reviewer",
    r#"Independent read-only reviewer focused on structural and architectural issues such as module boundary violations, API contract design, abstraction integrity, dependency direction, and cross-cutting concern impact in the review target."#,
    "review_architecture_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff", "Git"],
    reviewer_tool_exposure_overrides()
);

define_readonly_subagent_with_overrides!(
    FrontendReviewerAgent,
    REVIEWER_FRONTEND_AGENT_TYPE,
    "Frontend Reviewer",
    r#"Independent read-only reviewer focused on frontend-specific issues such as i18n key synchronization, frontend performance patterns (e.g., memoization, virtualization, effect/reactivity dependencies), accessibility, state management, frontend-backend API contract alignment, and platform boundary compliance in the review target."#,
    "review_frontend_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff", "Git"],
    reviewer_tool_exposure_overrides()
);

define_readonly_subagent_with_overrides!(
    ReviewJudgeAgent,
    REVIEW_JUDGE_AGENT_TYPE,
    "Review Quality Inspector",
    r#"Independent third-party arbiter that validates reviewer reports for logical consistency and evidence quality. It spot-checks specific code locations only when a claim needs verification, rather than re-reviewing the codebase from scratch."#,
    "review_quality_gate_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff", "Git"],
    reviewer_tool_exposure_overrides()
);

#[cfg(test)]
mod tests {
    use super::{
        ArchitectureReviewerAgent, BusinessLogicReviewerAgent, FrontendReviewerAgent,
        PerformanceReviewerAgent, ReviewJudgeAgent, SecurityReviewerAgent,
    };
    use crate::agentic::agents::{Agent, RequestContextPolicy};

    #[test]
    fn specialist_reviewers_use_isolated_instruction_context() {
        let agents: Vec<Box<dyn Agent>> = vec![
            Box::new(BusinessLogicReviewerAgent::new()),
            Box::new(PerformanceReviewerAgent::new()),
            Box::new(SecurityReviewerAgent::new()),
            Box::new(ArchitectureReviewerAgent::new()),
            Box::new(FrontendReviewerAgent::new()),
            Box::new(ReviewJudgeAgent::new()),
        ];

        for agent in agents {
            assert_eq!(
                agent.request_context_policy(),
                RequestContextPolicy::instructions_only()
            );
            assert!(agent.is_readonly());
            assert!(agent.default_tools().contains(&"GetFileDiff".to_string()));
        }
    }
}
