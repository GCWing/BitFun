//! Desktop adapter for local speech input.

use crate::api::AppState;
use bitfun_core_types::speech::{
    SpeechAppendAudioChunkRequest, SpeechAppendAudioChunkResponse,
    SpeechAppendRealtimeAudioRequest, SpeechCancelInputSessionRequest,
    SpeechCancelModelDownloadRequest, SpeechDeleteModelRequest, SpeechDownloadModelRequest,
    SpeechFinishInputSessionRequest, SpeechGetRealtimeConfigRequest, SpeechInputSession,
    SpeechListModelsResponse, SpeechModelProgressEvent, SpeechModelStatus, SpeechRealtimeConfig,
    SpeechRealtimeEvent, SpeechRealtimeSession, SpeechRealtimeSessionRequest,
    SpeechRealtimeSpeakRequest, SpeechRealtimeToolResultRequest, SpeechSaveRealtimeConfigRequest,
    SpeechStartInputSessionRequest, SpeechStartRealtimeSessionRequest, SpeechTranscriptionResult,
    SpeechVerifyModelRequest,
};
use bitfun_events::{
    SPEECH_MODEL_PROGRESS_EVENT, SPEECH_MODEL_STATUS_CHANGED_EVENT, SPEECH_REALTIME_EVENT,
};
use bitfun_services_integrations::speech::VolcengineRealtimeSpeechConfig;
use tauri::{AppHandle, Emitter, State};

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

#[tauri::command]
pub async fn speech_start_realtime_session(
    state: State<'_, AppState>,
    app: AppHandle,
    request: SpeechStartRealtimeSessionRequest,
) -> Result<SpeechRealtimeSession, String> {
    let global_config: bitfun_core::service::config::GlobalConfig = state
        .config_service
        .get_config(None)
        .await
        .map_err(|error| format!("Failed to load realtime voice configuration: {error}"))?;
    let voice_call = global_config.app.voice_call;
    if !voice_call.enabled {
        return Err("Realtime voice calling is disabled in settings".to_string());
    }
    if voice_call.provider != "volcengine" {
        return Err(format!(
            "Unsupported realtime voice provider: {}",
            voice_call.provider
        ));
    }

    let event_app = app.clone();
    state
        .speech_service
        .start_realtime_session(
            VolcengineRealtimeSpeechConfig {
                api_key: voice_call.api_key,
                voice: voice_call.voice,
                speed: voice_call.speed,
                loudness: voice_call.loudness,
                client_context: request.client_context,
            },
            move |event: SpeechRealtimeEvent| {
                if let Err(error) = event_app.emit(SPEECH_REALTIME_EVENT, &event) {
                    log::warn!("Failed to emit realtime speech event: {error}");
                }
            },
        )
        .await
        .map_err(|error| format!("Failed to start realtime speech session: {error}"))
}

#[tauri::command]
pub async fn speech_append_realtime_audio(
    state: State<'_, AppState>,
    request: SpeechAppendRealtimeAudioRequest,
) -> Result<(), String> {
    state
        .speech_service
        .append_realtime_audio(request)
        .await
        .map_err(|error| format!("Failed to append realtime speech audio: {error}"))
}

#[tauri::command]
pub async fn speech_commit_realtime_audio(
    state: State<'_, AppState>,
    request: SpeechRealtimeSessionRequest,
) -> Result<(), String> {
    state
        .speech_service
        .commit_realtime_audio(request)
        .await
        .map_err(|error| format!("Failed to commit realtime speech audio: {error}"))
}

#[tauri::command]
pub async fn speech_send_realtime_tool_result(
    state: State<'_, AppState>,
    request: SpeechRealtimeToolResultRequest,
) -> Result<(), String> {
    state
        .speech_service
        .send_realtime_tool_result(request)
        .await
        .map_err(|error| format!("Failed to send realtime speech tool result: {error}"))
}

#[tauri::command]
pub async fn speech_speak_realtime_text(
    state: State<'_, AppState>,
    request: SpeechRealtimeSpeakRequest,
) -> Result<(), String> {
    state
        .speech_service
        .speak_realtime_text(request)
        .await
        .map_err(|error| format!("Failed to speak realtime progress: {error}"))
}

#[tauri::command]
pub async fn speech_cancel_realtime_response(
    state: State<'_, AppState>,
    request: SpeechRealtimeSessionRequest,
) -> Result<(), String> {
    state
        .speech_service
        .cancel_realtime_response(request)
        .await
        .map_err(|error| format!("Failed to cancel realtime speech response: {error}"))
}

#[tauri::command]
pub async fn speech_close_realtime_session(
    state: State<'_, AppState>,
    request: SpeechRealtimeSessionRequest,
) -> Result<(), String> {
    state
        .speech_service
        .close_realtime_session(request)
        .await
        .map_err(|error| format!("Failed to close realtime speech session: {error}"))
}

#[tauri::command]
pub async fn speech_get_realtime_config(
    state: State<'_, AppState>,
    request: SpeechGetRealtimeConfigRequest,
) -> Result<SpeechRealtimeConfig, String> {
    let _ = request;
    let global_config: bitfun_core::service::config::GlobalConfig = state
        .config_service
        .get_config(None)
        .await
        .map_err(|error| format!("Failed to load controller realtime voice settings: {error}"))?;
    let config = global_config.app.voice_call;
    Ok(SpeechRealtimeConfig {
        enabled: config.enabled,
        provider: config.provider,
        api_key: config.api_key,
        voice: config.voice,
        speed: config.speed,
        loudness: config.loudness,
        microphone_device_id: config.microphone_device_id,
    })
}

#[tauri::command]
pub async fn speech_save_realtime_config(
    state: State<'_, AppState>,
    request: SpeechSaveRealtimeConfigRequest,
) -> Result<SpeechRealtimeConfig, String> {
    let api_key = request.api_key.trim().to_string();
    let voice = request.voice.trim().to_string();
    if request.enabled && api_key.is_empty() {
        return Err(
            "Volcengine realtime speech API key is required when voice calling is enabled"
                .to_string(),
        );
    }
    if voice.is_empty() {
        return Err("Volcengine realtime speech voice cannot be empty".to_string());
    }
    if !(-50..=100).contains(&request.speed) || !(-50..=100).contains(&request.loudness) {
        return Err("Realtime speech speed and loudness must be between -50 and 100".to_string());
    }
    let config = bitfun_core::service::config::VoiceCallConfig {
        enabled: request.enabled,
        provider: "volcengine".to_string(),
        api_key,
        voice,
        speed: request.speed,
        loudness: request.loudness,
        microphone_device_id: request.microphone_device_id,
    };
    state
        .config_service
        .set_config("app.voice_call", &config)
        .await
        .map_err(|error| format!("Failed to save controller realtime voice settings: {error}"))?;
    crate::api::remote_connect_api::notify_settings_changed();

    Ok(SpeechRealtimeConfig {
        enabled: config.enabled,
        provider: config.provider,
        api_key: config.api_key,
        voice: config.voice,
        speed: config.speed,
        loudness: config.loudness,
        microphone_device_id: config.microphone_device_id,
    })
}

fn emit_status(app: &AppHandle, status: &SpeechModelStatus) {
    if let Err(error) = app.emit(SPEECH_MODEL_STATUS_CHANGED_EVENT, status) {
        log::warn!("Failed to emit speech model status event: {error}");
    }
}
