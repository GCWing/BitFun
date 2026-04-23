//! Host abstraction for desktop automation (implemented in `bitfun-desktop`).

// Re-export optimizer types so downstream crates can import from computer_use_host.
pub use crate::agentic::tools::computer_use_optimizer::{ActionRecord, LoopDetectionResult};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Center of a **point crop** in **full-display native capture pixels** (same origin as full-screen computer-use JPEG pixels).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScreenshotCropCenter {
    pub x: u32,
    pub y: u32,
}

/// Native-pixel rectangle on the **captured display bitmap** (0..`native_width`, 0..`native_height`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComputerUseNavigationRect {
    pub x0: u32,
    pub y0: u32,
    pub width: u32,
    pub height: u32,
}

/// Subdivide the current navigation view into four tiles (model picks one per `screenshot` step).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseNavigateQuadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Center for host-applied **implicit** 500×500 confirmation crops (when a fresh screenshot is required).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseImplicitScreenshotCenter {
    #[default]
    Mouse,
    /// Best-effort focused text field / insertion area (macOS AX); other platforms fall back to mouse.
    TextCaret,
}

/// Parameters for [`ComputerUseHost::screenshot_display`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputerUseScreenshotParams {
    pub crop_center: Option<ScreenshotCropCenter>,
    pub navigate_quadrant: Option<ComputerUseNavigateQuadrant>,
    /// Clear stored navigation focus before applying this capture (next quadrant step starts from full display).
    pub reset_navigation: bool,
    /// Half-size of the point crop in **native** pixels (total width/height ≈ `2 * half`). `None` → [`COMPUTER_USE_POINT_CROP_HALF_DEFAULT`].
    pub point_crop_half_extent_native: Option<u32>,
    /// For `action: screenshot`: when the host applies an implicit 500×500 crop, use mouse vs text-focus center (see desktop host).
    pub implicit_confirmation_center: Option<ComputerUseImplicitScreenshotCenter>,
    /// For `action: screenshot`: crop the capture to the **focused window of
    /// the foreground application** instead of the default mouse-centered
    /// 500×500 region. The single most useful setting after `system.open_app`,
    /// `cmd+f`, or any keystroke that may have moved focus inside an app
    /// without moving the mouse — the model gets the WHOLE application
    /// window in one shot rather than a stale 500×500 around an unrelated
    /// pointer position. Falls back to a full-display capture (with a
    /// `warning`) when the host cannot resolve the focused window (e.g.
    /// missing AX permission or the app exposes no AX windows).
    pub crop_to_focused_window: bool,
}

/// Longest side of the navigation region must be **strictly below** this to allow `click` without a separate point crop (desktop).
pub const COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE: u32 = 500;

/// Native pixels added on **each** side after a quadrant choice before compositing the JPEG (avoids controls sitting exactly on the split line).
pub const COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX: u32 = 50;

/// Default **half** extent (native px) for point crop around `screenshot_crop_center_*` → total region up to **500×500**.
pub const COMPUTER_USE_POINT_CROP_HALF_DEFAULT: u32 = 250;

/// Minimum **half** extent for point crop (native px) — total region **≥ 128×128** when the display is large enough.
pub const COMPUTER_USE_POINT_CROP_HALF_MIN: u32 = 64;

/// Maximum **half** extent for point crop (native px). Historically capped at
/// 250 (= 500×500) to keep the "implicit confirmation" crop tight, but that
/// crop mode has been removed. The only consumer left is the focused-window
/// crop path, which legitimately needs to cover the entire window — anywhere
/// up to the full display in either dimension. Set high enough that
/// `screenshot_display`'s own per-display clamp is the effective ceiling.
pub const COMPUTER_USE_POINT_CROP_HALF_MAX: u32 = 16384;

/// Clamp optional model/host request to a valid point-crop half extent.
#[inline]
pub fn clamp_point_crop_half_extent(requested: Option<u32>) -> u32 {
    let v = requested.unwrap_or(COMPUTER_USE_POINT_CROP_HALF_DEFAULT);
    v.clamp(
        COMPUTER_USE_POINT_CROP_HALF_MIN,
        COMPUTER_USE_POINT_CROP_HALF_MAX,
    )
}

/// Suggest a tighter half-extent from AX **native** bounds size (smaller controls → smaller JPEG).
#[inline]
pub fn suggested_point_crop_half_extent_from_native_bounds(native_w: u32, native_h: u32) -> u32 {
    let max_edge = native_w.max(native_h).max(1);
    let half = max_edge.saturating_div(2).saturating_add(32);
    clamp_point_crop_half_extent(Some(half))
}

/// Snapshot of OS permissions relevant to computer use.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ComputerUsePermissionSnapshot {
    pub accessibility_granted: bool,
    pub screen_capture_granted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_note: Option<String>,
}

/// Frontmost application (for Computer use tool JSON).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ComputerUseForegroundApplication {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<i32>,
}

/// Mouse cursor position in **global** screen space (host native units, e.g. macOS Quartz points).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputerUsePointerGlobal {
    pub x: f64,
    pub y: f64,
}

/// Foreground app + pointer position after a Computer use action (best-effort per platform).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComputerUseSessionSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_application: Option<ComputerUseForegroundApplication>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_global: Option<ComputerUsePointerGlobal>,
}

/// Pixel rectangle of the **screen capture** in JPEG image coordinates (offset is zero when there is no frame padding).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComputerUseImageContentRect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

/// Approximate global screen rectangle covered by the screenshot image. Values
/// are in the same coordinate space as `ClickTarget::ScreenXy`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputerUseImageGlobalBounds {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Screenshot payload for the model and for pointer coordinate mapping.
/// The `ComputerUse` tool embeds these fields in tool-result JSON and adds **`hierarchical_navigation`**
/// (`full_display` vs `region_crop`, plus **`shortcut_policy`**).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputerScreenshot {
    /// Stable id for this exact screenshot coordinate basis. Follow-up
    /// `ClickTarget::ImageXy` / `ImageGrid` calls should pass this id so the
    /// host maps image pixels against the same frame the model saw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_id: Option<String>,
    pub bytes: Vec<u8>,
    pub mime_type: String,
    /// Dimensions of the image attached for the model (may be downscaled).
    pub image_width: u32,
    pub image_height: u32,
    /// Native capture dimensions for this display (before downscale).
    pub native_width: u32,
    pub native_height: u32,
    /// Top-left of this display in global screen space (for multi-monitor).
    pub display_origin_x: i32,
    pub display_origin_y: i32,
    /// Shrink factor for vision image vs native capture (Anthropic-style long-edge + megapixel cap).
    pub vision_scale: f64,
    /// When set, the **tip** of the drawn pointer overlay was placed at this pixel in the JPEG (`image_width` x `image_height`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_image_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_image_y: Option<i32>,
    /// When set, this JPEG is a crop around this center in **full-display native** pixels (see tool docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_crop_center: Option<ScreenshotCropCenter>,
    /// Half extent used for this point crop (native px); omitted when not a point crop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_crop_half_extent_native: Option<u32>,
    /// Native rectangle corresponding to this JPEG’s content (full display, quadrant drill region, or point-crop bounds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation_native_rect: Option<ComputerUseNavigationRect>,
    /// When true (desktop), `click` is allowed on this frame without an extra ~500×500 point crop — region is small enough for pointer positioning + `click`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub quadrant_navigation_click_ready: bool,
    /// Screen capture rectangle in JPEG pixel coordinates (offset zero when there is no frame padding); `ComputerUseMousePrecise` maps this rect to the display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_content_rect: Option<ComputerUseImageContentRect>,
    /// Approximate global screen rectangle represented by the screenshot. Use
    /// `ClickTarget::ImageXy` when clicking from the attached image; this field
    /// is a human/model hint and the host uses its precise internal map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_global_bounds: Option<ComputerUseImageGlobalBounds>,
    /// Condensed text representation of the UI tree, focusing on interactive elements (inspired by TuriX-CUA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_tree_text: Option<String>,
    /// Desktop: this JPEG was produced by implicit 500×500 confirmation crop (mouse or text focus center).
    #[serde(default, skip_serializing_if = "is_false")]
    pub implicit_confirmation_crop_applied: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Optional **global native** rectangle (same space as pointer / `display_origin` + capture) to limit
