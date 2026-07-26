use super::audio::pcm16_le_to_f32_samples;
use super::recognizer::{SpeechRecognizer, SpeechRecognizerWarmupRequest};
use super::types::{SpeechTranscribeRequest, SpeechTranscriptionResult};
use super::{BitFunError, BitFunResult};
use async_trait::async_trait;
use sherpa_onnx::{OfflineQwen3ASRModelConfig, OfflineRecognizer, OfflineRecognizerConfig};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Default)]
pub(super) struct Qwen3AsrInt8Recognizer {
    cache: Arc<Mutex<Option<CachedQwen3AsrRecognizer>>>,
}

struct CachedQwen3AsrRecognizer {
    conv_frontend_path: PathBuf,
    encoder_path: PathBuf,
    decoder_path: PathBuf,
    tokenizer_path: PathBuf,
    recognizer: OfflineRecognizer,
}

impl Qwen3AsrInt8Recognizer {
    pub(super) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SpeechRecognizer for Qwen3AsrInt8Recognizer {
    async fn warmup(&self, request: SpeechRecognizerWarmupRequest) -> BitFunResult<()> {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || {
            let paths = qwen3_paths(&request.model_dir);
            ensure_model_files(&paths)?;
            let mut cache = cache
                .lock()
                .map_err(|_| BitFunError::service("Speech recognizer cache lock is poisoned"))?;
            ensure_cached_recognizer(&mut cache, paths)?;
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

#[derive(Debug)]
struct Qwen3Paths {
    conv_frontend: PathBuf,
    encoder: PathBuf,
    decoder: PathBuf,
    tokenizer: PathBuf,
}

fn qwen3_paths(model_dir: &Path) -> Qwen3Paths {
    Qwen3Paths {
        conv_frontend: model_dir.join("conv_frontend.onnx"),
        encoder: model_dir.join("encoder.int8.onnx"),
        decoder: model_dir.join("decoder.int8.onnx"),
        tokenizer: model_dir.join("tokenizer"),
    }
}

fn transcribe_blocking(
    request: SpeechTranscribeRequest,
    cache: Arc<Mutex<Option<CachedQwen3AsrRecognizer>>>,
) -> BitFunResult<SpeechTranscriptionResult> {
    let started = Instant::now();
    let paths = qwen3_paths(&request.model_dir);
    ensure_model_files(&paths)?;

    let samples = pcm16_le_to_f32_samples(&request.pcm16_le)?;
    if samples.is_empty() {
        return Err(BitFunError::validation("No audio samples were provided"));
    }

    let text = {
        let mut cache = cache
            .lock()
            .map_err(|_| BitFunError::service("Speech recognizer cache lock is poisoned"))?;
        let cached = ensure_cached_recognizer(&mut cache, paths)?;
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

fn ensure_model_files(paths: &Qwen3Paths) -> BitFunResult<()> {
    if !paths.conv_frontend.is_file()
        || !paths.encoder.is_file()
        || !paths.decoder.is_file()
        || !paths.tokenizer.is_dir()
        || !paths.tokenizer.join("merges.txt").is_file()
        || !paths.tokenizer.join("tokenizer_config.json").is_file()
        || !paths.tokenizer.join("vocab.json").is_file()
    {
        return Err(BitFunError::NotFound(
            "Qwen3-ASR model files are missing; download or repair the model first".to_string(),
        ));
    }
    Ok(())
}

fn ensure_cached_recognizer(
    cache: &mut Option<CachedQwen3AsrRecognizer>,
    paths: Qwen3Paths,
) -> BitFunResult<&mut CachedQwen3AsrRecognizer> {
    let should_reload = cache.as_ref().is_none_or(|cached| {
        cached.conv_frontend_path != paths.conv_frontend
            || cached.encoder_path != paths.encoder
            || cached.decoder_path != paths.decoder
            || cached.tokenizer_path != paths.tokenizer
    });

    if should_reload {
        let recognizer = create_recognizer(&paths)?;
        *cache = Some(CachedQwen3AsrRecognizer {
            conv_frontend_path: paths.conv_frontend,
            encoder_path: paths.encoder,
            decoder_path: paths.decoder,
            tokenizer_path: paths.tokenizer,
            recognizer,
        });
    }

    cache
        .as_mut()
        .ok_or_else(|| BitFunError::service("Speech recognizer cache is empty"))
}

fn create_recognizer(paths: &Qwen3Paths) -> BitFunResult<OfflineRecognizer> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
        conv_frontend: Some(paths.conv_frontend.to_string_lossy().to_string()),
        encoder: Some(paths.encoder.to_string_lossy().to_string()),
        decoder: Some(paths.decoder.to_string_lossy().to_string()),
        tokenizer: Some(paths.tokenizer.to_string_lossy().to_string()),
        max_new_tokens: 512,
        ..OfflineQwen3ASRModelConfig::default()
    };
    config.model_config.num_threads = 3;
    config.decoding_method = Some("greedy_search".to_string());

    OfflineRecognizer::create(&config)
        .ok_or_else(|| BitFunError::service("Failed to create Qwen3-ASR speech recognizer"))
}
