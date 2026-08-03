mod background_command_output;
mod command;
mod completion;
mod control;
mod env_snapshot;
mod input;
mod local_shell;
mod progress;
mod shell_kind;
mod stdin;

pub use background_command_output::{
    background_command_output_capture, BackgroundCommandOutputMetadata,
    BackgroundCommandOutputStatus, ListBackgroundCommandOutputRequest,
    ListBackgroundCommandOutputResponse, ReadBackgroundCommandOutputRequest,
    ReadBackgroundCommandOutputResponse, StartBackgroundCommandOutputCapture,
    BACKGROUND_COMMAND_OUTPUT_CAPTURE_LIMIT_BYTES,
};
pub use command::ExecCommandTool;
pub use control::{control_exec_command_session, ExecCommandControlError, ExecControlTool};
pub use input::{send_exec_command_input, ExecCommandInputRequest};
pub use stdin::WriteStdinTool;
pub use tool_runtime::exec_command::{
    ExecCommandCompletion, ExecCommandCompletionSource, ExecCommandCompletionStatus,
    ExecCommandControlAction, ExecCommandControlOrigin, ExecCommandControlRequest,
    ExecCommandControlResponse,
};

/// Resolve the user's configured terminal shell (respects `terminal.default_shell`
/// setting) and wrap `cmd` in the matching shell invocation argv, identical to how
/// ExecCommand spawns commands. Returns argv like `["/bin/zsh", "-c", "devecocli build"]`
/// or `["cmd", "/c", "devecocli build"]` or `["powershell", "-Command", "..."]`.
pub(crate) async fn resolve_shell_argv_for_command(cmd: &str) -> Vec<String> {
    let shell = local_shell::resolve_local_exec_shell().await;
    let kind = shell_kind::exec_command_shell_kind(&shell.shell_type);
    tool_runtime::exec_command::exec_command_argv_for_shell(
        shell.path.to_string_lossy().to_string(),
        kind,
        cmd,
    )
}
