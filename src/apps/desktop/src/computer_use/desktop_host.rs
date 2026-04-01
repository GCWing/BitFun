//! Cross-platform `ComputerUseHost` via `screenshots` + `enigo`.

use async_trait::async_trait;
use bitfun_core::agentic::tools::computer_use_host::{
    clamp_point_crop_half_extent, ActionRecord, ComputerScreenshot, ComputerUseHost,
    ComputerUseImageContentRect, ComputerUseImplicitScreenshotCenter,
    ComputerUseInteractionScreenshotKind, ComputerUseInteractionState, ComputerUseNavigateQuadrant,
    ComputerUseNavigationRect, ComputerUsePermissionSnapshot, ComputerUseScreenshotParams,
    ComputerUseScreenshotRefinement, ComputerUseSessionSnapshot, LoopDetectionResult,
    OcrRegionNative, ScreenshotCropCenter, SomElement, UiElementLocateQuery, UiElementLocateResult,
    COMPUTER_USE_POINT_CROP_HALF_DEFAULT, COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE,
    COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
};
use bitfun_core::agentic::tools::computer_use_optimizer::ComputerUseOptimizer;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use bitfun_core::agentic::tools::computer_use_host::{
    ComputerUseForegroundApplication, ComputerUsePointerGlobal,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use enigo::{
    Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings,
};
use fontdue::{Font, FontSettings};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, Rgb, RgbImage};
use log::{debug, warn};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;
use screenshots::display_info::DisplayInfo;
use screenshots::Screen;
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
const VISION_PIXEL_NUDGE_AFTER_SCREENSHOT_MSG: &str = "Computer use refused: do not use `pointer_move_rel` or `ComputerUseMouseStep` immediately after a `screenshot` — nudging from the JPEG is inaccurate. First reposition with `move_to_text`, `click_element`, `click_label`, `locate` + `mouse_move` (`use_screen_coordinates`: true), or `mouse_move` using globals from tool JSON; then relative nudges are allowed if still needed.";

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

fn pointer_pixmap_cache() -> Option<&'static PointerPixmapCache> {
    POINTER_PIXMAP_CACHE
        .get_or_init(|| match rasterize_pointer_svg(POINTER_OVERLAY_SVG, 0.3375) {
            Ok(p) => Some(p),
            Err(e) => {
                warn!(
                    "computer_use: pointer SVG rasterize failed ({}); using fallback cross",
                    e
                );
                None
            }
        })
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

// ── SoM / overlay text: Inter (OFL) via fontdue ──

/// Inter (OFL); variable font from google/fonts OFL tree.
const COORD_AXIS_FONT_TTF: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");

static COORD_AXIS_FONT: OnceLock<Font> = OnceLock::new();

fn coord_axis_font() -> &'static Font {
    COORD_AXIS_FONT.get_or_init(|| {
        Font::from_bytes(COORD_AXIS_FONT_TTF, FontSettings::default())
            .expect("Inter TTF embedded for computer-use SoM/overlay labels")
    })
}

/// Alpha-blend grayscale coverage onto `img` (baseline-anchored glyph).
fn coord_blit_glyph(
    img: &mut RgbImage,
    baseline_x: i32,
    baseline_y: i32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
    fg: Rgb<u8>,
) {
    let w = metrics.width;
    let h = metrics.height;
    if w == 0 || h == 0 {
        return;
    }
    let iw = img.width() as i32;
    let ih = img.height() as i32;
    let xmin = metrics.xmin as i32;
    let ymin = metrics.ymin as i32;
    for row in 0..h {
        for col in 0..w {
            let alpha = bitmap[row * w + col] as u32;
            if alpha == 0 {
                continue;
            }
            let px = baseline_x + xmin + col as i32;
            let py = baseline_y + ymin + row as i32;
            if px < 0 || py < 0 || px >= iw || py >= ih {
                continue;
            }
            let dst = img.get_pixel(px as u32, py as u32);
            let inv = 255u32.saturating_sub(alpha);
            let nr = ((fg[0] as u32 * alpha + dst[0] as u32 * inv) / 255).min(255) as u8;
            let ng = ((fg[1] as u32 * alpha + dst[1] as u32 * inv) / 255).min(255) as u8;
            let nb = ((fg[2] as u32 * alpha + dst[2] as u32 * inv) / 255).min(255) as u8;
            img.put_pixel(px as u32, py as u32, Rgb([nr, ng, nb]));
        }
    }
}

