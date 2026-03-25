//! Cross-platform `ComputerUseHost` via `screenshots` + `enigo`.

use async_trait::async_trait;
use bitfun_core::agentic::tools::computer_use_host::{
    ComputerScreenshot, ComputerUseHost, ComputerUseImageContentRect,
    ComputerUseNavigateQuadrant, ComputerUseNavigationRect, ComputerUsePermissionSnapshot,
    ComputerUseScreenshotParams, ComputerUseScreenshotRefinement, ScreenshotCropCenter,
    COMPUTER_USE_QUADRANT_CLICK_READY_MAX_LONG_EDGE, COMPUTER_USE_QUADRANT_EDGE_EXPAND_PX,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use enigo::{
    Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings,
};
use fontdue::{Font, FontSettings};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, Rgb, RgbImage};
use log::warn;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;
use screenshots::display_info::DisplayInfo;
use screenshots::Screen;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Default pointer overlay; replace `assets/computer_use_pointer.svg` and rebuild to customize.
/// Hotspot in SVG user space must stay at **(0,0)** (arrow tip).
const POINTER_OVERLAY_SVG: &str = include_str!("../../assets/computer_use_pointer.svg");

#[derive(Debug)]
struct PointerPixmapCache {
    w: u32,
    h: u32,
    /// Premultiplied RGBA8 (`tiny-skia` / `resvg` format).
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

// ── Computer-use coordinate grid (100 px step): lines + anti-aliased axis labels (Inter OFL) ──

const COORD_GRID_DEFAULT_STEP: u32 = 100;
const COORD_GRID_MAJOR_STEP: u32 = 500;
/// Logical scale knob; mapped to TTF pixel size for `fontdue` (`scale * 3.5`).
const COORD_LABEL_SCALE: i32 = 11;

/// Inter (OFL); variable font from google/fonts OFL tree.
const COORD_AXIS_FONT_TTF: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");

static COORD_AXIS_FONT: OnceLock<Font> = OnceLock::new();

fn coord_axis_font() -> &'static Font {
    COORD_AXIS_FONT.get_or_init(|| {
        Font::from_bytes(COORD_AXIS_FONT_TTF, FontSettings::default())
            .expect("Inter TTF embedded for computer-use axis labels")
    })
}

