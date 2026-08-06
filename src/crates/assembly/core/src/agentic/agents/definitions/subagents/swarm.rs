use crate::agentic::agents::{Agent, AgentToolPolicyOverrides};
use crate::agentic::tools::framework::ToolExposure;
use crate::define_readonly_subagent_with_overrides;
use async_trait::async_trait;

pub struct SwarmPlannerAgent;

impl Default for SwarmPlannerAgent {
    fn default() -> Self {
        Self::new()
    }
}
impl SwarmPlannerAgent {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Agent for SwarmPlannerAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn id(&self) -> &str {
        "SwarmPlanner"
    }
    fn name(&self) -> &str {
        "Swarm Planner"
    }
    fn description(&self) -> &str {
        "Recursive planning agent that investigates scope and coordinates Swarm workers."
    }
    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "swarm_planner_agent"
    }
    fn default_tools(&self) -> Vec<String> {
        [
            "AgentSpawn",
            "AgentSendInput",
            "AgentInterrupt",
            "AgentWait",
            "Read",
            "Grep",
            "Glob",
            "LS",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }
    fn user_context_policy(&self) -> crate::agentic::agents::UserContextPolicy {
        crate::agentic::agents::UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_project_layout()
    }
}

fn reviewer_tool_exposure_overrides() -> AgentToolPolicyOverrides {
    let mut overrides = AgentToolPolicyOverrides::default();
    overrides.insert("GetFileDiff".to_string(), ToolExposure::Direct);
    overrides
}

define_readonly_subagent_with_overrides!(
    SwarmReviewerAgent,
    "SwarmReviewer",
    "Swarm Reviewer",
    "Read-only reviewer that independently validates a coherent change set from one or more Swarm Workers against their assignments and acceptance criteria.",
    "swarm_reviewer_agent",
    &["Read", "Grep", "Glob", "LS", "GetFileDiff"],
    reviewer_tool_exposure_overrides()
);

pub struct SwarmWorkerAgent {
    default_tools: Vec<String>,
}
impl Default for SwarmWorkerAgent {
    fn default() -> Self {
        Self::new()
    }
}
impl SwarmWorkerAgent {
    pub fn new() -> Self {
        Self {
            default_tools: [
                "Read",
                "view_image",
                "analyze_image",
                "Glob",
                "Grep",
                "Write",
                "Edit",
                "Delete",
                "ExecCommand",
                "WriteStdin",
                "ExecControl",
                "WebSearch",
                "WebFetch",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }
}
#[async_trait]
impl Agent for SwarmWorkerAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn id(&self) -> &str {
        "SwarmWorker"
    }
    fn name(&self) -> &str {
        "Swarm Worker"
    }
    fn description(&self) -> &str {
        "Execution agent for one bounded Swarm work package."
    }
    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "swarm_worker_agent"
    }
    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }
    fn user_context_policy(&self) -> crate::agentic::agents::UserContextPolicy {
        crate::agentic::agents::UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_project_layout()
    }
}