/// OCR to a screen region (e.g. one app window) and avoid matching text in other windows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OcrRegionNative {
    pub x0: i32,
    pub y0: i32,
    pub width: u32,
    pub height: u32,
}

/// A single OCR text match with global display coordinates.
/// Returned by [`ComputerUseHost::ocr_find_text_matches`].
#[derive(Debug, Clone)]
pub struct OcrTextMatch {
    pub text: String,
    pub confidence: f32,
    pub center_x: f64,
    pub center_y: f64,
    pub bounds_left: f64,
    pub bounds_top: f64,
    pub bounds_width: f64,
    pub bounds_height: f64,
}

/// Filter for native accessibility (macOS AX) BFS search — role/title/identifier substrings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiElementLocateQuery {
    #[serde(default)]
    pub title_contains: Option<String>,
    /// **Wide** text needle: matched against `title | value | description | help` of each AX node
    /// (case-insensitive substring). Use this when the on-screen visible text is not in `AXTitle`
    /// (e.g. a card whose label sits in `AXValue` of a child `AXStaticText`, or a button labelled
    /// only via `AXDescription`). Independent of `title_contains` — both can be supplied and
    /// `filter_combine` controls the boolean.
    #[serde(default)]
    pub text_contains: Option<String>,
    #[serde(default)]
    pub role_substring: Option<String>,
    #[serde(default)]
    pub identifier_contains: Option<String>,
    /// BFS depth from the application root (default 48, max 200).
    #[serde(default)]
    pub max_depth: Option<u32>,
    /// `"all"` (default): every non-empty filter must match the **same** element (AND).  
    /// `"any"`: at least one non-empty filter matches (OR) — useful when title and role are not both present on one node (e.g. search field with empty AXTitle).
    #[serde(default)]
    pub filter_combine: Option<String>,
    /// Direct AX-node-index pin from the most recent `get_app_state` snapshot for the same
    /// application. When present the host SHORT-CIRCUITS BFS and resolves the node from its
    /// per-pid cache. Always preferred over text/role filters when an `AppStateSnapshot` is
    /// available — guarantees the exact node the model already saw, not a re-ranked guess.
    #[serde(default)]
    pub node_idx: Option<u32>,
    /// Optional digest from the same `AppStateSnapshot` that produced `node_idx`. When set the
    /// host returns `AX_IDX_STALE` if the cached snapshot has rotated. Omit for a "loose" lookup.
    #[serde(default)]
    pub app_state_digest: Option<String>,
}

/// Matched element geometry from the accessibility tree: center plus **axis-aligned bounds** (four corners).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElementLocateResult {
    /// Same space as `ComputerUse` `use_screen_coordinates` / host pointer moves.
    pub global_center_x: f64,
    pub global_center_y: f64,
    /// Use with `ComputerUse` `screenshot_crop_center_x` / `y` (full-capture native indices).
    pub native_center_x: u32,
    pub native_center_y: u32,
    /// Element frame in **global** pointer space: top-left `(left, top)`, size `(width, height)`.
    /// Four corners: `(left, top)`, `(left+width, top)`, `(left, top+height)`, `(left+width, top+height)`.
    pub global_bounds_left: f64,
    pub global_bounds_top: f64,
    pub global_bounds_width: f64,
    pub global_bounds_height: f64,
    /// Tight **native** pixel bounds on the capture bitmap (full-display indices), derived from the global frame
    /// (mapping uses the display that contains the center; large spans may be approximate).
    pub native_bounds_min_x: u32,
    pub native_bounds_min_y: u32,
    pub native_bounds_max_x: u32,
    pub native_bounds_max_y: u32,
    pub matched_role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_identifier: Option<String>,
    /// Parent element role + title for disambiguation (e.g. "AXWindow: Settings").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_context: Option<String>,
    /// Total number of elements that matched the query (before ranking).
    /// If > 1, the model should consider whether this is the right one.
    #[serde(default)]
    pub total_matches: u32,
    /// Brief descriptions of other matches (up to 4) for disambiguation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_matches: Vec<String>,
    /// AX-tree node index of the matched element when resolvable from the most recent
    /// `get_app_state` cache (e.g. macOS). Pass back as `node_idx` for the cheapest possible
    /// follow-up `click_element` / `locate` call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_node_idx: Option<u32>,
    /// Which filter type produced the match: one of `"node_idx" | "text_contains" |
    /// "title_contains" | "role_substring" | "identifier_contains" | "climbed"`.
    /// `"climbed"` indicates a static-text leaf was promoted to its nearest clickable ancestor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_via: Option<String>,
}

/// Hit-tested accessibility node at a global screen point (OCR disambiguation).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct OcrAccessibilityHit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_context: Option<String>,
    /// One-line summary for the model (role, title, parent).
    pub description: String,
}

#[async_trait]
pub trait ComputerUseHost: Send + Sync + std::fmt::Debug {
    async fn permission_snapshot(&self) -> BitFunResult<ComputerUsePermissionSnapshot>;

    /// Platform-specific prompt (e.g. macOS accessibility dialog).
    async fn request_accessibility_permission(&self) -> BitFunResult<()>;

    /// Open settings or trigger OS screen-capture permission flow where supported.
    async fn request_screen_capture_permission(&self) -> BitFunResult<()>;

    /// Capture the display that contains `(0,0)`. See [`ComputerUseScreenshotParams`]: point crop, optional quadrant drill, refresh, reset.
    async fn screenshot_display(
        &self,
        params: ComputerUseScreenshotParams,
    ) -> BitFunResult<ComputerScreenshot>;

    /// Full-screen capture for **UI / human verification only**. Must **not** replace
    /// `last_pointer_map`, navigation focus, or `last_screenshot_refinement` (unlike [`screenshot_display`](Self::screenshot_display)).
    /// Desktop overrides with a side-effect-free capture; default delegates to a plain full-frame `screenshot_display` (may still advance navigation on naive embedders — override on desktop).
    async fn screenshot_peek_full_display(&self) -> BitFunResult<ComputerScreenshot> {
        self.screenshot_display(ComputerUseScreenshotParams::default())
            .await
    }

    /// OCR on **raw display pixels** (no pointer overlay). Desktop captures only the relevant region:
    /// optional `region_native`, else on macOS the frontmost window from Accessibility, else the primary display.
    /// Default returns a "not implemented" error. Desktop overrides with Vision (macOS), WinRT OCR (Windows), or Tesseract (Linux).
    async fn ocr_find_text_matches(
        &self,
        text_query: &str,
        region_native: Option<OcrRegionNative>,
    ) -> BitFunResult<Vec<OcrTextMatch>> {
        let _ = (text_query, region_native);
        Err(BitFunError::tool(
            "OCR text recognition is not available on this host.".to_string(),
        ))
    }

    /// Best-effort accessibility element at a global screen point (native hit-test).
    /// Desktop uses AX (macOS) / UIA (Windows). Returns `None` when unavailable or on miss.
    async fn accessibility_hit_at_global_point(
        &self,
        _gx: f64,
        _gy: f64,
    ) -> BitFunResult<Option<OcrAccessibilityHit>> {
        Ok(None)
    }

    /// JPEG crop (no pointer overlay) around `(gx, gy)` for OCR candidate previews.
    async fn ocr_preview_crop_jpeg(
        &self,
        _gx: f64,
        _gy: f64,
        _half_extent_native: u32,
    ) -> BitFunResult<Vec<u8>> {
        Err(BitFunError::tool(
            "OCR preview crops are not available on this host.".to_string(),
        ))
    }

