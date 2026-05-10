//! Deep Review custom data propagation for generic tool execution contexts.
//!
//! Generic tool execution remains shared. This module only injects typed Deep
//! Review custom data when the parent launch context proves the tool call is
//! part of a Deep Review reviewer flow.

use serde_json::Value;
use std::collections::HashMap;

pub(crate) struct DeepReviewToolParentContext<'a> {
    pub tool_call_id: &'a str,
    pub session_id: &'a str,
    pub dialog_turn_id: &'a str,
}

fn context_var_str<'a>(context_vars: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    context_vars
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn append_tool_use_context_data(
    context_vars: &HashMap<String, String>,
    parent_context: Option<DeepReviewToolParentContext<'_>>,
    custom_data: &mut HashMap<String, Value>,
) {
    if let Some(raw_manifest) = context_vars.get("deep_review_run_manifest") {
        if let Ok(manifest) = serde_json::from_str::<Value>(raw_manifest) {
            custom_data.insert("deep_review_run_manifest".to_string(), manifest);
        }
    }

    if let Some(role) = context_var_str(context_vars, "deep_review_subagent_role") {
        custom_data.insert(
            "deep_review_subagent_role".to_string(),
            serde_json::json!(role),
        );
    }

    if let Some(subagent_type) = context_var_str(context_vars, "deep_review_subagent_type") {
        custom_data.insert(
            "deep_review_subagent_type".to_string(),
            serde_json::json!(subagent_type),
        );
    }

    if custom_data
        .get("deep_review_subagent_role")
        .and_then(Value::as_str)
        .is_some_and(|role| role == "reviewer")
    {
        if let Some(parent_context) = parent_context {
            custom_data.insert(
                "deep_review_parent_tool_call_id".to_string(),
                serde_json::json!(parent_context.tool_call_id),
            );
            custom_data.insert(
                "deep_review_parent_session_id".to_string(),
                serde_json::json!(parent_context.session_id),
            );
            custom_data.insert(
                "deep_review_parent_dialog_turn_id".to_string(),
                serde_json::json!(parent_context.dialog_turn_id),
            );
        }
    }
}