/// Axis numerals: synthetic bold via small 2×2 offset stack (still Inter Regular source).
fn coord_blit_glyph_bold(
    img: &mut RgbImage,
    baseline_x: i32,
    baseline_y: i32,
    metrics: &fontdue::Metrics,
    bitmap: &[u8],
    fg: Rgb<u8>,
) {
    coord_blit_glyph(img, baseline_x, baseline_y, metrics, bitmap, fg);
    coord_blit_glyph(img, baseline_x + 1, baseline_y, metrics, bitmap, fg);
    coord_blit_glyph(img, baseline_x, baseline_y + 1, metrics, bitmap, fg);
    coord_blit_glyph(img, baseline_x + 1, baseline_y + 1, metrics, bitmap, fg);
}

fn coord_measure_str_width(text: &str, px: f32) -> i32 {
    let font = coord_axis_font();
    let mut adv = 0f32;
    for c in text.chars() {
        adv += font.metrics(c, px).advance_width;
    }
    adv.ceil() as i32
}

/// Left-to-right string on one baseline.
fn coord_draw_text_h(img: &mut RgbImage, mut baseline_x: i32, baseline_y: i32, text: &str, fg: Rgb<u8>, px: f32) {
    let font = coord_axis_font();
    for c in text.chars() {
        let (m, bmp) = font.rasterize(c, px);
        coord_blit_glyph_bold(img, baseline_x, baseline_y, &m, &bmp, fg);
        baseline_x += m.advance_width.ceil() as i32;
    }
}

// ── Set-of-Mark (SoM) label rendering ──

/// Badge font size for SoM labels (smaller than axis labels).
const SOM_LABEL_PX: f32 = 28.0;
/// Badge background color (bright magenta -- high contrast on most UIs).
const SOM_BG: Rgb<u8> = Rgb([230, 40, 120]);
/// Badge text color.
const SOM_FG: Rgb<u8> = Rgb([255, 255, 255]);
/// Padding around the label text inside the badge.
const SOM_PAD_X: i32 = 4;
const SOM_PAD_Y: i32 = 2;

