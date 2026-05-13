//! Tool framework - Tool interface definition and execution context
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::session::EvidenceLedgerCheckpoint;
use crate::agentic::tools::post_call_hooks;
use crate::agentic::tools::restrictions::{
    is_local_path_within_root, is_remote_posix_path_within_root, ToolPathOperation,
    ToolRuntimeRestrictions,
};
use crate::agentic::tools::workspace_paths::{
    build_bitfun_runtime_uri, is_bitfun_runtime_uri, normalize_runtime_relative_path,
    parse_bitfun_runtime_uri,
};
use crate::agentic::workspace::WorkspaceServices;
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::get_path_manager_arc;
use crate::service::git::{GitDiffParams, GitService};
use crate::service::remote_ssh::workspace_state::remote_workspace_runtime_root;
use crate::service::{get_workspace_runtime_service_arc, WorkspaceRuntimeContext};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
pub use bitfun_agent_tools::{
    DynamicMcpToolInfo, DynamicToolInfo, ToolPathBackend, ToolPathResolution, ToolRenderOptions,
    ToolResult, ValidationResult,
};
use log::warn;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

/// Tool use context
#[derive(Debug, Clone)]
pub struct ToolUseContext {
    pub tool_call_id: Option<String>,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub dialog_turn_id: Option<String>,
    pub workspace: Option<WorkspaceBinding>,
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

    pub fn ws_fs(&self) -> Option<&dyn crate::agentic::workspace::WorkspaceFileSystem> {
        self.workspace_services.as_ref().map(|s| s.fs.as_ref())
    }

    pub fn ws_shell(&self) -> Option<&dyn crate::agentic::workspace::WorkspaceShell> {
        self.workspace_services.as_ref().map(|s| s.shell.as_ref())
    }

    pub async fn record_light_checkpoint(
        &self,
        tool_name: &str,
        target: &str,
        touched_files: Vec<String>,
    ) {
        let Some(session_id) = self.session_id.as_deref() else {
            return;
        };
        let Some(turn_id) = self.dialog_turn_id.as_deref() else {
            return;
        };
        let Some(coordinator) = get_global_coordinator() else {
            return;
        };

        let checkpoint = self.build_light_checkpoint(touched_files).await;
        coordinator
            .get_session_manager()
            .record_checkpoint_created(session_id, turn_id, tool_name, target, checkpoint);
    }

    async fn build_light_checkpoint(&self, touched_files: Vec<String>) -> EvidenceLedgerCheckpoint {
        let mut checkpoint = EvidenceLedgerCheckpoint {
            current_branch: None,
            dirty_state_summary: "workspace_unavailable".to_string(),
            touched_files,
            diff_hash: None,
        };

        if self.is_remote() {
            checkpoint.dirty_state_summary =
                "remote_workspace_git_metadata_unavailable".to_string();
            return checkpoint;
        }

        let Some(workspace_root) = self.workspace_root() else {
            return checkpoint;
        };

        match GitService::get_status(workspace_root).await {
            Ok(status) => {
                checkpoint.current_branch = Some(status.current_branch);
                checkpoint.dirty_state_summary = format!(
                    "staged={}, unstaged={}, untracked={}",
                    status.staged.len(),
                    status.unstaged.len(),
                    status.untracked.len()
                );
            }
            Err(error) => {
                checkpoint.dirty_state_summary = format!("git_status_unavailable: {}", error);
            }
        }

        checkpoint.diff_hash = self
            .checkpoint_diff_hash(workspace_root, &checkpoint.touched_files)
            .await;
        checkpoint
    }

    async fn checkpoint_diff_hash(
        &self,
        workspace_root: &Path,
        touched_files: &[String],
    ) -> Option<String> {
        let files = touched_files
            .iter()
            .filter_map(|file| git_relative_path(workspace_root, file))
            .collect::<Vec<_>>();

        if files.is_empty() {
            return None;
        }

        let mut diff = String::new();
        for staged in [false, true] {
            let params = GitDiffParams {
                files: Some(files.clone()),
                staged: Some(staged),
                ..Default::default()
            };
            match GitService::get_diff(workspace_root, &params).await {
                Ok(part) => diff.push_str(&part),
                Err(error) => {
                    warn!(
                        "Failed to collect checkpoint diff hash: staged={}, error={}",
                        staged, error
                    );
                    return None;
                }
            }
        }

        if diff.is_empty() {
            return None;
        }

        Some(hex::encode(Sha256::digest(diff.as_bytes())))
    }

    pub fn enforce_tool_runtime_restrictions(&self, tool_name: &str) -> BitFunResult<()> {
        self.runtime_tool_restrictions
            .ensure_tool_allowed(tool_name)
            .map_err(Into::into)
    }

