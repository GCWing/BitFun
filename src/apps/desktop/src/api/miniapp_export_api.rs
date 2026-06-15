//! MiniApp slide export rasterization via the desktop system WebView (WKWebView / WebView2).
//!
//! Renders one slide HTML page in a reused hidden host webview and returns the
//! base64-encoded PNG screenshot or single-page PDF. Used by presentation
//! MiniApps (e.g. ppt-live) for page-by-page export, matching the Sparo
//! `live_app_render_slide_page` behavior.

use std::path::PathBuf;
use std::time::Duration;

use bitfun_webdriver::platform::{print_page, take_screenshot, PrintOptions};
use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

const EXPORT_VIEWPORT_WIDTH: f64 = 1280.0;
const EXPORT_VIEWPORT_HEIGHT: f64 = 720.0;
const RENDER_TIMEOUT_MS: u64 = 30_000;
const RENDER_SETTLE_MS: u64 = 900;
/// Reused hidden host — one window, navigate per slide (avoids create/close flash per page).
const EXPORT_HOST_LABEL: &str = "miniapp-slide-export-host";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppRenderSlidePageRequest {
    pub html: String,
    pub format: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

fn wrap_slide_html(html: &str, width: u32, height: u32) -> String {
    let body = html.trim();
    if body.to_ascii_lowercase().starts_with("<!doctype")
        || body.to_ascii_lowercase().starts_with("<html")
    {
        return body.to_string();
    }
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><style>html,body{{margin:0;padding:0;width:{width}px;height:{height}px;overflow:hidden;}}</style></head><body>{body}</body></html>"
    )
}

/// Write slide HTML to app cache and return a `file://` URL for the export webview.
fn file_url_for_export_html<R: tauri::Runtime>(
    app: &AppHandle<R>,
    html: &str,
) -> Result<(tauri::Url, PathBuf), String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Failed to resolve app cache dir: {error}"))?;
    let export_dir = cache_dir.join("miniapp-slide-export");
    std::fs::create_dir_all(&export_dir)
        .map_err(|error| format!("Failed to create export cache dir: {error}"))?;
    let file_path = export_dir.join(format!("slide-{}.html", Uuid::new_v4()));
    std::fs::write(&file_path, html)
        .map_err(|error| format!("Failed to write export HTML: {error}"))?;
    let url = tauri::Url::from_file_path(&file_path)
        .map_err(|_| "Failed to build file URL for export webview".to_string())?;
    Ok((url, file_path))
}

fn ensure_export_host_window<R: tauri::Runtime>(
    app: &AppHandle<R>,
    width: u32,
    height: u32,
) -> Result<tauri::WebviewWindow<R>, String> {
    if let Some(window) = app.get_webview_window(EXPORT_HOST_LABEL) {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
            width as f64,
            height as f64,
        )));
        let _ = window.hide();
        return Ok(window);
    }

    let blank = "about:blank"
        .parse::<tauri::Url>()
        .map_err(|error| format!("Invalid blank URL: {error}"))?;
    let window = WebviewWindowBuilder::new(app, EXPORT_HOST_LABEL, WebviewUrl::External(blank))
        .visible(false)
        .inner_size(width as f64, height as f64)
        .decorations(false)
        .skip_taskbar(true)
        .build()
        .map_err(|error| format!("Failed to create export host webview: {error}"))?;
    let _ = window.hide();
    Ok(window)
}

async fn with_export_webview<R: tauri::Runtime, F, Fut, T>(
    app: &AppHandle<R>,
    html: String,
    width: u32,
    height: u32,
    task: F,
) -> Result<T, String>
where
    F: FnOnce(tauri::Webview<R>) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let window = ensure_export_host_window(app, width, height)?;
    let wrapped = wrap_slide_html(&html, width, height);
    let (url, temp_html) = file_url_for_export_html(app, &wrapped)?;

    window
        .navigate(url)
        .map_err(|error| format!("Failed to navigate export webview: {error}"))?;

    tokio::time::sleep(Duration::from_millis(RENDER_SETTLE_MS)).await;

    let webview = app
        .get_webview(EXPORT_HOST_LABEL)
        .ok_or_else(|| format!("Export webview is not ready: {EXPORT_HOST_LABEL}"))?;

    let result = task(webview).await;
    let _ = std::fs::remove_file(&temp_html);
    let _ = window.hide();
    result
}

#[tauri::command]
pub async fn miniapp_render_slide_page(
    app: AppHandle,
    request: MiniAppRenderSlidePageRequest,
) -> Result<String, String> {
    let width = request.width.unwrap_or(EXPORT_VIEWPORT_WIDTH as u32);
    let height = request.height.unwrap_or(EXPORT_VIEWPORT_HEIGHT as u32);
    let format = request.format.trim().to_ascii_lowercase();

    match format.as_str() {
        "png" => {
            with_export_webview(&app, request.html, width, height, |webview| async move {
                take_screenshot(webview, RENDER_TIMEOUT_MS)
                    .await
                    .map_err(|error| error.message)
            })
            .await
        }
        "pdf" => {
            let page_width_cm = (width as f64 / 96.0) * 2.54;
            let page_height_cm = (height as f64 / 96.0) * 2.54;
            let options = PrintOptions {
                orientation: Some("landscape".to_string()),
                scale: Some(1.0),
                background: Some(true),
                page_width: Some(page_width_cm),
                page_height: Some(page_height_cm),
                margin_top: Some(0.0),
                margin_bottom: Some(0.0),
                margin_left: Some(0.0),
                margin_right: Some(0.0),
                shrink_to_fit: Some(false),
                page_ranges: None,
            };
            with_export_webview(&app, request.html, width, height, |webview| async move {
                print_page(webview, RENDER_TIMEOUT_MS, &options)
                    .await
                    .map_err(|error| error.message)
            })
            .await
        }
        other => Err(format!("Unsupported slide render format: {other}")),
    }
}
