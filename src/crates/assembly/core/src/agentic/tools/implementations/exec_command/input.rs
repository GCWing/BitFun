use crate::service::remote_ssh::{get_global_remote_exec_process_manager, RemoteSendStdinRequest};
use crate::util::errors::{BitFunError, BitFunResult};
use terminal_core::{get_global_exec_process_manager, LocalSendStdinRequest};

#[derive(Debug, Clone)]
pub struct ExecCommandInputRequest {
    pub session_id: i32,
    pub chars: String,
    pub append_enter: bool,
    pub remote: bool,
}

pub async fn send_exec_command_input(request: ExecCommandInputRequest) -> BitFunResult<()> {
    if request.remote {
        get_global_remote_exec_process_manager()
            .send_stdin(RemoteSendStdinRequest {
                session_id: request.session_id,
                chars: request.chars,
                append_enter: request.append_enter,
            })
            .await
            .map_err(|error| BitFunError::tool(format!("ExecCommand input failed: {error}")))?;
        return Ok(());
    }

    get_global_exec_process_manager()
        .send_stdin(LocalSendStdinRequest {
            session_id: request.session_id,
            chars: request.chars,
            append_enter: request.append_enter,
        })
        .await
        .map_err(|error| BitFunError::tool(format!("ExecCommand input failed: {error}")))
}
