# taiji-publisher — Multi-Platform Video Publisher

Unified `PlatformPublisher` trait + `PublishScheduler` with exponential backoff retry. Implements Bilibili (biliup), social-auto-upload (Douyin/Xiaohongshu), Twitter, and WeChat MP publishing.

## Architecture Position

```
taiji-publisher (standalone — zero taiji internal deps)
  ├── PlatformPublisher trait (async)
  ├── PublishScheduler (JoinSet concurrent, max_concurrent:3)
  ├── BiliupPublisher, SocialPublisher
  ├── TwitterPublisher, WechatMpPublisher
  └── VideoAsset (16 fields, serde)
```

## Core Trait

```rust
#[async_trait]
pub trait PlatformPublisher: Send + Sync {
    fn platform_name(&self) -> &str;
    async fn check_auth(&self) -> Result<bool, String>;
    async fn upload(&self, video: &VideoAsset) -> Result<PublishResult, String>;
    async fn status(&self, publish_id: &str) -> Result<PublishStatus, String>;
    async fn update(&self, _video: &VideoAsset) -> Result<PublishResult, String> { ... }
    async fn unpublish(&self, _publish_id: &str) -> Result<PublishStatus, String> { ... }
}
```

## Quick Start

```rust
use taiji_publisher::{PublishScheduler, BiliupPublisher, VideoAsset};

let publishers: Vec<Box<dyn PlatformPublisher>> = vec![
    Box::new(BiliupPublisher::new(cookie_path)),
];
let scheduler = PublishScheduler::new(publishers)
    .with_max_concurrent(3)
    .with_retry(3, Duration::from_secs(1));

let results = scheduler.publish_all(&video_asset);
```

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
