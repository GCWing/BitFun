# BitFun Core 拆解与运行时迁移计划

本文只维护后续执行计划。稳定目标以
[`product-architecture.md`](../architecture/product-architecture.md) 为准；
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md) 补充接口与 crate 约束；
[`plugin-runtime-host-design.md`](../architecture/plugin-runtime-host-design.md) 是插件运行时和生态兼容的当前主线详细设计。
已完成事实归档在 [`core-decomposition-completed.md`](core-decomposition-completed.md)。

本计划文件名继续保留 `core-decomposition`，因为它记录的是 `bitfun-core` 收敛和 owner 迁移的执行路径。

## 1. 执行原则

- 当前第一优先级是插件生态和扩展能力支撑：Extension Contract、Plugin Runtime Host、candidate effect、安全校验和真实生态适配消费路径必须优先闭环。P0 固定首条体验是 OpenCode-compatible plugin 最小垂直切片，必须覆盖 source 注册/启用、Desktop command/settings、permission 确认、effect materialize 和 CLI diagnostics；ACP 外部 agent/tool bridge 只能作为 P0+ 互操作路径，不能替代该验收。
- 同时保护关键产品路径：ProductFull、Desktop、CLI、ACP，以及 Web / Mobile Web / Server / Remote / SDK 的显式降级或投影。
- `bitfun-core` 最终收敛为 compatibility facade、`product-full` 组装边界和少量迁移期 adapter。
- Product Assembly 是 composition root；除它以外，普通层级只能依赖 stable contract、port、descriptor 或被注入的 typed part。
- 新抽象必须同步删除、迁移或显著简化旧 core 主体路径；纯 facade、纯 guard、纯文档、纯 descriptor、纯 registry 或空接口不算完成。
- 禁止新增没有语义的链式透传。若 A 的真实需求是 C 的稳定接口，且 B 没有策略、校验、多实现选择、反腐或兼容责任，应让 A 直接依赖 C 的 contract/port。
- 产品特性和内核能力分开：长程任务、调度、权限、上下文、session/workspace、memory、DFX、hook/event 属于 Agent Kernel；
  `/goal`、UI、settings、命令和默认策略属于 Product Feature。
- 暂缓的是全量生态兼容、全入口 UI Extension 矩阵、任意可写 transform 和无约束插件 runtime；不是插件生态主线本身。
- 默认保持权限、工具 schema、事件语义、session 生命周期、remote 行为、MiniApp 行为和交付形态等价；若 P0 插件体验需要主动改变，必须有产品决策记录、用户影响、迁移/回滚、指标和验证。

## 2. 执行输入假设

已完成事实以 [`core-decomposition-completed.md`](core-decomposition-completed.md) 为准，本文只记录后续执行需要依赖的输入假设：

- workspace 已按 `interfaces -> assembly -> adapters -> services -> execution -> contracts` 物理目录展开，但概念 owner 仍需通过当前迁移继续收敛。
- Desktop、CLI、ACP 当前仍通过 `bitfun-core/product-full` 获取完整产品能力；P0 插件体验不能继续把这个状态固化为新入口依赖。
- Tool ABI、runtime services、agent runtime、product capabilities 和 plugin disabled / projection-only 基础边界已存在；OpenCode-compatible adapter fixture 合同开始基于真实 OpenCode config / local plugin source 形态验证 discovery / projection 映射，但真实 Plugin Runtime Host、Desktop/CLI 消费路径和 candidate effect 闭环仍未完成。
- Boundary scripts 可用于 owner 防回流、six-layer path、facade-only 文件和重点 feature gate，但插件 P0 需要补充更具体的 host / extension / adapter checks。

## 3. 当前目标差距

