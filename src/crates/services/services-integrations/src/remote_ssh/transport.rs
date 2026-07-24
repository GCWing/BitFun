//! Workspace process transport shared by SSH and local Docker targets.
//!
//! Callers should not need to know whether a long-lived stdio process is backed
//! by a russh channel or a local `docker exec` child. This module normalizes
//! stdin, stdout, stderr, exit status, and interrupt/kill control.

use anyhow::{anyhow, Context};
#[cfg(feature = "remote-ssh-concrete")]
use russh::client::Msg;
#[cfg(feature = "remote-ssh-concrete")]
use russh::{Channel, ChannelMsg, Sig};
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

const WORKSPACE_STDIO_BUFFER_SIZE: usize = 256 * 1024;

pub type WorkspaceReader = Pin<Box<dyn AsyncRead + Send>>;
pub type WorkspaceWriter = Pin<Box<dyn AsyncWrite + Send>>;
pub(crate) type WorkspaceSignalHook = Arc<
    dyn Fn(
            WorkspaceProcessSignal,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
        + Send
        + Sync,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceProcessSignal {
    Interrupt,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceProcessExit {
    pub exit_code: Option<i32>,
}

#[derive(Clone)]
pub struct WorkspaceProcessControl {
    sender: mpsc::Sender<WorkspaceProcessSignal>,
    signal_hook: Option<WorkspaceSignalHook>,
}

impl WorkspaceProcessControl {
    pub async fn interrupt(&self) -> anyhow::Result<()> {
        self.send(WorkspaceProcessSignal::Interrupt).await
    }

    pub async fn kill(&self) -> anyhow::Result<()> {
        self.send(WorkspaceProcessSignal::Kill).await
    }

    async fn send(&self, signal: WorkspaceProcessSignal) -> anyhow::Result<()> {
        let hook_result = match &self.signal_hook {
            Some(hook) => hook(signal).await,
            None => Ok(()),
        };
        if self.signal_hook.is_some()
            && matches!(signal, WorkspaceProcessSignal::Interrupt)
            && hook_result.is_ok()
        {
            // A target-aware hook (for example Docker process-group control)
            // handled the soft interrupt. Keep the owning transport open so
            // the caller can drain output and escalate to Kill after grace.
            return Ok(());
        }
        let send_result = self
            .sender
            .send(signal)
            .await
            .map_err(|_| anyhow!("Workspace process has already exited"));
        hook_result.and(send_result)
    }
}

#[derive(Clone)]
pub struct WorkspaceProcessCompletion {
    receiver: watch::Receiver<Option<WorkspaceProcessExit>>,
}

impl WorkspaceProcessCompletion {
    pub async fn wait(mut self) -> WorkspaceProcessExit {
        loop {
            if let Some(exit) = *self.receiver.borrow() {
                return exit;
            }
            if self.receiver.changed().await.is_err() {
                return WorkspaceProcessExit { exit_code: None };
            }
        }
    }
}

/// A transport-neutral, full-duplex workspace process.
///
/// The underlying SSH channel or Docker child is cancelled once all three IO
/// streams are dropped. `completion` and `control` do not keep the process
/// alive by themselves.
pub struct WorkspaceStdio {
    stdin: WorkspaceWriter,
    stdout: WorkspaceReader,
    stderr: WorkspaceReader,
    control: WorkspaceProcessControl,
    completion: WorkspaceProcessCompletion,
}

impl WorkspaceStdio {
    #[cfg(feature = "remote-ssh-concrete")]
    pub(crate) fn from_ssh_channel(channel: Channel<Msg>) -> Self {
        Self::from_ssh_channel_with_signal_hook(channel, None)
    }

    #[cfg(feature = "remote-ssh-concrete")]
    pub(crate) fn from_ssh_channel_with_signal_hook(
        channel: Channel<Msg>,
        signal_hook: Option<WorkspaceSignalHook>,
    ) -> Self {
        let pipes = WorkspacePipes::new(signal_hook);
        let control = pipes.control.clone();
        let completion = pipes.completion.clone();
        tokio::spawn(run_ssh_channel(channel, pipes.owner));
        Self {
            stdin: pipes.stdin,
            stdout: pipes.stdout,
            stderr: pipes.stderr,
            control,
            completion,
        }
    }

    #[cfg(test)]
    pub(crate) fn spawn_local_process(executable: &str, args: &[String]) -> anyhow::Result<Self> {
        Self::spawn_local_process_with_signal_hook(executable, args, None)
    }

    pub(crate) fn spawn_local_process_with_signal_hook(
        executable: &str,
        args: &[String],
        signal_hook: Option<WorkspaceSignalHook>,
    ) -> anyhow::Result<Self> {
        let mut child = Command::new(executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("Failed to start local executable '{}'", executable))?;
        let child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Local workspace process stdin is unavailable"))?;
        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Local workspace process stdout is unavailable"))?;
        let child_stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Local workspace process stderr is unavailable"))?;

        let pipes = WorkspacePipes::new(signal_hook);
        let control = pipes.control.clone();
        let completion = pipes.completion.clone();
        tokio::spawn(run_local_process(
            child,
            child_stdin,
            child_stdout,
            child_stderr,
            pipes.owner,
        ));
        Ok(Self {
            stdin: pipes.stdin,
            stdout: pipes.stdout,
            stderr: pipes.stderr,
            control,
            completion,
        })
    }

    pub fn into_parts(
        self,
    ) -> (
        WorkspaceWriter,
        WorkspaceReader,
        WorkspaceReader,
        WorkspaceProcessControl,
        WorkspaceProcessCompletion,
    ) {
        (
            self.stdin,
            self.stdout,
            self.stderr,
            self.control,
            self.completion,
        )
    }
}

struct WorkspaceLease {
    cancellation: CancellationToken,
    signal_hook: Option<WorkspaceSignalHook>,
    finished: Arc<AtomicBool>,
}

impl Drop for WorkspaceLease {
    fn drop(&mut self) {
        if self.finished.load(Ordering::Acquire) {
            return;
        }
        if let (Some(signal_hook), Ok(runtime)) = (
            self.signal_hook.clone(),
            tokio::runtime::Handle::try_current(),
        ) {
            let cancellation = self.cancellation.clone();
            runtime.spawn(async move {
                let _ = signal_hook(WorkspaceProcessSignal::Kill).await;
                cancellation.cancel();
            });
            return;
        }
        self.cancellation.cancel();
    }
}

struct LeasedIo {
    inner: DuplexStream,
    _lease: Arc<WorkspaceLease>,
}

impl AsyncRead for LeasedIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for LeasedIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

struct WorkspacePipeOwner {
    stdin: DuplexStream,
    stdout: DuplexStream,
    stderr: DuplexStream,
    control_rx: mpsc::Receiver<WorkspaceProcessSignal>,
    completion_tx: watch::Sender<Option<WorkspaceProcessExit>>,
    cancellation: CancellationToken,
    finished: Arc<AtomicBool>,
}

struct WorkspacePipes {
    stdin: WorkspaceWriter,
    stdout: WorkspaceReader,
    stderr: WorkspaceReader,
    control: WorkspaceProcessControl,
    completion: WorkspaceProcessCompletion,
    owner: WorkspacePipeOwner,
}

impl WorkspacePipes {
    fn new(signal_hook: Option<WorkspaceSignalHook>) -> Self {
        let cancellation = CancellationToken::new();
        let finished = Arc::new(AtomicBool::new(false));
        let lease = Arc::new(WorkspaceLease {
            cancellation: cancellation.clone(),
            signal_hook: signal_hook.clone(),
            finished: finished.clone(),
        });
        let (public_stdin, owner_stdin) = tokio::io::duplex(WORKSPACE_STDIO_BUFFER_SIZE);
        let (owner_stdout, public_stdout) = tokio::io::duplex(WORKSPACE_STDIO_BUFFER_SIZE);
        let (owner_stderr, public_stderr) = tokio::io::duplex(WORKSPACE_STDIO_BUFFER_SIZE);
        let (control_tx, control_rx) = mpsc::channel(8);
        let (completion_tx, completion_rx) = watch::channel(None);

        Self {
            stdin: Box::pin(LeasedIo {
                inner: public_stdin,
                _lease: lease.clone(),
            }),
            stdout: Box::pin(LeasedIo {
                inner: public_stdout,
                _lease: lease.clone(),
            }),
            stderr: Box::pin(LeasedIo {
                inner: public_stderr,
                _lease: lease,
            }),
            control: WorkspaceProcessControl {
                sender: control_tx,
                signal_hook,
            },
            completion: WorkspaceProcessCompletion {
                receiver: completion_rx,
            },
            owner: WorkspacePipeOwner {
                stdin: owner_stdin,
                stdout: owner_stdout,
                stderr: owner_stderr,
                control_rx,
                completion_tx,
                cancellation,
                finished,
            },
        }
    }
}

#[cfg(feature = "remote-ssh-concrete")]
async fn run_ssh_channel(mut channel: Channel<Msg>, mut pipes: WorkspacePipeOwner) {
    let mut stdin_buffer = vec![0u8; 16 * 1024];
    let mut stdin_closed = false;
    let mut exit_code = None;

    loop {
        tokio::select! {
            biased;

            signal = pipes.control_rx.recv() => {
                match signal {
                    Some(WorkspaceProcessSignal::Interrupt) => {
                        let _ = channel.signal(Sig::INT).await;
                    }
                    Some(WorkspaceProcessSignal::Kill) | None => {
                        let _ = channel.signal(Sig::KILL).await;
                        let _ = channel.close().await;
                        exit_code.get_or_insert(137);
                        break;
                    }
                }
            }

            read = pipes.stdin.read(&mut stdin_buffer), if !stdin_closed => {
                match read {
                    Ok(0) | Err(_) => {
                        stdin_closed = true;
                        let _ = channel.eof().await;
                    }
                    Ok(read) => {
                        if channel.data(&stdin_buffer[..read]).await.is_err() {
                            break;
                        }
                    }
                }
            }

            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) => {
                        if pipes.stdout.write_all(data.as_ref()).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if pipes.stderr.write_all(data.as_ref()).await.is_err() {
                            break;
                        }
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

            _ = pipes.cancellation.cancelled() => {
                let _ = channel.signal(Sig::KILL).await;
                let _ = channel.close().await;
                exit_code.get_or_insert(137);
                break;
            }
        }
    }

    pipes.finished.store(true, Ordering::Release);
    let _ = pipes.stdout.shutdown().await;
    let _ = pipes.stderr.shutdown().await;
    let _ = pipes
        .completion_tx
        .send(Some(WorkspaceProcessExit { exit_code }));
}

async fn copy_to_duplex<R>(mut reader: R, mut writer: DuplexStream)
where
    R: AsyncRead + Unpin,
{
    let _ = tokio::io::copy(&mut reader, &mut writer).await;
    let _ = writer.shutdown().await;
}

async fn run_local_process(
    mut child: tokio::process::Child,
    mut child_stdin: tokio::process::ChildStdin,
    child_stdout: tokio::process::ChildStdout,
    child_stderr: tokio::process::ChildStderr,
    mut pipes: WorkspacePipeOwner,
) {
    let mut owner_stdin = pipes.stdin;
    let stdin_task = tokio::spawn(async move {
        let _ = tokio::io::copy(&mut owner_stdin, &mut child_stdin).await;
        let _ = child_stdin.shutdown().await;
    });
    let stdout_task = tokio::spawn(copy_to_duplex(child_stdout, pipes.stdout));
    let stderr_task = tokio::spawn(copy_to_duplex(child_stderr, pipes.stderr));

    let exit_code = loop {
        tokio::select! {
            status = child.wait() => {
                break status.ok().and_then(|status| status.code());
            }
            signal = pipes.control_rx.recv() => {
                let fallback = match signal {
                    Some(WorkspaceProcessSignal::Interrupt) => 130,
                    Some(WorkspaceProcessSignal::Kill) | None => 137,
                };
                let _ = child.start_kill();
                break child.wait().await.ok().and_then(|status| status.code()).or(Some(fallback));
            }
            _ = pipes.cancellation.cancelled() => {
                let _ = child.start_kill();
                break child.wait().await.ok().and_then(|status| status.code()).or(Some(137));
            }
        }
    };

    pipes.finished.store(true, Ordering::Release);
    stdin_task.abort();
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    let _ = pipes
        .completion_tx
        .send(Some(WorkspaceProcessExit { exit_code }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(unix)]
    async fn local_process_round_trips_stdin_stdout_and_exit_status() {
        let transport = WorkspaceStdio::spawn_local_process(
            "sh",
            &[
                "-lc".to_string(),
                "cat; printf problem >&2; exit 7".to_string(),
            ],
        )
        .unwrap();
        let (mut stdin, mut stdout, mut stderr, _control, completion) = transport.into_parts();
        stdin.write_all(b"hello").await.unwrap();
        stdin.shutdown().await.unwrap();

        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        stdout.read_to_end(&mut stdout_bytes).await.unwrap();
        stderr.read_to_end(&mut stderr_bytes).await.unwrap();
        let exit = completion.wait().await;

        assert_eq!(stdout_bytes, b"hello");
        assert_eq!(stderr_bytes, b"problem");
        assert_eq!(exit.exit_code, Some(7));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn local_process_interrupt_completes_with_interrupt_status() {
        let transport = WorkspaceStdio::spawn_local_process(
            "sh",
            &["-lc".to_string(), "while :; do sleep 1; done".to_string()],
        )
        .unwrap();
        let (_stdin, _stdout, _stderr, control, completion) = transport.into_parts();

        control.interrupt().await.unwrap();
        let exit = tokio::time::timeout(std::time::Duration::from_secs(2), completion.wait())
            .await
            .expect("interrupt should terminate the supervised process");

        assert_eq!(exit.exit_code, Some(130));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn local_process_control_invokes_target_signal_hook_before_shutdown() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let hook_called = Arc::new(AtomicBool::new(false));
        let hook: WorkspaceSignalHook = {
            let hook_called = hook_called.clone();
            Arc::new(move |_| {
                let hook_called = hook_called.clone();
                Box::pin(async move {
                    hook_called.store(true, Ordering::SeqCst);
                    Ok(())
                })
            })
        };
        let transport = WorkspaceStdio::spawn_local_process_with_signal_hook(
            "sh",
            &["-lc".to_string(), "sleep 0.1; exit 7".to_string()],
            Some(hook),
        )
        .unwrap();
        let (_stdin, _stdout, _stderr, control, completion) = transport.into_parts();

        control.interrupt().await.unwrap();
        let exit = tokio::time::timeout(std::time::Duration::from_secs(2), completion.wait())
            .await
            .expect("interrupt should terminate the supervised process");

        assert!(hook_called.load(Ordering::SeqCst));
        assert_eq!(
            exit.exit_code,
            Some(7),
            "a hook-handled soft interrupt must not kill the owning transport"
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn dropping_all_io_streams_cancels_local_process() {
        let transport = WorkspaceStdio::spawn_local_process(
            "sh",
            &["-lc".to_string(), "while :; do sleep 1; done".to_string()],
        )
        .unwrap();
        let (stdin, stdout, stderr, _control, completion) = transport.into_parts();
        drop(stdin);
        drop(stdout);
        drop(stderr);

        let exit = tokio::time::timeout(std::time::Duration::from_secs(2), completion.wait())
            .await
            .expect("dropping all IO should terminate the supervised process");

        assert_eq!(exit.exit_code, Some(137));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn dropping_all_io_streams_invokes_target_kill_hook() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let hook_called = Arc::new(AtomicBool::new(false));
        let hook: WorkspaceSignalHook = {
            let hook_called = hook_called.clone();
            Arc::new(move |signal| {
                let hook_called = hook_called.clone();
                Box::pin(async move {
                    assert_eq!(signal, WorkspaceProcessSignal::Kill);
                    hook_called.store(true, Ordering::SeqCst);
                    Ok(())
                })
            })
        };
        let transport = WorkspaceStdio::spawn_local_process_with_signal_hook(
            "sh",
            &["-lc".to_string(), "while :; do sleep 1; done".to_string()],
            Some(hook),
        )
        .unwrap();
        let (stdin, stdout, stderr, _control, completion) = transport.into_parts();
        drop(stdin);
        drop(stdout);
        drop(stderr);

        let exit = tokio::time::timeout(std::time::Duration::from_secs(2), completion.wait())
            .await
            .expect("dropping all IO should terminate the supervised process");

        assert!(hook_called.load(Ordering::SeqCst));
        assert_eq!(exit.exit_code, Some(137));
    }
}