#[inline]
fn coord_label_px() -> f32 {
    COORD_LABEL_SCALE as f32 * 3.5
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

/// Vertically center a horizontal digit string on tick `py`.
fn coord_draw_u32_h_centered(img: &mut RgbImage, lx: i32, py: i32, n: u32, fg: Rgb<u8>, px: f32) {
    let s = n.to_string();
    let font = coord_axis_font();
    let (m_rep, _) = font.rasterize('8', px);
    let text_h = m_rep.height as i32;
    let baseline_y = py - (m_rep.ymin + text_h / 2);
    coord_draw_text_h(img, lx, baseline_y, &s, fg, px);
}

#[inline]
fn coord_plot(img: &mut RgbImage, x: i32, y: i32, c: Rgb<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    if x >= 0 && x < w && y >= 0 && y < h {
        img.put_pixel(x as u32, y as u32, c);
    }
}

fn coord_digit_block_width(digit_count: usize, px: f32) -> i32 {
    if digit_count == 0 {
        return 0;
    }
    let s: String = std::iter::repeat('8').take(digit_count).collect();
    coord_measure_str_width(&s, px)
}

/// Height of a vertical digit stack (top-to-bottom) for `nd` decimal digits.
fn coord_vertical_digit_stack_height(nd: usize, px: f32) -> i32 {
    if nd == 0 {
        return 0;
    }
    let font = coord_axis_font();
    let gap = (px * 0.22).ceil().max(1.0) as i32;
    let mut tot = 0i32;
    for _ in 0..nd {
        let (m, _) = font.rasterize('8', px);
        tot += m.height as i32 + gap;
    }
    tot - gap
}

/// Draw decimal `n` with digits stacked **top-to-bottom** (high-order digit at top).
/// Column is centered on `center_x` (tick position); narrow horizontal footprint for dense x-axis ticks.
fn coord_draw_u32_vertical_stack(
    img: &mut RgbImage,
    center_x: i32,
    top_y: i32,
    n: u32,
    fg: Rgb<u8>,
    px: f32,
) {
    let s = n.to_string();
    let font = coord_axis_font();
    let gap = (px * 0.22).ceil().max(1.0) as i32;
    let mut ty = top_y;
    for c in s.chars() {
        let (m, bmp) = font.rasterize(c, px);
        let top_left_x = center_x - m.width as i32 / 2;
        let top_left_y = ty;
        let baseline_x = top_left_x - m.xmin as i32;
        let baseline_y = top_left_y - m.ymin as i32;
        coord_blit_glyph_bold(img, baseline_x, baseline_y, &m, &bmp, fg);
        ty += m.height as i32 + gap;
    }
}

fn content_grid_step(min_side: u32) -> u32 {
    if min_side < 240 {
        25u32
    } else if min_side < 480 {
        50u32
    } else {
        COORD_GRID_DEFAULT_STEP
    }
}

/// Symmetric white margins (left = right, top = bottom) for ruler labels outside the capture.
/// `ruler_origin_*` is the **full-capture native** pixel index of the content’s top-left (0,0 for full screen; crop `x0,y0` for point crops) so label digit width fits large coordinates.
fn computer_use_margins(
    cw: u32,
    ch: u32,
    ruler_origin_x: u32,
    ruler_origin_y: u32,
) -> (u32, u32) {
    if cw < 2 || ch < 2 {
        return (0, 0);
    }
    let px = coord_label_px();
    let tick_len = 14i32;
    let pad = 12i32;
    let max_val_x = ruler_origin_x.saturating_add(cw.saturating_sub(1));
    let max_val_y = ruler_origin_y.saturating_add(ch.saturating_sub(1));
    let nd_x = (max_val_x.max(1).ilog10() as usize + 1).max(4);
    let nd_y = (max_val_y.max(1).ilog10() as usize + 1).max(4);
    let nd = nd_x.max(nd_y);
    let ml = (coord_digit_block_width(nd, px) + tick_len + pad).max(0) as u32;
    // Top/bottom: x-axis labels are vertical stacks — need height for `nd_x` digits.
    let x_stack_h = coord_vertical_digit_stack_height(nd_x, px);
    let mt = (x_stack_h + tick_len + pad).max(0) as u32;
    (ml, mt)
}

/// White border, grid lines on the capture only, numeric labels in the margin.
/// `ruler_origin_x/y`: **full-capture native** index of content pixel (0,0) — for a point crop, pass the crop’s `x0,y0` so tick labels match the same **whole-screen bitmap** space as a full-screen shot (not 0..crop_width only).
fn compose_computer_use_frame(
    content: RgbImage,
    ruler_origin_x: u32,
    ruler_origin_y: u32,
) -> (RgbImage, u32, u32) {
    let cw = content.width();
    let ch = content.height();
    if cw < 2 || ch < 2 {
        return (content, 0, 0);
    }
    let grid_step = content_grid_step(cw.min(ch));
    let (ml, mt) = computer_use_margins(cw, ch, ruler_origin_x, ruler_origin_y);
    let mr = ml;
    let mb = mt;
    let tw = ml + cw + mr;
    let th = mt + ch + mb;
    let label_px = coord_label_px();
    let tick_len = 14i32;
    let pad = 12i32;

    let mut out = RgbImage::new(tw, th);
    for p in out.pixels_mut() {
        *p = Rgb([255u8, 255, 255]);
    }
    for yy in 0..ch {
        for xx in 0..cw {
            out.put_pixel(ml + xx, mt + yy, *content.get_pixel(xx, yy));
        }
    }

    let grid = Rgb([52, 52, 68]);
    let grid_major = Rgb([95, 95, 118]);
    let tick = Rgb([180, 130, 40]);
    // Coordinate numerals in white margins — saturated red for visibility.
    let label = Rgb([200, 32, 40]);

    let cl = ml as i32;
    let ct = mt as i32;
    let cr = (ml + cw - 1) as i32;
    let cb = (mt + ch - 1) as i32;
    let wi = tw as i32;
    let hi = th as i32;

    let mut gx = grid_step as i32;
    while gx < cw as i32 {
        let major = (gx as u32) % COORD_GRID_MAJOR_STEP == 0;
        let thick = if major { 2 } else { 1 };
        let c = if major { grid_major } else { grid };
        for t in 0..thick {
            let px = cl + gx + t;
            if px >= cl && px <= cr {
                for py in ct..=cb {
                    coord_plot(&mut out, px, py, c);
                }
            }
        }
        gx += grid_step as i32;
    }

    let mut gy = grid_step as i32;
    while gy < ch as i32 {
        let major = (gy as u32) % COORD_GRID_MAJOR_STEP == 0;
        let thick = if major { 2 } else { 1 };
        let c = if major { grid_major } else { grid };
        for t in 0..thick {
            let py = ct + gy + t;
            if py >= ct && py <= cb {
                for px in cl..=cr {
                    coord_plot(&mut out, px, py, c);
                }
            }
        }
        gy += grid_step as i32;
    }

    let top_label_y = pad.max(2);
    for gxc in (0..cw as i32).step_by(grid_step as usize) {
        let tick_x = cl + gxc;
        for k in 0..tick_len.min(ct.max(1)) {
            coord_plot(&mut out, tick_x, ct - 1 - k, tick);
        }
        let val = ruler_origin_x.saturating_add(gxc.max(0) as u32);
        let col_w = coord_measure_str_width("8", label_px).max(1);
        let cx = tick_x.clamp(col_w / 2 + 2, wi - col_w / 2 - 2);
        coord_draw_u32_vertical_stack(&mut out, cx, top_label_y, val, label, label_px);
    }

    let bot_label_y = cb + tick_len + 4;
    for gxc in (0..cw as i32).step_by(grid_step as usize) {
        let tick_x = cl + gxc;
        for k in 0..tick_len {
            let y = cb + 1 + k;
            if y < hi {
                coord_plot(&mut out, tick_x, y, tick);
            }
        }
        let val = ruler_origin_x.saturating_add(gxc.max(0) as u32);
        let col_w = coord_measure_str_width("8", label_px).max(1);
        let cx = tick_x.clamp(col_w / 2 + 2, wi - col_w / 2 - 2);
        coord_draw_u32_vertical_stack(&mut out, cx, bot_label_y, val, label, label_px);
    }

    let left_numbers_x = pad.max(2);
    for gyc in (0..ch as i32).step_by(grid_step as usize) {
        let py = ct + gyc;
        for k in 0..tick_len.min(cl.max(1)) {
            coord_plot(&mut out, cl - 1 - k, py, tick);
        }
        let val = ruler_origin_y.saturating_add(gyc.max(0) as u32);
        let s = val.to_string();
        let dw = coord_measure_str_width(&s, label_px);
        let lx = left_numbers_x.min(cl - dw - 2).max(2);
        coord_draw_u32_h_centered(&mut out, lx, py, val, label, label_px);
    }

    let right_text_x = cr + tick_len + 4;
    for gyc in (0..ch as i32).step_by(grid_step as usize) {
        let py = ct + gyc;
        for k in 0..tick_len {
            let x = cr + 1 + k;
            if x < wi {
                coord_plot(&mut out, x, py, tick);
            }
        }
        let val = ruler_origin_y.saturating_add(gyc.max(0) as u32);
        let s = val.to_string();
        let dw = coord_measure_str_width(&s, label_px);
        let lx = right_text_x.min(wi - dw - 2).max(2);
        coord_draw_u32_h_centered(&mut out, lx, py, val, label, label_px);
    }

    (out, ml, mt)
}

/// JPEG quality for computer-use screenshots. Native display resolution is preserved (no downscale)
/// so `coordinate_mode` \"image\" pixel indices match the screen capture 1:1. Very large displays
/// increase request payload size; if the API rejects the image, lower quality or split workflows may be needed.
const JPEG_QUALITY: u8 = 75;

/// Half extent from the crop center in **native** capture pixels (target region up to 500×500).
const POINT_CROP_HALF_PX: u32 = 250;

#[inline]
fn clamp_center_to_native(cx: u32, cy: u32, nw: u32, nh: u32) -> (u32, u32) {
    if nw == 0 || nh == 0 {
        return (0, 0);
    }
    let cx = cx.min(nw - 1);
    let cy = cy.min(nh - 1);
    (cx, cy)
}

/// Top-left and size of the native crop rectangle around `(cx, cy)`, clamped to the bitmap (≤500×500 when the display is large enough).
fn crop_rect_around_point_native(cx: u32, cy: u32, nw: u32, nh: u32) -> (u32, u32, u32, u32) {
    let (cx, cy) = clamp_center_to_native(cx, cy, nw, nh);
    if nw == 0 || nh == 0 {
        return (0, 0, 1, 1);
    }
    let edge = POINT_CROP_HALF_PX.saturating_mul(2);
    let tw = edge.min(nw).max(1);
    let th = edge.min(nh).max(1);
    let mut x0 = cx.saturating_sub(POINT_CROP_HALF_PX);
    let mut y0 = cy.saturating_sub(POINT_CROP_HALF_PX);
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
    /// Composed JPEG size (includes white margin).
    image_w: u32,
    image_h: u32,
    /// Top-left of capture inside the JPEG.
    content_origin_x: u32,
    content_origin_y: u32,
    /// Native capture pixel size (the screen bitmap, no margin).
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

    /// Normalized 0..=1000 maps to the **capture** (same as pre-margin bitmap; independent of ruler padding).
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

pub struct DesktopComputerUseHost {
    last_pointer_map: Mutex<Option<PointerMap>>,
    /// When true, a fresh `screenshot_display` is required before `click` and before `key_chord` that sends Return/Enter
    /// (set after pointer moves / click; cleared after screenshot).
    click_needs_fresh_screenshot: Mutex<bool>,
    /// Last `screenshot_display` scope (full screen vs point crop) for tool hints and click rules.
    last_shot_refinement: Mutex<Option<ComputerUseScreenshotRefinement>>,
    /// Drill / crop context for the next `screenshot` (see [`ComputerUseNavFocus`]).
    navigation_focus: Mutex<Option<ComputerUseNavFocus>>,
}

impl std::fmt::Debug for DesktopComputerUseHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopComputerUseHost").finish_non_exhaustive()
    }
}