| 差距 | 影响 | 当前收敛要求 |
|---|---|---|
| Extension Contract 还没有形成最小闭环 | 插件、插件贡献的 tool provider、ACP 外部 agent、hook、UI contribution 容易各走各的入口 | 围绕 OpenCode-compatible plugin 第一条体验定义 extension point、descriptor、availability、trust/source、candidate effect 和 fallback 的最小稳定合同 |
| Plugin Runtime Host 缺少可执行边界 | 插件能力只能表达 disabled / projection-only，不能受控加载或执行外部贡献 | 建立 host lifecycle、dispatch envelope、deadline、diagnostics、failure quarantine 和 dispose 语义 |
| 真实生态适配缺少消费路径 | OpenCode-compatible plugin 能力无法验证合同是否可用 | 先落 OpenCode-compatible plugin 最小适配链路；ACP 外部 agent/tool bridge 可复用合同但不替代 P0 验收 |
| 部分 concrete owner 仍在 core 或产品命令路径 | 层级依赖和平台差异仍可能回流 | 只迁移与插件主线或关键产品路径直接相关的 owner，并同步删除或显著简化旧路径 |
| 部分调用存在薄 facade / 多层透传 | A 模块依赖 B 再依赖 C，真实责任不清 | 建立直接 contract/port 依赖，保留的 facade 必须说明兼容、反腐、多实现选择或 host 边界责任 |
| SDK minimal readiness 未闭环 | 独立 Agent Runtime 可能牵引 product-full 或 concrete provider | 只做内部 fake-provider smoke、minimal feature、cargo tree/metadata 对比和 API version 保护 |

暂缓但不删除的目标：

- 所有生态的一次性完整兼容。
- 所有入口的一次性完整 UI Extension 矩阵。
- 插件直接覆写 first-party 能力。
- 任意可写 transform、无限制 JS/TS runtime 和无约束 localhost API。
- 对外稳定 SDK 发布。

这些目标重新进入执行范围前，必须同时满足：有明确产品场景、已有真实消费路径、能删除或简化旧路径、完成安全评审，并补充 focused verification。

P0-A、P0-B、P0-C 不是三个可独立交付的抽象阶段。它们必须围绕同一个 OpenCode-compatible plugin 垂直切片推进：

- 任何新增 public descriptor、envelope、host facade 或 availability API，必须绑定这条真实消费路径。
- 如果为了分 PR 降低风险，需要先落内部实现，该实现不得作为稳定 public API 暴露，也不得要求其他模块先适配空 registry。
- P0 的验收以同一条 canonical scenario trace 为准，而不是以单独 crate 编译、descriptor 存在、host facade 可构建或任一单点消费路径为准。

## 4. 后续执行阶段

### Stage P0-A：Extension Contract 最小闭环

目标：先让 OpenCode-compatible plugin、插件贡献的 tool/hook/command、UI contribution 和 candidate effect 共享同一套最小合同；原生 MCP 继续走 Execution + Platform Adapter。

范围：

- 定义 `ExtensionPoint`、capability/effect descriptor、trust/source、data category、execution domain、availability、fallback 和 typed unsupported/unavailable。
- 定义 `PluginEffectCandidate` 的权限、副作用、审计、回滚和 owner 裁决语义。
- 定义最小 `UiContributionDescriptor`，只覆盖当前消费方需要的 slot / command / settings entry / readonly state view。
- 将已有 disabled / projection-only plugin binding 与 extension availability 对齐。
- 定义 Plugin Runtime availability 到产品状态的映射，避免 runtime binding、surface state 和 capability availability 各自新增 enum。

准出：

- 必须绑定同一条 OpenCode-compatible plugin canonical scenario trace：opencode config / local plugin source、descriptor、provider candidate、candidate effect、permission/effect gate 和产品可见状态都服务该端到端体验；不允许只新增 registry 或用单点消费路径宣布 P0-A 完成。
- 不暴露 React component、Tauri handle、runtime service manager、具体生态对象或 untyped `Any`。
- 新增或更新具体测试目标，并在 PR 中列出路径；测试至少覆盖 descriptor round-trip、unsupported/unavailable、candidate effect 拒绝路径和 availability 映射。

### Stage P0-B：Plugin Runtime Host 可执行边界

目标：建立受控插件 Host，让插件可以执行，但不能成为 kernel、permission、audit、tool result 或 UI implementation 的权威源。

范围：

