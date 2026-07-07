use super::background_command_output::{
    background_command_output_capture, BackgroundCommandOutputStatus,
    StartBackgroundCommandOutputCapture,
};
use super::completion::{exec_command_local_completion, exec_command_remote_completion};
use super::env_snapshot::{remote_env_snapshot_for, RemoteEnvSnapshot};
use super::local_shell::{resolve_local_exec_shell, ResolvedLocalExecShell};
use super::progress::ExecOutputProgressBridge;
use super::shell_kind::{exec_command_shell_kind, terminal_shell_type};
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::infrastructure::events::event_system::{
    get_global_event_system, BackendEvent::BackgroundCommandLifecycle,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::event::BackgroundCommandLifecycleInfo;
use async_trait::async_trait;
use bitfun_runtime_ports::{
    RemoteExecCommandRequest, RemoteExecOneShotCommandRequest, RemoteExecPort,
    RemoteExecProcessLifecycleEvent, RemoteExecProcessLifecycleStatus, TerminalExecCommandRequest,
    TerminalExecProcessLifecycleEvent, TerminalExecProcessLifecycleStatus,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use terminal_core::ShellType;
use tokio::sync::mpsc;
use tool_runtime::exec_command::{
    exec_command_argv_for_shell, exec_command_background_output_status,
    exec_command_lifecycle_background_output_status, exec_command_lifecycle_status_name,
    exec_command_noninteractive_env, exec_command_result_value, exec_command_run_input_from_input,
    exec_command_run_input_validation_message, exec_command_shell_escape,
    exec_command_shell_invocation_for_model, fallback_remote_exec_shell,
    parse_remote_exec_shell_probe_output, remote_exec_login_shell_command,
    remote_exec_non_tty_control_wrapper, remote_exec_shell_login_args,
    remote_exec_shell_probe_command, render_exec_command_response_for_assistant,
    ExecCommandLifecycleStatus, ExecCommandResultData, ExecCommandResultFields,
    ExecCommandShellMetadata, REMOTE_EXEC_SHELL_PROBE_TIMEOUT_MS,
};

#[derive(Debug, Clone)]
struct RemoteShell {
    path: String,
    shell_type: ShellType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecCommandShellPromptInfo {
    pub display_name: String,
    pub shell_type: String,
    pub path: String,
    pub invocation: String,
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

    pub(crate) async fn local_shell_prompt_info() -> ExecCommandShellPromptInfo {
        let shell = resolve_local_exec_shell().await;
        ExecCommandShellPromptInfo {
            display_name: shell.display_name,
            shell_type: shell.shell_type.to_string(),
            path: shell.path.to_string_lossy().to_string(),
            invocation: Self::shell_invocation_for_model(&shell.path, &shell.shell_type),
        }
    }

    fn command_env() -> HashMap<String, String> {
        exec_command_noninteractive_env()
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
        exec_command_argv_for_shell(
            path.to_string_lossy().to_string(),
            exec_command_shell_kind(shell_type),
            cmd,
        )
    }

    async fn resolve_remote_shell(
        remote_exec_port: &Arc<dyn RemoteExecPort>,
        connection_id: &str,
    ) -> RemoteShell {
        let result = remote_exec_port
            .exec_command_once(RemoteExecOneShotCommandRequest {
                connection_id: connection_id.to_string(),
                command: remote_exec_shell_probe_command().to_string(),
                timeout_ms: Some(REMOTE_EXEC_SHELL_PROBE_TIMEOUT_MS),
            })
            .await;

        if let Ok(result) = result {
            if !result.timed_out && !result.interrupted && result.exit_code == 0 {
                if let Some(shell) = parse_remote_shell_probe_output(&result.stdout) {
                    return shell;
                }
            }
        }

        let fallback = fallback_remote_exec_shell();
        RemoteShell {
            path: fallback.path,
            shell_type: terminal_shell_type(fallback.kind),
        }
    }

    fn remote_login_shell_command(
        workdir: &str,
        cmd: &str,
        shell: &RemoteShell,
        env_snapshot: Option<&RemoteEnvSnapshot>,
    ) -> String {
        remote_exec_login_shell_command(workdir, cmd, &shell.path, env_snapshot)
    }

    fn remote_non_tty_control_wrapper(cmd: &str, shell_path: &str) -> String {
        remote_exec_non_tty_control_wrapper(cmd, shell_path)
    }

    fn remote_shell_metadata(
        workdir: &str,
        shell: &RemoteShell,
        env_snapshot_applied: bool,
    ) -> ExecCommandShellMetadata {
        ExecCommandShellMetadata {
            name: shell.shell_type.name().to_string(),
            kind: shell.shell_type.to_string(),
            path: shell.path.clone(),
            invocation: format!(
                "`cd {} && env ... {} {} <cmd>`",
                exec_command_shell_escape(workdir),
                exec_command_shell_escape(&shell.path),
                remote_exec_shell_login_args().join(" ")
            ),
            remote_env_snapshot_applied: Some(env_snapshot_applied),
        }
    }

    fn shell_invocation_for_model(path: &Path, shell_type: &ShellType) -> String {
        exec_command_shell_invocation_for_model(
            &path.to_string_lossy(),
            exec_command_shell_kind(shell_type),
        )
    }

    fn shell_metadata_value(shell: &ResolvedLocalExecShell) -> ExecCommandShellMetadata {
        ExecCommandShellMetadata {
            name: shell.display_name.clone(),
            kind: shell.shell_type.to_string(),
            path: shell.path.to_string_lossy().to_string(),
            invocation: Self::shell_invocation_for_model(&shell.path, &shell.shell_type),
            remote_env_snapshot_applied: None,
        }
    }

    fn response_for_assistant(data: &Value) -> String {
        render_exec_command_response_for_assistant(data)
    }

    fn local_lifecycle_status(
        status: TerminalExecProcessLifecycleStatus,
    ) -> ExecCommandLifecycleStatus {
        match status {
            TerminalExecProcessLifecycleStatus::Running => ExecCommandLifecycleStatus::Running,
            TerminalExecProcessLifecycleStatus::Exited => ExecCommandLifecycleStatus::Exited,
            TerminalExecProcessLifecycleStatus::Interrupted => {
                ExecCommandLifecycleStatus::Interrupted
            }
            TerminalExecProcessLifecycleStatus::Killed => ExecCommandLifecycleStatus::Killed,
            TerminalExecProcessLifecycleStatus::Pruned => ExecCommandLifecycleStatus::Pruned,
        }
    }

    fn remote_lifecycle_status(
        status: RemoteExecProcessLifecycleStatus,
    ) -> ExecCommandLifecycleStatus {
        match status {
            RemoteExecProcessLifecycleStatus::Running => ExecCommandLifecycleStatus::Running,
            RemoteExecProcessLifecycleStatus::Exited => ExecCommandLifecycleStatus::Exited,
            RemoteExecProcessLifecycleStatus::Interrupted => {
                ExecCommandLifecycleStatus::Interrupted
            }
            RemoteExecProcessLifecycleStatus::Killed => ExecCommandLifecycleStatus::Killed,
            RemoteExecProcessLifecycleStatus::Pruned => ExecCommandLifecycleStatus::Pruned,
        }
    }

    fn now_unix_seconds() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn start_local_lifecycle_bridge(
        context: &ToolUseContext,
        _tool_name: &str,
    ) -> Option<mpsc::UnboundedSender<TerminalExecProcessLifecycleEvent>> {
        let capture_id = context.tool_call_id.clone()?;
        let agent_session_id = context.session_id.clone();
        let (tx, mut rx) = mpsc::unbounded_channel::<TerminalExecProcessLifecycleEvent>();
        tokio::spawn(async move {
            let event_system = get_global_event_system();
            let output_capture = background_command_output_capture();
            while let Some(event) = rx.recv().await {
                let status = Self::local_lifecycle_status(event.status);
                let capture_status = exec_command_lifecycle_background_output_status(status);
                if let Some(metadata) = output_capture
                    .update_lifecycle(
                        &capture_id,
                        event.session_id,
                        capture_status,
                        event.exit_code,
                    )
                    .await
                {
                    let timestamp = Self::now_unix_seconds();
                    let _ = event_system
                        .emit(BackgroundCommandLifecycle(BackgroundCommandLifecycleInfo {
                            agent_session_id: metadata
                                .agent_session_id
                                .or(agent_session_id.clone()),
                            exec_session_id: event.session_id,
                            command: metadata.command,
                            workdir: metadata.workdir,
                            remote: false,
                            tty: metadata.tty,
                            status: exec_command_lifecycle_status_name(status).to_string(),
                            exit_code: event.exit_code,
                            started_at: metadata.started_at,
                            ended_at: metadata.ended_at,
                            timestamp,
                        }))
                        .await;
                }
            }
        });
        Some(tx)
    }

    fn start_remote_lifecycle_bridge(
        context: &ToolUseContext,
        _tool_name: &str,
    ) -> Option<mpsc::UnboundedSender<RemoteExecProcessLifecycleEvent>> {
        let capture_id = context.tool_call_id.clone()?;
        let agent_session_id = context.session_id.clone();
        let (tx, mut rx) = mpsc::unbounded_channel::<RemoteExecProcessLifecycleEvent>();
        tokio::spawn(async move {
            let event_system = get_global_event_system();
            let output_capture = background_command_output_capture();
            while let Some(event) = rx.recv().await {
                let status = Self::remote_lifecycle_status(event.status);
                let capture_status = exec_command_lifecycle_background_output_status(status);
                if let Some(metadata) = output_capture
                    .update_lifecycle(
                        &capture_id,
                        event.session_id,
                        capture_status,
                        event.exit_code,
                    )
                    .await
                {
                    let timestamp = Self::now_unix_seconds();
                    let _ = event_system
                        .emit(BackgroundCommandLifecycle(BackgroundCommandLifecycleInfo {
                            agent_session_id: metadata
                                .agent_session_id
                                .or(agent_session_id.clone()),
                            exec_session_id: event.session_id,
                            command: metadata.command,
                            workdir: metadata.workdir,
                            remote: true,
                            tty: metadata.tty,
                            status: exec_command_lifecycle_status_name(status).to_string(),
                            exit_code: event.exit_code,
                            started_at: metadata.started_at,
                            ended_at: metadata.ended_at,
                            timestamp,
                        }))
                        .await;
                }
            }
        });
        Some(tx)
    }

    async fn call_remote_pipe(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let parsed_input = exec_command_run_input_from_input(input)
            .ok_or_else(|| BitFunError::tool("cmd is required for ExecCommand".to_string()))?;
        let cmd = parsed_input.cmd;
        let tty = parsed_input.tty;

        let workdir = Self::resolve_remote_workdir(input, context).await?;
        let connection_id = context
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.connection_id())
            .ok_or_else(|| {
                BitFunError::tool("remote connection id is required for ExecCommand".to_string())
            })?
            .to_string();
        let remote_exec_port = context.remote_exec_port().ok_or_else(|| {
            BitFunError::tool("remote exec runtime service is required for ExecCommand".to_string())
        })?;
        let yield_time_ms = parsed_input.yield_time_ms;
        let shell = Self::resolve_remote_shell(remote_exec_port, &connection_id).await;
        let env_snapshot = remote_env_snapshot_for(
            remote_exec_port,
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
        let output_capture_tx = if let Some(capture_id) = context.tool_call_id.as_ref() {
            Some(
                background_command_output_capture()
                    .start_capture(StartBackgroundCommandOutputCapture {
                        capture_id: capture_id.clone(),
                        agent_session_id: context.session_id.clone(),
                        command: cmd.to_string(),
                        workdir: Some(workdir.clone()),
                        remote: true,
                        tty,
                    })
                    .await,
            )
        } else {
            None
        };

        let request = RemoteExecCommandRequest {
            connection_id,
            command,
            tty,
            yield_time_ms: Some(yield_time_ms),
            max_output_chars: None,
            lifecycle_sink: Self::start_remote_lifecycle_bridge(context, self.name()),
            output_sink: output_capture_tx,
        };
        let progress_bridge = ExecOutputProgressBridge::start(context, self.name());
        let response_result = if let Some(bridge) = progress_bridge.as_ref() {
            remote_exec_port
                .exec_command_streaming(request, bridge.sender())
                .await
        } else {
            remote_exec_port.exec_command(request).await
        };
        if let Some(bridge) = progress_bridge {
            bridge.finish().await;
        }
        let response = match response_result {
            Ok(response) => response,
            Err(error) => {
                if let Some(capture_id) = context.tool_call_id.as_ref() {
                    background_command_output_capture()
                        .finish(capture_id, BackgroundCommandOutputStatus::Failed, None)
                        .await;
                }
                return Err(BitFunError::tool(format!(
                    "ExecCommand failed: {}",
                    error.message
                )));
            }
        };
        let completion = response.completion.map(exec_command_remote_completion);
        if let Some(capture_id) = context.tool_call_id.as_ref() {
            if let Some(session_id) = response.session_id {
                background_command_output_capture()
                    .set_session_id(capture_id, Some(session_id))
                    .await;
            }
            if response.session_id.is_none() {
                background_command_output_capture()
                    .finish(
                        capture_id,
                        exec_command_background_output_status(completion),
                        response.exit_code,
                    )
                    .await;
            }
        }

        let data = exec_command_result_value(ExecCommandResultData {
            fields: ExecCommandResultFields {
                chunk_id: response.chunk_id,
                wall_time_seconds: response.wall_time_seconds,
                output: response.output,
                session_id: response.session_id,
                exit_code: response.exit_code,
                original_output_chars: response.original_output_chars,
                completion,
                remote: true,
            },
            workdir: workdir.clone(),
            tty,
            shell: Self::remote_shell_metadata(&workdir, &shell, env_snapshot.is_some()),
        });
        let result_for_assistant = Self::response_for_assistant(&data);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

fn parse_remote_shell_probe_output(stdout: &str) -> Option<RemoteShell> {
    parse_remote_exec_shell_probe_output(stdout).map(|shell| RemoteShell {
        path: shell.path,
        shell_type: terminal_shell_type(shell.kind),
    })
}

#[async_trait]
impl Tool for ExecCommandTool {
    fn name(&self) -> &str {
        "ExecCommand"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Runs a shell command in a separate process.

TTY modes:
- `tty=false` (Default): 
  - runs without a PTY. Local commands use pipe-backed stdio; remote commands use a non-PTY SSH exec channel.
  - No interactive stdin is attached, so input-waiting programs may see EOF instead of a prompt.
  - programs may block-buffer pipe output, so output may appear only after the process exits. Use unbuffered flags/env vars such as `python -u` or `PYTHONUNBUFFERED=1` or TTY mode when progressive output matters.
- `tty=true`: allocates a PTY and gives the command terminal semantics. Use it for commands that need interactive stdin or terminal behavior.

Waiting and continuation:
- yield_time_ms waits for output until the process exits or the deadline is reached. It does not stop the process.
- If the process is still running after `yield_time_ms`, the result includes a numeric session_id.
- Use WriteStdin to poll for more output or send input to tty=true sessions, and ExecControl to interrupt or kill it.

Output:
- Output is only what was produced during this tool call's wait window.
- In non-TTY mode, stdout and stderr ordering is not guaranteed; use tty=true or redirect stderr with 2>&1 when terminal ordering matters."#
            .to_string())
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        let mut base = self.description().await?;
        if context.map(|c| c.is_remote()).unwrap_or(false) {
            base = format!(
                r#"**Remote workspace:** Commands run on the **SSH server** in the remote user's default POSIX shell, invoked as `<shell> -lc <cmd>`. Use **Unix** syntax and POSIX paths — not PowerShell, `cmd.exe`, or Windows paths.

{base}"#,
                base = base
            );
        }
        Ok(base)
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
                    "description": "How long to wait for output before yielding. Defaults to 30000 ms."
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
        if let Some(message) = exec_command_run_input_validation_message(input) {
            return ValidationResult {
                result: false,
                message: Some(message.to_string()),
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

        let parsed_input = exec_command_run_input_from_input(input)
            .ok_or_else(|| BitFunError::tool("cmd is required for ExecCommand".to_string()))?;
        let cmd = parsed_input.cmd;
        let workdir = Self::resolve_workdir(input, context)?;
        let tty = parsed_input.tty;
        let shell = resolve_local_exec_shell().await;
        let yield_time_ms = parsed_input.yield_time_ms;
        let terminal_port = context.terminal_port().ok_or_else(|| {
            BitFunError::tool("terminal runtime service is required for ExecCommand".to_string())
        })?;
        let output_capture_tx = if let Some(capture_id) = context.tool_call_id.as_ref() {
            Some(
                background_command_output_capture()
                    .start_capture(StartBackgroundCommandOutputCapture {
                        capture_id: capture_id.clone(),
                        agent_session_id: context.session_id.clone(),
                        command: cmd.to_string(),
                        workdir: Some(workdir.to_string_lossy().to_string()),
                        remote: false,
                        tty,
                    })
                    .await,
            )
        } else {
            None
        };

        let request = TerminalExecCommandRequest {
            argv: Self::argv_for_shell(&shell.path, &shell.shell_type, cmd),
            cwd: workdir.clone(),
            env: Self::command_env(),
            tty,
            yield_time_ms: Some(yield_time_ms),
            max_output_chars: None,
            lifecycle_sink: Self::start_local_lifecycle_bridge(context, self.name()),
            output_sink: output_capture_tx,
        };
        let progress_bridge = ExecOutputProgressBridge::start(context, self.name());
        let response_result = if let Some(bridge) = progress_bridge.as_ref() {
            terminal_port
                .exec_command_streaming(request, bridge.sender())
                .await
        } else {
            terminal_port.exec_command(request).await
        };
        if let Some(bridge) = progress_bridge {
            bridge.finish().await;
        }
        let response = match response_result {
            Ok(response) => response,
            Err(error) => {
                if let Some(capture_id) = context.tool_call_id.as_ref() {
                    background_command_output_capture()
                        .finish(capture_id, BackgroundCommandOutputStatus::Failed, None)
                        .await;
                }
                return Err(BitFunError::tool(format!(
                    "ExecCommand failed: {}",
                    error.message
                )));
            }
        };
        let completion = response.completion.map(exec_command_local_completion);
        if let Some(capture_id) = context.tool_call_id.as_ref() {
            if let Some(session_id) = response.session_id {
                background_command_output_capture()
                    .set_session_id(capture_id, Some(session_id))
                    .await;
            }
            if response.session_id.is_none() {
                background_command_output_capture()
                    .finish(
                        capture_id,
                        exec_command_background_output_status(completion),
                        response.exit_code,
                    )
                    .await;
            }
        }

        let data = exec_command_result_value(ExecCommandResultData {
            fields: ExecCommandResultFields {
                chunk_id: response.chunk_id,
                wall_time_seconds: response.wall_time_seconds,
                output: response.output,
                session_id: response.session_id,
                exit_code: response.exit_code,
                original_output_chars: response.original_output_chars,
                completion,
                remote: false,
            },
            workdir: workdir.to_string_lossy().to_string(),
            tty,
            shell: Self::shell_metadata_value(&shell),
        });
        let result_for_assistant = Self::response_for_assistant(&data);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::super::env_snapshot::RemoteEnvSnapshot;
    use super::ExecCommandTool;
    use super::{parse_remote_shell_probe_output, RemoteShell};
    use crate::agentic::tools::framework::{Tool, ToolUseContext};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::workspace::WorkspaceBinding;
    use crate::service::remote_ssh::workspace_state::workspace_session_identity;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use terminal_core::ShellType;
    use tool_runtime::exec_command::{
        remote_exec_shell_login_args, EXEC_COMMAND_POWERSHELL_UTF8_OUTPUT_PREFIX,
    };

    #[derive(Debug)]
    struct ShellProbeRemoteExecPort {
        response: bitfun_runtime_ports::RemoteExecOneShotCommandResponse,
    }

    impl bitfun_runtime_ports::RuntimeServicePort for ShellProbeRemoteExecPort {
        fn capability(&self) -> bitfun_runtime_ports::RuntimeServiceCapability {
            bitfun_runtime_ports::RuntimeServiceCapability::RemoteExec
        }
    }

    #[async_trait::async_trait]
    impl bitfun_runtime_ports::RemoteExecPort for ShellProbeRemoteExecPort {
        async fn exec_command_once(
            &self,
            _request: bitfun_runtime_ports::RemoteExecOneShotCommandRequest,
        ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::RemoteExecOneShotCommandResponse>
        {
            Ok(self.response.clone())
        }

        async fn exec_command(
            &self,
            _request: bitfun_runtime_ports::RemoteExecCommandRequest,
        ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::RemoteExecCommandResponse>
        {
            panic!("shell probe must not use managed remote exec sessions");
        }

        async fn exec_command_streaming(
            &self,
            _request: bitfun_runtime_ports::RemoteExecCommandRequest,
            _output_sink: bitfun_runtime_ports::RemoteExecStreamingOutputSink,
        ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::RemoteExecCommandResponse>
        {
            panic!("shell probe must not use managed remote exec sessions");
        }

        async fn write_stdin(
            &self,
            _request: bitfun_runtime_ports::RemoteWriteStdinRequest,
        ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::RemoteExecCommandResponse>
        {
            panic!("shell probe must not write stdin");
        }

        async fn write_stdin_streaming(
            &self,
            _request: bitfun_runtime_ports::RemoteWriteStdinRequest,
            _output_sink: bitfun_runtime_ports::RemoteExecStreamingOutputSink,
        ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::RemoteExecCommandResponse>
        {
            panic!("shell probe must not write stdin");
        }

        async fn send_stdin(
            &self,
            _request: bitfun_runtime_ports::RemoteSendStdinRequest,
        ) -> bitfun_runtime_ports::PortResult<()> {
            panic!("shell probe must not send stdin");
        }

        async fn control_session(
            &self,
            _request: bitfun_runtime_ports::RemoteExecControlRequest,
        ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::RemoteExecCommandResponse>
        {
            panic!("shell probe must not control managed sessions");
        }
    }

    #[test]
    fn powershell_commands_force_utf8_output() {
        let argv = ExecCommandTool::argv_for_shell(
            Path::new("pwsh"),
            &ShellType::PowerShellCore,
            "Get-Content README.md",
        );

        assert_eq!(argv[1], "-Command");
        assert!(argv[2].starts_with(EXEC_COMMAND_POWERSHELL_UTF8_OUTPUT_PREFIX));
        assert!(argv[2].contains("Get-Content README.md"));
    }

    #[test]
    fn powershell_utf8_output_prefix_is_not_duplicated() {
        let script = format!("{EXEC_COMMAND_POWERSHELL_UTF8_OUTPUT_PREFIX}Write-Output ok");
        let argv =
            ExecCommandTool::argv_for_shell(Path::new("pwsh"), &ShellType::PowerShellCore, &script);

        assert_eq!(argv[2], script);
    }

    #[test]
    fn response_notes_empty_non_tty_output_may_be_buffered() {
        let data = json!({
            "wall_time_seconds": 30.0,
            "output": "",
            "session_id": 42,
            "exit_code": null,
            "tty": false,
        });

        let assistant = ExecCommandTool::response_for_assistant(&data);

        assert!(assistant.contains("<note>"));
        assert!(assistant.contains("block-buffer pipe output"));
    }

    #[test]
    fn response_does_not_note_empty_tty_output() {
        let data = json!({
            "wall_time_seconds": 30.0,
            "output": "",
            "session_id": 42,
            "exit_code": null,
            "tty": true,
        });

        let assistant = ExecCommandTool::response_for_assistant(&data);

        assert!(!assistant.contains("<note>"));
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
    fn remote_shell_probe_preserves_unknown_shell_metadata() {
        let shell = parse_remote_shell_probe_output("\n/usr/local/bin/xonsh\n")
            .expect("shell should parse");
        let metadata = ExecCommandTool::remote_shell_metadata("/home/me/project", &shell, false);

        assert_eq!(shell.path, "/usr/local/bin/xonsh");
        assert_eq!(shell.shell_type, ShellType::Custom("xonsh".to_string()));
        assert_eq!(metadata.name, "xonsh");
        assert_eq!(metadata.kind, "xonsh");
        assert_eq!(metadata.path, "/usr/local/bin/xonsh");
    }

    #[tokio::test]
    async fn remote_shell_probe_uses_stdout_only() {
        let remote_exec_port: Arc<dyn bitfun_runtime_ports::RemoteExecPort> =
            Arc::new(ShellProbeRemoteExecPort {
                response: bitfun_runtime_ports::RemoteExecOneShotCommandResponse {
                    stdout: "/bin/bash\n".to_string(),
                    stderr: "/tmp/not-a-shell-from-stderr\n".to_string(),
                    exit_code: 0,
                    interrupted: false,
                    timed_out: false,
                },
            });

        let shell = ExecCommandTool::resolve_remote_shell(&remote_exec_port, "conn-1").await;

        assert_eq!(shell.path, "/bin/bash");
        assert_eq!(shell.shell_type, ShellType::Bash);
    }

    #[test]
    fn remote_shell_login_args_use_login_without_interactive_startup() {
        assert_eq!(remote_exec_shell_login_args(), &["-lc"]);
    }

    #[tokio::test]
    async fn description_with_context_adds_remote_note_for_remote_workspaces() {
        let tool = ExecCommandTool::new();
        let base = tool.description().await.expect("description should build");
        let session_identity =
            workspace_session_identity("/home/me/project", Some("conn-1"), Some("remote-host"))
                .expect("remote session identity should build");
        let remote_context = ToolUseContext {
            tool_call_id: None,
            agent_type: Some("agentic".to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new_remote(
                None,
                PathBuf::from("/home/me/project"),
                "conn-1".to_string(),
                "Remote Host".to_string(),
                session_identity,
            )),
            unlocked_collapsed_tools: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        };

        let remote_desc = tool
            .description_with_context(Some(&remote_context))
            .await
            .expect("contextual description should build");

        assert_ne!(base, remote_desc);
        assert!(remote_desc.contains("**Remote workspace:**"));
        assert!(remote_desc.contains("SSH server"));
        assert!(remote_desc.contains("POSIX"));
    }

    #[tokio::test]
    async fn local_exec_requires_injected_terminal_provider() {
        let tool = ExecCommandTool::new();
        let context = ToolUseContext {
            tool_call_id: None,
            agent_type: Some("agentic".to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        };
        let workdir = std::env::current_dir().expect("test workdir should exist");

        let error = tool
            .call_impl(
                &json!({
                    "cmd": "echo should-not-run",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "yield_time_ms": 0,
                }),
                &context,
            )
            .await
            .expect_err("local ExecCommand must require an injected terminal provider");

        assert!(error
            .to_string()
            .contains("terminal runtime service is required for ExecCommand"));
    }
}