impl DesktopComputerUseHost {
    pub fn new() -> Self {
        Self {
            last_pointer_map: Mutex::new(None),
            click_needs_fresh_screenshot: Mutex::new(true),
            last_shot_refinement: Mutex::new(None),
            navigation_focus: Mutex::new(None),
        }
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
            "up" => Key::UpArrow,
            "down" => Key::DownArrow,
            "left" => Key::LeftArrow,
            "right" => Key::RightArrow,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
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
        let mut out = RgbImage::new(w, h);
        for yy in 0..h {
            for xx in 0..w {
                out.put_pixel(xx, yy, *src.get_pixel(x0 + xx, y0 + yy));
            }
        }
        Ok(out)
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

    fn screenshot_sync_tool(
        params: ComputerUseScreenshotParams,
        nav_in: Option<ComputerUseNavFocus>,
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

        let screen = Screen::from_point(0, 0)
            .map_err(|e| BitFunError::tool(format!("Screen capture init: {}", e)))?;
        let rgba = screen.capture().map_err(|e| {
            BitFunError::tool(format!(
                "Screenshot failed (on macOS grant Screen Recording for BitFun): {}",
                e
            ))
        })?;
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
            let (ccx, ccy) = clamp_center_to_native(center.x, center.y, native_w, native_h);
            let (x0, y0, tw, th) = crop_rect_around_point_native(center.x, center.y, native_w, native_h);
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
            let (x0, y0, _, _) =
                crop_rect_around_point_native(center.x, center.y, native_w, native_h);
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

        let jpeg_bytes = Self::encode_jpeg(&frame, JPEG_QUALITY)?;

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
            navigation_native_rect: shot_navigation_rect,
            quadrant_navigation_click_ready,
            image_content_rect: Some(image_content_rect),
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
        let guard = self
            .click_needs_fresh_screenshot
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        if *guard {
            return Err(BitFunError::tool(
                "Computer use refused: run action screenshot first. After the last pointer move or click you must capture a new screenshot before click or before key_chord that sends Return/Enter.".to_string(),
            ));
        }
        Ok(())
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

#[async_trait]
impl ComputerUseHost for DesktopComputerUseHost {
    async fn permission_snapshot(&self) -> BitFunResult<ComputerUsePermissionSnapshot> {
        Ok(tokio::task::spawn_blocking(Self::permission_sync)
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))?)
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
        let nav_snapshot = *self
            .navigation_focus
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;

        let (shot, map, nav_out) = tokio::task::spawn_blocking(move || {
            Self::screenshot_sync_tool(params, nav_snapshot)
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;

        *self
            .last_pointer_map
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))? = Some(map);

        *self
            .navigation_focus
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))? = nav_out;

