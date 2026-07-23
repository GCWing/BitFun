//! Windows multi-tier screen capture: `PrintWindow` + GDI `BitBlt`, with DWM
//! extended-frame crop and occlusion detection.
//!
//! Ported from cua-driver-rs v0.6.8 (`platform-windows/src/capture.rs`).
//!
//! ## Tiered capture fallback chain
//!
//!   1. **`PrintWindow(PW_RENDERFULLCONTENT)`** — renders a window's contents
//!      even when occluded or off-screen, for GDI-backed surfaces. Sized to the
//!      whole window (`GetWindowRect`), not just the client area, so non-client
//!      chrome (title bar, VCL button strips) is captured.
//!   2. **WGC (Windows.Graphics.Capture)** — occlusion-immune UWP /
//!      DirectComposition capture through the dedicated WGC module.
//!   3. **Screen-region `BitBlt` fallback** — when WGC is unavailable or fails,
//!      `BitBlt` the matching pixels off the desktop DC. Works when the target is
//!      on-screen and not occluded.
//!
//! ## DWM extended-frame crop
//!
//! `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` reports the rect
//! *without* the invisible drop-shadow margin Win10+ draws around every
//! top-level window. The bitmap is cropped to it (with a 1-px inset) so the
//! result has no black trim or Win11 rounded-corner hairline.
//!
//! ## Occlusion flag
//!
//! [`screenshot_window_capture`] reports an `occluded` flag — the flag
//! is `true` when the capture fell through to the screen-region `BitBlt` path
//! AND another window was visibly covering the target at sample time (see
//! [`target_is_obscured`]). In that case the bitmap reflects the *covering*
//! window's pixels, not the target's; callers that surface the image should
//! attach an explicit warning.
//!
//! Per-Monitor V2 DPI awareness note: `GetWindowRect`, `GetSystemMetrics`, and
//! `BitBlt` all operate in PHYSICAL pixels under PMv2, so no DPI/96 scaling is
//! applied (scaling would shift and oversize the captured region).

#![allow(dead_code)]

use bitfun_core::util::errors::{BitFunError, BitFunResult};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use log::warn;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    GetWindowDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    RGBQUAD, SRCCOPY,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetWindowRect, IsIconic, WindowFromPoint, GA_ROOT,
};

/// `PW_RENDERFULLCONTENT` (0x2): render window contents even when occluded or
/// off-screen (GDI-backed surfaces only).
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2u32);

/// 1-px inset applied to the DWM extended-frame crop to strip the dark hairline
/// Win11 dialogs paint at the rounded-corner edge.
const DWM_CROP_INSET_PX: i32 = 1;

/// Encode raw BGRA bytes (top-down, row-major, as `GetDIBits` returns) as PNG.
///
/// Swaps B <-> R in place then defers to the `image` crate's encoder (BGRA is
/// not a PNG-encodable channel order). Alpha is preserved as-is, matching the
/// cua source. Caller guarantees `bgra.len() == width * height * 4`.
fn encode_bgra_to_png(bgra: &[u8], width: u32, height: u32) -> BitFunResult<Vec<u8>> {
    if bgra.len() as u64 != (width as u64) * (height as u64) * 4 {
        return Err(BitFunError::service(format!(
            "encode_bgra_to_png: buffer size {} != width({width}) * height({height}) * 4",
            bgra.len()
        )));
    }
    let mut rgba = bgra.to_vec();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2); // B <-> R, keep G + A
    }
    let buf: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, rgba)
        .ok_or_else(|| {
            BitFunError::io(format!(
                "invalid RGBA buffer for width={width} height={height}"
            ))
        })?;
    let mut out = Vec::new();
    DynamicImage::ImageRgba8(buf)
        .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| BitFunError::io(format!("PNG encode failed: {e}")))?;
    Ok(out)
}

