**中文** | [English](AGENTS.md)

# Core Agent 指南

## 适用范围

本文件适用于 `src/crates/assembly/core`。仓库级规则请看顶层 `AGENTS.md`；进入更具体目录后，优先遵循更近的局部指南。

## 定位

`bitfun-core` 是共享产品 runtime facade。它仍承载兼容路径和 `product-full` 组装边界，但新的拆解工作应优先遵循
`docs/architecture/core-decomposition.md` 与
`docs/architecture/agent-runtime-services-design.md` 中定义的 owner crate 边界。

主要区域：

- `src/agentic/`：agents、prompts、tools、sessions、execution、persistence
- `src/service/`：config、filesystem、terminal、git、LSP、MCP、remote connect、project context、AI memory
- `src/infrastructure/`：AI clients、app paths、event system、storage、debug log server
- `src/product_runtime/`：product-full 兼容 adapter 与 runtime service provider wiring

Agent 运行时心智模型：

```text
SessionManager -> Session -> DialogTurn -> ModelRound
```

## 边界规则

- 共享 core 必须保持平台无关。避免引入 `tauri::AppHandle` 等宿主 API；优先使用
  `bitfun_events::EventEmitter` 等共享抽象。
- 桌面端专属集成应放在 `src/apps/desktop`，再通过 transport / API layer 连接回来。
- 不要在没有窄 port/interface 边界的情况下新增 `service` 到 `agentic` 的跨层引用。
- 不要把平台专属逻辑、构建脚本行为、产品能力选择或 provider-specific AI 序列化写进 shared core。
- owner 从 core 外移时，在下游调用点被有意迁移前，用 facade 或 re-export 保持旧 import path。

## 拆解规则

- 将 `bitfun-core` 视为兼容 facade 与完整产品组装点，而不是新稳定契约的默认归属。
- 稳定 DTO、facts、ports 和纯决策应放到有明确边界的 owner crate；具体 manager、IO、平台 adapter 和产品执行在没有评审过的
  port/provider 设计与行为等价测试前继续留在 core。
- Tool 改动必须保持 expanded/collapsed exposure、prompt-visible manifest、`GetToolSpec`、权限行为、
  `ToolUseContext` 语义，以及 desktop/MCP/ACP catalog 行为等价。
- Runtime owner 迁移在目标 owner 具备评审过的 port/provider 设计和行为等价测试前，不应移动 concrete lifecycle、IO、event delivery、permission orchestration 或 remote/platform implementation。
- Product-domain 改动可以在有等价保护时迁移纯产品领域计划；filesystem writes、worker/host side effect、
  Git/AI concrete calls、marker IO 和 path-manager integration 仍留在 core，除非有经过评审的 owner 设计。
- Remote/service 改动必须保持 external protocol lifecycle、workspace projection、scheduler/session restore、
  terminal pre-warm 和 product execution 边界清晰。
- Feature 改动必须保持 `product-full` 作为兼容产品组装边界；默认能力选择只有在单独的 product matrix review 后才能变化。

## 归属参考

归属细节放在下列文件中，不要继续扩写本指南：

- `docs/architecture/core-decomposition.md`
- `docs/architecture/agent-runtime-services-design.md`
- `src/crates/execution/agent-runtime/AGENTS.md`
- `src/crates/execution/tool-contracts/AGENTS.md`
- `src/crates/execution/harness/AGENTS.md`
- `src/crates/contracts/product-domains/AGENTS.md`
- `src/crates/contracts/runtime-ports/` 与 `src/crates/execution/runtime-services/` 源码说明
- `src/crates/services/services-core/AGENTS.md`
- `src/crates/services/services-integrations/AGENTS.md`
- `src/crates/execution/tool-provider-groups/AGENTS.md`

部分子目录已有更细指南：

- `src/crates/adapters/ai-adapters/AGENTS.md`
- `src/agentic/execution/AGENTS.md`
- `src/agentic/deep_review/AGENTS.md`

## 验证

按触及行为选择最小检查：

```bash
cargo check --workspace
cargo test -p bitfun-core <test_name> -- --nocapture
node scripts/check-core-boundaries.mjs
```

仅改文档时运行 `git diff --check`。
