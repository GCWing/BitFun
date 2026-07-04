# OpenCode 插件兼容暴露面审计

本文独立审视 BitFun 当前核心迁移、公共 API 暴露面和未来受控接入 OpenCode
插件生态的风险。本文不替代 `core-decomposition.md` 和
`agent-runtime-services-design.md`，也不记录单次 PR 进度或维护独立执行路线图。

## 1. 复核方式

本文记录架构风险和设计结论，不固化本机路径或临时分支快照。复核时应重新执行以下最小证据检查：

- `git fetch gcwing main` 后对照 `gcwing/main` 检查 BitFun 当前 public surface、owner crate 和旧路径。
- 在 OpenCode 工作区对照当前插件、tool registry、event、permission 和 TUI contribution 设计。
- 在 Claw Code / Claude Code 类实现中对照 full product、lean automation / SDK 和 service 形态拆分。
- 使用 `cargo metadata --no-deps`、`cargo tree`、workspace boundary checks 和目标文档 diff 验证依赖边界。

审计目标：

1. 校验已有迁移是否真正形成 owner 边界，而不是只移动文件或新增 facade。
2. 校验当前公共 API 是否适合作为 Agent Runtime SDK、插件 host 或 OpenCode adapter 的基础。
3. 识别 OpenCode 分级适配前必须提前纳入计划的高风险工作。
4. 区分必须立即收敛的架构债务和不应过早稳定的外部兼容承诺。

## 2. 复核结论

上一次审计中的核心担忧成立：BitFun 已经拆出多个 owner crate，但公共暴露面、
旧路径兼容层和部分合同对象仍然偏宽，迁移完成度不能只按物理目录判断。

需要修正的判断是：代码行数、`pub mod` 数量或 `cargo tree` 依赖数量本身不是错误。
它们只是风险信号。真正需要阻断后续迁移的是以下情况：

- 新 owner 已存在，但上层仍把旧 owner 的具体对象当作主 API。
- 稳定合同同时承载多个领域的内部 wire shape，导致调用方被迫依赖过宽语义。
- Product Assembly 之外的模块同时认识接口和 concrete provider。
- 插件或 SDK 入口需要直接导入 `bitfun-core/product-full`、产品命令 registry 或完整
  `RuntimeServices` bundle 才能工作。

因此，后续计划不应追求“马上重写成 OpenCode 插件系统”，而应先做公共面收口：
把 stable external API、workspace-internal API 和 compatibility API 分开，再基于 BitFun
自己的 Plugin Runtime Host 承接 OpenCode adapter。

## 3. 竞品结构信号

### 3.1 OpenCode

OpenCode 的关键设计不是某个单独 crate 或包名，而是公共面分层：

- Server plugin API 和 TUI plugin API 分离。服务端插件拿到 project、worktree、
  client、tool、permission、hook 等能力；TUI 插件拿到 route、slot、keymap、
  dialog、toast、state、theme 等 UI 能力。
- Plugin host 通过稳定 handle、transform、registration API 连接内部服务，不把
  manager、registry 或内部状态对象直接暴露给插件。
- Tool definition 是不透明值对象；tool registry 在执行前 materialize 当前可用工具
  快照，并按 permission 过滤，执行时可以识别 stale tool call。
- Event 服务有 manifest、version、aggregate sequence、durable replay 和 listener
  隔离。插件消费事件合同，而不是直接读取 session 内部结构。
- Permission 服务保持权威；插件 hook 可参与 ask 流程，但不能绕过最终安全控制面。

对 BitFun 的结论：OpenCode adapter 不应成为 BitFun 内部真实 owner。
BitFun 需要先有 Rust Kernel API、UI Extension Contract、Tool ABI、Event Manifest 和
Permission/Effect Control Plane，再把 OpenCode API 映射到这些合同。

### 3.2 Claw Code

Claw Code 的产品拆分提供了另一个有用信号：完整 CLI、轻量 automation harness 和
独立 RAG service 被分成不同产品能力；安全、权限、NDJSON 输出、session 和 tool
contract 被明确约束。它也暴露了一个反例风险：runtime / tools / commands 的公共
导出面很容易随功能增加而变宽，长期会降低 SDK 边界的可解释性。

对 BitFun 的结论：完整产品、SDK、CLI、Web、ACP、Remote 不应共享同一套全量公开面。
轻量形态需要窄 API 和明确能力矩阵；完整形态可以由 Product Assembly 注入更多 provider。

### 3.3 Codex / Claude Code 类产品

同类产品的共同趋势是：Agent 内核、安全控制、工具执行、MCP/插件扩展、UI/命令入口和
平台 provider 分开演进。用户可见命令和设置负责把能力外放；内核保持 session、
permission、event、tool request、model routing 等通用事实；外部生态通过 descriptor、
hook、tool provider 或协议 bridge 接入。

