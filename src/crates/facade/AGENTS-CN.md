**中文** | [English](AGENTS.md)

# 门面与产品组装层

本层负责面向旧消费者的兼容导出和产品组装。它选择产品能力、交付形态和 provider 注册，并把下层 owner 接线起来。本层不应成为执行原语、服务实现、产品领域策略或协议实现的长期 owner。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `core` | 旧 `bitfun-core` facade、兼容 import 和 product-full 组装 | [AGENTS.md](core/AGENTS.md) |

## 放置规则

- 旧 import 兼容、product-full 接线和 assembly shim 放在这里。
- 与交付形态或旧 `bitfun-core` 兼容绑定的 provider 选择和注册可以放在这里。
- 稳定 owner 逻辑应下移到 `contracts`、`execution`、`services`、`product` 或 `integrations`。
- 保持现有 public import path，除非迁移明确移除它并补充兼容说明和测试。
- facade 增量应小而可追踪；大块功能增长通常说明 owner 没有充分下移。

## 依赖边界

- `facade/core` 可以依赖下层 owner 来组装当前产品 runtime。
- facade 可以依赖低层 integration adapter，但不实现协议序列化、认证、transport 或平台细节。
- 产品入口型协议 surface 可以调用 facade；低层 integration adapter 不应调用 facade。
- 避免在 facade 中直接使用宿主 API；Tauri 支持必须保持 feature-gated，并尽可能由 app 或 adapter 拥有。
