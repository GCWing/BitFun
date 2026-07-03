//! Host abstraction for desktop automation (implemented in `bitfun-desktop`).

// Re-export optimizer types so downstream crates can import from computer_use_host.
pub use crate::agentic::tools::computer_use_optimizer::{ActionRecord, LoopDetectionResult};
pub use bitfun_agent_tools::computer_use::{
    clamp_point_crop_half_extent, parse_windows_accelerator_display,
    suggested_point_crop_half_extent_from_native_bounds, AppClickParams, AppInfo, AppMenuShortcut,
    AppSelector, AppShortcutsSnapshot, AppStateSnapshot, AppWaitPredicate, AxNode,
    ClickIndexTarget, ClickTarget, ComputerScreenshot, ComputerUseDisplayInfo,
    ComputerUseForegroundApplication, ComputerUseImageContentRect, ComputerUseImageGlobalBounds,
    ComputerUseImplicitScreenshotCenter, ComputerUseInteractionScreenshotKind,
    ComputerUseInteractionState, ComputerUseLastMutationKind, ComputerUseNavigateQuadrant,
    ComputerUseNavigationRect, ComputerUsePermissionSnapshot, ComputerUsePointerGlobal,
    ComputerUseScreenshotParams, ComputerUseScreenshotRefinement, ComputerUseSessionSnapshot,
    InteractiveActionResult, InteractiveClickParams, InteractiveElement, InteractiveScrollParams,
    InteractiveTypeTextParams, InteractiveView, InteractiveViewOpts, OcrAccessibilityHit,
    OcrRegionNative, OcrTextMatch, OpenAppResult, ScreenshotCropCenter, UiElementLocateQuery,
    UiElementLocateResult, VisualActionResult, VisualClickParams, VisualImageRegion, VisualMark,
    VisualMarkView, VisualMarkViewOpts, COMPUTER_USE_POINT_CROP_HALF_DEFAULT,
    COMPUTER_USE_POINT_CROP_HALF_MAX, COMPUTER_USE_POINT_CROP_HALF_MIN,
    COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE, COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
};

use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;

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

    /// Press-drag-release gesture from `from` to `to` in **global screen
    /// coordinates** with the given `button` over `duration_ms`.
    ///
    /// Default implementation composes the foreground `mouse_move_global_f64` /
    /// `mouse_down` / `mouse_up` primitives (visible cursor movement). Hosts
    /// that can drag without moving the user's cursor (e.g. the desktop host's
    /// background `PostMessage` / CGEvent paths) override this.
    async fn drag(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        button: &str,
        duration_ms: u64,
    ) -> BitFunResult<()> {
        self.mouse_move_global_f64(from.0, from.1).await?;
        self.mouse_down(button).await?;
        let half = (duration_ms / 2).min(2_000);
        if half > 0 {
            self.wait_ms(half).await?;
        }
        self.mouse_move_global_f64(to.0, to.1).await?;
        if half > 0 {
            self.wait_ms(half).await?;
        }
        self.mouse_up(button).await
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

    /// Whether this host can enumerate a target application's menu-bar
    /// keyboard shortcuts (Codex-adjacent `get_app_shortcuts`). This is the
    /// **read** counterpart to [`Self::key_chord`] / [`Self::app_key_chord`]
    /// (which only **send** keys): macOS reads `AXMenuBar` /
    /// `AXMenuItemCmd*`, Windows reads the UIA menu tree's
    /// `AcceleratorKey` property. Hosts without a menu-introspection
    /// backend stay `false`.
    fn supports_app_shortcuts(&self) -> bool {
        false
    }

    /// Enumerate the keyboard shortcuts registered in a target
    /// application's menu bar. Unlike [`Self::get_app_state`], this does
    /// **not** require the app to be frontmost on macOS (menu structure is
    /// queryable from any running app's AX element); on Windows it
    /// resolves a top-level window owned by the target app. Default:
    /// unsupported.
    async fn get_app_shortcuts(&self, _app: AppSelector) -> BitFunResult<AppShortcutsSnapshot> {
        Err(BitFunError::tool(
            "get_app_shortcuts is not available on this host.".to_string(),
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

pub type ComputerUseHostRef = std::sync::Arc<dyn ComputerUseHost>;