对 BitFun 的结论：生态插件能力不应从 `/goal`、DeepReview、MiniApp 或某个产品命令
反推出来，而应由统一 extension contract 向产品特性注册 contribution。

## 4. BitFun 当前暴露面复核

### 4.1 `bitfun-core`

结论：当前 `bitfun-core` 仍是 compatibility facade 与 product-full assembly 的混合体。
它可以作为过渡层存在，但不能继续成为新功能主入口。

风险信号：

- `bitfun-core` 仍 re-export `ExecutionEngine`、`StreamProcessor`、`ToolPipeline`、
  `ToolRegistry`、`BackendEventManager`、`ConfigManager`、`WorkspaceManager` 等具体对象。
- no-default 构建仍牵引多个 runtime/service/transport crate 和若干三方库。该事实不等于错误，
  但说明它还不是可对外承诺的薄 SDK facade。
- core 中大量旧路径 re-export 合理用于兼容，但如果新代码继续依赖这些路径，迁移会回流。

准出要求：

- 新调用方不得新增 `bitfun-core::agentic::*`、`bitfun-core::service::*` 作为主依赖。
- 旧路径必须标记为 compatibility API；真实 owner 应在对应 crate 的 `api` / `prelude`
  或模块级合同中公开。
- `bitfun-core` 只能选择、组装或转发，不保留新 owner 的核心状态机或 concrete provider。

### 4.2 `bitfun-agent-runtime`

结论：它已承接大量 Agent 事实和决策逻辑，但顶层公共面仍像 workspace-internal 实现集合，
不适合直接作为外部 Agent Runtime SDK。

合理部分：

- 在 workspace 内迁移期，多个 `pub mod` 有助于从 core 旧路径转发并保持测试可见性。
- 当前 API version 仍是 preview，不应为了外部发布过早做破坏性收口。

需要补齐：

- 区分 `sdk` / `api` / `prelude` 的稳定外部面，与 workspace-internal 模块。
- DeepReview、custom agent、skills、thread goal 等产品或策略模块不能自然变成 SDK 顶层合同。
- SDK 调用方应只看到 builder、runner、request/response、event stream、typed error、
  registry/provider handle，而不是 session manager 内部结构。

### 4.3 `runtime-ports`

结论：`runtime-ports` 当前承担了过多领域合同。问题不是文件长度，而是不同领域 DTO 和
port 的版本、依赖和安全语义被绑在一起。

高风险混合领域：

- OS/service port、workspace filesystem/shell、terminal execution。
- permission、runtime event、remote workspace/projection/capability。
- agent dialog/session/thread goal/dynamic tool/transcript。

建议方向：

- 先按模块分组和导出面分类，避免立即大规模 crate 拆分带来 churn。
- 对外稳定面按 service ports、agent lifecycle、tool ABI、remote/session workspace、
  permission/effect、event manifest 分域。
- 合同对象避免泄漏当前实现字段。新字段必须有默认值、版本策略和兼容测试。

### 4.4 `RuntimeServices`

结论：`RuntimeServices` 适合作为 Product Assembly 构建出的内部 typed bundle，但不适合
直接作为插件或外部 SDK 的公共 API。

风险：

- 公开字段让调用方天然知道全部底层能力，容易退化为 service locator。
- 插件如果拿到完整 bundle，会绕过 capability-scoped handle 和 permission/effect 声明。

建议方向：

- Product Assembly 可以持有完整 bundle。
- Kernel、Execution、Extension、Plugin 只拿到能力子集 view，例如 workspace、tool、
  permission、event、artifact、remote facts。
- SDK API 暴露 builder 注入点和 narrow runtime context，不暴露完整 bundle 字段。

### 4.5 Tool contract

结论：当前 `tool-contracts` 已经承接了很多 provider-neutral 工具语义，但通用 ABI 和
BitFun 产品策略仍有混合。

应保留在 Tool ABI 的内容：

- tool name、schema、description、execute context、result、attachment、metadata。
- permission/effect 声明、readonly/concurrency facts、artifact reference、cancellation。
- materialized snapshot、stale call guard、provider identity。

应迁出或作为 decorator 的内容：

- collapsed tool 的产品提示策略。
- MiniApp headless restriction。
- delegation policy 对具体 tool 名称的默认拦截。
- 特定产品功能的 tool 排序和 manifest 文案。

### 4.6 Product capability / Harness

结论：`legacy_facade` 是合理过渡标记，不应被写成“已迁移完成”。它只说明路由计划已归档，
不说明 concrete workflow execution 已归 owner。

建议方向：

- capability pack 描述 service、tool group、harness provider、UI contribution、extension
  capability 的组合关系。
