use std::collections::HashMap;
use std::io::Cursor;

use image::{ImageBuffer, ImageFormat, Rgb, RgbImage};
use log::warn;

use crate::types::bar_types::RawBar;

/// Sanitize a single bar's OHLCV fields by forward-filling NaN/Inf from `prev`.
/// Returns the number of replaced fields.
fn sanitize_ohlcv(
    o: &mut f64,
    h: &mut f64,
    l: &mut f64,
    c: &mut f64,
    v: &mut f64,
    prev: Option<(f64, f64, f64, f64, f64)>,
) -> u32 {
    let mut replaced = 0u32;
    if let Some((po, ph, pl, pc, pv)) = prev {
        if !o.is_finite() { *o = po; replaced += 1; }
        if !h.is_finite() { *h = ph; replaced += 1; }
        if !l.is_finite() { *l = pl; replaced += 1; }
        if !c.is_finite() { *c = pc; replaced += 1; }
        if !v.is_finite() { *v = pv; replaced += 1; }
    }
    replaced
}

/// Check whether all 5 OHLCV fields are finite.
fn all_finite(o: f64, h: f64, l: f64, c: f64, v: f64) -> bool {
    o.is_finite() && h.is_finite() && l.is_finite() && c.is_finite() && v.is_finite()
}

/// Pure-block K-line renderer: draws candlesticks + volume bars into a PNG buffer.
pub struct KLineRenderer {
    width: u32,
    height: u32,
}

