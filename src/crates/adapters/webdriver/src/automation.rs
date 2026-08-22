//! Native WebView adapter for BitFun's shared browser-action contract.
//!
//! This module translates browser-engine primitives only. Snapshot refs,
//! selector resolution, click guards, waits, and every other product action
//! remain in `bitfun-core::BrowserActions` and are therefore identical for
//! CDP and built-in browser targets.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use image::codecs::jpeg::JpegEncoder;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::executor::session::{build_native_cookie, to_webdriver_cookie};
use crate::platform::{self, Cookie};
use crate::runtime;
use crate::server::AppState;
use crate::webdriver::Session;

use std::sync::Arc;

const SCRIPT_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone)]
pub struct EmbeddedWebviewAutomation {
    state: Arc<AppState>,
    session: Session,
}

impl EmbeddedWebviewAutomation {
    pub async fn attach(app: AppHandle, label: &str) -> Result<Self, String> {
        if app.get_webview(label).is_none() {
            return Err(format!("Webview not found: {label}"));
        }
        let state = crate::automation_state(app);
        let mut session = state.sessions.write().await.create(label.to_string());
        session.timeouts.page_load = 15_000;
        session.timeouts.script = SCRIPT_TIMEOUT_MS;
        if let Ok(stored) = state.sessions.write().await.get_mut(&session.id) {
            stored.timeouts = session.timeouts.clone();
        }
        Ok(Self { state, session })
    }

    pub fn target_id(&self) -> &str {
        &self.session.current_window
    }

    pub async fn detach(&self) {
        self.state.sessions.write().await.delete(&self.session.id);
    }

    fn webview(&self) -> Result<tauri::Webview, String> {
        self.state
            .app
            .get_webview(&self.session.current_window)
            .ok_or_else(|| format!("Webview not found: {}", self.session.current_window))
    }

    async fn run_script(&self, script: &str, args: Vec<Value>) -> Result<Value, String> {
        runtime::run_script(self.state.clone(), &self.session.id, script, args, false)
            .await
            .map_err(format_webdriver_error)
    }

    async fn evaluate_expression(&self, expression: &str) -> Result<Value, String> {
        self.run_script(
            "async (expression) => await (0, eval)(expression)",
            vec![Value::String(expression.to_string())],
        )
        .await
    }

    async fn perform_actions(&self, actions: Value) -> Result<(), String> {
        self.run_script(
            "async (actions) => { await window.__bitfunWd.performActions(actions); return null; }",
            vec![actions],
        )
        .await?;
        Ok(())
    }

