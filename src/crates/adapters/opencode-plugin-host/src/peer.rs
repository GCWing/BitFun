use crate::peer_runtime::{run_reader, run_writer};
use crate::{
    PluginGenerationLease, PluginHostCapabilities, PluginHostError, PluginInstanceOpenRequest,
    PluginPreparationSummary, PluginPrepareRequest, GENERATION_FENCING_V1,
};
use openbitfun_runtime_ports::{
    HookFunctionAfterRequest, HookFunctionAfterResult, HookFunctionAvailability,
    HookFunctionBeforeRequest, HookFunctionBeforeResult, HookFunctionCancelRequest,
    HookFunctionCancelResult, HookFunctionConfigContribution, HookFunctionConfigContributor,
    HookFunctionDiagnostic, HookFunctionDisposeRequest, HookFunctionDisposeResult,
    HookFunctionGeneration, HookFunctionHookKind, HookFunctionPluginDeclaration,
    HookFunctionPluginIdentity, HookFunctionRegistrationBatch, HookFunctionRegistrationSink,
    HookFunctionReverseAsk, HookFunctionReverseMetadata, HookFunctionReverseReply,
    HookFunctionReverseSink, HookFunctionRuntime, HookFunctionStartRequest,
    HookFunctionToolAttachment, HookFunctionToolRegistration, HookFunctionToolRequest,
    HookFunctionToolResult, PortError, PortErrorKind, PortResult,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
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

/// Removes an admitted RPC when its future is dropped by an outer timeout or
/// cancellation. The response reader normally removes pending entries first;
/// this guard closes the cancellation-safety gap for callers that stop polling.
struct PendingRequestGuard {
    state: Arc<PeerState>,
    request_id: String,
}

impl Drop for PendingRequestGuard {
    fn drop(&mut self) {
        let state = self.state.clone();
        let request_id = self.request_id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                state.remove_pending(&request_id).await;
            });
        }
    }
}

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

type RuntimeGenerationKey = (String, String, String);

/// Typed OpenCode hook/function data plane. OpenCode JSON-RPC wire shapes are
/// decoded here so capability owners only see provider-neutral contracts.
pub struct OpenCodeHookFunctionRuntime {
    client: PluginHostClient,
    reverse_sinks: Arc<RwLock<HashMap<RuntimeGenerationKey, Arc<dyn HookFunctionReverseSink>>>>,
    reverse_handlers_registered: Mutex<bool>,
}

impl OpenCodeHookFunctionRuntime {
    pub fn new(client: PluginHostClient) -> Self {
        Self {
            client,
            reverse_sinks: Arc::new(RwLock::new(HashMap::new())),
            reverse_handlers_registered: Mutex::new(false),
        }
    }

    async fn ensure_reverse_handlers(&self) -> PortResult<()> {
        let mut registered = self.reverse_handlers_registered.lock().await;
        if *registered {
            return Ok(());
        }

        let metadata_sinks = self.reverse_sinks.clone();
        self.client
            .register_handler("backend.tool.metadata", move |params| {
                let sinks = metadata_sinks.clone();
                async move {
                    let params: ReverseMetadataParams =
                        serde_json::from_value(params).map_err(invalid_reverse_params)?;
                    let generation = params.generation()?;
                    let sink = reverse_sink(&sinks, &generation).await?;
                    sink.metadata(HookFunctionReverseMetadata {
                        generation,
                        execution_id: params.execution_id,
                        title: params.title.unwrap_or_default(),
                        metadata: params.metadata.unwrap_or_default(),
                    })
                    .await
                    .map_err(reverse_port_error)?;
                    Ok(json!({}))
                }
            })
            .await
            .map_err(|error| map_invocation_error(error, false))?;

        let ask_sinks = self.reverse_sinks.clone();
        self.client
            .register_handler("backend.tool.ask", move |params| {
                let sinks = ask_sinks.clone();
                async move {
                    let params: ReverseAskParams =
                        serde_json::from_value(params).map_err(invalid_reverse_params)?;
                    let generation = params.generation()?;
                    let sink = reverse_sink(&sinks, &generation).await?;
                    match sink
                        .ask(HookFunctionReverseAsk {
                            generation,
                            execution_id: params.execution_id,
                            permission: params.permission,
                            patterns: params.patterns,
                            always: params.always,
                            metadata: params.metadata,
                        })
                        .await
                        .map_err(reverse_port_error)?
                    {
                        HookFunctionReverseReply::Once | HookFunctionReverseReply::Always => {
                            Ok(json!({}))
                        }
                        HookFunctionReverseReply::Reject { feedback } => Err(RpcHandlerError {
                            code: -32003,
                            message: feedback.unwrap_or_else(|| "permission denied".to_string()),
                            data: None,
                        }),
                    }
                }
            })
            .await
            .map_err(|error| map_invocation_error(error, false))?;

        *registered = true;
        Ok(())
    }
}