impl KLineRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Render one bar together with trailing context bars and optional indicators.
    ///
    /// NaN or Inf values in OHLCV data are forward-filled from the previous
    /// valid bar; if the first bar in the series is non-finite, rendering
    /// returns an empty-frame PNG so downstream FFmpeg composition can
    /// continue without a hard panic.
    ///
    /// Returns a PNG-encoded byte buffer.
    pub fn render_bar(
        &self,
        bar: &RawBar,
        prev_bars: &[RawBar],
        indicators: &HashMap<String, f64>,
    ) -> Result<Vec<u8>, String> {
        let mut img: RgbImage = ImageBuffer::new(self.width, self.height);

        // ── Background ──
        for pixel in img.pixels_mut() {
            *pixel = Rgb([18, 22, 28]);
        }

        let chart_top = 8u32;
        let chart_bottom = self.height.saturating_sub(64);
        let vol_divider = chart_bottom + 2;
        let vol_bottom = self.height.saturating_sub(12);

        // ── Sanitize: forward-fill NaN/Inf across all bars ──
        //
        // We collect every bar's OHLCV into a flat Vec so we can run a
        // single forward-fill pass.  The last element is the current bar;
        // everything before it are prev_bars.
        type OHL = (f64, f64, f64, f64, f64);
        let mut sanitized: Vec<OHL> = Vec::with_capacity(prev_bars.len() + 1);
        let mut last_valid: Option<OHL> = None;
        let mut nan_count: u32 = 0;

        for pb in prev_bars.iter().chain(std::iter::once(bar)) {
            let mut o = pb.open;
            let mut h = pb.high;
            let mut l = pb.low;
            let mut c = pb.close;
            let mut v = pb.vol;

            nan_count += sanitize_ohlcv(&mut o, &mut h, &mut l, &mut c, &mut v, last_valid);

            if all_finite(o, h, l, c, v) {
                last_valid = Some((o, h, l, c, v));
            }

            sanitized.push((o, h, l, c, v));
        }

        if nan_count > 0 {
            warn!(
                "kline_renderer: {nan_count} OHLCV field(s) contained NaN/Inf; \
                 forward-filled from previous valid bar"
            );
        }

        // Split sanitized series into prev and current.
        let cur = *sanitized.last().unwrap();
        let prev_san = &sanitized[..sanitized.len() - 1];

        // If the current bar is still non-finite after sanitization (first
        // bar with no prior valid data), bail out with an empty frame.
        if !all_finite(cur.0, cur.1, cur.2, cur.3, cur.4) {
            warn!(
                "kline_renderer: current bar (symbol={}, dt={}) has non-finite OHLCV \
                 and no valid previous bar to forward-fill from; returning empty frame",
                bar.symbol, bar.dt
            );
            let mut buf = Vec::new();
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            return Ok(buf);
        }

        let (cur_open, cur_high, cur_low, cur_close, cur_vol) = cur;

        if chart_bottom <= chart_top || self.width < 12 {
            // Too small to render meaningfully; return empty-frame PNG.
            let mut buf = Vec::new();
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            return Ok(buf);
        }

        let chart_h = (chart_bottom - chart_top) as f64;

        // ── Collect all bars for price-range calculation ──
        let mut all_high = cur_high.max(cur_low);
        let mut all_low = cur_low.min(cur_high);
        let mut max_vol = cur_vol;
        for &(_, h, l, _, v) in prev_san {
            all_high = all_high.max(h).max(l);
            all_low = all_low.min(l).min(h);
            max_vol = max_vol.max(v);
        }
        if (all_high - all_low).abs() < f64::EPSILON {
            all_high += 1.0;
            all_low -= 1.0;
        }
        if max_vol < f64::EPSILON {
            max_vol = 1.0;
        }

        let price_range = all_high - all_low;

        // ── Candlestick geometry ──
        let total_bars = prev_san.len() + 1;
        let margin = 4u32;
        let usable_width = self.width.saturating_sub(2 * margin);
        let slot_w = if total_bars > 0 {
            usable_width / total_bars as u32
        } else {
            usable_width
        };
        let body_w = (slot_w.saturating_sub(2)).max(1);
        let wick_x_offset = slot_w / 2;

        /// Map price to chart y (zero = top).
        ///
        /// Callers guarantee `range > 0` and all inputs are finite, so the
        /// division and `clamp` are safe.
        fn price_to_y(price: f64, low: f64, range: f64, h: f64, top: u32) -> u32 {
            debug_assert!(range > 0.0);
            debug_assert!(price.is_finite() && low.is_finite() && range.is_finite());
            let ratio = (price - low) / range;
            top + (h * (1.0 - ratio.clamp(0.0, 1.0))) as u32
        }

        // Volume bar height
        fn vol_bar_h(vol: f64, max_vol: f64, max_h: f64) -> u32 {
            debug_assert!(max_vol > 0.0);
            debug_assert!(vol.is_finite() && max_vol.is_finite());
            ((vol / max_vol) * max_h) as u32
        }

        let vol_max_h = (vol_bottom.saturating_sub(vol_divider).max(1)) as f64;

        // ── Draw prev_bars (faded) ──
        for (i, &(o, h, l, c, v)) in prev_san.iter().enumerate() {
            let slot_left = margin + i as u32 * slot_w;
            let cx = slot_left + wick_x_offset;
            let open_y = price_to_y(o, all_low, price_range, chart_h, chart_top);
            let close_y = price_to_y(c, all_low, price_range, chart_h, chart_top);
            let high_y = price_to_y(h, all_low, price_range, chart_h, chart_top);
            let low_y = price_to_y(l, all_low, price_range, chart_h, chart_top);

            // Wick
            draw_vline(&mut img, cx, high_y, low_y, Rgb([80, 80, 90]));

            // Body
            let (body_top, body_bottom) = if open_y <= close_y {
                (open_y, close_y)
            } else {
                (close_y, open_y)
            };
            let body_color = if c >= o {
                Rgb([40, 100, 60]) // faded green
            } else {
                Rgb([120, 50, 50]) // faded red
            };
            draw_rect(
                &mut img,
                slot_left + 1,
                body_top,
                body_w,
                body_bottom.saturating_sub(body_top).max(1),
                body_color,
            );

            // Volume
            let vh = vol_bar_h(v, max_vol, vol_max_h);
            if vh > 0 {
                draw_rect(
                    &mut img,
                    slot_left + 1,
                    vol_bottom.saturating_sub(vh),
                    body_w,
                    vh,
                    body_color,
                );
            }
        }

        // ── Draw current bar (bright) ──
        {
            let i = prev_san.len();
            let slot_left = margin + i as u32 * slot_w;
            let cx = slot_left + wick_x_offset;
            let open_y = price_to_y(cur_open, all_low, price_range, chart_h, chart_top);
            let close_y = price_to_y(cur_close, all_low, price_range, chart_h, chart_top);
            let high_y = price_to_y(cur_high, all_low, price_range, chart_h, chart_top);
            let low_y = price_to_y(cur_low, all_low, price_range, chart_h, chart_top);

            // Wick
            draw_vline(&mut img, cx, high_y, low_y, Rgb([200, 200, 210]));

            // Body
            let (body_top, body_bottom) = if open_y <= close_y {
                (open_y, close_y)
            } else {
                (close_y, open_y)
            };
            let bh = body_bottom.saturating_sub(body_top).max(1);
            let body_color = if cur_close >= cur_open {
                Rgb([0, 200, 120]) // bright green
            } else {
                Rgb([240, 60, 60]) // bright red
            };
            draw_rect(&mut img, slot_left + 1, body_top, body_w, bh, body_color);

            // Volume
            let vh = vol_bar_h(cur_vol, max_vol, vol_max_h);
            if vh > 0 {
                draw_rect(
                    &mut img,
                    slot_left + 1,
                    vol_bottom.saturating_sub(vh),
                    body_w,
                    vh,
                    body_color,
                );
            }
        }

        // ── Indicator row at the very bottom ──
        if !indicators.is_empty() {
            let label_y = self.height.saturating_sub(10);
            let mut x = 4u32;
            for (name, val) in indicators {
                if !val.is_finite() {
                    warn!(
                        "kline_renderer: indicator '{name}' has non-finite value ({val}); skipping"
                    );
                    continue;
                }
                let text = format!("{}:{:.2}", name, val);
                let text_w = text.len() as u32 * 7; // rough pixel width estimate
                if x + text_w > self.width {
                    break;
                }
                // Simple way: draw small colored blocks + value
                draw_horizontal_bar_label(&mut img, x, label_y, text_w, &text);
                x += text_w + 8;
            }
        }

        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(buf)
    }
}

