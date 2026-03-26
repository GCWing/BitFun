//! Desktop automation for Claw (Computer use).

use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::computer_use_host::{
    ComputerScreenshot, ComputerUseNavigateQuadrant, ComputerUseScreenshotParams,
    ComputerUseScreenshotRefinement, ScreenshotCropCenter,
    COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE, COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
};
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::ToolImageAttachment;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use log::{debug, warn};
use serde_json::{json, Value};

/// On-disk copy of each Computer use screenshot (pointer overlay included) for debugging.
/// Filenames: `cu_<ms>_full.jpg` (whole display) or `cu_<ms>_crop_<x>_<y>.jpg` when a point crop was requested.
const COMPUTER_USE_DEBUG_SUBDIR: &str = ".bitfun/computer_use_debug";

pub struct ComputerUseTool;

impl ComputerUseTool {
    pub fn new() -> Self {
        Self
    }

    fn primary_api_format(ctx: &ToolUseContext) -> String {
        ctx.options
            .as_ref()
            .and_then(|o| o.custom_data.as_ref())
            .and_then(|m| m.get("primary_model_provider"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase()
    }

    /// Screenshot tool results attach JPEGs via `tool_image_attachments`; only providers whose
    /// request converters emit multimodal tool output are supported (Anthropic + OpenAI-compatible).
    fn require_multimodal_tool_output_for_screenshot(ctx: &ToolUseContext) -> BitFunResult<()> {
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

    fn use_screen_coordinates(input: &Value) -> bool {
        input
            .get("use_screen_coordinates")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// `image` (default): x,y are pixel indices in the attached screenshot (`image_width` x `image_height`).
    /// `normalized`: x,y each in 0..=1000 across the captured display (coarser but easier for models).
    fn coordinate_mode(input: &Value) -> &str {
        input
            .get("coordinate_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("image")
    }

    fn resolve_xy_f64(
        host: &dyn crate::agentic::tools::computer_use_host::ComputerUseHost,
        input: &Value,
        x: i32,
        y: i32,
    ) -> BitFunResult<(f64, f64)> {
        if Self::use_screen_coordinates(input) {
            return Ok((x as f64, y as f64));
        }
        if Self::coordinate_mode(input) == "normalized" {
            host.map_normalized_coords_to_pointer_f64(x, y)
        } else {
            host.map_image_coords_to_pointer_f64(x, y)
        }
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
            (Some(_), Some(_)) => "The JPEG includes a **synthetic red cursor with gray border** marking the **actual mouse position** on this bitmap (not the OS arrow). The **tip** is the true click hotspot (same pixel as pointer_image_x and pointer_image_y). Use this marker and those numbers for mouse_move — do not ignore them or guess from the OS cursor alone.",
            _ => "No pointer overlay in this JPEG (pointer_image_x/y null): the cursor is not on this bitmap (e.g. another display). Do not infer position from the image; use global screen coordinates + use_screen_coordinates, or move the pointer onto this display and screenshot again.",
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
            "navigation_native_rect": shot.navigation_native_rect,
            "quadrant_navigation_click_ready": shot.quadrant_navigation_click_ready,
            "debug_screenshot_path": debug_rel,
        });
        let shortcut_policy = format!(
            "**First:** `key_chord` for shortcuts **and** system clipboard (copy/cut/paste/select-all per host OS) — avoid Edit-menu clicks and avoid long `type_text` when paste fits. **Then** pointer when shortcuts do not fit (then screenshot). **Default for click prep:** after a full-frame shot, chain `screenshot` + `screenshot_navigate_quadrant` until `quadrant_navigation_click_ready` (long edge < {} px). **Do not** skip to `screenshot_crop_center_*` from full screen unless justified. **Quadrant narrowing is never automatic:** each drill step must set `screenshot_navigate_quadrant` on that `screenshot` call; a bare `screenshot` only refreshes. Point crop (~500×500) is a **fallback**. **Small pointer tweaks:** prefer `mouse_move` with **`mouse_move_direction`** (+ optional `mouse_move_relative_pixels`) over absolute `x`/`y` — easier for vision models than sub-pixel absolute coords. Fresh screenshot before key_chord that sends Return/Enter.",
            COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE
        );
        let hierarchical_navigation = if shot.screenshot_crop_center.is_some() {
            json!({
                "phase": "region_crop",
                "image_is_crop_only": true,
                "shortcut_policy": shortcut_policy,
                "instruction": "Crop frame (~500×500): **margin ruler numbers** are **full-capture native** indices (same whole-screen bitmap space as a full-screen shot — not 0..500 local). `coordinate_mode` \"image\" uses **this JPEG’s** pixel grid (content area under the rulers). For another view, call screenshot with new `screenshot_crop_center_*` in that same full-capture space. See shortcut_policy."
            })
        } else if shot.quadrant_navigation_click_ready {
            json!({
                "phase": "quadrant_terminal",
                "image_is_crop_only": true,
                "shortcut_policy": shortcut_policy,
                "instruction": "Region is small enough for precise pointer: **`quadrant_navigation_click_ready`** is true. For **small** alignment fixes, prefer **`mouse_move`** with **`mouse_move_direction`** (`up`/`down`/`left`/`right`) and **`mouse_move_relative_pixels`**; use absolute `x`/`y` only for larger jumps. Then `click` (no extra point crop required). After pointer moves, screenshot again before the next click (host)."
            })
        } else if !Self::shot_covers_full_display(shot) {
            json!({
                "phase": "quadrant_drill",
                "image_is_crop_only": true,
                "shortcut_policy": shortcut_policy,
                "instruction": format!(
                    "**Keep drilling (default):** call **`screenshot`** again with **`screenshot_navigate_quadrant`**: `top_left` | `top_right` | `bottom_left` | `bottom_right` — pick the tile that contains your target. The host expands the chosen quadrant by **{} px** on each side (clamped) so split-edge controls stay in-frame. Repeat until `quadrant_navigation_click_ready`. To restart from the full display, set **`screenshot_reset_navigation`**: true on the next screenshot. Ruler numbers stay **full-display native**. See shortcut_policy.",
                    COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX
                )
            })
        } else {
            json!({
                "phase": "full_display",
                "image_is_crop_only": false,
                "host_auto_quadrant": false,
                "next_step_for_mouse_click": "**Preferred (A):** next tool call = `screenshot` **with** `screenshot_navigate_quadrant` set (top_left|top_right|bottom_left|bottom_right). Repeat until `quadrant_navigation_click_ready`. **Fallback (B):** `screenshot` with `screenshot_crop_center_x/y` only when quadrant drill is a poor fit. The host never splits the screen unless you pass `screenshot_navigate_quadrant`.",
                "shortcut_policy": shortcut_policy,
                "instruction": "Full frame: ruler indices are **full-display native** pixels. **You should start quadrant drill now:** next `screenshot` **must** include **`screenshot_navigate_quadrant`** (pick the quadrant that contains your click target). Repeat one quadrant per call until `quadrant_navigation_click_ready`, then `mouse_move` + `click`. Point crop is **secondary**. **`click` is rejected** on full-screen-only. See `next_step_for_mouse_click`, `recommended_next_for_click_targeting`, shortcut_policy."
            })
        };
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "hierarchical_navigation".to_string(),
                hierarchical_navigation,
            );
            if shot.screenshot_crop_center.is_none() && !shot.quadrant_navigation_click_ready {
                let rec = if Self::shot_covers_full_display(shot) {
                    "screenshot_navigate_quadrant"
                } else {
                    "screenshot_navigate_quadrant_until_click_ready"
                };
                obj.insert(
                    "recommended_next_for_click_targeting".to_string(),
                    Value::String(rec.to_string()),
                );
            }
        }
        let attach = ToolImageAttachment {
            mime_type: shot.mime_type.clone(),
            data_base64: b64,
        };
        let pointer_line = match (shot.pointer_image_x, shot.pointer_image_y) {
            (Some(px), Some(py)) => format!(
                " TRUE POINTER: **red cursor with gray border** (tip = hotspot) in the JPEG marks the mouse at this pixel — coordinate_mode \"image\" mouse_move target x={}, y={}. Align moves so the **tip** sits on your click target, then click. Prior screenshot is stale after mouse_move/pointer_* until you screenshot again.",
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
                "Region crop screenshot {}x{} around full-display native center ({}, {}). Use `image` coords in **this** bitmap only.{}.{} After pointer moves, screenshot again before click (host).",
                shot.image_width,
                shot.image_height,
                c.x,
                c.y,
                pointer_line,
                debug_line
            )
        } else if shot.quadrant_navigation_click_ready {
            format!(
                "Quadrant terminal {}x{} (native region {:?}). **`quadrant_navigation_click_ready`**: use `image` coords on this JPEG, then `mouse_move` + `click`.{}.{}",
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
                "Full screenshot {}x{} (vision_scale={}). Rulers + grid: **native** 0..={} x 0..={}. **Quadrant drill is not automatic** — the next narrowing step must set **`screenshot_navigate_quadrant`** on `screenshot` (repeat until `quadrant_navigation_click_ready`), or use point crop (`screenshot_crop_center_*`).{}.{} After pointer moves, fresh fine screenshot before click; Return/Enter in key_chord needs fresh screenshot (host).",
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

    fn parse_screenshot_crop_center(input: &Value) -> BitFunResult<Option<ScreenshotCropCenter>> {
        let xv = input.get("screenshot_crop_center_x");
        let yv = input.get("screenshot_crop_center_y");
        let x_none = xv.map_or(true, |v| v.is_null());
        let y_none = yv.map_or(true, |v| v.is_null());
        match (x_none, y_none) {
            (true, true) => Ok(None),
            (false, false) => {
                let x = xv
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "screenshot_crop_center_x must be a non-negative integer (full-display native pixels)."
                                .to_string(),
                        )
                    })?;
                let y = yv
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "screenshot_crop_center_y must be a non-negative integer (full-display native pixels)."
                                .to_string(),
                        )
                    })?;
                let x = u32::try_from(x).map_err(|_| {
                    BitFunError::tool("screenshot_crop_center_x is too large.".to_string())
                })?;
                let y = u32::try_from(y).map_err(|_| {
                    BitFunError::tool("screenshot_crop_center_y is too large.".to_string())
                })?;
                Ok(Some(ScreenshotCropCenter { x, y }))
            }
            _ => Err(BitFunError::tool(
                "screenshot_crop_center_x and screenshot_crop_center_y must both be set or both omitted for action screenshot."
                    .to_string(),
            )),
        }
    }

    /// True if the client sent non-null `screenshot_crop_center_x` and/or `y` (often `0` placeholders).
    fn input_has_screenshot_crop_fields(input: &Value) -> bool {
        let x = input.get("screenshot_crop_center_x");
        let y = input.get("screenshot_crop_center_y");
        x.map_or(false, |v| !v.is_null()) || y.map_or(false, |v| !v.is_null())
    }

    fn parse_screenshot_navigate_quadrant(input: &Value) -> BitFunResult<Option<ComputerUseNavigateQuadrant>> {
        let v = input
            .get("screenshot_navigate_quadrant")
            .filter(|x| !x.is_null())
            .and_then(|x| x.as_str());
        let Some(s) = v else {
            return Ok(None);
        };
        let n = s.trim().to_ascii_lowercase().replace('-', "_");
        Ok(Some(match n.as_str() {
            "top_left" | "topleft" | "upper_left" => ComputerUseNavigateQuadrant::TopLeft,
            "top_right" | "topright" | "upper_right" => ComputerUseNavigateQuadrant::TopRight,
            "bottom_left" | "bottomleft" | "lower_left" => ComputerUseNavigateQuadrant::BottomLeft,
            "bottom_right" | "bottomright" | "lower_right" => ComputerUseNavigateQuadrant::BottomRight,
            _ => {
                return Err(BitFunError::tool(
                    "screenshot_navigate_quadrant must be one of: top_left, top_right, bottom_left, bottom_right."
                        .to_string(),
                ));
            }
        }))
    }

    /// Second return value: crop fields were present but ignored because quadrant navigation wins.
    fn parse_screenshot_params(input: &Value) -> BitFunResult<(ComputerUseScreenshotParams, bool)> {
        let navigate = Self::parse_screenshot_navigate_quadrant(input)?;
        let reset_navigation = input
            .get("screenshot_reset_navigation")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if navigate.is_some() {
            let ignored_crop = Self::input_has_screenshot_crop_fields(input);
            return Ok((
                ComputerUseScreenshotParams {
                    crop_center: None,
                    navigate_quadrant: navigate,
                    reset_navigation,
                },
                ignored_crop,
            ));
        }
        let crop = Self::parse_screenshot_crop_center(input)?;
        Ok((
            ComputerUseScreenshotParams {
                crop_center: crop,
                navigate_quadrant: None,
                reset_navigation,
            },
            false,
        ))
    }

    /// Cardinal relative move for `action: mouse_move`. Same pixel space as `pointer_nudge` / `pointer_move_rel`.
    fn parse_mouse_move_cardinal(input: &Value) -> BitFunResult<Option<(i32, i32, String, i32)>> {
        let dir_val = match input.get("mouse_move_direction") {
            None => return Ok(None),
            Some(v) if v.is_null() => return Ok(None),
            Some(v) => v,
        };
        let dir = dir_val.as_str().ok_or_else(|| {
            BitFunError::tool(
                "mouse_move_direction must be a string: up, down, left, or right.".to_string(),
            )
        })?;
        let px = input
            .get("mouse_move_relative_pixels")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(32)
            .clamp(1, 400);
        let norm = dir.trim().to_ascii_lowercase().replace('-', "_");
        let (dx, dy, label) = match norm.as_str() {
            "up" => (0, -px, "up"),
            "down" => (0, px, "down"),
            "left" => (-px, 0, "left"),
            "right" => (px, 0, "right"),
            _ => {
                return Err(BitFunError::tool(
                    "mouse_move_direction must be one of: up, down, left, right.".to_string(),
                ));
            }
        };
        Ok(Some((dx, dy, label.to_string(), px)))
    }

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
            "Desktop Computer use (host OS: {}). {} \
**Automation priority (read order):** (1) **`key_chord`** — OS/app shortcuts **and** **system clipboard** (select all / copy / cut / paste via the host’s real modifier keys — see hint below). Prefer **paste** over **`type_text`** for long or duplicated content and whenever the user expects clipboard behavior; do **not** drive the mouse to Edit → Copy/Paste when chords exist. (2) **`type_text`** — short input, paste-blocked fields, or after chords failed. (3) **Mouse** (`screenshot` drill + `mouse_move` + `click`) — only when shortcuts and clipboard do not apply or after they failed. \
**`screenshot` image layout (read this):** Every **`screenshot`** returns a JPEG with **white margins on all four sides** showing **numeric coordinate tick labels** (full-capture native pixel indices — the same scale on full-screen and point-crop shots), and a **line grid** drawn on the captured desktop **inside** those margins. Read x/y from the **top/bottom/left/right** margin numbers to aim moves and for **point crop** (`screenshot_crop_center_*`) when that path is justified. The inner bitmap (below the rulers) is the live capture. \
**Default before `click` (mouse path):** After the **first** full **`screenshot`**, **your next screenshot call should set `screenshot_navigate_quadrant`** (one of `top_left`, `top_right`, `bottom_left`, `bottom_right`) — **do not** jump straight to **`screenshot_crop_center_*`** unless the target is already huge on screen or you have a stable native center from rulers. Chain **`screenshot` + `screenshot_navigate_quadrant`** until **`quadrant_navigation_click_ready`: true** in the tool JSON, then **`mouse_move`** + **`click`**. Tool results may include **`recommended_next_for_click_targeting`** — obey it. \
**Shortcut-first (default):** When a **standard OS or in-app shortcut** or **clipboard chord** achieves the same step (e.g. New/Open/Save, Copy/Cut/Paste, Undo/Redo, Find, Close tab/window, Quit, Refresh, tab/window switch, focus address bar, select all), you **must prefer `key_chord`** over moving the pointer and clicking — **do not** default to mouse for actions that have a well-known chord on this host. Use pointer + screenshots when **no** suitable shortcut exists, the target is only reachable by mouse, menus show no shortcut, or a shortcut attempt clearly failed (then **screenshot** and reassess). \
After `key_chord`, `type_text`, or `scroll`, when the **next step depends on what is on screen**, run **`screenshot`** (optionally `wait` ms first) and verify — do not chain many shortcuts without a screenshot when failure would mislead. \
**No blind submit or click (unchanged):** before **`click`** (any button) and before **`key_chord` that sends Return/Enter** (or any key that submits/confirms), you **must** run **`screenshot` first** and visually confirm focus and target — **never** click or press Enter without a fresh screenshot when the outcome matters. Same discipline after moving the pointer. \
**Quadrant drill (default zoom for precision; not automatic):** The app **never** splits the screen by itself. After an initial full **`screenshot`**, **each** narrowing step must be **`screenshot` + `screenshot_navigate_quadrant`** ∈ {{`top_left`,`top_right`,`bottom_left`,`bottom_right`}} — omitting that field only **refreshes** full screen (or the current drill region). The host returns the chosen quarter **plus {} px on each side** (clamped); rulers stay **full-display native**. Repeat until **`quadrant_navigation_click_ready`: true** (longest native side < {} px), then **`mouse_move`** and **`click`**. **`screenshot_reset_navigation`**: true restarts from full display. **If `screenshot_navigate_quadrant` is set, `screenshot_crop_center_*` are ignored**. **Point crop** (`screenshot_crop_center_*` only, no `screenshot_navigate_quadrant`) is a **fallback** when quadrant drill is a poor fit. \
**Screenshot zoom:** When you must **confirm** small text, dense UI, or the **red cursor** tip, **proactively** zoom — **prefer quadrant drill**; use point crop only when justified — **do not** rely only on huge full-display images when a smaller view answers the question. \
`mouse_move`: **absolute** `x`/`y` (`coordinate_mode` / `use_screen_coordinates`) **or** **relative cardinal** `mouse_move_direction` (`up`|`down`|`left`|`right`) with optional `mouse_move_relative_pixels` (default 32, same screenshot-pixel space as `pointer_nudge`). If `mouse_move_direction` is set, `x`/`y` are ignored. **Recommendation:** for **small** pointer adjustments (nudging the red tip onto a control), **prefer relative** `mouse_move_direction` over guessing absolute `x`/`y` — usually more reliable for vision models. `click` only at current pointer (optional button), never moves. \
**Host (desktop):** Call **`screenshot`** when you need current pixels; there is **no** automatic follow-up capture after other actions. Before **`click`**, after pointer moves, the host requires a fresh **fine** basis: **`quadrant_navigation_click_ready`** (preferred path) **or** a **point crop** — **full-screen-only** is **not** enough. Before **`key_chord`** with **Return/Enter**, a fresh **`screenshot`** (any mode) is required. Numeric fields in each tool result JSON are authoritative for that frame. \
`pointer_nudge` / `pointer_move_rel` for relative screen-pixel moves (same idea; `mouse_move` can cover cardinal nudges in one call). \
Each **`screenshot`** JPEG: **four-side margin coordinate scales** (numbers), **grid on the capture**, and a **synthetic mouse marker** when the pointer is on that display (**red** with **gray border**; **tip** = hotspot, same as **`pointer_image_x` / `pointer_image_y`**). On macOS, `mouse_move` uses sub-point Quartz when applicable. Also **wait**. **Per `action`:** send **only** the parameters that apply (e.g. for `screenshot` do not send `keys`, `button`, `x`/`y` for `mouse_move`, etc.) — extra keys may confuse you or the UI. macOS: Accessibility for the running binary.",
            os,
            keys,
            COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
            COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE
        ))
    }

    fn input_schema(&self) -> Value {
        let qpad = COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX;
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["screenshot", "mouse_move", "click", "pointer_nudge", "pointer_move_rel", "scroll", "key_chord", "type_text", "wait"],
                    "description": format!("**Before any mouse drill:** try `key_chord` (shortcuts + clipboard copy/cut/paste/select-all per OS). **action `screenshot`:** JPEG with **margin coordinate scales** + **grid** (**full-capture native** indices). **After a full-screen capture, the usual next step before click is (2):** **`screenshot_navigate_quadrant`** — 4-way drill; chosen quadrant **plus {} px per side** (clamped). Repeat until tool JSON `quadrant_navigation_click_ready`. **Modes:** (1) Plain / refresh — same region or full display (no narrowing). (2) **`screenshot_navigate_quadrant`** — **default zoom path** for mouse clicks. (3) **`screenshot_reset_navigation`**: true — full display base. (4) **`screenshot_crop_center_*`** — ~500×500 point crop (**fallback**, not the default from full screen). **Precedence:** if `screenshot_navigate_quadrant` is set, **`screenshot_crop_center_*` are ignored**. **Prefer** sending **only** fields relevant to `screenshot` for this call. When **`quadrant_navigation_click_ready`** is true, you may **`mouse_move` + `click`**. **Other actions:** `key_chord` + clipboard before `type_text`; red synthetic cursor when the mouse is on this display.", qpad)
                },
                "x": { "type": "integer", "description": "For mouse_move **absolute** mode only (omit when `mouse_move_direction` is set): pixel in **image** mode uses the last screenshot JPEG grid; **normalized** 0..=1000 on the captured display; or **use_screen_coordinates** for global px. **Small moves:** prefer `mouse_move_direction` + `mouse_move_relative_pixels` instead of tiny absolute deltas." },
                "y": { "type": "integer", "description": "For mouse_move **absolute** mode only: same as x (including **small moves** → prefer relative). Ignored when `mouse_move_direction` is set." },
                "mouse_move_direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "For mouse_move **relative** mode (**recommended for small nudges**): move the pointer in this cardinal direction by `mouse_move_relative_pixels` (screenshot/display pixels; macOS converts via last capture — screenshot first). **Takes precedence** over `x`/`y` when set."
                },
                "mouse_move_relative_pixels": {
                    "type": "integer",
                    "description": "For mouse_move with `mouse_move_direction`: distance in **screenshot/display pixels** (default 32, clamped 1..400). Use smaller values (e.g. 8–24) for fine alignment. Same semantics as `pointer_nudge` `pixels`."
                },
                "coordinate_mode": {
                    "type": "string",
                    "enum": ["image", "normalized"],
                    "description": "For mouse_move when use_screen_coordinates is false. \"image\" = pixels on the latest screenshot JPEG; \"normalized\" = 0..=1000 on the latest capture."
                },
                "use_screen_coordinates": {
                    "type": "boolean",
                    "description": "For mouse_move only: if true, x/y are global display coordinates in the host’s native units (on macOS: **points**, same space as `pointer_x`/`pointer_y` after a move — not necessarily physical Retina pixels)."
                },
                "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "For click only (default left). **Desktop:** after the last pointer move, `click` requires a fresh **fine** screenshot: **`quadrant_navigation_click_ready`** true **or** point crop (`screenshot_crop_center_*`). Full-screen-only basis is rejected (host)." },
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "For pointer_nudge: cardinal move in screen pixels"
                },
                "pixels": {
                    "type": "integer",
                    "description": "For pointer_nudge: distance in **screenshot/display pixels** (same scale as the last JPEG width/height on that display). On macOS this is converted to CG point deltas using the last capture; take screenshot first."
                },
                "delta_x": { "type": "integer", "description": "For pointer_move_rel: horizontal delta in screenshot/display pixels (negative=left). On macOS converted via last screenshot scale; screenshot first. For scroll: horizontal scroll amount (host-dependent)." },
                "delta_y": { "type": "integer", "description": "For pointer_move_rel: vertical delta in screenshot/display pixels (negative=up). On macOS converted via last screenshot scale; screenshot first. For scroll: vertical scroll amount (host-dependent)." },
                "keys": { "type": "array", "items": { "type": "string" }, "description": "For key_chord: **prefer this action** for standard shortcuts **and** **system clipboard** (e.g. select all + copy/cut/paste per host — see tool description OS hint). Do not use mouse menus for Copy/Paste when these chords work. OS-specific key names per Environment Information. If the chord includes **return** / **enter** (submit/confirm), **`screenshot` first** and verify — **no blind Enter.** Otherwise screenshot when the next action depends on UI." },
                "text": { "type": "string", "description": "For type_text: short or paste-blocked input only — **prefer `key_chord` paste** (and focus/select chords) when inserting longer or duplicated content from the system clipboard. Then screenshot if you need to confirm focus or field content before further steps." },
                "ms": { "type": "integer", "description": "Wait duration in milliseconds" },
                "screenshot_crop_center_x": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "For action `screenshot` only (point crop): X center in **full-capture native** pixels — same as margin tick labels on a prior full-screen shot. Pair with `screenshot_crop_center_y`. Omit **both** when using `screenshot_navigate_quadrant` or plain refresh. **Ignored** if `screenshot_navigate_quadrant` is set. ~500×500 region when both set."
                },
                "screenshot_crop_center_y": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "For action `screenshot` only (point crop): Y center in **full-capture native** pixels; pair with `screenshot_crop_center_x`. Omit **both** for quadrant drill or plain refresh. **Ignored** if `screenshot_navigate_quadrant` is set."
                },
                "screenshot_navigate_quadrant": {
                    "type": "string",
                    "enum": ["top_left", "top_right", "bottom_left", "bottom_right"],
                    "description": format!("For action `screenshot` only: **set this on the next screenshot after a full-frame shot** (default path before click). Pick one quadrant of the **current** region (or full display after reset); host returns that tile + **{} px** padding per side (clamped). Enum: `top_left`, `top_right`, `bottom_left`, `bottom_right`. **Takes precedence:** any `screenshot_crop_center_*` in the same call are **ignored**.", qpad)
                },
                "screenshot_reset_navigation": {
                    "type": "boolean",
                    "description": "For action `screenshot` only: if true, clear quadrant navigation before this capture so the base region is the **full** display (then apply `screenshot_navigate_quadrant` if set)."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
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

    async fn call_impl(&self, input: &Value, context: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        if context.agent_type.as_deref() != Some("Claw") {
            return Err(BitFunError::tool(
                "ComputerUse is only available in Claw assistant mode.".to_string(),
            ));
        }
        if context.is_remote() {
            return Err(BitFunError::tool(
                "ComputerUse cannot run while the session workspace is remote (SSH).".to_string(),
            ));
        }
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            BitFunError::tool("Computer use is only available in the BitFun desktop app.".to_string())
        })?;

        let host_ref = host.as_ref();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("action is required".to_string()))?;

        match action {
            "screenshot" => {
                Self::require_multimodal_tool_output_for_screenshot(context)?;
                let (params, ignored_crop_for_quadrant) = Self::parse_screenshot_params(input)?;
                let crop_for_debug = params.crop_center;
                let nav_debug = params.navigate_quadrant.map(|q| match q {
                    ComputerUseNavigateQuadrant::TopLeft => "nav_tl",
                    ComputerUseNavigateQuadrant::TopRight => "nav_tr",
                    ComputerUseNavigateQuadrant::BottomLeft => "nav_bl",
                    ComputerUseNavigateQuadrant::BottomRight => "nav_br",
                });
                let shot = host_ref.screenshot_display(params).await?;
                let debug_rel = Self::try_save_screenshot_for_debug(
                    &shot.bytes,
                    context,
                    crop_for_debug,
                    nav_debug,
                )
                .await;
                let (mut data, attach, mut hint) =
                    Self::pack_screenshot_tool_output(&shot, debug_rel).await?;
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("action".to_string(), Value::String("screenshot".to_string()));
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
                Ok(vec![ToolResult::ok_with_images(data, Some(hint), vec![attach])])
            }

            "pointer_nudge" => {
                let dir = input
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| BitFunError::tool("direction is required for pointer_nudge (up|down|left|right)".to_string()))?;
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
                let body = json!({
                    "success": true,
                    "action": "pointer_nudge",
                    "direction": dir,
                    "pixels": px,
                    "delta_x": dx,
                    "delta_y": dy,
                });
                let summary = format!(
                    "Nudged pointer by ({}, {}) px (direction {}, {} px).",
                    dx, dy, dir, px
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "pointer_move_rel" => {
                let dx = input.get("delta_x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let dy = input.get("delta_y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                if dx == 0 && dy == 0 {
                    return Err(BitFunError::tool(
                        "pointer_move_rel requires non-zero delta_x and/or delta_y (screen pixels)"
                            .to_string(),
                    ));
                }
                host_ref.pointer_move_relative(dx, dy).await?;
                let body = json!({
                    "success": true,
                    "action": "pointer_move_rel",
                    "delta_x": dx,
                    "delta_y": dy,
                });
                let summary = format!(
                    "Moved pointer relatively by ({}, {}) screen pixels.",
                    dx, dy
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "mouse_move" => {
                let last_ref = host_ref.last_screenshot_refinement();
                let snapshot_basis = match last_ref {
                    None => serde_json::Value::Null,
                    Some(ComputerUseScreenshotRefinement::FullDisplay) => json!("full_display"),
                    Some(ComputerUseScreenshotRefinement::RegionAroundPoint {
                        center_x,
                        center_y,
                    }) => {
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
                };

                if let Some((dx, dy, dir_label, px_used)) = Self::parse_mouse_move_cardinal(input)? {
                    host_ref.pointer_move_relative(dx, dy).await?;
                    let body = json!({
                        "success": true,
                        "action": "mouse_move",
                        "positioning": "relative_cardinal",
                        "mouse_move_direction": dir_label,
                        "mouse_move_relative_pixels": px_used,
                        "delta_x": dx,
                        "delta_y": dy,
                        "snapshot_coordinate_basis": snapshot_basis,
                    });
                    let summary = format!(
                        "mouse_move relative: {} by {} px (delta {}, {}).",
                        dir_label, px_used, dx, dy
                    );
                    return Ok(vec![ToolResult::ok(body, Some(summary))]);
                }

                let x = req_i32(input, "x")?;
                let y = req_i32(input, "y")?;
                let mode = Self::coordinate_mode(input);
                let use_screen = Self::use_screen_coordinates(input);
                let (sx64, sy64) = Self::resolve_xy_f64(host_ref, input, x, y)?;
                host_ref.mouse_move_global_f64(sx64, sy64).await?;
                let sx = sx64.round() as i32;
                let sy = sy64.round() as i32;
                let body = json!({
                    "success": true,
                    "action": "mouse_move",
                    "positioning": "absolute",
                    "x": x,
                    "y": y,
                    "pointer_x": sx,
                    "pointer_y": sy,
                    "coordinate_mode": mode,
                    "use_screen_coordinates": use_screen,
                    "snapshot_coordinate_basis": snapshot_basis,
                });
                let summary = format!(
                    "Moved pointer to global screen (~{}, ~{}, sub-point on macOS) (input {:?} {}, {}).",
                    sx, sy, mode, x, y
                );
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "click" => {
                let button = input
                    .get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left");
                host_ref.mouse_click(button).await?;
                let body = json!({
                    "success": true,
                    "action": "click",
                    "button": button,
                });
                let summary = format!("{} click at current pointer (does not move).", button);
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "scroll" => {
                let dx = input.get("delta_x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let dy = input.get("delta_y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                if dx == 0 && dy == 0 {
                    return Err(BitFunError::tool(
                        "scroll requires non-zero delta_x and/or delta_y".to_string(),
                    ));
                }
                host_ref.scroll(dx, dy).await?;
                let body = json!({ "success": true, "action": "scroll", "delta_x": dx, "delta_y": dy });
                let summary = format!("Scrolled by ({}, {}).", dx, dy);
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "key_chord" => {
                let keys: Vec<String> = input
                    .get("keys")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| BitFunError::tool("keys array is required".to_string()))?
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if keys.is_empty() {
                    return Err(BitFunError::tool("keys must not be empty".to_string()));
                }
                host_ref.key_chord(keys.clone()).await?;
                let body = json!({ "success": true, "action": "key_chord", "keys": keys });
                let summary = "Key chord sent.".to_string();
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "type_text" => {
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| BitFunError::tool("text is required".to_string()))?;
                host_ref.type_text(text).await?;
                let body = json!({ "success": true, "action": "type_text", "chars": text.chars().count() });
                let summary = format!("Typed {} character(s) into the focused target.", text.chars().count());
                Ok(vec![ToolResult::ok(body, Some(summary))])
            }
            "wait" => {
                let ms = input
                    .get("ms")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| BitFunError::tool("ms is required".to_string()))?;
                host_ref.wait_ms(ms).await?;
                Ok(vec![ToolResult::ok(
                    json!({ "success": true, "action": "wait", "ms": ms }),
                    Some(format!("Waited {} ms.", ms)),
                )])
            }
            _ => Err(BitFunError::tool(format!("Unknown action: {}", action))),
        }
    }
}

fn req_i32(input: &Value, key: &str) -> BitFunResult<i32> {
    input
        .get(key)
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .ok_or_else(|| BitFunError::tool(format!("{} is required (integer)", key)))
}
