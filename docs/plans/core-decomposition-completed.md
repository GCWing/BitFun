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
- `services-integrations` 已承接 remote-connect wire command routing、response assembly、workspace/session/poll/file/dialog/cancel/interaction helper 和 state tracker contract；core `RemoteServer` 只保留加密入口、initial sync host glue、全局 tracker adapter 与 concrete runtime host 接线。
- session restore 的 storage path resolution、turn-load request、restore timing facts 已进入 Runtime Services / Runtime Ports 边界；core 仍保留具体 persistence IO。

### 1.3 Tool 与 Product Capability 基线

- `tool-contracts`（Cargo package `bitfun-agent-tools`）已承接 provider-neutral tool DTO、manifest/catalog 策略、execution admission gate、collapsed unlock gate、static provider materialization 和 plan-to-registry assembly。
- `tool-execution`（Cargo package `tool-runtime`）已承接本地 Write / Edit / Delete / Glob、远程 Delete / Read / LS / Glob / Grep、tool pipeline batching plan、retry policy、state counting、cancellation state policy，以及 Bash shell 可复用策略：禁用命令、工作目录命令包装、非交互环境、AppleScript/IM guard、本地/远程结果渲染和 background result 文本。core BashTool / ToolPipeline 只保留 agent-facing adapter、终端 session、权限 UI/channel、checkpoint 采集、cancellation token/channel、scheduler delivery 和 host context glue。
- `services-integrations` 已承接本地 indexed workspace search 的 flashgrep daemon/session lifecycle、scan fallback、scope/path normalization、status/result conversion、preview mapping 和 daemon binary resolution；flashgrep protocol internals 已收回为 crate-private，core 通过 `WorkspaceSearchRepoConfig` hook 接入产品配置。同时承接 remote SSH workspace search 的 provider-neutral path/scope/probe/bundle/retry 策略与 concrete owner：remote flashgrep session/context cache、binary 安装/校验、stdio 协议请求、search/glob 组装和 FilesWithMatches fallback。core `WorkspaceSearchService` 只保留旧路径 facade、产品 config hook、workspace bootstrap hook 和 `BitFunError` 映射；core remote search 只保留 provider adapter、窄 stdio facade 与 russh bridge。
- `tool-contracts` 已承接 tool pipeline 的截断恢复提示和 write-like tool 分类；`tool-execution` 已承接 state counting 和 cancellation state policy；`agent-runtime` 已承接 tool confirmation plan/failure/wait-result 与 light checkpoint summary policy；core tool pipeline / tool context 只委托这些纯策略并保留执行状态存储、permission channel、Git facts 采集、scheduler 和 cancellation token glue。
- `tool-provider-groups`（Cargo package `bitfun-tool-packs`）已承接 tool provider group plan、按 id 选择和 unknown provider group 校验。
- `product-capabilities` 已承接 capability id、required service capability、tool provider group selection 和 harness provider selection 等 assembly facts。
- Product Assembly 已承接 `DeliveryProfile`、`CapabilitySet`、product-full provider plan、service availability report 和 profile-scoped harness registry 入口。

### 1.4 Agent Runtime、Harness 与 Product Domain

- `agent-runtime` 已承接 scheduler/background delivery 纯决策、turn outcome lifecycle plan、thread goal runtime、subagent visibility、prompt cache facts、prompt environment facts、mode/source presentation facts、scheduled-job lifecycle state、custom subagent schema/default/markdown IO/discovery/loading、post-call hook routing、tool confirmation plan、goal/user-question tool wire contract、SessionControl 输入契约、部分 event fact 映射、DeepResearch citation renumber 纯重排逻辑，以及 DeepReview policy / manifest / budget / queue / report enrichment / incremental cache / shared-context runtime state / task-execution provider-neutral packet、retry、backoff、capacity-skip shaping、provider capacity error decision、provider/admission queue step decision、provider/reviewer queue wait timing 状态机、provider retry attempt/elapsed 状态机、runtime tracker reliability signal shaping、runtime diagnostics log shaping 和 TaskTool retry/cancelled/result presentation。
- core 仍保留 concrete session manager、metadata/persistence IO、scheduler lifecycle、event emitter、permission UI/channel wait、concrete prompt assembly 主体、DeepReview task launch / provider wait side effect / report persistence、product `Tool` adapter 和具体 hook side effect；DeepResearch report 文件 IO / post-turn hook 已委托 `services-integrations`。
- `harness` 已建立 workflow descriptor、legacy route plan、provider registry，并注册 Deep Review、DeepResearch、MiniApp 的 legacy-facade provider；当前只证明 route/descriptor 边界，不代表 concrete workflow execution 已迁移。
- `product-domains` 已承接 MiniApp 纯状态、storage shape、runtime detection policy、worker capacity / idle / LRU policy、host method、`fs.*` / `shell.exec` host call plan、built-in bundle identity / seed-plan facts / marker wire format、create/update/draft/apply/customization workflow 持久化顺序、import bundle planning、compile request/path adaptation、permission policy request/path adaptation、import lifecycle orchestration、function-agent prompt / parser / response policy / ports，以及部分 function-agent Git snapshot/fallback 逻辑。MiniApp host dispatch、built-in seed/marker filesystem IO、JS worker process/pool lifecycle、storage filesystem IO 和 import bundle IO 已委托 `services-integrations`。

### 1.5 六层 workspace 布局

