# BitFun Core 拆解与运行时迁移计划

本文件维护后续执行计划。稳定目标以
[`product-architecture.md`](../architecture/product-architecture.md) 为准；
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md) 补充接口与 crate 约束；
[`plugin-runtime-host-design.md`](../architecture/plugin-runtime-host-design.md) 是插件运行时和生态兼容的当前主线详细设计。
已完成事实归档在 [`core-decomposition-completed.md`](core-decomposition-completed.md)。

本计划文件名继续保留 `core-decomposition`，因为它记录的是 `bitfun-core` 收敛和归属迁移的执行路径。

## 1. 执行原则

- 当前第一优先级是插件生态和扩展能力支撑：扩展契约、插件运行时主机、候选效果、安全校验和真实生态适配消费路径必须优先闭环。P0 固定首条体验是 OpenCode 兼容插件最小垂直切片，必须覆盖来源注册/启用、桌面命令/设置入口、权限确认、副作用物化和 CLI 诊断；ACP 外部智能体/工具桥接只能作为 P0+ 互操作路径，不能替代该验收。
- 同时保护关键产品路径：ProductFull、Desktop、CLI、ACP，以及 Web / Mobile Web / Server / Remote / SDK 的显式降级或投影。
- `bitfun-core` 最终收敛为兼容门面、`product-full` 组装边界和少量迁移期适配器。
- 产品组装是组装根；除它以外，普通层级只能依赖稳定契约、端口、描述符或被注入的类型化部件。
- 新抽象必须同步删除、迁移或显著简化旧 core 主体路径；纯门面、纯保护、纯文档、纯描述符、纯注册表或空接口不得作为完成条件。
- 中间层只有在具备当前消费方、稳定契约、版本/兼容证明、边界测试或准入清单、退场条件时才能长期保留；整改目标是降低核心接口受实现变动影响的频率，而不是机械缩短依赖路径。
- 产品特性和内核能力分开：长程任务、调度、权限、上下文、session/workspace、memory、DFX、hook/event 属于 Agent Kernel；
  `/goal`、UI、settings、命令和默认策略属于 Product Feature。
- 全量生态兼容、全入口 UI 扩展矩阵、任意可写转换和无约束插件运行时不进入当前阶段范围；插件生态主线仍为第一优先级。
- 默认保持权限、工具 schema、事件语义、会话生命周期、远端行为、MiniApp 行为和交付形态等价；若 P0 插件体验需要主动改变，必须有产品决策记录、用户影响、迁移/回滚、指标和验证。

## 2. 执行输入假设

已完成事实以 [`core-decomposition-completed.md`](core-decomposition-completed.md) 为准。本文件记录后续执行需要依赖的输入假设：

- workspace 已按 `interfaces -> assembly -> adapters -> services -> execution -> contracts` 物理目录展开，但概念归属仍需通过当前迁移继续收敛。
- Desktop、CLI、ACP 当前仍通过 `bitfun-core/product-full` 获取完整产品能力；P0 插件体验不能继续把这个状态固化为新入口依赖。
- 工具 ABI、运行时服务、智能体运行时、产品能力和插件 `disabled` / `projection-only` 基础边界已存在；OpenCode 兼容适配器测试夹具契约开始基于真实 OpenCode 配置 / 本地插件来源形态验证发现 / 投影映射，但真实插件运行时主机、Desktop/CLI 消费路径和候选效果闭环仍未完成。
- 边界脚本可用于归属防回流、六层路径、仅门面文件和重点特性开关，但插件 P0 需要补充更具体的主机 / 扩展 / 适配器检查。

## 3. 当前目标差距

