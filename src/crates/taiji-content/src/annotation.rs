use serde_json::{json, Value};

/// Inject Taiji annotation data into an ECharts option, returning an option with
/// markPoint/markLine/markArea.
///
/// `pipeline_export` is expected to contain frequency-prefixed keys (e.g. `bars:5m`,
/// `pivots:5m`), consistent with the Phase 3 `taiji_export` output format.
/// `mapping` is the content of annotation_mapping.json.
pub fn apply_annotations(
    echarts_option: &mut Value,
    pipeline_export: &Value,
    mapping: &Value,
) -> Result<(), String> {
    let bar_count = find_key_with_freq(pipeline_export, "bars:")
        .and_then(|b| b.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    // Ensure series[0] exists
    let series0 = echarts_option
        .get_mut("series")
        .and_then(|s| s.as_array_mut())
        .and_then(|a| a.first_mut())
        .ok_or_else(|| "echarts_option.series[0] is missing".to_string())?;

    apply_pivots(series0, pipeline_export, mapping);
    apply_trendlines(series0, pipeline_export, mapping, bar_count);
    apply_magnets(series0, pipeline_export, mapping);
    apply_triple_push(series0, pipeline_export, mapping);
    apply_vol_channel(echarts_option, pipeline_export, mapping, bar_count);

    Ok(())
}

/// Find the key with a frequency prefix (e.g. "pivots:5m").
/// Reuses the find_bars_key pattern from chart_option.rs.
fn find_key_with_freq<'a>(export: &'a Value, prefix: &str) -> Option<&'a Value> {
    export
        .as_object()?
        .iter()
        .find(|(k, _)| k.starts_with(prefix))
        .map(|(_, v)| v)
}

// ── 1. Pivot → markPoint ──────────────────────────────────────────────

fn apply_pivots(series0: &mut Value, pipeline_export: &Value, mapping: &Value) {
    let cfg = &mapping["pivot"];
    let pivots = match find_key_with_freq(pipeline_export, "pivots:").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return,
    };

    let max_count = cfg.get("max_count").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

    let mark_data: Vec<Value> = pivots
        .iter()
        .take(max_count)
        .filter_map(|p| {
            let ptype = p.get("ptype").and_then(|v| v.as_str())?;
            let idx = p.get("idx").and_then(|v| v.as_u64())? as f64;
            let price = p.get("price").and_then(|v| v.as_f64())?;

            Some(json!({
                "coord": [idx, price],
                "symbol": cfg.get("symbol_map").and_then(|m| m.get(ptype)).and_then(|v| v.as_str()).unwrap_or("pin"),
                "symbolRotate": cfg.get("symbol_rotate").and_then(|m| m.get(ptype)).and_then(|v| v.as_i64()).unwrap_or(0),
                "itemStyle": {
                    "color": cfg.get("color").and_then(|m| m.get(ptype)).and_then(|v| v.as_str()).unwrap_or("#666")
                }
            }))
        })
        .collect();

    if !mark_data.is_empty() {
        series0["markPoint"] = json!({ "data": mark_data });
    }
}

// ── 2. Trendline → markLine ───────────────────────────────────────────

fn apply_trendlines(
    series0: &mut Value,
    pipeline_export: &Value,
    mapping: &Value,
    bar_count: usize,
) {
    if bar_count < 2 {
        return;
    }
    let cfg = &mapping["trendline"];
    let trendlines =
        match find_key_with_freq(pipeline_export, "trendlines:").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return,
        };

    let last_idx = (bar_count - 1) as f64;

    let mark_data: Vec<Value> = trendlines
        .iter()
        .filter_map(|tl| {
            let slope = tl.get("slope").and_then(|v| v.as_f64())?;
            let intercept = tl.get("intercept").and_then(|v| v.as_f64())?;
            let state = tl.get("state").and_then(|v| v.as_str()).unwrap_or("Normal");
            let valid = tl.get("valid").and_then(|v| v.as_bool()).unwrap_or(true);

            if !valid {
                return None;
            }

            let y0 = intercept;
            let y_last = slope * last_idx + intercept;

            let default_style = json!({});
            let style = cfg
                .get("line_style")
                .and_then(|ls| ls.get(state))
                .unwrap_or(&default_style);

            Some(json!({
                "coords": [[0.0, y0], [last_idx, y_last]],
                "lineStyle": style
            }))
        })
        .collect();

    if !mark_data.is_empty() {
        merge_mark_data(series0, "markLine", mark_data);
    }
}

// ── 3. Magnet → markArea ──────────────────────────────────────────────

