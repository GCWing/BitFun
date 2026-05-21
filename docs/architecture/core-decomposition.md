# BitFun Core 拆解护栏（Core Decomposition Guardrails）

本文是逐步拆解 `bitfun-core` 的执行护栏（execution guardrail）。它用于补充
[`bitfun-core-decomposition-plan.md`](../plans/core-decomposition-plan.md)
中的详细里程碑计划。

目标是在不改变任何受支持构建形态（build shape）下产品行为的前提下，把稳定、
边界清晰的逻辑从较重的 `bitfun-core` runtime 聚合体中移出，从而减少不必要的
Rust 编译和链接面。

## 不可协商的不变量

- 拆解过程中不得改变产品行为。
- 不得为了提升本地速度而减少 CI 或 release 覆盖范围。
- 除非后续有明确的产品变更要求，否则产品 crate 必须保持相同的能力集合
  （capability set）。
- 构建脚本和安装器脚本不属于本次重构范围：
  - `package.json`
  - `scripts/dev.cjs`
  - `scripts/desktop-tauri-build.mjs`
  - `scripts/ensure-openssl-windows.mjs`
  - `scripts/ci/setup-openssl-windows.ps1`
  - `BitFun-Installer/**`
- 共享产品逻辑必须保持平台无关（platform-agnostic）。桌面端专属逻辑应保留在
  app adapters 中，再通过 transport/API layers 回流。
- 不要引入仓库级、机器相关的编译器或链接器默认配置，例如 `sccache`、`lld-link`
  或 `mold`。

## 执行顺序

按里程碑执行，不按孤立的重构想法零散推进：

1. **安全保护和最小编译面验证**
   - 在任何默认 feature 变轻之前，先加入 `product-full` feature 安全网。
   - 把已经独立成 crate 的 nested crate 移到 workspace 顶层路径。
   - 先抽取 `core-types`，承载稳定 DTO 和 port DTO；只有在 concrete runtime /
     network 转换依赖完成解耦后，才移动 `BitFunError`。
   - 如果 stream 测试可以不依赖完整 core 运行，则抽取 stream processing。
   - 移动重服务之前先引入 ports。第一层轻量边界位于 `bitfun-runtime-ports`；
     该 crate 只包含 DTO 和 trait。
   - 第一批 adapter 实现只视为边界搭建。只有相关 service migration 和回归测试
     完成后，才能声明 service/agent 的 concrete call site 已经被替换。
2. **中等粒度 owner crate**
   - 优先使用 8 到 12 个 owner crate，而不是大量小 crate。
   - 使用 `services-core` 和 `services-integrations`，不要为每个 service 文件夹
     单独建立 crate。
   - 使用 `agent-tools` 加 `tool-packs` feature group，不要为每个具体工具族
     单独建立 crate。
3. **Facade 收敛和边界强制**
   - `bitfun-core` 收敛为兼容门面（compatibility facade）和完整产品 runtime
     组装点（full product runtime assembly）。
   - 新 crate 抽出后，再加入轻量边界检查。
   - 更轻的默认 feature 只能作为单独且完整验证过的 PR 进行评估。

## Crate 归属目标（Crate Ownership Targets）

初始目标 crate 应保持中等粒度。下表同时包含目标 owner、当前完成态，以及属于拆解
边界的一些已有基础 crate；不得把 `target` 或 `partial` 误读为已完成迁移。

