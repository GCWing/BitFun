use crate::peer_runtime::{run_reader, run_writer};
use crate::{
    PluginGenerationLease, PluginHostCapabilities, PluginHostError, PluginInstanceOpenRequest,
    PluginPrepareRequest, GENERATION_FENCING_V1,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch, Mutex, Notify, RwLock, Semaphore};

const OUTBOUND_CAPACITY: usize = 128;
const HANDLER_CONCURRENCY: usize = 32;

pub(super) type HandlerFuture =
    Pin<Box<dyn Future<Output = Result<Value, RpcHandlerError>> + Send>>;
pub(super) type Handler = Arc<dyn Fn(Value) -> HandlerFuture + Send + Sync>;
pub(super) type PendingSender = oneshot::Sender<Result<Value, PluginHostError>>;

#[derive(Debug, Clone)]
pub struct RpcHandlerError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcHandlerError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

#[derive(Clone)]
pub struct PluginHostClient {
    state: Arc<PeerState>,
}

impl PluginHostClient {
    pub fn generation(&self) -> u64 {
        self.state.generation
    }

    pub fn is_closed(&self) -> bool {
        self.state.closed.load(Ordering::Acquire)
    }

    pub fn capabilities(&self) -> &PluginHostCapabilities {
        &self.state.capabilities
    }

    pub async fn set_log_level(&self, level: &str) -> Result<(), PluginHostError> {
        let result = self
            .request(
                "host.log.setLevel",
                json!({ "level": level }),
                Duration::from_secs(5),
            )
            .await?;
        if result.get("level").and_then(Value::as_str) == Some(level) {
            return Ok(());
        }
        Err(PluginHostError::Protocol(
            "host.log.setLevel returned an invalid level".to_string(),
        ))
    }

    pub async fn open_instance(
        &self,
        request: PluginInstanceOpenRequest,
        deadline: Duration,
    ) -> Result<Value, PluginHostError> {
        let params = serde_json::to_value(request)
            .map_err(|error| PluginHostError::Protocol(error.to_string()))?;
        self.request("host.instance.open", params, deadline).await
    }

    pub async fn prepare_plugins(
        &self,
        request: PluginPrepareRequest,
        deadline: Duration,
    ) -> Result<Value, PluginHostError> {
        let params = serde_json::to_value(request)
            .map_err(|error| PluginHostError::Protocol(error.to_string()))?;
        self.request("host.plugins.prepare", params, deadline).await
    }

