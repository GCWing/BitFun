//! Tool registry

use crate::agentic::tools::framework::{DynamicToolInfo, Tool};
use crate::agentic::tools::implementations::*;
use crate::util::errors::BitFunResult;
use bitfun_agent_tools::{
    DynamicToolDescriptor, DynamicToolProvider, PortError, PortErrorKind, PortResult, ToolDecorator,
};
use indexmap::IndexMap;
use log::{debug, info, trace, warn};
use std::sync::Arc;

type ToolRef = Arc<dyn Tool>;
type ToolDecoratorRef = Arc<dyn ToolDecorator<ToolRef>>;

#[derive(Debug, Clone)]
struct DynamicToolMetadata {
    provider_id: String,
    info: DynamicToolInfo,
}

struct SnapshotToolDecorator;

impl ToolDecorator<ToolRef> for SnapshotToolDecorator {
    fn decorate(&self, tool: ToolRef) -> ToolRef {
        crate::service::snapshot::wrap_tool_for_snapshot_tracking(tool)
    }
}

/// Tool registry - manages all available tools (using IndexMap to maintain registration order)
pub struct ToolRegistry {
    tools: IndexMap<String, ToolRef>,
    dynamic_tools: IndexMap<String, DynamicToolMetadata>,
    tool_decorator: ToolDecoratorRef,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self::with_tool_decorator(Arc::new(SnapshotToolDecorator))
    }

    /// Create a registry with an injected decoration boundary.
    ///
    /// The default production decorator preserves snapshot-aware wrapping while
    /// allowing future owner crates to replace this concrete service coupling
    /// through the `bitfun-runtime-ports` interface.
    pub fn with_tool_decorator(tool_decorator: ToolDecoratorRef) -> Self {
        let mut registry = Self {
            tools: IndexMap::new(),
            dynamic_tools: IndexMap::new(),
            tool_decorator,
        };

        // Register all tools
        registry.register_all_tools();
        registry
    }

    /// Dynamically register MCP tools
    pub fn register_mcp_tools(&mut self, tools: Vec<ToolRef>) {
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
        let to_remove: Vec<String> = self
            .dynamic_tools
            .iter()
            .filter(|(_, metadata)| {
                metadata
                    .info
                    .mcp
                    .as_ref()
                    .is_some_and(|info| info.server_id == server_id)
            })
            .map(|(tool_name, _)| tool_name.clone())
            .collect();

        for key in to_remove {
            info!("Unregistering dynamic tool: tool_name={}", key);
            self.tools.shift_remove(&key);
            self.dynamic_tools.shift_remove(&key);
        }
    }

    /// Remove all tools whose registry name starts with the given prefix.
    pub fn unregister_tools_by_prefix(&mut self, prefix: &str) -> usize {
        let to_remove: Vec<String> = self
            .tools
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        let count = to_remove.len();

        for key in to_remove {
            info!("Unregistering dynamic tool: tool_name={}", key);
            self.tools.shift_remove(&key);
            self.dynamic_tools.shift_remove(&key);
        }
        count
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
        // TerminalControl is now accessible via ControlHub's "terminal" domain,
        // but we keep it registered separately for backward compatibility.
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

        // GetFileDiff tool
        self.register_tool(Arc::new(GetFileDiffTool::new()));

        // Log tool
        self.register_tool(Arc::new(LogTool::new()));

        // Git version control tool
        self.register_tool(Arc::new(GitTool::new()));

        // CreatePlan tool
        self.register_tool(Arc::new(CreatePlanTool::new()));

        // Code review submit tool
        self.register_tool(Arc::new(CodeReviewTool::new()));

        // MiniApp Agent tool (single InitMiniApp)
        self.register_tool(Arc::new(InitMiniAppTool::new()));

        // ControlHub — unified browser/terminal/meta control entry point.
        // Local desktop and OS/system Computer Use is exposed as a dedicated tool.
        self.register_tool(Arc::new(ControlHubTool::new()));
        self.register_tool(Arc::new(ComputerUseTool::new()));

        // Playbook — predefined step-by-step operation guides for common tasks.
        self.register_tool(Arc::new(PlaybookTool::new()));
    }

    /// Register a single tool
    pub fn register_tool(&mut self, tool: ToolRef) {
        // Snapshot-aware wrapping happens once at registration time so every
        // subsequent lookup returns the same runtime implementation.
        let tool = self.tool_decorator.decorate(tool);
        let name = tool.name().to_string();
        let dynamic_info = tool.dynamic_tool_info().and_then(|info| {
            if info.provider_id.trim().is_empty() {
                None
            } else {
                Some(info)
            }
        });

        if let Some(info) = dynamic_info {
            self.dynamic_tools.insert(
                name.clone(),
                DynamicToolMetadata {
                    provider_id: info.provider_id.clone(),
                    info,
                },
            );
        } else {
            self.dynamic_tools.shift_remove(&name);
        }
        self.tools.insert(name, tool);
    }

    /// Get tool
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn get_dynamic_tool_info(&self, name: &str) -> Option<DynamicToolInfo> {
        self.dynamic_tools
            .get(name)
            .map(|metadata| metadata.info.clone())
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

#[async_trait::async_trait]
impl DynamicToolProvider for ToolRegistry {
    async fn list_dynamic_tools(&self) -> PortResult<Vec<DynamicToolDescriptor>> {
        let mut descriptors = Vec::new();

        for (name, tool) in self.tools.iter() {
            let Some(metadata) = self.dynamic_tools.get(name) else {
                continue;
            };
            let description = tool
                .description()
                .await
                .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?;

            descriptors.push(DynamicToolDescriptor {
                name: tool.name().to_string(),
                description,
                input_schema: tool.input_schema_for_model().await,
                provider_id: Some(metadata.provider_id.clone()),
            });
        }

        Ok(descriptors)
    }
}

#[cfg(test)]
mod tests {
    use super::create_tool_registry;
    use super::ToolRef;
    use super::ToolRegistry;
    use crate::agentic::tools::framework::{
        DynamicMcpToolInfo, DynamicToolInfo, Tool, ToolResult, ToolUseContext, ValidationResult,
    };
    use async_trait::async_trait;
    use bitfun_agent_tools::DynamicToolProvider;
    use serde_json::json;
    use serde_json::Value;
    use std::sync::Arc;

    struct DynamicMetadataTool {
        name: String,
        dynamic_info: Option<DynamicToolInfo>,
    }

    #[async_trait]
    impl Tool for DynamicMetadataTool {
        fn name(&self) -> &str {
            &self.name
        }

        async fn description(&self) -> crate::util::errors::BitFunResult<String> {
            Ok("dynamic test tool".to_string())
        }

        fn input_schema(&self) -> Value {
            json!({ "type": "object" })
        }

        fn dynamic_provider_id(&self) -> Option<&str> {
            self.dynamic_info
                .as_ref()
                .map(|info| info.provider_id.as_str())
        }

        fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> {
            self.dynamic_info.clone()
        }

        async fn validate_input(
            &self,
            _input: &Value,
            _context: Option<&ToolUseContext>,
        ) -> ValidationResult {
            ValidationResult {
                result: true,
                message: None,
                error_code: None,
                meta: None,
            }
        }

        async fn call_impl(
            &self,
            _input: &Value,
            _context: &ToolUseContext,
        ) -> crate::util::errors::BitFunResult<Vec<ToolResult>> {
            Ok(Vec::new())
        }
    }

    fn dynamic_tool(name: &str, provider_id: Option<&str>) -> ToolRef {
        Arc::new(DynamicMetadataTool {
            name: name.to_string(),
            dynamic_info: provider_id.map(|provider_id| DynamicToolInfo {
                provider_id: provider_id.to_string(),
                provider_kind: None,
                mcp: None,
            }),
        })
    }

    fn mcp_dynamic_tool(
        name: &str,
        _provider_id: Option<&str>,
        server_id: &str,
        server_name: &str,
        tool_name: &str,
    ) -> ToolRef {
        Arc::new(DynamicMetadataTool {
            name: name.to_string(),
            dynamic_info: Some(DynamicToolInfo {
                provider_id: server_id.to_string(),
                provider_kind: Some("mcp".to_string()),
                mcp: Some(DynamicMcpToolInfo {
                    server_id: server_id.to_string(),
                    server_name: server_name.to_string(),
                    tool_name: tool_name.to_string(),
                }),
            }),
        })
    }

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

    #[test]
    fn registry_preserves_builtin_tool_manifest_for_owner_migration() {
        let registry = create_tool_registry();
        let expected_names = vec![
            "LS",
            "Read",
            "Glob",
            "Grep",
            "Write",
            "Edit",
            "Delete",
            "Bash",
            "TerminalControl",
            "SessionControl",
            "SessionMessage",
            "SessionHistory",
            "TodoWrite",
            "Cron",
            "Task",
            "Skill",
            "AskUserQuestion",
            "WebSearch",
            "WebFetch",
            "ListMCPResources",
            "ReadMCPResource",
            "ListMCPPrompts",
            "GetMCPPrompt",
            "GenerativeUI",
            "GetFileDiff",
            "Log",
            "Git",
            "CreatePlan",
            "submit_code_review",
            "InitMiniApp",
            "ControlHub",
            "ComputerUse",
            "Playbook",
        ];

        assert_eq!(
            registry.get_tool_names(),
            expected_names,
            "builtin tool manifest must stay stable before moving registry ownership"
        );
        let runtime_names = registry
            .get_all_tools()
            .iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            runtime_names,
            registry.get_tool_names(),
            "runtime tool collection order must match registry key order"
        );
    }

    #[tokio::test]
    async fn registry_preserves_readonly_tool_manifest_for_owner_migration() {
        let readonly_names = super::get_readonly_tools()
            .await
            .expect("readonly tools")
            .iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            readonly_names,
            vec![
                "LS",
                "Read",
                "Glob",
                "Grep",
                "SessionHistory",
                "TodoWrite",
                "Skill",
                "AskUserQuestion",
                "WebSearch",
                "WebFetch",
                "ListMCPResources",
                "ReadMCPResource",
                "ListMCPPrompts",
                "GetMCPPrompt",
                "GenerativeUI",
                "GetFileDiff",
                "Log",
                "CreatePlan",
                "submit_code_review",
                "Playbook",
            ],
            "readonly tool manifest must stay stable before moving registry ownership"
        );
    }

    #[tokio::test]
    async fn dynamic_tool_provider_uses_explicit_provider_metadata() {
        let mut registry = ToolRegistry::new();
        registry.register_tool(dynamic_tool(
            "external_search",
            Some("github__enterprise/prod"),
        ));
        registry.register_tool(dynamic_tool("mcp__encoded__without_metadata", None));
        registry.register_tool(dynamic_tool("docs_lookup", Some("docs/provider")));

        let descriptors = registry
            .list_dynamic_tools()
            .await
            .expect("list dynamic tools");

        assert_eq!(
            descriptors
                .iter()
                .map(|descriptor| (descriptor.name.as_str(), descriptor.provider_id.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                ("external_search", Some("github__enterprise/prod")),
                ("docs_lookup", Some("docs/provider")),
            ],
            "dynamic provider descriptors must keep explicit metadata and registration order"
        );
        assert_eq!(descriptors[0].name, "external_search");
        assert_eq!(
            descriptors[0].provider_id.as_deref(),
            Some("github__enterprise/prod")
        );
    }

    #[tokio::test]
    async fn dynamic_tool_provider_preserves_descriptor_shape_and_order() {
        let mut registry = ToolRegistry::new();
        registry.register_tool(dynamic_tool("external_search", Some("provider-a")));
        registry.register_tool(dynamic_tool("local_docs", Some("provider-b")));

        let descriptors = registry
            .list_dynamic_tools()
            .await
            .expect("list dynamic tools");

        let dynamic_descriptors = descriptors
            .iter()
            .map(|descriptor| {
                (
                    descriptor.name.as_str(),
                    descriptor.description.as_str(),
                    descriptor.input_schema.clone(),
                    descriptor.provider_id.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            dynamic_descriptors,
            vec![
                (
                    "external_search",
                    "dynamic test tool",
                    json!({ "type": "object" }),
                    Some("provider-a"),
                ),
                (
                    "local_docs",
                    "dynamic test tool",
                    json!({ "type": "object" }),
                    Some("provider-b"),
                ),
            ],
            "dynamic descriptor shape and registration order must remain stable before provider owner migration"
        );
    }

    #[tokio::test]
    async fn registering_static_tool_clears_stale_dynamic_metadata_for_same_name() {
        let mut registry = ToolRegistry::new();
        registry.register_tool(dynamic_tool("external_search", Some("provider-a")));
        assert!(
            registry.get_dynamic_tool_info("external_search").is_some(),
            "dynamic metadata should be registered before overwrite"
        );

        registry.register_tool(dynamic_tool("external_search", None));

        assert!(
            registry.get_dynamic_tool_info("external_search").is_none(),
            "stale dynamic metadata must be removed when a static tool overwrites a dynamic tool"
        );
        let descriptors = registry
            .list_dynamic_tools()
            .await
            .expect("list dynamic tools");
        assert!(
            descriptors
                .iter()
                .all(|descriptor| descriptor.name != "external_search"),
            "stale dynamic descriptor must not leak after static overwrite"
        );
    }

    #[tokio::test]
    async fn dynamic_tool_provider_prefers_mcp_registry_metadata() {
        let mut registry = ToolRegistry::new();
        registry.register_tool(mcp_dynamic_tool(
            "mcp__github__search_repos",
            Some("stale-provider-id"),
            "github-server-id",
            "GitHub",
            "search_repos",
        ));

        let descriptors = registry
            .list_dynamic_tools()
            .await
            .expect("list dynamic tools");

        let descriptor = descriptors
            .into_iter()
            .find(|item| item.name == "mcp__github__search_repos")
            .expect("mcp descriptor");

        assert_eq!(descriptor.provider_id.as_deref(), Some("github-server-id"));
        assert_eq!(
            registry
                .get_dynamic_tool_info("mcp__github__search_repos")
                .expect("mcp metadata")
                .mcp
                .expect("mcp subtype metadata")
                .tool_name,
            "search_repos"
        );
    }
    #[test]
    fn registry_exposes_controlhub_and_computer_use() {
        let registry = create_tool_registry();
        assert!(
            registry.get_tool("ControlHub").is_some(),
            "ControlHub must remain registered for browser/terminal/meta control"
        );
        assert!(
            registry.get_tool("ComputerUse").is_some(),
            "ComputerUse must be registered as the dedicated desktop automation tool"
        );
    }

    #[test]
    fn registry_wraps_file_modification_tools_for_snapshot_tracking() {
        let registry = create_tool_registry();
        for tool_name in ["Write", "Edit", "Delete"] {
            let tool = registry
                .get_tool(tool_name)
                .unwrap_or_else(|| panic!("{tool_name} tool should be registered"));

            let assistant_text = tool.render_result_for_assistant(&json!({
                "success": true,
                "file_path": "E:/Projects/demo.txt"
            }));

            assert!(
                assistant_text.contains("snapshot system"),
                "expected snapshot wrapper text for {tool_name}, got: {assistant_text}"
            );
        }

        let read_text = registry
            .get_tool("Read")
            .expect("Read tool should be registered")
            .render_result_for_assistant(&json!({
                "content": "hello",
                "file_path": "E:/Projects/demo.txt"
            }));
        assert!(
            !read_text.contains("snapshot system"),
            "readonly tool should not be snapshot wrapped: {read_text}"
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

pub async fn get_readonly_registered_tool_names() -> Vec<String> {
    get_readonly_tools()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|tool| tool.name().to_string())
        .collect()
}
