//! MCP server process management
//!
//! Handles starting, stopping, monitoring, and restarting MCP server processes.

use super::connection::MCPConnection;
use super::{MCPServerConfig, MCPServerStatus, MCPServerTransport, MCPServerType};
use crate::mcp::protocol::{InitializeResult, MCPMessage, MCPServerInfo, MCPTransport};
use crate::mcp::server::{is_mcp_auth_error_message, merge_mcp_remote_headers};
use crate::mcp::{MCPRuntimeError, MCPRuntimeResult};
use bitfun_services_core::process_manager;
use log::{debug, error, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Child;
use tokio::sync::{mpsc, RwLock};

/// MCP server process.
pub struct MCPServerProcess {
    id: String,
    name: String,
    server_type: MCPServerType,
    status: Arc<RwLock<MCPServerStatus>>,
    child: Option<Child>,
    connection: Option<Arc<MCPConnection>>,
    server_info: Option<MCPServerInfo>,
    start_time: Option<Instant>,
    restart_count: u32,
    max_restarts: u32,
    health_check_interval: Duration,
    last_ping_time: Arc<RwLock<Option<Instant>>>,
    last_error_message: Arc<RwLock<Option<String>>>,
    message_rx: Option<mpsc::UnboundedReceiver<MCPMessage>>,
}

impl MCPServerProcess {
    /// Creates a new server process instance.
    pub fn new(id: String, name: String, server_type: MCPServerType) -> Self {
        Self {
            id,
            name,
            server_type,
            status: Arc::new(RwLock::new(MCPServerStatus::Uninitialized)),
            child: None,
            connection: None,
            server_info: None,
            start_time: None,
            restart_count: 0,
            max_restarts: 3,
            health_check_interval: Duration::from_secs(30),
            last_ping_time: Arc::new(RwLock::new(None)),
            last_error_message: Arc::new(RwLock::new(None)),
            message_rx: None,
        }
    }

    /// Starts the server process.
    pub async fn start(
        &mut self,
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> MCPRuntimeResult<()> {
        info!("Starting MCP server: name={} id={}", self.name, self.id);
        self.set_status(MCPServerStatus::Starting).await;

        #[cfg(windows)]
        let (final_command, final_args) = {
            let node_commands = ["npm", "npx", "node", "yarn", "pnpm"];
            let is_node_command = node_commands
                .iter()
                .any(|&cmd| command.eq_ignore_ascii_case(cmd));

            if is_node_command {
                debug!("Using cmd.exe for Node.js command: command={}", command);
                let mut cmd_args = vec!["/c".to_string(), command.to_string()];
                cmd_args.extend_from_slice(args);
                ("cmd.exe".to_string(), cmd_args)
            } else {
                (command.to_string(), args.to_vec())
            }
        };

        #[cfg(not(windows))]
        let (final_command, final_args) = (command.to_string(), args.to_vec());

        let mut cmd = process_manager::create_tokio_command(&final_command);
        cmd.args(&final_args);
        cmd.envs(env);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            error!(
                "Failed to spawn MCP server process: command={} error={}",
                final_command, e
            );
            MCPRuntimeError::process(format!(
                "Failed to start MCP server '{}': {}",
                final_command, e
            ))
        });
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                self.set_status_with_error(MCPServerStatus::Failed, Some(e.to_string()))
                    .await;
                return Err(e);
            }
        };

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| MCPRuntimeError::process("Failed to capture stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| MCPRuntimeError::process("Failed to capture stdout".to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel();

        let connection = Arc::new(MCPConnection::new(stdin, rx));
        self.message_rx = None; // The connection already owns rx

        MCPTransport::start_receive_loop(stdout, tx);

        self.connection = Some(connection.clone());
        self.child = Some(child);
        self.start_time = Some(Instant::now());

        if let Err(e) = self.handshake().await {
            error!(
                "MCP server handshake failed: name={} id={} error={}",
                self.name, self.id, e
            );
            let _ = self.stop().await;
            self.set_status_with_error(MCPServerStatus::Failed, Some(e.to_string()))
                .await;
            return Err(e);
        }

        self.set_status_with_error(MCPServerStatus::Connected, None)
            .await;
        self.restart_count = 0;
        info!(
            "MCP server started successfully: name={} id={}",
            self.name, self.id
        );

        self.start_health_check();

        Ok(())
    }

    /// Starts a remote server (Streamable HTTP).
    pub async fn start_remote(
        &mut self,
        data_dir: impl Into<PathBuf>,
        config: &MCPServerConfig,
    ) -> MCPRuntimeResult<()> {
        let url = config.url.as_deref().ok_or_else(|| {
            MCPRuntimeError::configuration(format!(
                "Remote MCP server '{}' is missing a URL",
                self.id
            ))
        })?;
        let transport = config.resolved_transport();
        if transport != MCPServerTransport::StreamableHttp {
            return Err(MCPRuntimeError::not_implemented(format!(
                "Remote MCP transport '{}' is not yet supported",
                transport.as_str()
            )));
        }
        info!(
            "Starting remote MCP server: name={} id={} transport={} url={}",
            self.name,
            self.id,
            transport.as_str(),
            url
        );
        self.set_status(MCPServerStatus::Starting).await;

        let merged_headers = merge_mcp_remote_headers(&config.headers, &config.env);

        let connection = Arc::new(
            MCPConnection::new_remote_with_data_dir(
                data_dir,
                &self.id,
                url.to_string(),
                merged_headers,
                true,
            )
            .await?,
        );
        self.connection = Some(connection.clone());
        self.start_time = Some(Instant::now());

        if let Err(e) = self.handshake().await {
            error!(
                "Remote MCP server handshake failed: name={} id={} url={} error={}",
                self.name, self.id, url, e
            );
            self.connection = None;
            self.message_rx = None;
            self.child = None;
            self.server_info = None;
            if is_mcp_auth_error_message(&e.to_string()) {
                self.set_status_with_error(MCPServerStatus::NeedsAuth, Some(e.to_string()))
                    .await;
            } else {
                self.set_status_with_error(MCPServerStatus::Failed, Some(e.to_string()))
                    .await;
            }
            return Err(e);
        }

        self.set_status_with_error(MCPServerStatus::Connected, None)
            .await;
        self.restart_count = 0;
        info!(
            "Remote MCP server started successfully: name={} id={}",
            self.name, self.id
        );

        self.start_health_check();

        Ok(())
    }

    /// Performs the handshake (`initialize`).
    async fn handshake(&mut self) -> MCPRuntimeResult<()> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| MCPRuntimeError::mcp("Connection not established".to_string()))?;

        debug!(
            "Initiating handshake with MCP server: name={} id={}",
            self.name, self.id
        );

        let result: InitializeResult = connection
            .initialize("BitFun", env!("CARGO_PKG_VERSION"))
            .await?;

        info!(
            "Handshake successful: server_name={} protocol={} resources={} prompts={} tools={}",
            result.server_info.name,
            result.protocol_version,
            result.capabilities.resources.is_some(),
            result.capabilities.prompts.is_some(),
            result.capabilities.tools.is_some()
        );

        self.server_info = Some(result.server_info);
        Ok(())
    }

    /// Stops the server process.
    pub async fn stop(&mut self) -> MCPRuntimeResult<()> {
        info!("Stopping MCP server: name={} id={}", self.name, self.id);
        self.set_status(MCPServerStatus::Stopping).await;

        if let Some(mut child) = self.child.take() {
            if let Err(e) = child.kill().await {
                warn!(
                    "Failed to kill MCP server process: name={} id={} error={}",
                    self.name, self.id, e
                );
            }
        }

        self.connection = None;
        self.message_rx = None;
        self.set_status(MCPServerStatus::Stopped).await;

        info!("MCP server stopped: name={} id={}", self.name, self.id);
        Ok(())
    }

    /// Restarts the server.
    pub async fn restart(
        &mut self,
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> MCPRuntimeResult<()> {
        if self.restart_count >= self.max_restarts {
            error!(
                "Max restart attempts reached: name={} id={} max_restarts={}",
                self.name, self.id, self.max_restarts
            );
            self.set_status_with_error(
                MCPServerStatus::Failed,
                Some(format!(
                    "Max restart attempts ({}) reached",
                    self.max_restarts
                )),
            )
            .await;
            return Err(MCPRuntimeError::mcp(format!(
                "Max restart attempts ({}) reached",
                self.max_restarts
            )));
        }

        self.restart_count += 1;
        info!(
            "Restarting MCP server: name={} id={} attempt={}/{}",
            self.name, self.id, self.restart_count, self.max_restarts
        );

        self.stop().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.start(command, args, env).await
    }

    /// Sets status.
    async fn set_status(&self, status: MCPServerStatus) {
        self.set_status_with_error(status, None).await;
    }

    async fn set_status_with_error(&self, status: MCPServerStatus, error: Option<String>) {
        let mut current_status = self.status.write().await;
        *current_status = status;
        let mut last_error_message = self.last_error_message.write().await;
        *last_error_message = error;
    }

    /// Gets status.
    pub async fn status(&self) -> MCPServerStatus {
        *self.status.read().await
    }

    /// Returns the last status/error detail associated with the process.
    pub async fn status_message(&self) -> Option<String> {
        self.last_error_message.read().await.clone()
    }

    /// Returns the connection.
    pub fn connection(&self) -> Option<Arc<MCPConnection>> {
        self.connection.clone()
    }

    /// Returns server info.
    pub fn server_info(&self) -> Option<&MCPServerInfo> {
        self.server_info.as_ref()
    }

    /// Starts health checks.
    fn start_health_check(&self) {
        let status = self.status.clone();
        let last_ping = self.last_ping_time.clone();
        let last_error_message = self.last_error_message.clone();
        let connection = self.connection.clone();
        let interval = self.health_check_interval;
        let server_name = self.name.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;

                let current_status = *status.read().await;
                if !matches!(
                    current_status,
                    MCPServerStatus::Connected | MCPServerStatus::Healthy
                ) {
                    debug!(
                        "Health check stopped: server_name={} status={:?}",
                        server_name, current_status
                    );
                    break;
                }

                if let Some(conn) = &connection {
                    match conn.ping().await {
                        Ok(_) => {
                            *status.write().await = MCPServerStatus::Healthy;
                            *last_ping.write().await = Some(Instant::now());
                            *last_error_message.write().await = None;
                        }
                        Err(e) => {
                            warn!(
                                "Health check failed: server_name={} error={}",
                                server_name, e
                            );
                            if is_mcp_auth_error_message(&e.to_string()) {
                                *status.write().await = MCPServerStatus::NeedsAuth;
                            } else {
                                *status.write().await = MCPServerStatus::Reconnecting;
                            }
                            *last_error_message.write().await = Some(e.to_string());
                        }
                    }
                } else {
                    break;
                }
            }
        });
    }

    /// Returns the id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the server type.
    pub fn server_type(&self) -> MCPServerType {
        self.server_type
    }

    /// Returns uptime.
    pub fn uptime(&self) -> Option<Duration> {
        self.start_time.map(|t| t.elapsed())
    }
}

impl Drop for MCPServerProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}