    pub async fn close_instance(
        &self,
        instance_id: &str,
        deadline: Duration,
    ) -> Result<bool, PluginHostError> {
        let result = self
            .request(
                "host.instance.close",
                json!({"instanceID": instance_id}),
                deadline,
            )
            .await?;
        result
            .get("closed")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                PluginHostError::Protocol(
                    "host.instance.close returned an invalid result".to_string(),
                )
            })
    }

    pub async fn call_hook(
        &self,
        lease: &PluginGenerationLease,
        hook: &str,
        input: Value,
        output: Value,
        deadline: Duration,
    ) -> Result<Value, PluginHostError> {
        self.require_generation_fencing()?;
        let result = self
            .request(
                "host.hook.call",
                json!({
                    "instanceID": lease.instance_id,
                    "generationKey": lease.generation_key,
                    "revision": lease.revision,
                    "hook": hook,
                    "input": input,
                    "output": output,
                }),
                deadline,
            )
            .await?;
        validate_fenced_response(&result, lease, None)?;
        if result.get("hook").and_then(Value::as_str) != Some(hook) {
            return Err(PluginHostError::Protocol(
                "host.hook.call response returned a mismatched hook name".to_string(),
            ));
        }
        Ok(result)
    }

    pub async fn execute_tool(
        &self,
        lease: &PluginGenerationLease,
        execution_id: &str,
        registration_id: &str,
        args: Value,
        context: Value,
        deadline: Duration,
    ) -> Result<Value, PluginHostError> {
        self.require_generation_fencing()?;
        let result = self
            .request(
                "host.tool.execute",
                json!({
                    "instanceID": lease.instance_id,
                    "generationKey": lease.generation_key,
                    "revision": lease.revision,
                    "executionID": execution_id,
                    "registrationID": registration_id,
                    "args": args,
                    "context": context,
                }),
                deadline,
            )
            .await?;
        validate_fenced_response(&result, lease, Some(execution_id))?;
        result.get("result").cloned().ok_or_else(|| {
            PluginHostError::Protocol("host.tool.execute response is missing result".to_string())
        })
    }

    pub async fn cancel_tool(
        &self,
        lease: &PluginGenerationLease,
        execution_id: &str,
        reason: Option<&str>,
        deadline: Duration,
    ) -> Result<Value, PluginHostError> {
        self.require_generation_fencing()?;
        self.request(
            "host.tool.cancel",
            json!({
                "instanceID": lease.instance_id,
                "generationKey": lease.generation_key,
                "revision": lease.revision,
                "executionID": execution_id,
                "reason": reason,
            }),
            deadline,
        )
        .await
    }

    pub async fn request(
        &self,
        method: &str,
        params: Value,
        deadline: Duration,
    ) -> Result<Value, PluginHostError> {
        self.request_inner(method, params, deadline, false).await
    }

    pub(crate) async fn request_during_shutdown(
        &self,
        method: &str,
        params: Value,
        deadline: Duration,
    ) -> Result<Value, PluginHostError> {
        self.request_inner(method, params, deadline, true).await
    }

    async fn request_inner(
        &self,
        method: &str,
        params: Value,
        deadline: Duration,
        allow_during_shutdown: bool,
    ) -> Result<Value, PluginHostError> {
        let sequence = self.state.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let request_id = format!("backend:{}:{}", self.state.generation, sequence);
        let (sender, receiver) = oneshot::channel();
        let exchange = async {
            self.state
                .register_pending(request_id.clone(), sender, allow_during_shutdown)
                .await?;
            log::debug!(
                "Plugin host RPC request sending: generation={}, request_id={}, method={}",
                self.state.generation,
                request_id,
                method
            );
            self.state
                .outbound
                .send(json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "method": method,
                    "params": params,
                }))
                .await
                .map_err(|_| {
                    PluginHostError::ConnectionClosed("JSON-RPC writer is closed".to_string())
                })?;
            receiver.await.map_err(|_| {
                PluginHostError::ConnectionClosed("JSON-RPC response channel is closed".to_string())
            })?
        };
        match tokio::time::timeout(deadline, exchange).await {
            Ok(result) => {
                if result.is_err() {
                    self.state.remove_pending(&request_id).await;
                }
                result
            }
            Err(_) => {
                self.state.remove_pending(&request_id).await;
                log::warn!(
                    "Plugin host RPC request timed out: generation={}, request_id={}, method={}",
                    self.state.generation,
                    request_id,
                    method
                );
                Err(PluginHostError::RequestTimeout {
                    method: method.to_string(),
                    request_id,
                })
            }
        }
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<(), PluginHostError> {
        if self.state.draining.load(Ordering::Acquire) {
            return Err(PluginHostError::ShuttingDown);
        }
        let permit = self.state.outbound.reserve().await.map_err(|_| {
            PluginHostError::ConnectionClosed("JSON-RPC writer is closed".to_string())
        })?;
        let _admission = self.state.admission.lock().await;
        if self.state.draining.load(Ordering::Acquire) {
            return Err(PluginHostError::ShuttingDown);
        }
        if self.is_closed() {
            return Err(PluginHostError::ConnectionClosed(
                "JSON-RPC peer is closed".to_string(),
            ));
        }
        permit.send(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }));
        log::debug!(
            "Plugin host RPC notification sent: generation={}, method={}",
            self.state.generation,
            method
        );
        Ok(())
    }

    pub async fn begin_draining(&self) -> usize {
        let _admission = self.state.admission.lock().await;
        self.state.draining.store(true, Ordering::Release);
        let pending = self.state.pending.lock().await;
        pending.len()
    }

    pub async fn wait_for_pending(&self, deadline: Duration) -> bool {
        let wait = async {
            loop {
                let notified = self.state.pending_empty.notified();
                if self.state.pending.lock().await.is_empty() {
                    return;
                }
                notified.await;
            }
        };
        tokio::time::timeout(deadline, wait).await.is_ok()
    }

    pub async fn close(&self, reason: impl Into<String>) {
        self.state.close(reason.into()).await;
    }

    pub async fn register_handler<F, Fut>(
        &self,
        method: &str,
        handler: F,
    ) -> Result<(), PluginHostError>
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, RpcHandlerError>> + Send + 'static,
    {
        let mut handlers = self.state.handlers.write().await;
        if handlers.contains_key(method) {
            return Err(PluginHostError::DuplicateHandler(method.to_string()));
        }
        handlers.insert(
            method.to_string(),
            Arc::new(move |params| Box::pin(handler(params))),
        );
        Ok(())
    }

    fn require_generation_fencing(&self) -> Result<(), PluginHostError> {
        if self.capabilities().supports(GENERATION_FENCING_V1) {
            Ok(())
        } else {
            Err(PluginHostError::Protocol(
                "plugin host did not negotiate generation-fencing-v1".to_string(),
            ))
        }
    }
}