/// Detect the all-black bitmap `PrintWindow` returns for DirectComposition-backed
/// UWP / WinUI3 surfaces.
///
/// Sparse-samples (every Nth pixel so the heuristic is cheap even on 4K windows)
/// and reports `true` when > 99.5% of sampled pixels are black (`B+G+R == 0`,
/// alpha ignored — that's the all-zero pattern DirectComposition leaves behind).
/// The threshold is intentionally aggressive so legitimate dark UI does not trip
/// the fallback.
fn is_mostly_black_bgra(data: &[u8], width: u32, height: u32) -> bool {
    if data.len() < 16 {
        return true;
    }
    let pixel_count = (width as usize).saturating_mul(height as usize);
    if pixel_count == 0 {
        return true;
    }
    let available = data.len() / 4;
    if available == 0 {
        return true;
    }
    let sample_count = available.min(pixel_count);
    let stride = (sample_count / 1024).max(1);
    let mut sampled = 0usize;
    let mut black = 0usize;
    for i in (0..sample_count).step_by(stride) {
        let off = i * 4;
        if off + 2 < data.len() {
            if data[off] == 0 && data[off + 1] == 0 && data[off + 2] == 0 {
                black += 1;
            }
            sampled += 1;
        }
    }
    // > 99.5% of sampled pixels are black -> treat as failed render.
    sampled > 0 && (black * 200) >= (sampled * 199)
}

/// Probe whether `hwnd` is currently obscured by another window.
///
/// Samples `WindowFromPoint` at 5 points (4 corners inset 2 px + center) and
/// considers the target occluded when 2+ samples return a window whose root
/// ancestor isn't `hwnd`'s root ancestor. The 2-of-5 threshold avoids false
/// positives from a single corner covered by a non-opaque layered overlay (e.g.
/// an agent cursor). Callers that surface a screen-region `BitBlt` result should
/// use this to warn that the bitmap may show the *covering* window's pixels.
fn target_is_obscured(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return false;
    }
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    if w <= 4 || h <= 4 {
        return false;
    }
    // 5 sample points: 4 corners (inset 2 px) + center.
    let pts: [(i32, i32); 5] = [
        (rect.left + 2, rect.top + 2),
        (rect.right - 3, rect.top + 2),
        (rect.left + 2, rect.bottom - 3),
        (rect.right - 3, rect.bottom - 3),
        ((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2),
    ];
    let target_root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let mut covered = 0usize;
    for (x, y) in &pts {
        let owner = unsafe { WindowFromPoint(POINT { x: *x, y: *y }) };
        if owner.is_invalid() {
            continue;
        }
        let owner_root = unsafe { GetAncestor(owner, GA_ROOT) };
        if owner_root != target_root {
            covered += 1;
        }
    }
    // 2-of-5 threshold: a single corner covered can be a non-opaque layered
    // overlay; two or more sample points missing means real content is covered.
    covered >= 2
}

/// Return `true` when `hwnd` is minimized (iconic).
///
/// `GetWindowRect` on an iconic HWND returns the off-screen "iconic position"
/// and `PrintWindow` paints nothing — the result is a degenerate all-black
/// ~28x160 PNG that an agent can't tell apart from a real blank screen.
/// Guarding here lets callers restore the window before retrying.
fn is_iconic(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }
    unsafe { IsIconic(hwnd).as_bool() }
}

/// Capture a window via Windows.Graphics.Capture (WGC), returning BGRA pixels +
/// `(width, height)`.
///
/// WGC is the only API that returns a UWP target's own composited pixels even
/// when occluded by another window.
fn screenshot_window_via_wgc(hwnd: HWND) -> BitFunResult<(Vec<u8>, u32, u32)> {
    crate::computer_use::windows_wgc_capture::capture_window_bgra(hwnd)
}

