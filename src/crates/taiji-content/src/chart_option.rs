//! ECharts option template rendering.
//!
//! ## Template rendering approach
//!
//! This module uses **manual `String::replace`** for template variable substitution
//! (step 8 in `build_echarts_option`). The template is a JSON file with
//! `{{variable}}` placeholders; each placeholder is replaced by a pre-serialized
//! JSON value or string literal.
//!
//! ### Cross-reference: `taiji-blog-gen` uses Tera
//!
//! [`taiji-blog-gen`](../taiji-blog-gen/src/main.rs) renders Hugo Markdown templates
//! via the **Tera** engine (`tera::Tera`), which provides typed context insertion,
//! conditionals, loops, and filters. That is a richer, more maintainable approach
//! for template-heavy workflows.
//!
//! ### Recommended unification
//!
//! The two crates currently use **different template rendering strategies** for
//! similar `{{variable}}` substitution tasks:
//!
//! | Crate | Engine | Pros | Cons |
//! |---|---|---|---|
//! | `taiji-content` (`chart_option.rs`) | Manual `String::replace` | Zero deps, fast compile | No conditionals, loops, or filters; fragile to template syntax changes |
//! | `taiji-blog-gen` (`main.rs`) | Tera v1 | Full template logic, typed context | Adds a dependency |
//!
//! **Recommendation**: Eventually migrate `chart_option.rs` to Tera (or share a
//! common `taiji-templates` helper crate) so all taiji crates use one rendering
//! engine. The `String::replace` approach is adequate for the current
//! simple-substitution ECharts template but will not scale if templates grow
//! conditional blocks or partials.
//!
//! For now, be aware when adding template features: **prefer extending Tera in
//! `taiji-blog-gen` as the reference pattern** rather than adding more
//! `String::replace` variants here.

use crate::types::render_config::VideoRenderConfig;
use serde_json::Value;

/// Build ECharts option JSON from pipeline export JSON + VideoRenderConfig.
///
/// `pipeline_export` is the output of the `taiji_export` Tauri command.
/// Extracts the candlestick array from the `bars:{freq}` key and injects it
/// into the template's `{{variable}}` placeholders.
///
/// Default color scheme follows Chinese futures market convention:
/// red for up (up_color=#ef5350), green for down (down_color=#26a69a).
pub fn build_echarts_option(
    pipeline_export: &Value,
    config: &VideoRenderConfig,
) -> Result<Value, String> {
    // 1. Find bars:* key
    let bars_obj = find_bars_key(pipeline_export)?;

    // 2. Extract candlestick array
    let bars = bars_obj
        .as_array()
        .ok_or_else(|| "bars value is not an array".to_string())?;

    // 3. Build x_labels
    let x_labels: Vec<String> = bars
        .iter()
        .map(|bar| {
            bar.get("dt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default()
        })
        .collect();

    // 4. Build candlestick_data: ECharts format [[open, close, low, high], ...]
    let candlestick_data: Vec<Vec<f64>> = bars
        .iter()
        .map(|bar| {
            let open = get_f64(bar, "open");
            let close = get_f64(bar, "close");
            let low = get_f64(bar, "low");
            let high = get_f64(bar, "high");
            vec![open, close, low, high]
        })
        .collect();

    // 5. Build volume_data
    let volume_data: Vec<f64> = bars.iter().map(|bar| get_f64(bar, "vol")).collect();

    // 6. Read template file
    //
    // TODO(taiji): Migrate to bitfun_services FileSystemService for path canonicalization
    // and file reads. Raw std::fs::canonicalize / std::fs::read_to_string should be
    // replaced with the platform-agnostic FileSystemService abstraction provided by
    // the BitFun services layer (src/crates/services).
    let template_path = std::fs::canonicalize(&config.kline_echarts_template).map_err(|e| {
        format!(
            "Failed to resolve template path {}: {}",
            config.kline_echarts_template.display(),
            e
        )
    })?;
    let template_str = std::fs::read_to_string(&template_path).map_err(|e| {
        format!(
            "Failed to read template file {}: {}",
            template_path.display(),
            e
        )
    })?;

    // 7. Serialize injection variables as JSON strings
    let x_labels_json = serde_json::to_string(&x_labels)
        .map_err(|e| format!("Failed to serialize x_labels: {}", e))?;
    let candlestick_json = serde_json::to_string(&candlestick_data)
        .map_err(|e| format!("Failed to serialize candlestick_data: {}", e))?;
    let volume_json = serde_json::to_string(&volume_data)
        .map_err(|e| format!("Failed to serialize volume_data: {}", e))?;

    // 8. Replace template placeholders
    let rendered = template_str
        .replace(
            "\"{{bg_color}}\"",
            &serde_json::to_string(&config.bg_color)
                .unwrap_or_else(|_| format!("\"{}\"", config.bg_color)),
        )
        .replace("\"{{up_color}}\"", "\"#ef5350\"")
        .replace("\"{{down_color}}\"", "\"#26a69a\"")
        .replace("\"{{vol_color}}\"", "\"rgba(100,180,255,0.6)\"")
        .replace("{{x_labels}}", &x_labels_json)
        .replace("{{candlestick_data}}", &candlestick_json)
        .replace("{{volume_data}}", &volume_json);

    // 9. Parse as JSON
    let option: Value = serde_json::from_str(&rendered)
        .map_err(|e| format!("Failed to parse rendered ECharts option JSON: {}", e))?;

    Ok(option)
}

/// Find the value whose key starts with "bars:" in pipeline_export JSON.
fn find_bars_key<'a>(pipeline_export: &'a Value) -> Result<&'a Value, String> {
    let obj = pipeline_export
        .as_object()
        .ok_or_else(|| "pipeline_export is not a JSON object".to_string())?;

    for (key, value) in obj {
        if key.starts_with("bars:") {
            return Ok(value);
        }
    }

    Err("No 'bars:*' key found in pipeline_export".to_string())
}

