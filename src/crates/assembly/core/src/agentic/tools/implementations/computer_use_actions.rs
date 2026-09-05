//! Computer Use desktop and OS/system action implementations.
//!
//! This module owns the action logic that used to live behind ControlHub's
//! desktop/system domains. ControlHub may still share the common error envelope
//! types, but it no longer owns these Computer Use behaviors.

use crate::agentic::tools::computer_use_host::{
    AppClickParams, AppSelector, AppWaitPredicate, ClickTarget, ComputerUseForegroundApplication,
    ComputerUseHostRef, InteractiveClickParams, InteractiveScrollParams, InteractiveTypeTextParams,
    InteractiveViewOpts, VisualClickParams, VisualMarkViewOpts,
};
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use openbitfun_services_core::system::{
    truncate_with_marker, LocalSystemActionError, LocalSystemProvider, RunScriptRequest,
};
use serde_json::{json, Value};

use super::control_hub::{coded_tool_error, err_response, ControlHubError, ErrorCode};

/// Per-PID consecutive-failure tracker for the AX-first `app_*` actions.
/// Key = target PID, value = `(target_signature, before_digest, count)`.
/// When the same `(action,target)` lands on an unchanged digest twice in a
/// row the dispatcher injects an `app_state.loop_warning` so the model is
/// forced off the failing path on its **next** turn (see the observe → act →
/// verify guidance in `computer_use_mode.md`).
type AppLoopTracker =
    std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i32, (String, String, u32)>>>;

static APP_LOOP_TRACKER: AppLoopTracker = std::sync::OnceLock::new();

fn loop_tracker_observe(
    pid: Option<i32>,
    action: &str,
    target_sig: &str,
    before_digest: &str,
    after_digest: &str,
    text_only: bool,
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
        // The primary model cannot consume screenshot images, so the classic
        // "just take a screenshot to see what's wrong" recovery is **not**
        // available — pointing the model at `screenshot` here would send it
        // into a hard-reject loop (the `screenshot` action is gated to
        // multimodal providers). Route it to the text-only observation +
        // targeting fallbacks instead so the agent always has a live path.
        let recovery = if text_only {
            "NEXT TURN you MUST switch tactic (do NOT call `screenshot` — the primary model is text-only and that action is rejected): \
             (1) re-run `get_app_state` for the frontmost app and pick a different `node_idx` (or `text_contains` / `title_contains` / `role_substring`), \
             (2) locate the visible text with `move_to_text` + `move_to_text_match_index`, or `click_target` with `target_text`, \
             (3) drive the app with `key_chord` shortcuts (e.g. command+F search, Tab focus, Return confirm), \
             (4) for messaging apps use `paste` (clipboard) + `key_chord` to submit, or `run_apple_script` (macOS) to drive the app directly."
        } else {
            "NEXT TURN you MUST: (1) run `desktop.screenshot { screenshot_window: false }` to see the full display, (2) switch tactic — different `node_idx`, different `ocr_text` needle, or a keyboard shortcut."
        };
        Some(format!(
            "Detected {} consecutive `{}` calls on the same target ({}) without any AX tree mutation (digest unchanged). The target is almost certainly invisible / disabled / in a Canvas-WebGL surface that AX cannot describe. {}",
            entry.2, action, target_sig, recovery
        ))
    } else {
        None
    }
}

/// Routing note attached to successful `system.open_url` results. The URL
/// opens in the user's default browser, which the agent can neither observe
/// nor control — without this note models routinely follow `open_url` with
/// desktop clicks at the browser window (exactly what the desktop browser
/// guard rejects).
const OPEN_URL_ROUTING_NOTE: &str = "The page is now open in the user's default browser; \
     OpenBitFun cannot observe or control that window. To read or interact with the page yourself, \
     use ControlHub domain=\"browser\" instead (browser.connect, then snapshot).";

/// Routing note attached to successful `system.open_file` results. The file
/// opens in an external application window the agent cannot see from this
/// result alone.
const OPEN_FILE_ROUTING_NOTE: &str = "The file is now open in an external application window; \
     this result gives no view into it. To interact with that window, use ComputerUse desktop \
     actions (take a screenshot first to observe it).";

/// Chromium-family **application identities**: macOS bundle ids and executable
/// basenames. Matched whole (bundle ids additionally match their channel
/// suffixes, e.g. `com.google.chrome.canary`) — never as a bare substring,
/// because "arc" is a substring of "search" and "edge" of "knowledge".
const CHROMIUM_APP_IDENTITIES: &[&str] = &[
    // macOS bundle ids
    "com.google.chrome",
    "org.chromium.chromium",
    "com.microsoft.edgemac",
    "com.brave.browser",
    "company.thebrowser.browser",
    // Windows / Linux executable basenames
    "chrome",
    "chrome.exe",
    "chromium",
    "chromium.exe",
    "google-chrome",
    "msedge",
    "msedge.exe",
    "brave",
    "brave.exe",
    "brave-browser",
    "arc",
    "arc.exe",
];

/// Product tokens unambiguous enough to identify a Chromium browser from a
/// human-readable name alone. "edge" and "arc" are ordinary English words and
/// are handled separately (see [`ComputerUseActions::name_suggests_chromium`]).
const CHROMIUM_NAME_TOKENS: &[&str] = &["chrome", "chromium", "brave"];

pub(crate) struct ComputerUseActions;

impl Default for ComputerUseActions {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerUseActions {
    pub(crate) fn new() -> Self {
        Self
    }

