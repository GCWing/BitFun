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
    BrowserKind, BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
use crate::agentic::tools::browser_control::cdp_client::CdpClient;
use crate::agentic::tools::browser_control::session_registry::{
    BrowserSession, BrowserSessionRegistry,
};
use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::computer_use_host::{
    AppClickParams, AppSelector, AppWaitPredicate, ClickTarget, ComputerUseForegroundApplication,
    ComputerUseHostRef, InteractiveClickParams, InteractiveScrollParams, InteractiveTypeTextParams,
    InteractiveViewOpts, VisualClickParams, VisualMarkViewOpts,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::AIConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
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

/// Per-PID consecutive-failure tracker for the AX-first `app_*` actions.
/// Key = target PID, value = `(target_signature, before_digest, count)`.
/// When the same `(action,target)` lands on an unchanged digest twice in a
/// row the dispatcher injects an `app_state.loop_warning` so the model is
/// forced off the failing path on its **next** turn (`/Screenshot policy/
/// Mandatory screenshot moments` in `claw_mode.md`).
static APP_LOOP_TRACKER: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<i32, (String, String, u32)>>,
> = std::sync::OnceLock::new();

fn loop_tracker_observe(
    pid: Option<i32>,
    action: &str,
    target_sig: &str,
    before_digest: &str,
    after_digest: &str,
) -> Option<String> {
    let pid = pid?;
    // A digest change means the action mutated the tree — that is real
    // progress and resets the streak even if the model picks the same
    // target name on purpose (e.g. clicking "Next" repeatedly).
    let progressed = before_digest != after_digest;
    let sig = format!("{action}:{target_sig}");
    let mut guard = APP_LOOP_TRACKER
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .ok()?;
    let entry = guard
        .entry(pid)
        .or_insert_with(|| (String::new(), String::new(), 0));
    if progressed {
        *entry = (sig, after_digest.to_string(), 1);
        return None;
    }
    if entry.0 == sig && entry.1 == before_digest {
        entry.2 = entry.2.saturating_add(1);
    } else {
        *entry = (sig, before_digest.to_string(), 1);
    }
    if entry.2 >= 2 {
        Some(format!(
            "Detected {} consecutive `{}` calls on the same target ({}) without any AX tree mutation (digest unchanged). The target is almost certainly invisible / disabled / in a Canvas-WebGL surface that AX cannot describe. NEXT TURN you MUST: (1) run `desktop.screenshot {{ screenshot_window: false }}` to see the full display, (2) switch tactic — different `node_idx`, different `ocr_text` needle, or a keyboard shortcut. Do NOT retry this same target a third time.",
            entry.2, action, target_sig
        ))
    } else {
        None
    }
}

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

    fn desktop_browser_guard_error(
        action: &str,
        foreground: Option<&ComputerUseForegroundApplication>,
    ) -> ControlHubError {
        let app_name = foreground
            .and_then(|app| app.name.as_deref())
            .unwrap_or("a web browser");
        ControlHubError::new(
            ErrorCode::GuardRejected,
            format!(
                "desktop.{} is blocked while {} is frontmost. Use ControlHub domain=\"browser\" for all browser interaction; desktop mouse/keyboard browser control is forbidden.",
                action, app_name
            ),
        )
        .with_hints([
            "Use browser.connect to attach via the test port, then drive the page with snapshot/click/fill/press_key",
            "For login/cookies/extensions, guide the user to start their default browser with the test port enabled before calling browser.connect",
            "For isolated project Web UI testing, use the headless browser flow instead of desktop automation",
        ])
    }

    fn is_probably_browser_app(foreground: &ComputerUseForegroundApplication) -> bool {
        let name = foreground.name.as_deref().unwrap_or("").to_ascii_lowercase();
        let bundle = foreground
            .bundle_id
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();

        const NAME_HINTS: &[&str] = &[
            "chrome",
            "chromium",
            "edge",
            "brave",
            "arc",
            "firefox",
            "safari",
            "browser",
            "浏览器",
        ];
        const BUNDLE_HINTS: &[&str] = &[
            "chrome",
            "chromium",
            "edge",
            "brave",
            "arc",
            "firefox",
            "safari",
            "browser",
        ];

        NAME_HINTS.iter().any(|hint| name.contains(hint))
            || BUNDLE_HINTS.iter().any(|hint| bundle.contains(hint))
    }

    async fn desktop_action_targets_browser(
        &self,
        action: &str,
        context: &ToolUseContext,
    ) -> Option<ControlHubError> {
        let guarded_actions = [
            "click",
            "click_target",
            "click_element",
            "move_to_target",
            "mouse_move",
            "pointer_move_rel",
            "scroll",
            "drag",
            "key_chord",
            "type_text",
            "paste",
            "locate",
            "move_to_text",
        ];
        if !guarded_actions.contains(&action) {
            return None;
        }
        let host = context.computer_use_host.as_ref()?;
        let snapshot = host.computer_use_session_snapshot().await;
        let foreground = snapshot.foreground_application.as_ref()?;
        if Self::is_probably_browser_app(foreground) {
            return Some(Self::desktop_browser_guard_error(action, Some(foreground)));
        }
        None
    }

    async fn desktop_domain_enabled() -> bool {
        if !computer_use_desktop_available() {
            return false;
        }
        let Ok(service) = GlobalConfigManager::get_service().await else {
            return false;
        };
        let ai: AIConfig = service.get_config(Some("ai")).await.unwrap_or_default();
        ai.computer_use_enabled
    }

    fn description_text(desktop_enabled: bool) -> String {
        let desktop_domain_doc = if desktop_enabled {
            r#"### domain: "desktop"  (Computer Use — only available in the BitFun desktop app)

#### desktop (AX-first, recommended for third-party apps)
- New Codex-style flow that targets a specific application by name / bundle
  id / pid and drives it through its Accessibility (AX) tree instead of the
  global mouse + screenshot loop. Strongly preferred whenever:
  * you need to drive an app that is NOT in the user's foreground, OR
  * you must not steal the user's mouse / keyboard focus, OR
  * the target widget has a stable AX role / title / identifier (most native
    macOS / AppKit / Catalyst / SwiftUI / Electron-with-AX-on apps qualify).
- Capability gating (read first, ALWAYS): `meta.capabilities` returns
  `domains.desktop.supports_ax_tree`, `domains.desktop.supports_background_input`,
  `domains.desktop.supports_interactive_view`, and
  `domains.desktop.supports_visual_mark_view`.
  AX tree and background input both `false` → the host cannot do AX-first yet;
  fall back to the legacy screenshot/click flow below. Background input
  `false` while AX tree `true` → AX *reads* work but writes will steal focus;
  tell the user.
- Actions (all under `domain: "desktop"`):
  * `list_apps { include_hidden? }` → ranked `[{ name, bundle_id?, pid,
    is_running, last_used_ms?, launch_count? }]`. Use this to resolve a
    fuzzy user phrase ("微信" / "WeChat" / "Cursor") to a concrete
    `AppSelector` before any other AX call.
  * `get_app_state { app: <AppSelector>, max_depth?, focus_window_only? }`
    → `{ app, window_title?, tree_text, nodes:[AxNode], digest, captured_at_ms }`.
    `tree_text` is the human-readable indent dump (Codex parity); `nodes` is
    the structured array with stable `idx` you pass to subsequent actions.
    `digest` is a sha1 of the tree — use it to detect "did anything change?"
    cheaply without re-diffing.
  * `app_click { app, target: { kind:"node_idx", idx } | { kind:"image_xy", x, y, screenshot_id? } | { kind:"image_grid", x0, y0, width, height, rows, cols, row, col, intersections?, screenshot_id? } | { kind:"visual_grid", rows, cols, row, col, intersections? } | { kind:"screen_xy", x, y },
                 click_count?, mouse_button?, modifier_keys?, wait_ms_after? }` → returns the
    fresh `AppStateSnapshot` after the click. Prefer `node_idx` over
    coordinate targets whenever the target appears in `nodes`. For Canvas /
    SVG / WebGL/custom-drawn surfaces, prefer `image_xy`: x/y are pixels in
    the screenshot attached to the latest `get_app_state` / `app_click`.
    Always pass `screenshot_id` from `app_state.screenshot_meta` when present
    so the host maps against the exact frame you clicked from.
    For board/grid/canvas controls, prefer `image_grid` over raw `image_xy`:
    specify the board rectangle in screenshot pixels and a zero-based
    `row`/`col`; set `intersections:true` for Go/Gomoku-style line
    intersections and `false`/omit it for cell centers.
    If the grid rectangle is not known, use `visual_grid`: the host captures
    the app, detects the regular visual grid from pixels, then clicks the
    requested zero-based row/col using the same captured coordinate basis.
    For games / animated WebViews, pass `wait_ms_after` (e.g. 300–600) so the
    returned screenshot captures the settled board.
  * `build_visual_mark_view { app, opts?: { max_points?, region?, include_grid? } }`
    → returns a numbered screenshot grid for arbitrary visual targets that
    AX/OCR cannot name (Canvas, games, maps, drawings, icon-only panels).
    Use this after `get_app_state` / `build_interactive_view` does not expose
    the target. Pass `region` in screenshot pixels to refine into a smaller
    area on the next attempt.
  * `visual_click { app, i, before_view_digest?, click_count?, mouse_button?, wait_ms_after?, return_view? }`
    → clicks the numbered visual mark using the exact screenshot coordinate
    basis from the marked view, then returns fresh app state.
  * `app_type_text { app, text, focus?: ClickTarget }` — focuses the optional
    target first, then types. Honors IME / emoji / CJK via paste-style
    injection where the host supports it.
  * `app_scroll { app, focus?: ClickTarget, dx, dy }` — pixel deltas inside
    the focused scroll container; use negative `dy` to scroll content up.
  * `app_key_chord { app, keys:["command","shift","p"], focus_idx? }` — sends
    a chord to the app *without* surfacing a global key event; modifier
    names match the legacy `key_chord` (command/control/option|alt/shift).
  * `app_wait_for { app, predicate, timeout_ms?, poll_ms? }` where
    `predicate` is one of `{ kind:"digest_changed", prev_digest }`,
    `{ kind:"title_contains", needle }`,
    `{ kind:"role_enabled", role, title? }`, `{ kind:"node_enabled", idx }`.
    This is the AX equivalent of the `wait` + re-screenshot loop and is
    REQUIRED between actions when the next step depends on a state change.
- Selector shape: `{ pid }` is most precise (always survives renames);
  `{ bundle_id }` is next-best (survives localization); `{ name }` matches
  on the localized window/app name. Combine fields and the host picks the
  strongest match. Unresolved selector → `error.code = APP_NOT_FOUND`.
- Stale node refs (e.g. you cached `idx=42` from a snapshot, then the app
  re-rendered) → `error.code = AX_NODE_STALE`. Always re-call
  `get_app_state` and re-resolve by role/title/identifier — never carry an
  `idx` across user-visible mutations without `app_wait_for`.
- If `supports_background_input` is `false` and the host still cannot
  silently inject into the target, AX-first writes return
  `error.code = BACKGROUND_INPUT_UNAVAILABLE` with a hint pointing at the
  legacy foreground click; don't retry without a strategy change.
- Envelope additions for AX-first results: each successful response embeds
  `target_app`, `app_state` (text dump), `app_state_nodes` (structured),
  `before_digest` (the digest seen *before* the action), `after_digest` (the
  digest *after*), and `background_input: bool` so the agent can verify the
  action landed without stealing focus.

#### desktop (legacy screenshot + global pointer)
- screenshot, click_target, move_to_target, click, click_element, mouse_move,
  pointer_move_rel, scroll, drag, key_chord, type_text, paste, wait, locate,
  move_to_text.
- **`click_target` / `move_to_target`** — preferred mouse primitive for
  common "click/move to this visible thing" requests. One call resolves the
  target by AX (`node_idx`, text/role/title/identifier filters, or
  `target_text`) first, OCR second (`target_text` / `text_query`), and
  explicit global `x`/`y` last. This collapses the old locate → move →
  guarded-click round-trip into a single authoritative action.
- **`screenshot`** — exactly two possible outputs: the focused application
  window (default, via Accessibility) OR the full display (fallback when
  AX cannot resolve the window). No crop / quadrant / mouse-centered
  options exist anymore. Old crop parameters (`screenshot_crop_center_x/y`,
  `screenshot_navigate_quadrant`, `screenshot_reset_navigation`,
  `screenshot_implicit_center`, `point_crop_half_extent_native`) are
  silently ignored. The only param that still has meaning is
  `screenshot_window: true` — and it just reaffirms the default; you
  rarely need to pass it.
- **`paste { text, clear_first?, submit?, submit_keys? }`** — STRONGLY PREFER
  this over `type_text` for any non-trivial input (CJK, emoji, multi-line,
  contact names, message bodies, anything > ~15 chars). Internally does
  `clipboard_set` + cmd/ctrl+v, optionally cmd/ctrl+a first to replace
  existing content, and optionally Return after to submit. Collapses the
  canonical "type a name into search and press enter" / "send a message"
  sequence into a single tool call AND avoids every IME failure mode that
  `type_text` is subject to. Use `submit_keys: ["command","return"]` for
  Slack-style apps where Return inserts a newline.
- `type_text` is a fallback for short Latin-only text into a focused input
  where you have no clipboard helper (Linux without wl-clipboard / xclip).
  In every other case `paste` is faster and more reliable.
- `key_chord` accepts EITHER `{"keys":["command","v"]}` (canonical) OR a
  bare `{"keys":"escape"}` / `{"key":"return"}` for single keys; both
  shapes are coerced. Modifier names: command, control, option/alt, shift.
- Multi-display routing (FIRST step on multi-monitor setups):
  * `list_displays` — returns every attached screen with `display_id`,
    `is_primary`, `is_active`, `has_pointer`, origin/size, and `scale_factor`.
    Always inspect this list before issuing screen-coordinate actions when
    `interaction_state.displays` has more than one entry; do NOT assume the
    cursor is on the screen the user is looking at.
  * `focus_display` — `{ display_id }` pins ALL subsequent screenshots /
    clicks / locates to that display until cleared. Pass `{ display_id: null }`
    (or omit) to fall back to the legacy "screen under the mouse" behavior.
    Pinning invalidates any cached screenshot, so the next `screenshot` is
    guaranteed to come from the chosen display.
- `interaction_state.displays` and `interaction_state.active_display_id`
  are present in every desktop tool result and tell you which display the
  next action will target. If that does not match the user's intent,
  either call `desktop.focus_display` BEFORE the next `screenshot` / `click`,
  OR pass `display_id: <id>` directly inside the next action's params —
  every desktop action accepts it as a one-shot pin equivalent (sticky:
  the pin persists for follow-up actions until you set `display_id: null`).
- Single-display setup (most users): you do NOT need `list_displays` /
  `focus_display`. Just call `screenshot` / `click_element` / etc.
  directly — `interaction_state.displays.length === 1` is your signal.
"#
        } else {
            r#"### domain: "desktop"
- Not available in this session because Computer Use is disabled.
- Do not attempt mouse, keyboard, OCR, display, or external desktop app control actions.
- To enable these actions, turn on the `computer use` setting in session configuration and use the BitFun desktop app.
"#
        };

        format!(
            r#"ControlHub — the SOLE control entry point for everything the agent can drive.

You will not find a separate `ComputerUse` or `SelfControl` tool: every desktop, browser,
app-self-control, terminal-signalling and system action is reachable through this one tool
via `{{ domain, action, params }}`.

## Decision tree — which domain do I use?

1. The user wants to change something inside the BitFun app itself
   (settings, models, scenes, BitFun's own buttons / forms)?
   → **domain: "app"**  (operates BitFun's own React UI through the SelfControl bridge)

2. The user wants to drive a website / web app in their *real* browser
   (preserving cookies, login, extensions)?
   → **domain: "browser"** (drives the user's default Chromium-family browser via CDP)

3. The user wants to operate another desktop application
   (third-party app windows, OS dialogs, system-wide keyboard / mouse, accessibility)?
   → **domain: "desktop"** (Computer Use: screenshot, click, key_chord, locate, ...)

4. The user wants to launch an app, run a shell / AppleScript, or query OS info?
   → **domain: "system"**

5. The user wants to signal an existing terminal session
   (kill, send SIGINT) — *not* run new commands; for that use the `Bash` tool?
   → **domain: "terminal"**

If you are unsure between two domains: prefer the smallest blast radius
(`app` < `browser` < `desktop` < `system`).

## Unified response envelope

Every call returns a JSON object with a stable shape:

  // success
  {{ "ok": true,  "domain": "...", "action": "...", "data": {{ ... }} }}
  // failure (still delivered as a normal tool result, NOT an exception)
  {{ "ok": false, "domain": "...", "action": "...",
    "error": {{ "code": "STALE_REF" | "NOT_FOUND" | "AMBIGUOUS" | "GUARD_REJECTED"
                       | "WRONG_DISPLAY" | "WRONG_TAB" | "INVALID_PARAMS"
                       | "PERMISSION_DENIED" | "TIMEOUT" | "NOT_AVAILABLE"
                       | "MISSING_SESSION" | "FRONTEND_ERROR" | "INTERNAL"
                       | "APP_NOT_FOUND" | "AX_NODE_STALE" | "AX_IDX_STALE"
                       | "AX_IDX_NOT_SUPPORTED" | "DESKTOP_COORD_OUT_OF_DISPLAY"
                       | "BACKGROUND_INPUT_UNAVAILABLE",
               "message": "...", "hints": [ "...next step..." ] }} }}

Branch on `ok` and on `error.code` deterministically. Never scrape the English `message`
for control flow.

## Domains and actions

### domain: "browser"  (DOM/CDP-only browser control; never use desktop mouse/keyboard for browser interaction)
- Two browser modes:
  * `connect {{ mode: "headless" }}` — attach to a headless test browser on the test port for project Web UI testing that does **not** depend on user login state.
  * `connect {{ mode: "default" }}` (default) — attach to the user's default browser via CDP for flows that require login state, cookies, extensions, or the user's real profile.
- In **all** browser cases, control the page through DOM/CDP actions only. Do **not** use `domain: "desktop"` mouse/keyboard actions to drive a browser.
- connect, navigate, snapshot, click, fill, type, select, press_key, scroll, wait,
  get_text, get_url, get_title, screenshot, evaluate, close, list_pages, tab_query,
  switch_page, list_sessions.
- Fast path (target a known tab in ONE call):
  * `connect {{ target_url? , target_title? , activate? }}` finds the first
    open tab whose URL / title contains the substring, registers it as the
    default session AND brings it to the front. Use this instead of
    `connect` → `list_pages` → `switch_page` for the common
    "drive my Gmail / GitHub PR / docs tab" flow. If the filter matches no
    tab you get `error.code = WRONG_TAB` (no silent fallback).
- Tab routing:
  * `list_pages` returns every page/tab the browser exposes; each entry
    carries `is_default_session` so you can tell which one ControlHub will
    drive next without an extra `list_sessions` round-trip.
  * `tab_query` (`{{ url_contains?, title_contains?, only_pages?, limit? }}`)
    is the preferred filter when you need to inspect candidates before
    committing to one.
  * `switch_page` (`{{ page_id, activate? }}`) sets the default CDP session
    AND, by default, calls `Page.bringToFront` so the user actually sees
    the tab being driven. Pass `activate: false` to keep the operation
    invisible (e.g. background scraping).
- Workflow: connect → navigate → snapshot (returns @e1, @e2 ... refs) → click/fill using refs.
- `snapshot` now traverses **open shadow roots** and **same-origin iframes**;
  each element entry includes `scope` (`document`/`shadow`/`iframe`) and
  `frame_path` so you can tell where in the DOM tree it lives. Pass
  `with_backend_node_ids: true` to also receive a stable
  `backend_node_id` per element (CDP DOM id, survives re-renders).
- Take a fresh snapshot after any DOM mutation; stale refs return `error.code = STALE_REF`.

{desktop_domain_doc}
### domain: "app"  (BitFun's own GUI via the SelfControl bridge)
- Introspection (pure-Rust, no UI round-trip — call these BEFORE bash/fs):
  * `app_self_describe` — one-shot snapshot: `{{ scenes, settingsTabs, miniapps, miniappSubsystemAvailable }}`. Use this whenever the user asks "what can BitFun do / what's installed / what scenes are there / what mini-apps are available" — DO NOT scan the user's workspace directories looking for app features, those directories are USER files, not BitFun installations.
  * `list_miniapps {{ includeRuntime?: bool }}` — installed mini-apps with `id / name / description / icon / category / openSceneId`.
  * `list_scenes` — all scene ids you can pass to `open_scene` (plus dynamic `miniapp:<id>` for installed mini-apps).
  * `list_settings_tabs` — all tab ids you can pass to `open_settings_tab`.
  * `list_tasks` — catalog of named recipes for `execute_task`.
- Navigation / mutation: get_page_state, wait_for_selector, click,
  click_by_text, input, scroll, open_scene, open_settings_tab, set_config,
  get_config, list_models, set_default_model, delete_model, execute_task,
  select_option, wait, press_key, read_text.
- `get_page_state` supports `{{ offset, limit }}` pagination (default
  `offset=0, limit=60`) and returns `pagination` + `webview_id` so you can
  page through long settings panels and tell which webview produced the
  response.
- `wait_for_selector` (`{{ selector, timeoutMs?, state? }}`) blocks until
  the element appears (state 'visible' also waits for a non-zero box).
  Errors with `code='TIMEOUT'`. Prefer it over a fixed `wait {{ durationMs }}`
  when the right delay isn't known.
- For well-known requests, prefer `execute_task` recipes:
  * "set Kimi as the main model" → `set_primary_model {{ modelQuery: "kimi" }}`
  * "open the mini app gallery / show me installed mini apps" → first
    `list_miniapps`, then `execute_task task=open_miniapp_gallery`
    (or `open_miniapp {{ miniAppId: "<id>" }}` to open a specific one).
- HARD RULE: when the user asks "BitFun 里有哪些 X" / "what mini-apps /
  scenes / settings does BitFun have" — answer with `app.app_self_describe`
  or the targeted `list_*` action. NEVER answer this kind of question by
  running `Bash` `ls` against the user's workspace; that path is for
  user files, not BitFun's own catalog.

### domain: "terminal"
- list_sessions, kill (`terminal_session_id`), interrupt (`terminal_session_id`).
  Use the `Bash` tool to *run* commands; this domain only signals existing sessions.
- Fast path: if there is exactly ONE live terminal session, you may omit
  `terminal_session_id` and ControlHub will target it automatically. With
  zero live sessions you get `error.code = MISSING_SESSION`; with multiple
  you get `AMBIGUOUS` plus the candidate ids in `error.hints`. Otherwise
  call `list_sessions` first.

### domain: "system"
- open_app (`app_name`), open_url (`url`), open_file (`path`, `app?`),
  clipboard_get (`max_bytes?`), clipboard_set (`text`),
  run_script (`script`, `script_type` = applescript|shell, optional
  `timeout_ms` ≤ 5 min, `max_output_bytes` ≤ 256 KB), get_os_info.
- `open_url` is the right tool when the goal is "show this URL to the user"
  (no CDP, no driving). Use `domain: "browser"` only when you actually need
  to interact with the page.
- `open_file` opens a local file with its default handler (or an explicit
  `app` on macOS) — high-frequency for "open this PDF / picture / spreadsheet".
- `clipboard_get` / `clipboard_set` are the universal cross-app bridge:
  the cheapest way to move text between apps that you'd otherwise have to
  drive separately. `clipboard_get` returns `{{ text, byte_length, truncated }}`;
  `clipboard_set {{ text }}` is the inverse. On Linux this requires
  wl-clipboard / xclip / xsel; missing-helper failures return `NOT_AVAILABLE`.
- `run_script` enforces the timeout and truncates large stdout/stderr; on
  timeout it returns `error.code = TIMEOUT` and the child process is killed.
  `get_os_info` includes `os`, `arch`, `os_version`, `hostname`.

### domain: "meta"  (introspection — call this BEFORE long control flows)
- `capabilities` — returns `{{ domains: {{ desktop, browser, app, terminal, system, meta }},
  host: {{ os, arch }}, schema_version }}`. Use it to confirm which domains are
  actually wired up on this runtime instead of guessing from the description.
- `route_hint` (`{{ intent }}`) — heuristic mapping of a free-form user intent
  ("把 BitFun 默认模型改成 Kimi") to a ranked list of candidate domains so the
  model has a sanity check before it commits to one. Always confirm with
  `meta.capabilities` and the domain docs; this is only a hint.

## Workflow tips
1. For cross-domain workflows (browser data → desktop paste, app config → external nav),
   call actions sequentially and verify each step's `ok` field before chaining.
2. After any UI mutation, re-acquire state (browser: snapshot, desktop: screenshot,
   app: get_page_state) before the next action.
3. When the model is the only one driving inputs, `wait` 200–500 ms after a click that
   triggers an animation before re-observing."#,
            desktop_domain_doc = desktop_domain_doc,
        )
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
                if !Self::desktop_domain_enabled().await {
                    return Ok(err_response(
                        "desktop",
                        action,
                        ControlHubError::new(
                            ErrorCode::NotAvailable,
                            "Computer Use is disabled for this session.",
                        )
                        .with_hint(
                            "Enable computer use in session settings to expose desktop control actions.",
                        ),
                    ));
                }
                self.handle_desktop(action, params, context).await
            },
            "browser" => self.handle_browser(action, params).await,
            "app" => self.handle_app(action, params, context).await,
            "terminal" => self.handle_terminal(action, params, context).await,
            "system" => self.handle_system(action, params, context).await,
            "meta" => self.handle_meta(action, params, context).await,
            other => Err(BitFunError::tool(format!(
                "Unknown domain: '{}'. Valid domains: desktop, browser, app, terminal, system, meta",
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
    // attempt e.g. `domain:"app"` on a host where the SelfControl bridge
    // is not registered, and only learn the truth from a runtime error.

    async fn handle_meta(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        match action {
            "capabilities" => {
                let desktop_available = Self::desktop_domain_enabled().await;
                // `app` (SelfControl bridge) and `terminal` (TerminalApi) are
                // both delivered through global registries rather than fields
                // on the context, so we can't be 100% sure here without
                // round-tripping. We report "likely available iff desktop is
                // available" because both bridges only exist in BitFun's
                // desktop runtime; the actual call will surface a clean
                // FRONTEND_ERROR / NOT_AVAILABLE if the bridge is offline.
                let likely_app_available = desktop_available;
                let likely_terminal_available = desktop_available;
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
                let mut script_types: Vec<&'static str> = vec!["shell"];
                if cfg!(target_os = "macos") {
                    script_types.push("applescript");
                }
                if which_exists("bash") {
                    script_types.push("bash");
                }
                if which_exists("pwsh") || which_exists("powershell") {
                    script_types.push("powershell");
                }
                if cfg!(target_os = "windows") {
                    script_types.push("cmd");
                }

                #[cfg(target_os = "linux")]
                let (display_server, desktop_env) = linux_session_info();
                #[cfg(not(target_os = "linux"))]
                let (display_server, desktop_env): (
                    Option<String>,
                    Option<String>,
                ) = (None, None);

                let desktop_host = context.computer_use_host.as_ref();
                let desktop_ax_tree = desktop_host
                    .map(|host| host.supports_ax_tree())
                    .unwrap_or(false);
                let desktop_background_input = desktop_host
                    .map(|host| host.supports_background_input())
                    .unwrap_or(false);
                let desktop_interactive_view = desktop_host
                    .map(|host| host.supports_interactive_view())
                    .unwrap_or(false);
                let desktop_visual_mark_view = desktop_host
                    .map(|host| host.supports_visual_mark_view())
                    .unwrap_or(false);

                let body = json!({
                    "domains": {
                        "desktop":  {
                            "available": desktop_available,
                            "reason": if desktop_available { Value::Null } else { json!("Only available in the BitFun desktop app") },
                            "supports_ax_tree": desktop_ax_tree,
                            "supports_background_input": desktop_background_input,
                            "supports_interactive_view": desktop_interactive_view,
                            "supports_visual_mark_view": desktop_visual_mark_view,
                        },
                        "browser":  {
                            "available": true,
                            "default_session_id": browser_default,
                            "session_count": browser_session_count,
                            "default_browser": browser_kind,
                            "cdp_supported": browser_cdp_supported,
                        },
                        "app":      { "available": likely_app_available, "reason": if likely_app_available { Value::Null } else { json!("BitFun front-end (SelfControl bridge) is only wired up in the desktop app") } },
                        "terminal": { "available": likely_terminal_available, "reason": if likely_terminal_available { Value::Null } else { json!("TerminalApi is only available in contexts that registered it") } },
                        "system":   {
                            "available": true,
                            "script_types": script_types,
                        },
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

                let app_kw = [
                    "bitfun",
                    "settings",
                    "scene",
                    "default model",
                    "primary model",
                    "fast model",
                    "切换模型",
                    "默认模型",
                    "设置",
                    "场景",
                ];
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

                for kw in app_kw {
                    if lower.contains(kw) {
                        push(
                            &mut suggestions,
                            "app",
                            90,
                            "Matches BitFun-internal UI keywords",
                        );
                        break;
                    }
                }
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
                            "desktop",
                            75,
                            "Matches third-party desktop window keywords",
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
                        push(&mut suggestions, "system", 70, "Matches OS/launch keywords");
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

    // ── Desktop domain ─────────────────────────────────────────────────

    async fn handle_desktop(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            BitFunError::tool(
                "Desktop control is only available in the BitFun desktop app".to_string(),
            )
        })?;

        // Phase 2: handle multi-display routing actions directly. Going
        // through the legacy ComputerUseTool dispatch isn't useful here
        // because there is no equivalent action there, and we want these
        // to be first-class ControlHub primitives so the model can pin a
        // target display before any screenshot/click flow.
        match action {
            // High-leverage UX primitive: paste arbitrary text into the
            // currently focused input via the system clipboard, optionally
            // clearing first and submitting after. This collapses the
            // canonical IM/search flow:
            //
            //   clipboard_set + key_chord(cmd+v) + key_chord(return)
            //
            // ...into a single tool call. It is also the **only** robust way
            // to enter CJK / emoji / multi-line text — `type_text` goes
            // through the per-character key path and is at the mercy of
            // every IME on the host. This is exactly the pattern Codex
            // uses (`pbcopy` + cmd+v) to keep WeChat / iMessage flows
            // smooth.
            //
            // Params:
            //   - text          (required) — text to paste
            //   - clear_first   (bool, default false) — cmd+a before paste,
            //                   so the new text REPLACES whatever was there
            //   - submit        (bool, default false) — press Return after
            //                   paste; switches to "send the message" mode
            //   - submit_keys   (array, default ["return"]) — override the
            //                   submit chord (e.g. ["command","return"] for
            //                   Slack / multi-line apps)
            //
            // Returns the same envelope as a `key_chord` so the model can
            // chain a verification screenshot exactly as before.
            "paste" => {
                let text = params
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "[INVALID_PARAMS] desktop.paste requires 'text'\nHints: example { \"action\":\"paste\", \"text\":\"hello\", \"submit\":true }"
                                .to_string(),
                        )
                    })?;
                let clear_first = params
                    .get("clear_first")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let submit = params
                    .get("submit")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let submit_keys: Vec<String> = match params.get("submit_keys") {
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                    Some(Value::String(s)) => vec![s.to_string()],
                    _ => vec!["return".to_string()],
                };

                if let Err(e) = clipboard_write(text).await {
                    return Ok(err_response(
                        "desktop",
                        "paste",
                        ControlHubError::new(
                            ErrorCode::NotAvailable,
                            format!("Clipboard write failed: {}", e),
                        )
                        .with_hint(
                            "Fall back to type_text or check that wl-clipboard / xclip is installed (Linux only)",
                        ),
                    ));
                }

                let paste_chord = match std::env::consts::OS {
                    "macos" => vec!["command".to_string(), "v".to_string()],
                    _ => vec!["control".to_string(), "v".to_string()],
                };

                if clear_first {
                    let select_all = match std::env::consts::OS {
                        "macos" => vec!["command".to_string(), "a".to_string()],
                        _ => vec!["control".to_string(), "a".to_string()],
                    };
                    host.key_chord(select_all).await?;
                }
                host.key_chord(paste_chord).await?;
                if submit {
                    host.computer_use_trust_pointer_after_text_input();
                    host.key_chord(submit_keys.clone()).await?;
                }

                let summary = match (clear_first, submit) {
                    (false, false) => format!("Pasted {} chars", text.chars().count()),
                    (true, false) => {
                        format!("Replaced focused field with {} chars", text.chars().count())
                    }
                    (false, true) => format!("Pasted {} chars and submitted", text.chars().count()),
                    (true, true) => {
                        format!("Replaced + submitted ({} chars)", text.chars().count())
                    }
                };
                return Ok(vec![ToolResult::ok(
                    json!({
                        "success": true,
                        "action": "paste",
                        "char_count": text.chars().count(),
                        "byte_length": text.len(),
                        "clear_first": clear_first,
                        "submitted": submit,
                        "submit_keys": if submit { Some(submit_keys) } else { None },
                    }),
                    Some(summary),
                )]);
            }

            // ── AX-first actions (Codex parity) ───────────────────────
            // These bypass the legacy ComputerUseTool because they
            // operate on the new typed AppSelector / AxNode envelope.
            "list_apps"
            | "get_app_state"
            | "app_click"
            | "app_type_text"
            | "app_scroll"
            | "app_key_chord"
            | "app_wait_for"
            | "build_interactive_view"
            | "interactive_click"
            | "interactive_type_text"
            | "interactive_scroll"
            | "build_visual_mark_view"
            | "visual_click" => {
                return self.handle_desktop_ax(host, action, params).await;
            }
            "focus_display" => {
                // Accept `null` (or omitted `display_id`) to clear the pin
                // and fall back to "screen under the pointer". An explicit
                // numeric id pins that display until cleared.
                let display_id = match params.get("display_id") {
                    Some(Value::Null) | None => None,
                    Some(v) => Some(v.as_u64().ok_or_else(|| {
                        BitFunError::tool(
                            "focus_display: 'display_id' must be a non-negative integer or null"
                                .to_string(),
                        )
                    })? as u32),
                };
                host.focus_display(display_id).await?;
                let displays = host.list_displays().await?;
                let summary = match display_id {
                    Some(id) => format!("Pinned display {}", id),
                    None => "Cleared display pin (will follow mouse)".to_string(),
                };
                return Ok(vec![ToolResult::ok(
                    json!({
                        "active_display_id": display_id,
                        "displays": displays,
                    }),
                    Some(summary),
                )]);
            }
            _ => {}
        }

        if let Some(err) = self.desktop_action_targets_browser(action, context).await {
            return Ok(err_response("desktop", action, err));
        }

        // UX shortcut: every screen-coordinate action accepts an optional
        // `display_id`. If present (and different from the currently pinned
        // display), pin it BEFORE forwarding so the model doesn't need a
        // separate `focus_display` round-trip. Pin is sticky — subsequent
        // actions on the same screen don't need to re-specify. Pass
        // `display_id: null` to clear the pin in the same call.
        let mut cu_input = params.clone();
        if let Value::Object(ref mut map) = cu_input {
            map.insert("action".to_string(), json!(action));
            // Strip the ControlHub-only field so the legacy ComputerUseTool
            // doesn't trip on an unrecognised parameter.
            map.remove("display_id");
        }

        let cu_tool = super::computer_use_tool::ComputerUseTool::new();
        cu_tool.call_impl(&cu_input, context).await
    }

    // ── Desktop AX-first dispatch (Codex parity) ──────────────────────
    // Routes the seven new app-targeted actions through the typed
    // `ComputerUseHost` API. Every successful response carries a
    // unified envelope: `target_app`, `background_input`,
    // `before_digest` and (for state queries) `app_state` /
    // `app_state_nodes` so the model can reason about the AX tree
    // before/after each action without re-querying.
    async fn handle_desktop_ax(
        &self,
        host: &ComputerUseHostRef,
        action: &str,
        params: &Value,
    ) -> BitFunResult<Vec<ToolResult>> {
        // ── Helpers ─────────────────────────────────────────────────
        fn parse_selector(v: &Value) -> BitFunResult<AppSelector> {
            let obj = v.get("app").ok_or_else(|| {
                BitFunError::tool(
                    "[INVALID_PARAMS] missing 'app' selector (pid|bundle_id|name)".to_string(),
                )
            })?;
            let sel: AppSelector = serde_json::from_value(obj.clone()).map_err(|e| {
                BitFunError::tool(format!(
                    "[INVALID_PARAMS] bad 'app' selector: {} (expect {{pid|bundle_id|name}})",
                    e
                ))
            })?;
            if sel.pid.is_none() && sel.bundle_id.is_none() && sel.name.is_none() {
                return Err(BitFunError::tool(
                    "[INVALID_PARAMS] 'app' must include at least one of pid|bundle_id|name"
                        .to_string(),
                ));
            }
            Ok(sel)
        }

        fn parse_click_target(v: &Value) -> BitFunResult<ClickTarget> {
            if v.get("kind").is_some() {
                return serde_json::from_value(v.clone()).map_err(|e| {
                    BitFunError::tool(format!(
                        "[INVALID_PARAMS] bad ClickTarget: {} (expected {{\"kind\":\"node_idx\",\"idx\":N}}, {{\"kind\":\"image_xy\",\"x\":0,\"y\":0}}, {{\"kind\":\"image_grid\",\"x0\":0,\"y0\":0,\"width\":300,\"height\":300,\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}}, {{\"kind\":\"visual_grid\",\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}}, {{\"kind\":\"screen_xy\",\"x\":0,\"y\":0}}, or {{\"kind\":\"ocr_text\",\"needle\":\"...\"}})",
                        e
                    ))
                });
            }
            if let Some(idx) = v.get("node_idx").and_then(|x| x.as_u64()) {
                return Ok(ClickTarget::NodeIdx { idx: idx as u32 });
            }
            if let Some(obj) = v.get("screen_xy") {
                let x = obj.get("x").and_then(|x| x.as_f64()).ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] screen_xy target requires numeric x".to_string(),
                    )
                })?;
                let y = obj.get("y").and_then(|y| y.as_f64()).ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] screen_xy target requires numeric y".to_string(),
                    )
                })?;
                return Ok(ClickTarget::ScreenXy { x, y });
            }
            if let Some(obj) = v.get("image_xy") {
                let x = obj.get("x").and_then(|x| x.as_i64()).ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] image_xy target requires integer x".to_string(),
                    )
                })?;
                let y = obj.get("y").and_then(|y| y.as_i64()).ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] image_xy target requires integer y".to_string(),
                    )
                })?;
                return Ok(ClickTarget::ImageXy {
                    x: x as i32,
                    y: y as i32,
                    screenshot_id: obj
                        .get("screenshot_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                });
            }
            if let Some(obj) = v.get("image_grid") {
                let target = json!({
                    "kind": "image_grid",
                    "x0": obj.get("x0").cloned().unwrap_or(Value::Null),
                    "y0": obj.get("y0").cloned().unwrap_or(Value::Null),
                    "width": obj.get("width").cloned().unwrap_or(Value::Null),
                    "height": obj.get("height").cloned().unwrap_or(Value::Null),
                    "rows": obj.get("rows").cloned().unwrap_or(Value::Null),
                    "cols": obj.get("cols").cloned().unwrap_or(Value::Null),
                    "row": obj.get("row").cloned().unwrap_or(Value::Null),
                    "col": obj.get("col").cloned().unwrap_or(Value::Null),
                    "intersections": obj.get("intersections").cloned().unwrap_or(json!(false)),
                    "screenshot_id": obj.get("screenshot_id").cloned().unwrap_or(Value::Null),
                });
                return serde_json::from_value(target).map_err(|e| {
                    BitFunError::tool(format!(
                        "[INVALID_PARAMS] bad image_grid target: {} (need x0,y0,width,height,rows,cols,row,col; optional intersections)",
                        e
                    ))
                });
            }
            if let Some(obj) = v.get("visual_grid") {
                let target = json!({
                    "kind": "visual_grid",
                    "rows": obj.get("rows").cloned().unwrap_or(Value::Null),
                    "cols": obj.get("cols").cloned().unwrap_or(Value::Null),
                    "row": obj.get("row").cloned().unwrap_or(Value::Null),
                    "col": obj.get("col").cloned().unwrap_or(Value::Null),
                    "intersections": obj.get("intersections").cloned().unwrap_or(json!(false)),
                    "wait_ms_after_detection": obj.get("wait_ms_after_detection").cloned().unwrap_or(Value::Null),
                });
                return serde_json::from_value(target).map_err(|e| {
                    BitFunError::tool(format!(
                        "[INVALID_PARAMS] bad visual_grid target: {} (need rows,cols,row,col; optional intersections)",
                        e
                    ))
                });
            }
            if v.get("x").is_some() || v.get("y").is_some() {
                let x = v.get("x").and_then(|x| x.as_f64()).ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] screen target requires numeric x".to_string(),
                    )
                })?;
                let y = v.get("y").and_then(|y| y.as_f64()).ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] screen target requires numeric y".to_string(),
                    )
                })?;
                return Ok(ClickTarget::ScreenXy { x, y });
            }
            if let Some(ocr) = v.get("ocr_text") {
                let needle = ocr
                    .get("needle")
                    .or_else(|| ocr.get("text"))
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "[INVALID_PARAMS] ocr_text target requires needle".to_string(),
                        )
                    })?;
                return Ok(ClickTarget::OcrText {
                    needle: needle.to_string(),
                });
            }
            Err(BitFunError::tool(
                "[INVALID_PARAMS] unsupported ClickTarget. Use {\"kind\":\"node_idx\",\"idx\":N}, {\"node_idx\":N}, {\"kind\":\"image_xy\",\"x\":0,\"y\":0}, {\"image_xy\":{\"x\":0,\"y\":0}}, {\"kind\":\"image_grid\",\"x0\":0,\"y0\":0,\"width\":300,\"height\":300,\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}, {\"kind\":\"visual_grid\",\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}, {\"kind\":\"screen_xy\",\"x\":0,\"y\":0}, or {\"ocr_text\":{\"needle\":\"...\"}}.".to_string(),
            ))
        }

        fn parse_wait_predicate(v: &Value) -> BitFunResult<AppWaitPredicate> {
            if v.get("kind").is_some() {
                return serde_json::from_value(v.clone()).map_err(|e| {
                    BitFunError::tool(format!(
                        "[INVALID_PARAMS] bad app_wait_for predicate: {}",
                        e
                    ))
                });
            }
            if let Some(obj) = v.get("digest_changed") {
                let prev_digest = obj
                    .get("prev_digest")
                    .or_else(|| obj.get("from"))
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "[INVALID_PARAMS] digest_changed requires prev_digest".to_string(),
                        )
                    })?;
                return Ok(AppWaitPredicate::DigestChanged {
                    prev_digest: prev_digest.to_string(),
                });
            }
            if let Some(obj) = v.get("title_contains") {
                let needle = obj
                    .get("needle")
                    .or_else(|| obj.get("title"))
                    .and_then(|x| x.as_str())
                    .or_else(|| obj.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "[INVALID_PARAMS] title_contains requires needle".to_string(),
                        )
                    })?;
                return Ok(AppWaitPredicate::TitleContains {
                    needle: needle.to_string(),
                });
            }
            if let Some(obj) = v.get("role_enabled") {
                let role = obj.get("role").and_then(|x| x.as_str()).ok_or_else(|| {
                    BitFunError::tool("[INVALID_PARAMS] role_enabled requires role".to_string())
                })?;
                return Ok(AppWaitPredicate::RoleEnabled {
                    role: role.to_string(),
                });
            }
            if let Some(obj) = v.get("node_enabled") {
                let idx = obj
                    .get("idx")
                    .and_then(|x| x.as_u64())
                    .or_else(|| obj.as_u64())
                    .ok_or_else(|| {
                        BitFunError::tool("[INVALID_PARAMS] node_enabled requires idx".to_string())
                    })?;
                return Ok(AppWaitPredicate::NodeEnabled { idx: idx as u32 });
            }
            Err(BitFunError::tool(
                "[INVALID_PARAMS] unsupported app_wait_for predicate. Use {\"kind\":\"digest_changed\",\"prev_digest\":\"...\"} or shorthand {\"digest_changed\":{\"prev_digest\":\"...\"}}.".to_string(),
            ))
        }

        fn parse_keys(v: &Value) -> Vec<String> {
            match v.get("keys").or_else(|| v.get("key")) {
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect(),
                Some(Value::String(s)) => vec![s.to_string()],
                _ => Vec::new(),
            }
        }

        // Build the JSON view of an AppStateSnapshot for the model. Excludes
        // the heavy `screenshot` payload (it is attached out-of-band as a
        // multimodal image, not as base64 inside the JSON tree, to keep token
        // budgets under control and let the provider deliver it as `image_url`).
        fn snap_state_json(
            snap: &crate::agentic::tools::computer_use_host::AppStateSnapshot,
        ) -> serde_json::Value {
            let mut v = json!({
                "app": snap.app,
                "window_title": snap.window_title,
                "digest": snap.digest,
                "captured_at_ms": snap.captured_at_ms,
                "tree_text": snap.tree_text,
                "has_screenshot": snap.screenshot.is_some(),
            });
            if let Some(shot) = snap.screenshot.as_ref() {
                if let Some(obj) = v.as_object_mut() {
                    let meta: serde_json::Value = json!({
                        "image_width": shot.image_width,
                        "image_height": shot.image_height,
                        "screenshot_id": shot.screenshot_id,
                        "native_width": shot.native_width,
                        "native_height": shot.native_height,
                        "vision_scale": shot.vision_scale,
                        "mime_type": shot.mime_type,
                        "image_content_rect": shot.image_content_rect,
                        "image_global_bounds": shot.image_global_bounds,
                            "coordinate_hint": "For visual surfaces, click pixels in this attached image with app_click target {kind:\"image_xy\", x, y, screenshot_id}. For known boards/grids/canvases, prefer {kind:\"image_grid\", x0, y0, width, height, rows, cols, row, col, intersections, screenshot_id}. If the grid rectangle is unknown, use {kind:\"visual_grid\", rows, cols, row, col, intersections}; the host detects the grid from app pixels.",
                        });
                    obj.insert("screenshot_meta".to_string(), meta);
                }
            }
            v
        }

        // Helper: build a `ToolResult` that *also* carries the focused-window
        // screenshot as an Anthropic-style multimodal image attachment. When
        // the host couldn't (or chose not to) capture, fall back to a regular
        // text-only `ToolResult::ok`.
        fn snap_result(
            data: serde_json::Value,
            summary: Option<String>,
            snap: &crate::agentic::tools::computer_use_host::AppStateSnapshot,
        ) -> ToolResult {
            use base64::Engine as _;
            if let Some(shot) = snap.screenshot.as_ref() {
                let attach = crate::util::types::ToolImageAttachment {
                    mime_type: shot.mime_type.clone(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&shot.bytes),
                };
                ToolResult::ok_with_images(data, summary, vec![attach])
            } else {
                ToolResult::ok(data, summary)
            }
        }

        // Build a JSON view of an InteractiveView that excludes the heavy
        // `screenshot.bytes` payload (the JPEG is attached out-of-band as a
        // multimodal image attachment, not as base64 inside the tree).
        fn build_interactive_view_json(
            view: &crate::agentic::tools::computer_use_host::InteractiveView,
        ) -> serde_json::Value {
            let mut v = json!({
                "app": view.app,
                "window_title": view.window_title,
                "digest": view.digest,
                "captured_at_ms": view.captured_at_ms,
                "elements": view.elements,
                "tree_text": view.tree_text,
                "loop_warning": view.loop_warning,
                "has_screenshot": view.screenshot.is_some(),
            });
            if let Some(shot) = view.screenshot.as_ref() {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert(
                        "screenshot_meta".to_string(),
                        json!({
                            "image_width": shot.image_width,
                            "image_height": shot.image_height,
                            "screenshot_id": shot.screenshot_id,
                            "native_width": shot.native_width,
                            "native_height": shot.native_height,
                            "vision_scale": shot.vision_scale,
                            "mime_type": shot.mime_type,
                            "image_content_rect": shot.image_content_rect,
                            "image_global_bounds": shot.image_global_bounds,
                            "coordinate_hint": "Numbered overlays are in JPEG image-pixel space. Reference elements via their `i` index using interactive_click / interactive_type_text / interactive_scroll. For pointer-only fallback, pass screenshot_id with image_xy/image_grid.",
                        }),
                    );
                }
            }
            v
        }

        fn build_visual_mark_view_json(
            view: &crate::agentic::tools::computer_use_host::VisualMarkView,
        ) -> serde_json::Value {
            let mut v = json!({
                "app": view.app,
                "window_title": view.window_title,
                "digest": view.digest,
                "captured_at_ms": view.captured_at_ms,
                "marks": view.marks,
                "has_screenshot": view.screenshot.is_some(),
            });
            if let Some(shot) = view.screenshot.as_ref() {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert(
                        "screenshot_meta".to_string(),
                        json!({
                            "image_width": shot.image_width,
                            "image_height": shot.image_height,
                            "screenshot_id": shot.screenshot_id,
                            "native_width": shot.native_width,
                            "native_height": shot.native_height,
                            "vision_scale": shot.vision_scale,
                            "mime_type": shot.mime_type,
                            "image_content_rect": shot.image_content_rect,
                            "image_global_bounds": shot.image_global_bounds,
                            "coordinate_hint": "Numbered visual marks are in JPEG image-pixel space. Reference marks via their `i` index using visual_click. To refine a dense area, call build_visual_mark_view again with opts.region in these screenshot pixels.",
                        }),
                    );
                }
            }
            v
        }

        // Build a JSON envelope for interactive_* action results. Includes
        // the post-action AppStateSnapshot (without screenshot bytes) and,
        // when present, the rebuilt InteractiveView.
        fn build_interactive_action_json(
            app: &crate::agentic::tools::computer_use_host::AppSelector,
            res: &crate::agentic::tools::computer_use_host::InteractiveActionResult,
            extras: serde_json::Value,
        ) -> serde_json::Value {
            let mut v = json!({
                "target_app": app,
                "app_state": snap_state_json(&res.snapshot),
                "app_state_nodes": res.snapshot.nodes,
                "loop_warning": res.snapshot.loop_warning,
                "execution_note": res.execution_note,
                "interactive_view": res.view.as_ref().map(build_interactive_view_json),
            });
            if let (Some(obj), Some(extras_obj)) = (v.as_object_mut(), extras.as_object()) {
                for (k, val) in extras_obj {
                    obj.insert(k.clone(), val.clone());
                }
            }
            v
        }

        fn build_visual_action_json(
            app: &crate::agentic::tools::computer_use_host::AppSelector,
            res: &crate::agentic::tools::computer_use_host::VisualActionResult,
            extras: serde_json::Value,
        ) -> serde_json::Value {
            let mut v = json!({
                "target_app": app,
                "app_state": snap_state_json(&res.snapshot),
                "app_state_nodes": res.snapshot.nodes,
                "loop_warning": res.snapshot.loop_warning,
                "execution_note": res.execution_note,
                "visual_mark_view": res.view.as_ref().map(build_visual_mark_view_json),
            });
            if let (Some(obj), Some(extras_obj)) = (v.as_object_mut(), extras.as_object()) {
                for (k, val) in extras_obj {
                    obj.insert(k.clone(), val.clone());
                }
            }
            v
        }

        // Attach the InteractiveView's annotated screenshot (if present)
        // as a multimodal image; otherwise fall back to text-only ok.
        fn interactive_view_result(
            data: serde_json::Value,
            summary: Option<String>,
            view: &crate::agentic::tools::computer_use_host::InteractiveView,
        ) -> ToolResult {
            use base64::Engine as _;
            if let Some(shot) = view.screenshot.as_ref() {
                let attach = crate::util::types::ToolImageAttachment {
                    mime_type: shot.mime_type.clone(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&shot.bytes),
                };
                ToolResult::ok_with_images(data, summary, vec![attach])
            } else {
                ToolResult::ok(data, summary)
            }
        }

        fn visual_mark_view_result(
            data: serde_json::Value,
            summary: Option<String>,
            view: &crate::agentic::tools::computer_use_host::VisualMarkView,
        ) -> ToolResult {
            use base64::Engine as _;
            if let Some(shot) = view.screenshot.as_ref() {
                let attach = crate::util::types::ToolImageAttachment {
                    mime_type: shot.mime_type.clone(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&shot.bytes),
                };
                ToolResult::ok_with_images(data, summary, vec![attach])
            } else {
                ToolResult::ok(data, summary)
            }
        }

        // Prefer attaching the rebuilt interactive view's screenshot when
        // available; otherwise fall back to the post-action snapshot's.
        fn interactive_action_result(
            data: serde_json::Value,
            summary: Option<String>,
            res: &crate::agentic::tools::computer_use_host::InteractiveActionResult,
        ) -> ToolResult {
            use base64::Engine as _;
            let shot_opt = res
                .view
                .as_ref()
                .and_then(|v| v.screenshot.as_ref())
                .or(res.snapshot.screenshot.as_ref());
            if let Some(shot) = shot_opt {
                let attach = crate::util::types::ToolImageAttachment {
                    mime_type: shot.mime_type.clone(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&shot.bytes),
                };
                ToolResult::ok_with_images(data, summary, vec![attach])
            } else {
                ToolResult::ok(data, summary)
            }
        }

        fn visual_action_result(
            data: serde_json::Value,
            summary: Option<String>,
            res: &crate::agentic::tools::computer_use_host::VisualActionResult,
        ) -> ToolResult {
            use base64::Engine as _;
            let shot_opt = res
                .view
                .as_ref()
                .and_then(|v| v.screenshot.as_ref())
                .or(res.snapshot.screenshot.as_ref());
            if let Some(shot) = shot_opt {
                let attach = crate::util::types::ToolImageAttachment {
                    mime_type: shot.mime_type.clone(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&shot.bytes),
                };
                ToolResult::ok_with_images(data, summary, vec![attach])
            } else {
                ToolResult::ok(data, summary)
            }
        }

        let bg = host.supports_background_input();
        let ax = host.supports_ax_tree();

        match action {
            "list_apps" => {
                let include_hidden = params
                    .get("include_hidden")
                    .and_then(|v| v.as_bool())
                    .unwrap_or_else(|| {
                        !params
                            .get("only_visible")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true)
                    });
                let apps = host.list_apps(include_hidden).await?;
                let n = apps.len();
                Ok(vec![ToolResult::ok(
                    json!({
                        "apps": apps,
                        "include_hidden": include_hidden,
                        "background_input": bg,
                        "ax_tree": ax,
                    }),
                    Some(format!("{} app(s) listed", n)),
                )])
            }
            "get_app_state" => {
                let app = parse_selector(params)?;
                let max_depth = params
                    .get("max_depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(32) as u32;
                let focus_window_only = params
                    .get("focus_window_only")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let snap = host
                    .get_app_state(app.clone(), max_depth, focus_window_only)
                    .await?;
                let summary = format!(
                    "AX state for {} (digest={}, {} nodes)",
                    snap.app.name,
                    &snap.digest[..snap.digest.len().min(12)],
                    snap.nodes.len()
                );
                let data = json!({
                    "target_app": app,
                    "background_input": bg,
                    "ax_tree": ax,
                    "app_state": snap_state_json(&snap),
                    "app_state_nodes": snap.nodes,
                    "before_digest": snap.digest,
                    "loop_warning": snap.loop_warning,
                });
                Ok(vec![snap_result(data, Some(summary), &snap)])
            }
            "app_click" => {
                let app = parse_selector(params)?;
                let target_v = params.get("target").cloned().ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] app_click requires 'target' ({node_idx|image_xy|screen_xy|ocr_text})"
                            .to_string(),
                    )
                })?;
                let target = parse_click_target(&target_v)?;
                let click_count = params
                    .get("click_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u8;
                let mouse_button = params
                    .get("mouse_button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left")
                    .to_string();
                let modifier_keys: Vec<String> = params
                    .get("modifier_keys")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let wait_ms_after = params
                    .get("wait_ms_after")
                    .or_else(|| params.get("post_click_wait_ms"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v.min(5_000) as u32);

                let before = host
                    .get_app_state(app.clone(), 8, false)
                    .await
                    .ok()
                    .map(|s| s.digest);

                let mut after = host
                    .app_click(AppClickParams {
                        app: app.clone(),
                        target: target.clone(),
                        click_count,
                        mouse_button,
                        modifier_keys,
                        wait_ms_after,
                    })
                    .await?;

                if after.loop_warning.is_none() {
                    let target_sig = serde_json::to_string(&target).unwrap_or_default();
                    after.loop_warning = loop_tracker_observe(
                        app.pid,
                        "app_click",
                        &target_sig,
                        before.as_deref().unwrap_or(""),
                        &after.digest,
                    );
                }

                let data = json!({
                    "target_app": app,
                    "click_target": target,
                    "background_input": bg,
                    "before_digest": before,
                    "app_state": snap_state_json(&after),
                    "app_state_nodes": after.nodes,
                    "loop_warning": after.loop_warning,
                });
                Ok(vec![snap_result(data, Some("clicked".to_string()), &after)])
            }
            "app_type_text" => {
                let app = parse_selector(params)?;
                let text = params
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "[INVALID_PARAMS] app_type_text requires 'text'".to_string(),
                        )
                    })?
                    .to_string();
                let focus: Option<ClickTarget> = match params.get("focus") {
                    Some(v) if !v.is_null() => Some(parse_click_target(v)?),
                    _ => None,
                };
                let before = host
                    .get_app_state(app.clone(), 8, false)
                    .await
                    .ok()
                    .map(|s| s.digest);
                let mut after = host
                    .app_type_text(app.clone(), &text, focus.clone())
                    .await?;
                if after.loop_warning.is_none() {
                    let target_sig = format!(
                        "focus={};len={}",
                        serde_json::to_string(&focus).unwrap_or_default(),
                        text.chars().count()
                    );
                    after.loop_warning = loop_tracker_observe(
                        app.pid,
                        "app_type_text",
                        &target_sig,
                        before.as_deref().unwrap_or(""),
                        &after.digest,
                    );
                }
                let data = json!({
                    "target_app": app,
                    "background_input": bg,
                    "char_count": text.chars().count(),
                    "focus": focus,
                    "before_digest": before,
                    "app_state": snap_state_json(&after),
                    "app_state_nodes": after.nodes,
                    "loop_warning": after.loop_warning,
                });
                Ok(vec![snap_result(
                    data,
                    Some(format!("typed {} chars", text.chars().count())),
                    &after,
                )])
            }
            "app_scroll" => {
                let app = parse_selector(params)?;
                let dx = params.get("dx").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let dy = params.get("dy").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let focus: Option<ClickTarget> = match params.get("focus") {
                    Some(v) if !v.is_null() => Some(parse_click_target(v)?),
                    _ => None,
                };
                let after = host.app_scroll(app.clone(), focus.clone(), dx, dy).await?;
                let data = json!({
                    "target_app": app,
                    "background_input": bg,
                    "dx": dx,
                    "dy": dy,
                    "focus": focus,
                    "app_state": snap_state_json(&after),
                    "app_state_nodes": after.nodes,
                    "loop_warning": after.loop_warning,
                });
                Ok(vec![snap_result(
                    data,
                    Some(format!("scrolled ({},{})", dx, dy)),
                    &after,
                )])
            }
            "app_key_chord" => {
                let app = parse_selector(params)?;
                let keys = parse_keys(params);
                if keys.is_empty() {
                    return Err(BitFunError::tool(
                        "[INVALID_PARAMS] app_key_chord requires non-empty 'keys'".to_string(),
                    ));
                }
                let focus_idx: Option<u32> = params
                    .get("focus_idx")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32);
                let after = host
                    .app_key_chord(app.clone(), keys.clone(), focus_idx)
                    .await?;
                let data = json!({
                    "target_app": app,
                    "background_input": bg,
                    "keys": keys,
                    "focus_idx": focus_idx,
                    "app_state": snap_state_json(&after),
                    "app_state_nodes": after.nodes,
                    "loop_warning": after.loop_warning,
                });
                Ok(vec![snap_result(
                    data,
                    Some("key chord sent".to_string()),
                    &after,
                )])
            }
            "app_wait_for" => {
                let app = parse_selector(params)?;
                let predicate_v = params.get("predicate").cloned().ok_or_else(|| {
                    BitFunError::tool(
                        "[INVALID_PARAMS] app_wait_for requires 'predicate'".to_string(),
                    )
                })?;
                let predicate = parse_wait_predicate(&predicate_v)?;
                let timeout_ms = params
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(8000) as u32;
                let poll_ms = params
                    .get("poll_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(150) as u32;
                let after = host
                    .app_wait_for(app.clone(), predicate.clone(), timeout_ms, poll_ms)
                    .await?;
                let data = json!({
                    "target_app": app,
                    "background_input": bg,
                    "predicate": predicate,
                    "app_state": snap_state_json(&after),
                    "app_state_nodes": after.nodes,
                    "loop_warning": after.loop_warning,
                });
                Ok(vec![snap_result(
                    data,
                    Some("predicate satisfied".to_string()),
                    &after,
                )])
            }
            "build_interactive_view" => {
                let app = parse_selector(params)?;
                let opts: InteractiveViewOpts = match params.get("opts") {
                    Some(v) if !v.is_null() => serde_json::from_value(v.clone()).map_err(|e| {
                        BitFunError::tool(format!(
                            "[INVALID_PARAMS] build_interactive_view 'opts' invalid: {}",
                            e
                        ))
                    })?,
                    _ => InteractiveViewOpts::default(),
                };
                let view = host.build_interactive_view(app.clone(), opts).await?;
                let view_json = build_interactive_view_json(&view);
                let summary = format!(
                    "interactive view for {} ({} elements, digest={})",
                    view.app.name,
                    view.elements.len(),
                    &view.digest[..view.digest.len().min(12)]
                );
                Ok(vec![interactive_view_result(
                    view_json,
                    Some(summary),
                    &view,
                )])
            }
            "interactive_click" => {
                let app = parse_selector(params)?;
                let p: InteractiveClickParams =
                    serde_json::from_value(params.clone()).map_err(|e| {
                        BitFunError::tool(format!(
                            "[INVALID_PARAMS] interactive_click params invalid: {}",
                            e
                        ))
                    })?;
                let i = p.i;
                let res = host.interactive_click(app.clone(), p).await?;
                let data = build_interactive_action_json(
                    &app,
                    &res,
                    json!({ "i": i, "action": "interactive_click" }),
                );
                let summary = format!("interactive_click i={}", i);
                Ok(vec![interactive_action_result(data, Some(summary), &res)])
            }
            "build_visual_mark_view" => {
                let app = parse_selector(params)?;
                let opts: VisualMarkViewOpts = match params.get("opts") {
                    Some(v) if !v.is_null() => serde_json::from_value(v.clone()).map_err(|e| {
                        BitFunError::tool(format!(
                            "[INVALID_PARAMS] build_visual_mark_view 'opts' invalid: {}",
                            e
                        ))
                    })?,
                    _ => VisualMarkViewOpts::default(),
                };
                let view = host.build_visual_mark_view(app.clone(), opts).await?;
                let view_json = build_visual_mark_view_json(&view);
                let summary = format!(
                    "visual mark view for {} ({} marks, digest={})",
                    view.app.name,
                    view.marks.len(),
                    &view.digest[..view.digest.len().min(12)]
                );
                Ok(vec![visual_mark_view_result(
                    view_json,
                    Some(summary),
                    &view,
                )])
            }
            "visual_click" => {
                let app = parse_selector(params)?;
                let p: VisualClickParams = serde_json::from_value(params.clone()).map_err(|e| {
                    BitFunError::tool(format!(
                        "[INVALID_PARAMS] visual_click params invalid: {}",
                        e
                    ))
                })?;
                let i = p.i;
                let res = host.visual_click(app.clone(), p).await?;
                let data = build_visual_action_json(
                    &app,
                    &res,
                    json!({ "i": i, "action": "visual_click" }),
                );
                let summary = format!("visual_click i={}", i);
                Ok(vec![visual_action_result(data, Some(summary), &res)])
            }
            "interactive_type_text" => {
                let app = parse_selector(params)?;
                let p: InteractiveTypeTextParams =
                    serde_json::from_value(params.clone()).map_err(|e| {
                        BitFunError::tool(format!(
                            "[INVALID_PARAMS] interactive_type_text params invalid: {}",
                            e
                        ))
                    })?;
                let i = p.i;
                let text_len = p.text.chars().count();
                let res = host.interactive_type_text(app.clone(), p).await?;
                let data = build_interactive_action_json(
                    &app,
                    &res,
                    json!({
                        "i": i,
                        "action": "interactive_type_text",
                        "text_chars": text_len,
                    }),
                );
                let summary = match i {
                    Some(idx) => format!("interactive_type_text i={} ({} chars)", idx, text_len),
                    None => format!("interactive_type_text focused ({} chars)", text_len),
                };
                Ok(vec![interactive_action_result(data, Some(summary), &res)])
            }
            "interactive_scroll" => {
                let app = parse_selector(params)?;
                let p: InteractiveScrollParams =
                    serde_json::from_value(params.clone()).map_err(|e| {
                        BitFunError::tool(format!(
                            "[INVALID_PARAMS] interactive_scroll params invalid: {}",
                            e
                        ))
                    })?;
                let (i, dx, dy) = (p.i, p.dx, p.dy);
                let res = host.interactive_scroll(app.clone(), p).await?;
                let data = build_interactive_action_json(
                    &app,
                    &res,
                    json!({
                        "i": i,
                        "dx": dx,
                        "dy": dy,
                        "action": "interactive_scroll",
                    }),
                );
                let summary = format!("interactive_scroll i={:?} dx={} dy={}", i, dx, dy);
                Ok(vec![interactive_action_result(data, Some(summary), &res)])
            }
            other => Err(BitFunError::tool(format!(
                "[INTERNAL] handle_desktop_ax called with unknown action: {}",
                other
            ))),
        }
    }

    // ── Browser domain ─────────────────────────────────────────────────

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
                let kind = BrowserLauncher::detect_default_browser()?;

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

                let user_data_dir = params
                    .get("user_data_dir")
                    .and_then(|v| v.as_str());
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

    // ── App domain (SelfControl) ───────────────────────────────────────

    async fn handle_app(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Discoverability shortcut: `execute_task` accepts a fixed catalog
        // of named recipes ("set_primary_model" etc.). The model needs to
        // know that catalog without first triggering the frontend's
        // unknown-task error path. Returning it from a pure-Rust action
        // means zero round-trip and the catalog stays in sync with what
        // the frontend actually accepts (kept in sync via the e2e test).
        if action == "list_tasks" {
            let tasks = json!([
                {
                    "name": "set_primary_model",
                    "description": "Set the primary (main) model for the active session.",
                    "params": { "modelQuery": "fuzzy match on model display name or id" },
                },
                {
                    "name": "set_fast_model",
                    "description": "Set the fast/secondary model.",
                    "params": { "modelQuery": "fuzzy match on model display name or id" },
                },
                {
                    "name": "open_model_settings",
                    "description": "Open the Settings → Models tab.",
                    "params": {},
                },
                {
                    "name": "return_to_session",
                    "description": "Switch back to the chat session scene.",
                    "params": {},
                },
                {
                    "name": "delete_model",
                    "description": "Delete a configured model.",
                    "params": { "modelQuery": "fuzzy match on model display name or id" },
                },
                {
                    "name": "enable_model",
                    "description": "Enable a configured model that is currently disabled (pure config write — no UI navigation needed). If the model is already enabled, returns a no-op message.",
                    "params": { "modelQuery": "fuzzy match on model display name or id" },
                },
                {
                    "name": "disable_model",
                    "description": "Disable a configured model. If the disabled model is the current primary/fast default, the default is automatically rolled forward to the next enabled model.",
                    "params": { "modelQuery": "fuzzy match on model display name or id" },
                },
                {
                    "name": "toggle_model",
                    "description": "Flip a model's enabled flag (enabled ⇄ disabled). Same default-rollover behaviour as disable_model when the toggled model was the active primary/fast.",
                    "params": { "modelQuery": "fuzzy match on model display name or id" },
                },
                {
                    "name": "open_live_app_gallery",
                    "description": "Open the Live Apps gallery scene (Apps overlay).",
                    "params": {},
                },
                {
                    "name": "open_live_app",
                    "description": "Open a specific installed Live App by id (use list_live_apps to discover ids).",
                    "params": { "liveAppId": "id of the Live App to open" },
                },
                {
                    "name": "open_miniapp_gallery",
                    "description": "Legacy alias for open_live_app_gallery.",
                    "params": {},
                },
                {
                    "name": "open_miniapp",
                    "description": "Legacy alias for open_live_app (same params: liveAppId / miniAppId).",
                    "params": { "miniAppId": "id of the Live App to open" },
                },
            ]);
            let count = tasks.as_array().map(|a| a.len()).unwrap_or(0);
            return Ok(vec![ToolResult::ok(
                json!({ "tasks": tasks }),
                Some(format!(
                    "{count} named tasks available for app.execute_task"
                )),
            )]);
        }

        // ── BitFun self-introspection (no frontend round-trip) ─────────
        // These actions answer "what does BitFun itself expose?" so the
        // model never needs to fall back to filesystem-scanning the user's
        // workspace to guess at the app's own capabilities.
        if action == "list_live_apps" || action == "list_miniapps" {
            let include_runtime = params
                .get("includeRuntime")
                .or_else(|| params.get("include_runtime"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            return Self::handle_list_live_apps(include_runtime).await;
        }
        if action == "list_scenes" {
            return Ok(vec![Self::scenes_tool_result()]);
        }
        if action == "list_settings_tabs" {
            return Ok(vec![Self::settings_tabs_tool_result()]);
        }
        if action == "app_self_describe" {
            return Self::handle_app_self_describe().await;
        }

        let mut sc_input = params.clone();
        if let Value::Object(ref mut map) = sc_input {
            map.insert("action".to_string(), json!(action));
        }
        let sc_tool = super::self_control_tool::SelfControlTool::new();
        sc_tool.call_impl(&sc_input, context).await
    }

    // ── BitFun self-introspection helpers ─────────────────────────────

    /// Static catalog of scenes the user can navigate to from the BitFun
    /// shell. Mirrors the entries in
    /// `src/web-ui/src/app/scenes/registry.ts::SCENE_TAB_REGISTRY`.
    /// Kept here as a static list so `app.list_scenes` can answer with
    /// zero frontend round-trip; the e2e suite asserts this list stays
    /// in sync with the TS registry.
    fn scene_catalog() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            ("welcome", "Welcome", "欢迎使用"),
            ("session", "Session (chat)", "会话"),
            ("terminal", "Terminal", "终端"),
            ("git", "Git", "Git"),
            ("settings", "Settings", "设置"),
            ("file-viewer", "File Viewer", "文件查看"),
            ("profile", "Profile", "个人资料"),
            ("agents", "Agents", "智能体"),
            ("skills", "Skills", "技能"),
            ("apps", "Live Apps", "应用"),
            ("browser", "Browser", "浏览器"),
            ("mermaid", "Mermaid Editor", "Mermaid 图表"),
            ("assistant", "Assistant", "助理"),
            ("insights", "Insights", "洞察"),
            ("shell", "Shell", "Shell"),
            ("panel-view", "Panel View", "面板视图"),
        ]
    }

    /// Settings tab catalog. Keep in sync with the settings store registry.
    fn settings_tab_catalog() -> Vec<(&'static str, &'static str)> {
        vec![
            ("basics", "Basic preferences (language, theme, etc.)"),
            ("models", "AI models (add / edit / set defaults / delete)"),
            ("session-config", "Default session behavior"),
            ("agents", "Agent management"),
            ("skills", "Skill packages"),
            ("tools", "Built-in tools and MCP servers"),
            ("about", "About BitFun"),
        ]
    }

    fn scenes_tool_result() -> ToolResult {
        let scenes: Vec<Value> = Self::scene_catalog()
            .into_iter()
            .map(|(id, label_en, label_zh)| {
                json!({ "id": id, "labelEn": label_en, "labelZh": label_zh })
            })
            .collect();
        let count = scenes.len();
        ToolResult::ok(
            json!({ "scenes": scenes }),
            Some(format!("{count} scenes available; pass any `id` to action `open_scene`. Per-app overlays use id `live-app:<appId>` (see app.list_live_apps).")),
        )
    }

    fn settings_tabs_tool_result() -> ToolResult {
        let tabs: Vec<Value> = Self::settings_tab_catalog()
            .into_iter()
            .map(|(id, desc)| json!({ "id": id, "description": desc }))
            .collect();
        let count = tabs.len();
        ToolResult::ok(
            json!({ "tabs": tabs }),
            Some(format!(
                "{count} settings tabs available; pass any `id` to action `open_settings_tab`."
            )),
        )
    }

    async fn handle_list_live_apps(include_runtime: bool) -> BitFunResult<Vec<ToolResult>> {
        let manager = match crate::live_app::try_get_global_live_app_manager() {
            Some(m) => m,
            None => {
                return Ok(vec![ToolResult::ok(
                    json!({ "liveApps": [], "available": false }),
                    Some("Live App subsystem is not initialized in this build.".to_string()),
                )]);
            }
        };

        let metas = manager
            .list()
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to list Live Apps: {e}")))?;

        let entries: Vec<Value> = metas
            .iter()
            .map(|meta| {
                let mut obj = serde_json::Map::new();
                obj.insert("id".to_string(), json!(meta.id));
                obj.insert("name".to_string(), json!(meta.name));
                obj.insert("description".to_string(), json!(meta.description));
                obj.insert("icon".to_string(), json!(meta.icon));
                obj.insert("category".to_string(), json!(meta.category));
                obj.insert("tags".to_string(), json!(meta.tags));
                obj.insert("version".to_string(), json!(meta.version));
                obj.insert("updatedAt".to_string(), json!(meta.updated_at));
                obj.insert(
                    "openSceneId".to_string(),
                    json!(format!("live-app:{}", meta.id)),
                );
                if include_runtime {
                    obj.insert(
                        "runtime".to_string(),
                        json!({
                            "sourceRevision": meta.runtime.source_revision,
                            "depsRevision": meta.runtime.deps_revision,
                            "depsDirty": meta.runtime.deps_dirty,
                            "workerRestartRequired": meta.runtime.worker_restart_required,
                        }),
                    );
                }
                Value::Object(obj)
            })
            .collect();

        let count = entries.len();
        let preview: String = metas
            .iter()
            .take(5)
            .map(|m| format!("{} (id={})", m.name, m.id))
            .collect::<Vec<_>>()
            .join(", ");
        let summary = if count == 0 {
            "No Live Apps installed.".to_string()
        } else if count <= 5 {
            format!("{count} Live App(s) installed: {preview}.")
        } else {
            format!("{count} Live App(s) installed; first 5: {preview}…")
        };

        Ok(vec![ToolResult::ok(
            json!({ "liveApps": entries, "count": count, "available": true }),
            Some(format!(
                "{summary} To open one: execute_task task=open_live_app params={{ liveAppId: <id> }}, or open_scene sceneId=live-app:<id>."
            )),
        )])
    }

    async fn handle_app_self_describe() -> BitFunResult<Vec<ToolResult>> {
        let scenes: Vec<Value> = Self::scene_catalog()
            .into_iter()
            .map(|(id, label_en, label_zh)| {
                json!({ "id": id, "labelEn": label_en, "labelZh": label_zh })
            })
            .collect();
        let settings_tabs: Vec<Value> = Self::settings_tab_catalog()
            .into_iter()
            .map(|(id, desc)| json!({ "id": id, "description": desc }))
            .collect();

        let (live_apps, live_app_available, live_app_count): (Vec<Value>, bool, usize) =
            match crate::live_app::try_get_global_live_app_manager() {
                Some(manager) => match manager.list().await {
                    Ok(metas) => {
                        let count = metas.len();
                        let entries = metas
                            .iter()
                            .map(|m| {
                                json!({
                                    "id": m.id,
                                    "name": m.name,
                                    "description": m.description,
                                    "category": m.category,
                                    "openSceneId": format!("live-app:{}", m.id),
                                })
                            })
                            .collect::<Vec<_>>();
                        (entries, true, count)
                    }
                    Err(_) => (vec![], true, 0),
                },
                None => (vec![], false, 0),
            };

        let summary = format!(
            "BitFun self-describe: {} scenes, {} settings tabs, {} Live App(s) installed.",
            scenes.len(),
            settings_tabs.len(),
            live_app_count,
        );

        Ok(vec![ToolResult::ok(
            json!({
                "scenes": scenes,
                "settingsTabs": settings_tabs,
                "liveApps": live_apps,
                "liveAppSubsystemAvailable": live_app_available,
            }),
            Some(summary),
        )])
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

    /// Returns the platform-specific command and args to open an application.
    ///
    /// Cross-platform notes:
    /// * macOS: `open -a <name>` resolves the app via LaunchServices.
    /// * Windows: `cmd /C start "" <name>` resolves through `App Paths` registry
    ///   and PATH. We deliberately keep the empty title argument (`""`) so
    ///   `start` treats the next token as the program, not as the window title.
    /// * Linux: `xdg-open` is for files/URLs, NOT for application names. We
    ///   try in order: `gtk-launch <name>` (uses `.desktop` files), then a
    ///   direct exec of the lower-cased name (handles `firefox`, `code`, etc.),
    ///   and finally fall back to `xdg-open` so callers passing a URL/path by
    ///   accident still work. The dispatcher in `handle_system` is aware of
    ///   this fallback chain.
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
            // Probe in order of correctness; the first executable on PATH wins.
            // `gtk-launch` is the canonical way to start a desktop application
            // by its .desktop id; if not present we fall back to a direct exec.
            if which_exists("gtk-launch") {
                ("gtk-launch".to_string(), vec![app_name.to_string()])
            } else {
                (app_name.to_string(), vec![])
            }
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

                // Phase 4 (p4_open_app_unify): consolidate the two historical
                // launch paths (ComputerUse host vs. raw shell `open`/`start`)
                // into one flow: prefer the host (it knows about
                // accessibility / focus-after-launch), fall back to the
                // platform shell, and *always* return the same envelope so
                // callers don't have to special-case the two paths.
                let mut host_attempted = false;
                let mut host_error: Option<String> = None;
                let method = "shell";

                // Only macOS has a working ComputerUseHost.open_app pathway today
                // (Accessibility-driven). On Windows / Linux the host either
                // doesn't exist or returns a NotImplemented stub, so we save a
                // round-trip by going straight to the platform shell. On macOS
                // we still prefer the host because it knows about
                // focus-after-launch and AX permission state.
                let prefer_host = cfg!(target_os = "macos") && context.computer_use_host.is_some();
                if prefer_host {
                    host_attempted = true;
                    let cu_input = json!({ "action": "open_app", "app_name": app_name });
                    match self.handle_desktop("open_app", &cu_input, context).await {
                        Ok(results) => {
                            // Re-wrap to the unified system-domain envelope so
                            // models see the same shape regardless of which
                            // backend serviced the call.
                            let host_payload = results
                                .first()
                                .map(|r| r.content())
                                .unwrap_or(Value::Null);
                            return Ok(vec![ToolResult::ok(
                                json!({
                                    "launched": true,
                                    "app": app_name,
                                    "method": "computer_use_host",
                                    "host_payload": host_payload,
                                }),
                                Some(format!("Opened {} via host", app_name)),
                            )]);
                        }
                        Err(e) => {
                            // Don't fail yet — try the shell fallback. Many
                            // hosts return error for sandboxed apps that
                            // launch fine via `open -a`.
                            host_error = Some(e.to_string());
                        }
                    }
                }

                // Build the platform-specific launch attempt list. On Linux
                // we try multiple strategies in order so the model doesn't
                // need to know whether the user has gtk-launch installed.
                let attempts: Vec<(String, Vec<String>)> = {
                    let primary = Self::platform_open_command(app_name);
                    #[cfg(target_os = "linux")]
                    {
                        let mut v = vec![primary];
                        // Fallback 1: direct exec of the lowercase name (handles
                        // `firefox`, `code`, `gnome-terminal`, etc. when the
                        // exec name matches the app name).
                        let lower = app_name.to_lowercase();
                        if v.iter().all(|(c, _)| c != &lower) {
                            v.push((lower, vec![]));
                        }
                        // Fallback 2: xdg-open — last-ditch, mostly for paths/URLs
                        // erroneously passed as app_name.
                        v.push(("xdg-open".to_string(), vec![app_name.to_string()]));
                        v
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        vec![primary]
                    }
                };

                let mut last_err: Option<String> = None;
                let mut output_opt = None;
                let mut chosen_cmd = String::new();
                let mut chosen_args: Vec<String> = vec![];
                for (cmd, args) in &attempts {
                    match std::process::Command::new(cmd).args(args).output() {
                        Ok(out) => {
                            if out.status.success() {
                                chosen_cmd = cmd.clone();
                                chosen_args = args.clone();
                                output_opt = Some(out);
                                break;
                            } else {
                                last_err = Some(format!(
                                    "{} exit={:?} stderr={}",
                                    cmd,
                                    out.status.code(),
                                    String::from_utf8_lossy(&out.stderr).trim()
                                ));
                            }
                        }
                        Err(e) => {
                            last_err = Some(format!("spawn {}: {}", cmd, e));
                        }
                    }
                }
                let _ = chosen_args;
                let output = output_opt.ok_or_else(|| {
                    BitFunError::tool(format!(
                        "open_app failed for '{}' across {} strategies: {} (host_error: {:?})",
                        app_name,
                        attempts.len(),
                        last_err.as_deref().unwrap_or("(no error)"),
                        host_error
                    ))
                })?;

                if output.status.success() {
                    let warning = host_error.map(|e| {
                        format!("computer_use_host open_app failed; shell fallback succeeded: {}", e)
                    });
                    Ok(vec![ToolResult::ok(
                        json!({
                            "launched": true,
                            "app": app_name,
                            "method": method,
                            "via_command": chosen_cmd,
                            "host_attempted": host_attempted,
                            "warning": warning,
                        }),
                        Some(format!("Opened {} via {}", app_name, chosen_cmd)),
                    )])
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    Err(BitFunError::tool(format!(
                        "open_app failed for '{}'. host_attempted={}, host_error={:?}, last_command='{}', stderr='{}'",
                        app_name, host_attempted, host_error, chosen_cmd, stderr
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
                // Phase 4: bound the runtime so a hung script can never wedge
                // the agent. Default 30 s, capped at 5 min to keep it sane.
                let timeout_ms = params
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30_000)
                    .clamp(100, 5 * 60 * 1000);
                // Phase 4: keep output payloads bounded — model context is
                // expensive and most scripts are happy with the head + tail.
                let max_output_bytes = params
                    .get("max_output_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(16 * 1024)
                    .clamp(1024, 256 * 1024) as usize;

                let (program, args) = match script_type {
                    "applescript" => {
                        #[cfg(target_os = "macos")]
                        {
                            (
                                "/usr/bin/osascript".to_string(),
                                vec!["-e".to_string(), script.to_string()],
                            )
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            let _ = script;
                            return Ok(err_response(
                                "system",
                                "run_script",
                                ControlHubError::new(
                                    ErrorCode::NotAvailable,
                                    "AppleScript is only available on macOS",
                                )
                                .with_hint("Use script_type='shell' (sh on Unix, PowerShell on Windows) or script_type='powershell'/'bash'"),
                            ));
                        }
                    }
                    // The "shell" alias picks the OS's *default* shell so the
                    // model can stay platform-agnostic. On Windows we now
                    // route to PowerShell rather than cmd.exe to avoid the
                    // GBK/CP936 stdout encoding nightmare and to give the
                    // model a consistent surface area.
                    "shell" => {
                        #[cfg(target_os = "windows")]
                        {
                            powershell_invocation(script)
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            (
                                "sh".to_string(),
                                vec!["-c".to_string(), script.to_string()],
                            )
                        }
                    }
                    "bash" => {
                        // Bash is universally requested but not always on
                        // PATH (Windows without WSL/git-bash). Detect and
                        // surface a structured NotAvailable instead of a
                        // confusing spawn-failure error.
                        if !which_exists("bash") {
                            return Ok(err_response(
                                "system",
                                "run_script",
                                ControlHubError::new(
                                    ErrorCode::NotAvailable,
                                    "bash is not on PATH",
                                )
                                .with_hint("Install Git for Windows / WSL, or use script_type='shell' / 'powershell' / 'cmd'"),
                            ));
                        }
                        (
                            "bash".to_string(),
                            vec!["-c".to_string(), script.to_string()],
                        )
                    }
                    "powershell" => {
                        // Prefer pwsh (PowerShell 7+, cross-platform) when
                        // available; fall back to legacy Windows powershell.
                        let prog = if which_exists("pwsh") {
                            "pwsh"
                        } else if which_exists("powershell") {
                            "powershell"
                        } else {
                            return Ok(err_response(
                                "system",
                                "run_script",
                                ControlHubError::new(
                                    ErrorCode::NotAvailable,
                                    "Neither pwsh nor powershell are on PATH",
                                )
                                .with_hint("Install PowerShell, or use script_type='shell' / 'bash'"),
                            ));
                        };
                        (
                            prog.to_string(),
                            vec![
                                "-NoProfile".to_string(),
                                "-NonInteractive".to_string(),
                                // -OutputEncoding utf8 is set inside the script
                                // wrapper below for consistent stdout handling.
                                "-Command".to_string(),
                                format!(
                                    "[Console]::OutputEncoding=[Text.Encoding]::UTF8; {}",
                                    script
                                ),
                            ],
                        )
                    }
                    "cmd" => {
                        #[cfg(target_os = "windows")]
                        {
                            // Force code-page 65001 (UTF-8) before running the
                            // user's script so stdout matches what we decode.
                            (
                                "cmd".to_string(),
                                vec![
                                    "/U".to_string(),
                                    "/C".to_string(),
                                    format!("chcp 65001>nul && {}", script),
                                ],
                            )
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            return Ok(err_response(
                                "system",
                                "run_script",
                                ControlHubError::new(
                                    ErrorCode::NotAvailable,
                                    "script_type='cmd' is only available on Windows",
                                )
                                .with_hint("Use script_type='shell' / 'bash' / 'powershell'"),
                            ));
                        }
                    }
                    other => {
                        return Err(BitFunError::tool(format!(
                            "Unknown script_type: '{}'. Valid: applescript (macOS), shell (OS default), bash, powershell, cmd (Windows)",
                            other
                        )))
                    }
                };

                // Use tokio::process so that on timeout we can actually KILL
                // the child process. The previous implementation wrapped
                // `std::process::Command::output()` in `spawn_blocking` +
                // `tokio::time::timeout`; on timeout the `timeout` future
                // returned, but the spawn_blocking thread kept blocking on
                // the still-running child, leaking a thread + process per
                // hung script.
                let started = std::time::Instant::now();
                let child = tokio::process::Command::new(&program)
                    .args(&args)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| {
                        BitFunError::tool(format!(
                            "Failed to spawn run_script ({}): {}",
                            script_type, e
                        ))
                    })?;

                let wait = child.wait_with_output();
                let output = match tokio::time::timeout(
                    std::time::Duration::from_millis(timeout_ms),
                    wait,
                )
                .await
                {
                    Err(_) => {
                        // Best-effort kill. `kill_on_drop(true)` above also
                        // ensures the OS reaps the process when `child`
                        // drops, but we issue an explicit SIGKILL first so
                        // it terminates immediately rather than after the
                        // tokio task tear-down race.
                        // NOTE: `wait_with_output` consumed `child`, so we
                        // can no longer call `child.kill()` directly here;
                        // the `kill_on_drop` flag handles it for us.
                        return Ok(err_response(
                            "system",
                            "run_script",
                            ControlHubError::new(
                                ErrorCode::Timeout,
                                format!(
                                    "run_script timed out after {} ms (script_type={}); child process killed",
                                    timeout_ms, script_type
                                ),
                            )
                            .with_hint(
                                "Increase 'timeout_ms', or split the script into shorter steps",
                            ),
                        ));
                    }
                    Ok(Err(e)) => {
                        return Err(BitFunError::tool(format!(
                            "Failed to wait for run_script ({}): {}",
                            script_type, e
                        )));
                    }
                    Ok(Ok(o)) => o,
                };

                let elapsed_ms = started.elapsed().as_millis() as u64;
                let stdout_full = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr_full = String::from_utf8_lossy(&output.stderr).to_string();
                let (stdout, stdout_truncated) = truncate_with_marker(&stdout_full, max_output_bytes);
                let (stderr, stderr_truncated) = truncate_with_marker(&stderr_full, max_output_bytes);

                if output.status.success() {
                    Ok(vec![ToolResult::ok(
                        json!({
                            "success": true,
                            "output": stdout,
                            "stderr": stderr,
                            "stdout_truncated": stdout_truncated,
                            "stderr_truncated": stderr_truncated,
                            "exit_code": output.status.code(),
                            "elapsed_ms": elapsed_ms,
                            "script_type": script_type,
                        }),
                        Some(if stdout.is_empty() {
                            format!("Script executed in {} ms", elapsed_ms)
                        } else {
                            stdout.lines().take(1).collect::<String>()
                        }),
                    )])
                } else {
                    Ok(err_response(
                        "system",
                        "run_script",
                        ControlHubError::new(
                            ErrorCode::Internal,
                            format!(
                                "Script exited with {:?}: {}",
                                output.status.code(),
                                stderr.lines().next().unwrap_or("(no stderr)")
                            ),
                        )
                        .with_hints([
                            format!("stderr={}", stderr),
                            format!("elapsed_ms={}", elapsed_ms),
                        ]),
                    ))
                }
            }
            "get_os_info" => {
                let os = std::env::consts::OS;
                let arch = std::env::consts::ARCH;
                // Phase 4: include OS version + hostname when available so
                // the model can adapt platform-specific paths / commands.
                let mut info = json!({
                    "os": os,
                    "arch": arch,
                    "rust_target_family": std::env::consts::FAMILY,
                });
                if let Some(v) = read_os_version() {
                    info["os_version"] = json!(v);
                }
                if let Ok(host) = hostname() {
                    info["hostname"] = json!(host);
                }
                // Linux-only: surface display server (X11 / Wayland) and the
                // current desktop environment so the model can pick the right
                // clipboard helper / window manipulation strategy without a
                // separate `run_script` round-trip.
                #[cfg(target_os = "linux")]
                {
                    let (display_server, desktop_env) = linux_session_info();
                    if let Some(s) = display_server {
                        info["display_server"] = json!(s);
                    }
                    if let Some(d) = desktop_env {
                        info["desktop_environment"] = json!(d);
                    }
                }
                // The set of `script_type` values the host can actually run.
                // Discoverability win: model no longer has to spawn a doomed
                // run_script call to learn that bash is missing on Windows.
                let mut script_types = vec!["shell"];
                if cfg!(target_os = "macos") {
                    script_types.push("applescript");
                }
                if which_exists("bash") {
                    script_types.push("bash");
                }
                if which_exists("pwsh") || which_exists("powershell") {
                    script_types.push("powershell");
                }
                if cfg!(target_os = "windows") {
                    script_types.push("cmd");
                }
                info["script_types"] = json!(script_types);
                Ok(vec![ToolResult::ok(
                    info.clone(),
                    Some(format!(
                        "{} {} ({})",
                        os,
                        info.get("os_version").and_then(|v| v.as_str()).unwrap_or(""),
                        arch
                    )),
                )])
            }
            // Cross-context primitive: read the system clipboard. Used by
            // models to pick up "what the user just copied" (verification
            // codes, selected text, generated SQL, etc.) without driving
            // the GUI. Returns text only — binary clipboard payloads are
            // out of scope.
            "clipboard_get" => {
                let max_bytes = params
                    .get("max_bytes")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(64 * 1024)
                    .clamp(64, 1024 * 1024);

                match clipboard_read().await {
                    Ok(text) => {
                        let (truncated, was_truncated) = truncate_with_marker(&text, max_bytes);
                        let len = text.len();
                        Ok(vec![ToolResult::ok(
                            json!({
                                "text": truncated,
                                "byte_length": len,
                                "truncated": was_truncated,
                            }),
                            Some(format!("{} bytes on clipboard", len)),
                        )])
                    }
                    Err(e) => Ok(err_response(
                        "system",
                        "clipboard_get",
                        ControlHubError::new(
                            ErrorCode::NotAvailable,
                            format!("Clipboard read failed: {}", e),
                        )
                        .with_hints(linux_clipboard_install_hints()),
                    )),
                }
            }

            // Cross-context primitive: place text on the system clipboard.
            // The user can then paste it into ANY app with cmd+v / ctrl+v —
            // dramatically simpler than driving each target GUI by hand.
            "clipboard_set" => {
                let text = params.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                    BitFunError::tool("clipboard_set requires 'text'".to_string())
                })?;
                match clipboard_write(text).await {
                    Ok(()) => Ok(vec![ToolResult::ok(
                        json!({
                            "success": true,
                            "byte_length": text.len(),
                        }),
                        Some(format!("Wrote {} bytes to clipboard", text.len())),
                    )]),
                    Err(e) => Ok(err_response(
                        "system",
                        "clipboard_set",
                        ControlHubError::new(
                            ErrorCode::NotAvailable,
                            format!("Clipboard write failed: {}", e),
                        )
                        .with_hints(linux_clipboard_install_hints()),
                    )),
                }
            }

            // Cross-context primitive: open a URL in the user's default
            // browser WITHOUT going through CDP. Use this when the goal is
            // "show this URL to the user" rather than "drive this page".
            // Avoids the CDP launch round-trip and works even when the
            // browser was started without --remote-debugging-port.
            "open_url" => {
                let url = params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| BitFunError::tool("open_url requires 'url'".to_string()))?;
                if !(url.starts_with("http://")
                    || url.starts_with("https://")
                    || url.starts_with("file://")
                    || url.starts_with("mailto:"))
                {
                    return Ok(err_response(
                        "system",
                        "open_url",
                        ControlHubError::new(
                            ErrorCode::InvalidParams,
                            format!("Refusing to open URL with unsupported scheme: {}", url),
                        )
                        .with_hint(
                            "Pass an http(s)://, file://, or mailto: URL. Use 'open_file' for local paths without a scheme.",
                        ),
                    ));
                }
                // NOTE: do NOT reuse platform_open_command — that helper
                // is for *apps* (uses `open -a` on macOS) and would treat
                // the URL as an application name, failing immediately.
                //
                // Windows: must NOT route through `cmd /C start "" <url>`.
                // `cmd` interprets `&`, `^`, `%`, `|` in the URL — so a query
                // string like `?a=1&b=2` gets the second arg dropped, and
                // long URLs may be silently truncated. Use rundll32 with the
                // URL protocol handler so the URL is passed verbatim and
                // routed through the same default-handler resolution Windows
                // uses for "Open in Browser" shell verbs.
                let (program, args) = match std::env::consts::OS {
                    "macos" => ("open".to_string(), vec![url.to_string()]),
                    "windows" => (
                        "rundll32".to_string(),
                        vec![
                            "url.dll,FileProtocolHandler".to_string(),
                            url.to_string(),
                        ],
                    ),
                    _ => ("xdg-open".to_string(), vec![url.to_string()]),
                };
                let status = std::process::Command::new(&program)
                    .args(&args)
                    .status()
                    .map_err(|e| {
                        BitFunError::tool(format!("Failed to spawn '{}': {}", program, e))
                    })?;
                if status.success() {
                    Ok(vec![ToolResult::ok(
                        json!({ "opened": true, "url": url, "method": program }),
                        Some(format!("Opened {} in default handler", url)),
                    )])
                } else {
                    Ok(err_response(
                        "system",
                        "open_url",
                        ControlHubError::new(
                            ErrorCode::Internal,
                            format!("'{}' exited with {:?}", program, status.code()),
                        ),
                    ))
                }
            }

            // Cross-context primitive: open a local file with its default
            // handler (or an explicitly named app on macOS). High-frequency
            // for "open this PDF / picture / spreadsheet for me".
            "open_file" => {
                let path_str = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                    BitFunError::tool("open_file requires 'path'".to_string())
                })?;
                let app_name = params.get("app").and_then(|v| v.as_str());

                let path = std::path::Path::new(path_str);
                if !path.exists() {
                    return Ok(err_response(
                        "system",
                        "open_file",
                        ControlHubError::new(
                            ErrorCode::NotFound,
                            format!("File does not exist: {}", path_str),
                        )
                        .with_hint("Check the absolute path; ~ is not expanded"),
                    ));
                }

                let (program, args) = match (std::env::consts::OS, app_name) {
                    ("macos", Some(app)) => (
                        "open".to_string(),
                        vec!["-a".to_string(), app.to_string(), path_str.to_string()],
                    ),
                    ("macos", None) => ("open".to_string(), vec![path_str.to_string()]),
                    // Windows file open: same rundll32 dance as open_url so
                    // paths with `&` / `%` survive intact when cmd would have
                    // mangled them. ShellExec_RunDLL also accepts file paths.
                    ("windows", _) => (
                        "rundll32".to_string(),
                        vec![
                            "url.dll,FileProtocolHandler".to_string(),
                            path_str.to_string(),
                        ],
                    ),
                    _ => ("xdg-open".to_string(), vec![path_str.to_string()]),
                };
                let status = std::process::Command::new(&program)
                    .args(&args)
                    .status()
                    .map_err(|e| {
                        BitFunError::tool(format!("Failed to spawn '{}': {}", program, e))
                    })?;
                if status.success() {
                    Ok(vec![ToolResult::ok(
                        json!({
                            "opened": true,
                            "path": path_str,
                            "with_app": app_name,
                            "method": program,
                        }),
                        Some(match app_name {
                            Some(a) => format!("Opened {} with {}", path_str, a),
                            None => format!("Opened {} with default handler", path_str),
                        }),
                    )])
                } else {
                    Ok(err_response(
                        "system",
                        "open_file",
                        ControlHubError::new(
                            ErrorCode::Internal,
                            format!("'{}' exited with {:?}", program, status.code()),
                        ),
                    ))
                }
            }

            other => Err(BitFunError::tool(format!(
                "Unknown system action: '{}'. Valid: open_app, run_script, get_os_info, open_url, open_file, clipboard_get, clipboard_set",
                other
            ))),
        }
    }
}

