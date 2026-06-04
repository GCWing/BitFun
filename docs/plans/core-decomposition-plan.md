# BitFun Core 拆解与运行时迁移执行计划

本文只记录活跃计划、执行节奏、剩余迁移队列和验收门禁。已完成事实移入
[`core-decomposition-completed.md`](core-decomposition-completed.md)，避免主计划继续膨胀为历史流水账。

架构基线见 [`core-decomposition.md`](../architecture/core-decomposition.md)，详细接口和 crate 内部设计见
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)。

## 1. 当前判断

- P0/P1/P2 的低风险准备和 owner container 化已经完成，不再拆成 helper、guard、facade cleanup 小 PR。
- 当前迁移已经进入高风险 runtime owner 阶段。后续 PR 必须按完整 owner 主题推进，不能把 PR 当作单个 commit 使用。
- `bitfun-core` 迁移期继续作为兼容 facade 和完整产品 runtime 组装点；新 owner crate 不得依赖回 `bitfun-core`。
- 目标不是立即让 `bitfun-core default = []`，而是先把接口、provider 注册、旧路径兼容和行为等价保护做实。
- 产品能力、权限语义、工具曝光、事件语义、session 行为、release / fast build 脚本和各产品形态能力集合不得因迁移改变。

## 2. 迁移关键内容

### 2.1 接口与实现分离

- 稳定接口属于 Stable Contracts、Runtime Services、Tool Runtime 或 Harness contract。
- 具体实现按 Tool、OS、Remote、Protocol provider 分类，保留在 app 或 integration owner 中。
- Product Assembly 是唯一注册点，负责把具体 provider 注入 typed builder / registry。
- Runtime、Tool、Harness 只消费接口或 registry，不直接创建 filesystem、terminal、MCP、ACP、remote host 等 concrete manager。

### 2.2 Runtime owner 拆分

- Agent Runtime SDK：session、turn、scheduler、prompt loop、subagent、background task、permission coordination、runtime events。
- Runtime Services：filesystem、workspace、session store、Git、terminal、network、MCP catalog、remote connection / projection 等 port 和 capability availability。
- Tool Runtime：manifest、catalog、permission gate、execution pipeline、tool hook、结果归一化。
- Harness Layer：SDD、Deep Review、DeepResearch、MiniApp 等多步骤工作流和策略编排。
- Product Capabilities：Code Agent、MiniApp、function-agent、Remote Control、MCP App、Computer Use 等能力包。

### 2.3 Remote 拆分原则

- Remote 不是 Agent Runtime SDK 的内部能力，也不只按 Desktop / CLI 入口区分。
- 稳定接口应拆为 remote connection、remote workspace、remote filesystem / terminal projection、remote capability facts。
- SSH、relay、本地隧道、远端 OS 差异、认证方式属于具体 Remote provider。
- remote workspace、terminal pre-warm、scheduler submit、session restore、file chunk / image fallback 等行为必须用等价测试保护。

### 2.4 目标 crate 创建或扩展准入

- 新目标 crate 不能为了“架构完整”提前创建。必须同时满足 owner 边界清晰、旧路径兼容可保留、focused tests 可落地、依赖收益可解释、boundary check 可防回流。
- `bitfun-runtime-services` 已按该准入建立基础壳层；继续扩展时仍必须保持 `RuntimeServicesBuilder` skeleton、Remote ports 和 fake provider 测试同时成立。
- `bitfun-agent-runtime` 只能在 session / turn / scheduler / prompt loop 中至少一个 owner 可脱离 `bitfun-core` 构建时创建。
- `bitfun-harness` 已按 Deep Review、DeepResearch、MiniApp 三个 legacy-facade provider 建立 descriptor / registry contract；继续扩展时不能把 concrete workflow execution 描述为已完成。
- 若某项迁移只能承接单个 helper，或测试仍必须依赖完整 `bitfun-core`，继续留在迁移期 facade。

### 2.5 Workspace crate 目录组织

`src/crates` 当前按 crate 名平铺，随着 owner crate 增多，可读性会下降。目录重组属于后置非功能性整理，
不应混入 runtime owner 迁移 PR。待 Agent Runtime、Tool Runtime、Runtime Services、Harness 和
Product Domains 的 owner 边界稳定后，再评估是否按 `contracts/`、`runtime/`、`services/`、
`integrations/`、`product/` 等目录分组，并用 Cargo path 更新、module index、boundary check 和
workspace build 证明没有行为或 feature 影响。