/// Draw SoM numbered labels on the frame at each element's mapped image position.
/// `elements`: SoM elements with global coordinates.
/// `margin_l`, `margin_t`: content area offset in the frame.
/// `map_fn`: maps global (f64,f64) -> Option<(i32,i32)> in content-area pixel space.
fn draw_som_labels<F>(
    frame: &mut RgbImage,
    elements: &[SomElement],
    margin_l: u32,
    margin_t: u32,
    map_fn: F,
) where
    F: Fn(f64, f64) -> Option<(i32, i32)>,
{
    let font = coord_axis_font();
    let (fw, fh) = frame.dimensions();

    for elem in elements {
        let Some((cx, cy)) = map_fn(elem.global_center_x, elem.global_center_y) else {
            continue;
        };

        // Map from content-area space to frame space
        let img_x = cx + margin_l as i32;
        let img_y = cy + margin_t as i32;

        // Measure label text width
        let label_text = elem.label.to_string();
        let text_w = coord_measure_str_width(&label_text, SOM_LABEL_PX);
        let (m_rep, _) = font.rasterize('8', SOM_LABEL_PX);
        let text_h = m_rep.height as i32;

        let badge_w = text_w + SOM_PAD_X * 2 + 2; // +2 for bold offset
        let badge_h = text_h + SOM_PAD_Y * 2;

        // Position badge at top-left of element's bounds (mapped to image),
        // but fall back to center if bounds mapping fails
        let (badge_x, badge_y) = {
            let bx = elem.bounds_left;
            let by = elem.bounds_top;
            if let Some((bix, biy)) = map_fn(bx, by) {
                (bix + margin_l as i32, biy + margin_t as i32)
            } else {
                // Fall back to center
                (img_x - badge_w / 2, img_y - badge_h / 2)
            }
        };

        // Clamp to frame bounds
        let bx0 = badge_x.max(0).min(fw as i32 - badge_w);
        let by0 = badge_y.max(0).min(fh as i32 - badge_h);

        // Draw badge background rectangle
        for dy in 0..badge_h {
            for dx in 0..badge_w {
                let px = bx0 + dx;
                let py = by0 + dy;
                if px >= 0 && px < fw as i32 && py >= 0 && py < fh as i32 {
                    frame.put_pixel(px as u32, py as u32, SOM_BG);
                }
            }
        }

        // Draw label text centered in badge
        let text_x = bx0 + SOM_PAD_X;
        let baseline_y = by0 + SOM_PAD_Y + text_h - (m_rep.ymin.max(0) as i32);
        coord_draw_text_h(frame, text_x, baseline_y, &label_text, SOM_FG, SOM_LABEL_PX);
    }
}

/// Returns the capture bitmap unchanged (no grid, rulers, or margins). Pointer and SoM overlays are applied later.
fn compose_computer_use_frame(
    content: RgbImage,
    _ruler_origin_x: u32,
    _ruler_origin_y: u32,
) -> (RgbImage, u32, u32) {
    (content, 0, 0)
}

fn implicit_confirmation_should_apply(click_needs: bool, params: &ComputerUseScreenshotParams) -> bool {
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
    if params.crop_center.is_some()
        || params.navigate_quadrant.is_some()
        || params.reset_navigation
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
fn implicit_global_center_for_confirmation(
    center: ComputerUseImplicitScreenshotCenter,
    mx: f64,
    my: f64,
) -> (f64, f64) {
    let _ = center;
    (mx, my)
}

/// JPEG quality for computer-use screenshots. Native display resolution is preserved (no downscale)
/// so `coordinate_mode` \"image\" pixel indices match the screen capture 1:1. Very large displays
/// increase request payload size; if the API rejects the image, lower quality or split workflows may be needed.
const JPEG_QUALITY: u8 = 75;

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
    let x1 = r
        .x0
        .saturating_add(r.width)
        .saturating_add(pad)
        .min(max_w);
    let y1 = r
        .y0
        .saturating_add(r.height)
        .saturating_add(pad)
        .min(max_h);
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
            return Err(BitFunError::tool("Invalid macOS pointer geometry.".to_string()));
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
    fn global_to_view_pixel(&self, mx: f64, my: f64, view_w: u32, view_h: u32) -> Option<(i32, i32)> {
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
}

/// What the last tool `screenshot` implied for **plain** follow-up captures (no crop / no `navigate_quadrant`).
/// **PointCrop** is not reused for plain refresh: the next bare `screenshot` shows the **full display** again so
/// "full" is never stuck at ~500×500 after a point crop. **Quadrant** plain refresh keeps the current drill tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComputerUseNavFocus {
    FullDisplay,
    Quadrant {
        rect: ComputerUseNavigationRect,
    },
    PointCrop {
        rect: ComputerUseNavigationRect,
    },
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
    /// from AX/OCR/globals (`mouse_move`, `move_to_text`, `click_element`, `click_label`) clears this.
    block_vision_pixel_nudge_after_screenshot: bool,
    /// After click / key / type / scroll / drag: recommend a **`screenshot`** to confirm UI state (Cowork verify).
    /// Cleared on the next successful `screenshot_display`.
    pending_verify_screenshot: bool,
    /// After `move_to_text` (global OCR coordinates): next guarded **`click`** may run without a prior
    /// `screenshot_display` / fine-crop basis — same idea as `click_element` relaxed guard.
    pointer_trusted_after_ocr_move: bool,
    /// Action optimizer for loop detection, history, and visual verification.
    optimizer: ComputerUseOptimizer,
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
    }

    /// Called after pointer mutation (move, step, relative), click, scroll, key_chord, or type_text.
    fn transition_after_pointer_mutation(&mut self) {
        self.click_needs_fresh_screenshot = true;
        self.pointer_trusted_after_ocr_move = false;
    }

    /// Called after click (same effect as pointer mutation for freshness).
    fn transition_after_click(&mut self) {
        self.click_needs_fresh_screenshot = true;
        self.pending_verify_screenshot = true;
        self.pointer_trusted_after_ocr_move = false;
    }

    /// Called after key, typing, scroll, or drag — UI likely changed; next `screenshot` should confirm.
    fn transition_after_committed_ui_action(&mut self) {
        self.pending_verify_screenshot = true;
    }
}

