//! Markdown report generation module.
//!
//! Converts taiji_export JSON (pipeline state + agent outputs) into Hugo Markdown
//! via Tera templates, producing daily and weekly trading review reports.

use crate::types::ReportConfig;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};

/// Markdown report generator backed by Tera templates.
pub struct ReportMdGenerator {
    tera: Tera,
}

impl ReportMdGenerator {
    /// Create a new generator with compile-time embedded templates.
    pub fn new() -> Result<Self, String> {
        let mut tera = Tera::default();
        tera.add_raw_template(
            "daily_report.tera",
            include_str!("../templates/daily_report.tera"),
        )
        .map_err(|e| format!("Failed to load daily_report template: {}", e))?;
        tera.add_raw_template(
            "weekly_report.tera",
            include_str!("../templates/weekly_report.tera"),
        )
        .map_err(|e| format!("Failed to load weekly_report template: {}", e))?;
        Ok(Self { tera })
    }

    /// Generate a daily report Markdown string from combined taiji_export JSON.
    ///
    /// `data` should contain pipeline state keys (`_meta`, `bars:{freq}`) plus
    /// agent output objects keyed by agent name (e.g. `structure_agent`,
    /// `decision_agent`, etc.).
    pub fn generate_daily_report(
        &self,
        data: &Value,
        config: &ReportConfig,
    ) -> Result<String, String> {
        let ctx = self.build_daily_context(data, config)?;
        self.tera
            .render("daily_report.tera", &ctx)
            .map_err(|e| format!("Template render error: {}", e))
    }

    /// Generate a weekly report Markdown string from combined taiji_export JSON.
    pub fn generate_weekly_report(
        &self,
        data: &Value,
        config: &ReportConfig,
    ) -> Result<String, String> {
        let ctx = self.build_weekly_context(data, config)?;
        self.tera
            .render("weekly_report.tera", &ctx)
            .map_err(|e| format!("Template render error: {}", e))
    }

