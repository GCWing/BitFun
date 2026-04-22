//! Desktop automation (Computer use).

use super::computer_use_input::{
    coordinate_mode, ensure_pointer_move_uses_screen_coordinates_only, parse_screenshot_params,
    use_screen_coordinates,
};
use super::computer_use_locate::execute_computer_use_locate;
use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::computer_use_host::{
    ComputerScreenshot, ComputerUseHost, ComputerUseNavigateQuadrant,
    ComputerUseScreenshotRefinement, OcrRegionNative, ScreenshotCropCenter, UiElementLocateQuery,
    COMPUTER_USE_POINT_CROP_HALF_MAX, COMPUTER_USE_POINT_CROP_HALF_MIN,
    COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE, COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
};
use crate::agentic::tools::computer_use_optimizer::hash_screenshot_bytes;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::ToolImageAttachment;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use log::{debug, warn};
use serde_json::{json, Value};

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

        // Add loop detection warning if a loop is detected
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
/// Filenames: `cu_<ms>_full.jpg` (whole display) or `cu_<ms>_crop_<x>_<y>.jpg` when a point crop was requested.
const COMPUTER_USE_DEBUG_SUBDIR: &str = ".bitfun/computer_use_debug";

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
**ACTION PRIORITY (CRITICAL):** Always think in this order:\n\
1. **Terminal/CLI/System commands first** — Use Bash tool for terminal commands, system scripts (e.g., macOS `osascript`), shell automation. Most efficient.\n\
2. **Keyboard shortcuts second** — Use **`key_chord`** / **`type_text`** for system/app shortcuts, navigation keys.\n\
3. **Precise UI control last** — Only when above fail: **`click_target`** / **`move_to_target`** (AX → OCR → screen coords in one call) → lower-level **`click_element`** / **`move_to_text`** → **`mouse_move`** + **`click`**.\n\
**Rhythm:** one action at a time; use **`wait`** when UI animates. Observe **`interaction_state`** and **`computer_use_context`** in tool JSON.\n\
**`click_target` / `move_to_target`:** Unified resolver: AX filters or `target_text` first, OCR second, explicit global x/y last. **`click_element` / `locate`:** Accessibility (AX/UIA/AT-SPI). **`move_to_text`:** OCR match + move pointer only. **`click`:** at current pointer only — use **`mouse_move`** or **`move_to_text`** / **`click_element`** first.\n\
**`mouse_move` / `drag`:** **`use_screen_coordinates`: true** with globals from tools. **`pointer_move_rel`:** relative nudge; host may block right after certain flows — follow tool errors.\n\
**`key_chord` / `type_text` / `scroll` / `wait`:** standard desktop automation without any screenshot step.\n",
            os, keys
        )
    }

    /// JSON Schema without `screenshot` or screenshot-only fields.
    fn input_schema_text_only() -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["click_target", "move_to_target", "click_element", "move_to_text", "click", "mouse_move", "scroll", "drag", "locate", "key_chord", "type_text", "pointer_move_rel", "wait", "open_app", "run_apple_script"],
                    "description": "The action to perform. **Primary model is text-only — no `screenshot`.** **ACTION PRIORITY:** 1) Use Bash tool for CLI/terminal/system commands first. 2) **`open_app`** to launch apps. **`run_apple_script`** for AppleScript (macOS). 3) Prefer `key_chord` for shortcuts/navigation. 4) Only when above fail: `click_target` / `move_to_target` (AX → OCR → screen coords in one call), then lower-level `click_element`, `move_to_text`, or `mouse_move` + `click`. Never guess coordinates."
                },
                "x": { "type": "integer", "description": "For `mouse_move` and `drag`: X in **global display** units when **`use_screen_coordinates`: true** (required). **Not** for `click`." },
                "y": { "type": "integer", "description": "For `mouse_move` and `drag`: Y in **global display** units when **`use_screen_coordinates`: true** (required). **Not** for `click`." },
                "coordinate_mode": { "type": "string", "enum": ["image", "normalized"], "description": "Ignored for `mouse_move` / `drag` — host rejects image/normalized positioning; always set **`use_screen_coordinates`: true**." },
                "use_screen_coordinates": { "type": "boolean", "description": "For `mouse_move`, `drag`: **must be true** — global display coordinates from `move_to_text`, `locate`, AX, or `pointer_global`. **Not** for `click`." },
                "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "For `click`, `click_element`, `drag`: mouse button (default left)." },
                "num_clicks": { "type": "integer", "minimum": 1, "maximum": 3, "description": "For `click`, `click_element`: 1=single (default), 2=double, 3=triple click." },
                "delta_x": { "type": "integer", "description": "For `pointer_move_rel`: horizontal delta (negative=left); also accepted as `dx`. For `scroll`: horizontal wheel delta." },
                "delta_y": { "type": "integer", "description": "For `pointer_move_rel`: vertical delta (negative=up); also accepted as `dy`. For `scroll`: vertical wheel delta." },
                "start_x": { "type": "integer", "description": "For `drag`: start X coordinate." },
                "start_y": { "type": "integer", "description": "For `drag`: start Y coordinate." },
                "end_x": { "type": "integer", "description": "For `drag`: end X coordinate." },
                "end_y": { "type": "integer", "description": "For `drag`: end Y coordinate." },
                "keys": { "type": "array", "items": { "type": "string" }, "description": "For `key_chord`: keys in order — modifiers first, then the main key. Desktop host waits after pressing modifiers so shortcuts register (important on macOS with IME)." },
                "text": { "type": "string", "description": "For `type_text`: text to type. Prefer clipboard paste (key_chord) for long content." },
                "ms": { "type": "integer", "description": "For `wait`: duration in milliseconds." },
                "target_text": { "type": "string", "description": "For `move_to_target` / `click_target`: visible or accessible text. The resolver tries AX first, then OCR." },
                "target_match_index": { "type": "integer", "minimum": 1, "description": "For `move_to_target` / `click_target`: optional 1-based OCR match index when you want a specific candidate." },
                "text_query": { "type": "string", "description": "For `move_to_text`, `move_to_target`, `click_target`: visible text to OCR-match on screen (case-insensitive substring)." },
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
                "identifier_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring on AXIdentifier." },
                "text_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring matched against ANY of AXTitle / AXValue / AXDescription / AXHelp. Prefer this when the visible text is shown via value/description (e.g. AXStaticText cards) instead of title." },
                "node_idx": { "type": "integer", "minimum": 0, "description": "For `locate`, `click_element`: jump straight to a node returned by the most recent `desktop.get_app_state` (field `idx`). Bypasses BFS. macOS only; other platforms return AX_IDX_NOT_SUPPORTED." },
                "app_state_digest": { "type": "string", "description": "For `locate`, `click_element`: optional `state_digest` from the same `get_app_state` call that produced `node_idx`. Stale digest yields AX_IDX_STALE so you re-snapshot." },
                "max_depth": { "type": "integer", "minimum": 1, "maximum": 200, "description": "For `locate`, `click_element`: max BFS depth (default 48). Ignored when `node_idx` is supplied." },
                "filter_combine": { "type": "string", "enum": ["all", "any"], "description": "For `locate`, `click_element`: `all` (default, AND) or `any` (OR) for filter combination. Priority: `node_idx` > `text_contains` > `title_contains`+`role_substring`." },
                "app_name": { "type": "string", "description": "For `open_app`: the application name to launch." },
                "script": { "type": "string", "description": "For `run_apple_script`: the AppleScript code to execute. macOS only." },
                "scroll_x": { "type": "integer", "description": "For `scroll`: optional global X coordinate to scroll at. Use with `scroll_y`." },
                "scroll_y": { "type": "integer", "description": "For `scroll`: optional global Y coordinate to scroll at. Use with `scroll_x`." }
            },
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
    ) -> BitFunResult<Vec<ToolResult>> {
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
    ) -> BitFunResult<Vec<ToolResult>> {
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

    fn primary_api_format(ctx: &ToolUseContext) -> String {
        ctx.custom_data
            .get("primary_model_provider")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase()
    }

    /// Screenshot tool results attach JPEGs via `tool_image_attachments`; only providers whose
    /// request converters emit multimodal tool output are supported (Anthropic + OpenAI-compatible).
    fn require_multimodal_tool_output_for_screenshot(ctx: &ToolUseContext) -> BitFunResult<()> {
        if !ctx.primary_model_supports_image_understanding() {
            return Err(BitFunError::tool(
                "The primary model does not accept images; do not use ComputerUse action `screenshot` or other image-producing steps. Use `click_element`, `locate`, `move_to_text` (with `move_to_text_match_index` when listed), `mouse_move` with globals from tool JSON, `key_chord`, etc.".to_string(),
            ));
        }
        let f = Self::primary_api_format(ctx);
        if matches!(
            f.as_str(),
            "anthropic" | "openai" | "response" | "responses"
        ) {
            return Ok(());
        }
        Err(BitFunError::tool(
            "Screenshot results include images in tool results; set the primary model to Anthropic (Claude) or OpenAI-compatible API format. Other providers are not supported for screenshots yet.".to_string(),
        ))
    }

    fn resolve_xy_f64(
        host: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        input: &Value,
        x: i32,
        y: i32,
    ) -> BitFunResult<(f64, f64)> {
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
    fn ensure_click_has_no_coordinate_fields(input: &Value) -> BitFunResult<()> {
        if input.get("x").is_some() || input.get("y").is_some() {
            return Err(BitFunError::tool(
                "click does not accept x or y. Position with move_to_text, click_element, or `mouse_move` with use_screen_coordinates: true (globals from tool results), then `click` with only button and num_clicks.".to_string(),
            ));
        }
        if input.get("coordinate_mode").is_some() {
            return Err(BitFunError::tool(
                "click does not accept coordinate_mode. Use `mouse_move` with use_screen_coordinates: true, then `click`.".to_string(),
            ));
        }
        if input.get("use_screen_coordinates").is_some() {
            return Err(BitFunError::tool(
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
            _ => "Match key_chord modifiers to the host OS in the system prompt Environment Information. Prefer standard clipboard chords (select all, copy, cut, paste) before long type_text.",
        }
    }

    async fn find_text_on_screen(
        host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        text_query: &str,
        region_native: Option<crate::agentic::tools::computer_use_host::OcrRegionNative>,
    ) -> BitFunResult<Vec<ScreenOcrTextMatch>> {
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
    ) -> BitFunResult<ResolvedDesktopTarget> {
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
                        return Err(BitFunError::tool(format!(
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

        Err(BitFunError::tool(
            "move_to_target/click_target requires a target: node_idx, target_text/text_query/text_contains/title_contains, role_substring, identifier_contains, or x/y with use_screen_coordinates: true.".to_string(),
        ))
    }

    /// Writes the exact JPEG sent to the model (including pointer overlay) under the workspace for debugging.
    async fn try_save_screenshot_for_debug(
        bytes: &[u8],
        context: &ToolUseContext,
        crop: Option<ScreenshotCropCenter>,
        nav_label: Option<&str>,
    ) -> Option<String> {
        let root = context.workspace_root()?;
        let dir = root.join(COMPUTER_USE_DEBUG_SUBDIR);
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
        Some(format!(
            "{}/{}",
            COMPUTER_USE_DEBUG_SUBDIR.replace('\\', "/"),
            fname
        ))
    }

    /// Build tool JSON + one JPEG attachment + assistant hint from an already-captured [`ComputerScreenshot`].
    async fn pack_screenshot_tool_output(
        shot: &ComputerScreenshot,
        debug_rel: Option<String>,
    ) -> BitFunResult<(Value, ToolImageAttachment, String)> {
        let b64 = B64.encode(&shot.bytes);
        let pointer_marker_note = match (shot.pointer_image_x, shot.pointer_image_y) {
            (Some(_), Some(_)) => "The JPEG includes a **synthetic red cursor with gray border** marking the **actual mouse position** on this bitmap (not the OS arrow). The **tip** is the true hotspot for **visual confirmation** only — **do not** use JPEG pixel indices for `mouse_move`; use `use_screen_coordinates: true` with globals from tool results (`pointer_global`, `move_to_text` global_center_*, `locate`, AX) or `move_to_text` / `click_element`.",
            _ => "No pointer overlay in this JPEG (pointer_image_x/y null): the cursor is not on this bitmap (e.g. another display). Do not infer position from the image; use global coordinates with `use_screen_coordinates: true`, or move the pointer onto this display and screenshot again.",
        };
        let mut data = json!({
            "success": true,
            "mime_type": shot.mime_type,
            "image_width": shot.image_width,
            "image_height": shot.image_height,
            "display_width_px": shot.image_width,
            "display_height_px": shot.image_height,
            "native_width": shot.native_width,
            "native_height": shot.native_height,
            "display_origin_x": shot.display_origin_x,
            "display_origin_y": shot.display_origin_y,
            "vision_scale": shot.vision_scale,
            "pointer_image_x": shot.pointer_image_x,
            "pointer_image_y": shot.pointer_image_y,
            "pointer_marker": pointer_marker_note,
            "screenshot_crop_center": shot.screenshot_crop_center,
            "point_crop_half_extent_native": shot.point_crop_half_extent_native,
            "navigation_native_rect": shot.navigation_native_rect,
            "quadrant_navigation_click_ready": shot.quadrant_navigation_click_ready,
            "image_content_rect": shot.image_content_rect,
            "image_global_bounds": shot.image_global_bounds,
            "implicit_confirmation_crop_applied": shot.implicit_confirmation_crop_applied,
            "debug_screenshot_path": debug_rel,
            "ui_tree_text": shot.ui_tree_text,
        });
        let shortcut_policy = format!(
            "**Verify step:** after **`click`**, **`key_chord`**, **`type_text`**, **`scroll`**, or **`drag`**, check **`interaction_state.recommend_screenshot_to_verify_last_action`** — when true, call **`screenshot`** next to confirm UI state (Cowork-style). \
**Targeting priority:** `click_element` → **`move_to_text`** (OCR + move; no prior `screenshot` for targeting) → **`screenshot`** (confirm / drill) + **`mouse_move`** (**`use_screen_coordinates`: true only**) + **`click`** last. **Screenshots are for confirmation and navigation — do not guess move targets from JPEG pixels.** **`click`** never moves the pointer. **Host-only mandatory screenshot:** before **`click`** or Enter **`key_chord`** when the pointer changed since the last capture — **not** before `mouse_move`, `scroll`, `type_text`, `locate`, `wait`, or non-Enter `key_chord`. **Valid basis for a guarded `click`:** `FullDisplay`, `quadrant_navigation_click_ready`, or point crop; or bare **`screenshot`** after a pointer-changing action (**~500×500** implicit confirmation around mouse/caret). **`mouse_move`** must use **global** coordinates (from `move_to_text` global_center_*, `locate`, AX, or `pointer_global`). **Bare confirmation `screenshot`:** whenever the host still requires a capture before **`click`** or Enter **`key_chord`** (`requires_fresh_screenshot_*`), a bare `screenshot` (no crop / no reset) is **~500×500** centered on **mouse** (`screenshot_implicit_center` default `mouse`) — **including during quadrant drill** and the **first** such capture in a session. Before Enter in a text field, set **`screenshot_implicit_center`: `text_caret`**. Use **`screenshot_reset_navigation`**: true for a **full-screen** capture instead. **If AX failed:** try **`move_to_text`** before a long screenshot drill. **Optional refinement** for tiny targets: `screenshot_navigate_quadrant` until `quadrant_navigation_click_ready` (long edge < {} px) or point crop. Small moves: **ComputerUseMouseStep** over tiny **ComputerUseMousePrecise** (screen globals only).",
            COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE
        );
        let region_crop_size_note = shot
            .point_crop_half_extent_native
            .map(|h| {
                let edge = h.saturating_mul(2);
                format!(
                    "Crop frame (~{}×{} native, half-extent {} px; clamped {}..{}): ",
                    edge,
                    edge,
                    h,
                    COMPUTER_USE_POINT_CROP_HALF_MIN,
                    COMPUTER_USE_POINT_CROP_HALF_MAX
                )
            })
            .unwrap_or_else(|| "Crop frame (~500×500 native, half-extent 250 px): ".to_string());
        let hierarchical_navigation = if shot.screenshot_crop_center.is_some() {
            json!({
                "phase": "region_crop",
                "image_is_crop_only": true,
                "shortcut_policy": shortcut_policy,
                "instruction": format!(
                    "{}**Image pixel (0,0)** is the **top-left of this crop** in **full-capture native** space (same whole-screen bitmap as a full-screen shot — not local 0..crop only). This view is for **confirmation / drill** — do **not** use JPEG pixels for `mouse_move`. For another view, call screenshot with new `screenshot_crop_center_*` in that same full-capture space; optional `screenshot_crop_half_extent_native` adjusts crop size. See shortcut_policy.",
                    region_crop_size_note
                )
            })
        } else if shot.quadrant_navigation_click_ready {
            json!({
                "phase": "quadrant_terminal",
                "image_is_crop_only": true,
                "shortcut_policy": shortcut_policy,
                "instruction": "Region is small enough for precise pointer: **`quadrant_navigation_click_ready`** is true. **Do not** use **`ComputerUseMouseStep`** / **`pointer_move_rel`** immediately after a **`screenshot`** (host blocks — vision nudges are wrong). First **`move_to_text`**, **`mouse_move`** (`use_screen_coordinates`: true), or **`click_element`**, then optional **`ComputerUseMouseStep`** / **`ComputerUseMousePrecise`**. Then **`ComputerUseMouseClick`** (`action`: click). Host requires a **fresh** screenshot before the next **`click`** or Enter **`key_chord`** if pointer state changed since last capture (see shortcut_policy)."
            })
        } else if !Self::shot_covers_full_display(shot) {
            json!({
                "phase": "quadrant_drill",
                "image_is_crop_only": true,
                "shortcut_policy": shortcut_policy,
                "instruction": format!(
                    "**Keep drilling (default):** call **`screenshot`** again with **`screenshot_navigate_quadrant`**: `top_left` | `top_right` | `bottom_left` | `bottom_right` — pick the tile that contains your target. The host expands the chosen quadrant by **{} px** on each side (clamped) so split-edge controls stay in-frame. Repeat until `quadrant_navigation_click_ready`. To restart from the full display, set **`screenshot_reset_navigation`**: true on the next screenshot. Coordinates remain **full-display native**. See shortcut_policy.",
                    COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX
                )
            })
        } else {
            json!({
                "phase": "full_display",
                "image_is_crop_only": false,
                "host_auto_quadrant": false,
                "next_step_for_mouse_click": "**First:** **`move_to_text`** if visible text can name the target (OCR + move pointer; then **`click`** if you need a press). **If you must move by globals:** **`mouse_move`** with **`use_screen_coordinates`: true** and coordinates from **`locate`**, **`move_to_text`**, or **`pointer_global`** — **not** from guessing JPEG pixels. Then **`click`** when the host allows (`interaction_state.click_ready`). **Optional refinement:** `screenshot_crop_center_*`, quadrant drill, or **`screenshot_navigate_quadrant`** for smaller targets. Host never splits the screen unless you pass `screenshot_navigate_quadrant`.",
                "shortcut_policy": shortcut_policy,
                "instruction": "Full frame: JPEG aligns with **full-display native** space for **visual confirmation** only. **Prefer `move_to_text`** when readable text exists (then **`click`**). **Do not** derive `mouse_move` targets from this bitmap — use **`use_screen_coordinates`: true** with globals from tools, or AX/OCR actions. Then **`click`** when host allows (`click_ready`). For tiny targets, optionally narrow with `screenshot_crop_center_*` or quadrant drill. **`screenshot`**-heavy paths are **last** for targeting. See `next_step_for_mouse_click`, `recommended_next_for_click_targeting`, shortcut_policy."
            })
        };
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "hierarchical_navigation".to_string(),
                hierarchical_navigation,
            );
            if shot.screenshot_crop_center.is_none() && !shot.quadrant_navigation_click_ready {
                if Self::shot_covers_full_display(shot) {
                    obj.insert(
                        "recommended_next_for_click_targeting".to_string(),
                        Value::String(
                            "move_to_text_then_click_or_mouse_move_screen_globals_then_click"
                                .to_string(),
                        ),
                    );
                } else {
                    let rec = format!(
                        "move_to_text_first_then_{}",
                        "screenshot_navigate_quadrant_until_click_ready"
                    );
                    obj.insert(
                        "recommended_next_for_click_targeting".to_string(),
                        Value::String(rec),
                    );
                }
            }
        }
        let attach = ToolImageAttachment {
            mime_type: shot.mime_type.clone(),
            data_base64: b64,
        };
        let pointer_line = match (shot.pointer_image_x, shot.pointer_image_y) {
            (Some(px), Some(py)) => format!(
                " TRUE POINTER: **red cursor with gray border** (tip = hotspot) in the JPEG at image x={}, y={} — **confirmation only**; use **`mouse_move`** with **`use_screen_coordinates`: true** using globals from tool JSON (`pointer_global`, `move_to_text`, `locate`), then **`click`**. **Do not** use **`pointer_move_rel`** / **ComputerUseMouseStep** as the next action after this **`screenshot`** (host blocks). Prior screenshot is stale after **ComputerUseMousePrecise** / **ComputerUseMouseStep** / `pointer_move_rel` until you screenshot again.",
                px, py
            ),
            _ => " TRUE POINTER: not on this capture (pointer_image_x/y null). No red synthetic cursor — OS mouse may be on another display; use use_screen_coordinates with global coords or bring the pointer here and re-screenshot."
                .to_string(),
        };
        let debug_line = debug_rel
            .as_ref()
            .map(|p| {
                format!(
                    " Same JPEG saved under workspace: {} (verify red cursor tip vs pointer_image_*).",
                    p
                )
            })
            .unwrap_or_default();
        let hint = if let Some(c) = shot.screenshot_crop_center {
            format!(
                "Region crop screenshot {}x{} around full-display native center ({}, {}). **Confirm** UI state here — do **not** use JPEG pixels for `mouse_move`.{}.{} After pointer moves, screenshot again before click (host).",
                shot.image_width,
                shot.image_height,
                c.x,
                c.y,
                pointer_line,
                debug_line
            )
        } else if shot.quadrant_navigation_click_ready {
            format!(
                "Quadrant terminal {}x{} (native region {:?}). **`quadrant_navigation_click_ready`**: align with **ComputerUseMouseStep** / **`mouse_move`** (**`use_screen_coordinates`: true** only) / **ComputerUseMousePrecise**, then **`ComputerUseMouseClick`** (`action`: click) — **`click`** has no coordinates.{}.{}",
                shot.image_width,
                shot.image_height,
                shot.navigation_native_rect,
                pointer_line,
                debug_line
            )
        } else if !Self::shot_covers_full_display(shot) {
            format!(
                "Quadrant drill view {}x{} (native region {:?}). Call **`screenshot`** with **`screenshot_navigate_quadrant`** to subdivide, or **`screenshot_reset_navigation`**: true for full screen.{}.{}",
                shot.image_width,
                shot.image_height,
                shot.navigation_native_rect,
                pointer_line,
                debug_line
            )
        } else {
            let nx = shot.native_width.saturating_sub(1);
            let ny = shot.native_height.saturating_sub(1);
            format!(
                "Full screenshot {}x{} (vision_scale={}). **Display native** range **0..={}** x **0..={}** (JPEG matches this rect for **confirmation**). **Targeting:** prefer **`move_to_text`** when text is visible; **`screenshot` + quad** is lowest priority. **`mouse_move`** uses **`use_screen_coordinates`: true** with globals from tools — **not** JPEG guesses; then **`click`** when allowed (see `interaction_state`). **Only** guarded **`click`** / Enter **`key_chord`** need a fresh capture after pointer moves (see shortcut_policy).{}.{}",
                shot.image_width,
                shot.image_height,
                shot.vision_scale,
                nx,
                ny,
                pointer_line,
                debug_line
            )
        };
        Ok((data, attach, hint))
    }

    fn shot_covers_full_display(shot: &ComputerScreenshot) -> bool {
        if shot.screenshot_crop_center.is_some() {
            return false;
        }
        match shot.navigation_native_rect {
            None => true,
            Some(n) => {
                n.x0 == 0
                    && n.y0 == 0
                    && n.width == shot.native_width
                    && n.height == shot.native_height
            }
        }
    }
}

/// JSON for `snapshot_coordinate_basis` in mouse tool results (last screenshot refinement).
fn computer_use_snapshot_coordinate_basis(
    host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
) -> serde_json::Value {
    let last_ref = host_ref.last_screenshot_refinement();
    match last_ref {
        None => serde_json::Value::Null,
        Some(ComputerUseScreenshotRefinement::FullDisplay) => json!("full_display"),
        Some(ComputerUseScreenshotRefinement::RegionAroundPoint { center_x, center_y }) => {
            json!({
                "region_crop_center_full_display_native": { "x": center_x, "y": center_y }
            })
        }
        Some(ComputerUseScreenshotRefinement::QuadrantNavigation {
            x0,
            y0,
            width,
            height,
            click_ready,
        }) => {
            json!({
                "quadrant_native_rect": { "x0": x0, "y0": y0, "w": width, "h": height },
                "quadrant_navigation_click_ready": click_ready,
            })
        }
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
) -> BitFunResult<()> {
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
    Err(BitFunError::tool(format!(
        "[DESKTOP_COORD_OUT_OF_DISPLAY] global=({:.1},{:.1}) does not lie on any visible display. \
         Visible displays: [{}]. Hint: image-pixel coordinates are NOT screen coordinates. \
         Use screenshot.pointer_global, click_element/locate result.global_center_x/y, or move_to_text. \
         To convert image→global, use the screenshot's display_id + scale_factor.",
        gx,
        gy,
        bounds.join("; ")
    )))
}

/// Absolute pointer move (`ComputerUseMousePrecise` tool).
pub(crate) async fn computer_use_execute_mouse_precise(
    host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
    input: &Value,
) -> BitFunResult<Vec<ToolResult>> {
    ensure_pointer_move_uses_screen_coordinates_only(input)?;
    let snapshot_basis = computer_use_snapshot_coordinate_basis(host_ref);
    let x = req_i32(input, "x")?;
    let y = req_i32(input, "y")?;
    let mode = coordinate_mode(input);
    let use_screen = use_screen_coordinates(input);
    let (sx64, sy64) = ComputerUseTool::resolve_xy_f64(host_ref, input, x, y)?;
    if use_screen {
        ensure_global_xy_on_display(host_ref, sx64, sy64).await?;
    }
    host_ref.mouse_move_global_f64(sx64, sy64).await?;
    let sx = sx64.round() as i32;
    let sy = sy64.round() as i32;
    let input_coords = json!({
        "kind": "mouse_precise",
        "raw": { "x": x, "y": y, "coordinate_mode": mode, "use_screen_coordinates": use_screen },
        "resolved_global": { "x": sx64, "y": sy64 }
    });
    let body = computer_use_augment_result_json(
        host_ref,
        json!({
            "success": true,
            "tool": "ComputerUseMousePrecise",
            "positioning": "absolute",
            "x": x,
            "y": y,
            "pointer_x": sx,
            "pointer_y": sy,
            "coordinate_mode": mode,
            "use_screen_coordinates": use_screen,
            "snapshot_coordinate_basis": snapshot_basis,
        }),
        Some(input_coords),
    )
    .await;
    let summary = format!(
        "Moved pointer to global screen (~{}, ~{}, sub-point on macOS) (input {:?} {}, {}).",
        sx, sy, mode, x, y
    );
    Ok(vec![ToolResult::ok(body, Some(summary))])
}

/// Cardinal step move (`ComputerUseMouseStep` tool). Same pixel space as `pointer_move_rel`.
pub(crate) async fn computer_use_execute_mouse_step(
    host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
    input: &Value,
) -> BitFunResult<Vec<ToolResult>> {
    let dir = input
        .get("direction")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            BitFunError::tool(
                "direction is required for ComputerUseMouseStep (up|down|left|right)".to_string(),
            )
        })?;
    let px = input
        .get("pixels")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or(32)
        .clamp(1, 400);
    let (dx, dy) = match dir.to_lowercase().as_str() {
        "up" => (0, -px),
        "down" => (0, px),
        "left" => (-px, 0),
        "right" => (px, 0),
        _ => {
            return Err(BitFunError::tool(
                "direction must be up, down, left, or right".to_string(),
            ));
        }
    };
    host_ref.pointer_move_relative(dx, dy).await?;
    let input_coords = json!({
        "kind": "mouse_step",
        "direction": dir,
        "pixels": px,
        "delta_x": dx,
        "delta_y": dy
    });
    let body = computer_use_augment_result_json(
        host_ref,
        json!({
            "success": true,
            "tool": "ComputerUseMouseStep",
            "direction": dir,
            "pixels": px,
            "delta_x": dx,
            "delta_y": dy,
        }),
        Some(input_coords),
    )
    .await;
    let summary = format!(
        "Stepped pointer by ({}, {}) px (direction {}, {} px).",
        dx, dy, dir, px
    );
    Ok(vec![ToolResult::ok(body, Some(summary))])
}

/// Click and mouse-wheel at the **current** pointer (`ComputerUseMouseClick` tool).
pub(crate) async fn computer_use_execute_mouse_click_tool(
    host_ref: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
    input: &Value,
) -> BitFunResult<Vec<ToolResult>> {
    let act = input
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BitFunError::tool("action is required (click or wheel)".to_string()))?;
    match act {
        "click" => {
            let button = input
                .get("button")
                .and_then(|v| v.as_str())
                .unwrap_or("left");
            let num_clicks = input
                .get("num_clicks")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                .clamp(1, 3) as u32;
            for _ in 0..num_clicks {
                host_ref.mouse_click(button).await?;
            }
            let click_label = match num_clicks {
                2 => "double",
                3 => "triple",
                _ => "single",
            };
            let input_coords = json!({ "kind": "mouse_click", "action": "click", "button": button, "num_clicks": num_clicks });
            let body = computer_use_augment_result_json(
                host_ref,
                json!({
                    "success": true,
                    "tool": "ComputerUseMouseClick",
                    "action": "click",
                    "button": button,
                    "num_clicks": num_clicks,
                }),
                Some(input_coords),
            )
            .await;
            let summary = format!(
                "{} {} click at current pointer (does not move).",
                button, click_label
            );
            Ok(vec![ToolResult::ok(body, Some(summary))])
        }
        "wheel" => {
            let dx = input.get("delta_x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let dy = input.get("delta_y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            if dx == 0 && dy == 0 {
                return Err(BitFunError::tool(
                    "wheel requires non-zero delta_x and/or delta_y".to_string(),
                ));
            }
            host_ref.scroll(dx, dy).await?;
            let input_coords = json!({
                "kind": "mouse_click",
                "action": "wheel",
                "delta_x": dx,
                "delta_y": dy
            });
            let body = computer_use_augment_result_json(
                host_ref,
                json!({
                    "success": true,
                    "tool": "ComputerUseMouseClick",
                    "action": "wheel",
                    "delta_x": dx,
                    "delta_y": dy,
                }),
                Some(input_coords),
            )
            .await;
            let summary = format!("Mouse wheel at pointer: delta ({}, {}).", dx, dy);
            Ok(vec![ToolResult::ok(body, Some(summary))])
        }
        _ => Err(BitFunError::tool(
            "ComputerUseMouseClick action must be \"click\" or \"wheel\"".to_string(),
        )),
    }
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
) -> BitFunResult<Option<crate::agentic::tools::computer_use_host::OcrRegionNative>> {
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
        BitFunError::tool(
            "ocr_region_native must be an object { x0, y0, width, height } in global native pixels."
                .to_string(),
        )
    })?;
    let x0 = o.get("x0").and_then(|x| x.as_i64()).ok_or_else(|| {
        BitFunError::tool("ocr_region_native.x0 (integer) is required.".to_string())
    })? as i32;
    let y0 = o.get("y0").and_then(|x| x.as_i64()).ok_or_else(|| {
        BitFunError::tool("ocr_region_native.y0 (integer) is required.".to_string())
    })? as i32;
    let width = o.get("width").and_then(|x| x.as_u64()).ok_or_else(|| {
        BitFunError::tool("ocr_region_native.width (positive integer) is required.".to_string())
    })? as u32;
    let height = o.get("height").and_then(|x| x.as_u64()).ok_or_else(|| {
        BitFunError::tool("ocr_region_native.height (positive integer) is required.".to_string())
    })? as u32;
    if width == 0 || height == 0 {
        return Err(BitFunError::tool(
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

    async fn description(&self) -> BitFunResult<String> {
        let os = Self::host_os_label();
        let keys = Self::key_chord_os_hint();
        Ok(format!(
            "Desktop automation (host OS: {}). {} All actions in one tool. Send only parameters that apply to the chosen `action`. \
**ACTION PRIORITY (CRITICAL):** Always think in this order before choosing an action:\n\
1. **Terminal/CLI/System commands first** — Use Bash tool for terminal commands, system scripts (e.g., macOS `osascript`, AppleScript), shell automation. This is the MOST EFFICIENT approach.\n\
2. **Keyboard shortcuts second** — Use **`key_chord`** for system shortcuts, app shortcuts, navigation keys (Enter, Escape, Tab, Space, Arrow keys). Prefer over mouse when equivalent.\n\
3. **Precise UI control last** — Only when above methods fail: prefer **`click_target`** / **`move_to_target`** (AX → OCR → screen coords in one call). Use lower-level **`click_element`**, **`move_to_text`**, or **`mouse_move`** + **`click`** only when you need manual disambiguation.\n\
**Screenshot usage:** **`screenshot`** is ONLY for observing/confirming UI state and extracting text/information — NEVER use screenshot coordinates to control mouse movement. Always use precise methods (AX, OCR, system coordinates) for targeting.\n\
**Cowork-style loop:** **`screenshot`** (observe) → **one** action → **`screenshot`** (verify). Use **`wait`** if UI animates. When **`interaction_state.recommend_screenshot_to_verify_last_action`** is true, call **`screenshot`** next. \
**`click_target` / `move_to_target`:** Unified target resolver. In one call it tries AX (`node_idx`, `text_contains`, `title_contains`, `role_substring`, `identifier_contains`, or `target_text`) first, then OCR (`target_text` / `text_query`), then explicit global `x`/`y` with `use_screen_coordinates: true`. `click_target` moves and clicks authoritatively, avoiding the multi-step locate → move → screenshot → click loop for common targets. \
**`click_element`:** Lower-level Accessibility tree (AX/UIA/AT-SPI) locate + click. Provide `title_contains` / `role_substring` / `identifier_contains`. On macOS, **`TextArea`** and **`TextField`** match both `AXTextArea` and `AXTextField` (many chat apps use TextField for compose). If several text fields match, the host deprioritizes known **search** controls (e.g. WeChat `_SC_SEARCH_FIELD`) and prefers **lower** on-screen fields (composer). Bypasses coordinate screenshot guard. \
**`move_to_text`:** OCR-match visible text (`text_query`) and **move the pointer** to it (no click, no keys); **no prior `screenshot` required for targeting** (host captures **raw** pixels for Vision — no agent screenshot overlays; on macOS defaults to the **frontmost window** unless **`ocr_region_native`** overrides). Matching **strips whitespace** between CJK glyphs and allows **small edit distance** when Vision mis-reads one character. The host **trusts** the resulting globals — **next `click`** does **not** require an extra `screenshot` (same as AX). If **several** hits match, the host returns **preview JPEGs + accessibility** per candidate — pick **`move_to_text_match_index`** (1-based) and call **`move_to_text` again** with the same query/region, or narrow with **`ocr_region_native`**. Use **`click`** afterward if you need a mouse press. Prefer after `click_element` misses when text is visible. \
**`click`:** Press at **current pointer only** — **never** pass `x`, `y`, `coordinate_mode`, or `use_screen_coordinates`. Position first with **`move_to_text`**, **`mouse_move`** (**globals only**), or **`click_element`**. After pointer moves, **`screenshot`** again before the next guarded **`click`** when the host requires it. \
**`mouse_move` / `drag`:** **`use_screen_coordinates`: true** required — global coordinates from **`move_to_text`**, **`locate`**, AX, or **`pointer_global`**; never JPEG pixel guesses. \
**`scroll` / `type_text` / `pointer_move_rel` / `wait` / `locate`:** No mandatory pre-screenshot by themselves. **`pointer_move_rel`** (and **ComputerUseMouseStep**) are **blocked immediately after `screenshot`** until **`move_to_text`**, **`mouse_move`** (globals), or **`click_element`** — do not nudge from the JPEG. \
**`key_chord`:** Press key combination; prefer over **`click`** when shortcuts or **Enter**/**Escape**/**Tab** suffice. **Mandatory fresh screenshot only** when chord includes Return/Enter. \
**`screenshot`:** JPEG for **confirmation** (optional pointer overlay). When the host requires a fresh capture before **`click`** or Enter **`key_chord`**, a bare `screenshot` is **~500×500** around the **mouse** or **caret** (also during quadrant drill). Use **`screenshot_reset_navigation`**: true to force **full-screen** for wide context. \
**`type_text`:** Type text; prefer clipboard for long content. Does **not** move the pointer — **Enter** **`key_chord`** may follow without a mandatory `screenshot` unless you moved the pointer since the last capture. If **`screenshot`** shows the correct chat is already open and the input may be focused, **try `type_text` first** before spending steps on `click_element` / `move_to_text`.",
            os, keys,
        ))
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
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
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["screenshot", "click_target", "move_to_target", "click_element", "move_to_text", "click", "mouse_move", "scroll", "drag", "locate", "key_chord", "type_text", "pointer_move_rel", "wait", "open_app", "run_apple_script"],
                    "description": "The action to perform. **ACTION PRIORITY:** 1) Use Bash tool for CLI/terminal/system commands (most efficient). 2) **`open_app`** to launch apps by name. **`run_apple_script`** to run AppleScript (macOS). 3) Prefer **`key_chord`** for shortcuts/navigation keys over mouse. 4) Only when above fail: `click_target` / `move_to_target` (AX → OCR → screen coords in one call) before lower-level `click_element`, `move_to_text`, or `mouse_move` + `click`. **`screenshot`** is for observation/confirmation ONLY — never derive mouse coordinates from screenshots. `click` = press at **current pointer only** (no x/y params). `scroll` supports optional position (`scroll_x`/`scroll_y`). `type_text`, `drag`, `pointer_move_rel`, `wait`, `locate` = standard actions."
                },
                "x": { "type": "integer", "description": "For `mouse_move` and `drag`: X in **global display** units when **`use_screen_coordinates`: true** (required). **Not** for `click`." },
                "y": { "type": "integer", "description": "For `mouse_move` and `drag`: Y in **global display** units when **`use_screen_coordinates`: true** (required). **Not** for `click`." },
                "coordinate_mode": { "type": "string", "enum": ["image", "normalized"], "description": "Ignored for `mouse_move` / `drag` — host rejects image/normalized positioning; always set **`use_screen_coordinates`: true**." },
                "use_screen_coordinates": { "type": "boolean", "description": "For `mouse_move`, `drag`: **must be true** — global display coordinates (e.g. macOS points) from `move_to_text`, `locate`, AX, or `pointer_global`. **Not** for `click`." },
                "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "For `click`, `click_element`, `drag`: mouse button (default left)." },
                "num_clicks": { "type": "integer", "minimum": 1, "maximum": 3, "description": "For `click`, `click_element`: 1=single (default), 2=double, 3=triple click." },
                "delta_x": { "type": "integer", "description": "For `pointer_move_rel`: horizontal delta (negative=left); also accepted as `dx`. **Not** allowed as the first move after `screenshot` (host). For `scroll`: horizontal wheel delta." },
                "delta_y": { "type": "integer", "description": "For `pointer_move_rel`: vertical delta (negative=up); also accepted as `dy`. **Not** allowed as the first move after `screenshot` (host). For `scroll`: vertical wheel delta." },
                "start_x": { "type": "integer", "description": "For `drag`: start X coordinate." },
                "start_y": { "type": "integer", "description": "For `drag`: start Y coordinate." },
                "end_x": { "type": "integer", "description": "For `drag`: end X coordinate." },
                "end_y": { "type": "integer", "description": "For `drag`: end Y coordinate." },
                "keys": { "type": "array", "items": { "type": "string" }, "description": "For `key_chord`: keys in order — **modifiers first**, then the main key (e.g. `[\"command\",\"f\"]`). Desktop host waits after pressing modifiers so shortcuts register (important on macOS with IME). Modifiers: command, control, shift, alt/option. Arrows: `up`, `down`, … Host may require a fresh screenshot before Return/Enter when the pointer is stale." },
                "text": { "type": "string", "description": "For `type_text`: text to type. Prefer clipboard paste (key_chord) for long content." },
                "ms": { "type": "integer", "description": "For `wait`: duration in milliseconds." },
                "target_text": { "type": "string", "description": "For `move_to_target` / `click_target`: visible or accessible text. The resolver tries AX text first, then OCR text, without requiring a prior screenshot." },
                "target_match_index": { "type": "integer", "minimum": 1, "description": "For `move_to_target` / `click_target`: optional 1-based OCR match index when you want a specific candidate. Alias of `move_to_text_match_index` for the unified target actions." },
                "text_query": { "type": "string", "description": "For `move_to_text`, `move_to_target`, `click_target`: visible text to OCR-match on screen (case-insensitive substring)." },
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
                "identifier_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring on AXIdentifier." },
                "text_contains": { "type": "string", "description": "For `locate`, `click_element`: case-insensitive substring matched against ANY of AXTitle / AXValue / AXDescription / AXHelp. Best default when the visible label lives in value/description (e.g. AXStaticText cards)." },
                "node_idx": { "type": "integer", "minimum": 0, "description": "For `locate`, `click_element`: jump straight to a node returned by the most recent `desktop.get_app_state` (field `idx`). Bypasses BFS. macOS only; other platforms return AX_IDX_NOT_SUPPORTED." },
                "app_state_digest": { "type": "string", "description": "For `locate`, `click_element`: optional `state_digest` from the same `get_app_state` call that produced `node_idx`. Stale digest yields AX_IDX_STALE so you re-snapshot." },
                "max_depth": { "type": "integer", "minimum": 1, "maximum": 200, "description": "For `locate`, `click_element`: max BFS depth (default 48). Ignored when `node_idx` is supplied." },
                "filter_combine": { "type": "string", "enum": ["all", "any"], "description": "For `locate`, `click_element`: `all` (default, AND) or `any` (OR) for filter combination. Priority: `node_idx` > `text_contains` > `title_contains`+`role_substring`." },
                "screenshot_crop_center_x": { "type": "integer", "minimum": 0, "description": "For `screenshot`: point crop X center in full-capture native pixels." },
                "screenshot_crop_center_y": { "type": "integer", "minimum": 0, "description": "For `screenshot`: point crop Y center in full-capture native pixels." },
                "screenshot_crop_half_extent_native": { "type": "integer", "minimum": 0, "description": "For `screenshot`: half-size of point crop in native pixels (default 250)." },
                "screenshot_navigate_quadrant": { "type": "string", "enum": ["top_left", "top_right", "bottom_left", "bottom_right"], "description": "For `screenshot`: zoom into quadrant. Repeat until `quadrant_navigation_click_ready` is true." },
                "screenshot_reset_navigation": { "type": "boolean", "description": "For `screenshot`: reset to full display before this capture." },
                "screenshot_implicit_center": { "type": "string", "enum": ["mouse", "text_caret"], "description": "For `screenshot` when `requires_fresh_screenshot_before_click` / `requires_fresh_screenshot_before_enter` is true: center the implicit ~500×500 on the mouse (`mouse`, default) or on the focused text control (`text_caret`, macOS AX; falls back to mouse). Applies to the **first** confirmation capture too. Ignored when you set `screenshot_crop_center_*` / `screenshot_navigate_quadrant` / `screenshot_reset_navigation`." },
                "app_name": { "type": "string", "description": "For `open_app`: the application name to launch (e.g. \"Safari\", \"WeChat\", \"Visual Studio Code\")." },
                "script": { "type": "string", "description": "For `run_apple_script`: the AppleScript code to execute via `osascript`. macOS only." },
                "scroll_x": { "type": "integer", "description": "For `scroll`: optional global X coordinate to move pointer before scrolling. Use with `scroll_y`. Requires `use_screen_coordinates`: true." },
                "scroll_y": { "type": "integer", "description": "For `scroll`: optional global Y coordinate to move pointer before scrolling. Use with `scroll_x`. Requires `use_screen_coordinates`: true." }
            },
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

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
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

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if context.is_remote() {
            return Err(BitFunError::tool(
                "ComputerUse cannot run while the session workspace is remote (SSH).".to_string(),
            ));
        }
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            BitFunError::tool(
                "Computer use is only available in the BitFun desktop app.".to_string(),
            )
        })?;

        let host_ref = host.as_ref();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("action is required".to_string()))?;

        match action {
            "locate" => execute_computer_use_locate(input, context).await,

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
                    return Err(BitFunError::tool(
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
                        BitFunError::tool(
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
                        return Err(BitFunError::tool(format!(
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
                                return Err(BitFunError::tool(format!(
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

            // ---- NEW: mouse_move (absolute pointer move, consolidated from ComputerUseMousePrecise) ----
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

            // ---- NEW: scroll (consolidated from ComputerUseMouseClick wheel action) ----
            "scroll" => {
                let dx = input.get("delta_x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let dy = input.get("delta_y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                if dx == 0 && dy == 0 {
                    return Err(BitFunError::tool(
                        "scroll requires non-zero delta_x and/or delta_y".to_string(),
                    ));
                }
                // Positional scroll: move pointer to target before scrolling.
                let scroll_pos_x = input.get("scroll_x").and_then(|v| v.as_i64());
                let scroll_pos_y = input.get("scroll_y").and_then(|v| v.as_i64());
                if let (Some(sx), Some(sy)) = (scroll_pos_x, scroll_pos_y) {
                    host_ref.mouse_move_global_f64(sx as f64, sy as f64).await?;
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

                // Move to start, press, move to end, release.
                host_ref.mouse_move_global_f64(sx0, sy0).await?;
                host_ref.mouse_down(button).await?;
                // Small pause for apps that need time to register the press.
                host_ref.wait_ms(50).await?;
                host_ref.mouse_move_global_f64(sx1, sy1).await?;
                host_ref.wait_ms(50).await?;
                host_ref.mouse_up(button).await?;
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
                    return Err(BitFunError::tool(
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
                            return Err(BitFunError::tool(
                                "[INVALID_PARAMS] key_chord requires `keys` as a JSON array of key names\nHints: example { \"keys\": [\"command\", \"v\"] } | for a single key { \"keys\": [\"return\"] } | use lowercase canonical names: command, control, option, shift, return, escape, tab, space, delete, arrow_up/down/left/right, f1..f12"
                                    .to_string(),
                            ));
                        }
                    },
                    _ => {
                        return Err(BitFunError::tool(
                            "[INVALID_PARAMS] key_chord `keys` must be a string or array of strings\nHints: example { \"keys\": [\"command\", \"v\"] }".to_string(),
                        ));
                    }
                };
                if keys.is_empty() {
                    return Err(BitFunError::tool(
                        "[INVALID_PARAMS] key_chord `keys` must not be empty\nHints: example { \"keys\": [\"return\"] }".to_string(),
                    ));
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
                    .ok_or_else(|| BitFunError::tool("text is required".to_string()))?;
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
                    .ok_or_else(|| BitFunError::tool("ms is required".to_string()))?;
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
                        BitFunError::tool("open_app requires `app_name` parameter.".to_string())
                    })?;
                let result = host_ref.open_app(app_name).await?;
                let body = computer_use_augment_result_json(
                    host_ref,
                    json!({
                        "success": result.success,
                        "action": "open_app",
                        "app_name": result.app_name,
                        "process_id": result.process_id,
                        "error_message": result.error_message,
                    }),
                    None,
                )
                .await;
                let summary = if result.success {
                    format!(
                        "Opened app '{}'{}.",
                        result.app_name,
                        result
                            .process_id
                            .map(|p| format!(" (PID {})", p))
                            .unwrap_or_default()
                    )
                } else {
                    format!(
                        "Failed to open '{}': {}",
                        result.app_name,
                        result.error_message.as_deref().unwrap_or("unknown error")
                    )
                };
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }

            "run_apple_script" => {
                let script = input
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "run_apple_script requires `script` parameter.".to_string(),
                        )
                    })?;
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = script;
                    return Err(BitFunError::tool(
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
                    .map_err(|e| BitFunError::tool(format!("spawn: {}", e)))?
                    .map_err(|e| BitFunError::tool(format!("osascript: {}", e)))?;

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
                                format!(" Output: {}", &stdout[..stdout.len().min(200)])
                            }
                        )
                    } else {
                        format!("AppleScript error: {}", &stderr[..stderr.len().min(200)])
                    };
                    Ok(vec![ToolResult::ok(body, Some(summary))])
                }
            }

            _ => Err(BitFunError::tool(format!("Unknown action: {}", action))),
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

fn req_i32(input: &Value, key: &str) -> BitFunResult<i32> {
    input
        .get(key)
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .ok_or_else(|| BitFunError::tool(format!("{} is required (integer)", key)))
}
