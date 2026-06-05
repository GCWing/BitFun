**中文** | [English](AGENTS.md)

# 服务层

本层负责可复用的非 UI 服务实现和服务 adapter：filesystem、git、process/system、diagnostics、terminal、MCP、remote 以及持久化相关能力。通用服务应通过窄 API 或 port 被调用，而不是通过产品 facade 泄漏。产品特定 adapter 可以实现产品领域 port，但不得拥有产品策略。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `services-core` | filesystem、diff、diagnostics、session usage、token usage、system、process 等核心可复用服务 | [AGENTS.md](services-core/AGENTS.md) |
| `services-integrations` | announcement、file watch、function agents、git、MCP、remote connect、remote SSH 等具体集成服务 | [AGENTS.md](services-integrations/AGENTS.md) |
| `terminal` | Terminal API、PTY、shell integration 和 persistent terminal sessions | [AGENTS.md](terminal/AGENTS.md) |

## 放置规则

- 可被多个产品或 runtime 路径复用的具体 host/service 行为放到这里。
- UI 状态、产品 feature 选择和交付组装不要进入本层。
- 优先提供小而清晰的 service API，不要做混合多种职责的大 manager。
- 依赖平台能力的行为要通过 service module 或 feature gate 隔离。

## 依赖边界

- 通用 service crate 不应依赖产品 crate。
- service adapter 只有在 feature gate 后实现产品层定义的窄 port/DTO 时，才允许依赖 `product-domains`。当前例子是 `services-integrations` 为 `product-domains` function-agent ports 实现 Git snapshot。不要把这个例外扩展成 service 拥有产品策略。
- Services 不得依赖 `facade/core`、`src/apps`、前端代码或 Tauri `AppHandle`。
- Remote 和平台支持必须通过 typed service error 或明确 unsupported-state 处理失败，不要泛化成字符串错误。
