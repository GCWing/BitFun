# BitFun Core 拆解与运行时迁移计划

本文只维护后续执行计划。稳定目标以
[`core-decomposition.md`](../architecture/core-decomposition.md)、
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md) 和
[`plugin-runtime-host-design.md`](../architecture/plugin-runtime-host-design.md)
为准；已完成事实归档在 [`core-decomposition-completed.md`](core-decomposition-completed.md)。
设计文档默认保持稳定，只有目标架构本身需要修正时才修改。

## 1. 执行原则

- `bitfun-core` 最终收敛为 compatibility facade、`product-full` 组装边界和少量迁移期 adapter。
- 新抽象必须同步删除、迁移或显著简化旧 core 主体路径；纯 facade、纯 guard、纯文档或空接口不算完成。
- Product Assembly 是 composition root；除它以外，普通层级只能依赖稳定 contract、port、descriptor 或被注入的 typed part。
- 产品特性和内核能力分开：长程任务、调度、权限、上下文、session/workspace、memory、DFX、hook/event 属于 Agent Kernel；
  `/goal`、UI、settings、命令和默认策略属于 Product Feature。
- 主体进程插件 API 只暴露 `PluginRuntimeClient`、binding、envelope、candidate、trust 和 descriptor；不得感知具体生态 adapter。
- 任何会改变权限、工具 schema、事件语义、session 生命周期、remote 行为、MiniApp 行为、UI extension contract 或交付形态的变更必须暂停并单独评审。

## 2. 当前基线

- workspace 已按六层物理目录展开：`interfaces -> assembly -> adapters -> services -> execution -> contracts`。
- Runtime Services、Agent Runtime、Tool Contracts、Tool Execution、Harness、Product Domains、Services Core、Services Integrations 等 owner crate 已建立。
- `bitfun-core --no-default-features` 已裁掉多批 concrete provider 和 direct provider 依赖；Desktop、CLI、ACP 仍通过 `bitfun-core/product-full` 获取完整产品能力。
- Agentic frontend event projection 和 AgenticEvent projection manifest 已进入 `bitfun-events`；Tauri/WebSocket transport 不再内联事件字段映射或 legacy event allowlist。
- Tool ABI 基础合同已进入 `tool-contracts`：materialized snapshot、provider identity、default permission/effect filter、cancellation contract 和 stale-call guard 由 owner crate 提供，core 只投射现有产品 Tool 元数据。
- Terminal / ExecCommand、remote SSH concrete execution、workspace search、debug ingest、AI provider adapter runtime、browser CDP、WebFetch/WebSearch、review platform transport 等多批 owner 已迁出或收口到 port/provider。
- Boundary scripts 已覆盖核心 owner 防回流、six-layer path 解析、facade-only 文件、custom agent owner / custom subagent wrapper 保护和重点 feature gate。

## 3. 目标差距

| 差距 | 影响 | 收敛要求 |
|---|---|---|
| Plugin Runtime Host 仍是设计合同，未形成主进程窄接口和 disabled/projection stub | 主体进程可能继续感知具体 adapter 或运行单元 | 落地 `PluginRuntimeClient`、binding、capability matrix 和 Host facade 白名单 |
| UI Extension Contract 与产品形态矩阵仍需实现 | Desktop/Web/CLI/SDK/ACP 的插件 UI 行为可能不一致 | 建立 descriptor round-trip、fallback、unsupported/unavailable 和只读 state view |
| OpenCode compatibility adapter 仍缺少真实消费路径 | OpenCode 插件能力无法受控进入 BitFun | 先支持 discovery/read-only，再逐步开放 tool provider 和 UI contribution |
| 部分 concrete owner 仍在 core 或产品命令路径 | 层级依赖和平台差异仍可能回流 | 继续迁移 Computer Use OS action、Git/process/session host adapter、MCP auth URL helper 等 |
| SDK readiness 仍未闭环 | 独立 Agent Runtime SDK 可能牵引 product-full 或 concrete provider | fake-provider smoke、minimal feature、cargo tree/metadata 对比和 API version 保护 |

## 4. 后续大型阶段

### Stage C：Plugin Runtime Host Foundation

目标：建立主体进程的插件运行时窄接口，并将 OpenCode 等生态适配器限制在 Host 内部。

范围：

- 定义 `PluginRuntimeClient`、`PluginRuntimeBinding`、`PluginRuntimeAvailability`、disabled/projection-only stub。
- 定义 plugin dispatch/response envelope、effect candidate、trust policy、adapter manifest 和 diagnostics。
- 建立 Host facade 白名单、project execution domain、deadline、epoch、idempotency 和 failure quarantine。
- ACP bridge 保持 integration owner，只通过稳定 capability/effect 与插件运行时互操作。

准出：

- 主体进程不暴露 `OpenCodeCompatibilityAdapter` 等具体 adapter 类型。
- 插件、ACP、external skills 不得直接写 kernel 权威状态、permission decision、audit event 或 UI implementation。
- lifecycle / failure / dispose、permission candidate、event/audit 和 boundary focused checks 通过。

### Stage D：UI Extension Contract 与产品形态矩阵