    pub async fn execute(&self, method: &str, params: Option<Value>) -> Result<Value, String> {
        let params = params.unwrap_or_else(|| json!({}));
        match method {
            "Page.enable"
            | "Runtime.enable"
            | "Network.enable"
            | "DOM.enable"
            | "Page.setLifecycleEventsEnabled"
            | "Emulation.clearDeviceMetricsOverride" => Ok(json!({})),
            "Page.navigate" => {
                let url = required_string(&params, "url", method)?;
                let parsed = url
                    .parse::<tauri::Url>()
                    .map_err(|error| format!("Invalid navigation URL: {error}"))?;
                match parsed.scheme() {
                    "http" | "https" => {}
                    scheme => return Err(format!("Unsupported navigation protocol: {scheme}")),
                }
                self.webview()?
                    .navigate(parsed)
                    .map_err(|error| format!("Navigation failed: {error}"))?;
                Ok(json!({ "frameId": self.target_id() }))
            }
            "Page.reload" => {
                self.webview()?
                    .reload()
                    .map_err(|error| format!("Reload failed: {error}"))?;
                Ok(json!({}))
            }
            "Runtime.evaluate" => {
                let expression = required_string(&params, "expression", method)?;
                match self.evaluate_expression(expression).await {
                    Ok(value) => Ok(json!({
                        "result": {
                            "type": javascript_type(&value),
                            "value": value,
                        }
                    })),
                    Err(error) => Ok(json!({
                        "exceptionDetails": {
                            "text": error,
                            "exception": { "description": error },
                        }
                    })),
                }
            }
            "Input.dispatchMouseEvent" => self.dispatch_mouse_event(&params).await,
            "Input.insertText" => {
                let text = required_string(&params, "text", method)?;
                self.run_script(
                    "(text) => { const target = document.activeElement; if (!target) throw new Error('No focused element'); window.__bitfunWd.insertText(target, text); return null; }",
                    vec![Value::String(text.to_string())],
                )
                .await?;
                Ok(json!({}))
            }
            "Input.dispatchKeyEvent" => self.dispatch_key_event(&params).await,
            "Page.getLayoutMetrics" => {
                let value = self
                    .evaluate_expression(
                        "(() => ({ width: Math.max(document.documentElement.scrollWidth, document.body?.scrollWidth || 0), height: Math.max(document.documentElement.scrollHeight, document.body?.scrollHeight || 0) }))()",
                    )
                    .await?;
                let width = value.get("width").cloned().unwrap_or(json!(0));
                let height = value.get("height").cloned().unwrap_or(json!(0));
                Ok(json!({
                    "cssContentSize": { "x": 0, "y": 0, "width": width, "height": height },
                    "contentSize": { "x": 0, "y": 0, "width": width, "height": height },
                }))
            }
            "Page.captureScreenshot" => {
                let format = params
                    .get("format")
                    .and_then(Value::as_str)
                    .unwrap_or("png");
                let quality = params
                    .get("quality")
                    .and_then(Value::as_u64)
                    .unwrap_or(80)
                    .min(100) as u8;
                let png = platform::take_screenshot(self.webview()?, SCRIPT_TIMEOUT_MS)
                    .await
                    .map_err(format_webdriver_error)?;
                let data = if format.eq_ignore_ascii_case("jpeg") {
                    png_to_jpeg(png, quality).await?
                } else {
                    png
                };
                Ok(json!({ "data": data }))
            }
            "Network.getCookies" => {
                let cookies = self
                    .webview()?
                    .cookies()
                    .map_err(|error| format!("Failed to read cookies: {error}"))?;
                let cookies = cookies
                    .iter()
                    .map(to_webdriver_cookie)
                    .map(|cookie| {
                        let mut value = serde_json::to_value(cookie).unwrap_or(Value::Null);
                        if let Some(object) = value.as_object_mut() {
                            if let Some(expiry) = object.remove("expiry") {
                                object.insert("expires".to_string(), expiry);
                            }
                        }
                        value
                    })
                    .collect::<Vec<_>>();
                Ok(json!({ "cookies": cookies }))
            }
            "Network.setCookie" => {
                let cookie = cookie_from_cdp(&params)?;
                self.webview()?
                    .set_cookie(build_native_cookie(&cookie).map_err(format_webdriver_error)?)
                    .map_err(|error| format!("Failed to set cookie: {error}"))?;
                Ok(json!({ "success": true }))
            }
            "Page.close" => {
                self.webview()?
                    .close()
                    .map_err(|error| format!("Failed to close WebView: {error}"))?;
                Ok(json!({}))
            }
            "Emulation.setDeviceMetricsOverride" => Err(
                "Full-page device-metrics override is not available for an embedded WebView"
                    .to_string(),
            ),
            method if method.starts_with("DOM.") => Err(format!(
                "{method} is a CDP-only extension and is not available for an embedded WebView"
            )),
            other => Err(format!(
                "Browser primitive {other} is not supported by the embedded WebView adapter"
            )),
        }
    }

    async fn dispatch_mouse_event(&self, params: &Value) -> Result<Value, String> {
        let event_type = required_string(params, "type", "Input.dispatchMouseEvent")?;
        let x = params
            .get("x")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .round() as i64;
        let y = params
            .get("y")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .round() as i64;
        let actions = match event_type {
            "mousePressed" => json!([{
                "type": "pointer", "id": "bitfun-mouse",
                "parameters": { "pointerType": "mouse" },
                "actions": [
                    { "type": "pointerMove", "x": x, "y": y, "duration": 0, "origin": "viewport" },
                    { "type": "pointerDown", "button": 0 }
                ]
            }]),
            "mouseReleased" => json!([{
                "type": "pointer", "id": "bitfun-mouse",
                "parameters": { "pointerType": "mouse" },
                "actions": [
                    { "type": "pointerMove", "x": x, "y": y, "duration": 0, "origin": "viewport" },
                    { "type": "pointerUp", "button": 0 }
                ]
            }]),
            "mouseMoved" => json!([{
                "type": "pointer", "id": "bitfun-mouse",
                "parameters": { "pointerType": "mouse" },
                "actions": [
                    { "type": "pointerMove", "x": x, "y": y, "duration": 0, "origin": "viewport" }
                ]
            }]),
            "mouseWheel" => json!([{
                "type": "wheel", "id": "bitfun-wheel",
                "actions": [{
                    "type": "scroll", "x": x, "y": y,
                    "deltaX": params.get("deltaX").and_then(Value::as_i64).unwrap_or(0),
                    "deltaY": params.get("deltaY").and_then(Value::as_i64).unwrap_or(0),
                    "duration": 0, "origin": "viewport"
                }]
            }]),
            other => return Err(format!("Unsupported mouse event type: {other}")),
        };
        self.perform_actions(actions).await?;
        Ok(json!({}))
    }