| 差距 | 影响 | 当前收敛要求 |
|---|---|---|
| 扩展契约尚未形成最小闭环 | 插件、插件贡献的工具提供方、ACP 外部智能体、钩子、界面贡献缺少统一入口 | 围绕 OpenCode 兼容插件第一条体验定义扩展点、描述符、可用性、信任/来源、候选效果和回退的最小稳定契约 |
| 插件运行时主机缺少受控边界 | 插件能力只能表达 `disabled` / `projection-only`，不能受控加载或投影外部贡献 | 建立主机生命周期、分发信封、截止时间、诊断、失败隔离和 dispose 语义 |
| 真实生态适配缺少消费路径 | OpenCode 兼容插件能力无法验证契约是否可用 | 建立 OpenCode 兼容插件最小适配链路；ACP 外部智能体/工具桥接可复用契约但不替代 P0 验收 |
| 部分具体归属模块仍在 core 或产品命令路径 | 层级依赖和平台差异仍可能回流 | 只迁移与插件主线或关键产品路径直接相关的归属模块，并同步删除或显著简化旧路径 |
| 部分门面责任不清 | 调用方不清楚某层是在做兼容、反腐、聚合、选择还是单纯转发，接口稳定性无法评估 | 为每个保留门面写明归属模块、消费方、稳定契约、兼容范围和退场条件；无消费方或无归属模块的门面不继续扩张 |
| 内部 SDK 最小可用性未闭环 | 独立 Agent Runtime 可能牵引 product-full 或具体提供方 | 验证内部测试替身提供方、最小特性、cargo tree/metadata 对比和 API 版本保护 |

当前阶段之外的目标：

- 所有生态的一次性完整兼容。
- 所有入口的一次性完整 UI Extension 矩阵。
- 插件直接覆写内置能力。
- 任意可写转换、无限制 JS/TS 运行时和无约束 localhost API。
- 对外稳定 SDK 发布。

这些目标重新进入执行范围前，必须同时满足：有明确产品场景、已有真实消费路径、能删除或简化旧路径、完成安全评审，并补充聚焦验证。

P0-A、P0-B、P0-C 不是三个可独立交付的抽象阶段。它们必须围绕同一个 OpenCode 兼容插件垂直切片推进：

- 任何新增公开描述符、信封、主机门面或可用性 API，必须绑定该真实消费路径。
- 为降低 PR 风险而先交付的内部实现不得作为稳定公开 API 暴露，也不得要求其他模块适配空注册表。
- P0 的验收以同一条规范场景轨迹为准，而不是以单独 crate 编译、描述符存在、主机门面可构建或任一单点消费路径为准。

## 4. 后续执行阶段

### 阶段 P0-A：扩展契约最小闭环

目标：OpenCode 兼容插件、插件贡献的自定义工具 / 钩子、界面贡献和候选效果共享同一套最小契约；原生 MCP 继续走执行层 + 平台适配器。

范围：

- 定义 `ExtensionPoint`、能力/副作用描述符、信任/来源、数据类别、执行域、可用性、回退和 `unsupported` / `temporarily-unavailable` 类型化错误。
- 定义 `PluginEffectCandidate` 的权限、副作用、审计、回滚和归属模块裁决语义。
- 定义最小 `UiContributionDescriptor`，只覆盖当前消费方需要的槽位 / 命令 / 设置入口 / 只读状态视图。
- 将已有 `disabled` / `projection-only` 插件绑定与扩展可用性对齐。
- 定义插件运行时可用性到产品状态的映射，避免运行时绑定、入口状态和能力可用性各自新增 enum。

验收条件：

- 必须绑定同一条 OpenCode 兼容插件规范场景轨迹：OpenCode 配置 / 本地插件来源、描述符、提供方候选、候选效果、权限/副作用门禁和产品可见状态都服务该端到端体验；不允许只新增注册表或用单点消费路径宣布 P0-A 完成。
- 不暴露 React 组件、Tauri 句柄、运行时服务管理器、具体生态对象或无类型 `Any`。
- 新增或更新具体测试目标，并在 PR 中列出路径；测试至少覆盖描述符往返、`unsupported` / `temporarily-unavailable`、候选效果拒绝路径和可用性映射。

### 阶段 P0-B：插件运行时主机受控边界

