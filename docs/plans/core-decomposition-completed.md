# BitFun Core 拆解已完成内容归档

本文只记录已完成事实和明确未完成边界。活跃执行计划见
[`core-decomposition-plan.md`](core-decomposition-plan.md)。

各阶段的“明确未完成”描述保留的是当时阶段的边界，不等同于新的活跃队列。当前仍需执行的迁移只以
第 3 节和 `core-decomposition-plan.md` 的最终 PR-C / PR-D 为准。

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
  turn-outcome queue policy、dialog turn queue、active-turn facts、background running-turn injection construction、
  steering action、agent-session reply plan、cancelled-reply suppression state 和
  goal-continuation abort flags；prompt-loop 的 user-context policy、tool / skill / subagent listing reminder
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
  concrete non-custom agent definition loading、scheduler concrete 生命周期、event delivery、permission `Tool` handler
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
  registry lookup、input validation、confirmation channel / UI 副作用、实际执行和 concrete hook side effect。
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
- 早期 PR-C 只证明 capability / harness / tool provider 组装边界和 no-default / dependency profile 未扩大；不迁移缺少等价保护的
  concrete IO、MiniApp worker/host、function-agent Git/AI 或 scheduler/event/permission lifecycle。

### 1.8 Session Store / Restore Runtime Services Owner：restore 热路径边界

- `bitfun-runtime-ports` 已承接 session store / restore view 的稳定 request、storage path resolution、
  full/tail turn-load request、`SessionTurnLoadTiming` 和 `SessionViewRestoreTiming`。
- `SessionStorePort` 已从空 capability marker 扩展为 typed storage path resolution port；`bitfun-runtime-services`
  fake provider 和 contract test 覆盖该方法。
- `bitfun-core` 新增 `CoreSessionStorePort` concrete adapter，承接 local / remote / unresolved remote
  session storage path facts；`SessionManager` 保留旧方法签名，但 path resolution 委托 adapter。
- `PersistenceManager` 的 full/tail load hot path 改为消费 `SessionTurnLoadRequest`，原有
  `load_session_with_turns(_timed)` 和 `load_session_with_tail_turns(_timed)` API 保持兼容。
- Desktop `restore_session_view` 复用 `SessionViewRestoreRequest` 的 tail 归一规则，保留既有 16-turn
  UI view clamp、旧 response shape、tool-result preview compact 和 startup timing 记录。

明确未完成：

- `SessionManager` concrete 生命周期、auto-save / cleanup、event delivery、prompt assembly 和 runtime context restore
  仍在 core。
- session persistence 的具体文件 IO、metadata/index 写入、turn read/write、snapshot restore 和 cold restore 行为
  仍在 core concrete path；后续迁移必须补充端到端等价和性能保护。

### 1.9 Concrete Tool IO Runtime Owner：本地 tool IO 执行边界

- `bitfun-tool-runtime` 已承接本地 Write / Edit / Delete / Glob 的具体 filesystem/search 执行 primitive：
  文件写入的 created/overwritten/idempotent retry 结果、edit apply/write-back、delete target inspect/delete 和 glob
  `rg` / fallback walk 执行与浅层优先限流。
- `bitfun-core` 保留 agent-facing `Tool` adapter、tool name/schema/prompt stub、readonly/enabled exposure、
  permission admission、checkpoint hook、file-read freshness、workspace-search 优先路径、remote shell fallback、
  MCP/ACP catalog 和产品组装边界。
- 新增 `tool-runtime` 契约测试覆盖本地 write/edit/delete/glob owner 行为；core focused tests 继续覆盖原有
  FileWrite / FileEdit / Glob 兼容路径。

明确未完成：

- Bash / terminal lifecycle、indexed workspace search service、remote shell execution、permission `Tool` handler 和
  checkpoint orchestration 仍不属于 `bitfun-tool-runtime` concrete owner。
- 继续迁移 shell、terminal、remote 或 indexed search 时，必须先补 scheduler / terminal lifecycle / remote protocol
  等价保护，不能复用本地 filesystem primitive 的低风险假设。

### 1.10 Function-Agent Concrete Runtime Owner：Git / AI 具体服务边界

- `bitfun-product-domains` 继续拥有 function-agent prompt、parser、response policy、port 和 facade orchestration。
- `bitfun-core::product_domain_runtime::CoreProductDomainRuntime` 承接 function-agent 的 core runtime owner 入口，
  public `GitFunctionAgent` / `StartchatFunctionAgent` 直接通过该入口组装 Git / AI adapter 与 product-domain facade。
- `bitfun-core::function_agents::runtime_services` 承接 concrete Git snapshot、startchat Git/time snapshot、AI provider
  acquisition、AI transport error mapping、commit AI analysis 和 work-state AI analysis。
- 旧 `git_func_agent::AIAnalysisService`、`startchat_func_agent::AIWorkStateService`、`CommitGenerator` 和
  `WorkStateAnalyzer` 只保留兼容 re-export / facade，不再拥有 concrete runtime 主体逻辑。
- focused regression 覆盖 staged/unstaged diff 边界、no-HEAD fallback、非 Git workspace fallback、startchat snapshot
  语义、AI parser policy 和 product-domain facade contract。

明确未完成：

- function-agent concrete service 仍保留在 core runtime owner 中，尚未外移为独立 service integration crate。
- MiniApp worker process、host dispatch、permission execution、PathManager integration、builtin marker IO / seed 写盘仍在 core；
  后续迁移必须单独证明权限、进程生命周期、recompile 和用户 storage 保留行为等价。

### 1.11 Scheduled Job Lifecycle State Owner：运行时状态机边界