    async fn dispatch_key_event(&self, params: &Value) -> Result<Value, String> {
        let event_type = required_string(params, "type", "Input.dispatchKeyEvent")?;
        let key = params.get("key").and_then(Value::as_str).unwrap_or("");
        let value = webdriver_key(key);
        let action_type = match event_type {
            "keyDown" | "rawKeyDown" => "keyDown",
            "keyUp" => "keyUp",
            other => return Err(format!("Unsupported key event type: {other}")),
        };
        self.perform_actions(json!([{
            "type": "key", "id": "bitfun-keyboard",
            "actions": [{ "type": action_type, "value": value }]
        }]))
        .await?;
        Ok(json!({}))
    }
}

fn required_string<'a>(params: &'a Value, key: &str, method: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{method} requires '{key}'"))
}

fn javascript_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "object",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) | Value::Object(_) => "object",
    }
}

fn webdriver_key(key: &str) -> &str {
    match key {
        "Backspace" => "\u{E003}",
        "Tab" => "\u{E004}",
        "Enter" | "Return" => "\u{E007}",
        "Escape" | "Esc" => "\u{E00C}",
        "Space" => "\u{E00D}",
        "PageUp" => "\u{E00E}",
        "PageDown" => "\u{E00F}",
        "End" => "\u{E010}",
        "Home" => "\u{E011}",
        "ArrowLeft" => "\u{E012}",
        "ArrowUp" => "\u{E013}",
        "ArrowRight" => "\u{E014}",
        "ArrowDown" => "\u{E015}",
        "Delete" => "\u{E017}",
        other => other,
    }
}

fn cookie_from_cdp(params: &Value) -> Result<Cookie, String> {
    let name = required_string(params, "name", "Network.setCookie")?.to_string();
    let value = required_string(params, "value", "Network.setCookie")?.to_string();
    Ok(Cookie {
        name,
        value,
        path: params
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string),
        domain: params
            .get("domain")
            .and_then(Value::as_str)
            .map(str::to_string),
        secure: params
            .get("secure")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        http_only: params
            .get("httpOnly")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        expiry: params
            .get("expires")
            .or_else(|| params.get("expiry"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| value as u64),
        same_site: params
            .get("sameSite")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn format_webdriver_error(error: crate::server::response::WebDriverErrorResponse) -> String {
    if let Some(stacktrace) = error.stacktrace.filter(|value| !value.trim().is_empty()) {
        format!("{}: {}\n{}", error.error, error.message, stacktrace)
    } else {
        format!("{}: {}", error.error, error.message)
    }
}

async fn png_to_jpeg(png_base64: String, quality: u8) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let bytes = BASE64_STANDARD
            .decode(png_base64)
            .map_err(|error| format!("Invalid screenshot PNG: {error}"))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|error| format!("Failed to decode screenshot PNG: {error}"))?;
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality)
            .encode_image(&image)
            .map_err(|error| format!("Failed to encode screenshot JPEG: {error}"))?;
        Ok(BASE64_STANDARD.encode(jpeg))
    })
    .await
    .map_err(|error| format!("Screenshot conversion task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_translation_matches_webdriver_special_key_contract() {
        assert_eq!(webdriver_key("Enter"), "\u{E007}");
        assert_eq!(webdriver_key("Escape"), "\u{E00C}");
        assert_eq!(webdriver_key("a"), "a");
    }

    #[test]
    fn cdp_cookie_shape_maps_to_native_cookie_contract() {
        let cookie = cookie_from_cdp(&json!({
            "name": "session",
            "value": "abc",
            "httpOnly": true,
            "sameSite": "Lax",
            "expires": 1234,
        }))
        .expect("cookie");
        assert_eq!(cookie.name, "session");
        assert!(cookie.http_only);
        assert_eq!(cookie.expiry, Some(1234));
    }
}
