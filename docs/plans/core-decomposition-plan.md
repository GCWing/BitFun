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
- `opencode-adapter` 当前只提供 projection-only 来源发现适配器，不是生产插件运行时。

## 3. 当前差距

| 差距 | 影响 | 收敛要求 |
|---|---|---|
| 接口切面仍易混用 | 前后端线缆、插件扩展、host ABI 和生态适配容易互相穿透 | 以主架构文档的四个接口切面作为唯一口径，详细设计只补充归属和验证 |
| 公开接口预算只覆盖部分源码 | 可以继续在文档或代码中声明无消费方对象 | 扩展公开接口预算元数据，要求每个插件公开符号声明接口切面、消费方和退场条件 |
| 插件主机只完成受控边界 | 还不能证明 OpenCode-compatible 产品体验 | P0-B 只算主机边界；P0-C 才做来源发现、启用和最小候选效果消费 |
| OpenCode 适配容易反向定义 BitFun | 可能形成 OpenCode 专用产品入口或内部模型 | OpenCode 只作为反腐层输入，输出 BitFun 来源、诊断、候选效果或 unsupported |
| 部分 core / product-full 路径仍偏宽 | 新入口可能继续依赖旧大门面 | 只迁移与插件主线或关键产品路径直接相关的归属模块，并同步删除或显著简化旧路径 |

## 4. 后续 PR 阶段

### 阶段 A：接口切面和公开接口预算收口

目标：先确保架构文档和边界脚本不再鼓励新增无消费方接口。

范围：

- 收敛 `product-architecture.md`、`plugin-runtime-host-design.md`、`opencode-plugin-surface-audit.md` 的接口切面口径。
- 更新公开接口预算规则，使插件公开符号必须声明接口切面。
- 删除或降级文档中没有 OpenCode 对应、没有当前消费方或不能复用工具/事件/权限子接口的接口承诺。

验收：

- 没有新增稳定 Rust 接口。
- `node --test scripts/check-core-boundaries.test.mjs` 和 `node scripts/check-core-boundaries.mjs` 通过。
- 文档中 OpenCode-compatible P0 只保留配置导入、custom tool candidate、permission candidate、事件清单受控订阅和明确 unsupported 的最小路径。

### 阶段 B：插件运行时主机最小边界

目标：保持 P0-B 主机 ABI 安全、窄化、可测试。

范围：

- `PluginRuntimeClient` 只保留 availability、read_plugins、dispatch。
- 主机公开方法只保留 `new`、`restart`、`dispose_project`。
- Adapter trait 只保留 adapter_id、read_plugins、dispatch。
- read/dispatch 响应只能返回候选效果、诊断、隔离和状态只读视图。
- `HostRestarted` 是 P0-B 唯一清除条件。

验收：

- `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`
- `cargo test -p bitfun-runtime-ports --test plugin_runtime_host_contracts`
- `cargo test -p bitfun-plugin-runtime-host`
- 边界脚本阻止 host ABI 泄漏到产品入口、前端和 interface crate。

### 阶段 P0-C.1：OpenCode-compatible 最小来源与诊断

目标：只做 BitFun 主导的来源、信任和诊断闭环，不执行插件。

范围：

- 支持 BitFun 插件安装包、随产品携带包、项目/组织来源、受控外部包源作为权威来源。
- OpenCode 的 `opencode.json`、`.opencode/plugins` 和全局插件目录只作为可选导入输入。
- 导入输出为 BitFun 来源、manifest、hash、信任状态、诊断和 unsupported / projection-only 状态。

验收：

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- 不要求用户安装 OpenCode CLI。
- 不继承 OpenCode 启用顺序、权限语义或运行时状态。

### 阶段 P0-C.2：OpenCode custom tool 最小候选链路

目标：证明一个 OpenCode-compatible custom tool 可以映射为 BitFun 工具候选，但最终物化仍由工具 ABI、权限门禁和归属模块完成。

范围：

- custom tool 只映射为 provider candidate。
- 权限提示展示插件 id、来源、hash、能力、副作用、目标、风险、归属模块和审计/关联 id。
- 拒绝、超时、policy-denied 或主机失败不得写内核状态、审计成功或工具结果。

验收：

- 覆盖确认、拒绝、无副作用、隔离和诊断路径。
- 不新增插件专用工具 ABI。
- Desktop / CLI 只消费能力服务接口和读模型。

## 5. 后端复杂度整改清单

| 优先级 | 问题 | 方向 |
|---|---|---|
| P0 | 插件公开接口容易继续膨胀 | 公开接口预算必须声明接口切面、消费方、P0 场景、wire impact、退场条件 |
| P0 | OpenCode 适配可能成为内部模型 | 保持 `opencode-adapter` projection-only，生产消费必须通过插件主机和扩展贡献接口 |
| P1 | `runtime-ports` 单文件仍宽 | 先按模块分组和预算护栏收口；只有真实迁移收益明确时再拆 crate |
| P1 | `bitfun-core` 门面仍是事实大入口 | 新调用方不得依赖 `bitfun_core::agentic::*` / `service::*` 作为主路径 |
| P1 | Product capability 与 tool provider group 存在双重建模 | 短期以 provider group id 作为组装边界；长期收敛到单一能力事实 |
| P2 | 接口 handler 中仍有具体 IO | 后续按服务端口下沉到 services / adapters 归属模块 |

## 6. 固定执行流程

1. 同步最新 `gcwing/main`。
2. 对照 `product-architecture.md` 明确本次归属接口切面。
3. 先补边界保护，再迁移或新增实现。
4. 新增公开接口前更新预算规则；没有预算的公开符号视为失败。
5. 运行聚焦验证。
6. 从独立第三方角度审查是否存在接口膨胀、依赖回流、产品形态遗漏和安全绕过。
7. PR 说明必须列出变更范围、验证命令、未新增的能力边界和风险。

## 7. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| 插件公开接口预算 | `node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| 插件运行时主机 ABI | `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`，`cargo test -p bitfun-runtime-ports --test plugin_runtime_host_contracts`，`cargo test -p bitfun-plugin-runtime-host` |
| OpenCode P0-C.1 host-path 来源发现 | `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter` |
| OpenCode P0-C.2 私有 fixture 兼容保护 | `cargo test -p bitfun-opencode-adapter p0_c2_fixture` |
| 产品形态 / SDK 最小可用性 | `cargo test -p bitfun-product-capabilities --test plugin_product_shape`，`cargo test -p bitfun-product-capabilities --test product_sdk_assembly`，`cargo metadata --no-deps --format-version 1` |
| 大范围归属迁移 | `cargo check --workspace`，必要时补 focused test |

## 8. 暂停条件

- 新增公开插件、hook、event、UI、host 或可用性接口，但没有公开接口预算。
- 新增接口无当前消费方，或只服务未来完整兼容。
- OpenCode 配置、CLI 可用性、加载顺序或权限语义成为 BitFun 权威状态。
- 插件运行时主机直接写权限、审计、内核状态、工具结果或界面状态。
- 产品入口、前端或 interface crate 直接消费 `PluginRuntimeClient`、host 快照、生态原始载荷或插件执行单元句柄。
- ACP 外部智能体/工具桥接被当成 P0 插件体验替代方案。
