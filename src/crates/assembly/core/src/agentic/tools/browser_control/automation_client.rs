//! Target-neutral transport contract used by the shared browser action layer.

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::util::errors::OpenBitFunResult;

/// A lifecycle or diagnostic event emitted by a browser-engine adapter.
///
/// The shape intentionally matches CDP's method/params envelope because the
/// shared action layer only needs that small neutral fact. Native WebViews
/// synthesize the same lifecycle events without depending on CDP.
#[derive(Debug, Clone)]
pub struct BrowserAutomationEvent {
    pub method: String,
    pub params: Value,
}

/// User-facing actions guaranteed to route through the shared action/session
/// contract on both browser targets. Keep discovery metadata and tests wired
/// to this constant so target parity cannot silently drift.
pub const SHARED_BROWSER_ACTIONS: &[&str] = &[
    "connect",
    "tab_new",
    "navigate",
    "back",
    "forward",
    "reload",
    "snapshot",
    "click",
    "hover",
    "fill",
    "type",
    "check",
    "uncheck",
    "select",
    "press_key",
    "scroll",
    "auto_scroll",
    "wait",
    "get",
    "get_text",
    "get_url",
    "get_title",
    "get_html",
    "screenshot",
    "evaluate",
    "fetch",
    "cookies",
    "set_cookies",
    "read_article",
    "close",
    "list_pages",
    "tab_query",
    "switch_page",
    "list_sessions",
];

/// Explicit protocol extensions which cannot be promised by an arbitrary
/// native WebView engine.
pub const EXTERNAL_CDP_EXTENSIONS: &[&str] = &[
    "cdp",
    "network",
    "network_requests",
    "console",
    "errors",
    "trace",
    "dialog",
    "set_file_input_files",
];

/// Optional target features which cannot be implemented uniformly by every
/// browser engine. The normal DOM interaction surface is intentionally not
/// represented here: implementing [`BrowserAutomationClient`] means those
/// actions use the same [`super::BrowserActions`] code path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrowserAutomationCapabilities {
    /// Stable CDP backend node ids can be attached to snapshot entries.
    pub backend_node_ids: bool,
    /// The target can resize/capture beyond the visible viewport.
    pub full_page_screenshot: bool,
    /// A native file chooser can be populated by local file paths.
    pub file_input: bool,
    /// Arbitrary allow-listed CDP methods may be sent by the caller.
    pub raw_cdp: bool,
    /// CDP network, console, error, and trace event streams are available.
    pub protocol_diagnostics: bool,
}

impl BrowserAutomationCapabilities {
    pub const fn cdp() -> Self {
        Self {
            backend_node_ids: true,
            full_page_screenshot: true,
            file_input: true,
            raw_cdp: true,
            protocol_diagnostics: true,
        }
    }

    pub const fn embedded_webview() -> Self {
        Self {
            backend_node_ids: false,
            full_page_screenshot: false,
            file_input: false,
            raw_cdp: false,
            protocol_diagnostics: false,
        }
    }
}

/// Minimal browser-engine port consumed by [`super::BrowserActions`].
///
/// The command names form an internal adapter envelope. External Chromium
/// passes them through to CDP; the desktop adapter translates the same
/// primitives to the native WebView/WebDriver implementation. Product logic
/// must stay in `BrowserActions`, never in either adapter.
#[async_trait]
pub trait BrowserAutomationClient: Send + Sync {
    async fn send(&self, method: &str, params: Option<Value>) -> OpenBitFunResult<Value>;

    fn subscribe_events(&self) -> broadcast::Receiver<BrowserAutomationEvent>;

    fn is_connected(&self) -> bool;

    fn capabilities(&self) -> BrowserAutomationCapabilities;

    fn target_kind(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_actions_and_cdp_extensions_are_disjoint_and_unique() {
        let shared = SHARED_BROWSER_ACTIONS
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let extensions = EXTERNAL_CDP_EXTENSIONS
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(shared.len(), SHARED_BROWSER_ACTIONS.len());
        assert_eq!(extensions.len(), EXTERNAL_CDP_EXTENSIONS.len());
        assert!(shared.is_disjoint(&extensions));
        for required in ["snapshot", "click", "fill", "scroll", "screenshot"] {
            assert!(shared.contains(required));
        }
    }
}