/// Fallback capture path: `BitBlt` the desktop DC over the rectangle covered by
/// `hwnd`'s on-screen bounds.
///
/// Works for UWP / DirectComposition surfaces that `PrintWindow` can't reach,
/// as long as the window is on-screen and not occluded. Returns
/// `(bgra_pixels, width, height)`.
unsafe fn screenshot_via_screen_region(hwnd: HWND) -> BitFunResult<(Vec<u8>, i32, i32)> {
    let mut rect = RECT::default();
    // SAFETY: `rect` is a valid out-parameter and a stale/invalid HWND is
    // reported by the Win32 API.
    unsafe { GetWindowRect(hwnd, &mut rect) }.map_err(|e| {
        BitFunError::service(format!("screen-region fallback: GetWindowRect failed: {e}"))
    })?;
    // Under Per-Monitor V2 DPI awareness, GetWindowRect returns PHYSICAL pixels
    // and BitBlt operates in physical pixels too — use the rect as-is.
    let physical_left = rect.left;
    let physical_top = rect.top;
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    if w <= 0 || h <= 0 {
        return Err(BitFunError::service(format!(
            "screen-region fallback: window has zero/negative bounds: {w}x{h}"
        )));
    }
    // SAFETY: the DC and bitmap handles are created in this block, remain live
    // through the copy, and are restored/released before the function returns.
    let screen_dc = unsafe { GetDC(None) }; // NULL HWND -> desktop DC
    let mem_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
    let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, w, h) };
    let old_bitmap = unsafe { SelectObject(mem_dc, bitmap.into()) };
    let blt_ok = unsafe {
        BitBlt(
            mem_dc,
            0,
            0,
            w,
            h,
            Some(screen_dc),
            physical_left,
            physical_top,
            SRCCOPY,
        )
    };
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (w * h * 4) as u32,
            ..Default::default()
        },
        bmiColors: [RGBQUAD::default(); 1],
    };
    let pixel_count = (w * h) as usize;
    let mut pixels = vec![0u8; pixel_count * 4];
    let ok = unsafe {
        GetDIBits(
            mem_dc,
            bitmap,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        SelectObject(mem_dc, old_bitmap);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);
    }
    if blt_ok.is_err() {
        return Err(BitFunError::service(format!(
            "screen-region fallback: BitBlt failed: {blt_ok:?}"
        )));
    }
    if ok == 0 {
        return Err(BitFunError::service(
            "screen-region fallback: GetDIBits returned 0",
        ));
    }
    Ok((pixels, w, h))
}

/// A captured window bitmap plus the screen-space geometry it maps to.
///
/// `origin_x`/`origin_y` are the **physical** screen coordinates of the
/// returned bitmap's top-left pixel, and `width`/`height` are its pixel
/// dimensions. Together they let the desktop host build a `PointerMap` that
/// converts an image pixel the vision model picked back into the screen
/// coordinate a background click should target.
pub(super) struct WindowCapture {
    pub png: Vec<u8>,
    pub occluded: bool,
    pub origin_x: i32,
    pub origin_y: i32,
    pub width: u32,
    pub height: u32,
}

/// Capture a window by HWND, returning the encoded PNG plus the screen-space
/// rectangle the bitmap covers (see [`WindowCapture`]).
///
/// Tiered fallback chain:
/// - **Primary**: `PrintWindow(PW_RENDERFULLCONTENT)` — captures occluded /
///   off-screen GDI windows.
/// - **WGC**: [`screenshot_window_via_wgc`] when PrintWindow is mostly black.
/// - **Fallback**: screen-region `BitBlt` when WGC fails. The `occluded` flag
///   is `true` when this path is taken AND [`target_is_obscured`] reports another
///   window covering the target — in that case the bitmap shows the *covering*
///   window's pixels.
///
/// Minimized windows are rejected up front via [`is_iconic`]. The DWM
/// extended-frame bounds are used to crop the invisible drop-shadow margin; the
/// returned `origin_*` account for that crop so coordinate mapping stays exact.
pub(super) fn screenshot_window_capture(hwnd: HWND) -> BitFunResult<WindowCapture> {
    unsafe { screenshot_window_bytes_unsafe(hwnd) }
}

