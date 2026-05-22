//! Core-owned runtime bindings for `ToolUseContext`.
//!
//! This module intentionally keeps service handles, workspace runtime lookup,
//! path enforcement, cancellation/post-call hooks, and checkpoint recording in
//! core. The portable facts projection stays in `framework.rs` and
//! `bitfun-agent-tools`.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::deep_review::tool_context;
use crate::agentic::session::EvidenceLedgerCheckpoint;
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::framework::{
    ToolPathBackend, ToolPathResolution, ToolResult, ToolUseContext,
    build_tool_path_policy_denial_message, build_tool_runtime_artifact_reference,
    build_tool_session_runtime_artifact_reference, is_tool_path_allowed_by_resolved_roots,
    resolve_tool_path_with_context, tool_path_is_effectively_absolute,
};
use crate::agentic::tools::pipeline::{ToolExecutionContext, ToolTask};
use crate::agentic::tools::post_call_hooks;
use crate::agentic::tools::restrictions::{
    is_local_path_within_root, is_remote_posix_path_within_root, ToolPathOperation,
};
use crate::agentic::tools::workspace_paths::{
    build_bitfun_runtime_uri, is_bitfun_runtime_uri, normalize_runtime_relative_path,
};
use crate::agentic::tools::ToolRuntimeRestrictions;
use crate::agentic::workspace::WorkspaceServices;
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::get_path_manager_arc;
use crate::service::git::{GitDiffParams, GitService};
use crate::service::remote_ssh::workspace_state::remote_workspace_runtime_root;
use crate::service::{get_workspace_runtime_service_arc, WorkspaceRuntimeContext};
use crate::util::errors::{BitFunError, BitFunResult};
use log::warn;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

pub(crate) async fn call_with_tool_runtime_hooks(
    tool_name: &str,
    input: &Value,
    context: &ToolUseContext,
    call_impl: impl Future<Output = BitFunResult<Vec<ToolResult>>>,
) -> BitFunResult<Vec<ToolResult>> {
    let result = if let Some(cancellation_token) = context.cancellation_token.as_ref() {
        tokio::select! {
            result = call_impl => {
                result
            }

            _ = cancellation_token.cancelled() => {
                Err(BitFunError::Cancelled("Tool execution cancelled".to_string()))
            }
        }
    } else {
        call_impl.await
    };

    if result.is_ok() {
        post_call_hooks::record_successful_tool_call(tool_name, input, context);
    }

    result
}

pub(crate) fn build_tool_use_context_for_task(
    task: &ToolTask,
    computer_use_host: Option<ComputerUseHostRef>,
    cancellation_token: CancellationToken,
) -> ToolUseContext {
    build_tool_use_context_for_execution_context(
        &task.context,
        Some(task.tool_call.tool_id.clone()),
        computer_use_host,
        cancellation_token,
    )
}

pub(crate) fn build_tool_use_context_for_execution_context(
    context: &ToolExecutionContext,
    tool_call_id: Option<String>,
    computer_use_host: Option<ComputerUseHostRef>,
    cancellation_token: CancellationToken,
) -> ToolUseContext {
    ToolUseContext {
        tool_call_id,
        agent_type: Some(context.agent_type.clone()),
        session_id: Some(context.session_id.clone()),
        dialog_turn_id: Some(context.dialog_turn_id.clone()),
        workspace: context.workspace.clone(),
        unlocked_collapsed_tools: context.unlocked_collapsed_tools.clone(),
        custom_data: build_tool_context_custom_data(context),
        computer_use_host,
        cancellation_token: Some(cancellation_token),
        runtime_tool_restrictions: context.runtime_tool_restrictions.clone(),
        workspace_services: context.workspace_services.clone(),
    }
}

pub(crate) fn build_tool_description_context(
    agent_type: &str,
    workspace: Option<&WorkspaceBinding>,
    workspace_services: Option<&WorkspaceServices>,
    primary_supports_image_understanding: bool,
    context_vars: &HashMap<String, String>,
) -> ToolUseContext {
    let mut custom_data = HashMap::new();
    custom_data.insert(
        "primary_model_supports_image_understanding".to_string(),
        Value::Bool(primary_supports_image_understanding),
    );
    for (key, value) in context_vars {
        custom_data.insert(key.clone(), Value::String(value.clone()));
    }

    ToolUseContext {
        tool_call_id: None,
        agent_type: Some(agent_type.to_string()),
        session_id: None,
        dialog_turn_id: None,
        workspace: workspace.cloned(),
        unlocked_collapsed_tools: Vec::new(),
        custom_data,
        computer_use_host: None,
        cancellation_token: None,
        runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
        workspace_services: workspace_services.cloned(),
    }
}

