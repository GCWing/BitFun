//! Agent Client Protocol (ACP) Support
//!
//! This module implements the Agent Client Protocol for BitFun CLI,
//! enabling integration with ACP-compatible editors and IDEs.
//!
//! ACP is a JSON-RPC 2.0 based protocol for communication between
//! code editors/IDEs and AI coding agents.

pub mod handlers;
pub mod protocol;
pub mod session;

use anyhow::{Context, Result};
use std::sync::Arc;

use crate::agent::AgenticSystem;

pub use protocol::*;
pub use session::*;

/// ACP Server - handles JSON-RPC communication over stdio
pub struct AcpServer {
    agentic_system: AgenticSystem,
    session_manager: Arc<AcpSessionManager>,
}

impl AcpServer {
    /// Create a new ACP server
    pub fn new(agentic_system: AgenticSystem) -> Self {
        Self {
            session_manager: Arc::new(AcpSessionManager::new()),
            agentic_system,
        }
    }

    /// Run the ACP server - reads JSON-RPC from stdin, writes to stdout
    pub async fn run(&self) -> Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        tracing::info!("Starting ACP server (JSON-RPC over stdio)");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                tracing::info!("EOF received, shutting down ACP server");
                break;
            }

            let request = line.trim();
            if request.is_empty() {
                continue;
            }

            tracing::debug!("Received ACP request: {}", request);

            match self.handle_request(request).await {
                Ok(Some(response)) => {
                    let response_json = serde_json::to_string(&response)?;
                    tracing::debug!("Sending ACP response: {}", response_json);
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
                Ok(None) => {
                    // Notification, no response needed
                    tracing::debug!("ACP notification processed (no response needed)");
                }
                Err(e) => {
                    tracing::error!("Error handling ACP request: {}", e);
                    // Send error response if we can parse the request ID
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(request) {
                        if let Some(id) = json_value.get("id") {
                            let error_response = JsonRpcResponse::error(
                                id.clone(),
                                -32603,
                                format!("Internal error: {}", e),
                            );
                            let error_json = serde_json::to_string(&error_response)?;
                            stdout.write_all(error_json.as_bytes()).await?;
                            stdout.write_all(b"\n").await?;
                            stdout.flush().await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle a single JSON-RPC request
    async fn handle_request(&self, request: &str) -> Result<Option<JsonRpcResponse>> {
        let rpc_request: JsonRpcRequest =
            serde_json::from_str(request).context("Failed to parse JSON-RPC request")?;

        handlers::handle_method(rpc_request, &self.agentic_system, &self.session_manager).await
    }
}
