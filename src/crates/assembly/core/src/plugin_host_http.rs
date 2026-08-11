use bitfun_opencode_plugin_host::{
    json_error_body, match_http_route, read_host_stream, BackendHttpRequest, BackendHttpResponse,
    HostStreamReadError, HttpRouteError, OpenCodeClientRoute, PluginHostClient,
    PluginHostStreamRegistry, RpcHandlerError, StreamCancelParams, StreamReadParams,
    StreamRegistryError, MAX_HTTP_BODY_BYTES,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Notify, OnceCell};

const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_DRAIN_TIMEOUT: Duration = Duration::from_secs(3);

static PLUGIN_HOST_BACKEND_BRIDGE: OnceCell<Arc<PluginHostBackendBridge>> = OnceCell::const_new();

pub(crate) struct PluginHostBackendBridge {
    client: PluginHostClient,
    streams: PluginHostStreamRegistry,
    draining: AtomicBool,
    active_requests: AtomicUsize,
    requests_drained: Notify,
}

struct ActiveRequest<'a> {
    bridge: &'a PluginHostBackendBridge,
}

impl Drop for ActiveRequest<'_> {
    fn drop(&mut self) {
        if self.bridge.active_requests.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.bridge.requests_drained.notify_waiters();
        }
    }
}

#[derive(Debug)]
pub(crate) struct RouteFailure {
    status: u16,
    code: &'static str,
    message: String,
}

