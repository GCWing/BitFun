//! Model-facing remote command execution runtime.
//!
//! This mirrors the local `terminal_core::ExecProcessManager` semantics for SSH
//! workspaces while keeping tool-owned command sessions separate from UI
//! terminal sessions.

use crate::remote_ssh::SSHConnectionManager;
use anyhow::{anyhow, Context};
use rand::Rng;
use russh::client::Msg;
use russh::{Channel, ChannelMsg, Sig};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use terminal_core::{spawn_pty, PtyEvent, ShellConfig, ShellType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

const DEFAULT_YIELD_TIME_MS: u64 = 10_000;
const MAX_RETAINED_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_REMOTE_EXEC_SESSIONS: usize = 64;
const MAX_COMPLETED_REMOTE_EXEC_SESSIONS: usize = 64;
const REMOTE_INTERRUPT_GRACE_TIMEOUT_MS: u64 = 3_000;
const REMOTE_CONTROL_DRAIN_TIMEOUT_MS: u64 = 500;

static GLOBAL_REMOTE_EXEC_MANAGER: OnceLock<Arc<RemoteExecProcessManager>> = OnceLock::new();

pub fn get_global_remote_exec_process_manager() -> Arc<RemoteExecProcessManager> {
    GLOBAL_REMOTE_EXEC_MANAGER
        .get_or_init(|| Arc::new(RemoteExecProcessManager::default()))
        .clone()
}

#[derive(Clone)]
pub struct RemoteExecCommandRequest {
    pub ssh_manager: SSHConnectionManager,
    pub connection_id: String,
    pub command: String,
    pub tty: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
    pub lifecycle_tx: Option<mpsc::UnboundedSender<RemoteExecProcessLifecycleEvent>>,
    pub output_capture_tx: Option<mpsc::UnboundedSender<String>>,
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

#[derive(Debug, Clone)]
pub struct RemoteExecCommandResponse {
    pub chunk_id: String,
    pub wall_time_seconds: f64,
    pub output: String,
    pub session_id: Option<i32>,
    pub exit_code: Option<i32>,
    pub original_output_chars: usize,
    pub completion: Option<RemoteExecSessionCompletion>,
}

pub type RemoteExecResult<T> = std::result::Result<T, RemoteExecError>;

#[derive(Debug, thiserror::Error)]
pub enum RemoteExecError {
    #[error("session not found: {0}")]
    SessionNotFound(i32),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExecProcessLifecycleStatus {
    Running,
    Exited,
    Interrupted,
    Killed,
    Pruned,
}

#[derive(Debug, Clone)]
pub struct RemoteExecProcessLifecycleEvent {
    pub session_id: i32,
    pub status: RemoteExecProcessLifecycleStatus,
    pub exit_code: Option<i32>,
}

pub struct RemoteExecProcessManager {
    sessions: Mutex<HashMap<i32, RemoteExecSessionEntry>>,
    completed_sessions: Mutex<HashMap<i32, CompletedRemoteExecSession>>,
}

impl Default for RemoteExecProcessManager {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            completed_sessions: Mutex::new(HashMap::new()),
        }
    }
}

struct RemoteExecSessionEntry {
    process: Arc<RemoteExecProcess>,
    tty: bool,
    cursor: OutputCursor,
    last_used: Instant,
    lifecycle_tx: Option<mpsc::UnboundedSender<RemoteExecProcessLifecycleEvent>>,
}

#[derive(Clone)]
struct CompletedRemoteExecSession {
    output: String,
    exit_code: Option<i32>,
    original_output_chars: usize,
    completion: RemoteExecSessionCompletion,
    completed_at: Instant,
}

struct RemoteExecProcess {
    output: Arc<OutputState>,
    command_tx: mpsc::Sender<RemoteExecProcessCommand>,
    out_of_band_control_action: StdMutex<Option<RemoteExecControlAction>>,
}

enum RemoteExecProcessCommand {
    Write(Vec<u8>),
    Control(RemoteExecControlAction),
}

#[derive(Debug, Clone, Copy)]
enum RemotePipeControlState {
    InterruptGrace { deadline: Instant },
    KillDrain { deadline: Instant },
}

impl RemotePipeControlState {
    fn deadline(self) -> Instant {
        match self {
            Self::InterruptGrace { deadline } | Self::KillDrain { deadline } => deadline,
        }
    }
}

struct OutputState {
    inner: Mutex<OutputInner>,
    notify: Notify,
    output_capture_tx: Option<mpsc::UnboundedSender<String>>,
}

struct OutputInner {
    chunks: VecDeque<(u64, OutputStream, Vec<u8>)>,
    next_seq: u64,
    retained_bytes: usize,
    capture_pending_utf8: PendingUtf8Streams,
    closed: bool,
    exit_code: Option<i32>,
}

#[derive(Clone)]
struct OutputCursor {
    next_seq: u64,
    pending_utf8: PendingUtf8Streams,
}

#[derive(Clone, Copy)]
enum OutputStream {
    Combined,
    Stdout,
    Stderr,
}

#[derive(Clone, Default)]
struct PendingUtf8Streams {
    combined: Vec<u8>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl PendingUtf8Streams {
    fn get_mut(&mut self, stream: OutputStream) -> &mut Vec<u8> {
        match stream {
            OutputStream::Combined => &mut self.combined,
            OutputStream::Stdout => &mut self.stdout,
            OutputStream::Stderr => &mut self.stderr,
        }
    }

    fn clear(&mut self) {
        self.combined.clear();
        self.stdout.clear();
        self.stderr.clear();
    }

    fn finish(&mut self) -> String {
        let mut output = String::new();
        output.push_str(&decode_utf8_stream(&mut self.combined, &[], true));
        output.push_str(&decode_utf8_stream(&mut self.stdout, &[], true));
        output.push_str(&decode_utf8_stream(&mut self.stderr, &[], true));
        output
    }
}

struct CollectedOutput {
    output: String,
    original_output_chars: usize,
    cursor: OutputCursor,
}

struct HeadTailText {
    head_budget: usize,
    tail_budget: usize,
    head: String,
    tail: VecDeque<char>,
    head_chars: usize,
    tail_chars: usize,
    omitted_chars: usize,
    total_chars: usize,
}

impl RemoteExecProcessManager {
    pub async fn exec_command(
        &self,
        request: RemoteExecCommandRequest,
    ) -> RemoteExecResult<RemoteExecCommandResponse> {
        self.exec_command_inner(request, None).await
    }

    pub async fn exec_command_streaming(
        &self,
        request: RemoteExecCommandRequest,
        output_tx: mpsc::Sender<String>,
    ) -> RemoteExecResult<RemoteExecCommandResponse> {
        self.exec_command_inner(request, Some(output_tx)).await
    }

    async fn exec_command_inner(
        &self,
        request: RemoteExecCommandRequest,
        output_tx: Option<mpsc::Sender<String>>,
    ) -> RemoteExecResult<RemoteExecCommandResponse> {
        let process = Arc::new(spawn_remote_process(request.clone()).await?);
        let cursor = OutputCursor {
            next_seq: 0,
            pending_utf8: PendingUtf8Streams::default(),
        };
        let session_id = self
            .store_session(
                Arc::clone(&process),
                request.tty,
                cursor.clone(),
                request.lifecycle_tx,
            )
            .await;
        let started_at = Instant::now();
        let collected = process
            .output
            .collect_until(
                cursor,
                deadline_from_now(request.yield_time_ms),
                request.max_output_chars.unwrap_or(usize::MAX),
                output_tx.as_ref(),
            )
            .await;

        let exit_code = process.output.exit_code().await;
        let closed = process.output.is_closed().await;
        let completion = if closed {
            Some(completion_for_closed_remote_process(
                process.out_of_band_control_action(),
            ))
        } else {
            None
        };
        self.update_or_remove_session(
            session_id,
            &process,
            collected.cursor.clone(),
            None,
            exit_code,
        )
        .await;

        Ok(RemoteExecCommandResponse {
            chunk_id: new_chunk_id(),
            wall_time_seconds: started_at.elapsed().as_secs_f64(),
            output: collected.output,
            session_id: (!closed).then_some(session_id),
            exit_code,
            original_output_chars: collected.original_output_chars,
            completion,
        })
    }

    pub async fn write_stdin(
        &self,
        request: RemoteWriteStdinRequest,
    ) -> RemoteExecResult<RemoteExecCommandResponse> {
        self.write_stdin_inner(request, None).await
    }

    pub async fn write_stdin_streaming(
        &self,
        request: RemoteWriteStdinRequest,
        output_tx: mpsc::Sender<String>,
    ) -> RemoteExecResult<RemoteExecCommandResponse> {
        self.write_stdin_inner(request, Some(output_tx)).await
    }

    pub async fn send_stdin(&self, request: RemoteSendStdinRequest) -> RemoteExecResult<()> {
        let (process, tty) = {
            let mut sessions = self.sessions.lock().await;
            let entry = sessions
                .get_mut(&request.session_id)
                .ok_or(RemoteExecError::SessionNotFound(request.session_id))?;
            entry.last_used = Instant::now();
            (Arc::clone(&entry.process), entry.tty)
        };

        let input = input_bytes_for_write(&request.chars, request.append_enter);
        if input.is_empty() {
            return Ok(());
        }
        if !tty {
            return Err(anyhow!("stdin input requires a tty session").into());
        }

        process
            .command_tx
            .send(RemoteExecProcessCommand::Write(input))
            .await
            .context("remote process has already exited")
            .map_err(RemoteExecError::from)
    }

    async fn write_stdin_inner(
        &self,
        request: RemoteWriteStdinRequest,
        output_tx: Option<mpsc::Sender<String>>,
    ) -> RemoteExecResult<RemoteExecCommandResponse> {
        let (process, tty, cursor) = {
            let mut sessions = self.sessions.lock().await;
            let Some(entry) = sessions.get_mut(&request.session_id) else {
                drop(sessions);
                if request.chars.is_empty() {
                    if let Some(completed) = self.take_completed_session(request.session_id).await {
                        return Ok(RemoteExecCommandResponse {
                            chunk_id: new_chunk_id(),
                            wall_time_seconds: 0.0,
                            output: completed.output,
                            session_id: None,
                            exit_code: completed.exit_code,
                            original_output_chars: completed.original_output_chars,
                            completion: Some(completed.completion),
                        });
                    }
                }
                return Err(RemoteExecError::SessionNotFound(request.session_id));
            };
            entry.last_used = Instant::now();
            (Arc::clone(&entry.process), entry.tty, entry.cursor.clone())
        };

        let input = input_bytes_for_write(&request.chars, request.append_enter);
        if !input.is_empty() && tty {
            process
                .command_tx
                .send(RemoteExecProcessCommand::Write(input))
                .await
                .context("remote process has already exited")?;
        }

        let started_at = Instant::now();
        let collected = process
            .output
            .collect_until(
                cursor,
                deadline_from_now(request.yield_time_ms),
                request.max_output_chars.unwrap_or(usize::MAX),
                output_tx.as_ref(),
            )
            .await;

        let closed = process.output.is_closed().await;
        let exit_code = process.output.exit_code().await;
        let completion = if closed {
            Some(completion_for_closed_remote_process(
                process.out_of_band_control_action(),
            ))
        } else {
            None
        };
        self.update_or_remove_session(
            request.session_id,
            &process,
            collected.cursor.clone(),
            completion.map(|completion| lifecycle_status_for_completion(completion.status)),
            exit_code,
        )
        .await;

        Ok(RemoteExecCommandResponse {
            chunk_id: new_chunk_id(),
            wall_time_seconds: started_at.elapsed().as_secs_f64(),
            output: collected.output,
            session_id: (!closed).then_some(request.session_id),
            exit_code,
            original_output_chars: collected.original_output_chars,
            completion,
        })
    }

    pub async fn control_session(
        &self,
        request: RemoteExecControlRequest,
    ) -> RemoteExecResult<RemoteExecCommandResponse> {
        let (process, cursor) = {
            let mut sessions = self.sessions.lock().await;
            let entry = sessions
                .get_mut(&request.session_id)
                .ok_or(RemoteExecError::SessionNotFound(request.session_id))?;
            entry.last_used = Instant::now();
            if request.origin == RemoteExecControlOrigin::OutOfBand {
                entry.process.mark_out_of_band_control(request.action);
            }
            (Arc::clone(&entry.process), entry.cursor.clone())
        };

        process
            .command_tx
            .send(RemoteExecProcessCommand::Control(request.action))
            .await
            .context("remote process has already exited")?;

        let started_at = Instant::now();
        let collected = process
            .output
            .collect_until(
                cursor.clone(),
                deadline_from_now(request.yield_time_ms),
                request.max_output_chars.unwrap_or(usize::MAX),
                None,
            )
            .await;

        let closed = process.output.is_closed().await;
        let exit_code = process.output.exit_code().await;
        let completion = closed.then_some(RemoteExecSessionCompletion {
            status: completion_status_for_control_action(request.action),
            source: match request.origin {
                RemoteExecControlOrigin::ModelTool => RemoteExecSessionCompletionSource::Process,
                RemoteExecControlOrigin::OutOfBand => {
                    RemoteExecSessionCompletionSource::OutOfBandControl
                }
            },
        });
        let lifecycle_status =
            completion.map(|completion| lifecycle_status_for_completion(completion.status));
        self.update_or_remove_session(
            request.session_id,
            &process,
            if request.origin == RemoteExecControlOrigin::ModelTool {
                collected.cursor.clone()
            } else {
                cursor
            },
            lifecycle_status,
            exit_code,
        )
        .await;
        if request.origin == RemoteExecControlOrigin::OutOfBand && closed {
            self.store_completed_session(
                request.session_id,
                CompletedRemoteExecSession {
                    output: collected.output.clone(),
                    exit_code,
                    original_output_chars: collected.original_output_chars,
                    completion: completion.expect("closed process should have completion"),
                    completed_at: Instant::now(),
                },
            )
            .await;
        }

        Ok(RemoteExecCommandResponse {
            chunk_id: new_chunk_id(),
            wall_time_seconds: started_at.elapsed().as_secs_f64(),
            output: collected.output,
            session_id: (!closed).then_some(request.session_id),
            exit_code,
            original_output_chars: collected.original_output_chars,
            completion,
        })
    }

    async fn store_session(
        &self,
        process: Arc<RemoteExecProcess>,
        tty: bool,
        cursor: OutputCursor,
        lifecycle_tx: Option<mpsc::UnboundedSender<RemoteExecProcessLifecycleEvent>>,
    ) -> i32 {
        let (session_id, pruned_entry) = {
            let mut sessions = self.sessions.lock().await;
            let pruned = if sessions.len() >= MAX_REMOTE_EXEC_SESSIONS {
                sessions
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(id, _)| *id)
                    .and_then(|id| sessions.remove(&id).map(|entry| (id, entry)))
            } else {
                None
            };

            let session_id = new_session_id(&sessions);
            sessions.insert(
                session_id,
                RemoteExecSessionEntry {
                    process: Arc::clone(&process),
                    tty,
                    cursor,
                    last_used: Instant::now(),
                    lifecycle_tx: lifecycle_tx.clone(),
                },
            );
            (session_id, pruned)
        };

        if let Some((pruned_session_id, entry)) = pruned_entry {
            emit_lifecycle(
                entry.lifecycle_tx.clone(),
                RemoteExecProcessLifecycleEvent {
                    session_id: pruned_session_id,
                    status: RemoteExecProcessLifecycleStatus::Pruned,
                    exit_code: None,
                },
            );
            entry.process.request_control(RemoteExecControlAction::Kill);
        }

        emit_lifecycle(
            lifecycle_tx.clone(),
            RemoteExecProcessLifecycleEvent {
                session_id,
                status: RemoteExecProcessLifecycleStatus::Running,
                exit_code: None,
            },
        );
        spawn_lifecycle_exit_watcher(session_id, process, lifecycle_tx);

        session_id
    }

    async fn update_or_remove_session(
        &self,
        session_id: i32,
        process: &RemoteExecProcess,
        cursor: OutputCursor,
        lifecycle_status: Option<RemoteExecProcessLifecycleStatus>,
        exit_code: Option<i32>,
    ) {
        if process.output.is_closed().await {
            let mut sessions = self.sessions.lock().await;
            if let Some(entry) = sessions.remove(&session_id) {
                if let Some(status) = lifecycle_status {
                    emit_lifecycle(
                        entry.lifecycle_tx.clone(),
                        RemoteExecProcessLifecycleEvent {
                            session_id,
                            status,
                            exit_code,
                        },
                    );
                }
            }
        } else {
            let mut sessions = self.sessions.lock().await;
            if let Some(entry) = sessions.get_mut(&session_id) {
                entry.cursor = cursor;
            }
        }
    }

    async fn store_completed_session(
        &self,
        session_id: i32,
        completed: CompletedRemoteExecSession,
    ) {
        let mut completed_sessions = self.completed_sessions.lock().await;
        if completed_sessions.len() >= MAX_COMPLETED_REMOTE_EXEC_SESSIONS {
            if let Some(oldest_session_id) = completed_sessions
                .iter()
                .min_by_key(|(_, session)| session.completed_at)
                .map(|(id, _)| *id)
            {
                completed_sessions.remove(&oldest_session_id);
            }
        }
        completed_sessions.insert(session_id, completed);
    }

    async fn take_completed_session(&self, session_id: i32) -> Option<CompletedRemoteExecSession> {
        self.completed_sessions.lock().await.remove(&session_id)
    }
}

impl Drop for RemoteExecProcess {
    fn drop(&mut self) {
        self.request_control(RemoteExecControlAction::Kill);
    }
}

impl RemoteExecProcess {
    fn mark_out_of_band_control(&self, action: RemoteExecControlAction) {
        if let Ok(mut out_of_band_action) = self.out_of_band_control_action.lock() {
            *out_of_band_action = Some(action);
        }
    }

    fn out_of_band_control_action(&self) -> Option<RemoteExecControlAction> {
        self.out_of_band_control_action
            .lock()
            .ok()
            .and_then(|action| *action)
    }

    fn request_control(&self, action: RemoteExecControlAction) {
        let _ = self
            .command_tx
            .try_send(RemoteExecProcessCommand::Control(action));
    }
}

async fn spawn_remote_process(
    request: RemoteExecCommandRequest,
) -> anyhow::Result<RemoteExecProcess> {
    if request.tty {
        if let Some(spec) = request
            .ssh_manager
            .local_container_exec_spec(&request.connection_id, &request.command, true)
            .await?
        {
            return spawn_local_container_pty_process(request, spec).await;
        }
        return spawn_remote_pty_process(request).await;
    }
    spawn_remote_pipe_process(request).await
}

async fn spawn_local_container_pty_process(
    request: RemoteExecCommandRequest,
    (executable, args): (String, Vec<String>),
) -> anyhow::Result<RemoteExecProcess> {
    let shell_config = ShellConfig {
        executable,
        args,
        env: HashMap::new(),
        cwd: None,
        login: false,
    };
    let process_id = u32::from_le_bytes(
        Uuid::new_v4().as_bytes()[..4]
            .try_into()
            .expect("UUID prefix is four bytes"),
    );
    let spawned = spawn_pty(
        process_id,
        &shell_config,
        ShellType::Custom("Docker".to_string()),
        80,
        24,
    )
    .map_err(|error| anyhow!("Failed to start local Docker PTY: {}", error))?;
    let output = Arc::new(OutputState::new(request.output_capture_tx.clone()));
    let (command_tx, mut command_rx) = mpsc::channel::<RemoteExecProcessCommand>(64);
    let owner_output = output.clone();
    let mut events = spawned.events;
    let writer = spawned.writer;
    let controller = spawned.controller;
    tokio::spawn(async move {
        let mut exit_code = None;
        loop {
            tokio::select! {
                biased;
                command = command_rx.recv() => {
                    match command {
                        Some(RemoteExecProcessCommand::Write(bytes)) => {
                            if writer.write(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Some(RemoteExecProcessCommand::Control(RemoteExecControlAction::Interrupt)) => {
                            let _ = writer.write(&[0x03]).await;
                        }
                        Some(RemoteExecProcessCommand::Control(RemoteExecControlAction::Kill)) | None => {
                            let _ = controller.shutdown(true).await;
                            exit_code = Some(137);
                            break;
                        }
                    }
                }
                event = events.recv() => {
                    match event {
                        Some(PtyEvent::Data(data)) => {
                            owner_output.push_chunk(OutputStream::Combined, data).await
                        }
                        Some(PtyEvent::Exit { exit_code: code }) => {
                            exit_code = code.map(|code| code as i32);
                            break;
                        }
                        None => break,
                        Some(_) => {}
                    }
                }
            }
        }
        owner_output.close(exit_code).await;
    });
    Ok(RemoteExecProcess {
        output,
        command_tx,
        out_of_band_control_action: StdMutex::new(None),
    })
}

async fn spawn_remote_pipe_process(
    request: RemoteExecCommandRequest,
) -> anyhow::Result<RemoteExecProcess> {
    let transport = request
        .ssh_manager
        .open_workspace_stdio(&request.connection_id, &request.command)
        .await?;
    let output = Arc::new(OutputState::new(request.output_capture_tx.clone()));
    let (command_tx, command_rx) = mpsc::channel::<RemoteExecProcessCommand>(8);
    tokio::spawn(workspace_pipe_owner(transport, command_rx, output.clone()));

    Ok(RemoteExecProcess {
        output,
        command_tx,
        out_of_band_control_action: StdMutex::new(None),
    })
}

async fn spawn_remote_pty_process(
    request: RemoteExecCommandRequest,
) -> anyhow::Result<RemoteExecProcess> {
    let channel = request
        .ssh_manager
        .open_pty_exec_channel(&request.connection_id, &request.command, 80, 24)
        .await?;
    let output = Arc::new(OutputState::new(request.output_capture_tx.clone()));
    let (command_tx, command_rx) = mpsc::channel::<RemoteExecProcessCommand>(64);
    tokio::spawn(remote_pty_owner(channel, command_rx, output.clone()));

    Ok(RemoteExecProcess {
        output,
        command_tx,
        out_of_band_control_action: StdMutex::new(None),
    })
}

async fn workspace_pipe_owner(
    transport: crate::remote_ssh::WorkspaceStdio,
    mut command_rx: mpsc::Receiver<RemoteExecProcessCommand>,
    output: Arc<OutputState>,
) {
    let (mut stdin, mut stdout, mut stderr, control, completion) = transport.into_parts();
    let mut completion_task = tokio::spawn(completion.wait());
    let mut exit_code = None;
    let mut control_state: Option<RemotePipeControlState> = None;
    let mut stdout_closed = false;
    let mut stderr_closed = false;
    let mut stdout_buffer = vec![0u8; 16 * 1024];
    let mut stderr_buffer = vec![0u8; 16 * 1024];

    loop {
        if exit_code.is_some() && stdout_closed && stderr_closed {
            break;
        }
        if let Some(state) = control_state {
            if Instant::now() >= state.deadline() {
                match state {
                    RemotePipeControlState::InterruptGrace { .. } => {
                        let _ = control.kill().await;
                        control_state = Some(RemotePipeControlState::KillDrain {
                            deadline: Instant::now()
                                + Duration::from_millis(REMOTE_CONTROL_DRAIN_TIMEOUT_MS),
                        });
                    }
                    RemotePipeControlState::KillDrain { .. } => {
                        break;
                    }
                }
            }
        }

        let wait_budget = control_state
            .map(RemotePipeControlState::deadline)
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .filter(|duration| !duration.is_zero())
            .unwrap_or_else(|| Duration::from_millis(100));

        tokio::select! {
            biased;

            command = command_rx.recv() => {
                match command {
                    Some(RemoteExecProcessCommand::Write(bytes)) => {
                        if stdin.write_all(&bytes).await.is_err() {
                            break;
                        }
                        let _ = stdin.flush().await;
                    }
                    Some(RemoteExecProcessCommand::Control(RemoteExecControlAction::Interrupt)) => {
                        let _ = control.interrupt().await;
                        control_state = Some(RemotePipeControlState::InterruptGrace {
                            deadline: Instant::now()
                                + Duration::from_millis(REMOTE_INTERRUPT_GRACE_TIMEOUT_MS),
                        });
                    }
                    Some(RemoteExecProcessCommand::Control(RemoteExecControlAction::Kill)) => {
                        let _ = control.kill().await;
                        control_state = Some(RemotePipeControlState::KillDrain {
                            deadline: Instant::now()
                                + Duration::from_millis(REMOTE_CONTROL_DRAIN_TIMEOUT_MS),
                        });
                    }
                    None => {
                        let _ = control.kill().await;
                        control_state = Some(RemotePipeControlState::KillDrain {
                            deadline: Instant::now()
                                + Duration::from_millis(REMOTE_CONTROL_DRAIN_TIMEOUT_MS),
                        });
                    }
                }
            }

            read = stdout.read(&mut stdout_buffer), if !stdout_closed => {
                match read {
                    Ok(0) | Err(_) => stdout_closed = true,
                    Ok(read) => {
                        output
                            .push_chunk(OutputStream::Stdout, stdout_buffer[..read].to_vec())
                            .await
                    }
                }
            }

            read = stderr.read(&mut stderr_buffer), if !stderr_closed => {
                match read {
                    Ok(0) | Err(_) => stderr_closed = true,
                    Ok(read) => {
                        output
                            .push_chunk(OutputStream::Stderr, stderr_buffer[..read].to_vec())
                            .await
                    }
                }
            }

            completed = &mut completion_task, if exit_code.is_none() => {
                exit_code = completed.ok().and_then(|exit| exit.exit_code);
                if exit_code.is_none() {
                    exit_code = Some(-1);
                }
            }

            _ = tokio::time::sleep(wait_budget), if control_state.is_some() => {}
        }
    }

    let _ = stdin.shutdown().await;
    if !completion_task.is_finished() {
        completion_task.abort();
    }
    output.close(exit_code).await;
}

async fn remote_pty_owner(
    mut channel: Channel<Msg>,
    mut command_rx: mpsc::Receiver<RemoteExecProcessCommand>,
    output: Arc<OutputState>,
) {
    let mut exit_code = None;
    let mut close_after_control_at: Option<Instant> = None;

    loop {
        if close_after_control_at.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = channel.close().await;
            break;
        }

        let wait_budget = close_after_control_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .filter(|duration| !duration.is_zero())
            .unwrap_or_else(|| Duration::from_millis(100));

        tokio::select! {
            biased;

            command = command_rx.recv() => {
                match command {
                    Some(RemoteExecProcessCommand::Write(bytes)) => {
                        let _ = channel.data(&bytes[..]).await;
                    }
                    Some(RemoteExecProcessCommand::Control(RemoteExecControlAction::Interrupt)) => {
                        let _ = channel.data(&[0x03u8][..]).await;
                    }
                    Some(RemoteExecProcessCommand::Control(RemoteExecControlAction::Kill)) => {
                        let _ = channel.signal(Sig::KILL).await;
                        let _ = channel.eof().await;
                        close_after_control_at = Some(
                            Instant::now() + Duration::from_millis(REMOTE_CONTROL_DRAIN_TIMEOUT_MS)
                        );
                    }
                    None => {
                        let _ = channel.signal(Sig::KILL).await;
                        let _ = channel.close().await;
                        break;
                    }
                }
            }

            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        output
                            .push_chunk(OutputStream::Combined, data.to_vec())
                            .await;
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit_code = Some(exit_status as i32);
                    }
                    Some(ChannelMsg::ExitSignal { signal_name, .. }) => {
                        exit_code = Some(match signal_name {
                            Sig::INT => 130,
                            Sig::KILL => 137,
                            Sig::TERM => 143,
                            _ => -1,
                        });
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                }
            }

            _ = tokio::time::sleep(wait_budget), if close_after_control_at.is_some() => {}
        }
    }

    output.close(exit_code).await;
}

