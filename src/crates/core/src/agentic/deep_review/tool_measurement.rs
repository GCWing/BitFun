//! Deep Review shared-context measurement hook for successful tool calls.
//!
//! The hook is intentionally narrow: only successful reviewer `Read` and
//! `GetFileDiff` calls are measured, and BitFun runtime URIs are ignored. It
//! records normalized metadata for diagnostics, not file contents.

use crate::agentic::deep_review_policy::record_deep_review_shared_context_tool_use;
use crate::agentic::tools::framework::ToolUseContext;
use crate::agentic::tools::workspace_paths::is_bitfun_runtime_uri;
use serde_json::Value;
use std::path::Path;

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

fn custom_data_str<'a>(context: &'a ToolUseContext, key: &str) -> Option<&'a str> {
    context
        .custom_data
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn maybe_record_shared_context_tool_use(
    tool_name: &str,
    input: &Value,
    context: &ToolUseContext,
) {
    if !tool_name.eq_ignore_ascii_case("Read") && !tool_name.eq_ignore_ascii_case("GetFileDiff") {
        return;
    }
    if !custom_data_str(context, "deep_review_subagent_role")
        .is_some_and(|role| role.eq_ignore_ascii_case("reviewer"))
    {
        return;
    }
    let Some(parent_turn_id) = custom_data_str(context, "deep_review_parent_dialog_turn_id") else {
        return;
    };
    let Some(file_path) = input
        .get("file_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let measured_path = if context.is_remote() {
        None
    } else {
        context
            .workspace_root()
            .and_then(|workspace_root| git_relative_path(workspace_root, file_path))
    }
    .unwrap_or_else(|| file_path.to_string());
    let subagent_type = custom_data_str(context, "deep_review_subagent_type")
        .or(context.agent_type.as_deref())
        .unwrap_or("unknown");

    record_deep_review_shared_context_tool_use(
        parent_turn_id,
        subagent_type,
        tool_name,
        &measured_path,
    );
}
