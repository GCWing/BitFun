//! Browser API — commands for the embedded browser feature.
//!
//! Browser webviews are created as native child webviews by this desktop
//! adapter so stream-specific initialization can run before page scripts.

use bitfun_core::agentic::tools::browser_control::BuiltInBrowserTarget;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

const VIDEO_DECODER_MODE_ENV: &str = "BITFUN_BROWSER_VIDEO_DECODER_MODE";

fn video_decoder_compatibility_script() -> String {
    let mode =
        std::env::var(VIDEO_DECODER_MODE_ENV).unwrap_or_else(|_| "prefer-software".to_string());
    let mode = match mode.as_str() {
        "prefer-hardware" | "prefer-software" => mode,
        _ => String::new(),
    };
    let mode_json = serde_json::to_string(&mode).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        r#"
const isWebView2 = Boolean(window.chrome && window.chrome.webview);
const isBitFunDocument = location.protocol === 'tauri:'
  || location.hostname === 'tauri.localhost'
  || (location.hostname === 'localhost' && location.port === '1422');
if (isWebView2 && !isBitFunDocument) {{
  const decoderMode = {mode_json};
  if (decoderMode && typeof VideoDecoder === 'function') {{
    const originalConfigure = VideoDecoder.prototype.configure;
    VideoDecoder.prototype.configure = function(config) {{
      const codec = typeof config?.codec === 'string' ? config.codec : '';
      const isH264 = /^avc[13]\./i.test(codec);
      if (isH264 && !config.hardwareAcceleration) {{
        return originalConfigure.call(this, {{ ...config, hardwareAcceleration: decoderMode }});
      }}
      return originalConfigure.call(this, config);
    }};

    // #region agent log
    if (location.hostname === '127.0.0.1' && location.port === '41953') {{
      void fetch('http://127.0.0.1:7469/log', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{
          hypothesis: 'D',
          location: 'browser_api.video_decoder_init',
          message: 'video decoder mode installed',
          data: {{ decoderMode }},
          timestamp: new Date().toISOString()
        }})
      }}).catch(() => {{}});
    }}
    // #endregion
  }}
}}
"#
    );

    script
}

