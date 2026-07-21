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
    // Tolerate a stray non-JSON legacy return by surfacing it as an error.
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| {
        format!("open_dialog_file: invalid json response: {e}: {json}")
    })?;

    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Err(error.to_owned());
    }

    let Some(paths) = value.get("paths").and_then(|v| v.as_array()) else {
        return Err(format!(
            "open_dialog_file: unexpected response, missing 'paths': {json}"
        ));
    };

    if paths.is_empty() {
        // user cancelled → null on the frontend
        return Ok(serde_json::Value::Null.to_string());
    }

    let strings: Vec<String> = paths
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_owned()))
        .collect();

    if strings.len() != paths.len() {
        return Err(format!("open_dialog_file: non-string element in paths: {json}"));
    }

    match strings.len() {
        1 => Ok(strings.into_iter().next().unwrap()),
        _ => serde_json::to_string(&strings)
            .map_err(|e| format!("open_dialog_file: failed to encode paths: {e}")),
    }
}