- `bitfun-agent-runtime::scheduled_job` 承接 scheduled-job runtime state、run status、默认 retry delay 和纯状态转移决策。
- manual trigger、scheduled trigger coalescing、pending retry wakeup、enqueue success、enqueue failure、missing-session auto-disable、turn started / completed / failed / cancelled 和 restart recovery 的状态字段更新由 agent-runtime owner 管理。
- `bitfun-core::service::cron::types` 保留 `CronJobState` / `CronJobRunStatus` 旧路径兼容 alias，保持 jobs.json state wire shape 不变。
- `CronService` 继续拥有 concrete store、schedule parsing、loop wakeup、session creation、scheduler submit、API filtering 和 product runtime integration。
- focused contract 覆盖 retry/coalescing/one-shot/missing-session/restart recovery 和 legacy cron state JSON shape；boundary check 覆盖 owner module、旧路径 alias 和 core service delegation。

明确未完成：

- concrete CronService loop、store、session creation、scheduler submit、event delivery、permission `Tool` handler、post-turn hook 和 agent definition loading 仍未迁移。
- 继续迁移上述路径必须先补端到端等价保护，不能只依赖 scheduled-job owner contract。

### 1.12 Agent Runtime Extension Boundary Closure：subagent、hook 与 confirmation

- `bitfun-agent-runtime::custom_subagent` 承接 custom subagent 的 source kind、definition schema、required-field 校验、默认 tools / readonly / review / model、front-matter 省略决策、markdown front-matter IO、目录 discovery、非递归 `.md` loading、路径优先级去重和 load error report。
- `bitfun-core::agentic::agents::definitions::custom::subagent` 保留旧路径兼容、路径持有和 `Agent` trait 适配；缺字段错误文本、默认值、保存省略规则和 markdown 读写委托 agent-runtime。
- `bitfun-core::agentic::agents::registry::custom` 只供应 workspace / user / home root，记录 load error，保留工具 / 模型校验、registry 写入和 project/user source 语义。
- `bitfun-agent-runtime::post_call_hooks` 承接 successful tool-call 后的 portable hook routing decision 和 executor orchestration；core 只实现 DeepReview shared-context measurement 的具体副作用。
- `bitfun-agent-runtime::tool_confirmation` 承接 confirmation gate plan、无 timeout 时的一年 deadline 兼容和 rejected / channel closed / timeout 的 legacy reason / error mapping；core 继续拥有 channel wait、UI state update、task state update 和 concrete `BitFunError` 转换。
- focused contract 覆盖 custom subagent 默认值、comma-format tools、默认字段省略、缺字段错误文本、markdown IO shape、discovery 优先级 / 去重 / 错误报告、successful tool-call hook routing/executor 和 confirmation plan/failure mapping；boundary check 覆盖 runtime owner、旧路径兼容、禁止旧 front-matter IO / discovery 回流和 core 委托。

明确未完成：

- 非 custom agent definition loading、event delivery、permission `Tool` handler、concrete hook side-effect execution 和 concrete scheduler lifecycle 仍未迁移。
- 继续迁移这些路径必须先补端到端等价保护，不能只依赖 owner contract test。

## 2. 已建立保护

- 新 owner crate 不得依赖回 `bitfun-core`。
- `product-full` 是完整产品能力保护开关。
- 构建脚本和 installer 相关脚本不作为 core 拆解的一部分修改。
- boundary check 覆盖已外移 owner 的旧路径 facade-only / 禁止回流状态。
- tool manifest、`GetToolSpec`、execution admission gate、MiniApp storage layout adapter、product-domain pure helper、remote workspace search fallback、MCP config / catalog / dynamic manifest、agent-runtime prompt cache、agent registry source/profile facts、custom subagent schema/default/markdown IO/discovery/loading、post-call hook routing/executor orchestration、tool confirmation plan/failure mapping、product capability pack、harness/tool provider assembly、session restore path/timing facts、本地 tool IO primitive、function-agent Git/AI concrete runtime 和 scheduled-job lifecycle state 等已有 focused baseline。

## 3. 当前剩余结论

- 低风险准备项已经完成，不再新增零散小 PR。
- 早期 PR-C 已收敛 Harness / Product Capability / Build-Benefit closure；PR-1 已收敛 session restore hot-path
  request / timing / storage path facts 和 Runtime Services port；PR-2 已收敛本地 Write / Edit / Delete / Glob
  concrete IO primitive；PR-3 已收敛 function-agent Git/AI concrete runtime owner closure；PR-4 已收敛 scheduled-job lifecycle state owner closure；Agent Runtime Extension Boundary Closure 已收敛 custom subagent schema/default/markdown IO/discovery/loading、post-call hook routing/executor orchestration 和 tool confirmation 计划 / 失败映射。后续不应继续拆零散 helper PR；
- 当前活跃迁移只剩 `core-decomposition-plan.md` 中的最终 PR-C / PR-D：最终 PR-C 关闭 Agent Runtime concrete delivery / permission / scheduler / event / hook / non-custom agent definition loading；最终 PR-D 关闭 Product Runtime、Runtime Services、Tool / Terminal / Search、Remote、MiniApp 和 function-agent 的剩余 concrete owner。最终 PR-C / PR-D 是收尾阶段编号，不再复用早期 PR-C 的范围。
- PR-D 完成并通过总体验收后，本文档范围内的 core decomposition runtime owner 迁移应关闭；后续只允许独立缺陷修复、feature matrix、构建收益优化、目录整理或产品行为变更评审，不再作为迁移计划遗漏追加。
- 缺陷修复、行为变更、冗余清理、三方库升级和构建脚本调整必须独立评估，不能伪装成 core decomposition 剩余里程碑。