| 目标 crate | 归属职责 | 当前状态 |
|---|---|---|
| `bitfun-core` | 兼容门面和完整产品 runtime 组装点 | active：仍是完整 runtime assembly 和旧路径 facade |
| `bitfun-core-types` | 稳定 DTO、port DTO、纯 domain type，以及最终的纯错误类型 | partial：AI 错误 DTO / helper 已迁入；`BitFunError` 仍保留在 core |
| `bitfun-events` | 已有的传输层无关事件 DTO 和事件抽象 | done：既有基础 crate |
| `bitfun-ai-adapters` | 已有 AI provider adapter，以及 provider / protocol DTO 归属 | done：既有 adapter crate |
| `bitfun-agent-stream` | Stream 聚合和 stream-focused 测试 | done：stream 聚合已独立 |
| `bitfun-runtime-ports` | 面向 service/agent 边界的轻量跨层 DTO 和 trait | partial：DTO/trait-only 边界已建立，包含 agent submission/transcript/cancel、remote state、runtime event 与 remote image attachment 契约；不拥有 runtime 实现 |
| `bitfun-agent-runtime` | Sessions、execution、coordination、agent system | target：crate 尚不存在，agent runtime 仍在 core |
| `bitfun-agent-tools` | 轻量 tool DTO / contract、portable tool context facts / provider、runtime restriction、pure manifest/exposure and GetToolSpec presentation/schema/static metadata/detail/result assembly / execution-plan contract、provider-backed tool catalog / GetToolSpec runtime facade、provider-backed GetToolSpec execution result helper / Tool-result vector adapter、generic contextual manifest resolver、generic catalog snapshot provider / GetToolSpec catalog provider、generic registry / static-provider / dynamic-provider / decorator-ref / snapshot-decorator adapter / runtime assembly container、generic readonly/enabled snapshot filter | partial：product registry snapshot access、`ToolUseContext` adapter、`GetToolSpec` Tool impl 和 concrete tools 仍在 core；core 当前从 `bitfun-tool-packs` provider plan 物化内置工具列表，static-provider 安装 assembly、decorator reference、generic snapshot decorator adapter、provider-backed catalog runtime facade 与 readonly/enabled 过滤规则已委托给 `bitfun-agent-tools` |
| `bitfun-tool-packs` | 由 feature group 隔离的工具 provider plan | partial：提供 basic / git / mcp / browser-web / computer-use / image-analysis / miniapp / agent-control feature-group 元数据和 product provider group plan；不得声明 concrete tools 已迁移 |
| `bitfun-services-core` | Config、session、workspace、storage、filesystem、system services | partial：部分 pure helper 已迁出；config/workspace/filesystem runtime 多数仍在 core |
| `bitfun-services-integrations` | Git、MCP、remote SSH、remote connect、file watch integrations | partial：MCP runtime 已迁入；remote SSH 仍只迁移低风险 contracts/helpers；remote-connect 已拥有 wire DTO、request builder、tracker state / registry lifecycle 与 tracker event reduction，dispatcher/product execution 仍在 core |
| `bitfun-product-domains` | Miniapp 和 function-agent 产品子域 | partial：pure decision、port、storage/builtin contract 可迁入；IO、worker、built-in asset seeding、Git/AI service runtime 仍在 core |
| `terminal-core` | 已有 terminal package，移动到 workspace 顶层 `src/crates/terminal` 路径 | done：已在 workspace 顶层 |
| `tool-runtime` | 已有 tool runtime，移动到 workspace 顶层路径 | done：已在 workspace 顶层 |

除非有实测证据证明继续拆分可以减少关键编译目标或测试目标，并且该模块已经具备稳定的
owner 边界，否则不要把一个 feature group 继续拆成更小的 crate。

## 依赖方向规则（Dependency Direction Rules）

- 新拆出的 crate 不得反向依赖 `bitfun-core`。
- `bitfun-core` 可以依赖新拆出的 crate，并通过 re-export 保持旧路径兼容。
- 在声明 P3 边界收敛前，运行 `node scripts/check-core-boundaries.mjs`，确认已拆出的
  owner crate 没有新增 `bitfun-core` 反向依赖，并确认 `core-types`、`runtime-ports`
  和 `agent-tools` 没有引入重 runtime / concrete service 依赖。
