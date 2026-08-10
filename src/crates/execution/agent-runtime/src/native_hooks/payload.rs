//! Stdin payload construction.
//!
//! Field names follow the Codex hook process interface exactly. Every event
//! payload carries the common fields (`session_id`, `transcript_path`, `cwd`,
//! `hook_event_name`, `model`, `permission_mode`), turn-scoped events add
//! `turn_id`, and each event contributes its documented event-specific fields.

use super::settings::AgentHookEvent;
use serde_json::{json, Map, Value};

/// Codex permission-mode vocabulary carried in every payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentHookPermissionMode {
    #[default]
    Default,
    AcceptEdits,
    Plan,
    DontAsk,
    BypassPermissions,
}

impl AgentHookPermissionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            AgentHookPermissionMode::Default => "default",
            AgentHookPermissionMode::AcceptEdits => "acceptEdits",
            AgentHookPermissionMode::Plan => "plan",
            AgentHookPermissionMode::DontAsk => "dontAsk",
            AgentHookPermissionMode::BypassPermissions => "bypassPermissions",
        }
    }
}

/// Fields shared by every hook payload.
#[derive(Debug, Clone)]
pub struct AgentHookPayloadCommon {
    pub session_id: String,
    /// Session transcript path when available; serialized as `null` otherwise.
    pub transcript_path: Option<String>,
    /// Working directory the hook observes and runs in.
    pub cwd: String,
    pub model: String,
    pub permission_mode: AgentHookPermissionMode,
    /// Present for turn-scoped events.
    pub turn_id: Option<String>,
}

/// Event-specific payload fields.
#[derive(Debug, Clone)]
pub enum AgentHookEventPayload {
    SessionStart {
        /// `startup` | `resume` | `clear` | `compact`
        source: String,
    },
    SessionEnd {
        reason: String,
    },
    SubagentStart {
        agent_id: String,
        agent_type: String,
    },
    PreToolUse {
        tool_name: String,
        tool_use_id: String,
        tool_input: Value,
    },
    PermissionRequest {
        tool_name: String,
        tool_input: Value,
    },
    PostToolUse {
        tool_name: String,
        tool_use_id: String,
        tool_input: Value,
        tool_response: Value,
    },
    PreCompact {
        /// `manual` | `auto`
        trigger: String,
    },
    PostCompact {
        trigger: String,
    },
    UserPromptSubmit {
        prompt: String,
    },
    SubagentStop {
        agent_id: String,
        agent_type: String,
        agent_transcript_path: Option<String>,
        stop_hook_active: bool,
        last_assistant_message: Option<String>,
    },
    Stop {
        stop_hook_active: bool,
        last_assistant_message: Option<String>,
    },
}

impl AgentHookEventPayload {
    pub const fn event(&self) -> AgentHookEvent {
        match self {
            AgentHookEventPayload::SessionStart { .. } => AgentHookEvent::SessionStart,
            AgentHookEventPayload::SessionEnd { .. } => AgentHookEvent::SessionEnd,
            AgentHookEventPayload::SubagentStart { .. } => AgentHookEvent::SubagentStart,
            AgentHookEventPayload::PreToolUse { .. } => AgentHookEvent::PreToolUse,
            AgentHookEventPayload::PermissionRequest { .. } => AgentHookEvent::PermissionRequest,
            AgentHookEventPayload::PostToolUse { .. } => AgentHookEvent::PostToolUse,
            AgentHookEventPayload::PreCompact { .. } => AgentHookEvent::PreCompact,
            AgentHookEventPayload::PostCompact { .. } => AgentHookEvent::PostCompact,
            AgentHookEventPayload::UserPromptSubmit { .. } => AgentHookEvent::UserPromptSubmit,
            AgentHookEventPayload::SubagentStop { .. } => AgentHookEvent::SubagentStop,
            AgentHookEventPayload::Stop { .. } => AgentHookEvent::Stop,
        }
    }

    /// The value matchers are evaluated against for this event, when the
    /// event supports matcher filtering.
    pub fn matcher_value(&self) -> Option<&str> {
        match self {
            AgentHookEventPayload::PreToolUse { tool_name, .. }
            | AgentHookEventPayload::PermissionRequest { tool_name, .. }
            | AgentHookEventPayload::PostToolUse { tool_name, .. } => Some(tool_name),
            AgentHookEventPayload::SubagentStart { agent_type, .. }
            | AgentHookEventPayload::SubagentStop { agent_type, .. } => Some(agent_type),
            AgentHookEventPayload::PreCompact { trigger }
            | AgentHookEventPayload::PostCompact { trigger } => Some(trigger),
            AgentHookEventPayload::SessionStart { source } => Some(source),
            AgentHookEventPayload::SessionEnd { .. }
            | AgentHookEventPayload::UserPromptSubmit { .. }
            | AgentHookEventPayload::Stop { .. } => None,
        }
    }
}