- 定义 host lifecycle、manifest validation、dispatch envelope、deadline、epoch、idempotency、diagnostics、dispose 和 failure quarantine。
- Product Assembly 只注入 host facade、trust policy、adapter set 和 typed availability；具体 runtime、worker、subprocess、source discovery、activation 和 package discovery 由 host 边界拥有。
- Host 只返回 descriptor、provider candidate 或 `PluginEffectCandidate`；所有可写效果必须重新进入 Tool ABI、permission/effect gate、安全控制面和能力 owner。
- Desktop 和 CLI 至少有明确 host availability；ACP 在 P0 只允许 status-only、projection-only 或 typed unsupported，不接入 command/effect host 闭环；Web / Mobile Web / Server / Remote / SDK 必须返回 typed unsupported、unavailable 或 projection-only。

准出：

- Host facade 不暴露具体生态 adapter 类型、UI implementation handle、Tauri handle、full `RuntimeServices` bundle、`bitfun-core/product-full` 或 raw `serde_json::Value` 稳定 ABI。
- 必须在同一 PR 或同一 feature-gated integration 中绑定真实 OpenCode-compatible plugin consumption；未绑定 consumer 的 Host 只能存在于私有模块或明确禁用的 feature 下，不得接入生产路径、不得暴露 public API、不得计入阶段完成。
- disabled、projection-only、host unavailable、host failure、dispose、deadline 和 permission/effect 具体测试通过，PR 中列出测试路径。
- 默认不开放无约束可写 transform、无约束 localhost API 或插件直接调用内部 service manager。

### Stage P0-C：OpenCode-compatible plugin 首条消费路径

目标：接入 OpenCode-compatible plugin 最小路径，证明插件生态合同能支撑实际能力。ACP 外部 agent/tool bridge 属于 P0+，可复用合同但不是本阶段替代方案。

范围：

- 建立 OpenCode-compatible adapter source discovery 和 support matrix，只声明当前支持能力，不支持能力返回 typed unsupported。
- 支持从 `opencode.json` plugin package list 和 `.opencode/plugins/*.js|ts` 本地插件源发现 OpenCode 兼容来源，完成 source/trust 校验、启用、禁用和配置错误诊断。
- 将外部 tool、event、permission、workspace/worktree、remote path、artifact ref 映射为 BitFun canonical contract。
- 绑定 canonical P0 消费方：Desktop settings entry + command contribution，command 调用 plugin-provided tool，并进入 permission/effect gate；用户确认后由 owner materialize effect 并产出可见结果。
- CLI 至少提供同一插件的 source/status/config diagnostics；hook、readonly state view 和额外 UI slot 属于可选扩展，未实现时返回 typed unsupported。

准出：

- adapter 不依赖 `bitfun-core/product-full`、full `RuntimeServices` bundle、UI implementation 或 concrete provider handle。
- adapter、permission/effect、event manifest、UI contribution 和产品形态具体测试通过，PR 中列出测试路径。
- PR 说明支持矩阵、未支持能力、降级方式和安全影响。

P0 产品验收指标：

- 插件可从 `opencode.json` package list 和 `.opencode/plugins/*.js|ts` 本地源注册，并在 Desktop settings / command entry 和 CLI diagnostics 中被发现；启用、禁用、trust 确认、配置校验、source 校验失败、host unavailable、deadline、failure quarantine 都有可诊断状态。
- 最小 diagnostics 字段包括 plugin id/source、trust/config validation result、source/config validation error、host availability reason、deadline/quarantine reason，并稳定输出至少一个可与 Desktop artifact/status 对齐的 correlation id 或 event id。
- 必须提供一个 canonical OpenCode-compatible fixture plugin：`opencode.json` package list、本地 plugin source、settings 插件卡片、command entry、plugin-provided tool、一个无害可见 effect/artifact、confirm happy path、deny/no-side-effect path 和 CLI diagnostics/audit 输出。
- command contribution 可被用户发现并触发 plugin-provided tool，且必须走同一 OpenCode-compatible plugin 垂直切片。
- canonical happy path 必须闭环：OpenCode-compatible plugin command -> plugin-provided tool -> permission confirm -> owner materialize effect -> Desktop 可见结果/artifact/status -> CLI diagnostics/audit 可追踪。
- permission ask 支持确认和拒绝；确认面板必须展示 plugin id/source/hash、requested capability/effect、target/artifact、risk level、owner、可回滚性、deny 后状态和 audit/event id；拒绝、超时、policy-denied 和 host failure 都不会写 kernel state、audit success 或 tool result。
- `PluginEffectCandidate` 有审计记录、owner 裁决、回滚语义和 diagnostics；被拒绝时不产生最终副作用，被确认并 materialize 后必须能追踪 owner、artifact/status 和 audit/event id。
- failure quarantine 必须定义 scope、清除条件和用户可执行恢复动作；Desktop settings 插件卡片必须显示 scope、原因、日志入口和合法动作，CLI diagnostics 必须输出对应 action hint、correlation/event id，例如 retry、disable、retrust、open log 或 clear quarantine。
- P0 必选面只含 Desktop settings/command 和 CLI diagnostics。ACP 在 P0 只允许 canonical availability/diagnostics projection、status-only 或 typed unsupported，不参与 command/effect 闭环；Web / Mobile Web / Server / Remote / SDK 返回 typed unsupported、unavailable 或 projection-only。
- 原生 MCP 能力不因 P0 插件路径被迁移或重复实现；只有插件贡献的 MCP/tool provider 进入 Plugin Runtime Host。

