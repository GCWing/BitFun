use std::collections::HashMap;
use std::fmt;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;

use super::error::{AppError, Result};
use super::protocol::{Request, RequestEnvelope, Response, ResponseEnvelope, ServerMessage};
use super::FLASHGREP_LOG_TARGET;

const JSONRPC_VERSION: &str = "2.0";

type PendingResponseSender = oneshot::Sender<Result<ResponseEnvelope>>;
type PendingResponses = HashMap<u64, PendingResponseSender>;

#[derive(Clone)]
pub(crate) struct ProtocolClient {
    inner: Arc<ProtocolClientInner>,
}

struct ProtocolClientInner {
    write_tx: mpsc::Sender<Vec<u8>>,
    pending: Mutex<PendingResponses>,
    closed: AtomicBool,
    next_id: AtomicU64,
    backend_name: String,
}

impl ProtocolClient {
    pub(crate) fn channel(backend_name: impl Into<String>) -> (Self, mpsc::Receiver<Vec<u8>>) {
        let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(128);
        (
            Self {
                inner: Arc::new(ProtocolClientInner {
                    write_tx,
                    pending: Mutex::new(HashMap::new()),
                    closed: AtomicBool::new(false),
                    next_id: AtomicU64::new(1),
                    backend_name: backend_name.into(),
                }),
            },
            write_rx,
        )
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_closed(&self) {
        self.inner.closed.store(true, Ordering::Relaxed);
    }

    pub(crate) async fn send_request_with_timeout(
        &self,
        request: Request,
        request_timeout: Option<Duration>,
    ) -> Result<Response> {
        if self.is_closed() {
            return Err(AppError::Protocol(format!(
                "{} is not running",
                self.inner.backend_name
            )));
        }

        let request_name = request_name(&request);
        let request_id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = RequestEnvelope {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(request_id),
            request,
        };
        let bytes = encode_content_length_message(&envelope)?;
        let (sender, receiver) = oneshot::channel();
        self.inner.pending.lock().await.insert(request_id, sender);

        if self.inner.write_tx.send(bytes).await.is_err() {
            self.inner.pending.lock().await.remove(&request_id);
            return Err(AppError::Protocol(format!(
                "{} write channel is closed",
                self.inner.backend_name
            )));
        }

        let response = match request_timeout {
            Some(duration) => match timeout(duration, receiver).await {
                Ok(result) => result.map_err(|_| {
                    AppError::Protocol(format!(
                        "{} closed without sending a response",
                        self.inner.backend_name
                    ))
                })??,
                Err(_) => {
                    self.inner.pending.lock().await.remove(&request_id);
                    return Err(AppError::Protocol(format!(
                        "{} request timed out: {request_name}",
                        self.inner.backend_name
                    )));
                }
            },
            None => receiver.await.map_err(|_| {
                AppError::Protocol(format!(
                    "{} closed without sending a response",
                    self.inner.backend_name
                ))
            })??,
        };

        decode_response(request_id, response)
    }

    pub(crate) async fn send_notification(&self, request: Request) -> Result<()> {
        if self.is_closed() {
            return Err(AppError::Protocol(format!(
                "{} is not running",
                self.inner.backend_name
            )));
        }

        let envelope = RequestEnvelope {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: None,
            request,
        };
        let bytes = encode_content_length_message(&envelope)?;
        self.inner.write_tx.send(bytes).await.map_err(|_| {
            AppError::Protocol(format!(
                "{} write channel is closed",
                self.inner.backend_name
            ))
        })
    }

    pub(crate) async fn handle_server_message(&self, message: ServerMessage) {
        match message {
            ServerMessage::Response(response) => {
                let Some(request_id) = response.id else {
                    return;
                };
                if let Some(sender) = self.inner.pending.lock().await.remove(&request_id) {
                    let _ = sender.send(Ok(response));
                }
            }
            ServerMessage::Notification(notification) => {
                log::trace!(
                    target: FLASHGREP_LOG_TARGET,
                    "Flashgrep protocol notification: backend={}, method={}",
                    self.inner.backend_name,
                    notification.method
                );
            }
        }
    }

    pub(crate) async fn close_with_message(&self, message: impl Into<String>) {
        self.mark_closed();
        self.reject_pending(message).await;
    }

    pub(crate) async fn reject_pending(&self, message: impl Into<String>) {
        let message = message.into();
        let mut pending = self.inner.pending.lock().await;
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err(AppError::Protocol(message.clone())));
        }
    }
}

