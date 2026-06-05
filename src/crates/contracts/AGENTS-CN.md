**中文** | [English](AGENTS.md)

# 契约层

本层负责可被 runtime、services、product、integrations、facade 和应用形态共享的稳定契约，不向上携带具体实现细节。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `core-types` | 共享 DTO、错误、session/surface 数据和小型 value type | [AGENTS.md](core-types/AGENTS.md) |
| `events` | 事件 payload 和 emitter 契约 | [AGENTS.md](events/AGENTS.md) |
| `runtime-ports` | runtime owner crate 使用的 trait 和 port | [AGENTS.md](runtime-ports/AGENTS.md) |

## 放置规则

- 只有跨多个 owner layer 稳定复用的类型才放到这里。
- 契约层应保持轻行为：允许少量校验 helper，不放 runtime、filesystem、network、UI 或平台行为。
- 优先定义窄 DTO 或 trait，不引入宽泛 facade object。
- 如果类型只服务单个 runtime 或 product crate，先留在所属 crate 内，等出现第二个 owner 再提取。

## 依赖边界

- 本层可以依赖 workspace 基础库和其他 contract crate。
- 本层不得依赖 `runtime`、`services`、`product`、`integrations`、`facade`、`src/apps`、前端包、Tauri 或 OS adapter。
- 新依赖必须服务契约形状本身，而不是为了实现层使用方便。
