# BitFun Core 拆解与运行时迁移计划

本文件维护后续执行计划。稳定目标以
[`product-architecture.md`](../architecture/product-architecture.md) 为准；
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md) 补充运行时和 crate 约束；
[`plugin-runtime-host-design.md`](../architecture/plugin-runtime-host-design.md) 定义插件主机内部 ABI。
已完成事实归档在 [`core-decomposition-completed.md`](core-decomposition-completed.md)。

## 1. 执行原则

- 插件生态和扩展能力仍是第一优先级，但优先级不等于无限扩接口。当前重点是最小稳定接口、受控主机边界和 OpenCode-compatible 关键场景。
- 产品组装是组装根；普通层级只能依赖稳定接口、端口、描述符或注入的类型化部件。
- 新抽象必须同步删除、迁移或显著简化旧路径；纯门面、空注册表、无消费方描述符或仅文档化的接口不得作为完成条件。
- 稳定接口优先保护实现频繁变更，不以机械缩短依赖路径为目标。
- 工具、事件和权限优先复用已有归属子接口，不在插件层重复建模。
- OpenCode 配置导入和 ACP 外部智能体/工具桥接只能作为兼容或互操作路径，不能替代 BitFun 插件来源主路径。
- 全量生态兼容、全入口 UI 扩展矩阵、任意可写转换、无约束 JS/TS runtime、无约束 localhost 接口和对外稳定 SDK 发布不进入当前阶段。

## 2. 当前输入假设

- workspace 已按 `interfaces -> assembly -> adapters -> services -> execution -> contracts` 物理目录展开，但概念归属仍需继续收敛。
- Desktop、CLI、ACP 仍有路径通过 `bitfun-core/product-full` 获取完整产品能力；后续插件主线不能把该状态固化为新入口依赖。
- 工具 ABI、事件清单、运行时服务、智能体运行时、产品能力和插件 `disabled` / `projection-only` 基础边界已存在。
- `runtime-ports` 的插件主机 ABI 已有公开接口预算脚本；后续不能绕过预算新增插件、hook、event、UI 或生态兼容对象。
- `opencode-adapter` 当前解释固定内容的受管包，提供诊断只读视图和 custom tool 候选映射；来源发现归 `services-integrations/plugin_source`。
- `services-integrations/plugin_source` 提供受管包发现、完整性校验、来源审核、激活持久化和实时凭证复核；`bitfun-core/plugin_runtime` 是唯一 OpenCode 生产组装点。
- CLI 已能预览、按精确内容哈希激活和停用包，并可在包缺失或损坏后清理残留激活记录。激活路径只通过 Plugin Runtime Host 返回需要权限的 custom tool 候选，不执行 JS/TS，也不依赖外部 OpenCode CLI。

## 3. 当前差距

| 差距 | 影响 | 收敛要求 |
|---|---|---|
| 激活写入在跨进程锁内执行稳定性复核 | 操作受统一期限约束，慢文件系统可能延长同一工作区的授权检查等待，但当前没有等待时间基线 | 先增加锁等待与慢文件系统测试；只有数据证明存在问题时再调整锁范围，同时保持来源、优先级、内容摘要和审核代次在提交点一致 |
| custom tool 只有静态候选，没有执行实现 | 候选不能形成可调用工具；直接注册会向模型暴露无法执行的伪工具 | 先完成一种明确制品的受限执行单元和真实工具提供方，再复用现有工具 ABI、权限与陈旧快照保护；执行不可用时只返回诊断 |
| 运行时插件没有安装和卸载流程 | 用户只能手工放置包，无法形成完整的动态插件体验 | 安装、卸载和状态清理作为独立产品流程；安装不自动审核或激活，也不复制外部生态凭据和配置批准 |
| 产品内置扩展尚未接入安装与组装链路 | 构建配置可以声明扩展锁，但运行时没有受产品清单约束的真实来源 | 由构建、安装器、更新和产品组装共同提供只读来源；不复用用户插件来源根、审核或卸载状态 |
| OpenCode 适配容易反向定义 BitFun | 可能形成 OpenCode 专用产品入口或内部模型 | OpenCode 只作为兼容适配输入，输出 BitFun 来源、诊断、候选项或 unsupported |
| 部分 core / product-full 路径仍偏宽 | 新入口可能继续依赖旧大门面 | 只迁移与插件主线或关键产品路径直接相关的归属模块，并同步删除或显著简化旧路径 |