/// Extract an f64 field from a JSON object, returning 0.0 when missing.
fn get_f64(obj: &Value, field: &str) -> f64 {
    obj.get(field).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::render_config::VideoRenderConfig;
    use serde_json::json;
    use std::path::PathBuf;

    fn test_config(template_path: PathBuf) -> VideoRenderConfig {
        VideoRenderConfig {
            resolution: (1920, 1080),
            fps: 30,
            bg_color: "#0a0e27".into(),
            brand_watermark: None,
            kline_echarts_template: template_path,
            annotation_mapping: PathBuf::from(
                "scripts/video-render-template/annotation_mapping.json",
            ),
        }
    }

    /// Write template content into a temp directory, returning (path, temp dir guard).
    ///
    /// TODO(taiji): Migrate temp file writes to bitfun_services FileSystemService
    /// when test infrastructure supports it.
    fn write_temp_template(content: &str) -> (PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("kline_echarts_option.json");
        std::fs::write(&path, content).expect("Failed to write temp template");
        (path, dir)
    }

    // ── Integration tests: full flow ──

    #[test]
    fn test_build_echarts_option_full_flow() {
        let template_content = r#"{
  "backgroundColor": "{{bg_color}}",
  "animation": false,
  "grid": [
    { "left": "5%", "right": "5%", "top": "5%", "height": "55%" },
    { "left": "5%", "right": "5%", "top": "65%", "height": "15%" }
  ],
  "xAxis": [
    { "type": "category", "data": {{x_labels}}, "gridIndex": 0, "axisLabel": { "show": true } },
    { "type": "category", "data": {{x_labels}}, "gridIndex": 1, "axisLabel": { "show": false } }
  ],
  "yAxis": [
    { "type": "value", "gridIndex": 0, "scale": true },
    { "type": "value", "gridIndex": 1, "scale": true }
  ],
  "series": [
    {
      "name": "Candlestick",
      "type": "candlestick",
      "xAxisIndex": 0, "yAxisIndex": 0,
      "data": {{candlestick_data}},
      "itemStyle": {
        "color": "{{up_color}}",
        "color0": "{{down_color}}",
        "borderColor": "{{up_color}}",
        "borderColor0": "{{down_color}}"
      }
    },
    {
      "name": "Volume",
      "type": "bar",
      "xAxisIndex": 1, "yAxisIndex": 1,
      "data": {{volume_data}},
      "itemStyle": { "color": "{{vol_color}}" }
    }
  ]
}"#;

        let (template_path, _temp_dir) = write_temp_template(template_content);
        let config = test_config(template_path);

        // Simulate taiji_export JSON — consistent with Pipeline::serialize_state output
        let pipeline_export = json!({
            "_meta": {
                "instrument": "ag2506",
                "timestamp": "2026-07-21T09:35:00+00:00",
                "freq": "5m"
            },
            "bars:5m": [
                {
                    "symbol": "ag2506",
                    "dt": "2026-07-21T09:30:00+00:00",
                    "freq": "5m",
                    "open": 4500.0,
                    "high": 4520.0,
                    "low": 4495.0,
                    "close": 4510.0,
                    "vol": 15000.0,
                    "amount": 6.75e7,
                    "open_interest": null,
                    "delta": null
                },
                {
                    "symbol": "ag2506",
                    "dt": "2026-07-21T09:35:00+00:00",
                    "freq": "5m",
                    "open": 4510.0,
                    "high": 4530.0,
                    "low": 4505.0,
                    "close": 4520.0,
                    "vol": 18000.0,
                    "amount": 8.13e7,
                    "open_interest": 50000.0,
                    "delta": 1200.0
                }
            ]
        });

        let result = build_echarts_option(&pipeline_export, &config)
            .expect("build_echarts_option should succeed");

        // Verify structure
        let series = result["series"].as_array().expect("series should be array");
        assert_eq!(
            series.len(),
            2,
            "should have 2 series (candlestick + volume)"
        );

        // Verify candlestick series
        let kline_series = &series[0];
        assert_eq!(kline_series["type"], "candlestick");
        assert_eq!(kline_series["name"], "Candlestick");
        let data = kline_series["data"]
            .as_array()
            .expect("candlestick data should be array");
        assert_eq!(data.len(), 2);

        // Verify first bar: [open, close, low, high]
        let bar0 = data[0].as_array().expect("bar0 should be array");
        assert_eq!(bar0[0], json!(4500.0)); // open
        assert_eq!(bar0[1], json!(4510.0)); // close
        assert_eq!(bar0[2], json!(4495.0)); // low
        assert_eq!(bar0[3], json!(4520.0)); // high

        // Verify volume series
        let vol_series = &series[1];
        assert_eq!(vol_series["type"], "bar");
        let vol_data = vol_series["data"]
            .as_array()
            .expect("volume data should be array");
        assert_eq!(vol_data.len(), 2);
        assert_eq!(vol_data[0], json!(15000.0));
        assert_eq!(vol_data[1], json!(18000.0));

        // Verify xAxis
        let x_axes = result["xAxis"].as_array().expect("xAxis should be array");
        let x0_data = x_axes[0]["data"]
            .as_array()
            .expect("xAxis[0].data should be array");
        assert_eq!(x0_data.len(), 2);
        assert_eq!(x0_data[0], "2026-07-21T09:30:00+00:00");

        // Verify background color
        assert_eq!(result["backgroundColor"], "#0a0e27");

        // Verify grid
        let grids = result["grid"].as_array().expect("grid should be array");
        assert_eq!(grids.len(), 2);
    }

    // ── Unit tests: find_bars_key ──

    #[test]
    fn test_find_bars_key_found() {
        let export = json!({
            "bars:5m": [{"open": 100.0, "close": 101.0, "high": 102.0, "low": 99.0, "vol": 500.0, "dt": "2026-07-21T09:30:00+00:00"}],
            "pivots:5m": []
        });
        let bars = find_bars_key(&export).expect("should find bars:5m");
        assert!(bars.is_array());
    }

    #[test]
    fn test_find_bars_key_not_found() {
        let export = json!({
            "pivots:5m": [],
            "trendlines:5m": []
        });
        let err = find_bars_key(&export).unwrap_err();
        assert!(err.contains("No 'bars:*' key"));
    }

    #[test]
    fn test_find_bars_key_not_object() {
        let export = json!("not an object");
        let err = find_bars_key(&export).unwrap_err();
        assert!(err.contains("not a JSON object"));
    }

    // ── Unit tests: get_f64 ──

    #[test]
    fn test_get_f64_present() {
        let obj = json!({"open": 100.5});
        assert!((get_f64(&obj, "open") - 100.5).abs() < 0.001);
    }

    #[test]
    fn test_get_f64_missing() {
        let obj = json!({"open": 100.5});
        assert_eq!(get_f64(&obj, "close"), 0.0);
    }

    // ── Edge case: empty bars ──

    #[test]
    fn test_empty_bars_produces_empty_arrays() {
        let template_content = r#"{
  "xAxis": [{"data": {{x_labels}}}],
  "yAxis": [{"type": "value"}],
  "series": [
    {"type": "candlestick", "data": {{candlestick_data}}},
    {"type": "bar", "data": {{volume_data}}}
  ]
}"#;

        let (template_path, _temp_dir) = write_temp_template(template_content);
        let config = test_config(template_path);

        let export = json!({
            "_meta": { "instrument": "ag2506" },
            "bars:5m": []
        });

        let result =
            build_echarts_option(&export, &config).expect("should succeed with empty bars");
        let x_data = result["xAxis"][0]["data"]
            .as_array()
            .expect("x data should be array");
        assert!(x_data.is_empty());

        let kline_data = result["series"][0]["data"]
            .as_array()
            .expect("kline data should be array");
        assert!(kline_data.is_empty());

        let vol_data = result["series"][1]["data"]
            .as_array()
            .expect("volume data should be array");
        assert!(vol_data.is_empty());
    }

    // ── Error test: missing template file ──

    #[test]
    fn test_missing_template_file() {
        let config = test_config(PathBuf::from("nonexistent/template.json"));
        let export = json!({"bars:5m": []});
        let err = build_echarts_option(&export, &config).unwrap_err();
        assert!(err.contains("Failed to resolve template path"));
    }

    // ── Error test: bars value is not an array ──

    #[test]
    fn test_bars_not_array() {
        let template_content = r#"{"series": [{"data": {{candlestick_data}}}]}"#;
        let (template_path, _temp_dir) = write_temp_template(template_content);
        let config = test_config(template_path);

        let export = json!({"bars:5m": "not an array"});
        let err = build_echarts_option(&export, &config).unwrap_err();
        assert!(err.contains("not an array"));
    }
}