- 已迁移回 `bitfun-core` 的 legacy facade 只能 re-export owner crate 或做窄错误 / 路径注入映射；例如 Git 旧路径、
  remote SSH types/workspace path + unresolved-key helper facade、MCP tool contract facade、MCP protocol types / JSON-RPC
  request builder facade、MCP config location / cursor-format / JSON config / config service helper facade、
  MCP server config facade、MCP OAuth auth facade、MCP server process auth/header helper、
  MCP remote transport Authorization normalization / client capability / rmcp mapping helper 和 announcement types facade
  由边界脚本检查，不得重新承载实现逻辑。
- 对仍嵌在 core runtime 文件中的旧公开类型，必须至少保留禁止回流检查；例如 MCP server
  type/status/config 已由 owner crate 拥有，`MCPServerProcess` 只保留 lifecycle、process 和 connection runtime 逻辑。
- `bitfun-runtime-ports` 必须保持 DTO/trait-only；不得依赖 concrete manager、
  service implementation、app crate 或 platform adapter。
- remote runtime port baseline 当前只提供契约和 core-owned adapter：`AgentSubmissionPort`
  仍拒绝 generic attachments；remote image DTO、turn cancellation、remote state 和 event facts
  不等于 remote-connect runtime 或多模态执行路径已经迁移。
- remote-connect command/response wire DTO、remote model catalog DTO、poll response assembly /
  model catalog poll delta、
  tracker state / registry lifecycle、remote tool preview slimming、legacy image context fallback /
  preference、restore target decision、cancel decision 与 remote file transfer
  size/chunk/name policy 可由
  `bitfun-services-integrations` 拥有；core 只保留 tracker host adapter、
  global dispatcher、session restore 执行、terminal pre-warm、`ImageContextData` adapter
  、file IO/path resolution 和实际 dialog submission routing。不要把 tracker state
  、wire DTO 或纯策略 helper 回写到 core。
- remote-connect runtime owner 进一步外移前必须保持迁移前快照：remote command/response
  shape、restore target、active-turn poll snapshot、cancel decision、image context fallback
  / preference、tracker fanout、file transfer 与 RemoteRelay/Bot queue policy。
- `bitfun-core-types` 不得依赖 runtime manager、service crate、agent runtime、
  app crate、Tauri、network client、process execution，或 `git2`、`rmcp`、`image`、
  `tokio-tungstenite` 等重集成依赖。
- 轻量 contract crate 不得吸收 CLI/TUI 依赖；`bitfun-cli`、`ratatui`、`crossterm`、
  `arboard`、`syntect-tui` 等仍属于 `src/apps/cli` app adapter / presentation layer。
- `ErrorCategory`、`AiErrorDetail` 以及纯 AI 错误分类/detail helper 应放在
  `bitfun-core-types` 中，并通过已有更高层路径 re-export 或委托，以保持公开行为稳定。
- 在剩余 concrete error-wrapper 依赖完成审核前，不要把 `BitFunError` 移入
  `bitfun-core-types`。错误边界中已经移除了 `reqwest::Error` 和
  `tokio::sync::AcquireError` 引用；`serde_json::Error`、`anyhow::Error` 以及历史
  `From<T>` 行为仍需要单独做兼容性处理后，才能移动该类型。
- Service crate 必须通过小型 port 调用 agent runtime，不要直接访问全局 coordinator。
- 迁移期间，adapter implementation 可以暂时放在 `bitfun-core` 中，但新的 service
  代码必须面向 port contract，而不是新增对 coordinator 或 manager 的直接依赖。
- Agent runtime 必须通过 ports/providers 依赖 service 行为，不要依赖 concrete 的重集成
  crate。
- 最新主干已把 subagent 可见性做成 mode-scoped registry 行为，并且 CLI `/subagents`
  管理入口也复用该查询 / override 语义；后续又新增 `Multitask` mode、内置
  `GeneralPurpose` subagent、`SubagentSessionLinked` routing 和 `Task.run_in_background` 后台结果投递。迁移 agent
  registry、subagent definitions 或 agent scheduler 前，必须先保留 mode visibility、
  hidden/custom/review 分组、desktop subagent API、CLI mode-aware list/config、
  background result running-turn injection 与 idle-session follow-up turn 等价测试；在此之前它们仍属于
  `bitfun-core` product runtime assembly 与各 app surface adapter 的组合边界。