impl RouteFailure {
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, "invalid_request", message)
    }

    pub(crate) fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403, "instance_scope_denied", message)
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }

    pub(crate) fn backend(message: impl Into<String>) -> Self {
        Self::new(502, "backend_failure", message)
    }

    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self::new(503, "backend_unavailable", message)
    }

    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl PluginHostBackendBridge {
    fn new(client: PluginHostClient) -> Self {
        Self {
            client,
            streams: PluginHostStreamRegistry::default(),
            draining: AtomicBool::new(false),
            active_requests: AtomicUsize::new(0),
            requests_drained: Notify::new(),
        }
    }

    fn admit(&self) -> Option<ActiveRequest<'_>> {
        if self.draining.load(Ordering::Acquire) {
            return None;
        }
        self.active_requests.fetch_add(1, Ordering::AcqRel);
        if self.draining.load(Ordering::Acquire) {
            if self.active_requests.fetch_sub(1, Ordering::AcqRel) == 1 {
                self.requests_drained.notify_waiters();
            }
            return None;
        }
        Some(ActiveRequest { bridge: self })
    }

    async fn handle_http(self: Arc<Self>, params: Value) -> Result<Value, RpcHandlerError> {
        let request: BackendHttpRequest = serde_json::from_value(params)
            .map_err(|error| invalid_rpc_params("backend.http.request", error))?;
        if request.instance_id.trim().is_empty()
            || request.instance_id.len() > 256
            || request.request_id.trim().is_empty()
            || request.request_id.len() > 256
            || request.method.len() > 16
            || request.headers.len() > 64
        {
            return Err(RpcHandlerError::new(
                -32602,
                "Invalid request identity, method, or header count for backend.http.request",
            ));
        }
        let started_at = Instant::now();
        let path = request.path.clone();
        let method = request.method.clone();
        let instance_id = request.instance_id.clone();
        let request_id = request.request_id.clone();
        let Some(_active) = self.admit() else {
            return self
                .http_error(
                    &instance_id,
                    503,
                    "host_draining",
                    "Plugin host is shutting down",
                    &path,
                )
                .await;
        };

        let route_match = match match_http_route(&method, &path) {
            Ok(route_match) => route_match,
            Err(HttpRouteError::InvalidPath) => {
                return self
                    .http_error(
                        &instance_id,
                        400,
                        "invalid_request",
                        "Request path is invalid",
                        &path,
                    )
                    .await
            }
            Err(HttpRouteError::NotFound) => {
                return self
                    .http_error(
                        &instance_id,
                        404,
                        "route_not_found",
                        "OpenCode client route was not found",
                        &path,
                    )
                    .await
            }
            Err(HttpRouteError::MethodNotAllowed) => {
                return self
                    .http_error(
                        &instance_id,
                        405,
                        "method_not_allowed",
                        "HTTP method is not allowed for this route",
                        &path,
                    )
                    .await
            }
        };
        let operation = route_match.route.operation();
        let context = match crate::plugin_host::plugin_host_instance_by_id(&instance_id).await {
            Some(context) => context,
            None => {
                return self
                    .http_error(
                        &instance_id,
                        404,
                        "instance_not_found",
                        "Plugin host instance was not found",
                        &path,
                    )
                    .await
            }
        };
        if !context.is_ready() {
            log::debug!(
                "Plugin client request admitted during activation: instance_id={}, request_id={}, operation={}",
                instance_id,
                request_id,
                operation
            );
        }
        if let Some(directory) = route_match.query_first("directory") {
            if !crate::plugin_host::instance_directories_equal(directory, &context.directory) {
                return self
                    .http_error(
                        &instance_id,
                        403,
                        "instance_scope_denied",
                        "Request directory does not belong to this plugin instance",
                        &path,
                    )
                    .await;
            }
        }
        if let Some(directory) = request.headers.iter().find_map(|(name, value)| {
            name.eq_ignore_ascii_case("x-opencode-directory")
                .then_some(value.as_str())
        }) {
            if !crate::plugin_host::instance_directories_equal(directory, &context.directory) {
                return self
                    .http_error(
                        &instance_id,
                        403,
                        "instance_scope_denied",
                        "Request directory does not belong to this plugin instance",
                        &path,
                    )
                    .await;
            }
        }
        let body = match request.body.as_ref() {
            Some(descriptor) => match read_host_stream(
                &self.client,
                &instance_id,
                descriptor,
                MAX_HTTP_BODY_BYTES,
                HTTP_REQUEST_TIMEOUT,
            )
            .await
            {
                Ok(body) => body,
                Err(HostStreamReadError::BodyTooLarge) => {
                    return self
                        .http_error(
                            &instance_id,
                            413,
                            "request_too_large",
                            "Request body exceeds the configured limit",
                            &path,
                        )
                        .await
                }
                Err(error) => {
                    return self
                        .http_error(
                            &instance_id,
                            502,
                            "backend_failure",
                            &format!("Failed to read request body: {error}"),
                            &path,
                        )
                        .await
                }
            },
            None => Vec::new(),
        };

        let outcome = tokio::time::timeout(
            HTTP_REQUEST_TIMEOUT,
            dispatch_route(&context, route_match.route, &route_match.query, &body),
        )
        .await;
        let response = match outcome {
            Ok(Ok(value)) => self.json_response(&instance_id, 200, value).await,
            Ok(Err(error)) => {
                self.http_error(
                    &instance_id,
                    error.status,
                    error.code,
                    &error.message,
                    &path,
                )
                .await
            }
            Err(_) => {
                self.http_error(
                    &instance_id,
                    504,
                    "backend_timeout",
                    "Backend route timed out",
                    &path,
                )
                .await
            }
        };
        let status = response
            .as_ref()
            .ok()
            .and_then(|value| value.get("status"))
            .and_then(Value::as_u64)
            .unwrap_or(500);
        log::info!(
            "Plugin client request completed: instance_id={}, request_id={}, method={}, path={}, status={}, duration_ms={}, route_status=A, operation={}",
            instance_id,
            request_id,
            method,
            path,
            status,
            u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
            operation
        );
        response
    }

    async fn json_response(
        &self,
        instance_id: &str,
        status: u16,
        value: Value,
    ) -> Result<Value, RpcHandlerError> {
        let bytes = serde_json::to_vec(&value).map_err(|error| {
            RpcHandlerError::new(
                -32603,
                format!("Failed to serialize HTTP response: {error}"),
            )
        })?;
        self.bytes_response(instance_id, status, "application/json", bytes)
            .await
    }

    async fn http_error(
        &self,
        instance_id: &str,
        status: u16,
        code: &str,
        message: &str,
        route: &str,
    ) -> Result<Value, RpcHandlerError> {
        self.bytes_response(
            instance_id,
            status,
            "application/json",
            json_error_body(code, message, route),
        )
        .await
    }

    async fn bytes_response(
        &self,
        instance_id: &str,
        status: u16,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<Value, RpcHandlerError> {
        let body = self
            .streams
            .add(instance_id, bytes)
            .await
            .map_err(stream_rpc_error)?;
        serde_json::to_value(BackendHttpResponse {
            status,
            status_text: None,
            headers: vec![("content-type".to_string(), content_type.to_string())],
            body: Some(body),
        })
        .map_err(|error| RpcHandlerError::new(-32603, error.to_string()))
    }

    pub(crate) async fn begin_draining(&self) {
        self.draining.store(true, Ordering::Release);
        let active_requests = self.active_requests.load(Ordering::Acquire);
        let active_streams = self.streams.active_count().await;
        log::info!(
            "Plugin client bridge draining started: active_requests={}, active_streams={}",
            active_requests,
            active_streams
        );
        let wait = async {
            loop {
                let notified = self.requests_drained.notified();
                if self.active_requests.load(Ordering::Acquire) == 0 {
                    return;
                }
                notified.await;
            }
        };
        if tokio::time::timeout(HTTP_DRAIN_TIMEOUT, wait)
            .await
            .is_err()
        {
            log::warn!(
                "Plugin client bridge request drain timed out: active_requests={}",
                self.active_requests.load(Ordering::Acquire)
            );
        }
        let streams_drained = self.streams.wait_until_empty(HTTP_DRAIN_TIMEOUT).await;
        if !streams_drained {
            log::warn!(
                "Plugin client bridge response stream drain timed out: active_streams={}",
                self.streams.active_count().await
            );
        }
        let cancelled = self.streams.cancel_all().await;
        log::info!(
            "Plugin client bridge draining completed: active_requests={}, cancelled_streams={}",
            self.active_requests.load(Ordering::Acquire),
            cancelled
        );
    }

    pub(crate) async fn cancel_instance_streams(&self, instance_id: &str) {
        let cancelled = self.streams.cancel_instance(instance_id).await;
        if cancelled > 0 {
            log::debug!(
                "Plugin client response streams cancelled: instance_id={}, stream_count={}",
                instance_id,
                cancelled
            );
        }
    }
}

