//! Remote Terminal Session Management with PTY support
//!
//! Architecture:
//! - Each PTY has a single owner task that exclusively holds the russh Channel
//! - Reading: owner task calls `channel.wait()` and broadcasts output via `broadcast::Sender`
//! - Writing: callers send `PtyCommand::Write` via `mpsc::Sender` → owner task → `channel.data()`
//! - This eliminates Mutex deadlock between read and write operations

use crate::service::remote_ssh::manager::SSHConnectionManager;
use crate::service::terminal::session::SessionSource;
use anyhow::Context;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{broadcast, mpsc, RwLock};

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || c == '/' || c == '.' || c == '-' || c == '_') {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[derive(Debug, Clone)]
pub struct RemoteTerminalSession {
    pub id: String,
    pub name: String,
    pub connection_id: String,
    pub cwd: String,
    pub pid: Option<u32>,
    pub status: SessionStatus,
    pub cols: u16,
    pub rows: u16,
    pub source: SessionSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Active,
    Inactive,
    Closed,
}

enum PtyCommand {
    Write(Vec<u8>),
    Resize(u32, u32),
    Close,
}

struct ActiveHandle {
    output_tx: broadcast::Sender<Vec<u8>>,
    cmd_tx: mpsc::Sender<PtyCommand>,
}

pub struct CreateSessionResult {
    pub session: RemoteTerminalSession,
    pub output_rx: broadcast::Receiver<Vec<u8>>,
}

pub struct RemoteTerminalManager {
    sessions: Arc<RwLock<HashMap<String, RemoteTerminalSession>>>,
    ssh_manager: Arc<tokio::sync::RwLock<Option<SSHConnectionManager>>>,
    handles: Arc<RwLock<HashMap<String, ActiveHandle>>>,
}

impl RemoteTerminalManager {
    pub fn new(ssh_manager: SSHConnectionManager) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ssh_manager: Arc::new(tokio::sync::RwLock::new(Some(ssh_manager))),
            handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_ssh_manager(&self, manager: SSHConnectionManager) {
        *self.ssh_manager.write().await = Some(manager);
    }

    /// Create a new remote terminal session.
    /// Returns a `CreateSessionResult` with a pre-subscribed output receiver.
    /// The owner task is spawned immediately — the output_rx is guaranteed to
    /// receive all data including the initial shell prompt.
    pub async fn create_session(
        &self,
        session_id: Option<String>,
        name: Option<String>,
        connection_id: &str,
        cols: u16,
        rows: u16,
        initial_cwd: Option<&str>,
        source: Option<SessionSource>,
    ) -> anyhow::Result<CreateSessionResult> {
        let ssh_guard = self.ssh_manager.read().await;
        let manager = ssh_guard.as_ref().context("SSH manager not initialized")?;

        let session_id = session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let name = name.unwrap_or_else(|| format!("Remote Terminal {}", &session_id[..8]));

        // Open PTY via manager, then extract the raw Channel
        let pty = manager.open_pty(connection_id, cols as u32, rows as u32).await?;
        let mut channel = pty.into_channel().await
            .ok_or_else(|| anyhow::anyhow!("Failed to extract channel from PTYSession — multiple references exist"))?;

        let cwd = if let Some(dir) = initial_cwd {
            dir.to_string()
        } else {
            match manager.execute_command(connection_id, "pwd").await {
                Ok((output, _, _)) => output.trim().to_string(),
                Err(_) => "/".to_string(),
            }
        };

        // broadcast for output, mpsc for commands to the owner task
        let (output_tx, output_rx) = broadcast::channel::<Vec<u8>>(1000);
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<PtyCommand>(100);

        let initial_cd = cwd.clone();

        let session = RemoteTerminalSession {
            id: session_id.clone(),
            name,
            connection_id: connection_id.to_string(),
            cwd,
            pid: None,
            status: SessionStatus::Active,
            cols,
            rows,
            source: source.unwrap_or_default(),
        };

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }
        {
            let mut handles = self.handles.write().await;
            handles.insert(session_id.clone(), ActiveHandle {
                output_tx: output_tx.clone(),
                cmd_tx,
            });
        }

        let mut writer = channel.make_writer();

        let task_session_id = session_id.clone();
        let task_handles = self.handles.clone();
        let task_sessions = self.sessions.clone();

        tokio::spawn(async move {
            log::info!("Remote PTY owner task started: session_id={}", task_session_id);

            // cd to workspace directory silently
            if initial_cd != "/" {
                let cd_cmd = format!("cd {} && clear\n", shell_escape(&initial_cd));
                if let Err(e) = writer.write_all(cd_cmd.as_bytes()).await {
                    log::warn!("Failed to cd to initial directory: {}", e);
                }
                let _ = writer.flush().await;
            }

            loop {
                tokio::select! {
                    biased; // prioritize commands over reads to avoid write starvation

                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(PtyCommand::Write(data)) => {
                                if let Err(e) = writer.write_all(&data).await {
                                    log::warn!("PTY write failed: session_id={}, error={}", task_session_id, e);
                                }
                                // flush to ensure data is sent immediately
                                let _ = writer.flush().await;
                            }
                            Some(PtyCommand::Resize(cols, rows)) => {
                                if let Err(e) = channel.window_change(cols, rows, 0, 0).await {
                                    log::warn!("PTY resize failed: session_id={}, error={}", task_session_id, e);
                                }
                            }
                            Some(PtyCommand::Close) | None => {
                                log::info!("PTY close requested: session_id={}", task_session_id);
                                let _ = channel.eof().await;
                                let _ = channel.close().await;
                                break;
                            }
                        }
                    }

                    msg = channel.wait() => {
                        match msg {
                            Some(russh::ChannelMsg::Data { data }) => {
                                let _ = output_tx.send(data.to_vec());
                            }
                            Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                                let _ = output_tx.send(data.to_vec());
                            }
                            Some(russh::ChannelMsg::Eof)
                            | Some(russh::ChannelMsg::Close)
                            | Some(russh::ChannelMsg::ExitStatus { .. }) => {
                                log::info!("Remote PTY closed: session_id={}", task_session_id);
                                break;
                            }
                            Some(_) => continue, // WindowAdjust, Success, etc.
                            None => {
                                log::info!("Remote PTY channel ended: session_id={}", task_session_id);
                                break;
                            }
                        }
                    }
                }
            }

            // Clean up
            {
                let mut handles = task_handles.write().await;
                handles.remove(&task_session_id);
            }
            {
                let mut sessions = task_sessions.write().await;
                if let Some(s) = sessions.get_mut(&task_session_id) {
                    s.status = SessionStatus::Closed;
                }
            }
            log::info!("Remote PTY owner task exited: session_id={}", task_session_id);
        });

        Ok(CreateSessionResult { session, output_rx })
    }

    pub async fn get_session(&self, session_id: &str) -> Option<RemoteTerminalSession> {
        self.sessions.read().await.get(session_id).cloned()
    }

    pub async fn list_sessions(&self) -> Vec<RemoteTerminalSession> {
        self.sessions.read().await.values()
            .filter(|s| s.status != SessionStatus::Closed)
            .cloned()
            .collect()
    }

    pub async fn write(&self, session_id: &str, data: &[u8]) -> anyhow::Result<()> {
        let handles = self.handles.read().await;
        let handle = handles.get(session_id).context("Session not found or PTY not active")?;
        handle.cmd_tx.send(PtyCommand::Write(data.to_vec())).await
            .map_err(|_| anyhow::anyhow!("PTY task has exited"))
    }

    pub async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> anyhow::Result<()> {
        {
            let mut sessions = self.sessions.write().await;
            if let Some(s) = sessions.get_mut(session_id) {
                s.cols = cols;
                s.rows = rows;
            }
        }
        let handles = self.handles.read().await;
        if let Some(handle) = handles.get(session_id) {
            handle.cmd_tx.send(PtyCommand::Resize(cols as u32, rows as u32)).await
                .map_err(|_| anyhow::anyhow!("PTY task has exited"))?;
        }
        Ok(())
    }

    pub async fn close_session(&self, session_id: &str) -> anyhow::Result<()> {
        // Send close command to owner task
        {
            let handles = self.handles.read().await;
            if let Some(handle) = handles.get(session_id) {
                let _ = handle.cmd_tx.send(PtyCommand::Close).await;
            }
        }
        // Also remove from sessions map immediately so it disappears from list
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);
        }
        Ok(())
    }

    pub async fn is_pty_active(&self, session_id: &str) -> bool {
        self.handles.read().await.contains_key(session_id)
    }

    pub async fn subscribe_output(&self, session_id: &str) -> anyhow::Result<broadcast::Receiver<Vec<u8>>> {
        let handles = self.handles.read().await;
        let handle = handles.get(session_id).context("Session not found or PTY not active")?;
        Ok(handle.output_tx.subscribe())
    }
}

impl Clone for RemoteTerminalManager {
    fn clone(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            ssh_manager: self.ssh_manager.clone(),
            handles: self.handles.clone(),
        }
    }
}