/// Incrementally decode a UTF-8 byte stream without treating a code point split
/// across transport chunks as invalid. Truly invalid sequences keep the
/// previous lossy-decoding contract.
fn decode_utf8_stream(pending: &mut Vec<u8>, chunk: &[u8], finish: bool) -> String {
    pending.extend_from_slice(chunk);
    let mut output = String::new();

    loop {
        match std::str::from_utf8(pending) {
            Ok(valid) => {
                output.push_str(valid);
                pending.clear();
                break;
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    output.push_str(
                        std::str::from_utf8(&pending[..valid_up_to])
                            .expect("UTF-8 validator reported a valid prefix"),
                    );
                    pending.drain(..valid_up_to);
                }
                match error.error_len() {
                    Some(invalid_len) => {
                        output.push('\u{fffd}');
                        pending.drain(..invalid_len);
                    }
                    None => break,
                }
            }
        }
    }

    if finish && !pending.is_empty() {
        output.push_str(&String::from_utf8_lossy(pending));
        pending.clear();
    }
    output
}

impl OutputState {
    fn new(output_capture_tx: Option<mpsc::UnboundedSender<String>>) -> Self {
        Self {
            inner: Mutex::new(OutputInner {
                chunks: VecDeque::new(),
                next_seq: 0,
                retained_bytes: 0,
                capture_pending_utf8: PendingUtf8Streams::default(),
                closed: false,
                exit_code: None,
            }),
            notify: Notify::new(),
            output_capture_tx,
        }
    }

