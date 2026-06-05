# BitFun Core 拆解已完成内容归档

本文只记录已完成事实摘要，不作为后续执行计划。后续执行路径以
[`core-decomposition-plan.md`](core-decomposition-plan.md) 为准；稳定架构目标以
[`core-decomposition.md`](../architecture/core-decomposition.md) 和
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
为准。

## 1. 已完成主线摘要

### 1.1 基础边界与 owner crate 基线

- 已建立 `product-full` 作为完整产品能力保护开关，产品入口显式启用完整能力。
- 已将原 nested `terminal-core`、`tool-runtime` 移到 workspace 顶层，保持旧 package / lib 语义。
- 已抽取 `bitfun-core-types`、`bitfun-events`、`bitfun-agent-stream`、`bitfun-runtime-ports` 等基础契约与轻量 owner。
- 已建立 `bitfun-services-core`、`bitfun-services-integrations`、`bitfun-agent-tools`、`bitfun-tool-packs`、`bitfun-product-domains`、`bitfun-runtime-services`、`bitfun-agent-runtime`、`bitfun-harness`、`bitfun-product-capabilities` 等 owner crate 基线。
- `bitfun-core` 已通过 facade / re-export 保持旧路径兼容，并逐步形成 `product_runtime`、`product_domain_runtime`、`service_agent_runtime` 等迁移期组装入口。

### 1.2 Runtime Services 与 ports

- `runtime-ports` 已承接 workspace、session store、remote workspace/projection、tool runtime handles、thread goal DTO、scheduled-job state 等稳定接口或事实。
- `runtime-services` 已建立 typed service bundle、builder、capability availability、provider 注册和 fake provider 测试基础。
- remote workspace facts、remote session metadata、remote file projection DTO、remote workspace/projection host trait 已归入稳定接口层，并保留旧路径 re-export。
- session restore 的 storage path resolution、turn-load request、restore timing facts 已进入 Runtime Services / Runtime Ports 边界；core 仍保留具体 persistence IO。

### 1.3 Tool 与 Product Capability 基线

- `tool-contracts`（Cargo package `bitfun-agent-tools`）已承接 provider-neutral tool DTO、manifest/catalog 策略、execution admission gate、collapsed unlock gate、static provider materialization 和 plan-to-registry assembly。
- `tool-execution`（Cargo package `tool-runtime`）已承接本地 Write / Edit / Delete / Glob 以及远程 Delete / Read / LS / Glob / Grep 的部分低层 execution helper；core 仍负责 agent-facing adapter、权限、checkpoint、remote shell 和 result 包装。
- `tool-provider-groups`（Cargo package `bitfun-tool-packs`）已承接 tool provider group plan、按 id 选择和 unknown provider group 校验。
- `product-capabilities` 已承接 capability id、required service capability、tool provider group selection 和 harness provider selection 等 assembly facts。
- Product Assembly 已承接 `DeliveryProfile`、`CapabilitySet`、product-full provider plan、service availability report 和 profile-scoped harness registry 入口。

### 1.4 Agent Runtime、Harness 与 Product Domain

- `agent-runtime` 已承接 scheduler/background delivery 纯决策、turn outcome lifecycle plan、thread goal runtime、subagent visibility、prompt cache facts、mode/source presentation facts、scheduled-job lifecycle state、custom subagent schema/default/markdown IO/discovery/loading、post-call hook routing、tool confirmation plan、goal/user-question tool wire contract、SessionControl 输入契约和部分 event fact 映射。
- core 仍保留 concrete session manager、metadata/persistence IO、scheduler lifecycle、event emitter、permission UI/channel wait、concrete prompt assembly、product `Tool` adapter 和具体 hook side effect。
- `harness` 已建立 workflow descriptor、legacy route plan、provider registry，并注册 Deep Review、DeepResearch、MiniApp 的 legacy-facade provider；当前只证明 route/descriptor 边界，不代表 concrete workflow execution 已迁移。
- `product-domains` 已承接 MiniApp 纯状态、runtime detection policy、worker capacity / idle / LRU policy、host method、`fs.*` / `shell.exec` host call plan、function-agent prompt / parser / response policy / ports，以及部分 MiniApp bundle identity 和 function-agent Git snapshot/fallback 逻辑。

### 1.5 六层 workspace 布局

- `src/crates` 已按六层物理布局整理：`interfaces/`、`assembly/`、`adapters/`、`services/`、`execution/`、`contracts/`。
- 旧 `surfaces` 和 `providers` 目标层级已被移除：协议入口归入 `interfaces`，协议/transport/provider 转换归入 `adapters`，OS/runtime infrastructure 具体实现归入 `services`。
- execution 下 tool 相关目录已按职责命名：`tool-contracts`、`tool-provider-groups`、`tool-execution`。Cargo package / lib 名保持兼容。
- `agent-stream` 已成为统一 stream DTO、tool-call 累积和 replay 契约 owner；provider stream 解析测试归属 `ai-adapters`。
- AGENTS、README、DeepReview path classifier、core boundary rules 和 Cargo workspace path 已同步到当前分层。

## 2. 已建立的保护

- owner crate 不得依赖回 `bitfun-core`。
- `product-full` 继续保护完整产品能力集合。
- boundary check 覆盖 owner crate 禁止依赖、旧路径 facade-only、回流约束、Product Assembly facade 收口和物理 crate layout。
- DeepReview 路径分类按六层物理 crate 解析，避免把同层多个 crate 合并成一个风险 area。
- focused baseline 已覆盖 tool manifest、GetToolSpec、execution admission、MiniApp storage / builtin asset、remote workspace fallback、MCP config/catalog、agent-runtime prompt cache、custom subagent、thread-goal tools、AskUserQuestion、DeepReview hook measurement、tool confirmation、product capability pack、session restore、local/remote tool IO helper、function-agent Git、scheduled-job state 等路径。

## 3. 明确未完成边界

- `bitfun-core` 仍是完整产品 runtime 组装点，不能声称已经退化为纯 compatibility facade。
- 产品入口仍主要通过 `bitfun-core/product-full` 获取完整能力；Product Assembly 已可表达当前完整能力集合，但尚未真正按交付形态裁剪 default feature / dependency。
- concrete session manager、scheduler lifecycle、event delivery、permission UI/channel wait、prompt assembly、session persistence IO、AI client factory / provider acquisition 仍在 core。
- Bash tool orchestration、terminal lifecycle / PTY、indexed workspace search service owner、remote shell executor abstraction、remote terminal concrete impl、MiniApp worker / host / seed / marker IO、Deep Review / DeepResearch / MiniApp concrete workflow execution 仍未完成 owner 迁移。
- no-default 与 product-full 的依赖边界已有数据基线，但 no-default 仍包含较大 concrete 依赖；不能声称各交付形态已达到最小依赖。
