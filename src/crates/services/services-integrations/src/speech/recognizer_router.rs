use super::qwen3_asr_int8::Qwen3AsrInt8Recognizer;
use super::recognizer::{SpeechRecognizer, SpeechRecognizerWarmupRequest};
use super::sensevoice_int8::SenseVoiceInt8Recognizer;
use super::types::{SpeechRecognizerKind, SpeechTranscribeRequest, SpeechTranscriptionResult};
use super::BitFunResult;
use async_trait::async_trait;

#[derive(Clone)]
pub(super) struct SpeechRecognizerRouter {
    sensevoice: SenseVoiceInt8Recognizer,
    qwen3_asr: Qwen3AsrInt8Recognizer,
}

impl SpeechRecognizerRouter {
    pub(super) fn new() -> Self {
        Self {
            sensevoice: SenseVoiceInt8Recognizer::new(),
            qwen3_asr: Qwen3AsrInt8Recognizer::new(),
        }
    }
}

#[async_trait]
impl SpeechRecognizer for SpeechRecognizerRouter {
    async fn warmup(&self, request: SpeechRecognizerWarmupRequest) -> BitFunResult<()> {
        match request.recognizer {
            SpeechRecognizerKind::SenseVoiceInt8 => self.sensevoice.warmup(request).await,
            SpeechRecognizerKind::Qwen3AsrInt8 => self.qwen3_asr.warmup(request).await,
        }
    }

    async fn unload(&self) -> BitFunResult<()> {
        self.sensevoice.unload().await?;
        self.qwen3_asr.unload().await?;
        Ok(())
    }

    async fn transcribe(
        &self,
        request: SpeechTranscribeRequest,
    ) -> BitFunResult<SpeechTranscriptionResult> {
        match request.recognizer {
            SpeechRecognizerKind::SenseVoiceInt8 => self.sensevoice.transcribe(request).await,
            SpeechRecognizerKind::Qwen3AsrInt8 => self.qwen3_asr.transcribe(request).await,
        }
    }
}
