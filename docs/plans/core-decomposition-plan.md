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

后续迁移固定收敛为 7 个大块 PR。每个 PR 都必须先补保护，再迁移 owner，最后回看文档和边界；如果发现必须改变功能语义，需要在 PR 中单独说明原因、影响范围和回滚边界。

| PR | 主题 | 完整范围 | 不允许混入 | 合入门禁 |
|---|---|---|---|---|
| PR1 | Product Assembly / Runtime Services Foundation | 创建 `bitfun-runtime-services`，补 `RuntimeServicesBuilder`、typed provider registration、capability availability、Remote ports、fake provider 和 boundary check 入口 | 具体 remote runtime、tool IO、product-domain IO、default feature 调整 | provider 注册路径可测试，Remote ports 不暴露 SSH / relay concrete handle，新增 crate 不依赖 `bitfun-core` |
| PR2 | Service / Agent Remote Runtime Owner | 在 remote connection、remote workspace、remote FS / terminal projection、workspace-root / persistence、`ImageContextData`、remote-SSH / relay provider 中完成一个完整 owner 主题的 port、provider、旧路径兼容和行为等价验证 | tool runtime、product-domain runtime、feature matrix、产品命令或 UI 行为变更 | remote/session/file/image/terminal/scheduler 行为等价，产品 surface 不变 |
| PR3 | Agent Runtime SDK Owner | 拆分 mode-scoped subagent visibility、agent registry facts、queue policy decision、scheduler submit/cancel facts 和 background delivery 边界；concrete scheduler 生命周期按保护程度逐步外移 | remote provider、tool IO、product-domain IO、默认 feature 调整 | subagent 可见性、queue/preempt/cancel、background reply、DeepResearch hook 等价 |
| PR4 | Harness / Product Capability Boundary | 建立 Harness provider contract，让 Deep Review、DeepResearch、MiniApp 等 workflow 通过 provider 注册，不侵入 Agent Runtime SDK | concrete service IO、tool IO、surface 命令语义变更 | 至少两个 workflow 可通过 provider contract 表达，旧路径兼容 |
| PR5 | Product-Domain Runtime Owner | MiniApp filesystem IO / worker / host / builtin seed 或 function-agent Git/AI 中完成一个完整 owner 主题，建立最小 port/provider 和 core adapter | tool runtime、service/agent runtime、surface 行为变更 | MiniApp/function-agent focused regression，PathManager/process/Git/AI 边界清晰 |
| PR6 | Tool Runtime Owner | 继续收敛 tool runtime owner：已完成 deterministic execution admission gate、`GetToolSpecTool` product runtime owner closure、manifest/catalog/snapshot owner closure；本阶段迁移 workspace service contract 与 collapsed unlock observation owner，core pipeline 只保留执行编排和旧路径兼容 | service/agent runtime、product-domain runtime、feature matrix、产品行为变更、全量 concrete tool IO 搬迁 | tool pipeline focused tests、product runtime focused tests、manifest facade parity tests、`bitfun-agent-tools` / `bitfun-runtime-ports` contract tests、boundary check |
| PR7 | Feature / Build-Benefit Evaluation | 评估 feature matrix、dependency profile、no-default 编译面和构建收益数据，确认是否具备收敛默认 feature 的条件 | runtime owner 迁移、default feature 副作用、构建脚本变更 | cargo metadata / cargo tree 证据，产品入口完整能力不变 |

### 4.1 PR1 具体实施计划

PR1 是后续高风险迁移的前置门禁，目标是提供可测试的 typed assembly 基础，而不是移动任何既有业务行为。

1. 新建 `bitfun-runtime-services` crate，并加入 workspace。
2. 在 `bitfun-runtime-ports` 中补齐 Runtime Services 所需的轻量 port trait 和 Remote port trait；这些 trait 只能描述能力和请求边界，不携带 SSH、relay、Tauri、process、filesystem manager 等 concrete handle。
3. 在 `bitfun-runtime-services` 中实现 `RuntimeServices`、`RuntimeServicesBuilder`、capability availability、typed unsupported error 和 provider registry。
4. 提供 `test_support` fake provider，覆盖本地 mandatory service、optional remote service 和 unsupported capability 三类注入路径。
5. 更新 `scripts/check-core-boundaries.mjs`，把 `bitfun-runtime-services` 纳入 no-core dependency 和轻量依赖边界检查。
6. 更新仓库入口文档中的模块索引，说明 `bitfun-runtime-services` 仍使用 core decomposition guardrails。
7. 运行 focused tests、边界检查和最小 Rust 验证；提交前从第三方视角检查是否出现 service locator、全局 mutable registry、反向依赖或功能语义漂移。

PR1 不迁移任何 concrete service owner，因此预期不会修改产品行为、默认能力集合、权限语义、工具曝光、事件语义、session 生命周期或构建脚本。

### 4.2 PR2 + PR3 合并实施计划

