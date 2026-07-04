use std::sync::Arc;

use async_trait::async_trait;
use bitfun_runtime_ports as ports;
use tokio::sync::mpsc;

use super::{
    get_global_remote_exec_process_manager, RemoteExecCommandRequest as ServiceCommandRequest,
    RemoteExecCommandResponse as ServiceCommandResponse,
    RemoteExecControlAction as ServiceControlAction,
    RemoteExecControlOrigin as ServiceControlOrigin,
    RemoteExecControlRequest as ServiceControlRequest, RemoteExecError,
    RemoteExecProcessLifecycleEvent as ServiceLifecycleEvent,
    RemoteExecProcessLifecycleStatus as ServiceLifecycleStatus, RemoteExecProcessManager,
    RemoteExecSessionCompletion as ServiceCompletion,
    RemoteExecSessionCompletionSource as ServiceCompletionSource,
    RemoteExecSessionCompletionStatus as ServiceCompletionStatus,
    RemoteSendStdinRequest as ServiceSendStdinRequest,
    RemoteWriteStdinRequest as ServiceStdinRequest, SSHCommandOptions, SSHCommandResult,
    SSHConnectionManager,
};

#[async_trait]
pub trait RemoteExecSshManagerProvider: Send + Sync + std::fmt::Debug {
    async fn ssh_manager(&self) -> ports::PortResult<SSHConnectionManager>;
}

pub struct RemoteExecRuntimePort {
    ssh_manager_provider: Arc<dyn RemoteExecSshManagerProvider>,
    manager: Arc<RemoteExecProcessManager>,
}

impl RemoteExecRuntimePort {
    pub fn new(ssh_manager_provider: Arc<dyn RemoteExecSshManagerProvider>) -> Self {
        Self {
            ssh_manager_provider,
            manager: get_global_remote_exec_process_manager(),
        }
    }

    async fn command_request(
        &self,
        request: ports::RemoteExecCommandRequest,
    ) -> ports::PortResult<ServiceCommandRequest> {
        Ok(ServiceCommandRequest {
            ssh_manager: self.ssh_manager_provider.ssh_manager().await?,
            connection_id: request.connection_id,
            command: request.command,
            tty: request.tty,
            yield_time_ms: request.yield_time_ms,
            max_output_chars: request.max_output_chars,
            lifecycle_tx: bridge_lifecycle_sink(request.lifecycle_sink),
            output_capture_tx: request.output_sink,
        })
    }
}

impl std::fmt::Debug for RemoteExecRuntimePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteExecRuntimePort")
            .field("ssh_manager_provider", &self.ssh_manager_provider)
            .field("manager", &"<RemoteExecProcessManager>")
            .finish()
    }
}

impl ports::RuntimeServicePort for RemoteExecRuntimePort {
    fn capability(&self) -> ports::RuntimeServiceCapability {
        ports::RuntimeServiceCapability::RemoteExec
    }
}

#[async_trait]
impl ports::RemoteExecPort for RemoteExecRuntimePort {
    async fn exec_command_once(
        &self,
        request: ports::RemoteExecOneShotCommandRequest,
    ) -> ports::PortResult<ports::RemoteExecOneShotCommandResponse> {
        let ssh_manager = self.ssh_manager_provider.ssh_manager().await?;
        ssh_manager
            .execute_command_with_options(
                &request.connection_id,
                &request.command,
                SSHCommandOptions {
                    timeout_ms: request.timeout_ms,
                    cancellation_token: None,
                },
            )
            .await
            .map(one_shot_response_from_service)
            .map_err(|error| {
                ports::PortError::new(ports::PortErrorKind::Backend, error.to_string())
            })
    }

    async fn exec_command(
        &self,
        request: ports::RemoteExecCommandRequest,
    ) -> ports::PortResult<ports::RemoteExecCommandResponse> {
        let request = self.command_request(request).await?;
        self.manager
            .exec_command(request)
            .await
            .map(response_from_service)
            .map_err(error_from_service)
    }

