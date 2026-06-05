# BitFun Core 拆解与运行时迁移执行计划

本文是活跃执行计划。稳定目标以
[`core-decomposition.md`](../architecture/core-decomposition.md) 和
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
为准；已完成事实归档在
[`core-decomposition-completed.md`](core-decomposition-completed.md)。

## 1. 执行原则

- 最终目标是让 `bitfun-core` 从完整 runtime / product logic 中心收敛为 compatibility facade 与 Product Assembly 边界。
- `src/crates` 采用六层物理布局：`interfaces -> assembly -> adapters -> services -> execution -> contracts`。依赖只能自上而下。
- `adapters` 负责协议、transport、外部 provider 和宿主通信转换；`services` 负责 OS、filesystem、terminal、MCP、remote、git、watch、process 等可复用具体实现。
- `execution` 是执行原语层，不是完整运行时实现层。Agent、Harness、stream、typed-service、tool contracts、tool provider groups 和 tool execution helper 分别保持清晰 owner。
- 新增抽象必须同时删除、迁移或显著简化现有 core 路径；纯 facade、纯 guard、纯文档或空接口不算 owner 迁移完成。
- 任何可能改变产品行为、权限语义、工具曝光、事件语义、session 生命周期、remote 行为、MiniApp 行为或发布形态的变更必须暂停并单独评审。
- 设计文档默认不随 PR 更新。只有目标架构本身需要修正时，才修改设计文档；阶段状态只写入本计划、completed 归档和 issue。

## 2. 当前代码基线判断

- 物理目录已按六层展开，旧 `surfaces` 和 `providers` 层级不再作为目标层级存在。
- Cargo package / lib 名保持兼容；例如 `tool-contracts` 仍发布为 `bitfun-agent-tools`，`tool-provider-groups` 仍发布为 `bitfun-tool-packs`，`tool-execution` 仍发布为 `tool-runtime`。
- Desktop / CLI / ACP 仍通过 `bitfun-core/product-full` 获取完整能力；Server / Web / Mobile Web 不直接依赖 core，但尚未完成按交付形态裁剪最小 feature / dependency。
- `runtime-services` 已有 typed builder、capability availability 和 core product runtime provider adapter，但不少 concrete provider 仍在 core 创建或持有。
- `tool-contracts` 已承接 provider-neutral tool manifest、admission、catalog、result policy 等纯策略；`tool-execution` 只承接部分低层 IO/search helper；Bash、terminal lifecycle、indexed search、remote shell、permission wait、checkpoint orchestration 和完整 execution pipeline 仍未完全迁移。
- `agent-stream` 已承接统一 stream DTO、tool-call 累积和 replay 契约；provider SSE / 响应解析测试归属 `ai-adapters`。
- `harness` 当前主要承接 descriptor / route plan / registry contract；Deep Review、DeepResearch、MiniApp 的 concrete workflow execution 仍在 core 或产品路径。
- `product-domains` 已承接 MiniApp / function-agent 的部分纯领域逻辑；worker、host side effect、AI acquisition、marker IO 等 concrete path 仍未完成 owner 迁移。

## 3. PR 准出门禁

每个迁移 PR 必须同时满足：

- 有完整 owner 主题，且范围足够迁移真实逻辑主体。
- 保留旧路径兼容，并删除、迁移或显著简化对应 core 主体路径。
- 有 focused regression、snapshot、contract test 或产品入口验证证明行为等价。
- boundary check 覆盖新 owner 和旧路径 facade，禁止反向依赖、Tauri 下沉、无类型 service locator 和全局 mutable registry 膨胀。
- PR 描述只说明本次 diff 的变更、风险、验证和剩余边界，不写过程信息。

不满足上述门禁时，不允许把变更作为独立 PR 提交。

## 4. 后续执行节奏

