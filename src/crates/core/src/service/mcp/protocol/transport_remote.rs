//! Remote MCP transport (Streamable HTTP)
//!
//! Uses the official `rmcp` Rust SDK to implement the MCP Streamable HTTP client transport.

use super::types::{
    InitializeResult as BitFunInitializeResult, MCPAnnotations, MCPCapability, MCPPrompt,
    MCPPromptArgument, MCPPromptMessage, MCPPromptMessageContent, MCPPromptMessageContentBlock,
    MCPResource, MCPResourceContent, MCPResourceIcon, MCPServerInfo, MCPTool, MCPToolAnnotations,
    MCPToolResult, MCPToolResultContent, PromptsGetResult, PromptsListResult, ResourcesListResult,
    ResourcesReadResult, ToolsListResult,
};
use crate::service::mcp::auth::build_authorization_manager;
use crate::util::errors::{BitFunError, BitFunResult};
use futures::StreamExt;
use log::{debug, error, info, warn};
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT, WWW_AUTHENTICATE,
};
use rmcp::model::{
    CallToolRequestParam, ClientCapabilities, ClientInfo, Content, GetPromptRequestParam,
    Implementation, JsonObject, LoggingLevel, LoggingMessageNotificationParam,
    PaginatedRequestParam, ProtocolVersion, ReadResourceRequestParam, RequestNoParam,
    ResourceContents,
};
use rmcp::service::RunningService;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::common::http_header::{
    EVENT_STREAM_MIME_TYPE, HEADER_LAST_EVENT_ID, HEADER_SESSION_ID, JSON_MIME_TYPE,
};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::streamable_http_client::{
    AuthRequiredError, SseError, StreamableHttpClient, StreamableHttpError,
    StreamableHttpPostResponse,
};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ClientHandler;
use rmcp::RoleClient;
use serde::de::DeserializeOwned;
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
    fn normalize_authorization_value(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        // If already includes a scheme (e.g. `Bearer xxx`), keep as-is.
        if trimmed.to_ascii_lowercase().starts_with("bearer ") {
            return Some(trimmed.to_string());
        }
        if trimmed.contains(char::is_whitespace) {
            return Some(trimmed.to_string());
        }

        // If the user provided a raw token, assume Bearer.
        Some(format!("Bearer {}", trimmed))
    }

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
                match Self::normalize_authorization_value(value) {
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

    fn build_client_info(client_name: &str, client_version: &str) -> ClientInfo {
        ClientInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ClientCapabilities::builder()
                .enable_roots()
                .enable_sampling()
                .enable_elicitation_with(rmcp::model::ElicitationCapability {
                    schema_validation: Some(true),
                })
                .build(),
            client_info: Implementation {
                name: client_name.to_string(),
                title: None,
                version: client_version.to_string(),
                icons: None,
                website_url: None,
            },
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
                Ok(map_initialize_result(info))
            }
            ClientState::Connecting { transport } => {
                let Some(transport) = transport.take() else {
                    return Err(BitFunError::MCPError(
                        "Remote MCP client already initializing".to_string(),
                    ));
                };

                let handler = BitFunRmcpClientHandler {
                    info: Self::build_client_info(client_name, client_version),
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

                Ok(map_initialize_result(info))
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
            resources: result.resources.into_iter().map(map_resource).collect(),
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
                .map(map_resource_content)
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
            prompts: result.prompts.into_iter().map(map_prompt).collect(),
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
                .map(map_prompt_message)
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
            tools: result.tools.into_iter().map(map_tool).collect(),
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

        Ok(map_tool_result(result))
    }
}

fn map_initialize_result(info: &rmcp::model::ServerInfo) -> BitFunInitializeResult {
    BitFunInitializeResult {
        protocol_version: info.protocol_version.to_string(),
        capabilities: map_server_capabilities(&info.capabilities),
        server_info: MCPServerInfo {
            name: info.server_info.name.clone(),
            version: info.server_info.version.clone(),
            description: info.server_info.title.clone().or(info.instructions.clone()),
            vendor: None,
        },
    }
}

fn map_server_capabilities(cap: &rmcp::model::ServerCapabilities) -> MCPCapability {
    MCPCapability {
        resources: cap
            .resources
            .as_ref()
            .map(|r| super::types::ResourcesCapability {
                subscribe: r.subscribe.unwrap_or(false),
                list_changed: r.list_changed.unwrap_or(false),
            }),
        prompts: cap
            .prompts
            .as_ref()
            .map(|p| super::types::PromptsCapability {
                list_changed: p.list_changed.unwrap_or(false),
            }),
        tools: cap.tools.as_ref().map(|t| super::types::ToolsCapability {
            list_changed: t.list_changed.unwrap_or(false),
        }),
        logging: cap.logging.as_ref().map(|o| Value::Object(o.clone())),
    }
}

fn map_tool(tool: rmcp::model::Tool) -> MCPTool {
    let schema = Value::Object((*tool.input_schema).clone());
    MCPTool {
        name: tool.name.to_string(),
        title: tool.title,
        description: tool.description.map(|d| d.to_string()),
        input_schema: schema,
        output_schema: tool
            .output_schema
            .map(|schema| Value::Object((*schema).clone())),
        icons: map_icons(tool.icons.as_ref()),
        annotations: tool.annotations.map(map_tool_annotations),
        meta: map_optional_via_json(tool.meta.as_ref()),
    }
}

fn map_resource(resource: rmcp::model::Resource) -> MCPResource {
    MCPResource {
        uri: resource.uri.clone(),
        name: resource.name.clone(),
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: resource.mime_type.clone(),
        icons: map_icons(resource.icons.as_ref()),
        size: resource.size.map(u64::from),
        annotations: map_annotations(resource.annotations.as_ref()),
        metadata: map_meta_to_hash_map(resource.meta.as_ref()),
    }
}

fn map_resource_content(contents: ResourceContents) -> MCPResourceContent {
    match contents {
        ResourceContents::TextResourceContents {
            uri,
            mime_type,
            text,
            meta,
            ..
        } => MCPResourceContent {
            uri,
            content: Some(text),
            blob: None,
            mime_type,
            annotations: None,
            meta: map_optional_via_json(meta.as_ref()),
        },
        ResourceContents::BlobResourceContents {
            uri,
            mime_type,
            blob,
            meta,
            ..
        } => MCPResourceContent {
            uri,
            content: None,
            blob: Some(blob),
            mime_type,
            annotations: None,
            meta: map_optional_via_json(meta.as_ref()),
        },
    }
}

fn map_prompt(prompt: rmcp::model::Prompt) -> MCPPrompt {
    MCPPrompt {
        name: prompt.name,
        title: prompt.title,
        description: prompt.description,
        arguments: prompt.arguments.map(|args| {
            args.into_iter()
                .map(|a| MCPPromptArgument {
                    name: a.name,
                    title: a.title,
                    description: a.description,
                    required: a.required.unwrap_or(false),
                })
                .collect()
        }),
        icons: map_icons(prompt.icons.as_ref()),
    }
}

fn map_prompt_message(message: rmcp::model::PromptMessage) -> MCPPromptMessage {
    let role = match message.role {
        rmcp::model::PromptMessageRole::User => "user",
        rmcp::model::PromptMessageRole::Assistant => "assistant",
    }
    .to_string();

    let content = match message.content {
        rmcp::model::PromptMessageContent::Text { text } => {
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::Text { text }))
        }
        rmcp::model::PromptMessageContent::Image { image } => {
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::Image {
                data: image.data.clone(),
                mime_type: image.mime_type.clone(),
            }))
        }
        rmcp::model::PromptMessageContent::Resource { resource } => {
            let mut mapped = map_resource_content(resource.resource.clone());
            if mapped.meta.is_none() {
                mapped.meta = map_optional_via_json(resource.meta.as_ref());
            }
            mapped.annotations = map_annotations(resource.annotations.as_ref());
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::Resource {
                resource: Box::new(mapped),
            }))
        }
        rmcp::model::PromptMessageContent::ResourceLink { link } => {
            MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::ResourceLink {
                uri: link.uri.clone(),
                name: Some(link.name.clone()),
                description: link.description.clone(),
                mime_type: link.mime_type.clone(),
            }))
        }
    };

    MCPPromptMessage { role, content }
}

