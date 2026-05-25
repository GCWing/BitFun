//! Tool framework - Tool interface definition and execution context
use crate::agentic::WorkspaceBinding;
use crate::agentic::tools::restrictions::ToolRuntimeRestrictions;
use crate::agentic::workspace::WorkspaceServices;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
pub use bitfun_agent_tools::{
    DynamicMcpToolInfo, DynamicToolInfo, PortableToolContextProvider, ToolContextFacts,
    ToolExposure, ToolPathBackend, ToolPathResolution, ToolRenderOptions, ToolResult,
    ToolWorkspaceKind, ValidationResult, build_tool_path_policy_denial_message,
    build_tool_runtime_artifact_reference, build_tool_session_runtime_artifact_reference,
    is_tool_path_allowed_by_resolved_roots, resolve_tool_path_with_context,
    tool_path_is_effectively_absolute,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// Tool use context
#[derive(Debug, Clone)]
pub struct ToolUseContext {
    pub tool_call_id: Option<String>,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub dialog_turn_id: Option<String>,
    pub workspace: Option<WorkspaceBinding>,
    pub unlocked_collapsed_tools: Vec<String>,
    /// Extended context data passed from execution layer to tools.
    pub custom_data: HashMap<String, Value>,
    /// Desktop automation (Computer use); only set in BitFun desktop.
    pub computer_use_host: Option<crate::agentic::tools::computer_use_host::ComputerUseHostRef>,
    // Cancel tool execution more timely, especially for tools like TaskTool that need to run for a long time
    pub cancellation_token: Option<CancellationToken>,
    pub runtime_tool_restrictions: ToolRuntimeRestrictions,
    /// Workspace I/O services (filesystem + shell) - use these instead of
    /// checking `get_remote_workspace_manager()` inside individual tools.
    pub workspace_services: Option<WorkspaceServices>,
}

impl ToolUseContext {
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace.as_ref().map(|binding| binding.root_path())
    }

    pub fn is_remote(&self) -> bool {
        self.workspace
            .as_ref()
            .map(|ws| ws.is_remote())
            .unwrap_or(false)
    }

    pub fn to_tool_context_facts(&self) -> ToolContextFacts {
        let workspace_kind = self.workspace.as_ref().map(|workspace| {
            if workspace.is_remote() {
                ToolWorkspaceKind::Remote
            } else {
                ToolWorkspaceKind::Local
            }
        });

        ToolContextFacts {
            tool_call_id: self.tool_call_id.clone(),
            agent_type: self.agent_type.clone(),
            session_id: self.session_id.clone(),
            dialog_turn_id: self.dialog_turn_id.clone(),
            workspace_kind,
            workspace_root: self.workspace.as_ref().map(|workspace| {
                workspace
                    .session_identity
                    .logical_workspace_path()
                    .to_string()
            }),
            runtime_tool_restrictions: self.runtime_tool_restrictions.clone(),
        }
    }

    /// Whether the session primary model accepts image inputs (from tool-definition / pipeline context).
    /// Defaults to **true** when unset (e.g. API listings without model metadata).
    pub fn primary_model_supports_image_understanding(&self) -> bool {
        self.custom_data
            .get("primary_model_supports_image_understanding")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
}

impl PortableToolContextProvider for ToolUseContext {
    fn tool_context_facts(&self) -> ToolContextFacts {
        self.to_tool_context_facts()
    }
}

#[cfg(test)]
mod context_facts_tests {
    use super::ToolUseContext;
    use crate::agentic::WorkspaceBinding;
    use crate::agentic::tools::{
        PortableToolContextProvider, ToolRuntimeRestrictions, ToolWorkspaceKind,
    };
    use crate::service::remote_ssh::workspace_state::workspace_session_identity;
    use std::collections::{BTreeSet, HashMap};
    use std::path::PathBuf;

    fn local_context(root: &str) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new(None, PathBuf::from(root))),
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    #[test]
    fn tool_context_facts_preserve_portable_fields_without_runtime_handles() {
        let context = ToolUseContext {
            tool_call_id: Some("call-1".to_string()),
            agent_type: Some("Agentic".to_string()),
            session_id: Some("session-1".to_string()),
            dialog_turn_id: Some("turn-1".to_string()),
            workspace: Some(WorkspaceBinding::new(None, PathBuf::from("/repo/project"))),
            unlocked_collapsed_tools: vec!["WebFetch".to_string()],
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions {
                allowed_tool_names: BTreeSet::from(["Read".to_string()]),
                denied_tool_names: BTreeSet::from(["Bash".to_string()]),
                path_policy: Default::default(),
            },
            workspace_services: None,
        };

        let facts = context.to_tool_context_facts();

        assert_eq!(facts.tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(facts.agent_type.as_deref(), Some("Agentic"));
        assert_eq!(facts.session_id.as_deref(), Some("session-1"));
        assert_eq!(facts.dialog_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(facts.workspace_kind, Some(ToolWorkspaceKind::Local));
        assert_eq!(facts.workspace_root.as_deref(), Some("/repo/project"));
        assert!(facts.runtime_tool_restrictions.is_tool_allowed("Read"));
        assert!(!facts.runtime_tool_restrictions.is_tool_allowed("Bash"));

        let value = serde_json::to_value(&facts).expect("serialize context facts");
        assert!(value.get("unlockedCollapsedTools").is_none());
        assert!(value.get("computer_use_host").is_none());
        assert!(value.get("workspace_services").is_none());
        assert!(value.get("cancellation_token").is_none());
    }

    #[test]
    fn tool_context_facts_omit_runtime_owner_fields_even_when_context_is_populated() {
        let mut custom_data = HashMap::new();
        custom_data.insert(
            "checkpoint".to_string(),
            serde_json::json!({ "kind": "runtime-only" }),
        );

        let context = ToolUseContext {
            tool_call_id: Some("call-runtime".to_string()),
            agent_type: Some("Agentic".to_string()),
            session_id: Some("session-runtime".to_string()),
            dialog_turn_id: Some("turn-runtime".to_string()),
            workspace: Some(WorkspaceBinding::new(None, PathBuf::from("/repo/runtime"))),
            unlocked_collapsed_tools: vec!["WebFetch".to_string(), "Git".to_string()],
            custom_data,
            computer_use_host: None,
            cancellation_token: Some(tokio_util::sync::CancellationToken::new()),
            runtime_tool_restrictions: ToolRuntimeRestrictions {
                allowed_tool_names: BTreeSet::from(["Read".to_string(), "GetToolSpec".to_string()]),
                denied_tool_names: BTreeSet::from(["Bash".to_string()]),
                path_policy: Default::default(),
            },
            workspace_services: None,
        };

        let facts = PortableToolContextProvider::tool_context_facts(&context);

        assert_eq!(facts.tool_call_id.as_deref(), Some("call-runtime"));
        assert_eq!(facts.workspace_kind, Some(ToolWorkspaceKind::Local));
        assert_eq!(facts.workspace_root.as_deref(), Some("/repo/runtime"));
        assert!(facts.runtime_tool_restrictions.is_tool_allowed("Read"));
        assert!(
            facts
                .runtime_tool_restrictions
                .is_tool_allowed("GetToolSpec")
        );
        assert!(!facts.runtime_tool_restrictions.is_tool_allowed("Bash"));

        let value = serde_json::to_value(&facts).expect("serialize runtime context facts");
        for runtime_only_field in [
            "unlockedCollapsedTools",
            "customData",
            "computerUseHost",
            "cancellationToken",
            "workspaceServices",
        ] {
            assert!(
                value.get(runtime_only_field).is_none(),
                "{runtime_only_field} must remain outside portable facts"
            );
        }
    }

    #[test]
    fn tool_context_facts_use_normalized_remote_workspace_identity() {
        let session_identity = workspace_session_identity(
            "/home/wsp//projects/test/",
            Some("conn-1"),
            Some("ssh.dev"),
        )
        .expect("remote identity");
        let context = ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: Some("session-remote".to_string()),
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new_remote(
                Some("workspace-remote".to_string()),
                PathBuf::from("/home/wsp//projects/test/"),
                "conn-1".to_string(),
                "Dev SSH".to_string(),
                session_identity,
            )),
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        };

        let facts = context.to_tool_context_facts();

        assert_eq!(facts.workspace_kind, Some(ToolWorkspaceKind::Remote));
        assert_eq!(
            facts.workspace_root.as_deref(),
            Some("/home/wsp/projects/test")
        );

        let value = serde_json::to_value(&facts).expect("serialize remote context facts");
        assert!(value.get("connectionId").is_none());
        assert!(value.get("connectionName").is_none());
        assert!(value.get("workspace_services").is_none());
    }

    #[test]
    fn tool_use_context_implements_portable_context_provider() {
        fn assert_provider<T: PortableToolContextProvider>() {}
        assert_provider::<ToolUseContext>();

        let context = local_context("/repo/project");

        let facts = PortableToolContextProvider::tool_context_facts(&context);

        assert_eq!(facts.workspace_kind, Some(ToolWorkspaceKind::Local));
        assert_eq!(facts.workspace_root.as_deref(), Some("/repo/project"));
    }
}

/// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name
    fn name(&self) -> &str;

    /// Tool description
    async fn description(&self) -> BitFunResult<String>;

    /// Tool description with execution context.
    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        self.description().await
    }

    /// Short description used in condensed tool listings such as GetToolSpec.
    fn short_description(&self) -> String;

    /// Default exposure level when building the model tool manifest.
    ///
    /// This is tool-owned metadata: registries and agent manifests may use it
    /// as the baseline before applying any higher-level overrides.
    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Expanded
    }

    /// Input mode definition - using JSON Schema
    fn input_schema(&self) -> Value;

    /// JSON Schema sent to the model (may depend on app language or other runtime config).
    /// Default: same as [`input_schema`].
    async fn input_schema_for_model(&self) -> Value {
        self.input_schema()
    }

    /// JSON Schema for the model when tool listing has a [`ToolUseContext`] (e.g. primary model vision capability).
    /// Default: ignores context and delegates to [`input_schema_for_model`].
    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        let _ = context;
        self.input_schema_for_model().await
    }

    /// Input JSON Schema - optional extra schema
    fn input_json_schema(&self) -> Option<Value> {
        None
    }

    /// MCP Apps: URI of UI resource (ui://) declared in tool metadata. Used when tool result
    /// does not contain a resource - the host fetches from this pre-declared URI.
    fn ui_resource_uri(&self) -> Option<String> {
        None
    }

    /// Dynamic tool provider identity used by boundary adapters.
    ///
    /// Keep this as explicit metadata instead of deriving ownership from tool
    /// names so future tool registries can change naming without breaking
    /// provider routing.
    fn dynamic_provider_id(&self) -> Option<&str> {
        None
    }

    /// Rich metadata for dynamic tools. Prefer this over encoding dynamic ownership in tool names.
    fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> {
        self.dynamic_provider_id()
            .map(|provider_id| DynamicToolInfo {
                provider_id: provider_id.to_string(),
                provider_kind: None,
                mcp: None,
            })
    }

    /// User friendly name
    fn user_facing_name(&self) -> String {
        self.name().to_string()
    }

    /// Whether to enable
    async fn is_enabled(&self) -> bool {
        true
    }

    /// Whether this tool is available for a specific execution context.
    async fn is_available_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        self.is_enabled().await
    }

    /// Whether to be readonly
    fn is_readonly(&self) -> bool {
        false
    }

    /// Whether to be concurrency safe
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        self.is_readonly()
    }

    /// Whether to need permissions
    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        !self.is_readonly()
    }

    /// Whether this tool manages its own execution timeout (for example via the
    /// subagent timeout handle) and should not be wrapped by the global tool
    /// pipeline timeout.
    fn manages_own_execution_timeout(&self) -> bool {
        false
    }

    /// Whether to support streaming output
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Validate input
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

    /// Render result for assistant
    fn render_result_for_assistant(&self, _output: &Value) -> String {
        "Tool result".to_string()
    }

    /// Render tool use message
    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        format!("Using {}: {}", self.name(), input)
    }

    /// Render tool use rejected message
    fn render_tool_use_rejected_message(&self) -> String {
        format!("{} tool use was rejected", self.name())
    }

    /// Render tool result message
    fn render_tool_result_message(&self, _output: &Value) -> String {
        format!("{} completed", self.name())
    }

    /// Execute the tool's concrete business logic.
    /// Implementors should put the actual tool behavior here and assume
    /// [`call`] will wrap it with cross-cutting concerns such as cancellation.
    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>>;

    /// Unified tool entry point.
    /// This method owns shared framework behavior and delegates the actual
    /// execution to [`call_impl`], so most tools should override `call_impl`
    /// instead of overriding this method directly.
    async fn call(&self, input: &Value, context: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        crate::agentic::tools::tool_context_runtime::call_with_tool_runtime_hooks(
            self.name(),
            input,
            context,
            self.call_impl(input, context),
        )
        .await
    }
}

