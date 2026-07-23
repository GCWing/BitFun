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