fn map_tool_result(result: rmcp::model::CallToolResult) -> MCPToolResult {
    let mapped: Vec<MCPToolResultContent> = result
        .content
        .into_iter()
        .filter_map(map_content_block)
        .collect();

    MCPToolResult {
        content: if mapped.is_empty() {
            None
        } else {
            Some(mapped)
        },
        is_error: result.is_error.unwrap_or(false),
        structured_content: result.structured_content,
        meta: map_optional_json_value(result.meta.as_ref()),
    }
}

fn map_content_block(content: Content) -> Option<MCPToolResultContent> {
    match content.raw {
        rmcp::model::RawContent::Text(text) => Some(MCPToolResultContent::Text { text: text.text }),
        rmcp::model::RawContent::Image(image) => Some(MCPToolResultContent::Image {
            data: image.data,
            mime_type: image.mime_type,
        }),
        rmcp::model::RawContent::Resource(resource) => Some(MCPToolResultContent::Resource {
            resource: Box::new(map_resource_content(resource.resource)),
        }),
        rmcp::model::RawContent::Audio(audio) => Some(MCPToolResultContent::Audio {
            data: audio.data,
            mime_type: audio.mime_type,
        }),
        rmcp::model::RawContent::ResourceLink(link) => Some(MCPToolResultContent::ResourceLink {
            uri: link.uri,
            name: Some(link.name),
            description: link.description,
            mime_type: link.mime_type,
        }),
    }
}

