//! ControlHub — unified entry point for all control capabilities.
//!
//! Routes requests by `domain` to the appropriate backend:
//!   desktop  → ComputerUseHost (existing)
//!   browser  → CDP-based browser control (new)
//!   app      → SelfControl (existing front-end service)
//!   terminal → TerminalApi (existing)
//!   system   → OS-level utilities (open_app, run_script, etc.)

use crate::agentic::tools::browser_control::actions::BrowserActions;
use crate::agentic::tools::browser_control::browser_launcher::{
    BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
use crate::agentic::tools::browser_control::cdp_client::CdpClient;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global CDP client singleton — persists across tool invocations.
static CDP_CLIENT: std::sync::OnceLock<Arc<RwLock<Option<CdpClient>>>> = std::sync::OnceLock::new();

fn get_cdp_client_slot() -> Arc<RwLock<Option<CdpClient>>> {
    CDP_CLIENT
        .get_or_init(|| Arc::new(RwLock::new(None)))
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

    async fn dispatch(
        &self,
        domain: &str,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        match domain {
            "desktop" => self.handle_desktop(action, params, context).await,
            "browser" => self.handle_browser(action, params).await,
            "app" => self.handle_app(action, params, context).await,
            "terminal" => self.handle_terminal(action, params, context).await,
            "system" => self.handle_system(action, params, context).await,
            other => Err(BitFunError::tool(format!(
                "Unknown domain: '{}'. Valid domains: desktop, browser, app, terminal, system",
                other
            ))),
        }
    }

    // ── Desktop domain ─────────────────────────────────────────────────

    async fn handle_desktop(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if context.computer_use_host.is_none() {
            return Err(BitFunError::tool(
                "Desktop control is only available in the BitFun desktop app".to_string(),
            ));
        }

        // Reconstruct the input in ComputerUse format and delegate
        let mut cu_input = params.clone();
        if let Value::Object(ref mut map) = cu_input {
            map.insert("action".to_string(), json!(action));
        }

        let cu_tool = super::computer_use_tool::ComputerUseTool::new();
        cu_tool.call_impl(&cu_input, context).await
    }

    // ── Browser domain ─────────────────────────────────────────────────

    async fn handle_browser(&self, action: &str, params: &Value) -> BitFunResult<Vec<ToolResult>> {
        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .unwrap_or(DEFAULT_CDP_PORT);

        match action {
            "connect" => {
                let kind = BrowserLauncher::detect_default_browser()?;
                let launch_result = BrowserLauncher::launch_with_cdp(&kind, port).await?;

                match &launch_result {
                    LaunchResult::AlreadyConnected | LaunchResult::Launched => {
                        let client = CdpClient::connect_to_first_page(port).await?;
                        let version = CdpClient::get_version(port).await?;
                        let cdp_slot = get_cdp_client_slot();
                        let mut slot = cdp_slot.write().await;
                        *slot = Some(client);

                        let result = json!({
                            "success": true,
                            "browser": kind.to_string(),
                            "browser_version": version.browser,
                            "port": port,
                            "status": if matches!(launch_result, LaunchResult::AlreadyConnected) { "already_connected" } else { "launched" },
                        });
                        Ok(vec![ToolResult::ok(
                            result.clone(),
                            Some(format!("Connected to {} on CDP port {}", kind, port)),
                        )])
                    }
                    LaunchResult::LaunchedButCdpNotReady { message, .. } => {
                        Ok(vec![ToolResult::ok(
                            json!({ "success": false, "status": "cdp_not_ready", "message": message }),
                            Some(message.clone()),
                        )])
                    }
                    LaunchResult::BrowserRunningWithoutCdp { instructions, .. } => {
                        Ok(vec![ToolResult::ok(
                            json!({ "success": false, "status": "needs_restart", "instructions": instructions }),
                            Some(instructions.clone()),
                        )])
                    }
                }
            }

            "list_pages" => {
                let pages = CdpClient::list_pages(port).await?;
                let summary: Vec<Value> = pages
                    .iter()
                    .map(|p| {
                        json!({
                            "id": p.id,
                            "title": p.title,
                            "url": p.url,
                            "type": p.page_type,
                        })
                    })
                    .collect();
                Ok(vec![ToolResult::ok(
                    json!({ "pages": summary }),
                    Some(format!("{} page(s) found", pages.len())),
                )])
            }

            "switch_page" => {
                let page_id = params
                    .get("page_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool("switch_page requires 'page_id'".to_string())
                    })?;
                let pages = CdpClient::list_pages(port).await?;
                let page = pages
                    .iter()
                    .find(|p| p.id == page_id)
                    .ok_or_else(|| BitFunError::tool(format!("Page '{}' not found", page_id)))?;
                let ws_url = page
                    .web_socket_debugger_url
                    .as_ref()
                    .ok_or_else(|| BitFunError::tool("Page has no WebSocket URL".to_string()))?;
                let client = CdpClient::connect(ws_url).await?;
                let cdp_slot = get_cdp_client_slot();
                let mut slot = cdp_slot.write().await;
                *slot = Some(client);
                Ok(vec![ToolResult::ok(
                    json!({ "success": true, "page_id": page_id, "title": page.title }),
                    Some(format!("Switched to page: {}", page.title)),
                )])
            }

            _ => {
                let cdp_slot = get_cdp_client_slot();
                let slot = cdp_slot.read().await;
                let client = slot.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "Browser not connected. Use action 'connect' first.".to_string(),
                    )
                })?;
                let actions = BrowserActions::new(client);

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
                        let result = actions.snapshot().await?;
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
                        let text = actions.get_text(selector).await?;
                        Ok(vec![ToolResult::ok(
                            json!({ "text": text }),
                            Some(text),
                        )])
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
                        let result = actions.evaluate(expression).await?;
                        let display = result
                            .get("result")
                            .and_then(|r| r.get("value"))
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| result.to_string());
                        Ok(vec![ToolResult::ok(result, Some(display))])
                    }
                    "close" => {
                        let result = actions.close_page().await?;
                        Ok(vec![ToolResult::ok(result, Some("Page closed".to_string()))])
                    }
                    other => Err(BitFunError::tool(format!(
                        "Unknown browser action: '{}'. Valid: connect, navigate, snapshot, click, fill, type, select, press_key, scroll, wait, get_text, get_url, get_title, screenshot, evaluate, close, list_pages, switch_page",
                        other
                    ))),
                }
            }
        }
    }

    // ── App domain (SelfControl) ───────────────────────────────────────

    async fn handle_app(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let mut sc_input = params.clone();
        if let Value::Object(ref mut map) = sc_input {
            map.insert("action".to_string(), json!(action));
        }
        let sc_tool = super::self_control_tool::SelfControlTool::new();
        sc_tool.call_impl(&sc_input, context).await
    }

    // ── Terminal domain ────────────────────────────────────────────────

    async fn handle_terminal(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let terminal_session_id = params
            .get("terminal_session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool("Terminal actions require 'terminal_session_id'".to_string())
            })?;

        let mut input = params.clone();
        if let Value::Object(ref mut map) = input {
            map.insert("action".to_string(), json!(action));
            map.insert(
                "terminal_session_id".to_string(),
                json!(terminal_session_id),
            );
        }

        let tool = super::terminal_control_tool::TerminalControlTool::new();
        tool.call_impl(&input, context).await
    }

    /// Returns the platform-specific command and args to open an application.
    fn platform_open_command(app_name: &str) -> (String, Vec<String>) {
        #[cfg(target_os = "macos")]
        {
            (
                "open".to_string(),
                vec!["-a".to_string(), app_name.to_string()],
            )
        }
        #[cfg(target_os = "windows")]
        {
            (
                "cmd".to_string(),
                vec![
                    "/C".to_string(),
                    "start".to_string(),
                    "".to_string(),
                    app_name.to_string(),
                ],
            )
        }
        #[cfg(target_os = "linux")]
        {
            ("xdg-open".to_string(), vec![app_name.to_string()])
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            ("open".to_string(), vec![app_name.to_string()])
        }
    }

    // ── System domain ──────────────────────────────────────────────────

    async fn handle_system(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        match action {
            "open_app" => {
                let app_name = params
                    .get("app_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| BitFunError::tool("open_app requires 'app_name'".to_string()))?;
                // Delegate to ComputerUse's open_app if host is available
                let cu_input = json!({ "action": "open_app", "app_name": app_name });
                if context.computer_use_host.is_some() {
                    return self.handle_desktop("open_app", &cu_input, context).await;
                }
                // Fallback: use platform-specific shell command
                let (cmd, args) = Self::platform_open_command(app_name);
                let output = std::process::Command::new(&cmd)
                    .args(&args)
                    .output()
                    .map_err(|e| BitFunError::tool(format!("Failed to open app: {}", e)))?;
                if output.status.success() {
                    Ok(vec![ToolResult::ok(
                        json!({ "success": true, "app": app_name }),
                        Some(format!("Opened {}", app_name)),
                    )])
                } else {
                    let err = String::from_utf8_lossy(&output.stderr);
                    Err(BitFunError::tool(format!(
                        "Failed to open {}: {}",
                        app_name, err
                    )))
                }
            }
            "run_script" => {
                let script = params
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| BitFunError::tool("run_script requires 'script'".to_string()))?;
                let script_type = params
                    .get("script_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("applescript");

                match script_type {
                    "applescript" => {
                        #[cfg(target_os = "macos")]
                        {
                            let output = std::process::Command::new("/usr/bin/osascript")
                                .args(["-e", script])
                                .output()
                                .map_err(|e| {
                                    BitFunError::tool(format!("Failed to run AppleScript: {}", e))
                                })?;
                            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                            if output.status.success() {
                                Ok(vec![ToolResult::ok(
                                    json!({ "success": true, "output": stdout }),
                                    Some(if stdout.is_empty() {
                                        "Script executed".to_string()
                                    } else {
                                        stdout
                                    }),
                                )])
                            } else {
                                Err(BitFunError::tool(format!("AppleScript error: {}", stderr)))
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            let _ = script;
                            Err(BitFunError::tool(
                                "AppleScript is only available on macOS".to_string(),
                            ))
                        }
                    }
                    "shell" => {
                        #[cfg(target_os = "windows")]
                        let output = std::process::Command::new("cmd")
                            .args(["/C", script])
                            .output()
                            .map_err(|e| {
                                BitFunError::tool(format!("Failed to run script: {}", e))
                            })?;
                        #[cfg(not(target_os = "windows"))]
                        let output = std::process::Command::new("sh")
                            .args(["-c", script])
                            .output()
                            .map_err(|e| {
                                BitFunError::tool(format!("Failed to run script: {}", e))
                            })?;
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        if output.status.success() {
                            Ok(vec![ToolResult::ok(
                                json!({ "success": true, "output": stdout }),
                                Some(if stdout.is_empty() {
                                    "Script executed".to_string()
                                } else {
                                    stdout
                                }),
                            )])
                        } else {
                            Err(BitFunError::tool(format!("Script error: {}", stderr)))
                        }
                    }
                    other => Err(BitFunError::tool(format!(
                        "Unknown script_type: '{}'. Valid: applescript, shell",
                        other
                    ))),
                }
            }
            "get_os_info" => {
                let os = std::env::consts::OS;
                let arch = std::env::consts::ARCH;
                Ok(vec![ToolResult::ok(
                    json!({ "os": os, "arch": arch }),
                    Some(format!("{} ({})", os, arch)),
                )])
            }
            other => Err(BitFunError::tool(format!(
                "Unknown system action: '{}'. Valid: open_app, run_script, get_os_info",
                other
            ))),
        }
    }
}

#[async_trait]
impl Tool for ControlHubTool {
    fn name(&self) -> &str {
        "ControlHub"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Unified control hub for the Agentic OS. All control operations go through this single tool.

## Domains and Actions

### domain: "browser" — Control the user's default browser (Chrome/Edge/etc.) via CDP
Preserves user login sessions, cookies, and extensions.
- **connect**: Connect to or launch the browser with CDP debug port. Optional: `port` (default 9222).
- **navigate**: Go to a URL. Requires: `url`.
- **snapshot**: Get interactive elements on the page (returns refs like @e1, @e2...).
- **click**: Click element by CSS selector or @ref. Requires: `selector`.
- **fill**: Clear and type into an input. Requires: `selector`, `value`.
- **type**: Type text at the currently focused element. Requires: `text`.
- **select**: Select a dropdown option. Requires: `selector`, `option_text`.
- **press_key**: Press a keyboard key. Requires: `key`.
- **scroll**: Scroll the page. Optional: `direction` (up/down/top/bottom), `amount`.
- **wait**: Wait for duration or condition. Optional: `duration_ms`, `condition`.
- **get_text**: Get element text. Requires: `selector`.
- **get_url**: Get current page URL.
- **get_title**: Get current page title.
- **screenshot**: Capture the page as JPEG.
- **evaluate**: Run JavaScript. Requires: `expression`.
- **close**: Close the current page.
- **list_pages**: List all open browser tabs.
- **switch_page**: Switch to a different tab. Requires: `page_id`.

### domain: "desktop" — Desktop automation (screenshot, keyboard, mouse, accessibility)
Only available in BitFun desktop app.
- **screenshot**, **click**, **click_element**, **mouse_move**, **scroll**, **drag**,
  **key_chord**, **type_text**, **wait**, **locate**, **open_app**, **run_apple_script**,
  **move_to_text**, **pointer_move_rel**
- Uses the same parameters as the ComputerUse tool.

### domain: "app" — Control BitFun's own GUI
- **get_page_state**, **click**, **click_by_text**, **input**, **scroll**,
  **open_scene**, **open_settings_tab**, **set_config**, **get_config**,
  **list_models**, **set_default_model**, **execute_task**, **select_option**,
  **wait**, **press_key**, **read_text**, **delete_model**
- Uses the same parameters as the SelfControl tool.

### domain: "terminal" — Control terminal sessions
- **kill**: Close a terminal session. Requires: `terminal_session_id`.
- **interrupt**: Send SIGINT. Requires: `terminal_session_id`.

### domain: "system" — OS-level operations
- **open_app**: Launch an application. Requires: `app_name`.
- **run_script**: Run a script (e.g., AppleScript). Requires: `script`. Optional: `script_type`.
- **get_os_info**: Get OS and architecture info.

## Workflow Tips
1. For browser tasks: always `connect` first, then `navigate`, then `snapshot` to discover elements.
2. After any page change (click, navigate), take a fresh `snapshot` before interacting.
3. Use `@eN` refs from `snapshot` results for reliable element targeting.
4. For cross-domain workflows (e.g., browser data → desktop paste), call actions sequentially."#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "enum": ["browser", "desktop", "app", "terminal", "system"],
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
        let domain = input
            .get("domain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("Missing 'domain'".to_string()))?;
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("Missing 'action'".to_string()))?;
        let params = input.get("params").cloned().unwrap_or(json!({}));

        self.dispatch(domain, action, &params, context).await
    }
}
