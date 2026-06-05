use super::env_snapshot::{remote_env_snapshot_for, RemoteEnvSnapshot};
use super::progress::ExecOutputProgressBridge;
use super::rendering::render_exec_response_for_assistant;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::service::remote_ssh::{
    get_global_remote_exec_process_manager, get_remote_workspace_manager, RemoteExecCommandRequest,
    SSHCommandOptions, SSHConnectionManager,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use terminal_core::{
    get_global_exec_process_manager, LocalExecCommandRequest, ShellDetector, ShellType,
};

const DEFAULT_MAX_OUTPUT_CHARS: u64 = 10_000;
const REMOTE_SHELL_PROBE_TIMEOUT_MS: u64 = 3_000;
const REMOTE_NON_TTY_INTERRUPT_GRACE_SECONDS: u64 = 2;
const POWERSHELL_UTF8_OUTPUT_PREFIX: &str =
    "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;\n";

#[derive(Debug, Clone)]
struct RemoteShell {
    path: String,
    shell_type: ShellType,
}

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
            ("LANG".to_string(), "C.UTF-8".to_string()),
            ("LC_CTYPE".to_string(), "C.UTF-8".to_string()),
            ("COLORTERM".to_string(), String::new()),
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
                context.workspace.as_ref().map(|workspace| {
                    workspace
                        .session_identity
                        .logical_workspace_path()
                        .to_string()
                })
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

    async fn resolve_remote_workdir(
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<String> {
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

        if !raw.starts_with('/') {
            return Err(BitFunError::tool(
                "workdir must be an absolute remote path for ExecCommand".to_string(),
            ));
        }

        let resolved = context.resolve_workspace_tool_path(&raw)?;
        let fs = context.ws_fs().ok_or_else(|| {
            BitFunError::tool("remote workspace filesystem is required for ExecCommand".to_string())
        })?;
        let is_dir = fs.is_dir(&resolved).await.map_err(|error| {
            BitFunError::tool(format!(
                "failed to check remote workdir '{}': {}",
                resolved, error
            ))
        })?;
        if !is_dir {
            return Err(BitFunError::tool(format!(
                "remote workdir does not exist or is not a directory: {}",
                resolved
            )));
        }
        Ok(resolved)
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

    async fn resolve_remote_shell(
        ssh_manager: &SSHConnectionManager,
        connection_id: &str,
    ) -> RemoteShell {
        let probe_command = concat!(
            "printf '%s\\n' \"${SHELL:-}\"; ",
            "getent passwd \"$(id -un)\" 2>/dev/null | cut -d: -f7; ",
            "command -v bash 2>/dev/null; ",
            "command -v zsh 2>/dev/null; ",
            "command -v sh 2>/dev/null"
        );
        let result = ssh_manager
            .execute_command_with_options(
                connection_id,
                probe_command,
                SSHCommandOptions {
                    timeout_ms: Some(REMOTE_SHELL_PROBE_TIMEOUT_MS),
                    cancellation_token: None,
                },
            )
            .await;

        if let Ok(result) = result {
            if !result.timed_out && !result.interrupted && result.exit_code == 0 {
                if let Some(shell) = parse_remote_shell_probe_output(&result.stdout) {
                    return shell;
                }
            }
        }

        RemoteShell {
            path: "/bin/bash".to_string(),
            shell_type: ShellType::Bash,
        }
    }

    fn remote_login_shell_command(
        workdir: &str,
        cmd: &str,
        shell: &RemoteShell,
        env_snapshot: Option<&RemoteEnvSnapshot>,
    ) -> String {
        let env_words = remote_command_env_words(Self::merged_remote_env(env_snapshot));
        let shell_args = remote_shell_login_args().join(" ");

        format!(
            "cd {} && env {} {} {} {}",
            shell_escape(workdir),
            env_words,
            shell_escape(&shell.path),
            shell_args,
            shell_escape(cmd)
        )
    }

    fn remote_non_tty_control_wrapper(cmd: &str, shell_path: &str) -> String {
        let escaped_shell = shell_escape(shell_path);
        let escaped_cmd = shell_escape(cmd);
        format!(
            r#"__bitfun_shell={escaped_shell}
__bitfun_cmd={escaped_cmd}
if command -v setsid >/dev/null 2>&1; then
  setsid "$__bitfun_shell" -lc "$__bitfun_cmd" &
else
  "$__bitfun_shell" -lc "$__bitfun_cmd" &
fi
__bitfun_child=$!
__bitfun_pgid=$__bitfun_child
__bitfun_stop() {{
  __bitfun_signal=${{1:-INT}}
  __bitfun_exit=${{2:-130}}
  __bitfun_grace=${{3:-{REMOTE_NON_TTY_INTERRUPT_GRACE_SECONDS}}}
  trap - INT TERM
  kill -"$__bitfun_signal" "-$__bitfun_pgid" 2>/dev/null || kill -"$__bitfun_signal" "$__bitfun_child" 2>/dev/null || true
  if [ "$__bitfun_grace" -gt 0 ]; then
    sleep "$__bitfun_grace"
  fi
  kill -KILL "-$__bitfun_pgid" 2>/dev/null || kill -KILL "$__bitfun_child" 2>/dev/null || true
  wait "$__bitfun_child" 2>/dev/null || true
  exit "$__bitfun_exit"
}}
trap '__bitfun_stop INT 130 {REMOTE_NON_TTY_INTERRUPT_GRACE_SECONDS}' INT
trap '__bitfun_stop KILL 137 0' TERM
wait "$__bitfun_child"
__bitfun_status=$?
trap - INT TERM
exit "$__bitfun_status""#
        )
    }

    fn merged_remote_env(env_snapshot: Option<&RemoteEnvSnapshot>) -> HashMap<String, String> {
        let mut env = env_snapshot
            .map(|snapshot| snapshot.env.clone())
            .unwrap_or_default();
        env.extend(Self::command_env());
        env
    }

    fn remote_shell_metadata(
        workdir: &str,
        shell: &RemoteShell,
        env_snapshot_applied: bool,
    ) -> Value {
        json!({
            "name": shell.shell_type.name(),
            "type": shell.shell_type.to_string(),
            "path": shell.path,
            "invocation": format!(
                "`cd {} && env ... {} {} <cmd>`",
                shell_escape(workdir),
                shell_escape(&shell.path),
                remote_shell_login_args().join(" ")
            ),
            "remote_env_snapshot_applied": env_snapshot_applied,
        })
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
        let mut status_lines = Vec::new();
        if let Some(exit_code) = data.get("exit_code").and_then(Value::as_i64) {
            status_lines.push(format!("Process exited with code {exit_code}."));
        } else if let Some(session_id) = data.get("session_id").and_then(Value::as_i64) {
            status_lines.push(format!(
                "Process is still running. session_id: {session_id}"
            ));
        }
        render_exec_response_for_assistant(data, status_lines, 3)
    }

    async fn call_remote_pipe(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let cmd = input
            .get("cmd")
            .and_then(Value::as_str)
            .ok_or_else(|| BitFunError::tool("cmd is required for ExecCommand".to_string()))?;
        let tty = input.get("tty").and_then(Value::as_bool).unwrap_or(false);

        let workdir = Self::resolve_remote_workdir(input, context).await?;
        let connection_id = context
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.connection_id())
            .ok_or_else(|| {
                BitFunError::tool("remote connection id is required for ExecCommand".to_string())
            })?
            .to_string();
        let ssh_manager = get_remote_workspace_manager()
            .ok_or_else(|| {
                BitFunError::tool(
                    "remote workspace manager is not initialized for ExecCommand".to_string(),
                )
            })?
            .get_ssh_manager()
            .await
            .ok_or_else(|| {
                BitFunError::tool(
                    "remote SSH manager is not initialized for ExecCommand".to_string(),
                )
            })?;
        let yield_time_ms = input.get("yield_time_ms").and_then(Value::as_u64);
        let max_output_chars = input
            .get("max_output_chars")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_OUTPUT_CHARS)
            .try_into()
            .unwrap_or(usize::MAX);
        let shell = Self::resolve_remote_shell(&ssh_manager, &connection_id).await;
        let env_snapshot = remote_env_snapshot_for(
            ssh_manager.clone(),
            &connection_id,
            &shell.path,
            &shell.shell_type,
        )
        .await;
        let command_body = if tty {
            cmd.to_string()
        } else {
            Self::remote_non_tty_control_wrapper(cmd, &shell.path)
        };
        let command = Self::remote_login_shell_command(
            &workdir,
            &command_body,
            &shell,
            env_snapshot.as_ref(),
        );

        let request = RemoteExecCommandRequest {
            ssh_manager,
            connection_id,
            command,
            tty,
            yield_time_ms,
            max_output_chars: Some(max_output_chars),
        };
        let progress_bridge = ExecOutputProgressBridge::start(context, self.name());
        let response_result = if let Some(bridge) = progress_bridge.as_ref() {
            get_global_remote_exec_process_manager()
                .exec_command_streaming(request, bridge.sender())
                .await
        } else {
            get_global_remote_exec_process_manager()
                .exec_command(request)
                .await
        };
        if let Some(bridge) = progress_bridge {
            bridge.finish().await;
        }
        let response = response_result
            .map_err(|error| BitFunError::tool(format!("ExecCommand failed: {error}")))?;

        let data = json!({
            "chunk_id": response.chunk_id,
            "wall_time_seconds": response.wall_time_seconds,
            "output": response.output,
            "session_id": response.session_id,
            "exit_code": response.exit_code,
            "original_output_chars": response.original_output_chars,
            "workdir": workdir.clone(),
            "tty": tty,
            "remote": true,
            "shell": Self::remote_shell_metadata(&workdir, &shell, env_snapshot.is_some()),
        });
        let result_for_assistant = Self::response_for_assistant(&data);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn parse_remote_shell_probe_output(stdout: &str) -> Option<RemoteShell> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| is_plausible_remote_shell_path(line))
        .map(|path| RemoteShell {
            path: path.to_string(),
            shell_type: ShellType::from_executable(path),
        })
}

