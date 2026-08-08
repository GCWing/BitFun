//! SessionControl manages persisted workspace-scoped sessions.
//!
//! The `cancel` action only cancels the target session's current running dialog turn.
//! It does not permanently stop the session itself, and it does not clear queued
//! messages that may still run later through the scheduler.

use super::util::normalize_path;
use crate::agentic::agents::{get_agent_registry, AcpAgent};
use crate::agentic::coordination::{get_global_coordinator, get_global_scheduler};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::restrictions::{get_session_role, validate_delegation, AgentRole};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_agent_runtime::session_control::{
    compact_session_display_name, render_session_control_tool_use_message,
    resolve_session_control_cancel_route, session_control_agent_type_or_default,
    session_control_cancel_result_message, session_control_cancel_status,
    session_control_created_result_message, session_control_creator_marker,
    session_control_deleted_result_message, session_control_session_name_or_default,
    validate_session_control_input, validate_session_id, SessionControlAction,
    SessionControlCancelRoute, SessionControlInput, SessionControlValidationContext,
    SessionControlValidationResult,
};
use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::{
    AcpClientCreateRequest, AcpClientCreateResult, AcpClientPort, AgentSessionCreateRequest,
    AgentSessionListRequest, AgentSessionSummary, AgentSessionWorkspaceBinding,
    AgentSessionWorkspaceRequest, AgentSubmissionSource, AgentTurnCancellationRequest,
};
use bitfun_services_core::session::merge_session_custom_metadata;
use bitfun_services_core::session::tree::SessionTreeManager;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

/// SessionControl tool - create, cancel, delete, or list persisted sessions
/// list: list persistent sessions created by SessionControl.
/// list_tasks: list child conversation sessions spawned by Task.
pub struct SessionControlTool;

const CANCEL_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub(crate) struct SessionControlWorkspaceTarget {
    display_workspace: String,
    project_workspace: String,
    execution_target: Option<SessionExecutionTarget>,
    workspace_id: Option<String>,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
}

impl Default for SessionControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionControlTool {
    pub fn new() -> Self {
        Self
    }

    fn current_workspace_session<'a>(
        &self,
        context: &'a ToolUseContext,
        workspace: &str,
    ) -> Option<&'a str> {
        let current_session_id = context.session_id.as_deref()?;
        let current_workspace = context.workspace_root()?;
        let normalized_current_workspace =
            normalize_path(current_workspace.to_string_lossy().as_ref());

        if normalized_current_workspace == workspace {
            Some(current_session_id)
        } else {
            None
        }
    }

    fn creator_session_marker(&self, context: &ToolUseContext) -> BitFunResult<String> {
        let creator_session_id = context.session_id.as_ref().ok_or_else(|| {
            BitFunError::tool("create requires a creator session in tool context".to_string())
        })?;
        Ok(session_control_creator_marker(creator_session_id))
    }

    /// Real ACP session creation: create an external ACP flow session via
    /// AcpClientPort (returns the `acp_<client>_<uuid>` session id +
    /// `acp:<client>` agent type), equivalent to the frontend
    /// `create_acp_flow_session` / desktop `AcpClientPort::create_session` —
    /// persisted record + external process startup + failure rollback
    /// (desktop acp_client_port.rs:97-149). Does not create a local internal
    /// session, so it does not write createdBy/subagent metadata, does not
    /// persist SessionRelationship, and does not mount the legion tree; the
    /// legion side communicates via SessionMessage direct path (acp: flow
    /// session fork) with the returned session_id.
    async fn create_acp_session_via_port(
        &self,
        workspace: &SessionControlWorkspaceTarget,
        client_id: &str,
        session_name: Option<String>,
        port: &dyn AcpClientPort,
    ) -> BitFunResult<AcpClientCreateResult> {
        port.create_session(AcpClientCreateRequest {
            client_id: client_id.to_string(),
            workspace_path: workspace.display_workspace.clone(),
            session_name,
            remote_connection_id: workspace.remote_connection_id.clone(),
        })
        .await
        .map_err(|error| {
            BitFunError::tool(format!(
                "ACP client port failed ({:?}): {}",
                error.kind, error.message
            ))
        })
    }

    async fn resolve_effective_workspace(
        &self,
        action: SessionControlAction,
        session_id: Option<&str>,
        workspace_param: Option<&str>,
        context: &ToolUseContext,
        runtime: &AgentRuntime,
    ) -> BitFunResult<SessionControlWorkspaceTarget> {
        match action {
            SessionControlAction::Cancel
            | SessionControlAction::Delete
            | SessionControlAction::Compact => {
                let session_id = session_id.ok_or_else(|| {
                    BitFunError::tool(format!("session_id is required for {}", action.as_str()))
                })?;
                if let Some(binding) = runtime
                    .resolve_session_workspace_binding(AgentSessionWorkspaceRequest {
                        session_id: session_id.to_string(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?
                {
                    return Ok(Self::workspace_target_from_binding(binding));
                }
                Err(BitFunError::NotFound(format!(
                    "Workspace for session '{}' could not be resolved",
                    session_id
                )))
            }
            SessionControlAction::Create | SessionControlAction::List => {
                // Explicit workspace parameter wins; fall back to the current
                // workspace binding from context when omitted, so the tool can
                // list/create across workspaces.
                if let Some(workspace) = workspace_param {
                    return Ok(SessionControlWorkspaceTarget {
                        display_workspace: normalize_path(workspace),
                        project_workspace: normalize_path(workspace),
                        execution_target: None,
                        workspace_id: None,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    });
                }
                let workspace = context.workspace.as_ref().ok_or_else(|| {
                    BitFunError::tool(format!(
                        "workspace is required for {} when the current workspace is unavailable",
                        action.as_str()
                    ))
                })?;
                Ok(Self::workspace_target_from_context(workspace))
            }
        }
    }

    pub(crate) fn workspace_target_from_context(
        workspace: &crate::agentic::WorkspaceBinding,
    ) -> SessionControlWorkspaceTarget {
        SessionControlWorkspaceTarget {
            display_workspace: normalize_path(&workspace.root_path_string()),
            project_workspace: normalize_path(&workspace.project_root_path_string()),
            execution_target: workspace.execution_target.clone(),
            workspace_id: workspace.workspace_id.clone(),
            remote_connection_id: workspace.connection_id().map(ToOwned::to_owned),
            remote_ssh_host: if workspace.is_remote() {
                Some(workspace.session_identity.hostname.clone())
                    .filter(|value| !value.trim().is_empty())
            } else {
                None
            },
        }
    }

    fn workspace_target_from_binding(
        binding: AgentSessionWorkspaceBinding,
    ) -> SessionControlWorkspaceTarget {
        let project_workspace = binding
            .project_workspace_path
            .clone()
            .unwrap_or_else(|| binding.workspace_path.clone());
        SessionControlWorkspaceTarget {
            display_workspace: binding.workspace_path,
            project_workspace,
            execution_target: binding.execution_target,
            workspace_id: binding.workspace_id,
            remote_connection_id: binding.remote_connection_id,
            remote_ssh_host: binding.remote_ssh_host,
        }
    }

    fn validation_context(context: Option<&ToolUseContext>) -> SessionControlValidationContext<'_> {
        SessionControlValidationContext {
            current_session_id: context.and_then(|value| value.session_id.as_deref()),
            has_workspace_root: context.and_then(|value| value.workspace_root()).is_some(),
        }
    }

    fn into_validation_result(result: SessionControlValidationResult) -> ValidationResult {
        ValidationResult {
            result: result.result,
            message: result.message,
            error_code: result.error_code,
            meta: result.meta,
        }
    }

    #[allow(dead_code)]
    async fn ensure_session_exists(
        &self,
        runtime: &AgentRuntime,
        workspace: &SessionControlWorkspaceTarget,
        session_id: &str,
    ) -> BitFunResult<()> {
        let existing_sessions = runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: workspace.project_workspace.clone(),
                remote_connection_id: workspace.remote_connection_id.clone(),
                remote_ssh_host: workspace.remote_ssh_host.clone(),
                include_hidden: false,
            })
            .await
            .map_err(|error| {
                BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
            })?;
        if existing_sessions
            .iter()
            .any(|session| session.session_id == session_id)
        {
            Ok(())
        } else {
            Err(BitFunError::NotFound(format!(
                "Session '{}' not found in workspace '{}'",
                session_id, workspace.display_workspace
            )))
        }
    }

    /// Build the `result_for_assistant` text for the `list` action.
    ///
    /// Default (`detail == false`) is the compact tree output: one line per
    /// session with `sessionId | agentType | status | compact name` so the
    /// model context stays small even when session names are long task
    /// descriptions. Full session names (and the JSON tree) are still
    /// available through the `data` payload and through `detail == true`,
    /// which preserves the legacy verbose tree output.
    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        sessions: &[AgentSessionSummary],
        current_session_id: Option<&str>,
        tree: Option<&SessionTreeManager>,
        short_names: &HashMap<String, Option<String>>,
        detail: bool,
    ) -> String {
        if sessions.is_empty() {
            return format!("No sessions found in workspace '{}'.", workspace);
        }

        let mut lines = vec![format!(
            "Found {} session(s) in workspace '{}'",
            sessions.len(),
            workspace
        )];
        lines.push(String::new());
        if let Some(current_session_id) = current_session_id {
            lines.push(format!("Note: '{}' is your session_id", current_session_id));
            lines.push(String::new());
        }

        if detail {
            // --- Full tree JSON view (legacy verbose output) ---
            // The full `sessions` array and parsed `tree` remain available in the
            // result `data` payload for programmatic consumers.
            lines.push("## Session Tree (JSON)".to_string());
            lines.push("```json".to_string());
            lines.push(self.build_session_tree_json(sessions, tree));
            lines.push("```".to_string());
        } else {
            // --- Compact tree text view (default) ---
            lines.push("## Sessions (compact)".to_string());
            lines.push("format: [sessionId] agentType | status | name".to_string());
            lines.extend(build_compact_tree_lines(sessions, tree, short_names));
        }
        lines.join("\n")
    }

    /// Build a JSON tree structure from the flat session list.
    /// Sessions are grouped by `parent_session_id` into a forest of root nodes.
    fn build_session_tree_json(
        &self,
        sessions: &[AgentSessionSummary],
        tree: Option<&SessionTreeManager>,
    ) -> String {
        build_session_tree_json_impl(sessions, tree)
    }
}

