use super::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct TerminalExecCommandRequest {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub tty: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
    pub lifecycle_sink: Option<TerminalExecLifecycleSink>,
    pub output_sink: Option<TerminalExecOutputSink>,
}

#[derive(Debug, Clone)]
pub struct TerminalWriteStdinRequest {
    pub session_id: i32,
    pub chars: String,
    pub append_enter: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TerminalSendStdinRequest {
    pub session_id: i32,
    pub chars: String,
    pub append_enter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalExecControlAction {
    Interrupt,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalExecControlOrigin {
    ModelTool,
    OutOfBand,
}

#[derive(Debug, Clone)]
pub struct TerminalExecControlRequest {
    pub session_id: i32,
    pub action: TerminalExecControlAction,
    pub origin: TerminalExecControlOrigin,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalExecSessionCompletionStatus {
    Exited,
    Interrupted,
    Killed,
    Pruned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalExecSessionCompletionSource {
    Process,
    OutOfBandControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalExecSessionCompletion {
    pub status: TerminalExecSessionCompletionStatus,
    pub source: TerminalExecSessionCompletionSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalExecCommandResponse {
    pub chunk_id: String,
    pub wall_time_seconds: f64,
    pub output: String,
    pub session_id: Option<i32>,
    pub exit_code: Option<i32>,
    pub original_output_chars: usize,
    pub completion: Option<TerminalExecSessionCompletion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalExecProcessLifecycleStatus {
    Running,
    Exited,
    Interrupted,
    Killed,
    Pruned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecProcessLifecycleEvent {
    pub session_id: i32,
    pub status: TerminalExecProcessLifecycleStatus,
    pub exit_code: Option<i32>,
}

pub type TerminalExecLifecycleSink = mpsc::UnboundedSender<TerminalExecProcessLifecycleEvent>;
pub type TerminalExecOutputSink = mpsc::UnboundedSender<String>;
pub type TerminalExecStreamingOutputSink = mpsc::Sender<String>;

#[async_trait::async_trait]
pub trait TerminalPort: RuntimeServicePort + std::fmt::Debug {
    /// Authoritative liveness query for an ExecCommand session.
    /// Providers that cannot inspect state fail closed.
    async fn is_session_active(&self, _session_id: i32) -> PortResult<bool> {
        Ok(false)
    }

    async fn exec_command(
        &self,
        request: TerminalExecCommandRequest,
    ) -> PortResult<TerminalExecCommandResponse>;

    async fn exec_command_streaming(
        &self,
        request: TerminalExecCommandRequest,
        output_sink: TerminalExecStreamingOutputSink,
    ) -> PortResult<TerminalExecCommandResponse>;

    async fn write_stdin(
        &self,
        request: TerminalWriteStdinRequest,
    ) -> PortResult<TerminalExecCommandResponse>;

    async fn write_stdin_streaming(
        &self,
        request: TerminalWriteStdinRequest,
        output_sink: TerminalExecStreamingOutputSink,
    ) -> PortResult<TerminalExecCommandResponse>;

    async fn send_stdin(&self, request: TerminalSendStdinRequest) -> PortResult<()>;

    async fn control_session(
        &self,
        request: TerminalExecControlRequest,
    ) -> PortResult<TerminalExecCommandResponse>;
}