    /// Map `(x, y)` from the **last** screenshot's image pixel grid to global pointer pixels.
    /// Fails if no screenshot was taken in this process since startup (or since last host reset).
    fn map_image_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)>;

    /// Same as `map_image_coords_to_pointer` but **sub-point** precision (macOS: use for `ComputerUseMousePrecise`).
    fn map_image_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        let (a, b) = self.map_image_coords_to_pointer(x, y)?;
        Ok((a as f64, b as f64))
    }

    /// Map `(x, y)` with each axis in `0..=1000` to the captured display in native pointer pixels.
    /// `(0,0)` ≈ top-left of capture, `(1000,1000)` ≈ bottom-right (inclusive mapping).
    fn map_normalized_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)>;

    fn map_normalized_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        let (a, b) = self.map_normalized_coords_to_pointer(x, y)?;
        Ok((a as f64, b as f64))
    }

    /// Absolute move in host global display coordinates (on macOS: CG space, **double** precision).
    async fn mouse_move_global_f64(&self, gx: f64, gy: f64) -> BitFunResult<()> {
        self.mouse_move(gx.round() as i32, gy.round() as i32).await
    }

    async fn mouse_move(&self, x: i32, y: i32) -> BitFunResult<()>;

    /// Move the pointer by `(dx, dy)` in **global screen pixels** (same space as `ComputerUseMousePrecise` absolute).
    async fn pointer_move_relative(&self, dx: i32, dy: i32) -> BitFunResult<()>;

    /// Click at the **current** pointer position only (does not move). Use `ComputerUseMousePrecise` / `ComputerUseMouseStep` / `pointer_move_rel` first.
    /// `button`: "left" | "right" | "middle"
    /// On desktop, enforces the vision fine-screenshot guard (unlike [`mouse_click_authoritative`](Self::mouse_click_authoritative)).
    async fn mouse_click(&self, button: &str) -> BitFunResult<()>;

    /// Click at the current pointer after the host has moved it to a **trusted** target (`click_element`, `move_to_text`).
    /// Skips the vision fine-screenshot / stale-pointer guard that [`mouse_click`](Self::mouse_click) applies after a pointer move.
    /// Default: delegates to [`mouse_click`](Self::mouse_click).
    async fn mouse_click_authoritative(&self, button: &str) -> BitFunResult<()> {
        self.mouse_click(button).await
    }

    /// Press a mouse button and hold it at the current pointer position.
    /// `button`: "left" | "right" | "middle"
    async fn mouse_down(&self, _button: &str) -> BitFunResult<()> {
        Err(BitFunError::tool(
            "mouse_down is not supported on this host.".to_string(),
        ))
    }

    /// Release a mouse button at the current pointer position.
    /// `button`: "left" | "right" | "middle"
    async fn mouse_up(&self, _button: &str) -> BitFunResult<()> {
        Err(BitFunError::tool(
            "mouse_up is not supported on this host.".to_string(),
        ))
    }

    async fn scroll(&self, delta_x: i32, delta_y: i32) -> BitFunResult<()>;

    /// Press key combination; names like "command", "control", "shift", "alt", "return", "tab", "escape", "space", or single letters.
    async fn key_chord(&self, keys: Vec<String>) -> BitFunResult<()>;

    /// Type Unicode text (synthesized key events; may be imperfect for some IMEs).
    async fn type_text(&self, text: &str) -> BitFunResult<()>;

    async fn wait_ms(&self, ms: u64) -> BitFunResult<()>;

    /// Current frontmost app and global pointer position for tool-result JSON (`computer_use_context`).
    /// Default: empty. Desktop overrides with platform queries (typically after each tool action).
    async fn computer_use_session_snapshot(&self) -> ComputerUseSessionSnapshot {
        ComputerUseSessionSnapshot::default()
    }

    /// After a successful `screenshot_display`, the model may `mouse_click` (until the pointer moves again).
    fn computer_use_after_screenshot(&self) {}

    /// After `ComputerUseMousePrecise` / `ComputerUseMouseStep` / relative pointer moves: the next `mouse_click` must be preceded by a new screenshot.
    fn computer_use_after_pointer_mutation(&self) {}

    /// After `mouse_click`, require a fresh screenshot before the next click (unless pointer moved, which also invalidates).
    fn computer_use_after_click(&self) {}

    /// After a committed UI action that should be **visually confirmed** on the next `screenshot`
    /// (Cowork-style: observe → act → verify). Desktop sets a pending flag; cleared when `screenshot_display` runs.
    fn computer_use_after_committed_ui_action(&self) {}

    /// Record what the most recent action *was* (Click, Scroll, KeyChord …)
    /// so the next `interaction_state.last_mutation` reports it. Hosts that
    /// don't track this can leave the default no-op.
    fn computer_use_record_mutation(&self, _kind: ComputerUseLastMutationKind) {}

    /// After `move_to_text` positioned the pointer with **trusted global OCR coordinates** (not JPEG guesses),
    /// clear the stale-capture guard so the next **`click`** or Enter **`key_chord`** may proceed without another `screenshot`.
    fn computer_use_trust_pointer_after_ocr_move(&self) {}

    /// After `type_text`: the pointer did not move; clear the stale-capture guard so Enter **`key_chord`**
    /// is not blocked solely because of a prior click / scroll.
    fn computer_use_trust_pointer_after_text_input(&self) {}

    /// Refuse `mouse_click` if the pointer moved (or a click happened) since the last screenshot,
    /// or if the latest capture is not a valid “fine” basis (desktop: ~500×500 point crop **or**
    /// quadrant navigation region with longest side < [`COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE`]).
    fn computer_use_guard_click_allowed(&self) -> BitFunResult<()> {
        Ok(())
    }

    /// Relaxed click guard for AX-based `click_element`: skips the fine-screenshot requirement.
    /// AX coordinates are authoritative, so no quadrant drill or point crop is needed.
    fn computer_use_guard_click_allowed_relaxed(&self) -> BitFunResult<()> {
        Ok(())
    }

    /// What the **last** `screenshot_display` captured (e.g. coordinate hints for the model).
    /// Default: unknown (`None`). Desktop sets after each `screenshot_display`.
    fn last_screenshot_refinement(&self) -> Option<ComputerUseScreenshotRefinement> {
        None
    }

    /// Derive structured interaction readiness and guidance from the current session state.
    /// Default: empty/default state. Desktop overrides with state-driven implementation.
    fn computer_use_interaction_state(&self) -> ComputerUseInteractionState {
        ComputerUseInteractionState::default()
    }

    /// Search the frontmost app’s accessibility tree (macOS AX) for a matching control and return a stable center.
    /// Default: unsupported outside the desktop host / non-macOS.
    async fn locate_ui_element_screen_center(
        &self,
        _query: UiElementLocateQuery,
    ) -> BitFunResult<UiElementLocateResult> {
        Err(BitFunError::tool(
            "Native UI element (accessibility) lookup is not available on this host.".to_string(),
        ))
    }

    /// Enumerate the condensed UI tree text representation for the screenshot context.
    /// Default: no UI tree text.
    async fn enumerate_ui_tree_text(&self) -> Option<String> {
        None
    }

    /// Record a completed action for loop detection and history tracking.
    /// Default: no-op. Desktop host overrides with optimizer integration.
    fn record_action(&self, _action_type: &str, _action_params: &str, _success: bool) {}

    /// Update the screenshot hash for visual change detection.
    /// Default: no-op. Desktop host overrides with optimizer integration.
    fn update_screenshot_hash(&self, _hash: u64) {}

    /// Check if the agent is stuck in a repeating action loop.
    /// Returns a detection result with suggestions if a loop is found.
    /// Default: no loop detected.
    fn detect_action_loop(&self) -> LoopDetectionResult {
        LoopDetectionResult {
            is_loop: false,
            pattern_length: 0,
            repetitions: 0,
            suggestion: String::new(),
        }
    }

    /// Get action history for context and backtracking.
    /// Default: empty history.
    fn get_action_history(&self) -> Vec<ActionRecord> {
        vec![]
    }

    /// Launch a macOS/Windows/Linux application by name and return its PID.
    /// Default: unsupported. Desktop host overrides with platform-specific implementation.
    async fn open_app(&self, _app_name: &str) -> BitFunResult<OpenAppResult> {
        Err(BitFunError::tool(
            "open_app is not available on this host.".to_string(),
        ))
    }

    /// Enumerate all physical displays attached to the host. The returned
    /// list is what the model sees in `interaction_state.displays` and what
    /// `ControlHub` exposes via `desktop.list_displays`.
    ///
    /// Default: empty (non-desktop hosts can't enumerate displays).
    async fn list_displays(&self) -> BitFunResult<Vec<ComputerUseDisplayInfo>> {
        Ok(vec![])
    }

    /// Pin subsequent screenshots / clicks / locates to the display with
    /// `display_id`. Pass `None` to clear the preference and fall back to
    /// "screen under the pointer". Hosts that don't track a preferred
    /// display can leave the default no-op.
    ///
    /// This is the explicit fix for the original bug — instead of guessing
    /// the target display from the cursor (which is wrong whenever the user
    /// has the keyboard focus on a different screen), the model can
    /// announce "I am working on display N" and the host will commit to it.
    async fn focus_display(&self, _display_id: Option<u32>) -> BitFunResult<()> {
        Err(BitFunError::tool(
            "focus_display is not available on this host.".to_string(),
        ))
    }

    /// Currently pinned display id, if any. Surfaced to the model via
    /// `interaction_state.active_display_id`.
    fn focused_display_id(&self) -> Option<u32> {
        None
    }

    // -------------------------------------------------------------------
    // Codex-style AX-first desktop API (Phase 1: trait surface only).
    //
    // All methods default to `not available` so existing platform hosts
    // (macOS/Linux/Windows desktop, headless test hosts) continue to
    // compile and behave exactly as before. Concrete implementations are
    // landed in subsequent phases (macos_ax_dump, desktop_host PID-events,
    // linux/windows AT-SPI/UIA, ControlHub dispatch).
    // -------------------------------------------------------------------

    /// Whether this host can dispatch synthetic input events to a target
    /// application **without** stealing the user's foreground focus or
    /// moving their physical cursor. macOS desktop will set this to true
    /// once the `CGEventPostToPid` + private-source path is wired and the
    /// startup self-check passes; non-macOS hosts stay `false` for now.
    fn supports_background_input(&self) -> bool {
        false
    }

    /// Whether this host can dump a structured accessibility tree per
    /// running application (Codex-style `<app_state>` payload). macOS uses
    /// AX, Linux uses AT-SPI2, Windows uses UIA. Hosts without an AX
    /// backend stay `false` so the model falls back to the screenshot path.
    fn supports_ax_tree(&self) -> bool {
        false
    }

    /// Enumerate running applications, sorted by recency / launch count
    /// (Codex's `list_apps`). Default: empty list — callers should treat an
    /// empty result as "not available on this host".
    async fn list_apps(&self, _include_hidden: bool) -> BitFunResult<Vec<AppInfo>> {
        Ok(vec![])
    }

    /// Dump the accessibility tree of a target application, returning a
    /// stable [`AppStateSnapshot`] (Codex's `get_app_state`). Default:
    /// unsupported. Implementations cache `idx → element` so
    /// [`Self::app_click`] etc. can address nodes by index.
    async fn get_app_state(
        &self,
        _app: AppSelector,
        _max_depth: u32,
        _focus_window_only: bool,
    ) -> BitFunResult<AppStateSnapshot> {
        Err(BitFunError::tool(
            "get_app_state is not available on this host.".to_string(),
        ))
    }

    /// Click inside a target application. When [`ClickTarget::NodeIdx`] is
    /// used, the host first tries the AX action path
    /// (`AXUIElementPerformAction`) and falls back to a PID-scoped
    /// synthetic mouse event. Returns the after-state snapshot so the
    /// model can verify the change in a single round-trip.
    async fn app_click(&self, _params: AppClickParams) -> BitFunResult<AppStateSnapshot> {
        Err(BitFunError::tool(
            "app_click is not available on this host.".to_string(),
        ))
    }

    /// Type text into a target application, optionally focusing a node
    /// first via AX `kAXValue`/`kAXFocused`. Returns the after-state.
    async fn app_type_text(
        &self,
        _app: AppSelector,
        _text: &str,
        _focus: Option<ClickTarget>,
    ) -> BitFunResult<AppStateSnapshot> {
        Err(BitFunError::tool(
            "app_type_text is not available on this host.".to_string(),
        ))
    }

    /// Scroll inside a target application; `dx`/`dy` are pixel deltas in
    /// host pointer space. Optional `focus` narrows the scroll target via
    /// AX `kAXScrollPosition`.
    async fn app_scroll(
        &self,
        _app: AppSelector,
        _focus: Option<ClickTarget>,
        _dx: i32,
        _dy: i32,
    ) -> BitFunResult<AppStateSnapshot> {
        Err(BitFunError::tool(
            "app_scroll is not available on this host.".to_string(),
        ))
    }

    /// Send a key chord (e.g. `["command", "f"]`) to a target application
    /// via PID-scoped events. Optional `focus_idx` first focuses an AX node.
    async fn app_key_chord(
        &self,
        _app: AppSelector,
        _keys: Vec<String>,
        _focus_idx: Option<u32>,
    ) -> BitFunResult<AppStateSnapshot> {
        Err(BitFunError::tool(
            "app_key_chord is not available on this host.".to_string(),
        ))
    }

    /// Poll an application's AX tree until `pred` matches or `timeout_ms`
    /// elapses. Returns the matching snapshot. Default: unsupported.
    async fn app_wait_for(
        &self,
        _app: AppSelector,
        _pred: AppWaitPredicate,
        _timeout_ms: u32,
        _poll_ms: u32,
    ) -> BitFunResult<AppStateSnapshot> {
        Err(BitFunError::tool(
            "app_wait_for is not available on this host.".to_string(),
        ))
    }

    // -------------------------------------------------------------------
    // Interactive-View (Set-of-Mark) API — TuriX-CUA inspired.
    //
    // Goal: collapse the model's "where do I click?" decision into a single
    // numeric index `i` that is rendered as a coloured numbered box on top
    // of a focused-window screenshot. The model picks `i`, the host
    // resolves it back to an authoritative AX action — no coordinate
    // guessing, no JPEG-pixel arithmetic.
    //
    // Defaults are `not available` so non-desktop / non-AX hosts continue
    // to compile and behave exactly as before.
    // -------------------------------------------------------------------

    /// Whether this host can build a Set-of-Mark interactive view (filtered
    /// AX elements + numbered overlay screenshot). Hosts without an AX
    /// backend stay `false`.
    fn supports_interactive_view(&self) -> bool {
        false
    }

    /// Build a Set-of-Mark view for the given application: filters the AX
    /// tree to interactive elements, assigns a dense `i` index per element,
    /// and overlays numbered colour-coded boxes on the focused-window
    /// screenshot. The returned [`InteractiveView`] is the **default** input
    /// surface the model should use for desktop GUI work.
    async fn build_interactive_view(
        &self,
        _app: AppSelector,
        _opts: InteractiveViewOpts,
    ) -> BitFunResult<InteractiveView> {
        Err(BitFunError::tool(
            "build_interactive_view is not available on this host.".to_string(),
        ))
    }

    /// Click an element by its [`InteractiveElement::i`] index from the most
    /// recent [`InteractiveView`] of the same application. Returns the
    /// after-state view (re-built post-action) when `return_view=true`, else
    /// just the bare [`AppStateSnapshot`] for cheaper polling.
    async fn interactive_click(
        &self,
        _app: AppSelector,
        _params: InteractiveClickParams,
    ) -> BitFunResult<InteractiveActionResult> {
        Err(BitFunError::tool(
            "interactive_click is not available on this host.".to_string(),
        ))
    }

    /// Type text into an element by its `i` index (focuses first via AX,
    /// then dispatches PID-scoped key events / paste). When `i` is `None`,
    /// types into the currently focused element.
    async fn interactive_type_text(
        &self,
        _app: AppSelector,
        _params: InteractiveTypeTextParams,
    ) -> BitFunResult<InteractiveActionResult> {
        Err(BitFunError::tool(
            "interactive_type_text is not available on this host.".to_string(),
        ))
    }

    /// Scroll inside (or over) an element by its `i` index. Pass `i=None`
    /// to scroll over the focused window.
    async fn interactive_scroll(
        &self,
        _app: AppSelector,
        _params: InteractiveScrollParams,
    ) -> BitFunResult<InteractiveActionResult> {
        Err(BitFunError::tool(
            "interactive_scroll is not available on this host.".to_string(),
        ))
    }

    /// Whether this host can build a generic visual mark view for arbitrary
    /// non-AX/non-OCR surfaces. Unlike [`Self::build_interactive_view`], this
    /// does not require accessibility nodes; it marks candidate points in the
    /// screenshot itself.
    fn supports_visual_mark_view(&self) -> bool {
        false
    }

    async fn build_visual_mark_view(
        &self,
        _app: AppSelector,
        _opts: VisualMarkViewOpts,
    ) -> BitFunResult<VisualMarkView> {
        Err(BitFunError::tool(
            "build_visual_mark_view is not available on this host.".to_string(),
        ))
    }

    async fn visual_click(
        &self,
        _app: AppSelector,
        _params: VisualClickParams,
    ) -> BitFunResult<VisualActionResult> {
        Err(BitFunError::tool(
            "visual_click is not available on this host.".to_string(),
        ))
    }
}