目标：建立受控插件主机边界和候选分发语义，让产品组装与 AgentRuntime 能接收契约校验后的主机绑定；P0-B 不交付 Desktop/CLI 插件消费、来源发现、激活或副作用物化，插件也不能成为内核、权限、审计、工具结果或界面实现的权威源。

范围：

- 定义主机生命周期、分发信封校验、截止时间、epoch、幂等性、诊断、dispose、失败隔离和 `HostRestarted` 清理路径；清单/来源发现与激活属于 P0-C / 主机监控器后续范围。
- 产品组装只接受或注入经过契约校验的类型化主机绑定与可用性事实；具体运行时、worker、subprocess、适配器集合、来源发现、激活和包发现由 P0-C 主机监控器 / 主机边界拥有。
- P0-B 主机只返回提供方候选、诊断、隔离和状态/读模型投影；所有可写效果必须重新进入工具 ABI、权限/副作用门禁、安全控制面和能力归属模块。描述符 / 界面投影属于后续真实消费路径。
- 产品组装 / 主机测试夹具层必须有明确主机可用性事实；桌面设置入口、桌面命令入口和 CLI 诊断均属于 P0-C 或后续消费 PR。ACP 在 P0 只允许 `status-only`、`projection-only` 或 `unsupported` 类型化错误，不接入命令/副作用主机闭环；Web / Mobile Web / Server / Remote / SDK 必须返回 `unsupported`、`temporarily-unavailable` 或 `projection-only`。

验收条件：

- 主机门面不暴露具体生态适配器类型、界面实现句柄、Tauri 句柄、完整 `RuntimeServices` bundle、`bitfun-core/product-full` 或原始 `serde_json::Value` 稳定 ABI。
- 必须在同一 PR 或同一受特性开关保护的集成中绑定当前阶段的真实消费方：产品组装只能注入契约校验后的类型化主机绑定，AgentRuntimeBuilder 必须能接收并保留该绑定，主机归属 crate 必须验证适配器边界分发/读取/隔离 schema、主机诊断和活跃隔离默认拒绝行为。真实 OpenCode 兼容 Desktop/CLI 消费属于 P0-C；P0-B 不得把仅有主机边界宣称为桌面设置、主机命令入口、CLI 诊断、用户可见恢复动作或副作用物化已完成。
- `disabled`、`projection-only`、主机 `temporarily-unavailable`、主机失败、dispose、deadline 和权限/副作用具体测试通过，PR 中列出测试路径。
- 默认不开放无约束可写转换、无约束 localhost API 或插件直接调用内部服务管理器。

### 阶段 P0-C：OpenCode 兼容插件首条消费路径

目标：接入 OpenCode 兼容插件最小路径，证明插件生态契约能支撑实际能力。ACP 外部智能体/工具桥接属于 P0+，可复用契约但不是本阶段替代方案。

范围：

- 建立 OpenCode 兼容适配器来源发现和支持矩阵，只声明当前支持能力，不支持能力返回 `unsupported` 类型化错误。
- 支持从 `opencode.json` 插件包列表和 `.opencode/plugins/*.js|ts` 本地插件源发现 OpenCode 兼容来源，完成来源/信任校验、启用、禁用和配置错误诊断。
- 将外部工具、事件、权限、工作区/worktree、远端路径、产物引用映射为 BitFun 规范契约。
- 绑定规范 P0 消费方：桌面设置入口 + 主机命令入口；该命令调用插件提供的自定义工具 / 提供方候选，并进入权限/副作用门禁；用户确认后由归属模块物化副作用并产出可见结果。
- CLI 至少提供同一插件的来源/状态/配置诊断；钩子、只读状态视图和额外 UI 槽位属于可选扩展，未实现时返回 `unsupported` 类型化错误。

验收条件：

- 适配器不依赖 `bitfun-core/product-full`、完整 `RuntimeServices` bundle、界面实现或具体提供方句柄。
- 适配器、权限/副作用、事件清单、界面贡献和产品形态具体测试通过，PR 中列出测试路径。
- PR 说明支持矩阵、未支持能力、降级方式和安全影响。