pub(crate) fn build_write_preflight_context(
    agent_type: &str,
    session_id: &str,
    dialog_turn_id: &str,
    workspace: Option<WorkspaceBinding>,
    unlocked_collapsed_tools: Vec<String>,
    runtime_tool_restrictions: ToolRuntimeRestrictions,
    workspace_services: Option<WorkspaceServices>,
) -> ToolUseContext {
    ToolUseContext {
        tool_call_id: None,
        agent_type: Some(agent_type.to_string()),
        session_id: Some(session_id.to_string()),
        dialog_turn_id: Some(dialog_turn_id.to_string()),
        workspace,
        unlocked_collapsed_tools,
        custom_data: HashMap::new(),
        computer_use_host: None,
        cancellation_token: None,
        runtime_tool_restrictions,
        workspace_services,
    }
}

fn build_tool_context_custom_data(context: &ToolExecutionContext) -> HashMap<String, Value> {
    let mut map = HashMap::new();

    if let Some(turn_index) = context.context_vars.get("turn_index") {
        if let Ok(n) = turn_index.parse::<u64>() {
            map.insert("turn_index".to_string(), serde_json::json!(n));
        }
    }

    if let Some(provider) = context.context_vars.get("primary_model_provider") {
        if !provider.is_empty() {
            map.insert(
                "primary_model_provider".to_string(),
                serde_json::json!(provider),
            );
        }
    }
    if let Some(supports_images) = context
        .context_vars
        .get("primary_model_supports_image_understanding")
    {
        if let Ok(flag) = supports_images.parse::<bool>() {
            map.insert(
                "primary_model_supports_image_understanding".to_string(),
                serde_json::json!(flag),
            );
        }
    }
    if let Some(write_tool_mode) = context.context_vars.get("write_tool_mode") {
        if !write_tool_mode.is_empty() {
            map.insert(
                "write_tool_mode".to_string(),
                serde_json::json!(write_tool_mode),
            );
        }
    }
    if let Some(acp_transport) = context.context_vars.get("acp_transport") {
        if let Ok(flag) = acp_transport.parse::<bool>() {
            map.insert("acp_transport".to_string(), serde_json::json!(flag));
        }
    }

    let deep_review_parent_context = context.subagent_parent_info.as_ref().map(|parent_info| {
        tool_context::DeepReviewToolParentContext {
            tool_call_id: parent_info.tool_call_id.as_str(),
            session_id: parent_info.session_id.as_str(),
            dialog_turn_id: parent_info.dialog_turn_id.as_str(),
        }
    });
    tool_context::append_tool_use_context_data(
        &context.context_vars,
        deep_review_parent_context,
        &mut map,
    );

    map
}

impl ToolUseContext {
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

        let is_allowed = is_tool_path_allowed_by_resolved_roots(
            resolution,
            &resolved_roots,
            |resolution, root| -> BitFunResult<bool> {
                match resolution.backend {
                    ToolPathBackend::Local => is_local_path_within_root(
                        Path::new(&resolution.resolved_path),
                        Path::new(&root.resolved_path),
                    ),
                    ToolPathBackend::RemoteWorkspace => Ok(is_remote_posix_path_within_root(
                        &resolution.resolved_path,
                        &root.resolved_path,
                    )),
                }
            },
        )?;

        if is_allowed {
            return Ok(());
        }

