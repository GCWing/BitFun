//! Tool registry

use crate::agentic::tools::framework::Tool;
use crate::agentic::tools::implementations::*;
use crate::util::errors::BitFunResult;
use indexmap::IndexMap;
use log::{debug, info, trace, warn};
use std::sync::Arc;

/// Tool registry - manages all available tools (using IndexMap to maintain registration order)
pub struct ToolRegistry {
    tools: IndexMap<String, Arc<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        let mut registry = Self {
            tools: IndexMap::new(),
        };

        // Register all tools
        registry.register_all_tools();
        registry
    }

    /// Dynamically register MCP tools
    pub fn register_mcp_tools(&mut self, tools: Vec<Arc<dyn Tool>>) {
        let tool_count = tools.len();
        info!("Registering MCP tools: count={}", tool_count);

        let before_count = self.tools.len();
        debug!("Tool count before registration: {}", before_count);

        for (index, tool) in tools.into_iter().enumerate() {
            let name = tool.name().to_string();
            debug!(
                "Registering MCP tool [{}/{}]: {}",
                index + 1,
                tool_count,
                name
            );

            // Check if a tool with the same name already exists
            if self.tools.contains_key(&name) {
                warn!(
                    "Tool already exists, will be overwritten: tool_name={}",
                    name
                );
            }

            self.register_tool(tool);
            debug!("MCP tool registered: tool_name={}", name);
        }

        let after_count = self.tools.len();
        let added_count = after_count - before_count;

        info!(
            "MCP tools registration completed: before={}, after={}, added={}",
            before_count, after_count, added_count
        );
    }

    /// Remove all tools from the MCP server
    pub fn unregister_mcp_server_tools(&mut self, server_id: &str) {
        let prefix = format!("mcp__{}__", server_id);
        let to_remove: Vec<String> = self
            .tools
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();

        for key in to_remove {
            info!("Unregistering MCP tool: tool_name={}", key);
            self.tools.shift_remove(&key);
        }
    }

    /// Register all tools
    fn register_all_tools(&mut self) {
        // Agent dispatch tool (Dispatcher mode)
        self.register_tool(Arc::new(AgentDispatchTool::new()));

        // Basic tool set
        self.register_tool(Arc::new(LSTool::new()));
        self.register_tool(Arc::new(FileReadTool::new()));
        self.register_tool(Arc::new(GlobTool::new()));
        self.register_tool(Arc::new(GrepTool::new()));
        self.register_tool(Arc::new(FileWriteTool::new()));
        self.register_tool(Arc::new(FileEditTool::new()));
        self.register_tool(Arc::new(DeleteFileTool::new()));
        self.register_tool(Arc::new(BashTool::new()));
        self.register_tool(Arc::new(TerminalControlTool::new()));
        self.register_tool(Arc::new(SessionControlTool::new()));
        self.register_tool(Arc::new(SessionMessageTool::new()));
        self.register_tool(Arc::new(SessionHistoryTool::new()));

        // TodoWrite tool
        self.register_tool(Arc::new(TodoWriteTool::new()));

        // Cron scheduled jobs tool
        self.register_tool(Arc::new(CronTool::new()));

        // TaskTool, execute subagent
        self.register_tool(Arc::new(TaskTool::new()));

        // Skill tool
        self.register_tool(Arc::new(SkillTool::new()));

        // AskUserQuestion tool
        self.register_tool(Arc::new(AskUserQuestionTool::new()));

        // Web tool
        self.register_tool(Arc::new(WebSearchTool::new()));
        self.register_tool(Arc::new(WebFetchTool::new()));
        self.register_tool(Arc::new(ListMCPResourcesTool::new()));
        self.register_tool(Arc::new(ReadMCPResourceTool::new()));
        self.register_tool(Arc::new(ListMCPPromptsTool::new()));
        self.register_tool(Arc::new(GetMCPPromptTool::new()));

        self.register_tool(Arc::new(GenerativeUITool::new()));

        // DesignTokens tool (Design mode — token alignment / proposal / commit)
        self.register_tool(Arc::new(DesignTokensTool::new()));

        // DesignArtifact tool (Design mode — produces right-panel Design Canvas artifacts)
        self.register_tool(Arc::new(DesignArtifactTool::new()));
        // GetFileDiff tool
        self.register_tool(Arc::new(GetFileDiffTool::new()));

        // Log tool
        self.register_tool(Arc::new(LogTool::new()));

        // CreatePlan tool
        self.register_tool(Arc::new(CreatePlanTool::new()));

        // Code review submit tool
        self.register_tool(Arc::new(CodeReviewTool::new()));

        // Live App tool (InitLiveApp)
        self.register_tool(Arc::new(InitLiveAppTool::new()));

        // ControlHub — sole unified control entry point that aggregates ALL control
        // capabilities (app, desktop, browser, terminal, system, meta) into a single
        // tool. Legacy split control tools are intentionally NOT registered
        // here: their implementations are kept internal where needed and reused by
        // ControlHub, but the model only ever sees one control tool to eliminate
        // cross-tool selection mistakes.
        self.register_tool(Arc::new(ControlHubTool::new()));

        // Playbook — predefined step-by-step operation guides for common tasks.
        self.register_tool(Arc::new(PlaybookTool::new()));
    }

    /// Register a single tool
    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) {
        // Snapshot-aware wrapping happens once at registration time so every
        // subsequent lookup returns the same runtime implementation.
        let tool = crate::service::snapshot::wrap_tool_for_snapshot_tracking(tool);
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Get tool
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Get all tool names
    pub fn get_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get all tools
    pub fn get_all_tools(&self) -> Vec<Arc<dyn Tool>> {
        trace!(
            "ToolRegistry::get_all_tools() called: total={}",
            self.tools.len()
        );
        self.tools.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::create_tool_registry;
    use serde_json::json;

    #[test]
    fn registry_includes_webfetch_tool() {
        let registry = create_tool_registry();
        assert!(registry.get_tool("WebFetch").is_some());
    }

    #[test]
    fn registry_includes_cron_tool() {
        let registry = create_tool_registry();
        assert!(registry.get_tool("Cron").is_some());
    }

    /// Phase 0 contract: ControlHub is the sole control entry point. Legacy
    /// split control tools must NOT be visible to the model; their
    /// implementations are reused internally only.
    #[test]
    fn registry_exposes_controlhub_only_for_control_capabilities() {
        let registry = create_tool_registry();
        assert!(
            registry.get_tool("ControlHub").is_some(),
            "ControlHub must be registered as the unified control tool"
        );
        assert!(
            registry.get_tool("ComputerUse").is_none(),
            "Legacy split control tools must remain hidden (Phase 0 dedup)"
        );
    }

    #[test]
    fn registry_wraps_file_modification_tools_for_snapshot_tracking() {
        let registry = create_tool_registry();
        let tool = registry
            .get_tool("Write")
            .expect("Write tool should be registered");

        let assistant_text = tool.render_result_for_assistant(&json!({
            "success": true,
            "file_path": "E:/Projects/demo.txt"
        }));

        assert!(
            assistant_text.contains("snapshot system"),
            "expected snapshot wrapper text, got: {}",
            assistant_text
        );
    }
}

/// Get all tools from the snapshot-aware global registry.
pub async fn get_all_tools() -> Vec<Arc<dyn Tool>> {
    let registry = get_global_tool_registry();
    let registry_lock = registry.read().await;
    registry_lock.get_all_tools()
}

/// Get readonly tools
pub async fn get_readonly_tools() -> BitFunResult<Vec<Arc<dyn Tool>>> {
    let all_tools = get_all_tools().await;
    let mut readonly_tools = Vec::new();

    for tool in all_tools {
        if tool.is_readonly() && tool.is_enabled().await {
            readonly_tools.push(tool);
        }
    }

    Ok(readonly_tools)
}

/// Create default tool registry - factory function
pub fn create_tool_registry() -> ToolRegistry {
    ToolRegistry::new()
}

// Global tool registry instance
use std::sync::OnceLock;
use tokio::sync::RwLock as TokioRwLock;

static GLOBAL_TOOL_REGISTRY: OnceLock<Arc<TokioRwLock<ToolRegistry>>> = OnceLock::new();

/// Get global tool registry
pub fn get_global_tool_registry() -> Arc<TokioRwLock<ToolRegistry>> {
    GLOBAL_TOOL_REGISTRY
        .get_or_init(|| {
            info!("Initializing global tool registry");
            Arc::new(TokioRwLock::new(ToolRegistry::new()))
        })
        .clone()
}

/// Backward-compatible alias for callers that expect MCP tools to be included.
pub async fn get_all_registered_tools() -> Vec<Arc<dyn Tool>> {
    get_all_tools().await
}

/// Get all registered tool names
pub async fn get_all_registered_tool_names() -> Vec<String> {
    let all_tools = get_all_registered_tools().await;
    all_tools
        .into_iter()
        .map(|tool| tool.name().to_string())
        .collect()
}
