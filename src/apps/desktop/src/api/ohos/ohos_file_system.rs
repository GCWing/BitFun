use crate::AppState;
use bitfun_core::util::open_dialog_file;
use log::error;
use tauri::State;

#[tauri::command]
pub async fn open_oh_file_dialog() -> Result<String, String> {
    open_dialog_file().await
}