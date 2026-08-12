use crate::agentic::agents::AgentToolPolicyOverrides;
use crate::agentic::deep_review_policy::{REVIEW_JUDGE_AGENT_TYPE, REVIEW_WORKER_AGENT_TYPE};
use crate::agentic::tools::framework::ToolExposure;
use crate::define_readonly_subagent_with_overrides;

fn reviewer_tool_exposure_overrides() -> AgentToolPolicyOverrides {
    let mut overrides = AgentToolPolicyOverrides::default();
    overrides.insert("GetFileDiff".to_string(), ToolExposure::Direct);
    overrides
}

// 审查工具全家桶配齐：submit_code_review 提交审查结果，
// AskUserQuestion 向上级提判断问题（通用 subagent deny 列表明确保留
// AskUserQuestion），ReviewPlatform 访问宿主 PR/MR 平台。保持只读。
const REVIEWER_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "LS",
    "GetFileDiff",
    "submit_code_review",
    "ReviewPlatform",
    "AskUserQuestion",
];

define_readonly_subagent_with_overrides!(
    ReviewWorkerAgent,
    REVIEW_WORKER_AGENT_TYPE,
    "Dynamic Review Worker",
    r#"Read-only Review worker for one bounded assignment. The owning Review agent supplies the concrete lens, question, scope, and evidence limits at launch time; this worker never selects its own broader role or target."#,
    "review_worker_agent",
    REVIEWER_TOOLS,
    reviewer_tool_exposure_overrides()
);

define_readonly_subagent_with_overrides!(
    ReviewJudgeAgent,
    REVIEW_JUDGE_AGENT_TYPE,
    "Review Quality Inspector",
    r#"Independent third-party arbiter that validates reviewer reports for logical consistency and evidence quality. It spot-checks specific code locations only when a claim needs verification, rather than re-reviewing the codebase from scratch."#,
    "review_quality_gate_agent",
    REVIEWER_TOOLS,
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
            // 审查类智能体统一配齐 submit_code_review（severity 结构化提交）
            // + AskUserQuestion（向上级提判断问题）。
            assert!(
                agent.default_tools().contains(&"submit_code_review".to_string()),
                "specialist reviewer must include submit_code_review"
            );
            assert!(
                agent.default_tools().contains(&"AskUserQuestion".to_string()),
                "specialist reviewer must include AskUserQuestion"
            );
            assert!(
                agent.default_tools().contains(&"ReviewPlatform".to_string()),
                "specialist reviewer must include ReviewPlatform"
            );
        }
    }
}
