use crate::{
    read_frame, write_frame, DiscoveryRecord, HealthResult, InitializeRequest, LocalIpcEndpoint,
    LocalIpcStream, RuntimeIpcError, RuntimeIpcFrame, RuntimeIpcIoError, RuntimeIpcOperation,
    RuntimeIpcOperationResult, RuntimeIpcTransportError, PROTOCOL_VERSION,
};
use std::fmt;
use std::path::Path;
use std::time::Duration;

pub struct RuntimeIpcClient {
    stream: LocalIpcStream,
    instance_identity: String,
    request_timeout: Duration,
    next_request_id: u64,
}

impl fmt::Debug for RuntimeIpcClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeIpcClient")
            .field("instance_identity", &self.instance_identity)
            .field("request_timeout", &self.request_timeout)
            .field("next_request_id", &self.next_request_id)
            .finish_non_exhaustive()
    }
}

impl RuntimeIpcClient {
    pub async fn connect(
        runtime_root: &Path,
        discovery: &DiscoveryRecord,
        client_id: &str,
        client_version: &str,
        timeout: Duration,
    ) -> Result<Self, RuntimeIpcClientError> {
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
        let mut stream = endpoint.connect(timeout).await?;
        let request_id = 1;
        let frame = RuntimeIpcFrame::Initialize {
            request_id,
            request: InitializeRequest {
                protocol_version: PROTOCOL_VERSION,
                instance_identity: discovery.instance_identity.as_str().to_string(),
                token: discovery.token.clone(),
                client_id: client_id.to_string(),
                client_version: client_version.to_string(),
            },
        };
        timeout_io(timeout, write_frame(&mut stream, &frame)).await?;
        let response = timeout_io(timeout, read_frame(&mut stream)).await?;
        match response {
            RuntimeIpcFrame::Initialized {
                request_id: response_id,
                result,
            } if response_id == request_id
                && result.protocol_version == PROTOCOL_VERSION
                && result.instance_identity == discovery.instance_identity.as_str()
                && result.capabilities.health => {}
            RuntimeIpcFrame::Error {
                request_id: Some(response_id),
                error,
            } if response_id == request_id => return Err(RuntimeIpcClientError::Remote(error)),
            _ => return Err(RuntimeIpcClientError::UnexpectedResponse),
        }

        Ok(Self {
            stream,
            instance_identity: discovery.instance_identity.as_str().to_string(),
            request_timeout: timeout,
            next_request_id: 2,
        })
    }

    pub async fn health(&mut self) -> Result<HealthResult, RuntimeIpcClientError> {
        let request_id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(RuntimeIpcClientError::RequestIdExhausted)?;
        timeout_io(
            self.request_timeout,
            write_frame(
                &mut self.stream,
                &RuntimeIpcFrame::Request {
                    request_id,
                    operation: RuntimeIpcOperation::Health,
                },
            ),
        )
        .await?;
        let response = timeout_io(self.request_timeout, read_frame(&mut self.stream)).await?;
        match response {
            RuntimeIpcFrame::Response {
                request_id: response_id,
                result:
                    RuntimeIpcOperationResult::Health {
                        instance_identity,
                        process_id,
                    },
            } if response_id == request_id && instance_identity == self.instance_identity => {
                Ok(HealthResult {
                    instance_identity,
                    process_id,
                })
            }
            RuntimeIpcFrame::Error {
                request_id: Some(response_id),
                error,
            } if response_id == request_id => Err(RuntimeIpcClientError::Remote(error)),
            _ => Err(RuntimeIpcClientError::UnexpectedResponse),
        }
    }
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
    #[error("runtime IPC returned an unexpected response")]
    UnexpectedResponse,
    #[error("runtime IPC request identifiers are exhausted")]
    RequestIdExhausted,
    #[error("runtime IPC request was rejected: {0:?}")]
    Remote(RuntimeIpcError),
    #[error(transparent)]
    Transport(#[from] RuntimeIpcTransportError),
    #[error(transparent)]
    Io(#[from] RuntimeIpcIoError),
}