fn map_icons(icons: Option<&Vec<rmcp::model::Icon>>) -> Option<Vec<MCPResourceIcon>> {
    icons.map(|icons| {
        icons
            .iter()
            .map(|icon| MCPResourceIcon {
                src: icon.src.clone(),
                mime_type: icon.mime_type.clone(),
                sizes: icon.sizes.as_ref().map(|sizes| {
                    Value::Array(sizes.iter().cloned().map(Value::String).collect::<Vec<_>>())
                }),
            })
            .collect()
    })
}

fn map_annotations(annotations: Option<&rmcp::model::Annotations>) -> Option<MCPAnnotations> {
    annotations.map(|annotations| MCPAnnotations {
        audience: annotations
            .audience
            .as_ref()
            .map(|audience| audience.iter().map(map_role).collect()),
        priority: annotations.priority.map(f64::from),
        last_modified: annotations
            .last_modified
            .map(|timestamp| timestamp.to_rfc3339()),
    })
}

fn map_tool_annotations(annotations: rmcp::model::ToolAnnotations) -> MCPToolAnnotations {
    MCPToolAnnotations {
        title: annotations.title,
        read_only_hint: annotations.read_only_hint,
        destructive_hint: annotations.destructive_hint,
        idempotent_hint: annotations.idempotent_hint,
        open_world_hint: annotations.open_world_hint,
    }
}

fn map_role(role: &rmcp::model::Role) -> String {
    match role {
        rmcp::model::Role::User => "user",
        rmcp::model::Role::Assistant => "assistant",
    }
    .to_string()
}

fn map_meta_to_hash_map(meta: Option<&rmcp::model::Meta>) -> Option<HashMap<String, Value>> {
    meta.and_then(|meta| match serde_json::to_value(meta.clone()).ok()? {
        Value::Object(map) => Some(map.into_iter().collect()),
        _ => None,
    })
}

fn map_optional_json_value<T>(value: Option<&T>) -> Option<Value>
where
    T: serde::Serialize,
{
    value.and_then(|value| serde_json::to_value(value).ok())
}