    async fn push_chunk(&self, stream: OutputStream, chunk: Vec<u8>) {
        if chunk.is_empty() {
            return;
        }
        let capture_text = {
            let mut inner = self.inner.lock().await;
            let capture_text = self.output_capture_tx.as_ref().map(|_| {
                decode_utf8_stream(inner.capture_pending_utf8.get_mut(stream), &chunk, false)
            });
            let seq = inner.next_seq;
            inner.next_seq = inner.next_seq.saturating_add(1);
            inner.retained_bytes = inner.retained_bytes.saturating_add(chunk.len());
            inner.chunks.push_back((seq, stream, chunk));
            while inner.retained_bytes > MAX_RETAINED_OUTPUT_BYTES {
                if let Some((_, _, dropped)) = inner.chunks.pop_front() {
                    inner.retained_bytes = inner.retained_bytes.saturating_sub(dropped.len());
                } else {
                    break;
                }
            }
            capture_text
        };
        if let (Some(tx), Some(text)) = (&self.output_capture_tx, capture_text) {
            if !text.is_empty() {
                let _ = tx.send(text);
            }
        }
        self.notify.notify_waiters();
    }

    async fn close(&self, exit_code: Option<i32>) {
        let capture_tail = {
            let mut inner = self.inner.lock().await;
            inner.closed = true;
            inner.exit_code = exit_code;
            inner.capture_pending_utf8.finish()
        };
        if let Some(tx) = &self.output_capture_tx {
            if !capture_tail.is_empty() {
                let _ = tx.send(capture_tail);
            }
        }
        self.notify.notify_waiters();
    }

