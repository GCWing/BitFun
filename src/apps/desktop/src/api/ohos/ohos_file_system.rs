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

/// Tell the HarmonyOS shell which color mode the webview should adopt.
///
/// `mode` is one of `"light"`, `"dark"`, or `"system"`:
/// - `light`/`dark` — pin the app to that appearance; the ArkTS side returns `""`.
/// - `system` — release the override (`COLOR_MODE_NOT_SET`) and the ArkTS side
///   returns the real system color mode (`"light"` or `"dark"`) so the web-ui can
///   resolve a concrete theme without relying on `prefers-color-scheme`, which the
///   OHOS webview does not update live. The web-ui also polls this return value to
///   follow live system theme changes.
#[tauri::command]
pub async fn set_theme_mode(mode: String) -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_theme_mode").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    // call_async + promise.await so the ArkTS callback's return value (the system
    // color mode for `system`) reaches the web-ui. Fixed modes return "".
    let promise = function
        .call_async(Ok(mode))
        .await
        .map_err(|e| e.to_string())?;
    let result = promise.await.map_err(|e| e.to_string())?;
    Ok(result)
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