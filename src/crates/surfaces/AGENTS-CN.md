**中文** | [English](AGENTS.md)

# 产品入口层

本层放置通过外部协议或宿主入口暴露已组装产品行为的 Rust crate。UI 应用和交付宿主仍优先阅读离代码最近的 `AGENTS.md`。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `acp` | 基于已组装产品 runtime 的 Agent Client Protocol 入口 | [AGENTS.md](acp/AGENTS.md) |

## 放置规则

- 依赖 `facade/core` 或产品组装计划的协议入口放在这里。
- provider DTO、transport emitter、平台 adapter 等低层适配留在 `integrations`。
- 可复用服务实现留在 `services`。

## 依赖边界

- surface crate 可以依赖 `facade/core` 暴露选定交付形态。
- surface crate 不拥有产品策略、可复用服务或执行原语。
