//! Portable SessionControl tool decisions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Worktree options accepted by `create` for automatically creating a managed
/// worktree together with the session (re-exported from core-types so the
/// portable session-control decisions share the wire contract).
pub use bitfun_core_types::WorktreeSessionOptions as SessionControlWorktreeOptions;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionControlAction {
    Create,
    Cancel,
    Delete,
    List,
    Compact,
    Rename,
}

impl SessionControlAction {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Cancel => "cancel",
            Self::Delete => "delete",
            Self::List => "list",
            Self::Compact => "compact",
            Self::Rename => "rename",
        }
    }
}

/// Re-export of the shared agent type enum from runtime-ports.
/// Covers official agent types (agentic / Plan / Cowork / DeepResearch)
/// plus any custom / external agent type strings (incl. `acp__` sessions).
pub use bitfun_runtime_ports::AgentType as SessionControlAgentType;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SessionControlInput {
    pub action: SessionControlAction,
    pub workspace: Option<String>,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub agent_type: Option<SessionControlAgentType>,
    /// Optional compact display name used by `list` compact output. Only
    /// meaningful for `create`; the value is persisted as `shortName` in the
    /// session's custom metadata so it survives restarts.
    pub short_name: Option<String>,
    /// Optional model id used when creating the session. Only meaningful for
    /// `create`; forwarded to the session config so the session is created
    /// with the requested model (mirrors the Task(spawn) model_id parameter).
    pub model_id: Option<String>,
    /// Optional worktree options for `create`: when present, a managed
    /// worktree is created together with the session (git worktree add via
    /// WorktreeService) and the session is bound to it. `None` keeps the
    /// legacy behavior (session runs in the project checkout). Only allowed
    /// for `create` and rejected for remote workspaces.
    #[serde(default)]
    pub worktree: Option<SessionControlWorktreeOptions>,
    /// When true, `list` emits the full session tree (session_name included)
    /// instead of the compact per-session line output. Only meaningful for
    /// `list`.
    pub detail: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionControlValidationContext<'a> {
    pub current_session_id: Option<&'a str>,
    pub has_workspace_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionControlValidationResult {
    pub result: bool,
    pub message: Option<String>,
    pub error_code: Option<i32>,
    pub meta: Option<Value>,
}

impl Default for SessionControlValidationResult {
    fn default() -> Self {
        Self {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionControlCancelRoute {
    RequesterViaScheduler { requester_session_id: String },
    CoordinatorDirect,
}

pub fn resolve_session_control_cancel_route(
    requester_session_id: Option<&str>,
    scheduler_available: bool,
) -> SessionControlCancelRoute {
    match (requester_session_id, scheduler_available) {
        (Some(requester_session_id), true) => SessionControlCancelRoute::RequesterViaScheduler {
            requester_session_id: requester_session_id.to_string(),
        },
        _ => SessionControlCancelRoute::CoordinatorDirect,
    }
}

fn invalid(message: impl Into<String>) -> SessionControlValidationResult {
    SessionControlValidationResult {
        result: false,
        message: Some(message.into()),
        error_code: Some(400),
        meta: None,
    }
}

pub fn validate_session_id(session_id: &str) -> Result<(), String> {
    bitfun_core_types::validate_session_id(session_id)
}

pub fn default_session_name() -> &'static str {
    "New Session"
}

pub fn session_control_session_name_or_default(session_name: Option<&str>) -> String {
    session_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_session_name())
        .to_string()
}

/// Maximum number of characters a user-provided short name may keep. The cap
/// bounds `list` compact output; validation rejects longer values and the
/// compact renderer truncates defensively.
pub const SHORT_NAME_MAX_CHARS: usize = 60;

/// Maximum number of characters a compact display name keeps from the full
/// session name when no explicit short name is set. Aliased to
/// [`SHORT_NAME_MAX_CHARS`] so both paths share a single bound.
pub const COMPACT_SESSION_NAME_MAX_CHARS: usize = SHORT_NAME_MAX_CHARS;

/// Truncate a compact display name to at most [`COMPACT_SESSION_NAME_MAX_CHARS`]
/// characters with a trailing ellipsis. Character-based truncation keeps
/// multi-byte (CJK) names intact.
fn truncate_compact_display_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.chars().count() <= COMPACT_SESSION_NAME_MAX_CHARS {
        return trimmed.to_string();
    }
    let truncated: String = trimmed
        .chars()
        .take(COMPACT_SESSION_NAME_MAX_CHARS)
        .collect();
    format!("{truncated}...")
}

/// Resolve the compact display name used by `list` compact output: the
/// explicit short name wins; otherwise the full session name is truncated to
/// [`COMPACT_SESSION_NAME_MAX_CHARS`] characters with a trailing ellipsis.
/// Both paths share the same character-based cap, so multi-byte (CJK) names
/// stay intact and a short name cannot exceed the bound.
pub fn compact_session_display_name(session_name: &str, short_name: Option<&str>) -> String {
    if let Some(short_name) = short_name.filter(|value| !value.trim().is_empty()) {
        return truncate_compact_display_name(short_name);
    }
    truncate_compact_display_name(session_name)
}

pub fn session_control_agent_type_or_default(
    agent_type: Option<&SessionControlAgentType>,
) -> String {
    agent_type
        .map(|agent_type| agent_type.as_str().to_string())
        .unwrap_or_else(|| "agentic".to_string())
}

pub fn session_control_creator_marker(creator_session_id: &str) -> String {
    format!("session-{creator_session_id}")
}

fn validate_workspace_shape(workspace: &str) -> SessionControlValidationResult {
    if workspace.trim().is_empty() {
        return invalid("workspace is required and cannot be empty");
    }

    if !Path::new(workspace.trim()).is_absolute() {
        return invalid("workspace must be an absolute path");
    }

    SessionControlValidationResult::default()
}

fn validate_mutating_action_target(
    action: &SessionControlAction,
    input: &SessionControlInput,
    context: SessionControlValidationContext<'_>,
) -> SessionControlValidationResult {
    if input.agent_type.is_some() {
        return invalid("agent_type is only allowed for create");
    }
    // Rename 例外：session_name 是 rename 的新标题（必填），其余 action 仍只允许
    // create 携带 session_name。
    if input.session_name.is_some() && !matches!(action, SessionControlAction::Rename) {
        return invalid("session_name is only allowed for create");
    }
    if input.short_name.is_some() {
        return invalid("short_name is only allowed for create");
    }
    if input.model_id.is_some() {
        return invalid("model_id is only allowed for create");
    }
    if input.worktree.is_some() {
        return invalid("worktree is only allowed for create");
    }
    if input.detail.is_some() {
        return invalid("detail is only allowed for list");
    }

    let Some(session_id) = input.session_id.as_deref() else {
        return invalid(format!("session_id is required for {}", action.as_str()));
    };
    if let Err(message) = validate_session_id(session_id) {
        return invalid(message);
    }

    // Rename 必须提供非空新标题。
    if matches!(action, SessionControlAction::Rename) {
        let Some(session_name) = input.session_name.as_deref() else {
            return invalid("session_name is required for rename");
        };
        if session_name.trim().is_empty() {
            return invalid("session_name must not be empty for rename");
        }
    }

    // 守卫只依赖会话绑定等价判定：目标 session_id 与当前会话一致即拒绝，
    // 不再依赖 workspace_root，避免远程/未绑定上下文绕过"不能操作当前会话"限制。
    // Compact 例外：允许压缩自己（含自己、含常驻 subagent 工位——契约）。
    if !matches!(action, SessionControlAction::Compact)
        && context.current_session_id == Some(session_id)
    {
        return invalid(format!(
            "cannot {} the current session from SessionControl",
            action.as_str()
        ));
    }

    SessionControlValidationResult::default()
}

pub fn validate_session_control_input(
    input: &SessionControlInput,
    context: SessionControlValidationContext<'_>,
) -> SessionControlValidationResult {
    if let Some(workspace) = input.workspace.as_deref() {
        let should_validate_workspace = matches!(
            input.action,
            SessionControlAction::Create | SessionControlAction::List
        );
        if !should_validate_workspace {
            return validate_mutating_action_target(&input.action, input, context);
        }

        let workspace_validation = validate_workspace_shape(workspace);
        if !workspace_validation.result {
            return workspace_validation;
        }
    }

    match input.action {
        SessionControlAction::Create => {
            // workspace is optional: when omitted it falls back to the current
            // workspace binding from context.
            if input.workspace.is_none() && !context.has_workspace_root {
                return invalid("workspace is required for create");
            }
            if input.session_id.is_some() {
                return invalid("session_id is not allowed for create");
            }
            if input.detail.is_some() {
                return invalid("detail is only allowed for list");
            }
            if let Some(short_name) = input.short_name.as_deref() {
                if short_name.trim().chars().count() > SHORT_NAME_MAX_CHARS {
                    return invalid(format!(
                        "short_name must be at most {SHORT_NAME_MAX_CHARS} characters"
                    ));
                }
            }
            if input
                .model_id
                .as_deref()
                .is_some_and(|model_id| model_id.trim().is_empty())
            {
                return invalid("model_id must not be empty when provided");
            }
            if let Some(worktree) = input.worktree.as_ref() {
                if worktree
                    .base_ref
                    .as_deref()
                    .is_some_and(|base_ref| base_ref.trim().is_empty())
                {
                    return invalid("worktree.base_ref must not be empty when provided");
                }
                // worktree 与 ACP 真会话（agent_type `acp__<client>`）互斥：
                // ACP 会话是外部进程记录，不承载本地 worktree execution_target，
                // 同时携带会导致 worktree 被静默忽略/成为孤儿。
                if input
                    .agent_type
                    .as_ref()
                    .is_some_and(|agent_type| agent_type.as_str().starts_with("acp__"))
                {
                    return invalid("worktree is not supported with acp__ agent types");
                }
            }
            if context.current_session_id.is_none() {
                return invalid("create requires a creator session in tool context");
            }
        }
        SessionControlAction::Cancel
        | SessionControlAction::Delete
        | SessionControlAction::Compact
        | SessionControlAction::Rename => {
            return validate_mutating_action_target(&input.action, input, context);
        }
        SessionControlAction::List => {
            // workspace is optional: when omitted it falls back to the current
            // workspace binding from context.
            if input.workspace.is_none() && !context.has_workspace_root {
                return invalid("workspace is required for list");
            }
            if input.agent_type.is_some() {
                return invalid("agent_type is only allowed for create");
            }
            if input.session_name.is_some() {
                return invalid("session_name is only allowed for create");
            }
            if input.short_name.is_some() {
                return invalid("short_name is only allowed for create");
            }
            if input.model_id.is_some() {
                return invalid("model_id is only allowed for create");
            }
            if input.worktree.is_some() {
                return invalid("worktree is only allowed for create");
            }
            if input.session_id.is_some() {
                return invalid("session_id is not allowed for list");
            }
        }
    }

    SessionControlValidationResult::default()
}

pub fn render_session_control_tool_use_message(input: &Value) -> String {
    let action = input
        .get("action")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let workspace = input
        .get("workspace")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown workspace");
    let session_id = input
        .get("session_id")
        .and_then(|value| value.as_str())
        .unwrap_or("auto");

    match action {
        "create" => format!("Create session in {workspace}"),
        "cancel" => format!("Cancel active turn for session {session_id}"),
        "delete" => format!("Delete session {session_id}"),
        "compact" => format!("Compact session {session_id}"),
        "rename" => format!("Rename session {session_id}"),
        "list" => format!("List sessions in {workspace}"),
        _ => format!("Manage sessions in {workspace}"),
    }
}

pub fn session_control_renamed_result_message(
    session_id: &str,
    workspace: &str,
    session_name: &str,
) -> String {
    format!("Renamed session '{session_id}' to '{session_name}' in workspace '{workspace}'.")
}

pub fn session_control_created_result_message(
    session_id: &str,
    workspace: &str,
    agent_type: &str,
) -> String {
    format!("Created session '{session_id}' in workspace '{workspace}' using agent type '{agent_type}'.")
}

pub fn session_control_cancel_status(cancelled_turn_id: Option<&str>) -> &'static str {
    if cancelled_turn_id.is_some() {
        "cancel_requested"
    } else {
        "no_active_turn"
    }
}