P0 产品验收指标：

- 插件可从 `opencode.json` 包列表和 `.opencode/plugins/*.js|ts` 本地源注册，并在桌面设置 / 命令入口和 CLI 诊断中被发现；启用、禁用、信任确认、配置校验、来源校验失败、主机 `temporarily-unavailable`、deadline 和失败隔离都有可诊断状态。
- 最小诊断字段包括插件 id/来源、信任/配置校验结果、来源/配置校验错误、主机可用性原因、截止时间/隔离原因，并稳定输出至少一个可与 Desktop 产物/状态对齐的关联 id 或事件 id。
- 必须提供一个规范 OpenCode 兼容测试插件：`opencode.json` 包列表、本地插件来源、settings 插件卡片、主机命令入口、插件提供的自定义工具 / 提供方候选、一个无害可见副作用/产物、确认路径、拒绝且无副作用路径和 CLI 诊断/审计输出。
- 主机命令入口可被用户发现，并触发插件提供的自定义工具 / 提供方候选，且必须走同一 OpenCode 兼容插件垂直切片。
- 规范成功路径必须闭环：BitFun 主机插件命令 -> 插件提供的自定义工具 / 提供方候选 -> 权限确认 -> 归属模块物化副作用 -> Desktop 可见结果/产物/状态 -> CLI 诊断/审计可追踪。
- 权限提示支持确认和拒绝；确认面板必须展示插件 id/来源/hash、请求能力/副作用、目标/产物、风险等级、归属模块、可回滚性、拒绝后状态和审计/事件 id；拒绝、超时、`policy-denied` 和主机失败都不会写内核状态、审计成功或工具结果。
- `PluginEffectCandidate` 有审计记录、归属模块裁决、回滚语义和诊断；被拒绝时不产生最终副作用，被确认并物化后必须能追踪归属模块、产物/状态和审计/事件 id。
- 失败隔离必须定义范围、清除条件、诊断引用和审计引用；P0-B 不提供用户可执行恢复动作，只验证 `HostRestarted` 清除条件、主机内部重启清理、读模型投影和后续分发默认拒绝行为。任何用户可执行动作，包括清除隔离、重试、禁用、重新信任和打开日志，都必须等 P0-C / P0+ 有归属模块支持的恢复端口、审计事实与真实消费方后再暴露。
- P0 必选面只包含桌面设置/命令入口和 CLI 诊断。ACP 在 P0 只允许规范可用性/诊断投影、`status-only` 或 `unsupported` 类型化错误，不参与命令/效果闭环；Web / Mobile Web / Server / Remote / SDK 返回 `unsupported`、`temporarily-unavailable` 或 `projection-only`。
- 原生 MCP 能力不因 P0 插件路径被迁移或重复实现；只有插件贡献的 MCP/工具提供方进入插件运行时主机。

## 5. 后端复杂度整改清单

以下清单来自当前代码审视，用于约束后续实现；本次文档变更不代表相关代码整改已经完成。