    fn desktop_browser_guard_error(
        action: &str,
        foreground: Option<&ComputerUseForegroundApplication>,
    ) -> ControlHubError {
        let app_name = foreground
            .and_then(|app| app.name.as_deref())
            .unwrap_or("a Chromium-family browser");
        ControlHubError::new(
            ErrorCode::GuardRejected,
            format!(
                "ComputerUse `{}` is blocked because it would drive {}, a Chromium-family browser — not because your task is browser-related. No ComputerUse input action may drive such a browser, including the `app_*`, `interactive_*` and `visual_*` variants; use ControlHub domain=\"browser\" instead. Non-Chromium browsers (Firefox/Safari) and native apps stay desktop-controllable.",
                action, app_name
            ),
        )
        .with_hints([
            "If your target is NOT the browser: the guard only looks at the app this action would drive, so switch focus with `key_chord` [\"alt\",\"tab\"] / [\"command\",\"tab\"] (never guarded) or `open_app`, or skip focus entirely and pass an explicit non-browser `app` selector ({pid|bundle_id|name}, from `list_apps`) to `app_click` / `app_type_text` / `app_scroll` / `app_key_chord`",
            "Page content: call ControlHub browser.connect first — Chrome 144+ and Edge use a user-approved connection to the current real profile; other supported Chromium browsers reuse a real-profile endpoint when available and otherwise fall back to OpenBitFun's persistent managed profile — then drive the page with snapshot/click/fill/press_key",
            "Browser chrome (address bar, tabs, back/forward, reload, downloads): use browser.navigate / tab_new / switch_page / back / forward / reload / close instead of mouse+keyboard",
            "File picker or <input type=file>: do NOT drive the native dialog — use browser.set_file_input_files { selector, files: [\"/abs/path\"] }. For JS alert/confirm/prompt use browser.dialog",
            "For Chrome or Edge login/cookies/extensions, keep using the guarded CDP path; for one-time setup, ask the user to click Enable default CDP in OpenBitFun Settings > Browser control, enable Remote debugging in the browser-owned page, and approve OpenBitFun",
            "For isolated project Web UI testing, use the headless browser flow instead of desktop automation",
        ])
    }

    /// Structured app identity: macOS bundle id, or the executable basename on
    /// platforms that report one. Deliberately **not** the display name: on
    /// Windows `foreground.name` is the foreground *window title*
    /// (`GetWindowTextW`), which is user content, not an app identity.
    fn app_identity(foreground: &ComputerUseForegroundApplication) -> Option<&str> {
        foreground
            .bundle_id
            .as_deref()
            .or(foreground.process_name.as_deref())
            .map(str::trim)
            .filter(|id| !id.is_empty())
    }

    fn identity_is_chromium(identity: &str) -> bool {
        let id = identity.trim().to_ascii_lowercase();
        CHROMIUM_APP_IDENTITIES.iter().any(|known| {
            id == *known
                // Channel variants: com.google.chrome.canary, com.brave.browser.beta …
                || (known.contains('.')
                    && !known.ends_with(".exe")
                    && id.starts_with(&format!("{known}.")))
        })
    }

    /// Whole-token match on a human-readable app name or window title, used only
    /// when no structured identity is available. Substring matching is not an
    /// option here: "Search" contains "arc", "Knowledge Base" contains "edge".
    fn name_suggests_chromium(name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        let tokens: Vec<&str> = name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .collect();
        tokens.iter().any(|token| CHROMIUM_NAME_TOKENS.contains(token))
            // "edge" needs the product phrase, "arc" the trailing-app-name shape
            // Chromium browsers give their windows ("Page title — Arc").
            || tokens.windows(2).any(|pair| matches!(pair, ["microsoft", "edge"]))
            || matches!(tokens.last(), Some(&"arc"))
    }

    fn is_probably_browser_app(foreground: &ComputerUseForegroundApplication) -> bool {
        // Only Chromium-family browsers are guarded: they are the only ones the
        // ControlHub browser domain can drive over CDP. Firefox/Safari (and other
        // non-Chromium browsers) have no CDP path, so desktop control must stay
        // allowed for them — blocking both surfaces would leave no control path.
        match Self::app_identity(foreground) {
            Some(identity) => Self::identity_is_chromium(identity),
            None => Self::name_suggests_chromium(foreground.name.as_deref().unwrap_or("")),
        }
    }

    /// Identifiers carried by an explicit `app` selector. `{"pid":N}` carries
    /// none, so it cannot be classified here.
    fn selector_labels(app: &Value) -> Vec<&str> {
        match app {
            Value::String(name) => vec![name.as_str()],
            Value::Object(_) => ["name", "bundle_id"]
                .iter()
                .filter_map(|key| app.get(*key).and_then(Value::as_str))
                .filter(|label| !label.trim().is_empty())
                .collect(),
            _ => Vec::new(),
        }
    }

    fn selector_is_chromium(app: &Value) -> bool {
        Self::selector_labels(app)
            .iter()
            .any(|label| Self::identity_is_chromium(label) || Self::name_suggests_chromium(label))
    }

    /// `alt+tab` / `command+tab` and their shift variants: the OS app switcher.
    /// It is the only way to move focus off a browser with the keyboard, so
    /// guarding it would leave a non-browser task with no way to reach its
    /// target app.
    fn is_focus_switch_chord(action: &str, params: &Value) -> bool {
        if action != "key_chord" {
            return false;
        }
        let Some(keys) = params.get("keys").and_then(Value::as_array) else {
            return false;
        };
        let keys: Vec<String> = keys
            .iter()
            .filter_map(Value::as_str)
            .map(|key| key.trim().to_ascii_lowercase())
            .collect();
        keys.iter().any(|key| key == "tab")
            && keys.iter().all(|key| {
                matches!(
                    key.as_str(),
                    "tab"
                        | "alt"
                        | "option"
                        | "command"
                        | "cmd"
                        | "meta"
                        | "super"
                        | "shift"
                        | "control"
                        | "ctrl"
                )
            })
    }

