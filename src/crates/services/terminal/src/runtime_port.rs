use std::sync::Arc;

use bitfun_runtime_ports::{
    PortError, PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
    TerminalExecCommandRequest, TerminalExecCommandResponse, TerminalExecControlAction,
    TerminalExecControlOrigin, TerminalExecControlRequest, TerminalExecLifecycleSink,
    TerminalExecProcessLifecycleEvent, TerminalExecProcessLifecycleStatus,
    TerminalExecSessionCompletion, TerminalExecSessionCompletionSource,
    TerminalExecSessionCompletionStatus, TerminalExecStreamingOutputSink, TerminalPort,
    TerminalSendStdinRequest, TerminalWriteStdinRequest,
};
use tokio::sync::mpsc;

use crate::exec::{
    get_global_exec_process_manager, ExecCommandRequest, ExecCommandResponse, ExecControlAction,
    ExecControlOrigin, ExecControlRequest, ExecProcessLifecycleEvent, ExecProcessLifecycleStatus,
    ExecProcessManager, ExecSessionCompletion, ExecSessionCompletionSource,
    ExecSessionCompletionStatus, SendStdinRequest, WriteStdinRequest,
};
use crate::TerminalError;

#[derive(Clone)]
pub struct TerminalRuntimePort {
    manager: Arc<ExecProcessManager>,
}

impl TerminalRuntimePort {
    pub fn new(manager: Arc<ExecProcessManager>) -> Self {
        Self { manager }
    }

    pub fn global() -> Self {
        Self::new(get_global_exec_process_manager())
    }
}

impl Default for TerminalRuntimePort {
    fn default() -> Self {
        Self::global()
    }
}

impl std::fmt::Debug for TerminalRuntimePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalRuntimePort")
            .field("capability", &RuntimeServiceCapability::Terminal)
            .finish_non_exhaustive()
    }
}

impl RuntimeServicePort for TerminalRuntimePort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Terminal
    }
}

#[async_trait::async_trait]
impl TerminalPort for TerminalRuntimePort {
    async fn exec_command(
        &self,
        request: TerminalExecCommandRequest,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.manager
            .exec_command(local_exec_command_request(request))
            .await
            .map(terminal_exec_command_response)
            .map_err(terminal_port_error)
    }

    async fn exec_command_streaming(
        &self,
        request: TerminalExecCommandRequest,
        output_sink: TerminalExecStreamingOutputSink,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.manager
            .exec_command_streaming(local_exec_command_request(request), output_sink)
            .await
            .map(terminal_exec_command_response)
            .map_err(terminal_port_error)
    }

    async fn write_stdin(
        &self,
        request: TerminalWriteStdinRequest,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.manager
            .write_stdin(local_write_stdin_request(request))
            .await
            .map(terminal_exec_command_response)
            .map_err(terminal_port_error)
    }

    async fn write_stdin_streaming(
        &self,
        request: TerminalWriteStdinRequest,
        output_sink: TerminalExecStreamingOutputSink,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.manager
            .write_stdin_streaming(local_write_stdin_request(request), output_sink)
            .await
            .map(terminal_exec_command_response)
            .map_err(terminal_port_error)
    }

    async fn send_stdin(&self, request: TerminalSendStdinRequest) -> PortResult<()> {
        self.manager
            .send_stdin(SendStdinRequest {
                session_id: request.session_id,
                chars: request.chars,
                append_enter: request.append_enter,
            })
            .await
            .map_err(terminal_port_error)
    }

    async fn control_session(
        &self,
        request: TerminalExecControlRequest,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.manager
            .control_session(ExecControlRequest {
                session_id: request.session_id,
                action: local_control_action(request.action),
                origin: local_control_origin(request.origin),
                yield_time_ms: request.yield_time_ms,
                max_output_chars: request.max_output_chars,
            })
            .await
            .map(terminal_exec_command_response)
            .map_err(terminal_port_error)
    }
}

fn local_exec_command_request(request: TerminalExecCommandRequest) -> ExecCommandRequest {
    ExecCommandRequest {
        argv: request.argv,
        cwd: request.cwd,
        env: request.env,
        tty: request.tty,
        yield_time_ms: request.yield_time_ms,
        max_output_chars: request.max_output_chars,
        lifecycle_tx: lifecycle_tx(request.lifecycle_sink),
        output_capture_tx: request.output_sink,
    }
}

fn local_write_stdin_request(request: TerminalWriteStdinRequest) -> WriteStdinRequest {
    WriteStdinRequest {
        session_id: request.session_id,
        chars: request.chars,
        append_enter: request.append_enter,
        yield_time_ms: request.yield_time_ms,
        max_output_chars: request.max_output_chars,
    }
}

fn lifecycle_tx(
    target_tx: Option<TerminalExecLifecycleSink>,
) -> Option<mpsc::UnboundedSender<ExecProcessLifecycleEvent>> {
    let target_tx = target_tx?;
    let (tx, mut rx) = mpsc::unbounded_channel::<ExecProcessLifecycleEvent>();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = target_tx.send(terminal_lifecycle_event(event));
        }
    });
    Some(tx)
}

