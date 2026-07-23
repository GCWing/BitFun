use std::sync::Arc;

use serde_json::Value;

use crate::runtime::run_script;
use crate::server::response::WebDriverErrorResponse;
use crate::server::AppState;

pub(crate) fn navigate_to() -> &'static str {
    "(url) => { window.location.href = url; return null; }"
}

pub(crate) fn go_back() -> &'static str {
    "() => { window.history.back(); return null; }"
}

pub(crate) fn go_forward() -> &'static str {
    "() => { window.history.forward(); return null; }"
}

pub(crate) fn refresh() -> &'static str {
    "() => { window.location.reload(); return null; }"
}

pub(crate) fn title() -> &'static str {
    "() => document.title || ''"
}

pub(crate) fn source() -> &'static str {
    "() => document.documentElement ? document.documentElement.outerHTML : ''"
}

pub(crate) fn ready_state() -> &'static str {
    "() => document.readyState || ''"
}

pub(crate) async fn exec_navigation_action(
    state: Arc<AppState>,
    session_id: &str,
    script: &str,
    args: Vec<Value>,
) -> Result<(), WebDriverErrorResponse> {
    run_script(state, session_id, script, args, false).await?;
    Ok(())
}

pub(crate) async fn exec_document_value(
    state: Arc<AppState>,
    session_id: &str,
    script: &str,
) -> Result<Value, WebDriverErrorResponse> {
    run_script(state, session_id, script, Vec::new(), false).await
}