// =====================================================================
// Codex-style AX-first data types (Phase 1: surface-only definitions).
// =====================================================================

/// Identifies a target application for the Codex-style `app_*` actions.
/// At least one of `name` / `bundle_id` / `pid` must be set; hosts pick
/// the most specific available (pid > bundle_id > name).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSelector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
}

impl AppSelector {
    /// Convenience: select by name only (e.g. `"Safari"`).
    pub fn by_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            bundle_id: None,
            pid: None,
        }
    }

    /// Convenience: select by pid only.
    pub fn by_pid(pid: i32) -> Self {
        Self {
            name: None,
            bundle_id: None,
            pid: Some(pid),
        }
    }

    /// Convenience: select by bundle id (macOS).
    pub fn by_bundle_id(bundle_id: impl Into<String>) -> Self {
        Self {
            name: None,
            bundle_id: Some(bundle_id.into()),
            pid: None,
        }
    }

    /// True when no selector field is populated.
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.bundle_id.is_none() && self.pid.is_none()
    }
}

/// One running application, returned by [`ComputerUseHost::list_apps`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    /// Whether the application currently has at least one running process.
    pub running: bool,
    /// Unix-epoch milliseconds of last user activation, when the host can
    /// resolve it from LaunchServices / equivalent. Used for ordering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_ms: Option<i64>,
    /// Cumulative launch count, when the host can resolve it.
    #[serde(default)]
    pub launch_count: u64,
}

