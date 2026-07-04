//! AX-first orchestration for the desktop Computer Use host: the
//! `app_*` window-scoped actions (click/type/scroll/key-chord/wait) and the
//! `interactive_*` / `visual_*` cached-index action families (build view,
//! click/type/scroll by cached index), plus their shared digest/grid-
//! detection helpers.
//!
//! Extracted from `desktop_host/mod.rs` (no behavior change) so the
//! AX-orchestration surface has a single, independently reviewable home
//! instead of living inline inside the multi-thousand-line host file.

#[cfg(target_os = "macos")]
use super::macos;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use super::resolve_pid;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use super::LINUX_LEGACY_AX_UNAVAILABLE;
#[cfg(target_os = "macos")]
use super::{require_macos_background_input, resolve_pid_macos};
use super::{CachedInteractiveView, CachedVisualMarkView, DesktopComputerUseHost};
use bitfun_core::agentic::tools::computer_use_host::{
    AppClickParams, AppSelector, AppStateSnapshot, AppWaitPredicate, ClickTarget,
    ComputerScreenshot, ComputerUseHost, InteractiveActionResult, InteractiveClickParams,
    InteractiveScrollParams, InteractiveTypeTextParams, InteractiveView, InteractiveViewOpts,
    VisualActionResult, VisualClickParams, VisualMark, VisualMarkView, VisualMarkViewOpts,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use std::time::{Duration, Instant};

impl DesktopComputerUseHost {
    pub(super) async fn app_click_impl(
        &self,
        params: AppClickParams,
    ) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &params.app).await?;
            let self_pid = std::process::id() as i32;
            let mut click_coords: Option<(f64, f64)> = None;
            log::info!(
                target: "computer_use::app_click",
                "app_click.enter pid={} self_pid={} same_process={} target={:?} button={} click_count={} modifier_keys={:?}",
                pid,
                self_pid,
                pid == self_pid,
                params.target,
                params.mouse_button,
                params.click_count,
                params.modifier_keys
            );
            // Try AX press path when the target is a node idx and the cache
            // still holds a live ref; otherwise inject background events at
            // the resolved global coordinate.
            let ax_ok = match &params.target {
                ClickTarget::NodeIdx { idx } => {
                    let idx = *idx;
                    // Run AX lookup + AXPress under @try/@catch on a blocking
                    // thread; either a missing ref or a thrown NSException
                    // simply degrades to the bg_click fallback below.
                    tokio::task::spawn_blocking(move || {
                        macos::catch_objc(|| {
                            Ok(
                                if let Some(r) =
                                    crate::computer_use::macos_ax_dump::cached_ref_loose(pid, idx)
                                {
                                    matches!(
                                        crate::computer_use::macos_ax_write::try_ax_press(r),
                                        crate::computer_use::macos_ax_write::AxWriteOutcome::Ok
                                    )
                                } else {
                                    false
                                },
                            )
                        })
                        .unwrap_or(false)
                    })
                    .await
                    .unwrap_or(false)
                }
                ClickTarget::ScreenXy { .. }
                | ClickTarget::ImageXy { .. }
                | ClickTarget::ImageGrid { .. }
                | ClickTarget::VisualGrid { .. }
                | ClickTarget::OcrText { .. } => false,
            };
            if !ax_ok {
                require_macos_background_input()?;
                let (x, y): (f64, f64) = match &params.target {
                    ClickTarget::ScreenXy { x, y } => (*x, *y),
                    ClickTarget::ImageXy {
                        x,
                        y,
                        screenshot_id,
                    } => self.map_app_image_coords_to_pointer_f64(
                        pid,
                        *x,
                        *y,
                        screenshot_id.as_deref(),
                    )?,
                    ClickTarget::ImageGrid { screenshot_id, .. } => {
                        let (ix, iy) =
                            Self::image_grid_target_to_xy(&params.target)?.ok_or_else(|| {
                                BitFunError::tool("invalid image_grid target".to_string())
                            })?;
                        self.map_app_image_coords_to_pointer_f64(
                            pid,
                            ix,
                            iy,
                            screenshot_id.as_deref(),
                        )?
                    }
                    ClickTarget::VisualGrid {
                        rows,
                        cols,
                        row,
                        col,
                        intersections,
                        wait_ms_after_detection,
                    } => {
                        let shot = self.screenshot_for_app_pid(pid).await?;
                        let (x0, y0, width, height) =
                            detect_regular_grid_rect_from_screenshot(&shot, *rows, *cols)?;
                        let target = ClickTarget::ImageGrid {
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
                        let (ix, iy) =
                            Self::image_grid_target_to_xy(&target)?.ok_or_else(|| {
                                BitFunError::tool("invalid detected visual_grid target".to_string())
                            })?;
                        if let Some(wait) = wait_ms_after_detection {
                            if *wait > 0 {
                                tokio::time::sleep(Duration::from_millis(*wait as u64)).await;
                            }
                        }
                        self.map_app_image_coords_to_pointer_f64(
                            pid,
                            ix,
                            iy,
                            shot.screenshot_id.as_deref(),
                        )?
                    }
                    ClickTarget::NodeIdx { idx } => {
                        // Best-effort: re-snapshot to read the node's frame.
                        // Skip the screenshot — this snapshot is internal-only;
                        // the post-click re-snapshot below is the one returned
                        // to the model and carries the visual evidence.
                        let snap = self
                            .get_app_state_inner(params.app.clone(), 32, false, false)
                            .await?;
                        let node = snap.nodes.iter().find(|n| n.idx == *idx).ok_or_else(|| {
                            BitFunError::tool(format!(
                                "AX_NODE_STALE: idx={} no longer present in app state",
                                idx
                            ))
                        })?;
                        // Refuse to fall back to (0,0) on the desktop —
                        // that would silently click the menu bar / Finder
                        // icon. The caller must re-snapshot to acquire a
                        // node with a real on-screen frame.
                        let (fx, fy, fw, fh) = node.frame_global.ok_or_else(|| {
                            BitFunError::tool(format!(
                                "AX_NODE_STALE: idx={} has no AXFrame (likely off-screen or window minimised)",
                                idx
                            ))
                        })?;
                        if fw <= 0.0 || fh <= 0.0 {
                            return Err(BitFunError::tool(format!(
                                "AX_NODE_STALE: idx={} has zero-size frame ({}x{})",
                                idx, fw, fh
                            )));
                        }
                        (fx + fw / 2.0, fy + fh / 2.0)
                    }
                    ClickTarget::OcrText { needle } => {
                        // Codex parity: when the AX tree doesn't expose the
                        // target widget (Canvas, WebGL, custom-drawn cell),
                        // fall back to OCR-on-screenshot. We screenshot the
                        // whole screen rather than just the target window
                        // because window-relative regions need extra plumbing
                        // and the matcher already filters by confidence.
                        let matches = self.ocr_find_text_matches(needle, None).await?;
                        let best = matches.into_iter().max_by(|a, b| {
                            a.confidence
                                .partial_cmp(&b.confidence)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        let m = best.ok_or_else(|| {
                            BitFunError::tool(format!(
                                "NOT_FOUND: no OCR match for needle {:?}",
                                needle
                            ))
                        })?;
                        (m.center_x, m.center_y)
                    }
                };
                let click_coords_val = Some((x, y));
                click_coords = click_coords_val;
                let mods: Vec<crate::computer_use::macos_bg_input::BgModifier> = params
                    .modifier_keys
                    .iter()
                    .filter_map(|m| crate::computer_use::macos_bg_input::BgModifier::from_str(m))
                    .collect();
                let btn = match params.mouse_button.as_str() {
                    "right" => crate::computer_use::macos_bg_input::BgMouseButton::Right,
                    "middle" => crate::computer_use::macos_bg_input::BgMouseButton::Middle,
                    _ => crate::computer_use::macos_bg_input::BgMouseButton::Left,
                };
                let cnt = params.click_count.max(1) as u32;
                log::info!(
                    target: "computer_use::app_click",
                    "app_click.bg_dispatch pid={} self_pid={} same_process={} resolved_x={:.2} resolved_y={:.2} click_count={}",
                    pid, self_pid, pid == self_pid, x, y, cnt
                );

                // Capture pre-click digest so we can detect "click delivered
                // but UI did not change" and apply a foreground fallback when
                // the target lives in our own process (the most common cause
                // of `bg_click → WKWebView no-op` in single-process Tauri).
                let pre_digest_opt = match self
                    .get_app_state_inner(params.app.clone(), 0, false, false)
                    .await
                {
                    Ok(s) => Some(s.digest),
                    Err(e) => {
                        debug!(
                            target: "computer_use::app_click",
                            "pre_digest_unavailable error={}",
                            e
                        );
                        None
                    }
                };

                // Resolve window-id and bundle-id for focus-without-raise
                // activation and Chromium click routing.
                let bundle_id_opt = params
                    .app
                    .bundle_id
                    .clone()
                    .or_else(|| crate::computer_use::macos_bg_input::bundle_id_for_pid(pid));
                let is_chromium = crate::computer_use::macos_bg_input::is_chromium_electron(
                    bundle_id_opt.as_deref(),
                );
                let win_id_and_bounds = tokio::task::spawn_blocking(move || {
                    macos::catch_objc(|| {
                        let wid =
                            crate::computer_use::macos_bg_input::frontmost_window_id_for_pid(pid);
                        let bounds =
                            crate::computer_use::macos_ax_ui::window_bounds_global_for_pid(pid)
                                .ok();
                        Ok::<_, BitFunError>((wid, bounds))
                    })
                })
                .await
                .unwrap_or(Ok((None, None)))
                .unwrap_or((None, None));
                let (win_id, win_bounds) = win_id_and_bounds;

                // Best-effort foreground activation — required for WKWebView
                // and many Cocoa hit-testers to actually deliver our
                // synthetic events. Uses focus-without-raise SPI when a
                // window_id is available, falling back to public API.
                let activate_pid = pid;
                let activate_wid = win_id;
                let _ = tokio::task::spawn_blocking(move || {
                    macos::catch_objc(|| {
                        crate::computer_use::macos_bg_input::activate_pid_macos_with_window(
                            activate_pid,
                            activate_wid,
                        )
                    })
                })
                .await;

                let mods_for_bg = mods.clone();
                let win_bounds_for_click = win_bounds;
                let wid_for_click = win_id;
                tokio::task::spawn_blocking(move || {
                    macos::catch_objc(|| {
                        // Use Chromium 5-event recipe for Chromium/Electron
                        // targets when we have a window-id and window bounds.
                        if is_chromium {
                            if let (Some(wid), Some((wx, wy, _, _))) =
                                (wid_for_click, win_bounds_for_click)
                            {
                                return crate::computer_use::macos_bg_input::bg_click_chromium(
                                    pid,
                                    x,
                                    y,
                                    x - wx as f64,
                                    y - wy as f64,
                                    wid,
                                    cnt,
                                    &mods_for_bg,
                                );
                            }
                        }
                        crate::computer_use::macos_bg_input::bg_click(
                            pid,
                            (x, y),
                            btn,
                            cnt,
                            &mods_for_bg,
                        )
                    })
                })
                .await
                .map_err(|e| BitFunError::tool(e.to_string()))??;

                // Same-process fallback: if `bg_click` left the digest
                // unchanged AND the target is our own process (bitfun-desktop
                // hosting an embedded mini-app WebView), retry with the
                // foreground click path. This trades a momentary cursor
                // movement for actually landing the click in the WebView.
                if pid == self_pid {
                    let settle = params.wait_ms_after.unwrap_or(120).min(5_000);
                    tokio::time::sleep(Duration::from_millis(settle.max(80) as u64)).await;
                    let post_digest_opt = self
                        .get_app_state_inner(params.app.clone(), 0, false, false)
                        .await
                        .ok()
                        .map(|s| s.digest);
                    let unchanged =
                        matches!((&pre_digest_opt, &post_digest_opt), (Some(a), Some(b)) if a == b);
                    if unchanged {
                        warn!(
                            target: "computer_use::app_click",
                            "bg_click_no_effect_self_pid_falling_back_to_foreground pid={} x={:.2} y={:.2} digest={:?}",
                            pid, x, y, post_digest_opt
                        );
                        // Foreground fallback uses the user's real cursor +
                        // synthetic enigo click so the WKWebView's hit-test
                        // path is identical to a human click.
                        let btn_str = match btn {
                            crate::computer_use::macos_bg_input::BgMouseButton::Right => "right",
                            crate::computer_use::macos_bg_input::BgMouseButton::Middle => "middle",
                            _ => "left",
                        };
                        self.mouse_move_global_f64(x, y).await?;
                        for _ in 0..cnt {
                            self.mouse_click_authoritative(btn_str).await?;
                        }
                    }
                }
            }
            let settle_ms = params.wait_ms_after.unwrap_or(120).min(5_000);
            if settle_ms > 0 {
                tokio::time::sleep(Duration::from_millis(settle_ms as u64)).await;
            }
            // Re-snapshot so the caller can see the new state + new digest.
            let result_snap = self.get_app_state(params.app, 32, false).await?;
            // Debug-only: annotate the returned screenshot with the click
            // target coordinates so logs show where the click landed.
            if let Some((cx, cy)) = click_coords {
                if log::log_enabled!(target: "computer_use::debug_overlay", log::Level::Debug) {
                    if let Some(ref shot) = result_snap.screenshot {
                        match crate::computer_use::debug_overlay::annotate_screenshot_with_click(
                            &shot.bytes,
                            "image/jpeg",
                            cx as u32,
                            cy as u32,
                        ) {
                            Ok(_annotated) => {
                                debug!(
                                    target: "computer_use::debug_overlay",
                                    "click_annotated pid={} x={:.0} y={:.0} original_bytes={}",
                                    pid, cx, cy, shot.bytes.len()
                                );
                            }
                            Err(e) => {
                                debug!(
                                    target: "computer_use::debug_overlay",
                                    "click_annotation_failed pid={} error={}",
                                    pid, e
                                );
                            }
                        }
                    }
                }
            }
            Ok(result_snap)
        }
        #[cfg(target_os = "windows")]
        {
            // Resolve the target to a global screen point, then deliver an
            // invisible PostMessage click to the foreground window (the same
            // window the AX snapshot describes).
            let (x, y) = self.resolve_click_target_windows(&params.target).await?;
            let hwnd_raw = crate::computer_use::windows_ax_ui::foreground_window_handle();
            if hwnd_raw == 0 {
                return Err(BitFunError::tool(
                    "app_click: no foreground window to target on Windows.".to_string(),
                ));
            }
            let button = params.mouse_button.clone();
            let count = params.click_count.max(1) as usize;
            let modifiers = params.modifier_keys.clone();
            log::info!(
                target: "computer_use::app_click",
                "app_click.windows post_click_screen x={:.1} y={:.1} button={} count={} mods={:?}",
                x, y, button, count, modifiers
            );
            tokio::task::spawn_blocking(move || {
                let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
                crate::computer_use::windows_bg_input::post_click_screen(
                    hwnd,
                    x.round() as i32,
                    y.round() as i32,
                    &button,
                    count,
                    &modifiers,
                )
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;

            let settle_ms = params.wait_ms_after.unwrap_or(120).min(5_000);
            if settle_ms > 0 {
                tokio::time::sleep(Duration::from_millis(settle_ms as u64)).await;
            }
            self.get_app_state(params.app, 32, false).await
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = params;
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn app_type_text_impl(
        &self,
        app: AppSelector,
        text: &str,
        focus: Option<ClickTarget>,
    ) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &app).await?;
            let focus_target_idx = match &focus {
                Some(ClickTarget::NodeIdx { idx }) => Some(*idx),
                _ => None,
            };
            // If a focus target is provided, click it first to give focus.
            if let Some(target) = focus {
                let click = AppClickParams {
                    app: app.clone(),
                    target,
                    click_count: 1,
                    mouse_button: "left".to_string(),
                    modifier_keys: vec![],
                    wait_ms_after: None,
                };
                let _ = self.app_click(click).await?;
            }
            require_macos_background_input()?;
            log::info!(
                target: "computer_use::app_type_text",
                "app_type_text.bg_dispatch pid={} char_count={}",
                pid,
                text.chars().count()
            );
            // Resolve window-id and activate with focus-without-raise SPI
            // when available. Falls back to public NSRunningApplication.
            // Also best-effort AX-focus the previously-clicked element so
            // `bg_type_text` lands in the right text field even when the
            // click activated the window but didn't move key focus.
            let activate_pid = pid;
            let _ = tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| {
                    let wid = crate::computer_use::macos_bg_input::frontmost_window_id_for_pid(
                        activate_pid,
                    );
                    crate::computer_use::macos_bg_input::activate_pid_macos_with_window(
                        activate_pid,
                        wid,
                    )?;
                    // Best-effort: AX-focus the target node so the text
                    // channel delivers to the right field. `Ok` even on
                    // failure — the bg_type_text fallback still works.
                    if let Some(idx) = focus_target_idx {
                        if let Some(r) =
                            crate::computer_use::macos_ax_dump::cached_ref_loose(activate_pid, idx)
                        {
                            let _ = crate::computer_use::macos_ax_write::try_ax_focus(r);
                        }
                    }
                    Ok::<_, BitFunError>(())
                })
            })
            .await;
            let txt = text.to_string();
            // Use bg_type_text_auto which routes to terminal-safe key-event
            // typing when the target is a terminal emulator.
            tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| {
                    crate::computer_use::macos_bg_input::bg_type_text_auto(pid, &txt)
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(target_os = "windows")]
        {
            // Click the focus target first (if any) so keystrokes land in the
            // right control, then deliver the text. Cloaked `SendInput` is the
            // most reliable path (works for both classic Win32 edit controls and
            // modern XAML/WinUI/WPF surfaces that ignore posted `WM_CHAR`); it
            // falls back to `PostMessage(WM_CHAR)` internally when foreground
            // cannot be claimed.
            if let Some(target) = focus {
                let click = AppClickParams {
                    app: app.clone(),
                    target,
                    click_count: 1,
                    mouse_button: "left".to_string(),
                    modifier_keys: vec![],
                    wait_ms_after: None,
                };
                let _ = self.app_click(click).await?;
            }
            let hwnd_raw = crate::computer_use::windows_ax_ui::foreground_window_handle();
            if hwnd_raw == 0 {
                return Err(BitFunError::tool(
                    "app_type_text: no foreground window to target on Windows.".to_string(),
                ));
            }
            let txt = text.to_string();
            log::info!(
                target: "computer_use::app_type_text",
                "app_type_text.windows char_count={}",
                txt.chars().count()
            );
            tokio::task::spawn_blocking(move || {
                let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
                crate::computer_use::windows_bg_input::inject_text_cloaked(hwnd, &txt)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, text, focus);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn app_scroll_impl(
        &self,
        app: AppSelector,
        focus: Option<ClickTarget>,
        dx: i32,
        dy: i32,
    ) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &app).await?;
            if let Some(target) = focus {
                let click = AppClickParams {
                    app: app.clone(),
                    target,
                    click_count: 1,
                    mouse_button: "left".to_string(),
                    modifier_keys: vec![],
                    wait_ms_after: None,
                };
                let _ = self.app_click(click).await?;
            }
            require_macos_background_input()?;
            let activate_pid = pid;
            let _ = tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| {
                    let wid = crate::computer_use::macos_bg_input::frontmost_window_id_for_pid(
                        activate_pid,
                    );
                    crate::computer_use::macos_bg_input::activate_pid_macos_with_window(
                        activate_pid,
                        wid,
                    )
                })
            })
            .await;
            tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| crate::computer_use::macos_bg_input::bg_scroll(pid, dx, dy))
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(target_os = "windows")]
        {
            let hwnd_raw = crate::computer_use::windows_ax_ui::foreground_window_handle();
            if hwnd_raw == 0 {
                return Err(BitFunError::tool(
                    "app_scroll: no foreground window to target on Windows.".to_string(),
                ));
            }
            // Anchor point: the focus target's center when given, else the
            // foreground window center. `post_scroll_screen` resolves the
            // deepest child at that point and posts WM_VSCROLL / WM_HSCROLL.
            let (sx, sy) = if let Some(target) = &focus {
                let (x, y) = self.resolve_click_target_windows(target).await?;
                (x.round() as i32, y.round() as i32)
            } else {
                Self::windows_foreground_window_center(hwnd_raw).ok_or_else(|| {
                    BitFunError::tool(
                        "app_scroll: could not resolve foreground window center.".to_string(),
                    )
                })?
            };
            log::info!(
                target: "computer_use::app_scroll",
                "app_scroll.windows sx={} sy={} dx={} dy={}",
                sx, sy, dx, dy
            );
            tokio::task::spawn_blocking(move || {
                let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
                crate::computer_use::windows_bg_input::post_scroll_screen(hwnd, sx, sy, dx, dy)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, focus, dx, dy);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn app_key_chord_impl(
        &self,
        app: AppSelector,
        keys: Vec<String>,
        focus_idx: Option<u32>,
    ) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &app).await?;
            if let Some(idx) = focus_idx {
                let click = AppClickParams {
                    app: app.clone(),
                    target: ClickTarget::NodeIdx { idx },
                    click_count: 1,
                    mouse_button: "left".to_string(),
                    modifier_keys: vec![],
                    wait_ms_after: None,
                };
                let _ = self.app_click(click).await?;
            }
            require_macos_background_input()?;
            let activate_pid = pid;
            let _ = tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| {
                    let wid = crate::computer_use::macos_bg_input::frontmost_window_id_for_pid(
                        activate_pid,
                    );
                    crate::computer_use::macos_bg_input::activate_pid_macos_with_window(
                        activate_pid,
                        wid,
                    )
                })
            })
            .await;
            tokio::task::spawn_blocking(move || -> BitFunResult<()> {
                macos::catch_objc(|| {
                    let (mods, kc) =
                        crate::computer_use::macos_bg_input::parse_key_sequence(&keys)?;
                    crate::computer_use::macos_bg_input::bg_key_chord(pid, &mods, kc)?;
                    Ok(())
                })
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(target_os = "windows")]
        {
            // Focus the target node first (if any) so the chord lands in the
            // right control.
            if let Some(idx) = focus_idx {
                let click = AppClickParams {
                    app: app.clone(),
                    target: ClickTarget::NodeIdx { idx },
                    click_count: 1,
                    mouse_button: "left".to_string(),
                    modifier_keys: vec![],
                    wait_ms_after: None,
                };
                let _ = self.app_click(click).await?;
            }
            let hwnd_raw = crate::computer_use::windows_ax_ui::foreground_window_handle();
            if hwnd_raw == 0 {
                return Err(BitFunError::tool(
                    "app_key_chord: no foreground window to target on Windows.".to_string(),
                ));
            }
            let keys_for_parse = keys.clone();
            log::info!(
                target: "computer_use::app_key_chord",
                "app_key_chord.windows keys={:?}",
                keys
            );
            tokio::task::spawn_blocking(move || -> BitFunResult<()> {
                let (mods, keycode) =
                    crate::computer_use::windows_bg_input::parse_key_chord(&keys_for_parse)?;
                let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
                crate::computer_use::windows_bg_input::inject_key_cloaked(hwnd, keycode, &mods)
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, keys, focus_idx);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn app_wait_for_impl(
        &self,
        app: AppSelector,
        pred: AppWaitPredicate,
        timeout_ms: u32,
        poll_ms: u32,
    ) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
            let poll = Duration::from_millis(poll_ms.max(50) as u64);
            // Polling loop — skip the screenshot per iteration to keep
            // poll latency tight; the snapshot we ultimately return gets
            // an auto-attached screenshot below.
            let baseline = self
                .get_app_state_inner(app.clone(), 32, false, false)
                .await?;
            loop {
                let snap = self
                    .get_app_state_inner(app.clone(), 32, false, false)
                    .await?;
                let ok = match &pred {
                    AppWaitPredicate::DigestChanged { prev_digest } => {
                        snap.digest != *prev_digest && snap.digest != baseline.digest
                    }
                    AppWaitPredicate::TitleContains { needle } => snap
                        .window_title
                        .as_deref()
                        .map(|t| t.contains(needle.as_str()))
                        .unwrap_or(false),
                    AppWaitPredicate::RoleEnabled { role } => snap
                        .nodes
                        .iter()
                        .any(|n| n.role.as_str() == role && n.enabled),
                    AppWaitPredicate::NodeEnabled { idx } => snap
                        .nodes
                        .iter()
                        .find(|n| n.idx == *idx)
                        .map(|n| n.enabled)
                        .unwrap_or(false),
                };
                if ok || Instant::now() >= deadline {
                    // Final returned snap — auto-attach screenshot for parity
                    // with the rest of the `app_*` family.
                    let mut snap = snap;
                    if let Ok(pid) = resolve_pid_macos(self, &app).await {
                        if let Ok(shot) = self.screenshot_for_app_pid(pid).await {
                            snap.screenshot = Some(shot);
                        }
                    }
                    if snap.screenshot.is_none() {
                        if let Ok(shot) = self.screenshot_peek_full_display().await {
                            snap.screenshot = Some(shot);
                        }
                    }
                    return Ok(snap);
                }
                tokio::time::sleep(poll).await;
            }
        }
        #[cfg(target_os = "windows")]
        {
            let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
            let poll = Duration::from_millis(poll_ms.max(50) as u64);
            let baseline = self
                .get_app_state_inner(app.clone(), 32, false, false)
                .await?;
            loop {
                let snap = self
                    .get_app_state_inner(app.clone(), 32, false, false)
                    .await?;
                let ok = match &pred {
                    AppWaitPredicate::DigestChanged { prev_digest } => {
                        snap.digest != *prev_digest && snap.digest != baseline.digest
                    }
                    AppWaitPredicate::TitleContains { needle } => snap
                        .window_title
                        .as_deref()
                        .map(|t| t.contains(needle.as_str()))
                        .unwrap_or(false),
                    AppWaitPredicate::RoleEnabled { role } => snap
                        .nodes
                        .iter()
                        .any(|n| n.role.as_str() == role && n.enabled),
                    AppWaitPredicate::NodeEnabled { idx } => snap
                        .nodes
                        .iter()
                        .find(|n| n.idx == *idx)
                        .map(|n| n.enabled)
                        .unwrap_or(false),
                };
                if ok || Instant::now() >= deadline {
                    // Final returned snap — auto-attach a window screenshot for
                    // parity with the rest of the `app_*` family.
                    let mut snap = snap;
                    if snap.screenshot.is_none() {
                        let pid = Self::windows_foreground_pid();
                        let hwnd_raw =
                            crate::computer_use::windows_ax_ui::foreground_window_handle();
                        if hwnd_raw != 0 {
                            if let Ok(shot) =
                                self.screenshot_for_foreground_window(pid, hwnd_raw).await
                            {
                                snap.screenshot = Some(shot);
                            }
                        }
                    }
                    return Ok(snap);
                }
                tokio::time::sleep(poll).await;
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, pred, timeout_ms, poll_ms);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn build_interactive_view_impl(
        &self,
        app: AppSelector,
        opts: InteractiveViewOpts,
    ) -> BitFunResult<InteractiveView> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let pid = resolve_pid(self, &app).await?;
            let snap = self
                .get_app_state_inner(app.clone(), 64, opts.focus_window_only, true)
                .await?;
            let max_elements = opts
                .max_elements
                .map(|n| n as usize)
                .unwrap_or(80)
                .clamp(1, 200);
            let filter_opts = crate::computer_use::interactive_filter::FilterOpts {
                max_elements,
                clip_to_image_bounds: opts.focus_window_only,
            };
            let elements = crate::computer_use::interactive_filter::build_interactive_elements(
                &snap.nodes,
                snap.screenshot.as_ref(),
                &filter_opts,
            );
            let tree_text = if opts.include_tree_text {
                crate::computer_use::interactive_filter::render_element_tree_text(&elements)
            } else {
                String::new()
            };
            let digest = compute_interactive_view_digest(&elements);

            let mut screenshot_out: Option<ComputerScreenshot> = None;
            if opts.annotate_screenshot {
                if let Some(shot) = snap.screenshot.as_ref() {
                    match crate::computer_use::som_overlay::render_overlay(
                        &shot.bytes,
                        &elements,
                        Some(80),
                    ) {
                        Ok(jpeg) => {
                            let mut out = shot.clone();
                            out.bytes = jpeg;
                            out.mime_type = "image/jpeg".to_string();
                            screenshot_out = Some(out);
                        }
                        Err(e) => {
                            warn!(
                                target: "computer_use::interactive_view",
                                "som_overlay render failed (non-fatal): {}",
                                e
                            );
                            screenshot_out = Some(shot.clone());
                        }
                    }
                }
            } else {
                screenshot_out = snap.screenshot.clone();
            }

            let captured_at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default();

            let view = InteractiveView {
                app: snap.app.clone(),
                window_title: snap.window_title.clone(),
                elements: elements.clone(),
                tree_text,
                digest: digest.clone(),
                captured_at_ms,
                screenshot: screenshot_out,
                loop_warning: snap.loop_warning.clone(),
            };

            // Cache for subsequent `interactive_*` calls.
            {
                let mut s = self
                    .state
                    .lock()
                    .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
                s.interactive_view_cache.insert(
                    pid,
                    CachedInteractiveView {
                        digest: digest.clone(),
                        elements,
                    },
                );
            }
            Ok(view)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, opts);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn interactive_click_impl(
        &self,
        app: AppSelector,
        params: InteractiveClickParams,
    ) -> BitFunResult<InteractiveActionResult> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            // Resolve `i → node_idx` against the cached interactive view.
            // On `STALE_INTERACTIVE_VIEW` we transparently rebuild the
            // view ONCE and retry — this turns the most common UI-changed
            // failure into an internal recovery instead of a hard error
            // the model has to handle. Idempotency is preserved by
            // capping at one rebuild + one retry.
            let mut auto_rebuilt = false;
            let node_idx = match self
                .resolve_interactive_index(&app, params.i, params.before_view_digest.as_deref())
                .await
            {
                Ok(idx) => idx,
                Err(err) if is_stale_interactive_view_error(&err) => {
                    warn!(
                        target: "computer_use::interactive_view",
                        "interactive_click: STALE view detected, rebuilding once and retrying (i={}): {}",
                        params.i, err
                    );
                    let rebuilt = self
                        .build_interactive_view(app.clone(), InteractiveViewOpts::default())
                        .await?;
                    if rebuilt.elements.iter().any(|e| e.i == params.i) {
                        auto_rebuilt = true;
                        // Use the rebuilt view's digest, not the stale one
                        // the caller passed in.
                        self.resolve_interactive_index(&app, params.i, Some(&rebuilt.digest))
                            .await?
                    } else {
                        return Err(BitFunError::tool(format!(
                            "INTERACTIVE_INDEX_OUT_OF_RANGE: i={} not in rebuilt view (len={}); the UI has changed under you, re-call `build_interactive_view` and pick a fresh `i`",
                            params.i,
                            rebuilt.elements.len()
                        )));
                    }
                }
                Err(other) => return Err(other),
            };

            // Look up the cached element's image-pixel center as a
            // pointer fallback. Always available when `frame_image` was
            // populated at view-build time; covers Electron / Canvas /
            // custom-drawn widgets that AXPress can't dispatch into.
            let pointer_fallback_image_xy: Option<(i32, i32)> =
                self.cached_interactive_image_center(&app, params.i).await;

            // Primary path: AX-targeted click via `app_click`. On
            // failure, fall back to a pointer click at the element's
            // image-pixel center if we have one.
            let click_res = self
                .app_click(AppClickParams {
                    app: app.clone(),
                    target: ClickTarget::NodeIdx { idx: node_idx },
                    click_count: params.click_count.max(1),
                    mouse_button: params.mouse_button.clone(),
                    modifier_keys: params.modifier_keys.clone(),
                    wait_ms_after: params.wait_ms_after,
                })
                .await;

            let (snapshot, fallback_used) = match click_res {
                Ok(s) => (s, false),
                Err(e) if pointer_fallback_image_xy.is_some() => {
                    let (ix, iy) = pointer_fallback_image_xy.unwrap();
                    warn!(
                        target: "computer_use::interactive_view",
                        "interactive_click: AX path failed, falling back to image_xy=({},{}): {}",
                        ix, iy, e
                    );
                    let s = self
                        .app_click(AppClickParams {
                            app: app.clone(),
                            target: ClickTarget::ImageXy {
                                x: ix,
                                y: iy,
                                screenshot_id: None,
                            },
                            click_count: params.click_count.max(1),
                            mouse_button: params.mouse_button.clone(),
                            modifier_keys: params.modifier_keys.clone(),
                            wait_ms_after: params.wait_ms_after,
                        })
                        .await?;
                    (s, true)
                }
                Err(e) => return Err(e),
            };

            let view = if params.return_view {
                Some(
                    self.build_interactive_view(app, InteractiveViewOpts::default())
                        .await?,
                )
            } else {
                None
            };
            let mut note = format!("index_resolved_via_node_idx({})", node_idx);
            if auto_rebuilt {
                note.push_str(",auto_rebuilt_view_after_stale");
            }
            if fallback_used {
                note.push_str(",fallback_image_xy");
            }
            Ok(InteractiveActionResult {
                snapshot,
                view,
                execution_note: Some(note),
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn build_visual_mark_view_impl(
        &self,
        app: AppSelector,
        opts: VisualMarkViewOpts,
    ) -> BitFunResult<VisualMarkView> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let pid = resolve_pid(self, &app).await?;
            let mut snap = self
                .get_app_state_inner(app.clone(), 16, true, true)
                .await?;
            if snap.screenshot.is_none() {
                #[cfg(target_os = "macos")]
                {
                    if let Ok(shot) = self.screenshot_for_app_pid(pid).await {
                        snap.screenshot = Some(shot);
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    let hwnd_raw = crate::computer_use::windows_ax_ui::foreground_window_handle();
                    if hwnd_raw != 0 {
                        if let Ok(shot) = self.screenshot_for_foreground_window(pid, hwnd_raw).await
                        {
                            snap.screenshot = Some(shot);
                        }
                    }
                }
            }
            let shot = snap.screenshot.as_ref().ok_or_else(|| {
                BitFunError::tool(
                    "build_visual_mark_view: app screenshot unavailable; grant Screen Recording permission and retry".to_string(),
                )
            })?;

            let marks = build_regular_visual_marks(shot, &opts)?;
            let digest = compute_visual_mark_view_digest(&marks, shot.screenshot_id.as_deref());

            let mut screenshot_out: Option<ComputerScreenshot> = Some(shot.clone());
            if opts.include_grid && !marks.is_empty() {
                let overlay_elements = visual_marks_to_overlay_elements(&marks);
                match crate::computer_use::som_overlay::render_overlay(
                    &shot.bytes,
                    &overlay_elements,
                    Some(82),
                ) {
                    Ok(jpeg) => {
                        let mut out = shot.clone();
                        out.bytes = jpeg;
                        out.mime_type = "image/jpeg".to_string();
                        screenshot_out = Some(out);
                    }
                    Err(e) => {
                        warn!(
                            target: "computer_use::visual_mark_view",
                            "visual mark overlay render failed (non-fatal): {}",
                            e
                        );
                    }
                }
            }

            let captured_at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default();
            let view = VisualMarkView {
                app: snap.app.clone(),
                window_title: snap.window_title.clone(),
                marks: marks.clone(),
                digest: digest.clone(),
                captured_at_ms,
                screenshot: screenshot_out,
            };
            {
                let mut s = self
                    .state
                    .lock()
                    .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
                s.visual_mark_cache.insert(
                    pid,
                    CachedVisualMarkView {
                        digest,
                        marks,
                        screenshot_id: shot.screenshot_id.clone(),
                    },
                );
            }
            Ok(view)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, opts);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn visual_click_impl(
        &self,
        app: AppSelector,
        params: VisualClickParams,
    ) -> BitFunResult<VisualActionResult> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let mut auto_rebuilt = false;
            let mark = match self
                .resolve_visual_mark(&app, params.i, params.before_view_digest.as_deref())
                .await
            {
                Ok(mark) => mark,
                Err(err) if is_stale_visual_mark_view_error(&err) => {
                    warn!(
                        target: "computer_use::visual_mark_view",
                        "visual_click: STALE visual mark view detected, rebuilding once and retrying (i={}): {}",
                        params.i, err
                    );
                    let rebuilt = self
                        .build_visual_mark_view(app.clone(), VisualMarkViewOpts::default())
                        .await?;
                    let Some(mark) = rebuilt.marks.iter().find(|m| m.i == params.i).cloned() else {
                        return Err(BitFunError::tool(format!(
                            "VISUAL_INDEX_OUT_OF_RANGE: i={} not in rebuilt view (len={}); re-call `build_visual_mark_view` and pick a fresh `i`",
                            params.i,
                            rebuilt.marks.len()
                        )));
                    };
                    auto_rebuilt = true;
                    mark
                }
                Err(other) => return Err(other),
            };

            let screenshot_id = {
                let pid = resolve_pid(self, &app).await?;
                let s = self
                    .state
                    .lock()
                    .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
                s.visual_mark_cache
                    .get(&pid)
                    .and_then(|cached| cached.screenshot_id.clone())
            };

            let snapshot = self
                .app_click(AppClickParams {
                    app: app.clone(),
                    target: ClickTarget::ImageXy {
                        x: mark.x,
                        y: mark.y,
                        screenshot_id,
                    },
                    click_count: params.click_count.max(1),
                    mouse_button: params.mouse_button.clone(),
                    modifier_keys: params.modifier_keys.clone(),
                    wait_ms_after: params.wait_ms_after,
                })
                .await?;

            let view = if params.return_view {
                Some(
                    self.build_visual_mark_view(app, VisualMarkViewOpts::default())
                        .await?,
                )
            } else {
                None
            };
            let mut note = format!("visual_mark_image_xy({},{})", mark.x, mark.y);
            if auto_rebuilt {
                note.push_str(",auto_rebuilt_view_after_stale");
            }
            Ok(VisualActionResult {
                snapshot,
                view,
                execution_note: Some(note),
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn interactive_type_text_impl(
        &self,
        app: AppSelector,
        params: InteractiveTypeTextParams,
    ) -> BitFunResult<InteractiveActionResult> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let focus = if let Some(i) = params.i {
                let node_idx = self
                    .resolve_interactive_index(&app, i, params.before_view_digest.as_deref())
                    .await?;
                Some(ClickTarget::NodeIdx { idx: node_idx })
            } else {
                None
            };

            if params.clear_first {
                if let Some(target) = focus.clone() {
                    let _ = self
                        .app_click(AppClickParams {
                            app: app.clone(),
                            target,
                            click_count: 1,
                            mouse_button: "left".to_string(),
                            modifier_keys: vec![],
                            wait_ms_after: Some(60),
                        })
                        .await?;
                }
                // Select-all + delete to clear the field. The "select all"
                // accelerator is Cmd+A on macOS and Ctrl+A on Windows.
                #[cfg(target_os = "macos")]
                {
                    let pid = resolve_pid_macos(self, &app).await?;
                    tokio::task::spawn_blocking(move || -> BitFunResult<()> {
                        macos::catch_objc(|| {
                            let (m1, k1) =
                                crate::computer_use::macos_bg_input::parse_key_sequence(&[
                                    "cmd".to_string(),
                                    "a".to_string(),
                                ])?;
                            crate::computer_use::macos_bg_input::bg_key_chord(pid, &m1, k1)?;
                            let (m2, k2) =
                                crate::computer_use::macos_bg_input::parse_key_sequence(&[
                                    "delete".to_string(),
                                ])?;
                            crate::computer_use::macos_bg_input::bg_key_chord(pid, &m2, k2)?;
                            Ok(())
                        })
                    })
                    .await
                    .map_err(|e| BitFunError::tool(e.to_string()))??;
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = self
                        .app_key_chord(app.clone(), vec!["ctrl".to_string(), "a".to_string()], None)
                        .await?;
                    let _ = self
                        .app_key_chord(app.clone(), vec!["delete".to_string()], None)
                        .await?;
                }
            }

            let snapshot = self.app_type_text(app.clone(), &params.text, focus).await?;

            if params.press_enter_after {
                #[cfg(target_os = "macos")]
                {
                    let pid = resolve_pid_macos(self, &app).await?;
                    tokio::task::spawn_blocking(move || -> BitFunResult<()> {
                        macos::catch_objc(|| {
                            let (m, k) =
                                crate::computer_use::macos_bg_input::parse_key_sequence(&[
                                    "return".to_string(),
                                ])?;
                            crate::computer_use::macos_bg_input::bg_key_chord(pid, &m, k)?;
                            Ok(())
                        })
                    })
                    .await
                    .map_err(|e| BitFunError::tool(e.to_string()))??;
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = self
                        .app_key_chord(app.clone(), vec!["return".to_string()], None)
                        .await?;
                }
            }

            if let Some(wait) = params.wait_ms_after {
                tokio::time::sleep(Duration::from_millis(wait.min(5_000) as u64)).await;
            }

            let view = if params.return_view {
                Some(
                    self.build_interactive_view(app, InteractiveViewOpts::default())
                        .await?,
                )
            } else {
                None
            };
            Ok(InteractiveActionResult {
                snapshot,
                view,
                execution_note: Some("ax_focus_then_bg_type_text".to_string()),
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }

    pub(super) async fn interactive_scroll_impl(
        &self,
        app: AppSelector,
        params: InteractiveScrollParams,
    ) -> BitFunResult<InteractiveActionResult> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let focus = if let Some(i) = params.i {
                let node_idx = self
                    .resolve_interactive_index(&app, i, params.before_view_digest.as_deref())
                    .await?;
                Some(ClickTarget::NodeIdx { idx: node_idx })
            } else {
                None
            };
            let snapshot = self
                .app_scroll(app.clone(), focus, params.dx, params.dy)
                .await?;
            if let Some(wait) = params.wait_ms_after {
                tokio::time::sleep(Duration::from_millis(wait.min(5_000) as u64)).await;
            }
            let view = if params.return_view {
                Some(
                    self.build_interactive_view(app, InteractiveViewOpts::default())
                        .await?,
                )
            } else {
                None
            };
            Ok(InteractiveActionResult {
                snapshot,
                view,
                execution_note: Some("app_scroll".to_string()),
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(LINUX_LEGACY_AX_UNAVAILABLE.to_string()))
        }
    }
}

/// Stable lowercase-hex SHA1 over a *layout-only* canonical payload:
/// `i|node_idx|role|subrole|x_bucket,y_bucket,w_bucket,h_bucket`.
///
/// Deliberately omits `label` (textfield value, focused selection, live
/// counters etc. would otherwise turn every keystroke into a STALE error)
/// and snaps coordinates to an 8-pt grid so a 1-pixel re-layout from a
/// scrollbar appearing / IME bar resizing doesn't invalidate the cached
/// view either. The digest is meant to detect *structural* changes
/// (elements appeared, disappeared, or moved noticeably), not cosmetic
/// noise.
fn compute_interactive_view_digest(
    elements: &[bitfun_core::agentic::tools::computer_use_host::InteractiveElement],
) -> String {
    use sha1::{Digest, Sha1};
    const BUCKET: f64 = 8.0;
    let mut hasher = Sha1::new();
    for e in elements {
        let subrole = e.subrole.as_deref().unwrap_or("");
        let (x, y, w, h) = e.frame_global.unwrap_or((0.0, 0.0, 0.0, 0.0));
        let xb = (x / BUCKET).floor() as i64;
        let yb = (y / BUCKET).floor() as i64;
        let wb = (w / BUCKET).round().max(1.0) as i64;
        let hb = (h / BUCKET).round().max(1.0) as i64;
        let line = format!(
            "{}|{}|{}|{}|{},{},{},{}\n",
            e.i, e.node_idx, e.role, subrole, xb, yb, wb, hb,
        );
        hasher.update(line.as_bytes());
    }
    let bytes = hasher.finalize();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes.iter() {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn compute_visual_mark_view_digest(marks: &[VisualMark], screenshot_id: Option<&str>) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(screenshot_id.unwrap_or("").as_bytes());
    hasher.update(b"\n");
    for mark in marks {
        let frame = mark.frame_image.unwrap_or((0, 0, 0, 0));
        let line = format!(
            "{}|{}|{}|{},{},{},{}\n",
            mark.i, mark.x, mark.y, frame.0, frame.1, frame.2, frame.3
        );
        hasher.update(line.as_bytes());
    }
    let bytes = hasher.finalize();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes.iter() {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn build_regular_visual_marks(
    shot: &ComputerScreenshot,
    opts: &VisualMarkViewOpts,
) -> BitFunResult<Vec<VisualMark>> {
    if !opts.include_grid {
        return Ok(Vec::new());
    }

    let image_w = shot.image_width.max(1);
    let image_h = shot.image_height.max(1);
    let (mut x0, mut y0, mut width, mut height) = if let Some(region) = opts.region.as_ref() {
        (region.x0, region.y0, region.width, region.height)
    } else if let Some(rect) = shot.image_content_rect.as_ref() {
        (rect.left, rect.top, rect.width, rect.height)
    } else {
        (0, 0, image_w, image_h)
    };

    x0 = x0.min(image_w.saturating_sub(1));
    y0 = y0.min(image_h.saturating_sub(1));
    width = width.min(image_w.saturating_sub(x0)).max(1);
    height = height.min(image_h.saturating_sub(y0)).max(1);

    let max_points = opts.max_points.unwrap_or(64).clamp(4, 196);
    let aspect = (width as f64 / height.max(1) as f64).clamp(0.25, 4.0);
    let mut cols = ((max_points as f64 * aspect).sqrt().ceil() as u32).clamp(2, max_points);
    let mut rows = ((max_points as f64) / cols as f64).ceil() as u32;
    rows = rows.max(2);
    while rows.saturating_mul(cols) > max_points && rows > 2 {
        rows -= 1;
    }
    while rows.saturating_mul(cols) > max_points && cols > 2 {
        cols -= 1;
    }

    let mut marks = Vec::with_capacity(rows.saturating_mul(cols) as usize);
    for row in 0..rows {
        for col in 0..cols {
            if marks.len() >= max_points as usize {
                break;
            }
            let x = x0 as f64 + ((col as f64 + 0.5) * width as f64 / cols as f64);
            let y = y0 as f64 + ((row as f64 + 0.5) * height as f64 / rows as f64);
            let x = x.round().clamp(0.0, image_w.saturating_sub(1) as f64) as i32;
            let y = y.round().clamp(0.0, image_h.saturating_sub(1) as f64) as i32;
            let box_size_i32 = if width.min(height) < 180 { 18 } else { 24 };
            let half = box_size_i32 / 2;
            let fx = (x - half).max(0) as u32;
            let fy = (y - half).max(0) as u32;
            let box_size = box_size_i32 as u32;
            let fw = box_size.min(image_w.saturating_sub(fx)).max(1);
            let fh = box_size.min(image_h.saturating_sub(fy)).max(1);
            marks.push(VisualMark {
                i: marks.len() as u32,
                x,
                y,
                frame_image: Some((fx, fy, fw, fh)),
                label: None,
            });
        }
    }

    if marks.is_empty() {
        return Err(BitFunError::tool(
            "build_visual_mark_view: no visual marks generated for the requested region"
                .to_string(),
        ));
    }
    Ok(marks)
}

fn visual_marks_to_overlay_elements(
    marks: &[VisualMark],
) -> Vec<bitfun_core::agentic::tools::computer_use_host::InteractiveElement> {
    marks
        .iter()
        .map(
            |mark| bitfun_core::agentic::tools::computer_use_host::InteractiveElement {
                i: mark.i,
                node_idx: mark.i,
                role: "VisualMark".to_string(),
                subrole: None,
                label: mark.label.clone(),
                frame_image: mark.frame_image,
                frame_global: None,
                enabled: true,
                focused: false,
                ax_actionable: false,
            },
        )
        .collect()
}

pub(super) fn detect_regular_grid_rect_from_screenshot(
    shot: &ComputerScreenshot,
    rows: u32,
    cols: u32,
) -> BitFunResult<(i32, i32, u32, u32)> {
    if rows < 2 || cols < 2 {
        return Err(BitFunError::tool(
            "visual_grid requires rows and cols >= 2".to_string(),
        ));
    }

    let img = image::load_from_memory(&shot.bytes)
        .map_err(|e| BitFunError::tool(format!("visual_grid: decode screenshot failed: {e}")))?
        .to_rgb8();
    let (image_w, image_h) = img.dimensions();
    let (left, top, width, height) = shot
        .image_content_rect
        .as_ref()
        .map(|r| (r.left, r.top, r.width, r.height))
        .unwrap_or((0, 0, image_w, image_h));
    let right = left.saturating_add(width).min(image_w);
    let bottom = top.saturating_add(height).min(image_h);
    if right <= left + 8 || bottom <= top + 8 {
        return Err(BitFunError::tool(
            "visual_grid: screenshot content rect is too small".to_string(),
        ));
    }

    let vertical = projection_darkness(&img, left, top, right, bottom, true);
    let horizontal = projection_darkness(&img, left, top, right, bottom, false);
    let x_seq = detect_regular_line_sequence(&vertical, cols, left)?;
    let y_seq = detect_regular_line_sequence(&horizontal, rows, top)?;
    let x0 = *x_seq.first().unwrap_or(&left);
    let x1 = *x_seq.last().unwrap_or(&right.saturating_sub(1));
    let y0 = *y_seq.first().unwrap_or(&top);
    let y1 = *y_seq.last().unwrap_or(&bottom.saturating_sub(1));
    let w = x1.saturating_sub(x0).saturating_add(1).max(2);
    let h = y1.saturating_sub(y0).saturating_add(1).max(2);

    let aspect = w as f64 / h.max(1) as f64;
    if !(0.5..=2.0).contains(&aspect) {
        return Err(BitFunError::tool(format!(
            "visual_grid: detected grid is implausibly non-square (x0={}, y0={}, width={}, height={}, aspect={:.2}); pass image_grid with an explicit rectangle",
            x0, y0, w, h, aspect
        )));
    }

    Ok((x0 as i32, y0 as i32, w, h))
}

fn projection_darkness(
    img: &image::RgbImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    vertical: bool,
) -> Vec<f64> {
    let len = (if vertical { right - left } else { bottom - top }) as usize;
    let mut out = vec![0.0; len];
    if vertical {
        for x in left..right {
            let mut sum = 0.0;
            for y in top..bottom {
                let p = img.get_pixel(x, y).0;
                let gray = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
                sum += (255.0 - gray).max(0.0);
            }
            out[(x - left) as usize] = sum / (bottom - top).max(1) as f64;
        }
    } else {
        for y in top..bottom {
            let mut sum = 0.0;
            for x in left..right {
                let p = img.get_pixel(x, y).0;
                let gray = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
                sum += (255.0 - gray).max(0.0);
            }
            out[(y - top) as usize] = sum / (right - left).max(1) as f64;
        }
    }
    smooth_projection(&out, 2)
}

fn smooth_projection(values: &[f64], radius: usize) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(values.len());
    for i in 0..values.len() {
        let start = i.saturating_sub(radius);
        let end = (i + radius + 1).min(values.len());
        let sum: f64 = values[start..end].iter().sum();
        out.push(sum / (end - start).max(1) as f64);
    }
    out
}

fn detect_regular_line_sequence(
    projection: &[f64],
    count: u32,
    offset: u32,
) -> BitFunResult<Vec<u32>> {
    if projection.len() < count as usize {
        return Err(BitFunError::tool(
            "visual_grid: projection is smaller than requested grid count".to_string(),
        ));
    }
    let mut sorted = projection.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let baseline = sorted[sorted.len() / 2];
    let adjusted: Vec<f64> = projection
        .iter()
        .map(|v| (*v - baseline).max(0.0))
        .collect();
    let mut adjusted_sorted = adjusted.clone();
    adjusted_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let threshold = adjusted_sorted
        [(adjusted_sorted.len() * 95 / 100).min(adjusted_sorted.len().saturating_sub(1))]
    .max(1.0);
    let mut peaks: Vec<usize> = Vec::new();
    let min_gap = ((projection.len() as f64 / count.max(1) as f64) * 0.35).round() as usize;
    let mut i = 0usize;
    while i < projection.len() {
        if adjusted[i] < threshold {
            i += 1;
            continue;
        }
        let start = i;
        let mut best = i;
        let mut best_score = adjusted[i];
        while i < adjusted.len() && adjusted[i] >= threshold {
            if adjusted[i] > best_score {
                best = i;
                best_score = adjusted[i];
            }
            i += 1;
        }
        let end = i.saturating_sub(1);
        let center = if best_score <= threshold {
            (start + end) / 2
        } else {
            best
        };
        if let Some(last) = peaks.last_mut() {
            if center.saturating_sub(*last) < min_gap.max(2) {
                if adjusted[center] > adjusted[*last] {
                    *last = center;
                }
                continue;
            }
        }
        peaks.push(center);
    }
    if peaks.len() < 2 {
        if let Some(fallback) = top_regular_positions(&adjusted, count, offset, min_gap.max(2)) {
            return Ok(fallback);
        }
        return Err(BitFunError::tool(
            "visual_grid: could not find enough line peaks".to_string(),
        ));
    }

    let mut best: Option<(f64, Vec<u32>)> = None;
    let desired = count as usize;
    for a_idx in 0..peaks.len() {
        for b_idx in (a_idx + 1)..peaks.len() {
            let first = peaks[a_idx] as f64;
            let last = peaks[b_idx] as f64;
            let span = last - first;
            if span < desired.saturating_sub(1).max(1) as f64 * 4.0 {
                continue;
            }
            let step = span / desired.saturating_sub(1).max(1) as f64;
            let tolerance = (step * 0.18).max(3.0);
            let mut positions = Vec::with_capacity(desired);
            let mut score = 0.0;
            let mut matched = 0usize;
            for k in 0..desired {
                let expected = first + k as f64 * step;
                let nearest = peaks
                    .iter()
                    .min_by(|a, b| {
                        ((**a as f64 - expected).abs())
                            .partial_cmp(&((**b as f64 - expected).abs()))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .copied();
                let pos = if let Some(p) = nearest {
                    if (p as f64 - expected).abs() <= tolerance {
                        matched += 1;
                        p as f64
                    } else {
                        expected
                    }
                } else {
                    expected
                };
                let idx = pos
                    .round()
                    .clamp(0.0, projection.len().saturating_sub(1) as f64)
                    as usize;
                score += adjusted[idx];
                positions.push(offset + idx as u32);
            }
            if matched < (desired * 2 / 3).max(2) {
                continue;
            }
            score += matched as f64 * threshold;
            score += span * 0.02;
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, positions));
            }
        }
    }

    best.map(|(_, positions)| positions)
        .or_else(|| top_regular_positions(&adjusted, count, offset, min_gap.max(2)))
        .ok_or_else(|| {
            BitFunError::tool(
                "visual_grid: no regular grid sequence detected; pass image_grid with an explicit rectangle or build_visual_mark_view to choose a point"
                    .to_string(),
            )
        })
}

fn top_regular_positions(
    scores: &[f64],
    count: u32,
    offset: u32,
    min_gap: usize,
) -> Option<Vec<u32>> {
    let desired = count as usize;
    let mut ranked: Vec<usize> = (0..scores.len()).collect();
    ranked.sort_by(|a, b| {
        scores[*b]
            .partial_cmp(&scores[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut selected: Vec<usize> = Vec::with_capacity(desired);
    for idx in ranked {
        if scores[idx] <= 0.0 {
            break;
        }
        if selected.iter().any(|s| idx.abs_diff(*s) < min_gap.max(2)) {
            continue;
        }
        selected.push(idx);
        if selected.len() == desired {
            break;
        }
    }
    if selected.len() < desired {
        return None;
    }
    selected.sort_unstable();
    Some(
        selected
            .into_iter()
            .map(|idx| offset + idx as u32)
            .collect(),
    )
}

/// Returns `true` if the error reported by `resolve_interactive_index`
/// is the recoverable `STALE_INTERACTIVE_VIEW` variant. We match on the
/// error text rather than introducing a typed error enum because every
/// `BitFunError::tool` is already string-based throughout the host
/// surface; adding a new variant would ripple through ~40 callers.
fn is_stale_interactive_view_error(err: &BitFunError) -> bool {
    err.to_string().contains("STALE_INTERACTIVE_VIEW")
}

fn is_stale_visual_mark_view_error(err: &BitFunError) -> bool {
    err.to_string().contains("STALE_VISUAL_MARK_VIEW")
}

impl DesktopComputerUseHost {
    /// Return the image-pixel center `(x, y)` of the cached interactive
    /// element with the given `i`, when its `frame_image` is known. Used
    /// as a pointer-click fallback in `interactive_click` when AXPress
    /// fails (Electron / Canvas / custom-drawn surfaces).
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    async fn cached_interactive_image_center(
        &self,
        app: &AppSelector,
        i: u32,
    ) -> Option<(i32, i32)> {
        let pid = resolve_pid(self, app).await.ok()?;
        let s = self.state.lock().ok()?;
        let cached = s.interactive_view_cache.get(&pid)?;
        let el = cached.elements.iter().find(|e| e.i == i)?;
        let (ix, iy, iw, ih) = el.frame_image?;
        Some((
            (ix as i64 + (iw as i64) / 2) as i32,
            (iy as i64 + (ih as i64) / 2) as i32,
        ))
    }

    /// Resolve an `interactive_*` `i` index into the underlying AX `node_idx`
    /// using the per-pid cache populated by `build_interactive_view`. Returns
    /// a `STALE_INTERACTIVE_VIEW` tool error when the digest no longer matches
    /// (i.e. the UI changed between view + action) so the caller can re-build
    /// the interactive view before retrying.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    async fn resolve_interactive_index(
        &self,
        app: &AppSelector,
        i: u32,
        before_digest: Option<&str>,
    ) -> BitFunResult<u32> {
        let pid = resolve_pid(self, app).await?;
        let s = self
            .state
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        let cached = s.interactive_view_cache.get(&pid).ok_or_else(|| {
            BitFunError::tool(
                "INTERACTIVE_VIEW_MISSING: call `build_interactive_view` before `interactive_*` actions"
                    .to_string(),
            )
        })?;
        if let Some(want) = before_digest {
            let want = want.trim();
            if !want.is_empty() {
                let matches = if want.len() >= 8 && want.len() <= cached.digest.len() {
                    cached.digest.starts_with(want)
                } else {
                    want == cached.digest
                };
                if !matches {
                    return Err(BitFunError::tool(format!(
                        "STALE_INTERACTIVE_VIEW: before_view_digest={} but current cached digest={}; re-call `build_interactive_view` and reuse the new digest (full or >=8-char prefix)",
                        want, cached.digest
                    )));
                }
            }
        }
        let el = cached.elements.iter().find(|e| e.i == i).ok_or_else(|| {
            BitFunError::tool(format!(
                "INTERACTIVE_INDEX_OUT_OF_RANGE: i={} not in cached view (len={})",
                i,
                cached.elements.len()
            ))
        })?;
        Ok(el.node_idx)
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    async fn resolve_visual_mark(
        &self,
        app: &AppSelector,
        i: u32,
        before_digest: Option<&str>,
    ) -> BitFunResult<VisualMark> {
        let pid = resolve_pid(self, app).await?;
        let s = self
            .state
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        let cached = s.visual_mark_cache.get(&pid).ok_or_else(|| {
            BitFunError::tool(
                "VISUAL_MARK_VIEW_MISSING: call `build_visual_mark_view` before `visual_click`"
                    .to_string(),
            )
        })?;
        if let Some(want) = before_digest {
            let want = want.trim();
            if !want.is_empty() {
                let matches = if want.len() >= 8 && want.len() <= cached.digest.len() {
                    cached.digest.starts_with(want)
                } else {
                    want == cached.digest
                };
                if !matches {
                    return Err(BitFunError::tool(format!(
                        "STALE_VISUAL_MARK_VIEW: before_view_digest={} but current cached digest={}; re-call `build_visual_mark_view` and reuse the new digest (full or >=8-char prefix)",
                        want, cached.digest
                    )));
                }
            }
        }
        cached
            .marks
            .iter()
            .find(|mark| mark.i == i)
            .cloned()
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "VISUAL_INDEX_OUT_OF_RANGE: i={} not in cached visual mark view (len={})",
                    i,
                    cached.marks.len()
                ))
            })
    }
}