- DeepResearch 现在包含 citation renumber post-turn hook。迁移 agent runtime 或 prompt/report
  处理前，必须保留 `report.md` / `citations.md` / `display_map.json` 的 deterministic post-processing 行为；
  在此之前该 hook 仍属于 `bitfun-core` agent runtime assembly。
- 最新主干新增 on-demand tool spec discovery。`ToolExposure`、`GetToolSpec` 名称、
  collapsed stub、manifest ordering、generic collapsed exposure query、generic contextual
  prompt-manifest resolver、generic catalog snapshot provider、ToolCatalogRuntime / GetToolSpec catalog provider /
  prompt / schema / assistant-detail rendering / detail JSON 等 provider-neutral 契约可由 `bitfun-agent-tools`
  拥有；但产品 registry snapshot、`dyn Tool`
  / `ToolUseContext` adapter、product collapsed-tool catalog、context-aware tool
  schema/description 的实际调用、`GetToolSpecTool` Tool impl / `BitFunError` 映射和
  `ToolUseContext.unlocked_collapsed_tools` 仍属于 `bitfun-core` product tool runtime。
  继续迁移前必须证明 prompt-visible manifest、expanded/collapsed exposure、unlock state
  与 desktop/MCP/ACP tool catalog 等价。
- 当前 tool runtime 外移的低风险入口是 `StaticToolProvider` / `install_static_provider`
  / `ToolRuntimeAssembly` 合约归属 `bitfun-agent-tools`，并让 core 通过
  `runtime_assembly.rs` + `static_providers.rs`
  将内置工具列表收敛为 `core.basic`、`core.agent`、`core.session`、`core.integration`
  provider group。
  这不代表 concrete tools、`ToolUseContext`、product registry snapshot adapter 或
  `GetToolSpecTool` 执行已经迁移。
- `ToolContextFacts` 只记录 tool call、agent/session/turn、workspace kind/root
  与 runtime restriction 等可移植事实。它不携带 collapsed-tool unlock state、
  `computer_use_host`、workspace services、cancellation token、custom data 或任何
  可执行 service handle；workspace root 使用 session identity 的 logical path
  （remote 为 normalized remote root）。`PortableToolContextProvider` 只是只读 facts
  provider 合约，当前由 core `ToolUseContext` 实现；`ToolUseContext` 本体仍归 core 拥有。
- 最新主干的 remote workspace guard 和 search fallback/context 修复提高了 workspace/search
  迁移门槛。后续迁移 workspace 或 search runtime 时，必须保留 remote workspace metadata、
  startup runtime ensure、remote flashgrep fallback、preview mapping 和 local/remote fallback 语义。
- ACP startup timeout 和 operation diff fallback 属于 ACP/Web product surface 行为；后续只能通过
  stable contract 共享事实，不得把 ACP timeout、tool diff fallback 或 Web diff rendering 下沉到
  core-types、runtime-ports、agent-tools 等 contract crate。
- 最新主干的 remote ACP agents config 继续强化 ACP/app adapter owner：remote workspace
  复用 local ACP config，并通过 ACP client manager、remote shell、remote capability store 与
  workspace menu 串联。后续只能把 environment / capability facts 抽成 contract；ACP config
  persistence、remote probing 和 workspace surface selection 仍留在 ACP/app surface。
- 最新主干的 usage/cache 与 OpenAI Responses 修复提高了 AI adapter / stream 迁移门槛。
  `cached_content_token_count` 表示 cache reads / hits，`cache_creation_token_count` 与
  DeepSeek `prompt_cache_hit_tokens` mapping 必须保留为独立语义，不能在 `agent-stream`、
  `session_usage` 或 runtime budget 迁移中重新合并为 total usage。OpenAI Responses /
  Codex ChatGPT flat tool schema 是 provider adapter serialization，不应写死进
  `bitfun-agent-tools` 的 provider-neutral manifest contract。
