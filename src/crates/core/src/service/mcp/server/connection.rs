//! MCP connection management
//!
//! Handles communication connections to MCP servers and request/response management.

use crate::service::mcp::protocol::{
    create_initialize_request, create_ping_request, create_prompts_get_request,
    create_prompts_list_request, create_resources_list_request, create_resources_read_request,
    create_tools_call_request, create_tools_list_request, parse_response_result, InitializeResult,
    MCPError, MCPMessage, MCPResponse, MCPToolResult, MCPTransport, PromptsGetResult,
    PromptsListResult, RemoteMCPTransport, ResourcesListResult, ResourcesReadResult,
    ToolsListResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::ChildStdin;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};

/// Request/response waiter.
type ResponseWaiter = oneshot::Sender<MCPResponse>;

/// Transport type.
enum TransportType {
    Local(Arc<MCPTransport>),
    Remote(Arc<RemoteMCPTransport>),
}

/// Connection lifecycle / protocol events.
#[derive(Debug, Clone)]
pub enum MCPConnectionEvent {
    Notification {
        method: String,
        params: Option<Value>,
    },
    Request {
        request_id: Value,
        method: String,
        params: Option<Value>,
    },
    Closed,
}

/// MCP connection.
pub struct MCPConnection {
    transport: TransportType,
    pending_requests: Arc<RwLock<HashMap<u64, ResponseWaiter>>>,
    request_timeout: Option<Duration>,
    event_tx: broadcast::Sender<MCPConnectionEvent>,
}

impl MCPConnection {
    /// Creates a new local connection instance (stdin/stdout).
    pub fn new_local(stdin: ChildStdin, message_rx: mpsc::UnboundedReceiver<MCPMessage>) -> Self {
        let transport = Arc::new(MCPTransport::new(stdin));
        let pending_requests = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(64);

        let pending = pending_requests.clone();
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            Self::handle_messages(message_rx, pending, event_tx_clone).await;
        });

        Self {
            transport: TransportType::Local(transport),
            pending_requests,
            request_timeout: None,
            event_tx,
        }
    }

    /// Creates a new remote connection instance (Streamable HTTP).
    pub async fn new_remote(
        server_id: &str,
        url: String,
        headers: HashMap<String, String>,
        oauth_enabled: bool,
    ) -> BitFunResult<Self> {
        let request_timeout = None;
        let transport = Arc::new(
            RemoteMCPTransport::new(server_id, url, headers, request_timeout, oauth_enabled)
                .await?,
        );
        let pending_requests = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(64);

        Ok(Self {
            transport: TransportType::Remote(transport),
            pending_requests,
            request_timeout,
            event_tx,
        })
    }

    /// Returns the auth token for a remote connection.
    pub async fn get_auth_token(&self) -> Option<String> {
        match &self.transport {
            TransportType::Remote(transport) => transport.get_auth_token().await,
            TransportType::Local(_) => None,
        }
    }

    /// Whether this MCP server runs as a local stdio child process.
    pub fn is_local_stdio(&self) -> bool {
        matches!(self.transport, TransportType::Local(_))
    }

    /// Backward-compatible constructor (local connection).
    pub fn new(stdin: ChildStdin, message_rx: mpsc::UnboundedReceiver<MCPMessage>) -> Self {
        Self::new_local(stdin, message_rx)
    }

    /// Subscribes to connection events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<MCPConnectionEvent> {
        self.event_tx.subscribe()
    }

    /// Handles received messages.
    async fn handle_messages(
        mut rx: mpsc::UnboundedReceiver<MCPMessage>,
        pending_requests: Arc<RwLock<HashMap<u64, ResponseWaiter>>>,
        event_tx: broadcast::Sender<MCPConnectionEvent>,
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
                    let _ = event_tx.send(MCPConnectionEvent::Notification {
                        method: notification.method,
                        params: notification.params,
                    });
                }
                MCPMessage::Request(request) => {
                    warn!("Received unexpected request from MCP server");
                    let _ = event_tx.send(MCPConnectionEvent::Request {
                        request_id: request.id,
                        method: request.method,
                        params: request.params,
                    });
                }
            }
        }

        let _ = event_tx.send(MCPConnectionEvent::Closed);
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

                let response = if let Some(request_timeout) = self.request_timeout {
                    tokio::time::timeout(request_timeout, rx)
                        .await
                        .map_err(|_| {
                            BitFunError::Timeout(format!("Request timeout for method: {}", method))
                        })?
                } else {
                    rx.await
                };

                match response {
                    Ok(response) => Ok(response),
                    Err(_) => Err(BitFunError::MCPError(format!(
                        "Request channel closed for method: {}",
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
            TransportType::Remote(transport) => {
                transport.initialize(client_name, client_version).await
            }
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

    /// Sends a JSON-RPC success response for a server-initiated request.
    pub async fn send_response(&self, request_id: Value, result: Value) -> BitFunResult<()> {
        match &self.transport {
            TransportType::Local(transport) => transport.send_response(request_id, result).await,
            TransportType::Remote(_) => Err(BitFunError::NotImplemented(
                "Sending server-request responses is not supported for Streamable HTTP connections"
                    .to_string(),
            )),
        }
    }

    /// Sends a JSON-RPC error response for a server-initiated request.
    pub async fn send_error(&self, request_id: Value, error: MCPError) -> BitFunResult<()> {
        match &self.transport {
            TransportType::Local(transport) => transport.send_error(request_id, error).await,
            TransportType::Remote(_) => Err(BitFunError::NotImplemented(
                "Sending server-request errors is not supported for Streamable HTTP connections"
                    .to_string(),
            )),
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
