**中文** | [English](AGENTS.md)

# 产品组装层

本层负责产品组装、兼容导出、能力选择和 runtime 注册。它为不同交付形态把下层能力接线起来，但不拥有具体 adapter 行为、可复用 service 实现、OS 集成或稳定产品领域契约。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `core` | `bitfun-core` 兼容门面与 product-full 组装 | [AGENTS.md](core/AGENTS.md) |
| `external-sources` | 基于能力专属 provider 契约的生态中立来源生命周期协调 | 继承本指南 |
| `product-capabilities` | 产品能力 profile、tool group facts、service requirements 与 harness selection | [AGENTS.md](product-capabilities/AGENTS.md) |

## 放置规则

- product-full 接线、兼容 shim、能力 profile 选择和 adapter/service 注册放在这里。
- 产品领域规则属于 `contracts/product-domains`；组装层可以选择这些事实，但不拥有它们。
- 稳定 owner 逻辑下移到 `contracts`，可移植执行逻辑下移到 `execution`，协议适配下移到 `adapters`，可复用实现下移到 `services`。
- 保持现有 public import path，除非迁移明确移除并补充兼容说明和测试。
- 组装层新增内容必须小而可追溯；如果功能持续在此扩张，说明所有权还没有下移到合适 owner。

## 依赖边界

- `assembly/core` 可以依赖下层 owner 来组装当前产品 runtime。
- 组装 crate 不得依赖 `src/apps/*`。embedded relay 的 Cargo 反向依赖已经删除；其 TCP 绑定、静态资源 fallback
  和任务生命周期仍是 assembly 内的兼容路径，不得复制，也不能据此宣称宿主归属已经迁移完成。
- 组装层可以依赖 adapter 与 service crate，但不实现它们的协议序列化、认证、transport 或平台细节。
- 避免在组装层直接使用宿主 API；Tauri 支持必须保持 feature-gated，并尽可能由 app 或 adapter 拥有。
- interface crate 可以调用组装 API；adapter 和 service 不得依赖组装层。