    async fn exec_command_streaming(
        &self,
        request: ports::RemoteExecCommandRequest,
        output_sink: ports::RemoteExecStreamingOutputSink,
    ) -> ports::PortResult<ports::RemoteExecCommandResponse> {
        let request = self.command_request(request).await?;
        self.manager
            .exec_command_streaming(request, output_sink)
            .await
            .map(response_from_service)
            .map_err(error_from_service)
    }

    async fn write_stdin(
        &self,
        request: ports::RemoteWriteStdinRequest,
    ) -> ports::PortResult<ports::RemoteExecCommandResponse> {
        self.manager
            .write_stdin(stdin_request_to_service(request))
            .await
            .map(response_from_service)
            .map_err(error_from_service)
    }

    async fn write_stdin_streaming(
        &self,
        request: ports::RemoteWriteStdinRequest,
        output_sink: ports::RemoteExecStreamingOutputSink,
    ) -> ports::PortResult<ports::RemoteExecCommandResponse> {
        self.manager
            .write_stdin_streaming(stdin_request_to_service(request), output_sink)
            .await
            .map(response_from_service)
            .map_err(error_from_service)
    }

    async fn send_stdin(&self, request: ports::RemoteSendStdinRequest) -> ports::PortResult<()> {
        self.manager
            .send_stdin(ServiceSendStdinRequest {
                session_id: request.session_id,
                chars: request.chars,
                append_enter: request.append_enter,
            })
            .await
            .map_err(error_from_service)
    }

    async fn control_session(
        &self,
        request: ports::RemoteExecControlRequest,
    ) -> ports::PortResult<ports::RemoteExecCommandResponse> {
        self.manager
            .control_session(ServiceControlRequest {
                session_id: request.session_id,
                action: control_action_to_service(request.action),
                origin: control_origin_to_service(request.origin),
                yield_time_ms: request.yield_time_ms,
                max_output_chars: request.max_output_chars,
            })
            .await
            .map(response_from_service)
            .map_err(error_from_service)
    }
}

fn stdin_request_to_service(request: ports::RemoteWriteStdinRequest) -> ServiceStdinRequest {
    ServiceStdinRequest {
        session_id: request.session_id,
        chars: request.chars,
        append_enter: request.append_enter,
        yield_time_ms: request.yield_time_ms,
        max_output_chars: request.max_output_chars,
    }
}

fn one_shot_response_from_service(
    response: SSHCommandResult,
) -> ports::RemoteExecOneShotCommandResponse {
    ports::RemoteExecOneShotCommandResponse {
        stdout: response.stdout,
        stderr: response.stderr,
        exit_code: response.exit_code,
        interrupted: response.interrupted,
        timed_out: response.timed_out,
    }
}

fn response_from_service(response: ServiceCommandResponse) -> ports::RemoteExecCommandResponse {
    ports::RemoteExecCommandResponse {
        chunk_id: response.chunk_id,
        wall_time_seconds: response.wall_time_seconds,
        output: response.output,
        session_id: response.session_id,
        exit_code: response.exit_code,
        original_output_chars: response.original_output_chars,
        completion: response.completion.map(completion_from_service),
    }
}

fn completion_from_service(completion: ServiceCompletion) -> ports::RemoteExecSessionCompletion {
    ports::RemoteExecSessionCompletion {
        status: match completion.status {
            ServiceCompletionStatus::Exited => ports::RemoteExecSessionCompletionStatus::Exited,
            ServiceCompletionStatus::Interrupted => {
                ports::RemoteExecSessionCompletionStatus::Interrupted
            }
            ServiceCompletionStatus::Killed => ports::RemoteExecSessionCompletionStatus::Killed,
            ServiceCompletionStatus::Pruned => ports::RemoteExecSessionCompletionStatus::Pruned,
        },
        source: match completion.source {
            ServiceCompletionSource::Process => ports::RemoteExecSessionCompletionSource::Process,
            ServiceCompletionSource::OutOfBandControl => {
                ports::RemoteExecSessionCompletionSource::OutOfBandControl
            }
        },
    }
}

