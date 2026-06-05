//! Model-facing command execution runtime.
//!
//! This runtime is intentionally separate from terminal sessions. Each
//! `exec_command` starts a fresh local process; a session id is only retained
//! while that process is still running so later calls can poll or write stdin.

use crate::{TerminalError, TerminalResult};
use chardetng::EncodingDetector;
use encoding_rs::{Encoding, IBM866, WINDOWS_1252};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize, SlavePty};
use rand::Rng;
use std::collections::{HashMap, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinHandle;
use uuid::Uuid;

const DEFAULT_YIELD_TIME_MS: u64 = 10_000;
const DEFAULT_MAX_OUTPUT_CHARS: usize = 10_000;
const MAX_RETAINED_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_EXEC_SESSIONS: usize = 64;
#[cfg(unix)]
const PIPE_INTERRUPT_GRACE_TIMEOUT_MS: u64 = 2_000;
const PTY_EXIT_DRAIN_TIMEOUT_MS: u64 = 500;

static GLOBAL_EXEC_MANAGER: OnceLock<Arc<ExecProcessManager>> = OnceLock::new();

pub fn get_global_exec_process_manager() -> Arc<ExecProcessManager> {
    GLOBAL_EXEC_MANAGER
        .get_or_init(|| Arc::new(ExecProcessManager::default()))
        .clone()
}

#[derive(Debug, Clone)]
pub struct ExecCommandRequest {
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub tty: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct WriteStdinRequest {
    pub session_id: i32,
    pub chars: String,
    pub append_enter: bool,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecControlAction {
    Interrupt,
    Kill,
}

#[derive(Debug, Clone)]
pub struct ExecControlRequest {
    pub session_id: i32,
    pub action: ExecControlAction,
    pub yield_time_ms: Option<u64>,
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ExecCommandResponse {
    pub chunk_id: String,
    pub wall_time_seconds: f64,
    pub output: String,
    pub session_id: Option<i32>,
    pub exit_code: Option<i32>,
    pub original_output_chars: usize,
}

pub struct ExecProcessManager {
    sessions: Mutex<HashMap<i32, ExecSessionEntry>>,
}

impl Default for ExecProcessManager {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

struct ExecSessionEntry {
    process: Arc<ExecProcess>,
    tty: bool,
    cursor: OutputCursor,
    last_used: tokio::time::Instant,
}

struct ExecProcess {
    output: Arc<OutputState>,
    writer: Option<mpsc::Sender<Vec<u8>>>,
    terminator: StdMutex<Option<Terminator>>,
    helper_tasks: StdMutex<Vec<JoinHandle<()>>>,
    pty_handles: Arc<StdMutex<Option<PtyKeepAlive>>>,
}

enum Terminator {
    Pty(Box<dyn portable_pty::ChildKiller + Send + Sync>),
    Pipe(mpsc::Sender<ExecControlAction>),
}

struct PtyKeepAlive {
    _master: Box<dyn MasterPty + Send>,
    _slave: Option<Box<dyn SlavePty + Send>>,
}

struct OutputState {
    inner: Mutex<OutputInner>,
    notify: Notify,
}

struct OutputInner {
    chunks: VecDeque<(u64, Vec<u8>)>,
    next_seq: u64,
    retained_bytes: usize,
    closed: bool,
    exit_code: Option<i32>,
}

#[derive(Clone)]
struct OutputCursor {
    next_seq: u64,
}

struct HeadTailText {
    max_chars: usize,
    head_budget: usize,
    tail_budget: usize,
    head: String,
    tail: VecDeque<char>,
    head_chars: usize,
    tail_chars: usize,
    omitted_chars: usize,
    total_chars: usize,
}

impl ExecProcessManager {
    pub async fn exec_command(
        &self,
        request: ExecCommandRequest,
    ) -> TerminalResult<ExecCommandResponse> {
        self.exec_command_inner(request, None).await
    }

    pub async fn exec_command_streaming(
        &self,
        request: ExecCommandRequest,
        output_tx: mpsc::Sender<String>,
    ) -> TerminalResult<ExecCommandResponse> {
        self.exec_command_inner(request, Some(output_tx)).await
    }

    async fn exec_command_inner(
        &self,
        request: ExecCommandRequest,
        output_tx: Option<mpsc::Sender<String>>,
    ) -> TerminalResult<ExecCommandResponse> {
        let process = Arc::new(spawn_exec_process(&request).await?);
        let cursor = OutputCursor { next_seq: 0 };
        let started_at = tokio::time::Instant::now();
        let collected = process
            .output
            .collect_until(
                cursor,
                deadline_from_now(request.yield_time_ms),
                request.max_output_chars.unwrap_or(DEFAULT_MAX_OUTPUT_CHARS),
                output_tx.as_ref(),
            )
            .await;

        let exit_code = process.output.exit_code().await;
        let session_id = if process.output.is_closed().await {
            None
        } else {
            let session_id = self
                .store_session(process, request.tty, collected.cursor.clone())
                .await;
            Some(session_id)
        };

        Ok(ExecCommandResponse {
            chunk_id: new_chunk_id(),
            wall_time_seconds: started_at.elapsed().as_secs_f64(),
            output: collected.output,
            session_id,
            exit_code,
            original_output_chars: collected.original_output_chars,
        })
    }

    pub async fn write_stdin(
        &self,
        request: WriteStdinRequest,
    ) -> TerminalResult<ExecCommandResponse> {
        self.write_stdin_inner(request, None).await
    }

    pub async fn write_stdin_streaming(
        &self,
        request: WriteStdinRequest,
        output_tx: mpsc::Sender<String>,
    ) -> TerminalResult<ExecCommandResponse> {
        self.write_stdin_inner(request, Some(output_tx)).await
    }

    async fn write_stdin_inner(
        &self,
        request: WriteStdinRequest,
        output_tx: Option<mpsc::Sender<String>>,
    ) -> TerminalResult<ExecCommandResponse> {
        let (process, tty, cursor) = {
            let mut sessions = self.sessions.lock().await;
            let entry = sessions
                .get_mut(&request.session_id)
                .ok_or_else(|| TerminalError::SessionNotFound(request.session_id.to_string()))?;
            entry.last_used = tokio::time::Instant::now();
            (Arc::clone(&entry.process), entry.tty, entry.cursor.clone())
        };

        let input = input_bytes_for_write(&request.chars, request.append_enter);
        if !input.is_empty() && tty {
            let writer = process
                .writer
                .as_ref()
                .ok_or(TerminalError::ProcessNotRunning)?;
            writer
                .send(input)
                .await
                .map_err(|_| TerminalError::ProcessNotRunning)?;
        }

        let started_at = tokio::time::Instant::now();
        let collected = process
            .output
            .collect_until(
                cursor,
                deadline_from_now(request.yield_time_ms),
                request.max_output_chars.unwrap_or(DEFAULT_MAX_OUTPUT_CHARS),
                output_tx.as_ref(),
            )
            .await;

        let closed = process.output.is_closed().await;
        let exit_code = process.output.exit_code().await;
        if closed {
            self.remove_session(request.session_id).await;
        } else {
            let mut sessions = self.sessions.lock().await;
            if let Some(entry) = sessions.get_mut(&request.session_id) {
                entry.cursor = collected.cursor.clone();
            }
        }

        Ok(ExecCommandResponse {
            chunk_id: new_chunk_id(),
            wall_time_seconds: started_at.elapsed().as_secs_f64(),
            output: collected.output,
            session_id: (!closed).then_some(request.session_id),
            exit_code,
            original_output_chars: collected.original_output_chars,
        })
    }

    pub async fn control_session(
        &self,
        request: ExecControlRequest,
    ) -> TerminalResult<ExecCommandResponse> {
        let (process, tty, cursor) = {
            let mut sessions = self.sessions.lock().await;
            let entry = sessions
                .get_mut(&request.session_id)
                .ok_or_else(|| TerminalError::SessionNotFound(request.session_id.to_string()))?;
            entry.last_used = tokio::time::Instant::now();
            (Arc::clone(&entry.process), entry.tty, entry.cursor.clone())
        };

        match request.action {
            ExecControlAction::Interrupt if tty => {
                process.write_input_bytes(vec![0x03]).await?;
            }
            ExecControlAction::Interrupt | ExecControlAction::Kill => {
                process.request_control(request.action);
            }
        }

        let started_at = tokio::time::Instant::now();
        let collected = process
            .output
            .collect_until(
                cursor,
                deadline_from_now(request.yield_time_ms),
                request.max_output_chars.unwrap_or(DEFAULT_MAX_OUTPUT_CHARS),
                None,
            )
            .await;

        let closed = process.output.is_closed().await;
        let exit_code = process.output.exit_code().await;
        if closed {
            self.remove_session(request.session_id).await;
        } else {
            let mut sessions = self.sessions.lock().await;
            if let Some(entry) = sessions.get_mut(&request.session_id) {
                entry.cursor = collected.cursor.clone();
            }
        }

        Ok(ExecCommandResponse {
            chunk_id: new_chunk_id(),
            wall_time_seconds: started_at.elapsed().as_secs_f64(),
            output: collected.output,
            session_id: (!closed).then_some(request.session_id),
            exit_code,
            original_output_chars: collected.original_output_chars,
        })
    }

    async fn store_session(
        &self,
        process: Arc<ExecProcess>,
        tty: bool,
        cursor: OutputCursor,
    ) -> i32 {
        let pruned = {
            let mut sessions = self.sessions.lock().await;
            let pruned = if sessions.len() >= MAX_EXEC_SESSIONS {
                sessions
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(id, _)| id.clone())
                    .and_then(|id| sessions.remove(&id))
            } else {
                None
            };
            let session_id = new_session_id(&sessions);
            sessions.insert(
                session_id,
                ExecSessionEntry {
                    process,
                    tty,
                    cursor,
                    last_used: tokio::time::Instant::now(),
                },
            );
            (session_id, pruned)
        };

        if let Some(entry) = pruned.1 {
            entry.process.terminate();
        }

        pruned.0
    }

    async fn remove_session(&self, session_id: i32) {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(&session_id);
    }
}

impl Drop for ExecProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

impl ExecProcess {
    async fn write_input_bytes(&self, bytes: Vec<u8>) -> TerminalResult<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let writer = self
            .writer
            .as_ref()
            .ok_or(TerminalError::ProcessNotRunning)?;
        writer
            .send(bytes)
            .await
            .map_err(|_| TerminalError::ProcessNotRunning)
    }

    fn request_control(&self, action: ExecControlAction) {
        if let Ok(mut terminator) = self.terminator.lock() {
            if let Some(terminator) = terminator.take() {
                match terminator {
                    Terminator::Pty(mut killer) => {
                        let _ = killer.kill();
                    }
                    Terminator::Pipe(tx) => {
                        let _ = tx.try_send(action);
                    }
                }
            }
        }
    }

    fn request_terminate(&self) {
        self.request_control(ExecControlAction::Kill);
    }

    fn terminate(&self) {
        self.request_terminate();

        if let Ok(mut tasks) = self.helper_tasks.lock() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }

        if let Ok(mut handles) = self.pty_handles.lock() {
            handles.take();
        }
    }
}

impl OutputState {
    fn new() -> Self {
        Self {
            inner: Mutex::new(OutputInner {
                chunks: VecDeque::new(),
                next_seq: 0,
                retained_bytes: 0,
                closed: false,
                exit_code: None,
            }),
            notify: Notify::new(),
        }
    }

    async fn push_chunk(&self, chunk: Vec<u8>) {
        if chunk.is_empty() {
            return;
        }
        {
            let mut inner = self.inner.lock().await;
            let seq = inner.next_seq;
            inner.next_seq = inner.next_seq.saturating_add(1);
            inner.retained_bytes = inner.retained_bytes.saturating_add(chunk.len());
            inner.chunks.push_back((seq, chunk));
            while inner.retained_bytes > MAX_RETAINED_OUTPUT_BYTES {
                if let Some((_, dropped)) = inner.chunks.pop_front() {
                    inner.retained_bytes = inner.retained_bytes.saturating_sub(dropped.len());
                } else {
                    break;
                }
            }
        }
        self.notify.notify_waiters();
    }

    async fn close(&self, exit_code: Option<i32>) {
        {
            let mut inner = self.inner.lock().await;
            inner.closed = true;
            inner.exit_code = exit_code;
        }
        self.notify.notify_waiters();
    }

    async fn is_closed(&self) -> bool {
        self.inner.lock().await.closed
    }

    async fn exit_code(&self) -> Option<i32> {
        self.inner.lock().await.exit_code
    }

    async fn drain_since_with_output(
        &self,
        cursor: &mut OutputCursor,
        sink: &mut HeadTailText,
        output_tx: Option<&mpsc::Sender<String>>,
    ) -> bool {
        let inner = self.inner.lock().await;
        for (seq, chunk) in inner.chunks.iter() {
            if *seq >= cursor.next_seq {
                let text = bytes_to_string_smart(chunk);
                sink.push_str(&text);
                if let Some(tx) = output_tx {
                    let _ = tx.try_send(text);
                }
            }
        }
        cursor.next_seq = inner.next_seq;
        inner.closed
    }

    async fn collect_until(
        &self,
        mut cursor: OutputCursor,
        deadline: tokio::time::Instant,
        max_output_chars: usize,
        output_tx: Option<&mpsc::Sender<String>>,
    ) -> CollectedOutput {
        let mut sink = HeadTailText::new(max_output_chars);

        loop {
            let closed = self
                .drain_since_with_output(&mut cursor, &mut sink, output_tx)
                .await;
            if closed || tokio::time::Instant::now() >= deadline {
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

struct CollectedOutput {
    output: String,
    original_output_chars: usize,
    cursor: OutputCursor,
}

impl HeadTailText {
    fn new(max_chars: usize) -> Self {
        let head_budget = max_chars / 2;
        let tail_budget = max_chars.saturating_sub(head_budget);
        Self {
            max_chars,
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
            self.total_chars = self.total_chars.saturating_add(1);
            if self.max_chars == 0 {
                self.omitted_chars = self.omitted_chars.saturating_add(1);
                continue;
            }
            if self.head_chars < self.head_budget {
                self.head.push(ch);
                self.head_chars += 1;
                continue;
            }

            if self.tail_budget == 0 {
                self.omitted_chars = self.omitted_chars.saturating_add(1);
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

async fn spawn_exec_process(request: &ExecCommandRequest) -> TerminalResult<ExecProcess> {
    if request.argv.is_empty() || request.argv[0].is_empty() {
        return Err(TerminalError::InvalidConfig(
            "missing command executable".to_string(),
        ));
    }
    if !request.cwd.is_dir() {
        return Err(TerminalError::InvalidConfig(format!(
            "working directory does not exist: {}",
            request.cwd.display()
        )));
    }

    if request.tty {
        spawn_pty_process(request).await
    } else {
        spawn_pipe_process(request).await
    }
}

async fn spawn_pty_process(request: &ExecCommandRequest) -> TerminalResult<ExecProcess> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut command = CommandBuilder::new(&request.argv[0]);
    command.cwd(&request.cwd);
    apply_sanitized_environment_to_pty(&mut command, &request.env);
    for arg in request.argv.iter().skip(1) {
        command.arg(arg);
    }

    let mut child = pair.slave.spawn_command(command)?;
    let killer = child.clone_killer();
    let output = Arc::new(OutputState::new());
    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    let writer = Arc::new(StdMutex::new(writer));
    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, mut output_rx) = mpsc::channel::<Vec<u8>>(128);

    let reader_task = tokio::task::spawn_blocking(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = buffer[..n].to_vec();
                    if output_tx.blocking_send(chunk).is_err() {
                        break;
                    }
                }
                Err(ref error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(ref error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    let output_task = tokio::spawn({
        let output = Arc::clone(&output);
        async move {
            while let Some(chunk) = output_rx.recv().await {
                output.push_chunk(chunk).await;
            }
        }
    });

    let writer_task = tokio::spawn({
        let writer = Arc::clone(&writer);
        async move {
            while let Some(bytes) = writer_rx.recv().await {
                if let Ok(mut guard) = writer.lock() {
                    let _ = guard.write_all(&bytes);
                    let _ = guard.flush();
                }
            }
        }
    });

    let wait_blocking = tokio::task::spawn_blocking(move || {
        child.wait().ok().map(|status| status.exit_code() as i32)
    });
    let wait_output = Arc::clone(&output);
    let pty_handles = Arc::new(StdMutex::new(Some(PtyKeepAlive {
        _master: pair.master,
        _slave: if cfg!(windows) {
            Some(pair.slave)
        } else {
            None
        },
    })));
    let close_pty_handles = Arc::clone(&pty_handles);
    let close_task = tokio::spawn(async move {
        let code = wait_blocking.await.ok().flatten();
        writer_task.abort();
        if let Ok(mut handles) = close_pty_handles.lock() {
            handles.take();
        }

        let mut reader_task = reader_task;
        if tokio::time::timeout(
            Duration::from_millis(PTY_EXIT_DRAIN_TIMEOUT_MS),
            &mut reader_task,
        )
        .await
        .is_err()
        {
            reader_task.abort();
        }

        let mut output_task = output_task;
        if tokio::time::timeout(
            Duration::from_millis(PTY_EXIT_DRAIN_TIMEOUT_MS),
            &mut output_task,
        )
        .await
        .is_err()
        {
            output_task.abort();
        }
        wait_output.close(code).await;
    });

    Ok(ExecProcess {
        output,
        writer: Some(writer_tx),
        terminator: StdMutex::new(Some(Terminator::Pty(killer))),
        helper_tasks: StdMutex::new(vec![close_task]),
        pty_handles,
    })
}

async fn spawn_pipe_process(request: &ExecCommandRequest) -> TerminalResult<ExecProcess> {
    let mut command = Command::new(&request.argv[0]);
    command.args(request.argv.iter().skip(1));
    command.current_dir(&request.cwd);
    command.env_clear();
    for (key, value) in sanitized_environment(&request.env) {
        command.env(key, value);
    }
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    configure_pipe_process_group(&mut command);
    command.kill_on_drop(true);

    let mut child = command.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let output = Arc::new(OutputState::new());

    let mut reader_tasks = Vec::new();
    if let Some(stdout) = stdout {
        reader_tasks.push(spawn_pipe_reader(stdout, Arc::clone(&output)));
    }
    if let Some(stderr) = stderr {
        reader_tasks.push(spawn_pipe_reader(stderr, Arc::clone(&output)));
    }

    let (control_tx, mut control_rx) = mpsc::channel::<ExecControlAction>(1);
    let wait_output = Arc::clone(&output);
    let wait_task = tokio::spawn(async move {
        let code = tokio::select! {
            status = child.wait() => status.ok().and_then(|status| status.code()),
            action = control_rx.recv() => {
                control_pipe_child(&mut child, action.unwrap_or(ExecControlAction::Kill)).await
            }
        };
        for task in reader_tasks {
            let _ = task.await;
        }
        wait_output.close(code).await;
    });

    Ok(ExecProcess {
        output,
        writer: None,
        terminator: StdMutex::new(Some(Terminator::Pipe(control_tx))),
        helper_tasks: StdMutex::new(vec![wait_task]),
        pty_handles: Arc::new(StdMutex::new(None)),
    })
}

#[cfg(unix)]
fn configure_pipe_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::EPERM) || libc::setpgid(0, 0) == -1 {
                    return Err(err);
                }
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_pipe_process_group(_command: &mut Command) {}

async fn control_pipe_child(
    child: &mut tokio::process::Child,
    action: ExecControlAction,
) -> Option<i32> {
    match action {
        ExecControlAction::Interrupt => interrupt_pipe_child(child).await,
        ExecControlAction::Kill => kill_pipe_child(child).await,
    }
}

#[cfg(windows)]
async fn interrupt_pipe_child(child: &mut tokio::process::Child) -> Option<i32> {
    kill_pipe_child(child).await
}

#[cfg(windows)]
async fn kill_pipe_child(child: &mut tokio::process::Child) -> Option<i32> {
    if let Some(pid) = child.id() {
        let pid = pid.to_string();
        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid, "/T", "/F"]);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let taskkill_result = command.status().await;
        if taskkill_result.is_ok_and(|status| status.success()) {
            return child.wait().await.ok().and_then(|status| status.code());
        }
    }

    let _ = child.kill().await;
    child.wait().await.ok().and_then(|status| status.code())
}

#[cfg(unix)]
async fn interrupt_pipe_child(child: &mut tokio::process::Child) -> Option<i32> {
    signal_pipe_process_group(child, libc::SIGINT);
    tokio::time::sleep(Duration::from_millis(PIPE_INTERRUPT_GRACE_TIMEOUT_MS)).await;
    signal_pipe_process_group(child, libc::SIGKILL);
    let _ = child.start_kill();
    child.wait().await.ok().and_then(|status| status.code())
}

#[cfg(unix)]
async fn kill_pipe_child(child: &mut tokio::process::Child) -> Option<i32> {
    signal_pipe_process_group(child, libc::SIGKILL);
    let _ = child.start_kill();
    child.wait().await.ok().and_then(|status| status.code())
}

#[cfg(unix)]
fn signal_pipe_process_group(child: &tokio::process::Child, signal: libc::c_int) {
    let Some(pid) = child.id() else {
        return;
    };
    let pgid = pid as libc::pid_t;
    unsafe {
        libc::killpg(pgid, signal);
    }
}

#[cfg(not(any(unix, windows)))]
async fn interrupt_pipe_child(child: &mut tokio::process::Child) -> Option<i32> {
    kill_pipe_child(child).await
}

#[cfg(not(any(unix, windows)))]
async fn kill_pipe_child(child: &mut tokio::process::Child) -> Option<i32> {
    let _ = child.kill().await;
    child.wait().await.ok().and_then(|status| status.code())
}

fn spawn_pipe_reader<R>(mut reader: R, output: Arc<OutputState>) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buffer = vec![0u8; 8192];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => output.push_chunk(buffer[..n].to_vec()).await,
                Err(ref error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    })
}

fn apply_sanitized_environment_to_pty(
    command: &mut CommandBuilder,
    overlay: &HashMap<String, String>,
) {
    command.env_clear();
    for (key, value) in sanitized_environment(overlay) {
        command.env(key, value);
    }
}

fn sanitized_environment(overlay: &HashMap<String, String>) -> HashMap<String, String> {
    let mut env = HashMap::new();
    for (key, value) in std::env::vars() {
        if !is_tauri_host_env(&key) {
            env.insert(key, value);
        }
    }
    for (key, value) in overlay {
        env.insert(key.clone(), value.clone());
    }
    env
}

fn is_tauri_host_env(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    key == "TAURI_CONFIG"
        || key.starts_with("TAURI_ENV_")
        || key.starts_with("TAURI_ANDROID_PACKAGE_NAME_")
}

fn deadline_from_now(yield_time_ms: Option<u64>) -> tokio::time::Instant {
    tokio::time::Instant::now()
        + Duration::from_millis(yield_time_ms.unwrap_or(DEFAULT_YIELD_TIME_MS))
}

fn new_session_id(sessions: &HashMap<i32, ExecSessionEntry>) -> i32 {
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

fn input_bytes_for_write(chars: &str, append_enter: bool) -> Vec<u8> {
    let mut bytes = chars.as_bytes().to_vec();
    if append_enter {
        #[cfg(windows)]
        bytes.push(b'\r');
        #[cfg(not(windows))]
        bytes.push(b'\n');
    }
    bytes
}

fn bytes_to_string_smart(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }

    decode_bytes(bytes, detect_encoding(bytes))
}

fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let (encoding, _is_confident) = detector.guess_assess(None, true);

    if encoding == IBM866 && looks_like_windows_1252_punctuation(bytes) {
        return WINDOWS_1252;
    }

    encoding
}

fn decode_bytes(bytes: &[u8], encoding: &'static Encoding) -> String {
    let (decoded, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        decoded.into_owned()
    }
}

const WINDOWS_1252_PUNCT_BYTES: [u8; 8] = [0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x99];

fn looks_like_windows_1252_punctuation(bytes: &[u8]) -> bool {
    let mut saw_extended_punctuation = false;
    let mut saw_ascii_word = false;

    for &byte in bytes {
        if byte >= 0xA0 {
            return false;
        }
        if (0x80..=0x9F).contains(&byte) {
            if !WINDOWS_1252_PUNCT_BYTES.contains(&byte) {
                return false;
            }
            saw_extended_punctuation = true;
        }
        if byte.is_ascii_alphabetic() {
            saw_ascii_word = true;
        }
    }

    saw_extended_punctuation && saw_ascii_word
}

#[cfg(test)]
mod tests {
    use super::{
        bytes_to_string_smart, input_bytes_for_write, ExecCommandRequest, ExecControlAction,
        ExecControlRequest, ExecProcessManager, HeadTailText, WriteStdinRequest,
    };
    #[cfg(windows)]
    use crate::shell::{ShellDetector, ShellType};
    use encoding_rs::GBK;
    use std::collections::HashMap;

    #[cfg(windows)]
    fn shell_argv(script: &str) -> Vec<String> {
        vec![
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()),
            "/C".to_string(),
            script.to_string(),
        ]
    }

    #[cfg(not(windows))]
    fn shell_argv(script: &str) -> Vec<String> {
        vec!["sh".to_string(), "-lc".to_string(), script.to_string()]
    }

    #[cfg(windows)]
    fn default_windows_shell_argv(script: &str) -> Vec<String> {
        let shell = ShellDetector::get_default_shell();
        match shell.shell_type {
            ShellType::PowerShell | ShellType::PowerShellCore => {
                vec![
                    shell.path.to_string_lossy().to_string(),
                    "-Command".to_string(),
                    script.to_string(),
                ]
            }
            ShellType::Cmd => {
                vec![
                    shell.path.to_string_lossy().to_string(),
                    "/C".to_string(),
                    script.to_string(),
                ]
            }
            _ => vec![
                shell.path.to_string_lossy().to_string(),
                "-lc".to_string(),
                script.to_string(),
            ],
        }
    }

    #[tokio::test]
    async fn pipe_exec_returns_output_and_exit_code() {
        let manager = ExecProcessManager::default();
        let response = manager
            .exec_command(ExecCommandRequest {
                argv: shell_argv("echo bitfun_exec_test"),
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: false,
                yield_time_ms: Some(5_000),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("exec command should run");

        assert_eq!(response.exit_code, Some(0));
        assert!(response.session_id.is_none());
        assert!(response.output.contains("bitfun_exec_test"));
    }

    #[tokio::test]
    async fn delayed_poll_returns_unread_output_after_process_exit() {
        let manager = ExecProcessManager::default();
        #[cfg(windows)]
        let script = "echo first & powershell -NoProfile -Command \"Start-Sleep -Milliseconds 250\" & echo second";
        #[cfg(not(windows))]
        let script = "echo first; sleep 0.25; echo second";

        let first = manager
            .exec_command(ExecCommandRequest {
                argv: shell_argv(script),
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: false,
                yield_time_ms: Some(100),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("exec command should start");

        let session_id = first
            .session_id
            .expect("process should still be running after first yield");
        assert!(first.output.contains("first"));

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let second = manager
            .write_stdin(WriteStdinRequest {
                session_id,
                chars: String::new(),
                append_enter: false,
                yield_time_ms: Some(100),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("poll should return unread output");

        assert_eq!(second.exit_code, Some(0));
        assert!(second.session_id.is_none());
        assert!(second.output.contains("second"));
    }

    #[tokio::test]
    async fn tty_poll_after_process_exit_returns_exit_code() {
        let manager = ExecProcessManager::default();
        #[cfg(windows)]
        let script = "echo tty_first & powershell -NoProfile -Command \"Start-Sleep -Milliseconds 250\" & echo tty_second";
        #[cfg(not(windows))]
        let script = "echo tty_first; sleep 0.25; echo tty_second";

        let first = manager
            .exec_command(ExecCommandRequest {
                argv: shell_argv(script),
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: true,
                yield_time_ms: Some(100),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("tty command should start");

        let session_id = first
            .session_id
            .expect("tty process should still be running after first yield");

        tokio::time::sleep(std::time::Duration::from_millis(900)).await;

        let second = manager
            .write_stdin(WriteStdinRequest {
                session_id,
                chars: String::new(),
                append_enter: false,
                yield_time_ms: Some(100),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("tty poll should return final process state");

        assert_eq!(second.exit_code, Some(0));
        assert!(second.session_id.is_none());
        assert!(second.output.contains("tty_second"));

        let missing = manager
            .write_stdin(WriteStdinRequest {
                session_id,
                chars: String::new(),
                append_enter: false,
                yield_time_ms: Some(100),
                max_output_chars: Some(10_000),
            })
            .await
            .expect_err("finished tty session should be removed");
        assert!(missing.to_string().contains("Session not found"));
    }

    #[tokio::test]
    async fn tty_append_enter_completes_line_input() {
        let manager = ExecProcessManager::default();
        let first = manager
            .exec_command(ExecCommandRequest {
                argv: vec![
                    "python".to_string(),
                    "-c".to_string(),
                    "s=input(); print('got:'+s)".to_string(),
                ],
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: true,
                yield_time_ms: Some(500),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("tty command should start");

        let session_id = first
            .session_id
            .expect("python input should still be waiting");

        let second = manager
            .write_stdin(WriteStdinRequest {
                session_id,
                chars: "hello".to_string(),
                append_enter: true,
                yield_time_ms: Some(5_000),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("append_enter should submit the line");

        assert_eq!(second.exit_code, Some(0));
        assert!(second.output.contains("got:hello"));
    }

    #[tokio::test]
    async fn control_kill_terminates_running_pipe_session() {
        let manager = ExecProcessManager::default();
        #[cfg(windows)]
        let script =
            "echo before_kill & powershell -NoProfile -Command \"Start-Sleep -Seconds 30\"";
        #[cfg(not(windows))]
        let script = "echo before_kill; sleep 30";

        let first = manager
            .exec_command(ExecCommandRequest {
                argv: shell_argv(script),
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: false,
                yield_time_ms: Some(100),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("exec command should start");

        let session_id = first
            .session_id
            .expect("long-running process should still be active");
        let second = manager
            .control_session(ExecControlRequest {
                session_id,
                action: ExecControlAction::Kill,
                yield_time_ms: Some(5_000),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("kill should return process state");

        assert!(second.session_id.is_none());
        assert!(second.exit_code.is_some());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn control_interrupt_kills_running_pipe_process_group_after_grace() {
        let manager = ExecProcessManager::default();
        let unique = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        );
        let pid_file = std::env::temp_dir().join(format!("bitfun-exec-child-{unique}.pid"));
        let tick_file = std::env::temp_dir().join(format!("bitfun-exec-child-{unique}.tick"));
        let pid_path = pid_file.to_string_lossy();
        let tick_path = tick_file.to_string_lossy();
        let script = format!(
            "rm -f '{pid_path}' '{tick_path}'; \
             (trap '' INT TERM HUP; echo $$ > '{pid_path}'; i=0; while :; do echo $i >> '{tick_path}'; i=$((i+1)); sleep 1; done) & \
             wait $!"
        );

        let first = manager
            .exec_command(ExecCommandRequest {
                argv: shell_argv(&script),
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: false,
                yield_time_ms: Some(500),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("exec command should start");

        let session_id = first
            .session_id
            .expect("background child should keep shell waiting");
        let second = manager
            .control_session(ExecControlRequest {
                session_id,
                action: ExecControlAction::Interrupt,
                yield_time_ms: Some(5_000),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("interrupt should return process state");

        assert!(second.session_id.is_none());

        let ticks_after_interrupt = line_count(&tick_file);
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let ticks_later = line_count(&tick_file);
        assert_eq!(
            ticks_after_interrupt, ticks_later,
            "background child should stop writing ticks after interrupt cleanup"
        );

        if let Ok(pid) = std::fs::read_to_string(&pid_file) {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", pid.trim()])
                .status();
        }
        let _ = std::fs::remove_file(pid_file);
        let _ = std::fs::remove_file(tick_file);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn control_kill_terminates_python_child_started_by_default_windows_shell() {
        assert_default_windows_shell_python_child_control(ExecControlAction::Kill).await;
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn control_interrupt_terminates_python_child_started_by_default_windows_shell() {
        assert_default_windows_shell_python_child_control(ExecControlAction::Interrupt).await;
    }

    #[cfg(windows)]
    async fn assert_default_windows_shell_python_child_control(action: ExecControlAction) {
        let manager = ExecProcessManager::default();
        let script = r#"python -c "import time; [print(i, flush=True) or time.sleep(1) for i in range(30)]""#;

        let first = manager
            .exec_command(ExecCommandRequest {
                argv: default_windows_shell_argv(script),
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: false,
                yield_time_ms: Some(1_500),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("exec command should start");

        let session_id = first
            .session_id
            .expect("python loop should still be active");
        let second = manager
            .control_session(ExecControlRequest {
                session_id,
                action,
                yield_time_ms: Some(3_000),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("control action should return process state");

        assert!(
            second.session_id.is_none(),
            "control action should close the shell child process tree, got output: {}",
            second.output
        );
        assert!(second.exit_code.is_some());
    }

    #[tokio::test]
    async fn control_interrupt_writes_ctrl_c_for_tty_session() {
        let manager = ExecProcessManager::default();
        let first = manager
            .exec_command(ExecCommandRequest {
                argv: vec![
                    "python".to_string(),
                    "-c".to_string(),
                    "import time\ntry:\n    time.sleep(30)\nexcept KeyboardInterrupt:\n    print('interrupted')"
                        .to_string(),
                ],
                cwd: std::env::current_dir().expect("current dir"),
                env: HashMap::new(),
                tty: true,
                yield_time_ms: Some(500),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("tty command should start");

        let session_id = first
            .session_id
            .expect("sleeping process should still be active");
        let second = manager
            .control_session(ExecControlRequest {
                session_id,
                action: ExecControlAction::Interrupt,
                yield_time_ms: Some(5_000),
                max_output_chars: Some(10_000),
            })
            .await
            .expect("interrupt should return process state");

        assert!(second.session_id.is_none());
        assert!(second.output.contains("interrupted"));
    }

    #[test]
    fn append_enter_uses_platform_line_submit_byte() {
        let bytes = input_bytes_for_write("hello", true);
        #[cfg(windows)]
        assert_eq!(bytes, b"hello\r");
        #[cfg(not(windows))]
        assert_eq!(bytes, b"hello\n");
    }

    #[test]
    fn head_tail_text_preserves_head_and_tail() {
        let mut buffer = HeadTailText::new(10);
        buffer.push_str("abcdefghijklmnop");
        assert_eq!(buffer.total_chars, 16);

        let rendered = buffer.render();
        assert!(rendered.starts_with("abcde"));
        assert!(rendered.ends_with("lmnop"));
        assert!(rendered.contains("truncated"));
    }

    #[test]
    fn smart_decode_preserves_utf8() {
        assert_eq!(bytes_to_string_smart("小游戏平台".as_bytes()), "小游戏平台");
    }

    #[test]
    fn smart_decode_handles_gbk_chinese_output() {
        let (encoded, _, had_errors) = GBK.encode("小游戏平台");
        assert!(!had_errors);
        assert_eq!(bytes_to_string_smart(&encoded), "小游戏平台");
    }

    #[test]
    fn smart_decode_keeps_windows_1252_punctuation() {
        assert_eq!(
            bytes_to_string_smart(b"\x93\x94 test \x96 dash"),
            "\u{201C}\u{201D} test \u{2013} dash"
        );
    }

    #[cfg(unix)]
    fn line_count(path: &std::path::Path) -> usize {
        std::fs::read_to_string(path)
            .map(|contents| contents.lines().count())
            .unwrap_or(0)
    }
}
