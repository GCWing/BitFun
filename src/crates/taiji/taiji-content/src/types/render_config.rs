use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Video render config: complete parameters for Pipeline JSON → ECharts option → PNG frame sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoRenderConfig {
    /// Output resolution, default (1920, 1080)
    pub resolution: (u16, u16),
    /// Frame rate
    pub fps: u8,
    /// Background color, e.g. "#0a0e27"
    pub bg_color: String,
    /// Brand watermark PNG path, None = no overlay
    pub brand_watermark: Option<PathBuf>,
    /// ECharts option template file path
    pub kline_echarts_template: PathBuf,
    /// Taiji type → ECharts mark mapping table file path
    pub annotation_mapping: PathBuf,
}

/// Date range with start/end bounds.
///
/// **Canonical definition.** `taiji-growth`, `taiji-publisher`, and `taiji-backtest`
/// re-export this type from here to avoid duplication.
/// Import via `taiji_content::DateRange` or `taiji_content::types::render_config::DateRange`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_render_config_roundtrip() {
        let config = VideoRenderConfig {
            resolution: (1920, 1080),
            fps: 30,
            bg_color: "#0a0e27".into(),
            brand_watermark: None,
            kline_echarts_template: PathBuf::from(
                "scripts/video-render-template/kline_echarts_option.json",
            ),
            annotation_mapping: PathBuf::from(
                "scripts/video-render-template/annotation_mapping.json",
            ),
        };
        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: VideoRenderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.resolution, (1920, 1080));
    }

    #[test]
    fn test_date_range_roundtrip() {
        let range = DateRange {
            start: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
        };
        let json = serde_json::to_string(&range).unwrap();
        let roundtrip: DateRange = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.start.to_string(), "2026-07-01");
    }
}