    async fn is_closed(&self) -> bool {
        self.inner.lock().await.closed
    }

    async fn exit_code(&self) -> Option<i32> {
        self.inner.lock().await.exit_code
    }

    async fn wait_closed(&self) -> Option<i32> {
        loop {
            let notified = self.notify.notified();
            {
                let inner = self.inner.lock().await;
                if inner.closed {
                    return inner.exit_code;
                }
            }
            notified.await;
        }
    }

    async fn drain_since_with_output(
        &self,
        cursor: &mut OutputCursor,
        sink: &mut HeadTailText,
        output_tx: Option<&mpsc::Sender<String>>,
    ) -> bool {
        let inner = self.inner.lock().await;
        if let Some((first_seq, _, _)) = inner.chunks.front() {
            if cursor.next_seq < *first_seq {
                // Retention pruning may discard the beginning of a split code
                // point. Never combine that orphaned suffix with later bytes.
                cursor.pending_utf8.clear();
                cursor.next_seq = *first_seq;
            }
        }
        for (seq, stream, chunk) in inner.chunks.iter() {
            if *seq >= cursor.next_seq {
                let text = decode_utf8_stream(cursor.pending_utf8.get_mut(*stream), chunk, false);
                if !text.is_empty() {
                    sink.push_str(&text);
                    if let Some(tx) = output_tx {
                        let _ = tx.try_send(text);
                    }
                }
            }
        }
        cursor.next_seq = inner.next_seq;
        if inner.closed {
            let tail = cursor.pending_utf8.finish();
            if !tail.is_empty() {
                sink.push_str(&tail);
                if let Some(tx) = output_tx {
                    let _ = tx.try_send(tail);
                }
            }
        }
        inner.closed
    }

