//! Remote MCP transport (Streamable HTTP)
//!
//! Uses the official `rmcp` Rust SDK to implement the MCP Streamable HTTP client transport.

use super::types::{
    InitializeResult as BitFunInitializeResult, MCPToolResult, PromptsGetResult, PromptsListResult,
    ResourcesListResult, ResourcesReadResult, ToolsListResult,
};
use crate::service::mcp::auth::build_authorization_manager;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_services_integrations::mcp::config::normalize_mcp_authorization_value;
use bitfun_services_integrations::mcp::protocol::{
    create_mcp_client_info, map_rmcp_initialize_result, map_rmcp_prompt, map_rmcp_prompt_message,
    map_rmcp_resource, map_rmcp_resource_content, map_rmcp_tool, map_rmcp_tool_result,
};
use futures::StreamExt;
use log::{debug, error, info, warn};
use reqwest::header::{
    ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT, WWW_AUTHENTICATE,
};
use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::{
    CallToolRequestParam, ClientInfo, GetPromptRequestParam, JsonObject, LoggingLevel,
    LoggingMessageNotificationParam, PaginatedRequestParam, ReadResourceRequestParam,
    RequestNoParam,
};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::common::http_header::{
    EVENT_STREAM_MIME_TYPE, HEADER_LAST_EVENT_ID, HEADER_SESSION_ID, JSON_MIME_TYPE,
};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::streamable_http_client::{
    AuthRequiredError, SseError, StreamableHttpClient, StreamableHttpError,
    StreamableHttpPostResponse,
};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc as StdArc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use sse_stream::{Sse, SseStream};

#[derive(Clone)]
struct BitFunRmcpClientHandler {
    info: ClientInfo,
}

impl ClientHandler for BitFunRmcpClientHandler {
    fn get_info(&self) -> ClientInfo {
        self.info.clone()
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: rmcp::service::NotificationContext<RoleClient>,
    ) {
        let LoggingMessageNotificationParam {
            level,
            logger,
            data,
        } = params;
        let logger = logger.as_deref();
        match level {
            LoggingLevel::Critical | LoggingLevel::Error => {
                error!(
                    "MCP server log message: level={:?} logger={:?} data={}",
                    level, logger, data
                );
            }
            LoggingLevel::Warning => {
                warn!(
                    "MCP server log message: level={:?} logger={:?} data={}",
                    level, logger, data
                );
            }
            LoggingLevel::Notice | LoggingLevel::Info => {
                info!(
                    "MCP server log message: level={:?} logger={:?} data={}",
                    level, logger, data
                );
            }
            LoggingLevel::Debug => {
                debug!(
                    "MCP server log message: level={:?} logger={:?} data={}",
                    level, logger, data
                );
            }
            // Keep a default arm in case rmcp adds new levels.
            _ => {
                info!(
                    "MCP server log message: level={:?} logger={:?} data={}",
                    level, logger, data
                );
            }
        }
    }
}

enum ClientState {
    Connecting {
        transport: Option<StreamableHttpClientTransport<BitFunStreamableHttpClient>>,
    },
    Ready {
        service: Arc<RunningService<RoleClient, BitFunRmcpClientHandler>>,
    },
}

#[derive(Clone)]
struct BitFunStreamableHttpClient {
    client: reqwest::Client,
    oauth_manager: Option<Arc<Mutex<AuthorizationManager>>>,
}

impl BitFunStreamableHttpClient {
    async fn resolve_auth_token(
        &self,
        auth_token: Option<String>,
    ) -> Result<Option<String>, StreamableHttpError<reqwest::Error>> {
        if auth_token.is_some() {
            return Ok(auth_token);
        }

        let Some(oauth_manager) = &self.oauth_manager else {
            return Ok(None);
        };

        let token = oauth_manager.lock().await.get_access_token().await?;
        Ok(Some(token))
    }
}

impl StreamableHttpClient for BitFunStreamableHttpClient {
    type Error = reqwest::Error;

