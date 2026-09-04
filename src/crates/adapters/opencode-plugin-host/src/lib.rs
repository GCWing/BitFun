mod backend;
mod frame;
mod host_log;
mod http;
mod peer;
mod peer_runtime;
mod stream_registry;

use openbitfun_services_core::process_tree::{CleanupOutcome, ProcessTreeChild};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;

pub use backend::{
    register_backend_handlers, BackendDiagnostic, BackendDiagnosticError, BackendDiagnosticEvent,
    BackendDiagnosticSeverity, BackendRouteFailure, BackendRouteRequest, OpenCodeBackendHandler,
    PluginHostBackendBridge,
};
use frame::{read_frame, write_frame};
pub use http::OpenCodeClientRoute;
#[cfg(test)]
pub(crate) use http::{read_host_stream, HostStreamReadError, StreamDescriptor};
pub use peer::{
    hook_function_runtime, JsonRpcPeer, OpenCodeHookFunctionRuntime, PluginHostClient,
    RpcHandlerError,
};

const PROTOCOL_VERSION: u64 = 1;
const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const STARTUP_TERMINATE_GRACE: Duration = Duration::from_millis(500);
const MIN_NEGOTIATED_FRAME_BYTES: usize = 1024;
pub const CONFIG_CONTRIBUTORS_V1: &str = "config-contributors-v1";
pub const CONFIG_CONTRIBUTIONS_V2: &str = "config-contributions-v2";
pub const GENERATION_FENCING_V1: &str = "generation-fencing-v1";
static NEXT_CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginHostCapabilities {
    negotiated: BTreeSet<String>,
}

