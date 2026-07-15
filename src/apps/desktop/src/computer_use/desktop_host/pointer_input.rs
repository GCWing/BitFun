//! Low-level pointer (mouse) and keyboard input primitives for the desktop
//! Computer Use host: `enigo` dispatch plumbing, smooth cursor-path motion,
//! button/key name mapping, image/normalized coordinate → global-pointer
//! mapping, and the [`ComputerUseHost`] trait methods for mouse move/click/
//! drag/scroll/key-chord/type-text.
//!
//! Extracted from `desktop_host/mod.rs` (no behavior change) so the input
//! subsystem has a single, independently reviewable home instead of living
//! inline inside the multi-thousand-line host file.

#[cfg(target_os = "macos")]
use super::macos;
use super::DesktopComputerUseHost;
#[cfg(target_os = "windows")]
use bitfun_core::agentic::tools::computer_use_host::AppSelector;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use bitfun_core::agentic::tools::computer_use_host::ClickTarget;
use bitfun_core::agentic::tools::computer_use_host::{
    ComputerUseHost, ComputerUseLastMutationKind,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use log::debug;
use std::time::Duration;

/// Relative nudges (`pointer_move_rel`, `ComputerUseMouseStep`) right after a model-driven screenshot are almost always wrong when deltas are guessed from the image; block until a trusted absolute move.
const VISION_PIXEL_NUDGE_AFTER_SCREENSHOT_MSG: &str = "Computer use refused: do not use `pointer_move_rel` or `ComputerUseMouseStep` immediately after a `screenshot` — nudging from the JPEG is inaccurate. First reposition with `move_to_text`, `click_element`, `locate` + `mouse_move` (`use_screen_coordinates`: true), or `mouse_move` using globals from tool JSON; then relative nudges are allowed if still needed.";

impl DesktopComputerUseHost {
    pub(super) fn ensure_input_automation_allowed() -> BitFunResult<()> {
        #[cfg(target_os = "macos")]
        {
            if macos::ax_trusted() {
                return Ok(());
            }
            let exe = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "(unknown path)".to_string());
            Err(BitFunError::tool(format!(
                "macOS Accessibility is not enabled for this executable. System Settings > Privacy & Security > Accessibility: add and enable BitFun. Development builds use the debug binary at: {}",
                exe
            )))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(())
        }
    }

    fn with_enigo<F, T>(f: F) -> BitFunResult<T>
    where
        F: FnOnce(&mut Enigo) -> BitFunResult<T>,
    {
        Self::ensure_input_automation_allowed()?;
        let settings = Settings::default();
        let mut enigo =
            Enigo::new(&settings).map_err(|e| BitFunError::tool(format!("enigo init: {}", e)))?;
        f(&mut enigo)
    }

    /// Enigo on macOS uses Text Input Source / AppKit paths that must run on the main queue.
    /// Tokio `spawn_blocking` threads are not main; dispatch there hits `dispatch_assert_queue_fail`.
    ///
    /// On macOS, the main-queue dispatch is also wrapped in an Objective-C
    /// `@try/@catch` (via `objc2::exception::catch`) so that an `NSException`
    /// thrown by TSM / HIToolbox / AppKit during keyboard or text input is
    /// converted into a Rust error instead of propagating across the FFI
    /// boundary as a "foreign exception" — which would otherwise cause Rust's
    /// `catch_unwind` to abort the whole process (`SIGABRT`).
    pub(super) fn run_enigo_job<F, T>(job: F) -> BitFunResult<T>
    where
        F: FnOnce(&mut Enigo) -> BitFunResult<T> + Send,
        T: Send,
    {
        #[cfg(target_os = "macos")]
        {
            macos::run_on_main_for_enigo(move || Self::with_enigo(job))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::with_enigo(job)
        }
    }

    /// Absolute pointer move in Quartz global **points** with full float precision (avoids enigo integer truncation).
    #[cfg(target_os = "macos")]
    fn post_mouse_moved_cg_global(x: f64, y: f64) -> BitFunResult<()> {
        use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        use core_graphics::geometry::CGPoint;

        let source =
            CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| {
                BitFunError::tool("CGEventSource create failed (mouse_move)".to_string())
            })?;
        let pt = CGPoint { x, y };
        let ev = CGEvent::new_mouse_event(source, CGEventType::MouseMoved, pt, CGMouseButton::Left)
            .map_err(|_| BitFunError::tool("CGEvent MouseMoved failed".to_string()))?;
        ev.post(CGEventTapLocation::HID);
        Ok(())
    }

    /// Ease 0..1 for pointer paths (smooth acceleration/deceleration).
    fn smoothstep01(t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Move the pointer along a short visible path instead of warping in one event.
    #[cfg(target_os = "macos")]
    fn smooth_mouse_move_cg_global(x1: f64, y1: f64) -> BitFunResult<()> {
        const MIN_DIST: f64 = 2.5;
        const MIN_STEPS: usize = 8;
        const MAX_STEPS: usize = 85;
        const MAX_DURATION_MS: u64 = 400;

        let (x0, y0) = macos::quartz_mouse_location().unwrap_or((x1, y1));
        let dx = x1 - x0;
        let dy = y1 - y0;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < MIN_DIST {
            return Self::post_mouse_moved_cg_global(x1, y1);
        }
        let duration_ms = (70.0 + dist * 0.28).min(MAX_DURATION_MS as f64) as u64;
        let steps = ((dist / 5.5).ceil() as usize).clamp(MIN_STEPS, MAX_STEPS);
        let step_delay = Duration::from_millis((duration_ms / steps as u64).max(1));

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let te = Self::smoothstep01(t);
            let x = x0 + dx * te;
            let y = y0 + dy * te;
            Self::post_mouse_moved_cg_global(x, y)?;
            if i < steps {
                std::thread::sleep(step_delay);
            }
        }
        Ok(())
    }

    /// Windows/Linux: same smooth path using enigo absolute moves (single `Enigo` session).
    #[cfg(not(target_os = "macos"))]
    fn smooth_mouse_move_enigo_abs(x1: f64, y1: f64) -> BitFunResult<()> {
        const MIN_DIST: f64 = 2.5;
        const MIN_STEPS: usize = 8;
        const MAX_STEPS: usize = 85;
        const MAX_DURATION_MS: u64 = 400;

        Self::run_enigo_job(|e| {
            let (cx, cy) = e.location().map_err(|err| {
                BitFunError::tool(format!("smooth_mouse_move: pointer location: {}", err))
            })?;
            let x0 = cx as f64;
            let y0 = cy as f64;
            let dx = x1 - x0;
            let dy = y1 - y0;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < MIN_DIST {
                return e
                    .move_mouse(x1.round() as i32, y1.round() as i32, Coordinate::Abs)
                    .map_err(|err| BitFunError::tool(format!("mouse_move: {}", err)));
            }
            let duration_ms = (70.0 + dist * 0.28).min(MAX_DURATION_MS as f64) as u64;
            let steps = ((dist / 5.5).ceil() as usize).clamp(MIN_STEPS, MAX_STEPS);
            let step_delay = Duration::from_millis((duration_ms / steps as u64).max(1));

            for i in 1..=steps {
                let t = i as f64 / steps as f64;
                let te = Self::smoothstep01(t);
                let x = x0 + dx * te;
                let y = y0 + dy * te;
                e.move_mouse(x.round() as i32, y.round() as i32, Coordinate::Abs)
                    .map_err(|err| BitFunError::tool(format!("mouse_move: {}", err)))?;
                if i < steps {
                    std::thread::sleep(step_delay);
                }
            }
            Ok(())
        })
    }

    fn map_button(s: &str) -> BitFunResult<Button> {
        match s.to_lowercase().as_str() {
            "left" => Ok(Button::Left),
            "right" => Ok(Button::Right),
            "middle" => Ok(Button::Middle),
            _ => Err(BitFunError::tool(format!("Unknown mouse button: {}", s))),
        }
    }

    fn map_key(name: &str) -> BitFunResult<Key> {
        let n = name.to_lowercase();
        Ok(match n.as_str() {
            "command" | "meta" | "super" | "win" => Key::Meta,
            "control" | "ctrl" => Key::Control,
            "shift" => Key::Shift,
            "alt" | "option" => Key::Alt,
            "return" | "enter" => Key::Return,
            "tab" => Key::Tab,
            "escape" | "esc" => Key::Escape,
            "space" => Key::Space,
            "backspace" => Key::Backspace,
            "delete" => Key::Delete,
            "up" | "arrow_up" | "arrowup" => Key::UpArrow,
            "down" | "arrow_down" | "arrowdown" => Key::DownArrow,
            "left" | "arrow_left" | "arrowleft" => Key::LeftArrow,
            "right" | "arrow_right" | "arrowright" => Key::RightArrow,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" | "page_up" => Key::PageUp,
            "pagedown" | "page_down" => Key::PageDown,
            "capslock" | "caps_lock" => Key::CapsLock,
            "f1" => Key::F1,
            "f2" => Key::F2,
            "f3" => Key::F3,
            "f4" => Key::F4,
            "f5" => Key::F5,
            "f6" => Key::F6,
            "f7" => Key::F7,
            "f8" => Key::F8,
            "f9" => Key::F9,
            "f10" => Key::F10,
            "f11" => Key::F11,
            "f12" => Key::F12,
            s if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                Key::Unicode(c)
            }
            _ => {
                return Err(BitFunError::tool(format!("Unknown key name: {}", name)));
            }
        })
    }

    fn chord_includes_return_or_enter(keys: &[String]) -> bool {
        keys.iter()
            .any(|s| matches!(s.to_lowercase().as_str(), "return" | "enter" | "kp_enter"))
    }
}

