# taiji-alert — Multi-Channel Alert Module

Three-tier alert routing: Desktop notification (Warn) → Feishu Webhook (Error) → Email (Critical). HeartbeatMonitor detects 30-minute silence.

## Architecture Position

```
taiji-alert (standalone — zero taiji internal deps)
  ├── AlertManager (routing + aggregation window)
  ├── FeishuWebhookAlerter (interactive card)
  ├── EmailAlerter (lettre SMTP)
  ├── DesktopAlerter (Tauri notification)
  └── HeartbeatMonitor (30min silence detection)
```

## Core Types

```rust
pub enum AlertLevel { Warn, Error, Critical, Heartbeat }

pub struct AlertConfig {
    pub feishu_webhook_url: Option<String>,
    pub smtp_config: Option<SmtpConfig>,
    pub alert_level: AlertLevel,
    pub heartbeat_interval_min: u32,      // default 30
    pub aggregation_window_secs: u64,      // default 300
}
```

## Quick Start

```rust
use taiji_alert::{AlertManager, AlertConfig, AlertLevel, AlertMessage};

let manager = AlertManager::new(AlertConfig {
    feishu_webhook_url: Some("https://open.feishu.cn/...".into()),
    ..Default::default()
});

manager.alert(AlertMessage {
    level: AlertLevel::Error,
    title: "Pipeline failure".into(),
    body: "taiji_video_generate returned exit code 1".into(),
    source: "CronService::process_job".into(),
    ..Default::default()
}).await?;
```

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