本次 PR 合并推进 PR2 和 PR3，但仍按两个 owner 主题顺序实施，避免把 remote provider、agent scheduler
和产品 surface 行为混在同一个迁移步骤中。若实现过程中发现必须改变用户可见行为、默认 feature、权限语义或构建形态，
应暂停并在 PR 描述中单独说明设计偏移原因、影响范围和回滚边界。

#### 4.2.1 PR2：Service / Agent Remote Runtime Owner

目标是在不搬动 concrete SSH / relay / terminal / session restore 实现的前提下，把 remote workspace 与 projection
的稳定接口归入 `bitfun-runtime-ports`，并保留 `bitfun-services-integrations::remote_connect` 旧路径 re-export。

1. 在 `bitfun-runtime-ports` 中承接 remote workspace facts、remote session metadata、remote workspace file projection DTO
   和 `RemoteWorkspacePort` / `RemoteProjectionPort` owner trait。
2. 在 `bitfun-services-integrations::remote_connect` 中删除重复 owner 定义，改为 re-export 新 owner crate 的类型和 trait，
   保持现有调用方 import 路径兼容。
3. 让 core 侧 remote workspace / file adapter 继续作为具体 provider，实现新的 stable port；workspace-root、
   persistence、session restore、terminal pre-warm 和 scheduler submit 仍保留在 `bitfun-core`。
4. 补充 focused tests，覆盖 remote workspace / file projection 类型通过旧路径与新 owner 路径保持等价，以及
   `RuntimeServicesBuilder` 能注册带方法的 remote workspace / projection provider。
5. 更新 boundary check，防止 remote owner contract 回流到 `bitfun-core` 或 concrete service crate。

#### 4.2.2 PR3：Agent Runtime SDK Owner

目标是创建有真实 owner 的 `bitfun-agent-runtime`，只迁移 scheduler/background delivery 和 thread goal 这类可被
contract 保护的运行时决策，不外移 concrete scheduler 生命周期。

1. 新建 `bitfun-agent-runtime` crate，并加入 workspace；该 crate 只依赖 `bitfun-runtime-ports` 等稳定契约，不依赖
   `bitfun-core`、Tauri、CLI、ACP、Web UI 或 concrete service crate。
2. 先把 background delivery 的状态决策抽为 `bitfun-agent-runtime` 的纯 contract：Processing 注入当前运行 turn，
   Missing / Idle / Error 提交 agent-session follow-up turn。
3. 最新主干已把 persisted thread goal 的 portable DTO / status / continuation plan 放入 `bitfun-runtime-ports`，
   并新增 `get_goal` / `create_goal` / `update_goal` 工具；后续 Agent Runtime SDK 迁移必须把 `ThreadGoalRuntime`、
   store、token subscriber、round injection 和 goal tool handler 当作同一 owner 主题保护，不能把它误归入普通 concrete tool IO。
4. core scheduler 仅调用该决策结果，继续负责 injection buffer、submit、turn id、metadata 和实际生命周期执行。
5. 补充 `bitfun-agent-runtime` focused tests 与 core scheduler / thread goal 兼容验证，确保 background reply、cancel suppression、
   queue/preempt 和 DeepResearch/post-turn 相关语义没有漂移。
6. 更新 `AGENTS.md` / `AGENTS-CN.md` 和设计文档中的 crate 状态，描述 `bitfun-agent-runtime` 已承接的范围以及仍未外移的
   scheduler lifecycle、session manager、prompt loop、subagent registry 和 thread goal concrete runtime。

#### 4.2.3 本次 PR 验收

- 不修改产品命令、UI、默认 feature、release / fast build 脚本或产品能力集合。
- 不新增反向依赖、无类型 service locator、全局 mutable registry 或重复 runtime materialization。
- 必须通过 remote / runtime owner focused tests、boundary check、repo hygiene 和最小 Rust 编译验证。
- 提交前从第三方视角审查功能偏移、性能劣化、跨产品形态遗漏、文档与代码不一致，并修复发现的问题。

### 4.3 PR4：Harness / Product Capability Boundary 实施计划

PR4 的目标是建立 Harness contract 和迁移期 provider 注册边界，而不是外移 Deep Review、DeepResearch
或 MiniApp 的具体执行逻辑。

1. 新建 `bitfun-harness` crate，并加入 workspace；该 crate 只承接 provider-neutral workflow、capability、
   plan、step、outcome、error 和 registry contract。
2. 提供 descriptor-only `HarnessProvider`，支持 legacy-facade route plan；`execute` 在 PR4 阶段必须返回
   typed unsupported，避免形成“执行已迁移”的错觉。
3. 在 `bitfun-core::agentic::harness` 注册 Deep Review、DeepResearch、MiniApp 三个 legacy-facade provider，
   只表达现有 workflow 的归属和 route，不改变产品命令、session、tool、service IO 或 UI 语义。
4. 将 `bitfun-harness` 纳入 boundary check，禁止依赖 `bitfun-core`、具体 service crate、product-domain
   implementation、AI adapter、transport、Tauri、Git/MCP/image/WebSocket 等 concrete runtime 依赖。
