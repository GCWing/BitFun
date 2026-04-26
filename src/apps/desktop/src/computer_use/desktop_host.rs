//! Cross-platform `ComputerUseHost` via `screenshots` + `enigo`.

use async_trait::async_trait;
use bitfun_core::agentic::tools::computer_use_host::{
    clamp_point_crop_half_extent, ActionRecord, AppClickParams, AppInfo, AppSelector,
    AppStateSnapshot, AppWaitPredicate, ClickTarget, ComputerScreenshot, ComputerUseDisplayInfo,
    ComputerUseHost, ComputerUseImageContentRect, ComputerUseImageGlobalBounds,
    ComputerUseImplicitScreenshotCenter, ComputerUseInteractionScreenshotKind,
    ComputerUseInteractionState, ComputerUseLastMutationKind, ComputerUseNavigateQuadrant,
    ComputerUseNavigationRect, ComputerUsePermissionSnapshot, ComputerUseScreenshotParams,
    ComputerUseScreenshotRefinement, ComputerUseSessionSnapshot, InteractiveActionResult,
    InteractiveClickParams, InteractiveScrollParams, InteractiveTypeTextParams, InteractiveView,
    InteractiveViewOpts, LoopDetectionResult, OcrRegionNative, ScreenshotCropCenter,
    UiElementLocateQuery, UiElementLocateResult, VisualActionResult, VisualClickParams, VisualMark,
    VisualMarkView, VisualMarkViewOpts, COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE,
    COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use bitfun_core::agentic::tools::computer_use_host::{
    ComputerUseForegroundApplication, ComputerUsePointerGlobal,
};
use bitfun_core::agentic::tools::computer_use_optimizer::ComputerUseOptimizer;
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, Rgb, RgbImage};
use log::{debug, info, warn};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;
use screenshots::display_info::DisplayInfo;
use screenshots::Screen;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Default pointer overlay; replace `assets/computer_use_pointer.svg` and rebuild to customize.
/// Hotspot in SVG user space must stay at **(0,0)** (arrow tip).
const POINTER_OVERLAY_SVG: &str = include_str!("../../assets/computer_use_pointer.svg");

/// Screenshot cache validity duration (ms) - reuse full capture for subsequent crops within this window
const SCREENSHOT_CACHE_TTL_MS: u64 = 300;

/// Error text when `click_needs_fresh_screenshot` blocks `click` or Enter `key_chord` (single source of truth).
const STALE_CAPTURE_TOOL_MESSAGE: &str = "Computer use refused: call **`screenshot`** first. Use a **bare** `screenshot` (do not set `screenshot_reset_navigation`) — the host applies a **~500×500** crop around the **mouse**. Before Return/Enter in a focused text field, set **`screenshot_implicit_center`**: **`text_caret`**. This is required after the pointer moved since the last capture, before **`click`** or before **`key_chord`** that includes Return/Enter.";

/// Relative nudges (`pointer_move_rel`, `ComputerUseMouseStep`) right after a model-driven screenshot are almost always wrong when deltas are guessed from the image; block until a trusted absolute move.
const VISION_PIXEL_NUDGE_AFTER_SCREENSHOT_MSG: &str = "Computer use refused: do not use `pointer_move_rel` or `ComputerUseMouseStep` immediately after a `screenshot` — nudging from the JPEG is inaccurate. First reposition with `move_to_text`, `click_element`, `locate` + `mouse_move` (`use_screen_coordinates`: true), or `mouse_move` using globals from tool JSON; then relative nudges are allowed if still needed.";

#[derive(Debug, Clone)]
struct ScreenshotCacheEntry {
    rgba: image::RgbaImage,
    screen: Screen,
    capture_time: Instant,
}

#[derive(Debug)]
struct PointerPixmapCache {
    w: u32,
    h: u32,
    /// Premultiplied RGBA8 (`tiny-skya` / `resvg` format).
    rgba: Vec<u8>,
}

static POINTER_PIXMAP_CACHE: OnceLock<Option<PointerPixmapCache>> = OnceLock::new();
static SCREENSHOT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn pointer_pixmap_cache() -> Option<&'static PointerPixmapCache> {
    POINTER_PIXMAP_CACHE
        .get_or_init(
            || match rasterize_pointer_svg(POINTER_OVERLAY_SVG, 0.3375) {
                Ok(p) => Some(p),
                Err(e) => {
                    warn!(
                        "computer_use: pointer SVG rasterize failed ({}); using fallback cross",
                        e
                    );
                    None
                }
            },
        )
        .as_ref()
}

fn rasterize_pointer_svg(svg: &str, scale: f32) -> Result<PointerPixmapCache, String> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let w = ((size.width() * scale).ceil() as u32).max(1);
    let h = ((size.height() * scale).ceil() as u32).max(1);
    let mut pixmap = Pixmap::new(w, h).ok_or_else(|| "pixmap allocation failed".to_string())?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(PointerPixmapCache {
        w,
        h,
        rgba: pixmap.data().to_vec(),
    })
}

/// Alpha-composite premultiplied RGBA onto `img` with SVG (0,0) at `(cx, cy)`.
fn blend_pointer_pixmap(img: &mut RgbImage, cx: i32, cy: i32, p: &PointerPixmapCache) {
    let iw = img.width() as i32;
    let ih = img.height() as i32;
    for row in 0..p.h {
        for col in 0..p.w {
            let i = ((row * p.w + col) * 4) as usize;
            if i + 3 >= p.rgba.len() {
                break;
            }
            let pr = p.rgba[i];
            let pg = p.rgba[i + 1];
            let pb = p.rgba[i + 2];
            let pa = p.rgba[i + 3] as u32;
            if pa == 0 {
                continue;
            }
            let px = cx + col as i32;
            let py = cy + row as i32;
            if px < 0 || py < 0 || px >= iw || py >= ih {
                continue;
            }
            let dst = img.get_pixel(px as u32, py as u32);
            let inv = 255 - pa;
            let nr = (pr as u32 + dst[0] as u32 * inv / 255).min(255) as u8;
            let ng = (pg as u32 + dst[1] as u32 * inv / 255).min(255) as u8;
            let nb = (pb as u32 + dst[2] as u32 * inv / 255).min(255) as u8;
            img.put_pixel(px as u32, py as u32, Rgb([nr, ng, nb]));
        }
    }
}

#[cfg(test)]
mod visual_grid_tests {
    use super::*;

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
            detect_regular_grid_rect_from_screenshot(&shot, 15, 15).expect("detect grid");
        assert!((x0 - left as i32).abs() <= 6, "x0={x0}");
        assert!((y0 - top as i32).abs() <= 6, "y0={y0}");
        assert!((width as i32 - size as i32).abs() <= 12, "width={width}");
        assert!((height as i32 - size as i32).abs() <= 12, "height={height}");
    }
}

fn draw_pointer_fallback_cross(img: &mut RgbImage, cx: i32, cy: i32) {
    const ARM: i32 = 2;
    const OUTLINE: Rgb<u8> = Rgb([255, 255, 255]);
    const CORE: Rgb<u8> = Rgb([40, 40, 48]);
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut plot = |x: i32, y: i32, c: Rgb<u8>| {
        if x >= 0 && x < w && y >= 0 && y < h {
            img.put_pixel(x as u32, y as u32, c);
        }
    };
    for t in -ARM..=ARM {
        for k in -1..=1 {
            plot(cx + t, cy + k, OUTLINE);
            plot(cx + k, cy + t, OUTLINE);
        }
    }
    for t in -ARM..=ARM {
        plot(cx + t, cy, CORE);
        plot(cx, cy + t, CORE);
    }
}

/// Returns the capture bitmap unchanged (no grid, rulers, or margins). Pointer overlays are applied later.
fn compose_computer_use_frame(
    content: RgbImage,
    _ruler_origin_x: u32,
    _ruler_origin_y: u32,
) -> (RgbImage, u32, u32) {
    (content, 0, 0)
}

#[allow(dead_code)] // legacy: crop logic disabled at the entry point in screenshot_display
fn implicit_confirmation_should_apply(
    click_needs: bool,
    params: &ComputerUseScreenshotParams,
) -> bool {
    // Applies on **every** bare `screenshot` while confirmation is required — including the
    // first capture in a session (`last_shot_refinement` may still be `None`), so click/Enter
    // guards get a ~500×500 around the mouse (or `text_caret` when requested) instead of full screen.
    //
    // **Always** apply when `click_needs` (even during quadrant/point-crop drill): previously we
    // skipped implicit crop while `navigation_focus` was Quadrant/PointCrop, which produced large
    // confirmation JPEGs; confirmation shots must stay ~500×500 around the pointer/caret.
    if !click_needs {
        return false;
    }
    if params.crop_center.is_some() || params.navigate_quadrant.is_some() || params.reset_navigation
    {
        return false;
    }
    true
}

