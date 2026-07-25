# taiji-content

**路径**: src/crates/taiji/taiji-content
**描述**: Taiji content workshop — video rendering, TTS, and FFmpeg composition

## 依赖
- 内部: bitfun-core（../../assembly/core）
- 外部: serde, serde_json, chrono, log, image

## 模块结构
- `annotation` — 视频标注/注释
- `chart_option` — ECharts option JSON 模板（String::replace 渲染）
- `composer` — FFmpeg 视频合成编排
- `cron_job` — 定时任务
- `kline_renderer` — K 线图渲染
- `live_stream` — 直播流
- `types` — 类型定义（含 DateRange 等）

## 核心类型
- `DateRange` — 日期范围（被多个 crate re-export）

## 属于领域
- content / media
