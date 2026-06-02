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

后续迁移收敛为 PR-A / PR-B / PR-C 三个大型 PR。每个 PR 必须迁移真实 owner 逻辑，并同时包含旧路径兼容、focused tests、boundary check 和提交前对抗性审核。只新增抽象、只补 facade 或只增加 guard 不满足准出要求。

| PR | 主题 | 完整范围 | 不允许混入 | 合入门禁 |
|---|---|---|---|---|
| PR-A | Agent Runtime SDK Owner Closure | 承接 prompt cache policy / identity / DTO / in-memory store、shared mode profile / context policy、mode / subagent source presentation facts，并保持 core agent registry 与 session manager 旧路径兼容 | concrete scheduler 生命周期、event emitter、post-turn hook、permission `Tool` handler、custom subagent file IO、产品命令或默认 feature 变更 | `bitfun-agent-runtime` 独立测试、core agents / prompt-cache focused tests、boundary check、repo hygiene、`cargo check -p bitfun-core --features product-full` |
| PR-B | Product-Domain + Tool Runtime Owner Closure | 完成 MiniApp builtin bundle asset owner、ToolUseContext runtime handle bundle、product registry materialization、collapsed unlock lifecycle state，并保持旧路径与产品 tool exposure 兼容 | Agent Runtime scheduler / event 行为、Harness execution、feature matrix、UI 或产品语义变更、MiniApp worker / host dispatch、function-agent Git / AI concrete service、具体 IO tool 行为 | MiniApp builtin seed regressions、function-agent facade regressions、tool pipeline focused regressions，runtime/service port 边界清晰，产品 surface 不变 |
| PR-C | Harness / Capability / Build-Benefit Closure | 推进 Harness execution / Product Capability pack / service-tool orchestration；评估 MiniApp worker/host、function-agent Git/AI、具体 IO tools 是否具备进一步外移保护；同时评估 feature matrix、dependency profile、no-default 编译面、构建收益和可选 crate 目录分组 | 默认 feature 副作用、未验证的构建脚本调整、未补等价保护的进程/权限/AI provider 迁移 | Harness workflow 等价、capability pack 注册可测、cargo metadata / cargo tree 证据，产品入口完整能力不变 |

### 4.1 PR-A 实施状态

PR-A 只迁移无 IO、无副作用、可由 contract test 证明等价的 Agent Runtime SDK owner 事实。当前分支已经覆盖：

1. `bitfun-agent-runtime::prompt_cache` 承接 prompt cache schema、policy、identity、DTO、scope key 和 in-memory store。
2. `bitfun-agent-runtime::agents` 承接 shared coding mode profile、mode presentation rank、shared user-context policy、`SubAgentSource` DTO、source kind 与 presentation rank。
3. `bitfun-core` 保留 `agentic::session::prompt_cache`、agent mode module 和 registry DTO 旧路径 re-export；session persistence、clone/cold restore、agent definition loading、custom subagent file IO、scheduler lifecycle 和 event delivery 仍在 core。
4. `scripts/check-core-boundaries.mjs` 已增加 prompt cache、shared mode profile/context、mode/source presentation 和 `SubAgentSource` 的防回流规则。
5. focused tests 覆盖 runtime contracts、core agent registry、prompt cache restore/clone/invalidation 和 boundary check。

不继续纳入 PR-A 的内容：concrete scheduler 生命周期、event delivery / post-turn hook、permission coordination 的 `Tool` handler 和 custom subagent file IO。这些路径直接连接事件发送、调度执行或文件/配置 IO，若迁移必须在 PR-B/PR-C 前单独补可观测行为等价保护，不能作为“纯 owner 事实迁移”处理。

### 4.2 PR-B 实施状态

PR-B 收敛 Product-Domain 与 Tool Runtime 的真实 owner 逻辑，但不移动会改变进程、权限、Git/AI provider 或具体 IO 行为的实现。当前分支覆盖：

1. `bitfun-product-domains::miniapp::builtin::BUILTIN_APPS` 承接内置 MiniApp bundle identity、版本和 embedded asset；`bitfun-core::miniapp::builtin` 保留旧路径 re-export、seed 写盘、marker IO、用户 storage 保留和 recompile。
2. `bitfun-runtime-ports::ToolRuntimeHandles` 承接 tool execution context 的 workspace services 与 cancellation handle bundle；`ToolUseContext` 保留 core owner 类型、path/runtime lookup、portable facts 投影和具体 tool 调用上下文。
3. `product_runtime/materialization.rs` 承接 product provider group plan 到 concrete tool 的 materialization，保持 provider order、tool name 和 registry exposure 不变。
4. `product_runtime/unlock_state.rs` 承接 collapsed unlock 的 message-derived lifecycle state；`ExecutionEngine` 不再直接解析 `GetToolSpec` result。

不继续纳入 PR-B 的内容：MiniApp worker process / host dispatch、builtin marker IO / seed 写盘、function-agent Git / AI concrete service、具体 IO tools。这些路径连接进程执行、权限检查、文件系统、shell、Git、AI provider 或用户可见工具行为，后续若迁移必须先补等价保护后再以完整 owner PR 评估。

### 4.3 PR-C 实施状态

