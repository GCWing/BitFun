use super::*;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct RemoteExecCommandRequest {
    pub connection_id: String,
    pub command: String,
    pub tty: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
    pub lifecycle_sink: Option<RemoteExecLifecycleSink>,
    pub output_sink: Option<RemoteExecOutputSink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExecOneShotCommandRequest {
    pub connection_id: String,
    pub command: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExecOneShotCommandResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub interrupted: bool,
    pub timed_out: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteWriteStdinRequest {
    pub session_id: i32,
    pub chars: String,
    pub append_enter: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct RemoteSendStdinRequest {
    pub session_id: i32,
    pub chars: String,
    pub append_enter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExecControlAction {
    Interrupt,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExecControlOrigin {
    ModelTool,
    OutOfBand,
}

#[derive(Debug, Clone)]
pub struct RemoteExecControlRequest {
    pub session_id: i32,
    pub action: RemoteExecControlAction,
    pub origin: RemoteExecControlOrigin,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExecSessionCompletionStatus {
    Exited,
    Interrupted,
    Killed,
    Pruned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExecSessionCompletionSource {
    Process,
    OutOfBandControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteExecSessionCompletion {
    pub status: RemoteExecSessionCompletionStatus,
    pub source: RemoteExecSessionCompletionSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteExecCommandResponse {
    pub chunk_id: String,
    pub wall_time_seconds: f64,
    pub output: String,
    pub session_id: Option<i32>,
    pub exit_code: Option<i32>,
    pub original_output_chars: usize,
    pub completion: Option<RemoteExecSessionCompletion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExecProcessLifecycleStatus {
    Running,
    Exited,
    Interrupted,
    Killed,
    Pruned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExecProcessLifecycleEvent {
    pub session_id: i32,
    pub status: RemoteExecProcessLifecycleStatus,
    pub exit_code: Option<i32>,
}

pub type RemoteExecLifecycleSink = mpsc::UnboundedSender<RemoteExecProcessLifecycleEvent>;
pub type RemoteExecOutputSink = mpsc::UnboundedSender<String>;
pub type RemoteExecStreamingOutputSink = mpsc::Sender<String>;
#[async_trait::async_trait]
pub trait RemoteExecPort: RuntimeServicePort + std::fmt::Debug {
    /// Authoritative liveness query for a remote ExecCommand session.
    async fn is_session_active(&self, _session_id: i32) -> PortResult<bool> {
        Ok(false)
    }

    async fn exec_command_once(
        &self,
        request: RemoteExecOneShotCommandRequest,
    ) -> PortResult<RemoteExecOneShotCommandResponse>;

    async fn exec_command(
        &self,
        request: RemoteExecCommandRequest,
    ) -> PortResult<RemoteExecCommandResponse>;

    async fn exec_command_streaming(
        &self,
        request: RemoteExecCommandRequest,
        output_sink: RemoteExecStreamingOutputSink,
    ) -> PortResult<RemoteExecCommandResponse>;

    async fn write_stdin(
        &self,
        request: RemoteWriteStdinRequest,
    ) -> PortResult<RemoteExecCommandResponse>;

    async fn write_stdin_streaming(
        &self,
        request: RemoteWriteStdinRequest,
        output_sink: RemoteExecStreamingOutputSink,
    ) -> PortResult<RemoteExecCommandResponse>;

    async fn send_stdin(&self, request: RemoteSendStdinRequest) -> PortResult<()>;

    async fn control_session(
        &self,
        request: RemoteExecControlRequest,
    ) -> PortResult<RemoteExecCommandResponse>;
}
