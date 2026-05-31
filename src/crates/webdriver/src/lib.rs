mod executor;
mod platform;
mod runtime;
pub mod server;
pub mod webdriver;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;
use tauri::AppHandle;

use server::AppState;

const DEFAULT_WEBDRIVER_LABEL: &str = "main";

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);

pub fn maybe_start(app: AppHandle) {
    if !(cfg!(debug_assertions) || cfg!(feature = "embedded")) {
        return;
    }

    let Some(port) = std::env::var("BITFUN_WEBDRIVER_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
    else {
        return;
    };

    if SERVER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    let preferred_label =
        std::env::var("BITFUN_WEBDRIVER_LABEL").unwrap_or_else(|_| DEFAULT_WEBDRIVER_LABEL.into());
    let state = Arc::new(AppState::new(app.clone(), preferred_label, port));

    runtime::register_listener(app, state.clone());
    server::start(state);
}

pub fn handle_bridge_result(payload: Value) -> Result<(), String> {
    runtime::handle_invoke_payload(payload)
}
