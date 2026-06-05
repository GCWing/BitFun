**中文** | [English](AGENTS.md)

# 服务实现层

本层负责接触本地系统或 runtime infrastructure 的可复用具体实现：filesystem、git、file watch、terminal、MCP、remote connectivity、process lifecycle 以及类似 OS/network 能力。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `services-core` | 不包含产品组装决策的本地 service primitive | [AGENTS.md](services-core/AGENTS.md) |
| `services-integrations` | MCP、git、remote、file watch 与产品领域 port 的具体实现 | [AGENTS.md](services-integrations/AGENTS.md) |
| `terminal` | PTY、shell integration 与 terminal session infrastructure | [AGENTS.md](terminal/AGENTS.md) |

## 放置规则

- 具体 OS、process、filesystem、git、terminal、MCP、remote SSH、file watch 和 network service 实现放在这里。
- 需要具体依赖的 `contracts`、`execution` 或 `contracts/product-domains` port 实现在这里。
- 协议/transport projection 放在 `adapters`，产品能力选择放在 `assembly`。

## 依赖边界

- Services 可以依赖 `contracts`，实现 runtime port 时可以窄依赖 provider-neutral 的 `execution` crate。
- Services 不得依赖 `assembly/core`、interface crate、产品 UI 或 app command handler。
- Service 之间直接依赖必须保持窄边界；可复用契约应下沉到 `contracts` 或 `execution`，避免形成大范围耦合。