        let refinement = Self::refinement_from_shot(&shot);
        *self
            .last_shot_refinement
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))? = Some(refinement);

        ComputerUseHost::computer_use_after_screenshot(self);

        Ok(shot)
    }

    async fn screenshot_peek_full_display(&self) -> BitFunResult<ComputerScreenshot> {
        let (shot, _map, _) = tokio::task::spawn_blocking(|| {
            Self::screenshot_sync_tool(ComputerUseScreenshotParams::default(), None)
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        Ok(shot)
    }

    fn last_screenshot_refinement(&self) -> Option<ComputerUseScreenshotRefinement> {
        self.last_shot_refinement
            .lock()
            .ok()
            .and_then(|g| *g)
    }

    fn map_image_coords_to_pointer_f64(&self, x: i32, y: i32) -> BitFunResult<(f64, f64)> {
        let guard = self
            .last_pointer_map
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        let Some(map) = *guard else {
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
        let guard = self
            .last_pointer_map
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        let Some(map) = *guard else {
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
        #[cfg(target_os = "macos")]
        {
            tokio::task::spawn_blocking(move || {
                Self::run_enigo_job(|_| Self::post_mouse_moved_cg_global(gx, gy))
            })
            .await
            .map_err(|e| BitFunError::tool(e.to_string()))??;
            ComputerUseHost::computer_use_after_pointer_mutation(self);
            return Ok(());
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.mouse_move(gx.round() as i32, gy.round() as i32).await
        }
    }

    async fn mouse_move(&self, x: i32, y: i32) -> BitFunResult<()> {
        tokio::task::spawn_blocking(move || {
            Self::run_enigo_job(|e| {
                e.move_mouse(x, y, Coordinate::Abs)
                    .map_err(|err| BitFunError::tool(format!("mouse_move: {}", err)))
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        ComputerUseHost::computer_use_after_pointer_mutation(self);
        Ok(())
    }

    async fn pointer_move_relative(&self, dx: i32, dy: i32) -> BitFunResult<()> {
        if dx == 0 && dy == 0 {
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            // enigo `Coordinate::Rel` uses `location()` on macOS, which mixes NSEvent + main-display
            // pixel height — not the same space as `CGEvent` / our screenshot mapping. Use Quartz
            // position + scale from the last capture (display points per screenshot pixel).
            let geo = {
                let guard = self
                    .last_pointer_map
                    .lock()
                    .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
                let Some(map) = *guard else {
                    return Err(BitFunError::tool(
                        "Run action screenshot first: on macOS, pointer_move_relative / pointer_nudge convert pixel deltas using the last capture scale."
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
        ComputerUseHost::computer_use_guard_click_allowed(self)?;
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
        Ok(())
    }

    async fn key_chord(&self, keys: Vec<String>) -> BitFunResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
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
                    for k in &mapped[..mapped.len() - 1] {
                        e.key(*k, Direction::Press)
                            .map_err(|err| BitFunError::tool(format!("key press: {}", err)))?;
                    }
                    let last = *mapped.last().unwrap();
                    e.key(last, Direction::Click)
                        .map_err(|err| BitFunError::tool(format!("key click: {}", err)))?;
                    for k in mapped[..mapped.len() - 1].iter().rev() {
                        e.key(*k, Direction::Release)
                            .map_err(|err| BitFunError::tool(format!("key release: {}", err)))?;
                    }
                }
                #[cfg(target_os = "macos")]
                if chord_has_modifier {
                    std::thread::sleep(std::time::Duration::from_millis(95));
                }
                Ok(())
            })
        })
        .await
        .map_err(|e| BitFunError::tool(e.to_string()))??;
        ComputerUseHost::computer_use_after_pointer_mutation(self);
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
        ComputerUseHost::computer_use_after_pointer_mutation(self);
        Ok(())
    }

    async fn wait_ms(&self, ms: u64) -> BitFunResult<()> {
        tokio::time::sleep(Duration::from_millis(ms.max(1))).await;
        Ok(())
    }

    fn computer_use_after_screenshot(&self) {
        if let Ok(mut g) = self.click_needs_fresh_screenshot.lock() {
            *g = false;
        }
    }

    fn computer_use_after_pointer_mutation(&self) {
        if let Ok(mut g) = self.click_needs_fresh_screenshot.lock() {
            *g = true;
        }
    }

    fn computer_use_after_click(&self) {
        if let Ok(mut g) = self.click_needs_fresh_screenshot.lock() {
            *g = true;
        }
    }

    fn computer_use_guard_click_allowed(&self) -> BitFunResult<()> {
        self.computer_use_guard_verified_ui()?;
        let refine = self
            .last_shot_refinement
            .lock()
            .map_err(|e| BitFunError::tool(format!("lock: {}", e)))?;
        match *refine {
            Some(ComputerUseScreenshotRefinement::RegionAroundPoint { .. }) => {}
            Some(ComputerUseScreenshotRefinement::QuadrantNavigation {
                click_ready: true,
                ..
            }) => {}
            _ => {
                return Err(BitFunError::tool(
                    "Click refused: use a **fine** screenshot basis — either a **~500×500 point crop** (`screenshot_crop_center_x` / `y` in full-display native pixels) **or** keep drilling with `screenshot_navigate_quadrant` until `quadrant_navigation_click_ready` is true in the tool result, then `mouse_move` + `click`. Full-screen alone is not enough.".to_string(),
                ));
            }
        }
        Ok(())
    }
}