| 优先级 | 问题 | 证据 | 整改方向 |
|---|---|---|---|
| P0 | ACP 入口仍直接绑定 `bitfun-core/product-full`，协议入口会被完整产品运行时牵引 | `src/crates/interfaces/acp/Cargo.toml`、`src/crates/interfaces/acp/src/runtime.rs`、`src/crates/interfaces/acp/src/client/manager.rs` | 定义或复用智能体/会话/工具/配置/进程稳定端口，由产品组装注入实现；目标是让 ACP 从 `ProductFullCompatibility` 收敛到 `NoDirectCoreDependency` |
| P0 | 插件运行时契约过薄，`PluginDispatchEnvelope` / `PluginResponseEnvelope` 仍像长期 JSON ABI | `src/crates/contracts/runtime-ports/src/lib.rs` | 在真实主机前补类型化契约：扩展点、来源、能力、截止时间、epoch、数据类别、副作用、幂等性、诊断、候选效果 |
| P1 | `bitfun-core` 门面仍是事实上的大入口 | `src/crates/assembly/core/src/lib.rs`、`src/crates/assembly/core/Cargo.toml` | 建立门面导出准入清单；新调用方禁止依赖 `bitfun_core::agentic::*` / `service::*`；每个 re-export 写明归属模块、迁移目标和删除条件 |
| P1 | LSP/Git/service 门面缺少稳定性说明 | `src/crates/assembly/core/src/service/lsp/**`、`src/crates/assembly/core/src/service/git/**` | 旧 import 可兼容保留，但必须标明门面是兼容层、反腐层还是迁移层；新稳定能力优先落到归属 crate 的契约/端口，并给出版本和退场策略 |
| P1 | `runtime-ports` 单文件合同过宽 | `src/crates/contracts/runtime-ports/src/lib.rs` | 先拆模块而非必拆 crate：plugin、agent_session、remote、tool_provider、events、session_store、service_capability；新增插件合同不得继续堆到单文件 |
| P1 | Product capability 与 tool pack feature group 双重建模 | `src/crates/assembly/product-capabilities/src/lib.rs`、`src/crates/execution/tool-provider-groups/src/lib.rs` | 短期保留提供方组 id 作为组装选择边界；长期提升唯一稳定能力事实，避免 product/tool/extension 三套分类 |
| P2 | API 适配器层仍直接做文件 IO | `src/crates/adapters/api-layer/src/handlers.rs` | handler 只接收 FileSystem/Workspace 端口或服务适配器；文件副作用下沉到服务归属模块 |

## 6. 后续收敛阶段

### 阶段 D1：关键路径具体归属收敛

目标：继续把插件主线和关键产品路径上的具体归属从 `bitfun-core` / 产品命令路径收口到对应归属 crate。

范围：

- 进程/会话主机适配器、SDK 面向的具体提供方选择、DeepReview / prompt-cache / product command 主机适配器、扩展主机适配器等仍由 core 持有的产品耦合 I/O 归属。
- 产品组装负责选择提供方；内核、执行层、产品特性和扩展契约不直接依赖平台具体实现。
- 每次迁移必须有旧路径删除、兼容门面责任收窄，或实现变更没有外溢到能力服务接口契约 / 插件 API 的证据。

验收条件：

- 至少完成一个可证明的归属迁移、旧路径删除，或门面责任收窄并补齐归属模块 / 消费方 / 契约 / 验证 / 退场记录。
- `cargo check --workspace`、`cargo check -p bitfun-core --no-default-features` 或更小的 focused Rust check 按影响范围通过。
- 边界脚本没有新增 core 回流。

### 阶段 D2：稳定接口预算与门面责任收敛

目标：让高频实现变更停留在归属 crate、适配器、兼容门面或产品组装内部，降低能力服务接口契约和扩展契约的变更频率。

范围：

- 盘点 core 门面、产品命令门面、运行时服务门面、适配器门面、扩展门面和前端 API 包装层的稳定职责。
- 对每个保留门面记录归属模块、消费方、稳定契约、版本策略、兼容范围、可替换实现和退场条件。
- 对没有归属模块、没有消费方、没有版本/兼容职责的层，删除、降级为内部模块，或标记为迁移期临时层。
- 对能力服务接口契约和扩展契约新增字段时，必须说明是版本化新增、只读投影、实验字段还是稳定语义。

验收条件：

- 每个改动都能说明保护了哪个稳定契约，或减少了哪个实现变更对产品入口/插件 API 的影响。
- 不引入新的全局可变注册表、无类型服务定位器、无消费方描述符或没有归属模块的长期公开 API。

### 阶段 D3：内部 SDK 最小可用性与产品形态保护

目标：验证 Agent Runtime 的最小嵌入边界不会牵引完整产品实现，同时不扩大为公开 SDK 发布项目。

范围：