fn terminal_exec_command_response(response: ExecCommandResponse) -> TerminalExecCommandResponse {
    TerminalExecCommandResponse {
        chunk_id: response.chunk_id,
        wall_time_seconds: response.wall_time_seconds,
        output: response.output,
        session_id: response.session_id,
        exit_code: response.exit_code,
        original_output_chars: response.original_output_chars,
        completion: response.completion.map(terminal_completion),
    }
}

fn terminal_completion(completion: ExecSessionCompletion) -> TerminalExecSessionCompletion {
    TerminalExecSessionCompletion {
        status: match completion.status {
            ExecSessionCompletionStatus::Exited => TerminalExecSessionCompletionStatus::Exited,
            ExecSessionCompletionStatus::Interrupted => {
                TerminalExecSessionCompletionStatus::Interrupted
            }
            ExecSessionCompletionStatus::Killed => TerminalExecSessionCompletionStatus::Killed,
            ExecSessionCompletionStatus::Pruned => TerminalExecSessionCompletionStatus::Pruned,
        },
        source: match completion.source {
            ExecSessionCompletionSource::Process => TerminalExecSessionCompletionSource::Process,
            ExecSessionCompletionSource::OutOfBandControl => {
                TerminalExecSessionCompletionSource::OutOfBandControl
            }
        },
    }
}

fn terminal_lifecycle_event(event: ExecProcessLifecycleEvent) -> TerminalExecProcessLifecycleEvent {
    TerminalExecProcessLifecycleEvent {
        session_id: event.session_id,
        status: match event.status {
            ExecProcessLifecycleStatus::Running => TerminalExecProcessLifecycleStatus::Running,
            ExecProcessLifecycleStatus::Exited => TerminalExecProcessLifecycleStatus::Exited,
            ExecProcessLifecycleStatus::Interrupted => {
                TerminalExecProcessLifecycleStatus::Interrupted
            }
            ExecProcessLifecycleStatus::Killed => TerminalExecProcessLifecycleStatus::Killed,
            ExecProcessLifecycleStatus::Pruned => TerminalExecProcessLifecycleStatus::Pruned,
        },
        exit_code: event.exit_code,
    }
}

fn local_control_action(action: TerminalExecControlAction) -> ExecControlAction {
    match action {
        TerminalExecControlAction::Interrupt => ExecControlAction::Interrupt,
        TerminalExecControlAction::Kill => ExecControlAction::Kill,
    }
}

fn local_control_origin(origin: TerminalExecControlOrigin) -> ExecControlOrigin {
    match origin {
        TerminalExecControlOrigin::ModelTool => ExecControlOrigin::ModelTool,
        TerminalExecControlOrigin::OutOfBand => ExecControlOrigin::OutOfBand,
    }
}

fn terminal_port_error(error: TerminalError) -> PortError {
    let kind = match &error {
        TerminalError::SessionNotFound(_) => PortErrorKind::NotFound,
        TerminalError::InvalidConfig(_) => PortErrorKind::InvalidRequest,
        TerminalError::ProcessNotRunning => PortErrorKind::NotAvailable,
        TerminalError::Timeout(_) => PortErrorKind::Timeout,
        TerminalError::Io(_)
        | TerminalError::Pty(_)
        | TerminalError::Session(_)
        | TerminalError::Shell(_)
        | TerminalError::Serialization(_)
        | TerminalError::FlowControl(_)
        | TerminalError::Anyhow(_) => PortErrorKind::Backend,
    };
    PortError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::TerminalRuntimePort;
    use bitfun_runtime_ports::{
        RuntimeServiceCapability, RuntimeServicePort, TerminalExecCommandRequest,
        TerminalExecSessionCompletionStatus, TerminalPort, TerminalSendStdinRequest,
    };

    #[test]
    fn terminal_runtime_port_reports_terminal_capability() {
        let port = TerminalRuntimePort::default();

        assert_eq!(port.capability(), RuntimeServiceCapability::Terminal);
    }

    #[tokio::test]
    async fn exec_command_runs_short_command_through_runtime_port() {
        let port = TerminalRuntimePort::default();
        #[cfg(windows)]
        let argv = vec![
            "cmd".to_string(),
            "/C".to_string(),
            "echo bitfun-terminal-port".to_string(),
        ];
        #[cfg(not(windows))]
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            "printf bitfun-terminal-port".to_string(),
        ];

        let response = port
            .exec_command(TerminalExecCommandRequest {
                argv,
                cwd: std::env::current_dir().expect("current dir should be available"),
                env: std::collections::HashMap::new(),
                tty: false,
                yield_time_ms: Some(30_000),
                max_output_chars: None,
                lifecycle_sink: None,
                output_sink: None,
            })
            .await
            .expect("short command should run through terminal port");

        assert!(
            response.output.contains("bitfun-terminal-port"),
            "unexpected terminal output: {:?}",
            response.output
        );
        assert_eq!(response.exit_code, Some(0));
        assert_eq!(
            response.completion.map(|completion| completion.status),
            Some(TerminalExecSessionCompletionStatus::Exited)
        );
    }

    #[tokio::test]
    async fn missing_session_maps_to_not_found_port_error() {
        let port = TerminalRuntimePort::default();
        let error = port
            .send_stdin(TerminalSendStdinRequest {
                session_id: 987_654,
                chars: "x".to_string(),
                append_enter: false,
            })
            .await
            .expect_err("missing session should be a typed port error");

        assert_eq!(error.kind, bitfun_runtime_ports::PortErrorKind::NotFound);
    }
}
