//! Cross-platform `ComputerUseHost` via `screenshots` + `enigo`.

mod screenshot;
use screenshot::{ComputerUseNavFocus, PointerMap, ScreenshotCacheEntry};

use async_trait::async_trait;
use bitfun_core::agentic::tools::computer_use_host::{
    ActionRecord, AppClickParams, AppInfo, AppSelector, AppShortcutsSnapshot, AppStateSnapshot,
    AppWaitPredicate, ClickTarget, ComputerScreenshot, ComputerUseDisplayInfo, ComputerUseHost,
    ComputerUseInteractionScreenshotKind, ComputerUseInteractionState, ComputerUseLastMutationKind,
    ComputerUsePermissionSnapshot, ComputerUseScreenshotParams, ComputerUseScreenshotRefinement,
    ComputerUseSessionSnapshot, InteractiveActionResult, InteractiveClickParams,
    InteractiveScrollParams, InteractiveTypeTextParams, InteractiveView, InteractiveViewOpts,
    LoopDetectionResult, UiElementLocateQuery, UiElementLocateResult, VisualActionResult,
    VisualClickParams, VisualMark, VisualMarkView, VisualMarkViewOpts,
};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use bitfun_core::agentic::tools::computer_use_host::{
    ComputerUseForegroundApplication, ComputerUsePointerGlobal,
};
use bitfun_core::agentic::tools::computer_use_optimizer::ComputerUseOptimizer;
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use log::debug;
use screenshots::display_info::DisplayInfo;
use screenshots::Screen;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// Error text when `click_needs_fresh_screenshot` blocks `click` or Enter `key_chord` (single source of truth).
const STALE_CAPTURE_TOOL_MESSAGE: &str = "Computer use refused: call **`screenshot`** first. Use a **bare** `screenshot` (do not set `screenshot_reset_navigation`) — the host applies a **~500×500** crop around the **mouse**. Before Return/Enter in a focused text field, set **`screenshot_implicit_center`**: **`text_caret`**. This is required after the pointer moved since the last capture, before **`click`** or before **`key_chord`** that includes Return/Enter.";

static SCREENSHOT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
mod visual_grid_tests {
    use super::*;
    use bitfun_core::agentic::tools::computer_use_host::ComputerUseImageContentRect;
    use image::codecs::jpeg::JpegEncoder;
    use image::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn detects_regular_grid_rect_from_synthetic_screenshot() {
        let mut img = RgbImage::from_pixel(420, 360, Rgb([245, 245, 245]));
        let left = 60u32;
        let top = 40u32;
        let size = 280u32;
        for i in 0..15u32 {
            let pos = i * (size - 1) / 14;
            for d in 0..2 {
                let x = left + pos + d;
                if x < left + size {
                    for y in top..top + size {
                        img.put_pixel(x, y, Rgb([25, 25, 25]));
                    }
                }
                let y = top + pos + d;
                if y < top + size {
                    for x in left..left + size {
                        img.put_pixel(x, y, Rgb([25, 25, 25]));
                    }
                }
            }
        }

        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 92)
            .encode_image(&DynamicImage::ImageRgb8(img))
            .expect("encode synthetic grid");
        let shot = ComputerScreenshot {
            screenshot_id: Some("test-shot".to_string()),
            bytes,
            mime_type: "image/jpeg".to_string(),
            image_width: 420,
            image_height: 360,
            native_width: 420,
            native_height: 360,
            display_origin_x: 0,
            display_origin_y: 0,
            vision_scale: 1.0,
            pointer_image_x: None,
            pointer_image_y: None,
            screenshot_crop_center: None,
            point_crop_half_extent_native: None,
            navigation_native_rect: None,
            quadrant_navigation_click_ready: false,
            image_content_rect: Some(ComputerUseImageContentRect {
                left: 0,
                top: 0,
                width: 420,
                height: 360,
            }),
            image_global_bounds: None,
            ui_tree_text: None,
            implicit_confirmation_crop_applied: false,
        };

        let (x0, y0, width, height) =
            super::ax_orchestration::detect_regular_grid_rect_from_screenshot(&shot, 15, 15)
                .expect("detect grid");
        assert!((x0 - left as i32).abs() <= 6, "x0={x0}");
        assert!((y0 - top as i32).abs() <= 6, "y0={y0}");
        assert!((width as i32 - size as i32).abs() <= 12, "width={width}");
        assert!((height as i32 - size as i32).abs() <= 12, "height={height}");
    }
}

/// Unified mutable session state for computer use — one mutex instead of five.
/// State transitions are applied centrally after each action (screenshot, pointer move, click, etc.).
#[derive(Debug)]
struct ComputerUseSessionMutableState {
    pointer_map: Option<PointerMap>,
    /// When true, a fresh `screenshot_display` is required before `click` and before `key_chord` that sends Return/Enter
    /// (set after pointer moves / click; cleared after screenshot).
    click_needs_fresh_screenshot: bool,
    /// Last `screenshot_display` scope (full screen vs point crop) for tool hints and click rules.
    last_shot_refinement: Option<ComputerUseScreenshotRefinement>,
    /// Drill / crop context for the next `screenshot` (see [`ComputerUseNavFocus`]).
    navigation_focus: Option<ComputerUseNavFocus>,
    /// Cached full-screen screenshot for fast consecutive crops.
    screenshot_cache: Option<ScreenshotCacheEntry>,
    /// After `screenshot`, block `pointer_move_rel` / `ComputerUseMouseStep` until an absolute move
    /// from AX/OCR/globals (`mouse_move`, `move_to_text`, `click_element`) clears this.
    block_vision_pixel_nudge_after_screenshot: bool,
    /// After click / key / type / scroll / drag: recommend a **`screenshot`** to confirm UI state (Cowork verify).
    /// Cleared on the next successful `screenshot_display`.
    pending_verify_screenshot: bool,
    /// After `move_to_text` (global OCR coordinates): next guarded **`click`** may run without a prior
    /// `screenshot_display` / fine-crop basis — same idea as `click_element` relaxed guard.
    pointer_trusted_after_ocr_move: bool,
    /// Action optimizer for loop detection, history, and visual verification.
    optimizer: ComputerUseOptimizer,
    /// Most-recent action **kind** that mutated UI / pointer state. Surfaced
    /// to the model via `interaction_state.last_mutation` so it can pair the
    /// right verification step (e.g. after `Click` + `pending_verify` ⇒ take
    /// a confirming `screenshot`; after `TypeText` ⇒ may chain Enter without
    /// re-screenshotting because typing does not move the pointer).
    last_mutation_kind: Option<ComputerUseLastMutationKind>,
    /// Caller-pinned target display (set via `desktop.focus_display`).
    /// When set, all subsequent screenshots / peeks / locates use this
    /// display instead of "screen under the mouse pointer". The model
    /// uses this to disambiguate multi-monitor targets explicitly.
    preferred_display_id: Option<u32>,
    /// Most-recent Set-of-Mark interactive view per pid. Used to resolve
    /// `interactive_*` numeric `i` indices back to AX node indices and to
    /// detect stale-view usage via `before_view_digest`.
    interactive_view_cache: std::collections::HashMap<i32, CachedInteractiveView>,
    visual_mark_cache: std::collections::HashMap<i32, CachedVisualMarkView>,
    /// Most-recent focused-window screenshot coordinate map per application
    /// pid. `app_click(target: image_xy | image_grid)` must use the same
    /// image basis the model saw from `get_app_state`, not whichever global
    /// computer-use screenshot happened to run last.
    app_pointer_maps: std::collections::HashMap<i32, PointerMap>,
    /// Exact screenshot-id keyed coordinate maps. This is the strongest
    /// addressing basis for arbitrary visual targets because it survives
    /// interleaved app_state / screenshot / interactive_view calls.
    screenshot_pointer_maps: std::collections::HashMap<String, PointerMap>,
}

