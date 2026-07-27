use crate::{
    read_frame, write_frame, DiscoveryRecord, DiscoveryStore, InitializeResult, LocalIpcEndpoint,
    LocalIpcListener, LocalIpcStream, RuntimeInstanceIdentity, RuntimeInstanceLock,
    RuntimeIpcCapabilities, RuntimeIpcDiscoveryError, RuntimeIpcError, RuntimeIpcErrorCode,
    RuntimeIpcFrame, RuntimeIpcIoError, RuntimeIpcOperation, RuntimeIpcOperationResult,
    RuntimeIpcTransportError, PROTOCOL_VERSION,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use uuid::Uuid;

const MAX_CONNECTION_LIMIT: usize = 1024;

#[derive(Debug, Clone)]
pub struct RuntimeIpcServerConfig {
    pub server_version: String,
    pub idle_timeout: Duration,
    pub io_timeout: Duration,
    pub max_connections: usize,
}

pub struct RuntimeIpcServer {
    listener: LocalIpcListener,
    endpoint: LocalIpcEndpoint,
    discovery_store: DiscoveryStore,
    discovery_record: DiscoveryRecord,
    _instance_lock: RuntimeInstanceLock,
    connection: Arc<ConnectionConfig>,
    idle_timeout: Duration,
    max_connections: usize,
}

impl RuntimeIpcServer {
    pub async fn bind(
        runtime_root: &Path,
        identity: RuntimeInstanceIdentity,
        config: RuntimeIpcServerConfig,
    ) -> Result<Self, RuntimeIpcServerError> {
        validate_server_config(&config)?;
        let instance_lock = RuntimeInstanceLock::try_acquire(runtime_root, &identity)?;
        let owner_id = Uuid::new_v4().simple().to_string();
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let endpoint = LocalIpcEndpoint::for_instance(runtime_root, &identity)?;
        let listener = LocalIpcListener::bind(endpoint.clone()).await?;
        let discovery_store = DiscoveryStore::new(runtime_root, identity.clone());
        let discovery_record = DiscoveryRecord::new(
            identity.clone(),
            endpoint.discovery_value().to_string(),
            std::process::id(),
            token.clone(),
            owner_id,
        );
        discovery_store.write(&discovery_record)?;

        Ok(Self {
            listener,
            endpoint,
            discovery_store,
            discovery_record,
            _instance_lock: instance_lock,
            connection: Arc::new(ConnectionConfig {
                instance_identity: identity.as_str().to_string(),
                token,
                server_version: config.server_version,
                io_timeout: config.io_timeout,
            }),
            idle_timeout: config.idle_timeout,
            max_connections: config.max_connections,
        })
    }

    pub fn discovery_record(&self) -> &DiscoveryRecord {
        &self.discovery_record
    }

    pub fn endpoint(&self) -> &LocalIpcEndpoint {
        &self.endpoint
    }

    pub async fn serve(mut self) -> Result<(), RuntimeIpcServerError> {
        let result = self.serve_until_idle().await;
        let cleanup = self
            .discovery_store
            .remove_if_owned(&self.discovery_record)
            .map(|_| ())
            .map_err(RuntimeIpcServerError::Discovery);
        result.and(cleanup)
    }

    async fn serve_until_idle(&mut self) -> Result<(), RuntimeIpcServerError> {
        let mut clients = JoinSet::new();
        loop {
            if clients.is_empty() {
                tokio::select! {
                    accepted = self.listener.accept() => {
                        let stream = accepted?;
                        spawn_connection(&mut clients, stream, self.connection.clone());
                    }
                    _ = tokio::time::sleep(self.idle_timeout) => break,
                }
            } else if clients.len() >= self.max_connections {
                observe_connection(clients.join_next().await)?;
            } else {
                tokio::select! {
                    accepted = self.listener.accept() => {
                        let stream = accepted?;
                        spawn_connection(&mut clients, stream, self.connection.clone());
                    }
                    completed = clients.join_next() => {
                        observe_connection(completed)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for RuntimeIpcServer {
    fn drop(&mut self) {
        let _ = self.discovery_store.remove_if_owned(&self.discovery_record);
    }
}

fn spawn_connection(
    clients: &mut JoinSet<Result<(), RuntimeIpcServerError>>,
    stream: LocalIpcStream,
    config: Arc<ConnectionConfig>,
) {
    clients.spawn(async move { handle_connection(stream, &config).await });
}

fn observe_connection(
    completed: Option<Result<Result<(), RuntimeIpcServerError>, tokio::task::JoinError>>,
) -> Result<(), RuntimeIpcServerError> {
    match completed {
        Some(Ok(Ok(()))) | Some(Ok(Err(_))) => Ok(()),
        Some(Err(error)) => Err(RuntimeIpcServerError::ConnectionTask(error)),
        None => Ok(()),
    }
}

struct ConnectionConfig {
    instance_identity: String,
    token: String,
    server_version: String,
    io_timeout: Duration,
}

async fn handle_connection(
    mut stream: LocalIpcStream,
    config: &ConnectionConfig,
) -> Result<(), RuntimeIpcServerError> {
    let first = match timeout_read(config.io_timeout, &mut stream).await {
        Ok(frame) => frame,
        Err(RuntimeIpcServerError::Disconnected) => return Ok(()),
        Err(error) => return Err(error),
    };
    let (request_id, request) = match first {
        RuntimeIpcFrame::Initialize {
            request_id,
            request,
        } => (request_id, request),
        frame => {
            send_error(
                &mut stream,
                config.io_timeout,
                request_id_of(&frame),
                RuntimeIpcErrorCode::InvalidRequest,
                "initialize must be the first frame",
            )
            .await?;
            return Ok(());
        }
    };

    if !constant_time_eq(request.token.as_bytes(), config.token.as_bytes()) {
        send_error(
            &mut stream,
            config.io_timeout,
            Some(request_id),
            RuntimeIpcErrorCode::Unauthorized,
            "runtime IPC authentication failed",
        )
        .await?;
        return Ok(());
    }
    if request.protocol_version != PROTOCOL_VERSION {
        send_error(
            &mut stream,
            config.io_timeout,
            Some(request_id),
            RuntimeIpcErrorCode::IncompatibleProtocol,
            "runtime IPC protocol version is incompatible",
        )
        .await?;
        return Ok(());
    }
    if request.instance_identity != config.instance_identity {
        send_error(
            &mut stream,
            config.io_timeout,
            Some(request_id),
            RuntimeIpcErrorCode::WrongInstance,
            "runtime IPC endpoint belongs to another instance",
        )
        .await?;
        return Ok(());
    }
    if !valid_client_fact(&request.client_id) || !valid_client_fact(&request.client_version) {
        send_error(
            &mut stream,
            config.io_timeout,
            Some(request_id),
            RuntimeIpcErrorCode::InvalidRequest,
            "runtime IPC client identity is invalid",
        )
        .await?;
        return Ok(());
    }

    timeout_write(
        config.io_timeout,
        &mut stream,
        &RuntimeIpcFrame::Initialized {
            request_id,
            result: InitializeResult {
                protocol_version: PROTOCOL_VERSION,
                instance_identity: config.instance_identity.clone(),
                server_version: config.server_version.clone(),
                capabilities: RuntimeIpcCapabilities { health: true },
            },
        },
    )
    .await?;

    loop {
        let frame = match timeout_read(config.io_timeout, &mut stream).await {
            Ok(frame) => frame,
            Err(RuntimeIpcServerError::Disconnected) => return Ok(()),
            Err(error) => return Err(error),
        };
        match frame {
            RuntimeIpcFrame::Request {
                request_id,
                operation: RuntimeIpcOperation::Health,
            } => {
                timeout_write(
                    config.io_timeout,
                    &mut stream,
                    &RuntimeIpcFrame::Response {
                        request_id,
                        result: RuntimeIpcOperationResult::Health {
                            instance_identity: config.instance_identity.clone(),
                            process_id: std::process::id(),
                        },
                    },
                )
                .await?;
            }
            frame => {
                send_error(
                    &mut stream,
                    config.io_timeout,
                    request_id_of(&frame),
                    RuntimeIpcErrorCode::InvalidRequest,
                    "runtime IPC frame is not valid after initialization",
                )
                .await?;
                return Ok(());
            }
        }
    }
}

async fn timeout_read(
    timeout: Duration,
    stream: &mut LocalIpcStream,
) -> Result<RuntimeIpcFrame, RuntimeIpcServerError> {
    match tokio::time::timeout(timeout, read_frame(stream)).await {
        Err(_) => Err(RuntimeIpcServerError::IoTimeout),
        Ok(Err(RuntimeIpcIoError::Io(error)))
            if matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
            ) =>
        {
            Err(RuntimeIpcServerError::Disconnected)
        }
        Ok(Err(error)) => Err(RuntimeIpcServerError::Io(error)),
        Ok(Ok(frame)) => Ok(frame),
    }
}

async fn timeout_write(
    timeout: Duration,
    stream: &mut LocalIpcStream,
    frame: &RuntimeIpcFrame,
) -> Result<(), RuntimeIpcServerError> {
    tokio::time::timeout(timeout, write_frame(stream, frame))
        .await
        .map_err(|_| RuntimeIpcServerError::IoTimeout)?
        .map_err(RuntimeIpcServerError::Io)
}

async fn send_error(
    stream: &mut LocalIpcStream,
    timeout: Duration,
    request_id: Option<u64>,
    code: RuntimeIpcErrorCode,
    message: &str,
) -> Result<(), RuntimeIpcServerError> {
    timeout_write(
        timeout,
        stream,
        &RuntimeIpcFrame::Error {
            request_id,
            error: RuntimeIpcError {
                code,
                message: message.to_string(),
            },
        },
    )
    .await
}

fn request_id_of(frame: &RuntimeIpcFrame) -> Option<u64> {
    match frame {
        RuntimeIpcFrame::Initialize { request_id, .. }
        | RuntimeIpcFrame::Initialized { request_id, .. }
        | RuntimeIpcFrame::Request { request_id, .. }
        | RuntimeIpcFrame::Response { request_id, .. } => Some(*request_id),
        RuntimeIpcFrame::Error { request_id, .. } => *request_id,
    }
}

fn valid_client_fact(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let length = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..length {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

fn validate_server_config(config: &RuntimeIpcServerConfig) -> Result<(), RuntimeIpcServerError> {
    if config.server_version.is_empty()
        || config.server_version.len() > 128
        || config.server_version.chars().any(char::is_control)
        || config.idle_timeout.is_zero()
        || config.io_timeout.is_zero()
        || config.max_connections == 0
        || config.max_connections > MAX_CONNECTION_LIMIT
    {
        return Err(RuntimeIpcServerError::InvalidConfig);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeIpcServerError {
    #[error("runtime IPC server configuration is invalid")]
    InvalidConfig,
    #[error("runtime IPC connection timed out")]
    IoTimeout,
    #[error("runtime IPC client disconnected")]
    Disconnected,
    #[error("runtime IPC connection task failed")]
    ConnectionTask(#[source] tokio::task::JoinError),
    #[error(transparent)]
    Discovery(#[from] RuntimeIpcDiscoveryError),
    #[error(transparent)]
    Transport(#[from] RuntimeIpcTransportError),
    #[error(transparent)]
    Io(#[from] RuntimeIpcIoError),
}