    /// Rejects physical input actions that would drive a CDP-drivable browser.
    /// Read-only observation actions (`screenshot`, `locate`, `describe_screen`,
    /// `get_app_state`, `build_*_view`, …) and scripts stay allowed. Called by
    /// `ComputerUseTool::call_impl` before dispatch.
    pub(crate) async fn desktop_action_targets_browser(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> Option<ControlHubError> {
        // Every action that produces physical input, app-scoped and
        // interactive/visual variants included: guarding only the frontmost
        // primitives would let the model bypass the boundary by renaming the
        // same click (`app_click` with an explicit browser selector).
        const GUARDED_ACTIONS: &[&str] = &[
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
            "move_to_text",
            "app_click",
            "app_type_text",
            "app_scroll",
            "app_key_chord",
            "interactive_click",
            "interactive_type_text",
            "interactive_scroll",
            "visual_click",
        ];
        if !GUARDED_ACTIONS.contains(&action) || Self::is_focus_switch_chord(action, params) {
            return None;
        }
        if let Some(app) = params.get("app") {
            if Self::selector_is_chromium(app) {
                return Some(Self::desktop_browser_guard_error(action, None));
            }
            // A selector naming another app drives that app whatever is
            // frontmost — and answering from the selector alone also skips the
            // host round-trip. A pid-only selector names nothing: fall through.
            if !Self::selector_labels(app).is_empty() {
                return None;
            }
        }
        let host = context.computer_use_host.as_ref()?;
        let snapshot = host.computer_use_session_snapshot().await;
        let foreground = snapshot.foreground_application.as_ref()?;
        if Self::is_probably_browser_app(foreground) {
            return Some(Self::desktop_browser_guard_error(action, Some(foreground)));
        }
        None
    }
    // ── Desktop domain ─────────────────────────────────────────────────

    pub(crate) async fn handle_desktop(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            OpenBitFunError::tool(
                "Desktop control is only available in the OpenBitFun desktop app".to_string(),
            )
        })?;

