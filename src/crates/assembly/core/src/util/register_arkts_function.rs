use crate::infrastructure::events::{emit_global_event, BackendEvent};
use lazy_static::lazy_static;
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Promise;
use napi_ohos::threadsafe_function::ThreadsafeFunction;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
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

/// The OHOS-side event name the web-ui listens for to follow live system color
/// mode changes. Defined here so rust and the web-ui reference the same string.
pub const SYSTEM_COLOR_SCHEME_CHANGED_EVENT: &str = "bitfun:system-color-scheme-changed";

/// Event name the embedded browser webview emits to signal page-load lifecycle
/// (started/finished) so the web-ui's `useEmbeddedBrowserWebview` hook can
/// update `isLoading`, the address bar URL, and re-inject the
/// `BLANK_TARGET_INTERCEPT_SCRIPT` + `STREAM_RENDER_OPTIMIZATION_SCRIPT`.
/// Mirrors the desktop `BROWSER_WEBVIEW_PAGE_LOAD_EVENT` constant in
/// `apps/desktop/src/lib.rs` (kept duplicated to avoid a cross-crate visibility
/// tweak for one string).
pub const BROWSER_WEBVIEW_PAGE_LOAD_EVENT: &str = "browser-webview-page-load";

/// Dedicated single-threaded tokio runtime for `notify_system_color_mode`. The
/// `#[napi]` callback runs on a HarmonyOS thread that has no tokio runtime in
/// context, so we cannot rely on `Handle::try_current()` captured at host init
/// (the Tauri `.setup` closure is not guaranteed to run on a tokio-driven thread
/// on OHOS — unlike desktop, which is why the earlier capture-from-setup design
/// silently left this `None` and dropped every color-mode change). Instead we
/// lazily build a runtime here, mirroring the proven pattern in
/// `get_app_config_bool` (`system_api.rs`): `OnceLock<Runtime>` + `get_or_init`.
static SYSTEM_COLOR_MODE_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Lazily creates (on first call) the runtime used to drive
/// `emit_global_event` from the napi callback. Keeping a persistent runtime
/// (rather than building one per call) avoids re-creating the reactor and
/// thread on every system color-mode change.
fn system_color_mode_runtime() -> &'static tokio::runtime::Runtime {
    SYSTEM_COLOR_MODE_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build system color mode runtime")
    })
}

/// Called from ArkTS (`NativeModule.notifySystemColorMode`) when the HarmonyOS
/// system color mode changes (via `EntryAbility.onConfigurationUpdate`) or on a
/// cold-start best-effort initial report. Forwards the color mode
/// (`"light" | "dark"`) to the web-ui through the global event system, which
/// re-resolves the "follow system" theme without polling. Runs the emit to
/// completion on the dedicated runtime (blocking the HarmonyOS callback thread
/// briefly, same as `get_app_config_bool`); `emit_global_event` is a fast
/// channel send, so the blocking is negligible.
#[napi]
pub fn notify_system_color_mode(mode: String) {
    let normalized = match mode.as_str() {
        "light" | "dark" => mode,
        _ => return,
    };
    system_color_mode_runtime().block_on(async move {
        let payload = serde_json::json!({ "scheme": normalized });
        if let Err(error) = emit_global_event(BackendEvent::Custom {
            event_name: SYSTEM_COLOR_SCHEME_CHANGED_EVENT.to_string(),
            payload,
        })
        .await
        {
            log::warn!("Failed to emit system color mode change: {error}");
        }
    });
}

/// Dedicated single-threaded tokio runtime for `emit_browser_page_load`. Same
/// rationale as `system_color_mode_runtime`: the `#[napi]` callback runs on a
/// HarmonyOS thread with no tokio runtime in context, so we lazily build a
/// persistent runtime here to drive `emit_global_event` (a fast channel send,
/// so blocking is negligible) without re-creating the reactor per page-load
/// event.
static BROWSER_PAGE_LOAD_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn browser_page_load_runtime() -> &'static tokio::runtime::Runtime {
    BROWSER_PAGE_LOAD_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build browser page load runtime")
    })
}

/// Called from ArkTS (`NativeModule.emitBrowserPageLoad`) by the embedded
/// browser service when an ArkUI `Web` component fires `onPageBegin` /
/// `onPageEnd`. Forwards the page-load lifecycle event
/// `{ label, event: "started" | "finished", url }` to the web-ui through the
/// global event system; the web-ui's `useEmbeddedBrowserWebview` hook listens
/// on `BROWSER_WEBVIEW_PAGE_LOAD_EVENT` and updates `isLoading`, the address
/// bar URL, and re-injects the page-init scripts. Mirrors the desktop path in
/// `apps/desktop/src/lib.rs` `.on_page_load` handler that emits the same event
/// to the `"main"` webview via `webview.emit_to(...)`.
#[napi]
pub fn emit_browser_page_load(label: String, event: String, url: String) {
    browser_page_load_runtime().block_on(async move {
        let payload = serde_json::json!({
            "label": label,
            "event": event,
            "url": url,
        });
        if let Err(error) = emit_global_event(BackendEvent::Custom {
            event_name: BROWSER_WEBVIEW_PAGE_LOAD_EVENT.to_string(),
            payload,
        })
        .await
        {
            log::warn!("Failed to emit browser page load event: {error}");
        }
    });
}

pub async fn call_arkts_string_function(
    function_name: &str,
    input: String,
) -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get(function_name).cloned()
    };

    let Some(function) = function else {
        return Err(format!("{function_name} has not registered"));
    };

    let promise = function
        .call_async(Ok(input))
        .await
        .map_err(|error| error.to_string())?;
    promise.await.map_err(|error| error.to_string())
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