pub fn session_control_cancel_result_message(
    session_id: &str,
    workspace: &str,
    cancelled_turn_id: Option<&str>,
) -> String {
    if let Some(turn_id) = cancelled_turn_id {
        format!(
            "Cancellation requested for the active turn '{turn_id}' in session '{session_id}' within workspace '{workspace}'. The session remains available for future work, and queued messages are not cleared."
        )
    } else {
        format!(
            "Session '{session_id}' in workspace '{workspace}' has no active turn to cancel. The session remains available for future work."
        )
    }
}

pub fn session_control_deleted_result_message(session_id: &str, workspace: &str) -> String {
    format!("Deleted session '{session_id}' from workspace '{workspace}'.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn context(current: Option<&str>) -> SessionControlValidationContext<'_> {
        SessionControlValidationContext {
            current_session_id: current,
            has_workspace_root: true,
        }
    }

    #[test]
    fn compact_action_parses_payload_session_id() {
        let input: SessionControlInput = serde_json::from_value(json!({
            "action": "compact",
            "session_id": "worker_1",
        }))
        .expect("compact payload must parse");
        assert_eq!(input.action, SessionControlAction::Compact);
        assert_eq!(input.session_id.as_deref(), Some("worker_1"));
        assert_eq!(SessionControlAction::Compact.as_str(), "compact");
    }

    #[test]
    fn compact_validation_requires_session_id() {
        let input = SessionControlInput {
            action: SessionControlAction::Compact,
            workspace: None,
            session_id: None,
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
        assert!(result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("session_id is required"));
    }

    #[test]
    fn compact_validation_rejects_non_mutating_fields() {
        let input = SessionControlInput {
            action: SessionControlAction::Compact,
            workspace: None,
            session_id: Some("worker_1".to_string()),
            session_name: Some("should not be allowed".to_string()),
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
        assert_eq!(
            result.message.as_deref(),
            Some("session_name is only allowed for create")
        );
    }

    #[test]
    fn compact_validation_allows_current_session() {
        // Contract: compact supports "含自己" (current session and resident
        // subagent workstations). The mutating guard must NOT reject self.
        let input = SessionControlInput {
            action: SessionControlAction::Compact,
            workspace: None,
            session_id: Some("self_1".to_string()),
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(Some("self_1")));
        assert!(
            result.result,
            "compact of the current session must be allowed: {:?}",
            result.message
        );
    }

    #[test]
    fn compact_validation_rejects_invalid_session_id() {
        let input = SessionControlInput {
            action: SessionControlAction::Compact,
            workspace: None,
            session_id: Some("bad/id".to_string()),
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
    }

    #[test]
    fn compact_render_mentions_session() {
        let rendered = render_session_control_tool_use_message(&json!({
            "action": "compact",
            "session_id": "worker_1",
        }));
        assert!(rendered.contains("Compact session"));
        assert!(rendered.contains("worker_1"));
    }

    #[test]
    fn create_deserializes_and_validates_model_id() {
        let input: SessionControlInput = serde_json::from_value(json!({
            "action": "create",
            "workspace": std::env::temp_dir().to_string_lossy().to_string(),
            "model_id": "claude-sonnet-4",
        }))
        .expect("create payload with model_id must parse");
        assert_eq!(input.model_id.as_deref(), Some("claude-sonnet-4"));

        let result = validate_session_control_input(&input, context(Some("creator_1")));
        assert!(result.result, "{:?}", result.message);
    }

    #[test]
    fn create_rejects_blank_model_id() {
        let input = SessionControlInput {
            action: SessionControlAction::Create,
            workspace: Some(std::env::temp_dir().to_string_lossy().to_string()),
            session_id: None,
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: Some("   ".to_string()),
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(Some("creator_1")));
        assert!(!result.result);
        assert_eq!(
            result.message.as_deref(),
            Some("model_id must not be empty when provided")
        );
    }

    #[test]
    fn create_deserializes_and_validates_worktree_options() {
        let input: SessionControlInput = serde_json::from_value(json!({
            "action": "create",
            "workspace": std::env::temp_dir().to_string_lossy().to_string(),
            "worktree": {
                "baseRef": "main",
                "copyLocalChanges": true
            }
        }))
        .expect("create payload with worktree options must parse");
        assert_eq!(
            input.worktree.as_ref().and_then(|w| w.base_ref.as_deref()),
            Some("main")
        );
        assert!(input
            .worktree
            .as_ref()
            .is_some_and(|w| w.copy_local_changes));

        let result = validate_session_control_input(&input, context(Some("creator_1")));
        assert!(result.result, "{:?}", result.message);
    }

    #[test]
    fn create_rejects_blank_worktree_base_ref() {
        let input = SessionControlInput {
            action: SessionControlAction::Create,
            workspace: Some(std::env::temp_dir().to_string_lossy().to_string()),
            session_id: None,
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: Some(bitfun_core_types::WorktreeSessionOptions {
                base_ref: Some("   ".to_string()),
                copy_local_changes: false,
            }),
            detail: None,
        };
        let result = validate_session_control_input(&input, context(Some("creator_1")));
        assert!(!result.result);
        assert_eq!(
            result.message.as_deref(),
            Some("worktree.base_ref must not be empty when provided")
        );
    }

    #[test]
    fn non_create_actions_reject_worktree() {
        let input = SessionControlInput {
            action: SessionControlAction::Delete,
            workspace: None,
            session_id: Some("worker_1".to_string()),
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: Some(bitfun_core_types::WorktreeSessionOptions::default()),
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
        assert_eq!(
            result.message.as_deref(),
            Some("worktree is only allowed for create")
        );
    }

    #[test]
    fn create_rejects_worktree_with_acp_agent_type() {
        let input = SessionControlInput {
            action: SessionControlAction::Create,
            workspace: Some(std::env::temp_dir().to_string_lossy().to_string()),
            session_id: None,
            session_name: None,
            agent_type: Some(SessionControlAgentType::from("acp__codebuddy")),
            short_name: None,
            model_id: None,
            worktree: Some(bitfun_core_types::WorktreeSessionOptions::default()),
            detail: None,
        };
        let result = validate_session_control_input(&input, context(Some("creator_1")));
        assert!(!result.result);
        assert!(result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("worktree is not supported with acp__ agent types"));
    }

    #[test]
    fn create_legacy_payload_without_worktree_is_compatible() {
        // 向后兼容：无 worktree 参数的旧 payload 正常解析且 worktree = None。
        let input: SessionControlInput = serde_json::from_value(json!({
            "action": "create",
            "workspace": std::env::temp_dir().to_string_lossy().to_string(),
            "session_name": "legacy",
        }))
        .expect("legacy payload without worktree must parse");
        assert!(input.worktree.is_none());
        let result = validate_session_control_input(&input, context(Some("creator_1")));
        assert!(result.result, "{:?}", result.message);
    }

    #[test]
    fn non_create_actions_reject_model_id() {
        let input = SessionControlInput {
            action: SessionControlAction::Compact,
            workspace: None,
            session_id: Some("worker_1".to_string()),
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: Some("claude-sonnet-4".to_string()),
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
        assert_eq!(
            result.message.as_deref(),
            Some("model_id is only allowed for create")
        );
    }

    #[test]
    fn rename_action_parses_payload_session_id_and_name() {
        let input: SessionControlInput = serde_json::from_value(json!({
            "action": "rename",
            "session_id": "worker_1",
            "session_name": "new-title",
        }))
        .expect("rename payload must parse");
        assert_eq!(input.action, SessionControlAction::Rename);
        assert_eq!(input.session_id.as_deref(), Some("worker_1"));
        assert_eq!(input.session_name.as_deref(), Some("new-title"));
        assert_eq!(SessionControlAction::Rename.as_str(), "rename");
    }

    #[test]
    fn rename_validation_requires_session_id() {
        let input = SessionControlInput {
            action: SessionControlAction::Rename,
            workspace: None,
            session_id: None,
            session_name: Some("new-title".to_string()),
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
        assert!(result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("session_id is required"));
    }

    #[test]
    fn rename_validation_requires_session_name() {
        let input = SessionControlInput {
            action: SessionControlAction::Rename,
            workspace: None,
            session_id: Some("worker_1".to_string()),
            session_name: None,
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
        assert!(result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("session_name is required for rename"));
    }

    #[test]
    fn rename_validation_rejects_blank_session_name() {
        let input = SessionControlInput {
            action: SessionControlAction::Rename,
            workspace: None,
            session_id: Some("worker_1".to_string()),
            session_name: Some("   ".to_string()),
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(!result.result);
        assert_eq!(
            result.message.as_deref(),
            Some("session_name must not be empty for rename")
        );
    }

    #[test]
    fn rename_validation_accepts_valid_input() {
        let input = SessionControlInput {
            action: SessionControlAction::Rename,
            workspace: None,
            session_id: Some("worker_1".to_string()),
            session_name: Some("new-title".to_string()),
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(None));
        assert!(result.result, "{:?}", result.message);
    }

    #[test]
    fn rename_validation_rejects_current_session() {
        let input = SessionControlInput {
            action: SessionControlAction::Rename,
            workspace: None,
            session_id: Some("self_1".to_string()),
            session_name: Some("new-title".to_string()),
            agent_type: None,
            short_name: None,
            model_id: None,
            worktree: None,
            detail: None,
        };
        let result = validate_session_control_input(&input, context(Some("self_1")));
        assert!(!result.result);
        assert!(result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("cannot rename the current session"));
    }

    #[test]
    fn rename_render_mentions_session() {
        let rendered = render_session_control_tool_use_message(&json!({
            "action": "rename",
            "session_id": "worker_1",
        }));
        assert!(rendered.contains("Rename session"));
        assert!(rendered.contains("worker_1"));
    }

    #[test]
    fn renamed_result_message_mentions_id_and_new_name() {
        let message = session_control_renamed_result_message("worker_1", "/ws", "new-title");
        assert!(message.contains("worker_1"));
        assert!(message.contains("new-title"));
        assert!(message.contains("/ws"));
    }
}
