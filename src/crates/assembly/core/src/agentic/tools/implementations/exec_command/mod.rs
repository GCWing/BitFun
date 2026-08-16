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

use crate::agentic::tools::framework::ToolUseContext;
use bitfun_core_types::MINIMAL_HARNESS_PROFILE_ID;

pub(crate) const HARNESS_PROFILE_CONTEXT_KEY: &str = "harness_profile_id";

async fn command_controls_available(context: Option<&ToolUseContext>) -> bool {
    let Some(context) = context else {
        return true;
    };
    let is_minimal = context
        .custom_data
        .get(HARNESS_PROFILE_CONTEXT_KEY)
        .and_then(|value| value.as_str())
        == Some(MINIMAL_HARNESS_PROFILE_ID);
    if !is_minimal {
        return true;
    }

    let Some(agent_session_id) = context.session_id.as_ref() else {
        return false;
    };
    let remote = context.is_remote();
    let activities = background_command_output_capture()
        .list(ListBackgroundCommandOutputRequest {
            agent_session_id: Some(agent_session_id.clone()),
        })
        .await
        .activities;

    for activity in activities {
        if activity.remote != remote || activity.status != BackgroundCommandOutputStatus::Running {
            continue;
        }
        let Some(exec_session_id) = activity.exec_session_id else {
            continue;
        };
        let active = if remote {
            match context.remote_exec_port() {
                Some(port) => port.is_session_active(exec_session_id).await,
                None => return false,
            }
        } else {
            match context.terminal_port() {
                Some(port) => port.is_session_active(exec_session_id).await,
                None => return false,
            }
        };
        if matches!(active, Ok(true)) {
            return true;
        }
    }

    false
}