    async fn get_stream(
        &self,
        uri: StdArc<str>,
        session_id: StdArc<str>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<Sse, SseError>>,
        StreamableHttpError<Self::Error>,
    > {
        let auth_token = self.resolve_auth_token(auth_token).await?;
        let mut request_builder = self
            .client
            .get(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "))
            .header(HEADER_SESSION_ID, session_id.as_ref());
        if let Some(last_event_id) = last_event_id {
            request_builder = request_builder.header(HEADER_LAST_EVENT_ID, last_event_id);
        }
        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }

        let response = request_builder.send().await?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        let response = response.error_for_status()?;

        match response.headers().get(CONTENT_TYPE) {
            Some(ct) => {
                if !ct.as_bytes().starts_with(EVENT_STREAM_MIME_TYPE.as_bytes())
                    && !ct.as_bytes().starts_with(JSON_MIME_TYPE.as_bytes())
                {
                    return Err(StreamableHttpError::UnexpectedContentType(Some(
                        String::from_utf8_lossy(ct.as_bytes()).to_string(),
                    )));
                }
            }
            None => {
                return Err(StreamableHttpError::UnexpectedContentType(None));
            }
        }

        let event_stream = SseStream::from_byte_stream(response.bytes_stream()).boxed();
        Ok(event_stream)
    }

    async fn delete_session(
        &self,
        uri: StdArc<str>,
        session: StdArc<str>,
        auth_token: Option<String>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        let auth_token = self.resolve_auth_token(auth_token).await?;
        let mut request_builder = self.client.delete(uri.as_ref());
        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }
        let response = request_builder
            .header(HEADER_SESSION_ID, session.as_ref())
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        let _ = response.error_for_status()?;
        Ok(())
    }

    async fn post_message(
        &self,
        uri: StdArc<str>,
        message: rmcp::model::ClientJsonRpcMessage,
        session_id: Option<StdArc<str>>,
        auth_token: Option<String>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let auth_token = self.resolve_auth_token(auth_token).await?;
        let mut request = self
            .client
            .post(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "));
        if let Some(auth_header) = auth_token {
            request = request.bearer_auth(auth_header);
        }
        if let Some(session_id) = session_id {
            request = request.header(HEADER_SESSION_ID, session_id.as_ref());
        }

        let response = request.json(&message).send().await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(header) = response.headers().get(WWW_AUTHENTICATE) {
                let header = header
                    .to_str()
                    .map_err(|_| {
                        StreamableHttpError::UnexpectedServerResponse(std::borrow::Cow::from(
                            "invalid www-authenticate header value",
                        ))
                    })?
                    .to_string();
                return Err(StreamableHttpError::AuthRequired(AuthRequiredError {
                    www_authenticate_header: header,
                }));
            }
        }

        let status = response.status();
        let response = response.error_for_status()?;

        if matches!(
            status,
            reqwest::StatusCode::ACCEPTED | reqwest::StatusCode::NO_CONTENT
        ) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }

        let session_id = response
            .headers()
            .get(HEADER_SESSION_ID)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .map(|s| s.to_string());

        match content_type.as_deref() {
            Some(ct) if ct.as_bytes().starts_with(EVENT_STREAM_MIME_TYPE.as_bytes()) => {
                let event_stream = SseStream::from_byte_stream(response.bytes_stream()).boxed();
                Ok(StreamableHttpPostResponse::Sse(event_stream, session_id))
            }
            Some(ct) if ct.as_bytes().starts_with(JSON_MIME_TYPE.as_bytes()) => {
                let message: rmcp::model::ServerJsonRpcMessage = response.json().await?;
                Ok(StreamableHttpPostResponse::Json(message, session_id))
            }
            _ => {
                // Compatibility: some servers return 200 with an empty body but omit Content-Type.
                // Treat this as Accepted for notifications (e.g. notifications/initialized).
                let bytes = response.bytes().await?;
                let trimmed = bytes
                    .iter()
                    .copied()
                    .skip_while(|b| b.is_ascii_whitespace())
                    .collect::<Vec<_>>();

                if status.is_success() && trimmed.is_empty() {
                    return Ok(StreamableHttpPostResponse::Accepted);
                }

                if let Ok(message) =
                    serde_json::from_slice::<rmcp::model::ServerJsonRpcMessage>(&bytes)
                {
                    return Ok(StreamableHttpPostResponse::Json(message, session_id));
                }

                Err(StreamableHttpError::UnexpectedContentType(content_type))
            }
        }
    }
}

/// Remote MCP transport backed by Streamable HTTP.
pub struct RemoteMCPTransport {
    url: String,
    default_headers: HeaderMap,
    oauth_manager: Option<Arc<Mutex<AuthorizationManager>>>,
    request_timeout: Option<Duration>,
    state: Mutex<ClientState>,
}

impl RemoteMCPTransport {
    fn build_default_headers(headers: &HashMap<String, String>) -> HeaderMap {
        let mut header_map = HeaderMap::new();

        for (name, value) in headers {
            let Ok(header_name) = HeaderName::from_str(name) else {
                warn!(
                    "Invalid HTTP header name in MCP config (skipping): {}",
                    name
                );
                continue;
            };

            let header_value_str = if header_name == reqwest::header::AUTHORIZATION {
                match normalize_mcp_authorization_value(value) {
                    Some(v) => v,
                    None => continue,
                }
            } else {
                value.trim().to_string()
            };

            let Ok(header_value) = HeaderValue::from_str(&header_value_str) else {
                warn!(
                    "Invalid HTTP header value in MCP config (skipping): header={}",
                    name
                );
                continue;
            };

            header_map.insert(header_name, header_value);
        }

        if !header_map.contains_key(USER_AGENT) {
            header_map.insert(
                USER_AGENT,
                HeaderValue::from_static("BitFun-MCP-Client/1.0"),
            );
        }

        header_map
    }

