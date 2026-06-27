use serde_json::Value;

pub const TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES: usize = 1024;
pub const USER_STEERING_INTERRUPTED_MESSAGE: &str = "Tool execution skipped because the user sent a new steering message for the running turn. Stop the remaining old tool plan and handle the new user message next.";
pub const USER_REJECTED_TOOL_MESSAGE: &str =
    "The user rejected this tool call. Do not retry it unless the user explicitly asks you to. If you cannot complete the task without running this tool call, stop and ask the user how to proceed.";
pub const TOOL_CONFIRMATION_TIMEOUT_MESSAGE: &str =
    "The tool confirmation window expired before the user responded. Do not retry the same tool call unless the user explicitly asks you to. If you still need this tool to complete the task, stop and ask the user how to proceed.";

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
            "[Your previous {tool_name} call was truncated mid-stream by max_tokens and was auto-repaired before execution; the file may have been written with partial content. Use the latest Read result for that file (or call Read once if no current Read result is available) to inspect what is on disk. To finish it, issue ONE {tool_name} call with `mode`: \"a\" and `content` set only to the missing continuation. If you do not have a concrete plan for the continuation, stop tool-calling and tell the user the output was truncated (suggest raising max_tokens). Do NOT rewrite the whole file with overwrite mode.]\n\nOriginal tool result follows.\n\n"
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

pub fn build_tool_confirmation_timeout_presentation(
    tool_name: &str,
) -> ToolExecutionErrorPresentation {
    ToolExecutionErrorPresentation {
        result_json: serde_json::json!({
            "status": "cancelled",
            "category": "confirmation_timeout",
            "tool_name": tool_name,
            "message": TOOL_CONFIRMATION_TIMEOUT_MESSAGE,
        }),
        result_for_assistant: TOOL_CONFIRMATION_TIMEOUT_MESSAGE.to_string(),
    }
}

pub fn build_tool_execution_timeout_presentation(
    tool_name: &str,
    timeout_secs: Option<u64>,
) -> ToolExecutionErrorPresentation {
    let timeout_seconds_text = timeout_secs
        .map(|seconds| format!("{seconds} seconds"))
        .unwrap_or_else(|| "an unspecified limit".to_string());
    let message = format!(
        "This tool call was cancelled because the global tool execution time limit ({timeout_seconds_text}) expired before the tool finished."
    );

    let mut result_json = serde_json::json!({
        "status": "timeout",
        "category": "execution_timeout",
        "tool_name": tool_name,
        "message": message,
    });
    if let Some(timeout_secs) = timeout_secs {
        result_json["timeout_seconds"] = Value::from(timeout_secs);
    }

    ToolExecutionErrorPresentation {
        result_json,
        result_for_assistant: message,
    }
}

pub fn build_user_rejected_tool_presentation(tool_name: &str) -> ToolExecutionErrorPresentation {
    build_user_rejected_tool_presentation_with_instruction(tool_name, None)
}

pub fn build_user_rejected_tool_presentation_with_instruction(
    tool_name: &str,
    instruction: Option<&str>,
) -> ToolExecutionErrorPresentation {
    let normalized_instruction = instruction.map(str::trim).filter(|value| !value.is_empty());
    let message = if let Some(instruction) = normalized_instruction {
        format!(
            "The user rejected this tool call with the following instruction: \"{instruction}\". Do not retry it unless the user explicitly asks you to. If you cannot complete the task without running this tool call, stop and ask the user how to proceed."
        )
    } else {
        USER_REJECTED_TOOL_MESSAGE.to_string()
    };

    let mut result_json = serde_json::json!({
        "status": "rejected",
        "category": "user_rejected",
        "tool_name": tool_name,
        "message": message,
    });
    if let Some(instruction) = normalized_instruction {
        result_json["instruction"] = Value::String(instruction.to_string());
    }

    ToolExecutionErrorPresentation {
        result_json,
        result_for_assistant: message,
    }
}

pub fn build_invalid_tool_call_error_message(
    tool_name: &str,
    tool_is_error: bool,
    recovered_from_truncation: bool,
    _raw_arguments_preview: Option<String>,
) -> String {
    if tool_name.is_empty() && tool_is_error {
        "Missing valid tool name and arguments are invalid JSON.".to_string()
    } else if tool_name.is_empty() {
        "Missing valid tool name.".to_string()
    } else if recovered_from_truncation {
        format!(
            "Tool arguments were truncated by the model (likely hit max_tokens). Refusing to execute a partial '{tool_name}' call. Increase max_tokens, split the work into smaller calls, or retry."
        )
    } else {
        "Arguments are invalid JSON.".to_string()
    }
}
