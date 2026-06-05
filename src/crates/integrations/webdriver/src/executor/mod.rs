mod element;
mod interaction;
mod navigation;
mod session;
mod window;

use std::sync::Arc;

use serde_json::Value;
use tauri::{Manager, WebviewWindow};

use crate::platform::{self, PrintOptions};
use crate::runtime;
use crate::server::response::WebDriverErrorResponse;
use crate::server::AppState;
use crate::webdriver::Session;

pub struct BridgeExecutor {
    pub(crate) state: Arc<AppState>,
    pub(crate) session: Session,
}

impl BridgeExecutor {
    pub fn new(state: Arc<AppState>, session: Session) -> Self {
        Self { state, session }
    }

    pub async fn from_session_id(
        state: Arc<AppState>,
        session_id: &str,
    ) -> Result<Self, WebDriverErrorResponse> {
        let session = state.sessions.read().await.get_cloned(session_id)?;
        Ok(Self::new(state, session))
    }

    pub async fn run_script(
        &self,
        script: &str,
        args: Vec<Value>,
        async_mode: bool,
    ) -> Result<Value, WebDriverErrorResponse> {
        runtime::run_script(
            self.state.clone(),
            &self.session.id,
            script,
            args,
            async_mode,
        )
        .await
        .map_err(map_bridge_error)
    }

    pub async fn take_screenshot(&self) -> Result<String, WebDriverErrorResponse> {
        let webview = self
            .state
            .app
            .get_webview(&self.session.current_window)
            .ok_or_else(|| {
                WebDriverErrorResponse::no_such_window(format!(
                    "Webview not found: {}",
                    self.session.current_window
                ))
            })?;

        platform::take_screenshot(webview, self.session.timeouts.script).await
    }

    pub async fn print_page(
        &self,
        options: PrintOptions,
    ) -> Result<String, WebDriverErrorResponse> {
        let webview = self
            .state
            .app
            .get_webview(&self.session.current_window)
            .ok_or_else(|| {
                WebDriverErrorResponse::no_such_window(format!(
                    "Webview not found: {}",
                    self.session.current_window
                ))
            })?;

        platform::print_page(webview, self.session.timeouts.script, &options).await
    }

    pub(crate) fn webview_window(&self) -> Result<WebviewWindow, WebDriverErrorResponse> {
        self.state
            .app
            .get_webview_window(&self.session.current_window)
            .ok_or_else(|| {
                WebDriverErrorResponse::no_such_window(format!(
                    "Window not found: {}",
                    self.session.current_window
                ))
            })
    }
}

fn map_bridge_error(error: WebDriverErrorResponse) -> WebDriverErrorResponse {
    if error.error != "javascript error" {
        return error;
    }

    let message = error.message.to_ascii_lowercase();
    if message.contains("stale element reference") {
        return WebDriverErrorResponse::stale_element_reference("The element reference is stale");
    }
    if message.contains("unsupported locator strategy") {
        return WebDriverErrorResponse::invalid_selector(error.message);
    }
    if message.contains("no shadow root found") {
        return WebDriverErrorResponse::no_such_shadow_root("Element does not have a shadow root");
    }
    if message.contains("no alert is currently open") {
        return WebDriverErrorResponse::no_such_alert("No alert is currently open");
    }
    if message.contains("unable to locate frame")
        || message.contains("frame window is not available")
        || message.contains("element is not a frame")
        || message.contains("invalid frame reference")
        || message.contains("unsupported frame reference")
    {
        return WebDriverErrorResponse::no_such_frame("Unable to locate frame");
    }
    if message.contains("element not found") {
        return WebDriverErrorResponse::no_such_element("No such element");
    }

    error
}