    /// Creates a new streamable HTTP remote transport instance.
    pub async fn new(
        server_id: &str,
        url: String,
        headers: HashMap<String, String>,
        request_timeout: Option<Duration>,
        oauth_enabled: bool,
    ) -> BitFunResult<Self> {
        let default_headers = Self::build_default_headers(&headers);
        let oauth_manager = if oauth_enabled
            && !default_headers.contains_key(reqwest::header::AUTHORIZATION)
        {
            let (manager, initialized) = build_authorization_manager(server_id, &url).await?;
            if initialized {
                Some(Arc::new(Mutex::new(manager)))
            } else {
                info!(
                    "Remote MCP OAuth configured but credentials are not authorized yet: server_id={}",
                    server_id
                );
                None
            }
        } else {
            None
        };

        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(false)
            .use_rustls_tls()
            .default_headers(default_headers.clone())
            .build()
            .unwrap_or_else(|e| {
                warn!("Failed to create HTTP client, using default config: {}", e);
                reqwest::Client::new()
            });

        let transport = StreamableHttpClientTransport::with_client(
            BitFunStreamableHttpClient {
                client: http_client,
                oauth_manager: oauth_manager.clone(),
            },
            StreamableHttpClientTransportConfig::with_uri(url.clone()),
        );

        Ok(Self {
            url,
            default_headers,
            oauth_manager,
            request_timeout,
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
        })
    }

