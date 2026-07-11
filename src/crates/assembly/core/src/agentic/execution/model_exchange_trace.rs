use super::types::RoundContext;
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::ai::AIClient;
use crate::service::config::{
    GlobalConfigManager, ModelExchangeTracingConfig, ModelExchangeTracingMode,
};
use crate::service::workspace_runtime::get_workspace_runtime_service_arc;
use async_trait::async_trait;
use bitfun_ai_adapters::{
    ModelExchangeRequestAttempt, ModelExchangeRequestTraceHandle, ModelExchangeResponseTrace,
    ModelExchangeTraceConfig, ModelExchangeTraceSink,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use uuid::Uuid;

const TRACE_LAYOUT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelExchangeTraceRecord {
    version: u32,
    trace_id: String,
    sequence: u64,
    recorded_at: DateTime<Utc>,
    session_id: String,
    turn_id: String,
    operation_kind: String,
    operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operation_trigger: Option<String>,
    capture_mode: ModelExchangeTracingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<ModelExchangeResponseTrace>,
    request: ModelExchangeTraceRequestRecord,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ModelExchangeTraceOperation<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub trigger: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelExchangeTraceRequestRecord {
    provider: String,
    api_format: String,
    model_id: String,
    request_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
    attempt_number: usize,
}

#[derive(Debug, Clone, Copy)]
struct ModelExchangeTracePolicy {
    mode: ModelExchangeTracingMode,
    capture_request_body: bool,
    capture_response_text: bool,
    capture_reasoning: bool,
    capture_tool_calls: bool,
    capture_usage: bool,
    capture_provider_metadata: bool,
}

impl ModelExchangeTracePolicy {
    fn from_config(config: ModelExchangeTracingConfig) -> Option<Self> {
        match config.mode {
            ModelExchangeTracingMode::Off => None,
            ModelExchangeTracingMode::Full => Some(Self {
                mode: ModelExchangeTracingMode::Full,
                capture_request_body: true,
                capture_response_text: true,
                capture_reasoning: true,
                capture_tool_calls: true,
                capture_usage: true,
                capture_provider_metadata: true,
            }),
            ModelExchangeTracingMode::UsageOnly => Some(Self {
                mode: ModelExchangeTracingMode::UsageOnly,
                capture_request_body: false,
                capture_response_text: false,
                capture_reasoning: false,
                capture_tool_calls: false,
                capture_usage: true,
                capture_provider_metadata: false,
            }),
        }
    }
}

#[derive(Debug)]
struct WorkspaceModelExchangeTraceSink {
    trace_session_dir: PathBuf,
    policy: ModelExchangeTracePolicy,
    session_id: String,
    turn_id: String,
    operation_kind: String,
    operation_id: String,
    operation_trigger: Option<String>,
    provider: String,
    api_format: String,
    model_id: String,
    trace_paths: DashMap<String, PathBuf>,
}

impl WorkspaceModelExchangeTraceSink {
    fn new(
        trace_session_dir: PathBuf,
        policy: ModelExchangeTracePolicy,
        session_id: String,
        turn_id: String,
        operation_kind: String,
        operation_id: String,
        operation_trigger: Option<String>,
        provider: String,
        api_format: String,
        model_id: String,
    ) -> Self {
        Self {
            trace_session_dir,
            policy,
            session_id,
            turn_id,
            operation_kind,
            operation_id,
            operation_trigger,
            provider,
            api_format,
            model_id,
            trace_paths: DashMap::new(),
        }
    }

    async fn allocate_sequence(&self) -> Result<u64, String> {
        let key = self.trace_session_dir.to_string_lossy().to_string();
        let allocator = sequence_allocators()
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(None)))
            .clone();
        let mut guard = allocator.lock().await;
        let current = match *guard {
            Some(value) => value,
            None => {
                let detected = detect_last_sequence(&self.trace_session_dir).await?;
                *guard = Some(detected);
                detected
            }
        };
        let next = current.saturating_add(1);
        *guard = Some(next);
        Ok(next)
    }

    fn trace_path(&self, sequence: u64) -> PathBuf {
        self.trace_session_dir
            .join(format!("request-{:06}.json", sequence))
    }

    async fn write_record(
        &self,
        path: &Path,
        record: &ModelExchangeTraceRecord,
    ) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| format!("Failed to create trace directory: {}", error))?;
        }

        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|error| format!("Failed to serialize trace record: {}", error))?;
        tokio::fs::write(path, bytes)
            .await
            .map_err(|error| format!("Failed to write trace record: {}", error))
    }

    async fn read_record(&self, path: &Path) -> Result<ModelExchangeTraceRecord, String> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|error| format!("Failed to read trace record: {}", error))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("Failed to deserialize trace record: {}", error))
    }

    async fn update_response(
        &self,
        trace_id: &str,
        response: &ModelExchangeResponseTrace,
    ) -> Result<(), String> {
        let Some(path) = self
            .trace_paths
            .get(trace_id)
            .map(|entry| entry.value().clone())
        else {
            return Ok(());
        };

        let mut record = self.read_record(&path).await?;
        record.response = Some(self.sanitize_response(response));
        self.write_record(&path, &record).await?;
        self.trace_paths.remove(trace_id);
        Ok(())
    }

    fn sanitize_response(
        &self,
        response: &ModelExchangeResponseTrace,
    ) -> ModelExchangeResponseTrace {
        ModelExchangeResponseTrace {
            kind: response.kind.clone(),
            assistant_text: self
                .policy
                .capture_response_text
                .then(|| response.assistant_text.clone())
                .flatten(),
            thinking: self
                .policy
                .capture_reasoning
                .then(|| response.thinking.clone())
                .flatten(),
            tool_calls: self
                .policy
                .capture_tool_calls
                .then(|| response.tool_calls.clone())
                .flatten(),
            usage: self
                .policy
                .capture_usage
                .then(|| response.usage.clone())
                .flatten(),
            provider_metadata: self
                .policy
                .capture_provider_metadata
                .then(|| response.provider_metadata.clone())
                .flatten(),
            partial_recovery_reason: response.partial_recovery_reason.clone(),
            error: response.error.clone(),
        }
    }
}