## 4. 已完成基线

| 里程碑 | 已交付边界 |
|---|---|
| 接口切面与预算 | 四个接口切面、插件公开接口预算和依赖边界检查 |
| 插件运行时主机 | `availability`、`read_plugins`、`dispatch`，以及期限、代次、幂等、隔离、诊断和重启清理 |
| P0-C.1 | 受管包发现、完整性校验、工作区来源审核、CLI 管理和诊断 |
| P0-C.2 | 精确内容哈希激活、生产组装、OpenCode custom tool 静态候选和权限提示 |
| 停用恢复 | 按工作区和包清理缺失或损坏包的残留激活记录；保留来源审核历史并提供幂等、代次和持久化保护 |

上述基线不包含插件代码执行、工具注册、安装卸载、产品内置来源或外部 OpenCode 目录导入。详细完成事实归档在 [`core-decomposition-completed.md`](core-decomposition-completed.md)。

## 5. 后续 PR 顺序与范围

### PR2：首个可执行 custom tool

范围：

- 只支持一种经确认的插件制品和依赖规则，由 BitFun 管理受限执行单元，不依赖用户安装 OpenCode CLI。
- 执行单元实际加载工具定义并提供输入 schema 与调用实现后，产品组装才通过现有工具 ABI 注册提供方。
- 实际加载的工具标识必须与同一内容哈希下确认的候选集合一致；不一致时整个包不注册工具并返回诊断。
- 工具 ABI 归属层的插件工具包装在权限批准后调用主机执行接口；主机校验调用边界并转交生态适配器，适配器调用产品组装注入的执行服务；具体工作进程、沙箱和执行单元生命周期属于 `services` 实现。插件工具包装负责把类型化插件输出转换为既有工具结果，现有 `dispatch` 继续只返回候选和诊断。
- 每次调用复用现有工具权限、陈旧快照保护、期限、取消和工具结果；包变化、停用、撤销或执行单元退出使旧调用失效。
- 工作进程具备资源上限、环境变量白名单、崩溃回收和隔离状态；不同时实现可写钩子、界面贡献、在线仓库或多生态运行时。

完成条件：选择一个来自真实目标插件的自包含工具场景，明确目标用户、输入、输出和失败状态，并在 CLI/TUI 中经过权限确认后真实执行；聚焦测试覆盖权限拒绝、期限、取消、资源超限、受控环境、包变化或停用后的旧调用失效，以及执行单元崩溃回收。不支持或执行单元不可用的插件不进入工具快照，插件失败不导致产品进程退出。未选定真实场景时只能作为内部技术验证，不能发布为可用插件能力。

### PR3：运行时插件安装与卸载

范围：

- 首版只接收用户明确选择的本地来源，安装到用户级或项目级 BitFun 受管目录。
- 在临时目录完成清单、路径、大小和哈希校验后原子提交；冲突、覆盖和回滚行为可解释。
- 安装不自动写入 `SourceApproved` 或激活记录；卸载先使激活和执行单元失效，再处理包和状态。
- 不复制 OpenCode 配置、凭据或批准状态，不包含在线插件仓库、自动更新和组织策略。

完成条件：安装、重复安装、内容冲突、失败回滚、卸载、运行中卸载和手工损坏均有端到端 CLI 测试。新安装或内容替换后明确显示“未审核、未激活、不可调用”；相同内容的重复安装不改变原状态，冲突或失败回滚保留原包及其审核和激活状态。包已复制不能被解释为可用插件。