/// One node of a Codex-style accessibility tree.
///
/// Indices are dense and stable **within a single
/// [`AppStateSnapshot`]** — they are only valid until the next
/// `get_app_state` / `app_*` call, after which the host re-dumps the tree
/// and assigns fresh indices. Callers that need to chain mutations should
/// use the snapshot returned from the previous mutation as the new
/// addressing basis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxNode {
    /// Stable index inside this snapshot. Zero is the application root.
    pub idx: u32,
    /// Parent index, `None` for the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_idx: Option<u32>,
    /// Native role string (e.g. macOS AX `AXButton`).
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    pub enabled: bool,
    pub focused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// Frame in **global** pointer space: `(x, y, width, height)`. `None`
    /// when the AX backend cannot resolve the position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_global: Option<(f64, f64, f64, f64)>,
    /// Names of supported AX actions (e.g. `kAXPress`, `kAXShowMenu`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// Localized role description (`AXRoleDescription` on macOS), e.g.
    /// "standard window", "close button", "scroll area", "HTML content",
    /// "tab group". Codex-style renderers prefer this over [`Self::role`]
    /// because it matches what a sighted user would call the element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_description: Option<String>,
    /// Native AX subrole (e.g. `AXCloseButton`, `AXFullScreenButton`,
    /// `AXMinimizeButton`, `AXSecureTextField`). Useful for button
    /// disambiguation when `role` is generic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    /// `AXHelp` / tooltip text — frequently the only place an icon-only
    /// button explains itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// `AXURL` for `AXWebArea` / "HTML content" nodes (e.g. Tauri
    /// `tauri://localhost`, Electron `file://…`, Safari pages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// `AXExpanded` for disclosure controls / collapsible sidebars.
    /// `Some(true)` = expanded, `Some(false)` = collapsed, `None` =
    /// attribute not exposed by the element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
}

/// Snapshot of an application's AX tree. Returned by
/// [`ComputerUseHost::get_app_state`] and as the after-state of every
/// `app_*` mutation so the model can verify changes in one round-trip.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppStateSnapshot {
    /// Identity of the captured application.
    pub app: AppInfo,
    /// Title of the focused window when `focus_window_only=true`, else
    /// the frontmost-window title (best effort).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    /// Codex-style human-readable text rendering of the tree (used in the
    /// model prompt). Indices in `tree_text` match `nodes[i].idx`.
    pub tree_text: String,
    /// Structured nodes, dense indexing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<AxNode>,
    /// Stable digest of the snapshot (lowercase hex SHA1 of the canonical
    /// node payload). Used as `before_app_state_digest` to detect "no-op"
    /// mutations and as a cheap equality check between successive
    /// snapshots.
    pub digest: String,
    /// Unix-epoch milliseconds when the snapshot was captured.
    pub captured_at_ms: u64,
    /// **Auto-attached** focused-window screenshot (Codex parity). The host
    /// captures the visible pixels of the target app's frontmost window
    /// every time `get_app_state` (or any `app_*` mutation) returns, so
    /// the model is never blind on canvas / WebView / WebGL surfaces that
    /// the AX tree cannot describe (e.g. the Gobang board). `None` only
    /// when the host explicitly opted out (e.g. inner `app_wait_for`
    /// polls) or the capture itself failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<ComputerScreenshot>,
    /// Optional per-snapshot warning emitted by the host when it detects
    /// the agent is targeting the same node / coordinate repeatedly without
    /// progress. The recommended remediation is encoded directly in the
    /// message and the model is expected to switch tactic (take a real
    /// `screenshot`, fall back to keyboard, re-locate via OCR, …) on the
    /// **very next** turn rather than retry the failing target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_warning: Option<String>,
}

// =====================================================================
// Interactive-View (Set-of-Mark) data types — TuriX-CUA inspired.
// =====================================================================

/// Options for [`ComputerUseHost::build_interactive_view`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractiveViewOpts {
    /// When `true` (default) only emit elements inside the focused window
    /// of the target application; when `false` emit every interactive
    /// element across all windows of the app (heavier overlay).
    #[serde(default = "default_focus_window_only_true")]
    pub focus_window_only: bool,
    /// Maximum number of interactive elements to include / annotate. The
    /// host trims by visual area (largest first) when exceeded so the
    /// overlay stays legible. `None` → host default (typically ~80).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_elements: Option<u32>,
    /// When `true` (default), the host paints numbered coloured boxes on a
    /// fresh focused-window screenshot. Set `false` to skip the overlay
    /// (text-only payload — cheaper, useful for retries / loop probes).
    #[serde(default = "default_annotate_true")]
    pub annotate_screenshot: bool,
    /// When `true` (default), include the compact `tree_text` rendering of
    /// the filtered elements alongside the structured `elements` array.
    #[serde(default = "default_include_tree_text_true")]
    pub include_tree_text: bool,
}

fn default_focus_window_only_true() -> bool {
    true
}
fn default_annotate_true() -> bool {
    true
}
fn default_include_tree_text_true() -> bool {
    true
}

impl Default for InteractiveViewOpts {
    fn default() -> Self {
        Self {
            focus_window_only: true,
            max_elements: None,
            annotate_screenshot: true,
            include_tree_text: true,
        }
    }
}

