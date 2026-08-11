//! ControlHub — unified entry point for browser, terminal, and routing metadata.
//!
//! Routes requests by `domain` to the appropriate backend:
//!   browser  → CDP-based browser control (new)
//!   terminal → TerminalApi (existing)
//!   meta     → capability and route introspection
//!
//! Local desktop and OS/system actions are intentionally surfaced through the
//! dedicated ComputerUse tool/agent, not through public ControlHub domains.

use crate::agentic::tools::browser_control::actions::{BrowserActions, MAX_WAIT_MS};
use crate::agentic::tools::browser_control::browser_launcher::{
    BrowserKind, BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
use crate::agentic::tools::browser_control::cdp_client::{CdpClient, CdpPageInfo, CdpVersionInfo};
use crate::agentic::tools::browser_control::session_registry::{
    BrowserSession, BrowserSessionRegistry, BrowserSessionState, DialogHandler,
};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::infrastructure::events::{get_global_event_system, BackendEvent};
use crate::service::config::{get_global_config_service, GlobalConfig};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::ToolImageAttachment;
use async_trait::async_trait;
use bitfun_services_core::system::{truncate_with_marker, LocalSystemProvider};
use serde_json::{json, Value};
use std::sync::Arc;

use super::control_hub::{err_response, ControlHubError, ErrorCode};

/// Process-wide registry of CDP sessions. Replaces the previous single
/// global `Option<CdpClient>` slot whose `*slot = Some(client)` semantics
/// silently dropped the prior page connection on every `connect` /
/// `switch_page`, breaking concurrent multi-tab work and racing
/// in-flight `wait` / lifecycle subscriptions.
static BROWSER_SESSIONS: std::sync::OnceLock<Arc<BrowserSessionRegistry>> =
    std::sync::OnceLock::new();

const OPEN_BUILT_IN_BROWSER_EVENT: &str = "agentic://open-built-in-browser";

/// `connect { mode: "headless" }` only attaches, it never launches. It must
/// therefore not default to the logical port used by the `default` mode:
/// otherwise a session that already connected the user's browser can never
/// reach a headless browser, because `verify_headless_cdp_browser` hard-rejects
/// the headed browser sitting on that port.
const DEFAULT_HEADLESS_CDP_PORT: u16 = DEFAULT_CDP_PORT + 1;

/// Computer Use is an independent switch from browser control (`ai.computer_use_enabled`
/// defaults to off, and there is no desktop host outside the desktop app), so ControlHub
/// must never route recovery through a tool that is absent from the agent's tool list.
const COMPUTER_USE_UNAVAILABLE_HINT: &str = "Desktop automation is unavailable in this session (Computer Use is off in Settings, or there is no desktop host). Do not call ComputerUse — it is not in your tool list. Use ControlHub browser.open_builtin for http(s) URLs and ExecCommand for local files and scripts; ask the user to enable Computer Use in Settings if desktop control is genuinely required.";

fn browser_sessions() -> Arc<BrowserSessionRegistry> {
    BROWSER_SESSIONS
        .get_or_init(|| Arc::new(BrowserSessionRegistry::new()))
        .clone()
}

pub struct ControlHubTool;

impl Default for ControlHubTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlHubTool {
    pub fn new() -> Self {
        Self
    }

    fn browser_connect_mode_from_params(params: &Value) -> &'static str {
        match params.get("mode").and_then(|v| v.as_str()) {
            Some("headless") => "headless",
            Some("default") => "default",
            _ => "default",
        }
    }

    fn default_browser_connect_hints(kind: &BrowserKind, port: u16) -> Vec<String> {
        match kind {
            BrowserKind::Chrome | BrowserKind::Edge => {
                let setup_url = if matches!(kind, BrowserKind::Chrome) {
                    "chrome://inspect/#remote-debugging"
                } else {
                    "edge://inspect/#remote-debugging"
                };
                vec![
                    format!(
                        "{} can connect BitFun to the current real profile, preserving its open tabs, cookies, extensions, and login state.",
                        kind
                    ),
                    format!(
                        "For one-time setup, ask the user to click Enable default CDP in BitFun Settings > Browser control. BitFun opens {}; enable Remote debugging there (the browser remembers this for normal future starts), then approve BitFun's connection dialog in {}.",
                        setup_url, kind
                    ),
                    "After approval, keep using browser.connect / snapshot / click / fill; BitFun retains one guarded browser connection to avoid repeated prompts.".to_string(),
                ]
            }
            _ => {
                let exe = BrowserLauncher::browser_executable(kind);
                vec![
                    format!(
                        "If {} already publishes DevToolsActivePort from its normal user-data directory, BitFun reuses that real profile automatically; otherwise it starts a persistent managed profile.",
                        kind
                    ),
                    format!(
                        "If CDP is not ready on test port {}, retry browser.connect — it starts \"{}\" with BitFun's managed profile.",
                        port, exe
                    ),
                    "After the browser is listening, use browser.connect / snapshot / click / fill to drive the DOM directly.".to_string(),
                ]
            }
        }
    }

    async fn browser_version(port: u16) -> BitFunResult<CdpVersionInfo> {
        if let Some(connection) = CdpClient::browser_connection(port).await {
            connection.client.browser_version().await
        } else {
            CdpClient::get_version(port).await
        }
    }

    async fn browser_pages(port: u16) -> BitFunResult<Vec<CdpPageInfo>> {
        if let Some(connection) = CdpClient::browser_connection(port).await {
            connection.client.browser_pages().await
        } else {
            CdpClient::list_pages(port).await
        }
    }

    async fn create_browser_page(port: u16, url: Option<&str>) -> BitFunResult<CdpPageInfo> {
        if let Some(connection) = CdpClient::browser_connection(port).await {
            connection.client.create_browser_page(url).await
        } else {
            CdpClient::create_page(port, url).await
        }
    }

    async fn connect_page(port: u16, page: &CdpPageInfo) -> BitFunResult<CdpClient> {
        if let Some(connection) = CdpClient::browser_connection(port).await {
            connection.client.attach_to_page(&page.id).await
        } else {
            let ws_url = page.web_socket_debugger_url.as_ref().ok_or_else(|| {
                BitFunError::tool("Page has no WebSocket debugger URL".to_string())
            })?;
            CdpClient::connect(ws_url).await
        }
    }

    fn headless_browser_connect_hints(port: u16) -> Vec<String> {
        vec![
            "For project Web UI testing that does not depend on user login state, use the dedicated headless browser flow instead of the user's browser.".to_string(),
            format!(
                "browser.connect {{ mode: \"headless\" }} only attaches — it never starts a browser. Start one first, e.g. \"{}\" --headless=new --remote-debugging-port={} --user-data-dir=<disposable profile dir>, then retry.",
                BrowserLauncher::browser_executable(&BrowserKind::Chrome),
                port
            ),
            format!(
                "If your headless browser listens elsewhere, pass the port explicitly: browser.connect {{ mode: \"headless\", port: <your port> }} (default headless test port is {}).",
                port
            ),
            "Do not switch to desktop mouse/keyboard browser control in headless mode.".to_string(),
        ]
    }

    /// Whether the `ComputerUse` tool is actually present in this session's
    /// tool list. Computer Use and browser control are independent switches,
    /// so ControlHub stays fully usable while Computer Use is off — but any
    /// hint that tells the agent to "use ComputerUse" must be gated on this,
    /// otherwise the agent burns turns calling a tool it does not have.
    async fn computer_use_available(context: &ToolUseContext) -> bool {
        use super::computer_use_tool::ComputerUseTool;
        ComputerUseTool::new()
            .is_available_in_context(Some(context))
            .await
    }

    /// `connect { mode: "headless" }` cannot launch a headless browser yet —
    /// it only attaches to whatever is already listening on the CDP port,
    /// which may be the user's real logged-in browser. Verify from the
    /// /json/version handshake that the endpoint reports itself as headless
    /// (Chromium reports e.g. "HeadlessChrome/126.0") before the session is
    /// labelled a disposable test browser; mislabelling the user's default
    /// browser invites destructive automation on their real profile.
    fn verify_headless_cdp_browser(
        version: &CdpVersionInfo,
        port: u16,
    ) -> Result<(), ControlHubError> {
        let browser = version.browser.as_deref().unwrap_or("");
        if browser.to_ascii_lowercase().contains("headless") {
            return Ok(());
        }
        let reported = if browser.is_empty() {
            "unknown"
        } else {
            browser
        };
        Err(ControlHubError::new(
            ErrorCode::NotAvailable,
            format!(
                "The browser on test port {} reports as '{}', which is not a headless browser. Headless mode requires a real headless browser instance and must never attach to the user's default browser.",
                port, reported
            ),
        )
        .with_hints(Self::headless_browser_connect_hints(port))
        .with_hint(
            "Use connect { mode: \"default\" } for the user-approved current Chrome or Edge profile, or the compatible managed-profile fallback instead.",
        ))
    }

    fn normalize_builtin_browser_url(
        raw_url: &str,
        computer_use_available: bool,
    ) -> Result<String, ControlHubError> {
        let trimmed = raw_url.trim();
        if trimmed.is_empty() {
            return Err(ControlHubError::new(
                ErrorCode::InvalidParams,
                "browser.open_builtin requires params.url.",
            )
            .with_hint(
                "Pass an http(s) URL or domain, e.g. { \"url\": \"https://example.com\" }.",
            ));
        }

        let normalized = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        };

        let lower = normalized.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err(ControlHubError::new(
                ErrorCode::InvalidParams,
                "Only http and https URLs can be opened in the built-in browser.",
            )
            .with_hint(if computer_use_available {
                "Use WebFetch/WebSearch for reading content, or ComputerUse for local files and OS-level URL opening."
            } else {
                "Use WebFetch/WebSearch for reading content; local files must be handled with ExecCommand or opened by the user (ComputerUse is not available in this session)."
            }));
        }

        Ok(normalized)
    }

    fn description_text() -> String {
        r#"ControlHub — the unified control entry point for browser, terminal, and routing metadata.

Use this tool via `{ domain, action, params }` for browser automation, terminal signalling, and capability/routing introspection. Local computer and operating-system actions have moved out of ControlHub: use the dedicated `ComputerUse` tool/agent for desktop UI control, screenshots, OCR, mouse/keyboard input, app launching, file/url opening, clipboard access, OS facts, and local scripts.

## Domains

### domain: "browser"  (DOM/CDP browser control)
- Default URL-opening policy:
  * For requests that only open, show, preview, or view a URL, use `open_builtin`. This is the default browser-opening action and keeps the page inside BitFun.
  * Do not call `connect`, `tab_new`, or `navigate` merely to display a URL. Use the CDP workflow only when the agent must read page content or interact with the DOM.
- UI action:
  * `open_builtin { url, title?, replace_existing? }` — open an http(s) URL in BitFun's built-in right-side browser panel. This changes the BitFun UI only; it does not fetch page text for reasoning. The panel is display-only for the user — the agent cannot snapshot, read, or interact with it; use `connect` + `snapshot` when page content is needed.
- Automation modes (external browser):
  * `connect { mode: "default" }` (default) — on Chrome 144+ and current Edge, request a user-approved connection to the currently running real profile so existing tabs and login state are preserved. Other supported Chromium browsers also reuse the real profile when it publishes DevToolsActivePort; otherwise BitFun starts or attaches its persistent managed profile on port 9222.
  * `connect { mode: "headless" }` — attach to an already-running headless browser on the headless test port 9223. This mode never starts a browser; when nothing is listening it returns `NOT_AVAILABLE` together with the exact launch command.
  * `params.port` overrides the CDP port for `connect` and for every other CDP action; after `connect`, actions reuse the connected session's port automatically.
- Actions: open_builtin, connect, tab_new, navigate, back, forward, reload, snapshot, click, hover, fill, type, check, uncheck, select, press_key, scroll, auto_scroll, wait, get, get_text, get_url, get_title, get_html, screenshot, evaluate, fetch, cookies, set_cookies, set_file_input_files, cdp, network, console, errors, trace, dialog, read_article, close, list_pages, tab_query, switch_page, list_sessions.
- Pausing:
  * `wait { duration_ms }` — pause for a fixed time, up to 60 minutes (`ms` and `seconds` are accepted spellings). This is the action to use when you must idle between rounds of work, e.g. `{ "duration_ms": 1800000 }` to resume in 30 minutes. It needs no browser session, and the result reports the `ms` actually waited, so check that figure before assuming the full pause happened.
  * `wait { condition, timeout_ms? }` — wait on the page instead: 'load' | 'domcontentloaded' | 'networkidle' | a CSS/@ref selector, bounded by `timeout_ms` (default 15s). Requires a connected session. When a `condition` is present it always wins, and any duration you pass becomes its timeout rather than a separate sleep.
  * A `wait` carrying neither is rejected with `INVALID_PARAMS` — it never silently returns.
  * `wait` holds the turn open for its whole duration, so it suits a one-off pause, not a schedule. For work that should repeat ("produce another round every 30 minutes") or resume more than an hour out, create a job with the `Cron` tool instead — it ends the turn and re-invokes you when the job fires, rather than idling with the context loaded.
- Automation workflow: connect -> navigate -> snapshot (returns @e1, @e2 ... refs) -> click/fill with `{ "selector": "@e1" }` (the key `ref` is accepted too).
- Take a fresh snapshot after any DOM mutation; a stale `@eN` ref returns `error.code = STALE_REF`, while a selector that matches nothing returns `NOT_FOUND`.

### domain: "terminal"
- list_sessions, kill (`terminal_session_id`), interrupt (`terminal_session_id`).
- Use the `ExecCommand` tool to run new commands (stop those with `ExecControl`); this domain only signals pre-existing UI terminal sessions, not `ExecCommand` sessions.

### domain: "meta"
- `capabilities` — returns `{ domains: { browser, terminal, meta }, local_client: { os, arch }, workspace_execution: { is_remote }, schema_version }`.
- `route_hint` — maps a free-form intent to the appropriate ControlHub domain, or tells you to use `ComputerUse` for local computer/system/desktop work.

## Unified Response Envelope

Every call returns a stable JSON shape:

  // success
  { "ok": true,  "domain": "...", "action": "...", "data": { ... } }
  // failure
  { "ok": false, "domain": "...", "action": "...", "error": { "code": "...", "message": "...", "hints": ["..."] } }

Branch on `ok` and `error.code`, not on English messages.
"#
        .to_string()
    }

    async fn dispatch(
        &self,
        domain: &str,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        match domain {
            "desktop" => {
                let hint = if context.is_remote() {
                    "Desktop automation (screenshots, OCR, mouse, keyboard) is not available in remote workspace sessions. Use ExecCommand for shell-based alternatives on the remote SSH host."
                } else if !Self::computer_use_available(context).await {
                    COMPUTER_USE_UNAVAILABLE_HINT
                } else {
                    "Use the dedicated ComputerUse tool/agent for screenshots, OCR, mouse, keyboard, and desktop app control."
                };
                Ok(err_response(
                    "desktop",
                    action,
                    ControlHubError::new(
                        ErrorCode::InvalidParams,
                        "The desktop domain has moved out of ControlHub.",
                    )
                    .with_hint(hint),
                ))
            }
            "browser" => self.handle_browser(action, params, context).await,
            "terminal" => self.handle_terminal(action, params, context).await,
            "system" => {
                let hint = if context.is_remote() {
                    "System actions (open_app, open_url, clipboard, OS info, local scripts) are not available in remote workspace sessions. Use ExecCommand for shell-based alternatives on the remote SSH host."
                } else if !Self::computer_use_available(context).await {
                    COMPUTER_USE_UNAVAILABLE_HINT
                } else {
                    "Use the dedicated ComputerUse tool/agent for open_app, open_url, open_file, clipboard, OS info, and local scripts."
                };
                Ok(err_response(
                    "system",
                    action,
                    ControlHubError::new(
                        ErrorCode::InvalidParams,
                        "The system domain has moved out of ControlHub.",
                    )
                    .with_hint(hint),
                ))
            }
            "meta" => self.handle_meta(action, params, context).await,
            // Structured rather than `Err`: `map_dispatch_error`'s keyword
            // classifier matches nothing in this message and would report a
            // routing mistake as INTERNAL, which the model cannot recover from.
            other => {
                let hint = if !Self::computer_use_available(context).await {
                    COMPUTER_USE_UNAVAILABLE_HINT
                } else {
                    "Use the dedicated ComputerUse tool/agent for desktop and OS-level actions."
                };
                Ok(err_response(
                    other,
                    action,
                    ControlHubError::new(
                        ErrorCode::UnknownDomain,
                        format!(
                            "Unknown domain: '{}'. Valid ControlHub domains: browser, terminal, meta.",
                            other
                        ),
                    )
                    .with_hint(hint),
                ))
            }
        }
    }

    // ── Meta domain ────────────────────────────────────────────────────
    //
    // Phase 2: model-discoverable introspection so a single ControlHub call
    // tells the agent (a) which domains are actually wired up on this host
    // and (b) which domain it should pick for a given free-form intent.
    // Without this, the model has to guess from the description and may
    // pick an unavailable domain, only learning the truth from a runtime error.

    async fn handle_meta(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        match action {
            "capabilities" => {
                // `terminal` (TerminalApi) is delivered through a global
                // registry rather than a field on the context, so we can't be
                // 100% sure here without round-tripping. We report "likely
                // available iff a desktop host is present" because that bridge
                // only exists in BitFun's desktop runtime; the actual call will
                // surface a clean error if the bridge is offline.
                let likely_terminal_available = context.computer_use_host.is_some();
                let browser_default = browser_sessions().default_id().await;
                let browser_session_count = browser_sessions().list().await.len();
                let os = std::env::consts::OS;
                let arch = std::env::consts::ARCH;

                // Probe which browser the host considers default. We surface
                // both the kind AND whether it is CDP-driveable (Safari/
                // Firefox aren't, so the model can fall back to system.open_url
                // instead of attempting a doomed `browser.connect`).
                let (browser_kind, browser_cdp_supported) =
                    match crate::agentic::tools::browser_control::browser_launcher::BrowserLauncher::detect_default_browser() {
                        Ok(k) => {
                            let supported = !matches!(
                                k,
                                crate::agentic::tools::browser_control::browser_launcher::BrowserKind::Unknown(_)
                            );
                            (Some(k.to_string()), supported)
                        }
                        Err(_) => (None, false),
                    };

                let local_system = LocalSystemProvider::new().system_info();
                let (display_server, desktop_env) = (
                    local_system.display_server,
                    local_system.desktop_environment,
                );

                let is_remote = context.is_remote();
                let workspace_execution = if is_remote {
                    json!({
                        "is_remote": true,
                        "note": "Workspace file and shell tools operate on the remote SSH host, not the local client."
                    })
                } else {
                    json!({
                        "is_remote": false
                    })
                };

                let computer_use = Self::computer_use_available(context).await;
                let body = json!({
                    "domains": {
                        "browser":  {
                            "available": true,
                            "default_session_id": browser_default,
                            "session_count": browser_session_count,
                            "default_browser": browser_kind,
                            "cdp_supported": browser_cdp_supported,
                            "ui_surface": {
                                "built_in_browser_panel": true,
                                "open_action": "open_builtin",
                                "event": OPEN_BUILT_IN_BROWSER_EVENT,
                            },
                        },
                        "terminal": { "available": likely_terminal_available, "reason": if likely_terminal_available { Value::Null } else { json!("TerminalApi is only available in contexts that registered it") } },
                        "meta":     { "available": true },
                    },
                    "local_client": {
                        "os": os,
                        "arch": arch,
                        "display_server": display_server,
                        "desktop_environment": desktop_env,
                    },
                    "workspace_execution": workspace_execution,
                    "computer_use": {
                        "available": computer_use,
                        "reason": if computer_use {
                            Value::Null
                        } else if is_remote {
                            json!("Not available in remote workspace sessions")
                        } else {
                            json!("Computer Use is disabled (ai.computer_use_enabled = false) or no desktop host is present")
                        },
                    },
                    "schema_version": "1.4",
                });
                // The value of a capability probe is entirely in the field
                // values, so the assistant-visible text must be the payload
                // itself — a one-line summary tells the model nothing.
                let assistant =
                    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
                Ok(vec![ToolResult::ok(body, Some(assistant))])
            }
            "route_hint" => {
                // Best-effort heuristic mapping a free-form intent to one
                // (or two ranked) domains. The model is still expected to
                // make the final call — this is a hint, not a binding.
                let intent = params
                    .get("intent")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool("route_hint requires 'intent' (string)".to_string())
                    })?;
                let lower = intent.to_lowercase();

                // `domain` here is always a real ControlHub domain (or the
                // sentinel "unavailable"). A tool name such as ComputerUse is
                // reported separately as `tool`, because a model that copies
                // `suggested_domain` back into `{ domain: ... }` would
                // otherwise send an unroutable request.
                let mut suggestions: Vec<(&'static str, Option<&'static str>, u32, &'static str)> =
                    vec![];
                let push =
                    |s: &mut Vec<(&'static str, Option<&'static str>, u32, &'static str)>,
                     domain: &'static str,
                     tool: Option<&'static str>,
                     score: u32,
                     why: &'static str| {
                        s.push((domain, tool, score, why));
                    };

                let browser_kw = [
                    "http",
                    "https",
                    "url",
                    "browser",
                    "google",
                    "tab",
                    "built-in browser",
                    "builtin browser",
                    "embedded browser",
                    "side browser",
                    "right-side browser",
                    "网页",
                    "浏览器",
                    "网站",
                    "内置浏览器",
                    "侧边浏览器",
                ];
                let desktop_kw = [
                    "screenshot",
                    "click on",
                    "window",
                    "dialog",
                    "finder",
                    "vscode",
                    "桌面",
                    "应用窗口",
                    "外部应用",
                ];
                let terminal_kw = ["kill terminal", "interrupt", "ctrl+c", "stop process"];
                let system_kw = [
                    "open ",
                    "applescript",
                    "shell script",
                    "运行脚本",
                    "启动应用",
                    "open app",
                ];

                for kw in browser_kw {
                    if lower.contains(kw) {
                        push(
                            &mut suggestions,
                            "browser",
                            None,
                            85,
                            "Matches browser/URL keywords; default to browser.open_builtin for opening or showing URLs, and use browser.connect only when DOM reading or interaction is required",
                        );
                        break;
                    }
                }
                let is_remote = context.is_remote();
                let computer_use = Self::computer_use_available(context).await;
                for kw in desktop_kw {
                    if lower.contains(kw) {
                        if is_remote {
                            push(
                                &mut suggestions,
                                "unavailable",
                                None,
                                75,
                                "Desktop automation is not available in remote workspace sessions. Use ExecCommand for shell-based alternatives on the remote SSH host.",
                            );
                        } else if !computer_use {
                            push(
                                &mut suggestions,
                                "unavailable",
                                None,
                                75,
                                "Matches local desktop keywords, but Computer Use is off in this session and ComputerUse is not in your tool list. Use ExecCommand for local work, or ask the user to enable Computer Use in Settings.",
                            );
                        } else {
                            push(
                                &mut suggestions,
                                "unavailable",
                                Some("ComputerUse"),
                                75,
                                "Matches local desktop/system keywords; this is not a ControlHub domain — call the ComputerUse tool instead",
                            );
                        }
                        break;
                    }
                }
                for kw in terminal_kw {
                    if lower.contains(kw) {
                        push(
                            &mut suggestions,
                            "terminal",
                            None,
                            80,
                            "Matches terminal-signal keywords",
                        );
                        break;
                    }
                }
                for kw in system_kw {
                    if lower.contains(kw) {
                        if is_remote {
                            push(
                                &mut suggestions,
                                "unavailable",
                                None,
                                70,
                                "System actions (open_app, clipboard, OS info, local scripts) are not available in remote workspace sessions. Use ExecCommand for shell-based alternatives on the remote SSH host.",
                            );
                        } else if !computer_use {
                            push(
                                &mut suggestions,
                                "unavailable",
                                None,
                                70,
                                "Matches OS/launch keywords, but Computer Use is off in this session and ComputerUse is not in your tool list. Use ExecCommand for local scripts and file handling, or ask the user to enable Computer Use in Settings.",
                            );
                        } else {
                            push(
                                &mut suggestions,
                                "unavailable",
                                Some("ComputerUse"),
                                70,
                                "Matches OS/launch keywords; this is not a ControlHub domain — call the ComputerUse tool instead",
                            );
                        }
                        break;
                    }
                }
                suggestions.sort_by_key(|suggestion| std::cmp::Reverse(suggestion.2));

                let ranked: Vec<Value> = suggestions
                    .iter()
                    .map(|(d, tool, score, why)| {
                        json!({ "domain": d, "tool": tool, "score": score, "why": why })
                    })
                    .collect();
                let top = suggestions.first().copied();
                let suggested = top.map(|(d, _, _, _)| d.to_string());
                let suggested_tool = top.and_then(|(_, tool, _, _)| tool);
                Ok(vec![ToolResult::ok(
                    json!({
                        "intent": intent,
                        "suggested_domain": suggested,
                        "suggested_tool": suggested_tool,
                        "ranked": ranked,
                        "note": "Heuristic only — confirm by reading meta.capabilities and the domain-specific docs. `suggested_domain` is always a ControlHub domain (or \"unavailable\"); a separate tool to call, if any, is in `suggested_tool`.",
                    }),
                    Some(match (&suggested, suggested_tool) {
                        (Some(d), Some(t)) => format!("Best guess: domain={} tool={}", d, t),
                        (Some(d), None) => format!("Best guess: domain={}", d),
                        (None, _) => "No confident routing match".to_string(),
                    }),
                )])
            }
            other => Err(BitFunError::tool(format!(
                "Unknown meta action: '{}'. Valid actions: capabilities, route_hint",
                other
            ))),
        }
    }

    fn is_allowed_browser_cdp_method(method: &str) -> bool {
        matches!(
            method,
            "Accessibility.getFullAXTree"
                | "DOM.getDocument"
                | "DOM.getBoxModel"
                | "DOM.getContentQuads"
                | "DOM.querySelector"
                | "DOM.querySelectorAll"
                | "DOM.scrollIntoViewIfNeeded"
                | "DOM.setFileInputFiles"
                | "DOMSnapshot.captureSnapshot"
                | "Input.dispatchMouseEvent"
                | "Input.dispatchKeyEvent"
                | "Input.insertText"
                | "Network.getCookies"
                | "Network.getResponseBody"
                | "Network.setCookie"
                | "Page.getLayoutMetrics"
                | "Page.captureScreenshot"
                | "Runtime.enable"
                | "Emulation.setDeviceMetricsOverride"
                | "Emulation.clearDeviceMetricsOverride"
        )
    }

    /// Sleep for `requested_ms`, interruptibly.
    ///
    /// The sleep races the turn's cancellation token: a 30-minute pace wait
    /// that ignored it would leave the user unable to stop the agent for half
    /// an hour. Cancellation surfaces as `BitFunError::Cancelled`, which the
    /// pipeline already records as a terminal cancelled state rather than a
    /// tool failure.
    async fn wait_for_duration(
        requested_ms: u64,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let waited_ms = requested_ms.min(MAX_WAIT_MS);
        let sleep = tokio::time::sleep(std::time::Duration::from_millis(waited_ms));

        if let Some(token) = context.cancellation_token() {
            tokio::select! {
                _ = sleep => {}
                _ = token.cancelled() => {
                    return Err(BitFunError::Cancelled(format!(
                        "browser.wait cancelled before the {} pause elapsed",
                        format_duration_ms(waited_ms)
                    )));
                }
            }
        } else {
            sleep.await;
        }

        let (data, summary) = Self::wait_outcome(requested_ms);
        Ok(vec![ToolResult::ok(data, Some(summary))])
    }

    /// Describe a completed duration wait.
    ///
    /// The old payload carried no duration at all, so a wait that returned
    /// instantly was indistinguishable from one that ran to completion; both
    /// printed "Wait completed". State the elapsed time, and say plainly when
    /// the request was clamped instead of quietly waiting less than asked.
    fn wait_outcome(requested_ms: u64) -> (Value, String) {
        let waited_ms = requested_ms.min(MAX_WAIT_MS);
        let clamped = waited_ms != requested_ms;
        let summary = if clamped {
            format!(
                "Waited {} (requested {} — clamped to the {} maximum)",
                format_duration_ms(waited_ms),
                format_duration_ms(requested_ms),
                format_duration_ms(MAX_WAIT_MS)
            )
        } else {
            format!("Waited {}", format_duration_ms(waited_ms))
        };
        (
            json!({
                "success": true,
                "action": "wait",
                "ms": waited_ms,
                "requested_ms": requested_ms,
                "clamped": clamped,
            }),
            summary,
        )
    }

    async fn handle_browser(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // A duration wait is a pure pause: it touches no page, so it must not
        // require (or even resolve) a CDP session — agents pace themselves with
        // this long before they open a browser. Condition waits fall through to
        // the session-backed path below, where any duration the caller also
        // passed becomes the condition's timeout rather than a separate sleep.
        if action == "wait" && wait_condition(params).is_none() {
            if let Some(requested_ms) = wait_duration_ms(params) {
                return Self::wait_for_duration(requested_ms, context).await;
            }
            return Ok(err_response(
                "browser",
                "wait",
                ControlHubError::new(
                    ErrorCode::InvalidParams,
                    "browser.wait requires either a duration or a `condition`.",
                )
                .with_hint(
                    "To pause, pass `duration_ms` (alias `ms`), e.g. { \"duration_ms\": 1800000 } for 30 minutes.",
                )
                .with_hint(
                    "To wait on the page, pass `condition`: 'load' | 'domcontentloaded' | 'networkidle' | a CSS/@ref selector.",
                ),
            ));
        }

        let session_id_param = params
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let port = match params.get("port").and_then(|v| v.as_u64()) {
            Some(p) => p as u16,
            None if action == "connect" => {
                if Self::browser_connect_mode_from_params(params) == "headless" {
                    DEFAULT_HEADLESS_CDP_PORT
                } else {
                    DEFAULT_CDP_PORT
                }
            }
            // Port-addressed actions after `connect` (list_pages / tab_query /
            // tab_new / switch_page) must reuse the connected session's port,
            // otherwise a headless session's follow-up calls fall back to the
            // headed browser's port.
            None => browser_sessions()
                .get(session_id_param.as_deref())
                .await
                .map(|s| s.port)
                .unwrap_or(DEFAULT_CDP_PORT),
        };

        match action {
            "open_builtin" => {
                let raw_url = params.get("url").and_then(Value::as_str).unwrap_or("");
                let computer_use = Self::computer_use_available(context).await;
                let url = match Self::normalize_builtin_browser_url(raw_url, computer_use) {
                    Ok(url) => url,
                    Err(error) => return Ok(err_response("browser", "open_builtin", error)),
                };
                let title = params
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("Browser")
                    .to_string();
                let replace_existing = params
                    .get("replace_existing")
                    .or_else(|| params.get("replaceExisting"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true);

                get_global_event_system()
                    .emit(BackendEvent::Custom {
                        event_name: OPEN_BUILT_IN_BROWSER_EVENT.to_string(),
                        payload: json!({
                            "url": url,
                            "title": title,
                            "replaceExisting": replace_existing,
                        }),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!("failed to open built-in browser: {error}"))
                    })?;

                Ok(vec![ToolResult::ok(
                    json!({
                        "success": true,
                        "url": url,
                        "title": title,
                        "replace_existing": replace_existing,
                        "observable_by_agent": false,
                        "note": "The built-in browser panel is display-only for the user; the agent cannot observe or interact with its content.",
                        "hints": [
                            "Do not call snapshot/get_text/click against this panel — it is not a CDP session.",
                            "To read page content or interact with the DOM, use browser.connect followed by snapshot, then click/fill via @eN refs.",
                        ],
                    }),
                    Some(format!(
                        "Opened {url} in the built-in browser side panel (display-only for the user; not observable by the agent — use browser.connect + snapshot to read or interact with a page)."
                    )),
                )])
            }

            "connect" => {
                let mode = Self::browser_connect_mode_from_params(params);

                if mode == "headless" && !BrowserLauncher::is_cdp_available(port).await {
                    return Ok(err_response(
                        "browser",
                        "connect",
                        ControlHubError::new(
                            ErrorCode::NotAvailable,
                            format!(
                                "Headless browser test port {} is not available. Start the dedicated headless browser first, then connect via ControlHub browser actions.",
                                port
                            ),
                        )
                        .with_hints(Self::headless_browser_connect_hints(port)),
                    ));
                }

                let kind = if let Some(browser_str) = params.get("browser").and_then(|v| v.as_str())
                {
                    parse_browser_kind(browser_str)
                } else if mode == "headless" {
                    Ok(BrowserKind::Chrome)
                } else {
                    let config = get_global_config_service()
                        .await?
                        .get_config::<GlobalConfig>(None)
                        .await?;
                    BrowserLauncher::resolve_browser_kind(Some(
                        &config.ai.browser_control_preferred_browser,
                    ))
                }?;

                let user_data_dir = params.get("user_data_dir").and_then(|v| v.as_str());
                let launch_result = if mode == "headless" {
                    LaunchResult::AlreadyConnected
                } else if user_data_dir.is_none()
                    && CdpClient::browser_connection_for_kind(port, &kind)
                        .await
                        .is_some()
                {
                    LaunchResult::AlreadyConnected
                } else {
                    // Every browser shares the same logical tool port. When
                    // the selection or explicit profile changes, stop routing
                    // new actions through the previously retained browser.
                    if CdpClient::browser_connection(port).await.is_some() {
                        CdpClient::remove_browser_connection(port).await;
                    }
                    BrowserLauncher::launch_with_cdp_opts(&kind, port, user_data_dir).await?
                };

                let uses_user_profile = match &launch_result {
                    LaunchResult::UserProfileReady { endpoint } => {
                        if let Err(error) = CdpClient::connect_user_profile_browser(
                            port,
                            endpoint.port,
                            &kind,
                            &endpoint.web_socket_url,
                        )
                        .await
                        {
                            return Ok(err_response(
                                "browser",
                                "connect",
                                ControlHubError::new(
                                    ErrorCode::NotAvailable,
                                    format!(
                                        "{} did not approve the connection to the current profile, or the approval request timed out.",
                                        kind
                                    ),
                                )
                                .with_hint(error.to_string())
                                .with_hints(Self::default_browser_connect_hints(&kind, port)),
                            ));
                        }
                        true
                    }
                    LaunchResult::UserProfileSetupRequired {
                        setup_url,
                        instructions,
                        ..
                    } => {
                        return Ok(err_response(
                            "browser",
                            "connect",
                            ControlHubError::new(
                                ErrorCode::NotAvailable,
                                format!(
                                    "{} needs one-time setup before BitFun can use the current logged-in profile.",
                                    kind
                                ),
                            )
                            .with_hint(instructions)
                            .with_hint(format!("{} setup page: {setup_url}", kind))
                            .with_hints(Self::default_browser_connect_hints(&kind, port)),
                        ));
                    }
                    _ => CdpClient::browser_connection(port).await.is_some(),
                };

                // UX shortcut: a frequent flow is "drive my Gmail tab" /
                // "drive the GitHub PR I'm looking at". Without `target_*`
                // the model needed `connect` → `list_pages` → `switch_page`
                // (3 round-trips and one chance to pick the wrong id). With
                // `target_url` / `target_title` we collapse those into a
                // single `connect` call: pick the first page whose URL or
                // title contains the substring, register it as the default
                // session, and bring it to the front.
                let target_url = params
                    .get("target_url")
                    .and_then(|v| v.as_str())
                    .map(str::to_lowercase);
                let target_title = params
                    .get("target_title")
                    .and_then(|v| v.as_str())
                    .map(str::to_lowercase);
                let activate = params
                    .get("activate")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                match &launch_result {
                    LaunchResult::AlreadyConnected
                    | LaunchResult::Launched
                    | LaunchResult::UserProfileReady { .. } => {
                        let version = Self::browser_version(port).await?;
                        if mode == "headless" {
                            if let Err(error) = Self::verify_headless_cdp_browser(&version, port) {
                                return Ok(err_response("browser", "connect", error));
                            }
                        }
                        let pages = Self::browser_pages(port).await?;
                        let connected_browser = if mode == "headless" {
                            "Headless test browser".to_string()
                        } else {
                            kind.to_string()
                        };

                        // Selection: explicit target_* > first real page > first.
                        let matched_by_target = if target_url.is_some() || target_title.is_some() {
                            pages.iter().find(|p| {
                                if !uses_user_profile && p.web_socket_debugger_url.is_none() {
                                    return false;
                                }
                                let url_ok = target_url
                                    .as_ref()
                                    .map(|n| p.url.to_lowercase().contains(n))
                                    .unwrap_or(true);
                                let title_ok = target_title
                                    .as_ref()
                                    .map(|n| p.title.to_lowercase().contains(n))
                                    .unwrap_or(true);
                                p.page_type.as_deref() == Some("page") && url_ok && title_ok
                            })
                        } else {
                            None
                        };

                        // Tell the model when its filter found nothing instead
                        // of silently falling back to the first tab and
                        // confusing the next action.
                        if (target_url.is_some() || target_title.is_some())
                            && matched_by_target.is_none()
                        {
                            return Ok(err_response(
                                "browser",
                                "connect",
                                ControlHubError::new(
                                    ErrorCode::WrongTab,
                                    format!(
                                        "No open tab matched target_url={:?} target_title={:?}",
                                        target_url, target_title
                                    ),
                                )
                                .with_hints([
                                    "Call browser.list_pages or browser.tab_query first to inspect open tabs",
                                    "Loosen the substring (e.g. domain only) and try again",
                                ]),
                            ));
                        }

                        let page = matched_by_target
                            .or_else(|| {
                                pages.iter().find(|p| {
                                    p.page_type.as_deref() == Some("page")
                                        && (uses_user_profile
                                            || p.web_socket_debugger_url.is_some())
                                })
                            })
                            .or_else(|| pages.first())
                            .ok_or_else(|| {
                                BitFunError::tool("No browser pages found via CDP".to_string())
                            })?;
                        let client = Self::connect_page(port, page).await?;
                        let session = BrowserSession {
                            session_id: page.id.clone(),
                            port,
                            client: Arc::new(client),
                            state: Arc::new(BrowserSessionState::new()),
                        };
                        browser_sessions().register(session.clone()).await;

                        // Enable CDP observers so network/console/error events
                        // start recording immediately for later query via
                        // browser.network / browser.console / browser.errors.
                        let _ = BrowserActions::new(session.client.as_ref())
                            .enable_observers()
                            .await;

                        // If the model targeted a specific tab AND wants it
                        // foregrounded (default), bring it to front the same
                        // way switch_page does. Failure here is non-fatal —
                        // we still return the connected session.
                        let mut activated = false;
                        let mut activate_warning: Option<String> = None;
                        let targeted = matched_by_target.is_some();
                        if targeted && activate {
                            match session.client.send("Page.bringToFront", None).await {
                                Ok(_) => activated = true,
                                Err(e) => {
                                    activate_warning = Some(format!(
                                        "Page.bringToFront failed: {} (session is connected, but the tab is not in the foreground)",
                                        e
                                    ));
                                }
                            }
                        }

                        let mut result = json!({
                            "success": true,
                            "browser": connected_browser,
                            "browser_mode": mode,
                            "browser_profile": if uses_user_profile { "current_user" } else { "managed" },
                            "browser_version": version.browser,
                            "port": port,
                            "session_id": session.session_id,
                            "page_url": page.url,
                            "page_title": page.title,
                            "matched_by_target": targeted,
                            "activated": activated,
                            "status": if mode == "headless" {
                                "attached"
                            } else if uses_user_profile {
                                "connected_user_profile"
                            } else if matches!(launch_result, LaunchResult::AlreadyConnected) {
                                "already_connected"
                            } else {
                                "launched"
                            },
                        });
                        if let Some(w) = activate_warning {
                            result["warning"] = json!(w);
                        }
                        let summary = if uses_user_profile {
                            format!(
                                "Connected to the current {} profile via user-approved DOM/CDP (session {}, page '{}')",
                                connected_browser, session.session_id, page.title
                            )
                        } else if targeted {
                            format!(
                                "Connected to {} via DOM/CDP (session {}, page '{}')",
                                connected_browser, session.session_id, page.title
                            )
                        } else {
                            format!(
                                "Connected to {} on test port {} via DOM/CDP (session {})",
                                connected_browser, port, session.session_id
                            )
                        };
                        Ok(vec![ToolResult::ok(result, Some(summary))])
                    }
                    LaunchResult::UserProfileSetupRequired {
                        setup_url,
                        instructions,
                        ..
                    } => Ok(err_response(
                        "browser",
                        "connect",
                        ControlHubError::new(
                            ErrorCode::NotAvailable,
                            format!(
                                "{} needs one-time setup before BitFun can use the current logged-in profile.",
                                kind
                            ),
                        )
                        .with_hint(instructions)
                        .with_hint(format!("{} setup page: {setup_url}", kind))
                        .with_hints(Self::default_browser_connect_hints(&kind, port)),
                    )),
                    LaunchResult::LaunchedButCdpNotReady { message, .. } => Ok(err_response(
                        "browser",
                        "connect",
                        ControlHubError::new(ErrorCode::Timeout, message.clone())
                            .with_hints(Self::default_browser_connect_hints(&kind, port)),
                    )),
                    LaunchResult::BrowserRunningWithoutCdp { instructions, .. } => Ok(err_response(
                        "browser",
                        "connect",
                        ControlHubError::new(
                            ErrorCode::NotAvailable,
                            "The user's default browser is running without the test port enabled.",
                        )
                        .with_hint(instructions)
                        .with_hints(Self::default_browser_connect_hints(&kind, port)),
                    )),
                }
            }

            "list_pages" => {
                let pages = Self::browser_pages(port).await?;
                let default_id = browser_sessions().default_id().await;
                let summary: Vec<Value> = pages
                    .iter()
                    .map(|p| {
                        json!({
                            "id": p.id,
                            "title": p.title,
                            "url": p.url,
                            "type": p.page_type,
                            "is_default_session": Some(&p.id) == default_id.as_ref(),
                        })
                    })
                    .collect();
                Ok(vec![ToolResult::ok(
                    json!({
                        "pages": summary,
                        "default_session_id": default_id,
                    }),
                    Some(format!(
                        "{} page(s) found (id | title | url):\n{}",
                        summary.len(),
                        page_table(&summary)
                    )),
                )])
            }

            // Phase 2: filter pages by url substring / title substring without
            // forcing the model to ingest the entire `list_pages` payload.
            // This is essential when the user has dozens of tabs open and we
            // don't want to dump 50 KB of CDP page records into context.
            "tab_query" => {
                let url_contains = params
                    .get("url_contains")
                    .and_then(|v| v.as_str())
                    .map(str::to_lowercase);
                let title_contains = params
                    .get("title_contains")
                    .and_then(|v| v.as_str())
                    .map(str::to_lowercase);
                let only_pages = params
                    .get("only_pages")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let limit = params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(20)
                    .max(1);

                let pages = Self::browser_pages(port).await?;
                let default_id = browser_sessions().default_id().await;
                let total = pages.len();
                let filtered: Vec<Value> = pages
                    .into_iter()
                    .filter(|p| {
                        if only_pages && p.page_type.as_deref() != Some("page") {
                            return false;
                        }
                        if let Some(ref needle) = url_contains {
                            if !p.url.to_lowercase().contains(needle) {
                                return false;
                            }
                        }
                        if let Some(ref needle) = title_contains {
                            if !p.title.to_lowercase().contains(needle) {
                                return false;
                            }
                        }
                        true
                    })
                    .take(limit)
                    .map(|p| {
                        json!({
                            "id": p.id,
                            "title": p.title,
                            "url": p.url,
                            "type": p.page_type,
                            "is_default_session": Some(&p.id) == default_id.as_ref(),
                        })
                    })
                    .collect();
                let matched = filtered.len();
                Ok(vec![ToolResult::ok(
                    json!({
                        "pages": filtered,
                        "matched": matched,
                        "total": total,
                        "default_session_id": default_id,
                    }),
                    Some(format!(
                        "{} of {} page(s) matched (id | title | url):\n{}",
                        matched,
                        total,
                        page_table(&filtered)
                    )),
                )])
            }

            "tab_new" => {
                let url = params.get("url").and_then(|v| v.as_str());
                let activate = params
                    .get("activate")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let page = Self::create_browser_page(port, url).await?;
                let client = Self::connect_page(port, &page).await?;
                let session = BrowserSession {
                    session_id: page.id.clone(),
                    port,
                    client: Arc::new(client),
                    state: Arc::new(BrowserSessionState::new()),
                };
                browser_sessions().register(session.clone()).await;
                let _ = BrowserActions::new(session.client.as_ref())
                    .enable_observers()
                    .await;
                if activate {
                    let _ = session.client.send("Page.bringToFront", None).await;
                }
                Ok(vec![ToolResult::ok(
                    json!({
                        "success": true,
                        "session_id": session.session_id,
                        "page_url": page.url,
                        "page_title": page.title,
                        "activated": activate,
                    }),
                    Some(format!(
                        "New tab opened: {} (session {})",
                        page.title, session.session_id
                    )),
                )])
            }

            "switch_page" => {
                let page_id = params
                    .get("page_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool("switch_page requires 'page_id'".to_string())
                    })?;
                // Phase 2: by default ALSO surface the chosen tab in the
                // user's actual browser window via `Page.bringToFront`. The
                // legacy behavior only swapped the CDP session under the
                // hood, leaving the user staring at the old tab while the
                // model "drove" an invisible one. Models can opt out by
                // passing `activate: false` for headless background tabs.
                let activate = params
                    .get("activate")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let registry = browser_sessions();
                let mut reused = false;
                let session = if registry.set_default(page_id).await.is_ok() {
                    reused = true;
                    registry.get(Some(page_id)).await?
                } else {
                    let pages = Self::browser_pages(port).await?;
                    let page = pages.iter().find(|p| p.id == page_id).ok_or_else(|| {
                        BitFunError::tool(format!("Page '{}' not found", page_id))
                    })?;
                    let client = Self::connect_page(port, page).await?;
                    let session = BrowserSession {
                        session_id: page.id.clone(),
                        port,
                        client: Arc::new(client),
                        state: Arc::new(BrowserSessionState::new()),
                    };
                    registry.register(session.clone()).await;
                    let _ = BrowserActions::new(session.client.as_ref())
                        .enable_observers()
                        .await;
                    session
                };

                let mut activated = false;
                let mut activate_warning: Option<String> = None;
                if activate {
                    match session.client.send("Page.bringToFront", None).await {
                        Ok(_) => activated = true,
                        Err(e) => {
                            // Don't fail the whole switch — the session is
                            // still valid, the user just won't see the new
                            // tab front-and-center yet.
                            activate_warning = Some(format!(
                                "Page.bringToFront failed: {} (session is switched, but the tab is not in the foreground)",
                                e
                            ));
                        }
                    }
                }

                let mut body = json!({
                    "success": true,
                    "page_id": page_id,
                    "session_id": session.session_id,
                    "reused": reused,
                    "activated": activated,
                });
                if let Some(w) = &activate_warning {
                    body["warning"] = json!(w);
                }
                Ok(vec![ToolResult::ok(
                    body,
                    Some(format!(
                        "Switched to page {} ({})",
                        page_id,
                        if activated {
                            "brought to front"
                        } else {
                            "background"
                        }
                    )),
                )])
            }

            "list_sessions" | "network" | "network_requests" | "console" | "errors" | "trace" => {
                match action {
                    "list_sessions" => {
                        let registry = browser_sessions();
                        let ids = registry.list().await;
                        let default = registry.default_id().await;
                        Ok(vec![ToolResult::ok(
                            json!({
                                "sessions": ids,
                                "default_session_id": default,
                            }),
                            Some(format!(
                                "{} session(s) tracked (default={}):\n{}",
                                ids.len(),
                                default.as_deref().unwrap_or("-"),
                                ids.join("\n")
                            )),
                        )])
                    }
                    "network" | "network_requests" => {
                        let session = browser_sessions().get(session_id_param.as_deref()).await?;
                        let state = &session.state;
                        let sub = params.get("sub_command").and_then(|v| v.as_str());
                        match sub {
                            Some("clear") => {
                                state.clear_network().await;
                                Ok(vec![ToolResult::ok(
                                    json!({ "success": true, "cleared": true }),
                                    Some("Network events cleared".to_string()),
                                )])
                            }
                            Some("summary") => {
                                let total = state
                                    .query_network(None, None, None, None, usize::MAX)
                                    .await
                                    .len();
                                let requests = state
                                    .query_network_requests(None, None, None, None, 50)
                                    .await;
                                Ok(vec![ToolResult::ok(
                                    json!({
                                        "total_events": total,
                                        "requests": requests,
                                    }),
                                    Some(format!(
                                        "Network summary: {} total events\n{}",
                                        total,
                                        event_list(&requests)
                                    )),
                                )])
                            }
                            _ => {
                                let filter = params.get("filter").and_then(|v| v.as_str());
                                let method = params.get("method").and_then(|v| v.as_str());
                                let status = params.get("status").and_then(|v| v.as_str());
                                let since = params.get("since").and_then(|v| v.as_str());
                                let limit =
                                    params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20)
                                        as usize;
                                let events = if sub == Some("requests") {
                                    state
                                        .query_network_requests(
                                            filter, method, status, since, limit,
                                        )
                                        .await
                                } else {
                                    state
                                        .query_network(filter, method, status, since, limit)
                                        .await
                                };
                                Ok(vec![ToolResult::ok(
                                    json!({ "events": events, "count": events.len() }),
                                    Some(format!(
                                        "{} network event(s):\n{}",
                                        events.len(),
                                        event_list(&events)
                                    )),
                                )])
                            }
                        }
                    }
                    "console" => {
                        let session = browser_sessions().get(session_id_param.as_deref()).await?;
                        let state = &session.state;
                        let sub = params.get("sub_command").and_then(|v| v.as_str());
                        if sub == Some("clear") {
                            state.clear_console().await;
                            return Ok(vec![ToolResult::ok(
                                json!({ "success": true, "cleared": true }),
                                Some("Console events cleared".to_string()),
                            )]);
                        }
                        let filter = params.get("filter").and_then(|v| v.as_str());
                        let since = params.get("since").and_then(|v| v.as_str());
                        let limit =
                            params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                        let events = state.query_console(filter, since, limit).await;
                        Ok(vec![ToolResult::ok(
                            json!({ "events": events, "count": events.len() }),
                            Some(format!(
                                "{} console event(s):\n{}",
                                events.len(),
                                event_list(&events)
                            )),
                        )])
                    }
                    "errors" => {
                        let session = browser_sessions().get(session_id_param.as_deref()).await?;
                        let state = &session.state;
                        let sub = params.get("sub_command").and_then(|v| v.as_str());
                        if sub == Some("clear") {
                            state.clear_errors().await;
                            return Ok(vec![ToolResult::ok(
                                json!({ "success": true, "cleared": true }),
                                Some("JS error events cleared".to_string()),
                            )]);
                        }
                        let filter = params.get("filter").and_then(|v| v.as_str());
                        let since = params.get("since").and_then(|v| v.as_str());
                        let limit =
                            params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                        let events = state.query_errors(filter, since, limit).await;
                        Ok(vec![ToolResult::ok(
                            json!({ "events": events, "count": events.len() }),
                            Some(format!(
                                "{} JS error event(s):\n{}",
                                events.len(),
                                event_list(&events)
                            )),
                        )])
                    }
                    "trace" => {
                        let session = browser_sessions().get(session_id_param.as_deref()).await?;
                        let state = &session.state;
                        let sub = params.get("sub_command").and_then(|v| v.as_str());
                        match sub {
                            Some("start") => {
                                let result = state.trace_start().await;
                                Ok(vec![ToolResult::ok(
                                    result,
                                    Some("CDP trace recording started".to_string()),
                                )])
                            }
                            Some("stop") => {
                                let limit =
                                    params.get("limit").and_then(|v| v.as_u64()).unwrap_or(200)
                                        as usize;
                                let result = state.trace_stop(limit).await;
                                Ok(vec![ToolResult::ok(
                                    result,
                                    Some("CDP trace recording stopped".to_string()),
                                )])
                            }
                            Some("status") => {
                                let result = state.trace_status().await;
                                Ok(vec![ToolResult::ok(
                                    result,
                                    Some("CDP trace status".to_string()),
                                )])
                            }
                            Some("clear") => {
                                let result = state.trace_clear().await;
                                Ok(vec![ToolResult::ok(
                                    result,
                                    Some("CDP trace cleared".to_string()),
                                )])
                            }
                            _ => {
                                let limit =
                                    params.get("limit").and_then(|v| v.as_u64()).unwrap_or(100)
                                        as usize;
                                let result = state.trace_stop(limit).await;
                                Ok(vec![ToolResult::ok(
                                    result,
                                    Some("CDP trace events".to_string()),
                                )])
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }

            _ => {
                // Resolve a session: explicit `session_id` if present, else
                // the registry's default. This replaces the prior "global
                // singleton" pattern that was racy across concurrent tasks.
                let session = browser_sessions().get(session_id_param.as_deref()).await?;
                let actions = BrowserActions::new(session.client.as_ref());

                match action {
                    "navigate" => {
                        let url = params
                            .get("url")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("navigate requires 'url'".to_string())
                            })?;
                        let result = actions.navigate(url).await?;
                        Ok(vec![ToolResult::ok(result, Some(format!("Navigated to {}", url)))])
                    }
                    "snapshot" => {
                        let with_backend = params
                            .get("with_backend_node_ids")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let result = actions.snapshot_with_options(with_backend).await?;
                        let el_count = result
                            .get("elements")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        // The whole `@eN` workflow lives in the rendered
                        // snapshot text; a bare element count leaves the model
                        // with no ref to click or fill.
                        let snapshot_text =
                            result.get("snapshot").and_then(|v| v.as_str()).unwrap_or("");
                        let url = result.get("url").and_then(|v| v.as_str()).unwrap_or("");
                        let title = result.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let assistant = format!(
                            "Snapshot ({} interactive elements)\nurl: {}\ntitle: {}\n{}",
                            el_count, url, title, snapshot_text
                        );
                        Ok(vec![ToolResult::ok(result, Some(assistant))])
                    }
                    "click" => {
                        let selector = match selector_param(params) {
                            Some(s) => s,
                            None => {
                                return Ok(missing_selector_response("click"));
                            }
                        };
                        let result = actions.click(selector).await?;
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Clicked {}", selector)),
                        )])
                    }
                    "fill" => {
                        let selector = match selector_param(params) {
                            Some(s) => s,
                            None => {
                                return Ok(missing_selector_response("fill"));
                            }
                        };
                        let value = params
                            .get("value")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("fill requires 'value'".to_string())
                            })?;
                        let result = actions.fill(selector, value).await?;
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Filled {} with text", selector)),
                        )])
                    }
                    "type" => {
                        let text = params
                            .get("text")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("type requires 'text'".to_string())
                            })?;
                        let result = actions.type_text(text).await?;
                        Ok(vec![ToolResult::ok(result, Some("Typed text".to_string()))])
                    }
                    "select" => {
                        let selector = match selector_param(params) {
                            Some(s) => s,
                            None => {
                                return Ok(missing_selector_response("select"));
                            }
                        };
                        let option_text = params
                            .get("option_text")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("select requires 'option_text'".to_string())
                            })?;
                        let result = actions.select(selector, option_text).await?;
                        // Phase 3: the underlying JS returns `{ error, available }`
                        // shaped success bodies for "select not found" and
                        // "option not found" cases. Lift those into the
                        // unified ControlHub error envelope so the model can
                        // branch on `error.code` instead of scraping JSON.
                        if let Some(err_msg) = result.get("error").and_then(|v| v.as_str()) {
                            let lowered = err_msg.to_lowercase();
                            let (code, hint) = if lowered.contains("select not found") {
                                (
                                    unresolved_selector_code(selector),
                                    format!(
                                        "No <select> matched '{}'. Take a fresh snapshot and verify the selector.",
                                        selector
                                    ),
                                )
                            } else if lowered.contains("option not found") {
                                (
                                    ErrorCode::NotFound,
                                    "Inspect `available` in error.hints for valid option labels."
                                        .to_string(),
                                )
                            } else {
                                (ErrorCode::Internal, "Browser returned an unexpected select error".to_string())
                            };
                            let mut chub_err = ControlHubError::new(code, err_msg)
                                .with_hint(hint);
                            if let Some(avail) = result.get("available") {
                                chub_err = chub_err.with_hint(format!(
                                    "available_options={}",
                                    avail
                                ));
                            }
                            return Ok(err_response("browser", "select", chub_err));
                        }
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Selected '{}'", option_text)),
                        )])
                    }
                    "press_key" => {
                        let key = params
                            .get("key")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("press_key requires 'key'".to_string())
                            })?;
                        let result = actions.press_key(key).await?;
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Pressed {}", key)),
                        )])
                    }
                    "scroll" => {
                        let direction = params
                            .get("direction")
                            .and_then(|v| v.as_str())
                            .unwrap_or("down");
                        let amount = params.get("amount").and_then(|v| v.as_i64());
                        let result = actions.scroll(direction, amount).await?;
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Scrolled {}", direction)),
                        )])
                    }
                    "wait" => {
                        // Duration waits already returned from the session-free
                        // path in `handle_browser`; only condition waits, which
                        // genuinely need the page, reach here. A duration passed
                        // alongside the condition bounds it instead of being
                        // dropped.
                        let cond = wait_condition(params);
                        let result = actions
                            .wait(None, cond, wait_condition_timeout_ms(params))
                            .await?;
                        let summary = match result.get("timed_out").and_then(|v| v.as_bool()) {
                            Some(true) => format!(
                                "Timed out waiting for '{}'",
                                cond.unwrap_or("condition")
                            ),
                            _ => format!("Waited for '{}'", cond.unwrap_or("condition")),
                        };
                        Ok(vec![ToolResult::ok(result, Some(summary))])
                    }
                    "get_text" => {
                        let selector = match selector_param(params) {
                            Some(s) => s,
                            None => {
                                return Ok(missing_selector_response("get_text"));
                            }
                        };
                        match actions.get_text(selector).await? {
                            Some(text) => Ok(vec![ToolResult::ok(
                                json!({ "text": text, "found": true }),
                                Some(text),
                            )]),
                            None => Ok(err_response(
                                "browser",
                                "get_text",
                                ControlHubError::new(
                                    unresolved_selector_code(selector),
                                    format!("No element matched selector '{}'", selector),
                                )
                                .with_hint(
                                    "Take a fresh snapshot and verify the @ref / CSS selector",
                                ),
                            )),
                        }
                    }
                    "get_url" => {
                        let url = actions.get_url().await?;
                        Ok(vec![ToolResult::ok(
                            json!({ "url": url }),
                            Some(url),
                        )])
                    }
                    "get_title" => {
                        let title = actions.get_title().await?;
                        Ok(vec![ToolResult::ok(
                            json!({ "title": title }),
                            Some(title),
                        )])
                    }
                    "screenshot" => {
                        // Gate before the CDP round-trip: when images cannot
                        // reach the model, a "success" carrying base64 the
                        // model never sees just invites endless retries with
                        // different params. Soft-degrade inside the envelope
                        // rather than erroring, so the turn is not broken.
                        let text_only = !context.primary_model_supports_image_understanding();
                        let no_multimodal_tool_output = !context
                            .primary_model_facts()
                            .multimodal_tool_output_supported();
                        if text_only || no_multimodal_tool_output {
                            let reason = if text_only {
                                "primary_model_is_text_only"
                            } else {
                                "provider_no_multimodal_tool_output"
                            };
                            return Ok(vec![ToolResult::ok(
                                json!({
                                    "success": true,
                                    "action": "screenshot",
                                    "screenshot_unavailable": true,
                                    "reason": reason,
                                    "instruction": "Screenshots cannot reach the model in this session. Use `snapshot` (page text with @eN refs) plus `get_text` / `get_html` / `get_title` / `get_url` to observe the page, then act by @eN ref. Do not retry `screenshot`.",
                                }),
                                Some("screenshot unavailable in this session: use snapshot / get_text to observe the page.".to_string()),
                            )]);
                        }
                        let format = params
                            .get("format")
                            .and_then(|v| v.as_str())
                            .unwrap_or("jpeg");
                        let quality = params
                            .get("quality")
                            .and_then(|v| v.as_u64())
                            .map(|q| q as u8);
                        let from_surface = params
                            .get("from_surface")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let full_page = params
                            .get("full_page")
                            .or_else(|| params.get("fullPage"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let mut result = actions
                            .screenshot_with_options_ext(format, quality, from_surface, full_page)
                            .await?;
                        let data_len = result
                            .get("data_length")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        // Move the base64 out of `data` into the image
                        // attachment: leaving it in `data` would duplicate the
                        // whole image and, on any fallback that serializes
                        // `data`, flood the context window.
                        let (mime_type, data_base64) = {
                            let obj = result.as_object_mut().ok_or_else(|| {
                                BitFunError::tool(
                                    "screenshot returned a non-object result".to_string(),
                                )
                            })?;
                            let normalized = obj
                                .get("format")
                                .and_then(|v| v.as_str())
                                .unwrap_or("jpeg")
                                .to_string();
                            obj.remove("data_url");
                            let data = obj
                                .remove("base64_data")
                                .and_then(|v| v.as_str().map(str::to_string))
                                .unwrap_or_default();
                            (format!("image/{}", normalized), data)
                        };
                        if data_base64.is_empty() {
                            return Ok(vec![ToolResult::ok(
                                result,
                                Some("Screenshot returned no image data.".to_string()),
                            )]);
                        }
                        Ok(vec![ToolResult::ok_with_images(
                            result,
                            Some(format!(
                                "Screenshot captured ({} bytes base64), attached as an image.",
                                data_len
                            )),
                            vec![ToolImageAttachment {
                                mime_type,
                                data_base64,
                            }],
                        )])
                    }
                    "evaluate" => {
                        let expression = params
                            .get("expression")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("evaluate requires 'expression'".to_string())
                            })?;
                        let await_promise = params
                            .get("await_promise")
                            .or_else(|| params.get("awaitPromise"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let return_by_value = params
                            .get("return_by_value")
                            .or_else(|| params.get("returnByValue"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        // Bound the size of the returned value so a runaway
                        // `JSON.stringify(document)` can't blow up the model
                        // context window. Default 16 KiB; clamp to [1 KiB, 256 KiB].
                        let max_value_bytes = params
                            .get("max_value_bytes")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(16 * 1024)
                            .clamp(1024, 256 * 1024) as usize;
                        let mut result = actions
                            .evaluate_with_options(expression, await_promise, return_by_value)
                            .await?;
                        let mut truncated = false;
                        if let Some(value) = result.pointer_mut("/result/value") {
                            let serialized = value.to_string();
                            if serialized.len() > max_value_bytes {
                                let (clip, was) =
                                    truncate_with_marker(&serialized, max_value_bytes);
                                truncated = was;
                                *value = json!(clip);
                            }
                        }
                        if let Some(obj) = result.as_object_mut() {
                            obj.insert("truncated".to_string(), json!(truncated));
                        }
                        let display = result
                            .get("result")
                            .and_then(|r| r.get("value"))
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| result.to_string());
                        Ok(vec![ToolResult::ok(result, Some(display))])
                    }
                    "back" => {
                        let result = actions.back().await?;
                        Ok(vec![ToolResult::ok(result, Some("Navigated back".to_string()))])
                    }
                    "forward" => {
                        let result = actions.forward().await?;
                        Ok(vec![ToolResult::ok(result, Some("Navigated forward".to_string()))])
                    }
                    "reload" | "refresh" => {
                        let ignore_cache = params
                            .get("ignore_cache")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let result = actions.reload(ignore_cache).await?;
                        Ok(vec![ToolResult::ok(result, Some("Page reloaded".to_string()))])
                    }
                    "hover" => {
                        let selector = match selector_param(params) {
                            Some(s) => s,
                            None => {
                                return Ok(missing_selector_response("hover"));
                            }
                        };
                        let result = actions.hover(selector).await?;
                        Ok(vec![ToolResult::ok(result, Some(format!("Hovered {}", selector)))])
                    }
                    "check" | "uncheck" => {
                        let selector = match selector_param(params) {
                            Some(s) => s,
                            None => {
                                return Ok(missing_selector_response(action));
                            }
                        };
                        let result = actions.set_checked(selector, action == "check").await?;
                        Ok(vec![ToolResult::ok(result, Some(format!("Set checked on {}", selector)))])
                    }
                    "get" => {
                        let selector = match selector_param(params) {
                            Some(s) => s,
                            None => {
                                return Ok(missing_selector_response("get"));
                            }
                        };
                        let attribute = params
                            .get("attribute")
                            .and_then(|v| v.as_str())
                            .unwrap_or("text");
                        match actions.get_attribute(selector, attribute).await? {
                            Some(value) => {
                                let display = value.to_string();
                                Ok(vec![ToolResult::ok(
                                    json!({ "value": value, "found": true, "selector": selector, "attribute": attribute }),
                                    Some(display),
                                )])
                            }
                            None => Ok(err_response(
                                "browser",
                                "get",
                                ControlHubError::new(
                                    unresolved_selector_code(selector),
                                    format!("No element matched selector '{}'", selector),
                                )
                                .with_hint("Take a fresh snapshot and verify the @ref / CSS selector"),
                            )),
                        }
                    }
                    "get_html" | "content" => {
                        let selector = selector_param(params);
                        let result = if let Some(sel) = selector {
                            actions.get_attribute(sel, "html").await?
                        } else {
                            actions.get_attribute("html", "html").await? // will fallback to document
                        };
                        match result {
                            Some(value) => {
                                let html = value.as_str().unwrap_or("").to_string();
                                let assistant = clip_for_assistant(&html);
                                Ok(vec![ToolResult::ok(
                                    json!({ "html": html, "found": true }),
                                    Some(assistant),
                                )])
                            }
                            None => {
                                // Fallback: evaluate document.documentElement.outerHTML
                                let result = actions.evaluate("document.documentElement.outerHTML").await?;
                                let html = result
                                    .get("result")
                                    .and_then(|r| r.get("value"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let assistant = clip_for_assistant(html);
                                Ok(vec![ToolResult::ok(
                                    json!({ "html": html, "found": true }),
                                    Some(assistant),
                                )])
                            }
                        }
                    }
                    "auto_scroll" => {
                        let direction = params
                            .get("direction")
                            .and_then(|v| v.as_str())
                            .unwrap_or("down");
                        let max_scrolls = params
                            .get("max_scrolls")
                            .or_else(|| params.get("maxScrolls"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(20);
                        let delay_ms = params
                            .get("delay_ms")
                            .or_else(|| params.get("delayMs"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(800);
                        let result = actions.auto_scroll(direction, max_scrolls, delay_ms).await?;
                        Ok(vec![ToolResult::ok(result, Some(format!("Auto-scrolled {}", direction)))])
                    }
                    "fetch" => {
                        let url = params
                            .get("url")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("fetch requires 'url'".to_string())
                            })?;
                        let method = params
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("GET");
                        let headers = params
                            .get("headers")
                            .cloned()
                            .unwrap_or(json!({}));
                        let body = params
                            .get("body")
                            .and_then(|v| v.as_str());
                        let result = actions.fetch(url, method, headers, body).await?;
                        // The in-page fetch reports transport/CORS failures as
                        // `{ error }` and HTTP failures as `ok: false`. Both
                        // used to be wrapped in a "Fetched <url>" success line,
                        // so the model read a CORS rejection as a fetched page.
                        if let Some(message) = result.get("error").and_then(|v| v.as_str()) {
                            return Ok(err_response(
                                "browser",
                                "fetch",
                                ControlHubError::new(
                                    ErrorCode::NotAvailable,
                                    format!("browser.fetch failed for {}: {}", url, message),
                                )
                                .with_hints([
                                    "browser.fetch runs inside the connected page, so it is bound by that page's CORS policy — it only reaches URLs same-origin with the current page.",
                                    "For a different origin, navigate to it first (browser.navigate) and then read the page with snapshot / get_text / read_article, or use WebFetch when no login session is needed.",
                                ]),
                            ));
                        }
                        // An HTTP error status is a real response, not a tool
                        // failure: keep the body (it usually explains why) and
                        // put the status in the summary so it cannot be missed.
                        let summary = match result.get("status").and_then(|v| v.as_u64()) {
                            Some(code) => {
                                let status_text = result
                                    .get("status_text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                format!("Fetched {url} — HTTP {code} {status_text}")
                                    .trim_end()
                                    .to_string()
                            }
                            None => format!("Fetched {url}"),
                        };
                        Ok(vec![ToolResult::ok(result, Some(summary))])
                    }
                    "cookies" | "get_cookies" => {
                        let urls = params
                            .get("urls")
                            .and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect());
                        let result = actions.get_cookies(urls).await?;
                        let cookie_list = result.get("cookies").cloned().unwrap_or(json!([]));
                        let cookies = cookie_list.as_array().map(|a| a.len()).unwrap_or(0);
                        let assistant = format!(
                            "{} cookie(s):\n{}",
                            cookies,
                            serde_json::to_string_pretty(&cookie_list)
                                .unwrap_or_else(|_| cookie_list.to_string())
                        );
                        Ok(vec![ToolResult::ok(result, Some(assistant))])
                    }
                    "set_cookies" => {
                        let cookies = params
                            .get("cookies")
                            .and_then(|v| v.as_array())
                            .ok_or_else(|| {
                                BitFunError::tool("set_cookies requires 'cookies' array".to_string())
                            })?;
                        let result = actions.set_cookies(cookies).await?;
                        let set = result.get("set").and_then(|v| v.as_u64()).unwrap_or(0);
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("{} cookie(s) set", set)),
                        )])
                    }
                    "set_file_input_files" | "file_upload" => {
                        let selector = selector_param(params);
                        let files: Vec<String> = params
                            .get("files")
                            .and_then(|v| v.as_array())
                            .ok_or_else(|| {
                                BitFunError::tool("set_file_input_files requires 'files' array".to_string())
                            })?
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect();
                        let result = actions.set_file_input_files(selector, &files).await?;
                        Ok(vec![ToolResult::ok(result, Some("Files set on input".to_string()))])
                    }
                    "cdp" => {
                        let method = params
                            .get("method")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("cdp requires 'method'".to_string())
                            })?;
                        if !Self::is_allowed_browser_cdp_method(method) {
                            return Ok(err_response(
                                "browser",
                                "cdp",
                                ControlHubError::new(
                                    ErrorCode::InvalidParams,
                                    format!("CDP method '{}' is not in the allowlist", method),
                                )
                                .with_hint("Only safe DOM/Input/Page/Network/Runtime/Emulation methods are allowed for sandbox protection"),
                            ));
                        }
                        let cdp_params = params.get("params").cloned();
                        let result = session.client.send(method, cdp_params).await?;
                        Ok(vec![ToolResult::ok(
                            json!({ "success": true, "method": method, "result": result }),
                            Some(format!("CDP {} executed", method)),
                        )])
                    }
                    "dialog" => {
                        let response = params
                            .get("response")
                            .and_then(|v| v.as_str())
                            .unwrap_or("accept");
                        let accept = response != "dismiss";
                        let prompt_text = params
                            .get("prompt_text")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        session.state.arm_dialog(DialogHandler { accept, prompt_text }).await;
                        let _ = session.client.send("Page.enable", None).await;
                        Ok(vec![ToolResult::ok(
                            json!({ "success": true, "dialog_armed": true, "accept": accept }),
                            Some("Dialog handler armed".to_string()),
                        )])
                    }
                    "read_article" => {
                        if let Some(url) = params.get("url").and_then(|v| v.as_str()) {
                            actions.navigate(url).await?;
                        }
                        let result = actions.read_article().await?;
                        let article = result.get("article").cloned().unwrap_or(Value::Null);
                        let excerpt = article
                            .get("excerpt")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Article: {}", excerpt)),
                        )])
                    }
                    // Note: the former `frame` / `frame_main` actions were
                    // removed — they only stored an `active_frame` marker that
                    // nothing consumed, so they never changed the execution
                    // context of subsequent actions. Snapshot / click / fill
                    // resolve elements across same-origin iframes directly.
                    "close" => {
                        let result = actions.close_page().await?;
                        // After a close, drop the session so subsequent calls
                        // don't try to talk through a half-dead WebSocket.
                        browser_sessions().remove(&session.session_id).await;
                        Ok(vec![ToolResult::ok(result, Some("Page closed".to_string()))])
                    }
                    other => Err(BitFunError::tool(format!(
                        "Unknown browser action: '{}'. Valid: connect, tab_new, navigate, back, forward, reload, snapshot, click, hover, fill, type, check, uncheck, select, press_key, scroll, auto_scroll, wait, get, get_text, get_url, get_title, get_html, screenshot, evaluate, fetch, cookies, set_cookies, set_file_input_files, cdp, network, console, errors, trace, dialog, read_article, close, list_pages, tab_query, switch_page, list_sessions",
                        other
                    ))),
                }
            }
        }
    }

    // ── Terminal domain ────────────────────────────────────────────────

    async fn handle_terminal(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Phase 4: enumerate live terminal sessions so the model can resolve
        // a `terminal_session_id` *before* attempting `kill` / `interrupt`.
        // Previously this required digging through earlier `Bash` results.
        if action == "list_sessions" {
            let api = crate::service::terminal::api::TerminalApi::from_singleton()
                .map_err(|e| BitFunError::tool(format!("TerminalApi unavailable: {}", e)))?;
            let sessions = api
                .list_sessions()
                .await
                .map_err(|e| BitFunError::tool(format!("list_sessions failed: {}", e)))?;
            let summary: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "terminal_session_id": s.id,
                        "name": s.name,
                        "cwd": s.cwd,
                        "pid": s.pid,
                        "status": s.status,
                    })
                })
                .collect();
            let count = summary.len();
            return Ok(vec![ToolResult::ok(
                json!({ "sessions": summary, "count": count }),
                Some(format!("{} terminal session(s) live", count)),
            )]);
        }

        // UX shortcut: when there is exactly one live terminal session,
        // make `terminal_session_id` optional. The 95th-percentile flow is
        // "Bash launched a long-running command, please interrupt it" and
        // the user has no other terminals open — forcing a `list_sessions`
        // round-trip just to copy the only id back wastes a turn.
        let resolved_id: String = match params.get("terminal_session_id").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                let api = crate::service::terminal::api::TerminalApi::from_singleton()
                    .map_err(|e| BitFunError::tool(format!("TerminalApi unavailable: {}", e)))?;
                let sessions = api
                    .list_sessions()
                    .await
                    .map_err(|e| BitFunError::tool(format!("list_sessions failed: {}", e)))?;
                let live: Vec<_> = sessions
                    .iter()
                    .filter(|s| {
                        s.status.eq_ignore_ascii_case("running")
                            || s.status.eq_ignore_ascii_case("active")
                            || s.status.eq_ignore_ascii_case("idle")
                    })
                    .collect();
                if live.len() == 1 {
                    live[0].id.clone()
                } else if live.is_empty() {
                    return Ok(err_response(
                        "terminal",
                        action,
                        ControlHubError::new(
                            ErrorCode::MissingSession,
                            "No live terminal sessions to target",
                        )
                        .with_hint(
                            "Use the Bash tool to start a command, then this action becomes meaningful",
                        ),
                    ));
                } else {
                    let ids: Vec<&str> = live.iter().map(|s| s.id.as_str()).collect();
                    return Ok(err_response(
                        "terminal",
                        action,
                        ControlHubError::new(
                            ErrorCode::Ambiguous,
                            format!(
                                "{} live terminal sessions; pass 'terminal_session_id' to disambiguate",
                                live.len()
                            ),
                        )
                        .with_hint(format!("live_session_ids={:?}", ids))
                        .with_hint("Call terminal.list_sessions to see names + cwd"),
                    ));
                }
            }
        };

        let mut input = params.clone();
        if let Value::Object(ref mut map) = input {
            map.insert("action".to_string(), json!(action));
            map.insert("terminal_session_id".to_string(), json!(resolved_id));
        }

        let tool = super::terminal_control_tool::TerminalControlTool::new();
        tool.call_impl(&input, context).await
    }
}

