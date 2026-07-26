use super::audio::pcm16_le_to_f32_samples;
use super::recognizer::{SpeechRecognizer, SpeechRecognizerWarmupRequest};
use super::types::{SpeechTranscribeRequest, SpeechTranscriptionResult};
use super::{BitFunError, BitFunResult};
use async_trait::async_trait;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Default)]
pub(super) struct SenseVoiceInt8Recognizer {
    cache: Arc<Mutex<Option<CachedSenseVoiceRecognizer>>>,
}

struct CachedSenseVoiceRecognizer {
    model_path: PathBuf,
    tokens_path: PathBuf,
    language: String,
    recognizer: OfflineRecognizer,
}

impl SenseVoiceInt8Recognizer {
    pub(super) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SpeechRecognizer for SenseVoiceInt8Recognizer {
    async fn warmup(&self, request: SpeechRecognizerWarmupRequest) -> BitFunResult<()> {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || {
            let model_dir = request.model_dir;
            let language = request.language;
            let model_path = model_dir.join("model.int8.onnx");
            let tokens_path = model_dir.join("tokens.txt");
            ensure_model_files(&model_path, &tokens_path)?;
            let mut cache = cache
                .lock()
                .map_err(|_| BitFunError::service("Speech recognizer cache lock is poisoned"))?;
            ensure_cached_recognizer(&mut cache, model_path, tokens_path, language)?;
            Ok(())
        })
        .await
        .map_err(|e| BitFunError::service(format!("Speech recognizer warmup task failed: {e}")))?
    }

    async fn unload(&self) -> BitFunResult<()> {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || {
            let mut cache = cache
                .lock()
                .map_err(|_| BitFunError::service("Speech recognizer cache lock is poisoned"))?;
            *cache = None;
            Ok(())
        })
        .await
        .map_err(|e| BitFunError::service(format!("Speech recognizer unload task failed: {e}")))?
    }

    async fn transcribe(
        &self,
        request: SpeechTranscribeRequest,
    ) -> BitFunResult<SpeechTranscriptionResult> {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || transcribe_blocking(request, cache))
            .await
            .map_err(|e| BitFunError::service(format!("Speech transcription task failed: {e}")))?
    }
}

fn transcribe_blocking(
    request: SpeechTranscribeRequest,
    cache: Arc<Mutex<Option<CachedSenseVoiceRecognizer>>>,
) -> BitFunResult<SpeechTranscriptionResult> {
    let started = Instant::now();
    let model_path = request.model_dir.join("model.int8.onnx");
    let tokens_path = request.model_dir.join("tokens.txt");
    ensure_model_files(&model_path, &tokens_path)?;

    let samples = pcm16_le_to_f32_samples(&request.pcm16_le)?;
    if samples.is_empty() {
        return Err(BitFunError::validation("No audio samples were provided"));
    }

    let text = {
        let mut cache = cache
            .lock()
            .map_err(|_| BitFunError::service("Speech recognizer cache lock is poisoned"))?;
        let cached = ensure_cached_recognizer(
            &mut cache,
            model_path,
            tokens_path,
            request.language.clone(),
        )?;
        let stream = cached.recognizer.create_stream();
        stream.accept_waveform(request.sample_rate as i32, &samples);
        cached.recognizer.decode(&stream);

        stream
            .get_result()
            .ok_or_else(|| BitFunError::service("Failed to read speech result"))?
            .text
            .trim()
            .to_string()
    };

    let audio_duration_seconds = samples.len() as f64 / request.sample_rate as f64;
    Ok(SpeechTranscriptionResult {
        text,
        language: request.language,
        duration_ms: started.elapsed().as_millis() as u64,
        audio_duration_seconds,
    })
}

fn ensure_model_files(model_path: &Path, tokens_path: &Path) -> BitFunResult<()> {
    if !model_path.is_file() || !tokens_path.is_file() {
        return Err(BitFunError::NotFound(
            "SenseVoice model files are missing; download or repair the model first".to_string(),
        ));
    }
    Ok(())
}

fn ensure_cached_recognizer(
    cache: &mut Option<CachedSenseVoiceRecognizer>,
    model_path: PathBuf,
    tokens_path: PathBuf,
    language: String,
) -> BitFunResult<&mut CachedSenseVoiceRecognizer> {
    let should_reload = cache.as_ref().is_none_or(|cached| {
        cached.model_path != model_path
            || cached.tokens_path != tokens_path
            || cached.language != language
    });

    if should_reload {
        let recognizer = create_recognizer(&model_path, &tokens_path, &language)?;
        *cache = Some(CachedSenseVoiceRecognizer {
            model_path,
            tokens_path,
            language,
            recognizer,
        });
    }

    cache
        .as_mut()
        .ok_or_else(|| BitFunError::service("Speech recognizer cache is empty"))
}

fn create_recognizer(
    model_path: &Path,
    tokens_path: &Path,
    language: &str,
) -> BitFunResult<OfflineRecognizer> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
        model: Some(model_path.to_string_lossy().to_string()),
        language: Some(language.to_string()),
        use_itn: true,
    };
    config.model_config.tokens = Some(tokens_path.to_string_lossy().to_string());

    OfflineRecognizer::create(&config)
        .ok_or_else(|| BitFunError::service("Failed to create speech recognizer"))
}