fn control_action_to_service(action: ports::RemoteExecControlAction) -> ServiceControlAction {
    match action {
        ports::RemoteExecControlAction::Interrupt => ServiceControlAction::Interrupt,
        ports::RemoteExecControlAction::Kill => ServiceControlAction::Kill,
    }
}

fn control_origin_to_service(origin: ports::RemoteExecControlOrigin) -> ServiceControlOrigin {
    match origin {
        ports::RemoteExecControlOrigin::ModelTool => ServiceControlOrigin::ModelTool,
        ports::RemoteExecControlOrigin::OutOfBand => ServiceControlOrigin::OutOfBand,
    }
}

fn bridge_lifecycle_sink(
    lifecycle_sink: Option<ports::RemoteExecLifecycleSink>,
) -> Option<mpsc::UnboundedSender<ServiceLifecycleEvent>> {
    let lifecycle_sink = lifecycle_sink?;
    let (tx, mut rx) = mpsc::unbounded_channel::<ServiceLifecycleEvent>();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = lifecycle_sink.send(lifecycle_event_from_service(event));
        }
    });
    Some(tx)
}

fn lifecycle_event_from_service(
    event: ServiceLifecycleEvent,
) -> ports::RemoteExecProcessLifecycleEvent {
    ports::RemoteExecProcessLifecycleEvent {
        session_id: event.session_id,
        status: match event.status {
            ServiceLifecycleStatus::Running => ports::RemoteExecProcessLifecycleStatus::Running,
            ServiceLifecycleStatus::Exited => ports::RemoteExecProcessLifecycleStatus::Exited,
            ServiceLifecycleStatus::Interrupted => {
                ports::RemoteExecProcessLifecycleStatus::Interrupted
            }
            ServiceLifecycleStatus::Killed => ports::RemoteExecProcessLifecycleStatus::Killed,
            ServiceLifecycleStatus::Pruned => ports::RemoteExecProcessLifecycleStatus::Pruned,
        },
        exit_code: event.exit_code,
    }
}

fn error_from_service(error: RemoteExecError) -> ports::PortError {
    match error {
        RemoteExecError::SessionNotFound(session_id) => ports::PortError::new(
            ports::PortErrorKind::NotFound,
            format!("remote exec session not found: {session_id}"),
        ),
        RemoteExecError::Other(error) => {
            ports::PortError::new(ports::PortErrorKind::Backend, error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_response_preserves_stdout_stderr_and_timeout_flags() {
        let response = one_shot_response_from_service(SSHCommandResult {
            stdout: "/bin/bash\n".to_string(),
            stderr: "/tmp/noise\n".to_string(),
            exit_code: 0,
            interrupted: false,
            timed_out: true,
        });

        assert_eq!(response.stdout, "/bin/bash\n");
        assert_eq!(response.stderr, "/tmp/noise\n");
        assert_eq!(response.exit_code, 0);
        assert!(!response.interrupted);
        assert!(response.timed_out);
    }

    #[test]
    fn completion_and_lifecycle_mapping_preserve_remote_status() {
        let completion = completion_from_service(ServiceCompletion {
            status: ServiceCompletionStatus::Killed,
            source: ServiceCompletionSource::OutOfBandControl,
        });
        assert_eq!(
            completion.status,
            ports::RemoteExecSessionCompletionStatus::Killed
        );
        assert_eq!(
            completion.source,
            ports::RemoteExecSessionCompletionSource::OutOfBandControl
        );

        let lifecycle = lifecycle_event_from_service(ServiceLifecycleEvent {
            session_id: 42,
            status: ServiceLifecycleStatus::Interrupted,
            exit_code: Some(130),
        });
        assert_eq!(lifecycle.session_id, 42);
        assert_eq!(
            lifecycle.status,
            ports::RemoteExecProcessLifecycleStatus::Interrupted
        );
        assert_eq!(lifecycle.exit_code, Some(130));
    }

    #[test]
    fn session_not_found_maps_to_port_not_found() {
        let error = error_from_service(RemoteExecError::SessionNotFound(77));

        assert_eq!(error.kind, ports::PortErrorKind::NotFound);
        assert!(error.message.contains("77"));
    }
}