## 5. 后端复杂度整改清单

以下清单来自当前代码审视，用于约束后续实现，不表示本次文档变更已经完成代码整改。

| 优先级 | 问题 | 证据 | 整改方向 |
|---|---|---|---|
| P0 | ACP 入口仍直接绑定 `bitfun-core/product-full`，协议入口会被完整产品运行时牵引 | `src/crates/interfaces/acp/Cargo.toml`、`src/crates/interfaces/acp/src/runtime.rs`、`src/crates/interfaces/acp/src/client/manager.rs` | 定义或复用 agent/session/tool/config/process stable ports，由 Product Assembly 注入实现；目标是让 ACP 从 `ProductFullCompatibility` 收敛到 `NoDirectCoreDependency` |
| P0 | Plugin Runtime Contract 过薄，`PluginDispatchEnvelope` / `PluginResponseEnvelope` 仍像长期 JSON ABI | `src/crates/contracts/runtime-ports/src/lib.rs` | 在真实 Host 前补 typed contract：extension point、source、capability、deadline、epoch、data category、side effects、idempotency、diagnostics、effect candidate |
| P1 | `bitfun-core` facade 仍是事实上的大入口 | `src/crates/assembly/core/src/lib.rs`、`src/crates/assembly/core/Cargo.toml` | 建立 facade export allowlist；新调用方禁止依赖 `bitfun_core::agentic::*` / `service::*`；每个 re-export 写明 owner、迁移目标和删除条件 |
| P1 | LSP/Git/service facade 是典型 A -> B -> C 薄透传 | `src/crates/assembly/core/src/service/lsp/**`、`src/crates/assembly/core/src/service/git/**` | 旧 import 可兼容保留，新代码直接依赖 `bitfun_core_types::lsp`、`bitfun_services_core::lsp`、`bitfun_services_integrations::git` 或更窄 port |
| P1 | `runtime-ports` 单文件合同过宽 | `src/crates/contracts/runtime-ports/src/lib.rs` | 先拆模块而非必拆 crate：plugin、agent_session、remote、tool_provider、events、session_store、service_capability；新增插件合同不得继续堆到单文件 |
| P1 | Product capability 与 tool pack feature group 双重建模 | `src/crates/assembly/product-capabilities/src/lib.rs`、`src/crates/execution/tool-provider-groups/src/lib.rs` | 短期保留 provider group id 作为 assembly 选择边界；长期提升唯一 stable capability fact，避免 product/tool/extension 三套 taxonomy |
| P2 | API adapter 层仍直接做文件 IO | `src/crates/adapters/api-layer/src/handlers.rs` | handler 只接收 FileSystem/Workspace port 或 service adapter；文件副作用下沉到 services owner |

## 6. 后续收敛阶段

### Stage D1：关键路径 Concrete Owner 收敛

目标：继续把插件主线和关键产品路径上的 concrete owner 从 `bitfun-core` / 产品命令路径收口到对应 owner crate。

范围：

- process/session host adapter、SDK-facing concrete provider 选择、DeepReview / prompt-cache / product command host adapter、extension host adapter 等仍由 core 持有的产品耦合 I/O owner。
- Product Assembly 负责选择 provider；Kernel、Execution、Product Feature 和 Extension Contract 不直接依赖 platform concrete。
- 每次迁移必须有旧路径删除、兼容 facade 收窄或调用链缩短的证据。