## 3. 执行节奏

每个高风险 PR 按同一节奏执行：

1. **同步主干。** 变基到远端 `main`，检查最新主干是否引入新的 tool、remote、session、scheduler、CLI、mobile-web 或 product-surface 行为。
2. **确认组装门禁。** 高风险迁移前必须先有最小 Product Assembly / Runtime Services skeleton，能把 provider 注册到 typed builder / registry。
3. **确定 owner 主题。** 每个 PR 只迁移一个完整 owner 主题；预保护、迁移、旧路径兼容、文档更新和对抗性审核属于同一个 PR。
4. **先补保护。** 在移动 owner 前补 owner 设计、输入输出盘点、旧路径兼容方案、等价测试或 snapshot。
5. **再移动实现。** 只移动已被 port/provider 保护的逻辑；发现需要改变产品行为时暂停并单独评审。
6. **回看边界。** 检查是否新增反向依赖、万能 context、无类型 service locator、全局 mutable registry 或重复 runtime materialization。
7. **提交前审核。** 从第三方角度审查功能偏移、性能劣化、产品形态遗漏和文档一致性；不满足时不提交 PR。

## 4. 后续迁移队列

早期 PR-A / PR-B / PR-C、scheduler owner decision 扩展、PR-1 Session Store / Restore Runtime Services Owner、PR-2 Concrete Tool IO Runtime Owner、PR-3 Function-Agent Concrete Runtime Owner、PR-4 Scheduled Job Lifecycle State Owner 和 Agent Runtime Extension Boundary Closure 已进入完成归档。当前活跃队列只保留最终 PR-C / PR-D 两个大块迁移 PR；这里的最终 PR-C / PR-D 是收尾阶段编号，不再复用早期 PR-C 的范围。两者完成并通过验收后，本文档范围内的 core decomposition runtime owner 迁移应关闭，不再继续新增迁移 PR。后续如仍有缺陷修复、feature matrix、构建收益优化或产品行为变更，应作为独立工作评审，不能伪装成迁移收尾。

每个 PR 必须迁移真实 owner 逻辑，并同时包含旧路径兼容、focused tests、boundary check 和提交前对抗性审核。只新增抽象、只补 facade 或只增加 guard 不满足准出要求。

| PR | 主题 | 完整范围 | 不允许混入 | 合入门禁 |
|---|---|---|---|---|
| PR-C | Agent Runtime Concrete Delivery / Permission Closure | 收敛 Agent Runtime 侧剩余 concrete delivery：event delivery、permission `Tool` handler、goal `Tool` handler、thread-goal metadata / token subscriber / scheduler delivery adapter、非 custom agent definition loading、concrete scheduler lifecycle、post-turn / hook side-effect execution，以及这些路径需要保留在 core 的最小 compatibility adapter | 用 owner contract test 替代端到端行为证明；改变 session lifecycle、event ordering、goal tool wire shape、DeepResearch citation/post-turn behavior、permission UI 语义或默认 agent / subagent 可见性 | queue/preempt/cancel/goal verification/event focused tests，DeepResearch citation/post-turn tests，permission tool handler tests，non-custom agent definition loading tests，`cargo check --workspace` |
| PR-D | Product Runtime / Service / Tool Final Closure | 收敛剩余 concrete runtime owner：session persistence / cold restore / metadata IO、workspace-root / persistence / workspace service reads、remote FS / terminal / image context concrete impl、Bash / terminal lifecycle / indexed workspace search / remote shell execution、MiniApp worker / host / seed / marker IO、function-agent concrete service 外移，以及必要的 final boundary / directory organization cleanup | 改变产品能力集合、工具曝光、权限、checkpoint、remote fallback、MiniApp worker 生命周期、function-agent Git/AI 行为、release/installer 构建形态；把独立 feature matrix 或构建性能优化混入迁移 | session persistence / restore tests，remote workspace / file / terminal focused tests，terminal / shell / indexed-search focused tests，MiniApp import/sync/recompile/worker tests，function-agent Git/AI focused tests，`cargo check --workspace` |