/// Truncate `s` to at most `max_bytes`, appending an explicit marker so the
/// model can see that data was dropped (and how much). Returns
/// `(truncated_string, was_truncated)`.
fn truncate_with_marker(s: &str, max_bytes: usize) -> (String, bool) {
    if s.len() <= max_bytes {
        return (s.to_string(), false);
    }
    let head_n = max_bytes.saturating_sub(64);
    let head = safe_str_slice(s, head_n);
    let omitted = s.len().saturating_sub(head_n);
    (
        format!("{}\n... [{} bytes omitted] ...\n", head, omitted),
        true,
    )
}

/// Parse a leading `"[CODE] rest"` prefix produced by the front-end
/// `SelfControlEventListener` so we can recover the structured `ErrorCode`
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

/// Slice `s` to ≤ `n` bytes without splitting a UTF-8 codepoint.
fn safe_str_slice(s: &str, n: usize) -> &str {
    if n >= s.len() {
        return s;
    }
    let mut cut = n;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    &s[..cut]
}

/// Read a short OS version string. Best-effort: returns `None` on platforms
/// where we can't determine it cheaply.
fn read_os_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(format!("macOS {}", s))
        }
    }
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
    #[cfg(target_os = "linux")]
    {
        // /etc/os-release is the canonical lookup.
        let txt = std::fs::read_to_string("/etc/os-release").ok()?;
        for line in txt.lines() {
            if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                return Some(rest.trim_matches('"').to_string());
            }
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

fn hostname() -> std::io::Result<String> {
    // Prefer environment variables on each OS so we never have to spawn a
    // subprocess for a value that's already in our address space, and so we
    // never ingest a non-UTF-8 byte stream from `hostname.exe` on Windows
    // running a CJK code page.
    #[cfg(target_os = "windows")]
    {
        if let Ok(name) = std::env::var("COMPUTERNAME") {
            if !name.is_empty() {
                return Ok(name);
            }
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Ok(name) = std::env::var("HOSTNAME") {
            if !name.is_empty() {
                return Ok(name);
            }
        }
        if let Ok(bytes) = std::fs::read("/etc/hostname") {
            let s = String::from_utf8_lossy(&bytes).trim().to_string();
            if !s.is_empty() {
                return Ok(s);
            }
        }
    }
    let out = std::process::Command::new("hostname").output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Cheap PATH lookup for an executable name. Used to decide between e.g.
/// `pwsh` and `powershell`, or to surface a structured `NOT_AVAILABLE`
/// error when the requested interpreter isn't installed.
fn which_exists(name: &str) -> bool {
    let paths = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let exts: Vec<String> = if cfg!(target_os = "windows") {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.BAT;.CMD;.COM".to_string())
            .split(';')
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&paths) {
        for ext in &exts {
            let mut candidate = dir.join(name);
            if !ext.is_empty() {
                let stem = candidate.file_name().map(|n| n.to_os_string());
                if let Some(mut stem) = stem {
                    stem.push(ext);
                    candidate.set_file_name(stem);
                }
            }
            if candidate.exists() {
                return true;
            }
        }
    }
    false
}

/// Build a `(program, args)` pair for invoking a PowerShell snippet on Windows
/// with UTF-8 output forced. Centralised so the "shell" alias and an explicit
/// `script_type='powershell'` produce the same encoding.
#[cfg(target_os = "windows")]
fn powershell_invocation(script: &str) -> (String, Vec<String>) {
    let prog = if which_exists("pwsh") {
        "pwsh"
    } else {
        "powershell"
    };
    (
        prog.to_string(),
        vec![
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            format!(
                "[Console]::OutputEncoding=[Text.Encoding]::UTF8; {}",
                script
            ),
        ],
    )
}

/// Build OS-specific install hints for the clipboard helper. On Linux we
/// inspect the session type so the suggestion matches what the user actually
/// needs (Wayland users wasting time installing xclip is a real failure mode).
fn linux_clipboard_install_hints() -> Vec<String> {
    match std::env::consts::OS {
        "linux" => {
            #[cfg(target_os = "linux")]
            {
                let (server, _) = linux_session_info();
                match server.as_deref() {
                    Some("wayland") => vec![
                        "Wayland session detected — install wl-clipboard (e.g. `sudo apt install wl-clipboard` / `sudo dnf install wl-clipboard`)".to_string(),
                        "Fallback for XWayland apps: also install xclip or xsel".to_string(),
                    ],
                    Some("x11") | Some("tty") => vec![
                        "X11 session detected — install xclip (`sudo apt install xclip`) or xsel (`sudo apt install xsel`)".to_string(),
                    ],
                    _ => vec![
                        "Install wl-clipboard (Wayland) OR xclip/xsel (X11). Run `echo $XDG_SESSION_TYPE` to know which one applies.".to_string(),
                    ],
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                vec!["Install wl-clipboard (Wayland) or xclip/xsel (X11)".to_string()]
            }
        }
        _ => vec!["Make sure the system clipboard helper is available on this host".to_string()],
    }
}

/// Best-effort detection of the Linux desktop session metadata (display
/// server + desktop environment). Returns `(display_server, desktop_env)`,
/// either of which may be `None` if the environment doesn't expose it.
#[cfg(target_os = "linux")]
fn linux_session_info() -> (Option<String>, Option<String>) {
    let server = std::env::var("XDG_SESSION_TYPE")
        .ok()
        .filter(|s| !s.is_empty());
    let de = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .or_else(|| std::env::var("DESKTOP_SESSION").ok())
        .filter(|s| !s.is_empty());
    (server, de)
}

/// Cross-platform clipboard read. Shells out to the canonical helper for
/// the current OS so we don't pull in a heavyweight dependency for what is
/// fundamentally a 1-line operation. Linux auto-detects Wayland → X11.
async fn clipboard_read() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let out = tokio::process::Command::new("pbpaste")
            .output()
            .await
            .map_err(|e| format!("spawn pbpaste: {}", e))?;
        if !out.status.success() {
            return Err(format!("pbpaste exit={:?}", out.status.code()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
    #[cfg(target_os = "windows")]
    {
        let out = tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-Clipboard -Raw"])
            .output()
            .await
            .map_err(|e| format!("spawn powershell: {}", e))?;
        if !out.status.success() {
            return Err(format!("Get-Clipboard exit={:?}", out.status.code()));
        }
        // PowerShell appends CRLF; trim a single trailing newline so the
        // returned text matches what the user actually copied.
        let mut s = String::from_utf8_lossy(&out.stdout).to_string();
        if s.ends_with("\r\n") {
            s.truncate(s.len() - 2);
        } else if s.ends_with('\n') {
            s.truncate(s.len() - 1);
        }
        Ok(s)
    }
    #[cfg(target_os = "linux")]
    {
        // Wayland first (modern session), then X11 fallbacks.
        let candidates: &[(&str, &[&str])] = if std::env::var("WAYLAND_DISPLAY").is_ok() {
            &[
                ("wl-paste", &["--no-newline"]),
                ("xclip", &["-selection", "clipboard", "-o"]),
                ("xsel", &["--clipboard", "--output"]),
            ]
        } else {
            &[
                ("xclip", &["-selection", "clipboard", "-o"]),
                ("xsel", &["--clipboard", "--output"]),
                ("wl-paste", &["--no-newline"]),
            ]
        };
        for (bin, args) in candidates {
            if let Ok(out) = tokio::process::Command::new(bin).args(*args).output().await {
                if out.status.success() {
                    return Ok(String::from_utf8_lossy(&out.stdout).to_string());
                }
            }
        }
        Err("no clipboard helper found (install wl-clipboard, xclip, or xsel)".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("clipboard not implemented for this OS".to_string())
    }
}

/// Cross-platform clipboard write. Streams `text` into the helper's stdin
/// rather than embedding it in argv so newlines / quotes / shell metachars
/// are preserved verbatim.
async fn clipboard_write(text: &str) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    async fn pipe(bin: &str, args: &[&str], text: &str) -> Result<(), String> {
        let mut child = tokio::process::Command::new(bin)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn {}: {}", bin, e))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .await
                .map_err(|e| format!("write {} stdin: {}", bin, e))?;
        }
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| format!("wait {}: {}", bin, e))?;
        if !out.status.success() {
            return Err(format!("{} exit={:?}", bin, out.status.code()));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        pipe("pbcopy", &[], text).await
    }
    #[cfg(target_os = "windows")]
    {
        // PowerShell's Set-Clipboard reads from the pipeline; pipe text in
        // via stdin to preserve binary fidelity.
        pipe(
            "powershell",
            &["-NoProfile", "-Command", "$input | Set-Clipboard"],
            text,
        )
        .await
    }
    #[cfg(target_os = "linux")]
    {
        let candidates: &[(&str, &[&str])] = if std::env::var("WAYLAND_DISPLAY").is_ok() {
            &[
                ("wl-copy", &[]),
                ("xclip", &["-selection", "clipboard"]),
                ("xsel", &["--clipboard", "--input"]),
            ]
        } else {
            &[
                ("xclip", &["-selection", "clipboard"]),
                ("xsel", &["--clipboard", "--input"]),
                ("wl-copy", &[]),
            ]
        };
        let mut last_err = String::new();
        for (bin, args) in candidates {
            match pipe(bin, args, text).await {
                Ok(()) => return Ok(()),
                Err(e) => last_err = e,
            }
        }
        Err(format!(
            "no clipboard helper succeeded (install wl-clipboard, xclip, or xsel): {}",
            last_err
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = text;
        Err("clipboard not implemented for this OS".to_string())
    }
}

#[async_trait]
impl Tool for ControlHubTool {
    fn name(&self) -> &str {
        "ControlHub"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(Self::description_text(Self::desktop_domain_enabled().await))
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(Self::description_text(Self::desktop_domain_enabled().await))
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "enum": ["browser", "desktop", "app", "terminal", "system", "meta"],
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
                    .with_hint("Set domain to one of: app, browser, desktop, terminal, system."),
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

    // Frontend SelfControl sends back `[CODE] message\nHints: a | b` strings —
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
        for d in ["desktop", "browser", "app", "terminal", "system", "meta"] {
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
        for d in ["desktop", "browser", "app", "terminal", "system", "meta"] {
            assert!(
                domains.get(d).is_some(),
                "domain {d} missing from capabilities payload: {payload}"
            );
        }
        // Without a desktop host wired into the test context, desktop/app/terminal
        // must report unavailable so the model doesn't waste turns calling them.
        assert_eq!(
            domains
                .get("desktop")
                .and_then(|v| v.get("available"))
                .and_then(|v| v.as_bool()),
            Some(false),
            "desktop must be unavailable without a host"
        );
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

    #[tokio::test]
    async fn app_list_scenes_returns_known_scene_ids() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("app", "list_scenes", &json!({}), &ctx)
            .await
            .expect("list_scenes should succeed");
        let payload = results.first().unwrap().content();
        let arr = payload.get("scenes").and_then(|v| v.as_array()).unwrap();
        let ids: Vec<&str> = arr
            .iter()
            .filter_map(|s| s.get("id").and_then(|v| v.as_str()))
            .collect();
        for must_have in ["session", "settings", "apps", "welcome"] {
            assert!(
                ids.iter().any(|id| *id == must_have),
                "scene `{must_have}` missing from list_scenes catalog: {ids:?}"
            );
        }
    }

    #[tokio::test]
    async fn app_list_settings_tabs_returns_models_tab() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("app", "list_settings_tabs", &json!({}), &ctx)
            .await
            .expect("list_settings_tabs should succeed");
        let payload = results.first().unwrap().content();
        let arr = payload.get("tabs").and_then(|v| v.as_array()).unwrap();
        assert!(arr
            .iter()
            .any(|t| t.get("id").and_then(|v| v.as_str()) == Some("models")));
    }

    #[tokio::test]
    async fn app_list_live_apps_returns_unavailable_when_subsystem_absent() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("app", "list_live_apps", &json!({}), &ctx)
            .await
            .expect("list_live_apps should succeed even without subsystem");
        let payload = results.first().unwrap().content();
        // Without a global LiveAppManager the action must succeed-with-empty
        // and signal availability=false, NOT error out — otherwise the model
        // would assume the action itself is broken.
        assert_eq!(
            payload.get("available").and_then(|v| v.as_bool()),
            Some(false)
        );
        let arr = payload.get("liveApps").and_then(|v| v.as_array()).unwrap();
        assert!(arr.is_empty());
    }

    #[tokio::test]
    async fn app_self_describe_includes_scenes_settings_and_live_apps_keys() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("app", "app_self_describe", &json!({}), &ctx)
            .await
            .expect("app_self_describe should succeed");
        let payload = results.first().unwrap().content();
        for key in [
            "scenes",
            "settingsTabs",
            "liveApps",
            "liveAppSubsystemAvailable",
        ] {
            assert!(
                payload.get(key).is_some(),
                "self-describe payload missing `{key}`: {payload}"
            );
        }
    }

    #[tokio::test]
    async fn app_list_tasks_includes_open_live_app_recipes() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("app", "list_tasks", &json!({}), &ctx)
            .await
            .expect("list_tasks should succeed");
        let payload = results.first().unwrap().content();
        let names: Vec<String> = payload
            .get("tasks")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .filter_map(|t| {
                t.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        for required in [
            "open_live_app_gallery",
            "open_live_app",
            "set_primary_model",
        ] {
            assert!(
                names.iter().any(|n| n == required),
                "task `{required}` missing from execute_task catalog: {names:?}"
            );
        }
    }

    #[tokio::test]
    async fn route_hint_picks_app_for_bitfun_intent() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "meta",
                "route_hint",
                &json!({ "intent": "切换 BitFun 默认模型" }),
                &ctx,
            )
            .await
            .unwrap();
        let payload = results.first().unwrap().content();
        let arr = payload.get("ranked").and_then(|v| v.as_array()).unwrap();
        assert!(arr
            .iter()
            .any(|s| s.get("domain").and_then(|v| v.as_str()) == Some("app")));
    }

    #[test]
    fn parse_bracket_code_prefix_extracts_code_and_rest() {
        // Standard SelfControl frontend shape.
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
            "app",
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
            "app",
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
    async fn description_advertises_paste_as_canonical_text_input() {
        // Regression guard: the prompt-side guidance and the tool-side
        // description must both surface `paste` so the model picks it
        // over `type_text` for CJK / emoji / IM messages.
        let desc = ControlHubTool::new().description().await.unwrap();
        assert!(
            desc.contains("`paste"),
            "description must call out `paste` as a first-class action"
        );
        assert!(
            desc.contains("PREFER") || desc.contains("prefer") || desc.contains("STRONGLY"),
            "description must steer the model AWAY from type_text for non-trivial input"
        );
    }

    #[tokio::test]
    async fn description_documents_two_browser_modes_and_forbids_desktop_browser_automation() {
        let desc = ControlHubTool::new().description().await.unwrap();
        assert!(
            desc.contains("Two browser modes"),
            "description must describe the two browser control modes"
        );
        assert!(
            desc.contains("mode: \"headless\"") && desc.contains("mode: \"default\""),
            "description must mention both browser connect modes"
        );
        assert!(
            desc.contains("Do **not** use `domain: \"desktop\"` mouse/keyboard actions to drive a browser."),
            "description must explicitly forbid desktop browser automation"
        );
    }

    #[tokio::test]
    async fn desktop_paste_without_host_returns_clean_error() {
        // In `cargo test -p bitfun-core` there is no ComputerUseHost
        // (desktop runtime not booted). The tool must surface a structured
        // error rather than panicking, so the model knows desktop control
        // is unavailable on this transport.
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let err = tool
            .dispatch(
                "desktop",
                "paste",
                &json!({ "text": "hi", "submit": true }),
                &ctx,
            )
            .await
            .expect_err("must fail without ComputerUseHost");
        assert!(
            err.to_string().contains("Desktop control"),
            "expected desktop-host availability hint, got: {}",
            err
        );
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
            hints.iter().any(|v| v.as_str().unwrap_or("").contains("headless")),
            "expected headless guidance in hints: {}",
            payload
        );
    }

    #[tokio::test]
    async fn system_open_url_rejects_unsupported_scheme() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "system",
                "open_url",
                &json!({ "url": "javascript:alert(1)" }),
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
    async fn system_open_file_returns_not_found_for_missing_path() {
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "system",
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

        // system.script_types must always include `shell`.
        let script_types = payload
            .get("domains")
            .and_then(|d| d.get("system"))
            .and_then(|s| s.get("script_types"))
            .and_then(|v| v.as_array())
            .expect("system.script_types missing");
        assert!(
            script_types.iter().any(|s| s.as_str() == Some("shell")),
            "script_types must include 'shell': {script_types:?}"
        );
        // On macOS we must additionally see applescript.
        if cfg!(target_os = "macos") {
            assert!(
                script_types
                    .iter()
                    .any(|s| s.as_str() == Some("applescript")),
                "macOS host must advertise applescript: {script_types:?}"
            );
        }
        // On Windows we must additionally see cmd.
        if cfg!(target_os = "windows") {
            assert!(
                script_types.iter().any(|s| s.as_str() == Some("cmd")),
                "Windows host must advertise cmd: {script_types:?}"
            );
        }

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
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch("system", "get_os_info", &json!({}), &ctx)
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
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let results = tool
            .dispatch(
                "system",
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
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let err = tool
            .dispatch(
                "system",
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
        let tool = ControlHubTool::new();
        let ctx = empty_context();
        let probe = if cfg!(target_os = "windows") {
            // PowerShell prints with the Unicode code page configured above.
            "Write-Output 'hello-bitfun'"
        } else {
            "echo hello-bitfun"
        };
        let results = tool
            .dispatch(
                "system",
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