#[cfg(test)]
mod shared_context_tests {
    use super::{Tool, ToolResult, ToolUseContext};
    use crate::agentic::deep_review_policy::deep_review_shared_context_measurement_snapshot;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::util::errors::BitFunResult;
    use async_trait::async_trait;
    use serde_json::{Value, json};
    use std::collections::HashMap;

    struct MeasurementReadTool;

    #[async_trait]
    impl Tool for MeasurementReadTool {
        fn name(&self) -> &str {
            "Read"
        }

        async fn description(&self) -> BitFunResult<String> {
            Ok("Read file".to_string())
        }

        fn short_description(&self) -> String {
            "Read file".to_string()
        }

        fn input_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string" }
                }
            })
        }

        async fn call_impl(
            &self,
            _input: &Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<ToolResult>> {
            Ok(vec![ToolResult::ok(
                json!({ "ok": true }),
                Some("ok".to_string()),
            )])
        }
    }

    #[tokio::test]
    async fn call_records_deep_review_read_file_measurement_without_touching_result() {
        let parent_turn_id = format!("turn-framework-measure-{}", uuid::Uuid::new_v4());
        let mut custom_data = HashMap::new();
        custom_data.insert(
            "deep_review_parent_dialog_turn_id".to_string(),
            json!(parent_turn_id.clone()),
        );
        custom_data.insert("deep_review_subagent_role".to_string(), json!("reviewer"));
        custom_data.insert(
            "deep_review_subagent_type".to_string(),
            json!("ReviewSecurity"),
        );
        let context = ToolUseContext {
            tool_call_id: Some("tool-read".to_string()),
            agent_type: Some("ReviewSecurity".to_string()),
            session_id: Some("subagent-session".to_string()),
            dialog_turn_id: Some("subagent-turn".to_string()),
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            custom_data,
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        };
        let tool = MeasurementReadTool;

        let result = tool
            .call(&json!({ "file_path": ".\\src\\lib.rs" }), &context)
            .await
            .expect("read tool call should succeed");
        tool.call(&json!({ "file_path": "src/lib.rs" }), &context)
            .await
            .expect("read tool call should succeed");

        assert_eq!(result.len(), 1);
        let snapshot = deep_review_shared_context_measurement_snapshot(&parent_turn_id);
        assert_eq!(snapshot.total_calls, 2);
        assert_eq!(snapshot.duplicate_calls, 1);
        assert_eq!(snapshot.repeated_contexts[0].tool_name, "Read");
        assert_eq!(snapshot.repeated_contexts[0].file_path, "src/lib.rs");
    }
}
