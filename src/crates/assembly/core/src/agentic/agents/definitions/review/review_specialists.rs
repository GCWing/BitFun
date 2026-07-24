use crate::agentic::agents::AgentToolPolicyOverrides;
use crate::agentic::deep_review_policy::{REVIEW_JUDGE_AGENT_TYPE, REVIEW_WORKER_AGENT_TYPE};
use crate::agentic::tools::framework::ToolExposure;
use crate::define_readonly_subagent_with_overrides;

fn reviewer_tool_exposure_overrides() -> AgentToolPolicyOverrides {
    let mut overrides = AgentToolPolicyOverrides::default();
    overrides.insert("GetFileDiff".to_string(), ToolExposure::Direct);
    overrides
}

define_readonly_subagent_with_overrides!(
    ReviewWorkerAgent,
    REVIEW_WORKER_AGENT_TYPE,
    "Dynamic Review Worker",
    r#"Read-only Review worker for one bounded assignment. The owning Review agent supplies the concrete lens, question, scope, and evidence limits at launch time; this worker never selects its own broader role or target."#,
    "review_worker_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff"],
    reviewer_tool_exposure_overrides()
);

define_readonly_subagent_with_overrides!(
    ReviewJudgeAgent,
    REVIEW_JUDGE_AGENT_TYPE,
    "Review Quality Inspector",
    r#"Independent third-party arbiter that validates reviewer reports for logical consistency and evidence quality. It spot-checks specific code locations only when a claim needs verification, rather than re-reviewing the codebase from scratch."#,
    "review_quality_gate_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff"],
    reviewer_tool_exposure_overrides()
);

#[cfg(test)]
mod tests {
    use super::{ReviewJudgeAgent, ReviewWorkerAgent};
    use crate::agentic::agents::{Agent, UserContextPolicy};

    #[test]
    fn specialist_reviewers_use_workspace_context_and_instructions() {
        let agents: Vec<Box<dyn Agent>> = vec![
            Box::new(ReviewWorkerAgent::new()),
            Box::new(ReviewJudgeAgent::new()),
        ];

        for agent in agents {
            assert_eq!(
                agent.user_context_policy(),
                UserContextPolicy::empty()
                    .with_workspace_context()
                    .with_workspace_instructions()
            );
            assert!(agent.is_readonly());
            assert!(agent.default_tools().contains(&"GetFileDiff".to_string()));
            assert!(!agent.default_tools().contains(&"Git".to_string()));
        }
    }
}