fn find_browser_webview(app: &tauri::AppHandle, label: &str) -> Result<tauri::Webview, String> {
    app.get_webview(label)
        .ok_or_else(|| format!("Webview not found: {label}"))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewEvalRequest {
    pub label: String,
    pub script: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewNavigateRequest {
    pub label: String,
    pub url: String,
    #[serde(default)]
    pub open_request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewBoundsRequest {
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewCreateRequest {
    pub label: String,
    pub url: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub open_request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAgentTargetStateRequest {
    pub label: String,
    pub active: bool,
    #[serde(default)]
    pub open_request_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct BrowserTargetRecord {
    active: bool,
    last_active_seq: u64,
    url: String,
    title: Option<String>,
    open_request_id: Option<String>,
}

#[derive(Default)]
struct BrowserTargetRegistry {
    next_seq: u64,
    records: HashMap<String, BrowserTargetRecord>,
}

impl BrowserTargetRegistry {
    fn active_label_for_open_request(&self, request_id: &str) -> Option<String> {
        self.records.iter().find_map(|(label, record)| {
            (record.active && record.open_request_id.as_deref() == Some(request_id))
                .then(|| label.clone())
        })
    }
}

static BROWSER_TARGETS: OnceLock<Mutex<BrowserTargetRegistry>> = OnceLock::new();

fn lock_browser_targets() -> std::sync::MutexGuard<'static, BrowserTargetRegistry> {
    BROWSER_TARGETS
        .get_or_init(|| Mutex::new(BrowserTargetRegistry::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn validate_browser_label(label: &str) -> Result<(), String> {
    if label.starts_with("embedded-browser-view-")
        || label.starts_with("embedded-browser-panel-view-")
    {
        Ok(())
    } else {
        Err("invalid browser webview label".to_string())
    }
}

fn register_browser_target(label: &str, url: &str, open_request_id: Option<String>) {
    lock_browser_targets().records.insert(
        label.to_string(),
        BrowserTargetRecord {
            url: url.to_string(),
            open_request_id,
            ..BrowserTargetRecord::default()
        },
    );
}

fn associate_browser_open_request(label: &str, open_request_id: Option<&str>) {
    let Some(open_request_id) = open_request_id
        .map(str::trim)
        .filter(|request_id| !request_id.is_empty())
    else {
        return;
    };
    if let Some(record) = lock_browser_targets().records.get_mut(label) {
        record.open_request_id = Some(open_request_id.to_string());
    }
}

pub(crate) fn update_browser_target_url(label: &str, url: &str) {
    if let Some(record) = lock_browser_targets().records.get_mut(label) {
        record.url = url.to_string();
        // A navigation invalidates the cached document title. The async
        // automation host refreshes it from the page before target discovery
        // is returned to ControlHub.
        record.title = None;
    }
}

pub(crate) fn update_browser_target_metadata(label: &str, url: &str, title: &str) {
    if let Some(record) = lock_browser_targets().records.get_mut(label) {
        record.url = url.to_string();
        record.title = Some(title.to_string());
    }
}

pub(crate) fn unregister_browser_target(label: &str) {
    lock_browser_targets().records.remove(label);
}

pub(crate) fn list_browser_targets(app: &tauri::AppHandle) -> Vec<BuiltInBrowserTarget> {
    let mut registry = lock_browser_targets();
    registry
        .records
        .retain(|label, _| app.get_webview(label).is_some());
    let mut targets = registry
        .records
        .iter()
        .map(|(label, record)| {
            let url = app
                .get_webview(label)
                .and_then(|webview| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| webview.url()))
                        .ok()
                        .and_then(Result::ok)
                })
                .map(|url| url.to_string())
                .filter(|url| !url.is_empty())
                .unwrap_or_else(|| record.url.clone());
            (
                record.last_active_seq,
                BuiltInBrowserTarget {
                    id: label.clone(),
                    url,
                    title: record.title.clone().unwrap_or_default(),
                    active: record.active,
                },
            )
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| right.0.cmp(&left.0));
    targets.into_iter().map(|(_, target)| target).collect()
}

/// Resolve the exact active WebView that acknowledged one browser-open
/// request. The request ID is presentation lifecycle metadata and is kept out
/// of the public browser target DTO.
pub(crate) fn browser_target_for_open_request(
    app: &tauri::AppHandle,
    request_id: &str,
) -> Option<BuiltInBrowserTarget> {
    let label = {
        let registry = lock_browser_targets();
        registry.active_label_for_open_request(request_id)
    }?;
    list_browser_targets(app)
        .into_iter()
        .find(|target| target.id == label && target.active)
}

fn validate_webview_bounds(x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width <= 1.0
        || height <= 1.0
    {
        Err("invalid webview bounds".to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn browser_webview_create(
    app: tauri::AppHandle,
    request: WebviewCreateRequest,
) -> Result<(), String> {
    validate_browser_label(&request.label)?;
    validate_webview_bounds(request.x, request.y, request.width, request.height)?;

    let url = request
        .url
        .parse::<tauri::Url>()
        .map_err(|e| format!("invalid url: {e}"))?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => return Err(format!("unsupported protocol: {scheme}")),
    }

    let window = app
        .get_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let mut builder =
        tauri::webview::WebviewBuilder::new(request.label, tauri::WebviewUrl::External(url))
            .initialization_script(video_decoder_compatibility_script())
            .transparent(false)
            .background_color(tauri::window::Color(0, 0, 0, 255));

    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        builder = builder.devtools(true);
    }

    let webview = window
        .add_child(
            builder,
            tauri::LogicalPosition::new(request.x, request.y),
            tauri::LogicalSize::new(request.width, request.height),
        )
        .map_err(|e| format!("failed to create browser webview: {e}"))?;

    webview
        .hide()
        .map_err(|e| format!("failed to hide browser webview before positioning: {e}"))?;
    let target_url = webview.url().map(|url| url.to_string()).unwrap_or_default();
    register_browser_target(webview.label(), &target_url, request.open_request_id);
    Ok(())
}

/// Advertise which built-in browser surface the Agent should target. This is
/// lifecycle metadata only; browser actions remain in the shared Rust action
/// layer and native WebView adapter.
#[tauri::command]
pub async fn browser_webview_set_agent_target_state(
    request: BrowserAgentTargetStateRequest,
) -> Result<(), String> {
    validate_browser_label(&request.label)?;
    let mut registry = lock_browser_targets();
    if request.active {
        registry.next_seq = registry.next_seq.saturating_add(1);
        let next_seq = registry.next_seq;
        for record in registry.records.values_mut() {
            record.active = false;
        }
        let record = registry.records.entry(request.label).or_default();
        record.active = true;
        record.last_active_seq = next_seq;
        if let Some(request_id) = request
            .open_request_id
            .as_deref()
            .map(str::trim)
            .filter(|request_id| !request_id.is_empty())
        {
            record.open_request_id = Some(request_id.to_string());
        }
    } else if let Some(record) = registry.records.get_mut(&request.label) {
        record.active = false;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_webview_eval(
    app: tauri::AppHandle,
    request: WebviewEvalRequest,
) -> Result<(), String> {
    find_browser_webview(&app, &request.label)?
        .eval(&request.script)
        .map_err(|e| format!("eval failed: {e}"))
}

#[tauri::command]
pub async fn browser_webview_navigate(
    app: tauri::AppHandle,
    request: WebviewNavigateRequest,
) -> Result<(), String> {
    let url = request
        .url
        .parse::<tauri::Url>()
        .map_err(|e| format!("invalid url: {e}"))?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => return Err(format!("unsupported protocol: {scheme}")),
    }

    find_browser_webview(&app, &request.label)?
        .navigate(url)
        .map_err(|e| format!("navigate failed: {e}"))?;
    // A failed URL parse or native navigation must never acknowledge an open
    // request merely because an older target is already active.
    associate_browser_open_request(&request.label, request.open_request_id.as_deref());
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewLabelRequest {
    pub label: String,
}

#[tauri::command]
pub async fn browser_webview_reload(
    app: tauri::AppHandle,
    request: WebviewLabelRequest,
) -> Result<(), String> {
    find_browser_webview(&app, &request.label)?
        .reload()
        .map_err(|e| format!("reload failed: {e}"))
}

#[tauri::command]
pub async fn browser_webview_set_bounds(
    app: tauri::AppHandle,
    request: WebviewBoundsRequest,
) -> Result<(), String> {
    validate_webview_bounds(request.x, request.y, request.width, request.height)?;

    let webview = app
        .get_webview(&request.label)
        .ok_or_else(|| format!("Webview not found: {}", request.label))?;

    webview
        .set_bounds(tauri::Rect {
            position: tauri::Position::Logical(tauri::LogicalPosition::new(request.x, request.y)),
            size: tauri::Size::Logical(tauri::LogicalSize::new(request.width, request.height)),
        })
        .map_err(|e| format!("set bounds failed: {e}"))
}

/// Return the current URL of a browser webview.
///
/// Uses `catch_unwind` to guard against a known wry bug where
/// `WKWebView::URL()` returns nil (e.g. after navigating to an invalid
/// address), causing an `unwrap()` panic inside `url_from_webview`.
#[tauri::command]
pub async fn browser_get_url(
    app: tauri::AppHandle,
    request: WebviewLabelRequest,
) -> Result<String, String> {
    let webview = find_browser_webview(&app, &request.label)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| webview.url()));

    match result {
        Ok(Ok(url)) => Ok(url.to_string()),
        Ok(Err(e)) => Err(format!("url failed: {e}")),
        Err(_) => Err("url unavailable (webview URL is nil)".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_request_correlation_requires_the_exact_active_target() {
        let mut registry = BrowserTargetRegistry::default();
        registry.records.insert(
            "embedded-browser-panel-view-old".to_string(),
            BrowserTargetRecord {
                active: true,
                open_request_id: Some("old-request".to_string()),
                ..BrowserTargetRecord::default()
            },
        );
        registry.records.insert(
            "embedded-browser-panel-view-new".to_string(),
            BrowserTargetRecord {
                active: false,
                open_request_id: Some("new-request".to_string()),
                ..BrowserTargetRecord::default()
            },
        );

        assert_eq!(
            registry
                .active_label_for_open_request("old-request")
                .as_deref(),
            Some("embedded-browser-panel-view-old")
        );
        assert_eq!(registry.active_label_for_open_request("new-request"), None);

        registry
            .records
            .get_mut("embedded-browser-panel-view-new")
            .expect("new target")
            .active = true;
        assert_eq!(
            registry
                .active_label_for_open_request("new-request")
                .as_deref(),
            Some("embedded-browser-panel-view-new")
        );
    }
}
