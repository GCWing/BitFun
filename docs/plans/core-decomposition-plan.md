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

### 2.4 目标 crate 创建准入

- 新目标 crate 不能为了“架构完整”提前创建。必须同时满足 owner 边界清晰、旧路径兼容可保留、focused tests 可落地、依赖收益可解释、boundary check 可防回流。
- `bitfun-runtime-services` 优先级最高，但创建前必须先有最小 `RuntimeServicesBuilder` skeleton、Remote ports 和 fake provider 测试。
- `bitfun-agent-runtime` 只能在 session / turn / scheduler / prompt loop 中至少一个 owner 可脱离 `bitfun-core` 构建时创建。
- `bitfun-harness` 只能在至少两个 workflow 通过 provider contract 接入时创建，不能只为单个 Deep Review 或 MiniApp helper 拆 crate。
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

| 顺序 | 主题 | 完整范围 | 不允许混入 | 合入门禁 |
|---|---|---|---|---|
| 0 | Product Assembly / Runtime Services Foundation | 建立最小 Product Assembly skeleton、`RuntimeServicesBuilder` skeleton、Remote ports、fake provider 和 boundary check 入口 | 具体 remote runtime、tool IO、product-domain IO、default feature 调整 | provider 注册路径可测试，Remote ports 不暴露 SSH / relay concrete handle |
| 1 | Service / Agent Remote Runtime Owner | 在 remote connection、remote workspace、remote FS / terminal projection、workspace-root / persistence、`ImageContextData`、remote-SSH / relay provider 中选择一个 owner 主题，完成 port、provider、旧路径兼容和行为等价验证 | tool runtime、product-domain runtime、feature matrix、产品命令或 UI 行为变更 | remote/session/file/image/terminal/scheduler 行为等价，产品 surface 不变 |
| 2 | Agent Runtime SDK Owner | 拆分 mode-scoped subagent visibility、agent registry facts、queue policy decision、scheduler submit/cancel facts 和 background delivery 边界；concrete scheduler 生命周期按保护程度逐步外移 | remote provider、tool IO、product-domain IO、默认 feature 调整 | subagent 可见性、queue/preempt/cancel、background reply、DeepResearch hook 等价 |
| 3 | Harness / Product Capability Boundary | 建立 Harness provider contract，让 Deep Review、DeepResearch、MiniApp 等 workflow 通过 provider 注册，不侵入 Agent Runtime SDK | concrete service IO、tool IO、surface 命令语义变更 | 至少两个 workflow 可通过 provider contract 表达，旧路径兼容 |
| 4 | Product-Domain Runtime Owner | MiniApp filesystem IO / worker / host / builtin seed 或 function-agent Git/AI 中选择一个 owner 主题，建立最小 port/provider 和 core adapter | tool runtime、service/agent runtime、surface 行为变更 | MiniApp/function-agent focused regression，PathManager/process/Git/AI 边界清晰 |
| 5 | Tool Runtime Owner | 仅在收益明确时迁移 `ToolUseContext` projection、manifest execution、`GetToolSpecTool` execution、snapshot wrapper、collapsed unlock state 或具体工具 IO 中的一个 owner 主题 | service/agent runtime、product-domain runtime、feature matrix、产品行为变更 | tool visibility、manifest、`GetToolSpec`、snapshot、Deep Review tool flow 等价 |
| 6 | Feature / Build-Benefit Evaluation | 评估 feature matrix、dependency profile、no-default 编译面和构建收益数据 | runtime owner 迁移、default feature 副作用、构建脚本变更 | cargo metadata / cargo tree 证据，产品入口完整能力不变 |

当前优先级更偏向 **Product Assembly / Runtime Services Foundation**，随后进入 **Service / Agent Remote Runtime Owner**。原因是 Remote 与 OS/terminal/file/network 的实现边界最容易继续牵引 core，但必须先有 typed registration 和 Remote ports，避免继续临时接线。

## 5. 每类 PR 的保护重点

### 5.1 Service / Agent Remote Runtime Owner

- 先定义 remote connection、workspace、projection、capability facts port。
- 保留 core adapter 读取 workspace-root、persistence、session restore、scheduler submit，直到有端到端 remote regression。
- SSH、relay、tunnel、远端 OS、认证差异留在 Remote provider。
- 验证 remote command/response wire、restore -> terminal pre-warm -> scheduler submit 顺序、file full/chunk/info、image context fallback、remote workspace startup guard。

### 5.2 Agent Registry / Scheduler Owner

- 先迁移只读 facts、queue policy decision、runtime event facts，不先移动 concrete scheduler 生命周期。
- 保留 mode-scoped visibility、hidden/custom/review grouping、background result delivery、running-turn injection 和 idle-session follow-up 语义。
- 验证 subagent availability、queue/preempt/cancel suppression、DeepResearch citation / post-turn hook、goal verification events。

### 5.3 Product-Domain Runtime Owner

- MiniApp 优先拆 storage/process/asset/Git/AI 的最小 port，避免把 PathManager、worker process、host dispatch、builtin marker IO 下沉到 domain crate。
- function-agent 保留 Git/AI provider acquisition、error mapping、no-HEAD diff fallback、非 Git workspace fallback、`analyzed_at` 时序。
- 验证 MiniApp import/sync/recompile/rollback/deps state、builtin seed marker、customized update metadata、function-agent prompt/response policy。

### 5.4 Tool Runtime Owner

- 不直接搬全部 concrete tools。先拆 portable facts projection、manifest/catalog snapshot、`GetToolSpec`、snapshot wrapper 或 collapsed unlock 中一个 owner。
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