- 测试替身提供方基础验证、最小特性、cargo metadata/tree 对比和 API 版本保护。
- ProductFull、Desktop、CLI、ACP 保持完整能力；Web / Mobile Web / Server / Remote / SDK 显式 `temporarily-unavailable` / `unsupported` / `projection-only`。
- 插件运行时绑定覆盖 `disabled` / `projection-only` / 主机可用性的形态保护。

验收条件：

- 最小可用性基础验证不依赖 `bitfun-core/product-full`、Desktop、Tauri、Git 提供方、MCP 客户端、AI HTTP 客户端、remote SSH 或产品 UI。
- 产品形态检查能证明非完整入口不会隐式继承完整桌面或插件能力。

## 7. 固定执行流程

1. 同步最新 `main`，检查主干新增的 CLI、工具、终端、会话、调度、远端、MiniApp、ACP、插件或能力服务接口契约变更。
2. 对照 `product-architecture.md` 明确本次归属边界，不从旧计划标签继承完成判断。
3. 插件主线变更先明确扩展点、主机边界、候选效果、安全裁决和真实消费路径。
4. 先补等价保护和边界保护，再迁移实现主体。
5. 删除、迁移或显著简化 core 中对应旧路径。
6. 运行聚焦验证、边界检查和必要的特性 / 依赖 / 产品形态对比。
7. 从独立第三方角度审查功能漂移、性能劣化、依赖回流、产品形态遗漏、安全绕过和文档一致性。
8. 合入后只更新 completed 摘要和 issue 状态；设计文档只有目标架构变更时才修改。

## 8. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| Workspace layout / Cargo path | `cargo metadata --no-deps --format-version 1` |
| 能力服务接口契约 / 多入口投影 | 仅文档变更固定目标为 `pnpm run check:repo-hygiene`、`node scripts/check-core-boundaries.mjs`；首个新增能力服务接口 DTO / 入口适配器 / 投影的实现 PR 必须确定归属 crate、入口文件和可执行固定验证目标，并在同 PR 更新本矩阵后运行该目标，同时补最近的前端 / Rust 聚焦测试；最低覆盖归属模块、规范 DTO/读模型、稳定状态词、内部绑定 -> Server/API 状态映射、版本策略、入口映射、类型化错误、事件流，以及插件描述符/状态到插件状态投影 / 界面贡献投影的投影边界；不得在计划中引用当前不存在的 crate/test 作为验收命令 |
| 扩展契约 / 描述符 / 可用性 | `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`；同时更新 crate-local 公开 API 预算/准入清单或 `scripts/core-boundaries/rules/**`，并让 `scripts/check-core-boundaries.mjs` 阻止原始 JSON ABI、无消费方公开描述符/信封和 product-full 回流；最低覆盖描述符往返、可用性映射、候选效果拒绝路径 |
| 插件运行时主机 | 固定目标为 `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`、`cargo test -p bitfun-runtime-ports --test plugin_runtime_host_contracts` 和 `cargo test -p bitfun-plugin-runtime-host`；主机归属 crate 位于 `src/crates/execution/plugin-runtime-host`，只拥有可移植生命周期、分发幂等、截止时间诊断、dispose、失败隔离、HostRestarted 清理和诊断读模型投影；最低覆盖生命周期、分发信封、截止时间、dispose、活跃隔离阻断、重启清理、权限提示/诊断序列化、权限/副作用门禁；缺失固定目标本身构成验收阻断 |
| OpenCode 兼容适配器测试夹具契约 | 固定测试夹具目标为 `cargo test -p bitfun-opencode-adapter opencode_fixture_contracts`；若适配器归属 crate 使用不同名称，同一 PR 必须先更新本矩阵并提供确定命令；最低覆盖真实 `opencode.json` 插件包发现、真实 `.opencode/plugins/*.js\|ts` 本地插件来源发现、有效测试夹具配置状态、信任投影、npm 包 projection-only 诊断、unsupported 钩子诊断 / 类型化状态、无效配置/来源在投影前拒绝、自定义工具提供方候选和权限提示候选；该命令只证明真实 OpenCode 输入形态的发现 / 投影契约，不等同于 P0 完成 |
| OpenCode 兼容产品垂直切片 | 后续主机 / Desktop / CLI 消费 PR 必须在 PR 内确定归属 crate、入口文件和固定 P0 目标命令；最低候选范围是 `src/crates/contracts/runtime-ports/tests/plugin_runtime_host_contracts.rs` 承接主机生命周期/副作用门禁契约、主机归属 crate 增加 `opencode_product_vertical_slice` 同名测试、桌面设置/命令入口增加基础验证或聚焦测试、CLI 诊断/审计增加聚焦测试；最低覆盖桌面设置/命令入口、CLI 诊断/审计、界面贡献回退、确认路径、拒绝且无副作用路径和归属模块物化副作用；缺失固定目标本身构成验收阻断，临时测试或 PR 文案不能替代该目标 |
| 产品特性 / 能力可用性 | `cargo test -p bitfun-product-capabilities`；若能力集合或可用性变化，补对应产品能力测试 |
| 智能体内核 / 权限 / 事件 | `cargo test -p bitfun-agent-runtime`，`cargo check -p bitfun-core --no-default-features` |
| 运行时服务 / 后端事件 | `cargo test -p bitfun-runtime-services`；事件投递变化时补具体测试路径 |
| Tool / MCP / terminal / sandbox | `cargo test -p bitfun-agent-tools`，`cargo test -p tool-runtime`；terminal / exec-command / MCP 变化时补具体测试路径 |
| Harness / Product Domains | `cargo test -p bitfun-harness`，`cargo test -p bitfun-product-domains`；DeepReview / MiniApp 变化时补具体测试路径 |
| 产品形态 / 内部 SDK 最小可用性 | 固定目标为 `cargo test -p bitfun-product-capabilities --test plugin_product_shape`、`cargo test -p bitfun-product-capabilities --test product_sdk_assembly`、`cargo metadata --no-deps --format-version 1`；覆盖 ProductFull / Desktop / CLI 可承载主机形态、ACP / Web / Server / Remote / SDK / Mobile Web 非 P0 可用性，以及 SDK 测试替身提供方基础验证，证明非 P0 入口不是完整插件运行时且内部 SDK 最小可用性不牵引 `product-full` / 具体提供方 |
| 大范围归属迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |

## 9. 暂停条件

- 新归属 crate 必须依赖回 `bitfun-core` 才能编译或测试。
- 智能体内核吸收产品特性、UI 状态、Tauri、产品命令、AI 提供方、MCP 客户端、进程执行、Git 提供方、具体插件适配器等具体依赖。
- 产品组装变成无类型服务定位器或全局可变 app 状态。
- 插件运行时主机直接写权限、审计、内核状态、工具结果或界面实现。
- 兼容适配器直接依赖 `bitfun-core/product-full`、完整 `RuntimeServices`、Tauri 句柄、React 组件或具体提供方句柄。
- PR 只新增抽象，没有迁移、删除、真实消费路径或显著简化旧 core 主体路径。
- 新增公开插件描述符、信封、主机 API 或可用性 API，但没有绑定 OpenCode 兼容插件第一条体验。
- 新增公开插件描述符、信封、主机 API 或可用性 API，只在 PR 正文说明归属模块/消费方/P0 轨迹/线缆影响/退场条件，没有落入 crate-local 预算/准入清单或边界脚本可检查规则。
- 插件运行时契约把原始 `serde_json::Value` 作为长期稳定 ABI，或没有携带来源、能力、截止时间、epoch、副作用、诊断等安全事实。
- ACP 外部智能体/工具桥接被当作 P0 插件体验替代方案，而不是 P0+ 互操作路径。
- SDK 门面必须暴露 `bitfun-core`、`product-full`、具体服务管理器或产品命令注册表才能完成基本智能体执行。
- 全量 UI 扩展矩阵、全量生态兼容或无约束可写转换在没有产品场景、安全评审和聚焦验证前进入当前 PR。