PR-C 收敛 Harness / Product Capability / Build-Benefit closure，不把缺少等价保护的 concrete runtime 行为强行外移。当前分支覆盖：

1. 新增 `bitfun-product-capabilities`，承接产品能力包 assembly facts：capability id、required runtime service capability、tool provider group id selection 和 harness provider selection。
2. `bitfun-harness` 承接 provider-neutral harness descriptor 与 descriptor registry builder；`bitfun-core::agentic::harness` 改为消费 `bitfun-product-capabilities::default_product_harness_registry()`，core 不再硬编码 Deep Review / DeepResearch / MiniApp harness provider descriptor。
3. `bitfun-tool-packs` 承接 tool provider group plan、按 id 选择 plan 和未知 provider group 校验；`ProductToolRuntime` 改为通过 Product Capability assembly 获取默认产品 tool provider group plan，tool group id 选择由 Product Capability pack 表达。
4. `bitfun-agent-tools` 承接 provider-neutral static provider materialization 和 plan-to-registry assembly；core 不再拥有 provider plan 遍历、provider group 构建、未知工具项错误处理或 registry 安装主体算法，只保留具体 tool constructor factory、product plan adapter 和兼容 facade。
5. Product Capability assembly 同时暴露 service requirement、tool provider group plan 和 harness provider selection；上层组装器可用外部 service availability 检查 capability 缺口，不需要依赖 concrete runtime service bundle。
6. `scripts/check-core-boundaries.mjs` 增加 product-capabilities 防回流、harness descriptor owner、agent-tools generic materializer / registry assembly 和 core concrete factory adapter 检查，确保 owner 迁移不会回退成 core 内主体算法。
7. tool-pack selector 对未知 tool provider group 显式报错，static provider materializer 对未知 concrete tool 显式报错，避免配置错误被静默过滤成工具能力缺失。
8. cargo tree / metadata 证据显示 `bitfun-product-capabilities` 只依赖 `bitfun-harness`、`bitfun-runtime-ports`、`bitfun-tool-packs`；`bitfun-core --no-default-features` 不选入 product owner deps，相关 owner 依赖保持 optional。

PR-C 评估后不继续迁移的内容：MiniApp worker process / host dispatch / builtin marker IO / seed 写盘、function-agent Git / AI concrete service、具体 IO tools、Agent Runtime concrete scheduler / event delivery / permission `Tool` handler / post-turn hook。这些路径仍缺少足够的端到端等价保护或会触及进程、权限、AI provider、filesystem/shell、checkpoint hook、session lifecycle 等用户可见行为，后续若迁移必须单独以完整 owner PR 处理。

## 5. 每类 PR 的保护重点

### 5.1 Service / Agent Remote Runtime Owner

- 先定义 remote connection、workspace、projection、capability facts port。
- 保留 core adapter 读取 workspace-root、persistence、session restore、scheduler submit，直到有端到端 remote regression。
- SSH、relay、tunnel、远端 OS、认证差异留在 Remote provider。
- 验证 remote command/response wire、restore -> terminal pre-warm -> scheduler submit 顺序、file full/chunk/info、image context fallback、remote workspace startup guard。

### 5.2 Agent Registry / Scheduler Owner

- 先迁移只读 facts、queue policy decision、runtime event facts，不先移动 concrete scheduler 生命周期。
- 保留 mode-scoped visibility、hidden/custom/review grouping、background result delivery、running-turn injection、idle-session follow-up 和 persisted thread goal continuation 语义。
- thread goal runtime、subagent visibility/availability、round-boundary yield/injection、turn-outcome queue
  policy、finish-reason label、session-state event label 和 turn-outcome event fact 已归入
  `bitfun-agent-runtime`；后续只允许 core 继续作为 metadata store、config/file IO adapter、concrete prompt
  assembly、concrete scheduler lifecycle、scheduler delivery、event delivery 和 `Tool` adapter。
- 验证 subagent availability、queue/preempt/cancel suppression、DeepResearch citation / post-turn hook、goal verification events、`get_goal` / `create_goal` / `update_goal` tool response wire shape。

### 5.3 Product-Domain Runtime Owner

- MiniApp 已将 builtin bundle identity、版本和 embedded asset 放入 `bitfun-product-domains`；core 继续负责 seed 写盘、marker IO、用户 storage 保留、recompile、PathManager、worker process 和 host dispatch。
- 后续若继续迁移 MiniApp worker / host，必须先拆清 process runtime、permission policy、host primitive dispatch、draft worker 与 active worker 的等价边界，不能把 PathManager 或 worker process 下沉到 domain crate。
- function-agent 保留 Git/AI provider acquisition、error mapping、no-HEAD diff fallback、非 Git workspace fallback、`analyzed_at` 时序。
- 验证 MiniApp import/sync/recompile/rollback/deps state、builtin seed marker、customized update metadata、function-agent prompt/response policy。

### 5.4 Tool Runtime Owner

- 已完成 deterministic execution admission gate 迁移；core 仅保留状态更新、registry lookup、input validation、confirmation、实际执行和 hook。
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
- 后续不直接搬全部 concrete tools；具体 IO tools 只有在权限、filesystem/shell 行为和 checkpoint hook 均有等价保护时才允许进入 PR-C。
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
