# taiji-publisher

**路径**: src/crates/taiji/taiji-publisher
**描述**: Taiji multi-platform video publisher

## 依赖
- 内部: taiji-content
- 外部: serde, serde_json, chrono, async-trait, futures, tokio, reqwest

## 模块结构
- `lib.rs` — VideoAsset 类型 + PlatformPublisher trait + PublishResult/PublishStatus
- `biliup` — Bilibili 发布器
- `publisher_twitter` — Twitter/X 发布器
- `publisher_wechat_mp` — 微信公众号发布器
- `social_auto` — 社交媒体自动发布
- `publish_scheduler` — 发布调度器
- `process_util` — 处理工具

## 核心类型
- `VideoAsset` — 视频发布资产描述
- `PlatformPublisher` — 多平台统一发布 trait
- `PublishResult` / `PublishStatus` — 发布结果与状态

## 属于领域
- content / publishing