    async fn collect_until(
        &self,
        mut cursor: OutputCursor,
        deadline: Instant,
        max_output_chars: usize,
        output_tx: Option<&mpsc::Sender<String>>,
    ) -> CollectedOutput {
        let mut sink = HeadTailText::new(max_output_chars);

        loop {
            let closed = self
                .drain_since_with_output(&mut cursor, &mut sink, output_tx)
                .await;
            if closed || Instant::now() >= deadline {
                break;
            }

            tokio::select! {
                _ = self.notify.notified() => {}
                _ = tokio::time::sleep_until(deadline) => break,
            }
        }

        let original_output_chars = sink.total_chars;
        CollectedOutput {
            output: sink.render(),
            original_output_chars,
            cursor,
        }
    }
}

fn emit_lifecycle(
    lifecycle_tx: Option<mpsc::UnboundedSender<RemoteExecProcessLifecycleEvent>>,
    event: RemoteExecProcessLifecycleEvent,
) {
    if let Some(tx) = lifecycle_tx {
        let _ = tx.send(event);
    }
}

fn completion_status_for_control_action(
    action: RemoteExecControlAction,
) -> RemoteExecSessionCompletionStatus {
    match action {
        RemoteExecControlAction::Interrupt => RemoteExecSessionCompletionStatus::Interrupted,
        RemoteExecControlAction::Kill => RemoteExecSessionCompletionStatus::Killed,
    }
}