fn global_to_native_full_pixel_center(
    gx: f64,
    gy: f64,
    native_w: u32,
    native_h: u32,
    d: &DisplayInfo,
) -> (u32, u32) {
    #[cfg(target_os = "macos")]
    {
        let geo = MacPointerGeo::from_display(native_w, native_h, d);
        let lx = gx - geo.disp_ox;
        let ly = gy - geo.disp_oy;
        if lx < 0.0 || lx >= geo.disp_w || ly < 0.0 || ly >= geo.disp_h {
            return clamp_center_to_native(native_w / 2, native_h / 2, native_w, native_h);
        }
        let full_ix = ((lx / geo.disp_w) * geo.full_px_w as f64).floor() as u32;
        let full_iy = ((ly / geo.disp_h) * geo.full_px_h as f64).floor() as u32;
        clamp_center_to_native(full_ix, full_iy, native_w, native_h)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let disp_w = d.width as f64;
        let disp_h = d.height as f64;
        if disp_w <= 0.0 || disp_h <= 0.0 || native_w == 0 || native_h == 0 {
            return (0, 0);
        }
        let lx = gx - d.x as f64;
        let ly = gy - d.y as f64;
        if lx < 0.0 || lx >= disp_w || ly < 0.0 || ly >= disp_h {
            return clamp_center_to_native(native_w / 2, native_h / 2, native_w, native_h);
        }
        let full_ix = ((lx / disp_w) * native_w as f64).floor() as u32;
        let full_iy = ((ly / disp_h) * native_h as f64).floor() as u32;
        clamp_center_to_native(full_ix, full_iy, native_w, native_h)
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
fn implicit_global_center_for_confirmation(
    center: ComputerUseImplicitScreenshotCenter,
    mx: f64,
    my: f64,
) -> (f64, f64) {
    match center {
        ComputerUseImplicitScreenshotCenter::Mouse => (mx, my),
        ComputerUseImplicitScreenshotCenter::TextCaret => {
            crate::computer_use::macos_ax_ui::global_point_for_text_caret_screenshot(mx, my)
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn implicit_global_center_for_confirmation(
    center: ComputerUseImplicitScreenshotCenter,
    mx: f64,
    my: f64,
) -> (f64, f64) {
    let _ = center;
    (mx, my)
}

/// JPEG quality for computer-use screenshots. Visually near-lossless tier; combined with the
/// adaptive byte-budget downscale below, oversize captures are halved until they fit
/// [`SCREENSHOT_MAX_BYTES`] so the model API receives a manageable payload without sacrificing
/// quality on small/medium app windows.
const JPEG_QUALITY: u8 = 85;

/// Soft byte budget for a single screenshot JPEG sent to the model. When the encoded image
/// exceeds this, the host halves the resolution (Lanczos3) and re-encodes, looping until it fits
/// or the long edge falls below [`SCREENSHOT_MIN_LONG_EDGE`].
const SCREENSHOT_MAX_BYTES: usize = 3 * 1024 * 1024;

/// Hard floor on the long edge during the byte-budget downscale loop, so a pathological
/// capture cannot be reduced to an unreadable thumbnail just to fit the budget.
const SCREENSHOT_MIN_LONG_EDGE: u32 = 512;

#[inline]
fn clamp_center_to_native(cx: u32, cy: u32, nw: u32, nh: u32) -> (u32, u32) {
    if nw == 0 || nh == 0 {
        return (0, 0);
    }
    let cx = cx.min(nw - 1);
    let cy = cy.min(nh - 1);
    (cx, cy)
}

/// Top-left and size of the native crop rectangle around `(cx, cy)`, clamped to the bitmap.
/// `half_px` is the distance from center to each edge (see [`clamp_point_crop_half_extent`]).
fn crop_rect_around_point_native(
    cx: u32,
    cy: u32,
    nw: u32,
    nh: u32,
    half_px: u32,
) -> (u32, u32, u32, u32) {
    let (cx, cy) = clamp_center_to_native(cx, cy, nw, nh);
    if nw == 0 || nh == 0 {
        return (0, 0, 1, 1);
    }
    let edge = half_px.saturating_mul(2);
    let tw = edge.min(nw).max(1);
    let th = edge.min(nh).max(1);
    let mut x0 = cx.saturating_sub(half_px);
    let mut y0 = cy.saturating_sub(half_px);
    if x0.saturating_add(tw) > nw {
        x0 = nw.saturating_sub(tw);
    }
    if y0.saturating_add(th) > nh {
        y0 = nh.saturating_sub(th);
    }
    (x0, y0, tw, th)
}

#[inline]
fn full_navigation_rect(nw: u32, nh: u32) -> ComputerUseNavigationRect {
    ComputerUseNavigationRect {
        x0: 0,
        y0: 0,
        width: nw.max(1),
        height: nh.max(1),
    }
}

fn intersect_navigation_rect(
    a: ComputerUseNavigationRect,
    b: ComputerUseNavigationRect,
) -> Option<ComputerUseNavigationRect> {
    let ax1 = a.x0.saturating_add(a.width);
    let ay1 = a.y0.saturating_add(a.height);
    let bx1 = b.x0.saturating_add(b.width);
    let by1 = b.y0.saturating_add(b.height);
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    let x1 = ax1.min(bx1);
    let y1 = ay1.min(by1);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    Some(ComputerUseNavigationRect {
        x0,
        y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

/// Expand `r` by `pad` pixels left/up/right/down, clamped to `0..max_w` × `0..max_h`.
fn expand_navigation_rect_edges(
    r: ComputerUseNavigationRect,
    pad: u32,
    max_w: u32,
    max_h: u32,
) -> ComputerUseNavigationRect {
    let x0 = r.x0.saturating_sub(pad);
    let y0 = r.y0.saturating_sub(pad);
    let x1 = r.x0.saturating_add(r.width).saturating_add(pad).min(max_w);
    let y1 = r.y0.saturating_add(r.height).saturating_add(pad).min(max_h);
    let width = x1.saturating_sub(x0).max(1);
    let height = y1.saturating_sub(y0).max(1);
    ComputerUseNavigationRect {
        x0,
        y0,
        width,
        height,
    }
}

fn quadrant_split_rect(
    r: ComputerUseNavigationRect,
    q: ComputerUseNavigateQuadrant,
) -> ComputerUseNavigationRect {
    let hw = r.width / 2;
    let hh = r.height / 2;
    let rw = r.width - hw;
    let rh = r.height - hh;
    match q {
        ComputerUseNavigateQuadrant::TopLeft => ComputerUseNavigationRect {
            x0: r.x0,
            y0: r.y0,
            width: hw,
            height: hh,
        },
        ComputerUseNavigateQuadrant::TopRight => ComputerUseNavigationRect {
            x0: r.x0 + hw,
            y0: r.y0,
            width: rw,
            height: hh,
        },
        ComputerUseNavigateQuadrant::BottomLeft => ComputerUseNavigationRect {
            x0: r.x0,
            y0: r.y0 + hh,
            width: hw,
            height: rh,
        },
        ComputerUseNavigateQuadrant::BottomRight => ComputerUseNavigationRect {
            x0: r.x0 + hw,
            y0: r.y0 + hh,
            width: rw,
            height: rh,
        },
    }
}

/// macOS: map JPEG/bitmap pixels to/from **CoreGraphics global display coordinates** (same as
/// `CGDisplayBounds` / `CGEventGetLocation`): origin at the **top-left of the main display**, Y
/// increases **downward**. Not AppKit bottom-left / Y-up.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct MacPointerGeo {
    disp_ox: f64,
    disp_oy: f64,
    disp_w: f64,
    disp_h: f64,
    full_px_w: u32,
    full_px_h: u32,
    crop_x0: u32,
    crop_y0: u32,
}

#[cfg(target_os = "macos")]
impl MacPointerGeo {
    fn from_display(full_w: u32, full_h: u32, d: &DisplayInfo) -> Self {
        Self {
            disp_ox: d.x as f64,
            disp_oy: d.y as f64,
            disp_w: d.width as f64,
            disp_h: d.height as f64,
            full_px_w: full_w,
            full_px_h: full_h,
            crop_x0: 0,
            crop_y0: 0,
        }
    }

    fn with_crop(mut self, x0: u32, y0: u32) -> Self {
        self.crop_x0 = x0;
        self.crop_y0 = y0;
        self
    }

    /// Map **continuous** framebuffer pixel center `(cx, cy)` (0.5 = middle of left/top pixel) to CG global.
    fn full_pixel_center_to_global_f64(&self, cx: f64, cy: f64) -> BitFunResult<(f64, f64)> {
        if self.disp_w <= 0.0 || self.disp_h <= 0.0 || self.full_px_w == 0 || self.full_px_h == 0 {
            return Err(BitFunError::tool(
                "Invalid macOS pointer geometry.".to_string(),
            ));
        }
        let px_w = self.full_px_w as f64;
        let px_h = self.full_px_h as f64;
        let max_cx = (self.full_px_w.saturating_sub(1) as f64) + 0.5;
        let max_cy = (self.full_px_h.saturating_sub(1) as f64) + 0.5;
        let cx = cx.clamp(0.5, max_cx);
        let cy = cy.clamp(0.5, max_cy);
        let gx = self.disp_ox + (cx / px_w) * self.disp_w;
        let gy = self.disp_oy + (cy / px_h) * self.disp_h;
        Ok((gx, gy))
    }

    /// `CGEventGetLocation` global mouse -> full-buffer pixel; then optional crop to view.
    fn global_to_view_pixel(
        &self,
        mx: f64,
        my: f64,
        view_w: u32,
        view_h: u32,
    ) -> Option<(i32, i32)> {
        if self.disp_w <= 0.0 || self.disp_h <= 0.0 || self.full_px_w == 0 || self.full_px_h == 0 {
            return None;
        }
        let lx = mx - self.disp_ox;
        let ly = my - self.disp_oy;
        if lx < 0.0 || lx >= self.disp_w || ly < 0.0 || ly >= self.disp_h {
            return None;
        }
        let full_ix = ((lx / self.disp_w) * self.full_px_w as f64).floor() as i32;
        let full_iy = ((ly / self.disp_h) * self.full_px_h as f64).floor() as i32;
        let full_ix = full_ix.clamp(0, self.full_px_w.saturating_sub(1) as i32);
        let full_iy = full_iy.clamp(0, self.full_px_h.saturating_sub(1) as i32);
        let vx = full_ix - self.crop_x0 as i32;
        let vy = full_iy - self.crop_y0 as i32;
        if vx >= 0 && vy >= 0 && (vx as u32) < view_w && (vy as u32) < view_h {
            Some((vx, vy))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PointerMap {
    /// Screenshot JPEG width/height (same as capture when there is no frame padding).
    image_w: u32,
    image_h: u32,
    /// Top-left of capture inside the JPEG (0 when there is no padding).
    content_origin_x: u32,
    content_origin_y: u32,
    /// Native capture pixel size (the cropped/visible bitmap).
    content_w: u32,
    content_h: u32,
    native_w: u32,
    native_h: u32,
    origin_x: i32,
    origin_y: i32,
    #[cfg(target_os = "macos")]
    macos_geo: Option<MacPointerGeo>,
}

impl PointerMap {
    /// Continuous mapping: **composed JPEG** pixel `(x,y)` -> global (macOS CG).
    fn map_image_to_global_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        if self.image_w == 0
            || self.image_h == 0
            || self.content_w == 0
            || self.content_h == 0
            || self.native_w == 0
            || self.native_h == 0
        {
            return Err(BitFunError::tool(
                "Invalid screenshot coordinate map (zero dimension).".to_string(),
            ));
        }
        let ox = self.content_origin_x as i32;
        let oy = self.content_origin_y as i32;
        let cx_img = x - ox;
        let cy_img = y - oy;
        let max_cx = self.content_w.saturating_sub(1) as i32;
        let max_cy = self.content_h.saturating_sub(1) as i32;
        let cx_img = cx_img.clamp(0, max_cx) as f64;
        let cy_img = cy_img.clamp(0, max_cy) as f64;
        let cw = self.content_w as f64;
        let ch = self.content_h as f64;
        let nw = self.native_w as f64;
        let nh = self.native_h as f64;

        #[cfg(target_os = "macos")]
        if let Some(g) = self.macos_geo {
            let cx = g.crop_x0 as f64 + (cx_img + 0.5) * nw / cw;
            let cy = g.crop_y0 as f64 + (cy_img + 0.5) * nh / ch;
            return g.full_pixel_center_to_global_f64(cx, cy);
        }

        let center_full_x = self.origin_x as f64 + (cx_img + 0.5) * nw / cw;
        let center_full_y = self.origin_y as f64 + (cy_img + 0.5) * nh / ch;
        Ok((center_full_x, center_full_y))
    }

    /// Normalized 0..=1000 maps to the **capture** bitmap.
    fn map_normalized_to_global_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        if self.native_w == 0 || self.native_h == 0 {
            return Err(BitFunError::tool(
                "Invalid screenshot coordinate map (zero native dimension).".to_string(),
            ));
        }
        let nw = self.native_w as f64;
        let nh = self.native_h as f64;
        let tx = (x.clamp(0, 1000) as f64) / 1000.0;
        let ty = (y.clamp(0, 1000) as f64) / 1000.0;

        #[cfg(target_os = "macos")]
        if let Some(g) = self.macos_geo {
            let cx = g.crop_x0 as f64 + tx * (nw - 1.0).max(0.0) + 0.5;
            let cy = g.crop_y0 as f64 + ty * (nh - 1.0).max(0.0) + 0.5;
            return g.full_pixel_center_to_global_f64(cx, cy);
        }

        let gx = self.origin_x as f64 + tx * (nw - 1.0).max(0.0) + 0.5;
        let gy = self.origin_y as f64 + ty * (nh - 1.0).max(0.0) + 0.5;
        Ok((gx, gy))
    }

    fn image_global_bounds(&self) -> Option<ComputerUseImageGlobalBounds> {
        if self.image_w == 0 || self.image_h == 0 {
            return None;
        }
        let (x0, y0) = self.map_image_to_global_f64(0, 0).ok()?;
        let (x1, y1) = self
            .map_image_to_global_f64(
                self.image_w.saturating_sub(1) as i32,
                self.image_h.saturating_sub(1) as i32,
            )
            .ok()?;
        Some(ComputerUseImageGlobalBounds {
            left: x0.min(x1),
            top: y0.min(y1),
            width: (x1 - x0).abs(),
            height: (y1 - y0).abs(),
        })
    }
}

/// What the last tool `screenshot` implied for **plain** follow-up captures (no crop / no `navigate_quadrant`).
/// **PointCrop** is not reused for plain refresh: the next bare `screenshot` shows the **full display** again so
/// "full" is never stuck at ~500×500 after a point crop. **Quadrant** plain refresh keeps the current drill tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComputerUseNavFocus {
    FullDisplay,
    Quadrant { rect: ComputerUseNavigationRect },
    PointCrop { rect: ComputerUseNavigationRect },
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
        let host = Self {
            state: Mutex::new(ComputerUseSessionMutableState::new()),
        };
        host.run_background_input_self_check();
        host
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
            return Self::session_snapshot_macos();
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

    fn refinement_from_shot(shot: &ComputerScreenshot) -> ComputerUseScreenshotRefinement {
        use ComputerUseScreenshotRefinement as R;
        if let Some(c) = shot.screenshot_crop_center {
            return R::RegionAroundPoint {
                center_x: c.x,
                center_y: c.y,
            };
        }
        let Some(nav) = shot.navigation_native_rect else {
            return R::FullDisplay;
        };
        let full = nav.x0 == 0
            && nav.y0 == 0
            && nav.width == shot.native_width
            && nav.height == shot.native_height;
        if full {
            R::FullDisplay
        } else {
            R::QuadrantNavigation {
                x0: nav.x0,
                y0: nav.y0,
                width: nav.width,
                height: nav.height,
                click_ready: shot.quadrant_navigation_click_ready,
            }
        }
    }

    fn ensure_input_automation_allowed() -> BitFunResult<()> {
        #[cfg(target_os = "macos")]
        {
            if macos::ax_trusted() {
                return Ok(());
            }
            let exe = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "(unknown path)".to_string());
            return Err(BitFunError::tool(format!(
                "macOS Accessibility is not enabled for this executable. System Settings > Privacy & Security > Accessibility: add and enable BitFun. Development builds use the debug binary at: {}",
                exe
            )));
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
    fn run_enigo_job<F, T>(job: F) -> BitFunResult<T>
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

    fn encode_jpeg(rgb: &RgbImage, quality: u8) -> BitFunResult<Vec<u8>> {
        let mut buf = Vec::new();
        let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
        enc.encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ColorType::Rgb8,
        )
        .map_err(|e| BitFunError::tool(format!("JPEG encode: {}", e)))?;
        Ok(buf)
    }

    /// JPEG for OCR only: **no** pointer overlay — raw capture pixels.
    const OCR_RAW_JPEG_QUALITY: u8 = 85;

    /// Build [`ComputerScreenshot`] from a raw RGB crop; image pixels map 1:1 to `native_*` at `display_origin_*`.
    fn raw_shot_from_rgb_crop(
        rgb: RgbImage,
        display_origin_x: i32,
        display_origin_y: i32,
        native_w: u32,
        native_h: u32,
    ) -> BitFunResult<ComputerScreenshot> {
        let jpeg_bytes = Self::encode_jpeg(&rgb, Self::OCR_RAW_JPEG_QUALITY)?;
        let iw = rgb.width();
        let ih = rgb.height();
        Ok(ComputerScreenshot {
            screenshot_id: Some(Self::next_screenshot_id()),
            bytes: jpeg_bytes,
            mime_type: "image/jpeg".to_string(),
            image_width: iw,
            image_height: ih,
            native_width: native_w,
            native_height: native_h,
            display_origin_x,
            display_origin_y,
            vision_scale: 1.0_f64,
            pointer_image_x: None,
            pointer_image_y: None,
            screenshot_crop_center: None,
            point_crop_half_extent_native: None,
            navigation_native_rect: None,
            quadrant_navigation_click_ready: false,
            image_content_rect: Some(ComputerUseImageContentRect {
                left: 0,
                top: 0,
                width: iw,
                height: ih,
            }),
            image_global_bounds: Some(ComputerUseImageGlobalBounds {
                left: display_origin_x as f64,
                top: display_origin_y as f64,
                width: native_w as f64,
                height: native_h as f64,
            }),
            implicit_confirmation_crop_applied: false,
            ui_tree_text: None,
        })
    }

    /// Full primary-display region in **global logical coordinates** (same as `CGDisplayBounds` / AX).
    fn ocr_full_primary_display_region() -> BitFunResult<OcrRegionNative> {
        let screen = Screen::from_point(0, 0)
            .map_err(|e| BitFunError::tool(format!("Screen capture init (OCR raw): {}", e)))?;
        let d = screen.display_info;
        Ok(OcrRegionNative {
            x0: d.x,
            y0: d.y,
            width: d.width,
            height: d.height,
        })
    }

    /// Region to OCR: explicit `ocr_region_native`, else (macOS) frontmost window from AX, else full primary display.
    fn ocr_resolve_region_for_capture(
        region_native: Option<OcrRegionNative>,
    ) -> BitFunResult<OcrRegionNative> {
        if let Some(r) = region_native {
            return Ok(r);
        }
        #[cfg(target_os = "macos")]
        {
            match crate::computer_use::macos_ax_ui::frontmost_window_bounds_global() {
                Ok((x0, y0, w, h)) => Ok(OcrRegionNative {
                    x0,
                    y0,
                    width: w,
                    height: h,
                }),
                Err(e) => {
                    warn!(
                        "computer_use OCR: frontmost window bounds failed ({}); falling back to full primary display.",
                        e
                    );
                    Self::ocr_full_primary_display_region()
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::ocr_full_primary_display_region()
        }
    }

    /// Square region in global logical coordinates for raw OCR preview crops around `(cx, cy)`.
    fn ocr_region_square_around_point(
        cx: f64,
        cy: f64,
        half: u32,
    ) -> BitFunResult<OcrRegionNative> {
        let hh = half as f64;
        let x0 = (cx - hh).floor() as i32;
        let y0 = (cy - hh).floor() as i32;
        let w = half.saturating_mul(2).max(1);
        Ok(OcrRegionNative {
            x0,
            y0,
            width: w,
            height: w,
        })
    }

    /// Capture **raw** display pixels (no pointer overlay), cropped to `region` intersected with the chosen display.
    ///
    /// `region` and [`DisplayInfo::width`]/[`height`] are **global logical points** (CG / AX). The framebuffer
    /// is **physical pixels** on Retina; intersect in point space, then map to pixels like [`MacPointerGeo`].
    fn screenshot_raw_native_region(region: OcrRegionNative) -> BitFunResult<ComputerScreenshot> {
        let cx = region.x0 + region.width as i32 / 2;
        let cy = region.y0 + region.height as i32 / 2;
        let screen = Screen::from_point(cx, cy)
            .or_else(|_| Screen::from_point(0, 0))
            .map_err(|e| BitFunError::tool(format!("Screen capture init (OCR raw): {}", e)))?;
        let rgba = screen
            .capture()
            .map_err(|e| BitFunError::tool(format!("Screenshot failed (OCR raw): {}", e)))?;
        let (full_px_w, full_px_h) = rgba.dimensions();
        let d = screen.display_info;
        let disp_w = d.width as f64;
        let disp_h = d.height as f64;
        if disp_w <= 0.0 || disp_h <= 0.0 || full_px_w == 0 || full_px_h == 0 {
            return Err(BitFunError::tool(
                "Invalid display geometry for OCR raw crop.".to_string(),
            ));
        }
        let ox = d.x as f64;
        let oy = d.y as f64;
        let full_rgb = DynamicImage::ImageRgba8(rgba).to_rgb8();
        // Region from AX / user: global logical coords (points).
        let rx0 = region.x0 as f64;
        let ry0 = region.y0 as f64;
        let rw = region.width as f64;
        let rh = region.height as f64;
        let ix0 = rx0.max(ox);
        let iy0 = ry0.max(oy);
        let ix1 = (rx0 + rw).min(ox + disp_w);
        let iy1 = (ry0 + rh).min(oy + disp_h);
        if ix1 <= ix0 || iy1 <= iy0 {
            return Err(BitFunError::tool(
                "OCR region does not intersect the captured display. Focus the target app or set ocr_region_native."
                    .to_string(),
            ));
        }
        let px0_f = ((ix0 - ox) / disp_w) * full_px_w as f64;
        let py0_f = ((iy0 - oy) / disp_h) * full_px_h as f64;
        let px1_f = ((ix1 - ox) / disp_w) * full_px_w as f64;
        let py1_f = ((iy1 - oy) / disp_h) * full_px_h as f64;
        let px0 = px0_f.floor().max(0.0) as u32;
        let py0 = py0_f.floor().max(0.0) as u32;
        let px1 = px1_f.ceil().min(full_px_w as f64) as u32;
        let py1 = py1_f.ceil().min(full_px_h as f64) as u32;
        if px1 <= px0 || py1 <= py0 {
            return Err(BitFunError::tool(
                "OCR crop rectangle is empty after point-to-pixel mapping.".to_string(),
            ));
        }
        let crop_w = px1 - px0;
        let crop_h = py1 - py0;
        let cropped = Self::crop_rgb(&full_rgb, px0, py0, crop_w, crop_h)?;
        let span_w = ((crop_w as f64 / full_px_w as f64) * disp_w)
            .round()
            .max(1.0) as u32;
        let span_h = ((crop_h as f64 / full_px_h as f64) * disp_h)
            .round()
            .max(1.0) as u32;
        let origin_gx = (ox + (px0 as f64 / full_px_w as f64) * disp_w).round() as i32;
        let origin_gy = (oy + (py0 as f64 / full_px_h as f64) * disp_h).round() as i32;
        Self::raw_shot_from_rgb_crop(cropped, origin_gx, origin_gy, span_w, span_h)
    }

    /// Rasterizes `assets/computer_use_pointer.svg` via **resvg** (vector → antialiased pixmap).
    /// **Tip** in SVG user space **(0,0)** is placed at `(cx, cy)` = click hotspot.
    fn draw_pointer_marker(img: &mut RgbImage, cx: i32, cy: i32) {
        if let Some(pm) = pointer_pixmap_cache() {
            blend_pointer_pixmap(img, cx, cy, pm);
        } else {
            draw_pointer_fallback_cross(img, cx, cy);
        }
    }

    fn crop_rgb(src: &RgbImage, x0: u32, y0: u32, w: u32, h: u32) -> BitFunResult<RgbImage> {
        let (sw, sh) = src.dimensions();
        if x0.saturating_add(w) > sw || y0.saturating_add(h) > sh {
            return Err(BitFunError::tool("Tile crop out of bounds.".to_string()));
        }
        let view = image::imageops::crop_imm(src, x0, y0, w, h);
        Ok(view.to_image())
    }

    /// Pointer position in **scaled image** pixels, if it lies inside the captured display.
    #[cfg(not(target_os = "macos"))]
    #[allow(clippy::too_many_arguments)]
    fn pointer_in_scaled_image(
        origin_x: i32,
        origin_y: i32,
        native_w: u32,
        native_h: u32,
        tw: u32,
        th: u32,
        gx: i32,
        gy: i32,
    ) -> Option<(i32, i32)> {
        if native_w == 0 || native_h == 0 {
            return None;
        }
        let lx = gx - origin_x;
        let ly = gy - origin_y;
        let nw = native_w as i32;
        let nh = native_h as i32;
        if lx < 0 || ly < 0 || lx >= nw || ly >= nh {
            return None;
        }
        let ix = (((lx as f64 + 0.5) * tw as f64) / (native_w as f64))
            .floor()
            .clamp(0.0, tw.saturating_sub(1) as f64) as i32;
        let iy = (((ly as f64 + 0.5) * th as f64) / (native_h as f64))
            .floor()
            .clamp(0.0, th.saturating_sub(1) as f64) as i32;
        Some((ix, iy))
    }

    fn screenshot_sync_tool_with_capture(
        params: ComputerUseScreenshotParams,
        nav_in: Option<ComputerUseNavFocus>,
        rgba: image::RgbaImage,
        screen: Screen,
        ui_tree_text: Option<String>,
        implicit_confirmation_crop_applied: bool,
    ) -> BitFunResult<(ComputerScreenshot, PointerMap, Option<ComputerUseNavFocus>)> {
        if params.crop_center.is_some() && params.navigate_quadrant.is_some() {
            return Err(BitFunError::tool(
                "Use either screenshot_crop_center_* or screenshot_navigate_quadrant, not both."
                    .to_string(),
            ));
        }

        let (native_w, native_h) = rgba.dimensions();
        let origin_x = screen.display_info.x;
        let origin_y = screen.display_info.y;

        #[cfg(target_os = "macos")]
        let full_geo = MacPointerGeo::from_display(native_w, native_h, &screen.display_info);

        let dyn_img = DynamicImage::ImageRgba8(rgba);
        let full_frame = dyn_img.to_rgb8();

        let full_rect = full_navigation_rect(native_w, native_h);
        let focus_in = if params.reset_navigation {
            None
        } else {
            nav_in
        };
        let focus = match focus_in {
            None => None,
            Some(ComputerUseNavFocus::FullDisplay) => Some(ComputerUseNavFocus::FullDisplay),
            Some(ComputerUseNavFocus::Quadrant { rect }) => Some(ComputerUseNavFocus::Quadrant {
                rect: intersect_navigation_rect(rect, full_rect).unwrap_or(full_rect),
            }),
            Some(ComputerUseNavFocus::PointCrop { rect }) => Some(ComputerUseNavFocus::PointCrop {
                rect: intersect_navigation_rect(rect, full_rect).unwrap_or(full_rect),
            }),
        };

        let (
            content_rgb,
            map_origin_x,
            map_origin_y,
            map_native_w,
            map_native_h,
            content_w,
            content_h,
            screenshot_crop_center,
            ruler_origin_native_x,
            ruler_origin_native_y,
            shot_navigation_rect,
            quadrant_navigation_click_ready,
            persist_nav_focus,
        ) = if let Some(center) = params.crop_center {
            let half = clamp_point_crop_half_extent(params.point_crop_half_extent_native);
            let (ccx, ccy) = clamp_center_to_native(center.x, center.y, native_w, native_h);
            let (x0, y0, tw, th) =
                crop_rect_around_point_native(center.x, center.y, native_w, native_h, half);
            let cropped = Self::crop_rgb(&full_frame, x0, y0, tw, th)?;
            let ox = origin_x + x0 as i32;
            let oy = origin_y + y0 as i32;
            let nav_r = ComputerUseNavigationRect {
                x0,
                y0,
                width: tw,
                height: th,
            };
            (
                cropped,
                ox,
                oy,
                tw,
                th,
                tw,
                th,
                Some(ScreenshotCropCenter { x: ccx, y: ccy }),
                x0,
                y0,
                Some(nav_r),
                false,
                Some(ComputerUseNavFocus::PointCrop { rect: nav_r }),
            )
        } else if let Some(q) = params.navigate_quadrant {
            let base = match focus {
                None | Some(ComputerUseNavFocus::FullDisplay) => full_rect,
                Some(ComputerUseNavFocus::Quadrant { rect })
                | Some(ComputerUseNavFocus::PointCrop { rect }) => rect,
            };
            let Some(base) = intersect_navigation_rect(base, full_rect) else {
                return Err(BitFunError::tool(
                    "Navigation focus is outside the display.".to_string(),
                ));
            };
            if base.width < 2 || base.height < 2 {
                return Err(BitFunError::tool(
                    "Quadrant navigation: region is too small to subdivide further.".to_string(),
                ));
            }
            let split = quadrant_split_rect(base, q);
            let expanded = expand_navigation_rect_edges(
                split,
                COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
                native_w,
                native_h,
            );
            let Some(new_rect) = intersect_navigation_rect(expanded, full_rect) else {
                return Err(BitFunError::tool(
                    "Quadrant crop out of bounds.".to_string(),
                ));
            };
            let cropped = Self::crop_rgb(
                &full_frame,
                new_rect.x0,
                new_rect.y0,
                new_rect.width,
                new_rect.height,
            )?;
            let ox = origin_x + new_rect.x0 as i32;
            let oy = origin_y + new_rect.y0 as i32;
            let long_edge = new_rect.width.max(new_rect.height);
            let click_ready = long_edge < COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE;
            (
                cropped,
                ox,
                oy,
                new_rect.width,
                new_rect.height,
                new_rect.width,
                new_rect.height,
                None,
                new_rect.x0,
                new_rect.y0,
                Some(new_rect),
                click_ready,
                Some(ComputerUseNavFocus::Quadrant { rect: new_rect }),
            )
        } else {
            let (base, persist_nav_focus) = match focus {
                None | Some(ComputerUseNavFocus::FullDisplay) => {
                    (full_rect, Some(ComputerUseNavFocus::FullDisplay))
                }
                Some(ComputerUseNavFocus::Quadrant { rect }) => {
                    (rect, Some(ComputerUseNavFocus::Quadrant { rect }))
                }
                Some(ComputerUseNavFocus::PointCrop { .. }) => {
                    // Bare screenshot after point crop → full display again (do not keep ~500×500 as "full").
                    (full_rect, Some(ComputerUseNavFocus::FullDisplay))
                }
            };
            let is_full =
                base.x0 == 0 && base.y0 == 0 && base.width == native_w && base.height == native_h;
            let (
                content_rgb,
                map_origin_x,
                map_origin_y,
                map_native_w,
                map_native_h,
                content_w,
                content_h,
                ruler_origin_native_x,
                ruler_origin_native_y,
            ) = if is_full {
                (
                    full_frame, origin_x, origin_y, native_w, native_h, native_w, native_h, 0u32,
                    0u32,
                )
            } else {
                let cropped =
                    Self::crop_rgb(&full_frame, base.x0, base.y0, base.width, base.height)?;
                let ox = origin_x + base.x0 as i32;
                let oy = origin_y + base.y0 as i32;
                (
                    cropped,
                    ox,
                    oy,
                    base.width,
                    base.height,
                    base.width,
                    base.height,
                    base.x0,
                    base.y0,
                )
            };
            let long_edge = content_w.max(content_h);
            let quadrant_navigation_click_ready =
                !is_full && long_edge < COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE;
            (
                content_rgb,
                map_origin_x,
                map_origin_y,
                map_native_w,
                map_native_h,
                content_w,
                content_h,
                None,
                ruler_origin_native_x,
                ruler_origin_native_y,
                Some(base),
                quadrant_navigation_click_ready,
                persist_nav_focus,
            )
        };

        let (mut frame, margin_l, margin_t) =
            compose_computer_use_frame(content_rgb, ruler_origin_native_x, ruler_origin_native_y);

        #[cfg(target_os = "macos")]
        let macos_map_geo = if let Some(center) = params.crop_center {
            let half = clamp_point_crop_half_extent(params.point_crop_half_extent_native);
            let (x0, y0, _, _) =
                crop_rect_around_point_native(center.x, center.y, native_w, native_h, half);
            full_geo.with_crop(x0, y0)
        } else {
            full_geo.with_crop(ruler_origin_native_x, ruler_origin_native_y)
        };

        #[cfg(target_os = "macos")]
        let (pointer_image_x, pointer_image_y) = match macos::quartz_mouse_location() {
            Ok((mx, my)) => {
                match macos_map_geo.global_to_view_pixel(mx, my, content_w, content_h) {
                    Some((ix, iy)) => {
                        let px = ix + margin_l as i32;
                        let py = iy + margin_t as i32;
                        Self::draw_pointer_marker(&mut frame, px, py);
                        (Some(px), Some(py))
                    }
                    None => (None, None),
                }
            }
            Err(_) => (None, None),
        };

        #[cfg(not(target_os = "macos"))]
        let (pointer_image_x, pointer_image_y) = {
            let pointer_loc = Self::run_enigo_job(|e| {
                e.location()
                    .map_err(|err| BitFunError::tool(format!("pointer location: {}", err)))
            });
            match pointer_loc {
                Ok((gx, gy)) => match Self::pointer_in_scaled_image(
                    map_origin_x,
                    map_origin_y,
                    map_native_w,
                    map_native_h,
                    content_w,
                    content_h,
                    gx,
                    gy,
                ) {
                    Some((ix, iy)) => {
                        let px = ix + margin_l as i32;
                        let py = iy + margin_t as i32;
                        Self::draw_pointer_marker(&mut frame, px, py);
                        (Some(px), Some(py))
                    }
                    None => (None, None),
                },
                Err(_) => (None, None),
            }
        };

        // Adaptive byte-budget downscale: encode at JPEG_QUALITY first, then halve the resolution
        // (Lanczos3) and re-encode while the payload exceeds SCREENSHOT_MAX_BYTES. Small/medium
        // app-window captures keep native resolution; only oversize full-screen / multi-monitor
        // captures get reduced. Stops once another halve would push the long edge below
        // SCREENSHOT_MIN_LONG_EDGE to avoid producing an unreadable thumbnail.
        let mut current_frame = frame;
        let mut jpeg_bytes = Self::encode_jpeg(&current_frame, JPEG_QUALITY)?;
        let mut vision_scale: f64 = 1.0;
        while jpeg_bytes.len() > SCREENSHOT_MAX_BYTES
            && current_frame.width().max(current_frame.height()) / 2 >= SCREENSHOT_MIN_LONG_EDGE
        {
            let new_w = (current_frame.width() / 2).max(1);
            let new_h = (current_frame.height() / 2).max(1);
            let dyn_img = DynamicImage::ImageRgb8(current_frame);
            current_frame = dyn_img
                .resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3)
                .to_rgb8();
            vision_scale *= 2.0;
            jpeg_bytes = Self::encode_jpeg(&current_frame, JPEG_QUALITY)?;
        }
        let pointer_image_x =
            pointer_image_x.map(|px| (f64::from(px) / vision_scale).round() as i32);
        let pointer_image_y =
            pointer_image_y.map(|py| (f64::from(py) / vision_scale).round() as i32);
        let final_frame = current_frame;

        let (image_w, image_h) = final_frame.dimensions();
        let image_content_rect = ComputerUseImageContentRect {
            left: 0,
            top: 0,
            width: image_w,
            height: image_h,
        };

        let point_crop_half_extent_native = params
            .crop_center
            .map(|_| clamp_point_crop_half_extent(params.point_crop_half_extent_native));

        #[cfg(target_os = "macos")]
        let map = PointerMap {
            image_w,
            image_h,
            content_origin_x: 0,
            content_origin_y: 0,
            content_w: image_w,
            content_h: image_h,
            native_w: map_native_w,
            native_h: map_native_h,
            origin_x: map_origin_x,
            origin_y: map_origin_y,
            macos_geo: Some(macos_map_geo),
        };
        #[cfg(not(target_os = "macos"))]
        let map = PointerMap {
            image_w,
            image_h,
            content_origin_x: 0,
            content_origin_y: 0,
            content_w: image_w,
            content_h: image_h,
            native_w: map_native_w,
            native_h: map_native_h,
            origin_x: map_origin_x,
            origin_y: map_origin_y,
        };
        let image_global_bounds = map.image_global_bounds();

        let screenshot_id = Self::next_screenshot_id();
        let shot = ComputerScreenshot {
            screenshot_id: Some(screenshot_id),
            bytes: jpeg_bytes,
            mime_type: "image/jpeg".to_string(),
            image_width: image_w,
            image_height: image_h,
            native_width: map_native_w,
            native_height: map_native_h,
            display_origin_x: map_origin_x,
            display_origin_y: map_origin_y,
            vision_scale,
            pointer_image_x,
            pointer_image_y,
            screenshot_crop_center,
            point_crop_half_extent_native,
            navigation_native_rect: shot_navigation_rect,
            quadrant_navigation_click_ready,
            image_content_rect: Some(image_content_rect),
            image_global_bounds,
            implicit_confirmation_crop_applied,
            ui_tree_text,
        };

        Ok((shot, map, persist_nav_focus))
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

    /// Kept for compatibility / potential future call sites. Phase 1 routed
    /// the only previous caller (`key_chord` Enter/Return) through
    /// `computer_use_guard_click_allowed` instead, so this is currently dead
    /// code but a thinner guard variant might be useful again.
    #[allow(dead_code)]
    fn computer_use_guard_verified_ui(&self) -> BitFunResult<()> {
        let s = self
            .state
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        if s.click_needs_fresh_screenshot {
            return Err(BitFunError::tool(STALE_CAPTURE_TOOL_MESSAGE.to_string()));
        }
        Ok(())
    }

    /// Best-effort current mouse position in global screen coordinates.
    fn current_mouse_position() -> (f64, f64) {
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

    /// Resolve a screen capture from cache (if still valid and same screen) or capture fresh.
    ///
    /// Phase 2 fix: when the model has called `desktop.focus_display`, we
    /// commit to that screen instead of trusting the mouse pointer. This is
    /// the explicit fix for the user's original complaint — on multi-monitor
    /// setups the cursor often lives on a different screen than the one the
    /// user is reasoning about (e.g. focus is on the laptop screen, mouse
    /// is parked on the secondary monitor) and the legacy "screen at mouse
    /// pointer" heuristic captured the wrong display.
    fn resolve_screenshot_capture(
        cached: Option<ScreenshotCacheEntry>,
        mouse_x: f64,
        mouse_y: f64,
        preferred_display_id: Option<u32>,
    ) -> BitFunResult<(image::RgbaImage, Screen)> {
        let mx = mouse_x.round() as i32;
        let my = mouse_y.round() as i32;
        let target_display_id = preferred_display_id
            .or_else(|| Screen::from_point(mx, my).ok().map(|s| s.display_info.id));

        if let Some(cache) = cached {
            let screen_id_match = Some(cache.screen.display_info.id) == target_display_id;
            if cache.capture_time.elapsed() < Duration::from_millis(SCREENSHOT_CACHE_TTL_MS)
                && screen_id_match
            {
                debug!(
                    "Using cached screenshot (age: {}ms)",
                    cache.capture_time.elapsed().as_millis()
                );
                return Ok((cache.rgba, cache.screen));
            }
        }

        let screen = if let Some(id) = preferred_display_id {
            Self::find_screen_by_id(id)
                .or_else(|| Screen::from_point(mx, my).ok())
                .or_else(|| Screen::from_point(0, 0).ok())
                .ok_or_else(|| {
                    BitFunError::tool("Screen capture init: no display available".to_string())
                })?
        } else {
            Screen::from_point(mx, my)
                .or_else(|_| Screen::from_point(0, 0))
                .map_err(|e| BitFunError::tool(format!("Screen capture init: {}", e)))?
        };
        let rgba = screen.capture().map_err(|e| {
            BitFunError::tool(format!(
                "Screenshot failed (on macOS grant Screen Recording for BitFun): {}",
                e
            ))
        })?;
        Ok((rgba, screen))
    }

    /// Find a [`Screen`] by its display id from the host's enumeration.
    fn find_screen_by_id(display_id: u32) -> Option<Screen> {
        Screen::all()
            .ok()
            .and_then(|all| all.into_iter().find(|s| s.display_info.id == display_id))
    }

    /// Snapshot of all attached displays, with `is_active` / `has_pointer`
    /// flags resolved relative to `preferred_display_id` and the current
    /// mouse position.
    fn enumerate_displays(
        preferred_display_id: Option<u32>,
        mouse_x: f64,
        mouse_y: f64,
    ) -> Vec<ComputerUseDisplayInfo> {
        let mx = mouse_x.round() as i32;
        let my = mouse_y.round() as i32;
        let pointer_display_id = Screen::from_point(mx, my).ok().map(|s| s.display_info.id);
        let active_id = preferred_display_id.or(pointer_display_id);

        let screens = match Screen::all() {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        screens
            .into_iter()
            .map(|s| {
                let d = s.display_info;
                ComputerUseDisplayInfo {
                    display_id: d.id,
                    is_primary: d.is_primary,
                    is_active: Some(d.id) == active_id,
                    has_pointer: Some(d.id) == pointer_display_id,
                    origin_x: d.x,
                    origin_y: d.y,
                    width_logical: d.width,
                    height_logical: d.height,
                    scale_factor: d.scale_factor,
                    foreground_app: None,
                }
            })
            .collect()
    }

    fn chord_includes_return_or_enter(keys: &[String]) -> bool {
        keys.iter()
            .any(|s| matches!(s.to_lowercase().as_str(), "return" | "enter" | "kp_enter"))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{BitFunError, BitFunResult};
    use core_foundation::base::{CFRelease, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use dispatch::Queue;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "System", kind = "dylib")]
    unsafe extern "C" {
        fn pthread_main_np() -> i32;
    }

    /// Run work that may call TSM / HIToolbox (enigo keyboard & text) on the main dispatch queue.
    ///
    /// The closure is wrapped in `objc2::exception::catch` so that any
    /// Objective-C `NSException` thrown by TSM / HIToolbox / AppKit (which
    /// historically appears as `__rust_foreign_exception` and aborts the
    /// process when it crosses back into the Rust runtime) is converted into
    /// a `BitFunError` we can return to the caller. The closure must itself
    /// return a `BitFunResult<T>` so we can flatten the two error sources
    /// (ObjC exception + Rust-side error) into one.
    pub fn run_on_main_for_enigo<F, T>(f: F) -> BitFunResult<T>
    where
        F: FnOnce() -> BitFunResult<T> + Send,
        T: Send,
    {
        let work = move || catch_only(f);
        unsafe {
            if pthread_main_np() != 0 {
                work()
            } else {
                Queue::main().exec_sync(work)
            }
        }
    }

    /// Run a closure on the main dispatch queue under an Objective-C
    /// `@try/@catch`. This is the correct wrapper for calls that may reach
    /// AppKit / HIToolbox / Accessibility code paths from a background
    /// (`tokio::spawn_blocking`) worker thread.
    ///
    /// Two failure modes are defended against simultaneously:
    ///
    ///   1. `NSException` thrown by the framework (caught and converted into
    ///      `BitFunError`).
    ///   2. AppKit's `__assert_rtn` "Must only be used from the main thread"
    ///      `SIGTRAP` which fires when AX cross-process callbacks (e.g.
    ///      `AXUIElementCopyActionNames` → `_NSThemeWidgetCell.accessibility…`
    ///      → `_WMWindow performUpdatesUsingBlock:`) are evaluated off the
    ///      main thread. `objc2::exception::catch` cannot intercept this
    ///      trap; the only fix is to actually run the closure on the main
    ///      thread, which is what this helper does.
    ///
    /// If we're already on the main thread we run inline (avoids
    /// `dispatch_sync(main)` deadlock).
    pub fn catch_objc<F, T>(f: F) -> BitFunResult<T>
    where
        F: FnOnce() -> BitFunResult<T> + Send,
        T: Send,
    {
        unsafe {
            let on_main = pthread_main_np() != 0;
            if on_main {
                catch_only(f)
            } else {
                Queue::main().exec_sync(move || catch_only(f))
            }
        }
    }

    /// Run a closure under an Objective-C `@try/@catch` **on the current
    /// thread** (no main-queue dispatch). Use this for closures that borrow
    /// non-`Send` data and that are guaranteed not to reach AppKit's
    /// main-thread-only AX callbacks (e.g. Vision OCR on an in-memory
    /// screenshot buffer).
    pub fn catch_objc_local<F, T>(f: F) -> BitFunResult<T>
    where
        F: FnOnce() -> BitFunResult<T>,
    {
        catch_only(f)
    }

    fn catch_only<F, T>(f: F) -> BitFunResult<T>
    where
        F: FnOnce() -> BitFunResult<T>,
    {
        use std::panic::AssertUnwindSafe;
        match objc2::exception::catch(AssertUnwindSafe(f)) {
            Ok(inner) => inner,
            Err(Some(exc)) => Err(BitFunError::tool(format!("Objective-C exception: {}", exc))),
            Err(None) => Err(BitFunError::tool("Objective-C exception (nil)".to_string())),
        }
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
        fn CGEventCreate(source: *const c_void) -> *const c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    }

    /// Mouse location in Quartz global coordinates (same space as `CGEvent` / `CGWarpMouseCursorPosition`).
    pub fn quartz_mouse_location() -> BitFunResult<(f64, f64)> {
        unsafe {
            let ev = CGEventCreate(std::ptr::null());
            if ev.is_null() {
                return Err(BitFunError::tool(
                    "CGEventCreate returned null (pointer overlay).".to_string(),
                ));
            }
            let pt = CGEventGetLocation(ev);
            CFRelease(ev as *const _);
            Ok((pt.x, pt.y))
        }
    }

    pub fn ax_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub fn screen_capture_preflight() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn request_ax_prompt() {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let val = CFBoolean::true_value();
        let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), val.as_CFType())]);
        unsafe {
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *const _);
        }
    }

    pub fn request_screen_capture() -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
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

    fn map_app_image_coords_to_pointer_f64(
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

    fn image_grid_target_to_xy(target: &ClickTarget) -> BitFunResult<Option<(i32, i32)>> {
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
    #[cfg(target_os = "macos")]
    async fn screenshot_for_app_pid(&self, pid: i32) -> BitFunResult<ComputerScreenshot> {
        let window_target_rect = macos::catch_objc(|| {
            crate::computer_use::macos_ax_ui::window_bounds_global_for_pid(pid)
        })
        .ok()
        .map(|(x, y, w, h)| (x as f64, y as f64, w as f64, h as f64));

        let (cached, preferred_display_id) = {
            let s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            (s.screenshot_cache.clone(), s.preferred_display_id)
        };
        let (mouse_x, mouse_y) = Self::current_mouse_position();
        let effective_pref_display_id = if let Some((wx, wy, ww, wh)) = window_target_rect {
            let cx_g = wx + ww / 2.0;
            let cy_g = wy + wh / 2.0;
            Screen::from_point(cx_g.round() as i32, cy_g.round() as i32)
                .ok()
                .map(|s| s.display_info.id)
                .or(preferred_display_id)
        } else {
            preferred_display_id
        };

        let (rgba, screen) =
            Self::resolve_screenshot_capture(cached, mouse_x, mouse_y, effective_pref_display_id)?;
        let (native_w, native_h) = rgba.dimensions();
        let params = if let Some((wx, wy, ww, wh)) = window_target_rect {
            let cx_g = wx + ww / 2.0;
            let cy_g = wy + wh / 2.0;
            let (cx, cy) = global_to_native_full_pixel_center(
                cx_g,
                cy_g,
                native_w,
                native_h,
                &screen.display_info,
            );
            let disp_w = screen.display_info.width as f64;
            let disp_h = screen.display_info.height as f64;
            let scale_x = if disp_w > 0.0 {
                native_w as f64 / disp_w
            } else {
                1.0
            };
            let scale_y = if disp_h > 0.0 {
                native_h as f64 / disp_h
            } else {
                1.0
            };
            let half_native = ((ww * scale_x).max(wh * scale_y) / 2.0).ceil() as u32 + 16;
            let max_half = (native_w.max(native_h) / 2).max(64);
            ComputerUseScreenshotParams {
                crop_center: Some(ScreenshotCropCenter { x: cx, y: cy }),
                navigate_quadrant: None,
                reset_navigation: false,
                point_crop_half_extent_native: Some(half_native.clamp(64, max_half)),
                implicit_confirmation_center: None,
                crop_to_focused_window: false,
            }
        } else {
            ComputerUseScreenshotParams::default()
        };

        {
            let mut s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            s.screenshot_cache = Some(ScreenshotCacheEntry {
                rgba: rgba.clone(),
                screen,
                capture_time: Instant::now(),
            });
        }

        let (shot, map, nav_out) = tokio::task::spawn_blocking(move || {
            Self::screenshot_sync_tool_with_capture(params, None, rgba, screen, None, false)
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        let refinement = Self::refinement_from_shot(&shot);
        {
            let mut s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            s.transition_after_screenshot(map, refinement, nav_out);
            s.app_pointer_maps.insert(pid, map);
            if let Some(id) = shot.screenshot_id.clone() {
                s.screenshot_pointer_maps.insert(id, map);
            }
        }
        Ok(shot)
    }

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
            if !macos::ax_trusted() {
                // Re-trigger the system prompt in case the user dismissed it
                // earlier — without this they have no way back to the dialog
                // short of digging through System Settings manually.
                macos::request_ax_prompt();
                return Err(BitFunError::tool(
                    "[PERMISSION_DENIED] macOS Accessibility permission not granted to BitFun. \
                     The system has been asked to surface the permission dialog (System Settings → \
                     Privacy & Security → Accessibility → enable BitFun). After granting, retry \
                     `desktop.get_app_state` and the AX tree will include all WebView subtree nodes."
                        .to_string(),
                ));
            }
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
            Ok(snap)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, max_depth, focus_window_only, capture_screenshot);
            Err(BitFunError::tool(
                "get_app_state is only available on macOS in this build".to_string(),
            ))
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
            tokio::task::spawn_blocking(|| macos::request_ax_prompt())
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
        let (nav_snapshot, cached, click_needs, preferred_display_id) = {
            let s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            (
                s.navigation_focus,
                s.screenshot_cache.clone(),
                s.click_needs_fresh_screenshot,
                s.preferred_display_id,
            )
        };

        let (mouse_x, mouse_y) = Self::current_mouse_position();

        // === Crop policy: full window OR full display, NOTHING ELSE ===
        //
        // The historical crop logic (mouse-centered 500×500 implicit
        // confirmation crop, `crop_center` / `navigate_quadrant` /
        // `point_crop_half_extent_native` quadrant drilling) is **disabled**
        // at the entry point. Models always get one of two pictures:
        //
        //   1. The **focused application window** (via AX) — used by default
        //      when AX can resolve it. This is the right view 99% of the
        //      time: the model can see the entire app it just acted on.
        //   2. The **full display** — fallback when AX cannot resolve the
        //      window (no permission, no AX windows, non-macOS).
        //
        // All incoming crop / quadrant / implicit-center params are stripped
        // before they reach the rendering pipeline. The accompanying click
        // guard (`quadrant_navigation_click_ready`) is also relaxed since
        // every screenshot now provides full context for
        // click_element / move_to_text / mouse_move targeting.
        let _ = click_needs; // intentionally unused — no more click_needs-gated crop variants
        let window_target_rect: Option<(f64, f64, f64, f64)> = {
            #[cfg(target_os = "macos")]
            {
                // Wrap the AX call in @try/@catch: a buggy frontmost app
                // (e.g. one that throws NSAccessibilityException out of an
                // attribute callback) used to crash the whole process via
                // __rust_foreign_exception. Now we just fall back to a
                // full-display screenshot and log the failure.
                let res = macos::catch_objc(|| {
                    crate::computer_use::macos_ax_ui::frontmost_window_bounds_global()
                });
                match res {
                    Ok((x, y, w, h)) => Some((x as f64, y as f64, w as f64, h as f64)),
                    Err(e) => {
                        debug!(
                            "Focused-window lookup failed, falling back to full-display capture: {}",
                            e
                        );
                        None
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                None
            }
        };

        // If the focused window lives on a different display than the cached /
        // preferred one, override display selection so we capture the correct screen.
        let effective_pref_display_id = if let Some((wx, wy, ww, wh)) = window_target_rect {
            let cx_g = wx + ww / 2.0;
            let cy_g = wy + wh / 2.0;
            Screen::from_point(cx_g.round() as i32, cy_g.round() as i32)
                .ok()
                .map(|s| s.display_info.id)
                .or(preferred_display_id)
        } else {
            preferred_display_id
        };

        let (rgba, screen) =
            Self::resolve_screenshot_capture(cached, mouse_x, mouse_y, effective_pref_display_id)?;
        let (native_w, native_h) = rgba.dimensions();

        // === Build the ONE allowed param set ===
        //
        // Either (a) focused-window crop, or (b) full-display capture. All
        // model-supplied crop / quadrant / implicit-center fields are
        // discarded here on purpose so the rendering pipeline can never
        // produce a mouse-centered 500×500 or a quadrant tile again.
        let _ = params; // discard incoming crop fields entirely
        let implicit_applied = false; // legacy flag, always false now
        let params = if let Some((wx, wy, ww, wh)) = window_target_rect {
            let cx_g = wx + ww / 2.0;
            let cy_g = wy + wh / 2.0;
            let (cx, cy) = global_to_native_full_pixel_center(
                cx_g,
                cy_g,
                native_w,
                native_h,
                &screen.display_info,
            );
            let disp_w = screen.display_info.width as f64;
            let disp_h = screen.display_info.height as f64;
            let scale_x = if disp_w > 0.0 {
                native_w as f64 / disp_w
            } else {
                1.0
            };
            let scale_y = if disp_h > 0.0 {
                native_h as f64 / disp_h
            } else {
                1.0
            };
            // half_extent must cover the longer side of the window in native
            // pixels (+ 16px visual padding so window edges aren't flush
            // with the frame). Clamped to the display so we never request
            // more than what we just captured.
            let half_native = ((ww * scale_x).max(wh * scale_y) / 2.0).ceil() as u32 + 16;
            let max_half = (native_w.max(native_h) / 2).max(64);
            let half_native = half_native.clamp(64, max_half);
            ComputerUseScreenshotParams {
                crop_center: Some(ScreenshotCropCenter { x: cx, y: cy }),
                navigate_quadrant: None,
                reset_navigation: false,
                point_crop_half_extent_native: Some(half_native),
                implicit_confirmation_center: None,
                crop_to_focused_window: false,
            }
        } else {
            ComputerUseScreenshotParams::default()
        };

        // Update cache in state
        {
            let mut s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            s.screenshot_cache = Some(ScreenshotCacheEntry {
                rgba: rgba.clone(),
                screen,
                capture_time: Instant::now(),
            });
        }

        let ui_tree_text = self.enumerate_ui_tree_text().await;

        let (shot, map, nav_out) = tokio::task::spawn_blocking(move || {
            Self::screenshot_sync_tool_with_capture(
                params,
                nav_snapshot,
                rgba,
                screen,
                ui_tree_text,
                implicit_applied,
            )
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;

        let refinement = Self::refinement_from_shot(&shot);
        {
            let mut s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            s.transition_after_screenshot(map, refinement, nav_out);
            if let Some(id) = shot.screenshot_id.clone() {
                s.screenshot_pointer_maps.insert(id, map);
            }
        }

        Ok(shot)
    }

    async fn screenshot_peek_full_display(&self) -> BitFunResult<ComputerScreenshot> {
        // Phase 1 fix: previously this captured `Screen::from_point(0, 0)`
        // (the primary display) which broke confirmation flows on multi-monitor
        // setups. We now prefer the screen that backs the most recent main
        // screenshot — that is the frame of reference the model is reasoning
        // against — falling back to the screen under the mouse, then primary.
        let (cached_screen, preferred_display_id) = {
            let s = self.state.lock().ok();
            s.map(|s| {
                (
                    s.screenshot_cache.as_ref().map(|c| c.screen),
                    s.preferred_display_id,
                )
            })
            .unwrap_or((None, None))
        };
        let (mouse_x, mouse_y) = Self::current_mouse_position();
        let ui_tree_text = self.enumerate_ui_tree_text().await;

        let (shot, _map, _) = tokio::task::spawn_blocking(move || {
            let mx = mouse_x.round() as i32;
            let my = mouse_y.round() as i32;
            // Phase 2 fix: honor `preferred_display_id` first so a model that
            // pinned a display via `desktop.focus_display` consistently sees
            // peek frames from that display, even if the cached screenshot
            // is from a different one.
            let pinned_screen = preferred_display_id.and_then(Self::find_screen_by_id);
            let screen = pinned_screen
                .or(cached_screen)
                .or_else(|| Screen::from_point(mx, my).ok())
                .or_else(|| Screen::from_point(0, 0).ok())
                .ok_or_else(|| {
                    BitFunError::tool(
                        "Screen capture init (peek): no display available".to_string(),
                    )
                })?;
            let rgba = screen
                .capture()
                .map_err(|e| BitFunError::tool(format!("Screenshot failed (peek): {}", e)))?;
            Self::screenshot_sync_tool_with_capture(
                ComputerUseScreenshotParams::default(),
                None,
                rgba,
                screen,
                ui_tree_text,
                false,
            )
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        Ok(shot)
    }

    async fn ocr_find_text_matches(
        &self,
        text_query: &str,
        region_native: Option<bitfun_core::agentic::tools::computer_use_host::OcrRegionNative>,
    ) -> BitFunResult<Vec<bitfun_core::agentic::tools::computer_use_host::OcrTextMatch>> {
        let region_opt = region_native.clone();
        let shot = tokio::task::spawn_blocking(move || {
            let region = Self::ocr_resolve_region_for_capture(region_opt)?;
            Self::screenshot_raw_native_region(region)
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        let query = text_query.to_string();
        let desktop_matches = tokio::task::spawn_blocking(move || {
            // Vision (`VNRecognizeTextRequest`) can throw `NSException` on
            // malformed images / OOM. Catch it so OCR failures degrade to
            // an empty match list instead of aborting the runtime.
            #[cfg(target_os = "macos")]
            {
                macos::catch_objc_local(|| super::screen_ocr::find_text_matches(&shot, &query))
            }
            #[cfg(not(target_os = "macos"))]
            {
                super::screen_ocr::find_text_matches(&shot, &query)
            }
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        Ok(desktop_matches
            .into_iter()
            .map(
                |m| bitfun_core::agentic::tools::computer_use_host::OcrTextMatch {
                    text: m.text,
                    confidence: m.confidence,
                    center_x: m.center_x,
                    center_y: m.center_y,
                    bounds_left: m.bounds_left,
                    bounds_top: m.bounds_top,
                    bounds_width: m.bounds_width,
                    bounds_height: m.bounds_height,
                },
            )
            .collect())
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
        let region = Self::ocr_region_square_around_point(gx, gy, half_extent_native)?;
        let shot = tokio::task::spawn_blocking(move || Self::screenshot_raw_native_region(region))
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
        Ok(shot.bytes)
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
                let output = std::process::Command::new("cmd")
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

    fn map_image_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)> {
        let (gx, gy) = self.map_image_coords_to_pointer_f64(x, y)?;
        Ok((gx.round() as i32, gy.round() as i32))
    }

    fn map_normalized_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
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

    fn map_normalized_coords_to_pointer(&self, x: i32, y: i32) -> BitFunResult<(i32, i32)> {
        let (gx, gy) = self.map_normalized_coords_to_pointer_f64(x, y)?;
        Ok((gx.round() as i32, gy.round() as i32))
    }

    async fn mouse_move_global_f64(&self, gx: f64, gy: f64) -> BitFunResult<()> {
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

    async fn mouse_move(&self, x: i32, y: i32) -> BitFunResult<()> {
        self.mouse_move_global_f64(x as f64, y as f64).await
    }

    async fn pointer_move_relative(&self, dx: i32, dy: i32) -> BitFunResult<()> {
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
            return Ok(());
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
            return Ok(());
        }
    }

    async fn mouse_click(&self, button: &str) -> BitFunResult<()> {
        debug!("computer_use: mouse_click button={}", button);
        ComputerUseHost::computer_use_guard_click_allowed(self)?;
        self.mouse_click_at_current_pointer(button).await
    }

    async fn mouse_click_authoritative(&self, button: &str) -> BitFunResult<()> {
        debug!("computer_use: mouse_click_authoritative button={}", button);
        self.mouse_click_at_current_pointer(button).await
    }

    async fn mouse_down(&self, button: &str) -> BitFunResult<()> {
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

    async fn mouse_up(&self, button: &str) -> BitFunResult<()> {
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

    async fn scroll(&self, delta_x: i32, delta_y: i32) -> BitFunResult<()> {
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

    async fn key_chord(&self, keys: Vec<String>) -> BitFunResult<()> {
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

    async fn type_text(&self, text: &str) -> BitFunResult<()> {
        if text.is_empty() {
            return Ok(());
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
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    fn supports_ax_tree(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            true
        }
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(not(target_os = "macos"))]
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

    async fn app_click(&self, params: AppClickParams) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &params.app).await?;
            let self_pid = std::process::id() as i32;
            info!(
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
                info!(
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

                // Best-effort foreground activation — required for WKWebView
                // and many Cocoa hit-testers to actually deliver our
                // synthetic events. No-op (returns false) when the pid is
                // already frontmost.
                let activate_pid = pid;
                let _ = tokio::task::spawn_blocking(move || {
                    macos::catch_objc(|| {
                        crate::computer_use::macos_bg_input::activate_pid_macos(activate_pid)
                    })
                })
                .await;

                let mods_for_bg = mods.clone();
                tokio::task::spawn_blocking(move || {
                    macos::catch_objc(|| {
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
            self.get_app_state(params.app, 32, false).await
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = params;
            Err(BitFunError::tool(
                "app_click is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn app_type_text(
        &self,
        app: AppSelector,
        text: &str,
        focus: Option<ClickTarget>,
    ) -> BitFunResult<AppStateSnapshot> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &app).await?;
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
            info!(
                target: "computer_use::app_type_text",
                "app_type_text.bg_dispatch pid={} char_count={}",
                pid,
                text.chars().count()
            );
            let activate_pid = pid;
            let _ = tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| {
                    crate::computer_use::macos_bg_input::activate_pid_macos(activate_pid)
                })
            })
            .await;
            let txt = text.to_string();
            tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| crate::computer_use::macos_bg_input::bg_type_text(pid, &txt))
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, text, focus);
            Err(BitFunError::tool(
                "app_type_text is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn app_scroll(
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
            tokio::task::spawn_blocking(move || {
                macos::catch_objc(|| crate::computer_use::macos_bg_input::bg_scroll(pid, dx, dy))
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            self.get_app_state(app, 32, false).await
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, focus, dx, dy);
            Err(BitFunError::tool(
                "app_scroll is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn app_key_chord(
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, keys, focus_idx);
            Err(BitFunError::tool(
                "app_key_chord is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn app_wait_for(
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, pred, timeout_ms, poll_ms);
            Err(BitFunError::tool(
                "app_wait_for is only available on macOS in this build".to_string(),
            ))
        }
    }

    fn supports_interactive_view(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn supports_visual_mark_view(&self) -> bool {
        cfg!(target_os = "macos")
    }

    async fn build_interactive_view(
        &self,
        app: AppSelector,
        opts: InteractiveViewOpts,
    ) -> BitFunResult<InteractiveView> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &app).await?;
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, opts);
            Err(BitFunError::tool(
                "build_interactive_view is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn interactive_click(
        &self,
        app: AppSelector,
        params: InteractiveClickParams,
    ) -> BitFunResult<InteractiveActionResult> {
        #[cfg(target_os = "macos")]
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(
                "interactive_click is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn build_visual_mark_view(
        &self,
        app: AppSelector,
        opts: VisualMarkViewOpts,
    ) -> BitFunResult<VisualMarkView> {
        #[cfg(target_os = "macos")]
        {
            let pid = resolve_pid_macos(self, &app).await?;
            let mut snap = self
                .get_app_state_inner(app.clone(), 16, true, true)
                .await?;
            if snap.screenshot.is_none() {
                if let Ok(shot) = self.screenshot_for_app_pid(pid).await {
                    snap.screenshot = Some(shot);
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, opts);
            Err(BitFunError::tool(
                "build_visual_mark_view is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn visual_click(
        &self,
        app: AppSelector,
        params: VisualClickParams,
    ) -> BitFunResult<VisualActionResult> {
        #[cfg(target_os = "macos")]
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
                let pid = resolve_pid_macos(self, &app).await?;
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(
                "visual_click is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn interactive_type_text(
        &self,
        app: AppSelector,
        params: InteractiveTypeTextParams,
    ) -> BitFunResult<InteractiveActionResult> {
        #[cfg(target_os = "macos")]
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
                let pid = resolve_pid_macos(self, &app).await?;
                tokio::task::spawn_blocking(move || -> BitFunResult<()> {
                    macos::catch_objc(|| {
                        let (m1, k1) = crate::computer_use::macos_bg_input::parse_key_sequence(&[
                            "cmd".to_string(),
                            "a".to_string(),
                        ])?;
                        crate::computer_use::macos_bg_input::bg_key_chord(pid, &m1, k1)?;
                        let (m2, k2) = crate::computer_use::macos_bg_input::parse_key_sequence(&[
                            "delete".to_string(),
                        ])?;
                        crate::computer_use::macos_bg_input::bg_key_chord(pid, &m2, k2)?;
                        Ok(())
                    })
                })
                .await
                .map_err(|e| BitFunError::tool(e.to_string()))??;
            }

            let snapshot = self.app_type_text(app.clone(), &params.text, focus).await?;

            if params.press_enter_after {
                let pid = resolve_pid_macos(self, &app).await?;
                tokio::task::spawn_blocking(move || -> BitFunResult<()> {
                    macos::catch_objc(|| {
                        let (m, k) = crate::computer_use::macos_bg_input::parse_key_sequence(&[
                            "return".to_string(),
                        ])?;
                        crate::computer_use::macos_bg_input::bg_key_chord(pid, &m, k)?;
                        Ok(())
                    })
                })
                .await
                .map_err(|e| BitFunError::tool(e.to_string()))??;
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(
                "interactive_type_text is only available on macOS in this build".to_string(),
            ))
        }
    }

    async fn interactive_scroll(
        &self,
        app: AppSelector,
        params: InteractiveScrollParams,
    ) -> BitFunResult<InteractiveActionResult> {
        #[cfg(target_os = "macos")]
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, params);
            Err(BitFunError::tool(
                "interactive_scroll is only available on macOS in this build".to_string(),
            ))
        }
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

fn detect_regular_grid_rect_from_screenshot(
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
    #[cfg(target_os = "macos")]
    async fn cached_interactive_image_center(
        &self,
        app: &AppSelector,
        i: u32,
    ) -> Option<(i32, i32)> {
        let pid = resolve_pid_macos(self, app).await.ok()?;
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
    #[cfg(target_os = "macos")]
    async fn resolve_interactive_index(
        &self,
        app: &AppSelector,
        i: u32,
        before_digest: Option<&str>,
    ) -> BitFunResult<u32> {
        let pid = resolve_pid_macos(self, app).await?;
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

    #[cfg(target_os = "macos")]
    async fn resolve_visual_mark(
        &self,
        app: &AppSelector,
        i: u32,
        before_digest: Option<&str>,
    ) -> BitFunResult<VisualMark> {
        let pid = resolve_pid_macos(self, app).await?;
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
