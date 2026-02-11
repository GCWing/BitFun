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

            self.tools.insert(name.clone(), tool);
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
        let prefix = format!("mcp_{}_", server_id);
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
        // Basic tool set
        self.register_tool(Arc::new(LSTool::new()));
        self.register_tool(Arc::new(FileReadTool::new()));
        self.register_tool(Arc::new(GlobTool::new()));
        self.register_tool(Arc::new(GrepTool::new()));
        self.register_tool(Arc::new(FileWriteTool::new()));
        self.register_tool(Arc::new(FileEditTool::new()));
        self.register_tool(Arc::new(DeleteFileTool::new()));
        self.register_tool(Arc::new(BashTool::new()));

        // TodoWrite tool
        self.register_tool(Arc::new(TodoWriteTool::new()));

        // TaskTool, execute subagent
        self.register_tool(Arc::new(TaskTool::new()));

        // Skill tool
        self.register_tool(Arc::new(SkillTool::new()));

        // AskUserQuestion tool
        self.register_tool(Arc::new(AskUserQuestionTool::new()));

        // Web tool
        self.register_tool(Arc::new(WebSearchTool::new()));

        // IDE control tool
        self.register_tool(Arc::new(IdeControlTool::new()));

        // Mermaid interactive chart tool
        self.register_tool(Arc::new(MermaidInteractiveTool::new()));

        // GetFileDiff tool
        self.register_tool(Arc::new(GetFileDiffTool::new()));

        // Log tool
        self.register_tool(Arc::new(LogTool::new()));

        // Linter tool (LSP diagnosis)
        self.register_tool(Arc::new(ReadLintsTool::new()));

        // Image analysis tool
        self.register_tool(Arc::new(AnalyzeImageTool::new()));

        // Git version control tool
        self.register_tool(Arc::new(GitTool::new()));

        // CreatePlan tool
        self.register_tool(Arc::new(CreatePlanTool::new()));

        // Code review submit tool
        self.register_tool(Arc::new(CodeReviewTool::new()));
    }

    /// Register a single tool
    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) {
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

/// Get all tools
/// - Snapshot initialized:
/// return tools only in the snapshot manager (wrapped file tools + built-in non-file tools)
/// **not containing** dynamically registered MCP tools.
/// - Snapshot not initialized:
/// return all tools in the global registry,
/// **containing** MCP tools.
/// If you need **always include** MCP tools, use [get_all_registered_tools]
pub async fn get_all_tools() -> Vec<Arc<dyn Tool>> {
    match crate::service::snapshot::ensure_global_snapshot_manager() {
        Ok(sandbox_manager) => {
            // Return wrapped tools in the snapshot manager
            sandbox_manager.get_wrapped_tools()
        }
        Err(e) => {
            warn!(
                "Snapshot manager not initialized, using global registry tools: {}",
                e
            );
            let registry = get_global_tool_registry();
            let guard = registry.read().await;
            guard.get_all_tools()
        }
    }
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

/// Get all registered tools (**always include** dynamically registered MCP tools)
/// - Snapshot initialized:
/// return wrapped file tools + other tools in the global registry (containing MCP tools)
/// - Snapshot not initialized: return all tools in the global registry.
pub async fn get_all_registered_tools() -> Vec<Arc<dyn Tool>> {
    // Try to use wrapped tools in the snapshot manager first (file operation tools don't need confirmation)
    match crate::service::snapshot::ensure_global_snapshot_manager() {
        Ok(sandbox_manager) => {
            // Get all tools in the global registry
            let registry = get_global_tool_registry();
            let registry_lock = registry.read().await;
            let all_tools = registry_lock.get_all_tools();

            // Get wrapped core file tools in the snapshot manager
            let wrapped_tools = sandbox_manager.get_wrapped_tools();

            // Create file tool name set
            let file_tool_names: std::collections::HashSet<String> =
                wrapped_tools.iter().map(|t| t.name().to_string()).collect();

            // Merge: use wrapped file tools + other original tools (like MCP tools)
            let mut result = wrapped_tools;
            for tool in all_tools {
                if !file_tool_names.contains(tool.name()) {
                    result.push(tool);
                }
            }

            result
        }
        Err(e) => {
            warn!(
                "Snapshot manager not initialized, using original tools: {}",
                e
            );
            // Snapshot not initialized, return original tools in the registry
            let registry = get_global_tool_registry();
            let registry_lock = registry.read().await;
            let tools = registry_lock.get_all_tools();
            tools
        }
    }
}

/// Get all registered tool names
pub async fn get_all_registered_tool_names() -> Vec<String> {
    let all_tools = get_all_registered_tools().await;
    all_tools
        .into_iter()
        .map(|tool| tool.name().to_string())
        .collect()
}

/// Get all should_end_turn tool names
pub async fn get_all_end_turn_tool_names() -> Vec<String> {
    let all_tools = get_all_registered_tools().await;
    all_tools
        .into_iter()
        .filter(|tool| tool.should_end_turn())
        .map(|tool| tool.name().to_string())
        .collect()
}
