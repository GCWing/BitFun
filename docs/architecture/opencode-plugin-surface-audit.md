# OpenCode 插件兼容暴露面审计

本文件审视 BitFun 当前核心迁移、公共 API 暴露面和未来受控接入 OpenCode
插件生态的风险。本文件不替代 [`product-architecture.md`](product-architecture.md) 和
[`agent-runtime-services-design.md`](agent-runtime-services-design.md)，也不记录单次 PR 进度或维护独立执行路线图。

## 1. 复核方式

本文件记录架构风险和设计结论，不固化本机路径或临时分支快照。复核时应重新执行以下最小证据检查：

- `git fetch gcwing main` 后对照 `gcwing/main` 检查 BitFun 当前公开暴露面、归属 crate 和旧路径。
- 在 OpenCode 工作区对照当前插件、工具注册表、事件、权限和 TUI 贡献设计。
- 在 Claw Code / Claude Code 类实现中对照完整产品、轻量自动化 / SDK 和服务形态拆分。
- 使用 `cargo metadata --no-deps`、`cargo tree`、工作区边界检查和目标文档 diff 验证依赖边界。

审计目标：

1. 校验已有迁移是否真正形成归属边界，而不是只移动文件或新增门面。
2. 校验当前公共 API 是否适合作为 Agent Runtime SDK、插件主机或 OpenCode 适配器的基础。
3. 识别 OpenCode 分级适配前必须提前纳入计划的高风险工作。
4. 区分必须立即收敛的架构债务和不应过早稳定的外部兼容承诺。

## 2. 复核结论

上一次审计中的核心担忧成立：BitFun 已经拆出多个归属 crate，但公共暴露面、
旧路径兼容层和部分契约对象仍然偏宽，迁移完成度不能只按物理目录判断。

代码行数、`pub mod` 数量或 `cargo tree` 依赖数量本身不构成架构错误；它们只作为风险信号。以下情况必须阻断后续迁移：

- 新归属模块已存在，但上层仍把旧归属模块的具体对象当作主 API。
- 稳定契约同时承载多个领域的内部线缆形态，导致调用方被迫依赖过宽语义。
- 产品组装之外的模块同时认识接口和具体提供方。
- 插件或 SDK 入口需要直接导入 `bitfun-core/product-full`、产品命令注册表或完整
  `RuntimeServices` bundle 才能工作。

后续计划应优先完成公共暴露面收口，而不是以“立即重写成 OpenCode 插件系统”为目标。稳定外部 API、工作区内部 API 和兼容 API 必须先分层，再由 BitFun 自身的插件运行时主机承接 OpenCode 适配器。

## 3. 竞品结构信号

### 3.1 OpenCode

OpenCode 对 BitFun 的主要参考价值在于公共面分层：

- Server 插件 API 和 TUI 插件 API 分离。服务端插件拿到项目、worktree、
  客户端、工具、权限、钩子等能力；TUI 插件拿到路由、槽位、键位映射、
  dialog、toast、状态、主题等界面能力。
- 插件主机通过稳定句柄、转换和注册 API 连接内部服务，不把
  管理器、注册表或内部状态对象直接暴露给插件。
- 工具定义是不透明值对象；工具注册表在执行前物化当前可用工具
  快照，并按权限过滤，执行时可以识别陈旧工具调用。
- 事件服务有清单、版本、聚合序列、持久回放和监听器隔离。插件消费事件契约，而不是直接读取会话内部结构。
- 权限服务保持权威；插件钩子可参与 ask 流程，但不能绕过最终安全控制面。

对 BitFun 的结论：OpenCode 适配器不应成为 BitFun 内部真实归属模块。
BitFun 需要先有 Rust 内核 API、界面扩展契约、工具 ABI、事件清单和
权限/副作用控制面，再把 OpenCode API 映射到这些契约。

### 3.2 Claw Code

Claw Code 的产品拆分提供了另一个参考信号：完整 CLI、轻量自动化工作流和
独立 RAG 服务被分成不同产品能力；安全、权限、NDJSON 输出、会话和工具
契约被明确约束。它也暴露了一个反例风险：运行时、工具和命令的公共
导出面容易随功能增加而变宽，长期会降低 SDK 边界的可解释性。

对 BitFun 的结论：完整产品、SDK、CLI、Web、ACP、Remote 不应共享同一套全量公开面。
轻量形态需要窄 API 和明确能力矩阵；完整形态可以由产品组装注入更多提供方。

### 3.3 Codex / Claude Code 类产品

同类产品的共同趋势是：Agent 内核、安全控制、工具执行、MCP/插件扩展、UI/命令入口和
平台提供方分开演进。用户可见命令和设置负责把能力外放；内核保持会话、
权限、事件、工具请求、模型路由等通用事实；外部生态通过描述符、
钩子、工具提供方或协议桥接接入。

对 BitFun 的结论：生态插件能力不应从 `/goal`、DeepReview、MiniApp 或某个产品命令
反推出来，而应由统一扩展契约向产品特性注册贡献。