        Err(BitFunError::validation(
            build_tool_path_policy_denial_message(
                &resolution.logical_path,
                operation,
                allowed_roots,
            ),
        ))
    }

    /// Resolve a user or model-supplied path for file/shell tools. Uses POSIX semantics when the
    /// workspace is remote SSH so Windows-hosted clients still resolve `/home/...` correctly.
    pub fn resolve_workspace_tool_path(&self, path: &str) -> BitFunResult<String> {
        let workspace_root_owned = self
            .workspace
            .as_ref()
            .map(|w| w.root_path_string())
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "A workspace path is required to resolve tool path: {}",
                    path
                ))
            })?;
        let resolved_path = crate::agentic::tools::workspace_paths::resolve_workspace_tool_path(
            path,
            Some(workspace_root_owned.as_str()),
            self.is_remote(),
        )?;

        // Remote SSH workspaces stay contained to the opened project tree. Local desktop
        // sessions may use any host path the OS user can access (Bash already has the same
        // reach); optional `path_policy` roots still apply via `enforce_path_operation`.
        if self.is_remote()
            && !is_remote_posix_path_within_root(&resolved_path, &workspace_root_owned)
        {
            return Err(BitFunError::tool(format!(
                "Path '{}' resolves outside current workspace '{}': {}",
                path, workspace_root_owned, resolved_path
            )));
        }

        Ok(resolved_path)
    }

    pub fn current_workspace_runtime_root(&self) -> BitFunResult<PathBuf> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            BitFunError::tool("A workspace is required to resolve runtime artifacts".to_string())
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
            BitFunError::tool("A workspace is required to ensure runtime artifacts".to_string())
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
        let runtime_root = if self.should_emit_runtime_uri() {
            None
        } else {
            Some(self.current_workspace_runtime_root()?)
        };
        build_tool_runtime_artifact_reference(
            relative_path,
            runtime_root.as_deref(),
            self.current_workspace_scope().as_deref(),
            self.should_emit_runtime_uri(),
        )
        .map_err(|error| BitFunError::tool(error.to_string()))
    }

    pub fn build_session_runtime_artifact_reference(
        &self,
        session_id: &str,
        relative_path: &str,
    ) -> BitFunResult<String> {
        let runtime_root = if self.should_emit_runtime_uri() {
            None
        } else {
            Some(self.current_workspace_runtime_root()?)
        };
        build_tool_session_runtime_artifact_reference(
            session_id,
            relative_path,
            runtime_root.as_deref(),
            self.current_workspace_scope().as_deref(),
            self.should_emit_runtime_uri(),
        )
        .map_err(|error| BitFunError::tool(error.to_string()))
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
            let workspace_scope = self.current_workspace_scope();
            let runtime_root = if self.workspace.is_some() {
                Some(self.current_workspace_runtime_root()?)
            } else {
                None
            };
            return resolve_tool_path_with_context(
                path,
                None,
                self.is_remote(),
                workspace_scope.as_deref(),
                runtime_root,
            )
            .map_err(|error| BitFunError::tool(error.to_string()));
        }

        let workspace_root_owned = self
            .workspace
            .as_ref()
            .map(|w| w.root_path_string())
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "A workspace path is required to resolve tool path: {}",
                    path
                ))
            })?;

        resolve_tool_path_with_context(
            path,
            Some(workspace_root_owned.as_str()),
            self.is_remote(),
            self.current_workspace_scope().as_deref(),
            None,
        )
        .map_err(|error| BitFunError::tool(error.to_string()))
    }

    /// Whether `path` is absolute for the active workspace (POSIX `/` for remote SSH).
    pub fn workspace_path_is_effectively_absolute(&self, path: &str) -> bool {
        tool_path_is_effectively_absolute(path, self.is_remote())
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

#[cfg(test)]
mod path_resolution_tests {
    use crate::agentic::tools::framework::ToolUseContext;
    use crate::agentic::tools::{ToolPathOperation, ToolPathPolicy, ToolRuntimeRestrictions};
    use crate::agentic::WorkspaceBinding;
    use crate::service::remote_ssh::workspace_state::workspace_session_identity;
    use std::collections::HashMap;
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

    fn remote_context(root: &str, workspace_id: Option<String>) -> ToolUseContext {
        let session_identity = workspace_session_identity(root, Some("conn-1"), Some("ssh.dev"))
            .expect("remote identity");
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new_remote(
                workspace_id,
                PathBuf::from(root),
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
        }
    }

    fn context_with_restrictions(
        root: &str,
        runtime_tool_restrictions: ToolRuntimeRestrictions,
    ) -> ToolUseContext {
        ToolUseContext {
            runtime_tool_restrictions,
            ..local_context(root)
        }
    }

    fn context_without_workspace() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    #[test]
    fn workspace_path_resolution_allows_absolute_paths_outside_local_workspace() {
        let context = local_context("/repo/project");

        let resolved = context
            .resolve_workspace_tool_path("/tmp/pr_body.md")
            .expect("local sessions may resolve paths outside the workspace root");

        assert_eq!(PathBuf::from(resolved), PathBuf::from("/tmp/pr_body.md"));
    }

    #[test]
    fn workspace_path_resolution_rejects_absolute_paths_outside_remote_workspace() {
        let context = remote_context("/home/wsp/projects/test", None);

        let err = context
            .resolve_workspace_tool_path("/tmp/pr_body.md")
            .expect_err("remote sessions must stay within the workspace root");

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

    #[test]
    fn remote_runtime_artifact_reference_uses_runtime_uri_scope() {
        let context = remote_context("/home/wsp/projects/test", Some("workspace-123".to_string()));

        let reference = context
            .build_runtime_artifact_reference(r"plans\demo.plan.md")
            .expect("remote runtime artifacts should use URI references");

        assert_eq!(
            reference,
            "bitfun://runtime/workspace-123/plans/demo.plan.md"
        );
    }

    #[test]
    fn runtime_uri_resolution_rejects_different_workspace_scope() {
        let context = remote_context("/home/wsp/projects/test", Some("workspace-123".to_string()));

        let err = context
            .resolve_tool_path("bitfun://runtime/workspace-456/plans/demo.plan.md")
            .expect_err("runtime artifact scopes must match the active workspace");

        assert!(err
            .to_string()
            .contains("does not match the current workspace"));
    }

    #[test]
    fn runtime_uri_scope_error_takes_precedence_without_workspace() {
        let context = context_without_workspace();

        let err = context
            .resolve_tool_path("bitfun://runtime/workspace-456/plans/demo.plan.md")
            .expect_err("runtime URI scope should be validated before runtime root lookup");

        assert!(
            err.to_string()
                .contains("does not match the current workspace")
        );
    }

    #[test]
    fn workspace_absolute_detection_uses_remote_posix_semantics() {
        let context = remote_context("/home/wsp/projects/test", None);

        assert!(
            context.workspace_path_is_effectively_absolute("/home/wsp/projects/test/src/lib.rs")
        );
        assert!(!context.workspace_path_is_effectively_absolute("src/lib.rs"));
    }

    #[test]
    fn path_policy_allows_only_configured_local_roots() {
        let temp_root = std::env::temp_dir().join(format!(
            "bitfun-tool-context-policy-{}",
            uuid::Uuid::new_v4()
        ));
        let allowed_root = temp_root.join("allowed");
        std::fs::create_dir_all(&allowed_root).expect("create allowed root");
        let context = context_with_restrictions(
            temp_root.to_string_lossy().as_ref(),
            ToolRuntimeRestrictions {
                path_policy: ToolPathPolicy {
                    write_roots: vec![allowed_root.to_string_lossy().to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let allowed = context
            .resolve_tool_path(&allowed_root.join("file.txt").to_string_lossy())
            .expect("allowed path should resolve");
        context
            .enforce_path_operation(ToolPathOperation::Write, &allowed)
            .expect("path within configured root should be allowed");

        let blocked = context
            .resolve_tool_path(&temp_root.join("blocked/file.txt").to_string_lossy())
            .expect("blocked path should still resolve before policy enforcement");
        let err = context
            .enforce_path_operation(ToolPathOperation::Write, &blocked)
            .expect_err("path outside configured root should be blocked");

        assert!(err.to_string().contains("is not allowed for write"));

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}

#[cfg(test)]
mod call_runtime_tests {
    use super::call_with_tool_runtime_hooks;
    use crate::agentic::tools::framework::{ToolResult, ToolUseContext};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::util::errors::{BitFunError, BitFunResult};
    use serde_json::json;
    use std::collections::HashMap;
    use tokio::time::{sleep, Duration};
    use tokio_util::sync::CancellationToken;

    fn context_with_cancellation(cancellation_token: CancellationToken) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: Some(cancellation_token),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    #[tokio::test]
    async fn tool_call_runtime_hook_returns_cancelled_before_impl_completes() {
        let cancellation_token = CancellationToken::new();
        cancellation_token.cancel();
        let context = context_with_cancellation(cancellation_token);

        let result = call_with_tool_runtime_hooks("Read", &json!({}), &context, async {
            sleep(Duration::from_secs(30)).await;
            Ok(vec![ToolResult::ok(json!({ "unexpected": true }), None)])
        })
        .await;

        assert!(
            matches!(result, Err(BitFunError::Cancelled(message)) if message == "Tool execution cancelled")
        );
    }

    #[tokio::test]
    async fn tool_call_runtime_hook_preserves_success_result_without_cancellation() {
        let context = ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        };

        let result: BitFunResult<Vec<ToolResult>> =
            call_with_tool_runtime_hooks("Read", &json!({}), &context, async {
                Ok(vec![ToolResult::ok(
                    json!({ "ok": true }),
                    Some("ok".to_string()),
                )])
            })
            .await;

        let result = result.expect("tool result should pass through");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content()["ok"], true);
    }
}

#[cfg(test)]
mod context_builder_tests {
    use super::{build_tool_description_context, build_write_preflight_context};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use serde_json::json;
    use std::collections::{BTreeSet, HashMap};

    #[test]
    fn tool_description_context_preserves_manifest_custom_data_shape() {
        let mut context_vars = HashMap::new();
        context_vars.insert("write_tool_mode".to_string(), "inline_content".to_string());
        context_vars.insert(
            "primary_model_supports_image_understanding".to_string(),
            "false".to_string(),
        );

        let context = build_tool_description_context("coding", None, None, true, &context_vars);

        assert_eq!(context.agent_type.as_deref(), Some("coding"));
        assert!(context.tool_call_id.is_none());
        assert!(context.session_id.is_none());
        assert!(context.dialog_turn_id.is_none());
        assert!(context.workspace.is_none());
        assert!(context.unlocked_collapsed_tools.is_empty());
        assert!(context.cancellation_token.is_none());
        assert!(context.workspace_services.is_none());
        assert!(context.runtime_tool_restrictions.is_tool_allowed("Write"));
        assert_eq!(
            context.custom_data["primary_model_supports_image_understanding"],
            json!("false")
        );
        assert_eq!(
            context.custom_data["write_tool_mode"],
            json!("inline_content")
        );
    }

    #[test]
    fn write_preflight_context_preserves_minimal_runtime_fields() {
        let restrictions = ToolRuntimeRestrictions {
            allowed_tool_names: BTreeSet::from(["Write".to_string()]),
            denied_tool_names: BTreeSet::from(["Delete".to_string()]),
            path_policy: Default::default(),
        };

        let context = build_write_preflight_context(
            "coding",
            "session-1",
            "turn-1",
            None,
            vec!["Write".to_string()],
            restrictions,
            None,
        );

        assert_eq!(context.agent_type.as_deref(), Some("coding"));
        assert_eq!(context.session_id.as_deref(), Some("session-1"));
        assert_eq!(context.dialog_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(context.unlocked_collapsed_tools, vec!["Write"]);
        assert!(context.tool_call_id.is_none());
        assert!(context.custom_data.is_empty());
        assert!(context.cancellation_token.is_none());
        assert!(context.workspace_services.is_none());
        assert!(context.runtime_tool_restrictions.is_tool_allowed("Write"));
        assert!(!context.runtime_tool_restrictions.is_tool_allowed("Delete"));
    }
}

#[cfg(test)]
mod task_context_tests {
    use super::build_tool_use_context_for_task;
    use crate::agentic::core::ToolCall;
    use crate::agentic::tools::pipeline::{
        SubagentParentInfo, ToolExecutionContext, ToolExecutionOptions, ToolTask,
    };
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use serde_json::json;
    use std::collections::{BTreeSet, HashMap};
    use tokio_util::sync::CancellationToken;

    fn task_with_context_vars() -> ToolTask {
        let mut context_vars = HashMap::new();
        context_vars.insert("turn_index".to_string(), "7".to_string());
        context_vars.insert("primary_model_provider".to_string(), "openai".to_string());
        context_vars.insert(
            "primary_model_supports_image_understanding".to_string(),
            "true".to_string(),
        );
        context_vars.insert("write_tool_mode".to_string(), "inline_content".to_string());
        context_vars.insert("acp_transport".to_string(), "true".to_string());
        context_vars.insert(
            "deep_review_run_manifest".to_string(),
            r#"{"run_id":"run-1"}"#.to_string(),
        );
        context_vars.insert(
            "deep_review_subagent_role".to_string(),
            "reviewer".to_string(),
        );
        context_vars.insert(
            "deep_review_subagent_type".to_string(),
            "ReviewSecurity".to_string(),
        );

        ToolTask::new(
            ToolCall {
                tool_id: "tool_context_1".to_string(),
                tool_name: "WebFetch".to_string(),
                arguments: json!({ "url": "https://example.com" }),
                raw_arguments: None,
                is_error: false,
                recovered_from_truncation: false,
            },
            ToolExecutionContext {
                session_id: "session_1".to_string(),
                dialog_turn_id: "turn_1".to_string(),
                round_id: "round_1".to_string(),
                agent_type: "agent".to_string(),
                workspace: None,
                context_vars,
                subagent_parent_info: Some(SubagentParentInfo {
                    tool_call_id: "parent_tool".to_string(),
                    session_id: "parent_session".to_string(),
                    dialog_turn_id: "parent_turn".to_string(),
                }),
                collapsed_tools: vec!["WebFetch".to_string()],
                unlocked_collapsed_tools: vec!["WebFetch".to_string()],
                allowed_tools: vec!["WebFetch".to_string()],
                runtime_tool_restrictions: ToolRuntimeRestrictions {
                    allowed_tool_names: BTreeSet::from(["WebFetch".to_string()]),
                    denied_tool_names: BTreeSet::from(["Bash".to_string()]),
                    path_policy: Default::default(),
                },
                steering_interrupt: None,
                workspace_services: None,
            },
            ToolExecutionOptions::default(),
        )
    }

    #[test]
    fn tool_task_context_materialization_preserves_runtime_fields() {
        let task = task_with_context_vars();

        let context = build_tool_use_context_for_task(&task, None, CancellationToken::new());

        assert_eq!(context.tool_call_id.as_deref(), Some("tool_context_1"));
        assert_eq!(context.agent_type.as_deref(), Some("agent"));
        assert_eq!(context.session_id.as_deref(), Some("session_1"));
        assert_eq!(context.dialog_turn_id.as_deref(), Some("turn_1"));
        assert_eq!(context.unlocked_collapsed_tools, vec!["WebFetch"]);
        assert!(context.cancellation_token.is_some());
        assert!(context
            .runtime_tool_restrictions
            .is_tool_allowed("WebFetch"));
        assert!(!context.runtime_tool_restrictions.is_tool_allowed("Bash"));
        assert_eq!(context.custom_data["turn_index"], json!(7));
        assert_eq!(
            context.custom_data["primary_model_provider"],
            json!("openai")
        );
        assert_eq!(
            context.custom_data["primary_model_supports_image_understanding"],
            json!(true)
        );
        assert_eq!(
            context.custom_data["write_tool_mode"],
            json!("inline_content")
        );
        assert_eq!(context.custom_data["acp_transport"], json!(true));
        assert_eq!(
            context.custom_data["deep_review_run_manifest"],
            json!({ "run_id": "run-1" })
        );
        assert_eq!(
            context.custom_data["deep_review_parent_tool_call_id"],
            json!("parent_tool")
        );
        assert_eq!(
            context.custom_data["deep_review_parent_session_id"],
            json!("parent_session")
        );
        assert_eq!(
            context.custom_data["deep_review_parent_dialog_turn_id"],
            json!("parent_turn")
        );

        let facts = context.to_tool_context_facts();
        let value = serde_json::to_value(&facts).expect("serialize context facts");
        assert_eq!(value["toolCallId"], "tool_context_1");
        assert_eq!(value["sessionId"], "session_1");
        assert!(value.get("unlockedCollapsedTools").is_none());
        assert!(value.get("customData").is_none());
        assert!(value.get("cancellationToken").is_none());
    }
}
