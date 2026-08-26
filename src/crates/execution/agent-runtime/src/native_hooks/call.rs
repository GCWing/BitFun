//! Provider-neutral hook calls shared by command, builtin and function hooks.

use super::kind::RuntimeHookKind;
use super::payload::AgentHookEventPayload;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct HookCall {
    pub kind: RuntimeHookKind,
    pub cwd: PathBuf,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub workspace_root: Option<PathBuf>,
    pub is_remote: bool,
    pub model: Option<String>,
    pub bypass_permissions: bool,
    pub payload: HookCallPayload,
}

#[derive(Debug, Clone)]
pub enum HookCallPayload {
    Lifecycle(AgentHookEventPayload),
    Config(Value),
    ToolUse {
        name: String,
        input: Value,
        custom_data: HashMap<String, Value>,
        agent_type: Option<String>,
    },
}