- 最新 Web 启动优化把 startup trace、deferred background scheduler、narrow tool initializer
  与历史会话 hydrate 放在 web app / Flow Chat surface。后续不能为了“共享启动能力”把
  `startupTrace`、`backgroundTaskScheduler`、history hydration 或 tool warmup 下沉到 core contract
  crate；只能通过 product checks 证明 app 仍可组装。
- 最新 CLI 重构新增大量 TUI、theme、selector、dialog 和 chat-state 代码，但仍位于
  `src/apps/cli`。后续 core decomposition 只能通过产品 check 验证 CLI 仍可组装，不应把
  CLI presentation 依赖迁入 core-types、runtime-ports 或 agent-tools。
- 最新 desktop close button 默认最小化到 system tray 是 desktop lifecycle surface 行为；
  后续若调整 desktop app lifecycle / window state，只能用 desktop product check 验证，
  不应把 close/minimize 策略抽入 shared core service。
- Tool framework crate 不得依赖 concrete service implementation。
- 产品 crate 可以通过显式 product feature 组装完整 runtime。
- 后续迁移必须先按风险分层处理：
  - 低风险：文档、boundary check、Cargo feature graph / dependency profile 基线、纯 DTO /
    contract 搬迁、旧路径 re-export、序列化 round-trip 测试、未启用的新 feature group 声明。
  - 中风险：在 owner crate 内为纯模块补 feature group、把 core 中的重依赖改为 optional 但
    仍由 `product-full` 启用、把只依赖 port 的 helper 迁入 owner crate。
  - 当前 `product-domains` 可继续承载 MiniApp runtime search plan、worker install 命令选择、
    package.json storage-shape helper、lifecycle / revision helper、host routing / allowlist helper、
    customization metadata / permission diff、built-in MiniApp bundle/hash/marker/source payload
    seed-decision contract、seed plan / marker wire helper 等纯决策 / 解析逻辑；实际 runtime detection、worker pool、
    storage IO、PathManager、进程执行、host dispatch 执行、customization draft 存储 / 应用与 builtin
    asset include / seeding / marker IO / recompile 仍留在 core product runtime。最新内置 PR Review
    MiniApp 依赖 core asset include、user-data seed、customized update runtime 与 source-hash input
    lookup；这些不是 `product-domains` runtime owner 已迁移的证据。
  - `product-domains` 可以先定义 MiniApp runtime/storage 与 function-agent Git/AI 的 port
    contract，并承载 function-agent 的纯 prompt / AI response parsing policy；core-owned adapter
    只能在不改变执行路径的前提下委托现有 service，并先补等价测试。IO/进程/AI/Git 执行 owner
    迁移仍属于后续高风险步骤。
  - 2026-05-18 product-domain readiness update: MiniApp draft DTO/response shape,
    draft/customization storage path shape, import layout / fallback payload
    contracts, manager lifecycle state-transition helpers, runtime executable
    search-plan helpers, customization draft-apply / built-in update-decline
    metadata policy, and Git function-agent diff truncation / commit prompt
    preparation now live in `bitfun-product-domains`. The same PR adds
    migration-before snapshots for core-owned MiniApp import / sync / recompile /
    rollback / dependency state paths and function-agent Git / AI response
    boundaries. Core still owns MiniApp filesystem IO, worker process execution,
    built-in asset seeding/source-hash input lookup, host-dispatch execution,
    `PathManager` integration, and function-agent Git/AI calls. Function-agent
    prompt templates, response JSON extraction, and domain error-mapping policy
    may move only with focused behavior-equivalence tests. The built-in MiniApp bundle/hash/marker/source payload
    seed-decision contract plus seed plan / marker wire helper can live in
    `bitfun-product-domains`, but bundled
    asset includes, marker IO, customized update runtime, and recompile
    orchestration remain core-owned.
  - 高风险：`ToolUseContext`、product tool registry / runtime manifest assembly / `GetToolSpec` 执行 owner 化、
    MCP concrete tool integration、remote-connect、remote SSH runtime、miniapp / function-agent runtime、
    agent registry、`bitfun-core default = []`
    或任何产品 crate feature set 调整。
