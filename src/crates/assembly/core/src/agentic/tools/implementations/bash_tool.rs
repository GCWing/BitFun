use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::workspace::WorkspaceCommandOptions;
use crate::infrastructure::events::event_system::get_global_event_system;
use crate::infrastructure::events::event_system::BackendEvent::{
    ToolExecutionProgress, ToolTerminalReady,
};
use crate::service::config::global::get_global_config_service;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::event::{ToolExecutionProgressInfo, ToolTerminalReadyInfo};
use async_trait::async_trait;
use bitfun_runtime_ports::AgentBackgroundResultRequest;
use futures::StreamExt;
use log::{debug, error, info};
use serde_json::{json, Value};
use std::path::Path;
use std::time::{Duration, Instant};
use terminal_core::session::SessionSource;
use terminal_core::shell::{ShellDetector, ShellType};
use terminal_core::{
    CommandCompletionReason, CommandStreamEvent, ExecuteCommandRequest, SignalRequest, TerminalApi,
    TerminalBindingOptions, TerminalSessionBinding,
};
use tokio::io::AsyncWriteExt;
use tool_runtime::shell::{
    banned_shell_command, bash_noninteractive_env, command_for_working_directory,
    detect_osascript_im_app, detect_osascript_keystroke_non_ascii,
    format_background_command_delivery_text, format_background_command_display_text,
    format_background_command_error_display_text, format_background_command_error_text,
    render_local_shell_result, render_remote_shell_result, BackgroundCommandDeliveryTextRequest,
    BackgroundCommandErrorTextRequest, BackgroundCommandStatusFacts, LocalShellResultRenderRequest,
    RemoteShellResultRenderRequest, BASH_INTERRUPT_OUTPUT_DRAIN_MS, BASH_RESULT_MAX_OUTPUT_LENGTH,
};

/// Result of shell resolution for bash tool
struct ResolvedShell {
    /// Shell type to use (None means use system default)
    shell_type: Option<ShellType>,
    /// Display name for the shell (for tool description)
    display_name: String,
}

fn json_object_metadata(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    }
}

struct BackgroundBashResultDelivery {
    parent_session_id: String,
    parent_agent_type: String,
    parent_workspace_path: Option<String>,
    parent_remote_connection_id: Option<String>,
    parent_remote_ssh_host: Option<String>,
    delivery_text: String,
    display_text: String,
    metadata: serde_json::Map<String, Value>,
    terminal_session_id: String,
    failure_context: &'static str,
}

async fn deliver_background_bash_result(delivery: BackgroundBashResultDelivery) {
    let BackgroundBashResultDelivery {
        parent_session_id,
        parent_agent_type,
        parent_workspace_path,
        parent_remote_connection_id,
        parent_remote_ssh_host,
        delivery_text,
        display_text,
        metadata,
        terminal_session_id,
        failure_context,
    } = delivery;
    let runtime = match CoreServiceAgentRuntime::global_agent_runtime_with_lifecycle_delivery() {
        Ok(runtime) => runtime,
        Err(error) => {
            error!(
                "Agent runtime lifecycle delivery is not available; background Bash {} dropped: session_id={}, terminal_session_id={}, error={}",
                failure_context, parent_session_id, terminal_session_id, error
            );
            return;
        }
    };

    if let Err(error) = runtime
        .deliver_background_result(AgentBackgroundResultRequest {
            session_id: parent_session_id.clone(),
            agent_type: parent_agent_type,
            workspace_path: parent_workspace_path,
            remote_connection_id: parent_remote_connection_id,
            remote_ssh_host: parent_remote_ssh_host,
            content: delivery_text,
            display_content: Some(display_text),
            metadata,
        })
        .await
    {
        error!(
            "Failed to deliver background Bash {}: session_id={}, terminal_session_id={}, error={}",
            failure_context,
            parent_session_id,
            terminal_session_id,
            CoreServiceAgentRuntime::runtime_error_message(error)
        );
    }
}