### PR4：首个随 CLI 发布的内置扩展

前置条件：PR2 已交付可执行工具路径；产品定制路线的 Customization-C0 已提供 Resolved Product Manifest，Customization-C2 已提供内置扩展锁和签名验证。上述产品定制能力应由独立前置 PR 交付，PR4 不吸收其余品牌、安装器、更新或组织模板范围。PR3 不是 PR4 的技术前置。

范围：

- 只覆盖 CLI 一个产品形态和一个可选（`optional`）内置扩展；构建产物携带只读扩展包，Resolved Product Manifest 固定 `id/version/hash/signer`。
- CLI 产品组装只加载 Manifest 声明且摘要一致的包，并复用 PR2 已完成的执行与工具路径。
- 用户/项目同 ID 包不能覆盖内置扩展；产品来源凭据不能替代运行时权限、审计和隔离。
- 不复用运行时插件的来源根、审核记录、安装状态、更新通道或卸载操作。
- Desktop/安装器、必需（`required`）语义、自动更新、升级、回滚和撤销不进入本 PR，分别按产品发行场景验收。

完成条件：先确认目标用户和一个实际 CLI 任务，再验证构建产物中的真实内置扩展可完成该任务；Manifest 摘要、签名凭据和产品策略均通过校验。缺失、摘要或签名不匹配、同 ID 冲突和运行期隔离均产生可观察降级，且不会读取用户插件审核或激活状态。演示工具不能作为阶段完成依据。

以上三个后续 PR 均不包含在线插件仓库、隐式 npm 安装、原始 OpenCode 目录批量导入、可写钩子、GUI/TUI 通用界面接口、Server/Remote 执行或 Codex/Claude 插件运行时。这些能力必须在出现独立产品场景和真实消费方后重新排期。

代码合入不等于产品能力已经发布。只有执行路径与对应来源路径都形成完整闭环后，产品入口才能显示“可用插件”；仅完成候选、安装或内置来源时，入口必须保持候选预览或 `projection-only`，并明确显示执行能力不可用。

## 6. 待确认决策

残留激活记录清理已完成，不依赖以下决策。技术依赖为：PR3 在作为可用插件体验发布前依赖 PR2；PR4 依赖 PR2 以及独立交付的 Customization-C0/C2 最小前置能力，PR3 不是其前置。开始 PR2 前必须确认执行载体、主机调用入口和首个真实工具场景。

| 决策 | 方案 | 主要影响 | 建议 |
|---|---|---|---|
| 首版 JS/TS 执行载体 | A. 随 BitFun 交付专用 JS/TS 工作进程；B. 只支持 BitFun 预编译扩展制品；C. 调用用户环境中的 Node/Bun | A 的环境、离线行为和跨平台版本一致，但增加产物体积、启动成本和安全更新责任；B 的运行边界最窄，但不能直接兼容常见 OpenCode 插件；C 的交付成本最低，但版本、依赖和安全行为不可控 | 建议 A；无论选择哪项，都不依赖 OpenCode CLI，也不隐式安装 npm 依赖 |
| 主机执行调用入口 | A. 为 `PluginRuntimeClient` 增加一个类型化工具执行操作；B. 新建独立插件执行接口；C. 复用现有 `dispatch` | A 只增加一个由插件工具包装消费的主机内部操作；B 隔离更强但增加接口数量；C 会混淆候选响应与最终调用语义 | 建议 A，并同步公开接口预算和接口测试；不采用 C |
| 首版插件副作用范围和真实场景 | A. 选择真实目标插件中的自包含纯计算/转换工具；B. 允许通过 BitFun 受控能力请求访问工作区；C. 允许执行单元直接访问文件、网络或进程 | A 不需要新增主机回调接口，但必须证明真实用户价值；B 更实用，需要调用关系、权限和取消的专项设计；C 难以审计和隔离 | 建议 A；未选定真实插件及用户任务前不启动 PR2，B 后续独立设计，C 不进入首版 |
| PR2-PR4 产品交付顺序 | A. 可执行工具 -> 本地安装 -> CLI 内置扩展；B. 本地安装先行；C. 内置来源先行 | A 优先形成可执行闭环；B/C 只能先交付 `projection-only` 来源，不能形成可用插件体验。此顺序不表示 PR3 是 PR4 的技术前置 | 建议 A；选择 B/C 时必须重写对应完成条件，且中间版本不能显示“可用插件” |

