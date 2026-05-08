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
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::{json, Value};

use super::control_hub::{err_response, ControlHubError, ErrorCode};

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
            "Detected {} consecutive `{}` calls on the same target ({}) without any AX tree mutation (digest unchanged). The target is almost certainly invisible / disabled / in a Canvas-WebGL surface that AX cannot describe. NEXT TURN you MUST: (1) run `desktop.screenshot {{ screenshot_window: false }}` to see the full display, (2) switch tactic — different `node_idx`, different `ocr_text` needle, or a keyboard shortcut.",
            entry.2, action, target_sig
        ))
    } else {
        None
    }
}

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
        let name = foreground
            .name
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
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
            "chrome", "chromium", "edge", "brave", "arc", "firefox", "safari", "browser",
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
    // ── Desktop domain ─────────────────────────────────────────────────

    pub(crate) async fn handle_desktop(
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
            // These operate on the typed AppSelector / AxNode envelope.
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
        if let Some(v) = params.get("display_id") {
            let target = match v {
                Value::Null => None,
                v => Some(v.as_u64().ok_or_else(|| {
                    BitFunError::tool(
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

    pub(crate) async fn handle_system(
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
                    match crate::util::process_manager::create_command(cmd).args(args).output() {
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
                // Optional caller-provided runtime bound. Omit or set to 0 to wait
                // for script completion without an internal cap.
                let timeout_ms = params
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .filter(|value| *value > 0);
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
                let output = if let Some(timeout_ms) = timeout_ms {
                    match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), wait)
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
                                    "Increase 'timeout_ms', set it to 0, or omit it to wait without a timeout",
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
                    }
                } else {
                    match wait.await {
                        Ok(o) => o,
                        Err(e) => {
                            return Err(BitFunError::tool(format!(
                                "Failed to wait for run_script ({}): {}",
                                script_type, e
                            )));
                        }
                    }
                };

                let elapsed_ms = elapsed_ms_u64(started);
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
pub(crate) fn truncate_with_marker(s: &str, max_bytes: usize) -> (String, bool) {
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
        let out = crate::util::process_manager::create_command("cmd")
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
pub(crate) fn which_exists(name: &str) -> bool {
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
pub(crate) fn linux_clipboard_install_hints() -> Vec<String> {
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
pub(crate) fn linux_session_info() -> (Option<String>, Option<String>) {
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
