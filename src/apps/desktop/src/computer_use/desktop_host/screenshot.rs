//! Screenshot capture, encode, and pointer-overlay pipeline for the desktop
//! Computer Use host: full/cropped/quadrant capture composition, JPEG
//! byte-budget downscaling, OCR region resolution, and coordinate mapping
//! ([`PointerMap`] / [`MacPointerGeo`]) between screenshot pixels and global
//! display coordinates.
//!
//! Extracted from `desktop_host/mod.rs` (no behavior change) so the
//! screenshot subsystem has a single, independently reviewable home instead
//! of living inline inside the multi-thousand-line host file.

#[cfg(target_os = "macos")]
use super::macos;
use super::DesktopComputerUseHost;
use bitfun_core::agentic::tools::computer_use_host::{
    clamp_point_crop_half_extent, ComputerScreenshot, ComputerUseDisplayInfo, ComputerUseHost,
    ComputerUseImageContentRect, ComputerUseImageGlobalBounds, ComputerUseNavigateQuadrant,
    ComputerUseNavigationRect, ComputerUseScreenshotParams, ComputerUseScreenshotRefinement,
    OcrRegionNative, OcrTextMatch, ScreenshotCropCenter,
    COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE, COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, Rgb, RgbImage};
use log::{debug, warn};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;
use screenshots::display_info::DisplayInfo;
use screenshots::Screen;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Default pointer overlay; replace `assets/computer_use_pointer.svg` and rebuild to customize.
/// Hotspot in SVG user space must stay at **(0,0)** (arrow tip).
const POINTER_OVERLAY_SVG: &str = include_str!("../../../assets/computer_use_pointer.svg");

/// Screenshot cache validity duration (ms) - reuse full capture for subsequent crops within this window
const SCREENSHOT_CACHE_TTL_MS: u64 = 300;

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

#[derive(Debug, Clone)]
pub(super) struct ScreenshotCacheEntry {
    pub(super) rgba: image::RgbaImage,
    pub(super) screen: Screen,
    pub(super) capture_time: Instant,
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
pub(super) struct MacPointerGeo {
    pub(super) disp_ox: f64,
    pub(super) disp_oy: f64,
    pub(super) disp_w: f64,
    pub(super) disp_h: f64,
    pub(super) full_px_w: u32,
    pub(super) full_px_h: u32,
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
pub(super) struct PointerMap {
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
    pub(super) macos_geo: Option<MacPointerGeo>,
}

impl PointerMap {
    /// Continuous mapping: **composed JPEG** pixel `(x,y)` -> global (macOS CG).
    pub(super) fn map_image_to_global_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
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
    pub(super) fn map_normalized_to_global_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
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
pub(super) enum ComputerUseNavFocus {
    FullDisplay,
    Quadrant { rect: ComputerUseNavigationRect },
    PointCrop { rect: ComputerUseNavigationRect },
}

impl DesktopComputerUseHost {
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
            let (gx, gy) = Self::current_mouse_position();
            match Self::pointer_in_scaled_image(
                map_origin_x,
                map_origin_y,
                map_native_w,
                map_native_h,
                content_w,
                content_h,
                gx.round() as i32,
                gy.round() as i32,
            ) {
                Some((ix, iy)) => {
                    let px = ix + margin_l as i32;
                    let py = iy + margin_t as i32;
                    Self::draw_pointer_marker(&mut frame, px, py);
                    (Some(px), Some(py))
                }
                None => (None, None),
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
    pub(super) fn enumerate_displays(
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
}

impl DesktopComputerUseHost {
    #[cfg(target_os = "macos")]
    pub(super) async fn screenshot_for_app_pid(
        &self,
        pid: i32,
    ) -> BitFunResult<ComputerScreenshot> {
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

    /// Capture the foreground window on Windows, build a [`ComputerScreenshot`]
    /// whose image pixels map 1:1 to the window's screen rectangle, and register
    /// the resulting [`PointerMap`] under both `pid` and the screenshot id so
    /// follow-up `ClickTarget::ImageXy` / `ImageGrid` calls resolve image pixels
    /// back to the right screen coordinates.
    ///
    /// `hwnd_raw` is the foreground window handle the AX snapshot was taken from
    /// (so the screenshot and the tree describe the same window). The capture is
    /// the window's own pixels (`PrintWindow`), cropped to the DWM extended
    /// frame, with `origin_*` adjusted for that crop.
    #[cfg(target_os = "windows")]
    pub(super) async fn screenshot_for_foreground_window(
        &self,
        pid: i32,
        hwnd_raw: isize,
    ) -> BitFunResult<ComputerScreenshot> {
        use windows::Win32::Foundation::HWND;

        let cap = tokio::task::spawn_blocking(move || {
            let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
            crate::computer_use::windows_capture::screenshot_window_capture(hwnd)
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;

        let img = image::load_from_memory(&cap.png)
            .map_err(|e| BitFunError::tool(format!("decode window capture PNG: {}", e)))?;
        let rgb = img.to_rgb8();
        let native_w = rgb.width();
        let native_h = rgb.height();

        let shot =
            Self::raw_shot_from_rgb_crop(rgb, cap.origin_x, cap.origin_y, native_w, native_h)?;

        // Image pixels map 1:1 to the captured window rectangle (no downscale),
        // so content == image == native and the screen origin is the window's
        // (DWM-frame-adjusted) top-left.
        let map = PointerMap {
            image_w: shot.image_width,
            image_h: shot.image_height,
            content_origin_x: 0,
            content_origin_y: 0,
            content_w: shot.image_width,
            content_h: shot.image_height,
            native_w,
            native_h,
            origin_x: cap.origin_x,
            origin_y: cap.origin_y,
        };
        {
            let mut s = self
                .state
                .lock()
                .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
            s.pointer_map = Some(map);
            s.app_pointer_maps.insert(pid, map);
            if let Some(id) = shot.screenshot_id.clone() {
                s.screenshot_pointer_maps.insert(id, map);
            }
        }
        Ok(shot)
    }
}

/// Inherent implementations backing the [`ComputerUseHost`] trait's screenshot
/// and OCR-capture methods (see `mod.rs`'s thin trait-method delegators).
impl DesktopComputerUseHost {
    pub(super) async fn screenshot_display_impl(
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

    pub(super) async fn screenshot_peek_full_display_impl(
        &self,
    ) -> BitFunResult<ComputerScreenshot> {
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

    pub(super) async fn ocr_find_text_matches_impl(
        &self,
        text_query: &str,
        region_native: Option<bitfun_core::agentic::tools::computer_use_host::OcrRegionNative>,
    ) -> BitFunResult<Vec<OcrTextMatch>> {
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
                macos::catch_objc_local(|| {
                    crate::computer_use::screen_ocr::find_text_matches(&shot, &query)
                })
            }
            #[cfg(not(target_os = "macos"))]
            {
                crate::computer_use::screen_ocr::find_text_matches(&shot, &query)
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

    pub(super) async fn ocr_preview_crop_jpeg_impl(
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
}