impl DesktopComputerUseHost {
    /// Perform a physical click at the current pointer without running [`ComputerUseHost::computer_use_guard_click_allowed`].
    /// Used after `mouse_move_global_f64` when coordinates came from AX or OCR (not from vision model image coords).
    async fn mouse_click_at_current_pointer(&self, button: &str) -> BitFunResult<()> {
        let button = button.to_string();
        tokio::task::spawn_blocking(move || {
            Self::run_enigo_job(|e| {
                let b = Self::map_button(&button)?;
                e.button(b, Direction::Click)
                    .map_err(|err| BitFunError::tool(format!("click: {}", err)))
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;

        // Flash a click highlight at current pointer (macOS only, non-blocking).
        #[cfg(target_os = "macos")]
        {
            if let Ok((mx, my)) = macos::quartz_mouse_location() {
                std::thread::spawn(move || {
                    flash_click_highlight_cg(mx, my);
                });
            }
        }

        ComputerUseHost::computer_use_after_click(self);
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub(super) fn map_app_image_coords_to_pointer_f64(
        &self,
        pid: i32,
        x: i32,
        y: i32,
        screenshot_id: Option<&str>,
    ) -> BitFunResult<(f64, f64)> {
        let map = {
            let s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            screenshot_id
                .and_then(|id| s.screenshot_pointer_maps.get(id).copied())
                .or_else(|| s.app_pointer_maps.get(&pid).copied())
                .or(s.pointer_map)
        };
        let Some(map) = map else {
            return Err(BitFunError::tool(
                "No screenshot coordinate map is available for this app. Call desktop.get_app_state for the target app first, then use app_click image_xy/image_grid against that returned screenshot_id.".to_string(),
            ));
        };
        map.map_image_to_global_f64(x, y)
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub(super) fn image_grid_target_to_xy(
        target: &ClickTarget,
    ) -> BitFunResult<Option<(i32, i32)>> {
        let ClickTarget::ImageGrid {
            x0,
            y0,
            width,
            height,
            rows,
            cols,
            row,
            col,
            intersections,
            ..
        } = target
        else {
            return Ok(None);
        };

        if *width == 0 || *height == 0 || *rows == 0 || *cols == 0 {
            return Err(BitFunError::tool(
                "image_grid requires positive width, height, rows, and cols.".to_string(),
            ));
        }
        if row >= rows || col >= cols {
            return Err(BitFunError::tool(format!(
                "image_grid row/col out of range: row={} col={} for rows={} cols={}",
                row, col, rows, cols
            )));
        }

        let (fx, fy) = if *intersections {
            let denom_x = cols.saturating_sub(1).max(1) as f64;
            let denom_y = rows.saturating_sub(1).max(1) as f64;
            (
                *x0 as f64 + (*col as f64 * width.saturating_sub(1) as f64 / denom_x),
                *y0 as f64 + (*row as f64 * height.saturating_sub(1) as f64 / denom_y),
            )
        } else {
            (
                *x0 as f64 + ((*col as f64 + 0.5) * *width as f64 / *cols as f64),
                *y0 as f64 + ((*row as f64 + 0.5) * *height as f64 / *rows as f64),
            )
        };

        Ok(Some((fx.round() as i32, fy.round() as i32)))
    }
}

/// Draw a transient red highlight circle at `(gx, gy)` in CoreGraphics global coordinates (macOS).
/// Uses a CGContext overlay window approach: draws into a temporary image and posts via overlay.
/// Runs synchronously on its own thread; caller should `std::thread::spawn`.
#[cfg(target_os = "macos")]
fn flash_click_highlight_cg(gx: f64, gy: f64) {
    use core_graphics::context::CGContext;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};

    const RADIUS: f64 = 18.0;
    const BORDER_WIDTH: f64 = 3.0;
    const DURATION_MS: u64 = 600;

    let _ = std::panic::catch_unwind(|| {
        let size = (RADIUS * 2.0 + BORDER_WIDTH * 2.0).ceil() as usize;
        let ctx = CGContext::create_bitmap_context(
            None,
            size,
            size,
            8,
            size * 4,
            &core_graphics::color_space::CGColorSpace::create_device_rgb(),
            core_graphics::base::kCGImageAlphaPremultipliedLast,
        );

        ctx.set_rgb_stroke_color(1.0, 0.0, 0.0, 0.85);
        ctx.set_line_width(BORDER_WIDTH);
        let inset = BORDER_WIDTH / 2.0;
        let rect = CGRect::new(
            &CGPoint::new(inset, inset),
            &CGSize::new(size as f64 - BORDER_WIDTH, size as f64 - BORDER_WIDTH),
        );
        ctx.stroke_ellipse_in_rect(rect);

        // The bitmap is drawn; sleep then discard (the visual feedback is best-effort).
        // On macOS the actual overlay window requires AppKit; as a lightweight alternative
        // we just log the click location for debugging.
        debug!("computer_use: click highlight at ({:.0}, {:.0})", gx, gy);
        std::thread::sleep(Duration::from_millis(DURATION_MS));
    });
}

impl DesktopComputerUseHost {
    /// Owning pid of the current foreground window (Windows), `0` when unknown.
    /// Used to key pointer maps / element caches for the foreground-targeted
    /// `app_*` actions.
    #[cfg(target_os = "windows")]
    pub(super) fn windows_foreground_pid() -> i32 {
        crate::computer_use::windows_ax_ui::foreground_window_pid()
            .map(|p| p as i32)
            .unwrap_or(0)
    }

    /// Screen-space center of the foreground window (physical pixels). Used as
    /// the default scroll anchor when no explicit focus target is given.
    #[cfg(target_os = "windows")]
    pub(super) fn windows_foreground_window_center(hwnd_raw: isize) -> Option<(i32, i32)> {
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        if hwnd_raw == 0 {
            return None;
        }
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            return None;
        }
        Some(((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2))
    }

    /// Resolve a [`ClickTarget`] into a **global screen** `(x, y)` on Windows.
    ///
    /// Mirrors the macOS coordinate-resolution arm of `app_click`, but every
    /// branch targets the foreground window (Windows snapshots are always of the
    /// foreground window). Image-pixel targets are mapped through the stored
    /// pointer map; `NodeIdx` reads the node's `frame_global` center from a
    /// fresh snapshot; `OcrText` runs OCR and takes the highest-confidence match.
    #[cfg(target_os = "windows")]
    pub(super) async fn resolve_click_target_windows(
        &self,
        target: &ClickTarget,
    ) -> BitFunResult<(f64, f64)> {
        let pid = Self::windows_foreground_pid();
        match target {
            ClickTarget::ScreenXy { x, y } => Ok((*x, *y)),
            ClickTarget::ImageXy {
                x,
                y,
                screenshot_id,
            } => self.map_app_image_coords_to_pointer_f64(pid, *x, *y, screenshot_id.as_deref()),
            ClickTarget::ImageGrid { screenshot_id, .. } => {
                let (ix, iy) = Self::image_grid_target_to_xy(target)?
                    .ok_or_else(|| BitFunError::tool("invalid image_grid target".to_string()))?;
                self.map_app_image_coords_to_pointer_f64(pid, ix, iy, screenshot_id.as_deref())
            }
            ClickTarget::VisualGrid {
                rows,
                cols,
                row,
                col,
                intersections,
                wait_ms_after_detection,
            } => {
                let hwnd_raw = crate::computer_use::windows_ax_ui::foreground_window_handle();
                let shot = self.screenshot_for_foreground_window(pid, hwnd_raw).await?;
                let (x0, y0, width, height) =
                    super::ax_orchestration::detect_regular_grid_rect_from_screenshot(
                        &shot, *rows, *cols,
                    )?;
                let detected = ClickTarget::ImageGrid {
                    x0,
                    y0,
                    width,
                    height,
                    rows: *rows,
                    cols: *cols,
                    row: *row,
                    col: *col,
                    intersections: *intersections,
                    screenshot_id: shot.screenshot_id.clone(),
                };
                let (ix, iy) = Self::image_grid_target_to_xy(&detected)?.ok_or_else(|| {
                    BitFunError::tool("invalid detected visual_grid target".to_string())
                })?;
                if let Some(wait) = wait_ms_after_detection {
                    if *wait > 0 {
                        tokio::time::sleep(Duration::from_millis(*wait as u64)).await;
                    }
                }
                self.map_app_image_coords_to_pointer_f64(pid, ix, iy, shot.screenshot_id.as_deref())
            }
            ClickTarget::NodeIdx { idx } => {
                let snap = self
                    .get_app_state_inner(AppSelector::default(), 32, false, false)
                    .await?;
                let node = snap.nodes.iter().find(|n| n.idx == *idx).ok_or_else(|| {
                    BitFunError::tool(format!(
                        "AX_NODE_STALE: idx={} no longer present in app state",
                        idx
                    ))
                })?;
                let (fx, fy, fw, fh) = node.frame_global.ok_or_else(|| {
                    BitFunError::tool(format!(
                        "AX_NODE_STALE: idx={} has no frame (off-screen or window minimised)",
                        idx
                    ))
                })?;
                if fw <= 0.0 || fh <= 0.0 {
                    return Err(BitFunError::tool(format!(
                        "AX_NODE_STALE: idx={} has zero-size frame ({}x{})",
                        idx, fw, fh
                    )));
                }
                Ok((fx + fw / 2.0, fy + fh / 2.0))
            }
            ClickTarget::OcrText { needle } => {
                let matches = self.ocr_find_text_matches(needle, None).await?;
                let best = matches.into_iter().max_by(|a, b| {
                    a.confidence
                        .partial_cmp(&b.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                let m = best.ok_or_else(|| {
                    BitFunError::tool(format!("NOT_FOUND: no OCR match for needle {:?}", needle))
                })?;
                Ok((m.center_x, m.center_y))
            }
        }
    }
}

/// Inherent implementations backing the [`ComputerUseHost`] trait's pointer,
/// keyboard, and coordinate-mapping methods (see `mod.rs`'s thin trait-method
/// delegators).
impl DesktopComputerUseHost {
    pub(super) fn map_image_coords_to_pointer_f64_impl(
        &self,
        x: i32,
        y: i32,
    ) -> BitFunResult<(f64, f64)> {
        let s = self
            .state
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        let Some(map) = s.pointer_map else {
            return Err(BitFunError::tool(
                "No screenshot yet in this session: run action screenshot first, then use x,y in the screenshot image pixel grid (image_width x image_height), or set use_screen_coordinates true with global screen pixels.".to_string(),
            ));
        };
        map.map_image_to_global_f64(x, y)
    }

    pub(super) fn map_normalized_coords_to_pointer_f64_impl(
        &self,
        x: i32,
        y: i32,
    ) -> BitFunResult<(f64, f64)> {
        let s = self
            .state
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        let Some(map) = s.pointer_map else {
            return Err(BitFunError::tool(
                "No screenshot yet: run screenshot first. For coordinate_mode \"normalized\", use x and y each in 0..=1000.".to_string(),
            ));
        };
        map.map_normalized_to_global_f64(x, y)
    }

    pub(super) async fn mouse_move_global_f64_impl(&self, gx: f64, gy: f64) -> BitFunResult<()> {
        debug!(
            "computer_use: mouse_move_global_f64 smooth target ({:.2}, {:.2})",
            gx, gy
        );
        tokio::task::spawn_blocking(move || {
            #[cfg(target_os = "macos")]
            {
                Self::run_enigo_job(|_| Self::smooth_mouse_move_cg_global(gx, gy))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Self::smooth_mouse_move_enigo_abs(gx, gy)
            }
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        self.clear_vision_pixel_nudge_block();
        ComputerUseHost::computer_use_after_pointer_mutation(self);
        Ok(())
    }

    pub(super) async fn pointer_move_relative_impl(&self, dx: i32, dy: i32) -> BitFunResult<()> {
        if dx == 0 && dy == 0 {
            return Ok(());
        }

        {
            let s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            if s.block_vision_pixel_nudge_after_screenshot {
                return Err(BitFunError::tool(
                    VISION_PIXEL_NUDGE_AFTER_SCREENSHOT_MSG.to_string(),
                ));
            }
        }

        #[cfg(target_os = "macos")]
        {
            // enigo `Coordinate::Rel` uses `location()` on macOS, which mixes NSEvent + main-display
            // pixel height — not the same space as `CGEvent` / our screenshot mapping. Use Quartz
            // position + scale from the last capture (display points per screenshot pixel).
            let geo = {
                let s = self
                    .state
                    .lock()
                    .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
                let Some(map) = s.pointer_map else {
                    return Err(BitFunError::tool(
                        "Run action screenshot first: on macOS, pointer_move_relative / ComputerUseMouseStep convert pixel deltas using the last capture scale."
                            .to_string(),
                    ));
                };
                map.macos_geo.ok_or_else(|| {
                    BitFunError::tool(
                        "Pointer map missing display geometry; take a screenshot then retry."
                            .to_string(),
                    )
                })?
            };

            tokio::task::spawn_blocking(move || {
                Self::run_enigo_job(|e| {
                    let (cx, cy) = macos::quartz_mouse_location().map_err(|err| {
                        BitFunError::tool(format!("quartz pointer (relative move): {}", err))
                    })?;
                    let px_w = geo.full_px_w.max(1) as f64;
                    let px_h = geo.full_px_h.max(1) as f64;
                    let dpt_x = dx as f64 * geo.disp_w / px_w;
                    let dpt_y = dy as f64 * geo.disp_h / px_h;
                    let nx = (cx + dpt_x).round() as i32;
                    let ny = (cy + dpt_y).round() as i32;
                    e.move_mouse(nx, ny, Coordinate::Abs)
                        .map_err(|err| BitFunError::tool(format!("pointer_move_relative: {}", err)))
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            ComputerUseHost::computer_use_after_pointer_mutation(self);
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            tokio::task::spawn_blocking(move || {
                Self::run_enigo_job(|e| {
                    e.move_mouse(dx, dy, Coordinate::Rel)
                        .map_err(|err| BitFunError::tool(format!("pointer_move_relative: {}", err)))
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            ComputerUseHost::computer_use_after_pointer_mutation(self);
            Ok(())
        }
    }

    pub(super) async fn mouse_click_impl(&self, button: &str) -> BitFunResult<()> {
        debug!("computer_use: mouse_click button={}", button);
        ComputerUseHost::computer_use_guard_click_allowed(self)?;
        self.mouse_click_at_current_pointer(button).await
    }

    pub(super) async fn mouse_click_authoritative_impl(&self, button: &str) -> BitFunResult<()> {
        debug!("computer_use: mouse_click_authoritative button={}", button);
        self.mouse_click_at_current_pointer(button).await
    }

    pub(super) async fn mouse_down_impl(&self, button: &str) -> BitFunResult<()> {
        debug!("computer_use: mouse_down button={}", button);
        let button = button.to_string();
        tokio::task::spawn_blocking(move || {
            Self::run_enigo_job(|e| {
                let b = Self::map_button(&button)?;
                e.button(b, Direction::Press)
                    .map_err(|err| BitFunError::tool(format!("mouse_down: {}", err)))
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        ComputerUseHost::computer_use_after_pointer_mutation(self);
        Ok(())
    }

    pub(super) async fn mouse_up_impl(&self, button: &str) -> BitFunResult<()> {
        debug!("computer_use: mouse_up button={}", button);
        let button = button.to_string();
        tokio::task::spawn_blocking(move || {
            Self::run_enigo_job(|e| {
                let b = Self::map_button(&button)?;
                e.button(b, Direction::Release)
                    .map_err(|err| BitFunError::tool(format!("mouse_up: {}", err)))
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        ComputerUseHost::computer_use_after_pointer_mutation(self);
        Ok(())
    }

    /// Press-drag-release gesture. The desktop host performs a **background**
    /// (non-disruptive) drag where supported: macOS posts `bg_drag` to the
    /// frontmost app's pid, Windows posts `post_drag_screen` to the foreground
    /// window. When the background path is unavailable it falls back to the
    /// foreground composite gesture (visible cursor movement).
    pub(super) async fn drag_impl(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        button: &str,
        duration_ms: u64,
    ) -> BitFunResult<()> {
        debug!(
            "computer_use: drag from=({:.1},{:.1}) to=({:.1},{:.1}) button={} dur={}ms",
            from.0, from.1, to.0, to.1, button, duration_ms
        );
        // Number of intermediate move samples for a smooth drag path.
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        const DRAG_STEPS: usize = 24;

        #[cfg(target_os = "macos")]
        {
            if crate::computer_use::macos_bg_input::supports_background_input() {
                if let Some(pid) = crate::computer_use::macos_bg_input::frontmost_pid_macos() {
                    let bg_button = match button {
                        "right" => crate::computer_use::macos_bg_input::BgDragButton::Right,
                        "middle" => crate::computer_use::macos_bg_input::BgDragButton::Middle,
                        _ => crate::computer_use::macos_bg_input::BgDragButton::Left,
                    };
                    let (fx, fy) = from;
                    let (tx, ty) = to;
                    tokio::task::spawn_blocking(move || {
                        macos::catch_objc(|| {
                            let wid =
                                crate::computer_use::macos_bg_input::frontmost_window_id_for_pid(
                                    pid,
                                );
                            crate::computer_use::macos_bg_input::bg_drag(
                                pid,
                                fx,
                                fy,
                                tx,
                                ty,
                                None,
                                None,
                                wid,
                                duration_ms,
                                DRAG_STEPS,
                                &[],
                                bg_button,
                            )
                        })
                    })
                    .await
                    .map_err(|e| BitFunError::tool(e.to_string()))??;
                    return Ok(());
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let hwnd_raw = crate::computer_use::windows_ax_ui::foreground_window_handle();
            if hwnd_raw != 0 {
                let bstr = button.to_string();
                let (fx, fy) = (from.0.round() as i32, from.1.round() as i32);
                let (tx, ty) = (to.0.round() as i32, to.1.round() as i32);
                tokio::task::spawn_blocking(move || {
                    let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
                    crate::computer_use::windows_bg_input::post_drag_screen(
                        hwnd,
                        fx,
                        fy,
                        tx,
                        ty,
                        duration_ms,
                        DRAG_STEPS,
                        &bstr,
                    )
                })
                .await
                .map_err(|e| BitFunError::tool(e.to_string()))??;
                return Ok(());
            }
        }

        // Foreground fallback: visible composite gesture (default behavior).
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

    pub(super) async fn scroll_impl(&self, delta_x: i32, delta_y: i32) -> BitFunResult<()> {
        if delta_x == 0 && delta_y == 0 {
            return Ok(());
        }
        tokio::task::spawn_blocking(move || {
            Self::run_enigo_job(|e| {
                if delta_x != 0 {
                    e.scroll(delta_x, Axis::Horizontal)
                        .map_err(|err| BitFunError::tool(format!("scroll horizontal: {}", err)))?;
                }
                if delta_y != 0 {
                    e.scroll(delta_y, Axis::Vertical)
                        .map_err(|err| BitFunError::tool(format!("scroll vertical: {}", err)))?;
                }
                Ok(())
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        ComputerUseHost::computer_use_after_pointer_mutation(self);
        ComputerUseHost::computer_use_after_committed_ui_action(self);
        ComputerUseHost::computer_use_record_mutation(self, ComputerUseLastMutationKind::Scroll);
        Ok(())
    }

    pub(super) async fn key_chord_impl(&self, keys: Vec<String>) -> BitFunResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        debug!("computer_use: key_chord keys={:?}", keys);
        if Self::chord_includes_return_or_enter(&keys) {
            // Phase 1 fix: Enter/Return commits whatever has focus (form
            // submit, send-button, default action), so it is just as
            // dangerous as a `click` and must clear the **same** guard chain
            // as `click`. The previous `guard_verified_ui` only blocked
            // `click_needs_fresh_screenshot`, so a user could fire Enter
            // after a coarse full-display screenshot without ever taking
            // the required fine screenshot. Routing through
            // `computer_use_guard_click_allowed` makes the two paths
            // consistent and prevents the model from "smuggling" a click
            // through an Enter key.
            Self::computer_use_guard_click_allowed(self)?;
        }
        let keys_for_job = keys;
        tokio::task::spawn_blocking(move || {
            Self::run_enigo_job(|e| {
                let mapped: Vec<Key> = keys_for_job
                    .iter()
                    .map(|s| Self::map_key(s))
                    .collect::<BitFunResult<_>>()?;
                let chord_has_modifier = keys_for_job.iter().any(|s| {
                    matches!(
                        s.to_lowercase().as_str(),
                        "command"
                            | "meta"
                            | "super"
                            | "win"
                            | "control"
                            | "ctrl"
                            | "shift"
                            | "alt"
                            | "option"
                    )
                });
                if mapped.len() == 1 {
                    e.key(mapped[0], Direction::Click)
                        .map_err(|err| BitFunError::tool(format!("key: {}", err)))?;
                } else {
                    let mods = &mapped[..mapped.len() - 1];
                    let last = *mapped.last().unwrap();
                    for k in mods {
                        e.key(*k, Direction::Press)
                            .map_err(|err| BitFunError::tool(format!("key press: {}", err)))?;
                    }
                    if chord_has_modifier {
                        // Modifiers must be registered before the main key; otherwise macOS / IME
                        // treats the letter as plain typing (e.g. Cmd+F becomes "f" in the text box).
                        #[cfg(target_os = "macos")]
                        std::thread::sleep(std::time::Duration::from_millis(160));
                        #[cfg(not(target_os = "macos"))]
                        std::thread::sleep(std::time::Duration::from_millis(55));
                    }
                    e.key(last, Direction::Click)
                        .map_err(|err| BitFunError::tool(format!("key click: {}", err)))?;
                    for k in mods.iter().rev() {
                        e.key(*k, Direction::Release)
                            .map_err(|err| BitFunError::tool(format!("key release: {}", err)))?;
                    }
                    if chord_has_modifier {
                        std::thread::sleep(std::time::Duration::from_millis(35));
                    }
                }
                Ok(())
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        ComputerUseHost::computer_use_after_pointer_mutation(self);
        ComputerUseHost::computer_use_after_committed_ui_action(self);
        ComputerUseHost::computer_use_record_mutation(self, ComputerUseLastMutationKind::KeyChord);
        Ok(())
    }

    pub(super) async fn type_text_impl(&self, text: &str) -> BitFunResult<()> {
        if text.is_empty() {
            return Ok(());
        }
        // On macOS, route through background input when the frontmost app
        // is a terminal emulator — enigo.text() uses Unicode string
        // injection which terminal emulators (Ghostty, iTerm2, Terminal.app)
        // silently drop. bg_type_text_auto detects this and switches to
        // per-keystroke key-event synthesis.
        #[cfg(target_os = "macos")]
        {
            if crate::computer_use::macos_bg_input::supports_background_input() {
                let frontmost = crate::computer_use::macos_bg_input::frontmost_pid_macos();
                if let Some(pid) = frontmost {
                    if crate::computer_use::macos_bg_input::is_terminal_emulator(pid) {
                        let txt = text.to_string();
                        tokio::task::spawn_blocking(move || {
                            macos::catch_objc(|| {
                                crate::computer_use::macos_bg_input::bg_type_text_auto(pid, &txt)
                            })
                        })
                        .await
                        .map_err(|e| BitFunError::tool(e.to_string()))??;
                        ComputerUseHost::computer_use_after_committed_ui_action(self);
                        ComputerUseHost::computer_use_trust_pointer_after_text_input(self);
                        ComputerUseHost::computer_use_record_mutation(
                            self,
                            ComputerUseLastMutationKind::TypeText,
                        );
                        return Ok(());
                    }
                }
            }
        }
        let owned = text.to_string();
        tokio::task::spawn_blocking(move || {
            Self::run_enigo_job(|e| {
                e.text(&owned)
                    .map_err(|err| BitFunError::tool(format!("type_text: {}", err)))
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        // Typing does not move the pointer; do not set click_needs (would block Enter after search).
        ComputerUseHost::computer_use_after_committed_ui_action(self);
        ComputerUseHost::computer_use_trust_pointer_after_text_input(self);
        ComputerUseHost::computer_use_record_mutation(self, ComputerUseLastMutationKind::TypeText);
        Ok(())
    }
}