| 阶段 | 目标 | 准出标准 |
|---|---|---|
| M6 Service / Adapter owner 深迁移 | 将 core 中仍直接持有的 service、remote、MCP、Git、terminal、AI/API/transport adapter 创建路径继续下沉到 `services` 或 `adapters` | core 对应路径明显删除或简化；service/adapter 依赖不反向指向 assembly；产品行为等价测试覆盖 |
| M7 Tool execution 深迁移 | 将 Bash、indexed search、remote shell、checkpoint/permission 边界和 tool execution pipeline 的可复用主体迁入 `tool-contracts` / `tool-execution` / `services` | prompt-visible manifest、permission、GetToolSpec、remote/local IO、oversized result 与 cancellation 行为等价 |
| M8 Harness / Agent runtime 收口 | 将 Deep Review / DeepResearch / MiniApp workflow concrete execution、scheduler lifecycle、event delivery、prompt assembly 和 session side effect 继续从 core 收敛到 owner 边界 | workflow route 与 side effect 有 focused tests；agent runtime 不依赖 service/app concrete impl |
| M9 Product shape / feature trimming | 基于 capability matrix 裁剪 no-default/product-full 和不同交付形态依赖 | cargo metadata / cargo tree 有对比数据；各产品形态构建与关键入口验证通过 |
| M10 Core facade 收口 | 将 `bitfun-core` 限定为 compatibility facade、product-full assembly 和少量迁移期 adapter | core 不再是事实 runtime owner；边界脚本阻止 owner 逻辑回流 |

## 5. 固定执行流程

1. 同步最新 `main`，检查主干新增的 tool、remote、session、scheduler、CLI、mobile-web、ACP 或 product interface 变更。
2. 对照 Issue #970 和设计文档确认本次 owner 边界，不从旧 plan 标签继承完成判断。
3. 先补等价保护，再迁移实现主体。
4. 删除、迁移或显著简化 core 中对应旧路径。
5. 运行最小但足够的 focused verification 和 boundary check。
6. 从独立第三方角度审查功能漂移、性能劣化、依赖回流、产品形态遗漏和文档一致性。
7. 合入后只更新 completed 摘要和 issue 状态；设计文档默认不修改。

## 6. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary script / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| Workspace layout / Cargo path | `cargo metadata --no-deps --format-version 1` |
| Runtime Services / ports | `cargo test -p bitfun-runtime-services`，`cargo check -p bitfun-core --features product-full` |
| Tool primitives | `cargo test -p bitfun-agent-tools`，`cargo test -p tool-runtime`，tool focused tests |
| Agent Runtime | `cargo test -p bitfun-agent-runtime`，core session / scheduler / goal / subagent focused tests |
| Harness | `cargo test -p bitfun-harness`，core harness focused tests |
| Product Domains | `cargo test -p bitfun-product-domains`，MiniApp / function-agent focused tests |
| Desktop / Tauri/API | `cargo check -p bitfun-desktop`，并确认 Tauri 未下沉到 execution 或 contracts |
| 大范围 owner 迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |
| feature / dependency 收益 | `cargo metadata`，`cargo tree`，对应 build/check 对比 |

## 7. 暂停条件

- 迁移必须改变用户可见行为、权限策略、工具 schema、默认能力集合或 release 构建形态。
- 新 owner crate 必须依赖回 `bitfun-core` 才能编译或测试。
- Execution / contract crate 开始吸收 Tauri、CLI/TUI、process execution、network client、Git provider、AI provider、MCP client 等 concrete dependency。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- PR 只新增抽象，没有迁移、删除或显著简化旧 core 主体路径。

## 8. 完成标准

- `bitfun-core` 只保留 compatibility facade、product-full / Product Assembly 兼容边界和清晰的迁移期 adapter。
- Agent Runtime、Runtime Services、Tool Contracts、Tool Provider Groups、Tool Execution、Harness、Product Capabilities、Product Domains、Adapters 和 Services 的职责边界可由代码结构、依赖检查和测试证明。
- 产品入口通过 Product Assembly / capability matrix 显式选择能力，不再被完整 core 隐式牵引。
- feature / dependency trimming 有数据证明，且不以功能缺失、权限漂移或性能劣化换取构建收益。