fn completion_for_closed_remote_process(
    out_of_band_control_action: Option<RemoteExecControlAction>,
) -> RemoteExecSessionCompletion {
    if let Some(action) = out_of_band_control_action {
        return RemoteExecSessionCompletion {
            status: completion_status_for_control_action(action),
            source: RemoteExecSessionCompletionSource::OutOfBandControl,
        };
    }

    RemoteExecSessionCompletion {
        status: RemoteExecSessionCompletionStatus::Exited,
        source: RemoteExecSessionCompletionSource::Process,
    }
}

fn lifecycle_status_for_completion(
    status: RemoteExecSessionCompletionStatus,
) -> RemoteExecProcessLifecycleStatus {
    match status {
        RemoteExecSessionCompletionStatus::Exited => RemoteExecProcessLifecycleStatus::Exited,
        RemoteExecSessionCompletionStatus::Interrupted => {
            RemoteExecProcessLifecycleStatus::Interrupted
        }
        RemoteExecSessionCompletionStatus::Killed => RemoteExecProcessLifecycleStatus::Killed,
        RemoteExecSessionCompletionStatus::Pruned => RemoteExecProcessLifecycleStatus::Pruned,
    }
}

fn spawn_lifecycle_exit_watcher(
    session_id: i32,
    process: Arc<RemoteExecProcess>,
    lifecycle_tx: Option<mpsc::UnboundedSender<RemoteExecProcessLifecycleEvent>>,
) {
    if lifecycle_tx.is_none() {
        return;
    }

    tokio::spawn(async move {
        let exit_code = process.output.wait_closed().await;
        let completion = completion_for_closed_remote_process(process.out_of_band_control_action());
        emit_lifecycle(
            lifecycle_tx,
            RemoteExecProcessLifecycleEvent {
                session_id,
                status: lifecycle_status_for_completion(completion.status),
                exit_code,
            },
        );
    });
}

