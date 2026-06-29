# BitFun Core 拆解与运行时迁移计划

本文只维护后续执行计划。稳定目标以
[`core-decomposition.md`](../architecture/core-decomposition.md) 和
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
为准；已完成事实归档在
[`core-decomposition-completed.md`](core-decomposition-completed.md)。设计文档默认保持稳定，只有目标架构本身需要修正时才修改。

## 1. 执行原则

- `bitfun-core` 最终收敛为 compatibility facade、`product-full` 组装边界和少量迁移期 adapter。
- 迁移按概念 owner 判断：Product Feature、Agent Kernel、Execution、Extension、Cross-platform Adapter、Stable Contracts。
- 外部系统不是 owner 层级；OS、Git、MCP server、AI provider、remote host、browser runtime 和 plugin package 只在
  adapter I/O 边界出现。除 Product Assembly 外，调用方应依赖 port、descriptor 或 stable contract，而不是 concrete provider。
- 新抽象必须同步删除、迁移或显著简化旧 core 主体路径；纯 facade、纯 guard、纯文档或空接口不算完成。
- 产品特性和内核能力必须分开：长程任务、调度、权限、上下文、session/workspace、memory、DFX、hook/event 属于内核；`/goal`、UI、settings、命令和默认策略属于产品特性。
- Product API 同时覆盖 Rust Kernel API、UI Extension Contract 和 Capability/Effect API；不能把所有能力堆进单一后端 API。
- 任何会改变权限、工具 schema、事件语义、session 生命周期、remote 行为、MiniApp 行为、UI extension contract 或交付形态的变更必须暂停并单独评审。

## 2. 当前基线

- workspace 已按六层物理目录展开：`interfaces -> assembly -> adapters -> services -> execution -> contracts`。
- `bitfun-core --no-default-features` 已裁掉 workspace-search owner、debug ingest HTTP server、AI provider adapter runtime 和 direct `reqwest`。
- Desktop / CLI / ACP 仍通过 `bitfun-core/product-full` 获取完整能力；Server / Remote / Web / Mobile Web 不直接依赖 core。
- Product Assembly 已按入口矩阵裁剪能力计划：完整兼容入口保留 product-full 能力，无直接 core 入口不再 materialize product-full capability packs、feature groups、runtime services、tool groups 或 harness routes。
- Runtime Services、Agent Runtime、Tool Contracts、Tool Execution、Harness、Product Domains、Services Core、Services Integrations 等 owner crate 已建立；部分 concrete 生命周期仍由 core concrete manager 或产品命令路径持有。
- Custom agent / mode / skill、Agent lifecycle、tool side-effect、Computer Use、file tool、MiniApp、DeepReview、DeepResearch、remote-connect、workspace search、remote SSH/SFTP/PTY 等多批 provider-neutral 或 concrete owner 已迁出。
- Root boundary scripts 已覆盖核心 owner 防回流、six-layer path 解析、facade-only 文件、custom agent owner / custom subagent wrapper 保护和重点 feature gate。
- Agent Runtime session workspace resolution、Cron / SessionControl / SessionMessage / SessionHistory 的 target session/workspace owner routing、`/goal` tool management runtime-port routing、session/config/context/lifecycle fact owner 收口，以及 `services-integrations` workspace search preview/result conversion 已纳入已完成摘要；后续计划只保留仍需迁移的 feature/kernel、security/control-plane、execution、extension 和 cross-platform adapter 主体工作。
- MiniApp built-in seed orchestration 已进入 `product-domains`，core 只保留 concrete host adapter；session state manager 已进入 `agent-runtime`，core 只保留兼容 re-export。

## 3. 大块 PR 节奏

后续不再按旧 H/M 标签判断完成度。每个 PR 必须包含实质迁移或旧路径显著简化，并在提交前做独立第三方视角的功能边界、依赖关系、不同产品形态和操作系统影响复审。

### PR-C：Execution 层深迁移

目标：

- 继续迁移 built-in tools、skills、MCP tool bridge、sandbox runner、local/remote tool runtime、harness descriptor / route plan 的实际 owner。
- 删除或显著简化 core 中对应 tool/harness 主体路径，保留兼容 facade。
- 区分 MCP tool bridge 与 MCP transport：tool bridge 属于 Execution，transport/client concrete 属于 Cross-platform Adapter。

保护：

- prompt-visible manifest、`GetToolSpec`、permission gate、tool result/artifact、collapsed/expanded exposure、MCP/ACP catalog 和 remote/local path containment 等价。
- `cargo test -p bitfun-agent-tools`、`cargo test -p tool-runtime`、harness / MCP focused tests 和 product shape tests 必跑。