fn is_plausible_remote_shell_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains('\0')
        && path.chars().all(|ch| !ch.is_control() || ch == '\t')
}

fn remote_command_env_words(env: HashMap<String, String>) -> String {
    let mut env: Vec<_> = env.into_iter().collect();
    env.sort_by(|(left, _), (right, _)| left.cmp(right));
    env.into_iter()
        .map(|(key, value)| shell_escape(&format!("{key}={value}")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn remote_shell_login_args() -> &'static [&'static str] {
    &["-lc"]
}

#[cfg(test)]
mod tests {
    use super::super::env_snapshot::RemoteEnvSnapshot;
    use super::{parse_remote_shell_probe_output, RemoteShell};
    use super::{ExecCommandTool, POWERSHELL_UTF8_OUTPUT_PREFIX};
    use std::collections::HashMap;
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

    #[test]
    fn remote_login_shell_command_wraps_workdir_env_shell_and_user_command() {
        let shell = RemoteShell {
            path: "/bin/bash".to_string(),
            shell_type: ShellType::Bash,
        };
        let command = ExecCommandTool::remote_login_shell_command(
            "/home/me/project",
            "printf 'hi'",
            &shell,
            None,
        );

        assert!(command.starts_with("cd '/home/me/project' && env "));
        assert!(command.contains("'BITFUN_NONINTERACTIVE=1'"));
        assert!(command.ends_with(" '/bin/bash' -lc 'printf '\\''hi'\\'''"));
    }

    #[test]
    fn remote_login_shell_command_injects_snapshot_before_tool_env() {
        let shell = RemoteShell {
            path: "/bin/bash".to_string(),
            shell_type: ShellType::Bash,
        };
        let snapshot = RemoteEnvSnapshot {
            env: HashMap::from([
                ("PATH".to_string(), "/home/me/.nvm/bin:/usr/bin".to_string()),
                ("TERM".to_string(), "xterm-256color".to_string()),
            ]),
        };
        let command = ExecCommandTool::remote_login_shell_command(
            "/home/me/project",
            "node --version",
            &shell,
            Some(&snapshot),
        );

        assert!(command.contains("'PATH=/home/me/.nvm/bin:/usr/bin'"));
        assert!(command.contains("'TERM=dumb'"));
        assert!(!command.contains("'TERM=xterm-256color'"));
    }

    #[test]
    fn remote_login_shell_command_uses_snapshot_without_interactive_startup() {
        let shell = RemoteShell {
            path: "/bin/bash".to_string(),
            shell_type: ShellType::Bash,
        };
        let snapshot = RemoteEnvSnapshot {
            env: HashMap::from([("PATH".to_string(), "/home/me/.nvm/bin:/usr/bin".to_string())]),
        };
        let command = ExecCommandTool::remote_login_shell_command(
            "/home/me/project",
            "node --version",
            &shell,
            Some(&snapshot),
        );

        assert!(command.contains("'PATH=/home/me/.nvm/bin:/usr/bin'"));
        assert!(command.ends_with(" '/bin/bash' -lc 'node --version'"));
        assert!(!command.contains(" -lic "));
    }

    #[test]
    fn remote_non_tty_control_wrapper_cleans_process_group_after_interrupt_grace() {
        let wrapper =
            ExecCommandTool::remote_non_tty_control_wrapper("python3 -c 'print(1)'", "/bin/bash");

        assert!(wrapper.contains("setsid \"$__bitfun_shell\" -lc \"$__bitfun_cmd\" &"));
        assert!(wrapper.contains("trap '__bitfun_stop INT 130 2' INT"));
        assert!(wrapper.contains("trap '__bitfun_stop KILL 137 0' TERM"));
        assert!(wrapper.contains("__bitfun_grace=${3:-2}"));
        assert!(wrapper.contains("sleep \"$__bitfun_grace\""));
        assert!(wrapper.contains("kill -KILL \"-$__bitfun_pgid\""));
        assert!(wrapper.contains("__bitfun_cmd='python3 -c '\\''print(1)'\\'''"));
    }

    #[test]
    fn remote_shell_probe_prefers_first_plausible_shell_path() {
        let shell = parse_remote_shell_probe_output("\n/bin/zsh\n/usr/bin/bash\n")
            .expect("shell should parse");

        assert_eq!(shell.path, "/bin/zsh");
        assert_eq!(shell.shell_type, ShellType::Zsh);
    }

    #[test]
    fn remote_shell_login_args_use_login_without_interactive_startup() {
        assert_eq!(super::remote_shell_login_args(), &["-lc"]);
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

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        if context.is_some_and(ToolUseContext::is_remote) {
            return Ok(r#"Runs a shell command in the current remote workspace.

Each call starts a separate remote SSH process. Remote commands run through the remote user's default shell with login semantics and, when available, a cached environment snapshot captured from a tool-owned interactive PTY so terminal PATH customizations such as nvm can load without running the user command through interactive shell startup. Remote tty=false runs as SSH exec; remote tty=true runs the same command inside a tool-owned remote PTY.
yield_time_ms waits for output until the process exits or the deadline is reached. It does not stop the process.
If the process is still running, the result includes a numeric session_id. Use WriteStdin to poll for more output or send input to tty=true sessions, and ExecControl to interrupt or kill it.
Output is only what was produced during this tool call's wait window."#
                .to_string());
        }

        self.description().await
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

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
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
            return self.call_remote_pipe(input, context).await;
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

        let request = LocalExecCommandRequest {
            argv: Self::argv_for_shell(&shell.path, &shell.shell_type, cmd),
            cwd: workdir.clone(),
            env: Self::command_env(),
            tty,
            yield_time_ms,
            max_output_chars: Some(max_output_chars),
        };
        let progress_bridge = ExecOutputProgressBridge::start(context, self.name());
        let response_result = if let Some(bridge) = progress_bridge.as_ref() {
            get_global_exec_process_manager()
                .exec_command_streaming(request, bridge.sender())
                .await
        } else {
            get_global_exec_process_manager()
                .exec_command(request)
                .await
        };
        if let Some(bridge) = progress_bridge {
            bridge.finish().await;
        }
        let response = response_result
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
