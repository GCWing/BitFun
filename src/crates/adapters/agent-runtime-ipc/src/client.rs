use crate::{
    read_frame_strict_with_limit, serialize_frame_with_limit, write_frame, DiscoveryRecord,
    HealthResult, InitializeRequest, LocalIpcEndpoint, RuntimeIpcCapabilities, RuntimeIpcError,
    RuntimeIpcFrame, RuntimeIpcFrameReader, RuntimeIpcIoError, RuntimeIpcOperation,
    RuntimeIpcOperationResult, RuntimeIpcTransportError, MAX_REQUEST_FRAME_BYTES,
    MAX_RESPONSE_FRAME_BYTES, PROTOCOL_VERSION,
};
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, watch, Mutex, OwnedMutexGuard};

const CLIENT_EVENT_BUFFER: usize = 256;
const CLIENT_COMMAND_BUFFER: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeIpcClientEvent {
    Runtime(crate::RuntimeIpcEvent),
    Disconnected,
}

#[derive(Clone)]
pub struct RuntimeIpcClient {
    commands: mpsc::Sender<ClientCommand>,
    instance_identity: String,
    request_timeout: Duration,
    request_gate: Arc<Mutex<()>>,
    events: broadcast::Sender<RuntimeIpcClientEvent>,
    capabilities: RuntimeIpcCapabilities,
    disconnect: watch::Sender<bool>,
}

struct ClientCommand {
    operation: RuntimeIpcOperation,
    response: oneshot::Sender<PendingResponse>,
    deadline: tokio::time::Instant,
    _request_gate: OwnedMutexGuard<()>,
}

enum PendingResponse {
    Result(RuntimeIpcOperationResult),
    Remote(RuntimeIpcError),
    RequestIdExhausted,
    Timeout,
    Io(RuntimeIpcIoError),
    Disconnected,
}