准出：

- 至少完成一个可证明的 owner 迁移或薄 facade 删除。
- `cargo check --workspace`、`cargo check -p bitfun-core --no-default-features` 或更小的 focused Rust check 按影响范围通过。
- 边界脚本没有新增 core 回流。

### Stage D2：链式依赖与 facade 瘦身

目标：针对 A -> B -> C 的纯透传路径做专项整改，让调用方依赖真实稳定接口而不是层层转发。

范围：

- 盘点 core facade、product command facade、runtime service facade、adapter facade、extension facade 和 frontend API wrapper。
- 对没有策略、校验、缓存、多实现选择、反腐、host 边界或兼容责任的薄层，删除或让调用方直接依赖 contract/port。
- 保留的 facade 必须写明责任：兼容门面、协议反腐、多实现选择、Product Assembly、Plugin Runtime Host 或迁移期临时层。

准出：

- 每个改动都能说明减少了哪条依赖链或删除了哪条旧路径。
- 不引入新的全局 mutable registry、untyped service locator 或无消费方 descriptor。

### Stage D3：内部 SDK minimal 与产品形态保护

目标：验证 Agent Runtime 的最小嵌入边界不会牵引完整产品实现，同时不扩大为公开 SDK 发布项目。

范围：

- fake-provider smoke、minimal feature、cargo metadata/tree 对比和 API version 保护。
- ProductFull、Desktop、CLI、ACP 保持完整能力；Web / Mobile Web / Server / Remote / SDK 显式 unavailable / unsupported / projection。
- 插件 runtime binding 覆盖 disabled / projection-only / host availability 的形态保护。

准出：

- minimal smoke 不依赖 `bitfun-core/product-full`、Desktop、Tauri、Git provider、MCP client、AI HTTP client、remote SSH 或产品 UI。
- 产品形态检查能证明非完整入口不会默默继承完整桌面或插件能力。

## 7. 固定执行流程

1. 同步最新 `main`，检查主干新增的 CLI、tool、terminal、session、scheduler、remote、MiniApp、ACP、plugin 或 product interface 变更。
2. 对照 `product-architecture.md` 明确本次 owner 边界，不从旧计划标签继承完成判断。
3. 插件主线变更先明确 extension point、host boundary、candidate effect、安全裁决和真实消费路径。
4. 先补等价保护和 boundary guard，再迁移实现主体。
5. 删除、迁移或显著简化 core 中对应旧路径。
6. 运行 focused verification、boundary check 和必要的 feature / dependency / product-shape 对比。
7. 从独立第三方角度审查功能漂移、性能劣化、依赖回流、产品形态遗漏、安全绕过和文档一致性。
8. 合入后只更新 completed 摘要和 issue 状态；设计文档只有目标架构变更时才修改。

