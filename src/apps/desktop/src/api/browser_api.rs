//! Browser API — commands for the embedded browser feature.

use serde::Deserialize;
use tauri::Manager;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewEvalRequest {
    pub label: String,
    pub script: String,
}

#[tauri::command]
pub async fn browser_webview_eval(
    app: tauri::AppHandle,
    request: WebviewEvalRequest,
) -> Result<(), String> {
    let webview = app
        .get_webview(&request.label)
        .ok_or_else(|| format!("Webview not found: {}", request.label))?;

    webview
        .eval(&request.script)
        .map_err(|e| format!("eval failed: {e}"))
}

// #region agent log
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewLabelRequest {
    pub label: String,
}

/// Pull debug logs from the injected inspector script.
/// The script stores logs in `window.__bitfun_debug_logs`.
/// We eval a script that writes them into URL hash, then read the URL.
#[tauri::command]
pub async fn browser_pull_debug_logs(
    app: tauri::AppHandle,
    request: WebviewLabelRequest,
) -> Result<String, String> {
    let webview = app
        .get_webview(&request.label)
        .ok_or_else(|| format!("Webview not found: {}", request.label))?;

    webview
        .eval(
            r#"(function(){
                var logs = window.__bitfun_debug_logs || [];
                window.__bitfun_debug_logs = [];
                try {
                    var hash = '__BFDEBUG__' + encodeURIComponent(JSON.stringify(logs));
                    history.replaceState(null, '', '#' + hash);
                } catch(e) {
                    history.replaceState(null, '', '#__BFDEBUG__ERR_' + encodeURIComponent(String(e)));
                }
            })()"#,
        )
        .map_err(|e| format!("eval failed: {e}"))?;

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let url = webview.url().map_err(|e| format!("url failed: {e}"))?;
    let fragment = url.fragment().unwrap_or("");

    if fragment.starts_with("__BFDEBUG__") {
        let encoded = &fragment[11..];
        let decoded = urlencoding::decode(encoded)
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| format!("decode_error:{}", encoded));

        webview
            .eval("try { history.replaceState(null, '', location.pathname + location.search); } catch(e) {}")
            .ok();

        Ok(decoded)
    } else {
        Ok(format!("no_debug_marker_in_fragment:{}", &fragment.chars().take(100).collect::<String>()))
    }
}
// #endregion
