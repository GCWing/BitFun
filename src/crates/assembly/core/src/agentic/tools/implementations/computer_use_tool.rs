//! Desktop automation (Computer use).

use super::computer_use_locate::execute_computer_use_locate;
use super::control_hub::{coded_tool_error, err_response, ErrorCode};
use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::computer_use_host::{
    AppSelector, ComputerScreenshot, ComputerUseHost, ComputerUseNavigateQuadrant, OcrRegionNative,
    ScreenshotCropCenter, UiElementLocateQuery,
};
use crate::agentic::tools::computer_use_optimizer::hash_screenshot_bytes;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext,
};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use crate::util::types::ToolImageAttachment;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use log::{debug, warn};
use openbitfun_agent_tools::computer_use::{
    build_screenshot_tool_body_and_hint, coordinate_mode,
    ensure_pointer_move_uses_screen_coordinates_only, parse_screenshot_params,
    use_screen_coordinates,
};
use openbitfun_core_types::product_identity::hidden_data_directory;
use serde_json::{json, Value};

fn computer_use_permission_resource(input: &Value) -> String {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let target = [
        "app_name",
        "url",
        "path",
        "title_contains",
        "identifier_contains",
    ]
    .into_iter()
    .find_map(|field| {
        input
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("{field}={value}"))
    });

    target.map_or_else(|| action.to_string(), |target| format!("{action}:{target}"))
}