#[async_trait]
impl ModelExchangeTraceSink for WorkspaceModelExchangeTraceSink {
    async fn request_attempt_started(
        &self,
        attempt: &ModelExchangeRequestAttempt,
    ) -> Option<ModelExchangeRequestTraceHandle> {
        let sequence = match self.allocate_sequence().await {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    "Model exchange trace sequence allocation failed: session_id={}, error={}",
                    self.session_id, error
                );
                return None;
            }
        };

        let trace_id = Uuid::new_v4().to_string();
        let path = self.trace_path(sequence);
        let record = ModelExchangeTraceRecord {
            version: TRACE_LAYOUT_VERSION,
            trace_id: trace_id.clone(),
            sequence,
            recorded_at: Utc::now(),
            session_id: self.session_id.clone(),
            turn_id: self.turn_id.clone(),
            operation_kind: self.operation_kind.clone(),
            operation_id: self.operation_id.clone(),
            operation_trigger: self.operation_trigger.clone(),
            capture_mode: self.policy.mode,
            response: None,
            request: ModelExchangeTraceRequestRecord {
                provider: self.provider.clone(),
                api_format: self.api_format.clone(),
                model_id: self.model_id.clone(),
                request_url: attempt.request_url.clone(),
                body: attempt.request_body.clone(),
                attempt_number: attempt.attempt_number,
            },
        };

        if let Err(error) = self.write_record(&path, &record).await {
            warn!(
                "Model exchange trace write failed: session_id={}, trace_id={}, error={}",
                self.session_id, trace_id, error
            );
            return None;
        }

        self.trace_paths.insert(trace_id.clone(), path);
        Some(ModelExchangeRequestTraceHandle { trace_id })
    }

    async fn request_attempt_failed(
        &self,
        handle: Option<&ModelExchangeRequestTraceHandle>,
        error: &str,
    ) {
        let Some(handle) = handle else {
            return;
        };

        if let Err(write_error) = self
            .update_response(
                &handle.trace_id,
                &ModelExchangeResponseTrace {
                    kind: "error".to_string(),
                    assistant_text: None,
                    thinking: None,
                    tool_calls: None,
                    usage: None,
                    provider_metadata: None,
                    partial_recovery_reason: None,
                    error: Some(error.to_string()),
                },
            )
            .await
        {
            warn!(
                "Model exchange trace failure update failed: session_id={}, trace_id={}, error={}",
                self.session_id, handle.trace_id, write_error
            );
        }
    }

    async fn request_attempt_completed(
        &self,
        handle: &ModelExchangeRequestTraceHandle,
        response: &ModelExchangeResponseTrace,
    ) {
        if let Err(error) = self.update_response(&handle.trace_id, response).await {
            warn!(
                "Model exchange trace completion update failed: session_id={}, trace_id={}, error={}",
                self.session_id, handle.trace_id, error
            );
        }
    }
}

