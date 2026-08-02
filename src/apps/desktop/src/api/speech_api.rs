//! Desktop adapter for local speech input.
//!
//! On the `ohos` target the local sherpa-onnx recognizer is unsupported and
//! the HarmonyOS system `speechRecognizer` is used instead — it ships its own
//! on-device model, so BitFun does not download or manage any speech model on
//! HarmonyOS. To keep the ohos build artifact free of the local model path
//! entirely, every command is compiled as one of two mutually exclusive
//! versions via `#[cfg(target_env = "ohos")]`:
//!
//! - ohos version: the four input-session commands route through
//!   [`ohos_speech_call`] to the ArkTS-registered `ohos_speech_*` bridge
//!   functions (see
//!   `src/apps/ohos/entry/src/main/ets/services/VoiceInputService.ets`);
//!   `list_models` reports a single installed system-recognizer stub so the
//!   existing frontend gating passes; download/delete/verify reject.
//! - non-ohos version: unchanged — drives `state.speech_service` (sherpa-onnx).

use bitfun_core_types::speech::{
    SpeechAppendAudioChunkRequest, SpeechAppendAudioChunkResponse, SpeechCancelInputSessionRequest,
    SpeechCancelModelDownloadRequest, SpeechDeleteModelRequest, SpeechDownloadModelRequest,
    SpeechFinishInputSessionRequest, SpeechInputSession, SpeechListModelsResponse,
    SpeechModelStatus, SpeechStartInputSessionRequest, SpeechTranscriptionResult,
    SpeechVerifyModelRequest,
};

#[cfg(target_env = "ohos")]
use bitfun_core::util::ohos_speech_call;
#[cfg(target_env = "ohos")]
use bitfun_core_types::speech::SpeechModelInstallState;

#[cfg(not(target_env = "ohos"))]
use crate::api::AppState;
#[cfg(not(target_env = "ohos"))]
use bitfun_core_types::speech::SpeechModelProgressEvent;
#[cfg(not(target_env = "ohos"))]
use bitfun_events::{SPEECH_MODEL_PROGRESS_EVENT, SPEECH_MODEL_STATUS_CHANGED_EVENT};
#[cfg(not(target_env = "ohos"))]
use tauri::{AppHandle, Emitter, State};