- 高风险项不能作为 P2/P3 普通收尾任务顺带执行，必须先有等价性测试、port/provider 设计、
  旧路径兼容策略和用户确认。
- 后续 runtime 迁移以 `docs/plans/core-decomposition-plan.md` 的里程碑表为准，不再按零散
  “剩余 PR 数量”临时拆分。LR1 已闭环为文档/边界/基线校准，不包含 runtime owner
  迁移；后续高风险队列只允许按 H1-H5 的单一 owner 主题推进：
  - H1：tool runtime owner 迁移。当前只完成迁移前/迁移中边界收敛：`bitfun-agent-tools`
    承载 provider-neutral tool contract、generic registry/static/dynamic provider、
    contextual manifest resolver、provider-backed tool catalog runtime facade、
    GetToolSpec presentation/schema/static tool surface/detail/
    result assembly / execution-plan helper、provider-backed runtime facade / execution result helper /
    Tool-result vector adapter，
    generic decorator reference / snapshot-decorator adapter / static-provider runtime assembly
    container，以及 generic readonly/enabled registry snapshot filter；
    core 仍持有 `ToolUseContext`、`GetToolSpecTool`
    Tool impl、collapsed unlock state source、product snapshot wrapper adapter、product registry
    snapshot access 与 concrete tools。
    已合入 PR #803 把 core `Tool` 到 provider-neutral contract 的 adapter 收敛到
    `tool_adapter.rs`，把 product catalog / manifest / GetToolSpec catalog-detail provider
    收敛到 `catalog_provider.rs`；本阶段把 provider-neutral GetToolSpec static tool surface
    （name / description / schema / readonly / concurrency / permission / validation / tool-use message）、
    execution plan / result assembly 与 provider-backed execution result helper 收敛到
    `bitfun-agent-tools`；本阶段继续把 provider-backed visible-tools / manifest / readonly
    catalog 查询收敛到 `ToolCatalogRuntime`，core 只保留 product registry snapshot、agent
    policy、`dyn Tool` / `ToolUseContext` adapter；本阶段继续把 static-provider 安装 assembly 委托到
    `ToolRuntimeAssembly`，并把 product provider group plan 迁入 `bitfun-tool-packs`；
    core 只保留 concrete tool materialization、product snapshot wrapper adapter、product provider/context 注入、
    `GetToolSpecTool` Tool impl、unlock state source 和错误映射；本阶段也把 decorator reference contract、generic snapshot decorator adapter、
    GetToolSpec runtime facade、Tool-result vector adapter 与 readonly enabled filtering 的通用规则委托给
    `bitfun-agent-tools`，不改变工具行为。
  - H2：product-domain runtime owner 迁移。MiniApp / function-agent 的纯 DTO/helper/port
    facade 已外移；本阶段进一步把 function-agent prompt template、AI response JSON
    extraction 与 domain error mapping 策略迁入 `product-domains`。filesystem IO、
    worker process、host dispatch、built-in asset seeding / marker IO 与 Git/AI 调用仍显式
    core-owned。
  - H3：remaining service/runtime owner。remote-connect 当前只迁移 tracker / wire /
    pure policy / registry lifecycle；dialog submission、terminal pre-warm、file IO/path
    resolution、`ImageContextData` adapter、remote-SSH runtime、agent registry/scheduler
    等必须另起 port/provider 设计和等价评审。
  - H4：facade and boundary finalization。`bitfun-core` 只在 runtime owner 迁移确认后
    收敛为 legacy facade + product runtime assembly。
  - H5：optional feature/build-benefit evaluation。`bitfun-core default = []`、per-product
    feature matrix、依赖版本收敛和构建收益评估只能在 H1-H4 后独立进行。