计划优先级：先完成 PR-C，再完成 PR-D。PR-D 合入后需要做一次总体验收：确认 plan / completed 文档没有剩余活跃迁移队列，`bitfun-core` 只保留兼容 facade 和产品组装，边界脚本覆盖已外移 owner，且所有产品形态仍保持既有能力集合。PR-1 到 PR-4 和 Agent Runtime Extension Boundary Closure 已进入完成归档；后续不应继续触碰 session restore 热路径、已迁移的本地 tool IO primitive、function-agent Git/AI concrete runtime、scheduled-job runtime state、custom subagent schema/default/markdown IO/discovery/loading、post-call hook routing/executor orchestration 或 tool confirmation 计划与失败映射，除非是修复等价测试发现的问题。

## 5. 每类 PR 的保护重点

### 5.1 Service / Agent Remote Runtime Owner

- 先定义 remote connection、workspace、projection、capability facts port。
- 保留 core adapter 读取 workspace-root、persistence、session restore、scheduler submit，直到有端到端 remote regression。
- SSH、relay、tunnel、远端 OS、认证差异留在 Remote provider。
- 验证 remote command/response wire、restore -> terminal pre-warm -> scheduler submit 顺序、file full/chunk/info、image context fallback、remote workspace startup guard。

### 5.2 Agent Registry / Scheduler Owner

- 已迁移只读 facts、queue policy decision、queue state、active-turn facts、background running-turn injection
  construction、steering action、agent-session reply plan、cancelled-reply suppression state、goal-continuation abort flags 和 runtime event facts；
  不移动 concrete scheduler 生命周期。
- 保留 mode-scoped visibility、hidden/custom/review grouping、background delivery entrypoint、idle-session follow-up 和 persisted thread goal continuation 语义。
- thread goal runtime、subagent visibility/availability、round-boundary yield/injection、turn-outcome queue
  policy、dialog turn queue、active-turn state、background running-turn injection construction、steering action、
  agent-session reply plan、cancel suppression、finish-reason label、session-state event label 和 turn-outcome event fact 已归入
  `bitfun-agent-runtime`；后续只允许 core 继续作为 metadata store、config/file IO adapter、concrete prompt
  assembly、concrete scheduler lifecycle、scheduler delivery、event delivery 和 `Tool` adapter。
- scheduled-job runtime state、run status、retry / coalescing / one-shot / missing-session disable / restart recovery
  transition 已归入 `bitfun-agent-runtime::scheduled_job`；core cron 继续拥有 store、schedule parsing、loop wakeup、session
  creation、scheduler submit 和 API wire compatibility。
- custom subagent definition schema、required-field 校验、默认值、front-matter 省略决策、markdown front-matter IO、目录 discovery / `.md` loading、successful tool post-call hook routing / executor orchestration
  和 tool confirmation 计划 / 失败映射已归入 `bitfun-agent-runtime`；core 继续供应 concrete root path、持有旧 `Agent` 适配、记录加载错误、执行工具 / 模型校验、写入 registry，并保留 DeepReview shared-context measurement 的具体副作用、confirmation channel wait / 状态更新 / UI 副作用和 `BitFunError` 映射。
- 若继续迁移 scheduler lifecycle、event delivery、permission `Tool` handler、非 custom agent definition loading 或 concrete hook side-effect execution，必须先补端到端等价保护，
  不能只用 owner contract test 证明。
- 验证 subagent availability、queue/preempt/cancel suppression、DeepResearch citation / post-turn hook、goal verification events、`get_goal` / `create_goal` / `update_goal` tool response wire shape。

### 5.3 Product-Domain Runtime Owner

- MiniApp 已将 builtin bundle identity、版本和 embedded asset 放入 `bitfun-product-domains`；core 继续负责 seed 写盘、marker IO、用户 storage 保留、recompile、PathManager、worker process 和 host dispatch。
- 后续若继续迁移 MiniApp worker / host，必须先拆清 process runtime、permission policy、host primitive dispatch、draft worker 与 active worker 的等价边界，不能把 PathManager 或 worker process 下沉到 domain crate。
- function-agent 的 prompt、parser 和 facade policy 已归属 `bitfun-product-domains`；core 通过 `CoreProductDomainRuntime` 和 `function_agents::runtime_services` 保留 concrete Git/AI provider acquisition、error mapping、no-HEAD diff fallback、非 Git workspace fallback、`analyzed_at` 时序和旧 import path 兼容。
- 验证 MiniApp import/sync/recompile/rollback/deps state、builtin seed marker、customized update metadata、function-agent prompt/response policy。

