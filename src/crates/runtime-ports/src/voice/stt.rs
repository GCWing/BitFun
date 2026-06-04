use async_trait::async_trait;
use futures::Stream;

#[derive(Debug, Clone)]
pub struct SttConfig {
    pub language: String,
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct SttResult {
    pub text: String,
    pub is_final: bool,
    pub confidence: Option<f32>,
}

#[async_trait]
pub trait SttProvider: Send + Sync {
    async fn start_listening(&self) -> Result<Box<dyn Stream<Item = SttResult> + Send + Unpin>, SttError>;
    async fn stop_listening(&self) -> Result<(), SttError>;
    fn provider_name(&self) -> &'static str;
}

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("microphone not available: {0}")]
    MicrophoneNotAvailable(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("not supported")]
    NotSupported,
}