- 当 concrete execution 迁移后，capability pack 只引用 owner provider id。
- 对 DeepReview、MiniApp、DeepResearch 等复杂功能，迁移完成必须包含执行主体迁移、
  UI/command descriptor、event/permission 等价和旧路径收敛。

## 5. OpenCode 适配的未计划高风险项

以下事项必须进入后续计划，否则 OpenCode 分级适配会把当前边界债务固化为外部合同。

| 风险项 | 风险 | 解决方法 |
|---|---|---|
| 插件生命周期 | 插件 install / activate / deactivate / reload / dispose 不清晰，容易泄漏状态或无法回滚 | 建立 BitFun Plugin Runtime Host lifecycle，所有 provider/contribution 注册必须可撤销 |
| Rust / UI API 混用 | OpenCode server plugin 和 TUI plugin 能力不同，混用会让 UI 依赖进入内核 | 通过 Rust Kernel API / Plugin Runtime Host contract 与 UI Extension Contract 分别承接，再由 Product Assembly 统一注册 |
| Tool ABI 不稳定 | 插件 tool、MCP tool、built-in tool 走不同路径，permission 与 stale call 行为不一致 | 建立统一 materialized tool snapshot、provider identity、permission/effect filter 和 stale call guard |
| Event 无版本合同 | 插件消费内部事件字段后，后续重构会破坏生态 | 定义 public event manifest、version、aggregate identity、durable/replay 口径和 UI projection |
| Permission hook 越权 | 插件 hook 可能绕过最终授权或写 audit state | hook 只能产出 candidate decision；最终 decision、audit、policy write 由安全控制面完成 |
| UI contribution 缺口 | 没有 slots/routes/keymap/dialog/prompt/state view，OpenCode TUI 插件无法等价映射 | 先定义 descriptor-only UI host contract，再按 Desktop/Web/CLI 能力逐步实现 |
| Workspace/Remote 不一致 | 插件假设本地路径会破坏 remote、relay、web、SDK 形态 | 暴露 workspace identity、logical path、artifact URI、remote capability facts，不暴露本地绝对路径 |
| 外部包安全 | JS/TS 插件 runtime 涉及包来源、权限、secret、网络和崩溃隔离 | 第一阶段只做 native extension contract 和受限 adapter；JS runtime 需独立安全评审 |
| Config/provider transform | 直接开放 provider/model/config transform 会影响全局行为 | 采用 support matrix，先开放只读或 scoped transform，再补审计与回滚 |
| 产品能力漂移 | Desktop、CLI、Web、ACP、SDK 对插件能力支持不同 | 在 Product Assembly 维护 capability matrix 和 unsupported/unavailable contract |

## 6. 与实施计划的关系

后续执行节奏只由 [`core-decomposition-plan.md`](../plans/core-decomposition-plan.md) 维护。
本文只提供风险排序和计划映射，避免审计文档成为第二份路线图。

| 风险主题 | 计划映射 | 必须保留的准出要求 |
|---|---|---|
| 旧 public surface 过宽 | Stage A：Public API Closure | stable external、workspace-internal、compatibility API 明确分层，并阻断旧 core 路径回流 |
| Tool ABI / runtime context 混合 | Stage B：Tool ABI、Event Manifest 与 Security Control Plane | materialized snapshot、provider identity、permission/effect filter、stale call guard、public event manifest、version、aggregate identity、replay/retention 具备测试 |
| Plugin Runtime Host 生命周期和安全桥接 | Stage C：Plugin Runtime Host Foundation | contribution 以 descriptor 暴露，Product Assembly 内部 materialize provider；注册可撤销，候选效果不能写权威状态 |
| UI 扩展合同缺口 | Stage D：UI Extension Contract 与产品形态矩阵 | descriptor-only、只读 state view、入口 fallback 和 unsupported/unavailable 行为具备 round-trip 测试 |
| OpenCode 分级适配 | Stage E：OpenCode Compatibility Adapter | support matrix、typed unsupported、permission/effect、event manifest、UI contribution 和远程/workspace 映射全部可验证 |
| 剩余 concrete owner | Stage F：剩余 Concrete Owner 与 SDK Readiness | Product Assembly 选择 concrete provider；普通层级只依赖 port、descriptor 或 stable contract |

## 7. 执行准则

- 不把 OpenCode API 直接稳定成 BitFun 内部 API。
- 不把 full `RuntimeServices` bundle 或 `bitfun-core/product-full` 暴露给插件或 SDK。
- 不把 UI implementation、Tauri state、React component 或具体 provider handle 下沉到内核。
- 不接受只新增抽象、不删除或收敛旧路径的迁移 PR。
- 不在安全控制面未完成前开放可写插件 hook。
- 任何会改变工具曝光、权限语义、事件字段、remote 行为或产品能力矩阵的变更必须单独评审。