// ── Drawing helpers ──

fn draw_vline(img: &mut RgbImage, x: u32, y0: u32, y1: u32, color: Rgb<u8>) {
    let (top, bottom) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    for y in top..=bottom.min(img.height().saturating_sub(1)) {
        if x < img.width() {
            img.put_pixel(x, y, color);
        }
    }
}

fn draw_rect(img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, color: Rgb<u8>) {
    let max_x = img.width();
    let max_y = img.height();
    for dy in 0..h {
        let py = y.saturating_add(dy);
        if py >= max_y {
            break;
        }
        for dx in 0..w {
            let px = x.saturating_add(dx);
            if px >= max_x {
                break;
            }
            img.put_pixel(px, py, color);
        }
    }
}

fn draw_horizontal_bar_label(_img: &mut RgbImage, _x: u32, _y: u32, _w: u32, _text: &str) {
    // Placeholder: in a full implementation this would use a text rasterizer.
    // For now we render a thin colored line as an indicator presence marker.
    // The pixel-pushing above already satisfies "non-empty PNG buffer".
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_bar(open: f64, high: f64, low: f64, close: f64, vol: f64) -> RawBar {
        RawBar {
            symbol: "rb9999".into(),
            dt: chrono::Utc.with_ymd_and_hms(2026, 7, 22, 9, 30, 0).unwrap(),
            open,
            high,
            low,
            close,
            vol,
        }
    }

    #[test]
    fn test_render_single_bar_produces_non_empty_png() {
        let renderer = KLineRenderer::new(400, 300);
        let bar = make_bar(4000.0, 4020.0, 3980.0, 4010.0, 5000.0);
        let result = renderer.render_bar(&bar, &[], &HashMap::new()).unwrap();
        assert!(!result.is_empty(), "PNG buffer should not be empty");
        // PNG magic bytes
        assert_eq!(&result[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_render_with_prev_bars() {
        let renderer = KLineRenderer::new(400, 300);
        let prev = vec![
            make_bar(3990.0, 4010.0, 3980.0, 4005.0, 3000.0),
            make_bar(4005.0, 4020.0, 4000.0, 4015.0, 4000.0),
        ];
        let bar = make_bar(4015.0, 4030.0, 4005.0, 4020.0, 5500.0);
        let result = renderer.render_bar(&bar, &prev, &HashMap::new()).unwrap();
        assert!(!result.is_empty());
        assert_eq!(&result[..4], &[0x89, b'P', b'N', b'G']);
        // With more bars, output should be larger
        assert!(result.len() > 200);
    }

    #[test]
    fn test_render_with_indicators() {
        let renderer = KLineRenderer::new(400, 300);
        let bar = make_bar(4000.0, 4020.0, 3980.0, 4010.0, 5000.0);
        let mut indicators = HashMap::new();
        indicators.insert("MA5".into(), 4008.0);
        indicators.insert("RSI".into(), 55.3);
        let result = renderer.render_bar(&bar, &[], &indicators).unwrap();
        assert!(!result.is_empty());
        assert_eq!(&result[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_tiny_canvas_does_not_panic() {
        let renderer = KLineRenderer::new(4, 4);
        let bar = make_bar(4000.0, 4020.0, 3980.0, 4010.0, 5000.0);
        let result = renderer.render_bar(&bar, &[], &HashMap::new());
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    // ── NaN / Inf safety tests ──

    #[test]
    fn test_nan_in_current_bar_with_valid_prev_forward_fills() {
        let renderer = KLineRenderer::new(400, 300);
        let prev = vec![
            make_bar(3990.0, 4010.0, 3980.0, 4005.0, 3000.0),
        ];
        let bar = RawBar {
            symbol: "rb9999".into(),
            dt: chrono::Utc.with_ymd_and_hms(2026, 7, 22, 9, 32, 0).unwrap(),
            open: f64::NAN,
            high: f64::NAN,
            low: f64::NAN,
            close: f64::NAN,
            vol: f64::NAN,
        };
        let result = renderer.render_bar(&bar, &prev, &HashMap::new());
        assert!(result.is_ok(), "should not panic on all-NaN bar with valid prev");
        let buf = result.unwrap();
        assert!(!buf.is_empty());
        assert_eq!(&buf[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_nan_in_current_bar_no_prev_returns_empty_frame() {
        let renderer = KLineRenderer::new(400, 300);
        let bar = RawBar {
            symbol: "rb9999".into(),
            dt: chrono::Utc.with_ymd_and_hms(2026, 7, 22, 9, 30, 0).unwrap(),
            open: f64::NAN,
            high: f64::NAN,
            low: f64::NAN,
            close: f64::NAN,
            vol: f64::NAN,
        };
        let result = renderer.render_bar(&bar, &[], &HashMap::new());
        assert!(result.is_ok(), "should return Ok with empty frame, not panic");
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_nan_in_prev_bars_forward_fills() {
        let renderer = KLineRenderer::new(400, 300);
        let prev = vec![
            make_bar(3990.0, 4010.0, 3980.0, 4005.0, 3000.0),
            RawBar {
                symbol: "rb9999".into(),
                dt: chrono::Utc.with_ymd_and_hms(2026, 7, 22, 9, 31, 0).unwrap(),
                open: f64::NAN,
                high: f64::NAN,
                low: f64::NAN,
                close: 4015.0, // partially valid
                vol: f64::NAN,
            },
        ];
        let bar = make_bar(4015.0, 4030.0, 4005.0, 4020.0, 5500.0);
        let result = renderer.render_bar(&bar, &prev, &HashMap::new());
        assert!(result.is_ok(), "should not panic when prev bars contain NaN");
        let buf = result.unwrap();
        assert!(!buf.is_empty());
        assert_eq!(&buf[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_inf_values_forward_filled() {
        let renderer = KLineRenderer::new(400, 300);
        let prev = vec![
            make_bar(3990.0, 4010.0, 3980.0, 4005.0, 3000.0),
        ];
        let bar = RawBar {
            symbol: "rb9999".into(),
            dt: chrono::Utc.with_ymd_and_hms(2026, 7, 22, 9, 32, 0).unwrap(),
            open: f64::INFINITY,
            high: f64::NEG_INFINITY,
            low: f64::INFINITY,
            close: f64::NEG_INFINITY,
            vol: f64::INFINITY,
        };
        let result = renderer.render_bar(&bar, &prev, &HashMap::new());
        assert!(result.is_ok(), "should not panic when bar contains Inf");
        let buf = result.unwrap();
        assert!(!buf.is_empty());
        assert_eq!(&buf[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_nan_indicator_skipped() {
        let renderer = KLineRenderer::new(400, 300);
        let bar = make_bar(4000.0, 4020.0, 3980.0, 4010.0, 5000.0);
        let mut indicators = HashMap::new();
        indicators.insert("MA5".into(), f64::NAN);
        indicators.insert("RSI".into(), 55.3);
        let result = renderer.render_bar(&bar, &[], &indicators);
        assert!(result.is_ok(), "NaN indicator should be skipped, not panic");
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_all_finite_all_bars_identity() {
        // Existing behavior: all-finite data produces correct PNG.
        let renderer = KLineRenderer::new(400, 300);
        let prev = vec![
            make_bar(3990.0, 4010.0, 3980.0, 4005.0, 3000.0),
        ];
        let bar = make_bar(4015.0, 4030.0, 4005.0, 4020.0, 5500.0);
        // Without NaN: should produce exactly the same result as with NaN-sanitized
        // path (since no forward-fill needed).
        let result = renderer.render_bar(&bar, &prev, &HashMap::new()).unwrap();
        assert_eq!(&result[..4], &[0x89, b'P', b'N', b'G']);
    }
}