fn parse_browser_kind(browser: &str) -> BitFunResult<BrowserKind> {
    match BrowserLauncher::browser_kind_from_config(browser) {
        Some(kind) => Ok(kind),
        None => BrowserLauncher::detect_default_browser(),
    }
}

/// Parse a leading `"[CODE] rest"` prefix produced by the front-end
/// front-end error prefix so we can recover the structured `ErrorCode`
/// in the backend instead of falling back to the heuristic classifier.
/// Returns `(code, rest_without_prefix)` or `None` if the input is not in
/// that shape.
fn parse_bracket_code_prefix(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if !s.starts_with('[') {
        return None;
    }
    let end = s.find(']')?;
    let code = s[1..end].trim();
    if code.is_empty() {
        return None;
    }
    // Make sure the bracketed token actually looks like a code
    // (UPPER_SNAKE_CASE) to avoid swallowing other bracketed prefixes.
    if !code
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    let rest = s[end + 1..].trim_start();
    Some((code, rest))
}

/// Split `"message\nHints: a | b"` into `(message, ["a", "b"])`. If there is
/// no `Hints:` block, returns `(input, [])`.
fn parse_hints_suffix(input: &str) -> (String, Vec<String>) {
    if let Some(idx) = input.rfind("\nHints:") {
        let (msg, hints_block) = input.split_at(idx);
        let hints_str = hints_block.trim_start_matches("\nHints:").trim();
        let hints = hints_str
            .split('|')
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .collect();
        (msg.trim().to_string(), hints)
    } else {
        (input.trim().to_string(), Vec::new())
    }
}

