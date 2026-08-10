use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Identity of the logical model-round attempt that owns one or more
/// adapter-level transport attempts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelExchangeRoundAttempt {
    pub attempt_id: String,
    pub attempt_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelExchangeRequestAttempt {
    pub request_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_body: Option<Value>,
    /// One-based retry number inside a single adapter request invocation.
    pub attempt_number: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_attempt: Option<ModelExchangeRoundAttempt>,
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
    round_attempt: Option<ModelExchangeRoundAttempt>,
}

impl ModelExchangeTraceConfig {
    pub fn new(sink: Arc<dyn ModelExchangeTraceSink>, capture_request_body: bool) -> Self {
        Self {
            sink,
            capture_request_body,
            round_attempt: None,
        }
    }

    pub fn with_round_attempt(mut self, attempt_id: String, attempt_index: u32) -> Self {
        self.round_attempt = Some(ModelExchangeRoundAttempt {
            attempt_id,
            attempt_index,
        });
        self
    }

    pub fn round_attempt(&self) -> Option<&ModelExchangeRoundAttempt> {
        self.round_attempt.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopTraceSink;

    #[async_trait]
    impl ModelExchangeTraceSink for NoopTraceSink {
        async fn request_attempt_started(
            &self,
            _attempt: &ModelExchangeRequestAttempt,
        ) -> Option<ModelExchangeRequestTraceHandle> {
            None
        }

        async fn request_attempt_failed(
            &self,
            _handle: Option<&ModelExchangeRequestTraceHandle>,
            _error: &str,
        ) {
        }

        async fn request_attempt_completed(
            &self,
            _handle: &ModelExchangeRequestTraceHandle,
            _response: &ModelExchangeResponseTrace,
        ) {
        }
    }

    #[test]
    fn round_attempt_is_scoped_without_mutating_the_base_trace_config() {
        let base = ModelExchangeTraceConfig::new(Arc::new(NoopTraceSink), true);
        let scoped = base
            .clone()
            .with_round_attempt("round-1:attempt:2".to_string(), 2);

        assert!(base.round_attempt().is_none());
        assert_eq!(
            scoped.round_attempt(),
            Some(&ModelExchangeRoundAttempt {
                attempt_id: "round-1:attempt:2".to_string(),
                attempt_index: 2,
            })
        );
        assert!(Arc::ptr_eq(&base.sink, &scoped.sink));
    }
}