impl fmt::Debug for RuntimeIpcClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeIpcClient")
            .field("instance_identity", &self.instance_identity)
            .field("request_timeout", &self.request_timeout)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl RuntimeIpcClient {
    pub async fn connect(
        runtime_root: &Path,
        discovery: &DiscoveryRecord,
        client_id: &str,
        client_version: &str,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, RuntimeIpcClientError> {
        if connect_timeout.is_zero() || request_timeout.is_zero() {
            return Err(RuntimeIpcClientError::InvalidTimeout);
        }
        if discovery.protocol_version != PROTOCOL_VERSION {
            return Err(RuntimeIpcClientError::IncompatibleProtocol {
                expected: PROTOCOL_VERSION,
                observed: discovery.protocol_version,
            });
        }
        validate_client_fact(client_id)?;
        validate_client_fact(client_version)?;
        let endpoint = LocalIpcEndpoint::parse_for_root(
            &discovery.endpoint,
            runtime_root,
            &discovery.instance_identity,
        )?;
        let mut stream = endpoint.connect(connect_timeout).await?;
        let request_id = 1;
        timeout_io(
            connect_timeout,
            write_frame(
                &mut stream,
                &RuntimeIpcFrame::Initialize {
                    request_id,
                    request: InitializeRequest {
                        protocol_version: PROTOCOL_VERSION,
                        instance_identity: discovery.instance_identity.as_str().to_string(),
                        token: discovery.token.clone(),
                        client_id: client_id.to_string(),
                        client_version: client_version.to_string(),
                    },
                },
            ),
        )
        .await?;
        let response = timeout_io(
            connect_timeout,
            read_frame_strict_with_limit(&mut stream, MAX_RESPONSE_FRAME_BYTES),
        )
        .await?;
        let capabilities = match response {
            RuntimeIpcFrame::Initialized {
                request_id: response_id,
                result,
            } if response_id == request_id
                && result.protocol_version == PROTOCOL_VERSION
                && result.instance_identity == discovery.instance_identity.as_str()
                && result.capabilities.health =>
            {
                result.capabilities
            }
            RuntimeIpcFrame::Error {
                request_id: Some(response_id),
                error,
            } if response_id == request_id => return Err(RuntimeIpcClientError::Remote(error)),
            _ => return Err(RuntimeIpcClientError::UnexpectedResponse),
        };

        let (events, _) = broadcast::channel(CLIENT_EVENT_BUFFER);
        let (commands, command_rx) = mpsc::channel(CLIENT_COMMAND_BUFFER);
        let (disconnect, disconnect_rx) = watch::channel(false);
        tokio::spawn(run_connection(
            stream,
            command_rx,
            events.clone(),
            disconnect_rx,
        ));

        Ok(Self {
            commands,
            instance_identity: discovery.instance_identity.as_str().to_string(),
            request_timeout,
            request_gate: Arc::new(Mutex::new(())),
            events,
            capabilities,
            disconnect,
        })
    }

    pub fn capabilities(&self) -> &RuntimeIpcCapabilities {
        &self.capabilities
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<RuntimeIpcClientEvent> {
        self.events.subscribe()
    }

    pub async fn request(
        &self,
        operation: RuntimeIpcOperation,
    ) -> Result<RuntimeIpcOperationResult, RuntimeIpcClientError> {
        let request_gate = self.request_gate.clone().lock_owned().await;
        serialize_frame_with_limit(
            &RuntimeIpcFrame::Request {
                request_id: u64::MAX,
                operation: operation.clone(),
            },
            MAX_REQUEST_FRAME_BYTES,
        )?;
        let deadline = tokio::time::Instant::now() + self.request_timeout;
        let (sender, receiver) = oneshot::channel();
        match tokio::time::timeout_at(
            deadline,
            self.commands.send(ClientCommand {
                operation,
                response: sender,
                deadline,
                _request_gate: request_gate,
            }),
        )
        .await
        {
            Err(_) => {
                let _ = self.disconnect.send(true);
                return Err(RuntimeIpcClientError::Timeout);
            }
            Ok(Err(_)) => return Err(RuntimeIpcClientError::Disconnected),
            Ok(Ok(())) => {}
        }

        let response = match tokio::time::timeout_at(deadline, receiver).await {
            Err(_) => {
                let _ = self.disconnect.send(true);
                return Err(RuntimeIpcClientError::Timeout);
            }
            Ok(Err(_)) => return Err(RuntimeIpcClientError::Disconnected),
            Ok(Ok(response)) => response,
        };
        match response {
            PendingResponse::Result(result) => Ok(result),
            PendingResponse::Remote(error) => Err(RuntimeIpcClientError::Remote(error)),
            PendingResponse::RequestIdExhausted => Err(RuntimeIpcClientError::RequestIdExhausted),
            PendingResponse::Timeout => Err(RuntimeIpcClientError::Timeout),
            PendingResponse::Io(error) => Err(RuntimeIpcClientError::Io(error)),
            PendingResponse::Disconnected => Err(RuntimeIpcClientError::Disconnected),
        }
    }

    pub async fn health(&self) -> Result<HealthResult, RuntimeIpcClientError> {
        match self.request(RuntimeIpcOperation::Health).await? {
            RuntimeIpcOperationResult::Health {
                instance_identity,
                process_id,
            } if instance_identity == self.instance_identity => Ok(HealthResult {
                instance_identity,
                process_id,
            }),
            _ => Err(RuntimeIpcClientError::UnexpectedResponse),
        }
    }
}

async fn run_connection(
    mut stream: crate::LocalIpcStream,
    mut commands: mpsc::Receiver<ClientCommand>,
    events: broadcast::Sender<RuntimeIpcClientEvent>,
    mut disconnect: watch::Receiver<bool>,
) {
    let mut next_request_id = 2u64;
    let mut pending = std::collections::HashMap::new();
    let mut frames = RuntimeIpcFrameReader::new(MAX_RESPONSE_FRAME_BYTES);
    loop {
        tokio::select! {
            biased;
            changed = disconnect.changed() => {
                if changed.is_err() || *disconnect.borrow() {
                    break;
                }
            }
            command = commands.recv() => {
                let Some(command) = command else {
                    break;
                };
                let Some(incremented) = next_request_id.checked_add(1) else {
                    let _ = command.response.send(PendingResponse::RequestIdExhausted);
                    break;
                };
                let request_id = next_request_id;
                next_request_id = incremented;
                if tokio::time::Instant::now() >= command.deadline {
                    let _ = command.response.send(PendingResponse::Timeout);
                    continue;
                }
                let frame = RuntimeIpcFrame::Request {
                    request_id,
                    operation: command.operation,
                };
                match tokio::time::timeout_at(command.deadline, write_frame(&mut stream, &frame)).await {
                    Err(_) => {
                        let _ = command.response.send(PendingResponse::Timeout);
                        break;
                    }
                    Ok(Err(error @ RuntimeIpcIoError::FrameTooLarge { .. })) => {
                        let _ = command.response.send(PendingResponse::Io(error));
                        continue;
                    }
                    Ok(Err(_)) => {
                        let _ = command.response.send(PendingResponse::Disconnected);
                        break;
                    }
                    Ok(Ok(())) => {}
                }
                pending.insert(request_id, (command.response, command._request_gate));
            },
            frame = frames.read_strict(&mut stream) => match frame {
                Ok(RuntimeIpcFrame::Response { request_id, result }) => {
                    if let Some((sender, _request_gate)) = pending.remove(&request_id) {
                        let _ = sender.send(PendingResponse::Result(result));
                    } else {
                        break;
                    }
                }
                Ok(RuntimeIpcFrame::Error {
                    request_id: Some(request_id),
                    error,
                }) => {
                    if let Some((sender, _request_gate)) = pending.remove(&request_id) {
                        let _ = sender.send(PendingResponse::Remote(error));
                    } else {
                        break;
                    }
                }
                Ok(RuntimeIpcFrame::Event { event }) => {
                    let _ = events.send(RuntimeIpcClientEvent::Runtime(event));
                }
                Ok(_) | Err(_) => break,
            }
        }
    }

    for (_, (sender, _request_gate)) in pending.drain() {
        let _ = sender.send(PendingResponse::Disconnected);
    }
    let _ = events.send(RuntimeIpcClientEvent::Disconnected);
}

async fn timeout_io<T>(
    timeout: Duration,
    future: impl std::future::Future<Output = Result<T, RuntimeIpcIoError>>,
) -> Result<T, RuntimeIpcClientError> {
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| RuntimeIpcClientError::Timeout)?
        .map_err(RuntimeIpcClientError::Io)
}

fn validate_client_fact(value: &str) -> Result<(), RuntimeIpcClientError> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(RuntimeIpcClientError::InvalidClientIdentity);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeIpcClientError {
    #[error("runtime IPC protocol mismatch: expected {expected}, observed {observed}")]
    IncompatibleProtocol { expected: u32, observed: u32 },
    #[error("runtime IPC client identity is invalid")]
    InvalidClientIdentity,
    #[error("runtime IPC request timed out")]
    Timeout,
    #[error("runtime IPC connection closed")]
    Disconnected,
    #[error("runtime IPC returned an unexpected response")]
    UnexpectedResponse,
    #[error("runtime IPC request identifiers are exhausted")]
    RequestIdExhausted,
    #[error("runtime IPC timeouts must be greater than zero")]
    InvalidTimeout,
    #[error("runtime IPC request was rejected: {0:?}")]
    Remote(RuntimeIpcError),
    #[error(transparent)]
    Transport(#[from] RuntimeIpcTransportError),
    #[error(transparent)]
    Io(#[from] RuntimeIpcIoError),
}