unsafe fn screenshot_window_bytes_unsafe(hwnd: HWND) -> BitFunResult<WindowCapture> {
    if hwnd.is_invalid() {
        return Err(BitFunError::service(
            "screenshot_window_bytes: invalid HWND",
        ));
    }
    // Bail on minimized (iconic) windows before any capture path: GetWindowRect
    // on an iconic HWND returns the off-screen iconic position and PrintWindow
    // paints nothing. The degenerate all-black PNG wastes model turns retrying
    // against a window minimized to the taskbar.
    if is_iconic(hwnd) {
        return Err(BitFunError::service(
            "cannot capture minimized window: it has no rendered content. \
             Restore the window first.",
        ));
    }
    // Tracks the screen-space top-left of the returned bitmap; set from the
    // window rect below and updated when the DWM crop trims the drop-shadow
    // margin so coordinate mapping stays exact.
    let mut origin_x: i32;
    let mut origin_y: i32;

    // Size the buffer to the WHOLE window (GetWindowRect), not just the client
    // area — PrintWindow draws the entire window at 1:1 from (0, 0). A
    // client-sized buffer loses non-client chrome (e.g. VCL/SAL dialogs put the
    // bottom button strip outside the standard Win32 client area).
    let mut win_rect = RECT::default();
    // SAFETY: `win_rect` is a valid out-parameter and `hwnd` was checked above.
    unsafe { GetWindowRect(hwnd, &mut win_rect) }.map_err(|e| {
        BitFunError::service(format!(
            "screenshot_window_bytes: GetWindowRect failed: {e}"
        ))
    })?;
    let w = win_rect.right - win_rect.left;
    let h = win_rect.bottom - win_rect.top;
    if w <= 0 || h <= 0 {
        return Err(BitFunError::service(format!(
            "screenshot_window_bytes: window has zero/negative size: {w}x{h}"
        )));
    }
    origin_x = win_rect.left;
    origin_y = win_rect.top;

    // SAFETY: the DC and bitmap handles are created here, remain live through
    // the capture, and are restored/released immediately after `GetDIBits`.
    let screen_dc = unsafe { GetWindowDC(Some(hwnd)) };
    let mem_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
    let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, w, h) };
    let old_bitmap = unsafe { SelectObject(mem_dc, bitmap.into()) };

    // Primary: PrintWindow with PW_RENDERFULLCONTENT. If it refuses, BitBlt
    // straight from the window DC as a last resort (best-effort — a failure
    // here surfaces downstream via the mostly-black detection + fallback).
    let pw_ok = unsafe { PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT) };
    if !pw_ok.as_bool() {
        let _ = unsafe { BitBlt(mem_dc, 0, 0, w, h, Some(screen_dc), 0, 0, SRCCOPY) };
    }

    // DWM extended-frame bounds: strip the invisible drop-shadow margin that
    // GetWindowRect counts but PrintWindow doesn't paint (leaves a black trim).
    // Best-effort — if the DWM call fails, keep the full-window bitmap as-is.
    let dwm_rect: Option<RECT> = {
        let mut r = RECT::default();
        // SAFETY: `r` is a live `RECT` out-buffer with the exact byte size
        // required by `DWMWA_EXTENDED_FRAME_BOUNDS`.
        let hr = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut r as *mut _ as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
        };
        hr.ok().map(|_| r)
    };

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (w * h * 4) as u32,
            ..Default::default()
        },
        bmiColors: [RGBQUAD::default(); 1],
    };

    let pixel_count = (w * h) as usize;
    let mut pixels = vec![0u8; pixel_count * 4];
    let ok = unsafe {
        GetDIBits(
            mem_dc,
            bitmap,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    };

    unsafe {
        SelectObject(mem_dc, old_bitmap);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem_dc);
        ReleaseDC(Some(hwnd), screen_dc);
    }

    if ok == 0 {
        return Err(BitFunError::service(
            "screenshot_window_bytes: GetDIBits returned 0",
        ));
    }

    // Crop to the DWM extended-frame bounds (with a 1-px inset) to remove the
    // invisible-shadow margin and the Win11 rounded-corner hairline. The crop
    // offset shifts the bitmap origin in screen space, so fold it into
    // origin_x/origin_y for coordinate mapping.
    let (pixels, w, h, crop_off_x, crop_off_y) =
        crop_to_dwm_frame(pixels, w, h, win_rect, dwm_rect);
    origin_x += crop_off_x;
    origin_y += crop_off_y;

    // Detect the all-black bitmap PrintWindow returns for DirectComposition-
    // backed surfaces. Recovery order:
    //   1. WGC (occlusion-immune; works for UWP / DirectComposition).
    //   2. Screen-region BitBlt (works when target is on-screen & visible),
    //      flagged occluded via target_is_obscured when another window covers it.
    if is_mostly_black_bgra(&pixels, w as u32, h as u32) {
        if let Ok((alt_pixels, alt_w, alt_h)) = screenshot_window_via_wgc(hwnd) {
            return Ok(WindowCapture {
                png: encode_bgra_to_png(&alt_pixels, alt_w, alt_h)?,
                occluded: false,
                origin_x: win_rect.left,
                origin_y: win_rect.top,
                width: alt_w,
                height: alt_h,
            });
        }
        let occluded = target_is_obscured(hwnd);
        match unsafe { screenshot_via_screen_region(hwnd) } {
            Ok((alt_pixels, alt_w, alt_h)) => {
                // Screen-region BitBlt captures the full GetWindowRect region
                // (no DWM crop), so its origin is the raw window top-left.
                return Ok(WindowCapture {
                    png: encode_bgra_to_png(&alt_pixels, alt_w as u32, alt_h as u32)?,
                    occluded,
                    origin_x: win_rect.left,
                    origin_y: win_rect.top,
                    width: alt_w as u32,
                    height: alt_h as u32,
                });
            }
            Err(e) => {
                warn!(
                    "screenshot_window_bytes: PrintWindow returned a mostly-black bitmap \
                     (UWP / DirectComposition target?); screen-region fallback failed: {e}"
                );
                // Fall through — return the (black) PrintWindow result so the
                // caller still gets an image rather than an outright error.
            }
        }
    }

    // PrintWindow reads from the target's own DC, so the bitmap is the target's
    // pixels even when occluded — no occluded warning on this path.
    Ok(WindowCapture {
        png: encode_bgra_to_png(&pixels, w as u32, h as u32)?,
        occluded: false,
        origin_x,
        origin_y,
        width: w as u32,
        height: h as u32,
    })
}