/// Merges [`ComputerUseHost::computer_use_session_snapshot`] + optional `input_coordinates` into tool JSON.
/// Also records the action for loop detection and adds loop warnings if detected.
pub(crate) async fn computer_use_augment_result_json(
    host: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
    mut body: Value,
    input_coordinates: Option<Value>,
) -> Value {
    let snap = host.computer_use_session_snapshot().await;
    let interaction = host.computer_use_interaction_state();

    // Record action for loop detection
    let action_type = body
        .get("action")
        .or_else(|| body.get("tool"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let action_params = input_coordinates
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_default();
    let success = body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    host.record_action(&action_type, &action_params, success);

    // Check for action loops
    let loop_result = host.detect_action_loop();

    if let Value::Object(map) = &mut body {
        map.insert(
            "computer_use_context".to_string(),
            json!({
                "foreground_application": snap.foreground_application,
                "pointer_global": snap.pointer_global,
                "input_coordinates": input_coordinates,
            }),
        );
        map.insert("interaction_state".to_string(), json!(interaction));

        // Loop hint surfaced to the model as a warning only — it never forces the
        // agent loop to stop. The model decides on its own whether to switch tactic.
        if loop_result.is_loop {
            map.insert(
                "loop_warning".to_string(),
                json!({
                    "detected": true,
                    "pattern_length": loop_result.pattern_length,
                    "repetitions": loop_result.repetitions,
                    "suggestion": loop_result.suggestion,
                }),
            );
        }
    }
    body
}

/// On-disk copy of each Computer use screenshot (pointer overlay included) for debugging.
/// Opt-in: only written when [`COMPUTER_USE_DEBUG_SCREENSHOTS_ENV`] is set to `1`;
/// the directory is pruned to the newest [`COMPUTER_USE_DEBUG_MAX_FILES`] files after each write.
/// Filenames: `cu_<ms>_full.jpg` (whole display) or `cu_<ms>_crop_<x>_<y>.jpg` when a point crop was requested.
const COMPUTER_USE_DEBUG_DIRECTORY_NAME: &str = "computer_use_debug";
/// Set to `1` to enable on-disk debug copies of Computer use screenshots.
const COMPUTER_USE_DEBUG_SCREENSHOTS_ENV: &str = "OPENBITFUN_COMPUTER_USE_DEBUG_SCREENSHOTS";
/// Newest debug screenshots retained in the product debug directory; older files are deleted.
const COMPUTER_USE_DEBUG_MAX_FILES: usize = 20;

fn computer_use_debug_subdir() -> String {
    format!(
        "{}/{}",
        hidden_data_directory(),
        COMPUTER_USE_DEBUG_DIRECTORY_NAME
    )
}

/// AX depth `describe_screen` walks into the focused window.
///
/// This was 8, which is fine for a native Cocoa app but far too shallow for
/// Electron / WebView clients — the ones agents are most often asked to drive.
/// Measured against a real Electron window (focused window only):
///
/// | depth | nodes | actionable | tree_text |
/// |------:|------:|-----------:|----------:|
/// |     8 |    17 |          7 |      1 KB |
/// |    12 |    25 |         15 |      2 KB |
/// |    16 |    50 |         40 |      5 KB |
/// |    20 |   207 |        197 |     27 KB |
/// |    24 |   233 |        223 |     31 KB |
/// |    32 |  1289 |       1279 |    206 KB |
///
/// At 8 the agent could see seven actionable elements in an entire app — not
/// enough to find a search field or a send button, which reads as "this app has
/// no AX tree" and pushes it onto OCR or screenshot guessing. The actionable
/// layer appears around 20; past that the payload grows far faster than the
/// number of things worth clicking.
const DESCRIBE_SCREEN_AX_DEPTH: u32 = 20;

/// Byte ceiling on the AX tree `describe_screen` returns.
///
/// The depth above is tuned against a typical rich window (~27 KB), but depth
/// is a poor proxy for size: a document, a long list or a deeply nested canvas
/// can multiply that. `describe_screen` is the action an agent calls most, so
/// it needs a bound that does not depend on the app behaving reasonably.
const DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES: usize = 60_000;

/// Byte ceiling on the AX tree carried by `get_app_state` and every `app_*`
/// action result.
///
/// Higher than the `describe_screen` cap because these are explicit requests
/// for an app's tree rather than a routine observation — but still a ceiling.
/// Measured unbounded output on a real Electron app was 390 KB from a single
/// `get_app_state`, roughly 100k tokens, which is most of a context window
/// spent on one look at one app.
pub(crate) const APP_STATE_TREE_TEXT_MAX_BYTES: usize = 120_000;

/// A routine observation must never be allowed a bigger tree than an explicit
/// query for one. Checked at compile time so reordering the two constants is a
/// build error rather than something a test has to notice.
const _: () = assert!(DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES < APP_STATE_TREE_TEXT_MAX_BYTES);

/// Trim an AX tree to `max_bytes` on a line boundary, appending a note that
/// says what was dropped and how to get it.
///
/// Silent truncation would be worse than the problem it solves: the agent would
/// read a partial tree as the whole UI and conclude a control does not exist.
pub(crate) fn clip_tree_text(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    // Walk back to a char boundary before slicing. The cap is a byte count, and
    // slicing a `str` at a byte index inside a multi-byte character panics —
    // which CJK app trees (the ones most likely to be large) would hit
    // constantly.
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    // A newline is single-byte, so its index is always a valid boundary too.
    let cut = text[..end].rfind('\n').unwrap_or(end);
    let kept_lines = text[..cut].lines().count();
    let total_lines = text.lines().count();
    format!(
        "{}\n[truncated] showing the first {} of {} AX nodes ({} of {} bytes). \
This is a size limit, not the end of the UI — a control you cannot find here may still exist. \
Narrow the view with `get_app_state` (`focus_window_only`, a smaller `max_depth`) or target it \
directly with `locate` / `move_to_text`.\n",
        &text[..cut],
        kept_lines,
        total_lines,
        cut,
        text.len(),
    )
}

pub struct ComputerUseTool;

impl Default for ComputerUseTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerUseTool {
    pub fn new() -> Self {
        Self
    }

    /// Tool description when the primary model is **text-only** (no `screenshot` / JPEG workflow).
    fn description_text_only() -> String {
        let os = Self::host_os_label();
        let keys = Self::key_chord_os_hint();
        format!(
            "Desktop automation (host OS: {}). {} \
The **primary model cannot consume images** in tool results — **do not** use **`screenshot`**.\n\
**OBSERVE & VERIFY (text-only):** Use **`describe_screen`** as your eyes — it returns a text snapshot (frontmost app + AX tree `ax_tree_text` with `node_idx`s + `ui_tree_text` + pointer) with NO image. Call it before acting when UI state is unknown, and after an action to verify the `ax_state_digest` changed. This replaces the `screenshot` observe→act→verify loop for text-only models.\n\
**ACTION PRIORITY (CRITICAL):** Always think in this order:\n\
1. **Terminal/CLI/System commands first** — Use the **`ExecCommand`** tool for terminal commands, system scripts (e.g., macOS `osascript`), shell automation. Most efficient.\n\
2. **Keyboard shortcuts second** — Use **`key_chord`** / **`type_text`** for system/app shortcuts, navigation keys. Unsure what shortcut a target app registers for a function (e.g. \"Save\")? Call **`get_app_shortcuts`** first instead of guessing or clicking through menus.\n\
3. **Precise UI control last** — Only when above fail: **`click_target`** / **`move_to_target`** (AX → OCR → screen coords in one call) → lower-level **`click_element`** / **`move_to_text`** → **`mouse_move`** + **`click`**.\n\
**Rhythm:** one action at a time; use **`wait`** when UI animates. Observe **`interaction_state`** and **`computer_use_context`** in tool JSON.\n\
**`click_target` / `move_to_target`:** Unified resolver: AX filters or `target_text` first, OCR second, explicit global x/y last. **`click_element` / `locate`:** Accessibility (AX/UIA/AT-SPI). **`move_to_text`:** OCR match + move pointer only. **`click`:** at current pointer only — use **`mouse_move`** or **`move_to_text`** / **`click_element`** first.\n\
**`mouse_move` / `drag`:** **`use_screen_coordinates`: true** with globals from tools. **`pointer_move_rel`:** relative nudge; host may block right after certain flows — follow tool errors.\n\
**`key_chord` / `type_text` / `scroll` / `wait`:** standard desktop automation without any screenshot step.\n",
            os, keys
        )
    }

    /// Whether `action` is implemented by the shared `ComputerUseActions::handle_desktop`
    /// dispatcher instead of inline in [`Self::call_impl`]. These actions used to live on
    /// the (now-removed) `ControlHub` desktop domain before it was folded into `ComputerUse`.
    fn routes_to_desktop_action_dispatcher(action: &str) -> bool {
        matches!(
            action,
            "list_displays"
                | "focus_display"
                | "paste"
                | "list_apps"
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
                | "visual_click"
        )
    }

    /// Property definitions that are byte-identical between the full
    /// (`input_schema`) and text-only (`input_schema_text_only`) variants.
    /// Kept in one place so fields that are NOT model-capability-specific
    /// cannot silently drift between the two hand-authored schemas.
    ///
    /// Fields that differ by design (richer guidance for the multimodal
    /// model, or `screenshot`-only fields) stay inline in each schema.
    fn shared_action_properties() -> Value {
        json!({
            "x": { "type": "integer", "description": "For `mouse_move` and `drag`: X in **global display** units when **`use_screen_coordinates`: true** (required). **Not** for `click`." },
            "y": { "type": "integer", "description": "For `mouse_move` and `drag`: Y in **global display** units when **`use_screen_coordinates`: true** (required). **Not** for `click`." },
            "coordinate_mode": { "type": "string", "enum": ["image", "normalized"], "description": "Ignored for `mouse_move` / `drag` — host rejects image/normalized positioning; always set **`use_screen_coordinates`: true**." },
            "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "For `click`, `click_element`, `drag`: mouse button (default left)." },
            "num_clicks": { "type": "integer", "minimum": 1, "maximum": 3, "description": "For `click`, `click_element`: 1=single (default), 2=double, 3=triple click." },
            "start_x": { "type": "integer", "description": "For `drag`: start X coordinate." },
            "start_y": { "type": "integer", "description": "For `drag`: start Y coordinate." },
            "end_x": { "type": "integer", "description": "For `drag`: end X coordinate." },
            "end_y": { "type": "integer", "description": "For `drag`: end Y coordinate." },
            "text": { "type": "string", "description": "For `type_text`: text to type. Prefer clipboard paste (key_chord) for long content." },
            "ms": { "type": "integer", "description": "For `wait`: duration in milliseconds." },
            "text_query": { "type": "string", "description": "For `move_to_text`, `move_to_target`, `click_target`: visible text to OCR-match on screen (case-insensitive substring)." },
            "identifier_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring on AXIdentifier." },
            "node_idx": { "type": "integer", "minimum": 0, "description": "For `locate`, `click_element`, `app_click`: jump straight to a node returned by the most recent `get_app_state` (field `idx`). Bypasses BFS. macOS only; other platforms return AX_IDX_NOT_SUPPORTED." },
            "app_state_digest": { "type": "string", "description": "For `locate`, `click_element`: optional `state_digest` from the same `get_app_state` call that produced `node_idx`. Stale digest yields AX_IDX_STALE so you re-snapshot." },
            "max_depth": { "type": "integer", "minimum": 1, "maximum": 200, "description": "For `locate`, `click_element`: max BFS depth (default 48). Ignored when `node_idx` is supplied." },
            "filter_combine": { "type": "string", "enum": ["all", "any"], "description": "For `locate`, `click_element`: `all` (default, AND) or `any` (OR) for filter combination. Priority: `node_idx` > `text_contains` > `title_contains`+`role_substring`." },
            "url": { "type": "string", "description": "For `open_url`: URL to open with the system/default browser." },
            "path": { "type": "string", "description": "For `open_file`: local file path to open with its default handler." },
            "app": { "type": ["string", "object"], "description": "For `open_file`: optional app name. For app-scoped actions (including `get_app_shortcuts`): selector object such as `{ \"name\": \"Safari\" }`, `{ \"bundle_id\": \"...\" }`, or `{ \"pid\": 123 }`." },
            "script_type": { "type": "string", "enum": ["applescript", "shell", "bash", "powershell", "cmd"], "description": "For `run_script`: script interpreter/type." },
            "timeout_ms": { "type": "integer", "description": "For `run_script`: timeout in milliseconds." },
            "max_output_bytes": { "type": "integer", "description": "For `run_script` / `clipboard_get`: maximum bytes to return." },
            "clear_first": { "type": "boolean", "description": "For `paste`: select all before pasting." },
            "submit": { "type": "boolean", "description": "For `paste`: press submit keys after pasting." },
            "submit_keys": { "type": "array", "items": { "type": "string" }, "description": "For `paste`: key chord to submit, default `[\"return\"]`." },
            "display_id": { "type": ["integer", "null"], "description": "For `focus_display` or display-pinned desktop actions: display id, or null to clear the pin." },
            "include_hidden": { "type": "boolean", "description": "For `list_apps`: include hidden/background apps." },
            "only_visible": { "type": "boolean", "description": "For `list_apps`: list only visible apps when true." },
            "target": { "type": "object", "description": "For `app_click`: click target such as `{ \"node_idx\": 3 }`, image/screen coordinates, or OCR text." },
            "focus": { "type": ["object", "null"], "description": "For app-scoped text/scroll actions: optional focus target." },
            "predicate": { "type": "object", "description": "For `app_wait_for`: wait predicate." },
            "dx": { "type": "integer", "description": "For app/interactive scroll actions: horizontal delta." },
            "dy": { "type": "integer", "description": "For app/interactive scroll actions: vertical delta." },
            "mouse_button": { "type": "string", "enum": ["left", "right", "middle"], "description": "For app/interactive/visual click actions." },
            "click_count": { "type": "integer", "minimum": 1, "maximum": 3, "description": "For app click actions." },
            "modifier_keys": { "type": "array", "items": { "type": "string" }, "description": "For app click actions: modifier keys to hold." },
            "wait_ms_after": { "type": "integer", "description": "For app click actions: post-click wait in milliseconds." },
            "focus_idx": { "type": "integer", "minimum": 0, "description": "For `app_key_chord`: optional node index to focus first." },
            "poll_ms": { "type": "integer", "description": "For `app_wait_for`: polling interval." }
        })
    }

    /// Builds a schema's `properties` object from action-specific overrides plus
    /// the fields shared with the other model-capability variant (see
    /// [`Self::shared_action_properties`]). The two sets never overlap.
    fn merge_with_shared_properties(specific: Value) -> Value {
        let mut properties = match Self::shared_action_properties() {
            Value::Object(map) => map,
            other => unreachable!("shared_action_properties must return an object, got {other:?}"),
        };
        match specific {
            Value::Object(specific_map) => properties.extend(specific_map),
            other => unreachable!("schema-specific properties must be an object, got {other:?}"),
        }
        Value::Object(properties)
    }

    /// JSON Schema without `screenshot` or screenshot-only fields.
    fn input_schema_text_only() -> Value {
        let properties = Self::merge_with_shared_properties(json!({
            "action": {
                "type": "string",
                "enum": ["click_target", "move_to_target", "click_element", "move_to_text", "click", "mouse_move", "scroll", "drag", "locate", "key_chord", "type_text", "pointer_move_rel", "wait", "list_displays", "focus_display", "paste", "list_apps", "get_app_state", "get_app_shortcuts", "describe_screen", "app_click", "app_type_text", "app_scroll", "app_key_chord", "app_wait_for", "open_app", "open_url", "open_file", "clipboard_get", "clipboard_set", "run_script", "run_apple_script", "get_os_info"],
                "description": "The action to perform. **Primary model is text-only — no `screenshot`.** **Browser boundary:** no input action here may drive a Chromium-family browser (Chrome/Edge/Brave/Arc) — use ControlHub domain=\"browser\" for those; switching focus away with `key_chord` [\"alt\",\"tab\"] / [\"command\",\"tab\"] or `open_app` is always allowed. **ACTION PRIORITY:** 1) Use the `ExecCommand` tool for CLI/terminal/system commands first. 2) **`open_app`** to launch apps. **`run_apple_script`** for AppleScript (macOS). 3) Prefer `key_chord` for shortcuts/navigation. Before guessing a shortcut, call **`get_app_shortcuts`** to look up what a target app actually has registered (e.g. \"what triggers Save in this app?\"), then fire it with `key_chord` / `app_key_chord` — avoids trial-and-error mouse clicks. 4) Only when above fail: `click_target` / `move_to_target` (AX → OCR → screen coords in one call), then lower-level `click_element`, `move_to_text`, or `mouse_move` + `click`. Never guess coordinates. **`describe_screen`** is the text-only equivalent of `screenshot`: it returns a structured text snapshot (frontmost app + AX tree + UI tree text + pointer + window geometry) with NO image — use it to observe and verify state when the primary model cannot view screenshots."
            },
            "use_screen_coordinates": { "type": "boolean", "description": "For `mouse_move`, `drag`: **must be true** — global display coordinates from `move_to_text`, `locate`, AX, or `pointer_global`. **Not** for `click`." },
            "delta_x": { "type": "integer", "description": "For `pointer_move_rel`: horizontal delta (negative=left); also accepted as `dx`. For `scroll`: horizontal wheel delta." },
            "delta_y": { "type": "integer", "description": "For `pointer_move_rel`: vertical delta (negative=up); also accepted as `dy`. For `scroll`: vertical wheel delta." },
            "keys": { "type": "array", "items": { "type": "string" }, "description": "For `key_chord`: keys in order — modifiers first, then the main key. Desktop host waits after pressing modifiers so shortcuts register (important on macOS with IME)." },
            "target_text": { "type": "string", "description": "For `move_to_target` / `click_target`: visible or accessible text. The resolver tries AX first, then OCR." },
            "target_match_index": { "type": "integer", "minimum": 1, "description": "For `move_to_target` / `click_target`: optional 1-based OCR match index when you want a specific candidate." },
            "move_to_text_match_index": { "type": "integer", "minimum": 1, "description": "For `move_to_text` and unified target actions: **1-based** OCR match index." },
            "ocr_region_native": {
                "type": "object",
                "description": "For `move_to_text`: optional global native rectangle for OCR. If omitted, macOS uses the frontmost window bounds from Accessibility; other OSes use the primary display.",
                "properties": {
                    "x0": { "type": "integer", "description": "Top-left X in global screen coordinates." },
                    "y0": { "type": "integer", "description": "Top-left Y in global screen coordinates." },
                    "width": { "type": "integer", "minimum": 1, "description": "Width in the same coordinate unit as x0/y0." },
                    "height": { "type": "integer", "minimum": 1, "description": "Height in the same coordinate unit as x0/y0." }
                }
            },
            "title_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring on AXTitle ONLY. Prefer `text_contains` (also covers AXValue/AXDescription/AXHelp)." },
            "role_substring": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring on AXRole **or AXSubrole** (e.g. \"Button\", \"SearchField\")." },
            "text_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring matched against ANY of AXTitle / AXValue / AXDescription / AXHelp. Prefer this when the visible text is shown via value/description (e.g. AXStaticText cards) instead of title." },
            "app_name": { "type": "string", "description": "For `open_app`: the application name to launch." },
            "script": { "type": "string", "description": "For `run_apple_script`: the AppleScript code to execute. macOS only." },
            "scroll_x": { "type": "integer", "description": "For `scroll`: optional global X coordinate to scroll at. Use with `scroll_y`." },
            "scroll_y": { "type": "integer", "description": "For `scroll`: optional global Y coordinate to scroll at. Use with `scroll_x`." }
        }));
        json!({
            "type": "object",
            "properties": properties,
            "required": ["action"],
            "additionalProperties": false
        })
    }

    /// Max OCR hits to attach as preview crops + AX (multimodal disambiguation).
    const MOVE_TO_TEXT_DISAMBIGUATION_MAX: usize = 8;
    /// Half-size in native screen pixels for each candidate preview (~400×400 logical crop).
    const MOVE_TO_TEXT_PREVIEW_HALF_NATIVE: u32 = 200;

    async fn move_to_text_disambiguation_response(
        host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        context: &ToolUseContext,
        text_query: &str,
        ocr_region_native: Option<OcrRegionNative>,
        matches: &[ScreenOcrTextMatch],
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        Self::require_multimodal_tool_output_for_screenshot(context)?;
        let take = matches.len().min(Self::MOVE_TO_TEXT_DISAMBIGUATION_MAX);
        let mut attachments: Vec<ToolImageAttachment> = Vec::with_capacity(take);
        let mut candidates: Vec<Value> = Vec::with_capacity(take);
        for (i, m) in matches.iter().take(take).enumerate() {
            let idx_1based = i + 1;
            let ax = host_ref
                .accessibility_hit_at_global_point(m.center_x, m.center_y)
                .await?;
            let jpeg = host_ref
                .ocr_preview_crop_jpeg(
                    m.center_x,
                    m.center_y,
                    Self::MOVE_TO_TEXT_PREVIEW_HALF_NATIVE,
                )
                .await?;
            attachments.push(ToolImageAttachment {
                mime_type: "image/jpeg".to_string(),
                data_base64: B64.encode(&jpeg),
            });
            candidates.push(json!({
                "match_index": idx_1based,
                "ocr_text": m.text,
                "confidence": m.confidence,
                "global_center_x": m.center_x,
                "global_center_y": m.center_y,
                "bounds_left": m.bounds_left,
                "bounds_top": m.bounds_top,
                "bounds_width": m.bounds_width,
                "bounds_height": m.bounds_height,
                "accessibility": ax,
                "preview_image_attachment_index": i,
            }));
        }
        let input_coords = json!({
            "kind": "move_to_text",
            "text_query": text_query,
            "ocr_region_native": ocr_region_native,
            "move_to_text_phase": "disambiguation",
        });
        let mut body = json!({
            "success": true,
            "action": "move_to_text",
            "move_to_text_phase": "disambiguation",
            "text_query": text_query,
            "ocr_region_native": ocr_region_native,
            "disambiguation_required": true,
            "instruction": "Several OCR hits for this substring. Each candidate has a **preview JPEG** (same order as `candidates`) and **accessibility** metadata at the OCR center. **Do not** derive `mouse_move` from JPEG pixels. Pick `match_index`, then call **`move_to_text` again** with the same `text_query`, same `ocr_region_native`, and **`move_to_text_match_index`** = that index. Pointer was not moved.",
            "candidates": candidates,
            "total_ocr_matches": matches.len(),
            "candidates_previewed": take,
        });
        if take < matches.len() {
            if let Some(obj) = body.as_object_mut() {
                obj.insert(
                    "truncation_note".to_string(),
                    json!(format!(
                        "Only the first {} of {} OCR matches are previewed; narrow `ocr_region_native` or `text_query` if needed.",
                        take, matches.len()
                    )),
                );
            }
        }
        let body = computer_use_augment_result_json(host_ref, body, Some(input_coords)).await;
        let hint = format!(
            "move_to_text: {} OCR matches — set move_to_text_match_index after viewing {} preview JPEGs + AX. Pointer not moved.",
            matches.len(),
            take
        );
        Ok(vec![ToolResult::ok_with_images(
            body,
            Some(hint),
            attachments,
        )])
    }

    /// Same as [`Self::move_to_text_disambiguation_response`] but **no image attachments** (primary model is text-only).
    async fn move_to_text_disambiguation_text_only(
        host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        text_query: &str,
        ocr_region_native: Option<OcrRegionNative>,
        matches: &[ScreenOcrTextMatch],
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        let take = matches.len().min(Self::MOVE_TO_TEXT_DISAMBIGUATION_MAX);
        let mut candidates: Vec<Value> = Vec::with_capacity(take);
        for (i, m) in matches.iter().take(take).enumerate() {
            let idx_1based = i + 1;
            let ax = host_ref
                .accessibility_hit_at_global_point(m.center_x, m.center_y)
                .await?;
            candidates.push(json!({
                "match_index": idx_1based,
                "ocr_text": m.text,
                "confidence": m.confidence,
                "global_center_x": m.center_x,
                "global_center_y": m.center_y,
                "bounds_left": m.bounds_left,
                "bounds_top": m.bounds_top,
                "bounds_width": m.bounds_width,
                "bounds_height": m.bounds_height,
                "accessibility": ax,
            }));
        }
        let input_coords = json!({
            "kind": "move_to_text",
            "text_query": text_query,
            "ocr_region_native": ocr_region_native,
            "move_to_text_phase": "disambiguation",
        });
        let mut body = json!({
            "success": true,
            "action": "move_to_text",
            "move_to_text_phase": "disambiguation",
            "text_query": text_query,
            "ocr_region_native": ocr_region_native,
            "disambiguation_required": true,
            "instruction": "Several OCR hits for this substring. The primary model **cannot** view screenshots — pick **`move_to_text_match_index`** using **`candidates`** (global_center_* + accessibility) only. Call **`move_to_text` again** with the same `text_query`, same `ocr_region_native`, and **`move_to_text_match_index`** = that index. Pointer was not moved.",
            "candidates": candidates,
            "total_ocr_matches": matches.len(),
            "candidates_previewed": take,
        });
        if take < matches.len() {
            if let Some(obj) = body.as_object_mut() {
                obj.insert(
                    "truncation_note".to_string(),
                    json!(format!(
                        "Only the first {} of {} OCR matches are listed; narrow `ocr_region_native` or `text_query` if needed.",
                        take, matches.len()
                    )),
                );
            }
        }
        let body = computer_use_augment_result_json(host_ref, body, Some(input_coords)).await;
        let hint = format!(
            "move_to_text: {} OCR matches — set move_to_text_match_index using text candidates (no image previews). Pointer not moved.",
            matches.len(),
        );
        Ok(vec![ToolResult::ok(body, Some(hint))])
    }

    /// Text-only observation action: returns a structured text snapshot of
    /// the desktop (frontmost app + AX tree + condensed UI tree text +
    /// pointer + displays) with **no image bytes**. This is the observe and
    /// verify step that closes the cowork loop for text-only primary models
    /// that cannot consume `screenshot` JPEGs.
    async fn describe_screen(
        host: &dyn ComputerUseHost,
        _input: &Value,
        text_only: bool,
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        // For a text-only model this *is* the observation step, so it clears
        // the same guard a `screenshot` would. Without this the guard can only
        // ever be cleared by a capture the model cannot consume.
        if text_only {
            host.computer_use_waive_fresh_capture_guard();
        }
        let session_snap = host.computer_use_session_snapshot().await;
        let interaction = host.computer_use_interaction_state();
        let pointer = session_snap.pointer_global.clone();
        let displays = interaction.displays.clone();

        // Build a frontmost-app selector from the session snapshot. The AX
        // tree (`get_app_state`) is the richest text signal; `enumerate_ui_tree_text`
        // is a condensed fallback that also covers apps whose `get_app_state`
        // AX dump is sparse (Canvas / WebView surfaces).
        let selector = session_snap
            .foreground_application
            .as_ref()
            .map(|fg| AppSelector {
                name: fg.name.clone(),
                bundle_id: fg.bundle_id.clone(),
                pid: fg.process_id,
            });

        let mut ax_tree_text: Option<String> = None;
        let mut ax_nodes_count: Option<usize> = None;
        let mut ax_digest: Option<String> = None;
        let mut window_title: Option<String> = None;
        // Why `ax_tree_text` is empty, when it is. A bare `null` here reads as
        // truncated tool output, and an agent that believes its own results are
        // being cut off will keep re-issuing the same call instead of switching
        // tactic — which is exactly what a null `ax_tree_text` used to cause.
        let ax_tree_status: &str = match selector.as_ref() {
            None => "no_foreground_app",
            Some(app) => match host
                .get_app_state(app.clone(), DESCRIBE_SCREEN_AX_DEPTH, true)
                .await
            {
                Ok(snap) => {
                    // Deliberately drop `snap.screenshot` (JPEG) — describe_screen
                    // never returns image bytes so text-only models are safe.
                    window_title = snap.window_title.clone();
                    ax_nodes_count = Some(snap.nodes.len());
                    ax_digest = Some(snap.digest.clone());
                    ax_tree_text = Some(clip_tree_text(
                        snap.tree_text,
                        DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES,
                    ))
                    .filter(|t| !t.trim().is_empty());
                    if ax_tree_text.is_some() {
                        "ok"
                    } else {
                        "empty_tree"
                    }
                }
                Err(e) => {
                    debug!("describe_screen: get_app_state failed: {}", e);
                    "query_failed"
                }
            },
        };

        let ui_tree_text = host.enumerate_ui_tree_text().await;

        // Turn each non-`ok` status into the tactic that actually works there,
        // so a sparse tree costs one redirect instead of a search.
        let ax_tree_note = match ax_tree_status {
            "ok" => None,
            "no_foreground_app" => Some(
                "No application is frontmost, so there is no AX tree to read. Use `list_apps` to \
find the target, then `open_app` (or `app_click` with an explicit `app` selector) to bring it forward."
                    .to_string(),
            ),
            "empty_tree" => Some(
                "The frontmost app exposes an empty accessibility tree — usual for Electron / \
WebView apps that have not enabled their web-content AX tree, and for an app running with no \
window. This is NOT truncated output: re-calling `describe_screen` returns the same thing. \
Check `window_count` via `open_app`, or target visible text with `move_to_text` / `click_target`."
                    .to_string(),
            ),
            "query_failed" => Some(
                "The AX query failed (commonly missing Accessibility trust, or the app exited). \
Grant Accessibility permission, or fall back to `move_to_text` / `click_target` on visible text."
                    .to_string(),
            ),
            _ => None,
        };

        let mut body = json!({
            "success": true,
            "action": "describe_screen",
            "image_bytes": false,
            "foreground_application": session_snap.foreground_application,
            "pointer_global": pointer,
            "displays": displays,
            "window_title": window_title,
            "ax_tree_text": ax_tree_text,
            "ax_tree_status": ax_tree_status,
            "ax_tree_note": ax_tree_note,
            "ax_nodes_count": ax_nodes_count,
            "ax_state_digest": ax_digest,
            "ui_tree_text": ui_tree_text,
            "output_is_complete": true,
        });

        let input_coords = json!({
            "kind": "describe_screen",
        });
        body = computer_use_augment_result_json(host, body, Some(input_coords)).await;

        // Guide the model to use the returned text fields as its "screen view":
        // pick `node_idx` from `ax_tree_text` for `app_click`/`click_element`, or
        // match visible text via `move_to_text`, and compare `ax_state_digest`
        // before/after an action to verify a mutation.
        let hint = format!(
            "describe_screen: complete text snapshot returned (no image, ax_tree_status={}). \
Use `ax_tree_text` node indices for `app_click`/`click_element`, match visible text with `move_to_text`, \
and compare `ax_state_digest` across actions to verify state changes.{}",
            ax_tree_status,
            if ax_tree_status == "ok" {
                ""
            } else {
                " No AX tree available — read `ax_tree_note` and switch tactic rather than repeating this call."
            }
        );
        Ok(vec![ToolResult::ok(body, Some(hint))])
    }

    /// Screenshot tool results attach JPEGs via `tool_image_attachments`; only providers whose
    /// request converters emit multimodal tool output are supported (Anthropic + OpenAI-compatible).
    fn require_multimodal_tool_output_for_screenshot(ctx: &ToolUseContext) -> OpenBitFunResult<()> {
        if !ctx.primary_model_supports_image_understanding() {
            return Err(OpenBitFunError::tool(
                "The primary model does not accept images; do not use ComputerUse action `screenshot` or other image-producing steps. Use `click_element`, `locate`, `move_to_text` (with `move_to_text_match_index` when listed), `mouse_move` with globals from tool JSON, `key_chord`, etc.".to_string(),
            ));
        }
        if ctx.primary_model_facts().multimodal_tool_output_supported() {
            return Ok(());
        }
        Err(OpenBitFunError::tool(
            "Screenshot results include images in tool results; set the primary model to Anthropic (Claude) or OpenAI-compatible API format. Other providers are not supported for screenshots yet.".to_string(),
        ))
    }

    fn resolve_xy_f64(
        host: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        input: &Value,
        x: i32,
        y: i32,
    ) -> OpenBitFunResult<(f64, f64)> {
        if use_screen_coordinates(input) {
            return Ok((x as f64, y as f64));
        }
        if coordinate_mode(input) == "normalized" {
            host.map_normalized_coords_to_pointer_f64(x, y)
        } else {
            host.map_image_coords_to_pointer_f64(x, y)
        }
    }

    /// `click` must not carry coordinate fields — use `mouse_move` (or `move_to_text`, etc.) separately.
    fn ensure_click_has_no_coordinate_fields(input: &Value) -> OpenBitFunResult<()> {
        if input.get("x").is_some() || input.get("y").is_some() {
            return Err(OpenBitFunError::tool(
                "click does not accept x or y. Position with move_to_text, click_element, or `mouse_move` with use_screen_coordinates: true (globals from tool results), then `click` with only button and num_clicks.".to_string(),
            ));
        }
        if input.get("coordinate_mode").is_some() {
            return Err(OpenBitFunError::tool(
                "click does not accept coordinate_mode. Use `mouse_move` with use_screen_coordinates: true, then `click`.".to_string(),
            ));
        }
        if input.get("use_screen_coordinates").is_some() {
            return Err(OpenBitFunError::tool(
                "click does not accept use_screen_coordinates. Use `mouse_move` with use_screen_coordinates, then `click`.".to_string(),
            ));
        }
        Ok(())
    }

    /// Runtime host OS label for tool description (desktop session matches this process).
    fn host_os_label() -> &'static str {
        match std::env::consts::OS {
            "macos" => "macOS",
            "windows" => "Windows",
            "linux" => "Linux",
            other => other,
        }
    }

    fn key_chord_os_hint() -> &'static str {
        match std::env::consts::OS {
            "macos" => "On this host use command/option/control/shift in key_chord (not Win/Linux names). **System clipboard (prefer over type_text when pasting):** command+a select all, command+c copy, command+x cut, command+v paste — combine with focus/selection shortcuts as needed.",
            "windows" => "On this host use meta (Windows key), alt, control, shift in key_chord. **System clipboard:** control+a/c/x/v for select all, copy, cut, paste.",
            "linux" => "On this host use control, alt, shift, and meta/super as appropriate for the desktop. **System clipboard:** typically control+a/c/x/v (match the app and DE).",
            _ => "Match key_chord modifiers to the host OS in Runtime Context. Prefer standard clipboard chords (select all, copy, cut, paste) before long type_text.",
        }
    }

    async fn find_text_on_screen(
        host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        text_query: &str,
        region_native: Option<crate::agentic::tools::computer_use_host::OcrRegionNative>,
    ) -> OpenBitFunResult<Vec<ScreenOcrTextMatch>> {
        let matches = host_ref
            .ocr_find_text_matches(text_query, region_native)
            .await?;
        Ok(matches
            .into_iter()
            .map(|m| ScreenOcrTextMatch {
                text: m.text,
                confidence: m.confidence,
                center_x: m.center_x,
                center_y: m.center_y,
                bounds_left: m.bounds_left,
                bounds_top: m.bounds_top,
                bounds_width: m.bounds_width,
                bounds_height: m.bounds_height,
            })
            .collect())
    }

    fn locate_query_has_any_target(query: &UiElementLocateQuery) -> bool {
        query.node_idx.is_some()
            || query.text_contains.is_some()
            || query.title_contains.is_some()
            || query.role_substring.is_some()
            || query.identifier_contains.is_some()
    }

    fn target_text_query<'a>(input: &'a Value, query: &'a UiElementLocateQuery) -> Option<&'a str> {
        input
            .get("target_text")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                input
                    .get("text_query")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .or_else(|| {
                query
                    .text_contains
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .or_else(|| {
                query
                    .title_contains
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
    }

    async fn resolve_target_point(
        host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        input: &Value,
    ) -> OpenBitFunResult<ResolvedDesktopTarget> {
        let mut query = parse_locate_query(input);
        if query.text_contains.is_none() {
            if let Some(target_text) = input
                .get("target_text")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                query.text_contains = Some(target_text.to_string());
            }
        }

        let mut ax_error: Option<String> = None;
        if Self::locate_query_has_any_target(&query) {
            match host_ref
                .locate_ui_element_screen_center(query.clone())
                .await
            {
                Ok(res) => {
                    return Ok(ResolvedDesktopTarget {
                        source: "ax".to_string(),
                        x: res.global_center_x,
                        y: res.global_center_y,
                        matched_text: res.matched_title.clone(),
                        matched_role: Some(res.matched_role),
                        matched_identifier: res.matched_identifier,
                        total_matches: Some(res.total_matches.max(1)),
                        selected_match_index: Some(1),
                        warning: (res.total_matches > 1).then(|| {
                            format!(
                                "{} AX elements matched; selected the host-ranked best match.",
                                res.total_matches
                            )
                        }),
                        ax_error: None,
                    });
                }
                Err(err) => {
                    ax_error = Some(err.to_string());
                }
            }
        }

        if let Some(text_query) = Self::target_text_query(input, &query) {
            let ocr_region_native = parse_ocr_region_native(input)?;
            let matches =
                Self::find_text_on_screen(host_ref, text_query, ocr_region_native).await?;
            if !matches.is_empty() {
                let requested_index = input
                    .get("move_to_text_match_index")
                    .or_else(|| input.get("target_match_index"))
                    .and_then(|v| v.as_u64())
                    .map(|u| u as usize);
                let selected = match requested_index {
                    Some(idx) if idx >= 1 && idx <= matches.len() => idx - 1,
                    Some(idx) => {
                        return Err(OpenBitFunError::tool(format!(
                            "target_match_index/move_to_text_match_index must be between 1 and {} (got {}).",
                            matches.len(),
                            idx
                        )));
                    }
                    None => matches
                        .iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| {
                            a.confidence
                                .partial_cmp(&b.confidence)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(idx, _)| idx)
                        .unwrap_or(0),
                };
                let m = &matches[selected];
                return Ok(ResolvedDesktopTarget {
                    source: "ocr".to_string(),
                    x: m.center_x,
                    y: m.center_y,
                    matched_text: Some(m.text.clone()),
                    matched_role: None,
                    matched_identifier: None,
                    total_matches: Some(matches.len() as u32),
                    selected_match_index: Some((selected + 1) as u32),
                    warning: (matches.len() > 1 && requested_index.is_none()).then(|| {
                        format!(
                            "{} OCR matches found for {:?}; selected the highest-confidence match. Pass target_match_index to pin another candidate.",
                            matches.len(),
                            text_query
                        )
                    }),
                    ax_error,
                });
            }
        }

        if input.get("x").is_some() || input.get("y").is_some() {
            ensure_pointer_move_uses_screen_coordinates_only(input)?;
            let x = req_i32(input, "x")?;
            let y = req_i32(input, "y")?;
            let (sx64, sy64) = Self::resolve_xy_f64(host_ref, input, x, y)?;
            if use_screen_coordinates(input) {
                ensure_global_xy_on_display(host_ref, sx64, sy64).await?;
            }
            return Ok(ResolvedDesktopTarget {
                source: "screen_xy".to_string(),
                x: sx64,
                y: sy64,
                matched_text: None,
                matched_role: None,
                matched_identifier: None,
                total_matches: None,
                selected_match_index: None,
                warning: None,
                ax_error,
            });
        }

        Err(OpenBitFunError::tool(
            "move_to_target/click_target requires a target: node_idx, target_text/text_query/text_contains/title_contains, role_substring, identifier_contains, or x/y with use_screen_coordinates: true.".to_string(),
        ))
    }

    /// Writes the exact JPEG sent to the model (including pointer overlay) under the workspace for debugging.
    /// No-op unless [`COMPUTER_USE_DEBUG_SCREENSHOTS_ENV`] is set to `1`.
    async fn try_save_screenshot_for_debug(
        bytes: &[u8],
        context: &ToolUseContext,
        crop: Option<ScreenshotCropCenter>,
        nav_label: Option<&str>,
    ) -> Option<String> {
        if std::env::var(COMPUTER_USE_DEBUG_SCREENSHOTS_ENV).as_deref() != Ok("1") {
            return None;
        }
        let root = context.workspace_root()?;
        let debug_subdir = computer_use_debug_subdir();
        let dir = root.join(&debug_subdir);
        if let Err(e) = tokio::fs::create_dir_all(&dir).await {
            warn!("computer_use debug screenshot mkdir: {}", e);
            return None;
        }
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let suffix = crop
            .map(|c| format!("crop_{}_{}", c.x, c.y))
            .or_else(|| nav_label.map(|s| s.to_string()))
            .unwrap_or_else(|| "full".to_string());
        let fname = format!("cu_{}_{}.jpg", ms, suffix);
        let path = dir.join(&fname);
        if let Err(e) = tokio::fs::write(&path, bytes).await {
            warn!(
                "computer_use debug screenshot write {}: {}",
                path.display(),
                e
            );
            return None;
        }
        match (crop, nav_label) {
            (Some(c), _) => debug!(
                "computer_use debug: wrote point crop center=({}, {}) -> {}",
                c.x,
                c.y,
                path.display()
            ),
            (None, Some(lab)) => debug!(
                "computer_use debug: wrote screenshot ({}) -> {}",
                lab,
                path.display()
            ),
            (None, None) => debug!(
                "computer_use debug: wrote full-screen screenshot -> {}",
                path.display()
            ),
        }
        Self::prune_debug_screenshots(&dir).await;
        Some(format!("{}/{}", debug_subdir.replace('\\', "/"), fname))
    }

    /// Keeps only the newest [`COMPUTER_USE_DEBUG_MAX_FILES`] files (by mtime) in the debug dir.
    async fn prune_debug_screenshots(dir: &std::path::Path) {
        let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
            return;
        };
        let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(meta) = entry.metadata().await else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            files.push((modified, entry.path()));
        }
        if files.len() <= COMPUTER_USE_DEBUG_MAX_FILES {
            return;
        }
        files.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, path) in files.into_iter().skip(COMPUTER_USE_DEBUG_MAX_FILES) {
            if let Err(e) = tokio::fs::remove_file(&path).await {
                warn!(
                    "computer_use debug screenshot prune {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    /// Build tool JSON + one JPEG attachment + assistant hint from an already-captured [`ComputerScreenshot`].
    async fn pack_screenshot_tool_output(
        shot: &ComputerScreenshot,
        debug_rel: Option<String>,
    ) -> OpenBitFunResult<(Value, ToolImageAttachment, String)> {
        let b64 = B64.encode(&shot.bytes);
        let (data, hint) = build_screenshot_tool_body_and_hint(shot, debug_rel);
        let attach = ToolImageAttachment {
            mime_type: shot.mime_type.clone(),
            data_base64: b64,
        };
        Ok((data, attach, hint))
    }
}

/// Verify a global (gx, gy) coordinate falls within at least one display reported by
/// the host. Returns a structured `DESKTOP_COORD_OUT_OF_DISPLAY` error otherwise.
///
/// This is the guard rail that prevents models from passing image-pixel coordinates
/// (taken from a screenshot crop) straight into `mouse_move(use_screen_coordinates=true)`.
pub(crate) async fn ensure_global_xy_on_display(
    host: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
    gx: f64,
    gy: f64,
) -> OpenBitFunResult<()> {
    let displays = host.list_displays().await.unwrap_or_default();
    if displays.is_empty() {
        // Host can't enumerate displays (non-desktop runtime) — skip the guard.
        return Ok(());
    }
    let on_any = displays.iter().any(|d| {
        let x0 = d.origin_x as f64;
        let y0 = d.origin_y as f64;
        let x1 = x0 + d.width_logical as f64;
        let y1 = y0 + d.height_logical as f64;
        gx >= x0 && gx < x1 && gy >= y0 && gy < y1
    });
    if on_any {
        return Ok(());
    }
    let bounds: Vec<String> = displays
        .iter()
        .map(|d| {
            format!(
                "display_id={} bounds=({},{})-({},{}) scale={:.2}",
                d.display_id,
                d.origin_x,
                d.origin_y,
                d.origin_x + d.width_logical as i32,
                d.origin_y + d.height_logical as i32,
                d.scale_factor
            )
        })
        .collect();
    Err(coded_tool_error(ErrorCode::DesktopCoordOutOfDisplay, format!("global=({:.1},{:.1}) does not lie on any visible display. \
         Visible displays: [{}]. Hint: image-pixel coordinates are NOT screen coordinates. \
         Use screenshot.pointer_global, click_element/locate result.global_center_x/y, or move_to_text. \
         To convert image→global, use the screenshot's display_id + scale_factor.", gx,
        gy,
        bounds.join("; "))))
}

/// Helper: build `UiElementLocateQuery` from tool input JSON.
fn parse_locate_query(input: &Value) -> UiElementLocateQuery {
    UiElementLocateQuery {
        title_contains: input
            .get("title_contains")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        role_substring: input
            .get("role_substring")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        identifier_contains: input
            .get("identifier_contains")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        max_depth: input
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        filter_combine: input
            .get("filter_combine")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        text_contains: input
            .get("text_contains")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        node_idx: input
            .get("node_idx")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        app_state_digest: input
            .get("app_state_digest")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

fn parse_ocr_region_native(
    input: &Value,
) -> OpenBitFunResult<Option<crate::agentic::tools::computer_use_host::OcrRegionNative>> {
    let v = input
        .get("ocr_region_native")
        .or_else(|| input.get("ocr_region"));
    let Some(val) = v else {
        return Ok(None);
    };
    if val.is_null() {
        return Ok(None);
    }
    let o = val.as_object().ok_or_else(|| {
        OpenBitFunError::tool(
            "ocr_region_native must be an object { x0, y0, width, height } in global native pixels."
                .to_string(),
        )
    })?;
    let x0 = o.get("x0").and_then(|x| x.as_i64()).ok_or_else(|| {
        OpenBitFunError::tool("ocr_region_native.x0 (integer) is required.".to_string())
    })? as i32;
    let y0 = o.get("y0").and_then(|x| x.as_i64()).ok_or_else(|| {
        OpenBitFunError::tool("ocr_region_native.y0 (integer) is required.".to_string())
    })? as i32;
    let width = o.get("width").and_then(|x| x.as_u64()).ok_or_else(|| {
        OpenBitFunError::tool("ocr_region_native.width (positive integer) is required.".to_string())
    })? as u32;
    let height = o.get("height").and_then(|x| x.as_u64()).ok_or_else(|| {
        OpenBitFunError::tool(
            "ocr_region_native.height (positive integer) is required.".to_string(),
        )
    })? as u32;
    if width == 0 || height == 0 {
        return Err(OpenBitFunError::tool(
            "ocr_region_native width and height must be greater than zero.".to_string(),
        ));
    }
    Ok(Some(
        crate::agentic::tools::computer_use_host::OcrRegionNative {
            x0,
            y0,
            width,
            height,
        },
    ))
}

#[async_trait]
impl Tool for ComputerUseTool {
    fn name(&self) -> &str {
        "ComputerUse"
    }

    async fn description(&self) -> OpenBitFunResult<String> {
        let os = Self::host_os_label();
        let keys = Self::key_chord_os_hint();
        Ok(format!(
            "Desktop automation (host OS: {}). {} All actions in one tool. Send only parameters that apply to the chosen `action`. \
**ACTION PRIORITY (CRITICAL):** Always think in this order before choosing an action:\n\
1. **Terminal/CLI/System commands first** — Use the **`ExecCommand`** tool for terminal commands, system scripts (e.g., macOS `osascript`, AppleScript), shell automation. This is the MOST EFFICIENT approach.\n\
2. **Keyboard shortcuts second** — Use **`key_chord`** for system shortcuts, app shortcuts, navigation keys (Enter, Escape, Tab, Space, Arrow keys). Prefer over mouse when equivalent. Don't know the shortcut for a target app's function? Call **`get_app_shortcuts`** to read its registered menu shortcuts (macOS `AXMenuBar`, Windows UIA menu tree), then fire it with `key_chord` / `app_key_chord` instead of clicking through menus.\n\
3. **Precise UI control last** — Only when above methods fail: prefer **`click_target`** / **`move_to_target`** (AX → OCR → screen coords in one call). Use lower-level **`click_element`**, **`move_to_text`**, or **`mouse_move`** + **`click`** only when you need manual disambiguation.\n\
**Screenshot usage:** **`screenshot`** is ONLY for observing/confirming UI state and extracting text/information — NEVER use screenshot coordinates to control mouse movement. Always use precise methods (AX, OCR, system coordinates) for targeting.\n\
**Cowork-style loop:** **`screenshot`** (observe) → **one** action → **`screenshot`** (verify). Use **`wait`** if UI animates. When **`interaction_state.recommend_screenshot_to_verify_last_action`** is true, call **`screenshot`** next. \
**`click_target` / `move_to_target`:** Unified target resolver. In one call it tries AX (`node_idx`, `text_contains`, `title_contains`, `role_substring`, `identifier_contains`, or `target_text`) first, then OCR (`target_text` / `text_query`), then explicit global `x`/`y` with `use_screen_coordinates: true`. `click_target` moves and clicks authoritatively, avoiding the multi-step locate → move → screenshot → click loop for common targets. \
**`click_element`:** Lower-level Accessibility tree (AX/UIA/AT-SPI) locate + click. Provide `title_contains` / `role_substring` / `identifier_contains`. On macOS, **`TextArea`** and **`TextField`** match both `AXTextArea` and `AXTextField` (many chat apps use TextField for compose). If several text fields match, the host deprioritizes known **search** controls (e.g. WeChat `_SC_SEARCH_FIELD`) and prefers **lower** on-screen fields (composer). Bypasses coordinate screenshot guard — but **not** the browser boundary: no ComputerUse input action (including `app_click` / `interactive_click` / `visual_click`) may drive a Chromium-family browser; use ControlHub domain=\"browser\" instead. \
**`move_to_text`:** OCR-match visible text (`text_query`) and **move the pointer** to it (no click, no keys); **no prior `screenshot` required for targeting** (host captures **raw** pixels for Vision — no agent screenshot overlays; on macOS defaults to the **frontmost window** unless **`ocr_region_native`** overrides). Matching **strips whitespace** between CJK glyphs and allows **small edit distance** when Vision mis-reads one character. The host **trusts** the resulting globals — **next `click`** does **not** require an extra `screenshot` (same as AX). If **several** hits match, the host returns **preview JPEGs + accessibility** per candidate — pick **`move_to_text_match_index`** (1-based) and call **`move_to_text` again** with the same query/region, or narrow with **`ocr_region_native`**. Use **`click`** afterward if you need a mouse press. Prefer after `click_element` misses when text is visible. \
**`click`:** Press at **current pointer only** — **never** pass `x`, `y`, `coordinate_mode`, or `use_screen_coordinates`. Position first with **`move_to_text`**, **`mouse_move`** (**globals only**), or **`click_element`**. After pointer moves, **`screenshot`** again before the next guarded **`click`** when the host requires it. \
**`mouse_move` / `drag`:** **`use_screen_coordinates`: true** required — global coordinates from **`move_to_text`**, **`locate`**, AX, or **`pointer_global`**; never JPEG pixel guesses. \
**`scroll` / `type_text` / `pointer_move_rel` / `wait` / `locate`:** No mandatory pre-screenshot by themselves. **`pointer_move_rel`** is **blocked immediately after `screenshot`** until **`move_to_text`**, **`mouse_move`** (globals), or **`click_element`** — do not nudge from the JPEG. \
**`key_chord`:** Press key combination; prefer over **`click`** when shortcuts or **Enter**/**Escape**/**Tab** suffice. **Mandatory fresh screenshot only** when chord includes Return/Enter. \
**`screenshot`:** JPEG for **confirmation** (optional pointer overlay). Capture prefers the focused application window when available, with full-display fallback. Set **`screenshot_window`: true** to request the focused window. Read **`screenshot_id`**, **`image_content_rect`**, and **`image_global_bounds`** for the image identity and coordinate basis; follow **`interaction_state`** for verification requirements. \
**`type_text`:** Type text; prefer clipboard for long content. Does **not** move the pointer — **Enter** **`key_chord`** may follow without a mandatory `screenshot` unless you moved the pointer since the last capture. If **`screenshot`** shows the correct chat is already open and the input may be focused, **try `type_text` first** before spending steps on `click_element` / `move_to_text`.",
            os, keys,
        ))
    }

    fn short_description(&self) -> String {
        "Inspect the screen and control desktop input for computer-use tasks.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> OpenBitFunResult<String> {
        let vision = context
            .map(|c| c.primary_model_supports_image_understanding())
            .unwrap_or(true);
        if vision {
            self.description().await
        } else {
            Ok(Self::description_text_only())
        }
    }

    fn input_schema(&self) -> Value {
        let properties = Self::merge_with_shared_properties(json!({
            "action": {
                "type": "string",
                "enum": ["screenshot", "describe_screen", "click_target", "move_to_target", "click_element", "move_to_text", "click", "mouse_move", "scroll", "drag", "locate", "key_chord", "type_text", "pointer_move_rel", "wait", "list_displays", "focus_display", "paste", "list_apps", "get_app_state", "get_app_shortcuts", "app_click", "app_type_text", "app_scroll", "app_key_chord", "app_wait_for", "build_interactive_view", "interactive_click", "interactive_type_text", "interactive_scroll", "build_visual_mark_view", "visual_click", "open_app", "open_url", "open_file", "clipboard_get", "clipboard_set", "run_script", "run_apple_script", "get_os_info"],
                "description": "The action to perform. **Browser boundary:** no input action here may drive a Chromium-family browser (Chrome/Edge/Brave/Arc) — use ControlHub domain=\"browser\" for those; switching focus away with `key_chord` [\"alt\",\"tab\"] / [\"command\",\"tab\"] or `open_app` is always allowed. **ACTION PRIORITY:** 1) Use the `ExecCommand` tool for CLI/terminal/system commands (most efficient). 2) **`open_app`** to launch apps by name. **`run_apple_script`** to run AppleScript (macOS). 3) Prefer **`key_chord`** for shortcuts/navigation keys over mouse. Not sure what shortcut a target app uses? Call **`get_app_shortcuts`** first to read its registered menu shortcuts, then fire the winner with `key_chord` / `app_key_chord` instead of clicking through menus. 4) Only when above fail: `click_target` / `move_to_target` (AX → OCR → screen coords in one call) before lower-level `click_element`, `move_to_text`, or `mouse_move` + `click`. **`screenshot`** is for observation/confirmation ONLY — never derive mouse coordinates from screenshots. `click` = press at **current pointer only** (no x/y params). `scroll` supports optional position (`scroll_x`/`scroll_y`). `type_text`, `drag`, `pointer_move_rel`, `wait`, `locate` = standard actions."
            },
            "use_screen_coordinates": { "type": "boolean", "description": "For `mouse_move`, `drag`: **must be true** — global display coordinates (e.g. macOS points) from `move_to_text`, `locate`, AX, or `pointer_global`. **Not** for `click`." },
            "delta_x": { "type": "integer", "description": "For `pointer_move_rel`: horizontal delta (negative=left); also accepted as `dx`. **Not** allowed as the first move after `screenshot` (host). For `scroll`: horizontal wheel delta." },
            "delta_y": { "type": "integer", "description": "For `pointer_move_rel`: vertical delta (negative=up); also accepted as `dy`. **Not** allowed as the first move after `screenshot` (host). For `scroll`: vertical wheel delta." },
            "keys": { "type": "array", "items": { "type": "string" }, "description": "For `key_chord`: keys in order — **modifiers first**, then the main key (e.g. `[\"command\",\"f\"]`). Desktop host waits after pressing modifiers so shortcuts register (important on macOS with IME). Modifiers: command, control, shift, alt/option. Arrows: `up`, `down`, … Host may require a fresh screenshot before Return/Enter when the pointer is stale." },
            "target_text": { "type": "string", "description": "For `move_to_target` / `click_target`: visible or accessible text. The resolver tries AX text first, then OCR text, without requiring a prior screenshot." },
            "target_match_index": { "type": "integer", "minimum": 1, "description": "For `move_to_target` / `click_target`: optional 1-based OCR match index when you want a specific candidate. Alias of `move_to_text_match_index` for the unified target actions." },
            "move_to_text_match_index": { "type": "integer", "minimum": 1, "description": "For `move_to_text` and unified target actions: **1-based** OCR match index. For `move_to_text`, use after a disambiguation response; for `click_target`, use to pin a candidate." },
            "ocr_region_native": {
                "type": "object",
                "description": "For `move_to_text`: optional global native rectangle for OCR. If omitted, macOS uses the frontmost window bounds from Accessibility; other OSes use the primary display. Overrides the automatic region when set. Requires x0, y0, width, height.",
                "properties": {
                    "x0": { "type": "integer", "description": "Top-left X in global screen coordinates (macOS: same logical space as CGDisplayBounds / pointer; not physical Retina pixels)." },
                    "y0": { "type": "integer", "description": "Top-left Y in global screen coordinates (macOS: logical, Y-down)." },
                    "width": { "type": "integer", "minimum": 1, "description": "Width in the same coordinate unit as x0/y0 (logical on macOS)." },
                    "height": { "type": "integer", "minimum": 1, "description": "Height in the same coordinate unit as x0/y0 (logical on macOS)." }
                }
            },
            "title_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring on AXTitle ONLY. Use same language as the app UI. Prefer `text_contains` (also covers AXValue/AXDescription/AXHelp) when in doubt." },
            "role_substring": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring on AXRole **or AXSubrole** (e.g. \"Button\", \"TextField\", \"SearchField\")." },
            "text_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring matched against ANY of AXTitle / AXValue / AXDescription / AXHelp. Best default when the visible label lives in value/description (e.g. AXStaticText cards)." },
            "screenshot_window": { "type": "boolean", "description": "For screenshot: request the focused application window, with full-display fallback when the host cannot resolve it." },
            "app_name": { "type": "string", "description": "For `open_app`: the application name to launch (e.g. \"Safari\", \"WeChat\", \"Visual Studio Code\")." },
            "script": { "type": "string", "description": "For `run_apple_script`: the AppleScript code to execute via `osascript`. macOS only." },
            "opts": { "type": "object", "description": "For `build_interactive_view` / `build_visual_mark_view`: optional view options." },
            "i": { "type": ["integer", "null"], "description": "For interactive/visual actions: element or mark index from the latest view." },
            "scroll_x": { "type": "integer", "description": "For `scroll`: optional global X coordinate to move pointer before scrolling. Use with `scroll_y`. Requires `use_screen_coordinates`: true." },
            "scroll_y": { "type": "integer", "description": "For `scroll`: optional global Y coordinate to move pointer before scrolling. Use with `scroll_x`. Requires `use_screen_coordinates`: true." }
        }));
        json!({
            "type": "object",
            "properties": properties,
            "required": ["action"],
            "additionalProperties": false
        })
    }

    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        let vision = context
            .map(|c| c.primary_model_supports_image_understanding())
            .unwrap_or(true);
        if vision {
            self.input_schema_for_model().await
        } else {
            Self::input_schema_text_only()
        }
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> OpenBitFunResult<Vec<PermissionIntent>> {
        Ok(vec![PermissionIntent::new(
            "computer_use",
            vec![computer_use_permission_resource(input)],
        )])
    }

    async fn is_enabled(&self) -> bool {
        if !computer_use_desktop_available() {
            return false;
        }
        let Ok(service) = GlobalConfigManager::get_service().await else {
            return false;
        };
        let ai: crate::service::config::types::AIConfig =
            service.get_config(Some("ai")).await.unwrap_or_default();
        ai.computer_use_enabled
    }

    async fn is_available_in_context(&self, context: Option<&ToolUseContext>) -> bool {
        if context.map(|ctx| ctx.is_remote()).unwrap_or(false) {
            return false;
        }
        self.is_enabled().await
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        if context.is_remote() {
            return Err(OpenBitFunError::tool(
                "ComputerUse cannot run while the session workspace is remote (SSH).".to_string(),
            ));
        }

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OpenBitFunError::tool("action is required".to_string()))?;

        // Browser-boundary guard: physical input actions (click/type/scroll/…)
        // must not drive a CDP-drivable (Chromium-family) browser from the
        // desktop side — the ControlHub browser domain owns that surface.
        // Read-only observation actions pass through.
        if let Some(err) = super::computer_use_actions::ComputerUseActions::new()
            .desktop_action_targets_browser(action, input, context)
            .await
        {
            return Ok(err_response("computer_use", action, err));
        }

        match action {
            "open_url" | "open_file" | "clipboard_get" | "clipboard_set" | "run_script"
            | "get_os_info" => {
                return super::computer_use_actions::ComputerUseActions::new()
                    .handle_system(action, input, context)
                    .await;
            }
            _ => {}
        }

        if Self::routes_to_desktop_action_dispatcher(action) {
            return super::computer_use_actions::ComputerUseActions::new()
                .handle_desktop(action, input, context)
                .await;
        }

        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            OpenBitFunError::tool(
                "Computer use is only available in the OpenBitFun desktop app.".to_string(),
            )
        })?;

        let host_ref = host.as_ref();

        match action {
            "locate" => execute_computer_use_locate(input, context).await,

            // Text-only observation: the "eyes" of the desktop loop when the
            // primary model cannot consume screenshot images. Returns a
            // structured text snapshot (frontmost app + AX tree + UI tree text
            // + pointer + displays) with NO image bytes. This is the observe and
            // verify step that closes the cowork loop for text-only models.
            "describe_screen" => {
                let text_only = !context.primary_model_supports_image_understanding();
                return Self::describe_screen(host_ref, input, text_only).await;
            }

            // Unified target resolver: AX first, OCR second, explicit screen
            // coordinates last. This is the preferred mouse path for common
            // "move/click the visible thing" requests because it avoids
            // spreading one intent across locate -> move -> click tool calls.
            "move_to_target" | "click_target" => {
                let should_click = action == "click_target";
                let target = Self::resolve_target_point(host_ref, input).await?;
                host_ref.mouse_move_global_f64(target.x, target.y).await?;
                if target.source == "ocr" {
                    ComputerUseHost::computer_use_trust_pointer_after_ocr_move(host_ref);
                }

                let button = input
                    .get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left");
                let num_clicks = input
                    .get("num_clicks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, 3) as u32;

                if should_click {
                    for _ in 0..num_clicks {
                        host_ref.mouse_click_authoritative(button).await?;
                    }
                }

                let target_source = target.source.clone();
                let input_coords = json!({
                    "kind": action,
                    "source": target_source,
                    "resolved_global": { "x": target.x, "y": target.y },
                    "button": if should_click { Some(button) } else { None },
                    "num_clicks": if should_click { Some(num_clicks) } else { None },
                });
                let mut result_json = json!({
                    "success": true,
                    "action": action,
                    "target_resolution_source": target.source,
                    "global_center_x": target.x,
                    "global_center_y": target.y,
                    "matched_text": target.matched_text,
                    "matched_role": target.matched_role,
                    "matched_identifier": target.matched_identifier,
                    "total_matches": target.total_matches,
                    "selected_match_index": target.selected_match_index,
                    "clicked": should_click,
                    "button": if should_click { Some(button) } else { None },
                    "num_clicks": if should_click { Some(num_clicks) } else { None },
                });
                if let Some(warning) = target.warning {
                    result_json["warning"] = json!(warning);
                }
                if let Some(ax_error) = target.ax_error {
                    result_json["ax_fallback_error"] = json!(ax_error);
                }
                let body =
                    computer_use_augment_result_json(host_ref, result_json, Some(input_coords))
                        .await;
                let summary = if should_click {
                    format!(
                        "Resolved target via {} and clicked at ({:.0}, {:.0}).",
                        body.get("target_resolution_source")
                            .and_then(|v| v.as_str())
                            .unwrap_or("target"),
                        target.x,
                        target.y
                    )
                } else {
                    format!(
                        "Resolved target via {} and moved pointer to ({:.0}, {:.0}).",
                        body.get("target_resolution_source")
                            .and_then(|v| v.as_str())
                            .unwrap_or("target"),
                        target.x,
                        target.y
                    )
                };
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            // ---- NEW: click_element (locate + move + click in one call) ----
            "click_element" => {
                let query = parse_locate_query(input);
                // Accept ANY locator that can plausibly identify a node:
                // - text_contains: wide needle over title|value|description|help
                // - node_idx: direct AX-snapshot pin (zero-ambiguity)
                // - title_contains / role_substring / identifier_contains: legacy filters
                // The previous restriction (title/role/identifier only) blocked
                // the most useful path — clicking by visible label that lives
                // in AXValue/AXDescription — and forced models into brittle
                // role guessing.
                if query.title_contains.is_none()
                    && query.text_contains.is_none()
                    && query.role_substring.is_none()
                    && query.identifier_contains.is_none()
                    && query.node_idx.is_none()
                {
                    return Err(OpenBitFunError::tool(
                        "click_element requires at least one of text_contains, title_contains, role_substring, identifier_contains, or node_idx.".to_string(),
                    ));
                }
                let button = input
                    .get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left");
                let num_clicks = input
                    .get("num_clicks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, 3) as u32;

                let res = host_ref
                    .locate_ui_element_screen_center(query.clone())
                    .await?;

                // Move pointer to AX center using global screen coordinates (authoritative).
                host_ref
                    .mouse_move_global_f64(res.global_center_x, res.global_center_y)
                    .await?;

                // Relaxed guard: AX coordinates are authoritative, no fine-screenshot needed.
                host_ref.computer_use_guard_click_allowed_relaxed()?;

                for _ in 0..num_clicks {
                    host_ref.mouse_click_authoritative(button).await?;
                }

                let click_label = match num_clicks {
                    2 => "double",
                    3 => "triple",
                    _ => "single",
                };
                let input_coords = json!({
                    "kind": "click_element",
                    "query": {
                        "title_contains": query.title_contains,
                        "role_substring": query.role_substring,
                        "identifier_contains": query.identifier_contains,
                        "filter_combine": query.filter_combine,
                    },
                    "button": button,
                    "num_clicks": num_clicks,
                });
                let mut result_json = json!({
                    "success": true,
                    "action": "click_element",
                    "matched_role": res.matched_role,
                    "matched_title": res.matched_title,
                    "matched_identifier": res.matched_identifier,
                    "global_center_x": res.global_center_x,
                    "global_center_y": res.global_center_y,
                    "button": button,
                    "num_clicks": num_clicks,
                });
                if let Some(ref pc) = res.parent_context {
                    result_json["parent_context"] = json!(pc);
                }
                if res.total_matches > 1 {
                    result_json["total_matches"] = json!(res.total_matches);
                    result_json["warning"] = json!(format!(
                        "{} elements matched; clicked the best-ranked one. See other_matches if wrong.",
                        res.total_matches
                    ));
                }
                if !res.other_matches.is_empty() {
                    result_json["other_matches"] = json!(res.other_matches);
                }
                let body =
                    computer_use_augment_result_json(host_ref, result_json, Some(input_coords))
                        .await;
                let match_info = if res.total_matches > 1 {
                    format!(" ({} matches)", res.total_matches)
                } else {
                    String::new()
                };
                let summary = format!(
                    "AX click_element: {} {} click on role={} at ({:.0}, {:.0}).{}",
                    button,
                    click_label,
                    res.matched_role,
                    res.global_center_x,
                    res.global_center_y,
                    match_info,
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            "move_to_text" => {
                let text_query = input
                    .get("text_query")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        OpenBitFunError::tool(
                            "move_to_text requires non-empty string field `text_query`."
                                .to_string(),
                        )
                    })?;
                let ocr_region_native = parse_ocr_region_native(input)?;
                let move_to_text_match_index = input
                    .get("move_to_text_match_index")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as u32);

                {
                    let matches =
                        Self::find_text_on_screen(host_ref, text_query, ocr_region_native.clone())
                            .await?;
                    if matches.is_empty() {
                        return Err(OpenBitFunError::tool(format!(
                            "move_to_text found no visible OCR match for {:?}. Take a fresh screenshot and try a shorter or more distinctive substring, or use click_element.",
                            text_query
                        )));
                    }

                    let n = matches.len();
                    if n > 1 && move_to_text_match_index.is_none() {
                        if context.primary_model_supports_image_understanding() {
                            return Self::move_to_text_disambiguation_response(
                                host_ref,
                                context,
                                text_query,
                                ocr_region_native.clone(),
                                &matches,
                            )
                            .await;
                        }
                        return Self::move_to_text_disambiguation_text_only(
                            host_ref,
                            text_query,
                            ocr_region_native.clone(),
                            &matches,
                        )
                        .await;
                    }

                    let sel: usize = match move_to_text_match_index {
                        None => 0,
                        Some(idx) => {
                            if idx < 1 || idx > n as u32 {
                                return Err(OpenBitFunError::tool(format!(
                                    "move_to_text_match_index must be between 1 and {} ({} OCR matches for {:?}).",
                                    n, n, text_query
                                )));
                            }
                            (idx - 1) as usize
                        }
                    };

                    let matched = &matches[sel];
                    host_ref
                        .mouse_move_global_f64(matched.center_x, matched.center_y)
                        .await?;
                    ComputerUseHost::computer_use_trust_pointer_after_ocr_move(host_ref);

                    let other_matches = matches
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != sel)
                        .take(4)
                        .map(|(_, m)| {
                            json!({
                                "text": m.text,
                                "confidence": m.confidence,
                                "center_x": m.center_x,
                                "center_y": m.center_y,
                            })
                        })
                        .collect::<Vec<_>>();

                    let input_coords = json!({
                        "kind": "move_to_text",
                        "text_query": text_query,
                        "ocr_region_native": &ocr_region_native,
                        "move_to_text_match_index": move_to_text_match_index,
                    });
                    let body = computer_use_augment_result_json(
                        host_ref,
                        json!({
                            "success": true,
                            "action": "move_to_text",
                            "move_to_text_phase": "move",
                            "text_query": text_query,
                            "ocr_region_native": ocr_region_native,
                            "matched_text": matched.text,
                            "confidence": matched.confidence,
                            "global_center_x": matched.center_x,
                            "global_center_y": matched.center_y,
                            "bounds_left": matched.bounds_left,
                            "bounds_top": matched.bounds_top,
                            "bounds_width": matched.bounds_width,
                            "bounds_height": matched.bounds_height,
                            "total_matches": matches.len(),
                            "move_to_text_match_index": move_to_text_match_index.unwrap_or(1),
                            "other_matches": other_matches,
                        }),
                        Some(input_coords),
                    )
                    .await;
                    let summary = format!(
                        "OCR move_to_text: matched {:?} at ({:.0}, {:.0}) [index {} of {}]. Pointer is from trusted global OCR — you may **`click`** next without a separate **`screenshot`** (host clears stale-capture guard).",
                        matched.text,
                        matched.center_x,
                        matched.center_y,
                        sel + 1,
                        matches.len()
                    );
                    Ok(vec![ToolResult::ok(body, Some(summary))])
                }
            }

            // ---- click: current pointer only; use `mouse_move` / `move_to_text` separately ----
            "click" => {
                Self::ensure_click_has_no_coordinate_fields(input)?;

                let button = input
                    .get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left");
                let num_clicks = input
                    .get("num_clicks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, 3) as u32;

                host_ref.computer_use_guard_click_allowed()?;

                for _ in 0..num_clicks {
                    host_ref.mouse_click_authoritative(button).await?;
                }

                let click_label = match num_clicks {
                    2 => "double",
                    3 => "triple",
                    _ => "single",
                };
                let input_coords = json!({
                    "kind": "click",
                    "button": button,
                    "num_clicks": num_clicks,
                    "at_current_pointer_only": true,
                });
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({
                        "success": true,
                        "action": "click",
                        "button": button,
                        "num_clicks": num_clicks,
                    }),
                    Some(input_coords),
                )
                .await;
                let summary = format!(
                    "{} {} click at current pointer only (no move).",
                    button, click_label
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            // ---- mouse_move (absolute pointer move in global screen coordinates) ----
            "mouse_move" => {
                ensure_pointer_move_uses_screen_coordinates_only(input)?;
                let x = req_i32(input, "x")?;
                let y = req_i32(input, "y")?;
                let (sx64, sy64) = Self::resolve_xy_f64(host_ref, input, x, y)?;
                if use_screen_coordinates(input) {
                    ensure_global_xy_on_display(host_ref, sx64, sy64).await?;
                }
                host_ref.mouse_move_global_f64(sx64, sy64).await?;
                let mode = coordinate_mode(input);
                let use_screen = use_screen_coordinates(input);
                let input_coords = json!({
                    "kind": "mouse_move",
                    "raw": { "x": x, "y": y, "coordinate_mode": mode, "use_screen_coordinates": use_screen },
                    "resolved_global": { "x": sx64, "y": sy64 },
                });
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({
                        "success": true,
                        "action": "mouse_move",
                        "x": x, "y": y,
                        "pointer_x": sx64.round() as i32,
                        "pointer_y": sy64.round() as i32,
                        "coordinate_mode": mode,
                        "use_screen_coordinates": use_screen,
                    }),
                    Some(input_coords),
                )
                .await;
                let summary = format!(
                    "Moved pointer to (~{}, ~{}).",
                    sx64.round() as i32,
                    sy64.round() as i32
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            // ---- scroll (mouse wheel; optional scroll_x/scroll_y move the pointer first) ----
            "scroll" => {
                let dx = input.get("delta_x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let dy = input.get("delta_y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                if dx == 0 && dy == 0 {
                    return Err(OpenBitFunError::tool(
                        "scroll requires non-zero delta_x and/or delta_y".to_string(),
                    ));
                }
                // Positional scroll: move pointer to target before scrolling.
                let scroll_pos_x = input.get("scroll_x").and_then(|v| v.as_i64());
                let scroll_pos_y = input.get("scroll_y").and_then(|v| v.as_i64());
                if let (Some(sx), Some(sy)) = (scroll_pos_x, scroll_pos_y) {
                    let (gx, gy) = (sx as f64, sy as f64);
                    // Same display-bounds guard as mouse_move/drag: reject
                    // image-pixel coordinates passed as globals.
                    ensure_global_xy_on_display(host_ref, gx, gy).await?;
                    host_ref.mouse_move_global_f64(gx, gy).await?;
                    host_ref.wait_ms(30).await?;
                }
                host_ref.scroll(dx, dy).await?;
                let input_coords = json!({ "kind": "scroll", "delta_x": dx, "delta_y": dy });
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({ "success": true, "action": "scroll", "delta_x": dx, "delta_y": dy }),
                    Some(input_coords),
                )
                .await;
                let summary = format!("Scrolled ({}, {}).", dx, dy);
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            // ---- NEW: drag (mouse_down at start + move to end + mouse_up) ----
            "drag" => {
                ensure_pointer_move_uses_screen_coordinates_only(input)?;
                let start_x = req_i32(input, "start_x")?;
                let start_y = req_i32(input, "start_y")?;
                let end_x = req_i32(input, "end_x")?;
                let end_y = req_i32(input, "end_y")?;
                let button = input
                    .get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left");

                let (sx0, sy0) = Self::resolve_xy_f64(host_ref, input, start_x, start_y)?;
                let (sx1, sy1) = Self::resolve_xy_f64(host_ref, input, end_x, end_y)?;

                // Delegate to the host `drag` gesture. The default trait impl
                // composes foreground mouse_down/move/up; desktop hosts override
                // it with background (non-disruptive) drag on macOS/Windows.
                let duration_ms = input
                    .get("duration_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100);
                host_ref
                    .drag((sx0, sy0), (sx1, sy1), button, duration_ms)
                    .await?;
                ComputerUseHost::computer_use_after_committed_ui_action(host_ref);

                let input_coords = json!({
                    "kind": "drag",
                    "start": { "x": start_x, "y": start_y },
                    "end": { "x": end_x, "y": end_y },
                    "button": button,
                });
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({
                        "success": true,
                        "action": "drag",
                        "start_global": { "x": sx0.round() as i32, "y": sy0.round() as i32 },
                        "end_global": { "x": sx1.round() as i32, "y": sy1.round() as i32 },
                        "button": button,
                    }),
                    Some(input_coords),
                )
                .await;
                let summary = format!(
                    "Dragged from (~{}, ~{}) to (~{}, ~{}).",
                    sx0.round() as i32,
                    sy0.round() as i32,
                    sx1.round() as i32,
                    sy1.round() as i32,
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            "screenshot" => {
                // Text-only soft gate: instead of hard-rejecting (which crashes
                // the agent loop when a stale hint or the model itself asks for
                // `screenshot`), return a success envelope that points the model
                // at the text-only observe action. The model keeps its turn and
                // switches to `describe_screen` / AX / OCR / keyboard tactics.
                if !context.primary_model_supports_image_understanding() {
                    // A text-only `screenshot` never captures anything, so it
                    // can never clear the stale-capture guard the usual way.
                    // Waive it here: otherwise the guard's own recovery advice
                    // ("call `screenshot` first") is an instruction the model
                    // can follow forever without ever being allowed to click.
                    host_ref.computer_use_waive_fresh_capture_guard();
                    let body = json!({
                        "success": true,
                        "action": "screenshot",
                        "screenshot_unavailable": true,
                        "reason": "primary_model_is_text_only",
                        "stale_capture_guard": "waived",
                        "instruction": "The primary model cannot consume image bytes, so `screenshot` produced nothing. Use `describe_screen` to observe the desktop as text (frontmost app + AX tree + UI tree text + pointer), then act with `click_target`/`click_element`/`move_to_text`/`key_chord`/`paste`. Never retry `screenshot`. The fresh-capture guard has been waived, so `click` and Enter `key_chord` are unblocked."
                    });
                    let input_coords = json!({ "kind": "screenshot", "text_only": true });
                    let body =
                        computer_use_augment_result_json(host_ref, body, Some(input_coords)).await;
                    return Ok(vec![ToolResult::ok(
                        body,
                        Some(
                            "screenshot unavailable (text-only model): use describe_screen to observe."
                                .to_string(),
                        ),
                    )]);
                }
                Self::require_multimodal_tool_output_for_screenshot(context)?;
                let (params, ignored_crop_for_quadrant) = parse_screenshot_params(input)?;
                let crop_for_debug = params.crop_center;
                let nav_debug = params.navigate_quadrant.map(|q| match q {
                    ComputerUseNavigateQuadrant::TopLeft => "nav_tl",
                    ComputerUseNavigateQuadrant::TopRight => "nav_tr",
                    ComputerUseNavigateQuadrant::BottomLeft => "nav_bl",
                    ComputerUseNavigateQuadrant::BottomRight => "nav_br",
                });
                let shot = host_ref.screenshot_display(params).await?;
                // Update screenshot hash for visual change detection
                let shot_hash = hash_screenshot_bytes(&shot.bytes);
                host_ref.update_screenshot_hash(shot_hash);
                let crop_for_debug = shot.screenshot_crop_center.or(crop_for_debug);
                let debug_rel = Self::try_save_screenshot_for_debug(
                    &shot.bytes,
                    context,
                    crop_for_debug,
                    nav_debug,
                )
                .await;
                let input_coords = json!({
                    "kind": "screenshot",
                    "screenshot_reset_navigation": params.reset_navigation,
                    "screenshot_crop_ignored_for_quadrant": ignored_crop_for_quadrant,
                    "screenshot_crop_center": shot.screenshot_crop_center.map(|c| json!({ "x": c.x, "y": c.y })),
                    "screenshot_crop_half_extent_native": shot.point_crop_half_extent_native,
                    "screenshot_implicit_confirmation_crop_applied": shot.implicit_confirmation_crop_applied,
                    "screenshot_navigate_quadrant": params.navigate_quadrant.map(|q| match q {
                        ComputerUseNavigateQuadrant::TopLeft => "top_left",
                        ComputerUseNavigateQuadrant::TopRight => "top_right",
                        ComputerUseNavigateQuadrant::BottomLeft => "bottom_left",
                        ComputerUseNavigateQuadrant::BottomRight => "bottom_right",
                    }),
                });
                let (mut data, attach, mut hint) =
                    Self::pack_screenshot_tool_output(&shot, debug_rel).await?;
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(
                        "action".to_string(),
                        Value::String("screenshot".to_string()),
                    );
                    if ignored_crop_for_quadrant {
                        obj.insert(
                            "screenshot_crop_center_ignored".to_string(),
                            Value::Bool(true),
                        );
                        obj.insert(
                            "screenshot_params_note".to_string(),
                            Value::String(
                                "screenshot_navigate_quadrant was set; screenshot_crop_center_x/y in this request were ignored."
                                    .to_string(),
                            ),
                        );
                        hint = format!(
                            "{} `screenshot_crop_center_*` were ignored because `screenshot_navigate_quadrant` takes precedence.",
                            hint
                        );
                    }
                }
                let data =
                    computer_use_augment_result_json(host_ref, data, Some(input_coords)).await;
                Ok(vec![ToolResult::ok_with_images(
                    data,
                    Some(hint),
                    vec![attach],
                )])
            }

            "pointer_move_rel" => {
                // Accept both `delta_x`/`delta_y` (canonical) and `dx`/`dy` (alias) so that
                // models which guess the natural form do not crash on the schema.
                let dx_alias_used = input.get("delta_x").is_none() && input.get("dx").is_some();
                let dy_alias_used = input.get("delta_y").is_none() && input.get("dy").is_some();
                let dx = input
                    .get("delta_x")
                    .or_else(|| input.get("dx"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let dy = input
                    .get("delta_y")
                    .or_else(|| input.get("dy"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                if dx == 0 && dy == 0 {
                    return Err(OpenBitFunError::tool(
                        "pointer_move_rel requires a non-zero delta. Accepts `delta_x`|`dx` and `delta_y`|`dy` (screen pixels); at least one must be non-zero.".to_string(),
                    ));
                }
                host_ref.pointer_move_relative(dx, dy).await?;
                let alias_note = match (dx_alias_used, dy_alias_used) {
                    (true, true) => Some("dx|dy"),
                    (true, false) => Some("dx"),
                    (false, true) => Some("dy"),
                    (false, false) => None,
                };
                let mut input_coords = json!({
                    "kind": "pointer_move_rel",
                    "delta_x": dx,
                    "delta_y": dy,
                });
                if let Some(a) = alias_note {
                    input_coords["deprecated_alias_used"] = json!(a);
                }
                let mut payload = json!({
                    "success": true,
                    "action": "pointer_move_rel",
                    "delta_x": dx,
                    "delta_y": dy,
                });
                if let Some(a) = alias_note {
                    payload["deprecated_alias_used"] = json!(a);
                }
                let body =
                    computer_use_augment_result_json(host_ref, payload, Some(input_coords)).await;
                let summary = format!(
                    "Moved pointer relatively by ({}, {}) screen pixels.",
                    dx, dy
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "key_chord" => {
                // UX: accept BOTH `keys: ["escape"]` (canonical) AND
                // `keys: "escape"` / `key: "escape"` (common mistakes from
                // the model). The wrong-shape variants are silently
                // coerced — in practice every regression caused by being
                // strict here costs a full round-trip to fix. Genuine
                // missing-keys is reported with an explicit example so
                // the model recovers in one shot.
                let keys: Vec<String> = match input.get("keys") {
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                    Some(Value::String(s)) => vec![s.to_string()],
                    None => match input.get("key").and_then(|v| v.as_str()) {
                        Some(s) => vec![s.to_string()],
                        None => {
                            return Err(coded_tool_error(ErrorCode::InvalidParams, "key_chord requires `keys` as a JSON array of key names\nHints: example { \"keys\": [\"command\", \"v\"] } | for a single key { \"keys\": [\"return\"] } | use lowercase canonical names: command, control, option, shift, return, escape, tab, space, delete, arrow_up/down/left/right, f1..f12"));
                        }
                    },
                    _ => {
                        return Err(coded_tool_error(ErrorCode::InvalidParams, "key_chord `keys` must be a string or array of strings\nHints: example { \"keys\": [\"command\", \"v\"] }"));
                    }
                };
                if keys.is_empty() {
                    return Err(coded_tool_error(ErrorCode::InvalidParams, "key_chord `keys` must not be empty\nHints: example { \"keys\": [\"return\"] }"));
                }
                host_ref.key_chord(keys.clone()).await?;
                let input_coords = json!({ "kind": "key_chord", "keys": keys });
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({ "success": true, "action": "key_chord", "keys": keys }),
                    Some(input_coords),
                )
                .await;
                let summary = "Key chord sent.".to_string();
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "type_text" => {
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| OpenBitFunError::tool("text is required".to_string()))?;
                host_ref.type_text(text).await?;
                let input_coords =
                    json!({ "kind": "type_text", "char_count": text.chars().count() });
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({ "success": true, "action": "type_text", "chars": text.chars().count() }),
                    Some(input_coords),
                )
                .await;
                let summary = format!(
                    "Typed {} character(s) into the focused target.",
                    text.chars().count()
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "wait" => {
                let ms = input
                    .get("ms")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| OpenBitFunError::tool("ms is required".to_string()))?;
                host_ref.wait_ms(ms).await?;
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({ "success": true, "action": "wait", "ms": ms }),
                    None,
                )
                .await;
                Ok(vec![ToolResult::ok(
                    body,
                    Some(format!("Waited {} ms.", ms)),
                )])
            }
            "open_app" => {
                let app_name = input
                    .get("app_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        OpenBitFunError::tool("open_app requires `app_name` parameter.".to_string())
                    })?;
                let result = host_ref.open_app(app_name).await?;
                // A live process with zero windows is the one launch outcome
                // that looks like success but leaves nothing to act on. Name it
                // explicitly and say what to do, rather than letting the agent
                // rediscover it through a chain of failing AX queries.
                let windowless = result.success && result.window_count == Some(0);
                let next_step = if windowless {
                    Some(format!(
                        "'{}' is running (PID {}) but owns no window, so there is nothing on screen to click. \
The host already retried via `open -b`. Re-run `open_app`, or ask the user to open the app's main window (e.g. from its Dock icon). \
Do not fall back to screen-coordinate clicks — there is no window to hit.",
                        result.app_name,
                        result
                            .process_id
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "?".to_string()),
                    ))
                } else {
                    None
                };
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({
                        "success": result.success,
                        "action": "open_app",
                        "app_name": result.app_name,
                        "process_id": result.process_id,
                        "error_message": result.error_message,
                        // Address the app by `bundle_id` from here on: the name
                        // used to launch it, its executable name and its bundle
                        // id are often three different strings.
                        "bundle_id": result.bundle_id,
                        "process_name": result.process_name,
                        "window_count": result.window_count,
                        "launch_path": result.launch_path,
                        "windowless": windowless,
                        "next_step": next_step,
                    }),
                    None,
                )
                .await;
                let summary = if !result.success {
                    format!(
                        "Failed to open '{}': {}",
                        result.app_name,
                        result.error_message.as_deref().unwrap_or("unknown error")
                    )
                } else if windowless {
                    format!(
                        "Opened '{}'{} but it has NO window — nothing is on screen to act on.",
                        result.app_name,
                        result
                            .process_id
                            .map(|p| format!(" (PID {})", p))
                            .unwrap_or_default()
                    )
                } else {
                    format!(
                        "Opened app '{}'{}{}.",
                        result.app_name,
                        result
                            .process_id
                            .map(|p| format!(" (PID {})", p))
                            .unwrap_or_default(),
                        result
                            .window_count
                            .map(|n| format!(", {} window(s)", n))
                            .unwrap_or_default()
                    )
                };
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            "run_apple_script" => {
                let script = input
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        OpenBitFunError::tool(
                            "run_apple_script requires `script` parameter.".to_string(),
                        )
                    })?;
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = script;
                    return Err(OpenBitFunError::tool(
                        "run_apple_script is only available on macOS.".to_string(),
                    ));
                }
                #[cfg(target_os = "macos")]
                {
                    let script_owned = script.to_string();
                    let output = tokio::task::spawn_blocking(move || {
                        std::process::Command::new("/usr/bin/osascript")
                            .args(["-e", &script_owned])
                            .output()
                    })
                    .await
                    .map_err(|e| OpenBitFunError::tool(format!("spawn: {}", e)))?
                    .map_err(|e| OpenBitFunError::tool(format!("osascript: {}", e)))?;

                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let success = output.status.success();

                    let body = computer_use_augment_result_json(
                        host_ref,
                        json!({
                            "success": success,
                            "action": "run_apple_script",
                            "stdout": stdout,
                            "stderr": stderr,
                        }),
                        None,
                    )
                    .await;
                    let summary = if success {
                        format!(
                            "AppleScript executed.{}",
                            if stdout.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    " Output: {}",
                                    crate::util::truncate_at_char_boundary(&stdout, 200)
                                )
                            }
                        )
                    } else {
                        format!(
                            "AppleScript error: {}",
                            crate::util::truncate_at_char_boundary(&stderr, 200)
                        )
                    };
                    Ok(vec![ToolResult::ok(body, Some(summary))])
                }
            }

            _ => Err(OpenBitFunError::tool(format!("Unknown action: {}", action))),
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedDesktopTarget {
    source: String,
    x: f64,
    y: f64,
    matched_text: Option<String>,
    matched_role: Option<String>,
    matched_identifier: Option<String>,
    total_matches: Option<u32>,
    selected_match_index: Option<u32>,
    warning: Option<String>,
    ax_error: Option<String>,
}

#[derive(Debug, Clone)]
struct ScreenOcrTextMatch {
    text: String,
    confidence: f32,
    center_x: f64,
    center_y: f64,
    bounds_left: f64,
    bounds_top: f64,
    bounds_width: f64,
    bounds_height: f64,
}

fn req_i32(input: &Value, key: &str) -> OpenBitFunResult<i32> {
    input
        .get(key)
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .ok_or_else(|| OpenBitFunError::tool(format!("{} is required (integer)", key)))
}

#[cfg(test)]
mod tests {
    use super::{
        clip_tree_text, ComputerUseTool, APP_STATE_TREE_TEXT_MAX_BYTES,
        DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES,
    };
    use crate::agentic::tools::computer_use_host::{
        ComputerScreenshot, ComputerUseForegroundApplication, ComputerUseHost,
        ComputerUsePermissionSnapshot, ComputerUseScreenshotParams, ComputerUseSessionSnapshot,
    };
    use crate::agentic::tools::framework::{Tool, ToolUseContext};
    use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
    use serde_json::{json, Value};

    #[test]
    fn computer_use_permission_resource_identifies_action_and_safe_target() {
        let tool = ComputerUseTool::new();
        let context = ToolUseContext::for_tool_listing(None, None);
        let intents = tool
            .permission_intents(
                &json!({
                    "action": "open_app",
                    "app_name": "Visual Studio Code",
                    "text": "secret text must not be projected",
                    "script": "secret script must not be projected"
                }),
                &context,
            )
            .expect("permission intent");

        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].action, "computer_use");
        assert_eq!(
            intents[0].resources,
            ["open_app:app_name=Visual Studio Code".to_string()]
        );
        assert!(!intents[0].resources[0].contains("secret"));
    }

    fn action_enum(schema: &Value) -> Vec<String> {
        schema
            .get("properties")
            .and_then(|p| p.get("action"))
            .and_then(|a| a.get("enum"))
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Text-only schema must NOT advertise `screenshot` (hard-rejected at runtime)
    /// but MUST advertise `describe_screen` (the text-only observe action).
    #[test]
    fn text_only_schema_omits_screenshot_and_offers_describe_screen() {
        let schema = ComputerUseTool::input_schema_text_only();
        let actions = action_enum(&schema);
        assert!(
            !actions.iter().any(|a| a == "screenshot"),
            "text-only schema must not list `screenshot` — it is rejected for text-only models. Got: {:?}",
            actions
        );
        assert!(
            actions.iter().any(|a| a == "describe_screen"),
            "text-only schema must list `describe_screen` as the observe action. Got: {:?}",
            actions
        );
    }

    /// Full (visual) schema keeps `screenshot` and also offers `describe_screen`.
    #[test]
    fn full_schema_keeps_screenshot_and_offers_describe_screen() {
        let schema = ComputerUseTool::new().input_schema();
        let actions = action_enum(&schema);
        assert!(actions.iter().any(|a| a == "screenshot"));
        assert!(actions.iter().any(|a| a == "describe_screen"));
    }

    /// Text-only tool description must steer the model to `describe_screen` and
    /// away from `screenshot`.
    #[test]
    fn text_only_description_steers_to_describe_screen() {
        let desc = ComputerUseTool::description_text_only();
        assert!(desc.contains("describe_screen"));
        assert!(desc.to_lowercase().contains("do not"));
    }

    fn property_keys(schema: &Value) -> std::collections::BTreeSet<String> {
        schema
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Every field in [`ComputerUseTool::shared_action_properties`] must appear,
    /// byte-identical, in both the full and text-only schemas. This is the
    /// invariant the shared-properties extraction exists to protect: if someone
    /// edits a "shared" field only in one schema override block, it stops being
    /// shared and this test should fail loudly instead of the drift going
    /// unnoticed.
    #[test]
    fn shared_action_properties_are_identical_in_both_schemas() {
        let shared = ComputerUseTool::shared_action_properties();
        let shared_map = shared.as_object().expect("shared properties is an object");
        assert!(
            !shared_map.is_empty(),
            "shared_action_properties should not be empty"
        );

        let full = ComputerUseTool::new().input_schema();
        let text_only = ComputerUseTool::input_schema_text_only();

        for (key, expected) in shared_map {
            assert_eq!(
                full.get("properties").and_then(|p| p.get(key)),
                Some(expected),
                "shared property `{key}` diverged in the full schema"
            );
            assert_eq!(
                text_only.get("properties").and_then(|p| p.get(key)),
                Some(expected),
                "shared property `{key}` diverged in the text-only schema"
            );
        }
    }

    /// Screenshot-only fields must exist solely in the full (multimodal) schema:
    /// text-only models never receive a `screenshot` action, so these params
    /// would be dead/misleading in that schema.
    #[test]
    fn screenshot_only_fields_are_absent_from_text_only_schema() {
        let full_keys = property_keys(&ComputerUseTool::new().input_schema());
        let text_only_keys = property_keys(&ComputerUseTool::input_schema_text_only());

        let screenshot_only_fields = ["screenshot_window"];
        for field in screenshot_only_fields {
            assert!(
                full_keys.contains(field),
                "full schema should contain `{field}`"
            );
            assert!(
                !text_only_keys.contains(field),
                "text-only schema should NOT contain `{field}`"
            );
        }
    }

    #[test]
    fn schemas_do_not_advertise_ignored_screenshot_inputs() {
        let full = property_keys(&ComputerUseTool::new().input_schema());
        let text = property_keys(&ComputerUseTool::input_schema_text_only());
        for field in [
            "screenshot_crop_center_x",
            "screenshot_crop_center_y",
            "screenshot_crop_half_extent_native",
            "screenshot_navigate_quadrant",
            "screenshot_reset_navigation",
            "screenshot_implicit_center",
        ] {
            assert!(
                !full.contains(field) && !text.contains(field),
                "ignored field {field}"
            );
        }
    }

    /// Visual-only actions must not be advertised to text-only models: their
    /// results are marked-up screenshots, and `interactive_type_text` /
    /// `interactive_scroll` address elements by the `i` index of a view only a
    /// vision-capable model can build. Their parameters go with them.
    #[test]
    fn visual_only_actions_are_absent_from_text_only_schema() {
        let full_actions = action_enum(&ComputerUseTool::new().input_schema());
        let text_only_actions = action_enum(&ComputerUseTool::input_schema_text_only());
        for action in [
            "build_interactive_view",
            "interactive_click",
            "build_visual_mark_view",
            "visual_click",
            "interactive_type_text",
            "interactive_scroll",
        ] {
            assert!(
                full_actions.iter().any(|a| a == action),
                "full schema should list `{action}`"
            );
            assert!(
                !text_only_actions.iter().any(|a| a == action),
                "text-only schema should NOT list `{action}`"
            );
        }
        let text_only_keys = property_keys(&ComputerUseTool::input_schema_text_only());
        assert!(
            !text_only_keys.contains("opts"),
            "`opts` only configures the removed view-building actions"
        );
        assert!(
            !text_only_keys.contains("i"),
            "`i` indexes a view no text-only action can build"
        );
        let full_keys = property_keys(&ComputerUseTool::new().input_schema());
        assert!(full_keys.contains("opts"));
        assert!(full_keys.contains("i"));
    }

    /// The `Bash` tool is not registered in the product tool registry
    /// (`ExecCommand` is), so naming it as the top-priority action sends the
    /// model at a tool that does not exist.
    #[tokio::test]
    async fn descriptions_and_schemas_never_reference_a_nonexistent_bash_tool() {
        let full_description = ComputerUseTool::new()
            .description()
            .await
            .expect("description");
        let text_only_description = ComputerUseTool::description_text_only();
        let full_schema = ComputerUseTool::new().input_schema().to_string();
        let text_only_schema = ComputerUseTool::input_schema_text_only().to_string();
        for blob in [
            full_description.as_str(),
            text_only_description.as_str(),
            full_schema.as_str(),
            text_only_schema.as_str(),
        ] {
            assert!(
                !blob.contains("Bash"),
                "ComputerUse text must not name the unregistered Bash tool"
            );
            assert!(blob.contains("ExecCommand"));
        }
    }

    /// Minimal host whose only signal is a Chromium-family frontmost app;
    /// every input primitive fails loudly so the test proves the browser
    /// guard rejects `click` before any physical input is attempted.
    #[derive(Debug)]
    struct ChromeForegroundHost;

    fn not_expected<T>() -> OpenBitFunResult<T> {
        Err(OpenBitFunError::tool(
            "not expected to be called in this test".to_string(),
        ))
    }

    #[async_trait::async_trait]
    impl ComputerUseHost for ChromeForegroundHost {
        async fn permission_snapshot(&self) -> OpenBitFunResult<ComputerUsePermissionSnapshot> {
            not_expected()
        }
        async fn request_accessibility_permission(&self) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn request_screen_capture_permission(&self) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn screenshot_display(
            &self,
            _params: ComputerUseScreenshotParams,
        ) -> OpenBitFunResult<ComputerScreenshot> {
            not_expected()
        }
        fn map_image_coords_to_pointer(&self, _x: i32, _y: i32) -> OpenBitFunResult<(i32, i32)> {
            not_expected()
        }
        fn map_normalized_coords_to_pointer(
            &self,
            _x: i32,
            _y: i32,
        ) -> OpenBitFunResult<(i32, i32)> {
            not_expected()
        }
        async fn mouse_move(&self, _x: i32, _y: i32) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn pointer_move_relative(&self, _dx: i32, _dy: i32) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn mouse_click(&self, _button: &str) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn scroll(&self, _delta_x: i32, _delta_y: i32) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn key_chord(&self, _keys: Vec<String>) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn type_text(&self, _text: &str) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn wait_ms(&self, _ms: u64) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn computer_use_session_snapshot(&self) -> ComputerUseSessionSnapshot {
            ComputerUseSessionSnapshot {
                foreground_application: Some(ComputerUseForegroundApplication {
                    name: Some("Google Chrome".to_string()),
                    bundle_id: Some("com.google.Chrome".to_string()),
                    process_name: Some("Google Chrome".to_string()),
                    process_id: Some(4242),
                }),
                pointer_global: None,
            }
        }
    }

    /// Host that records whether the stale-capture guard was waived, and
    /// reports no frontmost app so `describe_screen` exercises its
    /// nothing-to-observe branch.
    #[derive(Debug, Default)]
    struct GuardRecordingHost {
        waived: std::sync::atomic::AtomicBool,
    }

    #[async_trait::async_trait]
    impl ComputerUseHost for GuardRecordingHost {
        async fn permission_snapshot(&self) -> OpenBitFunResult<ComputerUsePermissionSnapshot> {
            not_expected()
        }
        async fn request_accessibility_permission(&self) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn request_screen_capture_permission(&self) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn screenshot_display(
            &self,
            _params: ComputerUseScreenshotParams,
        ) -> OpenBitFunResult<ComputerScreenshot> {
            not_expected()
        }
        fn map_image_coords_to_pointer(&self, _x: i32, _y: i32) -> OpenBitFunResult<(i32, i32)> {
            not_expected()
        }
        fn map_normalized_coords_to_pointer(
            &self,
            _x: i32,
            _y: i32,
        ) -> OpenBitFunResult<(i32, i32)> {
            not_expected()
        }
        async fn mouse_move(&self, _x: i32, _y: i32) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn pointer_move_relative(&self, _dx: i32, _dy: i32) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn mouse_click(&self, _button: &str) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn scroll(&self, _delta_x: i32, _delta_y: i32) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn key_chord(&self, _keys: Vec<String>) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn type_text(&self, _text: &str) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn wait_ms(&self, _ms: u64) -> OpenBitFunResult<()> {
            not_expected()
        }
        async fn computer_use_session_snapshot(&self) -> ComputerUseSessionSnapshot {
            ComputerUseSessionSnapshot::default()
        }
        fn computer_use_waive_fresh_capture_guard(&self) {
            self.waived.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn text_only_context(
        host: std::sync::Arc<GuardRecordingHost>,
    ) -> (ToolUseContext, std::sync::Arc<GuardRecordingHost>) {
        let mut context = ToolUseContext::for_tool_listing(None, None);
        context.primary_model_facts =
            tool_runtime::context::PrimaryModelFacts::new("m", "m", "anthropic", false);
        context.computer_use_host = Some(host.clone());
        (context, host)
    }

    /// A text-only `screenshot` captures nothing, so it can never clear the
    /// stale-capture guard through the normal path — yet the guard's own error
    /// tells the model to "call `screenshot` first". Left as it was, that is a
    /// closed loop: every `click` and Enter `key_chord` stays refused for the
    /// rest of the session, and the only way out is to bypass the tool entirely
    /// (the observed failure was an agent falling back to raw
    /// `osascript … keystroke return`, which skips every safety check the guard
    /// exists to enforce).
    #[tokio::test]
    async fn text_only_screenshot_waives_the_unsatisfiable_capture_guard() {
        let (context, host) = text_only_context(std::sync::Arc::new(GuardRecordingHost::default()));
        let results = ComputerUseTool::new()
            .call_impl(&json!({ "action": "screenshot" }), &context)
            .await
            .expect("text-only screenshot returns a soft envelope");
        assert!(
            host.waived.load(std::sync::atomic::Ordering::SeqCst),
            "text-only screenshot must waive the guard it can never satisfy"
        );
        let body = results[0].content();
        assert_eq!(
            body.get("stale_capture_guard").and_then(Value::as_str),
            Some("waived"),
            "the waiver must be visible to the model: {body}"
        );
        // The guard's own error text says "call `screenshot` first"; the
        // instruction here has to say that path is now open, or the model has
        // no reason to believe retrying the click will work.
        let instruction = body
            .get("instruction")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            instruction.contains("waived"),
            "instruction must tell the model the guard is cleared: {instruction}"
        );
    }

    /// `describe_screen` is the text-only equivalent of taking a look, so it
    /// clears the same guard a capture would.
    #[tokio::test]
    async fn text_only_describe_screen_waives_the_capture_guard() {
        let (context, host) = text_only_context(std::sync::Arc::new(GuardRecordingHost::default()));
        let _ = ComputerUseTool::new()
            .call_impl(&json!({ "action": "describe_screen" }), &context)
            .await
            .expect("describe_screen should succeed");
        assert!(
            host.waived.load(std::sync::atomic::Ordering::SeqCst),
            "describe_screen is the text-only observation step and must waive the guard"
        );
    }

    #[test]
    fn tree_text_under_the_cap_is_returned_verbatim() {
        let small = "[0] AXApplication\n  [1] AXWindow\n".to_string();
        assert_eq!(
            clip_tree_text(small.clone(), DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES),
            small
        );
    }

    /// Both caps must actually bound the payload. `get_app_state` is allowed a
    /// larger tree than a routine `describe_screen` because it is an explicit
    /// request for one, but "larger" is not "unbounded" — an uncapped
    /// `get_app_state` measured 390 KB on a real Electron app, roughly 100k
    /// tokens for a single look.
    #[test]
    fn both_ax_tree_caps_bound_the_payload() {
        let line = "[0] AXButton title=\"x\" frame=(0,0,10x10)\n";
        let huge = line.repeat(APP_STATE_TREE_TEXT_MAX_BYTES / line.len() + 5_000);
        for cap in [
            DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES,
            APP_STATE_TREE_TEXT_MAX_BYTES,
        ] {
            let out = clip_tree_text(huge.clone(), cap);
            assert!(out.contains("[truncated]"), "cap={cap}");
            let body = out.split("\n[truncated]").next().unwrap();
            assert!(
                body.len() <= cap,
                "cap={cap} but kept {} bytes of tree",
                body.len()
            );
        }
    }

    /// Truncation must announce itself. An agent that reads a clipped tree as
    /// the whole UI concludes a control does not exist and gives up on it.
    #[test]
    fn oversized_tree_text_is_clipped_on_a_line_boundary_and_says_so() {
        let line = "[0] AXButton title=\"x\" frame=(0,0,10x10)\n";
        let big = line.repeat(DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES / line.len() + 500);
        let out = clip_tree_text(big.clone(), DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES);

        assert!(out.len() < big.len(), "must actually shrink");
        assert!(
            out.contains("[truncated]"),
            "must announce the clip: {out:.200}"
        );
        assert!(
            out.contains("not the end of the UI"),
            "must warn that a missing control may still exist"
        );
        // Cutting mid-line would hand the model a malformed node.
        let body = out.split("\n[truncated]").next().unwrap();
        assert!(
            body.lines()
                .all(|l| l.is_empty() || l.starts_with("[0] AXButton")),
            "clip must land on a line boundary"
        );
    }

    /// The cap is a byte count but the tree is a `str`, so the clip has to land
    /// on a char boundary. A CJK app — exactly the kind whose tree gets large —
    /// would otherwise panic the whole tool call on a mid-character slice.
    #[test]
    fn oversized_cjk_tree_text_clips_without_panicking() {
        for label in ["范明裕", "飞书 · 消息", "🙂 emoji", "混合 mixed 内容"] {
            let line = format!("[0] AXStaticText title=\"{label}\"\n");
            let big = line.repeat(DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES / line.len() + 500);
            assert!(big.len() > DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES);

            let out = clip_tree_text(big.clone(), DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES);
            assert!(out.contains("[truncated]"), "must announce the clip");
            assert!(out.len() < big.len(), "must actually shrink");
        }
    }

    /// The cut offset must be safe for *every* alignment, not the one a given
    /// repeated line happens to produce.
    ///
    /// Shifting the content by one and two bytes is what makes this bite: a
    /// 3-byte character misaligns against the byte cap at two of every three
    /// offsets, and only those two panic. An unshifted string of `范` lands
    /// exactly on 60_000 and sails through a completely broken implementation —
    /// which is how the first version of this test passed without the fix.
    #[test]
    fn clip_lands_on_a_char_boundary_at_every_alignment() {
        for pad in 0..3 {
            // No newline anywhere, so the cut falls back to the boundary walk
            // rather than being rescued by `rfind('\n')`.
            let mut s = "a".repeat(pad);
            s.push_str(&"范".repeat(DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES / 3 + 10));
            assert!(s.len() > DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES);

            let out = clip_tree_text(s.clone(), DESCRIBE_SCREEN_TREE_TEXT_MAX_BYTES);
            assert!(out.contains("[truncated]"), "pad={pad}");
            let body = out.split("\n[truncated]").next().unwrap();
            assert!(
                body.chars().all(|c| c == 'a' || c == '范'),
                "clip split a character at pad={pad}"
            );
        }
    }

    /// An empty snapshot must say *why* it is empty. A bare `ax_tree_text:
    /// null` reads as truncated tool output, and an agent that believes its
    /// results are being cut off re-issues the same call instead of changing
    /// tactic.
    #[tokio::test]
    async fn describe_screen_explains_an_empty_ax_tree_instead_of_returning_bare_nulls() {
        let (context, _host) =
            text_only_context(std::sync::Arc::new(GuardRecordingHost::default()));
        let results = ComputerUseTool::new()
            .call_impl(&json!({ "action": "describe_screen" }), &context)
            .await
            .expect("describe_screen should succeed");
        let body = results[0].content();
        let data = body.get("data").unwrap_or(&body);
        assert_eq!(
            data.get("ax_tree_status").and_then(Value::as_str),
            Some("no_foreground_app"),
            "status must name the reason the tree is empty: {body}"
        );
        assert_eq!(
            data.get("output_is_complete").and_then(Value::as_bool),
            Some(true),
            "result must assert it is not truncated: {body}"
        );
        let note = data
            .get("ax_tree_note")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            note.contains("list_apps") || note.contains("open_app"),
            "note must offer a concrete next action: {note}"
        );
    }

    /// The browser-boundary guard must be reachable from `call_impl`: a
    /// physical input action while a Chromium-family browser is frontmost is
    /// rejected with the ControlHub browser-domain redirect instead of
    /// clicking into the page.
    #[tokio::test]
    async fn click_is_rejected_while_chromium_browser_is_frontmost() {
        let mut context = ToolUseContext::for_tool_listing(None, None);
        context.computer_use_host = Some(std::sync::Arc::new(ChromeForegroundHost));
        let results = ComputerUseTool::new()
            .call_impl(&json!({ "action": "click" }), &context)
            .await
            .expect("guard rejection is a structured envelope, not a hard error");
        let body = results[0].content();
        assert_eq!(
            body.get("ok").and_then(Value::as_bool),
            Some(false),
            "guarded click should return an error envelope: {body}"
        );
        let error_text = body.get("error").map(Value::to_string).unwrap_or_default();
        assert!(
            error_text.contains("browser"),
            "guard error should redirect to the ControlHub browser domain: {error_text}"
        );
    }

    /// Renaming the same physical input must not get through the boundary: the
    /// app-scoped and interactive/visual variants are guarded too.
    #[tokio::test]
    async fn app_scoped_input_is_rejected_while_chromium_browser_is_frontmost() {
        let mut context = ToolUseContext::for_tool_listing(None, None);
        context.computer_use_host = Some(std::sync::Arc::new(ChromeForegroundHost));
        for action in [
            "app_click",
            "app_type_text",
            "app_scroll",
            "app_key_chord",
            "interactive_click",
            "interactive_type_text",
            "interactive_scroll",
            "visual_click",
        ] {
            let results = ComputerUseTool::new()
                .call_impl(&json!({ "action": action }), &context)
                .await
                .expect("guard rejection is a structured envelope");
            assert_eq!(
                results[0].content().get("ok").and_then(Value::as_bool),
                Some(false),
                "`{action}` must be guarded"
            );
        }
    }

    /// An explicit browser selector is rejected on its own evidence, without
    /// asking the host what is frontmost.
    #[tokio::test]
    async fn app_selector_naming_chromium_is_rejected_without_a_foreground_signal() {
        let context = ToolUseContext::for_tool_listing(None, None);
        let results = ComputerUseTool::new()
            .call_impl(
                &json!({
                    "action": "app_click",
                    "app": { "name": "Google Chrome" },
                    "target": { "node_idx": 12 }
                }),
                &context,
            )
            .await
            .expect("guard rejection is a structured envelope");
        assert_eq!(
            results[0].content().get("ok").and_then(Value::as_bool),
            Some(false)
        );
    }

    /// The guard is positional, not task-related: a task whose target is not
    /// the browser must keep a way to reach it while the browser is frontmost.
    /// Both escape routes must therefore pass the guard untouched.
    #[tokio::test]
    async fn guard_leaves_an_escape_route_for_non_browser_targets() {
        let mut context = ToolUseContext::for_tool_listing(None, None);
        context.computer_use_host = Some(std::sync::Arc::new(ChromeForegroundHost));
        let actions = super::super::computer_use_actions::ComputerUseActions::new();
        for input in [
            // App switcher: the only keyboard way off a browser window.
            json!({ "action": "key_chord", "keys": ["command", "tab"] }),
            json!({ "action": "key_chord", "keys": ["alt", "tab"] }),
            // App-scoped input aimed at a different app.
            json!({ "action": "app_type_text", "app": { "name": "WeChat" }, "text": "hi" }),
        ] {
            let action = input.get("action").and_then(Value::as_str).expect("action");
            assert!(
                actions
                    .desktop_action_targets_browser(action, &input, &context)
                    .await
                    .is_none(),
                "{input} must not be guarded"
            );
        }
        // A normal chord in the browser is still rejected.
        assert!(actions
            .desktop_action_targets_browser(
                "key_chord",
                &json!({ "action": "key_chord", "keys": ["command", "t"] }),
                &context
            )
            .await
            .is_some());
    }

    /// The `action` enum, description, and a handful of other fields are
    /// deliberately different (richer guidance) between the two schemas. This
    /// test documents that the shared/override split does not silently
    /// collapse them into one shared copy.
    #[test]
    fn capability_specific_fields_may_differ_between_schemas() {
        let full = ComputerUseTool::new().input_schema();
        let text_only = ComputerUseTool::input_schema_text_only();
        assert_ne!(
            full.get("properties").and_then(|p| p.get("action")),
            text_only.get("properties").and_then(|p| p.get("action")),
            "`action` is expected to differ (screenshot presence, tailored guidance)"
        );
    }
}
