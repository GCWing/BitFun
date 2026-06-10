use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::agentic::tools::implementations::shell_command_safety;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use terminal_core::{
    get_global_exec_process_manager, LocalExecCommandRequest, ShellDetector, ShellType,
};

const DEFAULT_MAX_OUTPUT_CHARS: u64 = 10_000;
const POWERSHELL_UTF8_OUTPUT_PREFIX: &str =
    "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;\n";

pub struct ExecCommandTool;

impl Default for ExecCommandTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecCommandTool {
    pub fn new() -> Self {
        Self
    }

    fn command_env() -> HashMap<String, String> {
        HashMap::from([
            ("NO_COLOR".to_string(), "1".to_string()),
            ("TERM".to_string(), "dumb".to_string()),
            ("CLICOLOR".to_string(), "0".to_string()),
            ("PAGER".to_string(), "cat".to_string()),
            ("GIT_PAGER".to_string(), "cat".to_string()),
            ("GH_PAGER".to_string(), "cat".to_string()),
            ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
            ("GIT_EDITOR".to_string(), "true".to_string()),
            ("BITFUN_NONINTERACTIVE".to_string(), "1".to_string()),
        ])
    }

    fn resolve_workdir(input: &Value, context: &ToolUseContext) -> BitFunResult<PathBuf> {
        let raw = input
            .get("workdir")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|workdir| !workdir.is_empty())
            .map(str::to_string)
            .or_else(|| {
                context
                    .workspace_root()
                    .map(|path| path.to_string_lossy().to_string())
            })
            .ok_or_else(|| {
                BitFunError::tool("workspace root is required for ExecCommand".to_string())
            })?;

