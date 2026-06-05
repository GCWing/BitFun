**中文** | [English](AGENTS.md)

# 产品层

本层负责不绑定 UI 形态、应用进程、外部协议或平台 adapter 的产品领域事实和能力组装信息。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `product-domains` | MiniApp、function-agent 等产品领域契约 | [AGENTS.md](product-domains/AGENTS.md) |
| `product-capabilities` | Capability packs、delivery-profile facts、tool provider groups 和 harness registry facts | [AGENTS.md](product-capabilities/AGENTS.md) |

## 放置规则

- 被多个交付形态共享的产品概念、capability facts 和领域策略放到这里。
- UI 文案、route state、协议 adapter、Tauri command、OS 服务实现不要进入 product crate。
- Product capabilities 可以描述所需 runtime services 和 tool packs，但不实例化具体 service implementation。
- 产品规则需要平台数据时，应依赖 contract 或 service API；不要在 product code 中直接读取 host state。

## 依赖边界

- Product crate 可以依赖 `contracts` 和用于描述能力的少量 `runtime` facts。
- Product crate 不得依赖 `facade/core`、`src/apps`、前端代码或 Tauri。
- 除非 product crate 拥有 service 所需的领域类型，否则避免依赖 `services`；具体行为优先留在 services。