fn map_optional_via_json<T, U>(value: Option<&T>) -> Option<U>
where
    T: serde::Serialize,
    U: DeserializeOwned,
{
    value
        .and_then(|value| serde_json::to_value(value).ok())
        .and_then(|value| serde_json::from_value(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{AnnotateAble, Annotations, Content, Icon, Meta, RawResource};
    use serde_json::json;

    #[test]
    fn build_client_info_declares_supported_client_capabilities() {
        let info = RemoteMCPTransport::build_client_info("BitFun", "1.0.0");

        assert!(info.capabilities.roots.is_some());
        assert!(info.capabilities.sampling.is_some());
        assert!(info.capabilities.elicitation.is_some());
        assert_eq!(
            info.capabilities
                .elicitation
                .as_ref()
                .and_then(|cap| cap.schema_validation),
            Some(true)
        );
    }

    #[test]
    fn mapping_preserves_remote_tool_resource_and_prompt_metadata() {
        let mut tool_meta = Meta::default();
        tool_meta.insert("ui".to_string(), json!({ "resourceUri": "ui://widget" }));
        let tool = rmcp::model::Tool {
            name: "search".into(),
            title: Some("Search".to_string()),
            description: Some("Find items".into()),
            input_schema: Arc::new(serde_json::Map::new()),
            output_schema: Some(Arc::new(serde_json::Map::from_iter([(
                "type".to_string(),
                json!("object"),
            )]))),
            annotations: Some(
                rmcp::model::ToolAnnotations::new()
                    .read_only(true)
                    .destructive(false)
                    .idempotent(true)
                    .open_world(true),
            ),
            icons: Some(vec![Icon {
                src: "https://example.com/tool.png".to_string(),
                mime_type: Some("image/png".to_string()),
                sizes: Some(vec!["32x32".to_string()]),
            }]),
            meta: Some(tool_meta),
        };
        let mapped_tool = map_tool(tool);
        assert_eq!(mapped_tool.title.as_deref(), Some("Search"));
        assert_eq!(mapped_tool.output_schema, Some(json!({ "type": "object" })));
        assert_eq!(
            mapped_tool
                .annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint),
            Some(true)
        );
        assert_eq!(
            mapped_tool
                .meta
                .as_ref()
                .and_then(|meta| meta.ui.as_ref())
                .and_then(|ui| ui.resource_uri.as_deref()),
            Some("ui://widget")
        );

        let mut resource_meta = Meta::default();
        resource_meta.insert("source".to_string(), json!("catalog"));
        let resource = RawResource {
            uri: "file:///tmp/report.md".to_string(),
            name: "report".to_string(),
            title: Some("Quarterly Report".to_string()),
            description: Some("Report".to_string()),
            mime_type: Some("text/markdown".to_string()),
            size: Some(42),
            icons: Some(vec![Icon {
                src: "https://example.com/resource.png".to_string(),
                mime_type: Some("image/png".to_string()),
                sizes: Some(vec!["64x64".to_string()]),
            }]),
            meta: Some(resource_meta),
        }
        .annotate(Annotations {
            audience: Some(vec![rmcp::model::Role::User]),
            priority: Some(0.9),
            last_modified: None,
        });
        let mapped_resource = map_resource(resource);
        assert_eq!(mapped_resource.title.as_deref(), Some("Quarterly Report"));
        assert_eq!(mapped_resource.size, Some(42));
        assert_eq!(
            mapped_resource
                .annotations
                .as_ref()
                .and_then(|annotations| annotations.audience.as_ref())
                .cloned(),
            Some(vec!["user".to_string()])
        );
        assert_eq!(
            mapped_resource
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("source")),
            Some(&json!("catalog"))
        );

        let prompt = rmcp::model::Prompt {
            name: "summarize".to_string(),
            title: Some("Summarize".to_string()),
            description: Some("Summarize content".to_string()),
            arguments: Some(vec![rmcp::model::PromptArgument {
                name: "topic".to_string(),
                title: Some("Topic".to_string()),
                description: Some("Topic to summarize".to_string()),
                required: Some(true),
            }]),
            icons: Some(vec![Icon {
                src: "https://example.com/prompt.png".to_string(),
                mime_type: Some("image/png".to_string()),
                sizes: Some(vec!["16x16".to_string()]),
            }]),
            meta: None,
        };
        let mapped_prompt = map_prompt(prompt);
        assert_eq!(mapped_prompt.title.as_deref(), Some("Summarize"));
        assert_eq!(
            mapped_prompt
                .arguments
                .as_ref()
                .and_then(|arguments| arguments.first())
                .and_then(|argument| argument.title.as_deref()),
            Some("Topic")
        );
        assert!(mapped_prompt.icons.is_some());
    }

    #[test]
    fn mapping_preserves_structured_results_and_resource_links() {
        let resource_link = RawResource {
            uri: "file:///tmp/output.json".to_string(),
            name: "output".to_string(),
            title: Some("Output".to_string()),
            description: Some("Generated output".to_string()),
            mime_type: Some("application/json".to_string()),
            size: Some(7),
            icons: None,
            meta: None,
        };
        let mut result_meta = Meta::default();
        result_meta.insert("traceId".to_string(), json!("abc123"));
        let result = rmcp::model::CallToolResult {
            content: vec![
                Content::text("done"),
                Content::resource_link(resource_link),
                Content::image("aGVsbG8=", "image/png"),
            ],
            structured_content: Some(json!({ "ok": true })),
            is_error: Some(false),
            meta: Some(result_meta),
        };

        let mapped = map_tool_result(result);
        assert_eq!(mapped.structured_content, Some(json!({ "ok": true })));
        assert_eq!(mapped.meta, Some(json!({ "traceId": "abc123" })));
        assert!(matches!(
            mapped.content.as_ref().and_then(|content| content.get(1)),
            Some(MCPToolResultContent::ResourceLink { uri, .. }) if uri == "file:///tmp/output.json"
        ));
        assert!(matches!(
            mapped.content.as_ref().and_then(|content| content.get(2)),
            Some(MCPToolResultContent::Image { mime_type, .. }) if mime_type == "image/png"
        ));
    }

    #[test]
    fn mapping_preserves_prompt_message_blocks() {
        let prompt_message = rmcp::model::PromptMessage {
            role: rmcp::model::PromptMessageRole::User,
            content: rmcp::model::PromptMessageContent::Text {
                text: "hello".to_string(),
            },
        };
        let mapped = map_prompt_message(prompt_message);
        assert!(matches!(
            mapped.content,
            MCPPromptMessageContent::Block(ref block)
                if matches!(block.as_ref(), MCPPromptMessageContentBlock::Text { text } if text == "hello")
        ));

        let resource_link = RawResource {
            uri: "file:///tmp/input.md".to_string(),
            name: "input".to_string(),
            title: None,
            description: Some("input".to_string()),
            mime_type: Some("text/markdown".to_string()),
            size: None,
            icons: None,
            meta: None,
        }
        .no_annotation();
        let prompt_message = rmcp::model::PromptMessage {
            role: rmcp::model::PromptMessageRole::Assistant,
            content: rmcp::model::PromptMessageContent::ResourceLink {
                link: resource_link,
            },
        };
        let mapped = map_prompt_message(prompt_message);
        assert!(matches!(
            mapped.content,
            MCPPromptMessageContent::Block(ref block)
                if matches!(
                    block.as_ref(),
                    MCPPromptMessageContentBlock::ResourceLink { uri, .. }
                        if uri == "file:///tmp/input.md"
                )
        ));

        let embedded = rmcp::model::RawEmbeddedResource {
            meta: Some(Meta::default()),
            resource: ResourceContents::TextResourceContents {
                uri: "file:///tmp/embedded.txt".to_string(),
                mime_type: Some("text/plain".to_string()),
                text: "embedded".to_string(),
                meta: None,
            },
        }
        .no_annotation();
        let prompt_message = rmcp::model::PromptMessage {
            role: rmcp::model::PromptMessageRole::Assistant,
            content: rmcp::model::PromptMessageContent::Resource { resource: embedded },
        };
        let mapped = map_prompt_message(prompt_message);
        assert!(matches!(
            mapped.content,
            MCPPromptMessageContent::Block(ref block)
                if matches!(
                    block.as_ref(),
                    MCPPromptMessageContentBlock::Resource { resource }
                        if resource.uri == "file:///tmp/embedded.txt"
                )
        ));
    }
}
