# BitFun Core 拆解已完成内容归档

本文只记录已完成事实和明确未完成边界。活跃执行计划见
[`core-decomposition-plan.md`](core-decomposition-plan.md)。

## 1. 已完成主线

### 1.1 P0 / P1：安全边界与最小编译面验证

- 已建立 `product-full` 默认能力保护，产品 crate 显式启用完整能力。
- 已把既有 nested `terminal-core` 和 `tool-runtime` 移到 workspace 顶层，保持旧 package / lib 语义。
- 已抽出 `bitfun-core-types` 第一批纯类型、错误分类和轻量 helper。
- 已抽出 `bitfun-agent-stream`，让 stream processor 和相关测试可绕开完整 `bitfun-core`。
- 已引入 `bitfun-runtime-ports` 初始边界和旧路径 compatibility wrapper。
- 已补 `AgentSubmissionRequest.source` / `turnId` 显式化，以及 dynamic tool provider metadata 基线。

明确未完成：

- `BitFunError` / `BitFunResult` 仍继续 core-owned。
- remote-connect / cron / MCP concrete call-site、generic attachment / image context 接入、产品逻辑或边界行为变更不属于 P1 完成范围。

### 1.2 P2：中等粒度 owner crate 成型

- `bitfun-services-core`、`bitfun-services-integrations`、`bitfun-agent-tools`、`bitfun-tool-packs`、`bitfun-product-domains` 已加入 workspace。
- `bitfun-core` 通过 facade / re-export 保持旧路径兼容。
- 已迁移 Git feature group、remote-SSH identity / path helper、MCP runtime / dynamic provider、remote-connect wire / tracker / file / image / dialog helper。
- 已迁移 generic tool registry / provider / catalog / `GetToolSpec` helper 和 product provider plan。
- 已迁移 MiniApp / function-agent 的纯 domain helper、port / facade 和部分决策逻辑。
- 已补 `core-types`、`runtime-ports`、`agent-tools`、`product-domains`、`services-integrations` 的 boundary check 和 feature graph 保护。

明确未完成：

- remote-SSH runtime、remote FS / terminal、workspace-root source、persistence / workspace service reads、`ImageContextData` concrete impl 仍未迁移。
- concrete tools 仍未迁移。
- MiniApp filesystem IO / worker / host dispatch / builtin marker IO / seed 写盘、function-agent Git / AI concrete service 仍未迁移。
- agent definition loading / concrete scheduler lifecycle 仍未迁移。

### 1.3 H1-H5 基线收口

- Tool runtime 已完成 provider-neutral contract、file guidance marker、file-read freshness facts、tool-result storage policy / preview / rendered replacement contract 和 execution presentation policy。
- Product-domain 已完成 MiniApp 纯状态 owner、runtime detection policy、worker capacity / idle / LRU policy、host method / fs access / shell token / env 等纯决策，以及 function-agent prompt / response policy。
- Service / agent 已完成 remote-connect presentation assembly、remote model policy、remote command orchestration、dialog scheduler outcome assembly、scheduler queue routing / cancel suppression 等 portable contract closure。
- Core 内部已形成 `product_runtime.rs`、`product_domain_runtime.rs`、`service_agent_runtime.rs` 等 owner closure 入口，便于后续审查。
- H5 当前只完成 feature / dependency baseline：`bitfun-core --no-default-features` 可编译面、`product-full` 显式 owner feature 聚合、optional dependency owner 映射和产品入口显式装配检查。

明确未完成：

- H5 不代表 per-product feature matrix、构建收益或 runtime owner 深迁移完成。
- `bitfun-core default = []` 仍是独立评估项，不能混入 runtime owner 迁移。
- 具体 IO、scheduler 生命周期、workspace-root、persistence、MiniApp worker / host / seed 写盘、function-agent Git / AI 仍需后续完整 owner PR。

### 1.4 Runtime owner PR1-PR4：组装、remote、agent runtime 与 harness 边界

- `bitfun-runtime-services` 已建立 typed service bundle、builder、capability availability 和 fake provider 基础。
- remote workspace facts、remote session metadata、remote file projection DTO 和 remote workspace/projection host trait
  已归入 `bitfun-runtime-ports`，并由 `bitfun-services-integrations::remote_connect` 保留旧路径 re-export。
