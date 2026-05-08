**中文** | [English](AGENTS.md)

# AGENTS-CN.md

## 适用范围

本文件适用于 `src/crates/core`。仓库级规则请看顶层 `AGENTS.md`。

## 这里最重要的内容

`bitfun-core` 是共享产品逻辑中心。

主要区域：

- `src/agentic/`：agents、prompts、tools、sessions、execution、persistence
- `src/service/`：config、filesystem、terminal、git、LSP、MCP、remote connect、project context、AI memory
- `src/infrastructure/`：AI clients、app paths、event system、storage、debug log server

Agent 运行时心智模型：

```text
SessionManager → Session → DialogTurn → ModelRound
```

## 本模块规则

- 共享 core 必须保持平台无关
- 避免引入 `tauri::AppHandle` 等宿主 API
- 使用 `bitfun_events::EventEmitter` 等共享抽象
- 桌面端专属集成应放在 `src/apps/desktop`，再通过 transport / API layer 连接回来

这里已经有更细粒度规则：

- `src/crates/ai-adapters/AGENTS.md`
- `src/agentic/execution/AGENTS.md`

## 命令

```bash
cargo check --workspace
cargo test --workspace
cargo test -p bitfun-core <test_name> -- --nocapture
```

## 验证

```bash
cargo check --workspace && cargo test --workspace
```
