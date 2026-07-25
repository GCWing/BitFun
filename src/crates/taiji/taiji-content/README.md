# taiji-content — Taiji Content Workshop

Phase 4 video rendering pipeline core crate. Pipeline JSON → ECharts rendering → TTS voiceover → FFmpeg composition.

## Architecture

```
Pipeline JSON (taiji_export)
  ├─ chart_option.rs       → ECharts option builder
  ├─ annotation.rs         → Technical annotation overlay (Pivot/Trendline/Magnet/TriplePush/VolChannel)
  ├─ types/tts_types.rs    → TTS config type definitions
  ├─ composer.rs           → FFmpeg Builder + A/V sync validation
  ├─ types/compose_config.rs → Composition config types
  ├─ types/render_config.rs  → Render config + DateRange
  ├─ cron_job.rs           → Cron scheduling jobs
  └─ types/mod.rs          → Module index
```

## Dependencies

- `serde` / `serde_json` — JSON serialization
- `chrono` — Date/time types
- `log` — Logging

## Quick Start

```rust
use taiji_content::types::render_config::VideoRenderConfig;
use taiji_content::chart_option::build_echarts_option;

let config = VideoRenderConfig {
    resolution: (1920, 1080),
    fps: 30,
    bg_color: "#0a0e27".into(),
    brand_watermark: None,
    kline_echarts_template: "scripts/video-render-template/kline_echarts_option.json".into(),
    annotation_mapping: "scripts/video-render-template/annotation_mapping.json".into(),
};

let option = build_echarts_option(&pipeline_json, &config)?;
```

## Module Index

| Module | File | Description |
|--------|------|-------------|
| Type definitions | `types/render_config.rs` | `VideoRenderConfig` + `DateRange` |
| | `types/tts_types.rs` | `TtsConfig` + `TtsScript` + `TtsSegment` |
| | `types/compose_config.rs` | `ComposeConfig` + `EncodingProfile` |
| Candlestick rendering | `chart_option.rs` | `build_echarts_option()` — Pipeline JSON → ECharts option |
| Annotation overlay | `annotation.rs` | `apply_annotations()` — 5 Taiji types → ECharts marks |
| Video composition | `composer.rs` | `FfmpegComposer` Builder + `sync_test` A/V sync validation |
| Cron scheduling | `cron_job.rs` | `VideoCronJob` + `VideoScheduler` |

## Data Flow

```
taiji_export (pipeline_export.json)
       │
       ├─→ chart_option.rs      → ECharts option JSON
       │       │
       │       └─→ annotation.rs    → Annotation overlay (markPoint/markLine/markArea)
       │               │
       │               └─→ [Node.js frame_sequence.js]  → PNG frame sequence
       │
       ├─→ [TTS engine (Python)]  → MP3 + SRT
       │
       └─→ composer.rs          → FFmpeg composition → MP4
```

## Related Files

```
scripts/video-render-template/
├── kline_echarts_option.json   ← ECharts option template
└── annotation_mapping.json     ← Taiji type → ECharts mark mapping

scripts/render/
├── frame_sequence.js           ← PNG frame sequence generator
└── package.json                ← node-canvas + echarts

scripts/tts/
└── tts_engine.py               ← Edge TTS voiceover engine

scripts/compose/
└── compose_engine.py           ← FFmpeg composition engine

MiniApp/taiji-video-studio/     ← Video generation MiniApp
```