pub(super) async fn prepare_model_exchange_trace(
    context: &RoundContext,
    round_id: &str,
    ai_client: &AIClient,
) -> Option<ModelExchangeTraceConfig> {
    prepare_model_exchange_trace_for_workspace(
        &context.session_id,
        &context.dialog_turn_id,
        context.workspace.as_ref(),
        context.model_exchange_trace_dir.as_deref(),
        ModelExchangeTraceOperation {
            kind: "model_round",
            id: round_id,
            trigger: None,
        },
        ai_client,
    )
    .await
}

pub(super) async fn prepare_model_exchange_trace_for_workspace(
    session_id: &str,
    turn_id: &str,
    workspace: Option<&WorkspaceBinding>,
    model_exchange_trace_dir: Option<&Path>,
    operation: ModelExchangeTraceOperation<'_>,
    ai_client: &AIClient,
) -> Option<ModelExchangeTraceConfig> {
    let Some(policy) = current_model_exchange_trace_policy().await else {
        return None;
    };

    let Some(workspace) = workspace else {
        debug!(
            "Model exchange trace skipped because operation has no workspace: session_id={}, turn_id={}, operation_kind={}, operation_id={}",
            session_id, turn_id, operation.kind, operation.id
        );
        return None;
    };

    let trace_session_dir = match model_exchange_trace_dir {
        Some(path) => path.to_path_buf(),
        None => match get_workspace_runtime_service_arc()
            .ensure_runtime_for_workspace_binding(workspace)
            .await
        {
            Ok(result) => result.context.request_trace_session_dir(session_id),
            Err(error) => {
                warn!(
                    "Model exchange trace skipped because runtime init failed: session_id={}, operation_kind={}, operation_id={}, error={}",
                    session_id, operation.kind, operation.id, error
                );
                return None;
            }
        },
    };

    Some(ModelExchangeTraceConfig {
        sink: Arc::new(WorkspaceModelExchangeTraceSink::new(
            trace_session_dir,
            policy,
            session_id.to_string(),
            turn_id.to_string(),
            operation.kind.to_string(),
            operation.id.to_string(),
            operation.trigger.map(str::to_string),
            ai_client.config.format.clone(),
            ai_client.config.format.clone(),
            ai_client.config.model.clone(),
        )),
        capture_request_body: policy.capture_request_body,
    })
}

async fn current_model_exchange_trace_policy() -> Option<ModelExchangeTracePolicy> {
    let Ok(config_service) = GlobalConfigManager::get_service().await else {
        return None;
    };

    let tracing_config: ModelExchangeTracingConfig = config_service
        .get_config(Some("app.logging.model_exchange_tracing"))
        .await
        .unwrap_or_default();
    ModelExchangeTracePolicy::from_config(tracing_config)
}

async fn detect_last_sequence(session_dir: &Path) -> Result<u64, String> {
    let mut last_sequence = 0u64;
    let mut entries = match tokio::fs::read_dir(session_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(format!(
                "Failed to inspect trace session directory '{}': {}",
                session_dir.display(),
                error
            ));
        }
    };

    loop {
        let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| format!("Failed to iterate trace session directory: {}", error))?
        else {
            break;
        };

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Some(sequence) = parse_trace_sequence(file_name) else {
            continue;
        };
        last_sequence = last_sequence.max(sequence);
    }

    Ok(last_sequence)
}

fn parse_trace_sequence(file_name: &str) -> Option<u64> {
    file_name
        .strip_prefix("request-")?
        .strip_suffix(".json")?
        .parse::<u64>()
        .ok()
}

fn sequence_allocators() -> &'static DashMap<String, Arc<Mutex<Option<u64>>>> {
    static ALLOCATORS: OnceLock<DashMap<String, Arc<Mutex<Option<u64>>>>> = OnceLock::new();
    ALLOCATORS.get_or_init(DashMap::new)
}
