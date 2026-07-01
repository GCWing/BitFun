//! Product tool materialization owner.

use crate::agentic::tools::framework::Tool;
use crate::agentic::tools::implementations::*;
use crate::agentic::tools::registry::ProductToolDecoratorRef;
use bitfun_agent_tools::{
    StaticToolProviderFactory, ToolRegistry as AgentToolRegistry, ToolRuntimeAssembly,
};
use bitfun_tool_packs::ToolProviderGroupPlan;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::agentic::tools) struct ProductConcreteToolFactory;

impl StaticToolProviderFactory<dyn Tool> for ProductConcreteToolFactory {
    fn materialize_tool(&self, tool_name: &str) -> Option<Arc<dyn Tool>> {
        match tool_name {
            "LS" => Some(Arc::new(LSTool::new())),
            "Read" => Some(Arc::new(FileReadTool::new())),
            "view_image" => Some(Arc::new(ViewImageTool::new())),
            "analyze_image" => Some(Arc::new(AnalyzeImageTool::new())),
            "Glob" => Some(Arc::new(GlobTool::new())),
            "Grep" => Some(Arc::new(GrepTool::new())),
            "Write" => Some(Arc::new(FileWriteTool::new())),
            "Edit" => Some(Arc::new(FileEditTool::new())),
            "Delete" => Some(Arc::new(DeleteFileTool::new())),
            "ExecCommand" => Some(Arc::new(ExecCommandTool::new())),
            "WriteStdin" => Some(Arc::new(WriteStdinTool::new())),
            "ExecControl" => Some(Arc::new(ExecControlTool::new())),
            "GetTime" => Some(Arc::new(GetTimeTool::new())),
            "Task" => Some(Arc::new(TaskTool::new())),
            "LaunchReviewAgent" => Some(Arc::new(LaunchReviewAgentTool::new())),
            "Skill" => Some(Arc::new(SkillTool::new())),
            "AskUserQuestion" => Some(Arc::new(AskUserQuestionTool::new())),
            "TodoWrite" => Some(Arc::new(TodoWriteTool::new())),
            "get_goal" => Some(Arc::new(GetGoalTool::new())),
            "create_goal" => Some(Arc::new(CreateGoalTool::new())),
            "update_goal" => Some(Arc::new(UpdateGoalTool::new())),
            #[cfg(feature = "canvas-runtime")]
            "CreateCanvas" => Some(Arc::new(CreateCanvasTool::new())),
            #[cfg(feature = "canvas-runtime")]
            "ReadCanvas" => Some(Arc::new(ReadCanvasTool::new())),
            #[cfg(feature = "canvas-runtime")]
            "UpdateCanvas" => Some(Arc::new(UpdateCanvasTool::new())),
            #[cfg(feature = "canvas-runtime")]
            "PatchCanvas" => Some(Arc::new(PatchCanvasTool::new())),
            "CreatePlan" => Some(Arc::new(CreatePlanTool::new())),
            "submit_code_review" => Some(Arc::new(CodeReviewTool::new())),
            "GetToolSpec" => Some(Arc::new(GetToolSpecTool::new())),
            "GetFileDiff" => Some(Arc::new(GetFileDiffTool::new())),
            "SessionControl" => Some(Arc::new(SessionControlTool::new())),
            "SessionMessage" => Some(Arc::new(SessionMessageTool::new())),
            "SessionHistory" => Some(Arc::new(SessionHistoryTool::new())),
            "Cron" => Some(Arc::new(CronTool::new())),
            "WebSearch" => Some(Arc::new(WebSearchTool::new())),
            "WebFetch" => Some(Arc::new(WebFetchTool::new())),
            "ListMCPResources" => Some(Arc::new(ListMCPResourcesTool::new())),
            "ReadMCPResource" => Some(Arc::new(ReadMCPResourceTool::new())),
            "ListMCPPrompts" => Some(Arc::new(ListMCPPromptsTool::new())),
            "GetMCPPrompt" => Some(Arc::new(GetMCPPromptTool::new())),
            "GenerativeUI" => Some(Arc::new(GenerativeUITool::new())),
            "Git" => Some(Arc::new(GitTool::new())),
            "ReviewPlatform" => Some(Arc::new(ReviewPlatformTool::new())),
            "InitMiniApp" => Some(Arc::new(InitMiniAppTool::new())),
            "ControlHub" => Some(Arc::new(ControlHubTool::new())),
            "ComputerUse" => Some(Arc::new(ComputerUseTool::new())),
            "Playbook" => Some(Arc::new(PlaybookTool::new())),
            _ => None,
        }
    }
}

pub(in crate::agentic::tools) fn create_product_tool_registry_from_plan(
    plan: &[ToolProviderGroupPlan],
    tool_decorator: ProductToolDecoratorRef,
) -> AgentToolRegistry<dyn Tool> {
    let entries = plan
        .iter()
        .map(|group| (group.provider_id(), group.tool_names()));

    ToolRuntimeAssembly::with_tool_decorator(tool_decorator)
        .create_registry_from_static_provider_entries(entries, &ProductConcreteToolFactory)
        .expect("product capability tool provider plan must reference concrete core tools")
}
