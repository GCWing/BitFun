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
- Tool 相关轻量 contract、portable tool context facts/provider、纯 manifest/exposure contract、generic registry / static-provider / dynamic-provider container、file guidance marker、file-read freshness 比较策略和 oversized tool-result preview/rendering 纯策略归属 `bitfun-agent-tools`；core tool runtime 通过 `product_runtime.rs` 统一负责产品工具组装、`dyn Tool` 适配、snapshot decoration、runtime manifest assembly / context filtering，以及按需工具说明发现（`GetToolSpec`）执行。
- `ToolUseContext`、session file-read state storage、tool-result filesystem writes 与具体工具实现继续留在 core，除非已有评审过的 port/provider 方案和等价测试。
- Tool 迁移必须保持 expanded/collapsed exposure、prompt 可见 manifest、`ToolUseContext.unlocked_collapsed_tools`，以及 desktop/MCP/ACP tool catalog 行为等价。
- 不要把 OpenAI Responses / Codex ChatGPT flat tool schema 等 provider-specific 序列化行为写进 core tool contract；AI adapter 负责 provider 序列化，core 保持 provider-neutral manifest。
- 调整 session/token usage 路径时，`cached_content_token_count` 必须继续表示 cache reads/hits，`cache_creation_token_count` 必须作为独立 provider fact 保留。
- Function-agent commit-message 与 Startchat work-state orchestration 可以经由
  `bitfun-product-domains`；Git/AI service adapter、provider 获取、AI client
  调用和 transport error mapping 仍由 core 拥有。prompt template、JSON
  extraction/repair、domain error mapping 与 domain JSON parsing policy 可以放在
  `bitfun-product-domains`。
- MiniApp built-in bundle/hash/marker seed plan 与 marker wire helper 可以放在
  `bitfun-product-domains`；bundled asset include、filesystem writes、marker IO、
  customization metadata IO、recompile orchestration、worker process runtime 和
  host dispatch execution 仍由 core 拥有，直到有评审过的迁移和等价测试。
- Remote-connect wire/tracker/dialog orchestration 与 response wrapping 可以放在
  `bitfun-services-integrations`；remote workspace facts、session metadata、
  file projection DTO 和 remote workspace/projection host trait 属于
  `bitfun-runtime-ports`，`remote_connect` 只保留旧路径 re-export。workspace-root
  source selection、concrete scheduler/session restore、terminal pre-warm adapter
  和 product execution 仍由 core 拥有，直到有评审过的迁移和等价测试。
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
