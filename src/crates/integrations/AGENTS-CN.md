**中文** | [English](AGENTS.md)

# 具体适配实现层

本层负责低层外部协议、provider、transport 和平台 adapter。依赖 `facade/core` 暴露已组装产品行为的协议入口应放在 `src/crates/surfaces`，不要放在这里。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `ai-adapters` | AI provider DTO 和 provider-facing adapter helper | [AGENTS.md](ai-adapters/AGENTS.md) |
| `api-layer` | 基于 transport 抽象的平台无关 API handler | [AGENTS.md](api-layer/AGENTS.md) |
| `transport` | 跨平台通信 adapter 和 emitter | [AGENTS.md](transport/AGENTS.md) |
| `webdriver` | 内嵌 WebDriver 协议与 runtime 实现 | [AGENTS.md](webdriver/AGENTS.md) |

## 放置规则

- 主要职责是连接 BitFun contract 与外部系统的协议、framework、provider adapter 放在这里。
- 低层 adapter 可以依赖 contract 或窄 execution facts，不依赖产品组装。
- 不要把产品策略、可复用服务实现或 agent/tool 编排放入本层。

## 依赖边界

- integration crate 可依赖 `contracts`、execution facts 和窄 provider 依赖。
- integration crate 不得依赖 `facade/core`；产品入口型协议适配应移动到 `src/crates/surfaces`。
- 平台特定依赖应尽可能 optional 或隔离，避免较小交付形态被迫编译无关 adapter。