- 已完成的 MCP runtime/dynamic tools、remote-connect tracker/wire/pure policy、
  semantic baseline、product-domain port/facade 与 tool contract/helper 外移不得重复规划；
  如果后续发现这些已完成项存在实现错误，应在对应 H 阶段记录问题、风险和修复方案，
  不把行为变更混入当前结构收敛 PR。
- 已合入的 `Services/Product Runtime Owner Closure` 只收口已经有 port/contract 保护的低风险 owner：
  remote-SSH session identity / mirror path / unresolved-session layout 归属
  `bitfun-services-integrations`，MiniApp storage/import file layout、fallback payload
  和纯 lifecycle state transition 归属 `bitfun-product-domains`。
  core 继续持有 SSH manager、remote FS / terminal、MiniApp filesystem IO、worker runtime、
  `PathManager` 注入和兼容 facade；不声明 remote-connect、MiniApp IO、function-agent Git/AI
  runtime 或 tool runtime 已迁移。
- 本轮 product-domain runtime preflight PR 已补等价快照和边界锚点：MiniApp
  `import_from_path` / `sync_from_fs` / `recompile` / `rollback` / deps state、
  function-agent staged diff snapshot，以及 function-agent AI response JSON extraction 与
  error mapping 的策略等价测试。H2 仅迁移 function-agent prompt/response policy owner；
  后续若继续移动 MiniApp IO/worker 或 Git/AI 调用 runtime owner，必须以这些快照为行为等价基线。
- 当前 `product-domains` runtime port/facade closure 只迁移 port-backed owner
  orchestration：MiniApp 的 deps/restart/recompile/sync/rollback 状态持久化可经
  storage facade 执行，function-agent commit / work-state facade 可基于 Git/AI port
  组装结果。Git commit-message 与 Startchat work-state 产品路径可通过 core-owned Git/AI
  adapter 接入该 facade；Startchat 接线必须保留 legacy git-state、git-diff fallback、
  `analyze_git=false` time-info 与 `analyzed_at` 时序。
  core 仍持有 MiniApp filesystem IO、compiler 调度、worker process、host dispatch、
  built-in asset include / seed / marker IO / recompile，以及 function-agent Git/AI service adapter
  和 AI client 调用；`product-domains` 现在承接 function-agent prompt template、AI response JSON
  extraction、domain error mapping 与 JSON-to-domain DTO parsing policy。

## 产品表面边界（Product Surface Boundary）

BitFun 的重构目标不是把 Desktop、CLI、Remote、Server 和 ACP 强行收敛成同一套命令或 UI。
这些产品表面可以保持不同交互语义，但应逐步共享稳定的运行时事实和能力契约。简短原则是：
**surface divergence, capability convergence**。

- Surface presentation 留在 app adapters：Desktop pane / command center、CLI TUI、Remote card、
  ACP protocol 和 Server routes 不进入 `core-types`、`runtime-ports`、`agent-tools` 或 owner runtime crate。
- 可共享的是 capability contract：session/thread identity、environment identity、permission facts、
  artifact refs、event facts、review/diff/terminal/usage/report 等稳定 DTO，以及必要的 port trait。
- CLI/Desktop parity 不是迁移 presentation dependency 的理由；`ratatui`、`crossterm`、`arboard`、
  `syntect-tui`、Tauri、Web UI 或 remote card rendering 依赖必须继续留在对应 surface adapter。
- 命令是产品 affordance，能力是 runtime contract。类似 `/diff`、快捷键、状态卡或协议方法可以映射到
  同一 capability contract，但不要求共享命令实现。
- Permission / approval contract 必须能表达来源 surface、thread、turn 和 subagent identity；各 surface
  的审批 UI 可以不同。
- Product-surface refactor 只能在 contract 层先做 observational DTO / port 补强；若要改变 UI、命令、
  权限策略或功能逻辑，必须作为单独产品变更 PR，而不是 core decomposition 的副作用。

## Feature 安全规则