- `bitfun-agent-runtime` 已建立为可独立构建的 Agent Runtime SDK owner crate，当前承接 scheduler/background
  delivery 纯决策，thread goal runtime 的 turn accounting、goal mutation、continuation plan 和 tool response assembly，
  subagent query scope / visibility / availability 决策，以及 round-boundary yield / injection state 和
  turn-outcome queue policy；prompt-loop 的 user-context policy、tool / skill / subagent listing reminder
  ordering、prompt cache policy / identity / DTO / scope key / in-memory store、shared mode profile / context policy、
  mode / subagent source presentation facts 已归入该 crate；finish-reason label、session-state event label 和
  turn-outcome event fact 也已由 `bitfun-agent-runtime` 承接，core 只保留旧路径 re-export 或 concrete adapter。
- persisted thread goal 的 portable DTO、status、continuation plan 和 tool response contract 已归入
  `bitfun-runtime-ports`；`get_goal` / `create_goal` / `update_goal` 已进入产品 tool registry。
- `bitfun-harness` 已建立为可独立构建的 Harness contract crate，当前承接 workflow descriptor、legacy route
  plan 和 provider registry；`bitfun-core::agentic::harness` 注册 Deep Review、DeepResearch、MiniApp 三个
  legacy-facade provider。

明确未完成：

- `bitfun-agent-runtime` 不代表 session manager、session persistence / prompt-cache cold restore、concrete prompt assembly、
  concrete agent definition loading、custom subagent file IO、scheduler 生命周期、event delivery、permission `Tool` handler
  或 post-turn hook 已迁移；当前 event 迁移只覆盖无副作用的 wire label / fact 映射。
- thread goal 的 metadata store、token subscriber、scheduler delivery adapter 和 goal `Tool` handler 仍在
  `bitfun-core`；runtime 决策已经归属 `bitfun-agent-runtime`，后续不应再把它误归入普通 concrete tool IO。
- `bitfun-harness` 不代表 Deep Review、DeepResearch、MiniApp 的 concrete workflow execution 已迁移；PR4 provider
  只生成旧路径 route plan，实际执行仍在既有 core/product 路径。
- Product command registry、capability pack、Harness 对 Tool Runtime / Runtime Services 的实际 orchestration
  仍是后续迁移项。

### 1.5 Tool Runtime admission gate：执行准入 owner 迁移

- `bitfun-agent-tools` 已承接 deterministic tool execution admission gate：tool-call loop history / block
  message、allowed-list gate、runtime restriction gate 和 collapsed-tool unlock gate。
- `bitfun-core` 的 tool pipeline 已删除对应常量、历史结构、循环检测算法和三段准入分支，只保留状态更新、日志、错误映射、
  registry lookup、input validation、confirmation、实际执行和 hook。
- `GetToolSpecTool` concrete adapter 已从 generic concrete-tool implementations 目录迁入 `product_runtime`
  owner；generic implementations 只保留兼容 re-export，on-demand spec discovery 的 product runtime 边界、
  错误映射和 context section renderer 由同一 owner 管理。
- manifest / visible tools / readonly catalog / GetToolSpec catalog path 已收敛到 `product_runtime/catalog.rs`；
  `manifest_resolver.rs` 仅保留旧路径兼容 facade 和 parity regression。
- snapshot wrapper 已收敛到 `product_runtime/snapshot.rs`，避免 registry assembly、catalog 和 snapshot adapter
  继续堆在同一 owner 文件。
- `WorkspaceFileSystem`、`WorkspaceShell`、`WorkspaceServices`、workspace command / dir-entry contract 已归入
  `bitfun-runtime-ports`；`bitfun-core::agentic::workspace` 只保留旧路径 re-export 和 local / remote concrete adapter。
  为避免功能偏移，该 contract 暂时保留既有 `anyhow::Result` 和 `CancellationToken` 语义。
- `ToolRuntimeHandles` 已归入 `bitfun-runtime-ports`，承接 `ToolUseContext` 的 workspace services /
  cancellation handle bundle；core 继续拥有 `ToolUseContext` 类型、runtime lookup、portable facts 投影和具体 tool 调用上下文。
- product provider group plan 到 concrete tool 的 materialization 已迁入 `product_runtime/materialization.rs`；
  provider order、tool name 和 registry exposure 由 focused test 保护。
- collapsed unlock 的 message-derived lifecycle state 与 `GetToolSpec` observation adapter 已迁入
  `product_runtime/unlock_state.rs`；`ExecutionEngine` 不再直接解析 `GetToolSpec` tool result 或调用 generic collector。

