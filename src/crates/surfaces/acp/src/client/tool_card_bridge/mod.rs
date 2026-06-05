mod tool_name;
mod tool_params;

pub(super) fn acp_tool_name(
    title: &str,
    raw_input: Option<&serde_json::Value>,
    kind: Option<&agent_client_protocol::schema::ToolKind>,
) -> String {
    tool_name::acp_tool_name(title, raw_input, kind)
}

pub(super) fn normalize_tool_params(
    tool_name: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    tool_params::normalize_tool_params(tool_name, params)
}

#[cfg(test)]
mod tests {
    use super::{acp_tool_name, normalize_tool_params};
    use agent_client_protocol::schema::ToolKind;
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