- 在让任何默认 feature 变轻之前，先引入 `product-full`。
- 当前 `bitfun-core/product-full` 是阶段性 capability guardrail，不是最终 feature matrix
  或 capability source of truth。评估默认 feature 缩减前，必须先生成当前 feature graph baseline。
- 评估默认 feature 缩减之前，产品 crate 必须显式启用完整产品 runtime。
- `product-full` 是产品能力保护开关（product capability guardrail），不是新的万能聚合点
  （dumping ground）。每个新的 owner crate 都应暴露具体 feature group；只有为了保持既有
  产品形态时，`product-full` 才可以包含它们。
- 最终要么让 `bitfun-core/product-full` 显式聚合已经验证过的 owner crate capability feature，
  要么持续声明它不是完整能力矩阵；不得用它证明未迁移 runtime 已经完成 owner 化。
- 拆解完成后不要自动移除或减轻 `product-full`。如果未来要用 per-product explicit
  feature set 替代它，必须作为 P3 之后的独立评估，并且先通过完整产品矩阵。
- 不要把 feature 默认值变更和模块移动放在同一个变更中。
- 不要把改变产品构建产物能力集合作为减少本地测试编译面的副作用。
- 在任何 feature optionalization 之前，先提交只读保护网：记录 `bitfun-core`、desktop、CLI、
  ACP 和相关 owner crate 的 feature graph，明确哪些目标允许出现 `rmcp`、`git2`、`image`、
  `tokio-tungstenite`、`bitfun-relay-server`、Tauri / CLI presentation 依赖。
- owner crate 的 `product-full` 只聚合已经迁入且可独立验证的能力；不能为了让产品构建通过，
  让空 scaffold 或未迁移 runtime 假装已经拥有对应能力。

## 测试和验证策略（Test And Verification Policy）

先运行能够证明当前变更的最小验证，再在进入下一个里程碑前运行里程碑门禁。

对于保持行为不变的重构：

- 如果被移动的行为尚未被测试覆盖，先补测试，再移动逻辑。
- 当模块已经移出 `bitfun-core` 后，优先使用小 crate 测试。
- 如果变更影响 feature assembly、产品 crate manifest、desktop integration、CLI、
  server 或 transport path，则必须保留完整产品检查。
- 对功能逻辑偏移风险较高的迁移，必须先补“迁移前快照”测试或脚本输出，例如 tool registry
  工具清单、expanded/collapsed manifest、`GetToolSpec` 插入与 unlock state、
  dynamic provider metadata、snapshot wrapping 覆盖、remote-connect 消息字段、
  MCP tool/resource/prompt wire shape、miniapp permission policy、function-agent 输入输出契约。
- `product-domains` 与 core runtime 存在双路径阶段时，已抽出的 pure helper 必须配套 core
  adapter 等价测试或 snapshot；legacy function-agent runtime 在迁移前仍视为 core-owned
  runtime adapter，不得只修改 owner crate 一侧。
- boundary check 只能证明依赖方向，不能替代产品等价性验证。任何会移动 runtime owner 的 PR
  都必须同时说明旧路径兼容方式、产品能力不变证据和失败时的回滚边界。
- 编译收益必须和边界收敛分开陈述。若 PR 声明 build/check 收益，需记录
  `cargo check -p bitfun-core`、workspace check 和目标 crate check 的前后数据。

对于仅调整文档护栏的变更：

```powershell
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

期望结果：无 diff。

详细计划中列出了各里程碑门禁。没有针对对应门禁的最新验证证据时，不要声明里程碑完成。

## 冗余清理策略（Redundancy Cleanup Policy）

冗余清理不是主要的编译提速手段。只有在输入、输出、错误路径、副作用、日志、时序和平台
条件都能证明等价时，才抽取重复逻辑。

如果等价性不清晰，就保留重复代码。不要仅仅因为两个流程看起来相似，就创建新的共享抽象。

冗余清理 PR 必须独立于 crate splitting、feature 默认值变更和依赖升级。
