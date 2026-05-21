//! Core-owned static tool provider assembly.

use crate::agentic::tools::framework::Tool;
use crate::agentic::tools::implementations::*;
use bitfun_agent_tools::StaticToolProviderGroup;
use bitfun_tool_packs::product_tool_provider_group_plan;
use std::sync::Arc;

pub(crate) fn builtin_static_tool_providers() -> Vec<StaticToolProviderGroup<dyn Tool>> {
    product_tool_provider_group_plan()
        .iter()
        .map(|group| {
            StaticToolProviderGroup::new(group.provider_id(), materialize_tools(group.tool_names()))
        })
        .collect()
}

fn materialize_tools(tool_names: &[&str]) -> Vec<Arc<dyn Tool>> {
    tool_names
        .iter()
        .map(|tool_name| materialize_tool(tool_name))
        .collect()
}

fn materialize_tool(tool_name: &str) -> Arc<dyn Tool> {
    match tool_name {
        "LS" => Arc::new(LSTool::new()),
        "Read" => Arc::new(FileReadTool::new()),
        "Glob" => Arc::new(GlobTool::new()),
        "Grep" => Arc::new(GrepTool::new()),
        "Write" => Arc::new(FileWriteTool::new()),
        "Edit" => Arc::new(FileEditTool::new()),
        "Delete" => Arc::new(DeleteFileTool::new()),
        "Bash" => Arc::new(BashTool::new()),
        "Task" => Arc::new(TaskTool::new()),
        "Skill" => Arc::new(SkillTool::new()),
        "AskUserQuestion" => Arc::new(AskUserQuestionTool::new()),
        "TodoWrite" => Arc::new(TodoWriteTool::new()),
        "CreatePlan" => Arc::new(CreatePlanTool::new()),
        "submit_code_review" => Arc::new(CodeReviewTool::new()),
        "GetToolSpec" => Arc::new(GetToolSpecTool::new()),
        "GetFileDiff" => Arc::new(GetFileDiffTool::new()),
        "Log" => Arc::new(LogTool::new()),
        "TerminalControl" => Arc::new(TerminalControlTool::new()),
        "SessionControl" => Arc::new(SessionControlTool::new()),
        "SessionMessage" => Arc::new(SessionMessageTool::new()),
        "SessionHistory" => Arc::new(SessionHistoryTool::new()),
        "Cron" => Arc::new(CronTool::new()),
        "WebSearch" => Arc::new(WebSearchTool::new()),
        "WebFetch" => Arc::new(WebFetchTool::new()),
        "ListMCPResources" => Arc::new(ListMCPResourcesTool::new()),
        "ReadMCPResource" => Arc::new(ReadMCPResourceTool::new()),
        "ListMCPPrompts" => Arc::new(ListMCPPromptsTool::new()),
        "GetMCPPrompt" => Arc::new(GetMCPPromptTool::new()),
        "GenerativeUI" => Arc::new(GenerativeUITool::new()),
        "Git" => Arc::new(GitTool::new()),
        "InitMiniApp" => Arc::new(InitMiniAppTool::new()),
        "ControlHub" => Arc::new(ControlHubTool::new()),
        "ComputerUse" => Arc::new(ComputerUseTool::new()),
        "Playbook" => Arc::new(PlaybookTool::new()),
        _ => panic!("unknown product tool provider plan entry: {tool_name}"),
    }
}