/// Crop `pixels` (BGRA, top-down) to the DWM extended-frame bounds, removing the
/// invisible drop-shadow margin PrintWindow doesn't paint. No-op when the DWM
/// rect is unavailable or the computed crop is out of bounds.
///
/// Returns `(pixels, width, height, off_x, off_y)` where `off_x`/`off_y` are the
/// offset (in window-local pixels) from the original window top-left to the
/// cropped content's top-left — `0` when the crop is a no-op. Callers fold these
/// into the screen-space origin so coordinate mapping stays exact.
fn crop_to_dwm_frame(
    pixels: Vec<u8>,
    w: i32,
    h: i32,
    win_rect: RECT,
    dwm_rect: Option<RECT>,
) -> (Vec<u8>, i32, i32, i32, i32) {
    let Some(dwm) = dwm_rect else {
        return (pixels, w, h, 0, 0);
    };
    let off_x = (dwm.left - win_rect.left) + DWM_CROP_INSET_PX;
    let off_y = (dwm.top - win_rect.top) + DWM_CROP_INSET_PX;
    let crop_w = (dwm.right - dwm.left) - 2 * DWM_CROP_INSET_PX;
    let crop_h = (dwm.bottom - dwm.top) - 2 * DWM_CROP_INSET_PX;
    if off_x < 0
        || off_y < 0
        || crop_w <= 0
        || crop_h <= 0
        || off_x + crop_w > w
        || off_y + crop_h > h
    {
        return (pixels, w, h, 0, 0);
    }
    let stride_full = (w * 4) as usize;
    let stride_crop = (crop_w * 4) as usize;
    let mut cropped = vec![0u8; (crop_w * crop_h * 4) as usize];
    for row in 0..crop_h as usize {
        let src_row = (off_y as usize + row) * stride_full + (off_x as usize) * 4;
        let dst_row = row * stride_crop;
        cropped[dst_row..dst_row + stride_crop]
            .copy_from_slice(&pixels[src_row..src_row + stride_crop]);
    }
    (cropped, crop_w, crop_h, off_x, off_y)
}