        let path = PathBuf::from(&raw);
        if !path.is_absolute() {
            return Err(BitFunError::tool(
                "workdir must be an absolute path for ExecCommand".to_string(),
            ));
        }
        if !path.is_dir() {
            return Err(BitFunError::tool(format!(
                "workdir does not exist or is not a directory: {}",
                path.display()
            )));
        }
        Ok(path)
    }

    fn argv_for_shell(path: &Path, shell_type: &ShellType, cmd: &str) -> Vec<String> {
        let shell = path.to_string_lossy().to_string();
        match shell_type {
            ShellType::Bash
            | ShellType::Zsh
            | ShellType::Fish
            | ShellType::Sh
            | ShellType::Ksh
            | ShellType::Csh
            | ShellType::Custom(_) => vec![shell, "-lc".to_string(), cmd.to_string()],
            ShellType::PowerShell | ShellType::PowerShellCore => {
                vec![
                    shell,
                    "-Command".to_string(),
                    Self::powershell_command_with_utf8_output(cmd),
                ]
            }
            ShellType::Cmd => vec![shell, "/c".to_string(), cmd.to_string()],
        }
    }

    fn powershell_command_with_utf8_output(cmd: &str) -> String {
        let trimmed = cmd.trim_start();
        if trimmed.starts_with(POWERSHELL_UTF8_OUTPUT_PREFIX) {
            cmd.to_string()
        } else {
            format!("{POWERSHELL_UTF8_OUTPUT_PREFIX}{cmd}")
        }
    }

    fn shell_invocation_for_model(path: &Path, shell_type: &ShellType) -> String {
        let shell = path.to_string_lossy();
        match shell_type {
            ShellType::Bash
            | ShellType::Zsh
            | ShellType::Fish
            | ShellType::Sh
            | ShellType::Ksh
            | ShellType::Csh
            | ShellType::Custom(_) => format!("`{shell} -lc <cmd>`"),
            ShellType::PowerShell | ShellType::PowerShellCore => {
                format!("`{shell} -Command <cmd>`")
            }
            ShellType::Cmd => format!("`{shell} /c <cmd>`"),
        }
    }

    fn detected_shell_for_model() -> (String, PathBuf, ShellType, String) {
        let shell = ShellDetector::get_default_shell();
        let invocation = Self::shell_invocation_for_model(&shell.path, &shell.shell_type);
        (shell.display_name, shell.path, shell.shell_type, invocation)
    }

    fn response_for_assistant(data: &Value) -> String {
        let output = data.get("output").and_then(Value::as_str).unwrap_or("");
        let mut lines = Vec::new();
        if let Some(exit_code) = data.get("exit_code").and_then(Value::as_i64) {
            lines.push(format!("Process exited with code {exit_code}."));
        } else if let Some(session_id) = data.get("session_id").and_then(Value::as_i64) {
            lines.push(format!(
                "Process is still running. session_id: {session_id}"
            ));
        }
        lines.push(format!(
            "Wall time: {:.3} seconds",
            data.get("wall_time_seconds")
                .and_then(Value::as_f64)
                .unwrap_or_default()
        ));
        if !output.is_empty() {
            lines.push("Output:".to_string());
            lines.push(output.to_string());
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{ExecCommandTool, POWERSHELL_UTF8_OUTPUT_PREFIX};
    use std::path::Path;
    use terminal_core::ShellType;

    #[test]
    fn powershell_commands_force_utf8_output() {
        let argv = ExecCommandTool::argv_for_shell(
            Path::new("pwsh"),
            &ShellType::PowerShellCore,
            "Get-Content README.md",
        );

        assert_eq!(argv[1], "-Command");
        assert!(argv[2].starts_with(POWERSHELL_UTF8_OUTPUT_PREFIX));
        assert!(argv[2].contains("Get-Content README.md"));
    }

    #[test]
    fn powershell_utf8_output_prefix_is_not_duplicated() {
        let script = format!("{POWERSHELL_UTF8_OUTPUT_PREFIX}Write-Output ok");
        let argv =
            ExecCommandTool::argv_for_shell(Path::new("pwsh"), &ShellType::PowerShellCore, &script);

        assert_eq!(argv[2], script);
    }
}

#[async_trait]
impl Tool for ExecCommandTool {
    fn name(&self) -> &str {
        "ExecCommand"
    }

    async fn description(&self) -> BitFunResult<String> {
        let (shell_name, shell_path, _shell_type, shell_invocation) =
            Self::detected_shell_for_model();
        Ok(format!(
            r#"Runs a shell command.

Each call starts a separate process. Commands currently run through {shell_name} at `{shell_path}` as {shell_invocation}.
Use tty=true only for commands that need interactive stdin; otherwise leave tty=false.
yield_time_ms waits for output until the process exits or the deadline is reached. It does not stop the process.
If the process is still running, the result includes a numeric session_id. Use WriteStdin to poll or send input, and ExecControl to interrupt or kill it.
Output is only what was produced during this tool call's wait window."#,
            shell_path = shell_path.display(),
        ))
    }

    fn short_description(&self) -> String {
        "Run a command in a fresh process.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "cmd": {
                    "type": "string",
                    "description": "Shell command to execute."
                },
                "workdir": {
                    "type": "string",
                    "description": "Optional absolute working directory path. Defaults to the workspace root."
                },
                "tty": {
                    "type": "boolean",
                    "description": "Set true only for commands that need interactive stdin. Defaults to false."
                },
                "yield_time_ms": {
                    "type": "number",
                    "description": "How long to wait for output before yielding. This does not stop the process."
                },
                "max_output_chars": {
                    "type": "number",
                    "description": "Maximum output characters to return. Defaults to 10000; excess output keeps head and tail."
                }
            },
            "required": ["cmd"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        let Some(cmd) = input
            .and_then(|value| value.get("cmd"))
            .and_then(Value::as_str)
        else {
            return false;
        };
        shell_command_safety::exec_command_is_concurrency_safe(cmd)
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let cmd = input.get("cmd").and_then(Value::as_str).unwrap_or_default();
        if cmd.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("cmd is required for ExecCommand".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        if let Some(message) = shell_command_safety::denial_for_command(cmd) {
            return ValidationResult {
                result: false,
                message: Some(message),
                error_code: Some(403),
                meta: None,
            };
        }
        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if context.is_remote() {
            return Err(BitFunError::tool(
                "ExecCommand does not support remote workspaces yet.".to_string(),
            ));
        }

        let cmd = input
            .get("cmd")
            .and_then(Value::as_str)
            .ok_or_else(|| BitFunError::tool("cmd is required for ExecCommand".to_string()))?;
        let workdir = Self::resolve_workdir(input, context)?;
        let tty = input.get("tty").and_then(Value::as_bool).unwrap_or(false);
        let shell = ShellDetector::get_default_shell();
        let yield_time_ms = input.get("yield_time_ms").and_then(Value::as_u64);
        let max_output_chars = input
            .get("max_output_chars")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_OUTPUT_CHARS)
            .try_into()
            .unwrap_or(usize::MAX);

        let response = get_global_exec_process_manager()
            .exec_command(LocalExecCommandRequest {
                argv: Self::argv_for_shell(&shell.path, &shell.shell_type, cmd),
                cwd: workdir.clone(),
                env: Self::command_env(),
                tty,
                yield_time_ms,
                max_output_chars: Some(max_output_chars),
            })
            .await
            .map_err(|error| BitFunError::tool(format!("ExecCommand failed: {error}")))?;

        let data = json!({
            "chunk_id": response.chunk_id,
            "wall_time_seconds": response.wall_time_seconds,
            "output": response.output,
            "session_id": response.session_id,
            "exit_code": response.exit_code,
            "original_output_chars": response.original_output_chars,
            "workdir": workdir.to_string_lossy(),
            "tty": tty,
            "shell": {
                "name": shell.display_name,
                "type": shell.shell_type.to_string(),
                "path": shell.path.to_string_lossy(),
                "invocation": Self::shell_invocation_for_model(&shell.path, &shell.shell_type),
            },
        });
        let result_for_assistant = Self::response_for_assistant(&data);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}