5. 补充 `bitfun-harness` focused tests 和 core registry 兼容测试，证明至少两个以上 workflow 可以通过
   provider contract 表达，且 concrete execution 仍停留在旧路径。
6. 更新架构、计划和 AGENTS 文档，明确 PR4 只完成 contract / registry boundary；执行迁移、product
   command registry、capability pack 和 service/tool orchestration 仍属于后续 PR。

PR4 不迁移 concrete service IO、tool IO、surface command 语义、session manager、scheduler 生命周期或构建 feature。
如后续要让 Harness 实际执行 workflow，必须在独立 PR 中补行为等价测试和回滚边界。

## 5. 每类 PR 的保护重点

### 5.1 Service / Agent Remote Runtime Owner

- 先定义 remote connection、workspace、projection、capability facts port。
- 保留 core adapter 读取 workspace-root、persistence、session restore、scheduler submit，直到有端到端 remote regression。
- SSH、relay、tunnel、远端 OS、认证差异留在 Remote provider。
- 验证 remote command/response wire、restore -> terminal pre-warm -> scheduler submit 顺序、file full/chunk/info、image context fallback、remote workspace startup guard。

### 5.2 Agent Registry / Scheduler Owner

- 先迁移只读 facts、queue policy decision、runtime event facts，不先移动 concrete scheduler 生命周期。
- 保留 mode-scoped visibility、hidden/custom/review grouping、background result delivery、running-turn injection、idle-session follow-up 和 persisted thread goal continuation 语义。
- 验证 subagent availability、queue/preempt/cancel suppression、DeepResearch citation / post-turn hook、goal verification events、`get_goal` / `create_goal` / `update_goal` tool response wire shape。

### 5.3 Product-Domain Runtime Owner

- MiniApp 优先拆 storage/process/asset/Git/AI 的最小 port，避免把 PathManager、worker process、host dispatch、builtin marker IO 下沉到 domain crate。
- function-agent 保留 Git/AI provider acquisition、error mapping、no-HEAD diff fallback、非 Git workspace fallback、`analyzed_at` 时序。
- 验证 MiniApp import/sync/recompile/rollback/deps state、builtin seed marker、customized update metadata、function-agent prompt/response policy。

### 5.4 Tool Runtime Owner

- 已完成 deterministic execution admission gate 迁移；core 仅保留状态更新、registry lookup、input validation、confirmation、实际执行和 hook。
- 已完成 `GetToolSpecTool` concrete adapter 的 product runtime owner closure；generic concrete-tool implementations
  只保留兼容 re-export，不再拥有 on-demand spec discovery Tool impl。
- 已完成 manifest/catalog/snapshot owner closure；`manifest_resolver.rs` 只保留旧路径兼容 facade，product runtime
  的 `catalog.rs` / `snapshot.rs` 管理 resolved manifest DTO、visible tools、readonly catalog、GetToolSpec catalog
  path 和 snapshot wrapper。
- 本阶段继续完成两项 owner 收敛：`WorkspaceFileSystem`、`WorkspaceShell`、`WorkspaceServices` 等 workspace service
  contract 归入 `bitfun-runtime-ports`，core `workspace.rs` 只保留旧路径 re-export 和 local/remote concrete adapter；
  collapsed unlock 的 GetToolSpec observation adapter 归入 `product_runtime/unlock_state.rs`，`ExecutionEngine` 不再拥有
  GetToolSpec 结果解析细节。
- workspace service contract 暂时保留既有 `anyhow::Result` 和 `CancellationToken` 语义，避免在 owner 迁移 PR 中同时改变
  错误分类、取消语义或调用方边界；后续若要收敛为 portable `PortResult`，必须单独补错误映射等价测试。
- 后续不直接搬全部 concrete tools。只在 collapsed unlock persistence、product registry materialization、
  `ToolUseContext` concrete service handles 或具体工具 IO 中选择能减少旧路径的完整 owner。
- 保留 tool name、schema、prompt stub、readonly/enabled/filtering、unlock state 生命周期。
- 验证 builtin tool list、provider order、expanded/collapsed exposure、dynamic provider metadata、Deep Review 修改类工具 checkpoint hook。

## 6. 不可变更边界

- 不改变产品行为、默认能力集合、权限语义、工具曝光、事件语义或 session 生命周期。
- 不修改 `package.json`、`scripts/dev.cjs`、`scripts/desktop-tauri-build.mjs`、`scripts/ensure-openssl-windows.mjs`、`scripts/ci/setup-openssl-windows.ps1`、`BitFun-Installer/**`，除非单独作为产品构建变更评审。
- 不让新 crate 依赖回 `bitfun-core`。
- 不把 `bitfun-core` 重新包装成新的 `common`、`platform`、`app context` 或 service locator。
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
| Product surface 或 Tauri/API 触碰 | `cargo check -p bitfun-desktop`，必要时补 Web UI 或 mobile-web 验证 |
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