### 5.4 Tool Runtime Owner

- 已完成 deterministic execution admission gate 迁移；core 仅保留状态更新、registry lookup、input validation、confirmation channel / UI 副作用、实际执行和 concrete hook side effect。
- 已完成 `GetToolSpecTool` concrete adapter 的 product runtime owner closure；generic concrete-tool implementations
  只保留兼容 re-export，不再拥有 on-demand spec discovery Tool impl。
- 已完成 manifest/catalog/snapshot owner closure；`manifest_resolver.rs` 只保留旧路径兼容 facade，product runtime
  的 `catalog.rs` / `snapshot.rs` 管理 resolved manifest DTO、visible tools、readonly catalog、GetToolSpec catalog
  path 和 snapshot wrapper。
- 已完成 `WorkspaceFileSystem`、`WorkspaceShell`、`WorkspaceServices` 等 workspace service
  contract 归入 `bitfun-runtime-ports`，core `workspace.rs` 只保留旧路径 re-export 和 local/remote concrete adapter；
  `ToolRuntimeHandles` 归入 `bitfun-runtime-ports`，承接 ToolUseContext 的 workspace services / cancellation handle bundle。
- collapsed unlock 的 message-derived state 与 GetToolSpec observation adapter 已归入 `product_runtime/unlock_state.rs`，
  `ExecutionEngine` 不再拥有 GetToolSpec 结果解析细节。
- product provider group plan 到最终 registry 的 generic assembly 已归入 `bitfun-agent-tools`；
  `product_runtime/materialization.rs` 只保留 concrete factory / product plan adapter，`product_runtime.rs`
  只保留 product plan、decorator 与旧路径兼容入口。
- workspace service contract 暂时保留既有 `anyhow::Result` 和 `CancellationToken` 语义，避免在 owner 迁移 PR 中同时改变
  错误分类、取消语义或调用方边界；后续若要收敛为 portable `PortResult`，必须单独补错误映射等价测试。
- 本地 Write / Edit / Delete / Glob 的具体 filesystem/search 执行 primitive 已迁入 `bitfun-tool-runtime`；core 保留 `Tool` adapter、权限、checkpoint、file-read freshness、workspace-search 和 remote fallback。
- 后续若继续迁移 Bash、terminal、indexed workspace search 或 remote shell execution，必须先证明 scheduler、terminal lifecycle、remote protocol 和 checkpoint 行为等价，不能把它们当作普通本地 IO helper 直接搬移。
- 保留 tool name、schema、prompt stub、readonly/enabled/filtering、unlock state 生命周期。
- 验证 builtin tool list、provider order、expanded/collapsed exposure、dynamic provider metadata、Deep Review 修改类工具 checkpoint hook。

### 5.5 Core / Tauri 脱离保护

- `bitfun-core`、Agent Runtime SDK、Tool Runtime、Harness、Runtime Services contract 不应直接依赖
  Tauri handle、window、command macro、desktop API state 或 Tauri-specific path/event 类型。
- Desktop 形态中的 Tauri 逻辑只能停留在 `src/apps/desktop`、transport/API adapter 或 Product Assembly 的
  concrete provider 注册侧；下层只消费 typed port、DTO、event fact 或 capability availability。
- 迁移现有 Tauri-adjacent 调用时，先抽稳定 port / provider，再让 desktop provider 实现该 port；不得在同一 PR
  同时改变 command wire shape、权限语义、事件语义或构建脚本。
- 后续 PR 若触碰 desktop/Tauri 边界，必须显式列出哪些能力仍是 Desktop-only，哪些能力已经通过 port
  可被 CLI/Server/Remote/ACP 复用，并补 `cargo check -p bitfun-desktop` 及对应 focused regression。

## 6. 不可变更边界