/// Shared source for the agent_type enum of SessionControl/SessionMessage
/// create (and LegionControl load validation).
///
/// Returns every agent id that can back a created session: builtin/user
/// subagents, project subagents of the current workspace, builtin/user modes
/// and ACP bridge agents (`acp__<client_id>`). Unlike the TaskVisible query,
/// this deliberately includes Mode-category entries so external ACP
/// conversations are selectable; the create path validates the final value
/// through the registry anyway.
pub(crate) async fn get_available_agent_type_ids_for_creation(
    context: Option<&ToolUseContext>,
) -> Vec<String> {
    use crate::agentic::agents::get_agent_registry;
    let registry = get_agent_registry();
    let workspace_root = context.and_then(|ctx| ctx.workspace_root());
    registry.load_custom_agents(workspace_root).await;
    registry
        .get_agent_ids_for_session_creation(workspace_root)
        .await
}

/// R-26 / user-owner semantics: whether a calling session is exempt from the
/// R-2 created_by/ancestor authorization gate for session deletion.
///
/// The human user's main session (Commander role) is the owner and may delete
/// any session, including orphaned or detached children whose lineage was
/// broken by an earlier external deletion. When the RBAC master switch is off,
/// the gate is bypassed entirely.
fn caller_is_owner_session(caller_session_id: &str) -> bool {
    matches!(
        get_session_role(caller_session_id),
        Some(AgentRole::Commander)
    ) || !crate::service::config::rbac_enabled()
}

/// Dependency-free canonical uuid shape guard (8-4-4-4-12, 36 chars) for the
/// trailing segment of an ACP flow session id (`acp_<client_id>_<uuid>`).
/// Matches the strict checks used by the desktop `AcpClientPort`
/// (`client_id_from_session_id`) and `SessionMessage`
/// (`acp_flow_client_id_from_session_id`), so an internal session id that
/// merely starts with `acp_` is never mistaken for a flow session.
pub(crate) fn looks_like_uuid(segment: &str) -> bool {
    segment.len() == 36
        && segment.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// Determine whether a session id is an ACP flow session
/// (`acp_<client_id>_<uuid>`).
///
/// ACP flow sessions are created via SessionControl `acp__` / the ACP client
/// port; locally they only hold the provider=acp flow-session record
/// (interfaces/acp session_persistence.rs), and **do not write createdBy /
/// SessionRelationship etc. SessionMetadata**. Empty local metadata is
/// therefore the normal shape of an ACP flow session (not corruption), and
/// delete authorization must not reject cleanup merely because metadata is
/// missing.
///
/// The tail segment must be a canonical uuid (36 chars, dashed, hex) —
/// consistent with the strict validation of the desktop
/// `AcpClientPort::client_id_from_session_id` / `SessionMessage`
/// `acp_flow_client_id_from_session_id`, preventing arbitrary internal session
/// ids starting with `acp_` from being ghost-released and bypassing the RBAC
/// ownership model (PR #2139 R4).
pub(crate) fn is_acp_flow_session_id(session_id: &str) -> bool {
    let Some(rest) = session_id.strip_prefix("acp_") else {
        return false;
    };
    let Some((client_id, uuid_segment)) = rest.rsplit_once('_') else {
        return false;
    };
    !client_id.is_empty() && looks_like_uuid(uuid_segment)
}

/// P-06: ghost ACP flow-session delete authorization verdict.
///
/// When the target session metadata has no created_by (ghost) and is an ACP
/// flow session, authorization passes — ACP flow sessions are external-process
/// records, and metadata existing with empty created_by/relationship is their
/// designed shape (interfaces/acp session_persistence always writes a metadata
/// file on creation); otherwise the original created_by verdict is kept
/// (unchanged when metadata is complete).
fn ghost_acp_delete_authorized(created_by_is_none: bool, acp_flow_session: bool) -> bool {
    created_by_is_none && acp_flow_session
}

/// Authorization verdict switches: distinguish delete / cancel authorization
/// semantics.
///
/// - `allow_owner_bypass`: delete allows owner (Commander or RBAC off)
///   exemption; cancel has no owner exemption (keeps existing behavior).
/// - `allow_ghost_acp`: delete allows ghost ACP flow-session release (P-06);
///   cancel does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionMutationAuthOptions {
    pub allow_owner_bypass: bool,
    pub allow_ghost_acp: bool,
}

impl SessionMutationAuthOptions {
    pub(crate) const fn delete() -> Self {
        Self {
            allow_owner_bypass: true,
            allow_ghost_acp: true,
        }
    }

    pub(crate) const fn cancel() -> Self {
        Self {
            allow_owner_bypass: false,
            allow_ghost_acp: false,
        }
    }

    /// Delivery authorization (SessionMessage, PR #2139 #5): owner (Commander
    /// role or RBAC off) exemption, but no ghost ACP release — targets reaching
    /// the delivery authorization gate have already excluded the ACP flow
    /// direct path (flow direct path returns in dispatch_single before the
    /// gate after registry verification), so ghost ACP release does not apply
    /// to local delivery semantics.
    pub(crate) const fn deliver() -> Self {
        Self {
            allow_owner_bypass: true,
            allow_ghost_acp: false,
        }
    }
}

/// Shared session mutation (delete/cancel) authorization verdict, reused by
/// SessionControl and acp_control (PR #2139 R4).
///
/// Decision chain (each step equivalent to the existing SessionControl
/// delete/cancel semantics):
/// 1. daemon/warden session interception (R-A.04);
/// 2. owner exemption (delete only; Commander role or RBAC off);
/// 3. created_by match (`session-<caller>` marker); delete additionally
///    allows ghost ACP flow-session release (no created_by in metadata is the
///    designed shape of an ACP flow session);
/// 4. ancestor authorization: in-memory tree fast path, falling back to a
///    persisted metadata chain walk when the tree is empty (an empty tree
///    cannot be exploited to bypass authorization);
///
/// Returns `Ok(())` when authorized; `Err` is the rejection reason (tool error).
///
/// Dependency injection is `session_manager` + `tree` (rather than the whole
/// coordinator), making it easy to construct an isolated lightweight
/// authorization environment for unit tests.
pub(crate) async fn resolve_session_mutation_authorization(
    session_manager: &crate::agentic::session::session_manager::SessionManager,
    tree: &SessionTreeManager,
    caller_session_id: &str,
    target_session_id: &str,
    workspace_path: &std::path::Path,
    action_label: &str,
    options: SessionMutationAuthOptions,
) -> BitFunResult<()> {
    // R-A.04: Reject daemon/warden sessions (delete and cancel share this guard).
    {
        let is_daemon = if let Some(session) = session_manager.get_session(target_session_id) {
            session.config.is_daemon || session.agent_type.starts_with("warden-")
        } else {
            // Fall back to persisted metadata
            session_manager
                .load_session_metadata(workspace_path, target_session_id)
                .await
                .ok()
                .flatten()
                .map(|m| m.is_daemon || m.agent_type.starts_with("warden-"))
                .unwrap_or(false)
        };
        if is_daemon {
            return Err(BitFunError::tool(format!(
                "cannot {action_label} daemon/warden session '{target_session_id}'"
            )));
        }
    }

    // R-26 / user-owner semantics: the human user's main session (Commander
    // role) is the owner and may act on any session; when the RBAC master
    // switch is off, the gate is bypassed entirely. Cancel keeps the historical
    // stricter gate (no owner bypass).
    let caller_is_owner = options.allow_owner_bypass && caller_is_owner_session(caller_session_id);

    let acp_flow_session = is_acp_flow_session_id(target_session_id);
    let created_by_match = {
        let target_metadata = session_manager
            .load_session_metadata(workspace_path, target_session_id)
            .await
            .ok()
            .flatten();
        let creator = target_metadata
            .as_ref()
            .and_then(|metadata| metadata.created_by.as_deref());
        if options.allow_ghost_acp
            && ghost_acp_delete_authorized(creator.is_none(), acp_flow_session)
        {
            true
        } else {
            creator.is_some_and(|creator| {
                creator == session_control_creator_marker(caller_session_id)
            })
        }
    };

    if !caller_is_owner && !created_by_match {
        // Ancestor authorization: verify the calling session is an ancestor of
        // the target session. First try the in-memory tree (fast path). If the
        // tree is not yet populated (walk_ancestors returns empty), fall back
        // to a persisted metadata chain query so that an empty tree cannot be
        // exploited to bypass authorization.
        let tree_ancestors = tree.walk_ancestors(target_session_id);
        let ancestors: Vec<String> = if !tree_ancestors.is_empty() {
            // Fast path: tree is populated.
            tree_ancestors
        } else {
            // Fallback: tree is empty, walk persisted metadata chain.
            let mut metadata_ancestors = Vec::new();
            // Guard against cyclic metadata chains: never revisit a session id
            // already seen during this walk.
            let mut visited = std::collections::HashSet::new();
            visited.insert(target_session_id.to_string());
            let mut current = target_session_id.to_string();
            loop {
                let metadata = session_manager
                    .load_session_metadata(workspace_path, &current)
                    .await
                    .ok()
                    .flatten();
                match metadata.and_then(|m| m.relationship.and_then(|r| r.parent_session_id)) {
                    Some(parent_id) => {
                        if !visited.insert(parent_id.clone()) {
                            // Cycle detected; stop walking to avoid hanging on a
                            // corrupt lineage chain.
                            break;
                        }
                        metadata_ancestors.push(parent_id.clone());
                        current = parent_id;
                    }
                    None => break,
                }
            }
            metadata_ancestors
        };
        if ancestors.is_empty() {
            return Err(BitFunError::tool(format!(
                "cannot verify ancestor relationship for session '{target_session_id}': tree and metadata are both empty"
            )));
        }
        if !ancestors.iter().any(|id| id == caller_session_id) {
            return Err(BitFunError::tool(format!(
                "session '{caller_session_id}' is not authorized to {action_label} session '{target_session_id}': not a parent/ancestor and not the creator"
            )));
        }
    }

    Ok(())
}