impl PluginHostCapabilities {
    pub fn all_supported() -> Self {
        Self {
            negotiated: [
                CONFIG_CONTRIBUTORS_V1,
                CONFIG_CONTRIBUTIONS_V2,
                GENERATION_FENCING_V1,
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }

    pub fn supports(&self, capability: &str) -> bool {
        self.negotiated.contains(capability)
    }

    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.negotiated.iter().map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginGenerationLease {
    #[serde(rename = "instanceID")]
    pub instance_id: String,
    pub generation_key: String,
    pub revision: String,
}

#[derive(Debug)]
struct HandshakeNegotiation {
    max_frame_bytes: usize,
    capabilities: PluginHostCapabilities,
}

#[derive(Debug, Clone)]
pub struct PluginHostConfig {
    pub runtime_command: PathBuf,
    pub entry: PathBuf,
    pub working_directory: PathBuf,
    pub cache_directory: PathBuf,
    pub log_file: PathBuf,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDeclaration {
    pub spec: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPrepareRequest {
    pub plugins: Vec<PluginDeclaration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_base_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_install: Option<bool>,
}

/// Adapter-owned, protocol-independent facts needed by product assembly to
/// fence a reviewed plugin graph before import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPreparationSummary {
    pub review_digest: String,
    pub reviewed_count: usize,
    pub prepared_count: usize,
    pub failed_count: usize,
    pub content_digests: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstanceOpenRequest {
    #[serde(rename = "instanceID")]
    pub instance_id: String,
    pub generation_key: String,
    pub revision: String,
    pub project: Value,
    pub config: serde_json::Map<String, Value>,
    pub directory: String,
    pub worktree: String,
    pub plugins: Vec<PluginDeclaration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_content_digests: Option<std::collections::BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_review_digest: Option<String>,
}

#[derive(Debug, Error)]
pub enum PluginHostError {
    #[error("plugin host entry is not an absolute path: {0}")]
    RelativeEntry(PathBuf),
    #[error("plugin host cache directory is not an absolute path: {0}")]
    RelativeCacheDirectory(PathBuf),
    #[error("plugin host log file is not an absolute path: {0}")]
    RelativeLogFile(PathBuf),
    #[error("failed to prepare plugin host cache directory: {0}")]
    PrepareCache(#[source] std::io::Error),
    #[error("failed to bind plugin host listener: {0}")]
    Bind(#[source] std::io::Error),
    #[error("failed to start plugin host runtime: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("failed to prepare plugin host log: {0}")]
    PrepareLog(#[source] std::io::Error),
    #[error("plugin host did not connect within the startup timeout")]
    StartupTimeout,
    #[error("plugin host IPC failed: {0}")]
    Io(#[source] std::io::Error),
    #[error("plugin host handshake frame is invalid: {0}")]
    InvalidHandshake(String),
    #[error("plugin host JSON-RPC protocol error: {0}")]
    Protocol(String),
    #[error("plugin host JSON-RPC connection closed: {0}")]
    ConnectionClosed(String),
    #[error("plugin host is shutting down")]
    ShuttingDown,
    #[error("plugin host JSON-RPC request timed out: method={method}, request_id={request_id}")]
    RequestTimeout { method: String, request_id: String },
    #[error("plugin tool execution stopped after its deadline: execution_id={execution_id}")]
    ToolStoppedAfterTimeout { execution_id: String },
    #[error(
        "plugin tool execution outcome is unknown: execution_id={execution_id}, reason={reason}"
    )]
    ToolOutcomeUnknown {
        execution_id: String,
        reason: String,
    },
    #[error("plugin host JSON-RPC returned an error: code={code}, message={message}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<Value>,
    },
    #[error("plugin host JSON-RPC handler is already registered: {0}")]
    DuplicateHandler(String),
}

pub struct PluginHost {
    child: ProcessTreeChild,
    client: PluginHostClient,
    runtime: Arc<dyn openbitfun_runtime_ports::HookFunctionRuntime>,
    host_log: Option<host_log::HostLogDrain>,
    max_frame_bytes: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginHostShutdownPolicy {
    pub drain_timeout: Duration,
    pub rpc_timeout: Duration,
    pub exit_timeout: Duration,
    pub eof_timeout: Duration,
    pub terminate_grace: Duration,
}

impl Default for PluginHostShutdownPolicy {
    fn default() -> Self {
        Self {
            drain_timeout: Duration::from_secs(3),
            rpc_timeout: Duration::from_secs(5),
            exit_timeout: Duration::from_secs(2),
            eof_timeout: Duration::from_secs(1),
            terminate_grace: Duration::from_millis(500),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHostShutdownDisposition {
    Graceful,
    ExitedAfterShutdown,
    ExitedAfterConnectionClose,
    Forced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHostShutdownReport {
    pub generation: u64,
    pub disposition: PluginHostShutdownDisposition,
    pub rpc_completed: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

impl PluginHost {
    pub async fn start(config: PluginHostConfig) -> Result<Self, PluginHostError> {
        Self::start_with_timeout(config, STARTUP_TIMEOUT).await
    }

    pub async fn start_with_timeout(
        config: PluginHostConfig,
        startup_timeout: Duration,
    ) -> Result<Self, PluginHostError> {
        validate_config(&config)?;
        tokio::fs::create_dir_all(&config.cache_directory)
            .await
            .map_err(PluginHostError::PrepareCache)?;
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(PluginHostError::Bind)?;
        let address = listener.local_addr().map_err(PluginHostError::Bind)?;
        let token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        let mut command = Command::new(&config.runtime_command);
        command
            .arg(&config.entry)
            .current_dir(&config.working_directory)
            .env("OPENCODE_EXTENSION_HOST_RPC_ADDRESS", address.to_string())
            .env("OPENCODE_EXTENSION_HOST_RPC_TOKEN", &token)
            .env("OPENCODE_EXTENSION_HOST_LOG_LEVEL", &config.log_level)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = ProcessTreeChild::spawn(&mut command)
            .await
            .map_err(PluginHostError::Spawn)?;
        let host_log = host_log::attach_host_log(&mut child, &config.log_file)
            .await
            .map_err(PluginHostError::PrepareLog)?;

        let connection = accept_authenticated_connection(
            &listener,
            &token,
            &config.cache_directory,
            startup_timeout,
        )
        .await;
        let (stream, negotiation) = match connection {
            Ok(connection) => connection,
            Err(error) => {
                if let Err(cleanup_error) = child.terminate(STARTUP_TERMINATE_GRACE).await {
                    log::error!("Plugin host startup cleanup failed: {cleanup_error}");
                }
                let _ = host_log.flush(Duration::from_secs(1)).await;
                return Err(error);
            }
        };
        let generation = NEXT_CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed);
        let peer = JsonRpcPeer::start_with_capabilities(
            stream,
            generation,
            negotiation.max_frame_bytes,
            negotiation.capabilities,
        );
        let client = peer.client();
        let runtime = hook_function_runtime(client.clone());
        Ok(Self {
            child,
            client,
            runtime,
            host_log: Some(host_log),
            max_frame_bytes: negotiation.max_frame_bytes,
        })
    }

    pub fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    pub fn client(&self) -> PluginHostClient {
        self.client.clone()
    }

    pub fn runtime(&self) -> Arc<dyn openbitfun_runtime_ports::HookFunctionRuntime> {
        self.runtime.clone()
    }

    pub fn is_connected(&mut self) -> Result<bool, PluginHostError> {
        if self
            .child
            .try_wait()
            .map_err(PluginHostError::Io)?
            .is_some()
        {
            return Ok(false);
        }
        Ok(!self.client.is_closed())
    }

    pub async fn shutdown(mut self, policy: PluginHostShutdownPolicy) -> PluginHostShutdownReport {
        let started_at = Instant::now();
        let generation = self.client.generation();
        let pending = self.client.begin_draining().await;
        log::info!(
            "Plugin host shutdown started: generation={}, pending_requests={}, drain_deadline_ms={}, rpc_deadline_ms={}",
            generation,
            pending,
            policy.drain_timeout.as_millis(),
            policy.rpc_timeout.as_millis()
        );
        if !self.client.wait_for_pending(policy.drain_timeout).await {
            log::warn!(
                "Plugin host RPC drain timed out: generation={}, pending_requests={}",
                generation,
                pending
            );
        }

        let rpc_completed = self
            .client
            .request_during_shutdown("host.shutdown", json!({}), policy.rpc_timeout)
            .await
            .is_ok_and(|result| result.get("closed").and_then(Value::as_bool) == Some(true));
        if rpc_completed {
            log::info!(
                "Plugin host shutdown RPC completed: generation={}, duration_ms={}",
                generation,
                elapsed_ms(started_at)
            );
            if let Ok(Ok(status)) =
                tokio::time::timeout(policy.exit_timeout, self.child.wait()).await
            {
                let disposition = if status.success() {
                    PluginHostShutdownDisposition::Graceful
                } else {
                    PluginHostShutdownDisposition::ExitedAfterShutdown
                };
                let report =
                    shutdown_report(generation, disposition, true, status.code(), started_at);
                if report.disposition == PluginHostShutdownDisposition::Graceful {
                    log::info!(
                        "Plugin host exited gracefully: generation={}, exit_code={:?}, duration_ms={}",
                        generation,
                        report.exit_code,
                        report.duration_ms
                    );
                } else {
                    log::warn!(
                        "Plugin host exited after shutdown with a failure status: generation={}, exit_code={:?}, duration_ms={}",
                        generation,
                        report.exit_code,
                        report.duration_ms
                    );
                }
                self.flush_host_log(policy.eof_timeout).await;
                return report;
            }
            log::warn!(
                "Plugin host exit timed out after shutdown response: generation={}",
                generation
            );
        } else {
            log::warn!(
                "Plugin host shutdown RPC failed or timed out: generation={}",
                generation
            );
        }

        self.client
            .close("plugin host graceful shutdown fallback")
            .await;
        if let Ok(Ok(status)) = tokio::time::timeout(policy.eof_timeout, self.child.wait()).await {
            let report = shutdown_report(
                generation,
                PluginHostShutdownDisposition::ExitedAfterConnectionClose,
                rpc_completed,
                status.code(),
                started_at,
            );
            log::info!(
                "Plugin host exited after RPC connection close: generation={}, exit_code={:?}, duration_ms={}",
                generation,
                report.exit_code,
                report.duration_ms
            );
            self.flush_host_log(policy.eof_timeout).await;
            return report;
        }

        let cleanup = self.child.terminate(policy.terminate_grace).await;
        let exit_code = self
            .child
            .try_wait()
            .ok()
            .flatten()
            .and_then(|status| status.code());
        let report = shutdown_report(
            generation,
            PluginHostShutdownDisposition::Forced,
            rpc_completed,
            exit_code,
            started_at,
        );
        match cleanup {
            Ok(CleanupOutcome::AlreadyExited) => log::warn!(
                "Plugin host exited during forced cleanup: generation={}, duration_ms={}",
                generation,
                report.duration_ms
            ),
            Ok(_) => log::warn!(
                "Plugin host process tree terminated: generation={}, duration_ms={}",
                generation,
                report.duration_ms
            ),
            Err(error) => log::error!(
                "Plugin host process tree termination failed: generation={}, error={}",
                generation,
                error
            ),
        }
        self.flush_host_log(policy.eof_timeout).await;
        report
    }

    async fn flush_host_log(&mut self, deadline: Duration) {
        let Some(host_log) = self.host_log.take() else {
            return;
        };
        if !host_log.flush(deadline).await {
            log::warn!(
                "Plugin host log flush timed out: generation={}",
                self.client.generation()
            );
        }
    }
}

async fn accept_authenticated_connection(
    listener: &TcpListener,
    token: &str,
    cache_directory: &Path,
    startup_timeout: Duration,
) -> Result<(TcpStream, HandshakeNegotiation), PluginHostError> {
    tokio::time::timeout(startup_timeout, async {
        let (mut stream, _) = listener.accept().await.map_err(PluginHostError::Io)?;
        let negotiation = complete_handshake(&mut stream, token, cache_directory).await?;
        Ok((stream, negotiation))
    })
    .await
    .map_err(|_| PluginHostError::StartupTimeout)?
}

fn shutdown_report(
    generation: u64,
    disposition: PluginHostShutdownDisposition,
    rpc_completed: bool,
    exit_code: Option<i32>,
    started_at: Instant,
) -> PluginHostShutdownReport {
    PluginHostShutdownReport {
        generation,
        disposition,
        rpc_completed,
        exit_code,
        duration_ms: elapsed_ms(started_at),
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn validate_config(config: &PluginHostConfig) -> Result<(), PluginHostError> {
    if !config.entry.is_absolute() {
        return Err(PluginHostError::RelativeEntry(config.entry.clone()));
    }
    if !config.cache_directory.is_absolute() {
        return Err(PluginHostError::RelativeCacheDirectory(
            config.cache_directory.clone(),
        ));
    }
    if !config.log_file.is_absolute() {
        return Err(PluginHostError::RelativeLogFile(config.log_file.clone()));
    }
    Ok(())
}

async fn complete_handshake(
    stream: &mut TcpStream,
    expected_token: &str,
    cache_directory: &Path,
) -> Result<HandshakeNegotiation, PluginHostError> {
    let request = read_frame(stream, DEFAULT_MAX_FRAME_BYTES).await?;
    let jsonrpc = request.get("jsonrpc").and_then(Value::as_str);
    let method = request.get("method").and_then(Value::as_str);
    let request_id = request
        .get("id")
        .and_then(Value::as_str)
        .filter(|request_id| !request_id.is_empty());
    let params = request.get("params").and_then(Value::as_object);
    let token = params
        .and_then(|params| params.get("token"))
        .and_then(Value::as_str);
    let protocol_version = params
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_u64);
    let requested_frame_bytes = params
        .and_then(|params| params.get("maxFrameBytes"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    let requested_capabilities = params
        .and_then(|params| params.get("capabilities"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if jsonrpc != Some("2.0")
        || method != Some("backend.handshake")
        || request_id.is_none()
        || request.get("result").is_some()
        || request.get("error").is_some()
        || token != Some(expected_token)
        || protocol_version != Some(PROTOCOL_VERSION)
    {
        return Err(PluginHostError::InvalidHandshake(
            "method, token, request id, or protocol version did not match".to_string(),
        ));
    }
    let max_frame_bytes = requested_frame_bytes
        .unwrap_or(DEFAULT_MAX_FRAME_BYTES)
        .max(MIN_NEGOTIATED_FRAME_BYTES)
        .min(MAX_FRAME_BYTES);
    let negotiated = [
        CONFIG_CONTRIBUTORS_V1,
        CONFIG_CONTRIBUTIONS_V2,
        GENERATION_FENCING_V1,
    ]
    .into_iter()
    .filter(|capability| requested_capabilities.contains(capability))
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    let response = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "result": {
            "protocolVersion": PROTOCOL_VERSION,
            "maxFrameBytes": max_frame_bytes,
            "cacheDirectory": cache_directory.to_string_lossy(),
            "capabilities": negotiated
        }
    });
    write_frame(stream, &response, DEFAULT_MAX_FRAME_BYTES).await?;
    Ok(HandshakeNegotiation {
        max_frame_bytes,
        capabilities: PluginHostCapabilities { negotiated },
    })
}

#[cfg(test)]
mod tests;