impl HeadTailText {
    fn new(max_chars: usize) -> Self {
        let head_budget = max_chars / 2;
        let tail_budget = max_chars.saturating_sub(head_budget);
        Self {
            head_budget,
            tail_budget,
            head: String::new(),
            tail: VecDeque::new(),
            head_chars: 0,
            tail_chars: 0,
            omitted_chars: 0,
            total_chars: 0,
        }
    }

    fn push_str(&mut self, text: &str) {
        for ch in text.chars() {
            self.total_chars += 1;
            if self.head_chars < self.head_budget {
                self.head.push(ch);
                self.head_chars += 1;
                continue;
            }

            self.tail.push_back(ch);
            self.tail_chars += 1;
            if self.tail_chars > self.tail_budget {
                self.tail.pop_front();
                self.tail_chars -= 1;
                self.omitted_chars = self.omitted_chars.saturating_add(1);
            }
        }
    }

    fn render(self) -> String {
        if self.omitted_chars == 0 {
            let mut output = self.head;
            output.extend(self.tail);
            return output;
        }

        let mut output = self.head;
        output.push_str("\n... [truncated, middle omitted] ...\n");
        output.extend(self.tail);
        output
    }
}

fn deadline_from_now(yield_time_ms: Option<u64>) -> Instant {
    Instant::now() + Duration::from_millis(yield_time_ms.unwrap_or(DEFAULT_YIELD_TIME_MS))
}