/// Bash tool
pub struct BashTool;

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BashTool {
    pub fn new() -> Self {
        Self
    }

    fn resolve_working_directory(
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Option<String>> {
        let Some(raw_dir) = input.get("working_directory").and_then(|v| v.as_str()) else {
            return Ok(None);
        };
        let trimmed = raw_dir.trim();
        if trimmed.is_empty() {
            return Ok(context.workspace.as_ref().map(|w| w.root_path_string()));
        }
        context.resolve_workspace_tool_path(trimmed).map(Some)
    }

    async fn is_existing_workspace_directory(
        context: &ToolUseContext,
        resolved_dir: &str,
    ) -> BitFunResult<bool> {
        if context.is_remote() {
            let fs = context.ws_fs().ok_or_else(|| {
                BitFunError::tool(
                    "Remote workspace filesystem is unavailable; cannot validate working_directory"
                        .to_string(),
                )
            })?;
            fs.is_dir(resolved_dir).await.map_err(|e| {
                BitFunError::tool(format!("Failed to validate working_directory: {e}"))
            })
        } else {
            Ok(Path::new(resolved_dir).is_dir())
        }
    }

    /// Build environment variables that suppress interactive behaviors
    /// (pagers, editors, prompts) so agent-driven commands never block.
    pub fn noninteractive_env() -> std::collections::HashMap<String, String> {
        bash_noninteractive_env()
    }

    /// Resolve shell configuration for bash tool.
    /// If configured shell doesn't support integration, falls back to system default.
    async fn resolve_shell() -> ResolvedShell {
        // Try configured shell first, fall back to system default
        Self::try_configured_shell()
            .await
            .unwrap_or_else(Self::system_default_shell)
    }

    /// Try to get a valid configured shell that supports integration.
    async fn try_configured_shell() -> Option<ResolvedShell> {
        let config_service = get_global_config_service().await.ok()?;
        let shell_str: String = config_service
            .get_config::<String>(Some("terminal.default_shell"))
            .await
            .ok()
            .filter(|s| !s.is_empty())?;

        let parsed = ShellType::from_executable(&shell_str);
        if parsed.supports_integration() {
            Some(ResolvedShell {
                shell_type: Some(parsed.clone()),
                display_name: parsed.name().to_string(),
            })
        } else {
            debug!(
                "Configured shell '{}' does not support integration, using system default",
                shell_str
            );
            None
        }
    }

    /// Get system default shell configuration.
    fn system_default_shell() -> ResolvedShell {
        let detected = ShellDetector::get_default_shell();
        ResolvedShell {
            shell_type: None,
            display_name: detected.display_name,
        }
    }

    fn emit_terminal_ready_event(tool_use_id: &str, terminal_session_id: &str) {
        let event = ToolTerminalReady(ToolTerminalReadyInfo {
            tool_use_id: tool_use_id.to_string(),
            terminal_session_id: terminal_session_id.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        let event_system = get_global_event_system();
        tokio::spawn(async move {
            let _ = event_system.emit(event).await;
        });
    }

    fn cancellation_requested(context: &ToolUseContext) -> bool {
        context
            .cancellation_token()
            .is_some_and(|token| token.is_cancelled())
    }

    fn cancellation_error(stage: &str) -> BitFunError {
        BitFunError::cancelled(format!("Bash tool execution cancelled {}", stage))
    }

    fn background_output_file_reference(
        context: &ToolUseContext,
        chat_session_id: &str,
        tool_use_id: &str,
        output_file_path: &Path,
    ) -> String {
        context
            .build_session_runtime_artifact_reference(
                chat_session_id,
                &format!("tool-results/{}.txt", tool_use_id),
            )
            .unwrap_or_else(|_| output_file_path.display().to_string())
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    async fn description(&self) -> BitFunResult<String> {
        let shell_info = Self::resolve_shell().await.display_name;

        Ok(format!(
            r#"Executes a given command in a persistent shell session with optional timeout, ensuring proper handling and security measures.

Shell Environment: {shell_info}

IMPORTANT: This tool is for terminal operations like git, npm, docker, etc. DO NOT use it for file operations (reading, writing, editing, searching, finding files) - use the specialized tools for this instead.

Before executing the command, please follow these steps:

1. Directory Verification:
   - If the command will create new directories or files, first use `ls` to verify the parent directory exists and is the correct location
   - For example, before running "mkdir foo/bar", first use `ls foo` to check that "foo" exists and is the intended parent directory

2. Command Execution:
   - Always quote file paths that contain spaces with double quotes (e.g., cd "path with spaces/file.txt")
   - Examples of proper quoting:
     - cd "My Documents" (correct)
     - cd My Documents (incorrect - will fail)
     - python "scripts/with spaces/script.py" (correct)
     - python scripts/with spaces/script.py (incorrect - will fail)
   - After ensuring proper quoting, execute the command.
   - Capture the output of the command.

Usage notes:
  - The command argument is required and MUST be a single-line command.
  - DO NOT use multiline commands or HEREDOC syntax (e.g., <<EOF, heredoc with newlines). Only single-line commands are supported.
  - You can specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). If not specified, commands will timeout after 120000ms (2 minutes).
  - It is very helpful if you write a clear, concise description of what this command does. For simple commands, keep it brief (5-10 words). For complex commands (piped commands, obscure flags, or anything hard to understand at a glance), add enough context to clarify what it does.
  - If the output exceeds {BASH_RESULT_MAX_OUTPUT_LENGTH} characters, output will be truncated before being returned to you, with the tail of the output preserved because the ending is usually more important.
  - You can use the `run_in_background` parameter to run the command in a new dedicated background terminal session. The tool returns immediately without waiting for the command to finish. The final completion result will be delivered back to you automatically when it is done, and the full output will be saved to a session runtime file instead of being pasted back into chat. Only use this for long-running processes (e.g., dev servers, watchers) where you do not need the output right away. You do not need to append '&' to the command. NOTE: `timeout_ms` is ignored when `run_in_background` is true.
  - Each result includes a `<terminal_session_id>` tag identifying the terminal session. The persistent shell session ID remains constant throughout the entire conversation; background sessions each have their own unique ID.
  - The output may include the command echo and/or the shell prompt prefix (for example, a printed `PS` or `$` prompt line). Do not treat these as part of the command's actual result.
  - Avoid interactive commands that may block waiting for user input or open a pager/editor. Prefer non-interactive variants and explicit flags. For example, use `git --no-pager diff` instead of `git diff`, and avoid commands that prompt for confirmation unless the User explicitly asks for them.
  
  - Prefer specialized tools for workspace file operations: Glob for file discovery, Grep for content search, Read for reading, Edit for modifying, Write for creating, and Delete for deletion. Prefer the Git tool for Git subcommands such as status, diff, log, add, commit, branch, checkout, pull, and push. When Git appears in the Deferred Tool Listing, load its schema with GetToolSpec and execute it through CallDeferredTool; otherwise call Git directly. Use Bash for commands that genuinely need a shell, such as build/test/package CLIs, process control, scripts, and environment checks. Never use shell output only to communicate with the user.
  - When issuing multiple commands:
    - If the commands are independent and can run in parallel, make multiple tool calls in a single message. For Git inspection, prefer parallel Git tool calls such as `{{"operation":"status"}}` and `{{"operation":"diff","args":"--stat"}}` instead of Bash.
    - If the commands depend on each other and must run sequentially, use a single Bash call with '&&' to chain them together (e.g., `git add . && git commit -m "message" && git push`). For instance, if one operation must complete before another starts (like mkdir before cp, Write before Bash for git operations, or git add before git commit), run these operations sequentially instead.
    - Use ';' only when you need to run commands sequentially but don't care if earlier commands fail
    - DO NOT use newlines to separate commands (newlines are ok in quoted strings)
  - Try to maintain your current working directory throughout the session by using absolute paths and avoiding usage of `cd`. You may use `cd` if the User explicitly requests it.
    <good-example>
    pytest /foo/bar/tests
    </good-example>
    <bad-example>
    cd /foo/bar && pytest tests
    </bad-example>"#
        ))
    }

    fn short_description(&self) -> String {
        "Run commands in the persistent shell session.".to_string()
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        let mut base = self.description().await?;
        if context.map(|c| c.is_remote()).unwrap_or(false) {
            base = format!(
                r#"**Remote workspace:** Commands run on the **SSH server** in a shell whose initial working directory is the **remote workspace root** (same as running a terminal on that machine). The shell name shown below may reflect your **local** BitFun settings; the actual interpreter on the server is typically `sh`/`bash`. Use **Unix** syntax and POSIX paths — not PowerShell or Windows paths.

{base}"#,
                base = base
            );
        }
        if !context.map(|c| c.is_remote()).unwrap_or(false) {
            base.push_str(
                "\n\n**Desktop automation:** Prefer this tool for actions achievable from the **workspace shell** (build, test, git, scripts, CLIs). On **macOS**, `open -a \"AppName\"` can launch or foreground an app. Use the dedicated `ComputerUse` tool or agent for desktop UI perception/control such as screenshots, OCR, mouse, keyboard, app state, clipboard, and OS-level interactions.",
            );
        }
        Ok(base)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout_ms": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (default 120000, max 600000). Ignored when run_in_background is true."
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "If true, runs the command in a new dedicated background terminal session and returns immediately. The final completion result is delivered back automatically when the command finishes, and the full output is saved to a session runtime file instead of being injected into chat. Useful for long-running processes like dev servers or file watchers. timeout_ms is ignored when this is true."
                },
                "working_directory": {
                    "type": "string",
                    "description": "Optional directory to run the command in. Use a workspace-relative path or an absolute path inside the current workspace. Omit to reuse the persistent terminal's current directory."
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does in 5-10 words, in active voice. Examples:\nInput: ls\nOutput: List files in current directory\n\nInput: git status\nOutput: Show working tree status\n\nInput: npm install\nOutput: Install package dependencies\n\nInput: mkdir foo\nOutput: Create directory 'foo'"
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let command = input
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .ok_or_else(|| BitFunError::validation("command is required".to_string()))?;
        Ok(vec![PermissionIntent::new(
            "bash",
            vec![command.to_string()],
        )])
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let command = input.get("command").and_then(|v| v.as_str());
        let run_in_background = input
            .get("run_in_background")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if let Some(cmd) = command {
            if let Some(base_cmd) = banned_shell_command(cmd) {
                return ValidationResult {
                    result: false,
                    message: Some(format!(
                        "Command '{}' is not allowed for security reasons",
                        base_cmd
                    )),
                    error_code: Some(403),
                    meta: None,
                };
            }

            // Reject `osascript ... keystroke "<non-ASCII>"` — fundamentally
            // broken: AppleScript's `keystroke` sends raw key codes, not
            // Unicode, so CJK / emoji becomes garbage like "AAA…" in the
            // target app. This is exactly the WeChat-search-box failure
            // mode users keep hitting. Redirect to the canonical path.
            if let Some(literal) = detect_osascript_keystroke_non_ascii(cmd) {
                let preview: String = literal.chars().take(40).collect();
                return ValidationResult {
                    result: false,
                    message: Some(format!(
                        "Refused: `osascript ... keystroke \"{}…\"` cannot type non-ASCII text — \
                         AppleScript's `keystroke` sends raw key codes, not Unicode, so CJK / \
                         emoji / accented text comes out as garbage in the target app (e.g. \
                         the WeChat search box receives `AAA…` instead of `{}`). \n\n\
                         Use ControlHub instead:\n\
                         1. `system.open_app {{ app_name: \"<App>\" }}` to focus the app\n\
                         2. (optional) `desktop.key_chord {{ keys: [\"command\",\"f\"] }}` to focus search\n\
                         3. `desktop.paste {{ text: \"<your text>\", submit: true }}` — pastes via \
                            system clipboard, works for ANY language.\n\n\
                         For sending an IM message specifically, run the `im_send_message` \
                         playbook — it's the same 3-step flow pre-packaged.",
                        preview, preview
                    )),
                    error_code: Some(400),
                    meta: None,
                };
            }

            // Soft-block `osascript` driving chat / IM apps. These flows are
            // a constant source of frustration: no return value to verify,
            // brittle UI scripting, no CJK support via keystroke, and the
            // alternative (`system.open_app` + `desktop.paste` /
            // `im_send_message` playbook) is faster AND more reliable.
            if let Some(app) = detect_osascript_im_app(cmd) {
                return ValidationResult {
                    result: false,
                    message: Some(format!(
                        "Refused: driving {app} via `osascript` / AppleScript GUI scripting is unreliable \
                         (no CJK support in keystroke, no return value, easy to deadlock). \n\n\
                         Use the canonical IM-send recipe instead — same 3 deterministic calls:\n\
                         1. `ControlHub domain:\"system\" action:\"open_app\" {{ app_name:\"{app}\" }}`\n\
                         2. `ControlHub domain:\"desktop\" action:\"key_chord\" {{ keys:[\"command\",\"f\"] }}`\n\
                         3. `ControlHub domain:\"desktop\" action:\"paste\" {{ text:\"<contact>\", submit:true }}`\n\
                         4. `ControlHub domain:\"desktop\" action:\"paste\" {{ text:\"<message>\", submit:true }}`\n\n\
                         Or run the prepackaged `im_send_message` playbook with \
                         `{{ app_name, contact, message }}`. For Slack/Lark where Return inserts \
                         a newline, pass `submit_keys:[\"command\",\"return\"]`."
                    )),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("command is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        let Some(context) = context else {
            return ValidationResult {
                result: false,
                message: Some("tool context is required for Bash tool".to_string()),
                error_code: Some(400),
                meta: None,
            };
        };

        if context.session_id.as_deref().unwrap_or_default().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("session_id is required for Bash tool".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if context.workspace_root().is_none() {
            return ValidationResult {
                result: false,
                message: Some("workspace_path is required for Bash tool".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        match Self::resolve_working_directory(input, context) {
            Ok(Some(resolved_dir)) => {
                match Self::is_existing_workspace_directory(context, &resolved_dir).await {
                    Ok(true) => {}
                    Ok(false) => {
                        return ValidationResult {
                            result: false,
                            message: Some(format!(
                                "working_directory must be an existing directory inside the current workspace: {}",
                                resolved_dir
                            )),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                    Err(err) => {
                        return ValidationResult {
                            result: false,
                            message: Some(err.to_string()),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                }
            }
            Ok(None) => {}
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        // Warn if timeout_ms is set alongside run_in_background
        if run_in_background && input.get("timeout_ms").is_some() {
            return ValidationResult {
                result: true,
                message: Some(
                    "Note: timeout_ms is ignored when run_in_background is true".to_string(),
                ),
                error_code: None,
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

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
            // Clean up any command that uses the quoted HEREDOC pattern
            if command.contains("\"$(cat <<'EOF'") {
                // Simple regex-like parsing for HEREDOC
                if let Some(start) = command.find("\"$(cat <<'EOF'\n") {
                    if let Some(end) = command.find("\nEOF\n)") {
                        let prefix = &command[..start];
                        let content_start = start + "\"$(cat <<'EOF'\n".len();
                        let content = &command[content_start..end];
                        return format!("{} \"{}\"", prefix.trim(), content.trim());
                    }
                }
            }
            command.to_string()
        } else {
            "Executing command".to_string()
        }
    }

    async fn call_impl(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        Err(BitFunError::tool(
            "Bash tool call_impl should not be called".to_string(),
        ))
    }

    async fn call(&self, input: &Value, context: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        let start_time = Instant::now();

        // Get command parameter
        let command_str = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("command is required".to_string()))?;
        let requested_working_directory = Self::resolve_working_directory(input, context)?;

        if command_needs_light_checkpoint(command_str) {
            context
                .record_light_checkpoint("Bash", command_str, Vec::new())
                .await;
        }

        // Remote workspace: execute via injected workspace shell
        if context.is_remote() {
            let Some(ws_shell) = context.ws_shell() else {
                return Err(BitFunError::tool(
                    "Remote workspace shell is unavailable; refusing to run Bash locally for a remote session.".to_string(),
                ));
            };

            info!(
                "Executing command on remote workspace via SSH: {}",
                command_str
            );
            let remote_command =
                command_for_working_directory(command_str, requested_working_directory.as_deref());

            let timeout_ms = input
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(120_000);

            let exec_result = ws_shell
                .exec_with_options(
                    &remote_command,
                    WorkspaceCommandOptions {
                        timeout_ms: Some(timeout_ms),
                        cancellation_token: context.cancellation_token().cloned(),
                    },
                )
                .await
                .map_err(|e| {
                    BitFunError::tool(format!("Remote command execution failed: {}", e))
                })?;

            let output = exec_result.combined_output();

            let execution_time_ms = elapsed_ms_u64(start_time);
            let working_directory = context
                .workspace_root()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let working_directory = requested_working_directory.unwrap_or(working_directory);
            let result_for_assistant = render_remote_shell_result(RemoteShellResultRenderRequest {
                working_directory: &working_directory,
                stdout: &exec_result.stdout,
                stderr: &exec_result.stderr,
                interrupted: exec_result.interrupted,
                timed_out: exec_result.timed_out,
                exit_code: exec_result.exit_code,
            });

            let result = ToolResult::Result {
                data: json!({
                    "success": exec_result.exit_code == 0,
                    "command": command_str,
                    "stdout": exec_result.stdout,
                    "stderr": exec_result.stderr,
                    "output": output,
                    "exit_code": exec_result.exit_code,
                    "interrupted": exec_result.interrupted,
                    "timed_out": exec_result.timed_out,
                    "working_directory": working_directory,
                    "execution_time_ms": execution_time_ms,
                    "duration_ms": execution_time_ms,
                    "is_remote": true
                }),
                result_for_assistant: Some(result_for_assistant),
                image_attachments: None,
            };
            return Ok(vec![result]);
        }

        let run_in_background = input
            .get("run_in_background")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get session_id (for binding terminal session)
        let chat_session_id = context
            .session_id
            .as_ref()
            .ok_or_else(|| BitFunError::tool("session_id is required for Bash tool".to_string()))?;

        // Get tool call ID (for sending progress events)
        let tool_use_id = context
            .tool_call_id
            .clone()
            .unwrap_or_else(|| format!("bash_{}", uuid::Uuid::new_v4()));

        // 1. Get Terminal API
        let terminal_api = TerminalApi::from_singleton()
            .map_err(|e| BitFunError::tool(format!("Terminal not initialized: {}", e)))?;

        // 2. Resolve shell type
        let shell_type = Self::resolve_shell().await.shell_type;

        let binding = terminal_api.session_manager().binding();
        let workspace_path = context
            .workspace_root()
            .ok_or_else(|| {
                BitFunError::tool("workspace_path is required for Bash tool".to_string())
            })?
            .to_string_lossy()
            .to_string();

        if run_in_background {
            if Self::cancellation_requested(context) {
                return Err(Self::cancellation_error(
                    "before creating background session",
                ));
            }

            // For background commands, inherit CWD from an already-running primary session
            // if one exists; otherwise fall back to workspace path.  This avoids forcing a
            // primary session to be created just to read its working directory.
            let initial_cwd = if let Some(requested_dir) = requested_working_directory.as_ref() {
                requested_dir.clone()
            } else if let Some(existing_id) = binding.get(chat_session_id) {
                terminal_api
                    .get_session(&existing_id)
                    .await
                    .map(|s| s.cwd)
                    .unwrap_or_else(|_| workspace_path.clone())
            } else {
                workspace_path.clone()
            };

            return self
                .call_background(
                    command_str,
                    chat_session_id,
                    &initial_cwd,
                    context,
                    shell_type,
                    &binding,
                    start_time,
                )
                .await;
        }

        // 3. Foreground: get or create the primary terminal session
        let terminal_ready_started_at = Instant::now();
        let primary_session_id = binding
            .get_or_create(
                chat_session_id,
                TerminalBindingOptions {
                    working_directory: Some(workspace_path.clone()),
                    session_id: Some(chat_session_id.to_string()),
                    session_name: Some(format!(
                        "Chat-{}",
                        &chat_session_id[..8.min(chat_session_id.len())]
                    )),
                    shell_type: shell_type.clone(),
                    env: Some(Self::noninteractive_env()),
                    source: Some(SessionSource::Agent),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to create Terminal session: {}", e)))?;
        let terminal_ready_ms = elapsed_ms_u64(terminal_ready_started_at);

        Self::emit_terminal_ready_event(&tool_use_id, &primary_session_id);

        // Get actual working directory from primary session
        let primary_cwd = terminal_api
            .get_session(&primary_session_id)
            .await
            .map(|s| s.cwd)
            .unwrap_or_else(|_| workspace_path.clone());
        let execution_working_directory = requested_working_directory
            .as_ref()
            .cloned()
            .unwrap_or_else(|| primary_cwd.clone());
        let command_to_execute =
            command_for_working_directory(command_str, requested_working_directory.as_deref());

        // --- Foreground execution ---

        let tool_name = self.name().to_string();

        const DEFAULT_TIMEOUT_MS: u64 = 120_000;
        const MAX_TIMEOUT_MS: u64 = 600_000;
        let timeout_ms = Some(
            input
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(DEFAULT_TIMEOUT_MS)
                .min(MAX_TIMEOUT_MS),
        );

        debug!(
            "Bash tool executing command: {}, session_id: {}, tool_id: {}",
            command_to_execute, chat_session_id, tool_use_id
        );

        // 4. Create streaming execution request
        let request = ExecuteCommandRequest {
            session_id: primary_session_id.clone(),
            command: command_to_execute,
            timeout_ms,
            prevent_history: Some(true),
        };

        // 5. Execute command and handle streaming output
        let mut stream = terminal_api.execute_command_stream(request);
        let mut accumulated_output = String::new();
        let mut final_exit_code: Option<i32> = None;
        let mut was_interrupted = false;
        let mut timed_out = false;
        let mut final_shell_state: Option<String> = None;
        let mut command_started_after_ms: Option<u64> = None;
        let mut completion_reason_label = "stream_end".to_string();
        let mut interrupt_drain_deadline: Option<tokio::time::Instant> = None;
        let command_stream_started_at = Instant::now();

        // Get event system for sending progress
        let event_system = get_global_event_system();

        loop {
            let next_event = if let Some(deadline) = interrupt_drain_deadline {
                let now = tokio::time::Instant::now();
                if now >= deadline {
                    break;
                }

                match tokio::time::timeout_at(deadline, stream.next()).await {
                    Ok(event) => event,
                    Err(_) => break,
                }
            } else {
                stream.next().await
            };

            let Some(event) = next_event else {
                break;
            };

            // Check cancellation request
            if let Some(token) = context.cancellation_token() {
                if token.is_cancelled() && !was_interrupted {
                    debug!("Bash tool received cancellation request, sending interrupt signal, tool_id: {}", tool_use_id);
                    was_interrupted = true;
                    interrupt_drain_deadline = Some(
                        tokio::time::Instant::now()
                            + Duration::from_millis(BASH_INTERRUPT_OUTPUT_DRAIN_MS),
                    );

                    let _ = terminal_api
                        .signal(SignalRequest {
                            session_id: primary_session_id.clone(),
                            signal: "SIGINT".to_string(),
                        })
                        .await;

                    #[cfg(windows)]
                    {
                        final_exit_code = Some(-1073741510);
                    }
                    #[cfg(not(windows))]
                    {
                        final_exit_code = Some(130);
                    }
                }
            }

            match event {
                CommandStreamEvent::Started { command_id } => {
                    command_started_after_ms = Some(elapsed_ms_u64(command_stream_started_at));
                    debug!("Bash command started execution, command_id: {}", command_id);
                }
                CommandStreamEvent::Output { data } => {
                    accumulated_output.push_str(&data);

                    let progress_event = ToolExecutionProgress(ToolExecutionProgressInfo {
                        tool_use_id: tool_use_id.clone(),
                        tool_name: tool_name.clone(),
                        progress_message: data,
                        percentage: None,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    });

                    let event_system_clone = event_system.clone();
                    tokio::spawn(async move {
                        let _ = event_system_clone.emit(progress_event).await;
                    });
                }
                CommandStreamEvent::Completed {
                    exit_code,
                    total_output,
                    completion_reason,
                    shell_state,
                } => {
                    debug!(
                        "Bash command completed, exit_code: {:?}, tool_id: {}",
                        exit_code, tool_use_id
                    );
                    final_exit_code = exit_code.or(final_exit_code);
                    timed_out = completion_reason == CommandCompletionReason::TimedOut;
                    completion_reason_label = format!("{:?}", completion_reason);

                    if !timed_out && matches!(exit_code, Some(130) | Some(-1073741510)) {
                        was_interrupted = true;
                    }

                    if !total_output.is_empty() {
                        accumulated_output = total_output;
                    }

                    // Capture post-command terminal state for the AI agent
                    if shell_state.is_some() {
                        final_shell_state = shell_state;
                    }
                    break;
                }
                CommandStreamEvent::Error { message } => {
                    error!(
                        "Bash command execution error: {}, tool_id: {}",
                        message, tool_use_id
                    );
                    return Err(BitFunError::tool(format!(
                        "Command execution error: {}",
                        message
                    )));
                }
            }
        }

        // 6. Build result
        let execution_time_ms = elapsed_ms_u64(start_time);
        let command_stream_ms = elapsed_ms_u64(command_stream_started_at);
        info!(
            "Bash command completed: tool_id={}, terminal_session_id={}, duration_ms={}, terminal_ready_ms={}, command_started_after_ms={:?}, command_stream_ms={}, output_bytes={}, exit_code={:?}, interrupted={}, timed_out={}, completion_reason={}",
            tool_use_id,
            primary_session_id,
            execution_time_ms,
            terminal_ready_ms,
            command_started_after_ms,
            command_stream_ms,
            accumulated_output.len(),
            final_exit_code,
            was_interrupted,
            timed_out,
            completion_reason_label
        );

        let result_data = json!({
            "success": final_exit_code.unwrap_or(-1) == 0,
            "command": command_str,
            "output": accumulated_output,
            "exit_code": final_exit_code,
            "interrupted": was_interrupted,
            "timed_out": timed_out,
            "working_directory": execution_working_directory,
            "execution_time_ms": execution_time_ms,
            "terminal_session_id": primary_session_id,
        });

        let result_for_assistant = render_local_shell_result(LocalShellResultRenderRequest {
            terminal_session_id: &primary_session_id,
            working_directory: &execution_working_directory,
            output_text: &accumulated_output,
            interrupted: was_interrupted,
            timed_out,
            exit_code: final_exit_code.unwrap_or(-1),
            shell_state: final_shell_state.as_deref(),
        });

        Ok(vec![ToolResult::Result {
            data: result_data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

fn command_needs_light_checkpoint(command: &str) -> bool {
    let command = command.trim().to_ascii_lowercase();
    let mutating_prefixes = [
        "rm ",
        "rmdir ",
        "del ",
        "erase ",
        "move ",
        "mv ",
        "cp ",
        "git reset",
        "git clean",
        "git checkout",
        "git switch",
        "git merge",
        "git rebase",
        "git pull",
        "git stash",
        "git commit",
        "cargo fmt",
        "cargo fix",
        "rustfmt",
        "prettier --write",
    ];

    mutating_prefixes
        .iter()
        .any(|prefix| command.starts_with(prefix))
        || command.contains(" --fix")
        || command.contains(" > ")
        || command.contains(" >> ")
}

impl BashTool {
    fn background_output_file_path(
        context: &ToolUseContext,
        chat_session_id: &str,
        tool_use_id: &str,
    ) -> Option<std::path::PathBuf> {
        context
            .current_workspace_session_tool_result_path(
                chat_session_id,
                &format!("{}.txt", tool_use_id),
            )
            .ok()
    }

    /// Execute a command in a new background terminal session.
    /// Returns immediately with the new session ID.
    #[allow(clippy::too_many_arguments)]
    async fn call_background(
        &self,
        command_str: &str,
        chat_session_id: &str,
        initial_cwd: &str,
        context: &ToolUseContext,
        shell_type: Option<ShellType>,
        binding: &TerminalSessionBinding,
        start_time: Instant,
    ) -> BitFunResult<Vec<ToolResult>> {
        debug!(
            "Bash tool starting background command: {}, owner: {}",
            command_str, chat_session_id
        );

        if Self::cancellation_requested(context) {
            return Err(Self::cancellation_error(
                "before creating background terminal",
            ));
        }

        // Create a dedicated background terminal session sharing the primary session's cwd
        let bg_session_id = binding
            .create_background_session(
                chat_session_id,
                TerminalBindingOptions {
                    working_directory: Some(initial_cwd.to_string()),
                    session_id: None,
                    session_name: None,
                    shell_type,
                    env: Some(Self::noninteractive_env()),
                    source: Some(SessionSource::Agent),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| {
                BitFunError::tool(format!(
                    "Failed to create background terminal session: {}",
                    e
                ))
            })?;

        let tool_use_id = context
            .tool_call_id
            .clone()
            .unwrap_or_else(|| format!("bash_{}", uuid::Uuid::new_v4()));
        Self::emit_terminal_ready_event(&tool_use_id, &bg_session_id);

        if Self::cancellation_requested(context) {
            let terminal_api = TerminalApi::from_singleton()
                .map_err(|e| BitFunError::tool(format!("Terminal not initialized: {}", e)))?;
            let _ = terminal_api
                .close_session(terminal_core::CloseSessionRequest {
                    session_id: bg_session_id.clone(),
                    immediate: Some(true),
                })
                .await;
            return Err(Self::cancellation_error(
                "before sending background command",
            ));
        }

        // Store background output under the session-scoped runtime tool-results tree:
        // local:  ~/.bitfun/projects/<project-slug>/sessions/<chat-session-id>/tool-results/<tool-use-id>.txt
        // remote: ~/.bitfun/remote_ssh/<host>/<remote-path>/sessions/<chat-session-id>/tool-results/<tool-use-id>.txt
        let output_file_path =
            Self::background_output_file_path(context, chat_session_id, &tool_use_id).ok_or_else(
                || {
                    BitFunError::tool(
                        "Failed to prepare a background output file for Bash tool".to_string(),
                    )
                },
            )?;
        if let Some(parent) = output_file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                BitFunError::tool(format!(
                    "Failed to create background output directory: {}",
                    e
                ))
            })?;
        }
        let output_file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file_path)
            .await
            .map_err(|e| {
                BitFunError::tool(format!("Failed to open background output file: {}", e))
            })?;
        let output_file_reference = Self::background_output_file_reference(
            context,
            chat_session_id,
            &tool_use_id,
            &output_file_path,
        );

        debug!(
            "Background command started, session_id: {}, owner: {}",
            bg_session_id, chat_session_id
        );

        let parent_session_id = chat_session_id.to_string();
        let parent_agent_type = context
            .agent_type
            .clone()
            .unwrap_or_else(|| "Agentic".to_string());
        let parent_workspace_path = context
            .workspace_root()
            .map(|path| path.to_string_lossy().to_string());
        let parent_remote_connection_id = context
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.connection_id().map(ToOwned::to_owned));
        let parent_remote_ssh_host = context
            .workspace
            .as_ref()
            .filter(|workspace| workspace.is_remote())
            .map(|workspace| workspace.session_identity.hostname.clone())
            .filter(|value| !value.trim().is_empty());
        let command = command_str.to_string();
        let working_directory = initial_cwd.to_string();
        let terminal_session_id = bg_session_id.clone();
        let output_file_reference_for_task = output_file_reference.clone();
        let tool_use_id_for_task = tool_use_id.clone();

        tokio::spawn(async move {
            let mut writer = tokio::io::BufWriter::new(output_file);
            let mut output_persist_error: Option<String> = None;
            let mut saw_output_event = false;
            let mut saw_completion = false;
            let mut delivery_sent = false;

            let terminal_api = match TerminalApi::from_singleton() {
                Ok(api) => api,
                Err(error) => {
                    error!(
                        "Background Bash command could not access terminal singleton: session_id={}, error={}",
                        terminal_session_id, error
                    );
                    return;
                }
            };

            let mut stream = terminal_api.execute_command_stream(ExecuteCommandRequest {
                session_id: terminal_session_id.clone(),
                command: command.clone(),
                timeout_ms: None,
                prevent_history: Some(true),
            });

            while let Some(event) = stream.next().await {
                match event {
                    CommandStreamEvent::Started { command_id } => {
                        debug!(
                            "Background Bash command started execution, session_id={}, command_id={}",
                            terminal_session_id, command_id
                        );
                    }
                    CommandStreamEvent::Output { data } => {
                        saw_output_event = true;
                        if output_persist_error.is_none() {
                            if let Err(error) = writer.write_all(data.as_bytes()).await {
                                output_persist_error = Some(error.to_string());
                                error!(
                                    "Failed to write background Bash output: session_id={}, error={}",
                                    terminal_session_id, error
                                );
                            } else if let Err(error) = writer.flush().await {
                                output_persist_error = Some(error.to_string());
                                error!(
                                    "Failed to flush background Bash output: session_id={}, error={}",
                                    terminal_session_id, error
                                );
                            }
                        }
                    }
                    CommandStreamEvent::Completed {
                        exit_code,
                        total_output,
                        completion_reason,
                        shell_state: _,
                    } => {
                        saw_completion = true;

                        if !saw_output_event
                            && !total_output.is_empty()
                            && output_persist_error.is_none()
                        {
                            if let Err(error) = writer.write_all(total_output.as_bytes()).await {
                                output_persist_error = Some(error.to_string());
                                error!(
                                    "Failed to persist background Bash completion output: session_id={}, error={}",
                                    terminal_session_id, error
                                );
                            } else if let Err(error) = writer.flush().await {
                                output_persist_error = Some(error.to_string());
                                error!(
                                    "Failed to flush background Bash completion output: session_id={}, error={}",
                                    terminal_session_id, error
                                );
                            }
                        }

                        let timed_out = completion_reason == CommandCompletionReason::TimedOut;
                        let interrupted =
                            !timed_out && matches!(exit_code, Some(130) | Some(-1073741510));
                        let status = BackgroundCommandStatusFacts {
                            exit_code,
                            timed_out,
                            interrupted,
                        };
                        let delivery_text = format_background_command_delivery_text(
                            BackgroundCommandDeliveryTextRequest {
                                command: &command,
                                terminal_session_id: &terminal_session_id,
                                working_directory: &working_directory,
                                status,
                                output_file_reference: &output_file_reference_for_task,
                                output_persist_error: output_persist_error.as_deref(),
                            },
                        );
                        let display_text =
                            format_background_command_display_text(BackgroundCommandStatusFacts {
                                exit_code,
                                timed_out,
                                interrupted,
                            });
                        let metadata = json!({
                            "kind": "background_result",
                            "sourceKind": "bash_command",
                            "toolName": "Bash",
                            "toolCallId": tool_use_id_for_task.clone(),
                            "terminalSessionId": terminal_session_id.clone(),
                            "command": command.clone(),
                            "workingDirectory": working_directory.clone(),
                            "outputFile": output_file_reference_for_task.clone(),
                        });

                        deliver_background_bash_result(BackgroundBashResultDelivery {
                            parent_session_id: parent_session_id.clone(),
                            parent_agent_type: parent_agent_type.clone(),
                            parent_workspace_path: parent_workspace_path.clone(),
                            parent_remote_connection_id: parent_remote_connection_id.clone(),
                            parent_remote_ssh_host: parent_remote_ssh_host.clone(),
                            delivery_text,
                            display_text,
                            metadata: json_object_metadata(metadata),
                            terminal_session_id: terminal_session_id.clone(),
                            failure_context: "result",
                        })
                        .await;
                        delivery_sent = true;
                        break;
                    }
                    CommandStreamEvent::Error { message } => {
                        let delivery_text = format_background_command_error_text(
                            BackgroundCommandErrorTextRequest {
                                command: &command,
                                terminal_session_id: &terminal_session_id,
                                working_directory: &working_directory,
                                output_file_reference: &output_file_reference_for_task,
                                error: &message,
                                output_persist_error: output_persist_error.as_deref(),
                            },
                        );
                        let display_text = format_background_command_error_display_text();
                        let metadata = json!({
                            "kind": "background_result",
                            "sourceKind": "bash_command",
                            "toolName": "Bash",
                            "toolCallId": tool_use_id_for_task.clone(),
                            "terminalSessionId": terminal_session_id.clone(),
                            "command": command.clone(),
                            "workingDirectory": working_directory.clone(),
                            "outputFile": output_file_reference_for_task.clone(),
                            "error": message.clone(),
                        });

                        deliver_background_bash_result(BackgroundBashResultDelivery {
                            parent_session_id: parent_session_id.clone(),
                            parent_agent_type: parent_agent_type.clone(),
                            parent_workspace_path: parent_workspace_path.clone(),
                            parent_remote_connection_id: parent_remote_connection_id.clone(),
                            parent_remote_ssh_host: parent_remote_ssh_host.clone(),
                            delivery_text,
                            display_text,
                            metadata: json_object_metadata(metadata),
                            terminal_session_id: terminal_session_id.clone(),
                            failure_context: "error result",
                        })
                        .await;
                        delivery_sent = true;
                        break;
                    }
                }
            }

            if !saw_completion && !delivery_sent {
                let delivery_text =
                    format_background_command_error_text(BackgroundCommandErrorTextRequest {
                        command: &command,
                        terminal_session_id: &terminal_session_id,
                        working_directory: &working_directory,
                        output_file_reference: &output_file_reference_for_task,
                        error: "Background Bash command stream ended without a completion event.",
                        output_persist_error: output_persist_error.as_deref(),
                    });
                let display_text = format_background_command_error_display_text();
                let metadata = json!({
                    "kind": "background_result",
                    "sourceKind": "bash_command",
                    "toolName": "Bash",
                    "toolCallId": tool_use_id_for_task,
                    "terminalSessionId": terminal_session_id.clone(),
                    "command": command.clone(),
                    "workingDirectory": working_directory.clone(),
                    "outputFile": output_file_reference_for_task.clone(),
                    "error": "stream_ended_without_completion",
                });

                deliver_background_bash_result(BackgroundBashResultDelivery {
                    parent_session_id: parent_session_id.clone(),
                    parent_agent_type: parent_agent_type.clone(),
                    parent_workspace_path: parent_workspace_path.clone(),
                    parent_remote_connection_id: parent_remote_connection_id.clone(),
                    parent_remote_ssh_host: parent_remote_ssh_host.clone(),
                    delivery_text,
                    display_text,
                    metadata: json_object_metadata(metadata),
                    terminal_session_id: terminal_session_id.clone(),
                    failure_context: "stream-end result",
                })
                .await;
            }
        });

        let execution_time_ms = elapsed_ms_u64(start_time);
        let output_file_note = format!("\nFull output will be saved to: {}", output_file_reference);

        let result_data = json!({
            "success": true,
            "command": command_str,
            "output": format!("Command started in background terminal session.{}", output_file_note),
            "exit_code": null,
            "interrupted": false,
            "working_directory": initial_cwd,
            "execution_time_ms": execution_time_ms,
            "terminal_session_id": bg_session_id,
            "output_file": output_file_reference,
            "run_in_background": true,
        });

        let result_for_assistant = format!(
            "Command started in background terminal session (id: {}). Working directory: {}.{} Its final result will be delivered back automatically when it finishes. Do not poll for status updates. If your current path is blocked on this result and there is no other useful local work to do, it is fine to end the current turn.",
            bg_session_id, initial_cwd, output_file_note
        );

        Ok(vec![ToolResult::Result {
            data: result_data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_detection_flags_mutating_bash_commands() {
        assert!(command_needs_light_checkpoint("cargo fmt"));
        assert!(command_needs_light_checkpoint("pnpm lint --fix"));
        assert!(command_needs_light_checkpoint("rm -rf target/tmp"));
        assert!(!command_needs_light_checkpoint("cargo test"));
        assert!(!command_needs_light_checkpoint("git status"));
    }

    #[test]
    fn truncate_output_preserving_tail_keeps_end_of_output() {
        let input = "BEGIN-".to_string() + &"x".repeat(120) + "-IMPORTANT-END";

        let truncated = tool_runtime::shell::truncate_output_preserving_tail(&input, 80);

        assert!(truncated.contains("tail preserved"));
        assert!(truncated.ends_with("IMPORTANT-END"));
        assert!(!truncated.contains("BEGIN-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"));
        assert!(truncated.chars().count() <= 80);
    }

    #[test]
    fn detect_osascript_keystroke_non_ascii_flags_cjk_keystroke() {
        let cmd = r#"osascript -e 'tell application "System Events" to keystroke "尉怡青"'"#;
        let hit = detect_osascript_keystroke_non_ascii(cmd).expect("should flag CJK keystroke");
        assert!(hit.contains("尉怡青"));
    }

    #[test]
    fn detect_osascript_keystroke_non_ascii_flags_emoji_keystroke() {
        let cmd = r#"osascript -e 'tell application "System Events" to keystroke "hi 👋"'"#;
        assert!(detect_osascript_keystroke_non_ascii(cmd).is_some());
    }

    #[test]
    fn detect_osascript_keystroke_non_ascii_passes_pure_ascii() {
        let cmd = r#"osascript -e 'tell application "System Events" to keystroke "hello"'"#;
        assert!(detect_osascript_keystroke_non_ascii(cmd).is_none());
    }

    #[test]
    fn detect_osascript_keystroke_non_ascii_passes_non_osascript() {
        let cmd = r#"echo "尉怡青""#;
        assert!(detect_osascript_keystroke_non_ascii(cmd).is_none());
    }

    #[test]
    fn detect_osascript_im_app_flags_wechat() {
        let cmd = r#"osascript -e 'tell application "WeChat" to activate'"#;
        assert_eq!(detect_osascript_im_app(cmd), Some("WeChat"));
    }

    #[test]
    fn detect_osascript_im_app_flags_weixin_chinese() {
        let cmd = r#"osascript -e 'tell application "微信" to activate'"#;
        assert_eq!(detect_osascript_im_app(cmd), Some("微信"));
    }

    #[test]
    fn detect_osascript_im_app_passes_non_im() {
        let cmd = r#"osascript -e 'tell application "Finder" to activate'"#;
        assert!(detect_osascript_im_app(cmd).is_none());
    }

    #[test]
    fn render_result_marks_truncated_output_and_keeps_tail() {
        let long_output = "prefix\n".to_string()
            + &"y".repeat(BASH_RESULT_MAX_OUTPUT_LENGTH + 100)
            + "\nfinal-error";

        let rendered = render_local_shell_result(LocalShellResultRenderRequest {
            terminal_session_id: "session-1",
            working_directory: "/repo",
            output_text: &long_output,
            interrupted: false,
            timed_out: false,
            exit_code: 1,
            shell_state: None,
        });

        assert!(rendered.contains("<output truncated=\"true\">"));
        assert!(rendered.contains("tail preserved"));
        assert!(rendered.contains("final-error"));
        assert!(rendered.contains("<exit_code>1</exit_code>"));
    }

    #[test]
    fn render_remote_result_keeps_stdout_and_stderr_separate() {
        let rendered = render_remote_shell_result(RemoteShellResultRenderRequest {
            working_directory: "/repo",
            stdout: "stdout text",
            stderr: "stderr text",
            interrupted: false,
            timed_out: false,
            exit_code: 2,
        });

        assert!(rendered.contains("<remote_ssh>true</remote_ssh>"));
        assert!(rendered.contains("<exit_code>2</exit_code>"));
        assert!(rendered.contains("<stdout>stdout text</stdout>"));
        assert!(rendered.contains("<stderr>stderr text</stderr>"));
        assert!(!rendered.contains("<terminal_session_id>"));
    }

    #[test]
    fn render_remote_result_uses_shared_budget_with_stderr_priority() {
        let long_stdout = "prefix\n".to_string()
            + &"x".repeat(BASH_RESULT_MAX_OUTPUT_LENGTH + 100)
            + "\nstdout-tail";
        let long_stderr = "prefix\n".to_string()
            + &"z".repeat(BASH_RESULT_MAX_OUTPUT_LENGTH / 2)
            + "\nstderr-tail";

        let rendered = render_remote_shell_result(RemoteShellResultRenderRequest {
            working_directory: "/repo",
            stdout: &long_stdout,
            stderr: &long_stderr,
            interrupted: false,
            timed_out: false,
            exit_code: 1,
        });

        assert!(rendered.contains("<stdout truncated=\"true\">"));
        assert!(rendered.contains("stdout-tail"));
        assert!(!rendered.contains("<stderr truncated=\"true\">"));
        assert!(rendered.contains("stderr-tail"));
    }

    #[test]
    fn render_remote_result_gives_all_budget_to_oversized_stderr() {
        let long_stderr = "prefix\n".to_string()
            + &"z".repeat(BASH_RESULT_MAX_OUTPUT_LENGTH + 100)
            + "\nremote-final-error";

        let rendered = render_remote_shell_result(RemoteShellResultRenderRequest {
            working_directory: "/repo",
            stdout: "stdout text",
            stderr: &long_stderr,
            interrupted: false,
            timed_out: false,
            exit_code: 1,
        });

        assert!(rendered.contains("<stdout truncated=\"true\">"));
        assert!(rendered.contains("no budget remaining"));
        assert!(rendered.contains("<stderr truncated=\"true\">"));
        assert!(rendered.contains("tail preserved"));
        assert!(rendered.contains("remote-final-error"));
    }

    #[test]
    fn input_schema_accepts_working_directory() {
        let tool = BashTool::new();
        let schema = tool.input_schema();

        assert!(schema["properties"].get("working_directory").is_some());
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn command_is_prefixed_with_quoted_working_directory_when_requested() {
        let command =
            command_for_working_directory("pnpm install", Some("/Users/example/My Project"));

        assert_eq!(command, "cd '/Users/example/My Project' && pnpm install");
    }

    #[test]
    fn command_prefix_escapes_single_quotes_in_working_directory() {
        let command = command_for_working_directory("pwd", Some("/tmp/it's fine"));

        assert_eq!(command, "cd '/tmp/it'\\''s fine' && pwd");
    }

    #[test]
    fn command_result_includes_working_directory_for_model() {
        let rendered = render_local_shell_result(LocalShellResultRenderRequest {
            terminal_session_id: "session-1",
            working_directory: "/private/tmp",
            output_text: "ERR_PNPM_NO_PKG_MANIFEST No package.json found in /private/tmp",
            interrupted: false,
            timed_out: false,
            exit_code: 1,
            shell_state: None,
        });

        assert!(rendered.contains("<exit_code>1</exit_code>"));
        assert!(rendered.contains("<working_directory>/private/tmp</working_directory>"));
        assert!(rendered.contains("ERR_PNPM_NO_PKG_MANIFEST"));
    }

    #[test]
    fn background_delivery_text_points_to_saved_output_file() {
        let rendered =
            format_background_command_delivery_text(BackgroundCommandDeliveryTextRequest {
                command: "pnpm test",
                terminal_session_id: "bg-session-1",
                working_directory: "/repo",
                status: BackgroundCommandStatusFacts {
                    exit_code: Some(0),
                    timed_out: false,
                    interrupted: false,
                },
                output_file_reference: "/runtime/sessions/session/tool-results/bash_123.txt",
                output_persist_error: None,
            });

        assert!(rendered.contains("Background Bash command completed successfully."));
        assert!(rendered.contains("status=\"completed\""));
        assert!(rendered.contains("terminal_session_id=\"bg-session-1\""));
        assert!(rendered.contains(
            "Full output was saved to: /runtime/sessions/session/tool-results/bash_123.txt"
        ));
    }

    #[test]
    fn background_display_text_is_concise() {
        assert_eq!(
            format_background_command_display_text(BackgroundCommandStatusFacts {
                exit_code: Some(0),
                timed_out: false,
                interrupted: false,
            }),
            "Background Bash command completed successfully."
        );
        assert_eq!(
            format_background_command_display_text(BackgroundCommandStatusFacts {
                exit_code: Some(1),
                timed_out: false,
                interrupted: false,
            }),
            "Background Bash command completed with a non-zero exit code."
        );
        assert_eq!(
            format_background_command_display_text(BackgroundCommandStatusFacts {
                exit_code: None,
                timed_out: true,
                interrupted: false,
            }),
            "Background Bash command timed out."
        );
        assert_eq!(
            format_background_command_display_text(BackgroundCommandStatusFacts {
                exit_code: Some(130),
                timed_out: false,
                interrupted: true,
            }),
            "Background Bash command was interrupted."
        );
        assert_eq!(
            format_background_command_error_display_text(),
            "Background Bash command failed before producing a final completion result."
        );
    }
}