- 不改变产品行为、默认能力集合、权限语义、工具曝光、事件语义或 session 生命周期。
- 不修改 `package.json`、`scripts/dev.cjs`、`scripts/desktop-tauri-build.mjs`、`scripts/ensure-openssl-windows.mjs`、`scripts/ci/setup-openssl-windows.ps1`、`BitFun-Installer/**`，除非单独作为产品构建变更评审。
- 不让新 crate 依赖回 `bitfun-core`。
- 不把 `bitfun-core` 重新包装成新的 `common`、`platform`、`app context` 或 service locator。
- 不让 runtime owner 或 contract crate 吸收 Tauri / desktop app state；Tauri 只能作为具体 Desktop provider
  或 transport/API adapter 的实现细节。
- 不在同一 PR 中同时做 runtime owner 迁移、default feature 调整、三方库大版本升级和构建脚本变更。
- 不为了减少代码行数抽象语义并不等价的跨产品或跨平台流程。

构建脚本保护命令：

```powershell
git diff -- package.json scripts/dev.cjs scripts/desktop-tauri-build.mjs scripts/ensure-openssl-windows.mjs scripts/ci/setup-openssl-windows.ps1 BitFun-Installer
```

期望结果：没有 diff。

## 7. 验证矩阵

按触碰范围选择最小但足够的验证：

| 触碰范围 | 最小验证 |
|---|---|
| contract / DTO / boundary 文档 | `pnpm run check:repo-hygiene`，必要时补 `node scripts/check-core-boundaries.mjs` |
| Runtime ports / service boundary | `cargo test -p bitfun-runtime-ports`，`cargo check -p bitfun-core --features product-full` |
| Service integrations / Remote | owner crate focused tests，remote-connect / remote-SSH focused tests，`cargo check -p bitfun-core --features product-full` |
| Remote product surfaces | 触碰 remote connection / workspace / projection 时，按范围补 Desktop remote connect、relay / mobile session、ACP remote config reuse、CLI subagent / remote-adjacent path 验证 |
| Harness contract / registry | `cargo test -p bitfun-harness`，`cargo test -p bitfun-core --features product-full product_harness`，`node scripts/check-core-boundaries.mjs` |
| Tool runtime | `cargo test -p bitfun-agent-tools`，tool manifest / `GetToolSpec` / snapshot focused tests，`node scripts/check-core-boundaries.mjs` |
| Product domains | `cargo test -p bitfun-product-domains`，MiniApp / function-agent focused tests |
| Product surface 或 Tauri/API 触碰 | `cargo check -p bitfun-desktop`，检查 Tauri 依赖未下沉到 runtime owner，必要时补 Web UI 或 mobile-web 验证 |
| 大范围 owner 迁移 | `cargo check --workspace`；若行为面广，再补 `cargo test --workspace` |

任何声明构建收益的 PR 必须记录迁移前后 cargo metadata / cargo tree / check 数据；不声明收益时，也不得造成明显编译或运行时退化。

## 8. 暂停条件

- 必须改变用户可见行为、权限策略、产品命令、默认能力或 release 构建形态才能继续。
- 新 owner crate 必须依赖回 `bitfun-core` 才能编译。
- contract crate 开始吸收 Tauri、CLI/TUI、network client、process execution、`git2`、`rmcp`、`image`、`tokio-tungstenite` 等 concrete runtime 依赖。
- Remote / Tool / MiniApp / function-agent / scheduler 迁移无法给出迁移前后等价测试或可复核 snapshot。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- 某个产品 crate 需要减少 feature 才能通过编译。

## 9. 完成标准

- `bitfun-core` 只保留兼容 facade 和产品组装，不再承载新 runtime owner 实现。
- Agent Runtime SDK、Runtime Services、Tool Runtime、Harness、Product Capabilities 与 Concrete Integrations 的依赖方向可由边界检查证明。
- 至少有一组低层 contract / owner crate 可以绕开完整 `bitfun-core` 和对应 heavy dependency。
- 产品 crate 仍拥有拆解前的完整能力集合，且旧公开 import 路径保持兼容。
- Remote、Tool、MiniApp/function-agent、scheduler/registry 等高风险路径都有等价测试、旧路径兼容和回滚边界。
- 新增 crate 数量保持中等粒度；继续拆小必须有 owner、依赖或实测收益依据。
- 已完成事实只记录在归档文档中，主计划持续聚焦当前方向和待完成事项。
