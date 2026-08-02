use lazy_static::lazy_static;
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Promise;
use napi_ohos::threadsafe_function::ThreadsafeFunction;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
lazy_static! {
    pub static ref JS_THREADSAFE_FUNCTION: RwLock<HashMap<String, Arc<ThreadsafeFunction<String, Promise<String>>>>> =
        Default::default();
}
#[napi]
pub fn register_arkts_function(
    function_name: String,
    callback: ThreadsafeFunction<String, Promise<String>>,
) {
    JS_THREADSAFE_FUNCTION
        .write()
        .insert(function_name, Arc::new(callback));
}

pub async fn open_dialog_file(options: &str) -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("open_dialog_file").cloned()
    };

    let Some(function) = function else {
        return Err("open_dialog_file has not register".to_owned());
    };

    // 3. 调用 JS 函数
    // ThreadsafeFunction 本身是 Send 的，可以安全地在异地任务中使用
    let res = function.call_async(Ok(options.to_string())).await;
    match res {
        Ok(promise) => match promise.await {
            Ok(json) => parse_picker_result(&json),
            Err(err) => Err(err.to_string()),
        },

        Err(err) => Err(err.to_string()),
    }
}

/// Read file paths from the OHOS system pasteboard via the ArkTS bridge.
///
/// The ArkTS side (`CommonUtils.read_clipboard_files`) reads
/// `@ohos.pasteboard`, extracts file URIs from each record, applies the same
/// path normalization as `open_file_dialog`, and returns the same JSON
/// envelope (`{ "paths": [...] }` / `{ "paths": [] }` / `{ "error": "..." }`)
/// so we can reuse [`parse_paths_result`] to decode it into a `Vec<String>`.
/// An empty `paths` array (clipboard has no files) maps to an empty `Vec`,
/// which the frontend surfaces as "no files in clipboard".
pub async fn get_clipboard_files() -> Result<Vec<String>, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("get_clipboard_files").cloned()
    };

    let Some(function) = function else {
        return Err("get_clipboard_files has not register".to_owned());
    };

    let res = function.call_async(Ok(String::new())).await;
    match res {
        Ok(promise) => match promise.await {
            Ok(json) => parse_paths_result(&json),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

/// Decode the JSON envelope returned by the ArkTS picker / clipboard bridge.
///
/// Envelope shapes (shared by `CommonUtils.open_file_dialog` and
/// `CommonUtils.read_clipboard_files`):
/// - `{ "paths": [...] }` — success; array of normalized file paths.
/// - `{ "paths": [] }` — empty (user cancelled picker, or clipboard has no files).
/// - `{ "error": "..." }` — failure.
///
/// `prefix` is used in error messages to identify the originating call.
fn parse_paths_envelope(json: &str, prefix: &str) -> Result<Vec<String>, String> {
    // Tolerate a stray non-JSON legacy return by surfacing it as an error.
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("{prefix}: invalid json response: {e}: {json}"))?;

    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Err(error.to_owned());
    }

    let Some(paths) = value.get("paths").and_then(|v| v.as_array()) else {
        return Err(format!("{prefix}: unexpected response, missing 'paths': {json}"));
    };

    let strings: Vec<String> = paths
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_owned()))
        .collect();

    if strings.len() != paths.len() {
        return Err(format!("{prefix}: non-string element in paths: {json}"));
    }

    Ok(strings)
}

/// Decode the JSON envelope returned by `CommonUtils.open_file_dialog`.
///
/// Envelope shapes:
/// - `{ "paths": [...] }` — success; non-empty array of normalized file paths.
/// - `{ "paths": [] }` — user cancelled.
/// - `{ "error": "..." }` — picker failure.
///
/// Returns a single `String` so the Tauri command signature stays
/// `Result<String, String>` (unchanged contract). The frontend `WorkspaceAPI`
/// wrapper interprets it:
/// - empty → `"null"` (frontend turns into `null`)
/// - one path → the bare path string
/// - many paths → a JSON array string like `["a","b"]`
fn parse_picker_result(json: &str) -> Result<String, String> {
    let strings = parse_paths_envelope(json, "open_dialog_file")?;
    match strings.len() {
        0 => Ok(serde_json::Value::Null.to_string()),
        1 => Ok(strings.into_iter().next().unwrap()),
        _ => serde_json::to_string(&strings)
            .map_err(|e| format!("open_dialog_file: failed to encode paths: {e}")),
    }
}

/// Decode the JSON envelope returned by `CommonUtils.read_clipboard_files`.
///
/// Same envelope as the picker, but here the result is consumed as a
/// `Vec<String>` directly — an empty array (no files on the clipboard) is a
/// valid result, not cancellation.
fn parse_paths_result(json: &str) -> Result<Vec<String>, String> {
    parse_paths_envelope(json, "get_clipboard_files")
}

/// Invoke an ArkTS-registered speech bridge function by name.
///
/// Mirrors [`open_dialog_file`]: looks up the `ThreadsafeFunction` registered
/// on the ArkTS side via `RustModule.registerArktsFunction(name, ...)`, calls
/// it with a JSON string argument, and awaits the returned `Promise<String>`.
/// Used by the ohos branches of the `speech_*` Tauri commands to route voice
/// input to the HarmonyOS system `speechRecognizer` (via
/// `src/apps/ohos/.../services/VoiceInputService.ets`) instead of the local
/// sherpa-onnx recognizer, which is unsupported on the ohos target.
pub async fn ohos_speech_call(name: &str, json: &str) -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get(name).cloned()
    };

    let Some(function) = function else {
        log::error!("[ohos_speech] {} not registered by ArkTS", name);
        return Err(format!("{name} has not been registered by ArkTS"));
    };

    let res = function.call_async(Ok(json.to_string())).await;
    match res {
        Ok(promise) => match promise.await {
            Ok(json) => Ok(json),
            Err(err) => {
                log::error!("[ohos_speech] {} promise rejected: {} | {:?}", name, err.to_string(), err);
                Err(err.to_string())
            }
        },
        Err(err) => {
            log::error!("[ohos_speech] {} call_async failed: {} | {:?}", name, err.to_string(), err);
            Err(err.to_string())
        }
    }
}
