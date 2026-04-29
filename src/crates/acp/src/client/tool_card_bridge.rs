use agent_client_protocol::schema::ToolKind;

pub(super) fn acp_tool_name(
    title: &str,
    raw_input: Option<&serde_json::Value>,
    kind: Option<&ToolKind>,
) -> String {
    if let Some(name) = raw_input.and_then(tool_name_from_raw_input) {
        return normalize_tool_name(&name, title, raw_input, kind);
    }

    normalize_tool_name("", title, raw_input, kind)
}

pub(super) fn normalize_tool_params(
    tool_name: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let Some(object) = params.as_object() else {
        return params;
    };

    let mut normalized = object.clone();
    match tool_name {
        "Bash" => {
            if !normalized.contains_key("command") {
                if let Some(value) = normalized.get("cmd").cloned() {
                    normalized.insert("command".to_string(), value);
                }
            }
            if let Some(value) = normalized.get("command").cloned() {
                normalized.insert(
                    "command".to_string(),
                    serde_json::Value::String(command_value_to_display_text(&value)),
                );
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
                    if let Some(value) = normalized.get("oldString").cloned() {
                        normalized.insert("old_string".to_string(), value);
                    }
                }
                if !normalized.contains_key("new_string") {
                    if let Some(value) = normalized.get("newString").cloned() {
                        normalized.insert("new_string".to_string(), value);
                    }
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
            if !normalized.contains_key("pattern") {
                if let Some(value) = normalized
                    .get("glob")
                    .or_else(|| normalized.get("glob_pattern"))
                    .or_else(|| normalized.get("globPattern"))
                    .or_else(|| normalized.get("file_pattern"))
                    .or_else(|| normalized.get("filePattern"))
                    .cloned()
                {
                    normalized.insert("pattern".to_string(), value);
                }
            }
        }
        _ => {}
    }

    serde_json::Value::Object(normalized)
}

fn tool_name_from_raw_input(raw_input: &serde_json::Value) -> Option<String> {
    let object = raw_input.as_object()?;
    for key in [
        "tool",
        "toolName",
        "tool_name",
        "name",
        "function",
        "action",
    ] {
        let Some(value) = object.get(key).and_then(|value| value.as_str()) else {
            continue;
        };
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn normalize_tool_name(
    candidate: &str,
    title: &str,
    raw_input: Option<&serde_json::Value>,
    kind: Option<&ToolKind>,
) -> String {
    let candidate = candidate.trim();
    let normalized_candidate = normalize_known_tool_alias(candidate);
    if normalized_candidate != candidate || is_native_tool_name(&normalized_candidate) {
        return normalized_candidate;
    }

    let title_lower = title.trim().to_ascii_lowercase();
    let candidate_lower = candidate.to_ascii_lowercase();
    let haystack = format!("{} {}", candidate_lower, title_lower);
    let input = raw_input.and_then(|value| value.as_object());
    if let Some(input) = input {
        if has_any_key(input, &["command", "cmd"]) {
            return "Bash".to_string();
        }
        if has_any_key(
            input,
            &[
                "glob",
                "glob_pattern",
                "globPattern",
                "file_pattern",
                "filePattern",
            ],
        ) {
            return "Glob".to_string();
        }
        if has_any_key(
            input,
            &["pattern", "search_pattern", "searchPattern", "query"],
        ) {
            if contains_any(&haystack, &["web search", "search web"]) {
                return "WebSearch".to_string();
            }
            return "Grep".to_string();
        }
        if has_any_key(
            input,
            &["directory", "dir", "target_directory", "targetDirectory"],
        ) {
            return "LS".to_string();
        }

        let has_file_path = has_any_key(
            input,
            &[
                "file_path",
                "filePath",
                "target_file",
                "targetFile",
                "filename",
                "path",
            ],
        );
        if has_file_path {
            if has_any_key(input, &["content", "contents"]) {
                return "Write".to_string();
            }
            if has_any_key(
                input,
                &["old_string", "oldString", "new_string", "newString"],
            ) {
                return "Edit".to_string();
            }
            match kind {
                Some(ToolKind::Delete) => return "Delete".to_string(),
                Some(ToolKind::Edit) | Some(ToolKind::Move) => return "Edit".to_string(),
                Some(ToolKind::Read) => return "Read".to_string(),
                _ => {}
            }
        }
    }

    if contains_any(
        &haystack,
        &[
            "bash",
            "shell",
            "terminal",
            "command",
            "execute",
            "exec",
            "run command",
        ],
    ) {
        return "Bash".to_string();
    }
    if contains_any(&haystack, &["list", "directory", "folder", "ls"]) {
        return "LS".to_string();
    }
    if contains_any(
        &haystack,
        &["glob", "find file", "file search", "search files"],
    ) {
        return "Glob".to_string();
    }
    if contains_any(&haystack, &["grep", "search", "ripgrep", "rg"]) {
        return "Grep".to_string();
    }
    if contains_any(&haystack, &["write", "create file", "new file"]) {
        return "Write".to_string();
    }
    if contains_any(&haystack, &["edit", "patch", "replace", "modify"]) {
        return "Edit".to_string();
    }
    if contains_any(&haystack, &["delete", "remove", "unlink"]) {
        return "Delete".to_string();
    }
    if contains_any(&haystack, &["read", "open file", "view file"]) {
        return "Read".to_string();
    }
    if contains_any(&haystack, &["web search", "search web"]) {
        return "WebSearch".to_string();
    }

    match kind {
        Some(ToolKind::Read) => "Read".to_string(),
        Some(ToolKind::Edit) => "Edit".to_string(),
        Some(ToolKind::Delete) => "Delete".to_string(),
        Some(ToolKind::Move) => "Edit".to_string(),
        Some(ToolKind::Search) => "Grep".to_string(),
        Some(ToolKind::Execute) => "Bash".to_string(),
        Some(ToolKind::Fetch) => "WebSearch".to_string(),
        Some(ToolKind::Think) | Some(ToolKind::SwitchMode) | Some(ToolKind::Other) | Some(_) => {
            fallback_tool_name(candidate, title)
        }
        None => fallback_tool_name(candidate, title),
    }
}

fn fallback_tool_name(candidate: &str, title: &str) -> String {
    if !candidate.is_empty() {
        candidate.to_string()
    } else {
        let title = title.trim();
        if title.is_empty() {
            "ACP Tool".to_string()
        } else {
            title.to_string()
        }
    }
}

fn normalize_known_tool_alias(name: &str) -> String {
    match name.trim().to_ascii_lowercase().as_str() {
        "read" | "read_file" | "readfile" | "view" | "open" => "Read".to_string(),
        "ls" | "list" | "list_dir" | "list_directory" | "readdir" => "LS".to_string(),
        "grep" | "rg" | "search" | "text_search" => "Grep".to_string(),
        "glob" | "find" | "file_search" => "Glob".to_string(),
        "bash" | "sh" | "shell" | "terminal" | "command" | "cmd" | "execute" => "Bash".to_string(),
        "write" | "write_file" | "create" => "Write".to_string(),
        "edit" | "patch" | "replace" | "update" => "Edit".to_string(),
        "delete" | "remove" | "rm" => "Delete".to_string(),
        "todowrite" | "todo_write" | "todo" => "TodoWrite".to_string(),
        "websearch" | "web_search" | "search_web" => "WebSearch".to_string(),
        _ => name.to_string(),
    }
}

fn is_native_tool_name(name: &str) -> bool {
    matches!(
        name,
        "Read"
            | "Write"
            | "Edit"
            | "Delete"
            | "LS"
            | "Grep"
            | "Glob"
            | "Bash"
            | "TodoWrite"
            | "WebSearch"
    )
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn has_any_key(object: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> bool {
    keys.iter().any(|key| object.contains_key(*key))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_execute_tools_to_bash_card() {
        let input = json!({ "command": "pnpm test" });
        assert_eq!(
            acp_tool_name("Run shell command", Some(&input), Some(&ToolKind::Execute)),
            "Bash"
        );

        let params = normalize_tool_params("Bash", json!({ "cmd": "ls -la" }));
        assert_eq!(params["command"], "ls -la");
    }

    #[test]
    fn normalizes_bash_command_arrays_to_display_string() {
        let params = normalize_tool_params(
            "Bash",
            json!({
                "command": ["/bin/zsh", "-lc", "sed -n '1,120p' src/lib.rs"],
                "cwd": "/tmp/project"
            }),
        );

        assert_eq!(params["command"], "/bin/zsh -lc sed -n '1,120p' src/lib.rs");
        assert_eq!(params["cwd"], "/tmp/project");
    }

    #[test]
    fn normalizes_file_tools_to_native_cards() {
        let read_input = json!({ "path": "src/main.rs" });
        assert_eq!(
            acp_tool_name("Read file", Some(&read_input), Some(&ToolKind::Read)),
            "Read"
        );
        assert_eq!(
            normalize_tool_params("Read", read_input)["file_path"],
            "src/main.rs"
        );

        let write_input = json!({ "path": "README.md", "content": "hello" });
        assert_eq!(
            acp_tool_name("Create file", Some(&write_input), Some(&ToolKind::Edit)),
            "Write"
        );
    }

    #[test]
    fn normalizes_search_tools_to_grep_or_glob_cards() {
        let grep_input = json!({ "query": "AcpClientService" });
        assert_eq!(
            acp_tool_name("Search text", Some(&grep_input), Some(&ToolKind::Search)),
            "Grep"
        );
        assert_eq!(
            normalize_tool_params("Grep", grep_input)["pattern"],
            "AcpClientService"
        );

        let glob_input = json!({ "glob_pattern": "**/*.rs" });
        assert_eq!(
            acp_tool_name("Find files", Some(&glob_input), Some(&ToolKind::Search)),
            "Glob"
        );
        assert_eq!(
            normalize_tool_params("Glob", glob_input)["pattern"],
            "**/*.rs"
        );
    }

    #[test]
    fn search_with_path_stays_search_card() {
        let input = json!({ "pattern": "ToolEventData", "path": "src" });
        assert_eq!(
            acp_tool_name("Search text", Some(&input), Some(&ToolKind::Search)),
            "Grep"
        );
    }

    #[test]
    fn normalizes_camel_case_file_params() {
        let input = json!({
            "filePath": "src/lib.rs",
            "oldString": "before",
            "newString": "after"
        });
        assert_eq!(
            acp_tool_name("Edit file", Some(&input), Some(&ToolKind::Edit)),
            "Edit"
        );

        let params = normalize_tool_params("Edit", input);
        assert_eq!(params["file_path"], "src/lib.rs");
        assert_eq!(params["old_string"], "before");
        assert_eq!(params["new_string"], "after");
    }
}