fn validate_fenced_response(
    result: &Value,
    lease: &PluginGenerationLease,
    execution_id: Option<&str>,
) -> Result<(), PluginHostError> {
    let identity_matches = result.get("instanceID").and_then(Value::as_str)
        == Some(lease.instance_id.as_str())
        && result.get("generationKey").and_then(Value::as_str)
            == Some(lease.generation_key.as_str())
        && result.get("revision").and_then(Value::as_str) == Some(lease.revision.as_str());
    let execution_matches = execution_id
        .is_none_or(|expected| result.get("executionID").and_then(Value::as_str) == Some(expected));
    if identity_matches && execution_matches {
        return Ok(());
    }
    Err(PluginHostError::Protocol(
        "plugin host response generation lease does not match the request".to_string(),
    ))
}

pub struct JsonRpcPeer {
    client: PluginHostClient,
}

impl JsonRpcPeer {
    pub fn start(stream: TcpStream, generation: u64, max_frame_bytes: usize) -> Self {
        Self::start_with_capabilities(
            stream,
            generation,
            max_frame_bytes,
            PluginHostCapabilities::default(),
        )
    }

    pub fn start_with_capabilities(
        stream: TcpStream,
        generation: u64,
        max_frame_bytes: usize,
        capabilities: PluginHostCapabilities,
    ) -> Self {
        let (outbound, receiver) = mpsc::channel(OUTBOUND_CAPACITY);
        let state = Arc::new(PeerState {
            generation,
            capabilities,
            max_frame_bytes,
            sequence: AtomicU64::new(0),
            admission: Mutex::new(()),
            pending: Mutex::new(HashMap::new()),
            handlers: RwLock::new(HashMap::new()),
            handler_limit: Arc::new(Semaphore::new(HANDLER_CONCURRENCY)),
            outbound,
            closed: AtomicBool::new(false),
            draining: AtomicBool::new(false),
            pending_empty: Notify::new(),
            close_signal: watch::channel(false).0,
        });
        let (reader, writer) = stream.into_split();
        tokio::spawn(run_reader(reader, state.clone()));
        tokio::spawn(run_writer(writer, receiver, state.clone()));
        Self {
            client: PluginHostClient { state },
        }
    }

    pub fn client(&self) -> PluginHostClient {
        self.client.clone()
    }
}

pub(super) struct PeerState {
    pub(super) generation: u64,
    pub(super) capabilities: PluginHostCapabilities,
    pub(super) max_frame_bytes: usize,
    pub(super) sequence: AtomicU64,
    pub(super) admission: Mutex<()>,
    pub(super) pending: Mutex<HashMap<String, PendingSender>>,
    pub(super) handlers: RwLock<HashMap<String, Handler>>,
    pub(super) handler_limit: Arc<Semaphore>,
    pub(super) outbound: mpsc::Sender<Value>,
    pub(super) closed: AtomicBool,
    pub(super) draining: AtomicBool,
    pub(super) pending_empty: Notify,
    pub(super) close_signal: watch::Sender<bool>,
}

impl PeerState {
    async fn register_pending(
        &self,
        request_id: String,
        sender: PendingSender,
        allow_during_shutdown: bool,
    ) -> Result<(), PluginHostError> {
        let _admission = self.admission.lock().await;
        let mut pending = self.pending.lock().await;
        if self.draining.load(Ordering::Acquire) && !allow_during_shutdown {
            return Err(PluginHostError::ShuttingDown);
        }
        if self.closed.load(Ordering::Acquire) {
            return Err(PluginHostError::ConnectionClosed(
                "JSON-RPC peer is closed".to_string(),
            ));
        }
        pending.insert(request_id, sender);
        Ok(())
    }

    pub(super) async fn remove_pending(&self, request_id: &str) -> Option<PendingSender> {
        let mut pending = self.pending.lock().await;
        let sender = pending.remove(request_id);
        if pending.is_empty() {
            self.pending_empty.notify_waiters();
        }
        sender
    }

    pub(super) async fn close(&self, reason: String) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.close_signal.send_replace(true);
        let pending = std::mem::take(&mut *self.pending.lock().await);
        self.pending_empty.notify_waiters();
        for sender in pending.into_values() {
            let _ = sender.send(Err(PluginHostError::ConnectionClosed(reason.clone())));
        }
    }
}
