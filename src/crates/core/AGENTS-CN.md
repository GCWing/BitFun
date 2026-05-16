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
- core 拆解期间，`bitfun-core` 是兼容 facade 与完整产品 runtime assembly 点；新模块优先放到 `docs/architecture/core-decomposition.md` 指定的 owner crate。
- Tool 相关轻量 contract 与 generic registry/provider container 归属 `bitfun-agent-tools`；core tool runtime 当前负责产品工具组装、`dyn Tool` 适配、snapshot decoration、tool exposure / manifest resolution，以及按需工具说明发现（`GetToolSpec`）。
- `ToolUseContext` 与具体工具实现继续留在 core，除非已有评审过的 port/provider 方案和等价测试。
- Tool 迁移必须保持 expanded/collapsed exposure、prompt 可见 manifest、`ToolUseContext.unlocked_collapsed_tools`，以及 desktop/MCP/ACP tool catalog 行为等价。
- 不要在没有小型 port/interface 边界的情况下新增 `service` 到 `agentic` 的跨层引用。
- 不要在 core 拆解中把平台专属逻辑、构建脚本行为或产品能力选择下沉到 shared core。

这里已经有更细粒度规则：

- `src/crates/ai-adapters/AGENTS.md`
- `src/agentic/execution/AGENTS.md`
- `src/agentic/deep_review/AGENTS.md`

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
