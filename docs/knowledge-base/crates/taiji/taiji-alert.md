# taiji-alert

**路径**: src/crates/taiji/taiji-alert
**描述**: Taiji alert module — multi-channel alarm notification (Feishu webhook, email, desktop)

## 依赖
- 内部: 无
- 外部: serde, serde_json, reqwest, tokio, chrono, thiserror, tracing, lettre

## 模块结构
- `alerters` — 告警发送器（FeishuWebhookAlerter / DesktopAlerter / EmailAlerter）
- `heartbeat` — 心跳监控
- `lib.rs` — AlertLevel / AlertConfig / AlertMessage / AlertManager

## 核心类型
- `AlertLevel` — 告警等级（Heartbeat/Warn/Error/Critical）
- `AlertConfig` — 全局告警配置
- `AlertMessage` — 告警消息体
- `AlertManager` — 中央告警分发器（聚合+路由）
- `SmtpConfig` — SMTP 邮件配置

## 属于领域
- operations / monitoring