## 4. BitFun 当前暴露面复核

### 4.1 `bitfun-core`

结论：当前 `bitfun-core` 仍是兼容门面与 product-full 组装的混合体。它可作为过渡层存在，但不得继续成为新功能主入口。

风险信号：

- `bitfun-core` 仍 re-export `ExecutionEngine`、`StreamProcessor`、`ToolPipeline`、
  `ToolRegistry`、`BackendEventManager`、`ConfigManager`、`WorkspaceManager` 等具体对象。
- no-default 构建仍牵引多个运行时/服务/传输 crate 和若干三方库。该事实不等于错误，
  但说明它还不是可对外承诺的薄 SDK 门面。
- core 中大量旧路径 re-export 合理用于兼容，但如果新代码继续依赖这些路径，迁移会回流。

验收要求：

- 新调用方不得新增 `bitfun-core::agentic::*`、`bitfun-core::service::*` 作为主依赖。
- 旧路径必须标记为兼容 API；真实归属模块应在对应 crate 的 `api` / `prelude`
  或模块级契约中公开。
- `bitfun-core` 只能选择、组装或转发，不保留新归属模块的核心状态机或具体提供方。

### 4.2 `bitfun-agent-runtime`

结论：它已承接大量智能体事实和决策逻辑，但顶层公共面仍像工作区内部实现集合，
不适合直接作为外部 Agent Runtime SDK。

合理部分：

- 在 workspace 内迁移期，多个 `pub mod` 有助于从 core 旧路径转发并保持测试可见性。
- 当前 API version 仍是 preview，不应为了外部发布过早做破坏性收口。

需要补齐：

- 区分 `sdk` / `api` / `prelude` 的稳定外部面，与工作区内部模块。
- DeepReview、自定义智能体、skills、thread goal 等产品或策略模块不能自然变成 SDK 顶层契约。
- SDK 调用方应只看到 builder、runner、请求/响应、事件流、类型化错误、
  注册表/提供方句柄，而不是会话管理器内部结构。

### 4.3 `runtime-ports`

结论：`runtime-ports` 当前承担了过多领域契约。问题不是文件长度，而是不同领域 DTO 和
port 的版本、依赖和安全语义被绑在一起。

高风险混合领域：

- OS/服务端口、工作区文件系统/shell、终端执行。
- 权限、运行时事件、远端工作区/投影/能力。
- 智能体对话/会话/thread goal/动态工具/transcript。

建议方向：

- 先按模块分组和导出面分类，避免立即大规模 crate 拆分带来 churn。
- 对外稳定面按服务端口、智能体生命周期、工具 ABI、远端/会话工作区、
  权限/副作用、事件清单分域。
- 合同对象避免泄漏当前实现字段。新字段必须有默认值、版本策略和兼容测试。

### 4.4 `RuntimeServices`

结论：`RuntimeServices` 适合作为产品组装构建出的内部类型化能力包，但不适合
直接作为插件或外部 SDK 的公共 API。

风险：

- 公开字段让调用方天然知道全部底层能力，容易退化为服务定位器。
- 插件如果拿到完整 bundle，会绕过能力限定句柄和权限/副作用声明。

建议方向：

- 产品组装可以持有完整 bundle。
- 内核、执行层、扩展层、插件只拿到能力子集视图，例如工作区、工具、
  权限、事件、产物、远端事实。
- SDK API 暴露 builder 注入点和窄运行时上下文，不暴露完整 bundle 字段。

### 4.5 Tool contract

结论：当前 `tool-contracts` 已经承接了很多提供方无关工具语义，但通用 ABI 和
BitFun 产品策略仍有混合。

应保留在 Tool ABI 的内容：

- 工具名称、schema、描述、执行上下文、结果、附件、元数据。
- 权限/副作用声明、只读/并发事实、产物引用、取消。
- 物化快照、陈旧调用保护、提供方身份。

应迁出或作为 decorator 的内容：

- 折叠工具的产品提示策略。
- MiniApp headless restriction。
- delegation policy 对具体 tool 名称的默认拦截。
- 特定产品功能的工具排序和清单文案。

### 4.6 Product capability / Harness

结论：`legacy_facade` 是合理过渡标记，不应被写成“已迁移完成”。它只说明路由计划已归档，
不说明具体工作流执行已归属。

建议方向：

- 能力包描述服务、工具组、工作流提供方、界面贡献、扩展能力的组合关系。
- 当具体执行迁移后，能力包只引用归属提供方 id。
- 对 DeepReview、MiniApp、DeepResearch 等复杂功能，迁移完成必须包含执行主体迁移、
  界面/命令描述符、事件/权限等价和旧路径收敛。

## 5. OpenCode 适配的未计划高风险项

以下事项必须进入后续计划，否则 OpenCode 分级适配会把当前边界债务固化为外部合同。