/// Build the delete action result JSON.
/// Cascade child-deletion failures are surfaced as a structured list
/// (`cascade_failures`: `[{session_id, reason}, ...]`). Since the delete
/// action now cascades through `coordinator.delete_session_tree` with
/// all-or-nothing semantics, the list is always empty on success — any
/// member that cannot be deleted aborts the whole tree and surfaces as a
/// tool error instead. The field is kept for result-shape compatibility
/// with callers that parse the JSON contract.
fn build_delete_result_json(
    session_id: &str,
    workspace: &str,
    cascade_failures: &[(String, String)],
) -> Value {
    json!({
        "success": true,
        "action": "delete",
        "workspace": workspace,
        "session_id": session_id,
        "cascade_failures": cascade_failures
            .iter()
            .map(|(child_id, reason)| json!({
                "session_id": child_id,
                "reason": reason,
            }))
            .collect::<Vec<_>>(),
    })
}

/// Build a JSON tree structure from the flat session list.
/// Sessions are grouped by `parent_session_id` into a forest of root nodes.
pub(crate) fn build_session_tree_json_impl(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
) -> String {
    // children_by_parent: parent_session_id -> list of children
    let mut children_by_parent: HashMap<String, Vec<&AgentSessionSummary>> = HashMap::new();
    let mut roots: Vec<&AgentSessionSummary> = Vec::new();
    // Sessions whose parent chain is fully filtered out (no surviving ancestor
    // in this list). They are promoted to roots but flagged as orphaned.
    let mut orphaned: std::collections::HashSet<&str> = std::collections::HashSet::new();

    let known_ids: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.session_id.as_str()).collect();

    // R-19: resolve the effective parent of a session - the nearest ancestor
    // present in this (possibly filtered) list. When the direct parent is
    // filtered out (e.g. daemon/warden sessions), the child is re-hung onto the
    // nearest surviving ancestor instead of being promoted to a fake root,
    // which would break the lineage. The in-memory tree is used to walk past
    // filtered sessions.
    let resolve_effective_parent = |session: &AgentSessionSummary| -> Option<String> {
        let mut current = session.parent_session_id.clone()?;
        loop {
            if known_ids.contains(current.as_str()) {
                return Some(current);
            }
            match tree.and_then(|tree| tree.get_parent(&current)) {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    };

    for session in sessions {
        match resolve_effective_parent(session) {
            Some(parent_id) => {
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(session);
            }
            None => {
                if session.parent_session_id.is_some() {
                    // No surviving ancestor in this list — promote to a root
                    // but flag the broken lineage.
                    orphaned.insert(session.session_id.as_str());
                }
                roots.push(session);
            }
        }
    }

    /// Maximum recursion depth for tree serialization to prevent stack overflow.
    /// Authoritative value in `bitfun_core_types::session_tree::MAX_TREE_SERIALIZE_DEPTH`.
    const TREE_SERIALIZE_MAX_DEPTH: usize =
        bitfun_core_types::session_tree::MAX_TREE_SERIALIZE_DEPTH;

    fn serialize_node(
        session: &AgentSessionSummary,
        children_by_parent: &HashMap<String, Vec<&AgentSessionSummary>>,
        tree: Option<&SessionTreeManager>,
        orphaned: &std::collections::HashSet<&str>,
        recursion_depth: usize,
    ) -> serde_json::Value {
        let children: Vec<serde_json::Value> = if recursion_depth >= TREE_SERIALIZE_MAX_DEPTH {
            Vec::new()
        } else {
            children_by_parent
                .get(session.session_id.as_str())
                .map(|list| {
                    let mut sorted = list.to_vec();
                    sorted.sort_by_key(|s| s.created_at_ms);
                    sorted
                        .iter()
                        .map(|s| {
                            serialize_node(
                                s,
                                children_by_parent,
                                tree,
                                orphaned,
                                recursion_depth + 1,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let depth = tree
            .and_then(|t| t.get_depth(&session.session_id))
            .unwrap_or(0);

        let status = session
            .status
            .clone()
            .unwrap_or_else(|| "active".to_string());

        let mut map = serde_json::Map::new();
        map.insert("sessionId".to_string(), json!(session.session_id));
        map.insert("sessionName".to_string(), json!(session.session_name));
        map.insert("agentType".to_string(), json!(session.agent_type));
        map.insert("depth".to_string(), json!(depth));
        map.insert("status".to_string(), json!(status));
        if orphaned.contains(session.session_id.as_str()) {
            map.insert("orphaned".to_string(), json!(true));
        }
        map.insert("children".to_string(), json!(children));
        serde_json::Value::Object(map)
    }

    // Sort roots by created_at_ms descending (newest first)
    let mut sorted_roots = roots;
    sorted_roots.sort_by_key(|s| std::cmp::Reverse(s.created_at_ms));

    let forest: Vec<serde_json::Value> = sorted_roots
        .iter()
        .map(|s| serialize_node(s, &children_by_parent, tree, &orphaned, 0))
        .collect();

    serde_json::to_string_pretty(&forest).unwrap_or_else(|_| "[]".to_string())
}

/// Build the compact text tree used by the default `list` output: one line per
/// session with `sessionId | agentType | status | compact name`. The tree
/// shape mirrors [`build_session_tree_json_impl`] (same grouping, orphan
/// promotion, and sort orders); only the per-node rendering is text.
fn build_compact_tree_lines(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
    short_names: &HashMap<String, Option<String>>,
) -> Vec<String> {
    // children_by_parent: parent_session_id -> list of children
    let mut children_by_parent: HashMap<String, Vec<&AgentSessionSummary>> = HashMap::new();
    let mut roots: Vec<&AgentSessionSummary> = Vec::new();
    // Sessions whose parent chain has no surviving ancestor in this list:
    // promote to root but mark as orphaned (consistent with the JSON mode)
    let mut orphaned: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let known_ids: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.session_id.as_str()).collect();

    // R-19: resolve the effective parent of a session - the nearest ancestor
    // present in this (possibly filtered) list.
    let resolve_effective_parent = |session: &AgentSessionSummary| -> Option<String> {
        let mut current = session.parent_session_id.clone()?;
        loop {
            if known_ids.contains(current.as_str()) {
                return Some(current);
            }
            match tree.and_then(|tree| tree.get_parent(&current)) {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    };

    for session in sessions {
        match resolve_effective_parent(session) {
            Some(parent_id) => {
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(session);
            }
            None => {
                if session.parent_session_id.is_some() {
                    // The entire parent chain was filtered: promote to root
                    // while marking as orphaned (consistent with the JSON mode)
                    orphaned.insert(session.session_id.as_str());
                }
                roots.push(session);
            }
        }
    }

    fn compact_line(
        session: &AgentSessionSummary,
        short_names: &HashMap<String, Option<String>>,
        orphaned: &std::collections::HashSet<&str>,
    ) -> String {
        let status = session
            .status
            .clone()
            .unwrap_or_else(|| "active".to_string());
        let display_name = compact_session_display_name(
            &session.session_name,
            short_names
                .get(&session.session_id)
                .and_then(Option::as_deref),
        );
        let orphan_marker = if orphaned.contains(session.session_id.as_str()) {
            " (orphaned)"
        } else {
            ""
        };
        format!(
            "- [{}] {} | {} | {}{}",
            session.session_id, session.agent_type, status, display_name, orphan_marker
        )
    }

    fn collect_lines(
        session: &AgentSessionSummary,
        depth: usize,
        children_by_parent: &HashMap<String, Vec<&AgentSessionSummary>>,
        short_names: &HashMap<String, Option<String>>,
        orphaned: &std::collections::HashSet<&str>,
        lines: &mut Vec<String>,
    ) {
        let indent = "  ".repeat(depth);
        lines.push(format!(
            "{indent}{}",
            compact_line(session, short_names, orphaned)
        ));
        if let Some(children) = children_by_parent.get(session.session_id.as_str()) {
            let mut sorted = children.to_vec();
            sorted.sort_by_key(|s| s.created_at_ms);
            for child in sorted {
                collect_lines(
                    child,
                    depth + 1,
                    children_by_parent,
                    short_names,
                    orphaned,
                    lines,
                );
            }
        }
    }

    let mut sorted_roots = roots;
    sorted_roots.sort_by_key(|s| std::cmp::Reverse(s.created_at_ms));

    let mut lines = Vec::new();
    for root in sorted_roots {
        collect_lines(
            root,
            0,
            &children_by_parent,
            short_names,
            &orphaned,
            &mut lines,
        );
    }
    lines
}

#[async_trait]
impl Tool for SessionControlTool {
    fn name(&self) -> &str {
        "SessionControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage persisted workspace-scoped agent sessions.

Actions:
- "create": Create a new session. You may optionally provide session_name, short_name and agent_type.
- "cancel": Cancel the target session's currently running dialog turn. This does not delete the session or clear any queued messages that may still run later.
- "delete": Delete an existing session by session_id.
- "list": List all sessions. Sessions are displayed in a tree structure showing parent-child relationships (created via Task tool). By default the output is compact (sessionId | agentType | status | short name); pass "detail": true to expand the full session tree including full session names.

Related tools:
- Use Task (spawn) to launch subagents that appear as children in the session tree.
- Use SessionMessage to send messages to existing sessions.
- Use SessionHistory to export a session transcript.

Arguments:
- "workspace": Absolute workspace path. Optional for create and list; defaults to the current workspace when omitted. Ignored for cancel and delete.
- "session_name": Only used by create. Defaults to "New Session".
- "short_name": Only used by create. Optional compact display name (e.g. "secretary-standing"); it becomes the name shown in the compact list output, keeping the model context small. Ignored for ACP flow sessions.
- "detail": Only used by list. When true, the full session tree with full session names is returned instead of the compact output. Defaults to false.
- "agent_type": Only used by create. Defaults to "agentic". Allowed values are dynamically resolved from the available agent registry (common values include "agentic", "Plan", "Cowork", "DeepResearch", and any custom/external subagent types). Use "acp__<client_id>" to create a real external ACP agent session: the external client process is started immediately (same shape as the frontend create_acp_flow_session path).
  - "agentic": Coding-focused agent for implementation, debugging, and code changes.
  - "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
  - "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
  - "DeepResearch": Research agent for systematic investigation and evidence-driven reports.
- "session_id": Required for cancel and delete."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Create, list, cancel, and delete persisted agent sessions.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "cancel", "delete", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Optional absolute workspace path for create and list; defaults to the current workspace when omitted. Ignored for cancel and delete."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel and delete."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session."
                },
                "short_name": {
                    "type": "string",
                    "description": "Optional compact display name when creating a session (used by compact list output; ignored for ACP flow sessions)."
                },
                "detail": {
                    "type": "boolean",
                    "description": "When true, list returns the full session tree with full session names instead of the compact output."
                },
                "agent_type": {
                    "type": "string",
                    "description": "Optional agent type when creating a session (defaults to \"agentic\"). Valid values are dynamically resolved from the available agent registry. Use \"acp__<client_id>\" to create a real external ACP agent session (the external client process starts immediately)."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model id used when creating a session; the created session binds to this model."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    /// Dynamically resolves allowed agent_type values from the agent registry.
    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        let agent_type_ids = get_available_agent_type_ids_for_creation(context).await;
        let agent_type_enum: Vec<&str> = agent_type_ids.iter().map(|s| s.as_str()).collect();
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "cancel", "delete", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Optional absolute workspace path for create and list; defaults to the current workspace when omitted. Ignored for cancel and delete."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel and delete."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session."
                },
                "short_name": {
                    "type": "string",
                    "description": "Optional compact display name when creating a session (used by compact list output; ignored for ACP flow sessions)."
                },
                "detail": {
                    "type": "boolean",
                    "description": "When true, list returns the full session tree with full session names instead of the compact output."
                },
                "agent_type": {
                    "type": "string",
                    "enum": agent_type_enum,
                    "description": "Optional agent type when creating a session. Defaults to \"agentic\". Use \"acp__<client_id>\" to create a real external ACP agent session (the external client process starts immediately)."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model id used when creating a session; the created session binds to this model."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: SessionControlInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        Self::into_validation_result(validate_session_control_input(
            &parsed,
            Self::validation_context(context),
        ))
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        render_session_control_tool_use_message(input)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let runtime = CoreServiceAgentRuntime::agent_runtime(coordinator.clone())
            .map_err(BitFunError::tool)?;

        match params.action {
            SessionControlAction::Create => {
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Create,
                        None,
                        params.workspace.as_deref(),
                        context,
                        &runtime,
                    )
                    .await?;
                // R-14 B3: role-based delegation validation (fast fail). The
                // SessionControl create chain registers the new session with the
                // creator's role (B2), so the target is the inherited role; this
                // is a defensive check that stays permissive today and guards a
                // future explicit target-role channel from over-delegation.
                let creator_role = context.session_id.as_deref().and_then(get_session_role);
                let target_role = creator_role.clone().unwrap_or(AgentRole::Commander);
                validate_delegation(creator_role, target_role)?;
                let session_name =
                    session_control_session_name_or_default(params.session_name.as_deref());
                let agent_type = session_control_agent_type_or_default(params.agent_type.as_ref());

                // Real ACP session path: agent_type `acp__<client_id>` (ACP
                // bridge agent registry id, see AcpAgent::agent_id_for) creates
                // a real external ACP session directly via AcpClientPort —
                // equivalent to the frontend create_acp_flow_session (persisted
                // record + process startup + failure rollback), no longer
                // creating a local internal relay shell session. Flow-session
                // records only hold ACP metadata such as provider/acpClientId
                // (interfaces/acp session_persistence.rs:57-64), and do not
                // support createdBy/sessionKind=subagent or legion tree mounting
                // (lineage/register_child); the legion side communicates via
                // SessionMessage direct path (acp: flow session fork).
                if let Some(client_id) = agent_type
                    .strip_prefix(AcpAgent::agent_id_prefix())
                    .filter(|client_id| !client_id.trim().is_empty())
                {
                    let port = coordinator.acp_client_port().ok_or_else(|| {
                        BitFunError::tool(
                            "ACP client port is not available; the desktop host did not inject it"
                                .to_string(),
                        )
                    })?;
                    let created = self
                        .create_acp_session_via_port(
                            &workspace,
                            client_id,
                            params.session_name.clone(),
                            port.as_ref(),
                        )
                        .await?;
                    let result_for_assistant = session_control_created_result_message(
                        &created.session_id,
                        &workspace.display_workspace,
                        &created.agent_type,
                    );
                    return Ok(vec![ToolResult::Result {
                        data: json!({
                            "success": true,
                            "action": "create",
                            "workspace": workspace.display_workspace.clone(),
                            "session": {
                                "session_id": created.session_id,
                                "session_name": created.session_name,
                                "agent_type": created.agent_type,
                            }
                        }),
                        result_for_assistant: Some(result_for_assistant),
                        image_attachments: None,
                    }]);
                }

                // SESSION-01: validate agent_type via find_agent_entry (through
                // the public get_agent wrapper) before create: types not
                // registered in the agent registry are rejected directly,
                // preventing arbitrary strings from entering create_session and
                // forming zombie sessions.
                {
                    let registry = get_agent_registry();
                    let workspace_path = std::path::Path::new(&workspace.display_workspace);
                    registry.load_custom_agents(Some(workspace_path)).await;
                    if registry.get_agent(&agent_type, Some(workspace_path)).is_none() {
                        return Err(BitFunError::tool(format!(
                            "Unknown agent_type '{}' for SessionControl create; agent must be registered in the agent registry",
                            agent_type
                        )));
                    }
                }

                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));
                // SessionControl-created sessions are subagent sessions: force a 1M
                // context window and keep it stable across model-window refresh.
                metadata.insert("subagent".to_string(), json!(true));
                // Lineage facts forwarded through the free-form metadata map so the
                // SessionCreated event can carry the parent relationship. The
                // coordinator reads these keys defensively before emitting
                // (parent_session_id / subagent_type), keeping the event contract
                // in sync with the persisted SessionRelationship written below.
                metadata.insert(
                    "parentSessionId".to_string(),
                    json!(context.session_id.clone()),
                );
                metadata.insert("subagentType".to_string(), json!(agent_type.clone()));
                let session = runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type,
                        workspace_path: Some(workspace.display_workspace.clone()),
                        project_workspace_path: Some(workspace.project_workspace.clone()),
                        execution_target: workspace.execution_target.clone(),
                        workspace_id: workspace.workspace_id.clone(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                        model_id: params.model_id.clone(),
                        metadata,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let created_session_id = session.session_id.clone();
                let created_session_name = session.session_name.clone();
                let created_agent_type = session.agent_type.clone();
                let created_model_id = session.model_id.clone();

                // --- R-001/R-002: write SessionRelationship, depth inherited from parent ---
                {
                    use bitfun_services_core::session::types::{
                        SessionRelationship, SessionRelationshipKind,
                    };
                    let parent_session_id = context.session_id.clone();
                    // Read parent depth from persisted metadata, default 0 for root
                    let parent_depth = if let Some(ref pid) = parent_session_id {
                        coordinator
                            .session_manager
                            .load_session_metadata(
                                &std::path::PathBuf::from(&workspace.project_workspace),
                                pid,
                            )
                            .await
                            .ok()
                            .flatten()
                            .and_then(|m| m.relationship.and_then(|r| r.depth))
                            .unwrap_or(0u32)
                    } else {
                        0u32
                    };
                    let child_depth = parent_depth + 1;
                    // Guard against exceeding max depth (same as Task tool depth guard)
                    let max_depth = coordinator.session_tree().max_depth;
                    if child_depth > max_depth {
                        return Err(BitFunError::tool(format!(
                            "Session depth limit reached: child depth {} would exceed max allowed depth {}",
                            child_depth, max_depth
                        )));
                    }
                    let relationship = SessionRelationship {
                        kind: Some(SessionRelationshipKind::Subagent),
                        parent_session_id,
                        depth: Some(child_depth),
                        ..Default::default()
                    };
                    // SESSION-03: a lineage persistence failure would leave the
                    // child session an orphan after restart. Retry once to
                    // absorb transient IO faults; if it still fails, roll back
                    // the created child session so no orphan session without a
                    // parent-child record is left behind (never silently
                    // degrade to a log).
                    let mut lineage_result = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship.clone())
                        .await;
                    if lineage_result.is_err() {
                        log::warn!(
                            "SessionControl create: lineage persist failed for {}, retrying once: {:?}",
                            created_session_id,
                            lineage_result.as_ref().err()
                        );
                        lineage_result = coordinator
                            .session_manager
                            .persist_session_lineage(&created_session_id, relationship)
                            .await;
                    }
                    if let Err(e) = lineage_result {
                        // Roll back the creation: delete the just-created child
                        // session; if the rollback itself fails, still report
                        // so the caller knows there is an uncleaned session.
                        if let Err(rollback_error) = coordinator
                            .delete_session(
                                std::path::Path::new(&workspace.project_workspace),
                                &created_session_id,
                            )
                            .await
                        {
                            log::error!(
                                "SessionControl create: lineage persist failed for {} ({:?}), rollback of session also failed: {:?}",
                                created_session_id, e, rollback_error
                            );
                        }
                        return Err(BitFunError::tool(format!(
                            "failed to persist session lineage for {} after retry: {}",
                            created_session_id, e
                        )));
                    }

                    // R-003: Register in memory tree
                    if let Some(ref pid) = context.session_id {
                        if let Err(e) = coordinator.session_tree().register_child(
                            pid,
                            &created_session_id,
                            child_depth,
                        ) {
                            log::warn!(
                                "SessionControl create: failed to register child {} under {} in tree: {:?}",
                                created_session_id, pid, e
                            );
                        }
                    }

                    // Short name persistence: write `shortName` into the session
                    // custom metadata (same best-effort pattern as the RBAC role
                    // persistence) so the compact `list` output can show it
                    // without pulling the full session name into the context.
                    if let Some(short_name) = params
                        .short_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        if let Err(e) = coordinator
                            .session_manager
                            .update_session_metadata(
                                &std::path::PathBuf::from(&workspace.project_workspace),
                                &created_session_id,
                                |metadata| {
                                    merge_session_custom_metadata(
                                        metadata,
                                        serde_json::json!({ "shortName": short_name }),
                                    );
                                },
                            )
                            .await
                        {
                            log::warn!(
                                "SessionControl create: failed to persist short name for {}: {:?}",
                                created_session_id,
                                e
                            );
                        }
                    }
                }
                let result_for_assistant = session_control_created_result_message(
                    &created_session_id,
                    &workspace.display_workspace,
                    &created_agent_type,
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "create",
                        "workspace": workspace.display_workspace.clone(),
                        "session": {
                            "session_id": created_session_id,
                            "session_name": created_session_name,
                            "agent_type": created_agent_type,
                            "model_id": created_model_id,
                        }
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Cancel => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for cancel".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Cancel,
                        Some(session_id),
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot cancel the current session from SessionControl".to_string(),
                    ));
                }

                // R-2: Authorization (shared gate with acp_control; PR #2139 R4):
                // a caller may cancel a session it created (created_by marker
                // matches) OR any session in its descendant subtree. The "cannot
                // cancel the current session" guard above is preserved. Cancel
                // keeps the historical stricter gate: no owner bypass and no
                // ghost-ACP release.
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot cancel a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                resolve_session_mutation_authorization(
                    coordinator.get_session_manager(),
                    coordinator.session_tree(),
                    current_session_id,
                    session_id,
                    std::path::Path::new(&workspace.project_workspace),
                    "cancel",
                    SessionMutationAuthOptions::cancel(),
                )
                .await?;

                let scheduler = get_global_scheduler();
                let cancel_route = resolve_session_control_cancel_route(
                    context.session_id.as_deref(),
                    scheduler.is_some(),
                );
                let (runtime, requester_session_id) = match (cancel_route, scheduler) {
                    (
                        SessionControlCancelRoute::RequesterViaScheduler {
                            requester_session_id,
                        },
                        Some(scheduler),
                    ) => {
                        let runtime = CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports(
                            coordinator.clone(),
                            scheduler,
                        )
                        .map_err(BitFunError::tool)?;
                        (runtime, Some(requester_session_id))
                    }
                    _ => {
                        // Fallback covers unusual tool contexts and startup states where the
                        // global scheduler is not available; concrete cancellation still works.
                        (runtime.clone(), None)
                    }
                };
                let cancelled_turn_id = runtime
                    .cancel_turn(AgentTurnCancellationRequest {
                        session_id: session_id.to_string(),
                        turn_id: None,
                        source: Some(AgentSubmissionSource::AgentSession),
                        requester_session_id,
                        reason: None,
                        wait_timeout_ms: Some(CANCEL_WAIT_TIMEOUT.as_millis() as u64),
                        cancel_descendants: true,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?
                    .turn_id;
                let had_active_turn = cancelled_turn_id.is_some();
                let status = session_control_cancel_status(cancelled_turn_id.as_deref());
                let result_for_assistant = session_control_cancel_result_message(
                    session_id,
                    &workspace.display_workspace,
                    cancelled_turn_id.as_deref(),
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "cancel",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                        "had_active_turn": had_active_turn,
                        "cancelled_turn_id": cancelled_turn_id,
                        "status": status,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Delete => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for delete".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Delete,
                        Some(session_id),
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot delete the current session from SessionControl".to_string(),
                    ));
                }

                // R-2: Authorization (shared gate with acp_control; PR #2139 R4):
                // a caller may delete a session it created (created_by marker
                // matches) OR any session in its descendant subtree, with the
                // user-owner (Commander / RBAC-off) bypass and the P-06 ghost-ACP
                // release. The "cannot delete the current session" guard above is
                // preserved. Deletion of a daemon/warden session is rejected here
                // and the tree path enforces the same guard for every member.
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot delete a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                resolve_session_mutation_authorization(
                    coordinator.get_session_manager(),
                    coordinator.session_tree(),
                    current_session_id,
                    session_id,
                    std::path::Path::new(&workspace.project_workspace),
                    "delete",
                    SessionMutationAuthOptions::delete(),
                )
                .await?;

                // R-012: Cascade-delete the full descendant subtree through
                // `coordinator.delete_session_tree`, the same all-or-nothing
                // path used by the frontend UI delete. It pre-checks every
                // member (a processing or daemon/warden session anywhere in
                // the tree rejects the whole cascade) and deletes children
                // before the parent. The previous per-child failure-tolerant
                // loop could return success while a running child session
                // stayed on disk, which then resurrected as a ghost child
                // session on the next restart (ghost-session root cause R2);
                // the tree path aborts instead and reports which member is
                // not deletable. Deletion of a daemon/warden session was
                // already rejected above; the tree path enforces the same
                // guard for every member.
                coordinator
                    .delete_session_tree(
                        std::path::Path::new(&workspace.project_workspace),
                        workspace.remote_connection_id.as_deref(),
                        workspace.remote_ssh_host.as_deref(),
                        session_id,
                    )
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!(
                            "cannot delete session tree rooted at '{}': {}",
                            session_id, error
                        ))
                    })?;

                Ok(vec![ToolResult::Result {
                    data: build_delete_result_json(session_id, &workspace.display_workspace, &[]),
                    result_for_assistant: Some(session_control_deleted_result_message(
                        session_id,
                        &workspace.display_workspace,
                    )),
                    image_attachments: None,
                }])
            }
            SessionControlAction::List => {
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::List,
                        None,
                        params.workspace.as_deref(),
                        context,
                        &runtime,
                    )
                    .await?;
                let sessions = runtime
                    .list_sessions(AgentSessionListRequest {
                        workspace_path: workspace.project_workspace.clone(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                        // R-2: Full conversation management — include hidden
                        // Subagent/Ephemeral sessions; daemon/warden sessions
                        // are filtered below.
                        include_hidden: true,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;

                // Filter out daemon sessions (is_daemon=true or agent_type starts with "warden-")
                let sessions: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| !s.is_daemon && !s.agent_type.starts_with("warden-"))
                    .collect();

                // Resolve compact short names from persisted session metadata
                // (custom_metadata.shortName, written by create when a
                // short_name argument was provided). Best-effort: sessions
                // without metadata or without a shortName fall back to the
                // truncated full name in the compact output.
                // SESSION-06: read all persisted metadata in one batch
                // (list_session_metadata_including_internal), then extract
                // shortName per session, replacing the previous N+1 reads of
                // per-session serial load_session_metadata.
                let mut short_names: HashMap<String, Option<String>> = HashMap::new();
                let surfaced_session_ids: std::collections::HashSet<&str> =
                    sessions
                        .iter()
                        .map(|session| session.session_id.as_str())
                        .collect();
                let metadata_list = match coordinator
                    .session_manager
                    .persistence_manager()
                    .list_session_metadata_including_internal(
                        &std::path::PathBuf::from(&workspace.project_workspace),
                    )
                    .await
                {
                    Ok(metadata_list) => metadata_list,
                    // On batch read failure, treat it as "no shortName"
                    // (consistent with the previous per-item .ok().flatten()
                    // best-effort semantics; do not interrupt the list output).
                    Err(_) => Vec::new(),
                };
                for metadata in metadata_list {
                    // Keep only the shortName of filtered sessions
                    // (daemon/warden already removed above), preserving the
                    // output contract.
                    if !surfaced_session_ids.contains(metadata.session_id.as_str()) {
                        continue;
                    }
                    let short_name = metadata
                        .custom_metadata
                        .as_ref()
                        .and_then(|custom| custom.get("shortName"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                    short_names.insert(metadata.session_id, short_name);
                }

                let detail = params.detail.unwrap_or(false);
                let current_session_id =
                    self.current_workspace_session(context, &workspace.display_workspace);
                let result_for_assistant = self.build_list_result_for_assistant(
                    &workspace.display_workspace,
                    &sessions,
                    current_session_id,
                    Some(coordinator.session_tree().as_ref()),
                    &short_names,
                    detail,
                );

                let tree_json = self
                    .build_session_tree_json(&sessions, Some(coordinator.session_tree().as_ref()));
                let tree_value: Value = serde_json::from_str(&tree_json).unwrap_or(Value::Null);

                // SESSION-05: when detail=false, keep the machine-readable
                // `data.sessions` payload compact too. Each session's `name`
                // follows the same rule as the compact list lines: the short
                // name wins, otherwise the full session name is truncated to
                // 60 chars. The full sessions array stays available in the
                // detail=true payload, which the legacy verbose tree view
                // still relies on.
                let data_sessions: Vec<AgentSessionSummary> = if detail {
                    sessions
                } else {
                    sessions
                        .iter()
                        .map(|session| AgentSessionSummary {
                            session_name: compact_session_display_name(
                                &session.session_name,
                                short_names
                                    .get(&session.session_id)
                                    .and_then(Option::as_deref),
                            ),
                            ..session.clone()
                        })
                        .collect()
                };

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "workspace": workspace.display_workspace.clone(),
                        "current_session_id": current_session_id,
                        "count": data_sessions.len(),
                        "sessions": data_sessions,
                        "tree": tree_value,
                        "short_names": short_names,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Compact => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for compact".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Compact,
                        Some(session_id),
                        None,
                        context,
                        &runtime,
                    )
                    .await?;

                // Authorization follows the owner/ancestor/RBAC semantics (no
                // new relaxation); Compact additionally allows compacting
                // itself (including itself and resident subagent slots — the
                // contract).
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot compact a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                let caller_is_owner = caller_is_owner_session(current_session_id);
                let is_self = current_session_id == session_id;
                let created_by_match = {
                    let session_manager = coordinator.get_session_manager();
                    let target_metadata = session_manager
                        .load_session_metadata(
                            &std::path::PathBuf::from(&workspace.project_workspace),
                            session_id,
                        )
                        .await
                        .ok()
                        .flatten();
                    target_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.created_by.as_deref())
                        .is_some_and(|creator| {
                            creator == session_control_creator_marker(current_session_id)
                        })
                };
                if !caller_is_owner && !is_self && !created_by_match {
                    let tree = coordinator.session_tree();
                    let tree_ancestors = tree.walk_ancestors(session_id);
                    let ancestors: Vec<String> = if !tree_ancestors.is_empty() {
                        tree_ancestors
                    } else {
                        let session_manager = coordinator.get_session_manager();
                        let mut metadata_ancestors = Vec::new();
                        let mut visited = std::collections::HashSet::new();
                        visited.insert(session_id.to_string());
                        let mut current = session_id.to_string();
                        loop {
                            let metadata = session_manager
                                .load_session_metadata(
                                    &std::path::PathBuf::from(&workspace.project_workspace),
                                    &current,
                                )
                                .await
                                .ok()
                                .flatten();
                            match metadata
                                .and_then(|m| m.relationship.and_then(|r| r.parent_session_id))
                            {
                                Some(parent_id) => {
                                    if !visited.insert(parent_id.clone()) {
                                        break;
                                    }
                                    metadata_ancestors.push(parent_id.clone());
                                    current = parent_id;
                                }
                                None => break,
                            }
                        }
                        metadata_ancestors
                    };
                    if ancestors.is_empty() {
                        return Err(BitFunError::tool(format!(
                            "cannot verify ancestor relationship for session '{session_id}': tree and metadata are both empty"
                        )));
                    }
                    if !ancestors.contains(current_session_id) {
                        return Err(BitFunError::tool(format!(
                            "session '{current_session_id}' is not authorized to compact session '{session_id}': not a parent/ancestor and not the creator"
                        )));
                    }
                }

                // Idempotent: no context / already compacted -> applied=false
                // without error (guaranteed by the compaction execution layer);
                // non-Idle rejection is validated internally by
                // start_manual_compaction_task with a reason.
                let outcome = coordinator
                    .compact_session_with_outcome(session_id.to_string())
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!(
                            "cannot compact session '{session_id}': {}",
                            error
                        ))
                    })?;

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "compact",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                        "applied": outcome.applied,
                        "tokens_before": outcome.tokens_before,
                        "tokens_after": outcome.tokens_after,
                        "compression_ratio": outcome.compression_ratio,
                        "duration": outcome.duration_ms,
                        "summary_source": if outcome.has_summary {
                            Some(outcome.summary_source)
                        } else {
                            None
                        },
                    }),
                    result_for_assistant: Some(format!(
                        "Compacted session '{session_id}' in workspace '{}'.",
                        workspace.display_workspace
                    )),
                    image_attachments: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::ToolUseContext;
    use crate::agentic::WorkspaceBinding;
    use bitfun_core_types::{
        SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle,
    };
    use bitfun_runtime_ports::{
        AcpClientBitfunMessageRequest, AcpClientCancelRequest, AcpClientHistoryRequest,
        AcpClientHistoryResult, AcpClientListResult, AcpClientMessageRequest,
        AcpClientMessageResult, AcpClientReleaseRequest, AcpClientStreamChunk,
        AcpClientStreamChunkSink, PortError, PortErrorKind, PortResult, RuntimeServiceCapability,
        RuntimeServicePort,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    /// Minimal AcpClientPort fake: records create requests and returns the
    /// same flow-session shape the desktop implementation produces
    /// (`acp_<client>_<uuid>` / `acp:<client>`), with an optional failure flag
    /// to exercise the error mapping.
    #[derive(Debug, Default)]
    struct FakeAcpClientPort {
        created: Mutex<Vec<AcpClientCreateRequest>>,
        fail_create: Mutex<bool>,
    }

    impl RuntimeServicePort for FakeAcpClientPort {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::AcpClient
        }
    }

    #[async_trait]
    impl AcpClientPort for FakeAcpClientPort {
        async fn create_session(
            &self,
            request: AcpClientCreateRequest,
        ) -> PortResult<AcpClientCreateResult> {
            if *self.fail_create.lock().unwrap() {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "simulated start failure",
                ));
            }
            self.created.lock().unwrap().push(request.clone());
            Ok(AcpClientCreateResult {
                session_id: format!("acp_{}_{}", request.client_id, "session-1"),
                session_name: request
                    .session_name
                    .unwrap_or_else(|| format!("{} ACP", request.client_id)),
                agent_type: format!("acp:{}", request.client_id),
            })
        }

        async fn list_clients(&self) -> PortResult<AcpClientListResult> {
            Ok(AcpClientListResult { clients: vec![] })
        }

        async fn release_session(&self, _request: AcpClientReleaseRequest) -> PortResult<()> {
            Ok(())
        }

        async fn cancel_session(&self, _request: AcpClientCancelRequest) -> PortResult<()> {
            Ok(())
        }

        async fn send_message(
            &self,
            _request: AcpClientMessageRequest,
        ) -> PortResult<AcpClientMessageResult> {
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn send_message_stream(
            &self,
            _request: AcpClientMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<AcpClientMessageResult> {
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn send_message_to_bitfun_session(
            &self,
            _request: AcpClientBitfunMessageRequest,
        ) -> PortResult<AcpClientMessageResult> {
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn send_message_to_bitfun_session_stream(
            &self,
            _request: AcpClientBitfunMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<AcpClientMessageResult> {
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn delete_session_record(
            &self,
            _session_id: String,
            _workspace_path: Option<String>,
        ) -> PortResult<()> {
            Ok(())
        }

        async fn read_history(
            &self,
            _request: AcpClientHistoryRequest,
        ) -> PortResult<AcpClientHistoryResult> {
            Ok(AcpClientHistoryResult {
                session_id: String::new(),
                entries: vec![],
                truncated: false,
            })
        }
    }

    fn acp_workspace_target() -> SessionControlWorkspaceTarget {
        SessionControlWorkspaceTarget {
            display_workspace: "/repo/project".to_string(),
            project_workspace: "/repo/project".to_string(),
            execution_target: None,
            workspace_id: None,
            remote_connection_id: None,
            remote_ssh_host: None,
        }
    }

    #[tokio::test]
    async fn acp_create_forwards_client_workspace_and_session_name() {
        let port = FakeAcpClientPort::default();
        let created = SessionControlTool::new()
            .create_acp_session_via_port(
                &acp_workspace_target(),
                "codebuddy",
                Some("my acp".to_string()),
                &port,
            )
            .await
            .expect("acp create should succeed");

        let requests = port.created.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].client_id, "codebuddy");
        assert_eq!(requests[0].workspace_path, "/repo/project");
        assert_eq!(requests[0].session_name.as_deref(), Some("my acp"));
        // Shape consistent with the frontend create_acp_flow_session:
        // acp_<client>_<uuid> / acp:<client>
        assert_eq!(created.session_id, "acp_codebuddy_session-1");
        assert_eq!(created.agent_type, "acp:codebuddy");
    }

    #[tokio::test]
    async fn acp_create_keeps_service_default_session_name_when_omitted() {
        let port = FakeAcpClientPort::default();
        let created = SessionControlTool::new()
            .create_acp_session_via_port(&acp_workspace_target(), "codex", None, &port)
            .await
            .expect("acp create should succeed");

        assert!(port.created.lock().unwrap()[0].session_name.is_none());
        assert_eq!(created.session_name, "codex ACP");
    }

    #[tokio::test]
    async fn acp_create_maps_port_error_to_tool_error() {
        let port = FakeAcpClientPort::default();
        *port.fail_create.lock().unwrap() = true;
        let error = SessionControlTool::new()
            .create_acp_session_via_port(&acp_workspace_target(), "codebuddy", None, &port)
            .await
            .expect_err("port failure must surface as a tool error");
        assert!(error.to_string().contains("ACP client port failed"));
        assert!(error.to_string().contains("simulated start failure"));
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("temp workspace should be created");
            Self { path }
        }

        fn as_string(&self) -> String {
            self.path.to_string_lossy().to_string()
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn worktree_context_keeps_project_scope_for_session_operations() {
        let worktree_path = PathBuf::from("/worktrees/wt-1");
        let project_path = PathBuf::from("/repo");
        let execution_target = SessionExecutionTarget {
            kind: SessionExecutionTargetKind::ManagedWorktree,
            worktree_id: Some("wt-1".to_string()),
            root_path: "/worktrees/wt-1".to_string(),
            base_ref: Some("HEAD".to_string()),
            base_commit: Some("0123456789abcdef".to_string()),
            branch: None,
            lifecycle: Some(WorktreeLifecycle::Managed),
        };
        let binding = WorkspaceBinding::new(None, worktree_path.clone())
            .with_project_root_path(project_path.clone())
            .with_execution_target(Some(execution_target.clone()));

        let target = SessionControlTool::workspace_target_from_context(&binding);

        assert_eq!(PathBuf::from(target.display_workspace), worktree_path);
        assert_eq!(PathBuf::from(target.project_workspace), project_path);
        assert_eq!(target.execution_target, Some(execution_target));
    }

    #[tokio::test]
    async fn validate_cancel_requires_session_id() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_id is required for cancel")
        );
    }

    #[tokio::test]
    async fn validate_cancel_rejects_session_name() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "session_name": "should-not-be-here",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_name is only allowed for create")
        );
    }

    #[tokio::test]
    async fn validate_cancel_allows_missing_workspace() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_cancel_ignores_workspace_when_provided() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "workspace": "not-an-absolute-path",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_list_rejects_session_id() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                    "workspace": workspace.as_string(),
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_id is not allowed for list")
        );
    }

    #[tokio::test]
    async fn validate_list_requires_workspace() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("workspace is required for list")
        );
    }

    #[test]
    fn render_message_for_cancel_is_specific() {
        let tool = SessionControlTool::new();
        let message = tool.render_tool_use_message(
            &json!({
                "action": "cancel",
                "workspace": "/repo",
                "session_id": "worker_1",
            }),
            &ToolRenderOptions { verbose: false },
        );

        assert_eq!(message, "Cancel active turn for session worker_1");
    }

    // Cascade-failure surfacing (delete result JSON contract).
    // Full end-to-end cascade execution requires a global coordinator and
    // scheduler, which is not available in unit tests; these assert the
    // serialization contract that the delete path relies on, including the
    // session_id + reason shape for every failed child.
    #[test]
    fn delete_result_surfaces_cascade_failures() {
        let failures = vec![
            (
                "child_1".to_string(),
                "skipped: daemon/warden child session".to_string(),
            ),
            ("child_2".to_string(), "storage write failed".to_string()),
        ];
        let result = build_delete_result_json("parent", "/repo", &failures);

        assert_eq!(result["success"], true);
        assert_eq!(result["action"], "delete");
        assert_eq!(result["session_id"], "parent");
        let surfaced = result["cascade_failures"]
            .as_array()
            .expect("cascade_failures array");
        assert_eq!(surfaced.len(), 2);
        assert_eq!(surfaced[0]["session_id"], "child_1");
        assert_eq!(
            surfaced[0]["reason"],
            "skipped: daemon/warden child session"
        );
        assert_eq!(surfaced[1]["session_id"], "child_2");
        assert_eq!(surfaced[1]["reason"], "storage write failed");
    }

    #[test]
    fn delete_result_has_empty_cascade_failures_when_clean() {
        let result = build_delete_result_json("parent", "/repo", &[]);
        let surfaced = result["cascade_failures"]
            .as_array()
            .expect("cascade_failures array present");
        assert!(surfaced.is_empty());
    }

    #[test]
    fn commander_caller_is_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::{clear_session_role, set_session_role};
        let _ = set_session_role("delete-owner-commander", AgentRole::Commander);
        assert!(
            caller_is_owner_session("delete-owner-commander"),
            "the user's main session (Commander) may delete any session"
        );
        clear_session_role("delete-owner-commander");
    }

    #[test]
    fn unregistered_caller_degrades_to_non_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::clear_session_role;
        clear_session_role("delete-owner-unregistered");
        assert!(
            !caller_is_owner_session("delete-owner-unregistered"),
            "an unregistered caller must not bypass the R-2 authorization gate"
        );
    }

    #[test]
    fn executor_caller_is_not_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::{clear_session_role, set_session_role};
        let _ = set_session_role("delete-owner-executor", AgentRole::Executor);
        assert!(
            !caller_is_owner_session("delete-owner-executor"),
            "a subagent (Executor) must still pass the created_by/ancestor gate"
        );
        clear_session_role("delete-owner-executor");
    }

    #[test]
    fn reviewer_caller_is_not_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::{clear_session_role, set_session_role};
        let _ = set_session_role("delete-owner-reviewer", AgentRole::Reviewer);
        assert!(!caller_is_owner_session("delete-owner-reviewer"));
        clear_session_role("delete-owner-reviewer");
    }

    #[test]
    fn acp_flow_session_id_is_recognized() {
        // Strict uuid tail validation (PR #2139 R4): arbitrary acp_ prefixes
        // are no longer released; the tail must be a canonical uuid
        // (8-4-4-4-12, 36 chars, hex + dashes).
        assert!(is_acp_flow_session_id("acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!is_acp_flow_session_id("acp_opensource_abcdef")); // tail is not a uuid
        assert!(!is_acp_flow_session_id("session-1"));
        assert!(!is_acp_flow_session_id("acp__codex")); // agent type prefix, not a flow session id
        assert!(!is_acp_flow_session_id("acp_codex")); // no uuid tail
        assert!(!is_acp_flow_session_id("acp_codex_notauuid")); // tail is not a uuid shape
        assert!(!is_acp_flow_session_id(""));
        assert!(!is_acp_flow_session_id("acp_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b")); // client_id is empty
    }

    #[test]
    fn looks_like_uuid_accepts_only_canonical_shape() {
        assert!(looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5")); // one char short
        assert!(!looks_like_uuid("7f0e1a2b3c4d4e5f8a9b0c1d2e3f4a5b")); // no dashes
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5bZ")); // invalid hex
        assert!(!looks_like_uuid(""));
    }

    #[test]
    fn ghost_acp_session_delete_is_authorized_when_created_by_empty() {
        // P-06: ghost ACP flow session — metadata without created_by (not a
        // missing metadata file) + ACP flow session -> authorization releases
        // (ACP flow sessions always write a metadata file; empty created_by is
        // their designed shape).
        assert!(ghost_acp_delete_authorized(true, true));
        // Remaining combinations keep the original strict verdict (no release).
        assert!(!ghost_acp_delete_authorized(false, true));
        assert!(!ghost_acp_delete_authorized(true, false));
        assert!(!ghost_acp_delete_authorized(false, false));
    }

    #[test]
    fn ghost_acp_delete_bypasses_ancestor_gate_when_created_by_none() {
        // Anti-regression: metadata exists but created_by=None + acp prefix
        // -> created_by_match=true, so delete no longer falls into the ancestor
        // pre-check (the old error point :1422 'cannot verify ancestor' is no
        // longer reachable).
        let target_metadata = Some(crate::service::session::SessionMetadata::new(
            "acp_codebuddy_a4f68de7-c4ec-46a8-9aab-7e2bc417c3d0".to_string(),
            "codebuddy ACP".to_string(),
            "acp:codebuddy".to_string(),
            "auto".to_string(),
        ));
        let created_by_is_none = target_metadata
            .as_ref()
            .and_then(|metadata| metadata.created_by.as_deref())
            .is_none();
        assert!(created_by_is_none, "SessionMetadata::new 默认 created_by 应为 None");
        assert!(ghost_acp_delete_authorized(
            created_by_is_none,
            is_acp_flow_session_id("acp_codebuddy_a4f68de7-c4ec-46a8-9aab-7e2bc417c3d0"),
        ));
    }

    /// Build an isolated SessionManager (in-memory, does not persist to the
    /// user's real directory).
    fn test_session_manager() -> Arc<crate::agentic::session::session_manager::SessionManager> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::session_manager::{SessionManager, SessionManagerConfig};
        use crate::agentic::session::{PromptCachePolicy, SessionContextStore};
        use crate::infrastructure::app_paths::path_manager::PathManager;
        let user_root = std::env::temp_dir().join(format!(
            "bitfun-authz-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&user_root).expect("test user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence = PersistenceManager::new(Arc::new(path_manager))
            .expect("persistence manager");
        Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ))
    }

    #[tokio::test]
    async fn shared_authz_rejects_unrelated_caller_delete_without_metadata() {
        // Unauthorized: caller is not owner, target has no created_by and is
        // not an ACP flow session shape (tail is not a uuid) -> reject delete.
        // Non-ACP shape -> ghost release does not apply; no created_by ->
        // ancestor walk fails (tree and metadata both empty), consistent with
        // the existing SessionControl delete semantics (reject; no arbitrary
        // acp_ prefix bypass).
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-reject-delete");
        let workspace_string = workspace.as_string();
        let error = resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            "acp_codex_notauuid",
            std::path::Path::new(&workspace_string),
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect_err("unrelated caller without metadata must be rejected");
        assert!(
            error.to_string().contains("not authorized to delete")
                || error.to_string().contains("cannot verify ancestor relationship"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn shared_authz_rejects_unrelated_caller_cancel() {
        // Unauthorized: caller is not owner, target has no created_by and is
        // not an ACP flow session shape -> reject cancel (cancel has no owner
        // exemption and no ghost ACP release). Missing metadata makes the
        // ancestor walk fail, which is also a rejection.
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-reject-cancel");
        let workspace_string = workspace.as_string();
        let error = resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            "acp_codex_notauuid",
            std::path::Path::new(&workspace_string),
            "cancel",
            SessionMutationAuthOptions::cancel(),
        )
        .await
        .expect_err("unrelated caller without metadata must be rejected");
        assert!(
            error.to_string().contains("not authorized to cancel")
                || error.to_string().contains("cannot verify ancestor relationship"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn shared_authz_ghost_acp_delete_allowed_but_cancel_requires_shape() {
        // Ghost ACP flow session (strict uuid tail + no created_by): delete
        // releases (P-06 designed shape); but any acp_ prefix with a non-uuid
        // tail does not get the release.
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-ghost-acp");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let strict_acp_id = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b";

        // Strict ACP shape delete releases with no metadata (ghost).
        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            strict_acp_id,
            workspace_path,
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("strict acp flow session delete should be released");

        // cancel keeps delete's ghost release semantics (no created_by on a
        // flow session is the designed shape).
        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            strict_acp_id,
            workspace_path,
            "cancel",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("strict acp flow session cancel should be released");
    }

    #[tokio::test]
    async fn shared_authz_created_by_match_allows_caller() {
        // created_by match: target metadata created_by == session-<caller>
        // -> allow.
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-created-by");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let target_id = "target-1";
        let metadata = crate::service::session::SessionMetadata::new(
            target_id.to_string(),
            "target".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        let mut created_metadata = metadata.clone();
        created_metadata.created_by = Some(session_control_creator_marker("caller-1"));
        session_manager
            .save_session_metadata(workspace_path, &created_metadata)
            .await
            .expect("save metadata");

        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            target_id,
            workspace_path,
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("creator should be authorized to delete");
    }

    #[tokio::test]
    async fn shared_authz_owner_bypasses_delete_but_not_cancel() {
        // Owner (Commander role) delete exemption; cancel has no owner exemption.
        use crate::agentic::tools::restrictions::{clear_session_role, set_session_role};
        let _ = set_session_role("authz-owner", AgentRole::Commander);
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-owner");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "authz-owner",
            "acp_codex_notauuid",
            workspace_path,
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("owner should bypass delete gate");

        // cancel has no owner exemption: even as Commander role, a non-ACP
        // shape is still rejected (no metadata -> ancestor walk fails, which is
        // also a rejection).
        let error = resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "authz-owner",
            "acp_codex_notauuid",
            workspace_path,
            "cancel",
            SessionMutationAuthOptions::cancel(),
        )
        .await
        .expect_err("owner must not bypass cancel gate");
        assert!(
            error.to_string().contains("not authorized to cancel")
                || error.to_string().contains("cannot verify ancestor relationship"),
            "{error}"
        );
        clear_session_role("authz-owner");
    }

    fn summary(
        id: &str,
        parent: Option<&str>,
        is_daemon: bool,
        created_at_ms: u64,
    ) -> AgentSessionSummary {
        AgentSessionSummary {
            session_id: id.to_string(),
            session_name: format!("Session {id}"),
            agent_type: if is_daemon {
                "warden-daemon".to_string()
            } else {
                "agentic".to_string()
            },
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms,
            last_active_at_ms: created_at_ms,
            parent_session_id: parent.map(str::to_string),
            status: Some("active".to_string()),
            is_daemon,
        }
    }

    #[test]
    fn tree_repairs_lineage_when_parent_filtered_out() {
        // root <- daemon <- child; the daemon is filtered from the list, so the
        // child must be re-hung onto root instead of becoming a fake root.
        let tree = SessionTreeManager::new(8);
        tree.register_child("root", "daemon", 1).unwrap();
        tree.register_child("daemon", "child", 2).unwrap();

        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("daemon"), false, 2),
            summary("sibling", Some("root"), false, 3),
        ];

        let tree_json = build_session_tree_json_impl(&sessions, Some(&tree));
        let value: Value = serde_json::from_str(&tree_json).expect("valid tree json");
        let roots = value.as_array().expect("forest array");
        assert_eq!(roots.len(), 1, "single root after re-hang: {tree_json}");
        assert_eq!(roots[0]["sessionId"], "root");
        assert!(roots[0].get("orphaned").is_none());

        let children = roots[0]["children"].as_array().unwrap();
        let child_ids: Vec<&str> = children
            .iter()
            .map(|c| c["sessionId"].as_str().unwrap())
            .collect();
        // children sorted by created_at_ms ascending: child(2) then sibling(3)
        assert_eq!(child_ids, vec!["child", "sibling"]);
        assert!(children[0].get("orphaned").is_none());
        assert_eq!(
            children[0]["depth"], 2,
            "depth comes from the real tree, not the filtered list"
        );
    }

    #[test]
    fn tree_rehangs_to_nearest_surviving_ancestor() {
        // root <- daemon1 <- daemon2 <- child; both daemon layers are filtered,
        // so the child must be re-hung onto root (the nearest surviving ancestor).
        let tree = SessionTreeManager::new(8);
        tree.register_child("root", "daemon1", 1).unwrap();
        tree.register_child("daemon1", "daemon2", 2).unwrap();
        tree.register_child("daemon2", "child", 3).unwrap();

        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("daemon2"), false, 2),
        ];

        let tree_json = build_session_tree_json_impl(&sessions, Some(&tree));
        let value: Value = serde_json::from_str(&tree_json).expect("valid tree json");
        let roots = value.as_array().unwrap();
        assert_eq!(
            roots.len(),
            1,
            "single root after multi-level re-hang: {tree_json}"
        );
        assert_eq!(roots[0]["sessionId"], "root");
        let children = roots[0]["children"].as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["sessionId"], "child");
        assert!(children[0].get("orphaned").is_none());
        assert_eq!(children[0]["depth"], 3);
    }

    #[test]
    fn tree_marks_orphan_when_no_surviving_ancestor() {
        // The parent chain is entirely unknown (no tree, parent not in list):
        // the session is promoted to a root but flagged as orphaned.
        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("missing-parent"), false, 2),
        ];

        let tree_json = build_session_tree_json_impl(&sessions, None);
        let value: Value = serde_json::from_str(&tree_json).expect("valid tree json");
        let roots = value.as_array().unwrap();
        assert_eq!(roots.len(), 2);

        let root_node = roots.iter().find(|r| r["sessionId"] == "root").unwrap();
        assert!(root_node.get("orphaned").is_none());

        let orphan_node = roots.iter().find(|r| r["sessionId"] == "child").unwrap();
        assert_eq!(orphan_node["orphaned"], true);
    }

    // --- short_name / detail / compact output ---

    #[tokio::test]
    async fn validate_list_rejects_short_name() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                    "workspace": workspace.as_string(),
                    "short_name": "secretary",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("short_name is only allowed for create")
        );
    }

    #[tokio::test]
    async fn validate_list_allows_detail_flag() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                    "workspace": workspace.as_string(),
                    "detail": true,
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_cancel_rejects_detail_flag() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "detail": true,
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("detail is only allowed for list")
        );
    }

    #[tokio::test]
    async fn validate_create_allows_short_name() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");
        let mut context = empty_context();
        context.session_id = Some("creator-1".to_string());

        let validation = tool
            .validate_input(
                &json!({
                    "action": "create",
                    "workspace": workspace.as_string(),
                    "short_name": "secretary-standing",
                }),
                Some(&context),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_create_rejects_detail_flag() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");
        let mut context = empty_context();
        context.session_id = Some("creator-1".to_string());

        let validation = tool
            .validate_input(
                &json!({
                    "action": "create",
                    "workspace": workspace.as_string(),
                    "detail": true,
                }),
                Some(&context),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("detail is only allowed for list")
        );
    }

    #[test]
    fn compact_display_name_prefers_short_name_and_truncates() {
        let long_name = "task-description".repeat(10); // 150 chars
        assert_eq!(
            compact_session_display_name("abc", Some("秘书·常驻")),
            "秘书·常驻"
        );
        assert_eq!(compact_session_display_name("abc", Some("  ")), "abc");

        let truncated = compact_session_display_name(&long_name, None);
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.chars().count(), 60 + 3);

        assert_eq!(
            compact_session_display_name("short name", None),
            "short name"
        );
    }

    #[test]
    fn compact_list_uses_short_names_and_preserves_tree_indentation() {
        let tool = SessionControlTool::new();
        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("root"), false, 2),
        ];
        let mut short_names = HashMap::new();
        short_names.insert("root".to_string(), Some("秘书·常驻".to_string()));
        short_names.insert("child".to_string(), None);

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            false,
        );

        assert!(output.contains("[root] agentic | active | 秘书·常驻"));
        assert!(output.contains("  - [child] agentic | active | Session child"));
        assert!(output.contains("## Sessions (compact)"));
        assert!(!output.contains("## Session Tree (JSON)"));
    }

    #[test]
    fn compact_list_truncates_long_session_names_without_short_name() {
        let tool = SessionControlTool::new();
        let long_name = "派单提示词全文-".repeat(20); // 140 chars
        let mut root = summary("root", None, false, 1);
        root.session_name = long_name.clone();
        let sessions = vec![root];
        let short_names = HashMap::new();

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            false,
        );

        assert!(
            !output.contains(&long_name),
            "full session name must be omitted"
        );
        assert!(output.contains("..."));
        assert!(output.contains("[root] agentic | active | "));
    }

    #[test]
    fn detail_list_keeps_full_tree_json_output() {
        let tool = SessionControlTool::new();
        let sessions = vec![summary("root", None, false, 1)];
        let short_names = HashMap::new();

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            true,
        );

        assert!(output.contains("## Session Tree (JSON)"));
        assert!(output.contains("\"sessionName\": \"Session root\""));
        assert!(output.contains("\"sessionId\": \"root\""));
    }
}