fn apply_magnets(series0: &mut Value, pipeline_export: &Value, mapping: &Value) {
    let cfg = &mapping["magnet"];
    let magnets = match find_key_with_freq(pipeline_export, "magnets:").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return,
    };

    let area_color = cfg
        .get("area_color")
        .and_then(|v| v.as_str())
        .unwrap_or("rgba(100,180,255,0.15)");
    let border_real = cfg
        .get("border_color_real")
        .and_then(|v| v.as_str())
        .unwrap_or("#42a5f5");
    let border_phantom = cfg
        .get("border_color_phantom")
        .and_then(|v| v.as_str())
        .unwrap_or("#90caf9");

    let mark_data: Vec<Value> = magnets
        .iter()
        .filter_map(|m| {
            let upper = m.get("upper").and_then(|v| v.as_f64())?;
            let lower = m.get("lower").and_then(|v| v.as_f64())?;
            let is_real = m.get("is_real").and_then(|v| v.as_bool()).unwrap_or(false);

            let border_color = if is_real { border_real } else { border_phantom };

            Some(json!({
                "coords": [[{ "yAxis": lower, "xAxis": "min" }], [{ "yAxis": upper, "xAxis": "max" }]],
                "itemStyle": { "color": area_color },
                "lineStyle": { "color": border_color, "type": "dashed" }
            }))
        })
        .collect();

    if !mark_data.is_empty() {
        series0["markArea"] = json!({ "data": mark_data });
    }
}

// ── 4. TriplePush → markLine (vertical dashed line) ───────────────────

fn apply_triple_push(series0: &mut Value, pipeline_export: &Value, mapping: &Value) {
    let cfg = &mapping["triple_push"];
    let pushes =
        match find_key_with_freq(pipeline_export, "triple_push:").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return,
        };

    let label_text = cfg
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or("TriplePush");
    let line_style = cfg.get("line_style").unwrap_or(&Value::Null);

    let mut push_mark_data: Vec<Value> = Vec::new();

    for push in pushes {
        let points = match push.get("push_points").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => continue,
        };
        let overshoot = push
            .get("overshoot")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        for point in points {
            let x = match point.as_u64() {
                Some(v) => v as f64,
                None => continue,
            };
            push_mark_data.push(json!({
                "xAxis": x,
                "lineStyle": line_style,
                "label": {
                    "formatter": if overshoot { format!("{label_text}(Overshoot)") } else { label_text.to_string() },
                    "position": "start"
                }
            }));
        }
    }

    if !push_mark_data.is_empty() {
        merge_mark_data(series0, "markLine", push_mark_data);
    }
}

// ── 5. VolChannel → line series (auxiliary lines) ─────────────────────

fn apply_vol_channel(
    echarts_option: &mut Value,
    pipeline_export: &Value,
    mapping: &Value,
    bar_count: usize,
) {
    if bar_count == 0 {
        return;
    }
    let cfg = &mapping["vol_channel"];
    let vc = match find_key_with_freq(pipeline_export, "vol_channel:") {
        Some(v) => v,
        None => return,
    };

    let upper = match vc.get("upper").and_then(|v| v.as_f64()) {
        Some(v) => v,
        None => return,
    };
    let lower = match vc.get("lower").and_then(|v| v.as_f64()) {
        Some(v) => v,
        None => return,
    };
    let line_style = cfg.get("line_style").unwrap_or(&Value::Null);

    let upper_data: Vec<Value> = (0..bar_count).map(|i| json!([i as f64, upper])).collect();
    let lower_data: Vec<Value> = (0..bar_count).map(|i| json!([i as f64, lower])).collect();

    if let Some(series_arr) = echarts_option
        .get_mut("series")
        .and_then(|v| v.as_array_mut())
    {
        series_arr.push(json!({
            "type": "line",
            "data": upper_data,
            "lineStyle": line_style,
            "name": "VolUpper",
            "symbol": "none"
        }));
        series_arr.push(json!({
            "type": "line",
            "data": lower_data,
            "lineStyle": line_style,
            "name": "VolLower",
            "symbol": "none"
        }));
    }
}

// ── helpers ────────────────────────────────────────────────────────────

/// Merge mark data into the specified key (markLine / markPoint / markArea) of
/// series[0]. If data already exists, append; otherwise create.
fn merge_mark_data(series0: &mut Value, key: &str, new_data: Vec<Value>) {
    if let Some(existing) = series0.get_mut(key) {
        if let Some(arr) = existing.get_mut("data").and_then(|v| v.as_array_mut()) {
            arr.extend(new_data);
        } else {
            existing["data"] = json!(new_data);
        }
    } else {
        series0[key] = json!({ "data": new_data });
    }
}