## 7. 后端复杂度整改清单

| 优先级 | 问题 | 方向 |
|---|---|---|
| P0 | 插件公开接口容易继续膨胀 | 公开接口预算必须声明接口切面、消费方、P0 场景、wire impact、退场条件 |
| P0 | OpenCode 适配可能成为内部模型 | 保持 `opencode-adapter` 只做来源、诊断和候选映射；产品侧接入必须消费 Plugin Runtime Host / 扩展贡献接口，不直接依赖适配器内部类型 |
| P1 | `runtime-ports` 单文件仍宽 | 先按模块分组和预算护栏收口；只有真实迁移收益明确时再拆 crate |
| P1 | `bitfun-core` 门面仍是事实大入口 | 新调用方不得依赖 `bitfun_core::agentic::*` / `service::*` 作为主路径 |
| P1 | Product capability 与 tool provider group 存在双重建模 | 短期以 provider group id 作为组装边界；长期收敛到单一能力事实 |
| P1 | 激活写入可能增加同工作区锁等待 | 先记录锁等待和慢文件系统数据；只有超出期限预算时再设计锁范围或提交点调整，不在架构文档预设算法 |
| P2 | 接口 handler 中仍有具体 IO | 后续按服务端口下沉到 services / adapters 归属模块 |

## 8. 固定执行流程

1. 同步最新 `gcwing/main`。
2. 对照 `product-architecture.md` 明确本次归属接口切面。
3. 先补边界保护，再迁移或新增实现。
4. 新增公开接口前更新预算规则；没有预算的公开符号视为失败。
5. 运行聚焦验证。
6. 从独立第三方角度审查是否存在接口膨胀、依赖回流、产品形态遗漏和安全绕过。
7. PR 说明必须列出变更范围、验证命令、未新增的能力边界和风险。

## 9. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| 插件公开接口预算 | `node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| 插件运行时主机 ABI | `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`，`cargo test -p bitfun-runtime-ports --test plugin_runtime_host_contracts`，`cargo test -p bitfun-plugin-runtime-host` |
| OpenCode P0-C.1 受管包来源与信任 | `cargo test -p bitfun-product-domains --test plugin_source_contracts --features plugin-source`，`cargo test -p bitfun-services-integrations --no-default-features --features plugin-source plugin_source --lib`，`cargo test -p bitfun-cli --test plugin_source_cli` |
| OpenCode P0-C.2 激活与 custom tool 候选 | `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`，`cargo test -p bitfun-core plugin_runtime::tests --lib`，`cargo test -p bitfun-cli --test plugin_source_cli` |
| 产品形态 / SDK 最小可用性 | `cargo test -p bitfun-product-capabilities --test plugin_product_shape`，`cargo test -p bitfun-product-capabilities --test product_sdk_assembly`，`cargo metadata --no-deps --format-version 1` |
| 大范围归属迁移 | `cargo check --workspace`，必要时补 focused test |

## 10. 暂停条件

- 新增公开插件、hook、event、UI、host 或可用性接口，但没有公开接口预算。
- 新增接口无当前消费方，或只服务未来完整兼容。
- OpenCode 配置、CLI 可用性、加载顺序或权限语义成为 BitFun 权威状态。
- 插件运行时主机直接写权限、审计、内核状态、工具结果或界面状态。
- 产品入口、前端或 interface crate 直接消费 `PluginRuntimeClient`、host 快照、生态原始载荷或插件执行单元句柄。
- ACP 外部智能体/工具桥接被当成 P0 插件体验替代方案。
