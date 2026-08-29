mod tool_name;
mod tool_params;
mod tool_result;

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

/// Translate a completed tool's result into the shape its native card reads.
/// Results the card already understands, and results this cannot parse, are
/// returned unchanged.
pub(super) fn normalize_tool_result(
    tool_name: &str,
    result: serde_json::Value,
) -> serde_json::Value {
    tool_result::normalize_tool_result(tool_name, result)
}

#[cfg(test)]
mod tests {
    use super::{acp_tool_name, normalize_tool_params};
    use agent_client_protocol::schema::ToolKind;
    use serde_json::json;

    #[test]
    fn normalizes_execute_tools_to_exec_command_card() {
        let input = json!({ "command": "pnpm test" });
        assert_eq!(
            acp_tool_name("Run shell command", Some(&input), Some(&ToolKind::Execute)),
            "ExecCommand"
        );

        let params = normalize_tool_params("ExecCommand", json!({ "command": "ls -la" }));
        assert_eq!(params["cmd"], "ls -la");
    }

    #[test]
    fn normalizes_exec_command_arrays_to_display_string() {
        let params = normalize_tool_params(
            "ExecCommand",
            json!({
                "command": ["/bin/zsh", "-lc", "sed -n '1,120p' src/lib.rs"],
                "cwd": "/tmp/project"
            }),
        );

        assert_eq!(params["cmd"], "/bin/zsh -lc sed -n '1,120p' src/lib.rs");
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

    #[test]
    fn resolves_the_tool_identity_from_a_title_that_names_the_tool() {
        // DeepSeek Harness over ACP puts the tool name in the title and only
        // the arguments in rawInput, so the alias table has to reach the title.
        let todo_input = json!({ "todos": [{ "content": "ship it", "status": "pending" }] });
        assert_eq!(
            acp_tool_name("todo_write", Some(&todo_input), Some(&ToolKind::Other)),
            "TodoWrite"
        );
        assert_eq!(acp_tool_name("TodoWrite", None, None), "TodoWrite");

        let read_input = json!({ "file_path": "src/main.rs", "offset": 40 });
        assert_eq!(
            acp_tool_name("read", Some(&read_input), Some(&ToolKind::Read)),
            "Read"
        );

        let command_input = json!({ "command": "cargo test" });
        assert_eq!(
            acp_tool_name("shell", Some(&command_input), Some(&ToolKind::Execute)),
            "ExecCommand"
        );

        // An unknown harness tool keeps its own name rather than being forced
        // into a native card.
        assert_eq!(
            acp_tool_name(
                "subagent",
                Some(&json!({ "task": "review" })),
                Some(&ToolKind::Other)
            ),
            "subagent"
        );
    }

    #[test]
    fn splits_the_str_replace_editor_family_by_its_command() {
        // One tool name, several operations. Without the split every call would
        // land on ExecCommand, because the `command` key looks like a shell call.
        let edit_input = json!({
            "command": "str_replace",
            "path": "/repo/src/lib.rs",
            "old_str": "before",
            "new_str": "after"
        });
        assert_eq!(
            acp_tool_name(
                "str_replace_editor",
                Some(&edit_input),
                Some(&ToolKind::Edit)
            ),
            "Edit"
        );
        let params = normalize_tool_params("Edit", edit_input);
        assert_eq!(params["file_path"], "/repo/src/lib.rs");
        assert_eq!(params["old_string"], "before");
        assert_eq!(params["new_string"], "after");

        let view_input = json!({ "command": "view", "path": "/repo/README.md" });
        assert_eq!(
            acp_tool_name(
                "str_replace_editor",
                Some(&view_input),
                Some(&ToolKind::Read)
            ),
            "Read"
        );

        let create_input = json!({
            "command": "create",
            "path": "/repo/notes.md",
            "file_text": "hi"
        });
        assert_eq!(
            acp_tool_name(
                "str_replace_editor",
                Some(&create_input),
                Some(&ToolKind::Edit)
            ),
            "Write"
        );
        let params = normalize_tool_params("Write", create_input);
        assert_eq!(params["file_path"], "/repo/notes.md");
        assert_eq!(params["content"], "hi");
    }

    #[test]
    fn code_mode_gets_its_own_card_rather_than_an_empty_terminal() {
        // DeepSeek Harness's PTC preset answers every step with one `run_code`
        // call whose argument is a TypeScript program. As an Execute kind with
        // no `command`, it used to land on the command card and render blank.
        let input = json!({
            "code": "const files = await tools.bash({ command: \"ls\" });",
            "description": "List the project root",
        });
        assert_eq!(
            acp_tool_name("run_code", Some(&input), Some(&ToolKind::Execute)),
            "RunCode"
        );
        let params = normalize_tool_params("RunCode", input);
        assert_eq!(params["description"], "List the project root");

        // The shape alone is enough when the tool is named something else.
        assert_eq!(
            acp_tool_name(
                "python",
                Some(&json!({ "code": "print(1)" })),
                Some(&ToolKind::Execute)
            ),
            "RunCode"
        );
        // …and a harness that spells the program differently still fills the
        // field the card reads.
        assert_eq!(
            normalize_tool_params("RunCode", json!({ "source": "print(1)" }))["code"],
            "print(1)"
        );

        // A shell call that happens to carry a `code` argument is still ExecCommand:
        // the command is what ran.
        assert_eq!(
            acp_tool_name(
                "bash",
                Some(&json!({ "command": "echo hi", "code": "unused" })),
                Some(&ToolKind::Execute)
            ),
            "ExecCommand"
        );
    }

    #[test]
    fn descriptive_titles_still_reach_the_heuristics() {
        // Whole-string aliasing only: a prose title is not a tool name, so the
        // rawInput shape keeps deciding these.
        assert_eq!(
            acp_tool_name(
                "Run shell",
                Some(&json!({ "command": "ls" })),
                Some(&ToolKind::Execute)
            ),
            "ExecCommand"
        );
        assert_eq!(
            acp_tool_name(
                "Write to file",
                Some(&json!({ "file_path": "a.txt", "content": "hi" })),
                Some(&ToolKind::Edit)
            ),
            "Write"
        );
    }
}
