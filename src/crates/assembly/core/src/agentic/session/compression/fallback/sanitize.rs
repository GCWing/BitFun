use super::types::CompressionFallbackOptions;
use crate::agentic::core::{strip_prompt_markup, CompressedTodoItem, CompressedTodoSnapshot};
use serde_json::{Map, Value};

pub(super) fn sanitize_user_text(
    text: &str,
    options: &CompressionFallbackOptions,
) -> Option<String> {
    let normalized = strip_prompt_markup(text);
    sanitize_text(&normalized, options.user_chars)
}

pub(super) fn sanitize_assistant_text(
    text: &str,
    options: &CompressionFallbackOptions,
) -> Option<String> {
    sanitize_text(text, options.assistant_chars)
}

pub(super) fn sanitize_todo_snapshot(arguments: &Value) -> Option<CompressedTodoSnapshot> {
    let todos = arguments.get("todos")?.as_array()?;
    let mut compressed_todos = Vec::new();

    for todo in todos {
        let Some(todo_object) = todo.as_object() else {
            continue;
        };
        let Some(content) = todo_object
            .get("content")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|content| !content.is_empty())
        else {
            continue;
        };
        let status = todo_object
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending");
        let id = todo_object
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string);

        compressed_todos.push(CompressedTodoItem {
            id,
            content: content.to_string(),
            status: status.to_string(),
        });
    }

    if compressed_todos.is_empty() {
        return None;
    }

    Some(CompressedTodoSnapshot {
        todos: compressed_todos,
        summary: None,
    })
}

pub(super) fn sanitize_tool_arguments(
    tool_name: &str,
    arguments: &Value,
    options: &CompressionFallbackOptions,
) -> Option<Value> {
    let Some(object) = arguments.as_object() else {
        return sanitize_generic_value(arguments, options);
    };

    let sanitized = match tool_name {
        "Read" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "file_path");
            copy_field(object, &mut result, "start_line");
            copy_field(object, &mut result, "limit");
            result
        }
        "Write" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "file_path");
            insert_cleared_field(object, &mut result, "content");
            result
        }
        "Edit" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "file_path");
            copy_field(object, &mut result, "replace_all");
            insert_cleared_field(object, &mut result, "old_string");
            insert_cleared_field(object, &mut result, "new_string");
            result
        }
        "Grep" => {
            let mut result = Map::new();
            for key in [
                "pattern",
                "path",
                "glob",
                "type",
                "head_limit",
                "multiline",
                "-A",
                "-B",
                "-C",
                "-i",
                "-n",
                "output_mode",
            ] {
                copy_field(object, &mut result, key);
            }
            result
        }
        "Glob" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "pattern");
            copy_field(object, &mut result, "path");
            copy_field(object, &mut result, "limit");
            result
        }
        "LS" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "path");
            copy_field(object, &mut result, "ignore");
            copy_field(object, &mut result, "limit");
            result
        }
        "GetFileDiff" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "file_path");
            result
        }
        "DeleteFile" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "path");
            copy_field(object, &mut result, "recursive");
            result
        }
        "Git" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "operation");
            copy_field(object, &mut result, "working_directory");
            if let Some(args) = object.get("args") {
                if let Some(value) = sanitize_generic_value(args, options) {
                    result.insert("args".to_string(), value);
                }
            }
            result
        }
        "Bash" => {
            let mut result = Map::new();
            insert_sanitize_text(object, &mut result, "command", options.tool_command_chars);
            result
        }
        "TerminalControl" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "action");
            copy_field(object, &mut result, "terminal_session_id");
            result
        }
        "Skill" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "command");
            result
        }
        "CreatePlan" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "name");
            copy_field(object, &mut result, "overview");
            insert_cleared_field(object, &mut result, "plan");
            insert_cleared_field(object, &mut result, "todos");
            result
        }
        "WebSearch" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "query");
            result
        }
        "WebFetch" => {
            let mut result = Map::new();
            copy_field(object, &mut result, "url");
            result
        }
        _ => sanitize_generic_object(object, options),
    };

    if sanitized.is_empty() {
        None
    } else {
        Some(Value::Object(sanitized))
    }
}

pub(super) fn sanitize_generic_object(
    object: &Map<String, Value>,
    options: &CompressionFallbackOptions,
) -> Map<String, Value> {
    let mut sanitized = Map::new();

    for (key, value) in object {
        let heavy_string = matches!(
            key.as_str(),
            "content"
                | "contents"
                | "old_string"
                | "new_string"
                | "text"
                | "output"
                | "stdout"
                | "stderr"
                | "diff"
                | "file_diff"
                | "original_content"
                | "new_content"
                | "data_url"
                | "data_base64"
        );
        if heavy_string {
            if let Some(text) = value.as_str() {
                sanitized.insert(
                    format!("{key}_chars"),
                    Value::Number(serde_json::Number::from(text.chars().count() as u64)),
                );
            }
            continue;
        }

        if let Some(value) = sanitize_generic_value(value, options) {
            sanitized.insert(key.clone(), value);
        }
    }

    sanitized
}

pub(super) fn sanitize_generic_value(
    value: &Value,
    options: &CompressionFallbackOptions,
) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::Bool(_) | Value::Number(_) => Some(value.clone()),
        Value::String(text) => sanitize_text(text, options.tool_arg_chars).map(Value::String),
        Value::Array(values) => {
            let sanitized_values: Vec<Value> = values
                .iter()
                .take(5)
                .filter_map(|value| sanitize_generic_value(value, options))
                .collect();
            if sanitized_values.is_empty() {
                None
            } else {
                Some(Value::Array(sanitized_values))
            }
        }
        Value::Object(object) => {
            let sanitized_object = sanitize_generic_object(object, options);
            if sanitized_object.is_empty() {
                None
            } else {
                Some(Value::Object(sanitized_object))
            }
        }
    }
}

fn sanitize_text(text: &str, limit: usize) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let text_len = trimmed.chars().count();
    if text_len <= limit {
        return Some(trimmed.to_string());
    }

    let mut truncated: String = trimmed.chars().take(limit).collect();
    truncated.push_str(" ... [truncated]");
    Some(truncated)
}

fn copy_field(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

fn insert_sanitize_text(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    key: &str,
    limit: usize,
) {
    if let Some(value) = source.get(key).and_then(Value::as_str) {
        if let Some(text) = sanitize_text(value, limit) {
            target.insert(key.to_string(), Value::String(text));
        }
    }
}

fn insert_cleared_field(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if source.get(key).is_some() {
        target.insert(key.to_string(), Value::String("[cleared]".to_string()));
    }
}