#[derive(Debug, Clone)]
struct CachedInteractiveView {
    digest: String,
    /// `i` → `node_idx` map (dense, indexed by `i`).
    elements: Vec<bitfun_core::agentic::tools::computer_use_host::InteractiveElement>,
}

#[derive(Debug, Clone)]
struct CachedVisualMarkView {
    digest: String,
    marks: Vec<VisualMark>,
    screenshot_id: Option<String>,
}

impl ComputerUseSessionMutableState {
    fn new() -> Self {
        Self {
            pointer_map: None,
            click_needs_fresh_screenshot: true,
            last_shot_refinement: None,
            navigation_focus: None,
            screenshot_cache: None,
            block_vision_pixel_nudge_after_screenshot: false,
            pending_verify_screenshot: false,
            pointer_trusted_after_ocr_move: false,
            optimizer: ComputerUseOptimizer::new(),
            last_mutation_kind: None,
            preferred_display_id: None,
            interactive_view_cache: std::collections::HashMap::new(),
            visual_mark_cache: std::collections::HashMap::new(),
            app_pointer_maps: std::collections::HashMap::new(),
            screenshot_pointer_maps: std::collections::HashMap::new(),
        }
    }

    /// Called after a successful screenshot capture.
    fn transition_after_screenshot(
        &mut self,
        map: PointerMap,
        refinement: ComputerUseScreenshotRefinement,
        nav_focus: Option<ComputerUseNavFocus>,
    ) {
        self.pointer_map = Some(map);
        self.last_shot_refinement = Some(refinement);
        self.navigation_focus = nav_focus;
        self.click_needs_fresh_screenshot = false;
        self.pending_verify_screenshot = false;
        self.pointer_trusted_after_ocr_move = false;
        self.block_vision_pixel_nudge_after_screenshot = true;
        self.last_mutation_kind = Some(ComputerUseLastMutationKind::Screenshot);
    }

    /// Called after pointer mutation (move, step, relative), click, scroll, key_chord, or type_text.
    fn transition_after_pointer_mutation(&mut self) {
        self.click_needs_fresh_screenshot = true;
        self.pointer_trusted_after_ocr_move = false;
        // Note: `last_mutation_kind` is set explicitly by the calling
        // action (PointerMove / Click / Scroll / KeyChord / TypeText / Drag)
        // so we do not overwrite it here with a generic value.
    }

    /// Called after click (same effect as pointer mutation for freshness).
    fn transition_after_click(&mut self) {
        self.click_needs_fresh_screenshot = true;
        self.pending_verify_screenshot = true;
        self.pointer_trusted_after_ocr_move = false;
        self.last_mutation_kind = Some(ComputerUseLastMutationKind::Click);
    }

    /// Called after key, typing, scroll, or drag — UI likely changed; next `screenshot` should confirm.
    fn transition_after_committed_ui_action(&mut self) {
        self.pending_verify_screenshot = true;
    }

    fn record_mutation(&mut self, kind: ComputerUseLastMutationKind) {
        self.last_mutation_kind = Some(kind);
    }
}

pub struct DesktopComputerUseHost {
    state: Mutex<ComputerUseSessionMutableState>,
}

impl std::fmt::Debug for DesktopComputerUseHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopComputerUseHost")
            .finish_non_exhaustive()
    }
}