impl fmt::Debug for ProtocolClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtocolClient")
            .field("backend_name", &self.inner.backend_name)
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

pub(crate) async fn read_content_length_message<R>(reader: &mut R) -> Result<Option<ServerMessage>>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("Content-Length") {
            let length = value.trim().parse::<usize>().map_err(|error| {
                AppError::Protocol(format!("invalid Content-Length header: {error}"))
            })?;
            content_length = Some(length);
        }
    }

    let content_length =
        content_length.ok_or_else(|| AppError::Protocol("missing Content-Length header".into()))?;
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).await?;
    serde_json::from_slice(&body)
        .map_err(|error| AppError::Protocol(format!("failed to decode daemon message: {error}")))
}

pub(crate) fn drain_content_length_messages(buffer: &mut Vec<u8>) -> Result<Vec<ServerMessage>> {
    let mut messages = Vec::new();

    loop {
        let Some(header_end) = find_header_end(buffer) else {
            break;
        };
        let header = String::from_utf8_lossy(&buffer[..header_end]);
        let mut content_length = None;
        for line in header.lines() {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            if name.trim().eq_ignore_ascii_case("Content-Length") {
                content_length = Some(value.trim().parse::<usize>().map_err(|error| {
                    AppError::Protocol(format!("invalid Content-Length header: {error}"))
                })?);
            }
        }
        let content_length = content_length
            .ok_or_else(|| AppError::Protocol("missing Content-Length header".into()))?;
        let body_start = header_end + header_delimiter_len(buffer, header_end);
        let body_end = body_start + content_length;
        if buffer.len() < body_end {
            break;
        }
        let message = serde_json::from_slice::<ServerMessage>(&buffer[body_start..body_end])
            .map_err(|error| {
                AppError::Protocol(format!("failed to decode daemon message: {error}"))
            })?;
        buffer.drain(..body_end);
        messages.push(message);
    }

    Ok(messages)
}

fn encode_content_length_message(message: &impl Serialize) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(message)
        .map_err(|error| AppError::Protocol(format!("failed to encode request: {error}")))?;
    let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    framed.extend_from_slice(&body);
    Ok(framed)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .or_else(|| buffer.windows(2).position(|window| window == b"\n\n"))
}

fn header_delimiter_len(buffer: &[u8], header_end: usize) -> usize {
    if buffer
        .get(header_end..header_end + 4)
        .is_some_and(|delimiter| delimiter == b"\r\n\r\n")
    {
        4
    } else {
        2
    }
}

fn decode_response(request_id: u64, response: ResponseEnvelope) -> Result<Response> {
    if response.id != Some(request_id) {
        return Err(AppError::Protocol(format!(
            "daemon response id mismatch: expected {request_id:?}, got {:?}",
            response.id
        )));
    }

    if response.jsonrpc != JSONRPC_VERSION {
        return Err(AppError::Protocol(format!(
            "unsupported daemon jsonrpc version: {}",
            response.jsonrpc
        )));
    }

    if let Some(error) = response.error {
        return Err(AppError::Protocol(error.message));
    }

    response
        .result
        .ok_or_else(|| AppError::Protocol("daemon response missing result".into()))
}

fn request_name(request: &Request) -> &'static str {
    match request {
        Request::Initialize { .. } => "initialize",
        Request::Initialized => "initialized",
        Request::Ping => "ping",
        Request::BaseSnapshotBuild { .. } => "base_snapshot/build",
        Request::BaseSnapshotRebuild { .. } => "base_snapshot/rebuild",
        Request::TaskStatus { .. } => "task/status",
        Request::OpenRepo { .. } => "open_repo",
        Request::GetRepoStatus { .. } => "get_repo_status",
        Request::Search { .. } => "search",
        Request::Glob { .. } => "glob",
        Request::CloseRepo { .. } => "close_repo",
        Request::Shutdown => "shutdown",
    }
}
