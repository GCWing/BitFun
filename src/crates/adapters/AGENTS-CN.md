**中文** | [English](AGENTS.md)

# 适配层

本层负责协议、transport、外部 provider 与宿主通信相关的 adapter crate。Adapter 在产品/runtime 契约与具体协议之间做转换，不拥有产品策略，也不承载可复用 OS service 实现。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `ai-adapters` | AI provider 请求/响应 adapter 与 stream protocol glue | [AGENTS.md](ai-adapters/AGENTS.md) |
| `api-layer` | 产品宿主共用的后端 API adapter surface | [AGENTS.md](api-layer/AGENTS.md) |
| `transport` | Event transport emitter 与宿主 transport adapter | [AGENTS.md](transport/AGENTS.md) |
| `webdriver` | Embedded WebDriver protocol 与浏览器自动化 adapter | [AGENTS.md](webdriver/AGENTS.md) |

## 放置规则

- 协议序列化、transport projection、外部 provider 请求整形、宿主通信 adapter 放在这里。
- OS、filesystem、terminal、MCP、remote、git、watch 等可复用实现放在 `services`，除非代码只是协议转换。
- 交付 profile 选择和 adapter 注册属于 `assembly`。

## 依赖边界

- Adapter 可以依赖 `contracts`、`execution`，必要时可窄依赖 `services` 以通过协议暴露 service 能力。
- Adapter 不得依赖 `assembly/core`、产品 UI、app command handler 或 Tauri API，除非该宿主边界有明确 feature gate。
- 优先通过稳定契约解耦 adapter。Adapter 之间直接依赖必须有明确边界理由。
