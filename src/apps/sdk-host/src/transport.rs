//! Local newline-delimited JSON-RPC transport for the standalone SDK Host candidate.

use std::sync::Arc;
use std::time::Duration;

use bitfun_agent_runtime::sdk::AgentRuntime;
use futures_util::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::{timeout, Instant};
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};
use tokio_util::sync::CancellationToken;

use bitfun_sdk_host::host::{ConnectionControl, HostOutput, SdkHostConfig, SdkHostConnection};
use bitfun_sdk_host::protocol::{
    JsonRpcErrorResponse, JsonRpcRequest, RequestId, METHOD_INITIALIZE, METHOD_QUERY_CANCEL,
    METHOD_SESSION_CLOSE, METHOD_SHUTDOWN,
};

#[derive(Debug, Clone)]
pub struct SdkHostTransportConfig {
    pub max_line_bytes: usize,
    pub write_timeout_ms: u64,
    pub shutdown_total_timeout_ms: u64,
    pub host: SdkHostConfig,
}

impl Default for SdkHostTransportConfig {
    fn default() -> Self {
        Self {
            max_line_bytes: 1024 * 1024,
            write_timeout_ms: 5_000,
            shutdown_total_timeout_ms: 10_000,
            host: SdkHostConfig::default(),
        }
    }
}

struct JsonLineOutput<Writer> {
    writer: Mutex<Writer>,
    failed: CancellationToken,
    write_timeout: Duration,
}

impl<Writer> JsonLineOutput<Writer> {
    fn new(writer: Writer, write_timeout_ms: u64) -> Self {
        Self {
            writer: Mutex::new(writer),
            failed: CancellationToken::new(),
            write_timeout: Duration::from_millis(write_timeout_ms.max(1)),
        }
    }

    fn failure_token(&self) -> CancellationToken {
        self.failed.clone()
    }
}

#[async_trait::async_trait]
impl<Writer> HostOutput for JsonLineOutput<Writer>
where
    Writer: AsyncWrite + Unpin + Send,
{
    async fn send(&self, value: serde_json::Value) -> Result<(), ()> {
        let mut line = serde_json::to_vec(&value).map_err(|_| ())?;
        line.push(b'\n');
        let result = timeout(self.write_timeout, async {
            let mut writer = self.writer.lock().await;
            writer.write_all(&line).await.map_err(|_| ())?;
            writer.flush().await.map_err(|_| ())
        })
        .await;
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(())) | Err(_) => {
                self.failed.cancel();
                Err(())
            }
        }
    }
}

pub async fn serve_streams<Reader, Writer>(
    runtime: AgentRuntime,
    default_cwd: impl Into<String>,
    reader: Reader,
    writer: Writer,
    config: SdkHostTransportConfig,
) -> Result<(), std::io::Error>
where
    Reader: AsyncRead + Unpin,
    Writer: AsyncWrite + Unpin + Send + 'static,
{
    let max_in_flight_requests = config.host.max_in_flight_requests.max(1);
    let max_in_flight_control_requests = config.host.max_in_flight_control_requests.max(1);
    let shutdown_total_timeout = Duration::from_millis(config.shutdown_total_timeout_ms.max(1));
    let output = Arc::new(JsonLineOutput::new(writer, config.write_timeout_ms));
    let output_failed = output.failure_token();
    let connection =
        SdkHostConnection::with_output(runtime, default_cwd, output.clone(), config.host);
    let mut lines = FramedRead::new(
        reader,
        LinesCodec::new_with_max_length(config.max_line_bytes),
    );
    let mut parse_error_index = 0u64;
    let mut data_requests = JoinSet::new();
    let mut control_requests = JoinSet::new();

    loop {
        let line = tokio::select! {
            _ = output_failed.cancelled() => {
                data_requests.abort_all();
                control_requests.abort_all();
                graceful_shutdown(&connection, shutdown_total_timeout).await;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "SDK Host output is unavailable",
                ));
            }
            completed = data_requests.join_next(), if !data_requests.is_empty() => {
                if let Some(Ok(ConnectionControl::Shutdown)) = completed {
                    data_requests.abort_all();
                    control_requests.abort_all();
                    graceful_shutdown(&connection, shutdown_total_timeout).await;
                    return Ok(());
                }
                continue;
            }
            completed = control_requests.join_next(), if !control_requests.is_empty() => {
                if let Some(Ok(ConnectionControl::Shutdown)) = completed {
                    data_requests.abort_all();
                    control_requests.abort_all();
                    graceful_shutdown(&connection, shutdown_total_timeout).await;
                    return Ok(());
                }
                continue;
            }
            line = lines.next() => line,
        };
        let Some(line) = line else {
            data_requests.abort_all();
            control_requests.abort_all();
            graceful_shutdown(&connection, shutdown_total_timeout).await;
            return Ok(());
        };
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                parse_error_index += 1;
                let response = JsonRpcErrorResponse::parse_error(
                    "SDK Host request line is invalid or exceeds the size limit",
                    format!("parse:{parse_error_index}"),
                );
                if let Ok(value) = serde_json::to_value(response) {
                    let _ = output.send(value).await;
                }
                if let LinesCodecError::Io(error) = error {
                    data_requests.abort_all();
                    control_requests.abort_all();
                    graceful_shutdown(&connection, shutdown_total_timeout).await;
                    return Err(error);
                }
                continue;
            }
        };
        let value = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(value) => value,
            Err(_) => {
                parse_error_index += 1;
                let response = JsonRpcErrorResponse::parse_error(
                    "SDK Host request is not valid JSON",
                    format!("parse:{parse_error_index}"),
                );
                if let Ok(value) = serde_json::to_value(response) {
                    let _ = output.send(value).await;
                }
                continue;
            }
        };
        let is_notification = !value
            .as_object()
            .is_some_and(|object| object.contains_key("id"));
        let request_id = request_id_from_value(&value);
        if is_notification && !is_json_rpc_notification(&value) {
            parse_error_index += 1;
            let response = JsonRpcErrorResponse::invalid_request(
                None,
                "SDK Host notification is not a valid JSON-RPC notification",
                format!("invalid:{parse_error_index}"),
            );
            if let Ok(value) = serde_json::to_value(response) {
                let _ = output.send(value).await;
            }
            continue;
        }
        let request = match serde_json::from_value::<JsonRpcRequest>(value) {
            Ok(request) => request,
            Err(_) => {
                parse_error_index += 1;
                let response = JsonRpcErrorResponse::invalid_request(
                    request_id,
                    "SDK Host request is not a valid JSON-RPC request",
                    format!("invalid:{parse_error_index}"),
                );
                if let Ok(value) = serde_json::to_value(response) {
                    let _ = output.send(value).await;
                }
                continue;
            }
        };
        if request.id.is_none() && !is_notification {
            parse_error_index += 1;
            let response = JsonRpcErrorResponse::invalid_request(
                None,
                "SDK Host request id must be a string or integer",
                format!("invalid:{parse_error_index}"),
            );
            if let Ok(value) = serde_json::to_value(response) {
                let _ = output.send(value).await;
            }
            continue;
        }

        if request.method == METHOD_INITIALIZE {
            if !connection.is_initialized().await {
                drain_request_sets(
                    &mut data_requests,
                    &mut control_requests,
                    Duration::from_secs(5),
                )
                .await;
            }
            connection.handle_request(request).await;
            continue;
        }
        if request.method == METHOD_SHUTDOWN {
            if connection.handle_request(request).await == ConnectionControl::Shutdown {
                graceful_shutdown_with_requests(
                    &mut data_requests,
                    &mut control_requests,
                    &connection,
                    shutdown_total_timeout,
                )
                .await;
                return Ok(());
            }
            continue;
        }

        let is_control_request = matches!(
            request.method.as_str(),
            METHOD_QUERY_CANCEL | METHOD_SESSION_CLOSE
        );
        let request_set = if is_control_request {
            &mut control_requests
        } else {
            &mut data_requests
        };
        let capacity = if is_control_request {
            max_in_flight_control_requests
        } else {
            max_in_flight_requests
        };
        if request_set.len() >= capacity {
            connection.reject_overloaded(request.id.clone()).await;
            continue;
        }
        let connection = connection.clone();
        request_set.spawn(async move { connection.handle_request(request).await });
    }
}

