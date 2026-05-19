//! Core-owned static tool provider assembly.

use crate::agentic::tools::framework::Tool;
use crate::agentic::tools::implementations::*;
use bitfun_agent_tools::StaticToolProviderGroup;
use std::sync::Arc;

pub(crate) fn builtin_static_tool_providers() -> Vec<StaticToolProviderGroup<dyn Tool>> {
    vec![
        StaticToolProviderGroup::new(
            "core.basic",
            vec![
                Arc::new(LSTool::new()),
                Arc::new(FileReadTool::new()),
                Arc::new(GlobTool::new()),
                Arc::new(GrepTool::new()),
                Arc::new(FileWriteTool::new()),
                Arc::new(FileEditTool::new()),
                Arc::new(DeleteFileTool::new()),
                Arc::new(BashTool::new()),
            ],
        ),
        StaticToolProviderGroup::new(
            "core.agent",
            vec![
                Arc::new(TaskTool::new()),
                Arc::new(SkillTool::new()),
                Arc::new(AskUserQuestionTool::new()),
                Arc::new(TodoWriteTool::new()),
                Arc::new(CreatePlanTool::new()),
                Arc::new(CodeReviewTool::new()),
                Arc::new(GetToolSpecTool::new()),
                Arc::new(GetFileDiffTool::new()),
                Arc::new(LogTool::new()),
            ],
        ),
        StaticToolProviderGroup::new(
            "core.session",
            vec![
                Arc::new(TerminalControlTool::new()),
                Arc::new(SessionControlTool::new()),
                Arc::new(SessionMessageTool::new()),
                Arc::new(SessionHistoryTool::new()),
                Arc::new(CronTool::new()),
            ],
        ),
        StaticToolProviderGroup::new(
            "core.integration",
            vec![
                Arc::new(WebSearchTool::new()),
                Arc::new(WebFetchTool::new()),
                Arc::new(ListMCPResourcesTool::new()),
                Arc::new(ReadMCPResourceTool::new()),
                Arc::new(ListMCPPromptsTool::new()),
                Arc::new(GetMCPPromptTool::new()),
                Arc::new(GenerativeUITool::new()),
                Arc::new(GitTool::new()),
                Arc::new(InitMiniAppTool::new()),
                Arc::new(ControlHubTool::new()),
                Arc::new(ComputerUseTool::new()),
                Arc::new(PlaybookTool::new()),
            ],
        ),
    ]
}