    pub fn enforce_path_operation(
        &self,
        operation: ToolPathOperation,
        resolution: &ToolPathResolution,
    ) -> BitFunResult<()> {
        let allowed_roots = self
            .runtime_tool_restrictions
            .path_policy
            .roots_for(operation);
        if allowed_roots.is_empty() {
            return Ok(());
        }

        let mut resolved_roots = Vec::with_capacity(allowed_roots.len());
        for root in allowed_roots {
            resolved_roots.push(self.resolve_tool_path(root)?);
        }

        let mut is_allowed = false;
        for root in &resolved_roots {
            if root.backend != resolution.backend {
                continue;
            }

            let matches_root = match resolution.backend {
                ToolPathBackend::Local => is_local_path_within_root(
                    Path::new(&resolution.resolved_path),
                    Path::new(&root.resolved_path),
                )?,
                ToolPathBackend::RemoteWorkspace => {
                    is_remote_posix_path_within_root(&resolution.resolved_path, &root.resolved_path)
                }
            };

            if matches_root {
                is_allowed = true;
                break;
            }
        }

        if is_allowed {
            return Ok(());
        }

        Err(crate::util::errors::BitFunError::validation(format!(
            "Path '{}' is not allowed for {}. Allowed roots: {}",
            resolution.logical_path,
            operation.verb(),
            allowed_roots.join(", ")
        )))
    }

    /// Whether the session primary model accepts image inputs (from tool-definition / pipeline context).
    /// Defaults to **true** when unset (e.g. API listings without model metadata).
    pub fn primary_model_supports_image_understanding(&self) -> bool {
        self.custom_data
            .get("primary_model_supports_image_understanding")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    /// Resolve a user or model-supplied path for file/shell tools. Uses POSIX semantics when the
    /// workspace is remote SSH so Windows-hosted clients still resolve `/home/...` correctly.
    pub fn resolve_workspace_tool_path(&self, path: &str) -> BitFunResult<String> {
        let workspace_root_owned = self
            .workspace
            .as_ref()
            .map(|w| w.root_path_string())
            .ok_or_else(|| {
                crate::util::errors::BitFunError::tool(format!(
                    "A workspace path is required to resolve tool path: {}",
                    path
                ))
            })?;
        let resolved_path = crate::agentic::tools::workspace_paths::resolve_workspace_tool_path(
            path,
            Some(workspace_root_owned.as_str()),
            self.is_remote(),
        )?;

        let is_within_workspace = if self.is_remote() {
            is_remote_posix_path_within_root(&resolved_path, &workspace_root_owned)
        } else {
            is_local_path_within_root(Path::new(&resolved_path), Path::new(&workspace_root_owned))?
        };

        if !is_within_workspace {
            return Err(crate::util::errors::BitFunError::tool(format!(
                "Path '{}' resolves outside current workspace '{}': {}",
                path, workspace_root_owned, resolved_path
            )));
        }

        Ok(resolved_path)
    }

    pub fn current_workspace_runtime_root(&self) -> BitFunResult<PathBuf> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            crate::util::errors::BitFunError::tool(
                "A workspace is required to resolve runtime artifacts".to_string(),
            )
        })?;

        if workspace.is_remote() {
            let identity = &workspace.session_identity;
            Ok(remote_workspace_runtime_root(
                &identity.hostname,
                identity.logical_workspace_path(),
            ))
        } else {
            Ok(get_path_manager_arc().project_runtime_root(workspace.root_path()))
        }
    }

    pub fn current_workspace_scope(&self) -> Option<String> {
        self.workspace
            .as_ref()
            .and_then(|workspace| workspace.workspace_id.clone())
    }

    pub async fn ensure_current_workspace_runtime(&self) -> BitFunResult<WorkspaceRuntimeContext> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            crate::util::errors::BitFunError::tool(
                "A workspace is required to ensure runtime artifacts".to_string(),
            )
        })?;

        let runtime_service = get_workspace_runtime_service_arc();
        Ok(runtime_service
            .ensure_runtime_for_workspace_binding(workspace)
            .await?
            .context)
    }

    pub fn should_emit_runtime_uri(&self) -> bool {
        self.is_remote()
    }

    pub fn build_runtime_uri(&self, relative_path: &str) -> BitFunResult<String> {
        let scope = self
            .current_workspace_scope()
            .unwrap_or_else(|| "current".to_string());
        build_bitfun_runtime_uri(&scope, &normalize_runtime_relative_path(relative_path)?)
    }

    pub fn build_runtime_artifact_reference(&self, relative_path: &str) -> BitFunResult<String> {
        let normalized_relative_path = normalize_runtime_relative_path(relative_path)?;
        if self.should_emit_runtime_uri() {
            return self.build_runtime_uri(&normalized_relative_path);
        }

        let mut resolved_path = self.current_workspace_runtime_root()?;
        for segment in normalized_relative_path.split('/') {
            resolved_path.push(segment);
        }

        Ok(resolved_path.to_string_lossy().to_string())
    }

    pub fn build_session_runtime_artifact_reference(
        &self,
        session_id: &str,
        relative_path: &str,
    ) -> BitFunResult<String> {
        let normalized_relative_path = normalize_runtime_relative_path(relative_path)?;
        self.build_runtime_artifact_reference(&format!(
            "sessions/{}/{}",
            session_id, normalized_relative_path
        ))
    }

    pub fn current_workspace_session_dir(&self, session_id: &str) -> BitFunResult<PathBuf> {
        Ok(self
            .current_workspace_runtime_root()?
            .join("sessions")
            .join(session_id))
    }

    pub fn current_workspace_session_tool_results_dir(
        &self,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        Ok(self
            .current_workspace_session_dir(session_id)?
            .join("tool-results"))
    }

    pub fn current_workspace_session_tool_result_path(
        &self,
        session_id: &str,
        file_name: &str,
    ) -> BitFunResult<PathBuf> {
        Ok(self
            .current_workspace_session_tool_results_dir(session_id)?
            .join(file_name))
    }

    pub fn resolve_tool_path(&self, path: &str) -> BitFunResult<ToolPathResolution> {
        if is_bitfun_runtime_uri(path) {
            let parsed = parse_bitfun_runtime_uri(path)?;
            let workspace_scope = self.current_workspace_scope();
            let scope_matches = parsed.workspace_scope == "current"
                || workspace_scope.as_deref() == Some(parsed.workspace_scope.as_str());
            if !scope_matches {
                return Err(crate::util::errors::BitFunError::tool(format!(
                    "Runtime URI scope '{}' does not match the current workspace",
                    parsed.workspace_scope
                )));
            }

            let runtime_root = self.current_workspace_runtime_root()?;
            let mut resolved_path = runtime_root.clone();
            for segment in parsed.relative_path.split('/') {
                resolved_path.push(segment);
            }

            let effective_scope = workspace_scope.unwrap_or_else(|| parsed.workspace_scope.clone());
            let logical_path = build_bitfun_runtime_uri(&effective_scope, &parsed.relative_path)?;

            return Ok(ToolPathResolution {
                requested_path: path.to_string(),
                logical_path,
                resolved_path: resolved_path.to_string_lossy().to_string(),
                backend: ToolPathBackend::Local,
                runtime_scope: Some(effective_scope),
                runtime_root: Some(runtime_root),
            });
        }

        let resolved_path = self.resolve_workspace_tool_path(path)?;
        Ok(ToolPathResolution {
            requested_path: path.to_string(),
            logical_path: resolved_path.clone(),
            resolved_path,
            backend: if self.is_remote() {
                ToolPathBackend::RemoteWorkspace
            } else {
                ToolPathBackend::Local
            },
            runtime_scope: None,
            runtime_root: None,
        })
    }

    /// Whether `path` is absolute for the active workspace (POSIX `/` for remote SSH).
    pub fn workspace_path_is_effectively_absolute(&self, path: &str) -> bool {
        if is_bitfun_runtime_uri(path) {
            return true;
        }
        if self.is_remote() {
            crate::agentic::tools::workspace_paths::posix_style_path_is_absolute(path)
        } else {
            Path::new(path).is_absolute()
        }
    }
}