impl Default for DesktopComputerUseHost {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopComputerUseHost {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ComputerUseSessionMutableState::new()),
        }
    }

    pub fn prompt_for_missing_permissions(&self) {
        self.run_background_input_self_check();
    }

    fn next_screenshot_id() -> String {
        let seq = SCREENSHOT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("shot_{}_{}", ms, seq)
    }

    /// Codex-style startup probe: log whether AX/background-input capabilities
    /// are available so operators can diagnose missing permissions early.
    ///
    /// Behaviour parity with Codex: if the process is NOT yet
    /// Accessibility-trusted, immediately call
    /// `AXIsProcessTrustedWithOptions({kAXTrustedCheckOptionPrompt: true})`
    /// once. macOS responds by surfacing the system-modal "允许 X 通过辅助功能
    /// 控制您的电脑" dialog (deep-linked to System Settings → Privacy & Security
    /// → Accessibility). Without this call, the OS NEVER prompts and AX tree
    /// reads against other apps return only the top-level window structure
    /// (root window + a few descendants) — which is exactly the "shallow tree
    /// / agent goes blind" symptom we observed against the BitFun WebView.
    fn run_background_input_self_check(&self) {
        #[cfg(target_os = "macos")]
        {
            let bg_ok = crate::computer_use::macos_bg_input::supports_background_input();
            if bg_ok {
                log::info!(
                    "AX-first computer use ready: AXIsProcessTrustedWithOptions=true; CGEventPostToPid background input enabled"
                );
            } else {
                log::warn!(
                    "AX-first computer use disabled: process is NOT marked Accessibility-trusted. Triggering one-shot system prompt via AXIsProcessTrustedWithOptions(prompt:true) so macOS surfaces the Accessibility permission dialog (deep-link: System Settings → Privacy & Security → Accessibility)."
                );
                // Fire-and-forget. The dialog is async and modal at the macOS
                // level; we do not block startup waiting for the user to
                // approve. The next CU invocation will simply succeed once
                // permission lands. Subsequent BitFun launches skip the
                // prompt because `ax_trusted()` will already be true.
                macos::request_ax_prompt();
            }
            // Same idea for Screen Recording. Without it, focused-window
            // screenshots fall back to a desktop-wallpaper placeholder, which
            // is the second half of the "blind agent" failure mode.
            if !macos::screen_capture_preflight() {
                log::warn!(
                    "Screen Recording permission missing; window screenshots will be incomplete. Triggering CGRequestScreenCaptureAccess() to surface the system prompt."
                );
                let _ = macos::request_screen_capture();
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            log::info!(
                "AX-first background input is macOS-only in this build; legacy screen-coordinate desktop actions remain available"
            );
        }
    }

    fn clear_vision_pixel_nudge_block(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.block_vision_pixel_nudge_after_screenshot = false;
        }
    }

    /// Best-effort foreground app + pointer; safe to call from `spawn_blocking`.
    fn collect_session_snapshot_sync() -> ComputerUseSessionSnapshot {
        #[cfg(target_os = "macos")]
        {
            Self::session_snapshot_macos()
        }
        #[cfg(target_os = "windows")]
        {
            Self::session_snapshot_windows()
        }
        #[cfg(target_os = "linux")]
        {
            return Self::session_snapshot_linux();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            ComputerUseSessionSnapshot::default()
        }
    }

    #[cfg(target_os = "macos")]
    fn session_snapshot_macos() -> ComputerUseSessionSnapshot {
        let pointer = macos::quartz_mouse_location()
            .ok()
            .map(|(x, y)| ComputerUsePointerGlobal { x, y });
        let foreground = Self::macos_foreground_application();
        ComputerUseSessionSnapshot {
            foreground_application: foreground,
            pointer_global: pointer,
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_foreground_application() -> Option<ComputerUseForegroundApplication> {
        let out = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", r#"tell application "System Events"
  set p to first process whose frontmost is true
  return (unix id of p as text) & "|" & (name of p) & "|" & (try (bundle identifier of p as text) on error "" end try)
end tell"#])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        let parts: Vec<&str> = s.trim().splitn(3, '|').collect();
        if parts.len() < 2 {
            return None;
        }
        let pid = parts[0].trim().parse::<i32>().ok()?;
        let name = parts[1].trim();
        let bundle = parts.get(2).map(|x| x.trim()).filter(|x| !x.is_empty());
        Some(ComputerUseForegroundApplication {
            name: Some(name.to_string()),
            bundle_id: bundle.map(|b| b.to_string()),
            process_id: Some(pid),
        })
    }

    #[cfg(target_os = "windows")]
    fn session_snapshot_windows() -> ComputerUseSessionSnapshot {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetCursorPos, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
        };

        unsafe {
            let mut pt = POINT::default();
            let pointer = if GetCursorPos(&mut pt).is_ok() {
                Some(ComputerUsePointerGlobal {
                    x: pt.x as f64,
                    y: pt.y as f64,
                })
            } else {
                None
            };

            let hwnd = GetForegroundWindow();
            let foreground = if hwnd.is_invalid() {
                None
            } else {
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                let mut buf = [0u16; 512];
                let n = GetWindowTextW(hwnd, &mut buf) as usize;
                let title = if n > 0 {
                    String::from_utf16_lossy(&buf[..n.min(512)])
                } else {
                    String::new()
                };
                Some(ComputerUseForegroundApplication {
                    name: if title.is_empty() { None } else { Some(title) },
                    bundle_id: None,
                    process_id: Some(pid as i32),
                })
            };

            ComputerUseSessionSnapshot {
                foreground_application: foreground,
                pointer_global: pointer,
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn session_snapshot_linux() -> ComputerUseSessionSnapshot {
        // Best-effort: no standard API across Wayland/X11 without extra deps.
        ComputerUseSessionSnapshot::default()
    }

    fn permission_sync() -> ComputerUsePermissionSnapshot {
        #[cfg(target_os = "windows")]
        fn is_process_elevated() -> bool {
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::Security::{
                GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
            };
            use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
            unsafe {
                let mut token = HANDLE::default();
                if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                    return false;
                }
                let mut elevation = TOKEN_ELEVATION::default();
                let mut ret_len: u32 = 0;
                let ok = GetTokenInformation(
                    token,
                    TokenElevation,
                    Some(&mut elevation as *mut _ as *mut _),
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut ret_len,
                )
                .is_ok();
                let _ = windows::Win32::Foundation::CloseHandle(token);
                ok && elevation.TokenIsElevated != 0
            }
        }

        #[cfg(target_os = "macos")]
        {
            let platform_note = if cfg!(debug_assertions) && !macos::ax_trusted() {
                Some(
                    "Development build: grant Accessibility to target/debug/bitfun-desktop (path appears in errors if mouse fails)."
                        .to_string(),
                )
            } else {
                None
            };
            ComputerUsePermissionSnapshot {
                accessibility_granted: macos::ax_trusted(),
                screen_capture_granted: macos::screen_capture_preflight(),
                platform_note,
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Phase 4: real probe instead of always returning `true`.
            // Screen capture: enumerating displays via the `screenshots` crate
            // exercises the same DXGI/GDI path used for actual capture, so a
            // failure here is a strong signal that capture won't work either
            // (e.g. running under Session 0 / blocked by group policy).
            let screen_capture_granted = DisplayInfo::all().map(|d| !d.is_empty()).unwrap_or(false);

            // Accessibility / input injection: there is no opt-in permission
            // on Windows, but UIPI silently blocks input into elevated windows
            // when we are not elevated. Detect elevation so the model can warn
            // the user instead of silently mis-clicking.
            let elevated = is_process_elevated();
            let mut notes: Vec<&'static str> = Vec::new();
            if !screen_capture_granted {
                notes.push(
                    "Screen capture probe failed: no displays enumerated (Session 0 / RDP / policy?).",
                );
            }
            if !elevated {
                notes
                    .push("Not running elevated: UIPI may block input into Administrator windows.");
            }
            ComputerUsePermissionSnapshot {
                accessibility_granted: true,
                screen_capture_granted,
                platform_note: if notes.is_empty() {
                    None
                } else {
                    Some(notes.join(" "))
                },
            }
        }
        #[cfg(target_os = "linux")]
        {
            // Phase 4: probe display server type *and* the actual capture path.
            let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
            let wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
                || session_type.eq_ignore_ascii_case("wayland");
            let x11_display = std::env::var("DISPLAY").is_ok();

            let screen_capture_granted = DisplayInfo::all().map(|d| !d.is_empty()).unwrap_or(false);

            // Global keyboard / mouse injection on Linux requires either an
            // X11 session with XTEST (`enigo` / `rdev` work) *or* uinput on
            // Wayland (root). Without DISPLAY we can't inject synthetic input
            // even on a Wayland session running XWayland.
            let accessibility_granted = if wayland { false } else { x11_display };

            let mut notes: Vec<String> = Vec::new();
            if wayland {
                notes.push(
                    "Wayland session: synthetic input is unsupported; screen capture relies on xdg-desktop-portal."
                        .to_string(),
                );
            }
            if !x11_display && !wayland {
                notes.push(
                    "DISPLAY not set: no X server reachable for input injection.".to_string(),
                );
            }
            if !screen_capture_granted {
                notes.push(
                    "Screen capture probe failed: no displays enumerated by the screenshots crate."
                        .to_string(),
                );
            }
            ComputerUsePermissionSnapshot {
                accessibility_granted,
                screen_capture_granted,
                platform_note: if notes.is_empty() {
                    None
                } else {
                    Some(notes.join(" "))
                },
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            ComputerUsePermissionSnapshot {
                accessibility_granted: false,
                screen_capture_granted: false,
                platform_note: Some("Computer use is not supported on this OS.".to_string()),
            }
        }
    }

    /// Best-effort current mouse position in global screen coordinates.
    pub(super) fn current_mouse_position() -> (f64, f64) {
        #[cfg(target_os = "macos")]
        {
            macos::quartz_mouse_location().unwrap_or((0.0, 0.0))
        }
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
            unsafe {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    (pt.x as f64, pt.y as f64)
                } else {
                    (0.0, 0.0)
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            use enigo::Mouse;
            match Self::run_enigo_job(|e| {
                e.location()
                    .map_err(|err| BitFunError::tool(format!("pointer location: {}", err)))
            }) {
                Ok((x, y)) => (x as f64, y as f64),
                Err(_) => (0.0, 0.0),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            (0.0, 0.0)
        }
    }
}

mod ax_orchestration;
/// macOS Accessibility / screen-capture permission FFI and main-thread /
/// Objective-C exception dispatch helpers. See `desktop_host/macos.rs`.
#[cfg(target_os = "macos")]
mod macos;
mod pointer_input;

impl DesktopComputerUseHost {
    /// Internal `get_app_state` that lets callers opt out of the focused-window
    /// screenshot. The public trait method always passes `capture_screenshot=true`
    /// (Codex parity). Internal re-snapshots from `app_click` / `app_type_text` /
    /// `app_scroll` / `app_key_chord` pass `false` to avoid a redundant capture
    /// — the **outer** call (e.g. the one returned to the model) gets the image.
    pub(crate) async fn get_app_state_inner(
        &self,
        app: AppSelector,
        max_depth: u32,
        focus_window_only: bool,
        capture_screenshot: bool,
    ) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            // Pre-flight: without Accessibility trust macOS silently truncates
            // the AX subtree to the top-level window/container (~7 nodes for
            // a Tauri WebView app), with no exception. The agent then has no
            // actionable widgets to act on. Fail fast with a structured
            // `[PERMISSION_DENIED]` error so the model can surface the issue
            // (and the host's startup prompt is what produces the dialog).
            macos::require_ax_trust_for(
                "After granting, retry `desktop.get_app_state` and the AX tree will include all WebView subtree nodes.",
            )?;
            let pid = resolve_pid_macos(self, &app).await?;
            let mut snap = tokio::task::spawn_blocking(move || {
                // Wrap in @try/@catch — AX APIs can throw NSException for
                // sandboxed / partially-loaded / dying processes, and an
                // unwound foreign exception aborts the whole bitfun process
                // (`Rust cannot catch foreign exceptions, aborting`).
                macos::catch_objc(|| {
                    crate::computer_use::macos_ax_dump::dump_app_ax(
                        pid,
                        crate::computer_use::macos_ax_dump::DumpOpts {
                            max_depth,
                            focus_window_only,
                            ..Default::default()
                        },
                    )
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;

            // Auto-attach focused-window screenshot. Failures are non-fatal —
            // worst case the model still has the AX tree.
            if capture_screenshot {
                let started = std::time::Instant::now();
                match self.screenshot_for_app_pid(pid).await {
                    Ok(shot) => {
                        debug!(
                            "computer_use.app_state: attached screenshot ({}x{} jpeg, {} bytes, {}ms)",
                            shot.image_width,
                            shot.image_height,
                            shot.bytes.len(),
                            started.elapsed().as_millis()
                        );
                        snap.screenshot = Some(shot);
                    }
                    Err(e) => {
                        debug!(
                            "computer_use.app_state: screenshot capture failed (non-fatal): {}",
                            e
                        );
                    }
                }
            }
            // Register the snapshot in the element-token registry so
            // subsequent `app_click` calls can resolve `s{hex}:{idx}`
            // tokens back to this snapshot's element indices.
            let reg_pid = snap.app.pid.unwrap_or(0);
            let _ = bitfun_agent_tools::element_token::global().register_snapshot(
                reg_pid,
                0,
                snap.nodes.len(),
            );
            Ok(snap)
        }
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

            let hwnd_raw = {
                let target_hwnd = if app_selector_is_unspecified(&app) {
                    unsafe { GetForegroundWindow() }
                } else {
                    let pid = resolve_pid(self, &app).await? as u32;
                    crate::computer_use::windows_list_apps::find_top_window_for_pid(pid)
                        .ok_or_else(|| {
                            BitFunError::tool(format!(
                                "APP_NOT_FOUND: no visible top-level window for pid={pid} (app={app:?})"
                            ))
                        })?
                };

                if target_hwnd.is_invalid() {
                    return Err(BitFunError::tool(
                        "No target window for get_app_state (invalid HWND).".to_string(),
                    ));
                }

                target_hwnd.0 as isize
            };

            let mut snap = tokio::task::spawn_blocking(move || {
                let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
                crate::computer_use::windows_ax_ui::get_app_state_snapshot_for_window(
                    hwnd,
                    max_depth,
                    focus_window_only,
                )
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;

            let reg_pid = snap.app.pid.unwrap_or(0);

            // Auto-attach window screenshot (Codex parity). Failures are non-fatal.
            if capture_screenshot {
                let started = std::time::Instant::now();
                match self
                    .screenshot_for_foreground_window(reg_pid, hwnd_raw)
                    .await
                {
                    Ok(shot) => {
                        debug!(
                            "computer_use.app_state: attached window screenshot ({}x{}, {} bytes, {}ms)",
                            shot.image_width,
                            shot.image_height,
                            shot.bytes.len(),
                            started.elapsed().as_millis()
                        );
                        snap.screenshot = Some(shot);
                    }
                    Err(e) => {
                        debug!(
                            "computer_use.app_state: window screenshot failed (non-fatal): {}",
                            e
                        );
                    }
                }
            }

            // Register snapshot in element-token registry.
            let _ = bitfun_agent_tools::element_token::global().register_snapshot(
                reg_pid,
                0,
                snap.nodes.len(),
            );
            Ok(snap)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, max_depth, focus_window_only, capture_screenshot);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    /// Enumerate the registered keyboard shortcuts in `app`'s menu bar.
    /// Read-only counterpart to `key_chord` / `app_key_chord` (which only
    /// **send** keys). Unlike `get_app_state_inner`, macOS does not need
    /// the app to be frontmost — `AXMenuBar` is queryable from any running
    /// app's AX element — while Windows resolves a (not-necessarily-
    /// foreground) top-level window owned by the target pid.
    pub(crate) async fn get_app_shortcuts_inner(
        &self,
        app: AppSelector,
    ) -> BitFunResult<AppShortcutsSnapshot> {
        #[cfg(target_os = "macos")]
        {
            // Same `[PERMISSION_DENIED]` contract as `get_app_state_inner`
            // — without Accessibility trust, `AXMenuBar` silently returns
            // nothing rather than erroring, which would look like "this
            // app has no shortcuts" instead of "BitFun lacks permission".
            macos::require_ax_trust_for("After granting, retry `desktop.get_app_shortcuts`.")?;
            let pid = resolve_pid_macos(self, &app).await?;
            let (shortcuts, menu_items_without_shortcut) = tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| {
                    crate::computer_use::macos_ax_shortcuts::dump_app_menu_shortcuts(pid)
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;

            let captured_at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            Ok(AppShortcutsSnapshot {
                app: app_info_for_pid(self, pid).await,
                shortcuts,
                menu_items_without_shortcut,
                captured_at_ms,
            })
        }
        #[cfg(target_os = "windows")]
        {
            let pid = resolve_pid(self, &app).await? as u32;
            let hwnd_isize = crate::computer_use::windows_list_apps::find_top_window_for_pid(pid)
                .map(|h| h.0 as isize)
                .ok_or_else(|| {
                    BitFunError::tool(format!(
                        "APP_NOT_FOUND: no visible top-level window for pid={} (app={:?})",
                        pid, app
                    ))
                })?;

            let (shortcuts, menu_items_without_shortcut) = tokio::task::spawn_blocking(move || {
                let hwnd = windows::Win32::Foundation::HWND(hwnd_isize as *mut std::ffi::c_void);
                crate::computer_use::windows_ax_shortcuts::get_app_menu_shortcuts(hwnd)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;

            let captured_at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            Ok(AppShortcutsSnapshot {
                app: app_info_for_pid(self, pid as i32).await,
                shortcuts,
                menu_items_without_shortcut,
                captured_at_ms,
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = app;
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }
}

#[cfg(target_os = "macos")]
fn require_macos_background_input() -> BitFunResult<()> {
    if crate::computer_use::macos_bg_input::supports_background_input() {
        return Ok(());
    }
    Err(BitFunError::tool(
        "[BACKGROUND_INPUT_UNAVAILABLE] macOS Accessibility permission is required for background app input. Grant BitFun in System Settings -> Privacy & Security -> Accessibility, then retry desktop.meta/capabilities or desktop.get_app_state.".to_string(),
    ))
}

#[async_trait]
impl ComputerUseHost for DesktopComputerUseHost {
    async fn permission_snapshot(&self) -> BitFunResult<ComputerUsePermissionSnapshot> {
        Ok(tokio::task::spawn_blocking(Self::permission_sync)
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?)
    }

    fn computer_use_interaction_state(&self) -> ComputerUseInteractionState {
        let (last_ref, click_needs_fresh, pending_verify, last_mutation, preferred_display_id) = {
            let s = self.state.lock().unwrap();
            (
                s.last_shot_refinement,
                s.click_needs_fresh_screenshot,
                s.pending_verify_screenshot,
                s.last_mutation_kind.clone(),
                s.preferred_display_id,
            )
        };

        let (mouse_x, mouse_y) = Self::current_mouse_position();
        let displays = Self::enumerate_displays(preferred_display_id, mouse_x, mouse_y);
        let active_display_id = preferred_display_id.or_else(|| {
            displays
                .iter()
                .find(|d| d.has_pointer)
                .map(|d| d.display_id)
                .or_else(|| displays.iter().find(|d| d.is_primary).map(|d| d.display_id))
        });

        let (click_ready, screenshot_kind, mut recommended_next_action) =
            match last_ref {
                Some(ComputerUseScreenshotRefinement::RegionAroundPoint { .. }) => (
                    !click_needs_fresh,
                    Some(ComputerUseInteractionScreenshotKind::RegionCrop),
                    None,
                ),
                Some(ComputerUseScreenshotRefinement::QuadrantNavigation {
                    click_ready, ..
                }) if click_ready => (
                    !click_needs_fresh,
                    Some(ComputerUseInteractionScreenshotKind::QuadrantTerminal),
                    None,
                ),
                Some(ComputerUseScreenshotRefinement::QuadrantNavigation { .. }) => (
                    false,
                    Some(ComputerUseInteractionScreenshotKind::QuadrantDrill),
                    Some("screenshot_navigate_quadrant_until_click_ready".to_string()),
                ),
                Some(ComputerUseScreenshotRefinement::FullDisplay) => (
                    !click_needs_fresh,
                    Some(ComputerUseInteractionScreenshotKind::FullDisplay),
                    if click_needs_fresh {
                        Some("screenshot".to_string())
                    } else {
                        None
                    },
                ),
                None => (false, None, Some("screenshot".to_string())),
            };

        if pending_verify && recommended_next_action.is_none() {
            recommended_next_action = Some("screenshot".to_string());
        }

        ComputerUseInteractionState {
            click_ready,
            enter_ready: !click_needs_fresh,
            requires_fresh_screenshot_before_click: click_needs_fresh,
            requires_fresh_screenshot_before_enter: click_needs_fresh,
            recommend_screenshot_to_verify_last_action: pending_verify,
            last_screenshot_kind: screenshot_kind,
            last_mutation,
            recommended_next_action,
            displays,
            active_display_id,
        }
    }

    async fn request_accessibility_permission(&self) -> BitFunResult<()> {
        #[cfg(target_os = "macos")]
        {
            tokio::task::spawn_blocking(macos::request_ax_prompt)
                .await
                .map_err(|e| BitFunError::tool(e.to_string()))?;
        }
        Ok(())
    }

    async fn request_screen_capture_permission(&self) -> BitFunResult<()> {
        #[cfg(target_os = "macos")]
        {
            tokio::task::spawn_blocking(|| {
                let _ = macos::request_screen_capture();
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?;
        }
        Ok(())
    }

    async fn screenshot_display(
        &self,
        params: ComputerUseScreenshotParams,
    ) -> BitFunResult<ComputerScreenshot> {
        self.screenshot_display_impl(params).await
    }

    async fn screenshot_peek_full_display(&self) -> BitFunResult<ComputerScreenshot> {
        self.screenshot_peek_full_display_impl().await
    }

    async fn ocr_find_text_matches(
        &self,
        text_query: &str,
        region_native: Option<bitfun_core::agentic::tools::computer_use_host::OcrRegionNative>,
    ) -> BitFunResult<Vec<bitfun_core::agentic::tools::computer_use_host::OcrTextMatch>> {
        self.ocr_find_text_matches_impl(text_query, region_native)
            .await
    }

    async fn accessibility_hit_at_global_point(
        &self,
        gx: f64,
        gy: f64,
    ) -> BitFunResult<Option<bitfun_core::agentic::tools::computer_use_host::OcrAccessibilityHit>>
    {
        #[cfg(target_os = "macos")]
        {
            let hit = tokio::task::spawn_blocking(move || {
                crate::computer_use::macos_ax_ui::accessibility_hit_at_global_point(gx, gy)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?;
            return Ok(hit);
        }
        #[cfg(target_os = "windows")]
        {
            return tokio::task::spawn_blocking(move || {
                crate::computer_use::windows_ax_ui::accessibility_hit_at_global_point(gx, gy)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?;
        }
        #[cfg(target_os = "linux")]
        {
            let _ = (gx, gy);
            Ok(None)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = (gx, gy);
            Ok(None)
        }
    }

    async fn ocr_preview_crop_jpeg(
        &self,
        gx: f64,
        gy: f64,
        half_extent_native: u32,
    ) -> BitFunResult<Vec<u8>> {
        self.ocr_preview_crop_jpeg_impl(gx, gy, half_extent_native)
            .await
    }

    fn last_screenshot_refinement(&self) -> Option<ComputerUseScreenshotRefinement> {
        self.state.lock().ok().and_then(|s| s.last_shot_refinement)
    }

    async fn locate_ui_element_screen_center(
        &self,
        query: UiElementLocateQuery,
    ) -> BitFunResult<UiElementLocateResult> {
        Self::ensure_input_automation_allowed()?;
        #[cfg(target_os = "macos")]
        {
            return tokio::task::spawn_blocking(move || {
                crate::computer_use::macos_ax_ui::locate_ui_element_center(&query)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?;
        }
        #[cfg(target_os = "windows")]
        {
            return tokio::task::spawn_blocking(move || {
                crate::computer_use::windows_ax_ui::locate_ui_element_center(&query)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?;
        }
        #[cfg(target_os = "linux")]
        {
            return crate::computer_use::linux_ax_ui::locate_ui_element_center(query).await;
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err(BitFunError::tool(
                "Native UI element (accessibility) lookup is not available on this platform."
                    .to_string(),
            ))
        }
    }

    async fn enumerate_ui_tree_text(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            const UI_TREE_MAX_ELEMENTS: usize = 50;
            tokio::task::spawn_blocking(move || {
                // AX tree traversal can throw `NSException` from a misbehaving
                // frontmost app; the @try/@catch wrapper turns that into a
                // missing UI-tree text rather than crashing the whole process.
                macos::catch_objc(|| {
                    Ok(crate::computer_use::macos_ax_ui::enumerate_ui_tree_text(
                        UI_TREE_MAX_ELEMENTS,
                    ))
                })
                .unwrap_or_else(|e| {
                    debug!("UI-tree enumeration suppressed by ObjC catch: {}", e);
                    None
                })
            })
            .await
            .unwrap_or(None)
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    async fn open_app(
        &self,
        app_name: &str,
    ) -> BitFunResult<bitfun_core::agentic::tools::computer_use_host::OpenAppResult> {
        use bitfun_core::agentic::tools::computer_use_host::OpenAppResult;
        let name = app_name.to_string();

        #[cfg(target_os = "macos")]
        {
            let result = tokio::task::spawn_blocking(move || -> BitFunResult<OpenAppResult> {
                let output = std::process::Command::new("/usr/bin/osascript")
                    .args([
                        "-e",
                        &format!(
                            r#"tell application "{}" to activate
delay 1
tell application "System Events" to get unix id of first process whose frontmost is true"#,
                            name
                        ),
                    ])
                    .output()
                    .map_err(|e| BitFunError::tool(format!("open_app osascript: {}", e)))?;

                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let pid = stdout.trim().parse::<i32>().ok();
                    Ok(OpenAppResult {
                        app_name: name,
                        success: true,
                        process_id: pid,
                        error_message: None,
                    })
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Ok(OpenAppResult {
                        app_name: name,
                        success: false,
                        process_id: None,
                        error_message: Some(stderr.trim().to_string()),
                    })
                }
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            return Ok(result);
        }

        #[cfg(target_os = "windows")]
        {
            let result = tokio::task::spawn_blocking(move || -> BitFunResult<OpenAppResult> {
                let output = bitfun_core::util::process_manager::create_command("cmd")
                    .args(["/c", "start", "", &name])
                    .output()
                    .map_err(|e| BitFunError::tool(format!("open_app: {}", e)))?;
                Ok(OpenAppResult {
                    app_name: name,
                    success: output.status.success(),
                    process_id: None,
                    error_message: if output.status.success() {
                        None
                    } else {
                        Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
                    },
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            return Ok(result);
        }

        #[cfg(target_os = "linux")]
        {
            let result = tokio::task::spawn_blocking(move || -> BitFunResult<OpenAppResult> {
                let output = std::process::Command::new("xdg-open")
                    .arg(&name)
                    .output()
                    .or_else(|_| std::process::Command::new(&name).output())
                    .map_err(|e| BitFunError::tool(format!("open_app: {}", e)))?;
                Ok(OpenAppResult {
                    app_name: name,
                    success: output.status.success(),
                    process_id: None,
                    error_message: if output.status.success() {
                        None
                    } else {
                        Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
                    },
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            return Ok(result);
        }

        #[allow(unreachable_code)]
        Err(BitFunError::tool(
            "open_app is not supported on this platform.".to_string(),
        ))
    }

    fn map_image_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        self.map_image_coords_to_pointer_f64_impl(x, y)
    }

    fn map_image_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)> {
        let (gx, gy) = self.map_image_coords_to_pointer_f64(x, y)?;
        Ok((gx.round() as i32, gy.round() as i32))
    }

    fn map_normalized_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        self.map_normalized_coords_to_pointer_f64_impl(x, y)
    }

    fn map_normalized_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)> {
        let (gx, gy) = self.map_normalized_coords_to_pointer_f64(x, y)?;
        Ok((gx.round() as i32, gy.round() as i32))
    }

    async fn mouse_move_global_f64(&self, gx: f64, gy: f64) -> BitFunResult<()> {
        self.mouse_move_global_f64_impl(gx, gy).await
    }

    async fn mouse_move(&self, x: i32, y: i32) -> BitFunResult<()> {
        self.mouse_move_global_f64(x as f64, y as f64).await
    }

    async fn pointer_move_relative(&self, dx: i32, dy: i32) -> BitFunResult<()> {
        self.pointer_move_relative_impl(dx, dy).await
    }

    async fn mouse_click(&self, button: &str) -> BitFunResult<()> {
        self.mouse_click_impl(button).await
    }

    async fn mouse_click_authoritative(&self, button: &str) -> BitFunResult<()> {
        self.mouse_click_authoritative_impl(button).await
    }

    async fn mouse_down(&self, button: &str) -> BitFunResult<()> {
        self.mouse_down_impl(button).await
    }

    async fn mouse_up(&self, button: &str) -> BitFunResult<()> {
        self.mouse_up_impl(button).await
    }

    /// Press-drag-release gesture. The desktop host performs a **background**
    /// (non-disruptive) drag where supported: macOS posts `bg_drag` to the
    /// frontmost app's pid, Windows posts `post_drag_screen` to the foreground
    /// window. When the background path is unavailable it falls back to the
    /// foreground composite gesture (visible cursor movement).
    async fn drag(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        button: &str,
        duration_ms: u64,
    ) -> BitFunResult<()> {
        self.drag_impl(from, to, button, duration_ms).await
    }

    async fn scroll(&self, delta_x: i32, delta_y: i32) -> BitFunResult<()> {
        self.scroll_impl(delta_x, delta_y).await
    }

    async fn key_chord(&self, keys: Vec<String>) -> BitFunResult<()> {
        self.key_chord_impl(keys).await
    }

    async fn type_text(&self, text: &str) -> BitFunResult<()> {
        self.type_text_impl(text).await
    }

    async fn wait_ms(&self, ms: u64) -> BitFunResult<()> {
        tokio::time::sleep(Duration::from_millis(ms.max(1))).await;
        ComputerUseHost::computer_use_record_mutation(self, ComputerUseLastMutationKind::Wait);
        Ok(())
    }

    async fn computer_use_session_snapshot(&self) -> ComputerUseSessionSnapshot {
        tokio::task::spawn_blocking(Self::collect_session_snapshot_sync)
            .await
            .unwrap_or_else(|_| ComputerUseSessionSnapshot::default())
    }

    fn computer_use_after_screenshot(&self) {
        // Transition is handled centrally in screenshot_display via transition_after_screenshot.
    }

    fn computer_use_after_pointer_mutation(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.transition_after_pointer_mutation();
            // Default attribution: bare pointer mutations are pointer moves.
            // Specific mutation kinds (Scroll, KeyChord, TypeText, Drag) are
            // re-recorded by their own `computer_use_record_mutation` call
            // so the most recent kind wins.
            s.record_mutation(ComputerUseLastMutationKind::PointerMove);
        }
    }

    fn computer_use_record_mutation(&self, kind: ComputerUseLastMutationKind) {
        if let Ok(mut s) = self.state.lock() {
            s.record_mutation(kind);
        }
    }

    fn computer_use_after_click(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.transition_after_click();
        }
    }

    fn computer_use_after_committed_ui_action(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.transition_after_committed_ui_action();
        }
    }

    fn computer_use_trust_pointer_after_ocr_move(&self) {
        if let Ok(mut s) = self.state.lock() {
            // `mouse_move` already set click_needs; OCR globals are authoritative like AX.
            s.click_needs_fresh_screenshot = false;
            s.pointer_trusted_after_ocr_move = true;
        }
    }

    fn computer_use_trust_pointer_after_text_input(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.click_needs_fresh_screenshot = false;
        }
    }

    fn computer_use_guard_click_allowed(&self) -> BitFunResult<()> {
        let s = self
            .state
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        if s.click_needs_fresh_screenshot {
            return Err(BitFunError::tool(STALE_CAPTURE_TOOL_MESSAGE.to_string()));
        }
        if s.pointer_trusted_after_ocr_move {
            return Ok(());
        }
        // Crop / quadrant-drilling is gone — every screenshot is either the
        // focused window or the full display, both of which are sufficient
        // bases for a click. The only remaining guard is the cache freshness
        // check above (`click_needs_fresh_screenshot`).
        let _ = s.last_shot_refinement;
        Ok(())
    }

    fn computer_use_guard_click_allowed_relaxed(&self) -> BitFunResult<()> {
        // For AX-based click_element: we only require that no pointer mutation
        // happened since the last known state (i.e. we moved the pointer ourselves
        // inside click_element, so the flag is not set). No fine-screenshot needed.
        // This is intentionally permissive — AX coordinates are authoritative.
        Ok(())
    }

    fn record_action(&self, action_type: &str, action_params: &str, success: bool) {
        if let Ok(mut s) = self.state.lock() {
            s.optimizer
                .record_action(action_type.to_string(), action_params.to_string(), success);
        }
    }

    fn update_screenshot_hash(&self, hash: u64) {
        if let Ok(mut s) = self.state.lock() {
            s.optimizer.update_screenshot_hash(hash);
        }
    }

    fn detect_action_loop(&self) -> LoopDetectionResult {
        if let Ok(s) = self.state.lock() {
            s.optimizer.detect_loop()
        } else {
            LoopDetectionResult {
                is_loop: false,
                pattern_length: 0,
                repetitions: 0,
                suggestion: String::new(),
            }
        }
    }

    fn get_action_history(&self) -> Vec<ActionRecord> {
        if let Ok(s) = self.state.lock() {
            s.optimizer.get_history()
        } else {
            vec![]
        }
    }

    async fn list_displays(&self) -> BitFunResult<Vec<ComputerUseDisplayInfo>> {
        let preferred = self.state.lock().ok().and_then(|s| s.preferred_display_id);
        let (mx, my) = Self::current_mouse_position();
        Ok(Self::enumerate_displays(preferred, mx, my))
    }

    async fn focus_display(&self, display_id: Option<u32>) -> BitFunResult<()> {
        if let Some(id) = display_id {
            // Validate against the actual list of attached screens; rejecting
            // unknown ids early gives the model a clean error to recover from
            // (rather than silently capturing the wrong display later).
            let known = Screen::all()
                .map(|all| all.iter().any(|s| s.display_info.id == id))
                .unwrap_or(false);
            if !known {
                return Err(BitFunError::tool(format!(
                    "focus_display: unknown display_id {} (call desktop.list_displays first)",
                    id
                )));
            }
        }
        if let Ok(mut s) = self.state.lock() {
            s.preferred_display_id = display_id;
            // Pinning a new display invalidates any cached screenshot taken
            // from the old one — drop it so the next screenshot path picks
            // a fresh frame from the chosen screen.
            if display_id.is_some() {
                s.screenshot_cache = None;
                s.click_needs_fresh_screenshot = true;
            }
        }
        Ok(())
    }

    fn focused_display_id(&self) -> Option<u32> {
        self.state.lock().ok().and_then(|s| s.preferred_display_id)
    }

    // ── Codex-style AX-first desktop automation ─────────────────────────
    //
    // These override the trait defaults (which return "not available")
    // with real macOS implementations on macOS, and keep the defaults on
    // other platforms via cfg-gating.

    fn supports_background_input(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            crate::computer_use::macos_bg_input::supports_background_input()
        }
        #[cfg(target_os = "windows")]
        {
            // Windows uses PostMessageW / SendInput for background input.
            true
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            false
        }
    }

    fn supports_ax_tree(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            true
        }
        #[cfg(target_os = "windows")]
        {
            // Windows uses UI Automation (UIA) for the AX tree.
            true
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            false
        }
    }

    async fn list_apps(&self, include_hidden: bool) -> BitFunResult<Vec<AppInfo>> {
        #[cfg(target_os = "macos")]
        {
            tokio::task::spawn_blocking(move || {
                crate::computer_use::macos_list_apps::list_running_apps(include_hidden)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?
        }
        #[cfg(target_os = "windows")]
        {
            tokio::task::spawn_blocking(move || {
                crate::computer_use::windows_list_apps::list_running_apps(include_hidden)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = include_hidden;
            Ok(Vec::new())
        }
    }

    async fn get_app_state(
        &self,
        app: AppSelector,
        max_depth: u32,
        focus_window_only: bool,
    ) -> BitFunResult<AppStateSnapshot> {
        // Public path: always auto-attach a focused-window screenshot so the
        // model is never blind on Canvas / WebView / WebGL surfaces that the
        // AX tree can't describe (Codex parity — its `get_app_state` is the
        // single "eyes" of the desktop loop).
        self.get_app_state_inner(app, max_depth, focus_window_only, true)
            .await
    }

    fn supports_app_shortcuts(&self) -> bool {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            true
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            false
        }
    }

    async fn get_app_shortcuts(&self, app: AppSelector) -> BitFunResult<AppShortcutsSnapshot> {
        self.get_app_shortcuts_inner(app).await
    }

    async fn app_click(&self, params: AppClickParams) -> BitFunResult<AppStateSnapshot> {
        self.app_click_impl(params).await
    }

    async fn app_type_text(
        &self,
        app: AppSelector,
        text: &str,
        focus: Option<ClickTarget>,
    ) -> BitFunResult<AppStateSnapshot> {
        self.app_type_text_impl(app, text, focus).await
    }

    async fn app_scroll(
        &self,
        app: AppSelector,
        focus: Option<ClickTarget>,
        dx: i32,
        dy: i32,
    ) -> BitFunResult<AppStateSnapshot> {
        self.app_scroll_impl(app, focus, dx, dy).await
    }

    async fn app_key_chord(
        &self,
        app: AppSelector,
        keys: Vec<String>,
        focus_idx: Option<u32>,
    ) -> BitFunResult<AppStateSnapshot> {
        self.app_key_chord_impl(app, keys, focus_idx).await
    }

    async fn app_wait_for(
        &self,
        app: AppSelector,
        pred: AppWaitPredicate,
        timeout_ms: u32,
        poll_ms: u32,
    ) -> BitFunResult<AppStateSnapshot> {
        self.app_wait_for_impl(app, pred, timeout_ms, poll_ms).await
    }

    fn supports_interactive_view(&self) -> bool {
        cfg!(any(target_os = "macos", target_os = "windows"))
    }

    fn supports_visual_mark_view(&self) -> bool {
        cfg!(any(target_os = "macos", target_os = "windows"))
    }

    async fn build_interactive_view(
        &self,
        app: AppSelector,
        opts: InteractiveViewOpts,
    ) -> BitFunResult<InteractiveView> {
        self.build_interactive_view_impl(app, opts).await
    }

    async fn interactive_click(
        &self,
        app: AppSelector,
        params: InteractiveClickParams,
    ) -> BitFunResult<InteractiveActionResult> {
        self.interactive_click_impl(app, params).await
    }

    async fn build_visual_mark_view(
        &self,
        app: AppSelector,
        opts: VisualMarkViewOpts,
    ) -> BitFunResult<VisualMarkView> {
        self.build_visual_mark_view_impl(app, opts).await
    }

    async fn visual_click(
        &self,
        app: AppSelector,
        params: VisualClickParams,
    ) -> BitFunResult<VisualActionResult> {
        self.visual_click_impl(app, params).await
    }

    async fn interactive_type_text(
        &self,
        app: AppSelector,
        params: InteractiveTypeTextParams,
    ) -> BitFunResult<InteractiveActionResult> {
        self.interactive_type_text_impl(app, params).await
    }

    async fn interactive_scroll(
        &self,
        app: AppSelector,
        params: InteractiveScrollParams,
    ) -> BitFunResult<InteractiveActionResult> {
        self.interactive_scroll_impl(app, params).await
    }
}

/// Linux Computer Use is a **legacy compatibility layer** only: basic
/// screenshot + enigo input + AT-SPI locate/OCR. AX-first APIs (`get_app_state`,
/// `app_*`, interactive/visual views, shortcuts) are intentionally unavailable.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) const LINUX_LEGACY_AX_UNAVAILABLE: &str = "This Computer Use action requires macOS or Windows AX-first support. Linux only provides legacy screenshot, OCR locate, and pointer/keyboard input (X11 session required).";

#[cfg(target_os = "windows")]
fn app_selector_is_unspecified(app: &AppSelector) -> bool {
    app.pid.is_none() && app.name.is_none() && app.bundle_id.is_none()
}

/// Resolve an `AppSelector` to a concrete `pid`, cross-platform.
///
/// macOS: `pid > bundle_id > name`. Windows: `pid > name` (exact, then
/// substring); empty selector resolves to the foreground window's pid.
#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn resolve_pid(host: &DesktopComputerUseHost, app: &AppSelector) -> BitFunResult<i32> {
    #[cfg(target_os = "macos")]
    {
        resolve_pid_macos(host, app).await
    }
    #[cfg(target_os = "windows")]
    {
        if app_selector_is_unspecified(app) {
            return Ok(DesktopComputerUseHost::windows_foreground_pid());
        }
        if let Some(pid) = app.pid {
            return Ok(pid);
        }
        let apps = host.list_apps(true).await?;
        if let Some(name) = app.name.as_deref() {
            let needle = name.to_lowercase();
            if let Some(p) = apps
                .iter()
                .find(|a| a.name.to_lowercase() == needle)
                .and_then(|a| a.pid)
            {
                return Ok(p);
            }
            let mut candidates: Vec<&AppInfo> = apps
                .iter()
                .filter(|a| a.name.to_lowercase().contains(&needle))
                .collect();
            candidates.sort_by_key(|a| a.name.len());
            if let Some(p) = candidates.first().and_then(|a| a.pid) {
                return Ok(p);
            }
        }
        Err(BitFunError::tool(format!("APP_NOT_FOUND: {:?}", app)))
    }
}

/// Resolve an `AppSelector` to a concrete `pid` on macOS. Resolution
/// precedence (Codex parity): `pid > bundle_id > name`.
#[cfg(target_os = "macos")]
async fn resolve_pid_macos(host: &DesktopComputerUseHost, app: &AppSelector) -> BitFunResult<i32> {
    if let Some(pid) = app.pid {
        return Ok(pid);
    }
    let apps = host.list_apps(true).await?;
    if let Some(bid) = app.bundle_id.as_deref() {
        let needle = bid.to_lowercase();
        if let Some(p) = apps
            .iter()
            .find(|a| {
                a.bundle_id
                    .as_deref()
                    .map(|s| s.to_lowercase() == needle)
                    .unwrap_or(false)
            })
            .and_then(|a| a.pid)
        {
            return Ok(p);
        }
    }
    if let Some(name) = app.name.as_deref() {
        let needle = name.to_lowercase();
        // 1) Exact match against the localized application name (what the
        //    Dock / Spotlight shows, e.g. "BitFun").
        if let Some(p) = apps
            .iter()
            .find(|a| a.name.to_lowercase() == needle)
            .and_then(|a| a.pid)
        {
            return Ok(p);
        }
        // 2) Exact match against the bundle id's last segment (e.g. user
        //    asks for "BitFun" but `list_apps` returned name="bitfun-desktop"
        //    with bundle_id="ai.bitfun.desktop"). This keeps us aligned with
        //    Codex, which is robust to "Cursor" vs "com.todesktop....Cursor".
        if let Some(p) = apps
            .iter()
            .find(|a| {
                a.bundle_id
                    .as_deref()
                    .and_then(|b| b.rsplit('.').next())
                    .map(|seg| seg.to_lowercase() == needle)
                    .unwrap_or(false)
            })
            .and_then(|a| a.pid)
        {
            return Ok(p);
        }
        // 3) Substring match on either `name` or `bundle_id` (case-
        //    insensitive). Pick the shortest matching name to avoid
        //    accidentally targeting "Visual Studio Code Helper (GPU)".
        let mut candidates: Vec<&AppInfo> = apps
            .iter()
            .filter(|a| {
                a.name.to_lowercase().contains(&needle)
                    || a.bundle_id
                        .as_deref()
                        .map(|b| b.to_lowercase().contains(&needle))
                        .unwrap_or(false)
            })
            .collect();
        candidates.sort_by_key(|a| a.name.len());
        if let Some(p) = candidates.first().and_then(|a| a.pid) {
            return Ok(p);
        }
    }
    Err(BitFunError::tool(format!("APP_NOT_FOUND: {:?}", app)))
}

/// Best-effort `AppInfo` for `pid`, looked up from `list_apps`. Falls
/// back to a bare pid-only `AppInfo` when the process no longer has a
/// matching entry (e.g. it exited between resolution and this lookup) —
/// `get_app_shortcuts` should never fail just because the display name
/// couldn't be resolved.
async fn app_info_for_pid(host: &DesktopComputerUseHost, pid: i32) -> AppInfo {
    host.list_apps(true)
        .await
        .ok()
        .and_then(|apps| apps.into_iter().find(|a| a.pid == Some(pid)))
        .unwrap_or(AppInfo {
            name: String::new(),
            bundle_id: None,
            pid: Some(pid),
            running: true,
            last_used_ms: None,
            launch_count: 0,
        })
}
