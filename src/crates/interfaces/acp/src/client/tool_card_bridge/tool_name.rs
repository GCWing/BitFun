use agent_client_protocol::schema::ToolKind;

pub(super) fn acp_tool_name(
    title: &str,
    raw_input: Option<&serde_json::Value>,
    kind: Option<&ToolKind>,
) -> String {
    if let Some(name) = raw_input.and_then(tool_name_from_raw_input) {
        if let Some(mapped) = editor_family_tool_name(&name, raw_input) {
            return mapped;
        }
        return normalize_tool_name(&name, title, raw_input, kind);
    }

    if let Some(mapped) = editor_family_tool_name(title, raw_input) {
        return mapped;
    }

    // Agents that carry the tool identity in the title alone get the same alias
    // table a rawInput candidate gets. DeepSeek Harness over ACP is one: its
    // rawInput is the tool arguments (`{todos}`, `{command}`, …) with no name
    // key at all, so `todo_write` would otherwise reach the heuristics below and
    // match their "write" substring as a Write card. The match is whole-string,
    // so a descriptive title like "Run shell" still falls through.
    let trimmed_title = title.trim();
    let from_title = normalize_known_tool_alias(trimmed_title);
    if from_title != trimmed_title || is_native_tool_name(&from_title) {
        return from_title;
    }

    normalize_tool_name("", title, raw_input, kind)
}

/// Map the `str_replace_editor` family, whose single tool name multiplexes
/// several file operations behind a `command` argument.
///
/// The name alone cannot answer which card to draw: every view would open an
/// Edit card. Worse, the generic path never gets that far — the `command` key
/// reads as a shell call and the whole family lands on ExecCommand. DeepSeek Harness's
/// minimal preset ships exactly this tool as one of its two.
fn editor_family_tool_name(name: &str, raw_input: Option<&serde_json::Value>) -> Option<String> {
    if !matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "str_replace_editor" | "str_replace_based_edit_tool" | "text_editor"
    ) {
        return None;
    }
    let command = raw_input
        .and_then(|value| value.get("command"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    Some(match command {
        "view" => "Read".to_string(),
        "create" => "Write".to_string(),
        // `str_replace`, `insert`, `undo_edit`, and anything an agent adds
        // later: all of them modify a file that already exists.
        _ => "Edit".to_string(),
    })
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
        // A tool that executes and whose whole argument is a `code` blob runs a
        // program, not a shell line. DeepSeek Harness's Code Mode (PTC) preset
        // is one: it replaces every per-action tool with a single `run_code`.
        // Left to the `Execute` fallback below it would draw a terminal card
        // whose command is empty, because there is no command.
        if matches!(kind, Some(ToolKind::Execute))
            && has_any_key(input, &["code"])
            && !has_any_key(input, &["command", "cmd"])
        {
            return "RunCode".to_string();
        }
        if has_any_key(input, &["command", "cmd"]) {
            return "ExecCommand".to_string();
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
        return "ExecCommand".to_string();
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
        Some(ToolKind::Execute) => "ExecCommand".to_string(),
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
        "bash" | "sh" | "shell" | "terminal" | "command" | "cmd" | "execute" => {
            "ExecCommand".to_string()
        }
        "write" | "write_file" | "create" => "Write".to_string(),
        "edit" | "patch" | "replace" | "update" => "Edit".to_string(),
        "delete" | "remove" | "rm" => "Delete".to_string(),
        "run_code" | "runcode" | "execute_code" | "code_execute" => "RunCode".to_string(),
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
            | "ExecCommand"
            | "RunCode"
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