#[cfg(test)]
mod path_resolution_tests {
    use super::ToolUseContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn local_context(root: &str) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new(None, PathBuf::from(root))),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    fn context_without_workspace() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    #[test]
    fn workspace_path_resolution_rejects_absolute_paths_outside_local_workspace() {
        let context = local_context("/repo/project");

        let err = context
            .resolve_workspace_tool_path("/workspace")
            .expect_err("placeholder absolute paths must not escape the current workspace");

        assert!(err.to_string().contains("outside current workspace"));
    }

    #[test]
    fn workspace_path_resolution_rejects_root_without_workspace() {
        let context = context_without_workspace();

        let err = context
            .resolve_workspace_tool_path("/")
            .expect_err("workspace tools must not scan the host root without a workspace");

        assert!(err.to_string().contains("workspace path is required"));
    }

    #[test]
    fn workspace_path_resolution_allows_paths_inside_local_workspace() {
        let context = local_context("/repo/project");

        let resolved = context
            .resolve_workspace_tool_path("/repo/project/src/main.rs")
            .expect("absolute paths inside the workspace remain valid");

        assert_eq!(
            PathBuf::from(resolved),
            PathBuf::from("/repo/project/src/main.rs")
        );
    }
}

fn git_relative_path(workspace_root: &Path, path: &str) -> Option<String> {
    if is_bitfun_runtime_uri(path) {
        return None;
    }

    let path = Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(workspace_root).ok()?
    } else {
        path
    };

    Some(relative.to_string_lossy().replace('\\', "/"))
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
        let result = if let Some(cancellation_token) = context.cancellation_token.as_ref() {
            tokio::select! {
                result = self.call_impl(input, context) => {
                    result
                }

                _ = cancellation_token.cancelled() => {
                    Err(crate::util::errors::BitFunError::Cancelled("Tool execution cancelled".to_string()))
                }
            }
        } else {
            self.call_impl(input, context).await
        };
        if result.is_ok() {
            post_call_hooks::record_successful_tool_call(self.name(), input, context);
        }
        result
    }
}

#[cfg(test)]
mod shared_context_tests {
    use super::{Tool, ToolResult, ToolUseContext};
    use crate::agentic::deep_review_policy::deep_review_shared_context_measurement_snapshot;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::util::errors::BitFunResult;
    use async_trait::async_trait;
    use serde_json::{json, Value};
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
