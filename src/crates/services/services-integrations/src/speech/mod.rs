//! Local speech input services.

mod audio;
mod downloader;
mod error;
mod model_catalog;
mod model_store;
mod qwen3_asr_int8;
mod recognizer;
mod recognizer_router;
mod sensevoice_int8;
mod types;

use self::downloader::download_and_install_model;
use self::model_catalog::get_builtin_speech_model_manifest;
use self::model_store::SpeechModelStore;
use self::recognizer::{SpeechRecognizer, SpeechRecognizerWarmupRequest};
use self::recognizer_router::SpeechRecognizerRouter;
use self::types::SpeechTranscribeRequest;
pub use self::types::{
    DEFAULT_MAX_RECORDING_SECONDS, DEFAULT_SPEECH_SAMPLE_RATE, LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_ID,
    LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_REF, LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID,
    LOCAL_SENSEVOICE_SMALL_INT8_MODEL_REF,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
pub use bitfun_core_types::speech::*;
pub use error::{SpeechError, SpeechResult};
use error::{SpeechError as BitFunError, SpeechResult as BitFunResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SpeechStoragePaths {
    models_dir: PathBuf,
    downloads_dir: PathBuf,
    input_temp_dir: PathBuf,
}

impl SpeechStoragePaths {
    pub fn new(models_dir: PathBuf, downloads_dir: PathBuf, input_temp_dir: PathBuf) -> Self {
        Self {
            models_dir,
            downloads_dir,
            input_temp_dir,
        }
    }

    pub fn models_dir(&self) -> &std::path::Path {
        &self.models_dir
    }

    pub fn downloads_dir(&self) -> &std::path::Path {
        &self.downloads_dir
    }

    pub fn input_temp_dir(&self) -> &std::path::Path {
        &self.input_temp_dir
    }
}

#[derive(Clone)]
pub struct SpeechService {
    store: SpeechModelStore,
    recognizer: Arc<dyn SpeechRecognizer>,
    downloads: Arc<Mutex<HashMap<String, Arc<SpeechModelDownloadControl>>>>,
    sessions: Arc<Mutex<HashMap<String, SpeechInputSessionState>>>,
}

#[derive(Debug)]
struct SpeechModelDownloadControl {
    cancel: CancellationToken,
    finished: CancellationToken,
}

#[derive(Debug)]
struct SpeechInputSessionState {
    session: SpeechInputSession,
    audio_path: PathBuf,
    received_bytes: u64,
}

impl SpeechService {
    pub fn new(paths: SpeechStoragePaths) -> Self {
        Self {
            store: SpeechModelStore::new(paths),
            recognizer: Arc::new(SpeechRecognizerRouter::new()),
            downloads: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn list_models(&self) -> BitFunResult<SpeechListModelsResponse> {
        Ok(SpeechListModelsResponse {
            models: self.store.list_statuses().await?,
        })
    }

    pub async fn model_status(&self, model_id: &str) -> BitFunResult<SpeechModelStatus> {
        let manifest = get_builtin_speech_model_manifest(model_id)?;
        self.store.status_for_manifest(&manifest).await
    }

    pub async fn download_model<F>(
        &self,
        request: SpeechDownloadModelRequest,
        on_progress: F,
    ) -> BitFunResult<SpeechModelStatus>
    where
        F: Fn(SpeechModelProgressEvent) + Send + Sync + 'static,
    {
        let manifest = get_builtin_speech_model_manifest(&request.model_id)?;
        let control = Arc::new(SpeechModelDownloadControl {
            cancel: CancellationToken::new(),
            finished: CancellationToken::new(),
        });
        {
            let mut downloads = self.downloads.lock().await;
            if downloads.contains_key(&manifest.id) {
                return Err(BitFunError::validation(format!(
                    "Speech model is already downloading: {}",
                    manifest.id
                )));
            }
            downloads.insert(manifest.id.clone(), Arc::clone(&control));
        }

        let store = self.store.clone();
        let downloads = Arc::clone(&self.downloads);
        let task_control = Arc::clone(&control);
        let model_id = manifest.id.clone();
        let task = tokio::spawn(async move {
            let result = download_and_install_model(
                &store,
                &manifest,
                task_control.cancel.clone(),
                |progress| {
                    let status = SpeechModelStatus {
                        model_id: manifest.id.clone(),
                        display_name: manifest.display_name.clone(),
                        provider: manifest.provider.clone(),
                        version: manifest.version.clone(),
                        description: manifest.description.clone(),
                        languages: manifest.languages.clone(),
                        state: SpeechModelInstallState::Downloading,
                        installed_path: None,
                        installed_bytes: progress.downloaded_bytes,
                        expected_bytes: manifest.expected_bytes(),
                        progress: Some(progress),
                        error: None,
                    };
                    on_progress(SpeechModelProgressEvent { status });
                },
            )
            .await;

            task_control.finished.cancel();
            let mut active_downloads = downloads.lock().await;
            let is_current = active_downloads
                .get(&model_id)
                .map(|active| Arc::ptr_eq(active, &task_control))
                .unwrap_or(false);
            if is_current {
                active_downloads.remove(&model_id);
            }
            result
        });

        task.await.map_err(|error| {
            BitFunError::service(format!("Speech model download task failed: {error}"))
        })?
    }

    async fn cancel_active_download(&self, model_id: &str) {
        let control = self.downloads.lock().await.get(model_id).cloned();
        if let Some(control) = control {
            control.cancel.cancel();
            control.finished.cancelled().await;
        }
    }

    pub async fn cancel_model_download(
        &self,
        request: SpeechCancelModelDownloadRequest,
    ) -> BitFunResult<SpeechModelStatus> {
        let manifest = get_builtin_speech_model_manifest(&request.model_id)?;
        self.cancel_active_download(&manifest.id).await;
        self.store.status_for_manifest(&manifest).await
    }

    pub async fn delete_model(
        &self,
        request: SpeechDeleteModelRequest,
    ) -> BitFunResult<SpeechModelStatus> {
        let manifest = get_builtin_speech_model_manifest(&request.model_id)?;
        self.cancel_active_download(&manifest.id).await;
        self.recognizer.unload().await?;
        self.store.delete_model(&manifest).await
    }

    pub async fn verify_model(
        &self,
        request: SpeechVerifyModelRequest,
    ) -> BitFunResult<SpeechModelStatus> {
        let manifest = get_builtin_speech_model_manifest(&request.model_id)?;
        self.store.verify_model(&manifest).await
    }

    pub async fn start_input_session(
        &self,
        request: SpeechStartInputSessionRequest,
    ) -> BitFunResult<SpeechInputSession> {
        let model_id = request
            .model_id
            .unwrap_or_else(|| LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID.to_string());
        let manifest = get_builtin_speech_model_manifest(&model_id)?;
        if !self.store.has_required_files(&manifest).await {
            return Err(BitFunError::NotFound(
                "Speech model is not installed; download it before starting voice input"
                    .to_string(),
            ));
        }

        let sample_rate = request.sample_rate.unwrap_or(DEFAULT_SPEECH_SAMPLE_RATE);
        if sample_rate == 0 {
            return Err(BitFunError::validation(
                "Sample rate must be greater than zero",
            ));
        }
        let max_recording_seconds = request
            .max_recording_seconds
            .unwrap_or(DEFAULT_MAX_RECORDING_SECONDS);
        if max_recording_seconds == 0 {
            return Err(BitFunError::validation(
                "Recording limit must be greater than zero",
            ));
        }
        let language = request.language.unwrap_or_else(|| "auto".to_string());
        let model_dir = self.store.model_dir(&manifest);
        let recognizer = Arc::clone(&self.recognizer);
        let warmup_language = language.clone();
        let warmup_model_id = model_id.clone();
        let warmup_recognizer = manifest.recognizer;
        tokio::spawn(async move {
            if let Err(error) = recognizer
                .warmup(SpeechRecognizerWarmupRequest {
                    model_dir,
                    recognizer: warmup_recognizer,
                    language: warmup_language,
                })
                .await
            {
                log::warn!(
                    "Failed to warm up speech recognizer: model_id={}, error={}",
                    warmup_model_id,
                    error
                );
            }
        });

        let session_id = Uuid::new_v4().to_string();
        let temp_dir = self.store.paths().input_temp_dir().to_path_buf();
        fs::create_dir_all(&temp_dir).await?;
        let audio_path = temp_dir.join(format!("{session_id}.pcm"));
        fs::File::create(&audio_path).await?;

        let session = SpeechInputSession {
            session_id: session_id.clone(),
            model_id,
            language,
            sample_rate,
            max_recording_seconds,
        };
        self.sessions.lock().await.insert(
            session_id,
            SpeechInputSessionState {
                session: session.clone(),
                audio_path,
                received_bytes: 0,
            },
        );
        Ok(session)
    }

    pub async fn append_audio_chunk(
        &self,
        request: SpeechAppendAudioChunkRequest,
    ) -> BitFunResult<SpeechAppendAudioChunkResponse> {
        let bytes = BASE64_STANDARD
            .decode(request.pcm16_base64.as_bytes())
            .map_err(|e| BitFunError::validation(format!("Invalid base64 audio chunk: {e}")))?;
        if bytes.len() % 2 != 0 {
            return Err(BitFunError::validation(
                "PCM16 audio chunks must contain complete samples",
            ));
        }

        let mut sessions = self.sessions.lock().await;
        let state = sessions
            .get_mut(&request.session_id)
            .ok_or_else(|| BitFunError::NotFound("Speech input session not found".to_string()))?;
        let max_bytes =
            state.session.sample_rate as u64 * state.session.max_recording_seconds as u64 * 2;
        let remaining_bytes = max_bytes.saturating_sub(state.received_bytes);
        let accepted_bytes = bytes.len().min(remaining_bytes as usize) & !1;

        if accepted_bytes > 0 {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&state.audio_path)
                .await?;
            file.write_all(&bytes[..accepted_bytes]).await?;
            state.received_bytes += accepted_bytes as u64;
        }
        Ok(SpeechAppendAudioChunkResponse {
            received_bytes: state.received_bytes,
            received_seconds: audio::pcm16_duration_seconds(
                state.received_bytes,
                state.session.sample_rate,
            ),
            limit_reached: state.received_bytes >= max_bytes,
        })
    }

    pub async fn finish_input_session(
        &self,
        request: SpeechFinishInputSessionRequest,
    ) -> BitFunResult<SpeechTranscriptionResult> {
        let state = self
            .sessions
            .lock()
            .await
            .remove(&request.session_id)
            .ok_or_else(|| BitFunError::NotFound("Speech input session not found".to_string()))?;
        let manifest = get_builtin_speech_model_manifest(&state.session.model_id)?;
        let pcm16_le = fs::read(&state.audio_path).await?;
        let _ = fs::remove_file(&state.audio_path).await;
        if pcm16_le.is_empty() {
            return Err(BitFunError::validation("No speech audio was captured"));
        }

        self.recognizer
            .transcribe(SpeechTranscribeRequest {
                model_dir: self.store.model_dir(&manifest),
                recognizer: manifest.recognizer,
                pcm16_le,
                sample_rate: state.session.sample_rate,
                language: state.session.language,
            })
            .await
    }

    pub async fn cancel_input_session(
        &self,
        request: SpeechCancelInputSessionRequest,
    ) -> BitFunResult<()> {
        if let Some(state) = self.sessions.lock().await.remove(&request.session_id) {
            let _ = fs::remove_file(state.audio_path).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn append_audio_chunk_truncates_at_recording_limit() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-speech-limit-test-{}",
            Uuid::new_v4().simple()
        ));
        let service = SpeechService::new(SpeechStoragePaths::new(
            root.join("models"),
            root.join("downloads"),
            root.join("input"),
        ));
        fs::create_dir_all(&root).await.unwrap();
        let audio_path = root.join("session.pcm");
        fs::File::create(&audio_path).await.unwrap();
        let session_id = "limit-test-session".to_string();
        service.sessions.lock().await.insert(
            session_id.clone(),
            SpeechInputSessionState {
                session: SpeechInputSession {
                    session_id: session_id.clone(),
                    model_id: LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID.to_string(),
                    language: "auto".to_string(),
                    sample_rate: 2,
                    max_recording_seconds: 1,
                },
                audio_path: audio_path.clone(),
                received_bytes: 0,
            },
        );

        let response = service
            .append_audio_chunk(SpeechAppendAudioChunkRequest {
                session_id,
                pcm16_base64: BASE64_STANDARD.encode([1_u8, 2, 3, 4, 5, 6]),
            })
            .await
            .unwrap();

        assert_eq!(response.received_bytes, 4);
        assert_eq!(response.received_seconds, 1.0);
        assert!(response.limit_reached);
        assert_eq!(fs::read(&audio_path).await.unwrap(), vec![1, 2, 3, 4]);

        let _ = fs::remove_dir_all(root).await;
    }
}