#[async_trait]
impl Tool for ControlHubTool {
    fn name(&self) -> &str {
        "ControlHub"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(Self::description_text())
    }

    fn short_description(&self) -> String {
        "Control browser, terminal, and desktop helper domains through one tool.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        let mut base = Self::description_text();
        if context.map(|c| c.is_remote()).unwrap_or(false) {
            base.push_str("\n\n**Remote workspace:** Only `browser` and `meta` domains are available. `desktop` and `system` domains (screenshots, OCR, mouse/keyboard, app launching, clipboard, OS info, local scripts) are **not available** in remote sessions — the `ComputerUse` tool is disabled. Use `ExecCommand` for shell-based alternatives on the remote SSH host.");
        }
        Ok(base)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "enum": ["browser", "terminal", "meta"],
                    "description": "The control domain to target."
                },
                "action": {
                    "type": "string",
                    "description": "The atomic action to perform within the domain. For browser URL-opening or display requests, default to open_builtin; use connect and other CDP actions only for DOM reading or interaction."
                },
                "params": {
                    "type": "object",
                    "description": "Action-specific parameters. See domain documentation for details.",
                    "additionalProperties": true
                }
            },
            "required": ["domain", "action"]
        })
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn is_enabled(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let domain = input.get("domain").and_then(|v| v.as_str());
        let action = input.get("action").and_then(|v| v.as_str());

        if domain.is_none() {
            return ValidationResult {
                result: false,
                message: Some("Missing required field: domain".to_string()),
                error_code: None,
                meta: None,
            };
        }
        if action.is_none() {
            return ValidationResult {
                result: false,
                message: Some("Missing required field: action".to_string()),
                error_code: None,
                meta: None,
            };
        }
        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let domain = input.get("domain").and_then(|v| v.as_str()).unwrap_or("?");
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
        format!("ControlHub: {}.{}", domain, action)
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        // New unified envelope: prefer ok=true → data summary, ok=false → error.message.
        if let Some(ok) = output.get("ok").and_then(|v| v.as_bool()) {
            if ok {
                if let Some(s) = output.get("summary").and_then(|v| v.as_str()) {
                    return s.to_string();
                }
                return output.to_string();
            } else if let Some(err) = output.get("error") {
                let code = err.get("code").and_then(|v| v.as_str()).unwrap_or("ERROR");
                let msg = err.get("message").and_then(|v| v.as_str()).unwrap_or("");
                return format!("{}: {}", code, msg);
            }
        }
        // Legacy fallback: previous tool result shape with `result` field.
        if let Some(result) = output.get("result").and_then(|v| v.as_str()) {
            return result.to_string();
        }
        output.to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let domain = input.get("domain").and_then(|v| v.as_str()).unwrap_or("");
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("");

        if domain.is_empty() {
            return Ok(err_response(
                "?",
                action,
                ControlHubError::new(ErrorCode::InvalidParams, "Missing required field 'domain'.")
                    .with_hint("Set domain to one of: browser, terminal, meta. Use ComputerUse for desktop/system actions."),
            ));
        }
        if action.is_empty() {
            return Ok(err_response(
                domain,
                "?",
                ControlHubError::new(ErrorCode::InvalidParams, "Missing required field 'action'.")
                    .with_hint("Pick a valid action for this domain (see ControlHub description)."),
            ));
        }

        let params = input.get("params").cloned().unwrap_or(json!({}));
        let dispatched = self.dispatch(domain, action, &params, context).await;

        // Wrap legacy handler results into the unified envelope.
        match dispatched {
            Ok(results) => Ok(envelope_wrap_results(domain, action, results)),
            // Cancellation is the pipeline's own terminal state, not a tool
            // failure. Folding it into an `ok: false` envelope would both hide
            // the user's stop from the pipeline and invite the model to
            // "recover" from a turn that is already being torn down.
            Err(err @ BitFunError::Cancelled(_)) => Err(err),
            Err(err) => Ok(err_response(
                domain,
                action,
                map_dispatch_error(domain, action, err),
            )),
        }
    }
}