目标：为插件 UI contribution 提供声明式 descriptor，并明确不同交付形态的支持、禁用和降级行为。

范围：

- 定义 slot、route、command/keymap、prompt augmentation、dialog/toast、settings entry、state view descriptor。
- UI state view 只读；descriptor 不包含 React component、Tauri window、DOM node、renderer handle 或可执行代码。
- Product Assembly 维护 UI contribution registry、capability matrix 和 unsupported/unavailable fallback。
- 建立 Desktop、Web、CLI、Remote、ACP、SDK、Mobile Web 的插件能力矩阵。

准出：

- UI descriptor round-trip、host fallback、unsupported/unavailable 和 product-shape focused tests 通过。
- Web、Desktop、CLI 不因 UI Extension Contract 引入互相依赖。

### Stage E：OpenCode Compatibility Adapter

目标：在 Plugin Runtime Host、Tool ABI、Event Manifest 和 UI Extension Contract 可用后，实现受控 OpenCode 兼容适配。

范围：

- 建立 OpenCode server plugin hook support matrix：event、tool、permission.ask、tool.execute.before/after、tool.definition、config/provider/model/skill/MCP transform。
- 建立 OpenCode TUI plugin support matrix：slot、route、command/keymap、prompt、toast/dialog、theme、只读 state view。
- 将 OpenCode tool、event、permission、workspace/worktree、remote path、artifact ref 映射为 BitFun canonical contract。
- 不支持能力返回 typed unsupported；可写 transform 和外部 JS/TS plugin runtime 单独安全评审后再开放。

准出：

- OpenCode adapter 不依赖 `bitfun-core/product-full`、full `RuntimeServices` bundle、UI implementation 或 concrete provider handle。
- adapter、permission/effect、event manifest、UI contribution 和 Desktop/CLI/Server/Remote/ACP/Web/Mobile Web/SDK product shape checks 通过。

### Stage F：剩余 Concrete Owner 与 SDK Readiness

目标：完成剩余 concrete owner 收口，并验证独立 Agent Runtime SDK 边界。

范围：

- 继续迁移 Computer Use OS action、部分 Git/process/session host adapter、MCP auth URL helper 等剩余 concrete owner。
- Product Assembly 负责选择 provider；Kernel、Execution、Extension、Product Feature 不直接依赖 platform concrete。
- 建立 SDK minimal fake-provider smoke，确认 minimal feature 不牵引 Desktop、Tauri、Git provider、MCP client、AI HTTP client、remote SSH 或产品 UI。

准出：

- 至少完成 2-3 个 concrete owner 的实际迁移，并同步删除或简化 core 旧主体路径。
- `cargo check --workspace`、`cargo check -p bitfun-core --no-default-features`、SDK minimal smoke、cargo metadata/tree 对比和必要 product checks 通过。

## 5. 固定执行流程

1. 同步最新 `main`，检查主干新增的 CLI、tool、terminal、session、scheduler、remote、MiniApp、ACP、OpenCode、plugin、UI 或 product interface 变更。
2. 对照设计文档和 Issue #970 明确本次 owner 边界，不从旧计划标签继承完成判断。
3. 先补等价保护和 boundary guard，再迁移实现主体。
4. 删除、迁移或显著简化 core 中对应旧路径。
5. 运行 focused verification、boundary check 和必要的 feature / dependency / product-shape 对比。
6. 从独立第三方角度审查功能漂移、性能劣化、依赖回流、产品形态遗漏、安全绕过和文档一致性。
7. 合入后只更新 completed 摘要和 issue 状态；设计文档只有目标架构变更时才修改。

## 6. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| Workspace layout / Cargo path | `cargo metadata --no-deps --format-version 1` |
| Product Feature / capability matrix | `cargo test -p bitfun-product-capabilities`，feature pack focused tests，UI descriptor focused tests |
| Agent Kernel / permission / event | `cargo test -p bitfun-agent-runtime`，`cargo check -p bitfun-core --no-default-features` |
| Runtime Services / backend events | `cargo test -p bitfun-runtime-services`，backend event delivery focused tests |
| Tool / MCP / terminal / sandbox | `cargo test -p bitfun-agent-tools`，`cargo test -p tool-runtime`，terminal / exec-command / MCP focused tests |
| Extension / OpenCode / ACP | plugin runtime host focused tests，OpenCode adapter focused tests，ACP permission / external tool focused tests |
| Product shape / SDK | SDK fake-provider smoke，Desktop / CLI / Server / Remote / ACP / Web / Mobile Web capability matrix checks，cargo tree / metadata 对比 |
| 大范围 owner 迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |

## 7. 暂停条件

- 新 owner crate 必须依赖回 `bitfun-core` 才能编译或测试。
- Agent Kernel 吸收 product feature、UI state、Tauri、产品命令、AI provider、MCP client、process execution、Git provider 等 concrete dependency。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- Plugin Runtime Host 直接写 permission、audit、kernel state、tool result 或 UI implementation。
- PR 只新增抽象，没有迁移、删除或显著简化旧 core 主体路径。
- SDK facade 必须暴露 `bitfun-core`、`product-full`、concrete service manager 或产品命令 registry 才能完成基本 agent 执行。