    async fn await_with_optional_timeout<F, T>(
        timeout: Option<Duration>,
        future: F,
        timeout_message: impl Into<String>,
    ) -> BitFunResult<T>
    where
        F: Future<Output = T>,
    {
        if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, future)
                .await
                .map_err(|_| BitFunError::Timeout(timeout_message.into()))
        } else {
            Ok(future.await)
        }
    }

    /// Returns the auth token header value (if present).
    pub async fn get_auth_token(&self) -> Option<String> {
        if let Some(value) = self
            .default_headers
            .get(reqwest::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
        {
            return Some(value);
        }

        let oauth_manager = self.oauth_manager.as_ref()?;
        oauth_manager
            .lock()
            .await
            .get_access_token()
            .await
            .ok()
            .map(|token| format!("Bearer {}", token))
    }

    async fn service(
        &self,
    ) -> BitFunResult<Arc<RunningService<RoleClient, BitFunRmcpClientHandler>>> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready { service } => Ok(Arc::clone(service)),
            ClientState::Connecting { .. } => Err(BitFunError::MCPError(
                "Remote MCP client not initialized".to_string(),
            )),
        }
    }

    /// Initializes the remote connection (Streamable HTTP handshake).
    pub async fn initialize(
        &self,
        client_name: &str,
        client_version: &str,
    ) -> BitFunResult<BitFunInitializeResult> {
        let mut guard = self.state.lock().await;
        match &mut *guard {
            ClientState::Ready { service } => {
                let info = service.peer().peer_info().ok_or_else(|| {
                    BitFunError::MCPError("Handshake succeeded but server info missing".to_string())
                })?;
                Ok(map_rmcp_initialize_result(info))
            }
            ClientState::Connecting { transport } => {
                let Some(transport) = transport.take() else {
                    return Err(BitFunError::MCPError(
                        "Remote MCP client already initializing".to_string(),
                    ));
                };

                let handler = BitFunRmcpClientHandler {
                    info: create_mcp_client_info(client_name, client_version),
                };

                drop(guard);

                let transport_fut = rmcp::serve_client(handler.clone(), transport);
                let service = Self::await_with_optional_timeout(
                    self.request_timeout,
                    transport_fut,
                    format!("Timed out handshaking with MCP server: {}", self.url),
                )
                .await?
                .map_err(|e| BitFunError::MCPError(format!("Handshake failed: {}", e)))?;

                let service = Arc::new(service);
                let info = service.peer().peer_info().ok_or_else(|| {
                    BitFunError::MCPError("Handshake succeeded but server info missing".to_string())
                })?;

                let mut guard = self.state.lock().await;
                *guard = ClientState::Ready {
                    service: Arc::clone(&service),
                };

                Ok(map_rmcp_initialize_result(info))
            }
        }
    }

    /// Sends `ping` (heartbeat check).
    pub async fn ping(&self) -> BitFunResult<()> {
        let service = self.service().await?;
        let fut = service.send_request(rmcp::model::ClientRequest::PingRequest(
            RequestNoParam::default(),
        ));
        let result = Self::await_with_optional_timeout(
            self.request_timeout,
            fut,
            "MCP ping timeout".to_string(),
        )
        .await?
        .map_err(|e| BitFunError::MCPError(format!("MCP ping failed: {}", e)))?;

        match result {
            rmcp::model::ServerResult::EmptyResult(_) => Ok(()),
            other => Err(BitFunError::MCPError(format!(
                "Unexpected ping response: {:?}",
                other
            ))),
        }
    }

    pub async fn list_resources(
        &self,
        cursor: Option<String>,
    ) -> BitFunResult<ResourcesListResult> {
        let service = self.service().await?;
        let fut = service
            .peer()
            .list_resources(Some(PaginatedRequestParam { cursor }));
        let result = Self::await_with_optional_timeout(
            self.request_timeout,
            fut,
            "MCP resources/list timeout".to_string(),
        )
        .await?
        .map_err(|e| BitFunError::MCPError(format!("MCP resources/list failed: {}", e)))?;
        Ok(ResourcesListResult {
            resources: result
                .resources
                .into_iter()
                .map(map_rmcp_resource)
                .collect(),
            next_cursor: result.next_cursor,
        })
    }

    pub async fn read_resource(&self, uri: &str) -> BitFunResult<ResourcesReadResult> {
        let service = self.service().await?;
        let fut = service.peer().read_resource(ReadResourceRequestParam {
            uri: uri.to_string(),
        });
        let result = Self::await_with_optional_timeout(
            self.request_timeout,
            fut,
            "MCP resources/read timeout".to_string(),
        )
        .await?
        .map_err(|e| BitFunError::MCPError(format!("MCP resources/read failed: {}", e)))?;
        Ok(ResourcesReadResult {
            contents: result
                .contents
                .into_iter()
                .map(map_rmcp_resource_content)
                .collect(),
        })
    }

    pub async fn list_prompts(&self, cursor: Option<String>) -> BitFunResult<PromptsListResult> {
        let service = self.service().await?;
        let fut = service
            .peer()
            .list_prompts(Some(PaginatedRequestParam { cursor }));
        let result = Self::await_with_optional_timeout(
            self.request_timeout,
            fut,
            "MCP prompts/list timeout".to_string(),
        )
        .await?
        .map_err(|e| BitFunError::MCPError(format!("MCP prompts/list failed: {}", e)))?;
        Ok(PromptsListResult {
            prompts: result.prompts.into_iter().map(map_rmcp_prompt).collect(),
            next_cursor: result.next_cursor,
        })
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<HashMap<String, String>>,
    ) -> BitFunResult<PromptsGetResult> {
        let service = self.service().await?;

        let arguments = arguments.map(|args| {
            let mut obj = JsonObject::new();
            for (k, v) in args {
                obj.insert(k, Value::String(v));
            }
            obj
        });

        let fut = service.peer().get_prompt(GetPromptRequestParam {
            name: name.to_string(),
            arguments,
        });
        let result = Self::await_with_optional_timeout(
            self.request_timeout,
            fut,
            "MCP prompts/get timeout".to_string(),
        )
        .await?
        .map_err(|e| BitFunError::MCPError(format!("MCP prompts/get failed: {}", e)))?;

        Ok(PromptsGetResult {
            description: result.description,
            messages: result
                .messages
                .into_iter()
                .map(map_rmcp_prompt_message)
                .collect(),
        })
    }

    pub async fn list_tools(&self, cursor: Option<String>) -> BitFunResult<ToolsListResult> {
        let service = self.service().await?;
        let fut = service
            .peer()
            .list_tools(Some(PaginatedRequestParam { cursor }));
        let result = Self::await_with_optional_timeout(
            self.request_timeout,
            fut,
            "MCP tools/list timeout".to_string(),
        )
        .await?
        .map_err(|e| BitFunError::MCPError(format!("MCP tools/list failed: {}", e)))?;

        Ok(ToolsListResult {
            tools: result.tools.into_iter().map(map_rmcp_tool).collect(),
            next_cursor: result.next_cursor,
        })
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> BitFunResult<MCPToolResult> {
        let service = self.service().await?;

        let arguments = match arguments {
            None => None,
            Some(Value::Object(map)) => Some(map),
            Some(other) => {
                return Err(BitFunError::Validation(format!(
                    "MCP tool arguments must be an object, got: {}",
                    other
                )));
            }
        };

        let fut = service.peer().call_tool(CallToolRequestParam {
            name: name.to_string().into(),
            arguments,
        });
        let result = Self::await_with_optional_timeout(
            self.request_timeout,
            fut,
            "MCP tools/call timeout".to_string(),
        )
        .await?
        .map_err(|e| BitFunError::MCPError(format!("MCP tools/call failed: {}", e)))?;

        Ok(map_rmcp_tool_result(result))
    }
}