/// One interactive element inside an [`InteractiveView`]. The [`Self::i`]
/// field is the only handle the model is expected to use — every other
/// field is informational so the model can disambiguate between visually
/// similar boxes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractiveElement {
    /// Dense per-view index (0-based). The single source of truth the
    /// model passes back via [`ClickIndexTarget::Index`] /
    /// [`InteractiveClickParams::i`].
    pub i: u32,
    /// Underlying [`AxNode::idx`] in the snapshot embedded in this view.
    /// Hosts use this to round-trip back to existing `app_click` /
    /// `app_type_text` plumbing.
    pub node_idx: u32,
    /// Native AX role (`AXButton`, `AXTextField`, …). The overlay colour
    /// is derived from this.
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    /// Best human-readable label for the element (title → description →
    /// help → value, whichever is non-empty first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Frame in **JPEG image pixel** space of the overlay screenshot
    /// (`x, y, width, height`). When `annotate_screenshot=false` the host
    /// may return `None` for elements outside the captured window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_image: Option<(u32, u32, u32, u32)>,
    /// Frame in **global pointer** space (`x, y, width, height`). Useful
    /// for hosts that need a coordinate fallback when AX press fails.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_global: Option<(f64, f64, f64, f64)>,
    /// `true` when the element is focusable / actionable right now.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub focused: bool,
    /// Whether the host can dispatch a press via AX (vs. falling back to a
    /// pointer click).
    #[serde(default = "default_true")]
    pub ax_actionable: bool,
}

fn default_true() -> bool {
    true
}

/// Set-of-Mark interactive snapshot returned by
/// [`ComputerUseHost::build_interactive_view`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractiveView {
    /// Identity of the captured application.
    pub app: AppInfo,
    /// Title of the focused window (or `None` when the host could not
    /// resolve it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    /// Filtered + sorted interactive elements with dense `i` indices.
    pub elements: Vec<InteractiveElement>,
    /// Compact text rendering of `elements` (one element per line, prefixed
    /// with `[i] role "label"`). Empty string when
    /// `opts.include_tree_text=false`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tree_text: String,
    /// Stable lowercase-hex SHA1 over the canonical element payload.
    /// Subsequent `interactive_*` calls echo this back as
    /// `before_view_digest` so the host can detect "stale index" usage.
    pub digest: String,
    /// Unix-epoch milliseconds when the view was captured.
    pub captured_at_ms: u64,
    /// Annotated focused-window screenshot (numbered coloured boxes).
    /// `None` when `opts.annotate_screenshot=false` or the capture failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<ComputerScreenshot>,
    /// Loop / no-progress warning, mirrored from
    /// [`AppStateSnapshot::loop_warning`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_warning: Option<String>,
}

/// Where an [`ComputerUseHost::interactive_click`] should land. `Index`
/// is the canonical addressing mode; the other variants exist only so
/// hosts can transparently fall back to existing `app_click` paths when
/// AX press is rejected for a given element.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ClickIndexTarget {
    /// `i` value from [`InteractiveElement::i`].
    Index { i: u32 },
    /// Authoritative AX node index (used internally when the host falls
    /// back from a stale interactive index).
    NodeIdx { idx: u32 },
}

/// Parameters for [`ComputerUseHost::interactive_click`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractiveClickParams {
    /// Required: the `i` index from the most recent interactive view.
    pub i: u32,
    /// Echo of [`InteractiveView::digest`] so the host can detect stale
    /// indices when the UI changed between view + click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_view_digest: Option<String>,
    #[serde(default = "default_click_count_one")]
    pub click_count: u8,
    /// `"left"` / `"right"` / `"middle"`.
    #[serde(default = "default_left_button")]
    pub mouse_button: String,
    /// Modifier names (e.g. `["command"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifier_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_ms_after: Option<u32>,
    /// Whether the host should re-build the interactive view after the
    /// click (default `true` — the model gets a fresh annotated screenshot
    /// for the next turn). Set `false` when chaining many `interactive_*`
    /// calls in a row to save on overlay rendering.
    #[serde(default = "default_true")]
    pub return_view: bool,
}

fn default_click_count_one() -> u8 {
    1
}
fn default_left_button() -> String {
    "left".to_string()
}

/// Parameters for [`ComputerUseHost::interactive_type_text`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractiveTypeTextParams {
    /// `i` index of the text field. `None` types into whatever element is
    /// currently focused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i: Option<u32>,
    pub text: String,
    /// When `true`, host clears the field via `cmd+a` + `delete` (macOS)
    /// or equivalent before typing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub clear_first: bool,
    /// When `true`, host presses `return` after typing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub press_enter_after: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_view_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_ms_after: Option<u32>,
    #[serde(default = "default_true")]
    pub return_view: bool,
}

/// Parameters for [`ComputerUseHost::interactive_scroll`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractiveScrollParams {
    /// `i` index of the scroll target. `None` scrolls at pointer / focused
    /// window centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i: Option<u32>,
    /// Vertical scroll amount in lines / "wheel ticks" (positive = down).
    #[serde(default)]
    pub dy: i32,
    /// Horizontal scroll amount in lines / "wheel ticks" (positive = right).
    #[serde(default)]
    pub dx: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_view_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_ms_after: Option<u32>,
    #[serde(default = "default_true")]
    pub return_view: bool,
}

/// Result envelope for `interactive_*` actions. Always carries the bare
/// AX snapshot; the rendered [`InteractiveView`] is only populated when
/// the caller asked for it via `return_view=true`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractiveActionResult {
    pub snapshot: AppStateSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<InteractiveView>,
    /// Best-effort note about how the host actually executed the request
    /// (e.g. `"ax_press"`, `"pointer_click_fallback"`,
    /// `"index_resolved_via_node_idx"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_note: Option<String>,
}

/// Options for generic visual marking. This is intentionally UI-agnostic:
/// hosts should produce useful candidate points even when AX/OCR exposes
/// nothing, such as Canvas, games, maps, drawings, and icon-only controls.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualMarkViewOpts {
    /// Max candidate points to emit. Default keeps the overlay readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_points: Option<u32>,
    /// Optional region in screenshot image pixels to mark. When omitted,
    /// the host marks the whole app screenshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<VisualImageRegion>,
    /// Include regular grid points. Default true.
    #[serde(default = "default_true")]
    pub include_grid: bool,
}