### PR-D：Extension Host 与 OpenCode / ACP 适配收口

目标：

- 定义并落地最小 Extension Host 边界：plugin capability declaration、UI contribution descriptor、tool/hook/workflow provider mapping。
- 明确 OpenCode adapter 将外部 plugin API 映射到 BitFun Rust Kernel API、UI Extension Contract 和 Capability/Effect API。
- ACP 保持协议入口和 external agent/tool capability owner，不下沉到 Agent Kernel。

保护：

- 插件、OpenCode、ACP、external skills 不能直接写 kernel 权威状态、permission decision、audit event 或 UI implementation。
- 未声明能力默认受限；UI contribution descriptor 可 round-trip，并在不支持形态返回 unsupported/unavailable。

### PR-E：Cross-platform Adapter 与多形态 SDK 验证

目标：

- 收口 filesystem、network、process/thread/time、terminal、remote、Git、MCP transport、AI/provider protocol、browser/desktop automation 的 adapter/provider 边界。
- 进一步移除 `bitfun-core` 对 OS/provider concrete 的直接依赖。
- 建立 Desktop、CLI、Web、ACP、Remote、SDK 的最小能力矩阵和验证口径。
- 确认 Kernel、Execution、Extension、Product Feature 不直接依赖 platform concrete；具体 provider 只由 Product Assembly 注册。

保护：

- 不同操作系统、远程/本地、desktop/CLI/web/SDK 构建形态能力不漂移。
- `cargo check --workspace`、`cargo check -p bitfun-core --no-default-features`、SDK minimal smoke、cargo metadata/tree 对比和必要 product checks 必跑。

## 4. 固定执行流程

1. 同步最新 `main`，检查主干新增的 CLI、tool、terminal、session、scheduler、remote、MiniApp、ACP、OpenCode、plugin、UI 或 product interface 变更。
2. 对照设计文档和 Issue #970 明确本次 owner 边界，不从旧计划标签继承完成判断。
3. 先补等价保护和 boundary guard，再迁移实现主体。
4. 删除、迁移或显著简化 core 中对应旧路径。
5. 运行 focused verification、boundary check 和必要的 feature / dependency / product-shape 对比。
6. 从独立第三方角度审查功能漂移、性能劣化、依赖回流、产品形态遗漏、安全绕过和文档一致性。
7. 合入后只更新 completed 摘要和 issue 状态；设计文档只有目标架构变更时才修改。

## 5. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| Workspace layout / Cargo path | `cargo metadata --no-deps --format-version 1` |
| Product Feature / capability matrix | `cargo test -p bitfun-product-capabilities`，feature pack focused tests，相关 UI descriptor focused tests |
| Agent Kernel / permission / event | `cargo test -p bitfun-agent-runtime`，`cargo check -p bitfun-core --no-default-features` |
| Runtime Services / backend events | `cargo test -p bitfun-runtime-services`，backend event delivery focused tests |
| Services Core session migration | `cargo test -p bitfun-services-core merge_legacy_session_store`，core workspace-runtime focused tests |
| Remote Connect / IM bot support | `cargo test -p bitfun-services-integrations --features remote-connect --lib remote_connect::bot::`，`cargo test -p bitfun-core --features product-full remote_connect::bot::command_router` |
| Tool / MCP / terminal / sandbox | `cargo test -p bitfun-agent-tools`，`cargo test -p tool-runtime`，terminal / exec-command / MCP focused tests |
| Harness / Product Domains | `cargo test -p bitfun-harness`，`cargo test -p bitfun-product-domains`，DeepReview / MiniApp focused tests |
| Extension / OpenCode / ACP | extension host focused tests，UI contribution descriptor tests，ACP permission / external tool focused tests |
| Product shape / SDK | SDK fake-provider smoke，Desktop / CLI / Web / ACP capability matrix checks，cargo tree / metadata 对比 |
| 大范围 owner 迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |

## 6. 暂停条件

- 新 owner crate 必须依赖回 `bitfun-core` 才能编译或测试。
- Agent Kernel 吸收 product feature、UI state、Tauri、产品命令、AI provider、MCP client、process execution、Git provider 等 concrete dependency。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- Extension Host 直接写 permission、audit、kernel state 或 UI implementation。
- PR 只新增抽象，没有迁移、删除或显著简化旧 core 主体路径。
- SDK facade 必须暴露 `bitfun-core`、`product-full`、concrete service manager 或产品命令 registry 才能完成基本 agent 执行。
