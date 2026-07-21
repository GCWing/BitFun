use bitfun_core::util::{JS_THREADSAFE_FUNCTION, open_dialog_file};
use napi_ohos::threadsafe_function::ThreadsafeFunctionCallMode;

/// Open the HarmonyOS file/folder picker.
///
/// `options` is a JSON string with the shape of `OpenDialogOptions` from
/// `@tauri-apps/plugin-dialog` (a subset): `{ multiple?, directory?, filters? }`.
/// When `None` (frontend did not send the arg), falls back to empty options,
/// i.e. single-select MIXED mode — the legacy behavior. Returns a single string
/// (bare path, `"null"` on cancel, or a JSON array string when multi-select)
/// so the Tauri command signature stays `Result<String, String>`.
#[tauri::command]
pub async fn open_oh_file_dialog(options: Option<String>) -> Result<String, String> {
    let opts = options.as_deref().unwrap_or("");
    open_dialog_file(opts).await
}

#[tauri::command]
pub async fn set_theme_mode(theme: String) -> Result<(), String> {
        let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_theme_mode").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    function.call(Ok(theme),ThreadsafeFunctionCallMode::NonBlocking);
    Ok(())
}

#[tauri::command]
pub fn reveal_in_oh_explorer(path: String)  -> Result<(), String> {
            let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("reveal_in_explorer").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    function.call(Ok(path),ThreadsafeFunctionCallMode::NonBlocking);
    Ok(())
}