/// Render page records as `id | title | url` lines. The model needs the raw
/// `id` verbatim to call `switch_page`, so it must reach the assistant text —
/// a bare count leaves it unable to act on the result.
fn page_table(pages: &[Value]) -> String {
    let field = |page: &Value, key: &str| {
        page.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    pages
        .iter()
        .map(|page| {
            format!(
                "{} | {} | {}",
                field(page, "id"),
                field(page, "title"),
                field(page, "url")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render captured network/console/error events as compact JSON lines. Event
/// payloads vary by domain, so the whole record is serialized rather than
/// projected onto fixed columns.
fn event_list(events: &[Value]) -> String {
    events
        .iter()
        .map(|event| serde_json::to_string(event).unwrap_or_else(|_| event.to_string()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Accept either `selector` or `ref` for element-targeting actions. Half the
/// actions used to take only `selector`, so a model that had just learned
/// `ref` from a sibling action got an opaque failure.
fn selector_param(params: &Value) -> Option<&str> {
    ["selector", "ref"]
        .iter()
        .find_map(|key| params.get(*key).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|selector| !selector.is_empty())
}

/// A duration parameter, in milliseconds, under any of `keys`.
///
/// A model that writes `"1800000"` as a string still means 1_800_000 ms, and
/// one that writes `1.5` seconds means 1500 ms.
fn duration_param_ms(params: &Value, keys: &[&str], unit_ms: f64) -> Option<u64> {
    keys.iter()
        .find_map(|key| {
            let value = params.get(*key)?;
            value
                .as_f64()
                .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
                .filter(|n| n.is_finite() && *n >= 0.0)
        })
        .map(|n| (n * unit_ms).round() as u64)
}

/// Read a `wait` pause duration out of `params`, in milliseconds.
///
/// The action used to read `duration_ms` and nothing else, so the very
/// plausible `{ "ms": 1800000 }` was dropped on the floor and the call
/// returned instantly while still reporting success. Accept the obvious
/// spellings instead — a wait that silently does not wait is far worse than a
/// slightly wide parameter surface. Millisecond keys are checked before second
/// keys so a call carrying both cannot be read in the wrong unit.
///
/// A `condition` always wins: next to one, every duration key reads as "wait
/// for this, but no longer than" rather than as a pause, so this returns
/// `None` and [`wait_condition_timeout_ms`] takes the value instead. Sleeping
/// on `{ condition, timeout_ms }` would never look at the page at all.
fn wait_duration_ms(params: &Value) -> Option<u64> {
    const MS_KEYS: [&str; 5] = ["duration_ms", "ms", "wait_ms", "sleep_ms", "timeout_ms"];
    const SECOND_KEYS: [&str; 6] = [
        "duration_seconds",
        "duration_s",
        "seconds",
        "secs",
        "sleep_seconds",
        "wait_seconds",
    ];

    if wait_condition(params).is_some() {
        return None;
    }
    duration_param_ms(params, &MS_KEYS, 1.0)
        .or_else(|| duration_param_ms(params, &SECOND_KEYS, 1_000.0))
}

/// The `condition` a `wait` should watch for, if the caller named a usable one.
fn wait_condition(params: &Value) -> Option<&str> {
    params
        .get("condition")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|cond| !cond.is_empty())
}

/// Read the bound on a `wait { condition }` — how long to keep waiting for the
/// page before giving up.
fn wait_condition_timeout_ms(params: &Value) -> Option<u64> {
    duration_param_ms(
        params,
        &["condition_timeout_ms", "timeout_ms", "duration_ms", "ms"],
        1.0,
    )
    .or_else(|| duration_param_ms(params, &["timeout_seconds", "seconds"], 1_000.0))
}

/// Render a wait duration the way a person would say it, so the tool summary
/// reads as "Waited 30m" rather than "Wait completed" — the latter is exactly
/// what a zero-length wait used to print.
fn format_duration_ms(ms: u64) -> String {
    let total_secs = ms / 1_000;
    let (hours, minutes, seconds) = (
        total_secs / 3_600,
        (total_secs % 3_600) / 60,
        total_secs % 60,
    );
    if hours > 0 {
        format!("{hours}h{minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m{seconds:02}s")
    } else if total_secs > 0 {
        format!("{total_secs}s")
    } else {
        format!("{ms}ms")
    }
}

/// An `@eN` ref that no longer resolves means the snapshot it came from is
/// stale, which the model recovers from by re-snapshotting; a CSS selector
/// that matches nothing is simply not found.
fn unresolved_selector_code(selector: &str) -> ErrorCode {
    if selector.trim_start().starts_with('@') {
        ErrorCode::StaleRef
    } else {
        ErrorCode::NotFound
    }
}

/// Cap oversized text (page HTML) before it reaches the model, keeping the head
/// where document structure lives and saying explicitly that the tail was cut —
/// a silent truncation reads as a complete document.
fn clip_for_assistant(text: &str) -> String {
    const MAX_ASSISTANT_CHARS: usize = 20_000;
    if text.chars().count() <= MAX_ASSISTANT_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(MAX_ASSISTANT_CHARS).collect();
    format!(
        "{head}\n… (truncated, {} chars total; narrow the request with a `selector` or use browser.get_text / read_article)",
        text.chars().count()
    )
}

fn missing_selector_response(action: &str) -> Vec<ToolResult> {
    err_response(
        "browser",
        action,
        ControlHubError::new(
            ErrorCode::InvalidParams,
            format!(
                "browser.{} requires a target element: pass `selector` (CSS) or `ref` (an @eN reference from `snapshot`).",
                action
            ),
        )
        .with_hint("Call browser.snapshot first, then pass one of its @eN refs."),
    )
}

/// Re-wrap each [`ToolResult`] returned by a legacy handler into the unified
/// `{ ok: true, domain, action, data }` envelope so the model gets a consistent
/// shape across every domain. Image attachments are preserved.
fn envelope_wrap_results(domain: &str, action: &str, results: Vec<ToolResult>) -> Vec<ToolResult> {
    results
        .into_iter()
        .map(|r| match r {
            ToolResult::Result {
                data,
                result_for_assistant,
                image_attachments,
            } => {
                let summary = result_for_assistant.clone();
                let mut body = json!({
                    "ok": true,
                    "domain": domain,
                    "action": action,
                    "data": data,
                });
                if let Some(s) = result_for_assistant.as_ref() {
                    if let Some(obj) = body.as_object_mut() {
                        obj.insert("summary".to_string(), Value::String(s.clone()));
                    }
                }
                ToolResult::Result {
                    data: body,
                    result_for_assistant: summary,
                    image_attachments,
                }
            }
            other => other,
        })
        .collect()
}

/// Best-effort classification of a legacy `BitFunError` into a structured
/// ControlHub error. Domain handlers should be migrated to return structured
/// envelopes directly; this is the safety net for the transition.
fn map_dispatch_error(domain: &str, _action: &str, err: BitFunError) -> ControlHubError {
    let msg = err.to_string();

    // Frontend bridges may send back `[CODE] message\nHints: a | b` strings —
    // parse that prefix back into a structured ControlHubError so the model
    // sees the *actual* error code and hints instead of an INTERNAL fallback.
    // `BitFunError::Tool` wraps the message with `"Tool error: "`, so we try
    // both the raw form and the form after stripping that wrapper.
    let strip_candidate = msg
        .strip_prefix("Tool error: ")
        .or_else(|| msg.strip_prefix("Service error: "))
        .or_else(|| msg.strip_prefix("Agent error: "))
        .unwrap_or(msg.as_str());
    if let Some((code_str, rest)) =
        parse_bracket_code_prefix(strip_candidate).or_else(|| parse_bracket_code_prefix(&msg))
    {
        let (message, hints) = parse_hints_suffix(rest);
        let code = ErrorCode::from_str(code_str).unwrap_or(ErrorCode::FrontendError);
        let mut err = ControlHubError::new(code, message);
        for h in hints {
            err = err.with_hint(h);
        }
        return err;
    }

    let lower = msg.to_lowercase();
    let code = if lower.contains("not found") {
        ErrorCode::NotFound
    } else if lower.contains("ambiguous") {
        ErrorCode::Ambiguous
    } else if lower.contains("permission") || lower.contains("not allowed") {
        ErrorCode::PermissionDenied
    } else if lower.contains("timed out") || lower.contains("timeout") {
        ErrorCode::Timeout
    } else if lower.contains("stale") || lower.contains("take a fresh") {
        ErrorCode::StaleRef
    } else if lower.contains("refused") || lower.contains("guard") {
        ErrorCode::GuardRejected
    } else if lower.contains("only available in") || lower.contains("not available") {
        ErrorCode::NotAvailable
    } else if domain == "terminal" && lower.contains("session") {
        ErrorCode::MissingSession
    } else if domain == "browser"
        && (lower.contains("no longer connected")
            || lower.contains("tab was likely closed")
            || lower.contains("page was closed"))
    {
        ErrorCode::WrongTab
    } else {
        ErrorCode::Internal
    };
    ControlHubError::new(code, msg)
}

// ───────────────────────────────────────────────────────────────────────
// Phase 5 — unit tests covering the ControlHub facade surface that does
// not require a live ComputerUseHost / browser. Everything here exercises
// dispatch validation, the unified error envelope, the meta domain, and
// classify_browser_error so regressions are caught at `cargo test` time.
// ───────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod control_hub_tests {
    use super::*;
    use crate::agentic::tools::implementations::computer_use_actions::ComputerUseActions;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: std::collections::HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn unknown_domain_is_rejected_with_message_listing_valid_domains() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        // Surfaced as a structured `ok: false` envelope rather than a raised
        // error, so the model can branch on `error.code`.
        let results = tool
            .dispatch("nope", "any", &json!({}), &ctx)
            .await
            .expect("unknown domain is reported in-band");
        let payload = results.first().unwrap().content();
        assert_eq!(payload.get("ok").and_then(|v| v.as_bool()), Some(false));
        let error = payload.get("error").expect("error envelope");
        assert_eq!(
            error.get("code").and_then(|v| v.as_str()),
            Some("UNKNOWN_DOMAIN")
        );
        let msg = error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(msg.contains("Unknown domain"), "got: {msg}");
        for d in ["browser", "terminal", "meta"] {
            assert!(
                msg.contains(d),
                "valid domain {d} missing from error: {msg}"
            );
        }
        // ComputerUse is a separate tool, not a ControlHub domain — listing it
        // as one sent models chasing a domain that never existed.
        assert!(
            !msg.contains("ComputerUse"),
            "ComputerUse must not be advertised as a ControlHub domain: {msg}"
        );
    }

    #[test]
    fn wait_duration_accepts_the_spellings_models_actually_emit() {
        // `ms` is what the model reached for in the field; reading only
        // `duration_ms` dropped it and turned a 30-minute pause into a no-op.
        for key in ["duration_ms", "ms", "wait_ms", "sleep_ms"] {
            assert_eq!(
                wait_duration_ms(&json!({ key: 1_800_000u64 })),
                Some(1_800_000),
                "millisecond key {key} must be honoured"
            );
        }
        for key in [
            "duration_seconds",
            "duration_s",
            "seconds",
            "secs",
            "sleep_seconds",
            "wait_seconds",
        ] {
            assert_eq!(
                wait_duration_ms(&json!({ key: 90 })),
                Some(90_000),
                "second key {key} must be converted to milliseconds"
            );
        }
        assert_eq!(wait_duration_ms(&json!({ "ms": "1500" })), Some(1_500));
        assert_eq!(wait_duration_ms(&json!({ "seconds": 1.5 })), Some(1_500));
        // Both units present: milliseconds win, so the wait can never be read
        // a thousand times too short.
        assert_eq!(
            wait_duration_ms(&json!({ "seconds": 5, "duration_ms": 1_800_000u64 })),
            Some(1_800_000)
        );
        assert_eq!(wait_duration_ms(&json!({ "condition": "load" })), None);
        assert_eq!(wait_duration_ms(&json!({ "ms": -5 })), None);
        assert_eq!(wait_duration_ms(&json!({ "ms": "soon" })), None);
        // On its own `timeout_ms` can only mean the pause itself.
        assert_eq!(
            wait_duration_ms(&json!({ "timeout_ms": 5_000 })),
            Some(5_000)
        );
    }

    #[test]
    fn a_condition_wait_keeps_its_timeout_instead_of_becoming_a_sleep() {
        // `{ condition, timeout_ms }` means "wait for this, but no longer
        // than". Reading `timeout_ms` as a pause would sleep and never look at
        // the page at all.
        let params = json!({ "condition": "networkidle", "timeout_ms": 30_000 });
        assert_eq!(wait_duration_ms(&params), None);
        assert_eq!(wait_condition_timeout_ms(&params), Some(30_000));

        // A duration passed next to a condition bounds it rather than being
        // dropped on the floor.
        let params = json!({ "condition": "#done", "duration_ms": 45_000 });
        assert_eq!(wait_duration_ms(&params), None);
        assert_eq!(wait_condition_timeout_ms(&params), Some(45_000));

        // Nothing given: the action falls back to its own default.
        assert_eq!(
            wait_condition_timeout_ms(&json!({ "condition": "load" })),
            None
        );
        assert_eq!(
            wait_condition_timeout_ms(&json!({ "condition": "load", "timeout_seconds": 20 })),
            Some(20_000)
        );
    }

    #[tokio::test]
    async fn browser_wait_actually_sleeps_and_reports_the_elapsed_time() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let started = std::time::Instant::now();
        let results = tool
            // No browser session exists in this test: a duration wait must not
            // need one.
            .dispatch("browser", "wait", &json!({ "ms": 250 }), &ctx)
            .await
            .expect("duration wait should succeed");
        assert!(
            started.elapsed() >= std::time::Duration::from_millis(240),
            "wait returned after only {:?}",
            started.elapsed()
        );
        // `dispatch` returns the raw handler payload; `call_impl` is what adds
        // the `{ ok, domain, action, data }` envelope.
        let data = results.first().expect("one result").content();
        assert_eq!(data.get("ms").and_then(|v| v.as_u64()), Some(250));
        assert_eq!(data.get("clamped").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(data.get("success").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn browser_wait_clamps_absurd_durations_and_says_so() {
        // Asserted on the reporting helper rather than through `dispatch`, so
        // the test does not have to sit through the wait itself.
        let (data, summary) = ControlHubTool::wait_outcome(MAX_WAIT_MS * 3);
        assert_eq!(data.get("ms").and_then(|v| v.as_u64()), Some(MAX_WAIT_MS));
        assert_eq!(
            data.get("requested_ms").and_then(|v| v.as_u64()),
            Some(MAX_WAIT_MS * 3)
        );
        assert_eq!(data.get("clamped").and_then(|v| v.as_bool()), Some(true));
        // A shortened wait must announce itself; silently waiting less than
        // asked is how the agent ends up out of step with the schedule.
        assert!(summary.contains("clamped"), "got: {summary}");

        let (data, summary) = ControlHubTool::wait_outcome(1_800_000);
        assert_eq!(data.get("clamped").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(summary, "Waited 30m00s");
    }

    #[tokio::test]
    async fn browser_wait_without_duration_or_condition_is_rejected() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("browser", "wait", &json!({}), &ctx)
            .await
            .expect("reported in-band");
        let payload = results.first().unwrap().content();
        // Reporting success for a wait that did not wait is the failure this
        // guards: the agent moved straight on believing it had paused.
        assert_eq!(payload.get("ok").and_then(|v| v.as_bool()), Some(false));
        let error = payload.get("error").expect("error envelope");
        assert_eq!(
            error.get("code").and_then(|v| v.as_str()),
            Some("INVALID_PARAMS")
        );
    }

    #[tokio::test]
    async fn browser_wait_is_interrupted_by_cancellation() {
        let token = tokio_util::sync::CancellationToken::new();
        let mut ctx = empty_context();
        ctx.runtime_handles =
            bitfun_runtime_ports::ToolRuntimeHandles::new(None, Some(token.clone()));

        let started = std::time::Instant::now();
        // Driven through `call_impl` so this also covers the envelope layer,
        // which must let a cancellation through instead of reporting it as an
        // ordinary `ok: false` tool error.
        let waiter = tokio::spawn(async move {
            ControlHubTool::new()
                .call_impl(
                    &json!({
                        "domain": "browser",
                        "action": "wait",
                        "params": { "seconds": 600 },
                    }),
                    &ctx,
                )
                .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        token.cancel();
        let outcome = waiter.await.expect("wait task joins");

        // Without this, stopping the agent could not take effect until the
        // pause elapsed — ten minutes of an unstoppable turn.
        assert!(
            matches!(outcome, Err(BitFunError::Cancelled(_))),
            "a long pace wait must stay interruptible"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
    }

    #[test]
    fn wait_durations_are_formatted_for_humans() {
        assert_eq!(format_duration_ms(1_800_000), "30m00s");
        assert_eq!(format_duration_ms(MAX_WAIT_MS), "1h00m");
        assert_eq!(format_duration_ms(1_500), "1s");
        assert_eq!(format_duration_ms(250), "250ms");
    }

    #[tokio::test]
    async fn meta_capabilities_reports_local_client_and_domain_table() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("meta", "capabilities", &json!({}), &ctx)
            .await
            .expect("capabilities should succeed");
        let payload = results.first().expect("one result").content();
        let domains = payload.get("domains").expect("domains present");
        for d in ["browser", "terminal", "meta"] {
            assert!(
                domains.get(d).is_some(),
                "domain {d} missing from capabilities payload: {payload}"
            );
        }
        assert!(domains.get("desktop").is_none());
        assert!(domains.get("system").is_none());
        assert_eq!(
            payload
                .get("local_client")
                .and_then(|h| h.get("os"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            Some(std::env::consts::OS.to_string())
        );
    }

    #[tokio::test]
    async fn route_hint_picks_browser_for_url_intent() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "meta",
                "route_hint",
                &json!({ "intent": "open https://example.com in a new tab" }),
                &ctx,
            )
            .await
            .expect("route_hint succeeds");
        let payload = results.first().unwrap().content();
        let ranked = payload
            .get("ranked")
            .and_then(|v| v.as_array())
            .expect("ranked array");
        assert!(
            ranked
                .iter()
                .any(|s| { s.get("domain").and_then(|v| v.as_str()) == Some("browser") }),
            "browser must appear in ranked for URL intent: {payload}"
        );
        assert_eq!(
            payload.get("suggested_domain").and_then(|v| v.as_str()),
            Some("browser")
        );
        assert!(
            payload.to_string().contains("open_builtin"),
            "route hint should default URL-opening intents to browser.open_builtin: {payload}"
        );
    }

    #[tokio::test]
    async fn route_hint_picks_browser_for_builtin_browser_intent() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "meta",
                "route_hint",
                &json!({ "intent": "使用内置浏览器打开 example.com 网页" }),
                &ctx,
            )
            .await
            .expect("route_hint succeeds");
        let payload = results.first().unwrap().content();
        assert_eq!(
            payload.get("suggested_domain").and_then(|v| v.as_str()),
            Some("browser")
        );
        assert!(
            payload.to_string().contains("open_builtin"),
            "route hint should point built-in browser requests to browser.open_builtin: {payload}"
        );
    }

    #[test]
    fn route_hint_does_not_suggest_removed_app_domain() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let results = rt
            .block_on(tool.dispatch(
                "meta",
                "route_hint",
                &json!({ "intent": "切换 BitFun 默认模型" }),
                &ctx,
            ))
            .unwrap();
        let payload = results.first().unwrap().content();
        let arr = payload.get("ranked").and_then(|v| v.as_array()).unwrap();
        assert!(arr
            .iter()
            .all(|s| s.get("domain").and_then(|v| v.as_str()) != Some("app")));
    }

    #[test]
    fn parse_bracket_code_prefix_extracts_code_and_rest() {
        // Standard structured frontend error shape.
        let (code, rest) = parse_bracket_code_prefix("[NOT_FOUND] no element matched #x")
            .expect("must parse code");
        assert_eq!(code, "NOT_FOUND");
        assert_eq!(rest, "no element matched #x");

        // With trailing hints block (preserved untouched in `rest`).
        let (code, rest) = parse_bracket_code_prefix(
            "[AMBIGUOUS] multiple matches\nHints: refine selector | use index",
        )
        .unwrap();
        assert_eq!(code, "AMBIGUOUS");
        assert!(rest.starts_with("multiple matches"));
        assert!(rest.contains("Hints:"));
    }

    #[test]
    fn parse_bracket_code_prefix_rejects_non_code_brackets() {
        assert!(parse_bracket_code_prefix("[not a code] foo").is_none());
        assert!(parse_bracket_code_prefix("no prefix here").is_none());
        assert!(parse_bracket_code_prefix("[] empty").is_none());
    }

    #[test]
    fn parse_hints_suffix_splits_pipe_delimited_hints() {
        let (msg, hints) = parse_hints_suffix("the error\nHints: a | b | c");
        assert_eq!(msg, "the error");
        assert_eq!(hints, vec!["a", "b", "c"]);

        let (msg, hints) = parse_hints_suffix("just a message");
        assert_eq!(msg, "just a message");
        assert!(hints.is_empty());
    }

    #[test]
    fn map_dispatch_error_recovers_frontend_structured_errors() {
        // Front-end-shaped error string round-trips into a real
        // ControlHubError with the original code AND its hints — instead
        // of falling back to FRONTEND_ERROR / INTERNAL like the old
        // heuristic-only path did.
        let err = map_dispatch_error(
            "desktop",
            "click",
            BitFunError::tool(
                "[AMBIGUOUS] 3 matches for text 'Save'\nHints: pass index | use selector"
                    .to_string(),
            ),
        );
        assert!(matches!(err.code, ErrorCode::Ambiguous));
        assert!(err.message.contains("Save"));
        assert!(err.hints.iter().any(|h| h.contains("pass index")));
        assert!(err.hints.iter().any(|h| h.contains("use selector")));

        // Unknown frontend code should fall through to FRONTEND_ERROR.
        let err = map_dispatch_error(
            "desktop",
            "x",
            BitFunError::tool("[WAT_IS_THIS] ouch".to_string()),
        );
        assert!(matches!(err.code, ErrorCode::FrontendError));
    }

    #[test]
    fn map_dispatch_error_classifies_browser_dead_session_as_wrong_tab() {
        let err = map_dispatch_error(
            "browser",
            "click",
            BitFunError::tool(
                "Browser session 'AB' is no longer connected (the tab was likely closed)."
                    .to_string(),
            ),
        );
        assert!(matches!(err.code, ErrorCode::WrongTab));
    }

    #[test]
    fn map_dispatch_error_classifies_known_phrases() {
        let mk = |s: &str| BitFunError::tool(s.to_string());
        assert!(matches!(
            map_dispatch_error("browser", "select", mk("element not found")).code,
            ErrorCode::NotFound
        ));
        assert!(matches!(
            map_dispatch_error("browser", "wait", mk("Operation timed out")).code,
            ErrorCode::Timeout
        ));
        assert!(matches!(
            map_dispatch_error(
                "browser",
                "click",
                mk("stale reference, take a fresh snapshot")
            )
            .code,
            ErrorCode::StaleRef
        ));
        // "session ... not found" hits NotFound first (correct: that is what
        // the model needs to know), so verify the terminal-specific branch
        // trips on a phrasing that doesn't say "not found".
        assert!(matches!(
            map_dispatch_error("terminal", "kill", mk("invalid terminal session id")).code,
            ErrorCode::MissingSession
        ));
        assert!(matches!(
            map_dispatch_error("browser", "x", mk("something exploded")).code,
            ErrorCode::Internal
        ));
    }

    #[test]
    fn map_dispatch_error_recovers_structured_browser_action_errors() {
        use crate::agentic::tools::browser_control::actions;

        // Element-not-found built at the action source arrives as a
        // structured [NOT_FOUND] with its snapshot-recovery hint intact —
        // no reliance on the phrase-matching fallback.
        let err = map_dispatch_error(
            "browser",
            "click",
            actions::classify_evaluate_exception("Error: Element not found: @e7"),
        );
        assert!(matches!(err.code, ErrorCode::NotFound));
        assert!(
            err.hints.iter().any(|h| h.contains("take a new snapshot")),
            "recovery hint missing: {:?}",
            err.hints
        );

        // Dead CDP transport maps to WRONG_TAB with a re-attach instruction.
        let err = map_dispatch_error(
            "browser",
            "snapshot",
            actions::classify_transport_error(BitFunError::tool(
                "CDP send failed: broken pipe".to_string(),
            )),
        );
        assert!(matches!(err.code, ErrorCode::WrongTab));
        assert!(err.hints.iter().any(|h| h.contains("browser.connect")));

        // Cross-origin iframe coordinate failure maps to NOT_AVAILABLE and
        // tells the model to re-target via snapshot.
        let err = map_dispatch_error("browser", "click", actions::cross_origin_frame_error("@e3"));
        assert!(matches!(err.code, ErrorCode::NotAvailable));
        assert!(err.message.contains("cross-origin iframe"));
        assert!(err.hints.iter().any(|h| h.contains("same-origin")));
    }

    #[tokio::test]
    async fn description_does_not_advertise_removed_frame_actions() {
        // `frame` / `frame_main` only wrote an `active_frame` marker no code
        // ever read — advertising them tricked the model into no-op calls.
        let desc = ControlHubTool::new().description().await.unwrap();
        assert!(
            !desc.contains("frame_main"),
            "dead frame/frame_main actions must not be advertised: {desc}"
        );
    }

    #[tokio::test]
    async fn browser_open_builtin_marks_panel_not_observable_by_agent() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "browser",
                "open_builtin",
                &json!({ "url": "example.com" }),
                &ctx,
            )
            .await
            .expect("open_builtin succeeds without a frontend emitter");
        let payload = results.first().expect("one result").content();
        assert_eq!(
            payload.get("observable_by_agent").and_then(|v| v.as_bool()),
            Some(false),
            "open_builtin must state the panel is not agent-observable: {payload}"
        );
        let text = payload.to_string();
        assert!(
            text.contains("display-only"),
            "payload must say the panel is display-only: {text}"
        );
        assert!(
            text.contains("browser.connect"),
            "payload must route content reading to browser.connect + snapshot: {text}"
        );
    }

    #[tokio::test]
    async fn description_points_desktop_and_system_work_to_computer_use() {
        let desc = ControlHubTool::new().description().await.unwrap();
        assert!(
            desc.contains("ComputerUse"),
            "description must point local computer work to ComputerUse"
        );
        assert!(
            !desc.contains("domain: \"desktop\"") && !desc.contains("domain: \"system\""),
            "ControlHub description must not advertise desktop/system domains"
        );
    }

    #[tokio::test]
    async fn description_documents_wait_params_and_routes_schedules_to_cron() {
        let desc = ControlHubTool::new().description().await.unwrap();
        // The action took `duration_ms` and documented nothing, so the model
        // guessed `ms` and got a silent no-op.
        assert!(
            desc.contains("`wait { duration_ms }`") && desc.contains("`ms`"),
            "description must name the wait duration parameter and its aliases"
        );
        // A recurring pace should not be built out of hour-long waits that pin
        // the turn open.
        assert!(
            desc.contains("`Cron` tool"),
            "description must point repeating schedules at Cron"
        );
    }

    #[tokio::test]
    async fn description_defaults_url_opening_to_builtin_browser() {
        let desc = ControlHubTool::new().description().await.unwrap();
        assert!(
            desc.contains("Default URL-opening policy")
                && desc.contains("use `open_builtin`")
                && desc.contains("Do not call `connect`"),
            "description must default display-only URL requests to the built-in browser"
        );
        assert!(
            desc.contains("mode: \"headless\"") && desc.contains("mode: \"default\""),
            "description must mention both browser connect modes"
        );
    }

    #[tokio::test]
    async fn desktop_domain_returns_migration_error() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "desktop",
                "paste",
                &json!({ "text": "hi", "submit": true }),
                &ctx,
            )
            .await
            .expect("migration error is a structured result");
        let payload = results.first().expect("one result").content();
        assert_eq!(payload.get("ok").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            payload
                .get("error")
                .and_then(|v| v.get("code"))
                .and_then(|v| v.as_str()),
            Some("INVALID_PARAMS")
        );
        assert!(payload.to_string().contains("ComputerUse"));
    }

    #[tokio::test]
    async fn browser_connect_headless_requires_existing_test_port() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "browser",
                "connect",
                &json!({ "mode": "headless", "port": 1 }),
                &ctx,
            )
            .await
            .expect("dispatch should succeed and return a structured error");
        let payload: serde_json::Value =
            serde_json::from_value(results[0].content().clone()).unwrap();
        assert_eq!(payload["ok"], serde_json::Value::Bool(false));
        assert_eq!(payload["error"]["code"], "NOT_AVAILABLE");
        let hints = payload["error"]["hints"]
            .as_array()
            .expect("hints should be present");
        assert!(
            hints
                .iter()
                .any(|v| v.as_str().unwrap_or("").contains("headless")),
            "expected headless guidance in hints: {}",
            payload
        );
    }

    #[test]
    fn headless_connect_rejects_non_headless_cdp_endpoint() {
        // The port may be occupied by the user's real logged-in browser;
        // labelling that session "Headless test browser" would invite
        // destructive automation on their real profile.
        for reported in [Some("Chrome/126.0.6478.127"), None] {
            let version = CdpVersionInfo {
                browser: reported.map(str::to_string),
                protocol_version: None,
                web_socket_debugger_url: None,
            };
            let err = ControlHubTool::verify_headless_cdp_browser(&version, 9222)
                .expect_err("non-headless endpoint must be rejected in headless mode");
            assert!(matches!(err.code, ErrorCode::NotAvailable));
            assert!(
                err.message
                    .contains("must never attach to the user's default browser"),
                "error must forbid attaching the user's default browser: {}",
                err.message
            );
            assert!(
                err.hints.iter().any(|h| h.contains("mode: \"default\"")),
                "hints must offer the default interactive-browser mode: {:?}",
                err.hints
            );
        }
    }

    #[test]
    fn headless_connect_accepts_headless_cdp_endpoint() {
        // "Headless" matching must be case-insensitive across Chromium's
        // historical spellings.
        for reported in ["HeadlessChrome/126.0.6478.127", "headlesschrome/1.0"] {
            let version = CdpVersionInfo {
                browser: Some(reported.to_string()),
                protocol_version: None,
                web_socket_debugger_url: None,
            };
            assert!(
                ControlHubTool::verify_headless_cdp_browser(&version, 9222).is_ok(),
                "endpoint '{reported}' must be accepted as headless"
            );
        }
    }

    #[test]
    fn default_connect_hints_point_to_guarded_user_profile_not_raw_debug_port() {
        let hints = ControlHubTool::default_browser_connect_hints(&BrowserKind::Chrome, 9222);
        let joined = hints.join(" | ");
        assert!(
            joined.contains("current real profile")
                && joined.contains("chrome://inspect/#remote-debugging")
                && joined.contains("approve"),
            "hints must guide toward Chrome's guarded real-profile connection: {joined}"
        );
        assert!(
            !joined.contains("--remote-debugging-port"),
            "hints must not teach enabling a raw debug port on the user's everyday browser: {joined}"
        );
    }

    #[test]
    fn edge_connect_hints_use_its_guarded_real_profile_setup() {
        let hints = ControlHubTool::default_browser_connect_hints(&BrowserKind::Edge, 9222);
        let joined = hints.join(" | ");
        assert!(joined.contains("current real profile"), "{joined}");
        assert!(
            joined.contains("edge://inspect/#remote-debugging"),
            "{joined}"
        );
        assert!(joined.contains("approve"), "{joined}");
        assert!(!joined.contains("--remote-debugging-port"), "{joined}");
    }

    #[test]
    fn browser_open_builtin_normalizes_domain_url() {
        assert_eq!(
            ControlHubTool::normalize_builtin_browser_url("example.com", true).unwrap(),
            "https://example.com"
        );
    }

    #[tokio::test]
    async fn browser_open_builtin_rejects_unsupported_scheme() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "browser",
                "open_builtin",
                &json!({ "url": "file:///tmp/demo.html" }),
                &ctx,
            )
            .await
            .expect("dispatch should succeed and return a structured error");
        let payload: serde_json::Value =
            serde_json::from_value(results[0].content().clone()).unwrap();
        assert_eq!(payload["ok"], serde_json::Value::Bool(false));
        assert_eq!(payload["error"]["code"], "INVALID_PARAMS");
    }

    #[tokio::test]
    async fn system_open_url_rejects_unsupported_scheme() {
        let tool = ComputerUseActions::new();
        let ctx = empty_context();
        let results = tool
            .handle_system("open_url", &json!({ "url": "javascript:alert(1)" }), &ctx)
            .await
            .expect("dispatch should succeed and return a structured error");
        let payload: serde_json::Value =
            serde_json::from_value(results[0].content().clone()).unwrap();
        assert_eq!(payload["ok"], serde_json::Value::Bool(false));
        assert_eq!(payload["error"]["code"], "INVALID_PARAMS");
    }

    #[tokio::test]
    async fn system_open_file_returns_not_found_for_missing_path() {
        let tool = ComputerUseActions::new();
        let ctx = empty_context();
        let results = tool
            .handle_system(
                "open_file",
                &json!({ "path": "/definitely/does/not/exist/bitfun-test.xyz" }),
                &ctx,
            )
            .await
            .expect("dispatch should succeed and return a structured error");
        let payload: serde_json::Value =
            serde_json::from_value(results[0].content().clone()).unwrap();
        assert_eq!(payload["ok"], serde_json::Value::Bool(false));
        assert_eq!(payload["error"]["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn meta_capabilities_includes_browser_surface_facts() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("meta", "capabilities", &json!({}), &ctx)
            .await
            .expect("capabilities should succeed");
        let payload = results.first().unwrap().content();

        // schema_version must have been bumped since we added new fields.
        assert_eq!(
            payload.get("schema_version").and_then(|v| v.as_str()),
            Some("1.4"),
            "schema_version must be bumped to 1.4: {payload}"
        );

        // Computer Use availability is reported here because ControlHub stays
        // usable while it is off — the model needs to know before it is told
        // to fall back to desktop control.
        assert!(
            payload
                .get("computer_use")
                .and_then(|c| c.get("available"))
                .and_then(|v| v.as_bool())
                .is_some(),
            "computer_use.available must be reported: {payload}"
        );

        assert!(
            payload
                .get("domains")
                .and_then(|d| d.get("system"))
                .is_none(),
            "system must not be advertised by ControlHub capabilities: {payload}"
        );

        // browser.default_browser key must exist (value may be null on hosts
        // without any installed browser, but the field must be present so
        // the model knows the probe ran).
        assert!(
            payload
                .get("domains")
                .and_then(|d| d.get("browser"))
                .and_then(|b| b.get("cdp_supported"))
                .is_some(),
            "browser.cdp_supported missing: {payload}"
        );
    }

    #[tokio::test]
    async fn system_get_os_info_includes_script_types() {
        let tool = ComputerUseActions::new();
        let ctx = empty_context();
        let results = tool
            .handle_system("get_os_info", &json!({}), &ctx)
            .await
            .expect("get_os_info should succeed");
        let payload = results.first().unwrap().content();
        let script_types = payload
            .get("script_types")
            .and_then(|v| v.as_array())
            .expect("script_types missing from get_os_info");
        assert!(script_types.iter().any(|s| s.as_str() == Some("shell")));
    }

    #[tokio::test]
    async fn system_run_script_rejects_applescript_on_non_mac() {
        // On non-macOS hosts, `applescript` must come back as a structured
        // NOT_AVAILABLE rather than throwing — so the model can branch on
        // `error.code`.
        if cfg!(target_os = "macos") {
            return; // skip on macOS where applescript is genuinely available
        }
        let tool = ComputerUseActions::new();
        let ctx = empty_context();
        let results = tool
            .handle_system(
                "run_script",
                &json!({ "script": "say hi", "script_type": "applescript" }),
                &ctx,
            )
            .await
            .expect("dispatch returns the structured envelope");
        let payload = results.first().unwrap().content();
        assert_eq!(payload["ok"], serde_json::Value::Bool(false));
        assert_eq!(payload["error"]["code"], "NOT_AVAILABLE");
    }

    #[tokio::test]
    async fn system_run_script_unknown_type_lists_valid_options() {
        let tool = ComputerUseActions::new();
        let ctx = empty_context();
        let err = tool
            .handle_system(
                "run_script",
                &json!({ "script": "echo hi", "script_type": "ruby" }),
                &ctx,
            )
            .await
            .expect_err("unknown script_type must be a hard error");
        let msg = err.to_string();
        for must_have in ["applescript", "shell", "powershell", "cmd"] {
            assert!(
                msg.contains(must_have),
                "valid script_type `{must_have}` missing from error message: {msg}"
            );
        }
    }

    #[tokio::test]
    async fn system_run_script_shell_executes_and_captures_stdout() {
        // Real run: confirm the OS-default `shell` script_type resolves to
        // the right interpreter and that we get UTF-8 stdout back. This
        // protects against the historical Windows GBK regression where
        // CJK output became `???`.
        let tool = ComputerUseActions::new();
        let ctx = empty_context();
        let probe = if cfg!(target_os = "windows") {
            // PowerShell prints with the Unicode code page configured above.
            "Write-Output 'hello-bitfun'"
        } else {
            "echo hello-bitfun"
        };
        let results = tool
            .handle_system(
                "run_script",
                &json!({ "script": probe, "script_type": "shell" }),
                &ctx,
            )
            .await
            .expect("shell run_script should succeed");
        let payload = results.first().unwrap().content();
        assert_eq!(
            payload.get("success").and_then(|v| v.as_bool()),
            Some(true),
            "shell run_script payload: {payload}"
        );
        let out = payload.get("output").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            out.contains("hello-bitfun"),
            "expected stdout to contain 'hello-bitfun', got '{out}'"
        );
    }

    #[tokio::test]
    async fn terminal_list_sessions_without_singleton_returns_clean_error() {
        // The TerminalApi singleton is initialized only inside the desktop /
        // server runtimes, so in `cargo test -p bitfun-core` it must surface
        // a structured error rather than panicking.
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let err = tool
            .dispatch("terminal", "list_sessions", &json!({}), &ctx)
            .await
            .expect_err("must fail without TerminalApi singleton");
        let msg = err.to_string();
        assert!(
            msg.contains("TerminalApi") || msg.contains("list_sessions"),
            "expected TerminalApi/list_sessions hint, got: {msg}"
        );
    }
}