pub fn hook_function_runtime(client: PluginHostClient) -> Arc<dyn HookFunctionRuntime> {
    Arc::new(OpenCodeHookFunctionRuntime::new(client))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginIdentity {
    id: Option<String>,
    spec: String,
    entry: String,
    index: usize,
}

impl From<RawPluginIdentity> for HookFunctionPluginIdentity {
    fn from(value: RawPluginIdentity) -> Self {
        Self {
            id: value.id,
            spec: value.spec,
            entry: value.entry,
            index: value.index,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawConfigContributor {
    plugin: RawPluginIdentity,
    outcome: openbitfun_runtime_ports::HookFunctionContributorOutcome,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawConfigContribution {
    plugin: RawPluginIdentity,
    outcome: openbitfun_runtime_ports::HookFunctionContributorOutcome,
    config: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawToolRegistration {
    #[serde(rename = "registrationID")]
    registration_id: String,
    id: String,
    plugin: Option<RawPluginIdentity>,
    description: String,
    parameters: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawOpenInstanceResult {
    #[serde(rename = "instanceID")]
    instance_id: String,
    generation_key: String,
    revision: String,
    config: serde_json::Map<String, Value>,
    config_contributors: Vec<RawConfigContributor>,
    config_contributions: Vec<RawConfigContribution>,
    diagnostics: Vec<HookFunctionDiagnostic>,
    hooks: Vec<String>,
    tools: Vec<RawToolRegistration>,
    #[serde(rename = "auth")]
    _auth: Vec<Value>,
    #[serde(rename = "providers")]
    _providers: Vec<Value>,
    #[serde(rename = "workspaces")]
    _workspaces: Vec<Value>,
    #[serde(rename = "gatewayURL")]
    _gateway_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginPreparation {
    review_digest: String,
    reviewed: Vec<Value>,
    prepared: Vec<RawPreparedPlugin>,
    failed: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPreparedPlugin {
    identity: String,
    content_hash: Option<String>,
}

impl RawPluginPreparation {
    fn into_summary(self) -> PluginPreparationSummary {
        let content_digests = self
            .prepared
            .iter()
            .filter_map(|plugin| {
                plugin
                    .content_hash
                    .as_ref()
                    .map(|digest| (plugin.identity.clone(), digest.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        PluginPreparationSummary {
            review_digest: self.review_digest,
            reviewed_count: self.reviewed.len(),
            prepared_count: self.prepared.len(),
            failed_count: self.failed.len(),
            content_digests,
        }
    }
}

impl RawOpenInstanceResult {
    fn into_batch(
        self,
        expected: &HookFunctionGeneration,
    ) -> PortResult<HookFunctionRegistrationBatch> {
        if self.instance_id != expected.instance_id
            || self.generation_key != expected.generation_key
            || self.revision != expected.revision
        {
            return Err(PortError::new(
                PortErrorKind::Backend,
                "plugin host open result generation lease does not match the request",
            ));
        }
        let hooks = self
            .hooks
            .into_iter()
            .filter_map(|hook| match hook.as_str() {
                "tool.execute.before" => Some(HookFunctionHookKind::ToolExecuteBefore),
                "tool.execute.after" => Some(HookFunctionHookKind::ToolExecuteAfter),
                _ => None,
            })
            .collect();
        Ok(HookFunctionRegistrationBatch {
            generation: expected.clone(),
            config: self.config,
            config_contributors: self
                .config_contributors
                .into_iter()
                .map(|entry| HookFunctionConfigContributor {
                    plugin: entry.plugin.into(),
                    outcome: entry.outcome,
                })
                .collect(),
            config_contributions: self
                .config_contributions
                .into_iter()
                .map(|entry| HookFunctionConfigContribution {
                    plugin: entry.plugin.into(),
                    outcome: entry.outcome,
                    config: entry.config,
                })
                .collect(),
            diagnostics: self.diagnostics,
            hooks,
            tools: self
                .tools
                .into_iter()
                .map(|tool| HookFunctionToolRegistration {
                    registration_id: tool.registration_id,
                    id: tool.id,
                    plugin: tool.plugin.map(Into::into),
                    description: tool.description,
                    parameters: tool.parameters,
                })
                .collect(),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReverseMetadataParams {
    #[serde(rename = "instanceID")]
    instance_id: String,
    generation_key: Option<String>,
    revision: Option<String>,
    #[serde(rename = "executionID")]
    execution_id: String,
    title: Option<String>,
    metadata: Option<serde_json::Map<String, Value>>,
}

impl ReverseMetadataParams {
    fn generation(&self) -> Result<HookFunctionGeneration, RpcHandlerError> {
        reverse_generation(&self.instance_id, &self.generation_key, &self.revision)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReverseAskParams {
    #[serde(rename = "instanceID")]
    instance_id: String,
    generation_key: Option<String>,
    revision: Option<String>,
    #[serde(rename = "executionID")]
    execution_id: String,
    permission: String,
    patterns: Vec<String>,
    always: Vec<String>,
    metadata: serde_json::Map<String, Value>,
}

impl ReverseAskParams {
    fn generation(&self) -> Result<HookFunctionGeneration, RpcHandlerError> {
        reverse_generation(&self.instance_id, &self.generation_key, &self.revision)
    }
}

fn reverse_generation(
    instance_id: &str,
    generation_key: &Option<String>,
    revision: &Option<String>,
) -> Result<HookFunctionGeneration, RpcHandlerError> {
    Ok(HookFunctionGeneration {
        instance_id: instance_id.to_string(),
        generation_key: generation_key.clone().ok_or_else(|| {
            RpcHandlerError::new(
                -32602,
                "generationKey is required for plugin tool callbacks",
            )
        })?,
        revision: revision.clone().ok_or_else(|| {
            RpcHandlerError::new(-32602, "revision is required for plugin tool callbacks")
        })?,
    })
}

async fn reverse_sink(
    sinks: &RwLock<HashMap<RuntimeGenerationKey, Arc<dyn HookFunctionReverseSink>>>,
    generation: &HookFunctionGeneration,
) -> Result<Arc<dyn HookFunctionReverseSink>, RpcHandlerError> {
    sinks
        .read()
        .await
        .get(&generation_key(generation))
        .cloned()
        .ok_or_else(|| RpcHandlerError::new(-32004, "plugin generation is no longer active"))
}

fn invalid_reverse_params(error: serde_json::Error) -> RpcHandlerError {
    RpcHandlerError::new(
        -32602,
        format!("invalid plugin tool callback params: {error}"),
    )
}

fn reverse_port_error(error: PortError) -> RpcHandlerError {
    let code = if error.kind == PortErrorKind::PermissionDenied {
        -32003
    } else {
        -32603
    };
    RpcHandlerError::new(code, error.to_string())
}

fn generation_key(generation: &HookFunctionGeneration) -> RuntimeGenerationKey {
    (
        generation.instance_id.clone(),
        generation.generation_key.clone(),
        generation.revision.clone(),
    )
}

fn lease(generation: &HookFunctionGeneration) -> PluginGenerationLease {
    PluginGenerationLease {
        instance_id: generation.instance_id.clone(),
        generation_key: generation.generation_key.clone(),
        revision: generation.revision.clone(),
    }
}

fn map_invocation_error(
    error: PluginHostError,
    side_effect: bool,
) -> openbitfun_runtime_ports::PortError {
    use openbitfun_runtime_ports::{PortError, PortErrorKind};
    let kind = match error {
        // The tool may have completed before cancellation was observed, while
        // its authoritative response was lost at the deadline. Retrying can
        // duplicate side effects even when cancellation is later confirmed.
        PluginHostError::ToolStoppedAfterTimeout { .. } => PortErrorKind::OutcomeUnknown,
        PluginHostError::ToolOutcomeUnknown { .. } => PortErrorKind::OutcomeUnknown,
        PluginHostError::RequestTimeout { .. } if side_effect => PortErrorKind::OutcomeUnknown,
        PluginHostError::RequestTimeout { .. } => PortErrorKind::Timeout,
        PluginHostError::ShuttingDown => PortErrorKind::CleanupRequired,
        PluginHostError::ConnectionClosed(_) => PortErrorKind::OutcomeUnknown,
        PluginHostError::Rpc { code, .. } if code == -32003 => PortErrorKind::PermissionDenied,
        PluginHostError::Rpc { .. } => PortErrorKind::Backend,
        _ => PortErrorKind::Backend,
    };
    PortError::new(kind, error.to_string())
}

#[async_trait::async_trait]
impl HookFunctionRuntime for OpenCodeHookFunctionRuntime {
    fn availability(&self) -> HookFunctionAvailability {
        if self.client.is_closed() {
            HookFunctionAvailability::Unavailable {
                reason: "plugin host connection is closed".to_string(),
            }
        } else {
            HookFunctionAvailability::Available
        }
    }

    async fn start(
        &self,
        request: HookFunctionStartRequest,
        registrations: Arc<dyn HookFunctionRegistrationSink>,
        reverse: Arc<dyn HookFunctionReverseSink>,
        deadline: Duration,
    ) -> PortResult<HookFunctionGeneration> {
        self.ensure_reverse_handlers().await?;
        self.reverse_sinks
            .write()
            .await
            .insert(generation_key(&request.generation), reverse);
        let generation = request.generation.clone();
        let plugins = request
            .plugins
            .into_iter()
            .map(
                |plugin: HookFunctionPluginDeclaration| crate::PluginDeclaration {
                    spec: plugin.spec,
                    options: plugin.options,
                    base_directory: plugin.base_directory,
                },
            )
            .collect();
        let result = self
            .client
            .open_instance(
                PluginInstanceOpenRequest {
                    instance_id: generation.instance_id.clone(),
                    generation_key: generation.generation_key.clone(),
                    revision: generation.revision.clone(),
                    project: json!({
                        "id": request.project_id,
                        "worktree": request.project_worktree,
                        "time": {"created": request.project_created_at_ms},
                    }),
                    config: request.config,
                    directory: request.directory,
                    worktree: request.worktree,
                    plugins,
                    configuration_fingerprint: request.configuration_fingerprint,
                    expected_content_digests: (!request.expected_content_digests.is_empty())
                        .then_some(request.expected_content_digests),
                    expected_review_digest: request.expected_review_digest,
                },
                deadline,
            )
            .await
            .map_err(|error| map_invocation_error(error, false));
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.reverse_sinks
                    .write()
                    .await
                    .remove(&generation_key(&generation));
                return Err(error);
            }
        };
        let raw: RawOpenInstanceResult = serde_json::from_value(result).map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("plugin host open result is invalid: {error}"),
            )
        })?;
        let batch = raw.into_batch(&generation)?;
        if let Err(error) = registrations.publish_generation(batch).await {
            self.reverse_sinks
                .write()
                .await
                .remove(&generation_key(&generation));
            let _ = self
                .client
                .close_instance(&generation.instance_id, deadline)
                .await;
            return Err(error);
        }
        Ok(generation)
    }

    async fn transform_tool_before(
        &self,
        request: HookFunctionBeforeRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionBeforeResult> {
        let result = self
            .client
            .call_hook(
                &lease(&request.generation),
                "tool.execute.before",
                json!({
                    "tool": request.tool_name,
                    "sessionID": request.session_id,
                    "callID": request.call_id,
                }),
                json!({"args": request.args}),
                deadline,
            )
            .await;
        let result = match result {
            Ok(result) => result,
            Err(error @ PluginHostError::RequestTimeout { .. }) => {
                self.client
                    .close("plugin before-hook deadline exceeded".to_string())
                    .await;
                return Err(map_invocation_error(error, false));
            }
            Err(error) => return Err(map_invocation_error(error, false)),
        };
        let args = result
            .get("output")
            .and_then(|output| output.get("args"))
            .cloned()
            .ok_or_else(|| {
                PortError::new(
                    PortErrorKind::Backend,
                    "tool.execute.before response is missing output.args",
                )
            })?;
        Ok(HookFunctionBeforeResult { args })
    }

    async fn execute_tool(
        &self,
        request: HookFunctionToolRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionToolResult> {
        let result = self
            .client
            .execute_tool(
                &lease(&request.generation),
                &request.execution_id,
                &request.registration_id,
                request.args,
                serde_json::to_value(request.context).map_err(|error| {
                    PortError::new(
                        PortErrorKind::InvalidRequest,
                        format!("plugin tool context is invalid: {error}"),
                    )
                })?,
                deadline,
            )
            .await;
        let result = match result {
            Ok(result) => result,
            Err(PluginHostError::RequestTimeout { .. }) => {
                const CANCEL_CONFIRM_TIMEOUT: Duration = Duration::from_secs(5);
                let cancellation = self
                    .client
                    .cancel_tool(
                        &lease(&request.generation),
                        &request.execution_id,
                        Some("deadline_exceeded"),
                        CANCEL_CONFIRM_TIMEOUT,
                    )
                    .await;
                let reason = match cancellation {
                    Ok(result)
                        if result.get("cancelled").and_then(Value::as_bool) == Some(true) =>
                    {
                        "plugin tool response timed out and cancellation was confirmed".to_string()
                    }
                    Ok(_) => "plugin host did not confirm cancellation after the tool deadline"
                        .to_string(),
                    Err(error) => {
                        format!("plugin host cancellation failed after the tool deadline: {error}")
                    }
                };
                self.client.close(reason.clone()).await;
                return Err(map_invocation_error(
                    PluginHostError::ToolOutcomeUnknown {
                        execution_id: request.execution_id,
                        reason,
                    },
                    true,
                ));
            }
            Err(error) => return Err(map_invocation_error(error, true)),
        };
        let attachments = result
            .get("attachments")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|attachment| {
                Ok(HookFunctionToolAttachment {
                    mime: attachment
                        .get("mime")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            PortError::new(
                                PortErrorKind::Backend,
                                "plugin tool attachment is missing mime",
                            )
                        })?
                        .to_string(),
                    url: attachment
                        .get("url")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            PortError::new(
                                PortErrorKind::Backend,
                                "plugin tool attachment is missing url",
                            )
                        })?
                        .to_string(),
                    filename: attachment
                        .get("filename")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect::<PortResult<Vec<_>>>()?;
        Ok(HookFunctionToolResult {
            output: result,
            attachments,
        })
    }

    async fn transform_tool_after(
        &self,
        request: HookFunctionAfterRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionAfterResult> {
        let result = self
            .client
            .call_hook(
                &lease(&request.generation),
                "tool.execute.after",
                json!({
                    "tool": request.tool_name,
                    "sessionID": request.session_id,
                    "callID": request.call_id,
                    "args": request.args,
                }),
                serde_json::to_value(request.output).map_err(|error| {
                    PortError::new(
                        PortErrorKind::InvalidRequest,
                        format!("plugin after-hook output is invalid: {error}"),
                    )
                })?,
                deadline,
            )
            .await;
        let result = match result {
            Ok(result) => result,
            Err(error @ PluginHostError::RequestTimeout { .. }) => {
                self.client
                    .close("plugin after-hook deadline exceeded".to_string())
                    .await;
                return Err(map_invocation_error(error, false));
            }
            Err(error) => return Err(map_invocation_error(error, false)),
        };
        serde_json::from_value(result.get("output").cloned().ok_or_else(|| {
            PortError::new(
                PortErrorKind::Backend,
                "tool.execute.after response is missing output",
            )
        })?)
        .map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("tool.execute.after response is invalid: {error}"),
            )
        })
    }

    async fn cancel(
        &self,
        request: HookFunctionCancelRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionCancelResult> {
        let result = self
            .client
            .cancel_tool(
                &lease(&request.generation),
                &request.execution_id,
                request.reason.as_deref(),
                deadline,
            )
            .await
            .map_err(|error| map_invocation_error(error, true))?;
        Ok(HookFunctionCancelResult {
            stopped: result
                .get("cancelled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    async fn dispose(
        &self,
        request: HookFunctionDisposeRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionDisposeResult> {
        let closed = self
            .client
            .close_instance(&request.generation.instance_id, deadline)
            .await
            .map_err(|error| map_invocation_error(error, true))?;
        if closed {
            self.reverse_sinks
                .write()
                .await
                .remove(&generation_key(&request.generation));
        }
        Ok(HookFunctionDisposeResult { closed })
    }
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
    ) -> Result<PluginPreparationSummary, PluginHostError> {
        let params = serde_json::to_value(request)
            .map_err(|error| PluginHostError::Protocol(error.to_string()))?;
        let result = self
            .request("host.plugins.prepare", params, deadline)
            .await?;
        serde_json::from_value::<RawPluginPreparation>(result)
            .map(RawPluginPreparation::into_summary)
            .map_err(|error| {
                PluginHostError::Protocol(format!(
                    "host.plugins.prepare returned an invalid result: {error}"
                ))
            })
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
            let _pending_guard = PendingRequestGuard {
                state: self.state.clone(),
                request_id: request_id.clone(),
            };
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

    pub(crate) async fn register_handler<F, Fut>(
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