/// A fully-built payload ready to serialize onto a hook's stdin.
#[derive(Debug, Clone)]
pub struct AgentHookPayload {
    pub common: AgentHookPayloadCommon,
    pub event: AgentHookEventPayload,
}

impl AgentHookPayload {
    pub const fn event(&self) -> AgentHookEvent {
        self.event.event()
    }

    pub fn to_json(&self) -> Value {
        let event = self.event();
        let mut fields = Map::new();
        fields.insert("session_id".into(), json!(self.common.session_id));
        fields.insert(
            "transcript_path".into(),
            match &self.common.transcript_path {
                Some(path) => json!(path),
                None => Value::Null,
            },
        );
        fields.insert("cwd".into(), json!(self.common.cwd));
        fields.insert("hook_event_name".into(), json!(event.as_str()));
        fields.insert("model".into(), json!(self.common.model));
        fields.insert(
            "permission_mode".into(),
            json!(self.common.permission_mode.as_str()),
        );
        if event.is_turn_scoped() {
            if let Some(turn_id) = &self.common.turn_id {
                fields.insert("turn_id".into(), json!(turn_id));
            }
        }
        match &self.event {
            AgentHookEventPayload::SessionStart { source } => {
                fields.insert("source".into(), json!(source));
            }
            AgentHookEventPayload::SessionEnd { reason } => {
                fields.insert("reason".into(), json!(reason));
            }
            AgentHookEventPayload::SubagentStart {
                agent_id,
                agent_type,
            } => {
                fields.insert("agent_id".into(), json!(agent_id));
                fields.insert("agent_type".into(), json!(agent_type));
            }
            AgentHookEventPayload::PreToolUse {
                tool_name,
                tool_use_id,
                tool_input,
            } => {
                fields.insert("tool_name".into(), json!(tool_name));
                fields.insert("tool_use_id".into(), json!(tool_use_id));
                fields.insert("tool_input".into(), tool_input.clone());
            }
            AgentHookEventPayload::PermissionRequest {
                tool_name,
                tool_input,
            } => {
                fields.insert("tool_name".into(), json!(tool_name));
                fields.insert("tool_input".into(), tool_input.clone());
            }
            AgentHookEventPayload::PostToolUse {
                tool_name,
                tool_use_id,
                tool_input,
                tool_response,
            } => {
                fields.insert("tool_name".into(), json!(tool_name));
                fields.insert("tool_use_id".into(), json!(tool_use_id));
                fields.insert("tool_input".into(), tool_input.clone());
                fields.insert("tool_response".into(), tool_response.clone());
            }
            AgentHookEventPayload::PreCompact { trigger }
            | AgentHookEventPayload::PostCompact { trigger } => {
                fields.insert("trigger".into(), json!(trigger));
            }
            AgentHookEventPayload::UserPromptSubmit { prompt } => {
                fields.insert("prompt".into(), json!(prompt));
            }
            AgentHookEventPayload::SubagentStop {
                agent_id,
                agent_type,
                agent_transcript_path,
                stop_hook_active,
                last_assistant_message,
            } => {
                fields.insert("agent_id".into(), json!(agent_id));
                fields.insert("agent_type".into(), json!(agent_type));
                fields.insert(
                    "agent_transcript_path".into(),
                    match agent_transcript_path {
                        Some(path) => json!(path),
                        None => Value::Null,
                    },
                );
                fields.insert("stop_hook_active".into(), json!(stop_hook_active));
                if let Some(message) = last_assistant_message {
                    fields.insert("last_assistant_message".into(), json!(message));
                }
            }
            AgentHookEventPayload::Stop {
                stop_hook_active,
                last_assistant_message,
            } => {
                fields.insert("stop_hook_active".into(), json!(stop_hook_active));
                if let Some(message) = last_assistant_message {
                    fields.insert("last_assistant_message".into(), json!(message));
                }
            }
        }
        Value::Object(fields)
    }
}