        // Legacy desktop implementation shared by the dedicated ComputerUse
        // tool while ControlHub's public desktop domain remains disabled.
        match action {
            "list_displays" => {
                let displays = host.list_displays().await?;
                let active = host.focused_display_id();
                let count = displays.len();
                return Ok(vec![ToolResult::ok(
                    json!({
                        "displays": displays,
                        "active_display_id": active,
                    }),
                    Some(format!("{} display(s) detected", count)),
                )]);
            }
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
                        coded_tool_error(ErrorCode::InvalidParams, "desktop.paste requires 'text'\nHints: example { \"action\":\"paste\", \"text\":\"hello\", \"submit\":true }")
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

                if let Err(e) = LocalSystemProvider::new().clipboard_write_text(text).await {
                    return Ok(local_system_error_response("desktop", "paste", e));
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
                // A paste lands in whatever already had focus and never moves
                // the pointer, so it is not a reason to demand a fresh capture.
                // This must run for *every* paste, not just `submit: true`:
                // pasting and then sending a separate Enter `key_chord` is the
                // common shape, and leaving the guard armed refuses that Enter
                // with advice ("call `screenshot` first") that a text-only model
                // cannot act on.
                host.computer_use_trust_pointer_after_text_input();
                if submit {
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
            // These operate on the typed AppSelector / AxNode envelope.
            "list_apps"
            | "get_app_state"
            | "get_app_shortcuts"
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
                let text_only = !context.primary_model_supports_image_understanding();
                return self
                    .handle_desktop_ax(host, action, params, text_only)
                    .await;
            }
            "focus_display" => {
                // Accept `null` (or omitted `display_id`) to clear the pin
                // and fall back to "screen under the pointer". An explicit
                // numeric id pins that display until cleared.
                let display_id = match params.get("display_id") {
                    Some(Value::Null) | None => None,
                    Some(v) => Some(v.as_u64().ok_or_else(|| {
                        OpenBitFunError::tool(
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

        // UX shortcut: every screen-coordinate action accepts an optional
        // `display_id`. If present (and different from the currently pinned
        // display), pin it BEFORE forwarding so the model doesn't need a
        // separate `focus_display` round-trip. Pin is sticky — subsequent
        // actions on the same screen don't need to re-specify. Pass
        // `display_id: null` to clear the pin in the same call.
        if let Some(v) = params.get("display_id") {
            let target = match v {
                Value::Null => None,
                v => Some(v.as_u64().ok_or_else(|| {
                    OpenBitFunError::tool(
                        "display_id must be a non-negative integer or null".to_string(),
                    )
                })? as u32),
            };
            if host.focused_display_id() != target {
                host.focus_display(target).await?;
            }
        }

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
    // unified envelope: `target_app`, `background_input`, `before_digest`
    // and (for state queries) `app_state`, whose `tree_text` is the single
    // rendering of the AX tree — so the model can reason about state
    // before/after each action without re-querying.
    async fn handle_desktop_ax(
        &self,
        host: &ComputerUseHostRef,
        action: &str,
        params: &Value,
        text_only: bool,
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        // ── Helpers ─────────────────────────────────────────────────
        fn parse_selector(v: &Value) -> OpenBitFunResult<AppSelector> {
            let obj = v.get("app").ok_or_else(|| {
                coded_tool_error(
                    ErrorCode::InvalidParams,
                    "missing 'app' selector (pid|bundle_id|name)",
                )
            })?;
            let sel: AppSelector = serde_json::from_value(obj.clone()).map_err(|e| {
                coded_tool_error(
                    ErrorCode::InvalidParams,
                    format!("bad 'app' selector: {} (expect {{pid|bundle_id|name}})", e),
                )
            })?;
            if sel.pid.is_none() && sel.bundle_id.is_none() && sel.name.is_none() {
                return Err(coded_tool_error(
                    ErrorCode::InvalidParams,
                    "'app' must include at least one of pid|bundle_id|name",
                ));
            }
            Ok(sel)
        }

        fn parse_click_target(v: &Value) -> OpenBitFunResult<ClickTarget> {
            if v.get("kind").is_some() {
                return serde_json::from_value(v.clone()).map_err(|e| {
                    coded_tool_error(ErrorCode::InvalidParams, format!("bad ClickTarget: {} (expected {{\"kind\":\"node_idx\", \"idx\":N}}, {{\"kind\":\"image_xy\",\"x\":0,\"y\":0}}, {{\"kind\":\"image_grid\",\"x0\":0,\"y0\":0,\"width\":300,\"height\":300,\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}}, {{\"kind\":\"visual_grid\",\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}}, {{\"kind\":\"screen_xy\",\"x\":0,\"y\":0}}, or {{\"kind\":\"ocr_text\",\"needle\":\"...\"}})",
                        e))
                });
            }
            if let Some(idx) = v.get("node_idx").and_then(|x| x.as_u64()) {
                return Ok(ClickTarget::NodeIdx { idx: idx as u32 });
            }
            if let Some(obj) = v.get("screen_xy") {
                let x = obj.get("x").and_then(|x| x.as_f64()).ok_or_else(|| {
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        "screen_xy target requires numeric x",
                    )
                })?;
                let y = obj.get("y").and_then(|y| y.as_f64()).ok_or_else(|| {
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        "screen_xy target requires numeric y",
                    )
                })?;
                return Ok(ClickTarget::ScreenXy { x, y });
            }
            if let Some(obj) = v.get("image_xy") {
                let x = obj.get("x").and_then(|x| x.as_i64()).ok_or_else(|| {
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        "image_xy target requires integer x",
                    )
                })?;
                let y = obj.get("y").and_then(|y| y.as_i64()).ok_or_else(|| {
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        "image_xy target requires integer y",
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
                    coded_tool_error(ErrorCode::InvalidParams, format!("bad image_grid target: {} (need x0,y0,width,height,rows,cols,row,col; optional intersections)", e))
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
                    coded_tool_error(ErrorCode::InvalidParams, format!("bad visual_grid target: {} (need rows,cols,row,col; optional intersections)", e))
                });
            }
            if v.get("x").is_some() || v.get("y").is_some() {
                let x = v.get("x").and_then(|x| x.as_f64()).ok_or_else(|| {
                    coded_tool_error(ErrorCode::InvalidParams, "screen target requires numeric x")
                })?;
                let y = v.get("y").and_then(|y| y.as_f64()).ok_or_else(|| {
                    coded_tool_error(ErrorCode::InvalidParams, "screen target requires numeric y")
                })?;
                return Ok(ClickTarget::ScreenXy { x, y });
            }
            if let Some(ocr) = v.get("ocr_text") {
                let needle = ocr
                    .get("needle")
                    .or_else(|| ocr.get("text"))
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| {
                        coded_tool_error(
                            ErrorCode::InvalidParams,
                            "ocr_text target requires needle",
                        )
                    })?;
                return Ok(ClickTarget::OcrText {
                    needle: needle.to_string(),
                });
            }
            Err(coded_tool_error(ErrorCode::InvalidParams, "unsupported ClickTarget. Use {\"kind\":\"node_idx\",\"idx\":N}, {\"node_idx\":N}, {\"kind\":\"image_xy\",\"x\":0,\"y\":0}, {\"image_xy\":{\"x\":0,\"y\":0}}, {\"kind\":\"image_grid\",\"x0\":0,\"y0\":0,\"width\":300,\"height\":300,\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}, {\"kind\":\"visual_grid\",\"rows\":15,\"cols\":15,\"row\":7,\"col\":7,\"intersections\":true}, {\"kind\":\"screen_xy\",\"x\":0,\"y\":0}, or {\"ocr_text\":{\"needle\":\"...\"}}."))
        }

        fn parse_wait_predicate(v: &Value) -> OpenBitFunResult<AppWaitPredicate> {
            if v.get("kind").is_some() {
                return serde_json::from_value(v.clone()).map_err(|e| {
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        format!("bad app_wait_for predicate: {}", e),
                    )
                });
            }
            if let Some(obj) = v.get("digest_changed") {
                let prev_digest = obj
                    .get("prev_digest")
                    .or_else(|| obj.get("from"))
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| {
                        coded_tool_error(
                            ErrorCode::InvalidParams,
                            "digest_changed requires prev_digest",
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
                        coded_tool_error(ErrorCode::InvalidParams, "title_contains requires needle")
                    })?;
                return Ok(AppWaitPredicate::TitleContains {
                    needle: needle.to_string(),
                });
            }
            if let Some(obj) = v.get("role_enabled") {
                let role = obj.get("role").and_then(|x| x.as_str()).ok_or_else(|| {
                    coded_tool_error(ErrorCode::InvalidParams, "role_enabled requires role")
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
                        coded_tool_error(ErrorCode::InvalidParams, "node_enabled requires idx")
                    })?;
                return Ok(AppWaitPredicate::NodeEnabled { idx: idx as u32 });
            }
            Err(coded_tool_error(ErrorCode::InvalidParams, "unsupported app_wait_for predicate. Use {\"kind\":\"digest_changed\",\"prev_digest\":\"...\"} or shorthand {\"digest_changed\":{\"prev_digest\":\"...\"}}."))
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
        //
        // `tree_text` is the **only** rendering of the AX tree we send. Results
        // used to carry a sibling `app_state_nodes` array holding the same
        // nodes as verbose JSON — one object per node, ~15 lines each. It was
        // strictly redundant (`render_tree_text` already emits idx, role,
        // title, value, identifier, description, help, url, frame and the
        // enabled/focused/selected/expanded flags, with parentage implied by
        // indentation) and nothing consumed it, yet it accounted for ~78% of a
        // `get_app_state` result: a single observation of a windowless app
        // measured 107 KB, of which 82 KB was that duplicate.
        fn snap_state_json(
            snap: &crate::agentic::tools::computer_use_host::AppStateSnapshot,
        ) -> serde_json::Value {
            // Bounded here rather than at the `get_app_state` call site: every
            // `app_click` / `app_type_text` / `app_scroll` / `app_key_chord` /
            // `app_wait_for` result carries this same post-action tree, so they
            // all shared the same unbounded-payload risk.
            let tree_text = super::computer_use_tool::clip_tree_text(
                snap.tree_text.clone(),
                super::computer_use_tool::APP_STATE_TREE_TEXT_MAX_BYTES,
            );
            let mut v = json!({
                "app": snap.app,
                "window_title": snap.window_title,
                "digest": snap.digest,
                "captured_at_ms": snap.captured_at_ms,
                "tree_text": tree_text,
                "node_count": snap.nodes.len(),
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

        // Every ComputerUse action result that may carry a screenshot follows the same
        // shape: attach it as a multimodal image when present, otherwise fall back to a
        // text-only `ToolResult::ok`. `snap_result` / `interactive_view_result` /
        // `visual_mark_view_result` / `interactive_action_result` / `visual_action_result`
        // below all delegate to this single helper instead of re-implementing the
        // base64-encode-and-attach dance for each result type.
        fn result_with_optional_screenshot(
            data: serde_json::Value,
            summary: Option<String>,
            screenshot: Option<&crate::agentic::tools::computer_use_host::ComputerScreenshot>,
        ) -> ToolResult {
            use base64::Engine as _;
            match screenshot {
                Some(shot) => {
                    let attach = crate::util::types::ToolImageAttachment {
                        mime_type: shot.mime_type.clone(),
                        data_base64: base64::engine::general_purpose::STANDARD.encode(&shot.bytes),
                    };
                    ToolResult::ok_with_images(data, summary, vec![attach])
                }
                None => ToolResult::ok(data, summary),
            }
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
            result_with_optional_screenshot(data, summary, snap.screenshot.as_ref())
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
                "omitted_element_count": view.omitted_element_count,
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
            result_with_optional_screenshot(data, summary, view.screenshot.as_ref())
        }

        fn visual_mark_view_result(
            data: serde_json::Value,
            summary: Option<String>,
            view: &crate::agentic::tools::computer_use_host::VisualMarkView,
        ) -> ToolResult {
            result_with_optional_screenshot(data, summary, view.screenshot.as_ref())
        }

        // Prefer attaching the rebuilt interactive view's screenshot when
        // available; otherwise fall back to the post-action snapshot's.
        fn interactive_action_result(
            data: serde_json::Value,
            summary: Option<String>,
            res: &crate::agentic::tools::computer_use_host::InteractiveActionResult,
        ) -> ToolResult {
            let shot_opt = res
                .view
                .as_ref()
                .and_then(|v| v.screenshot.as_ref())
                .or(res.snapshot.screenshot.as_ref());
            result_with_optional_screenshot(data, summary, shot_opt)
        }

        fn visual_action_result(
            data: serde_json::Value,
            summary: Option<String>,
            res: &crate::agentic::tools::computer_use_host::VisualActionResult,
        ) -> ToolResult {
            let shot_opt = res
                .view
                .as_ref()
                .and_then(|v| v.screenshot.as_ref())
                .or(res.snapshot.screenshot.as_ref());
            result_with_optional_screenshot(data, summary, shot_opt)
        }

        // These actions only make sense with a marked-up screenshot the model
        // can look at; text-only models are steered to the AX/OCR text paths.
        if text_only
            && matches!(
                action,
                "build_interactive_view"
                    | "interactive_click"
                    | "build_visual_mark_view"
                    | "visual_click"
            )
        {
            return Err(coded_tool_error(
                ErrorCode::NotAvailable,
                format!(
                    "`{}` requires a vision-capable primary model (its result is a marked-up screenshot). Use `describe_screen` or `get_app_state` to observe as text, then act with `app_click`, `click_target`, `move_to_text`, or `key_chord`.",
                    action
                ),
            ));
        }
        // Same gate, different reason: these two act on the `i` index of an
        // interactive view, and building that view is itself vision-only.
        if text_only && matches!(action, "interactive_type_text" | "interactive_scroll") {
            let replacement = if action == "interactive_scroll" {
                "app_scroll"
            } else {
                "app_type_text"
            };
            return Err(coded_tool_error(
                ErrorCode::NotAvailable,
                format!(
                    "`{action}` addresses elements by the `i` index of an interactive view, which requires a vision-capable primary model. Use `{replacement}` with a `focus` target resolved from `get_app_state` / `describe_screen` instead."
                ),
            ));
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
                    "before_digest": snap.digest,
                    "loop_warning": snap.loop_warning,
                });
                Ok(vec![snap_result(data, Some(summary), &snap)])
            }
            "get_app_shortcuts" => {
                let app = parse_selector(params)?;
                let snap = host.get_app_shortcuts(app.clone()).await?;
                let summary = format!(
                    "{} keyboard shortcut(s) found for {}",
                    snap.shortcuts.len(),
                    snap.app.name
                );
                Ok(vec![ToolResult::ok(
                    json!({
                        "target_app": app,
                        "app": snap.app,
                        "shortcuts": snap.shortcuts,
                        "shortcuts_without_key_count": snap.menu_items_without_shortcut,
                        "captured_at_ms": snap.captured_at_ms,
                        "background_input": bg,
                        "ax_tree": ax,
                    }),
                    Some(summary),
                )])
            }
            "app_click" => {
                let app = parse_selector(params)?;
                let target_v = params.get("target").cloned().ok_or_else(|| {
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        "app_click requires 'target' ({node_idx|image_xy|screen_xy|ocr_text})",
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
                        text_only,
                    );
                }

                let data = json!({
                    "target_app": app,
                    "click_target": target,
                    "background_input": bg,
                    "before_digest": before,
                    "app_state": snap_state_json(&after),
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
                        coded_tool_error(ErrorCode::InvalidParams, "app_type_text requires 'text'")
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
                        text_only,
                    );
                }
                let data = json!({
                    "target_app": app,
                    "background_input": bg,
                    "char_count": text.chars().count(),
                    "focus": focus,
                    "before_digest": before,
                    "app_state": snap_state_json(&after),
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
                    return Err(coded_tool_error(
                        ErrorCode::InvalidParams,
                        "app_key_chord requires non-empty 'keys'",
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
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        "app_wait_for requires 'predicate'",
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
                        coded_tool_error(
                            ErrorCode::InvalidParams,
                            format!("build_interactive_view 'opts' invalid: {}", e),
                        )
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
                        coded_tool_error(
                            ErrorCode::InvalidParams,
                            format!("interactive_click params invalid: {}", e),
                        )
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
                        coded_tool_error(
                            ErrorCode::InvalidParams,
                            format!("build_visual_mark_view 'opts' invalid: {}", e),
                        )
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
                    coded_tool_error(
                        ErrorCode::InvalidParams,
                        format!("visual_click params invalid: {}", e),
                    )
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
                        coded_tool_error(
                            ErrorCode::InvalidParams,
                            format!("interactive_type_text params invalid: {}", e),
                        )
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
                        coded_tool_error(
                            ErrorCode::InvalidParams,
                            format!("interactive_scroll params invalid: {}", e),
                        )
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
            other => Err(coded_tool_error(
                ErrorCode::Internal,
                format!("handle_desktop_ax called with unknown action: {}", other),
            )),
        }
    }

    // ── Browser domain ─────────────────────────────────────────────────

    // ── System domain ──────────────────────────────────────────────────

    pub(crate) async fn handle_system(
        &self,
        action: &str,
        params: &Value,
        context: &ToolUseContext,
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        match action {
            "open_app" => {
                let app_name = params
                    .get("app_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| OpenBitFunError::tool("open_app requires 'app_name'".to_string()))?;

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

                let provider = LocalSystemProvider::new();
                let outcome = provider.open_app_shell(app_name).map_err(|e| {
                    OpenBitFunError::tool(format!("{} (host_error: {:?})", e.message(), host_error))
                })?;
                let warning = host_error.map(|e| {
                    format!("computer_use_host open_app failed; shell fallback succeeded: {}", e)
                });
                Ok(vec![ToolResult::ok(
                    json!({
                        "launched": true,
                        "app": app_name,
                        "method": method,
                        "via_command": outcome.via_command,
                        "host_attempted": host_attempted,
                        "warning": warning,
                    }),
                    Some(format!("Opened {} via {}", app_name, outcome.via_command)),
                )])
            }
            "run_script" => {
                let script = params
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| OpenBitFunError::tool("run_script requires 'script'".to_string()))?;
                let script_type = params
                    .get("script_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("applescript");
                // Optional caller-provided runtime bound. Omit or set to 0 to wait
                // for script completion without an internal cap.
                let timeout_ms = params
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .filter(|value| *value > 0);
                let max_output_bytes = params.get("max_output_bytes").and_then(|v| v.as_u64());
                let provider = LocalSystemProvider::new();
                let output = match provider
                    .run_script(RunScriptRequest {
                        script,
                        script_type,
                        timeout_ms,
                        max_output_bytes,
                    })
                    .await
                {
                    Ok(output) => output,
                    Err(error) => return map_run_script_error(error),
                };

                if output.success {
                    Ok(vec![ToolResult::ok(
                        json!({
                            "success": true,
                            "output": output.stdout,
                            "stderr": output.stderr,
                            "stdout_truncated": output.stdout_truncated,
                            "stderr_truncated": output.stderr_truncated,
                            "exit_code": output.exit_code,
                            "elapsed_ms": output.elapsed_ms,
                            "script_type": script_type,
                        }),
                        Some(if output.stdout.is_empty() {
                            format!("Script executed in {} ms", output.elapsed_ms)
                        } else {
                            output.stdout.lines().take(1).collect::<String>()
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
                                output.exit_code,
                                output.stderr.lines().next().unwrap_or("(no stderr)")
                            ),
                        )
                        .with_hints([
                            format!("stderr={}", output.stderr),
                            format!("elapsed_ms={}", output.elapsed_ms),
                        ]),
                    ))
                }
            }
            "get_os_info" => {
                let local = LocalSystemProvider::new().system_info();
                let mut info = json!({
                    "os": local.os,
                    "arch": local.arch,
                    "rust_target_family": local.rust_target_family,
                });
                if let Some(v) = local.os_version {
                    info["os_version"] = json!(v);
                }
                if let Some(host) = local.hostname {
                    info["hostname"] = json!(host);
                }
                if let Some(s) = local.display_server {
                    info["display_server"] = json!(s);
                }
                if let Some(d) = local.desktop_environment {
                    info["desktop_environment"] = json!(d);
                }
                info["script_types"] = json!(local.script_types);
                Ok(vec![ToolResult::ok(
                    info.clone(),
                    Some(format!(
                        "{} {} ({})",
                        local.os,
                        info.get("os_version").and_then(|v| v.as_str()).unwrap_or(""),
                        local.arch
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

                match LocalSystemProvider::new().clipboard_read_text().await {
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
                    Err(e) => Ok(local_system_error_response("system", "clipboard_get", e)),
                }
            }

            // Cross-context primitive: place text on the system clipboard.
            // The user can then paste it into ANY app with cmd+v / ctrl+v —
            // dramatically simpler than driving each target GUI by hand.
            "clipboard_set" => {
                let text = params.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                    OpenBitFunError::tool("clipboard_set requires 'text'".to_string())
                })?;
                match LocalSystemProvider::new().clipboard_write_text(text).await {
                    Ok(()) => Ok(vec![ToolResult::ok(
                        json!({
                            "success": true,
                            "byte_length": text.len(),
                        }),
                        Some(format!("Wrote {} bytes to clipboard", text.len())),
                    )]),
                    Err(e) => Ok(local_system_error_response("system", "clipboard_set", e)),
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
                    .ok_or_else(|| OpenBitFunError::tool("open_url requires 'url'".to_string()))?;
                match LocalSystemProvider::new().open_url(url) {
                    Ok(outcome) => Ok(vec![ToolResult::ok(
                        json!({
                            "opened": true,
                            "url": url,
                            "method": outcome.method,
                            "note": OPEN_URL_ROUTING_NOTE,
                        }),
                        Some(format!(
                            "Opened {} in default handler. {}",
                            url, OPEN_URL_ROUTING_NOTE
                        )),
                    )]),
                    Err(e) => Ok(local_system_error_response("system", "open_url", e)),
                }
            }

            // Cross-context primitive: open a local file with its default
            // handler (or an explicitly named app on macOS). High-frequency
            // for "open this PDF / picture / spreadsheet for me".
            "open_file" => {
                let path_str = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                    OpenBitFunError::tool("open_file requires 'path'".to_string())
                })?;
                let app_name = params.get("app").and_then(|v| v.as_str());

                match LocalSystemProvider::new().open_file(path_str, app_name) {
                    Ok(outcome) => Ok(vec![ToolResult::ok(
                        json!({
                            "opened": true,
                            "path": path_str,
                            "with_app": app_name,
                            "method": outcome.method,
                            "note": OPEN_FILE_ROUTING_NOTE,
                        }),
                        Some(format!(
                            "{}. {}",
                            match app_name {
                                Some(a) => format!("Opened {} with {}", path_str, a),
                                None => format!("Opened {} with default handler", path_str),
                            },
                            OPEN_FILE_ROUTING_NOTE
                        )),
                    )]),
                    Err(e) => Ok(local_system_error_response("system", "open_file", e)),
                }
            }

            other => Err(OpenBitFunError::tool(format!(
                "Unknown system action: '{}'. Valid: open_app, run_script, get_os_info, open_url, open_file, clipboard_get, clipboard_set",
                other
            ))),
        }
    }
}
fn local_system_error_response(
    domain: &'static str,
    action: &'static str,
    error: LocalSystemActionError,
) -> Vec<ToolResult> {
    let mut control_error =
        ControlHubError::new(error_code_from_local(error.stable_code()), error.message());
    if !error.hints().is_empty() {
        control_error = control_error.with_hints(error.hints().to_vec());
    }
    err_response(domain, action, control_error)
}

fn map_run_script_error(error: LocalSystemActionError) -> OpenBitFunResult<Vec<ToolResult>> {
    match error.stable_code() {
        "NOT_AVAILABLE" | "TIMEOUT" => {
            Ok(local_system_error_response("system", "run_script", error))
        }
        _ => Err(OpenBitFunError::tool(error.message().to_string())),
    }
}

fn error_code_from_local(code: &str) -> ErrorCode {
    match code {
        "INVALID_PARAMS" => ErrorCode::InvalidParams,
        "NOT_AVAILABLE" => ErrorCode::NotAvailable,
        "NOT_FOUND" => ErrorCode::NotFound,
        "TIMEOUT" => ErrorCode::Timeout,
        _ => ErrorCode::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::loop_tracker_observe;
    use super::ComputerUseActions;
    use super::{OPEN_FILE_ROUTING_NOTE, OPEN_URL_ROUTING_NOTE};
    use crate::agentic::tools::computer_use_host::ComputerUseForegroundApplication;
    use serde_json::json;

    // A unique PID avoids interference with the shared APP_LOOP_TRACKER state
    // across tests in the same process.
    const TEXT_ONLY_PID: i32 = 9_999_001;
    const VISUAL_PID: i32 = 9_999_002;

    fn first_warning(text_only: bool, pid: i32) -> String {
        // First call seeds (count=1, no warning). Second consecutive identical
        // (unchanged digest) call trips the guard (count>=2) and returns the hint.
        let _ = loop_tracker_observe(Some(pid), "app_click", "[1]", "d0", "d0", text_only);
        loop_tracker_observe(Some(pid), "app_click", "[1]", "d0", "d0", text_only)
            .expect("second consecutive no-progress call should warn")
    }

    /// Text-only recovery hint must NOT send the model to `screenshot` (that
    /// action is hard-rejected for text-only models and would loop forever).
    #[test]
    fn text_only_loop_warning_never_points_at_screenshot() {
        let warning = first_warning(true, TEXT_ONLY_PID);
        assert!(
            !warning.contains("desktop.screenshot") && !warning.contains("run `screenshot`"),
            "text-only loop warning must not tell the model to screenshot: {}",
            warning
        );
        assert!(
            warning.contains("describe_screen")
                || warning.contains("get_app_state")
                || warning.contains("move_to_text")
                || warning.contains("key_chord"),
            "text-only loop warning should offer a text-only recovery path: {}",
            warning
        );
    }

    /// Visual-capable models keep the classic screenshot recovery hint.
    #[test]
    fn visual_loop_warning_keeps_screenshot_recovery() {
        let warning = first_warning(false, VISUAL_PID);
        assert!(
            warning.contains("screenshot"),
            "visual loop warning should still offer screenshot recovery: {}",
            warning
        );
    }

    fn foreground(name: &str, bundle_id: &str) -> ComputerUseForegroundApplication {
        ComputerUseForegroundApplication {
            name: Some(name.to_string()),
            bundle_id: Some(bundle_id.to_string()),
            process_name: None,
            process_id: Some(1),
        }
    }

    /// A host that reports no app identity — on Windows `name` is the
    /// foreground *window title*, not an application name.
    fn titled(window_title: &str) -> ComputerUseForegroundApplication {
        ComputerUseForegroundApplication {
            name: Some(window_title.to_string()),
            bundle_id: None,
            process_name: None,
            process_id: Some(1),
        }
    }

    /// Windows shape: window title in `name`, executable basename in
    /// `process_name`.
    fn windows_app(window_title: &str, exe: &str) -> ComputerUseForegroundApplication {
        ComputerUseForegroundApplication {
            name: Some(window_title.to_string()),
            bundle_id: None,
            process_name: Some(exe.to_string()),
            process_id: Some(1),
        }
    }

    /// Only Chromium-family browsers are CDP-drivable via the ControlHub
    /// browser domain. Firefox/Safari must NOT trip the desktop browser guard
    /// or the user would have no control path at all.
    #[test]
    fn browser_guard_matches_only_chromium_family() {
        assert!(ComputerUseActions::is_probably_browser_app(&foreground(
            "Google Chrome",
            "com.google.Chrome"
        )));
        assert!(ComputerUseActions::is_probably_browser_app(&foreground(
            "Microsoft Edge",
            "com.microsoft.edgemac"
        )));
        assert!(ComputerUseActions::is_probably_browser_app(&foreground(
            "Google Chrome Canary",
            "com.google.Chrome.canary"
        )));
        assert!(!ComputerUseActions::is_probably_browser_app(&foreground(
            "Firefox",
            "org.mozilla.firefox"
        )));
        assert!(!ComputerUseActions::is_probably_browser_app(&foreground(
            "Safari",
            "com.apple.Safari"
        )));
    }

    /// The identity wins over the display name: an editor window whose title
    /// happens to contain a browser word is still an editor.
    #[test]
    fn browser_guard_prefers_identity_over_display_name() {
        assert!(!ComputerUseActions::is_probably_browser_app(&foreground(
            "chrome-devtools.ts — Code",
            "com.microsoft.VSCode"
        )));
    }

    /// Window titles are user content, not app identities. Substring hints on
    /// them locked desktop input out of ordinary Windows apps ("Knowledge Base"
    /// contains "edge", "Search Results" contains "arc").
    #[test]
    fn browser_guard_ignores_window_titles_that_merely_contain_browser_words() {
        for title in [
            "Knowledge Base - Obsidian",
            "edge_cases.ts - proj - Visual Studio Code",
            "Search Results in Documents",
            "Monarch",
            "Archive Utility",
            "Ledger Live",
        ] {
            assert!(
                !ComputerUseActions::is_probably_browser_app(&titled(title)),
                "`{title}` is not a browser"
            );
        }
    }

    /// Without an identity the window title is the only signal left, so real
    /// Chromium windows must still be recognised from it.
    #[test]
    fn browser_guard_still_matches_chromium_window_titles() {
        for title in [
            "Google - Google Chrome",
            "Inbox - Microsoft Edge",
            "OpenBitFun docs — Arc",
            "New Tab - Brave",
        ] {
            assert!(
                ComputerUseActions::is_probably_browser_app(&titled(title)),
                "`{title}` is a Chromium browser window"
            );
        }
    }

    /// Windows/Linux report an executable basename rather than a bundle id.
    #[test]
    fn browser_guard_matches_executable_basenames() {
        assert!(ComputerUseActions::is_probably_browser_app(&foreground(
            "Google - Google Chrome",
            "chrome.exe"
        )));
        assert!(ComputerUseActions::is_probably_browser_app(&foreground(
            "Inbox - Microsoft Edge",
            "msedge.exe"
        )));
        assert!(!ComputerUseActions::is_probably_browser_app(&foreground(
            "edge_cases.ts - Visual Studio Code",
            "Code.exe"
        )));
        assert!(!ComputerUseActions::is_probably_browser_app(&foreground(
            "Knowledge Base - Obsidian",
            "obsidian.exe"
        )));
    }

    /// An explicit `app` selector is classified without asking the host, so
    /// `app_click { app: { name: "Google Chrome" } }` cannot be used to reach
    /// the browser from the desktop side.
    #[test]
    fn app_selector_naming_a_chromium_browser_is_recognised() {
        assert!(ComputerUseActions::selector_is_chromium(
            &json!({ "name": "Google Chrome" })
        ));
        assert!(ComputerUseActions::selector_is_chromium(
            &json!({ "bundle_id": "com.microsoft.edgemac" })
        ));
        assert!(ComputerUseActions::selector_is_chromium(&json!(
            "Brave Browser"
        )));
        assert!(!ComputerUseActions::selector_is_chromium(
            &json!({ "name": "WeChat" })
        ));
        // pid-only carries no identity — the frontmost check decides instead.
        assert!(!ComputerUseActions::selector_is_chromium(
            &json!({ "pid": 123 })
        ));
    }

    /// The app switcher must stay callable while a browser is frontmost: it is
    /// the escape hatch for tasks whose target is not the browser at all.
    #[test]
    fn app_switcher_chords_are_never_guarded() {
        assert!(ComputerUseActions::is_focus_switch_chord(
            "key_chord",
            &json!({ "keys": ["alt", "tab"] })
        ));
        assert!(ComputerUseActions::is_focus_switch_chord(
            "key_chord",
            &json!({ "keys": ["command", "shift", "tab"] })
        ));
        assert!(!ComputerUseActions::is_focus_switch_chord(
            "key_chord",
            &json!({ "keys": ["command", "t"] })
        ));
        assert!(!ComputerUseActions::is_focus_switch_chord(
            "type_text",
            &json!({ "keys": ["alt", "tab"] })
        ));
    }

    /// The rejection must lead somewhere: a non-browser escape route, the
    /// ControlHub actions that own browser chrome / file pickers / dialogs, and
    /// no contradiction with `browser.connect`'s guarded approval flow.
    #[test]
    fn browser_guard_hints_offer_an_executable_way_out() {
        let error = ComputerUseActions::desktop_browser_guard_error("click", None);
        assert!(
            error
                .message
                .contains("not because your task is browser-related"),
            "{}",
            error.message
        );
        let hints = error.hints.join(" | ");
        assert!(hints.contains("browser.connect"), "{hints}");
        assert!(hints.contains("browser.set_file_input_files"), "{hints}");
        assert!(hints.contains("browser.dialog"), "{hints}");
        assert!(hints.contains("browser.navigate"), "{hints}");
        assert!(hints.contains("open_app"), "{hints}");
        assert!(hints.contains("app_click"), "{hints}");
        assert!(
            !hints.contains("test port enabled") && !hints.contains("--remote-debugging-port"),
            "must not teach the unsafe legacy default-profile debug-port flow: {hints}"
        );
    }

    /// `open_url` hands the page to the user's default browser, which the
    /// agent can neither observe nor control. The success note must say so
    /// and route follow-up page work to the ControlHub browser domain —
    /// never to desktop clicks (those trip the desktop browser guard).
    #[test]
    fn open_url_routing_note_points_at_browser_domain() {
        assert!(OPEN_URL_ROUTING_NOTE.contains("cannot observe or control"));
        assert!(OPEN_URL_ROUTING_NOTE.contains("ControlHub domain=\"browser\""));
        assert!(OPEN_URL_ROUTING_NOTE.contains("browser.connect"));
        assert!(OPEN_URL_ROUTING_NOTE.contains("snapshot"));
    }

    /// `open_file` opens an external app window; follow-up interaction goes
    /// through ComputerUse desktop actions (screenshot first), NOT the
    /// browser domain.
    #[test]
    fn open_file_routing_note_points_at_desktop_actions() {
        assert!(OPEN_FILE_ROUTING_NOTE.contains("external application window"));
        assert!(OPEN_FILE_ROUTING_NOTE.contains("ComputerUse desktop"));
        assert!(OPEN_FILE_ROUTING_NOTE.contains("screenshot"));
        assert!(!OPEN_FILE_ROUTING_NOTE.contains("browser"));
    }

    /// A genuine tree mutation (digest changes) must NOT trigger the warning,
    /// even on the same target — progress resets the streak.
    #[test]
    fn progressed_action_does_not_warn() {
        let pid = 9_999_003;
        let _ = loop_tracker_observe(Some(pid), "app_click", "[2]", "d0", "d1", true);
        let second = loop_tracker_observe(Some(pid), "app_click", "[2]", "d1", "d2", true);
        assert!(second.is_none(), "digest change = progress, no warning");
    }
}
