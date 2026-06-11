//! Voice API

use crate::api::app_state::AppState;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VoiceStatus {
    pub is_listening: bool,
    pub stt_provider: String,
    pub tts_provider: String,
}

#[tauri::command]
pub async fn voice_get_status(
    _state: State<'_, AppState>,
) -> Result<VoiceStatus, String> {
    Ok(VoiceStatus {
        is_listening: false,
        stt_provider: "webspeech".to_string(),
        tts_provider: "edge".to_string(),
    })
}

#[tauri::command]
pub async fn voice_start_listening(
    _state: State<'_, AppState>,
) -> Result<(), String> {
    Err("WebSpeech provider is controlled by frontend".to_string())
}

#[tauri::command]
pub async fn voice_stop_listening(
    _state: State<'_, AppState>,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn voice_speak(
    _state: State<'_, AppState>,
    _text: String,
) -> Result<(), String> {
    Err("TTS provider is controlled by frontend".to_string())
}

#[tauri::command]
pub async fn voice_stop_speaking(
    _state: State<'_, AppState>,
) -> Result<(), String> {
    Ok(())
}