## 8. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| Workspace layout / Cargo path | `cargo metadata --no-deps --format-version 1` |
| Extension Contract / descriptor / availability | `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`；同时更新 crate-local public API budget/allowlist 或 `scripts/core-boundaries/rules/**`，并让 `scripts/check-core-boundaries.mjs` 阻止 raw JSON ABI、无 consumer public descriptor/envelope 和 product-full 回流；最低覆盖 descriptor round-trip、availability 映射、candidate effect 拒绝路径 |
| Plugin Runtime Host | 固定目标为 `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`；若 host 代码进入本阶段，必须新增并运行 `cargo test -p bitfun-runtime-ports --test plugin_runtime_host_contracts` 或 host owner crate 的同名 contract test，并在本矩阵落定命令；最低覆盖 lifecycle、dispatch envelope、deadline、dispose、failure quarantine、permission prompt/diagnostic serialization、permission/effect gate；缺失固定目标本身就是准出阻断 |
| OpenCode-compatible adapter fixture contract | 固定 fixture 目标为 `cargo test -p bitfun-opencode-adapter opencode_fixture_contracts`；若 adapter owner crate 使用不同名称，同一 PR 必须先更新本矩阵并提供确定命令；最低覆盖真实 `opencode.json` plugin package discovery、真实 `.opencode/plugins/*.js\|ts` local plugin source discovery、valid-fixture config state、trust projection、npm package projection-only diagnostic、unsupported hook diagnostic / typed candidate、invalid config/source rejection before projection、custom tool provider candidate 和 permission prompt candidate；该命令只证明真实 OpenCode 输入形态的 discovery / projection 合同，不等同于 P0 完成 |
| OpenCode-compatible product vertical slice | 后续 Host / Desktop / CLI 消费 PR 必须在 PR 内落定 owner crate、入口文件和固定 P0 目标命令；最低候选范围是 `src/crates/contracts/runtime-ports/tests/plugin_runtime_host_contracts.rs` 承接 Host lifecycle/effect gate 合同、Host owner crate 增加 `opencode_product_vertical_slice` 同名测试、Desktop settings/command 入口增加 smoke 或 focused test、CLI diagnostics/audit 增加 focused test；最低覆盖 Desktop settings/command、CLI diagnostics/audit、UI contribution fallback、confirm happy path、deny/no-side-effect path 和 owner materialize effect；缺失固定目标本身就是准出阻断，临时测试或 PR 文案不能替代该目标 |
| Product Feature / capability availability | `cargo test -p bitfun-product-capabilities`；若能力集合或 availability 变化，补对应 product capability 测试 |
| Agent Kernel / permission / event | `cargo test -p bitfun-agent-runtime`，`cargo check -p bitfun-core --no-default-features` |
| Runtime Services / backend events | `cargo test -p bitfun-runtime-services`；事件投递变化时补具体测试路径 |
| Tool / MCP / terminal / sandbox | `cargo test -p bitfun-agent-tools`，`cargo test -p tool-runtime`；terminal / exec-command / MCP 变化时补具体测试路径 |
| Harness / Product Domains | `cargo test -p bitfun-harness`，`cargo test -p bitfun-product-domains`；DeepReview / MiniApp 变化时补具体测试路径 |
| Product shape / internal SDK minimal | 固定目标为 `cargo test -p bitfun-product-capabilities --test plugin_product_shape`、`cargo test -p bitfun-product-capabilities --test product_sdk_assembly`、`cargo metadata --no-deps --format-version 1`；覆盖 Desktop / CLI / ACP / Web / Server / Remote / SDK / Mobile Web capability availability 和 SDK fake-provider smoke，证明非 P0 入口不是 full plugin runtime 且 SDK minimal 不牵引 `product-full` / concrete provider；若 `plugin_product_shape` 尚不存在，同一 PR 必须新增该路径并在本矩阵落定命令 |
| 大范围 owner 迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |

## 9. 暂停条件

- 新 owner crate 必须依赖回 `bitfun-core` 才能编译或测试。
- Agent Kernel 吸收 product feature、UI state、Tauri、产品命令、AI provider、MCP client、process execution、Git provider、具体 plugin adapter 等 concrete dependency。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- Plugin Runtime Host 直接写 permission、audit、kernel state、tool result 或 UI implementation。
- Compatibility Adapter 直接依赖 `bitfun-core/product-full`、full `RuntimeServices`、Tauri handle、React component 或 concrete provider handle。
- PR 只新增抽象，没有迁移、删除、真实消费路径或显著简化旧 core 主体路径。
- 新增 public plugin descriptor、envelope、host API 或 availability API，但没有绑定 OpenCode-compatible plugin 第一条体验。
- 新增 public plugin descriptor、envelope、host API 或 availability API，只在 PR 正文说明 owner/consumer/P0 trace/wire impact/retirement condition，没有落入 crate-local budget/allowlist 或边界脚本可检查规则。
- Plugin Runtime Contract 把 raw `serde_json::Value` 作为长期稳定 ABI，或没有携带 source、capability、deadline、epoch、side effects、diagnostics 等安全事实。
- ACP 外部 agent/tool bridge 被当作 P0 插件体验替代方案，而不是 P0+ 互操作路径。
- SDK facade 必须暴露 `bitfun-core`、`product-full`、concrete service manager 或产品命令 registry 才能完成基本 agent 执行。
- 全量 UI Extension 矩阵、全量生态兼容或无约束可写 transform 在没有产品场景、安全评审和 focused verification 前进入当前 PR。
