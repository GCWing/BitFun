use crate::service::remote_ssh::{get_global_remote_exec_process_manager, RemoteSendStdinRequest};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::{TerminalPort, TerminalSendStdinRequest};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ExecCommandInputRequest {
    pub session_id: i32,
    pub chars: String,
    pub append_enter: bool,
    pub remote: bool,
}

pub async fn send_exec_command_input(
    request: ExecCommandInputRequest,
    terminal_port: Option<&Arc<dyn TerminalPort>>,
) -> BitFunResult<()> {
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

    let terminal_port = terminal_port.ok_or_else(|| {
        BitFunError::tool("terminal runtime service is required for ExecCommand input".to_string())
    })?;
    terminal_port
        .send_stdin(TerminalSendStdinRequest {
            session_id: request.session_id,
            chars: request.chars,
            append_enter: request.append_enter,
        })
        .await
        .map_err(|error| BitFunError::tool(format!("ExecCommand input failed: {}", error.message)))
}
