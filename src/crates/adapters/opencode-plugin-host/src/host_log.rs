use openbitfun_services_core::process_tree::ProcessTreeChild;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const LOG_CHANNEL_CAPACITY: usize = 256;
const READ_BUFFER_BYTES: usize = 4096;
const MAX_LOG_LINE_BYTES: usize = 32 * 1024;

struct HostLogLine {
    source: &'static str,
    bytes: Vec<u8>,
    truncated: bool,
}

pub(crate) struct HostLogDrain {
    readers: [JoinHandle<()>; 2],
    writer: JoinHandle<()>,
}

impl HostLogDrain {
    pub(crate) async fn flush(self, deadline: std::time::Duration) -> bool {
        tokio::time::timeout(deadline, async move {
            for reader in self.readers {
                let _ = reader.await;
            }
            let _ = self.writer.await;
        })
        .await
        .is_ok()
    }
}

#[derive(Default)]
struct DroppedLogLines {
    stdout: AtomicU64,
    stderr: AtomicU64,
}

impl DroppedLogLines {
    fn counter(&self, source: &str) -> &AtomicU64 {
        match source {
            "stdout" => &self.stdout,
            "stderr" => &self.stderr,
            _ => &self.stderr,
        }
    }
}

pub(crate) async fn attach_host_log(
    child: &mut ProcessTreeChild,
    log_file: &Path,
) -> io::Result<HostLogDrain> {
    let parent = log_file
        .parent()
        .ok_or_else(|| io::Error::other("plugin host log file has no parent directory"))?;
    tokio::fs::create_dir_all(parent).await?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .await?;
    let stdout = child
        .take_stdout()
        .ok_or_else(|| io::Error::other("plugin host stdout is not piped"))?;
    let stderr = child
        .take_stderr()
        .ok_or_else(|| io::Error::other("plugin host stderr is not piped"))?;
    let (sender, receiver) = mpsc::channel(LOG_CHANNEL_CAPACITY);
    let dropped = Arc::new(DroppedLogLines::default());
    let writer = tokio::spawn(write_log(file, receiver, dropped.clone()));
    let stdout_reader = tokio::spawn(read_log(stdout, "stdout", sender.clone(), dropped.clone()));
    let stderr_reader = tokio::spawn(read_log(stderr, "stderr", sender, dropped));
    Ok(HostLogDrain {
        readers: [stdout_reader, stderr_reader],
        writer,
    })
}

async fn read_log<R>(
    mut reader: R,
    source: &'static str,
    sender: mpsc::Sender<HostLogLine>,
    dropped: Arc<DroppedLogLines>,
) where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    let mut line = Vec::with_capacity(READ_BUFFER_BYTES);
    let mut truncated = false;
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(0) => {
                enqueue_line(&sender, &dropped, source, &mut line, truncated);
                return;
            }
            Ok(read) => read,
            Err(_) => return,
        };
        for byte in &buffer[..read] {
            if *byte == b'\n' {
                enqueue_line(&sender, &dropped, source, &mut line, truncated);
                truncated = false;
            } else if line.len() < MAX_LOG_LINE_BYTES {
                line.push(*byte);
            } else {
                truncated = true;
            }
        }
    }
}

fn enqueue_line(
    sender: &mpsc::Sender<HostLogLine>,
    dropped: &DroppedLogLines,
    source: &'static str,
    line: &mut Vec<u8>,
    truncated: bool,
) {
    if line.is_empty() && !truncated {
        return;
    }
    let bytes = std::mem::take(line);
    let message = HostLogLine {
        source,
        bytes,
        truncated,
    };
    if let Err(mpsc::error::TrySendError::Full(_)) = sender.try_send(message) {
        dropped.counter(source).fetch_add(1, Ordering::Relaxed);
    }
}

async fn write_log(
    mut file: File,
    mut receiver: mpsc::Receiver<HostLogLine>,
    dropped: Arc<DroppedLogLines>,
) {
    let mut flush_dropped = tokio::time::interval(std::time::Duration::from_secs(1));
    flush_dropped.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            line = receiver.recv() => {
                let Some(line) = line else {
                    break;
                };
                if write_dropped_lines(&mut file, &dropped).await.is_err()
                    || write_line(&mut file, &line).await.is_err()
                {
                    return;
                }
            }
            _ = flush_dropped.tick() => {
                if write_dropped_lines(&mut file, &dropped).await.is_err() {
                    return;
                }
            }
        }
    }
    let _ = write_dropped_lines(&mut file, &dropped).await;
    let _ = file.flush().await;
}

async fn write_line(file: &mut File, line: &HostLogLine) -> io::Result<()> {
    file.write_all(b"[").await?;
    file.write_all(line.source.as_bytes()).await?;
    file.write_all(b"] ").await?;
    file.write_all(&line.bytes).await?;
    if line.truncated {
        file.write_all(b" [truncated]").await?;
    }
    file.write_all(b"\n").await
}

async fn write_dropped_lines(file: &mut File, dropped: &DroppedLogLines) -> io::Result<()> {
    for source in ["stdout", "stderr"] {
        let count = dropped.counter(source).swap(0, Ordering::Relaxed);
        if count > 0 {
            file.write_all(
                format!("[plugin-host] dropped_lines={count}, source={source}\n").as_bytes(),
            )
            .await?;
        }
    }
    Ok(())
}
