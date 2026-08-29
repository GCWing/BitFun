pub(super) fn normalize_tool_params(
    tool_name: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let Some(object) = params.as_object() else {
        return params;
    };

    let mut normalized = object.clone();
    match tool_name {
        "ExecCommand" => {
            if !normalized.contains_key("cmd") {
                if let Some(value) = normalized.get("command").cloned() {
                    normalized.insert("cmd".to_string(), value);
                }
            }
            if let Some(value) = normalized.get("cmd").cloned() {
                normalized.insert(
                    "cmd".to_string(),
                    serde_json::Value::String(command_value_to_display_text(&value)),
                );
            }
        }
        "RunCode" => {
            if !normalized.contains_key("code") {
                if let Some(value) = normalized
                    .get("source")
                    .or_else(|| normalized.get("script"))
                    .or_else(|| normalized.get("program"))
                    .cloned()
                {
                    normalized.insert("code".to_string(), value);
                }
            }
        }
        "Read" | "Write" | "Edit" | "Delete" => {
            if !normalized.contains_key("file_path") {
                if let Some(value) = normalized
                    .get("path")
                    .or_else(|| normalized.get("target_file"))
                    .or_else(|| normalized.get("targetFile"))
                    .or_else(|| normalized.get("filePath"))
                    .or_else(|| normalized.get("filename"))
                    .cloned()
                {
                    normalized.insert("file_path".to_string(), value);
                }
            }
            if tool_name == "Edit" {
                if !normalized.contains_key("old_string") {
                    // `old_str`/`new_str` are the `str_replace_editor` family's
                    // spelling; without them its card shows a path and no diff.
                    if let Some(value) = normalized
                        .get("oldString")
                        .or_else(|| normalized.get("old_str"))
                        .cloned()
                    {
                        normalized.insert("old_string".to_string(), value);
                    }
                }
                if !normalized.contains_key("new_string") {
                    if let Some(value) = normalized
                        .get("newString")
                        .or_else(|| normalized.get("new_str"))
                        .cloned()
                    {
                        normalized.insert("new_string".to_string(), value);
                    }
                }
            }
            if tool_name == "Write" && !normalized.contains_key("content") {
                if let Some(value) = normalized
                    .get("file_text")
                    .or_else(|| normalized.get("contents"))
                    .cloned()
                {
                    normalized.insert("content".to_string(), value);
                }
            }
        }
        "LS" => {
            if !normalized.contains_key("path") {
                if let Some(value) = normalized
                    .get("directory")
                    .or_else(|| normalized.get("dir"))
                    .or_else(|| normalized.get("target_directory"))
                    .or_else(|| normalized.get("targetDirectory"))
                    .cloned()
                {
                    normalized.insert("path".to_string(), value);
                }
            }
        }
        "Grep" => {
            if !normalized.contains_key("pattern") {
                if let Some(value) = normalized
                    .get("query")
                    .or_else(|| normalized.get("text"))
                    .or_else(|| normalized.get("search_pattern"))
                    .or_else(|| normalized.get("searchPattern"))
                    .cloned()
                {
                    normalized.insert("pattern".to_string(), value);
                }
            }
        }
        "Glob" => {
            let pattern = if !normalized.contains_key("pattern") {
                normalized
                    .get("glob")
                    .or_else(|| normalized.get("glob_pattern"))
                    .or_else(|| normalized.get("globPattern"))
                    .or_else(|| normalized.get("file_pattern"))
                    .or_else(|| normalized.get("filePattern"))
                    .cloned()
            } else {
                None
            };
            if let Some(value) = pattern {
                normalized.insert("pattern".to_string(), value);
            }
        }
        _ => {}
    }

    serde_json::Value::Object(normalized)
}

fn command_value_to_display_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(command_value_to_display_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}
