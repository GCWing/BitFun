//! Portable post-call hook routing decisions.

use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Hook categories that concrete runtime integrations may execute after a
/// successful tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostCallHookKind {
    DeepReviewSharedContextToolUse,
}

pub const fn successful_tool_post_call_hooks() -> [PostCallHookKind; 1] {
    [PostCallHookKind::DeepReviewSharedContextToolUse]
}

pub trait SuccessfulToolPostCallHookExecutor<C> {
    fn record_deep_review_shared_context_tool_use(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &C,
    );
}

pub fn run_successful_tool_post_call_hooks<C, E>(
    tool_name: &str,
    input: &Value,
    context: &C,
    executor: &mut E,
) where
    E: SuccessfulToolPostCallHookExecutor<C>,
{
    for hook in successful_tool_post_call_hooks() {
        match hook {
            PostCallHookKind::DeepReviewSharedContextToolUse => {
                executor.record_deep_review_shared_context_tool_use(tool_name, input, context);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeepReviewSharedContextToolUseFacts<'a> {
    pub tool_name: &'a str,
    pub input: &'a Value,
    pub custom_data: &'a HashMap<String, Value>,
    pub workspace_root: Option<&'a Path>,
    pub is_remote: bool,
    pub agent_type: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewSharedContextToolUseRecord {
    pub parent_turn_id: String,
    pub subagent_type: String,
    pub tool_name: String,
    pub measured_path: String,
}

pub fn resolve_deep_review_shared_context_tool_use(
    facts: DeepReviewSharedContextToolUseFacts<'_>,
) -> Option<DeepReviewSharedContextToolUseRecord> {
    if !facts.tool_name.eq_ignore_ascii_case("Read")
        && !facts.tool_name.eq_ignore_ascii_case("GetFileDiff")
    {
        return None;
    }
    if !custom_data_str(facts.custom_data, "deep_review_subagent_role")
        .is_some_and(|role| role.eq_ignore_ascii_case("reviewer"))
    {
        return None;
    }
    let parent_turn_id = custom_data_str(facts.custom_data, "deep_review_parent_dialog_turn_id")?;
    let file_path = facts
        .input
        .get("file_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if is_bitfun_runtime_uri(file_path) {
        return None;
    }

    let measured_path = if facts.is_remote {
        None
    } else {
        facts
            .workspace_root
            .and_then(|workspace_root| git_relative_path(workspace_root, file_path))
    }
    .unwrap_or_else(|| file_path.to_string());
    let subagent_type = custom_data_str(facts.custom_data, "deep_review_subagent_type")
        .or(facts.agent_type)
        .unwrap_or("unknown");

    Some(DeepReviewSharedContextToolUseRecord {
        parent_turn_id: parent_turn_id.to_string(),
        subagent_type: subagent_type.to_string(),
        tool_name: facts.tool_name.to_string(),
        measured_path,
    })
}

fn custom_data_str<'a>(custom_data: &'a HashMap<String, Value>, key: &str) -> Option<&'a str> {
    custom_data
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn git_relative_path(workspace_root: &Path, path: &str) -> Option<String> {
    let path = Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(workspace_root).ok()?
    } else {
        path.strip_prefix(workspace_root).unwrap_or(path)
    };

    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn is_bitfun_runtime_uri(path: &str) -> bool {
    path.trim().starts_with("bitfun://runtime/")
}
