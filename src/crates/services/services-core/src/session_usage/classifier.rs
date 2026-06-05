use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageToolCategory {
    Git,
    Shell,
    File,
    Other,
}

pub fn classify_tool_usage(
    tool_name: &str,
    input: Option<&serde_json::Value>,
) -> UsageToolCategory {
    let normalized_tool = tool_name.trim().to_ascii_lowercase();

    if normalized_tool == "git" || normalized_tool.starts_with("git_") {
        return UsageToolCategory::Git;
    }

    if is_file_tool(&normalized_tool) {
        return UsageToolCategory::File;
    }

    if is_shell_tool(&normalized_tool) {
        if input
            .and_then(extract_command)
            .is_some_and(command_invokes_git)
        {
            return UsageToolCategory::Git;
        }
        return UsageToolCategory::Shell;
    }

    UsageToolCategory::Other
}

fn is_file_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read_file" | "write_file" | "edit_file" | "create_file" | "delete_file"
    )
}

fn is_shell_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "shell" | "terminal" | "run_command" | "execute_command" | "bash" | "powershell"
    )
}

fn extract_command(input: &serde_json::Value) -> Option<&str> {
    let object = input.as_object()?;
    ["command", "cmd", "script"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(|value| value.as_str()))
}

fn command_invokes_git(command: &str) -> bool {
    let first_token = command
        .trim_start()
        .trim_start_matches('&')
        .trim_start()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|ch| ch == '"' || ch == '\'' || ch == '`')
        .replace('\\', "/")
        .to_ascii_lowercase();

    first_token == "git"
        || first_token.ends_with("/git")
        || first_token.ends_with("/git.exe")
        || first_token == "git.exe"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_dedicated_git_tool_as_git() {
        assert_eq!(
            classify_tool_usage("git_status", None),
            UsageToolCategory::Git
        );
    }

    #[test]
    fn classify_shell_git_executable_as_git() {
        let input = serde_json::json!({ "command": "git status --short" });

        assert_eq!(
            classify_tool_usage("execute_command", Some(&input)),
            UsageToolCategory::Git
        );
    }

    #[test]
    fn do_not_classify_command_containing_git_text_as_git() {
        let input = serde_json::json!({ "command": "echo git status" });

        assert_eq!(
            classify_tool_usage("execute_command", Some(&input)),
            UsageToolCategory::Shell
        );
    }
}