| 风险项 | 风险 | 解决方法 |
|---|---|---|
| 插件生命周期 | 插件 install / activate / deactivate / reload / dispose 不清晰，容易泄漏状态或无法回滚 | 建立 BitFun 插件运行时主机生命周期，所有提供方/贡献注册必须可撤销 |
| Rust / UI API 混用 | OpenCode server plugin 和 TUI plugin 能力不同，混用会让 UI 依赖进入内核 | 通过 Rust 内核 API / 插件运行时主机契约与界面扩展契约分别承接，再由产品组装统一注册 |
| 工具 ABI 不稳定 | 插件工具、MCP 工具、内置工具走不同路径，权限与陈旧调用行为不一致 | 建立统一物化工具快照、提供方身份、权限/副作用过滤和陈旧调用保护 |
| 事件无版本契约 | 插件消费内部事件字段后，后续重构会破坏生态 | 定义公开事件清单、版本、聚合身份、持久/回放口径和界面投影 |
| 权限钩子越权 | 插件钩子可能绕过最终授权或写审计状态 | 钩子只能产出候选决策；最终决策、审计、策略写入由安全控制面完成 |
| 界面贡献缺口 | 没有槽位/路由/键位映射/对话框/提示/状态视图，OpenCode TUI 插件无法等价映射 | 定义描述符专用界面宿主契约，再按 Desktop/Web/CLI 能力逐步实现 |
| 工作区/远端不一致 | 插件假设本地路径会破坏 remote、relay、web、SDK 形态 | 暴露工作区身份、逻辑路径、产物 URI、远端能力事实，不暴露本地绝对路径 |
| 外部包安全 | JS/TS 插件运行时涉及包来源、权限、secret、网络和崩溃隔离 | 第一阶段范围限定为原生扩展契约和受限适配器；JS 运行时需独立安全评审 |
| 插件来源与配置导入混用 | 把 OpenCode 配置当成 BitFun 插件主配置会导致来源、信任、回滚和诊断归属不清 | BitFun 插件安装/打包来源是主入口；OpenCode 配置只作为可选导入源，导入后转换为 BitFun manifest、hash、信任和诊断事实 |
| 外部 OpenCode 安装依赖 | 要求用户本机安装 OpenCode CLI 会把 P0 插件体验绑定到外部工具可用性 | 用户安装的 OpenCode CLI 只属于 ACP/外部客户端互操作或迁移辅助；OpenCode-compatible 插件加载不得依赖该二进制存在 |
| 配置/提供方转换 | 直接开放提供方/模型/配置转换会影响全局行为 | 采用支持矩阵，先开放只读或限定范围转换；配置导入不得绕过 BitFun 插件来源、信任、审计与回滚 |
| 产品能力漂移 | Desktop、CLI、Web、ACP、SDK 对插件能力支持不同 | 在产品组装维护能力矩阵和 `unsupported` / `temporarily-unavailable` 契约 |

## 6. 与实施计划的关系

后续执行节奏由 [`core-decomposition-plan.md`](../plans/core-decomposition-plan.md) 维护。本文件提供风险排序和计划映射，避免审计文档成为第二份路线图。

| 风险主题 | 计划映射 | 必须保留的验收要求 |
|---|---|---|
| 旧公开暴露面过宽 | 阶段 A：公共 API 收口 | 稳定外部、工作区内部、兼容 API 明确分层，并阻断旧 core 路径回流 |
| 工具 ABI / 运行时上下文混合 | 阶段 B：工具 ABI、事件清单与安全控制面 | 物化快照、提供方身份、权限/副作用过滤、陈旧调用保护、公开事件清单、版本、聚合身份、回放/保留具备测试 |
| 插件运行时主机生命周期和安全桥接 | 阶段 C：插件运行时主机基础 | 贡献以描述符暴露，产品组装内部物化提供方；注册可撤销，候选效果不能写权威状态 |
| UI 扩展契约缺口 | 阶段 D：UI 扩展契约与产品形态矩阵 | 描述符专用、只读状态视图、入口回退和 `unsupported` / `temporarily-unavailable` 行为具备往返测试 |
| OpenCode 分级适配 | 阶段 E：OpenCode 兼容适配器 | 插件来源模型、可选配置导入、支持矩阵、类型化 `unsupported`、权限/副作用、事件清单、界面贡献和远程/工作区映射全部可验证 |
| 剩余具体归属模块 | 阶段 F：具体归属模块与 SDK 可用性 | 产品组装选择具体提供方；普通层级只依赖端口、描述符或稳定契约 |

## 7. 执行准则

- 不将 OpenCode API 直接稳定成 BitFun 内部 API。
- 不将 OpenCode 配置文件或用户本机 OpenCode CLI 作为 BitFun 插件生态的主入口。
- 不将完整 `RuntimeServices` bundle 或 `bitfun-core/product-full` 暴露给插件或 SDK。
- 不将界面实现、Tauri 状态、React 组件或具体提供方句柄下沉到内核。
- 不接受只新增抽象、不删除或收敛旧路径的迁移 PR。
- 不在安全控制面未完成前开放可写插件钩子。
- 任何会改变工具曝光、权限语义、事件字段、远端行为或产品能力矩阵的变更必须单独评审。