// ----------------------------------------------------------------------------
// speech_list_models
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_list_models() -> Result<SpeechListModelsResponse, String> {
    Ok(SpeechListModelsResponse {
        models: vec![ohos_system_model_status()],
    })
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_list_models(
    state: State<'_, AppState>,
) -> Result<SpeechListModelsResponse, String> {
    state
        .speech_service
        .list_models()
        .await
        .map_err(|error| format!("Failed to list speech models: {error}"))
}

// ----------------------------------------------------------------------------
// speech_download_model
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_download_model(
    request: SpeechDownloadModelRequest,
) -> Result<SpeechModelStatus, String> {
    Err(format!(
        "Speech model download is not supported on HarmonyOS; the system speechRecognizer ships its own on-device model (model_id={}).",
        request.model_id
    ))
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_download_model(
    state: State<'_, AppState>,
    app: AppHandle,
    request: SpeechDownloadModelRequest,
) -> Result<SpeechModelStatus, String> {
    let progress_app = app.clone();
    let status = state
        .speech_service
        .download_model(request, move |event: SpeechModelProgressEvent| {
            if let Err(error) = progress_app.emit(SPEECH_MODEL_PROGRESS_EVENT, &event) {
                log::warn!("Failed to emit speech model progress event: {error}");
            }
        })
        .await
        .map_err(|error| format!("Failed to download speech model: {error}"))?;
    emit_status(&app, &status);
    Ok(status)
}

// ----------------------------------------------------------------------------
// speech_cancel_model_download
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_cancel_model_download(
    request: SpeechCancelModelDownloadRequest,
) -> Result<SpeechModelStatus, String> {
    Err(format!(
        "Speech model download is not supported on HarmonyOS; nothing to cancel (model_id={}).",
        request.model_id
    ))
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_cancel_model_download(
    state: State<'_, AppState>,
    app: AppHandle,
    request: SpeechCancelModelDownloadRequest,
) -> Result<SpeechModelStatus, String> {
    let status = state
        .speech_service
        .cancel_model_download(request)
        .await
        .map_err(|error| format!("Failed to cancel speech model download: {error}"))?;
    emit_status(&app, &status);
    Ok(status)
}

// ----------------------------------------------------------------------------
// speech_delete_model
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_delete_model(
    request: SpeechDeleteModelRequest,
) -> Result<SpeechModelStatus, String> {
    Err(format!(
        "Speech model management is not supported on HarmonyOS; the system speechRecognizer ships its own on-device model (model_id={}).",
        request.model_id
    ))
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_delete_model(
    state: State<'_, AppState>,
    app: AppHandle,
    request: SpeechDeleteModelRequest,
) -> Result<SpeechModelStatus, String> {
    let status = state
        .speech_service
        .delete_model(request)
        .await
        .map_err(|error| format!("Failed to delete speech model: {error}"))?;
    emit_status(&app, &status);
    Ok(status)
}

// ----------------------------------------------------------------------------
// speech_verify_model
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_verify_model(
    request: SpeechVerifyModelRequest,
) -> Result<SpeechModelStatus, String> {
    Err(format!(
        "Speech model verification is not supported on HarmonyOS; the system speechRecognizer ships its own on-device model (model_id={}).",
        request.model_id
    ))
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_verify_model(
    state: State<'_, AppState>,
    app: AppHandle,
    request: SpeechVerifyModelRequest,
) -> Result<SpeechModelStatus, String> {
    let status = state
        .speech_service
        .verify_model(request)
        .await
        .map_err(|error| format!("Failed to verify speech model: {error}"))?;
    emit_status(&app, &status);
    Ok(status)
}

// ----------------------------------------------------------------------------
// speech_start_input_session
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_start_input_session(
    request: SpeechStartInputSessionRequest,
) -> Result<SpeechInputSession, String> {
    log::info!("[speech] start_input_session ohos branch, request={:?}", request);
    let payload = serde_json::to_string(&request)
        .map_err(|error| {
            log::error!("[speech] failed to encode start request: {}", error);
            format!("Failed to encode speech start request: {error}")
        })?;
    let response = match ohos_speech_call("ohos_speech_start", &payload).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("[speech] ohos_speech_start returned error: {}", e);
            return Err(format!("Failed to start speech input session: {e}"));
        }
    };
    log::info!("[speech] ohos_speech_start response: {}", response);
    let value: serde_json::Value = serde_json::from_str(&response)
        .map_err(|error| {
            log::error!("[speech] failed to parse start response json: {} | response={}", error, response);
            format!("Invalid speech start response from ArkTS: {error}: {response}")
        })?;
    if let Some(err_msg) = value.get("__error").and_then(|v| v.as_str()) {
        log::error!("[speech] ArkTS speech start failed: {}", err_msg);
        return Err(format!("ArkTS speech start failed: {err_msg}"));
    }
    serde_json::from_value::<SpeechInputSession>(value)
        .map_err(|error| {
            log::error!("[speech] failed to parse start session: {}", error);
            format!("Invalid speech start session from ArkTS: {error}")
        })
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_start_input_session(
    state: State<'_, AppState>,
    request: SpeechStartInputSessionRequest,
) -> Result<SpeechInputSession, String> {
    state
        .speech_service
        .start_input_session(request)
        .await
        .map_err(|error| format!("Failed to start speech input session: {error}"))
}

// ----------------------------------------------------------------------------
// speech_append_audio_chunk
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_append_audio_chunk(
    request: SpeechAppendAudioChunkRequest,
) -> Result<SpeechAppendAudioChunkResponse, String> {
    let payload = serde_json::to_string(&request)
        .map_err(|error| format!("Failed to encode speech append request: {error}"))?;
    let response = ohos_speech_call("ohos_speech_append", &payload).await?;
    serde_json::from_str::<SpeechAppendAudioChunkResponse>(&response)
        .map_err(|error| format!("Invalid speech append response from ArkTS: {error}: {response}"))
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_append_audio_chunk(
    state: State<'_, AppState>,
    request: SpeechAppendAudioChunkRequest,
) -> Result<SpeechAppendAudioChunkResponse, String> {
    state
        .speech_service
        .append_audio_chunk(request)
        .await
        .map_err(|error| format!("Failed to append speech audio chunk: {error}"))
}

// ----------------------------------------------------------------------------
// speech_finish_input_session
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_finish_input_session(
    request: SpeechFinishInputSessionRequest,
) -> Result<SpeechTranscriptionResult, String> {
    let payload = serde_json::to_string(&request)
        .map_err(|error| format!("Failed to encode speech finish request: {error}"))?;
    let response = ohos_speech_call("ohos_speech_finish", &payload).await?;
    serde_json::from_str::<SpeechTranscriptionResult>(&response)
        .map_err(|error| format!("Invalid speech transcription response from ArkTS: {error}: {response}"))
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_finish_input_session(
    state: State<'_, AppState>,
    request: SpeechFinishInputSessionRequest,
) -> Result<SpeechTranscriptionResult, String> {
    state
        .speech_service
        .finish_input_session(request)
        .await
        .map_err(|error| format!("Failed to transcribe speech input: {error}"))
}

// ----------------------------------------------------------------------------
// speech_cancel_input_session
// ----------------------------------------------------------------------------

#[cfg(target_env = "ohos")]
#[tauri::command]
pub async fn speech_cancel_input_session(
    request: SpeechCancelInputSessionRequest,
) -> Result<(), String> {
    let payload = serde_json::to_string(&request)
        .map_err(|error| format!("Failed to encode speech cancel request: {error}"))?;
    let _ = ohos_speech_call("ohos_speech_cancel", &payload).await?;
    Ok(())
}

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub async fn speech_cancel_input_session(
    state: State<'_, AppState>,
    request: SpeechCancelInputSessionRequest,
) -> Result<(), String> {
    state
        .speech_service
        .cancel_input_session(request)
        .await
        .map_err(|error| format!("Failed to cancel speech input session: {error}"))
}

// ----------------------------------------------------------------------------
// helpers (cfg-gated to the version that uses them)
// ----------------------------------------------------------------------------

#[cfg(not(target_env = "ohos"))]
fn emit_status(app: &AppHandle, status: &SpeechModelStatus) {
    if let Err(error) = app.emit(SPEECH_MODEL_STATUS_CHANGED_EVENT, status) {
        log::warn!("Failed to emit speech model status event: {error}");
    }
}

/// Synthetic `Installed` model status returned by `speech_list_models` on the
/// `ohos` target.
///
/// The frontend (`useComposerVoiceInput`) gates recording on
/// `modelInstalled === true` for the configured local model id. On HarmonyOS
/// there is no downloadable model — the system `speechRecognizer` ships its
/// own — so we report the default local model id (`sensevoice-small-int8`) as
/// installed to keep the existing frontend gating logic passing without any
/// frontend change. Downloads/deletes/verifies are rejected by the other
/// ohos commands, so no model file is ever touched.
#[cfg(target_env = "ohos")]
fn ohos_system_model_status() -> SpeechModelStatus {
    SpeechModelStatus {
        model_id: "sensevoice-small-int8".to_string(),
        display_name: "HarmonyOS System Speech Recognition".to_string(),
        provider: "ohos-system".to_string(),
        version: "1".to_string(),
        description: "On-device speech recognition provided by the HarmonyOS system speechRecognizer.".to_string(),
        languages: vec!["zh-CN".to_string(), "en-US".to_string()],
        state: SpeechModelInstallState::Installed,
        installed_path: None,
        installed_bytes: 0,
        expected_bytes: 0,
        progress: None,
        error: None,
    }
}

