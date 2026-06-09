**中文** | [English](AGENTS.md)

# 执行原语层

本层负责可复用的 agent、harness、stream、typed-service 和 tool 执行原语。它不是完整 Agent Runtime SDK，也不是组装后的产品 runtime。由产品组装决定某个交付形态启用哪些 execution primitive、tool provider group、harness provider、adapter 和 service。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `agent-runtime` | Agent registry、scheduler、prompt cache、hooks、goal、prompt facts、DeepReview provider-neutral state、DeepResearch citation renumbering 和 runtime control 契约 | [AGENTS.md](agent-runtime/AGENTS.md) |
| `agent-stream` | Provider-neutral stream DTO、tool-call 累积和 replay 契约 | [AGENTS.md](agent-stream/AGENTS.md) |
| `tool-contracts` | Tool 契约、execution gate、input validation 和 result presentation 契约；Cargo package 仍为 `bitfun-agent-tools` | [AGENTS.md](tool-contracts/AGENTS.md) |
| `harness` | Harness workflow 契约和 registry primitive | [AGENTS.md](harness/AGENTS.md) |
| `runtime-services` | Typed runtime service assembly 和 service availability facts | [AGENTS.md](runtime-services/AGENTS.md) |
| `tool-provider-groups` | Tool provider group facts 和 product-full tool group composition；Cargo package 仍为 `bitfun-tool-packs` | [AGENTS.md](tool-provider-groups/AGENTS.md) |
| `tool-execution` | 底层 file/search/tool IO helper；Cargo package 仍为 `tool-runtime` | [AGENTS.md](tool-execution/AGENTS.md) |

## 放置规则

- 可移植 execution 编排、agent lifecycle 契约、tool 契约、provider-neutral stream 契约和 execution facts 放到这里。
- 具体 filesystem、git、terminal、MCP server、remote SSH、OS 行为应放到 `services`，除非只是纯底层 tool primitive。
- 协议 projection 与外部 provider 请求整形放到 `adapters`。
- 产品 feature 选择和 delivery-profile 决策放到 `assembly`，不要放入 execution primitive。
- Tool packs 只描述 provider group 和所需服务；具体服务访问应通过 port 或 typed runtime service。

## 依赖边界

- Execution primitive crate 可以依赖 `contracts`，也可以依赖本层拥有的窄 provider-neutral DTO。
- Execution primitive crate 不得依赖 `assembly/core`、`src/apps`、前端代码、Tauri API 或产品形态 lifecycle。
- 本层不得依赖 `adapters`。新增对 `services` 的依赖时，必须在最近的模块文档或 PR 描述里说明边界原因。