- `src/crates` 已按六层物理布局整理：`interfaces/`、`assembly/`、`adapters/`、`services/`、`execution/`、`contracts/`。
- 旧 `surfaces` 和 `providers` 目标层级已被移除：协议入口归入 `interfaces`，协议/transport/provider 转换归入 `adapters`，OS/runtime infrastructure 具体实现归入 `services`。
- execution 下 tool 相关目录已按职责命名：`tool-contracts`、`tool-provider-groups`、`tool-execution`。Cargo package / lib 名保持兼容。
- `agent-stream` 已成为统一 stream DTO、tool-call 累积和 replay 契约 owner；provider stream 解析测试归属 `ai-adapters`。
- AGENTS、README、DeepReview path classifier、core boundary rules 和 Cargo workspace path 已同步到当前分层。
- `bitfun-core --no-default-features` 已裁掉 workspace-search owner、debug ingest HTTP server、AI provider adapter runtime 和 `reqwest` direct dependency；workspace search 旧路径、debug ingest、CLI credential acquisition 和 AI client runtime 只在显式 product feature 下组装，边界脚本覆盖关键 feature gate。

## 2. 已建立的保护

- owner crate 不得依赖回 `bitfun-core`。
- `product-full` 继续保护完整产品能力集合。
- boundary check 覆盖 owner crate 禁止依赖、旧路径 facade-only、回流约束、Product Assembly facade 收口和物理 crate layout。
- boundary check 覆盖 remote-connect command routing owner，要求 core 委托 `services-integrations`，并阻止 response assembly / command policy 回流 core。
- boundary check 覆盖 Bash shell helper owner，要求 core 委托 `tool-runtime::shell`，并阻止输出渲染、background result 文本、命令包装和 guard 策略回流 core。
- boundary check 覆盖本地 workspace search owner，要求 core search facade 委托 `services-integrations::workspace_search`，并阻止 flashgrep session、scan fallback、preview/result conversion 和 path normalization 回流 core；remote SSH search 纯策略与 concrete owner 由 `services-integrations::remote_ssh::workspace_search` 承接。
- boundary check 覆盖 tool truncation recovery presentation、confirmation wait-result、light checkpoint summary、batching plan、retry policy、DeepReview task-execution 纯策略、provider capacity error decision、provider/admission queue step decision、provider retry attempt/elapsed 状态机、TaskTool retry/cancelled/result presentation、DeepResearch citation renumber 纯重排 owner，以及 MiniApp manager workflow、compile request/path adaptation、permission policy request/path adaptation 和 import lifecycle orchestration owner，要求 core tool pipeline / tool context / DeepReview task adapter / DeepResearch citation hook / MiniApp manager 委托 `bitfun-agent-tools`、`bitfun-agent-runtime`、`tool-runtime::pipeline` 和 `bitfun-product-domains`。
- DeepReview 路径分类按六层物理 crate 解析，避免把同层多个 crate 合并成一个风险 area。
- focused baseline 已覆盖 tool manifest、GetToolSpec、execution admission、MiniApp storage / builtin asset、remote workspace fallback、MCP config/catalog、agent-runtime prompt cache、custom subagent、thread-goal tools、AskUserQuestion、DeepReview hook measurement、tool confirmation、product capability pack、session restore、local/remote tool IO helper、function-agent Git、scheduled-job state 等路径。

## 3. 明确未完成边界

- `bitfun-core` 仍是完整产品 runtime 组装点，不能声称已经退化为纯 compatibility facade。
- 产品入口仍主要通过 `bitfun-core/product-full` 获取完整能力；Product Assembly 已可表达当前完整能力集合，但尚未真正按交付形态裁剪 default feature / dependency。
- concrete session manager、scheduler lifecycle、event delivery、permission UI/channel wait、prompt assembly、session persistence IO、AI client factory / provider acquisition 仍在 core。
- Bash tool orchestration 的可复用 shell helper、本地 indexed workspace search owner、remote workspace search concrete owner、remote SSH/SFTP/PTY concrete owner、tool confirmation/checkpoint 纯策略、tool pipeline batching/retry/state-count/cancellation-state policy、DeepReview provider-neutral policy/report/cache/task-execution shaping、provider capacity error decision、provider/admission queue step decision、provider/reviewer queue wait timing 状态机、provider retry attempt/elapsed 状态机、runtime tracker reliability signal shaping、runtime diagnostics log shaping、TaskTool retry/cancelled/result presentation、DeepResearch citation renumber 纯重排 / report IO、MiniApp host dispatch、MiniApp built-in seed/marker IO、MiniApp JS worker process/pool lifecycle、MiniApp storage filesystem IO、MiniApp import bundle IO、MiniApp import bundle planning、MiniApp compile request/path adaptation、MiniApp permission policy request/path adaptation、MiniApp import lifecycle orchestration、MiniApp manager create/update/draft/apply/customization workflow 持久化顺序和 prompt environment facts 已迁出；permission UI/channel side effect、tool pipeline concrete token/channel/scheduler glue、DeepReview concrete launch/provider wait side effect/report persistence，以及 MiniApp AI acquisition 和更大 concrete workflow execution 仍未完成 owner 迁移。
- no-default 与 product-full 的依赖边界已有数据基线，且 no-default 已不再携带 workspace-search owner、debug ingest HTTP server、AI provider adapter runtime 或 `reqwest` direct dependency；remote-ssh 基础 workspace identity 仍是兼容期依赖，不能声称各交付形态已达到最小依赖。