pub(crate) async fn register_plugin_host_backend_handlers(
    client: PluginHostClient,
) -> crate::BitFunResult<Arc<PluginHostBackendBridge>> {
    let bridge = Arc::new(PluginHostBackendBridge::new(client.clone()));
    let http_bridge = bridge.clone();
    client
        .register_handler("backend.http.request", move |params| {
            let bridge = http_bridge.clone();
            async move { bridge.handle_http(params).await }
        })
        .await
        .map_err(plugin_host_handler_error)?;
    let read_bridge = bridge.clone();
    client
        .register_handler("backend.stream.read", move |params| {
            let bridge = read_bridge.clone();
            async move {
                let params: StreamReadParams = serde_json::from_value(params)
                    .map_err(|error| invalid_rpc_params("backend.stream.read", error))?;
                serde_json::to_value(
                    bridge
                        .streams
                        .read(params)
                        .await
                        .map_err(stream_rpc_error)?,
                )
                .map_err(|error| RpcHandlerError::new(-32603, error.to_string()))
            }
        })
        .await
        .map_err(plugin_host_handler_error)?;
    let cancel_bridge = bridge.clone();
    client
        .register_handler("backend.stream.cancel", move |params| {
            let bridge = cancel_bridge.clone();
            async move {
                let params: StreamCancelParams = serde_json::from_value(params)
                    .map_err(|error| invalid_rpc_params("backend.stream.cancel", error))?;
                serde_json::to_value(
                    bridge
                        .streams
                        .cancel(params)
                        .await
                        .map_err(stream_rpc_error)?,
                )
                .map_err(|error| RpcHandlerError::new(-32603, error.to_string()))
            }
        })
        .await
        .map_err(plugin_host_handler_error)?;
    PLUGIN_HOST_BACKEND_BRIDGE
        .set(bridge.clone())
        .map_err(|_| {
            crate::BitFunError::ProcessError(
                "Plugin host backend bridge is already initialized".to_string(),
            )
        })?;
    Ok(bridge)
}

pub(crate) fn plugin_host_backend_bridge() -> Option<Arc<PluginHostBackendBridge>> {
    PLUGIN_HOST_BACKEND_BRIDGE.get().cloned()
}

fn invalid_rpc_params(method: &str, error: serde_json::Error) -> RpcHandlerError {
    RpcHandlerError::new(-32602, format!("Invalid parameters for {method}: {error}"))
}

fn plugin_host_handler_error(
    error: bitfun_opencode_plugin_host::PluginHostError,
) -> crate::BitFunError {
    crate::BitFunError::ProcessError(format!(
        "Failed to register plugin host backend handler: {error}"
    ))
}

fn stream_rpc_error(error: StreamRegistryError) -> RpcHandlerError {
    match error {
        StreamRegistryError::InstanceMismatch => RpcHandlerError::new(-32003, error.to_string()),
        StreamRegistryError::InvalidMaxBytes => RpcHandlerError::new(-32602, error.to_string()),
        StreamRegistryError::Capacity | StreamRegistryError::BodyTooLarge => {
            RpcHandlerError::new(-32000, error.to_string())
        }
    }
}

fn parse_body<T: DeserializeOwned>(body: &[u8]) -> Result<T, RouteFailure> {
    if body.is_empty() {
        serde_json::from_value(json!({}))
            .map_err(|error| RouteFailure::bad_request(error.to_string()))
    } else {
        serde_json::from_slice(body).map_err(|error| RouteFailure::bad_request(error.to_string()))
    }
}

async fn dispatch_route(
    context: &crate::plugin_host::PluginHostInstance,
    route: OpenCodeClientRoute,
    query: &std::collections::HashMap<String, Vec<String>>,
    body: &[u8],
) -> Result<Value, RouteFailure> {
    crate::plugin_host_http_routes::dispatch_route(context, route, query, body).await
}

pub(crate) fn body_as<T: DeserializeOwned>(body: &[u8]) -> Result<T, RouteFailure> {
    parse_body(body)
}

pub(crate) type RouteResult = Result<Value, RouteFailure>;
pub(crate) use RouteFailure as Failure;