明确未完成：

- 具体 IO tools 仍未迁移；继续迁移必须先保护权限、filesystem/shell 行为、checkpoint hook 和产品 tool exposure。

### 1.6 Product-Domain builtin MiniApp bundle：asset owner 迁移

- 内置 MiniApp 的 bundle identity、版本和 embedded source assets 已归入
  `bitfun-product-domains::miniapp::builtin::BUILTIN_APPS`。
- `bitfun-core::miniapp::builtin` 只保留旧路径 re-export、seed 写盘、marker IO、用户 `storage.json` 保留和 recompile。
- 产品 seed 行为由既有 reseed/customization 回归和 product-domain bundle owner contract 保护。

明确未完成：

- MiniApp worker process、host dispatch、permission execution、PathManager integration、builtin marker IO /
  seed 写盘仍在 core；后续迁移必须单独证明权限与进程行为等价。

### 1.7 Product Capability pack：Harness / Tool / Service 组装闭环

- 新增 `bitfun-product-capabilities`，承接产品能力包 assembly facts：capability id、required runtime service
  capability、tool provider group id selection 和 harness provider selection。
- `bitfun-harness` 承接 provider-neutral harness descriptor 与 descriptor registry builder；`bitfun-core::agentic::harness`
  改为消费 product capability owner 提供的默认 harness registry，core 不再硬编码 Deep Review / DeepResearch / MiniApp provider descriptor。
- `ProductToolRuntime` 改为通过 product capability owner 解析默认 tool provider group plan，默认产品 tool provider
  order 保持不变。
- `bitfun-tool-packs` 承接 tool provider group plan、按 id 选择 plan 和未知 provider group 校验；
  product capability owner 不再拥有 provider plan 扫描和缺失 group 校验算法。
- `bitfun-agent-tools` 承接 provider-neutral static provider materialization 和 plan-to-registry
  assembly；core 只保留 concrete tool factory adapter、product plan adapter 和旧路径兼容入口，不再拥有 provider plan 遍历、provider group 构建、未知工具项错误处理或 registry 安装主体算法。
- Product Capability assembly 同时收敛 service requirement、tool provider group plan 和 harness provider selection；
  上层组装器可传入 service availability 来定位 capability 缺口，不需要让 capability owner 依赖 concrete service bundle。
- tool-pack selector 对未知 tool provider group 显式报错，static provider materializer 对未知 concrete tool 显式报错，避免配置错误被静默过滤成工具能力缺失。
- boundary check 覆盖 product-capabilities：禁止依赖 core、product-domain implementation、tool-runtime、concrete
  service crate、Tauri 和重 IO / protocol dependency。
- cargo tree / metadata 证据显示 product-capabilities 只依赖 harness、runtime-ports、tool-packs；core
  no-default 不选入 product owner deps，相关 owner 依赖保持 optional。
- PR-C 只证明 capability / harness / tool provider 组装边界和 no-default / dependency profile 未扩大；不迁移缺少等价保护的
  concrete IO、MiniApp worker/host、function-agent Git/AI 或 scheduler/event/permission lifecycle。

## 2. 已建立保护

- 新 owner crate 不得依赖回 `bitfun-core`。
- `product-full` 是完整产品能力保护开关。
- 构建脚本和 installer 相关脚本不作为 core 拆解的一部分修改。
- boundary check 覆盖已外移 owner 的旧路径 facade-only / 禁止回流状态。
- tool manifest、`GetToolSpec`、execution admission gate、MiniApp storage layout adapter、product-domain pure helper、remote workspace search fallback、MCP config / catalog / dynamic manifest、agent-runtime prompt cache、agent registry source/profile facts、product capability pack 和 harness/tool provider assembly 等已有 focused baseline。

## 3. 当前剩余结论

- 低风险准备项已经完成，不再新增零散小 PR。
- PR-C 已收敛 Harness / Product Capability / Build-Benefit closure。后续不应继续拆零散 helper PR；若继续迁移
  MiniApp worker/host、function-agent Git/AI、具体 IO tools、Agent Runtime concrete scheduler/event/permission/post-turn
  hook，必须先补端到端等价保护并作为新的完整 owner PR 评估。
- 缺陷修复、行为变更、冗余清理、三方库升级和构建脚本调整必须独立评估，不能伪装成 core decomposition 剩余里程碑。
