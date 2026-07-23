//! Browser API — commands for the embedded browser feature.
//!
//! Browser webviews are created as native child webviews by this desktop
//! adapter so stream-specific initialization can run before page scripts.

use serde::Deserialize;
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
}

fn validate_browser_label(label: &str) -> Result<(), String> {
    if label.starts_with("embedded-browser-view-")
        || label.starts_with("embedded-browser-panel-view-")
    {
        Ok(())
    } else {
        Err("invalid browser webview label".to_string())
    }
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
        .map_err(|e| format!("failed to hide browser webview before positioning: {e}"))
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
        .map_err(|e| format!("navigate failed: {e}"))
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
