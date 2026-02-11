//! MCP connection management
//!
//! Handles communication connections to MCP servers and request/response management.

use crate::service::mcp::protocol::{
    create_initialize_request, create_ping_request, create_prompts_get_request,
    create_prompts_list_request, create_resources_list_request, create_resources_read_request,
    create_tools_call_request, create_tools_list_request, parse_response_result,
    transport::MCPTransport,
    transport_remote::RemoteMCPTransport,
    InitializeResult, MCPMessage, MCPResponse, MCPToolResult, PromptsGetResult,
    PromptsListResult, ResourcesListResult, ResourcesReadResult, ToolsListResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::ChildStdin;
use tokio::sync::{mpsc, oneshot, RwLock};

/// Request/response waiter.
type ResponseWaiter = oneshot::Sender<MCPResponse>;

/// Transport type.
enum TransportType {
    Local(Arc<MCPTransport>),
    Remote(Arc<RemoteMCPTransport>),
}

/// MCP connection.
pub struct MCPConnection {
    transport: TransportType,
    pending_requests: Arc<RwLock<HashMap<u64, ResponseWaiter>>>,
    request_timeout: Duration,
}

impl MCPConnection {
    /// Creates a new local connection instance (stdin/stdout).
    pub fn new_local(stdin: ChildStdin, message_rx: mpsc::UnboundedReceiver<MCPMessage>) -> Self {
        let transport = Arc::new(MCPTransport::new(stdin));
        let pending_requests = Arc::new(RwLock::new(HashMap::new()));

        let pending = pending_requests.clone();
        tokio::spawn(async move {
            Self::handle_messages(message_rx, pending).await;
        });

        Self {
            transport: TransportType::Local(transport),
            pending_requests,
            request_timeout: Duration::from_secs(180),
        }
    }

    /// Creates a new remote connection instance (Streamable HTTP).
    pub fn new_remote(url: String, headers: HashMap<String, String>) -> Self {
        let request_timeout = Duration::from_secs(180);
        let transport = Arc::new(RemoteMCPTransport::new(url, headers, request_timeout));
        let pending_requests = Arc::new(RwLock::new(HashMap::new()));

        Self {
            transport: TransportType::Remote(transport),
            pending_requests,
            request_timeout,
        }
    }

    /// Returns the auth token for a remote connection.
    pub async fn get_auth_token(&self) -> Option<String> {
        match &self.transport {
            TransportType::Remote(transport) => transport.get_auth_token(),
            TransportType::Local(_) => None,
        }
    }

    /// Backward-compatible constructor (local connection).
    pub fn new(stdin: ChildStdin, message_rx: mpsc::UnboundedReceiver<MCPMessage>) -> Self {
        Self::new_local(stdin, message_rx)
    }

    /// Handles received messages.
    async fn handle_messages(
        mut rx: mpsc::UnboundedReceiver<MCPMessage>,
        pending_requests: Arc<RwLock<HashMap<u64, ResponseWaiter>>>,
    ) {
        while let Some(message) = rx.recv().await {
            match message {
                MCPMessage::Response(response) => {
                    if let Some(id) = response.id.as_u64() {
                        let mut pending = pending_requests.write().await;
                        if let Some(waiter) = pending.remove(&id) {
                            let _ = waiter.send(response);
                        } else {
                            warn!("Received response for unknown request ID: {}", id);
                        }
                    }
                }
                MCPMessage::Notification(notification) => {
                    debug!("Received MCP notification: method={}", notification.method);
                }
                MCPMessage::Request(_request) => {
                    warn!("Received unexpected request from MCP server");
                }
            }
        }
    }

    /// Sends a request and waits for the response.
    async fn send_request_and_wait(
        &self,
        method: String,
        params: Option<Value>,
    ) -> BitFunResult<MCPResponse> {
        match &self.transport {
            TransportType::Local(transport) => {
                let request_id = transport.send_request(method.clone(), params).await?;

                let (tx, rx) = oneshot::channel();
                {
                    let mut pending = self.pending_requests.write().await;
                    pending.insert(request_id, tx);
                }

                match tokio::time::timeout(self.request_timeout, rx).await {
                    Ok(Ok(response)) => Ok(response),
                    Ok(Err(_)) => Err(BitFunError::MCPError(format!(
                        "Request channel closed for method: {}",
                        method
                    ))),
                    Err(_) => Err(BitFunError::Timeout(format!(
                        "Request timeout for method: {}",
                        method
                    ))),
                }
            }
            TransportType::Remote(_transport) => Err(BitFunError::NotImplemented(
                "Generic JSON-RPC send_request is not supported for Streamable HTTP connections"
                    .to_string(),
            )),
        }
    }

    /// Initializes the connection.
    pub async fn initialize(
        &self,
        client_name: &str,
        client_version: &str,
    ) -> BitFunResult<InitializeResult> {
        match &self.transport {
            TransportType::Local(_) => {
                let request = create_initialize_request(0, client_name, client_version);
                let response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;
                parse_response_result(&response)
            }
            TransportType::Remote(transport) => transport.initialize(client_name, client_version).await,
        }
    }

    /// Lists resources.
    pub async fn list_resources(
        &self,
        cursor: Option<String>,
    ) -> BitFunResult<ResourcesListResult> {
        match &self.transport {
            TransportType::Local(_) => {
                let request = create_resources_list_request(0, cursor);
                let response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;
                parse_response_result(&response)
            }
            TransportType::Remote(transport) => transport.list_resources(cursor).await,
        }
    }

    /// Reads a resource.
    pub async fn read_resource(&self, uri: &str) -> BitFunResult<ResourcesReadResult> {
        match &self.transport {
            TransportType::Local(_) => {
                let request = create_resources_read_request(0, uri);
                let response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;
                parse_response_result(&response)
            }
            TransportType::Remote(transport) => transport.read_resource(uri).await,
        }
    }

    /// Lists prompts.
    pub async fn list_prompts(&self, cursor: Option<String>) -> BitFunResult<PromptsListResult> {
        match &self.transport {
            TransportType::Local(_) => {
                let request = create_prompts_list_request(0, cursor);
                let response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;
                parse_response_result(&response)
            }
            TransportType::Remote(transport) => transport.list_prompts(cursor).await,
        }
    }

    /// Gets a prompt.
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<HashMap<String, String>>,
    ) -> BitFunResult<PromptsGetResult> {
        match &self.transport {
            TransportType::Local(_) => {
                let request = create_prompts_get_request(0, name, arguments);
                let response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;
                parse_response_result(&response)
            }
            TransportType::Remote(transport) => transport.get_prompt(name, arguments).await,
        }
    }

    /// Lists tools.
    pub async fn list_tools(&self, cursor: Option<String>) -> BitFunResult<ToolsListResult> {
        match &self.transport {
            TransportType::Local(_) => {
                let request = create_tools_list_request(0, cursor);
                let response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;
                parse_response_result(&response)
            }
            TransportType::Remote(transport) => transport.list_tools(cursor).await,
        }
    }

    /// Calls a tool.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> BitFunResult<MCPToolResult> {
        match &self.transport {
            TransportType::Local(_) => {
                debug!("Calling MCP tool: name={}", name);
                let request = create_tools_call_request(0, name, arguments);

                let response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;

                parse_response_result(&response)
            }
            TransportType::Remote(transport) => transport.call_tool(name, arguments).await,
        }
    }

    /// Sends `ping` (heartbeat check).
    pub async fn ping(&self) -> BitFunResult<()> {
        match &self.transport {
            TransportType::Local(_) => {
                let request = create_ping_request(0);
                let _response = self
                    .send_request_and_wait(request.method.clone(), request.params)
                    .await?;
                Ok(())
            }
            TransportType::Remote(transport) => transport.ping().await,
        }
    }
}

/// MCP connection pool.
pub struct MCPConnectionPool {
    connections: Arc<RwLock<HashMap<String, Arc<MCPConnection>>>>,
}

impl MCPConnectionPool {
    /// Creates a new connection pool.
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Adds a connection.
    pub async fn add_connection(&self, server_id: String, connection: Arc<MCPConnection>) {
        let mut connections = self.connections.write().await;
        connections.insert(server_id, connection);
    }

    /// Gets a connection.
    pub async fn get_connection(&self, server_id: &str) -> Option<Arc<MCPConnection>> {
        let connections = self.connections.read().await;
        connections.get(server_id).cloned()
    }

    /// Removes a connection.
    pub async fn remove_connection(&self, server_id: &str) {
        let mut connections = self.connections.write().await;
        connections.remove(server_id);
    }

    /// Returns all connection IDs.
    pub async fn get_all_server_ids(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }
}

impl Default for MCPConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
