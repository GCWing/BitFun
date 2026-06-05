# BitFun Core 拆解与运行时迁移执行计划

本文是活跃执行计划。计划只从 Issue #970 原始目标、当前代码状态和两篇设计文档推导，不再沿用历史阶段标签
作为事实口径。已完成事实只归档在
[`core-decomposition-completed.md`](core-decomposition-completed.md)。

稳定设计基线：

- [`core-decomposition.md`](../architecture/core-decomposition.md)：初始状态、目标状态、分层和风险。
- [`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)：目标接口、crate 内部职责和质量保护。

## 1. 执行原则

- 最终目标是让 `bitfun-core` 从 concrete runtime / product logic 中心收敛为 compatibility facade 与产品组装边界。
- 依赖方向保持为：Product Surfaces / Protocol Surfaces -> Facade / Product Assembly
  -> Product Capabilities / Concrete Provider Adapters / Services -> Execution Primitives
  -> Stable Contracts / External Providers。
- 新增抽象必须同时删除、迁移或显著简化既有 core 路径；纯 facade、纯 guard、纯文档或只新增空接口不算 owner 迁移完成。
- 设计文档保持稳定，只在目标架构判断本身需要修正时修改；阶段状态和执行节奏只写入本计划和 completed 归档。
- 任何可能改变产品行为、权限语义、工具曝光、事件语义、session 生命周期、remote 行为、MiniApp 行为或发布形态的变更必须暂停并单独评审。

## 2. 当前代码基线判断

当前代码基线已包含 M1-M4 的 owner 迁移与 M5 的 workspace 分层整理。但当前代码仍未达到设计文档的最终目标状态：

- 产品入口仍通过 `bitfun-core` 的 `product-full` 获得完整能力；Desktop / CLI / ACP 已显式选择完整能力集合，
  Server / Web / Mobile Web 不直接依赖 core，但尚未完成按交付形态裁剪 default feature / dependency。
- `src/crates` 已按 `surfaces/`、`facade/`、`integrations/`、`services/`、`product/`、`execution/`、`contracts/` 分层，
  并由 boundary check 保护；package 名称和产品功能语义保持不变。
- `runtime-services` 已有 typed builder、capability availability 和 core product runtime provider adapter 组合；
  `product_assembly` 已收敛为兼容 facade，但许多 concrete provider 仍在 core 创建或持有。
- core 仍持有 `SessionManager`、`ExecutionEngine`、`PersistenceManager`、`CronService`、`MiniAppManager`、
  `RemoteFileService`、`RemoteTerminalManager`、`WorkspaceSearchService`、AI client factory 和大量 concrete tool adapter。
- `tool-runtime` 已迁移部分低风险本地 IO primitive，但 Bash、terminal lifecycle、indexed search、remote shell、
  permission UI/channel wait、checkpoint orchestration 和完整 execution pipeline 仍不是独立 Tool Runtime owner。
- `harness` 当前主要承接 descriptor / route plan / registry contract，Deep Review、DeepResearch、MiniApp 的 concrete workflow execution
  仍留在 core 或产品路径。
- feature / dependency 已有 no-default 与 product-full 的基线数据，但 no-default 仍包含较多 concrete 依赖，
  不能声称不同交付形态已经可以按最小依赖组合。

## 3. PR 准出门禁

每个迁移 PR 必须同时满足：

- 有完整 owner 主题，且范围足够迁移真实逻辑主体。
- 保留旧路径兼容，删除或明显简化对应 core 主体路径。
- 有 focused regression、snapshot、contract test 或产品入口验证证明行为等价。
- boundary check 覆盖新 owner 和旧路径 facade，禁止反向依赖、Tauri 下沉、无类型 service locator 和全局 mutable registry 膨胀。
- PR 描述只说明本次 diff 的变更、风险、验证和剩余边界，不写过程信息。

不满足上述门禁时，不允许把变更作为独立 PR 提交。

## 4. 当前计划闭环

M1-M5 已完成当前计划中的低风险边界收敛、保护基线和目录分层工作。
[`core-decomposition-completed.md`](core-decomposition-completed.md) 保留已完成事实与明确未完成边界。

| 阶段 | 已完成内容 | 未闭合边界 |
|---|---|---|
| M1 Product Assembly / Runtime Services | 建立 product-full provider plan、capability availability 和 service provider 基线；Product Assembly facade 已收口到兼容导出 | 具体 provider 构造仍大量留在 core product runtime adapter |
| M2 Tool Runtime IO/search helper | 将低层 filesystem/search 规划与 helper 迁入 `tool-runtime` | Bash、checkpoint、UI/channel 与 workspace shell 仍由 core adapter 组装 |
| M3 Agent Runtime lifecycle | turn outcome、SessionControl、thread goal、scheduler 决策等 runtime 纯逻辑进入 `agent-runtime` | concrete session manager、event emitter、permission wait 与 prompt assembly 仍在 core |
| M4 Harness / Product Domain | MiniApp 纯决策、bundle facts、function-agent Git concrete snapshot 等进入 owner crate | MiniApp worker/IO、AI provider acquisition 与 DeepReview concrete workflow 仍未完成迁移 |
| M5 feature matrix / directory layout | `src/crates` 已物理分层，feature matrix 与 no-default/product-full 数据基线已更新 | no-default 仍包含较多 concrete 依赖，产品形态尚未达到最小依赖闭环 |

### 4.1 M5 capability / feature matrix

| 产品形态 | 当前依赖 / feature 状态 | 判断 |
|---|---|---|
| Desktop | `src/apps/desktop` 以 `default-features = false` + `product-full` 依赖 `bitfun-core` | 保持完整产品能力；Tauri / WebDriver 仍属于 app / integration 层 |
| CLI | `src/apps/cli` 以 `default-features = false` + `product-full` 依赖 `bitfun-core`，并启用 ACP 协议入口 | 保持现有能力集合；后续可继续收敛 CLI feature set |
| ACP | `src/crates/surfaces/acp` 当前依赖 `bitfun-core/product-full` | ACP 是产品协议入口，不是 execution/runtime owner |
| Server | `src/apps/server` 当前不直接依赖 `bitfun-core` | 维持 server route / static runtime 边界 |
| Remote | 通过 `service-integrations`、`ssh-remote` 和 runtime ports 表达 | 属于能力组合和 provider 实现，不是独立产品 crate |
| Web / Mobile Web | 当前不直接依赖 Rust core crate，通过 API / transport / event DTO 消费能力 | 分层路径迁移不应改变前端行为 |

M5 已通过 `cargo metadata --no-deps` 验证 workspace 结构；`bitfun-core` no-default 依赖树为 649 行，`product-full` 为 1228 行。
`cargo check --workspace`、`node scripts/check-core-boundaries.mjs` 和 repo hygiene 已验证通过。

## 5. 执行节奏

M1-M5 已作为当前计划闭环。后续如继续推进设计文档中的 runtime / service / product owner 深迁移，需要从 Issue #970
和稳定设计文档重新确认 owner 边界；如果发现风险超过单 PR 可控范围，只允许按 owner 边界拆分，不允许拆成 facade / guard / helper 小 PR。

每个里程碑固定流程：

1. 同步最新 `main`，检查主干新增的 tool、remote、session、scheduler、CLI、mobile-web、ACP 或 product surface 变更。
2. 对照 Issue #970 和设计文档确认本次 owner 边界，不从旧 plan 标签继承完成判断。
3. 先补等价保护，再迁移实现主体。
4. 删除、迁移或显著简化 core 中对应旧路径。
5. 运行最小但足够的 focused verification 和 boundary check。
6. 从独立第三方角度审查功能漂移、性能劣化、依赖回流、产品形态遗漏和文档一致性。
7. 合入后只更新 completed 摘要和 issue 状态；设计文档默认不修改。

## 6. 验证矩阵

| 触碰范围 | 最小验证 |
|---|---|
| docs / boundary script | `pnpm run check:repo-hygiene`，必要时 `node scripts/check-core-boundaries.mjs` |
| Runtime Services / ports | `cargo test -p bitfun-runtime-services`，`cargo check -p bitfun-core --features product-full` |
| Tool Runtime | `cargo test -p bitfun-agent-tools`，`cargo test -p bitfun-tool-runtime`，tool focused tests |
| Agent Runtime | `cargo test -p bitfun-agent-runtime`，core session / scheduler / goal / subagent focused tests |
| Harness | `cargo test -p bitfun-harness`，core harness focused tests |
| Product Domains | `cargo test -p bitfun-product-domains`，MiniApp / function-agent focused tests |
| Desktop / Tauri/API | `cargo check -p bitfun-desktop`，并确认 Tauri 未下沉到 runtime owner |
| 大范围 owner 迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |
| feature / dependency 收益 | `cargo metadata`，`cargo tree`，对应 build/check 对比 |

## 7. 暂停条件

- 迁移必须改变用户可见行为、权限策略、工具 schema、默认能力集合或 release 构建形态才能继续。
- 新 owner crate 必须依赖回 `bitfun-core` 才能编译或测试。
- Runtime / contract crate 开始吸收 Tauri、CLI/TUI、process execution、network client、Git provider、AI provider、MCP client 等 concrete dependency。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- `bitfun-core::product_assembly` 重新承载 concrete provider 注册、harness registry 构造或非兼容 facade 逻辑。
- 无法为 remote、tool、MiniApp、function-agent、scheduler、session lifecycle 迁移提供等价测试或可复核 snapshot。
- PR 只新增抽象而没有迁移、删除或显著简化旧 core 主体路径。

## 8. 完成标准

- `bitfun-core` 只保留 compatibility facade 与 product-full / Product Assembly 兼容边界；Product Assembly 事实由 owner crate
  提供，core-specific adapter 留在清晰的 product runtime 边界内。
- Agent Runtime SDK、Runtime Services、Tool Runtime、Harness、Product Capabilities、Product Domains、Concrete Provider Adapters 和 Services
  的职责边界可被代码结构、依赖检查和测试证明。
- 产品入口通过 Product Assembly / capability matrix 显式选择能力和 provider，不再被完整 core 隐式牵引。
- 高风险路径具备旧路径兼容、等价保护、明确回滚边界和产品形态验证。
- feature / dependency trimming 有数据证明，且不以功能缺失、权限漂移或性能劣化换取构建收益。
