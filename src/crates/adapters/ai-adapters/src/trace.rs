use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelExchangeRequestAttempt {
    pub request_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_body: Option<Value>,
    pub attempt_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelExchangeRequestTraceHandle {
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelExchangeResponseTrace {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_recovery_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[async_trait]
pub trait ModelExchangeTraceSink: Send + Sync {
    async fn request_attempt_started(
        &self,
        attempt: &ModelExchangeRequestAttempt,
    ) -> Option<ModelExchangeRequestTraceHandle>;

    async fn request_attempt_failed(
        &self,
        handle: Option<&ModelExchangeRequestTraceHandle>,
        error: &str,
    );

    async fn request_attempt_completed(
        &self,
        handle: &ModelExchangeRequestTraceHandle,
        response: &ModelExchangeResponseTrace,
    );
}

#[derive(Clone)]
pub struct ModelExchangeTraceConfig {
    pub sink: Arc<dyn ModelExchangeTraceSink>,
    pub capture_request_body: bool,
}
