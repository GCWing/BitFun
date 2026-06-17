use serde_json::Value;

pub const TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES: usize = 1024;
pub const USER_STEERING_INTERRUPTED_MESSAGE: &str = "Tool execution skipped because the user sent a new steering message for the running turn. Stop the remaining old tool plan and handle the new user message next.";

#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecutionErrorPresentation {
    pub result_json: Value,
    pub result_for_assistant: String,
}

pub fn render_tool_result_for_assistant(tool_name: &str, data: &Value) -> String {
    serde_json::to_string_pretty(data)
        .or_else(|_| serde_json::to_string(data))
        .ok()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| format!("Tool {tool_name} returned no serializable result."))
}

pub fn is_write_like_tool_name(tool_name: &str) -> bool {
    matches!(tool_name, "Write" | "file_write" | "write_notebook")
}

pub fn build_tool_call_truncation_recovery_notice(tool_name: &str) -> String {
    if is_write_like_tool_name(tool_name) {
        return format!(
            "[Your previous {tool_name} call was truncated mid-stream by max_tokens and was auto-repaired before execution; the file may have been written with partial content. Use the latest Read result for that file (or call Read once if no current Read result is available) to inspect what is on disk. To finish it, issue ONE Edit call where `old_string` is a final unique substring from the current file and `new_string` is that same substring plus the continuation. If you do not have a concrete plan for the continuation, stop tool-calling and tell the user the output was truncated (suggest raising max_tokens). Do NOT rewrite the whole file with Write.]\n\nOriginal tool result follows.\n\n"
        );
    }

    format!(
        "[Your previous {tool_name} call was truncated mid-stream by max_tokens and was auto-repaired before execution. The tool ran with the repaired, potentially incomplete arguments. Review the tool result and continue normally; if important information is missing, issue a fresh complete {tool_name} call rather than trying to continue a file write.]\n\nOriginal tool result follows.\n\n"
    )
}

pub fn truncate_tool_arguments_preview(value: &Value) -> String {
    let raw = serde_json::to_string(value).unwrap_or_default();
    truncate_raw_tool_arguments_preview(&raw)
}

pub fn truncate_raw_tool_arguments_preview(raw: &str) -> String {
    truncate_raw_tool_arguments_preview_to(raw, TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES)
}

pub fn truncate_raw_tool_arguments_preview_to(raw: &str, max_bytes: usize) -> String {
    if raw.len() <= max_bytes {
        return raw.to_string();
    }

    let mut cut = max_bytes;
    while cut > 0 && !raw.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…[truncated, total {} bytes]", &raw[..cut], raw.len())
}

pub fn build_tool_execution_error_presentation(
    tool_name: &str,
    category: &str,
    error_message: &str,
    provided_arguments: Option<String>,
) -> ToolExecutionErrorPresentation {
    let mut result_json = serde_json::json!({
        "error": error_message,
        "category": category,
        "tool_name": tool_name,
        "message": format!("Tool '{tool_name}' failed ({category}): {error_message}"),
    });
    if let Some(args_preview) = provided_arguments.as_ref() {
        result_json["provided_arguments"] = Value::String(args_preview.clone());
    }

    let result_for_assistant = if let Some(args_preview) = provided_arguments.as_ref() {
        format!(
            "Tool '{tool_name}' failed ({category}): {error_message}\nProvided arguments: {args_preview}"
        )
    } else {
        format!("Tool '{tool_name}' failed ({category}): {error_message}")
    };

    ToolExecutionErrorPresentation {
        result_json,
        result_for_assistant,
    }
}

pub fn build_user_steering_interrupted_presentation(
    tool_name: &str,
) -> ToolExecutionErrorPresentation {
    ToolExecutionErrorPresentation {
        result_json: serde_json::json!({
            "status": "skipped",
            "category": "user_steering_interrupted",
            "tool_name": tool_name,
            "message": USER_STEERING_INTERRUPTED_MESSAGE,
        }),
        result_for_assistant: USER_STEERING_INTERRUPTED_MESSAGE.to_string(),
    }
}

pub fn build_invalid_tool_call_error_message(
    tool_name: &str,
    tool_is_error: bool,
    recovered_from_truncation: bool,
    raw_arguments_preview: Option<String>,
) -> String {
    let error_msg = if tool_name.is_empty() && tool_is_error {
        "Missing valid tool name and arguments are invalid JSON.".to_string()
    } else if tool_name.is_empty() {
        "Missing valid tool name.".to_string()
    } else if recovered_from_truncation {
        format!(
            "Tool arguments were truncated by the model (likely hit max_tokens). Refusing to execute a partial '{tool_name}' call. Increase max_tokens, split the work into smaller calls, or retry."
        )
    } else {
        "Arguments are invalid JSON.".to_string()
    };

    if let Some(raw_arguments_preview) = raw_arguments_preview {
        format!("{error_msg} Raw arguments: {raw_arguments_preview}")
    } else {
        error_msg
    }
}
