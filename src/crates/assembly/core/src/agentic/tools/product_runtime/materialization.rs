//! Product tool materialization owner.

use crate::agentic::tools::framework::Tool;
use crate::agentic::tools::implementations::*;
use crate::agentic::tools::product_runtime::CallDeferredTool;
use crate::agentic::tools::registry::ProductToolDecoratorRef;
use bitfun_agent_tools::{
    StaticToolMaterializationError, StaticToolProviderFactory, ToolRegistry as AgentToolRegistry,
    ToolRuntimeAssembly,
};
use bitfun_tool_packs::{
    tool_feature_group, unavailable_feature_groups, ToolPackFeatureGroup, ToolProviderGroupPlan,
};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProductToolMaterializationError {
    #[error("product capability plan requires tool groups absent from this binary: {groups}")]
    UnavailableFeatureGroups { groups: String },
    #[error("product tool {tool_name} in provider {provider_id} has no feature owner")]
    MissingFeatureOwner {
        provider_id: &'static str,
        tool_name: &'static str,
    },
    #[error(transparent)]
    StaticToolMaterialization(#[from] StaticToolMaterializationError),
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::agentic::tools) struct ProductConcreteToolFactory;

impl StaticToolProviderFactory<dyn Tool> for ProductConcreteToolFactory {
    fn materialize_tool(&self, tool_name: &str) -> Option<Arc<dyn Tool>> {
        match tool_name {
            "LS" => Some(Arc::new(LSTool::new())),
            "Read" => Some(Arc::new(FileReadTool::new())),
            #[cfg(feature = "tools-image-analysis")]
            "view_image" => Some(Arc::new(ViewImageTool::new())),
            #[cfg(feature = "tools-image-analysis")]
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
            "ListModels" => Some(Arc::new(ListModelsTool::new())),
            "Task" => Some(Arc::new(TaskTool::new())),
            "AgentWait" => Some(Arc::new(AgentWaitTool::new())),
            "LaunchReviewAgent" => Some(Arc::new(LaunchReviewAgentTool::new())),
            "Skill" => Some(Arc::new(SkillTool::new())),
            "AskUserQuestion" => Some(Arc::new(AskUserQuestionTool::new())),
            "TodoWrite" => Some(Arc::new(TodoWriteTool::new())),
            "get_goal" => Some(Arc::new(GetGoalTool::new())),
            "create_goal" => Some(Arc::new(CreateGoalTool::new())),
            "update_goal" => Some(Arc::new(UpdateGoalTool::new())),
            #[cfg(feature = "tools-canvas")]
            "CreateCanvas" => Some(Arc::new(CreateCanvasTool::new())),
            #[cfg(feature = "tools-canvas")]
            "ReadCanvas" => Some(Arc::new(ReadCanvasTool::new())),
            #[cfg(feature = "tools-canvas")]
            "UpdateCanvas" => Some(Arc::new(UpdateCanvasTool::new())),
            #[cfg(feature = "tools-canvas")]
            "PatchCanvas" => Some(Arc::new(PatchCanvasTool::new())),
            "CreatePlan" => Some(Arc::new(CreatePlanTool::new())),
            "PlanList" => Some(Arc::new(PlanListTool::new())),
            "PlanRead" => Some(Arc::new(PlanReadTool::new())),
            "PlanUpdate" => Some(Arc::new(PlanUpdateTool::new())),
            "submit_code_review" => Some(Arc::new(CodeReviewTool::new())),
            "DeepReview" => Some(Arc::new(DeepReviewTool::new())),
            "GetToolSpec" => Some(Arc::new(GetToolSpecTool::new())),
            "CallDeferredTool" => Some(Arc::new(CallDeferredTool::new())),
            #[cfg(feature = "tools-git")]
            "GetFileDiff" => Some(Arc::new(GetFileDiffTool::new())),
            "SessionControl" => Some(Arc::new(SessionControlTool::new())),
            "LegionControl" => Some(Arc::new(LegionControlTool::new())),
            "SessionMessage" => Some(Arc::new(SessionMessageTool::new())),
            "SessionHistory" => Some(Arc::new(SessionHistoryTool::new())),
            "acp_control" => Some(Arc::new(AcpControlTool::new())),
            "acp_message" => Some(Arc::new(AcpMessageTool::new())),
            "acp_history" => Some(Arc::new(AcpHistoryTool::new())),
            #[cfg(feature = "tools-agent-control")]
            "Cron" => Some(Arc::new(CronTool::new())),
            #[cfg(feature = "tools-browser-web")]
            "WebSearch" => Some(Arc::new(WebSearchTool::new())),
            #[cfg(feature = "tools-browser-web")]
            "WebFetch" => Some(Arc::new(WebFetchTool::new())),
            #[cfg(feature = "tools-mcp")]
            "ListMCPResources" => Some(Arc::new(ListMCPResourcesTool::new())),
            #[cfg(feature = "tools-mcp")]
            "ReadMCPResource" => Some(Arc::new(ReadMCPResourceTool::new())),
            #[cfg(feature = "tools-mcp")]
            "ListMCPPrompts" => Some(Arc::new(ListMCPPromptsTool::new())),
            #[cfg(feature = "tools-mcp")]
            "GetMCPPrompt" => Some(Arc::new(GetMCPPromptTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "GenerativeUI" => Some(Arc::new(GenerativeUITool::new())),
            #[cfg(feature = "tools-git")]
            "Git" => Some(Arc::new(GitTool::new())),
            #[cfg(feature = "tools-git")]
            "Worktree" => Some(Arc::new(WorktreeTool::new())),
            "WorkspaceScan" => Some(Arc::new(WorkspaceScanTool::new())),
            "KnowledgeBaseSearch" => Some(Arc::new(KnowledgeBaseSearchTool::new())),
            #[cfg(feature = "tools-git")]
            "ReviewPlatform" => Some(Arc::new(ReviewPlatformTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "InitMiniApp" => Some(Arc::new(InitMiniAppTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "FinalizeMiniApp" => Some(Arc::new(FinalizeMiniAppTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "PublishMiniApp" => Some(Arc::new(PublishMiniAppTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "PublishAppearance" => Some(Arc::new(PublishAppearanceTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "PageDeploy" => Some(Arc::new(PageDeployTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "PagePublish" => Some(Arc::new(PagePublishTool::new())),
            #[cfg(feature = "tools-browser-web")]
            "ControlHub" => Some(Arc::new(ControlHubTool::new())),
            #[cfg(feature = "tools-computer-use")]
            "ComputerUse" => Some(Arc::new(ComputerUseTool::new())),
            #[cfg(feature = "tools-miniapp")]
            "Playbook" => Some(Arc::new(PlaybookTool::new())),
            _ => None,
        }
    }
}

pub(in crate::agentic::tools) fn create_product_tool_registry_from_plan(
    plan: &[ToolProviderGroupPlan],
    requested_feature_groups: &[ToolPackFeatureGroup],
    tool_decorator: ProductToolDecoratorRef,
) -> Result<AgentToolRegistry<dyn Tool>, ProductToolMaterializationError> {
    let unavailable = unavailable_feature_groups(requested_feature_groups);
    if !unavailable.is_empty() {
        return Err(ProductToolMaterializationError::UnavailableFeatureGroups {
            groups: unavailable
                .iter()
                .map(|group| group.id())
                .collect::<Vec<_>>()
                .join(", "),
        });
    }

    let requested = requested_feature_groups
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut entries = Vec::new();
    for provider in plan {
        let mut tool_names = Vec::new();
        for tool_name in provider.tool_names() {
            let feature_group = tool_feature_group(tool_name).ok_or(
                ProductToolMaterializationError::MissingFeatureOwner {
                    provider_id: provider.provider_id(),
                    tool_name,
                },
            )?;
            if requested.contains(&feature_group) {
                tool_names.push(*tool_name);
            }
        }
        if !tool_names.is_empty() {
            entries.push((provider.provider_id(), tool_names));
        }
    }

    Ok(ToolRuntimeAssembly::with_tool_decorator(tool_decorator)
        .create_registry_from_static_provider_entries(entries, &ProductConcreteToolFactory)?)
}
