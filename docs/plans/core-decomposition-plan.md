# BitFun Core 拆解与运行时迁移执行计划

本文是活跃执行计划。计划只从 Issue #970 原始目标、当前代码状态和两篇设计文档推导，不再沿用历史阶段标签
作为事实口径。已完成事实只归档在
[`core-decomposition-completed.md`](core-decomposition-completed.md)。

稳定设计基线：

- [`core-decomposition.md`](../architecture/core-decomposition.md)：初始状态、目标状态、分层和风险。
- [`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)：目标接口、crate 内部职责和质量保护。

## 1. 执行原则

- 最终目标是让 `bitfun-core` 从 concrete runtime / product logic 中心收敛为 compatibility facade 与产品组装边界。
- 依赖方向保持为：Product Surfaces -> Product Assembly / Capabilities -> Harness / Tool Runtime / Agent Runtime SDK
  -> Runtime Services -> Stable Contracts / External Providers。
- 新增抽象必须同时删除、迁移或显著简化既有 core 路径；纯 facade、纯 guard、纯文档或只新增空接口不算 owner 迁移完成。
- 设计文档保持稳定，只在目标架构判断本身需要修正时修改；阶段状态和执行节奏只写入本计划和 completed 归档。
- 任何可能改变产品行为、权限语义、工具曝光、事件语义、session 生命周期、remote 行为、MiniApp 行为或发布形态的变更必须暂停并单独评审。

## 2. 当前代码基线判断

最新 `main` 已包含 function-agent Git concrete service owner 迁移，且当前分支已开始把 Product Assembly 与
Runtime Services provider 组合显式化。但当前代码仍未达到设计文档的目标状态：

- 产品入口仍通过 `bitfun-core` 的 `product-full` 获得完整能力；Product Assembly 已可表达当前完整能力集合，
  但尚未支撑按交付形态裁剪 feature / dependency。
- `runtime-services` 已有 typed builder、capability availability 和 core product assembly provider 组合，
  但许多 concrete provider 仍在 core 创建或持有。
- core 仍持有 `SessionManager`、`ExecutionEngine`、`PersistenceManager`、`CronService`、`MiniAppManager`、
  `RemoteFileService`、`RemoteTerminalManager`、`WorkspaceSearchService`、AI client factory 和大量 concrete tool adapter。
- `tool-runtime` 已迁移部分低风险本地 IO primitive，但 Bash、terminal lifecycle、indexed search、remote shell、
  permission UI/channel wait、checkpoint orchestration 和完整 execution pipeline 仍不是独立 Tool Runtime owner。
- `harness` 当前主要承接 descriptor / route plan / registry contract，Deep Review、DeepResearch、MiniApp 的 concrete workflow execution
  仍留在 core 或产品路径。
- feature / dependency trimming 还没有数据证明，不能声称不同交付形态已经可以按最小依赖组合。

## 3. PR 准出门禁

每个迁移 PR 必须同时满足：

- 有完整 owner 主题，且范围足够迁移真实逻辑主体。
- 保留旧路径兼容，删除或明显简化对应 core 主体路径。
- 有 focused regression、snapshot、contract test 或产品入口验证证明行为等价。
- boundary check 覆盖新 owner 和旧路径 facade，禁止反向依赖、Tauri 下沉、无类型 service locator 和全局 mutable registry 膨胀。
- PR 描述只说明本次 diff 的变更、风险、验证和剩余边界，不写过程信息。

不满足上述门禁时，不允许把变更作为独立 PR 提交。

## 4. 后续里程碑

### 4.1 M1（当前变更已闭环）：Product Assembly 与 Runtime Services concrete provider 组合

目标：让产品能力选择、service provider 注册和 capability availability 从 core 隐式聚合转为显式组装边界。

完成口径：

- Product Assembly 已建立 `DeliveryProfile`、`CapabilitySet`、tool provider plan、harness provider plan 和
  service availability report；当前 Desktop / CLI / Server / Remote / ACP / Web 均保持完整 product-full 能力集合，
  先证明“不减少能力”。
- core 的 tool runtime 与 harness registry 已改为消费显式 Product Assembly plan，旧 facade 输出保持等价。
- core product assembly provider 已把现有 session store path resolution、remote workspace/projection adapter，以及当前
  product-full 需要的 terminal / Git / Network / MCP catalog capability marker 注册进 `RuntimeServicesBuilder` 组合。
- 基础必选端口、search concrete、AI provider acquisition 和按交付形态裁剪不在 M1 中完成，继续留给后续 owner 迁移和
  feature/dependency 收尾阶段。

**不混入：** default feature 调整、构建收益声明、大规模目录移动、UI 行为变更。

**门禁：**
- `cargo check -p bitfun-core --features product-full`
- `cargo test -p bitfun-runtime-services`
- 涉及 remote / terminal / search / Git / AI 时补对应 focused tests。

### 4.2 M2（当前变更已闭环）：Tool Runtime concrete IO/search execution helper

目标：在不改变产品工具语义的前提下，让 `tool-runtime` 接管低层 filesystem / search 工具中可证明等价的执行 helper，
减少 core 对 remote shell 命令形态、stdout/stderr marker 和结果窗口裁剪的直接理解。

完成口径：
- `tool-runtime` 承接远程 Delete 命令规划、Read awk 命令规划与 marker 解析、LS 命令规划与 stdout entry 规整。
- `tool-runtime` 承接远程 Glob stdout 规整，以及远程 Grep 命令规划、result text 规整、match count 和 offset/limit windowing。
- core 删除对应重复拼接/解析逻辑，只保留 agent-facing `Tool` adapter、`ToolUseContext` path resolution、workspace shell 执行、
  checkpoint、UI/channel 副作用和 `ToolResult` 包装。
- `agent-tools` 继续拥有 manifest / admission / GetToolSpec 等 provider-neutral contract；`terminal-core` 继续拥有 PTY 与 terminal lifecycle。

**不混入：** 改变工具 schema、权限默认值、checkpoint 语义、remote fallback、shell 工作目录、terminal prewarm / PTY lifecycle、
prompt-visible manifest、GetToolSpec、readonly/enabled filtering、expanded/collapsed exposure、MCP/ACP catalog。

**门禁：**
- `cargo test -p tool-runtime`
- `cargo test -p bitfun-agent-tools`
- core Grep / remote filesystem focused tests
- `cargo check --workspace`
- `pnpm run check:repo-hygiene`
- `node scripts/check-core-boundaries.mjs`

### 4.3 M3（当前变更已闭环）：Agent Runtime lifecycle 决策闭环

目标：让 Agent Runtime SDK 接管 session / turn / scheduler / event / permission 中可迁移的 runtime kernel 主体。

完成口径：

- `agent-runtime` 接管 turn outcome 后处理 plan，统一决定 queue action、round 清理、finished-turn injection drain 和
  thread-goal continuation after-turn 动作；core scheduler 只执行清理、reply、goal continuation 和 dispatch 副作用。
- `agent-runtime` 接管 `SessionControl` 输入契约、默认值、tool-use render、create/cancel/delete 结果文案和 cancel route 决策；
  core tool adapter 只保留 workspace 解析、session manager 调用、scheduler/coordinator cancel 和 `ToolResult` 包装。
- `agent-runtime` 不新增三方依赖，不依赖 `bitfun-core`、Tauri、ACP、CLI TUI、desktop state 或 concrete service crate。
- concrete session manager、metadata / persistence IO、scheduler task lifecycle、event emitter wiring、permission UI/channel wait、
  concrete prompt assembly 和 product `Tool` adapter execution 继续留在 core compatibility / Product Assembly adapter。

**不混入：** 改变 `/goal`、AskUserQuestion、Task、subagent、post-turn hook、DeepReview measurement、token usage 或 continuation wire shape。

**门禁：**
- `cargo test -p bitfun-agent-runtime`
- `cargo test -p bitfun-core --features product-full` 中 session / scheduler / goal / subagent focused tests
- `cargo check -p bitfun-core --features product-full`
- boundary check 证明 agent-runtime 不依赖 core 或平台实现。

### 4.4 M4：Harness 与 Product Domain concrete workflow 闭环

目标：让 Harness / Product Domains 不再停留在 descriptor 或 pure policy 层，而是接管符合设计边界的工作流和 domain owner。

- MiniApp host primitive 的 `fs.*` / `shell.exec` 纯调用计划、参数默认值、错误契约和权限检查计划归入
  Product Domain；core 仅消费计划并继续负责权限 policy resolution、路径规范化、文件 IO、进程执行和输出映射。
- `net.fetch` / `os.info` 不迁入 Product Domain：前者依赖 `reqwest::Url` 与 HTTP client 语义，后者依赖平台 runtime facts，
  保留在 core concrete path 更符合最小依赖原则。
- MiniApp seed / marker 的 bundle、hash、marker wire format 与 seed decision 已由 Product Domain 持有；core 仍负责磁盘读写、
  customization metadata、PathManager integration 和 recompile。
- function-agent 已通过 Product Domain facade 与 core adapter 隔离 Git/AI 端口；AI provider acquisition 和 transport error mapping
  仍留在 core，除非后续单独评审稳定 AI runtime/provider 边界。
- Harness provider 继续保持 legacy route plan / descriptor owner；Deep Review / DeepResearch concrete execution 不混入本阶段。

**不混入：** 改变 MiniApp storage layout、worker 生命周期、host primitive 权限、Deep Review report 语义、function-agent prompt/response policy。

**门禁：**
- `cargo test -p bitfun-harness`
- `cargo test -p bitfun-product-domains`
- MiniApp import/sync/recompile/worker focused tests
- function-agent Git/AI focused tests
- 涉及 desktop / API 时补 `cargo check -p bitfun-desktop`

### 4.5 M5：feature matrix、依赖收益与目录组织收尾

目标：在 owner 边界稳定后，用数据证明不同产品形态可以最小依赖组合，并整理 workspace crate 可读性。

- 建立 Desktop / CLI / Server / Remote / ACP / Web 的 capability matrix 与 feature group 显式映射。
- 用 `cargo metadata`、`cargo tree`、`cargo check` 数据证明 no-default、product-full 和关键交付形态的依赖边界。
- 评估 `src/crates` 是否按 `contracts/`、`runtime/`、`services/`、`integrations/`、`product/` 等目录分组。
- 目录移动只允许在 owner 边界稳定后执行，并必须同步 Cargo path、AGENTS module index、boundary check 和 workspace build。

**不混入：** 运行时 owner 深迁移、三方库大版本升级、构建脚本行为变更、installer 发布流程变更。

**门禁：**
- `cargo metadata`
- `cargo tree` 对比
- `cargo check --workspace`
- `pnpm run check:repo-hygiene`
- `node scripts/check-core-boundaries.mjs`

## 5. 执行节奏

后续按 M4 收尾 -> M5 推进。每个里程碑原则上对应一个大 PR；如果发现风险超过单 PR 可控范围，只允许按 owner 边界拆分，
不允许拆成 facade / guard / helper 小 PR。

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
- 无法为 remote、tool、MiniApp、function-agent、scheduler、session lifecycle 迁移提供等价测试或可复核 snapshot。
- PR 只新增抽象而没有迁移、删除或显著简化旧 core 主体路径。

## 8. 完成标准

- `bitfun-core` 只保留 compatibility facade 与 product-full / Product Assembly 兼容边界。
- Agent Runtime SDK、Runtime Services、Tool Runtime、Harness、Product Capabilities、Product Domains 和 Concrete Integrations
  的职责边界可被代码结构、依赖检查和测试证明。
- 产品入口通过 Product Assembly / capability matrix 显式选择能力和 provider，不再被完整 core 隐式牵引。
- 高风险路径具备旧路径兼容、等价保护、明确回滚边界和产品形态验证。
- feature / dependency trimming 有数据证明，且不以功能缺失、权限漂移或性能劣化换取构建收益。
