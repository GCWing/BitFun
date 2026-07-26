**中文** | [English](AGENTS.md)

# 接口层

本层放置通过外部协议或宿主入口暴露已组装产品行为的 Rust crate。UI 应用和交付宿主仍位于 `src/apps`、`src/web-ui`、`src/mobile-web` 和 `BitFun-Installer`，并优先阅读离代码最近的 `AGENTS.md`。

## 模块

| Crate | 职责 | 本地文档 |
|---|---|---|
| `acp` | 基于已组装产品 runtime 的 Agent Client Protocol 接口 | [AGENTS.md](acp/AGENTS.md) |
| `sdk-host` | 版本化的本地 Agent SDK Host 协议与连接用例；进程启动和 stdio framing 仍由 `src/apps/sdk-host` 负责 | — |

## 放置规则

- 依赖 `assembly/core` 或已组装产品 profile 的协议入口放在这里。
- transport、协议转换和宿主通信 adapter 放在 `adapters`。
- OS、filesystem、terminal、MCP、remote、git 等可复用实现放在 `services`。

## 依赖边界

- interface crate 可以依赖 `assembly/core` 暴露选定交付形态。
- 可移植的 `sdk-host` 协议 crate 边界更窄：只能依赖稳定 Runtime/合同，不得依赖
  `bitfun-core`、`terminal-core`、具体 service、SDK Host app 或 CLI；具体 Host 组装保留在
  `src/apps/sdk-host`。
- interface crate 不拥有产品策略、可复用服务、协议传输内部实现或执行原语。
