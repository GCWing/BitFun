use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct TtsConfig {
    pub language: String,
    pub voice: String,
    pub speed: f32,
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[async_trait]
pub trait TtsProvider: Send + Sync {
    async fn speak(&self, text: &str) -> Result<(), TtsError>;
    async fn stop(&self) -> Result<(), TtsError>;
    fn available_voices(&self) -> Vec<TtsVoice>;
    fn set_voice(&self, voice_id: &str);
    fn provider_name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct TtsVoice {
    pub id: String,
    pub name: String,
    pub language: String,
    pub gender: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("audio output not available: {0}")]
    AudioOutputNotAvailable(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("not supported")]
    NotSupported,
}