    /// Batch-generate reports from all JSON files in `export_dir`.
    ///
    /// Reads every `.json` file, parses it, and renders a report according to
    /// `config.template` ("daily_report" or "weekly_report").  Output files are
    /// written to `output_dir` as `{instrument}_{date}.md`.
    pub fn generate_all(
        &self,
        export_dir: &Path,
        output_dir: &Path,
        config: &ReportConfig,
    ) -> Result<Vec<PathBuf>, String> {
        let mut generated = Vec::new();

        std::fs::create_dir_all(output_dir).map_err(|e| {
            format!(
                "Failed to create output dir {}: {}",
                output_dir.display(),
                e
            )
        })?;

        let entries = std::fs::read_dir(export_dir)
            .map_err(|e| format!("Failed to read export dir {}: {}", export_dir.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let raw = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let data: Value = serde_json::from_str(&raw)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

            let md = match config.template.as_str() {
                "weekly_report" => self.generate_weekly_report(&data, config)?,
                _ => self.generate_daily_report(&data, config)?,
            };

            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("report");
            let out_path = output_dir.join(format!("{}.md", stem));
            std::fs::write(&out_path, &md)
                .map_err(|e| format!("Failed to write {}: {}", out_path.display(), e))?;
            generated.push(out_path);
        }

        Ok(generated)
    }

    // ── private helpers ──

    fn build_daily_context(&self, data: &Value, config: &ReportConfig) -> Result<Context, String> {
        let mut ctx = Context::new();

        // _meta
        let meta = data.get("_meta");
        ctx.insert(
            "instrument",
            meta_str(meta, "instrument", &config.instrument),
        );
        ctx.insert("timestamp", meta_str(meta, "timestamp", ""));
        ctx.insert("freq", meta_str(meta, "freq", &config.freq));
        ctx.insert("date", &config.date_range.end.to_string());

        // bars
        let bars = self.extract_bars(data, &config.freq);
        ctx.insert("bars", &bars);

        // structure_agent
        self.inject_structure(&mut ctx, data);

        // magnet_agent
        self.inject_magnet(&mut ctx, data);

        // thrust_agent
        self.inject_thrust(&mut ctx, data);

        // resonance_agent
        self.inject_resonance(&mut ctx, data);

        // decision_agent
        self.inject_decision(&mut ctx, data);

        // risk_agent
        self.inject_risk(&mut ctx, data);

        Ok(ctx)
    }

    fn build_weekly_context(&self, data: &Value, config: &ReportConfig) -> Result<Context, String> {
        let mut ctx = Context::new();

        let meta = data.get("_meta");
        ctx.insert(
            "instrument",
            meta_str(meta, "instrument", &config.instrument),
        );
        ctx.insert("timestamp", meta_str(meta, "timestamp", ""));
        ctx.insert("freq", meta_str(meta, "freq", &config.freq));
        ctx.insert("date", &config.date_range.end.to_string());
        ctx.insert("date_start", &config.date_range.start.to_string());

        let bars = self.extract_bars(data, &config.freq);
        ctx.insert("bars", &bars);

        self.inject_structure(&mut ctx, data);
        self.inject_risk(&mut ctx, data);

        // weekly entries — extracted from `weekly_entries` array in data, or empty
        let entries: Vec<Value> = data
            .get("weekly_entries")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        ctx.insert("weekly_entries", &entries);

        // aggregate stats
        let stats = self.compute_weekly_stats(&entries);
        ctx.insert("weekly_signal_count", &stats.signal_count);
        ctx.insert("weekly_long_count", &stats.long_count);
        ctx.insert("weekly_short_count", &stats.short_count);
        ctx.insert("weekly_hold_count", &stats.hold_count);
        ctx.insert("weekly_avg_confidence", &stats.avg_confidence);

        Ok(ctx)
    }

    fn extract_bars(&self, data: &Value, freq: &str) -> Vec<Value> {
        let bars_key = format!("bars:{}", freq);
        data.get(&bars_key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    }

    // ── agent injectors ──

    fn inject_structure(&self, ctx: &mut Context, data: &Value) {
        let s = data.get("structure_agent");
        let a = s.and_then(|v| v.get("analysis"));
        ctx.insert("trend_direction", agent_str(a, "trend_direction", "N/A"));
        ctx.insert("trend_strength", &agent_f64(a, "trend_strength", 0.0));
        ctx.insert("pivot_structure", agent_str(a, "pivot_structure", "N/A"));
        ctx.insert("key_support", &agent_f64(a, "key_support", 0.0));
        ctx.insert("key_resistance", &agent_f64(a, "key_resistance", 0.0));
        ctx.insert("channel_state", agent_str(a, "channel_state", "N/A"));
        ctx.insert("structure_notes", agent_str(a, "notes", ""));
        ctx.insert(
            "structure_confidence",
            &s.and_then(|v| v.get("confidence"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
    }

    fn inject_magnet(&self, ctx: &mut Context, data: &Value) {
        let m = data.get("magnet_agent");
        let a = m.and_then(|v| v.get("analysis"));
        ctx.insert(
            "magnet_valid",
            &a.and_then(|v| v.get("magnet_valid"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        );
        ctx.insert("magnet_position", agent_str(a, "magnet_position", "N/A"));
        ctx.insert("magnet_state", &agent_str_or_null(a, "magnet_state"));
        ctx.insert("magnet_direction", &agent_str_or_null(a, "direction"));
        ctx.insert("magnet_channel_state", agent_str(a, "channel_state", "N/A"));
        ctx.insert(
            "magnet_confidence",
            &m.and_then(|v| v.get("confidence"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
    }

    fn inject_thrust(&self, ctx: &mut Context, data: &Value) {
        let t = data.get("thrust_agent");
        let a = t.and_then(|v| v.get("analysis"));
        ctx.insert(
            "thrust_found",
            &a.and_then(|v| v.get("triple_push_found"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        );
        ctx.insert(
            "thrust_count",
            &a.and_then(|v| v.get("push_count"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        );
        ctx.insert(
            "thrust_exhaustion",
            &a.and_then(|v| v.get("exhaustion"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        );
        ctx.insert("thrust_direction", &agent_str_or_null(a, "direction"));
        ctx.insert(
            "thrust_confidence",
            &t.and_then(|v| v.get("confidence"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
    }

    fn inject_resonance(&self, ctx: &mut Context, data: &Value) {
        let r = data.get("resonance_agent");
        let a = r.and_then(|v| v.get("analysis"));
        ctx.insert(
            "resonance",
            &a.and_then(|v| v.get("resonance"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        );
        ctx.insert("resonance_type", &agent_str_or_null(a, "resonance_type"));
        // aligned_agents / conflicting_agents are arrays — join them
        let aligned = a
            .and_then(|v| v.get("aligned_agents"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let conflicting = a
            .and_then(|v| v.get("conflicting_agents"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        ctx.insert("resonance_aligned", &aligned);
        ctx.insert("resonance_conflicting", &conflicting);
        ctx.insert(
            "resonance_confidence",
            &r.and_then(|v| v.get("confidence"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
    }

    fn inject_decision(&self, ctx: &mut Context, data: &Value) {
        let d = data.get("decision_agent");
        let dec = d.and_then(|v| v.get("decision"));
        ctx.insert("decision_action", agent_str(dec, "action", "N/A"));
        ctx.insert("decision_reasoning", agent_str(dec, "reasoning", ""));
        ctx.insert(
            "decision_confidence",
            &d.and_then(|v| v.get("confidence"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
    }

    fn inject_risk(&self, ctx: &mut Context, data: &Value) {
        let r = data.get("risk_agent");
        let c = r.and_then(|v| v.get("constraints"));
        ctx.insert(
            "risk_allow_long",
            &c.and_then(|v| v.get("allow_long"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        );
        ctx.insert(
            "risk_allow_short",
            &c.and_then(|v| v.get("allow_short"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        );
        ctx.insert(
            "risk_max_size",
            &c.and_then(|v| v.get("max_size"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
        let analysis = r.and_then(|v| v.get("analysis"));
        ctx.insert(
            "risk_atr",
            &analysis
                .and_then(|v| v.get("current_atr"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
        ctx.insert(
            "risk_kelly",
            &analysis
                .and_then(|v| v.get("kelly_fraction"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        );
        ctx.insert(
            "risk_per_trade_pct",
            &analysis
                .and_then(|v| v.get("risk_per_trade_pct"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.02),
        );
    }

    fn compute_weekly_stats(&self, entries: &[Value]) -> WeeklyStats {
        let signal_count = entries.len() as u64;
        let mut long_count: u64 = 0;
        let mut short_count: u64 = 0;
        let mut hold_count: u64 = 0;
        let mut confidence_sum: f64 = 0.0;

        for e in entries {
            let action = e.get("action").and_then(|v| v.as_str()).unwrap_or("Hold");
            match action {
                "Long" => long_count += 1,
                "Short" => short_count += 1,
                _ => hold_count += 1,
            }
            confidence_sum += e.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
        }

        let avg_confidence = if signal_count > 0 {
            (confidence_sum / signal_count as f64 * 100.0).round() / 100.0
        } else {
            0.0
        };

        WeeklyStats {
            signal_count,
            long_count,
            short_count,
            hold_count,
            avg_confidence,
        }
    }
}

struct WeeklyStats {
    signal_count: u64,
    long_count: u64,
    short_count: u64,
    hold_count: u64,
    avg_confidence: f64,
}

// ── JSON extraction helpers ──

fn meta_str<'a>(meta: Option<&'a Value>, key: &str, default: &'a str) -> &'a str {
    meta.and_then(|m| m.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or(default)
}

fn agent_str<'a>(analysis: Option<&'a Value>, key: &str, default: &'a str) -> &'a str {
    analysis
        .and_then(|a| a.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or(default)
}

fn agent_str_or_null(analysis: Option<&Value>, key: &str) -> String {
    analysis
        .and_then(|a| a.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn agent_f64(analysis: Option<&Value>, key: &str, default: f64) -> f64 {
    analysis
        .and_then(|a| a.get(key))
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DateRange;
    use chrono::NaiveDate;
    use serde_json::json;

    fn make_config() -> ReportConfig {
        ReportConfig {
            instrument: "ag2506".into(),
            freq: "5min".into(),
            date_range: DateRange {
                start: NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
            },
            template: "daily_report".into(),
            output_dir: PathBuf::from("output"),
        }
    }

    fn make_mock_data() -> Value {
        json!({
            "_meta": {
                "instrument": "ag2506",
                "timestamp": "2026-07-21T15:00:00Z",
                "freq": "5min"
            },
            "bars:5min": [
                {
                    "symbol": "ag2506",
                    "dt": "2026-07-21T14:55:00+00:00",
                    "freq": "5min",
                    "open": 4500.0,
                    "high": 4520.0,
                    "low": 4495.0,
                    "close": 4510.0,
                    "vol": 15000.0,
                    "amount": 6.75e7,
                    "open_interest": 50000.0,
                    "delta": 1200.0
                },
                {
                    "symbol": "ag2506",
                    "dt": "2026-07-21T15:00:00+00:00",
                    "freq": "5min",
                    "open": 4510.0,
                    "high": 4530.0,
                    "low": 4505.0,
                    "close": 4520.0,
                    "vol": 18000.0,
                    "amount": 8.13e7,
                    "open_interest": 51200.0,
                    "delta": 1200.0
                }
            ],
            "structure_agent": {
                "agent": "structure_agent",
                "timestamp": "2026-07-21T15:00:00Z",
                "instrument": "ag2506",
                "freq": "5min",
                "analysis": {
                    "trend_direction": "up",
                    "trend_strength": 0.73,
                    "pivot_structure": "higher_highs",
                    "key_support": 5610.0,
                    "key_resistance": 5650.0,
                    "channel_state": "expanding",
                    "notes": "通道扩张，趋势加速中"
                },
                "confidence": 0.80
            },
            "magnet_agent": {
                "agent": "magnet_agent",
                "timestamp": "2026-07-21T15:00:00Z",
                "instrument": "ag2506",
                "freq": "5min",
                "analysis": {
                    "magnet_position": "above",
                    "magnet_valid": true,
                    "magnet_state": "突破",
                    "direction": "up",
                    "oi_confirmation": true,
                    "vol_confirmation": true,
                    "mm1_target": 5700.0,
                    "mm1_progress_pct": 35.0,
                    "mm2_target": 5680.0,
                    "mm2_progress_pct": 60.0,
                    "resonance_levels": [],
                    "channel_state": "expanding"
                },
                "confidence": 0.82
            },
            "thrust_agent": {
                "agent": "thrust_agent",
                "timestamp": "2026-07-21T15:00:00Z",
                "instrument": "ag2506",
                "freq": "5min",
                "analysis": {
                    "triple_push_found": true,
                    "push_count": 3,
                    "direction": "up",
                    "exhaustion": true,
                    "overshoot": false,
                    "bos_detected": true,
                    "choch_detected": true
                },
                "confidence": 0.82
            },
            "resonance_agent": {
                "agent": "resonance_agent",
                "timestamp": "2026-07-21T15:00:00Z",
                "instrument": "ag2506",
                "freq": "5min",
                "analysis": {
                    "resonance": true,
                    "resonance_type": "bullish",
                    "aligned_agents": ["structure_agent", "delta_agent", "magnet_agent", "thrust_agent"],
                    "conflicting_agents": [],
                    "multi_tf_resonance": null
                },
                "confidence": 0.85
            },
            "decision_agent": {
                "agent": "decision_agent",
                "timestamp": "2026-07-21T15:00:00Z",
                "instrument": "ag2506",
                "freq": "5min",
                "decision": {
                    "action": "Long",
                    "entry": 4520.0,
                    "stop_loss": 4490.0,
                    "take_profit": 4620.0,
                    "size_pct": 0.15,
                    "reasoning": "四维共振看多：structure trend up(0.80), delta long_building(0.75), magnet above(0.82), thrust up exhaustion(0.82)。风控允许做多(ATR=28)。"
                },
                "confidence": 0.82
            },
            "risk_agent": {
                "agent": "risk_agent",
                "timestamp": "2026-07-21T15:00:00Z",
                "instrument": "ag2506",
                "freq": "5min",
                "analysis": {
                    "max_position_pct": 0.15,
                    "current_atr": 28.0,
                    "kelly_fraction": 0.25,
                    "risk_per_trade_pct": 0.02
                },
                "constraints": {
                    "allow_long": true,
                    "allow_short": false,
                    "max_size": 2.5,
                    "stop_distance_atr_mult": 2.5
                }
            }
        })
    }

    #[test]
    fn test_new_creates_generator() {
        let gen = ReportMdGenerator::new().expect("should create generator");
        // verify templates are loaded
        assert!(gen
            .tera
            .get_template_names()
            .any(|n| n == "daily_report.tera"));
        assert!(gen
            .tera
            .get_template_names()
            .any(|n| n == "weekly_report.tera"));
    }

    #[test]
    fn test_generate_daily_report_has_front_matter() {
        let gen = ReportMdGenerator::new().unwrap();
        let data = make_mock_data();
        let config = make_config();

        let md = gen
            .generate_daily_report(&data, &config)
            .expect("should generate daily report");

        // Hugo TOML front matter
        assert!(
            md.starts_with("+++"),
            "should start with TOML front matter delimiter"
        );
        assert!(md.contains("title ="), "should have title");
        assert!(md.contains("date ="), "should have date");
        assert!(md.contains("tags ="), "should have tags");
        assert!(md.contains("categories ="), "should have categories");
        assert!(md.contains("draft = false"), "draft should be false");

        // body content
        assert!(
            md.contains("## 一、市场结构分析"),
            "should have structure section"
        );
        assert!(
            md.contains("## 二、磁体定位分析"),
            "should have magnet section"
        );
        assert!(
            md.contains("## 三、三推形态分析"),
            "should have thrust section"
        );
        assert!(
            md.contains("## 四、共振分析"),
            "should have resonance section"
        );
        assert!(
            md.contains("## 五、交易决策"),
            "should have decision section"
        );
        assert!(md.contains("## 六、风控评估"), "should have risk section");
        assert!(md.contains("## 七、K线数据"), "should have bars table");

        // extracted values
        assert!(md.contains("ag2506"), "should contain instrument");
        assert!(md.contains("up"), "should contain trend direction");
        assert!(md.contains("4520"), "should contain close price");
        assert!(md.contains("Long"), "should contain decision action");
    }

    #[test]
    fn test_generate_weekly_report_has_front_matter() {
        let gen = ReportMdGenerator::new().unwrap();
        let config = make_config();

        let data = json!({
            "_meta": {
                "instrument": "ag2506",
                "timestamp": "2026-07-26T15:00:00Z",
                "freq": "5min"
            },
            "bars:5min": [
                {"dt": "2026-07-26T15:00:00+00:00", "open": 4520.0, "high": 4550.0, "low": 4510.0, "close": 4540.0, "vol": 20000.0}
            ],
            "structure_agent": {
                "analysis": {"trend_direction": "up", "trend_strength": 0.73, "pivot_structure": "higher_highs", "key_support": 5610.0, "key_resistance": 5650.0, "channel_state": "expanding"},
                "confidence": 0.80
            },
            "risk_agent": {
                "analysis": {"current_atr": 28.0, "kelly_fraction": 0.25, "risk_per_trade_pct": 0.02},
                "constraints": {"allow_long": true, "allow_short": false, "max_size": 2.5}
            },
            "weekly_entries": [
                {"date": "2026-07-21", "resonance_type": "bullish", "action": "Long", "confidence": 0.82},
                {"date": "2026-07-22", "resonance_type": "bullish", "action": "Long", "confidence": 0.75},
                {"date": "2026-07-23", "resonance_type": "none", "action": "Hold", "confidence": 0.40}
            ]
        });

        let md = gen
            .generate_weekly_report(&data, &config)
            .expect("should generate weekly report");

        assert!(
            md.starts_with("+++"),
            "should start with TOML front matter delimiter"
        );
        assert!(md.contains("周度复盘"), "should have weekly title");
        assert!(
            md.contains("## 一、周度行情概览"),
            "should have overview section"
        );
        assert!(
            md.contains("## 二、共振闭环回顾"),
            "should have resonance review"
        );
        assert!(
            md.contains("## 三、交易决策汇总"),
            "should have decision summary"
        );
        assert!(md.contains("## 四、风控汇总"), "should have risk summary");

        // aggregate stats
        assert!(md.contains("3"), "should have signal count 3");
        // avg_confidence: (0.82+0.75+0.40)/3 = 0.656... rounds to 0.66
    }

    #[test]
    fn test_generate_with_minimal_data_does_not_panic() {
        let gen = ReportMdGenerator::new().unwrap();
        let config = make_config();

        // Empty data — all agent fields absent
        let data = json!({
            "_meta": {
                "instrument": "ag2506",
                "timestamp": "2026-07-21T15:00:00Z",
                "freq": "5min"
            },
            "bars:5min": []
        });

        let md = gen
            .generate_daily_report(&data, &config)
            .expect("should generate with minimal data");

        // Should still have valid front matter
        assert!(md.starts_with("+++"));
        assert!(md.contains("ag2506"));
        assert!(md.contains("N/A"), "missing fields should show N/A");
    }

    #[test]
    fn test_generate_all_creates_files() {
        let gen = ReportMdGenerator::new().unwrap();
        let config = make_config();

        let export_dir = tempfile::tempdir().expect("failed to create temp export dir");
        let output_dir = tempfile::tempdir().expect("failed to create temp output dir");

        let data = make_mock_data();
        let export_path = export_dir.path().join("ag2506_2026-07-21.json");
        std::fs::write(&export_path, serde_json::to_string_pretty(&data).unwrap()).unwrap();

        let generated = gen
            .generate_all(export_dir.path(), output_dir.path(), &config)
            .expect("generate_all should succeed");

        assert_eq!(generated.len(), 1);
        let out_path = &generated[0];
        assert!(out_path.exists(), "output file should exist");

        let content = std::fs::read_to_string(out_path).unwrap();
        assert!(content.starts_with("+++"));
    }

    #[test]
    fn test_tera_render_with_mock_data() {
        let gen = ReportMdGenerator::new().unwrap();
        let data = make_mock_data();
        let config = make_config();

        let md = gen.generate_daily_report(&data, &config).unwrap();

        // Verify no Tera template syntax leaks through (all vars resolved)
        assert!(
            !md.contains("{{"),
            "should not contain unresolved Tera variables"
        );
        assert!(
            !md.contains("{%"),
            "should not contain unresolved Tera tags"
        );
    }
}
