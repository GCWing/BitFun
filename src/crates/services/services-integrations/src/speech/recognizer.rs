use super::types::{SpeechRecognizerKind, SpeechTranscribeRequest, SpeechTranscriptionResult};
use super::BitFunResult;
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(super) struct SpeechRecognizerWarmupRequest {
    pub model_dir: PathBuf,
    pub recognizer: SpeechRecognizerKind,
    pub language: String,
}

#[async_trait]
pub(super) trait SpeechRecognizer: Send + Sync {
    async fn warmup(&self, request: SpeechRecognizerWarmupRequest) -> BitFunResult<()>;

    async fn unload(&self) -> BitFunResult<()>;

    async fn transcribe(
        &self,
        request: SpeechTranscribeRequest,
    ) -> BitFunResult<SpeechTranscriptionResult>;
}
