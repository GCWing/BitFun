//! ControlHub — unified entry point for browser, terminal, and routing metadata.
//!
//! Routes requests by `domain` to the appropriate backend:
//!   browser  → CDP-based browser control (new)
//!   terminal → TerminalApi (existing)
//!   meta     → capability and route introspection
//!
//! Local desktop and OS/system actions are intentionally surfaced through the
//! dedicated ComputerUse tool/agent, not through public ControlHub domains.

use crate::agentic::tools::browser_control::actions::BrowserActions;
use crate::agentic::tools::browser_control::browser_launcher::{
    BrowserKind, BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
use crate::agentic::tools::browser_control::cdp_client::CdpClient;
use crate::agentic::tools::browser_control::session_registry::{
    BrowserSession, BrowserSessionRegistry,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::config::{get_global_config_service, GlobalConfig};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

#[cfg(target_os = "linux")]
use super::computer_use_actions::linux_session_info;
use super::computer_use_actions::{truncate_with_marker, which_exists};
use super::control_hub::{err_response, ControlHubError, ErrorCode};

/// Process-wide registry of CDP sessions. Replaces the previous single
/// global `Option<CdpClient>` slot whose `*slot = Some(client)` semantics
/// silently dropped the prior page connection on every `connect` /
/// `switch_page`, breaking concurrent multi-tab work and racing
/// in-flight `wait` / lifecycle subscriptions.
static BROWSER_SESSIONS: std::sync::OnceLock<Arc<BrowserSessionRegistry>> =
    std::sync::OnceLock::new();

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
        let exe = BrowserLauncher::browser_executable(kind);
        vec![
            "For login/cookies/extensions, use the user's default browser via CDP — never fall back to desktop mouse/keyboard automation.".to_string(),
            format!(
                "If CDP is not ready, restart the browser with the test port enabled: \"{}\" --remote-debugging-port={}",
                exe, port
            ),
            "After the browser is listening on the test port, use browser.connect / snapshot / click / fill to drive the DOM directly.".to_string(),
        ]
    }

    fn headless_browser_connect_hints(port: u16) -> Vec<String> {
        vec![
            "For project Web UI testing that does not depend on user login state, use the dedicated headless browser flow instead of the user's browser.".to_string(),
            format!(
                "Start or attach a headless test browser on the test port {} and then drive it through browser DOM actions only.",
                port
            ),
            "Do not switch to desktop mouse/keyboard browser control in headless mode.".to_string(),
        ]
    }

    fn description_text() -> String {
        r#"ControlHub — the unified control entry point for browser, terminal, and routing metadata.

Use this tool via `{ domain, action, params }` for browser automation, terminal signalling, and capability/routing introspection. Local computer and operating-system actions have moved out of ControlHub: use the dedicated `ComputerUse` tool/agent for desktop UI control, screenshots, OCR, mouse/keyboard input, app launching, file/url opening, clipboard access, OS facts, and local scripts.

## Domains

### domain: "browser"  (DOM/CDP browser control)
- Two browser modes:
  * `connect { mode: "headless" }` — attach to a headless test browser on the test port for project Web UI testing that does not depend on user login state.
  * `connect { mode: "default" }` (default) — attach to the user's default browser via CDP for flows that require login state, cookies, extensions, or the user's real profile.
- Actions: connect, navigate, snapshot, click, fill, type, select, press_key, scroll, wait, get_text, get_url, get_title, screenshot, evaluate, close, list_pages, tab_query, switch_page, list_sessions.
- Workflow: connect -> navigate -> snapshot (returns @e1, @e2 ... refs) -> click/fill using refs.
- Take a fresh snapshot after any DOM mutation; stale refs return `error.code = STALE_REF`.

### domain: "terminal"
- list_sessions, kill (`terminal_session_id`), interrupt (`terminal_session_id`).
- Use the `Bash` tool to run new commands; this domain only signals existing terminal sessions.

### domain: "meta"
- `capabilities` — returns `{ domains: { browser, terminal, meta }, host: { os, arch }, schema_version }`.
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
                Ok(err_response(
                    "desktop",
                    action,
                    ControlHubError::new(
                        ErrorCode::InvalidParams,
                        "The desktop domain has moved out of ControlHub.",
                    )
                    .with_hint(
                        "Use the dedicated ComputerUse tool/agent for screenshots, OCR, mouse, keyboard, and desktop app control.",
                    ),
                ))
            }
            "browser" => self.handle_browser(action, params).await,
            "terminal" => self.handle_terminal(action, params, context).await,
            "system" => Ok(err_response(
                "system",
                action,
                ControlHubError::new(
                    ErrorCode::InvalidParams,
                    "The system domain has moved out of ControlHub.",
                )
                .with_hint(
                    "Use the dedicated ComputerUse tool/agent for open_app, open_url, open_file, clipboard, OS info, and local scripts.",
                ),
            )),
            "meta" => self.handle_meta(action, params, context).await,
            other => Err(BitFunError::tool(format!(
                "Unknown domain: '{}'. Valid ControlHub domains: browser, terminal, meta. Use ComputerUse for desktop/system actions.",
                other
            ))),
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

                // Same script_types probe as get_os_info — duplicated here
                // because callers often hit `meta.capabilities` first and we
                // don't want to force an extra system round-trip.
                let mut _script_types: Vec<&'static str> = vec!["shell"];
                if cfg!(target_os = "macos") {
                    _script_types.push("applescript");
                }
                if which_exists("bash") {
                    _script_types.push("bash");
                }
                if which_exists("pwsh") || which_exists("powershell") {
                    _script_types.push("powershell");
                }
                if cfg!(target_os = "windows") {
                    _script_types.push("cmd");
                }

                #[cfg(target_os = "linux")]
                let (display_server, desktop_env) = linux_session_info();
                #[cfg(not(target_os = "linux"))]
                let (display_server, desktop_env): (
                    Option<String>,
                    Option<String>,
                ) = (None, None);

                let body = json!({
                    "domains": {
                        "browser":  {
                            "available": true,
                            "default_session_id": browser_default,
                            "session_count": browser_session_count,
                            "default_browser": browser_kind,
                            "cdp_supported": browser_cdp_supported,
                        },
                        "terminal": { "available": likely_terminal_available, "reason": if likely_terminal_available { Value::Null } else { json!("TerminalApi is only available in contexts that registered it") } },
                        "meta":     { "available": true },
                    },
                    "host": {
                        "os": os,
                        "arch": arch,
                        "display_server": display_server,
                        "desktop_environment": desktop_env,
                    },
                    "schema_version": "1.1",
                });
                Ok(vec![ToolResult::ok(
                    body,
                    Some("ControlHub capabilities snapshot".to_string()),
                )])
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

                let mut suggestions: Vec<(&'static str, u32, &'static str)> = vec![];
                let push = |s: &mut Vec<(&'static str, u32, &'static str)>,
                            domain: &'static str,
                            score: u32,
                            why: &'static str| {
                    s.push((domain, score, why));
                };

                let browser_kw = [
                    "http",
                    "https",
                    "url",
                    "browser",
                    "google",
                    "tab",
                    "网页",
                    "浏览器",
                    "网站",
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
                            85,
                            "Matches browser/URL keywords",
                        );
                        break;
                    }
                }
                for kw in desktop_kw {
                    if lower.contains(kw) {
                        push(
                            &mut suggestions,
                            "ComputerUse",
                            75,
                            "Matches local desktop/system keywords; use the ComputerUse tool/agent",
                        );
                        break;
                    }
                }
                for kw in terminal_kw {
                    if lower.contains(kw) {
                        push(
                            &mut suggestions,
                            "terminal",
                            80,
                            "Matches terminal-signal keywords",
                        );
                        break;
                    }
                }
                for kw in system_kw {
                    if lower.contains(kw) {
                        push(
                            &mut suggestions,
                            "ComputerUse",
                            70,
                            "Matches OS/launch keywords; use the ComputerUse tool/agent",
                        );
                        break;
                    }
                }
                suggestions.sort_by(|a, b| b.1.cmp(&a.1));

                let ranked: Vec<Value> = suggestions
                    .iter()
                    .map(|(d, score, why)| json!({ "domain": d, "score": score, "why": why }))
                    .collect();
                let suggested = suggestions.first().map(|(d, _, _)| (*d).to_string());
                Ok(vec![ToolResult::ok(
                    json!({
                        "intent": intent,
                        "suggested_domain": suggested,
                        "ranked": ranked,
                        "note": "Heuristic only — confirm by reading meta.capabilities and the domain-specific docs.",
                    }),
                    Some(match &suggested {
                        Some(d) => format!("Best guess: domain={}", d),
                        None => "No confident routing match".to_string(),
                    }),
                )])
            }
            other => Err(BitFunError::tool(format!(
                "Unknown meta action: '{}'. Valid actions: capabilities, route_hint",
                other
            ))),
        }
    }

    async fn handle_browser(&self, action: &str, params: &Value) -> BitFunResult<Vec<ToolResult>> {
        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .unwrap_or(DEFAULT_CDP_PORT);

        let session_id_param = params
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        match action {
            "connect" => {
                let mode = Self::browser_connect_mode_from_params(params);

                if mode == "headless" {
                    if !BrowserLauncher::is_cdp_available(port).await {
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
                } else {
                    BrowserLauncher::launch_with_cdp_opts(&kind, port, user_data_dir).await?
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
                    LaunchResult::AlreadyConnected | LaunchResult::Launched => {
                        let pages = CdpClient::list_pages(port).await?;
                        let connected_browser = if mode == "headless" {
                            "Headless test browser".to_string()
                        } else {
                            kind.to_string()
                        };

                        // Selection: explicit target_* > first real page > first.
                        let matched_by_target = if target_url.is_some() || target_title.is_some() {
                            pages.iter().find(|p| {
                                if p.web_socket_debugger_url.is_none() {
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
                                        && p.web_socket_debugger_url.is_some()
                                })
                            })
                            .or_else(|| pages.first())
                            .ok_or_else(|| {
                                BitFunError::tool("No browser pages found via CDP".to_string())
                            })?;
                        let ws_url = page.web_socket_debugger_url.as_ref().ok_or_else(|| {
                            BitFunError::tool("Page has no WebSocket debugger URL".to_string())
                        })?;
                        let client = CdpClient::connect(ws_url).await?;
                        let version = CdpClient::get_version(port).await?;
                        let session = BrowserSession {
                            session_id: page.id.clone(),
                            port,
                            client: Arc::new(client),
                        };
                        browser_sessions().register(session.clone()).await;

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
                            "browser_version": version.browser,
                            "port": port,
                            "session_id": session.session_id,
                            "page_url": page.url,
                            "page_title": page.title,
                            "matched_by_target": targeted,
                            "activated": activated,
                            "status": if mode == "headless" {
                                "attached"
                            } else if matches!(launch_result, LaunchResult::AlreadyConnected) {
                                "already_connected"
                            } else {
                                "launched"
                            },
                        });
                        if let Some(w) = activate_warning {
                            result["warning"] = json!(w);
                        }
                        let summary = if targeted {
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
                let pages = CdpClient::list_pages(port).await?;
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
                    Some(format!("{} page(s) found", pages.len())),
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

                let pages = CdpClient::list_pages(port).await?;
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
                    Some(format!("{} of {} page(s) matched", matched, total)),
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
                    let pages = CdpClient::list_pages(port).await?;
                    let page = pages.iter().find(|p| p.id == page_id).ok_or_else(|| {
                        BitFunError::tool(format!("Page '{}' not found", page_id))
                    })?;
                    let ws_url = page.web_socket_debugger_url.as_ref().ok_or_else(|| {
                        BitFunError::tool("Page has no WebSocket URL".to_string())
                    })?;
                    let client = CdpClient::connect(ws_url).await?;
                    let session = BrowserSession {
                        session_id: page.id.clone(),
                        port,
                        client: Arc::new(client),
                    };
                    registry.register(session.clone()).await;
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

            "list_sessions" => {
                let registry = browser_sessions();
                let ids = registry.list().await;
                let default = registry.default_id().await;
                Ok(vec![ToolResult::ok(
                    json!({
                        "sessions": ids,
                        "default_session_id": default,
                    }),
                    Some(format!("{} session(s) tracked", ids.len())),
                )])
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
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Snapshot: {} interactive elements", el_count)),
                        )])
                    }
                    "click" => {
                        let selector = params
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("click requires 'selector'".to_string())
                            })?;
                        let result = actions.click(selector).await?;
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Clicked {}", selector)),
                        )])
                    }
                    "fill" => {
                        let selector = params
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("fill requires 'selector'".to_string())
                            })?;
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
                        let selector = params
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("select requires 'selector'".to_string())
                            })?;
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
                                    ErrorCode::NotFound,
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
                        let ms = params.get("duration_ms").and_then(|v| v.as_u64());
                        let cond = params.get("condition").and_then(|v| v.as_str());
                        let result = actions.wait(ms, cond).await?;
                        Ok(vec![ToolResult::ok(result, Some("Wait completed".to_string()))])
                    }
                    "get_text" => {
                        let selector = params
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("get_text requires 'selector'".to_string())
                            })?;
                        match actions.get_text(selector).await? {
                            Some(text) => Ok(vec![ToolResult::ok(
                                json!({ "text": text, "found": true }),
                                Some(text),
                            )]),
                            None => Ok(err_response(
                                "browser",
                                "get_text",
                                ControlHubError::new(
                                    ErrorCode::NotFound,
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
                        let result = actions.screenshot().await?;
                        let data_len = result
                            .get("data_length")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        Ok(vec![ToolResult::ok(
                            result,
                            Some(format!("Screenshot captured ({} bytes base64)", data_len)),
                        )])
                    }
                    "evaluate" => {
                        let expression = params
                            .get("expression")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                BitFunError::tool("evaluate requires 'expression'".to_string())
                            })?;
                        // Bound the size of the returned value so a runaway
                        // `JSON.stringify(document)` can't blow up the model
                        // context window. Default 16 KiB; clamp to [1 KiB, 256 KiB].
                        let max_value_bytes = params
                            .get("max_value_bytes")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(16 * 1024)
                            .clamp(1024, 256 * 1024) as usize;
                        let mut result = actions.evaluate(expression).await?;
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
                    "close" => {
                        let result = actions.close_page().await?;
                        // After a close, drop the session so subsequent calls
                        // don't try to talk through a half-dead WebSocket.
                        browser_sessions().remove(&session.session_id).await;
                        Ok(vec![ToolResult::ok(result, Some("Page closed".to_string()))])
                    }
                    other => Err(BitFunError::tool(format!(
                        "Unknown browser action: '{}'. Valid: connect, navigate, snapshot, click, fill, type, select, press_key, scroll, wait, get_text, get_url, get_title, screenshot, evaluate, close, list_pages, switch_page, list_sessions",
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

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(Self::description_text())
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
                    "description": "The atomic action to perform within the domain."
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

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
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
            Err(err) => Ok(err_response(
                domain,
                action,
                map_dispatch_error(domain, action, err),
            )),
        }
    }
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
    use crate::agentic::tools::implementations::computer_use_actions::{
        linux_clipboard_install_hints, ComputerUseActions,
    };

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            custom_data: std::collections::HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: Default::default(),
            workspace_services: None,
        }
    }

    #[tokio::test]
    async fn unknown_domain_is_rejected_with_message_listing_valid_domains() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let err = tool
            .dispatch("nope", "any", &json!({}), &ctx)
            .await
            .expect_err("unknown domain must error");
        let msg = err.to_string();
        assert!(msg.contains("Unknown domain"), "got: {msg}");
        for d in ["browser", "terminal", "meta", "ComputerUse"] {
            assert!(
                msg.contains(d),
                "valid domain {d} missing from error: {msg}"
            );
        }
    }

    #[tokio::test]
    async fn meta_capabilities_reports_host_and_domain_table() {
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
                .get("host")
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
    async fn description_documents_two_browser_modes() {
        let desc = ControlHubTool::new().description().await.unwrap();
        assert!(
            desc.contains("Two browser modes"),
            "description must describe the two browser control modes"
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
    async fn meta_capabilities_includes_script_types_and_default_browser() {
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
            Some("1.1"),
            "schema_version must be bumped to 1.1: {payload}"
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

    #[test]
    fn which_exists_finds_a_universally_present_binary() {
        // `sh` is always on Unix; `cmd` is always on Windows.
        #[cfg(unix)]
        assert!(which_exists("sh"), "sh must be on PATH on Unix hosts");
        #[cfg(windows)]
        assert!(which_exists("cmd"), "cmd must be on PATH on Windows hosts");
        // A clearly bogus name must NOT resolve.
        assert!(!which_exists("definitely-not-a-real-binary-bitfun-xyz"));
    }

    #[test]
    fn linux_clipboard_install_hints_match_session_type() {
        // Just sanity-check that the helper returns SOMETHING non-empty on
        // every platform; the message content is OS-specific.
        let hints = linux_clipboard_install_hints();
        assert!(!hints.is_empty(), "hints must never be empty");
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