impl Default for VisualMarkViewOpts {
    fn default() -> Self {
        Self {
            max_points: None,
            region: None,
            include_grid: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualImageRegion {
    pub x0: u32,
    pub y0: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualMark {
    pub i: u32,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_image: Option<(u32, u32, u32, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualMarkView {
    pub app: AppInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    pub marks: Vec<VisualMark>,
    pub digest: String,
    pub captured_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<ComputerScreenshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualClickParams {
    pub i: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_view_digest: Option<String>,
    #[serde(default = "default_click_count_one")]
    pub click_count: u8,
    #[serde(default = "default_left_button")]
    pub mouse_button: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifier_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_ms_after: Option<u32>,
    #[serde(default = "default_true")]
    pub return_view: bool,
}

/// Result envelope for `visual_*` actions. This mirrors
/// [`InteractiveActionResult`], but carries a [`VisualMarkView`] because the
/// addressing basis is screenshot marks rather than AX elements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualActionResult {
    pub snapshot: AppStateSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<VisualMarkView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_note: Option<String>,
}

/// Where an [`ComputerUseHost::app_click`] should land.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ClickTarget {
    /// Global screen-space coordinates (same space as `mouse_move`).
    ScreenXy { x: f64, y: f64 },
    /// Pixel coordinates in the most recent screenshot attached by
    /// `get_app_state` / `screenshot`. This is the preferred target for
    /// visual surfaces such as Canvas, SVG boards, and WebGL scenes.
    ImageXy {
        x: i32,
        y: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        screenshot_id: Option<String>,
    },
    /// Grid target inside the most recent screenshot attached by
    /// `get_app_state` / `app_click`. This is for non-text visual surfaces
    /// such as boards and canvases where a single guessed pixel is brittle.
    ///
    /// `x0/y0/width/height` describe the board/grid rectangle in screenshot
    /// image pixels. `row` and `col` are zero-based. When `intersections` is
    /// true, rows/cols are line intersections (e.g. Go/Gomoku 15x15); when
    /// false, rows/cols are cells and the click lands in the cell center.
    ImageGrid {
        x0: i32,
        y0: i32,
        width: u32,
        height: u32,
        rows: u32,
        cols: u32,
        row: u32,
        col: u32,
        #[serde(default)]
        intersections: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        screenshot_id: Option<String>,
    },
    /// Self-locating regular visual grid target. The host captures the app
    /// screenshot, detects a regular line grid, then clicks the requested
    /// row/col in the detected grid. Use when the surface is custom-drawn and
    /// the grid rectangle is not exposed by AX/OCR.
    VisualGrid {
        rows: u32,
        cols: u32,
        row: u32,
        col: u32,
        #[serde(default)]
        intersections: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wait_ms_after_detection: Option<u32>,
    },
    /// AX node addressed by index inside the most recent
    /// [`AppStateSnapshot`] for this app.
    NodeIdx { idx: u32 },
    /// OCR text needle: the host screenshots the target app, runs OCR,
    /// and clicks the centre of the highest-confidence match. Used as a
    /// fallback when the AX tree does not expose the desired element
    /// (e.g. inside a Canvas / WebGL / custom-drawn surface).
    OcrText { needle: String },
}

/// Parameters for [`ComputerUseHost::app_click`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppClickParams {
    pub app: AppSelector,
    pub target: ClickTarget,
    /// Number of clicks (1 = single, 2 = double, 3 = triple).
    #[serde(default = "AppClickParams::default_click_count")]
    pub click_count: u8,
    /// `"left"` / `"right"` / `"middle"`.
    #[serde(default = "AppClickParams::default_button")]
    pub mouse_button: String,
    /// Modifier names held during the click (e.g. `["command"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifier_keys: Vec<String>,
    /// Optional settle delay before returning the after-state screenshot.
    /// Useful for game boards, WebViews, animations, and delayed AI moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_ms_after: Option<u32>,
}

impl AppClickParams {
    fn default_click_count() -> u8 {
        1
    }
    fn default_button() -> String {
        "left".to_string()
    }
}

/// Predicate for [`ComputerUseHost::app_wait_for`].
///
/// Hosts that don't yet implement AX waiting can simply return the
/// `app_wait_for is not available` default error; consumers fall back to
/// `wait_ms` + `get_app_state`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AppWaitPredicate {
    /// Wait until the AX tree digest changes from `prev_digest`.
    DigestChanged { prev_digest: String },
    /// Wait until any node's `title` contains the given substring.
    TitleContains { needle: String },
    /// Wait until any node has the given role and `enabled == true`.
    RoleEnabled { role: String },
    /// Wait until the node identified by `idx` reports `enabled=true`.
    NodeEnabled { idx: u32 },
}

/// One physical display reported by the desktop host. Returned by
/// [`ComputerUseHost::list_displays`] and surfaced to the model in
/// `interaction_state.displays` so it can pick the right screen explicitly
/// instead of falling back to whichever screen the mouse pointer happens
/// to be on (the original "computer use 在多屏时搞错操作的屏幕" failure mode).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputerUseDisplayInfo {
    /// Stable per-session id of the display. Pass back to
    /// [`ComputerUseHost::focus_display`] to pin subsequent screenshots /
    /// clicks to this screen.
    pub display_id: u32,
    /// Whether the OS marks this as the primary display.
    pub is_primary: bool,
    /// Whether this is the display ControlHub will currently capture by
    /// default (matches the host's `preferred_display_id`, falling back to
    /// the screen under the mouse pointer if no preference is pinned).
    pub is_active: bool,
    /// Whether the cursor is on this display right now.
    pub has_pointer: bool,
    /// Top-left corner in **global** logical coordinate space.
    pub origin_x: i32,
    pub origin_y: i32,
    /// Logical (DIP) size; native pixels = logical × `scale_factor`.
    pub width_logical: u32,
    pub height_logical: u32,
    pub scale_factor: f32,
    /// Best-effort name of the foreground window's app on this display, if
    /// the host can determine it. Useful for the model to confirm it is
    /// targeting the "right" screen (e.g. the one with the editor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_app: Option<String>,
}

/// Result of launching an application via [`ComputerUseHost::open_app`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAppResult {
    pub app_name: String,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Whether the latest screenshot JPEG was the full display, a point crop, or a quadrant-drill region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputerUseScreenshotRefinement {
    FullDisplay,
    RegionAroundPoint {
        center_x: u32,
        center_y: u32,
    },
    /// Partial-screen view from hierarchical quadrant navigation.
    QuadrantNavigation {
        x0: u32,
        y0: u32,
        width: u32,
        height: u32,
        click_ready: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseInteractionScreenshotKind {
    FullDisplay,
    RegionCrop,
    QuadrantDrill,
    QuadrantTerminal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseLastMutationKind {
    Screenshot,
    PointerMove,
    Click,
    Scroll,
    KeyChord,
    TypeText,
    Wait,
    Locate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ComputerUseInteractionState {
    pub click_ready: bool,
    pub enter_ready: bool,
    pub requires_fresh_screenshot_before_click: bool,
    pub requires_fresh_screenshot_before_enter: bool,
    /// When true, the last action (click, key, typing, scroll, etc.) changed the UI; take **`screenshot`**
    /// next to **confirm** the outcome (Cowork-style verify step), ideally after **`wait`** if the UI animates.
    #[serde(default, skip_serializing_if = "is_false")]
    pub recommend_screenshot_to_verify_last_action: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_screenshot_kind: Option<ComputerUseInteractionScreenshotKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_mutation: Option<ComputerUseLastMutationKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_next_action: Option<String>,
    /// Snapshot of all displays at the time of this interaction state.
    /// The model should consult this list before issuing screen-coordinate
    /// actions on multi-monitor setups so it can disambiguate targets via
    /// `desktop.focus_display` instead of relying on cursor location.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub displays: Vec<ComputerUseDisplayInfo>,
    /// Currently pinned display id (set via `desktop.focus_display`).
    /// `None` means "fall back to whichever screen the mouse is on" — the
    /// legacy behavior, kept for compatibility but discouraged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_display_id: Option<u32>,
}

pub type ComputerUseHostRef = std::sync::Arc<dyn ComputerUseHost>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_state_serializes_expected_shape() {
        let state = ComputerUseInteractionState {
            click_ready: false,
            enter_ready: true,
            requires_fresh_screenshot_before_click: true,
            requires_fresh_screenshot_before_enter: false,
            recommend_screenshot_to_verify_last_action: true,
            last_screenshot_kind: Some(ComputerUseInteractionScreenshotKind::FullDisplay),
            last_mutation: Some(ComputerUseLastMutationKind::Screenshot),
            recommended_next_action: Some("screenshot_navigate_quadrant".to_string()),
            displays: vec![],
            active_display_id: None,
        };

        let value = serde_json::to_value(&state).expect("serialize interaction state");

        assert_eq!(value["click_ready"], serde_json::json!(false));
        assert_eq!(value["enter_ready"], serde_json::json!(true));
        assert_eq!(
            value["requires_fresh_screenshot_before_click"],
            serde_json::json!(true)
        );
        assert_eq!(
            value["requires_fresh_screenshot_before_enter"],
            serde_json::json!(false)
        );
        assert_eq!(
            value["last_screenshot_kind"],
            serde_json::json!("full_display")
        );
        assert_eq!(value["last_mutation"], serde_json::json!("screenshot"));
        assert_eq!(
            value["recommended_next_action"],
            serde_json::json!("screenshot_navigate_quadrant")
        );
        assert_eq!(
            value["recommend_screenshot_to_verify_last_action"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn app_selector_constructors_populate_only_one_field() {
        let by_name = AppSelector::by_name("Safari");
        assert_eq!(by_name.name.as_deref(), Some("Safari"));
        assert!(by_name.bundle_id.is_none() && by_name.pid.is_none());
        assert!(!by_name.is_empty());

        let empty = AppSelector::default();
        assert!(empty.is_empty());
    }

    #[test]
    fn click_target_serializes_with_kind_tag() {
        let xy = ClickTarget::ScreenXy { x: 10.5, y: 20.0 };
        let v = serde_json::to_value(&xy).expect("serialize ScreenXy");
        assert_eq!(v["kind"], "screen_xy");
        assert_eq!(v["x"], serde_json::json!(10.5));

        let image_xy = ClickTarget::ImageXy {
            x: 100,
            y: 200,
            screenshot_id: Some("shot_1".to_string()),
        };
        let v = serde_json::to_value(&image_xy).expect("serialize ImageXy");
        assert_eq!(v["kind"], "image_xy");
        assert_eq!(v["x"], serde_json::json!(100));
        assert_eq!(v["screenshot_id"], serde_json::json!("shot_1"));

        let grid = ClickTarget::ImageGrid {
            x0: 10,
            y0: 20,
            width: 300,
            height: 300,
            rows: 15,
            cols: 15,
            row: 7,
            col: 7,
            intersections: true,
            screenshot_id: Some("shot_1".to_string()),
        };
        let v = serde_json::to_value(&grid).expect("serialize ImageGrid");
        assert_eq!(v["kind"], "image_grid");
        assert_eq!(v["intersections"], serde_json::json!(true));

        let visual_grid = ClickTarget::VisualGrid {
            rows: 15,
            cols: 15,
            row: 7,
            col: 7,
            intersections: true,
            wait_ms_after_detection: None,
        };
        let v = serde_json::to_value(&visual_grid).expect("serialize VisualGrid");
        assert_eq!(v["kind"], "visual_grid");
        assert_eq!(v["rows"], serde_json::json!(15));

        let node = ClickTarget::NodeIdx { idx: 7 };
        let v = serde_json::to_value(&node).expect("serialize NodeIdx");
        assert_eq!(v["kind"], "node_idx");
        assert_eq!(v["idx"], serde_json::json!(7));

        let round_trip: ClickTarget =
            serde_json::from_value(v).expect("deserialize node_idx click target");
        assert_eq!(round_trip, ClickTarget::NodeIdx { idx: 7 });
    }

    #[test]
    fn app_click_params_apply_defaults_on_deserialize() {
        let json = serde_json::json!({
            "app": { "name": "Safari" },
            "target": { "kind": "node_idx", "idx": 3 },
        });
        let parsed: AppClickParams =
            serde_json::from_value(json).expect("deserialize minimal AppClickParams");
        assert_eq!(parsed.click_count, 1);
        assert_eq!(parsed.mouse_button, "left");
        assert!(parsed.modifier_keys.is_empty());
        assert_eq!(parsed.wait_ms_after, None);
        assert_eq!(parsed.app.name.as_deref(), Some("Safari"));
        assert_eq!(parsed.target, ClickTarget::NodeIdx { idx: 3 });
    }

    #[test]
    fn interactive_view_opts_apply_defaults_on_minimal_json() {
        let parsed: InteractiveViewOpts =
            serde_json::from_value(serde_json::json!({})).expect("deserialize empty opts");
        assert!(parsed.focus_window_only);
        assert!(parsed.annotate_screenshot);
        assert!(parsed.include_tree_text);
        assert_eq!(parsed.max_elements, None);
    }

    #[test]
    fn interactive_view_round_trips() {
        let view = InteractiveView {
            app: AppInfo {
                name: "Safari".into(),
                bundle_id: Some("com.apple.Safari".into()),
                pid: Some(123),
                running: true,
                last_used_ms: None,
                launch_count: 0,
            },
            window_title: Some("Apple".into()),
            elements: vec![InteractiveElement {
                i: 0,
                node_idx: 17,
                role: "AXButton".into(),
                subrole: Some("AXCloseButton".into()),
                label: Some("Close".into()),
                frame_image: Some((10, 20, 30, 40)),
                frame_global: Some((11.0, 21.0, 30.0, 40.0)),
                enabled: true,
                focused: false,
                ax_actionable: true,
            }],
            tree_text: "[0] AXButton \"Close\"".into(),
            digest: "abc123".into(),
            captured_at_ms: 1700000000000,
            screenshot: None,
            loop_warning: None,
        };
        let v = serde_json::to_value(&view).expect("serialize view");
        assert_eq!(v["digest"], "abc123");
        assert_eq!(v["elements"][0]["i"], 0);
        assert_eq!(v["elements"][0]["node_idx"], 17);
        let back: InteractiveView = serde_json::from_value(v).expect("deserialize view");
        assert_eq!(back, view);
    }

    #[test]
    fn click_index_target_serializes_with_kind_tag() {
        let by_idx = ClickIndexTarget::Index { i: 5 };
        let v = serde_json::to_value(&by_idx).expect("serialize");
        assert_eq!(v["kind"], "index");
        assert_eq!(v["i"], 5);
        let back: ClickIndexTarget = serde_json::from_value(v).expect("deserialize");
        assert_eq!(back, ClickIndexTarget::Index { i: 5 });

        let by_node = ClickIndexTarget::NodeIdx { idx: 9 };
        let v = serde_json::to_value(&by_node).expect("serialize");
        assert_eq!(v["kind"], "node_idx");
        assert_eq!(v["idx"], 9);
    }

    #[test]
    fn interactive_click_params_apply_defaults() {
        let parsed: InteractiveClickParams = serde_json::from_value(serde_json::json!({"i": 3}))
            .expect("deserialize minimal click params");
        assert_eq!(parsed.i, 3);
        assert_eq!(parsed.click_count, 1);
        assert_eq!(parsed.mouse_button, "left");
        assert!(parsed.modifier_keys.is_empty());
        assert!(parsed.return_view);
    }

    #[test]
    fn visual_mark_params_apply_defaults() {
        let opts: VisualMarkViewOpts =
            serde_json::from_value(serde_json::json!({})).expect("deserialize minimal opts");
        assert_eq!(opts.max_points, None);
        assert_eq!(opts.region, None);
        assert!(opts.include_grid);

        let click: VisualClickParams = serde_json::from_value(serde_json::json!({"i": 5}))
            .expect("deserialize minimal visual click params");
        assert_eq!(click.i, 5);
        assert_eq!(click.click_count, 1);
        assert_eq!(click.mouse_button, "left");
        assert!(click.modifier_keys.is_empty());
        assert!(click.return_view);
    }

    #[test]
    fn interactive_type_text_params_round_trip() {
        let params = InteractiveTypeTextParams {
            i: Some(7),
            text: "hello".into(),
            clear_first: true,
            press_enter_after: true,
            before_view_digest: Some("d".into()),
            wait_ms_after: Some(100),
            return_view: true,
        };
        let v = serde_json::to_value(&params).expect("serialize");
        let back: InteractiveTypeTextParams = serde_json::from_value(v).expect("deserialize");
        assert_eq!(back, params);
    }

    #[test]
    fn interactive_scroll_params_apply_defaults() {
        let parsed: InteractiveScrollParams = serde_json::from_value(serde_json::json!({}))
            .expect("deserialize minimal scroll params");
        assert_eq!(parsed.i, None);
        assert_eq!(parsed.dx, 0);
        assert_eq!(parsed.dy, 0);
        assert!(parsed.return_view);
    }

    #[test]
    fn app_wait_predicate_round_trips_each_variant() {
        for pred in [
            AppWaitPredicate::DigestChanged {
                prev_digest: "abc".to_string(),
            },
            AppWaitPredicate::TitleContains {
                needle: "Save".to_string(),
            },
            AppWaitPredicate::RoleEnabled {
                role: "AXButton".to_string(),
            },
            AppWaitPredicate::NodeEnabled { idx: 12 },
        ] {
            let v = serde_json::to_value(&pred).expect("serialize predicate");
            let back: AppWaitPredicate = serde_json::from_value(v).expect("deserialize predicate");
            assert_eq!(back, pred);
        }
    }
}