fn is_json_rpc_notification(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    !object.contains_key("id")
        && object.get("jsonrpc").and_then(serde_json::Value::as_str) == Some("2.0")
        && object
            .get("method")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|method| !method.is_empty())
        && object
            .get("params")
            .is_none_or(|params| params.is_object() || params.is_array())
        && object
            .keys()
            .all(|key| matches!(key.as_str(), "jsonrpc" | "method" | "params"))
}

fn request_id_from_value(value: &serde_json::Value) -> Option<RequestId> {
    match value.get("id")? {
        serde_json::Value::String(value) => Some(RequestId::String(value.clone())),
        serde_json::Value::Number(value) => value.as_i64().map(RequestId::Number),
        _ => None,
    }
}

async fn graceful_shutdown(connection: &SdkHostConnection, total_timeout: Duration) {
    if !connection.shutdown_connection_bounded(total_timeout).await {
        tracing::warn!("SDK Host connection cleanup completed with residual errors");
    }
}

async fn graceful_shutdown_with_requests(
    data_requests: &mut JoinSet<ConnectionControl>,
    control_requests: &mut JoinSet<ConnectionControl>,
    connection: &SdkHostConnection,
    total_timeout: Duration,
) {
    let started_at = Instant::now();
    drain_request_sets(
        data_requests,
        control_requests,
        total_timeout.min(Duration::from_secs(5)) / 2,
    )
    .await;
    graceful_shutdown(
        connection,
        total_timeout.saturating_sub(started_at.elapsed()),
    )
    .await;
}

async fn drain_request_sets(
    data_requests: &mut JoinSet<ConnectionControl>,
    control_requests: &mut JoinSet<ConnectionControl>,
    drain_timeout: Duration,
) {
    let started_at = Instant::now();
    drain_requests(control_requests, drain_timeout).await;
    drain_requests(
        data_requests,
        drain_timeout.saturating_sub(started_at.elapsed()),
    )
    .await;
}

async fn drain_requests(requests: &mut JoinSet<ConnectionControl>, drain_timeout: Duration) {
    if timeout(drain_timeout, async {
        while requests.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        requests.abort_all();
        while requests.join_next().await.is_some() {}
    }
}

pub async fn serve_stdio(
    runtime: AgentRuntime,
    default_cwd: impl Into<String>,
) -> Result<(), std::io::Error> {
    serve_streams(
        runtime,
        default_cwd,
        tokio::io::stdin(),
        tokio::io::stdout(),
        SdkHostTransportConfig::default(),
    )
    .await
}