pub struct DesktopComputerUseHost {
    state: Mutex<ComputerUseSessionMutableState>,
}

impl std::fmt::Debug for DesktopComputerUseHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopComputerUseHost").finish_non_exhaustive()
    }
}

impl DesktopComputerUseHost {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ComputerUseSessionMutableState::new()),
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
            return Self::session_snapshot_windows();
        }
        #[cfg(target_os = "linux")]
        {
            return Self::session_snapshot_linux();
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        )))]
        {
            ComputerUseSessionSnapshot::default()
        }
    }

    #[cfg(target_os = "macos")]
    fn session_snapshot_macos() -> ComputerUseSessionSnapshot {
        let pointer = macos::quartz_mouse_location().ok().map(|(x, y)| ComputerUsePointerGlobal { x, y });
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
                    name: if title.is_empty() {
                        None
                    } else {
                        Some(title)
                    },
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
        let mut enigo = Enigo::new(&settings)
            .map_err(|e| BitFunError::tool(format!("enigo init: {}", e)))?;
        f(&mut enigo)
    }

    /// Enigo on macOS uses Text Input Source / AppKit paths that must run on the main queue.
    /// Tokio `spawn_blocking` threads are not main; dispatch there hits `dispatch_assert_queue_fail`.
    fn run_enigo_job<F, T>(job: F) -> BitFunResult<T>
    where
        F: FnOnce(&mut Enigo) -> BitFunResult<T> + Send,
        T: Send,
    {
        #[cfg(target_os = "macos")]
        {
            macos::run_on_main_for_enigo(|| Self::with_enigo(job))
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

        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| {
            BitFunError::tool("CGEventSource create failed (mouse_move)".to_string())
        })?;
        let pt = CGPoint { x, y };
        let ev = CGEvent::new_mouse_event(
            source,
            CGEventType::MouseMoved,
            pt,
            CGMouseButton::Left,
        )
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
                return Err(BitFunError::tool(format!(
                    "Unknown key name: {}",
                    name
                )));
            }
        })
    }

    fn encode_jpeg(rgb: &RgbImage, quality: u8) -> BitFunResult<Vec<u8>> {
        let mut buf = Vec::new();
        let mut enc = JpegEncoder::new_with_quality(&mut buf, quality);
        enc
            .encode(rgb.as_raw(), rgb.width(), rgb.height(), image::ColorType::Rgb8)
            .map_err(|e| BitFunError::tool(format!("JPEG encode: {}", e)))?;
        Ok(buf)
    }

    /// JPEG for OCR only: **no** pointer/SoM overlay — raw capture pixels.
    const OCR_RAW_JPEG_QUALITY: u8 = 75;

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
            som_labels: vec![],
            implicit_confirmation_crop_applied: false,
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
    fn ocr_resolve_region_for_capture(region_native: Option<OcrRegionNative>) -> BitFunResult<OcrRegionNative> {
        if let Some(r) = region_native {
            return Ok(r);
        }
        #[cfg(target_os = "macos")]
        {
            match crate::computer_use::macos_ax_ui::frontmost_window_bounds_global() {
                Ok((x0, y0, w, h)) => Ok(OcrRegionNative { x0, y0, width: w, height: h }),
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

    /// Capture **raw** display pixels (no pointer/SoM overlay), cropped to `region` intersected with the chosen display.
    ///
    /// `region` and [`DisplayInfo::width`]/[`height`] are **global logical points** (CG / AX). The framebuffer
    /// is **physical pixels** on Retina; intersect in point space, then map to pixels like [`MacPointerGeo`].
    fn screenshot_raw_native_region(region: OcrRegionNative) -> BitFunResult<ComputerScreenshot> {
        let cx = region.x0 + region.width as i32 / 2;
        let cy = region.y0 + region.height as i32 / 2;
        let screen = Screen::from_point(cx, cy)
            .or_else(|_| Screen::from_point(0, 0))
            .map_err(|e| BitFunError::tool(format!("Screen capture init (OCR raw): {}", e)))?;
        let rgba = screen.capture().map_err(|e| {
            BitFunError::tool(format!("Screenshot failed (OCR raw): {}", e))
        })?;
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
        let span_w = ((crop_w as f64 / full_px_w as f64) * disp_w).round().max(1.0) as u32;
        let span_h = ((crop_h as f64 / full_px_h as f64) * disp_h).round().max(1.0) as u32;
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
        som_elements: Vec<SomElement>,
        implicit_confirmation_crop_applied: bool,
    ) -> BitFunResult<(
        ComputerScreenshot,
        PointerMap,
        Option<ComputerUseNavFocus>,
    )> {
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
            Some(ComputerUseNavFocus::Quadrant { rect }) => {
                Some(ComputerUseNavFocus::Quadrant {
                    rect: intersect_navigation_rect(rect, full_rect).unwrap_or(full_rect),
                })
            }
            Some(ComputerUseNavFocus::PointCrop { rect }) => {
                Some(ComputerUseNavFocus::PointCrop {
                    rect: intersect_navigation_rect(rect, full_rect).unwrap_or(full_rect),
                })
            }
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
                Some(ComputerUseNavFocus::Quadrant { rect }) | Some(ComputerUseNavFocus::PointCrop { rect }) => {
                    rect
                }
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
            let expanded =
                expand_navigation_rect_edges(split, COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX, native_w, native_h);
            let Some(new_rect) = intersect_navigation_rect(expanded, full_rect) else {
                return Err(BitFunError::tool("Quadrant crop out of bounds.".to_string()));
            };
            let cropped =
                Self::crop_rgb(&full_frame, new_rect.x0, new_rect.y0, new_rect.width, new_rect.height)?;
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
            let is_full = base.x0 == 0
                && base.y0 == 0
                && base.width == native_w
                && base.height == native_h;
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
                    full_frame,
                    origin_x,
                    origin_y,
                    native_w,
                    native_h,
                    native_w,
                    native_h,
                    0u32,
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

        let (mut frame, margin_l, margin_t) = compose_computer_use_frame(
            content_rgb,
            ruler_origin_native_x,
            ruler_origin_native_y,
        );
        let image_content_rect = ComputerUseImageContentRect {
            left: margin_l,
            top: margin_t,
            width: content_w,
            height: content_h,
        };

        let (image_w, image_h) = frame.dimensions();
        let vision_scale = 1.0_f64;

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

        // Draw SoM (Set-of-Mark) numbered labels on the frame
        if !som_elements.is_empty() {
            #[cfg(target_os = "macos")]
            {
                let geo = macos_map_geo;
                draw_som_labels(
                    &mut frame,
                    &som_elements,
                    margin_l,
                    margin_t,
                    |gx, gy| geo.global_to_view_pixel(gx, gy, content_w, content_h),
                );
            }
            #[cfg(not(target_os = "macos"))]
            {
                // On non-macOS: map global -> content pixel using the linear mapping
                draw_som_labels(
                    &mut frame,
                    &som_elements,
                    margin_l,
                    margin_t,
                    |gx, gy| {
                        let ox = map_origin_x as f64;
                        let oy = map_origin_y as f64;
                        let nw = map_native_w as f64;
                        let nh = map_native_h as f64;
                        if nw <= 0.0 || nh <= 0.0 { return None; }
                        let rx = (gx - ox) / nw * content_w as f64;
                        let ry = (gy - oy) / nh * content_h as f64;
                        if rx < 0.0 || ry < 0.0 || rx >= content_w as f64 || ry >= content_h as f64 {
                            return None;
                        }
                        Some((rx.round() as i32, ry.round() as i32))
                    },
                );
            }
        }

        let jpeg_bytes = Self::encode_jpeg(&frame, JPEG_QUALITY)?;

        let point_crop_half_extent_native = params.crop_center.map(|_| {
            clamp_point_crop_half_extent(params.point_crop_half_extent_native)
        });

        let shot = ComputerScreenshot {
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
            som_labels: som_elements,
            implicit_confirmation_crop_applied,
        };

        #[cfg(target_os = "macos")]
        let map = PointerMap {
            image_w,
            image_h,
            content_origin_x: margin_l,
            content_origin_y: margin_t,
            content_w,
            content_h,
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
            content_origin_x: margin_l,
            content_origin_y: margin_t,
            content_w,
            content_h,
            native_w: map_native_w,
            native_h: map_native_h,
            origin_x: map_origin_x,
            origin_y: map_origin_y,
        };

        Ok((shot, map, persist_nav_focus))
    }

    fn permission_sync() -> ComputerUsePermissionSnapshot {
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
            ComputerUsePermissionSnapshot {
                accessibility_granted: true,
                screen_capture_granted: true,
                platform_note: None,
            }
        }
        #[cfg(target_os = "linux")]
        {
            let wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
            ComputerUsePermissionSnapshot {
                accessibility_granted: !wayland,
                screen_capture_granted: !wayland,
                platform_note: if wayland {
                    Some(
                        "Wayland: global automation may be limited; use an X11 session for best results."
                            .to_string(),
                    )
                } else {
                    None
                },
            }
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        )))]
        {
            ComputerUsePermissionSnapshot {
                accessibility_granted: false,
                screen_capture_granted: false,
                platform_note: Some("Computer use is not supported on this OS.".to_string()),
            }
        }
    }

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
    fn resolve_screenshot_capture(
        cached: Option<ScreenshotCacheEntry>,
        mouse_x: f64,
        mouse_y: f64,
    ) -> BitFunResult<(image::RgbaImage, Screen)> {
        let mx = mouse_x.round() as i32;
        let my = mouse_y.round() as i32;

        if let Some(cache) = cached {
            let screen_id_match = cache.screen.display_info.id
                == Screen::from_point(mx, my)
                    .map(|s| s.display_info.id)
                    .unwrap_or_default();
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

        let screen = Screen::from_point(mx, my)
            .or_else(|_| Screen::from_point(0, 0))
            .map_err(|e| BitFunError::tool(format!("Screen capture init: {}", e)))?;
        let rgba = screen.capture().map_err(|e| {
            BitFunError::tool(format!(
                "Screenshot failed (on macOS grant Screen Recording for BitFun): {}",
                e
            ))
        })?;
        Ok((rgba, screen))
    }

    fn chord_includes_return_or_enter(keys: &[String]) -> bool {
        keys.iter().any(|s| {
            matches!(
                s.to_lowercase().as_str(),
                "return" | "enter" | "kp_enter"
            )
        })
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
    pub fn run_on_main_for_enigo<F, T>(f: F) -> T
    where
        F: FnOnce() -> T + Send,
        T: Send,
    {
        unsafe {
            if pthread_main_np() != 0 {
                f()
            } else {
                Queue::main().exec_sync(f)
            }
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
    /// Used after `mouse_move_global_f64` when coordinates came from AX, OCR, or SoM (not from vision model image coords).
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
        ComputerUseHost::computer_use_after_click(self);
        Ok(())
    }
}

#[async_trait]
impl ComputerUseHost for DesktopComputerUseHost {
    async fn permission_snapshot(&self) -> BitFunResult<ComputerUsePermissionSnapshot> {
        Ok(tokio::task::spawn_blocking(Self::permission_sync)
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?)
    }

    fn computer_use_interaction_state(&self) -> ComputerUseInteractionState {
        let s = self.state.lock().unwrap();
        let last_ref = s.last_shot_refinement;
        let click_needs_fresh = s.click_needs_fresh_screenshot;
        let pending_verify = s.pending_verify_screenshot;

        let (click_ready, screenshot_kind, mut recommended_next_action) = match last_ref {
            Some(ComputerUseScreenshotRefinement::RegionAroundPoint { .. }) => (
                !click_needs_fresh,
                Some(ComputerUseInteractionScreenshotKind::RegionCrop),
                None,
            ),
            Some(ComputerUseScreenshotRefinement::QuadrantNavigation { click_ready, .. }) if click_ready => (
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
            last_mutation: None,
            recommended_next_action,
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
        let (nav_snapshot, cached, click_needs) = {
            let s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            (
                s.navigation_focus,
                s.screenshot_cache.clone(),
                s.click_needs_fresh_screenshot,
            )
        };

        // Get current mouse position to select the right screen
        let (mouse_x, mouse_y) = Self::current_mouse_position();

        // Resolve capture from cache or fresh
        let (rgba, screen) = Self::resolve_screenshot_capture(cached, mouse_x, mouse_y)?;
        let (native_w, native_h) = rgba.dimensions();

        let mut params = params;
        let mut implicit_applied = false;
        if implicit_confirmation_should_apply(click_needs, &params) {
            let center = params
                .implicit_confirmation_center
                .unwrap_or(ComputerUseImplicitScreenshotCenter::Mouse);
            let (gx, gy) = implicit_global_center_for_confirmation(center, mouse_x, mouse_y);
            let (cx, cy) = global_to_native_full_pixel_center(
                gx,
                gy,
                native_w,
                native_h,
                &screen.display_info,
            );
            params = ComputerUseScreenshotParams {
                crop_center: Some(ScreenshotCropCenter { x: cx, y: cy }),
                navigate_quadrant: None,
                reset_navigation: false,
                point_crop_half_extent_native: Some(COMPUTER_USE_POINT_CROP_HALF_DEFAULT),
                implicit_confirmation_center: None,
            };
            implicit_applied = true;
        }

        // Update cache in state
        {
            let mut s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            s.screenshot_cache = Some(ScreenshotCacheEntry {
                rgba: rgba.clone(),
                screen: screen.clone(),
                capture_time: Instant::now(),
            });
        }

        // Enumerate SoM elements (AX tree walk) for label overlay
        let som_elements = self.enumerate_som_elements().await;

        let (shot, map, nav_out) = tokio::task::spawn_blocking(move || {
            Self::screenshot_sync_tool_with_capture(
                params,
                nav_snapshot,
                rgba,
                screen,
                som_elements,
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
        }

        Ok(shot)
    }

    async fn screenshot_peek_full_display(&self) -> BitFunResult<ComputerScreenshot> {
        let (shot, _map, _) = tokio::task::spawn_blocking(|| {
            let screen = Screen::from_point(0, 0)
                .map_err(|e| BitFunError::tool(format!("Screen capture init (peek): {}", e)))?;
            let rgba = screen.capture().map_err(|e| {
                BitFunError::tool(format!("Screenshot failed (peek): {}", e))
            })?;
            Self::screenshot_sync_tool_with_capture(
                ComputerUseScreenshotParams::default(),
                None,
                rgba,
                screen,
                vec![], // No SoM labels for peek screenshots
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
            super::screen_ocr::find_text_matches(&shot, &query)
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        Ok(desktop_matches
            .into_iter()
            .map(|m| bitfun_core::agentic::tools::computer_use_host::OcrTextMatch {
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
        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        )))]
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
        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux"
        )))]
        {
            Err(BitFunError::tool(
                "Native UI element (accessibility) lookup is not available on this platform."
                    .to_string(),
            ))
        }
    }

    async fn enumerate_som_elements(&self) -> Vec<SomElement> {
        #[cfg(target_os = "macos")]
        {
            const SOM_MAX_ELEMENTS: usize = 50;
            tokio::task::spawn_blocking(move || {
                crate::computer_use::macos_ax_ui::enumerate_interactive_elements(SOM_MAX_ELEMENTS)
            })
            .await
            .unwrap_or_default()
        }
        #[cfg(not(target_os = "macos"))]
        {
            vec![]
        }
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
                    e.move_mouse(nx, ny, Coordinate::Abs).map_err(|err| {
                        BitFunError::tool(format!("pointer_move_relative: {}", err))
                    })
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
                    e.move_mouse(dx, dy, Coordinate::Rel).map_err(|err| {
                        BitFunError::tool(format!("pointer_move_relative: {}", err))
                    })
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
                    e.scroll(delta_x, Axis::Horizontal).map_err(|err| {
                        BitFunError::tool(format!("scroll horizontal: {}", err))
                    })?;
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
        Ok(())
    }

    async fn key_chord(&self, keys: Vec<String>) -> BitFunResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        debug!("computer_use: key_chord keys={:?}", keys);
        if Self::chord_includes_return_or_enter(&keys) {
            Self::computer_use_guard_verified_ui(self)?;
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
                        "command" | "meta" | "super" | "win" | "control" | "ctrl" | "shift" | "alt" | "option"
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
        Ok(())
    }

    async fn wait_ms(&self, ms: u64) -> BitFunResult<()> {
        tokio::time::sleep(Duration::from_millis(ms.max(1))).await;
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
        match s.last_shot_refinement {
            Some(ComputerUseScreenshotRefinement::RegionAroundPoint { .. }) => {}
            Some(ComputerUseScreenshotRefinement::QuadrantNavigation {
                click_ready: true,
                ..
            }) => {}
            // Fresh full-screen JPEG matches the display — valid for image-space `mouse_move` then
            // guarded `click` as long as `click_needs_fresh_screenshot` is false above.
            Some(ComputerUseScreenshotRefinement::FullDisplay) => {}
            _ => {
                return Err(BitFunError::tool(
                    "Click refused: use a **fine** screenshot basis — either a **~500×500 point crop** (`screenshot_crop_center_x` / `y` in full-display native pixels) **or** keep drilling with `screenshot_navigate_quadrant` until `quadrant_navigation_click_ready` is true in the tool result, then `ComputerUseMousePrecise` / `ComputerUseMouseStep` / **`ComputerUse`** **`mouse_move`** (**`use_screen_coordinates`: true** only) to position, then **`click`**. Or take a **full-screen** `screenshot` (no pointer move since capture), then **`mouse_move`** with globals from tool results, then **`click`**.".to_string(),
                ));
            }
        }
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
            s.optimizer.record_action(
                action_type.to_string(),
                action_params.to_string(),
                success,
            );
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
}