fn input_bytes_for_write(chars: &str, append_enter: bool) -> Vec<u8> {
    let mut bytes = chars.as_bytes().to_vec();
    if append_enter {
        bytes.push(b'\n');
    }
    bytes
}

fn new_session_id(sessions: &HashMap<i32, RemoteExecSessionEntry>) -> i32 {
    loop {
        let session_id = if cfg!(test) {
            sessions
                .keys()
                .copied()
                .max()
                .map(|max| std::cmp::max(max, 999) + 1)
                .unwrap_or(1000)
        } else {
            rand::thread_rng().gen_range(1_000..100_000)
        };

        if !sessions.contains_key(&session_id) {
            return session_id;
        }
    }
}

fn new_chunk_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        decode_utf8_stream, new_session_id, HeadTailText, OutputStream, PendingUtf8Streams,
    };
    use std::collections::HashMap;

    #[test]
    fn remote_exec_session_ids_match_local_test_baseline() {
        let sessions = HashMap::new();

        assert_eq!(new_session_id(&sessions), 1000);
    }

    #[test]
    fn head_tail_text_keeps_full_output_when_unbounded() {
        let mut buffer = HeadTailText::new(usize::MAX);
        buffer.push_str("abcdefghijklmnop");

        assert_eq!(buffer.total_chars, 16);
        assert_eq!(buffer.render(), "abcdefghijklmnop");
    }

    #[test]
    fn utf8_stream_decoder_preserves_code_points_split_across_chunks() {
        let bytes = "路径/文件.txt".as_bytes();
        let mut pending = Vec::new();
        let mut decoded = String::new();

        for byte in bytes {
            decoded.push_str(&decode_utf8_stream(&mut pending, &[*byte], false));
        }
        decoded.push_str(&decode_utf8_stream(&mut pending, &[], true));

        assert_eq!(decoded, "路径/文件.txt");
        assert!(pending.is_empty());
    }

    #[test]
    fn utf8_stream_decoder_keeps_stdout_and_stderr_boundaries_independent() {
        let chinese = "路".as_bytes();
        let mut pending = PendingUtf8Streams::default();

        assert_eq!(
            decode_utf8_stream(pending.get_mut(OutputStream::Stdout), &chinese[..1], false),
            ""
        );
        assert_eq!(
            decode_utf8_stream(pending.get_mut(OutputStream::Stderr), b"error", false),
            "error"
        );
        assert_eq!(
            decode_utf8_stream(pending.get_mut(OutputStream::Stdout), &chinese[1..], false),
            "路"
        );
    }
}
