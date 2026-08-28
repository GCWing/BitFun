use crate::http::{
    json_error_body, match_http_route, read_host_stream, BackendHttpRequest, BackendHttpResponse,
    HostStreamReadError, HttpRouteError, OpenCodeClientRoute, MAX_HTTP_BODY_BYTES,
};
use crate::stream_registry::{
    PluginHostStreamRegistry, StreamCancelParams, StreamReadParams, StreamRegistryError,
};
use crate::{PluginHostClient, PluginHostError, RpcHandlerError};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, Notify};

const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_DRAIN_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct BackendRouteRequest {
    pub instance_id: String,
    pub route: OpenCodeClientRoute,
    pub query: HashMap<String, Vec<String>>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl BackendRouteRequest {
    pub fn query_first(&self, key: &str) -> Option<&str> {
        self.query
            .get(key)
            .and_then(|values| values.first())
            .map(String::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct BackendRouteFailure {
    kind: BackendRouteFailureKind,
    message: String,
}

#[derive(Debug, Clone, Copy)]
enum BackendRouteFailureKind {
    BadRequest,
    Forbidden,
    NotFound,
    Unsupported,
    Backend,
    Unavailable,
}

impl BackendRouteFailure {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(BackendRouteFailureKind::BadRequest, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(BackendRouteFailureKind::Forbidden, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(BackendRouteFailureKind::NotFound, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(BackendRouteFailureKind::Unsupported, message)
    }

    pub fn backend(message: impl Into<String>) -> Self {
        Self::new(BackendRouteFailureKind::Backend, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(BackendRouteFailureKind::Unavailable, message)
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(kind: BackendRouteFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    fn status_and_code(&self) -> (u16, &'static str) {
        match self.kind {
            BackendRouteFailureKind::BadRequest => (400, "invalid_request"),
            BackendRouteFailureKind::Forbidden => (403, "instance_scope_denied"),
            BackendRouteFailureKind::NotFound => (404, "not_found"),
            BackendRouteFailureKind::Unsupported => (501, "unsupported_capability"),
            BackendRouteFailureKind::Backend => (502, "backend_failure"),
            BackendRouteFailureKind::Unavailable => (503, "backend_unavailable"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendDiagnosticSeverity {
    Debug,
    Info,
    Warning,
    Error,
}

impl BackendDiagnosticSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendDiagnostic {
    pub severity: BackendDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub plugin: Option<String>,
    pub method: Option<String>,
    pub data: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct BackendDiagnosticEvent {
    pub instance_id: Option<String>,
    pub diagnostic: BackendDiagnostic,
}

#[derive(Debug, Clone)]
pub enum BackendDiagnosticError {
    Unavailable(String),
    Backend(String),
}

#[async_trait]
pub trait OpenCodeBackendHandler: Send + Sync {
    async fn handle_route(
        &self,
        request: BackendRouteRequest,
    ) -> Result<Value, BackendRouteFailure>;

    async fn publish_diagnostic(
        &self,
        event: BackendDiagnosticEvent,
    ) -> Result<(), BackendDiagnosticError>;
}

pub struct PluginHostBackendBridge {
    client: PluginHostClient,
    handler: Arc<dyn OpenCodeBackendHandler>,
    streams: PluginHostStreamRegistry,
    drain_tx: watch::Sender<bool>,
    draining: AtomicBool,
    active_requests: AtomicUsize,
    requests_drained: Notify,
}

struct ActiveRequest<'a> {
    bridge: &'a PluginHostBackendBridge,
    drain_rx: watch::Receiver<bool>,
}

impl Drop for ActiveRequest<'_> {
    fn drop(&mut self) {
        if self.bridge.active_requests.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.bridge.requests_drained.notify_waiters();
        }
    }
}

impl PluginHostBackendBridge {
    fn new(client: PluginHostClient, handler: Arc<dyn OpenCodeBackendHandler>) -> Self {
        let (drain_tx, _) = watch::channel(false);
        Self {
            client,
            handler,
            streams: PluginHostStreamRegistry::default(),
            drain_tx,
            draining: AtomicBool::new(false),
            active_requests: AtomicUsize::new(0),
            requests_drained: Notify::new(),
        }
    }

    fn admit(&self) -> Option<ActiveRequest<'_>> {
        let drain_rx = self.drain_tx.subscribe();
        if self.draining.load(Ordering::Acquire) {
            return None;
        }
        self.active_requests.fetch_add(1, Ordering::AcqRel);
        if self.draining.load(Ordering::Acquire) || *drain_rx.borrow() {
            if self.active_requests.fetch_sub(1, Ordering::AcqRel) == 1 {
                self.requests_drained.notify_waiters();
            }
            return None;
        }
        Some(ActiveRequest {
            bridge: self,
            drain_rx,
        })
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
        let Some(mut active) = self.admit() else {
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
                    .await;
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
                    .await;
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
                    .await;
            }
        };
        let operation = route_match.route.operation();
        let body = tokio::select! {
            _ = active.drain_rx.changed() => return Err(draining_rpc_error()),
            body = async {
                match request.body.as_ref() {
                    Some(descriptor) => read_host_stream(
                        &self.client,
                        &instance_id,
                        descriptor,
                        MAX_HTTP_BODY_BYTES,
                        HTTP_REQUEST_TIMEOUT,
                    )
                    .await
                    .map(Some),
                    None => Ok(None),
                }
            } => match body {
                Ok(Some(body)) => body,
                Ok(None) => Vec::new(),
                Err(HostStreamReadError::BodyTooLarge) => {
                    return self
                        .http_error(
                            &instance_id,
                            413,
                            "request_too_large",
                            "Request body exceeds the configured limit",
                            &path,
                        )
                        .await;
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
                        .await;
                }
            }
        };

        let outcome = tokio::select! {
            _ = active.drain_rx.changed() => return Err(draining_rpc_error()),
            outcome = tokio::time::timeout(
                HTTP_REQUEST_TIMEOUT,
                self.handler.handle_route(BackendRouteRequest {
                    instance_id: instance_id.clone(),
                    route: route_match.route,
                    query: route_match.query,
                    headers: request.headers,
                    body,
                }),
            ) => outcome,
        };
        let response = match outcome {
            Ok(Ok(value)) => self.json_response(&instance_id, 200, value).await,
            Ok(Err(error)) => {
                let (status, code) = error.status_and_code();
                self.http_error(&instance_id, status, code, &error.message, &path)
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

    async fn handle_diagnostic(&self, params: Value) -> Result<Value, RpcHandlerError> {
        let params: RawDiagnosticPublishParams = serde_json::from_value(params)
            .map_err(|error| invalid_rpc_params("backend.diagnostic.publish", error))?;
        let severity = match params.diagnostic.severity.as_str() {
            "debug" => BackendDiagnosticSeverity::Debug,
            "info" => BackendDiagnosticSeverity::Info,
            "warning" => BackendDiagnosticSeverity::Warning,
            "error" => BackendDiagnosticSeverity::Error,
            _ => {
                return Err(RpcHandlerError::new(
                    -32602,
                    "backend.diagnostic.publish severity is invalid",
                ));
            }
        };
        self.handler
            .publish_diagnostic(BackendDiagnosticEvent {
                instance_id: params.instance_id,
                diagnostic: BackendDiagnostic {
                    severity,
                    code: params.diagnostic.code,
                    message: params.diagnostic.message,
                    plugin: params.diagnostic.plugin,
                    method: params.diagnostic.method,
                    data: params.diagnostic.data,
                },
            })
            .await
            .map_err(diagnostic_rpc_error)?;
        Ok(serde_json::json!({}))
    }

    pub async fn begin_draining(&self) -> bool {
        self.draining.store(true, Ordering::Release);
        let _ = self.drain_tx.send(true);
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
        let requests_drained = tokio::time::timeout(HTTP_DRAIN_TIMEOUT, wait).await.is_ok();
        if !requests_drained {
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
        requests_drained
    }

    pub async fn cancel_instance_streams(&self, instance_id: &str) {
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

pub async fn register_backend_handlers(
    client: PluginHostClient,
    handler: Arc<dyn OpenCodeBackendHandler>,
) -> Result<Arc<PluginHostBackendBridge>, PluginHostError> {
    let bridge = Arc::new(PluginHostBackendBridge::new(client.clone(), handler));
    let http_bridge = Arc::downgrade(&bridge);
    client
        .register_handler("backend.http.request", move |params| {
            let bridge = http_bridge.clone();
            async move {
                bridge
                    .upgrade()
                    .ok_or_else(backend_bridge_unavailable)?
                    .handle_http(params)
                    .await
            }
        })
        .await?;
    let read_bridge = Arc::downgrade(&bridge);
    client
        .register_handler("backend.stream.read", move |params| {
            let bridge = read_bridge.clone();
            async move {
                let bridge = bridge.upgrade().ok_or_else(backend_bridge_unavailable)?;
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
        .await?;
    let cancel_bridge = Arc::downgrade(&bridge);
    client
        .register_handler("backend.stream.cancel", move |params| {
            let bridge = cancel_bridge.clone();
            async move {
                let bridge = bridge.upgrade().ok_or_else(backend_bridge_unavailable)?;
                serde_json::to_value(
                    bridge
                        .streams
                        .cancel(
                            serde_json::from_value::<StreamCancelParams>(params).map_err(
                                |error| invalid_rpc_params("backend.stream.cancel", error),
                            )?,
                        )
                        .await
                        .map_err(stream_rpc_error)?,
                )
                .map_err(|error| RpcHandlerError::new(-32603, error.to_string()))
            }
        })
        .await?;
    let diagnostic_bridge = Arc::downgrade(&bridge);
    client
        .register_handler("backend.diagnostic.publish", move |params| {
            let bridge = diagnostic_bridge.clone();
            async move {
                bridge
                    .upgrade()
                    .ok_or_else(backend_bridge_unavailable)?
                    .handle_diagnostic(params)
                    .await
            }
        })
        .await?;
    Ok(bridge)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDiagnostic {
    severity: String,
    code: String,
    message: String,
    plugin: Option<String>,
    method: Option<String>,
    data: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDiagnosticPublishParams {
    #[serde(rename = "instanceID")]
    instance_id: Option<String>,
    diagnostic: RawDiagnostic,
}

fn invalid_rpc_params(method: &str, error: serde_json::Error) -> RpcHandlerError {
    RpcHandlerError::new(-32602, format!("Invalid parameters for {method}: {error}"))
}

fn draining_rpc_error() -> RpcHandlerError {
    RpcHandlerError::new(-32000, "Plugin host backend is draining")
}

fn backend_bridge_unavailable() -> RpcHandlerError {
    RpcHandlerError::new(-32004, "Plugin host backend bridge is unavailable")
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

fn diagnostic_rpc_error(error: BackendDiagnosticError) -> RpcHandlerError {
    match error {
        BackendDiagnosticError::Unavailable(message) => RpcHandlerError::new(-32004, message),
        BackendDiagnosticError::Backend(message) => RpcHandlerError::new(-32603, message),
    }
}

#[cfg(test)]
mod tests {
    use super::BackendRouteFailure;

    #[test]
    fn route_failure_mapping_stays_adapter_owned() {
        assert_eq!(
            BackendRouteFailure::unsupported("retired").status_and_code(),
            (501, "unsupported_capability")
        );
    }
}
