# 智能体内核、运行时服务与扩展接口设计

本文件是 [`product-architecture.md`](product-architecture.md) 的开发设计，定义目标模块、接口、
crate 内部结构和行为保护要求。本文件记录设计约束，不记录实现过程或验证记录。插件运行时主机、
生态兼容适配层、进程间通信和候选效果接口见
[`plugin-runtime-host-design.md`](plugin-runtime-host-design.md)；CLI 入口、配置兼容、产品定制和 CLI Agent
体验边界见 [`cli-product-line-design.md`](cli-product-line-design.md)。

阅读路径：第 1 节确认 SDK、内核、产品特性、扩展接口和 crate 边界；第 2-3 节说明稳定接口、
运行时服务、内核、工具和工作流；第 4 节说明产品组装与扩展注册；第 5 节作为质量保护和
目标态判定标准。

## 1. 设计目标与边界

- 智能体内核可被 Desktop、CLI、Server、Remote、ACP、Web 和独立 SDK 形态嵌入。
- 智能体内核对外提供稳定、窄口径的 Rust 运行时接口，而不是暴露 `bitfun-core`、产品命令路径或具体管理器。
- 产品特性把内核能力组装为用户侧能力，可能同时触达 Rust 和 UI，但不拥有内核状态机或平台实现。
- 运行时内部接口、能力服务接口、扩展接口和主机内部 ABI 分层表达；OpenCode / ACP / 插件适配器仅承担映射和注册。
- 智能体内核不感知平台差异、工具实现差异、界面宿主差异和构建形态差异。
- 工具、Skill、MCP、工作流和扩展使用通用接口和提供方 / 贡献注册，不绑定底层实现。
- 具体服务、界面宿主、插件运行时主机绑定和适配器清单集合由上层产品组装注入。
- 每个 crate 只依赖最小稳定集合，依赖方向可检查。

### 1.1 SDK 发布边界

Agent Runtime SDK 的发布边界以调用方能力为准，而不是以物理 crate 命名为准。达到目标状态时，外部调用方
应能在不依赖 `bitfun-core`、app crate、Tauri 或产品内部 manager 的情况下完成以下动作：

- 构建运行时：注入模型提供方、`RuntimeServices`、工具提供方、工作流提供方、智能体定义、
  钩子和运行时配置。
- 发起执行：创建或恢复会话，提交轮次，取消轮次，消费提供方无关事件流。
- 执行工具：通过稳定工具清单、权限请求、工具结果、产物引用和取消语义
  管理工具调用。
- 扩展能力：通过注册表注册子智能体、提示模块、skill、MCP 工具、接口工具、工作流和轮次后处理器。
- 处理运维语义：接收类型化错误、用量/成本/缓存事实、遥测事件、检查点/恢复事实和不支持能力。

因此，SDK 可用性准备的最低标准是：

- 公共门面只暴露 builder、runner、请求/响应 DTO、事件流、类型化错误和注册表接口。
- 所有 DTO 可序列化，所有运行时句柄通过类型化端口注入，不进入线缆 schema。
- `bitfun-agent-runtime`、工具原语、运行时服务和工作流能通过测试替身提供方独立测试。
- 内部 SDK 最小特性不牵引 Desktop、Tauri、Git 提供方、MCP 客户端、AI HTTP 客户端、remote SSH 或产品 UI。
- 完整产品能力只能通过产品组装或兼容 `bitfun-core/product-full` 组装，不反向污染 SDK 接口。

SDK 公共接口以 `AGENT_RUNTIME_SDK_API_VERSION` 标记兼容边界。当前接口版本为 v1 preview：
小版本更新允许增加可选 builder hook、DTO 字段或注册表查询能力，但不得改变既有端口语义、
错误分类、session / turn 标识含义或默认 feature 依赖。任何需要调用方改写现有嵌入代码的变更，
必须提升接口版本并提供兼容迁移路径。

只要外部调用方仍必须导入 `bitfun-core`、启用 `product-full`、持有具体服务管理器、读取产品命令
注册表或依赖全局可变状态，SDK 发布边界就不成立。

### 1.2 内核与特性的分界

内核能力和产品特性必须分开判断：

| 领域 | 属于内核 | 属于产品特性 |
|---|---|---|
| 长程任务 | 任务身份、队列、恢复、取消、事件、持久化事实 | `/goal` 命令、默认目标模板、UI 展示、设置项和产品文案 |
| 权限 | 权限事实、来源身份、决策请求、审计事件 | 桌面弹窗、CLI 提示、Web 状态视图和产品默认选项 |
| 上下文 | 会话/工作区事实、上下文组装接口、记忆端口 | 具体入口的上下文展示、快捷命令和特性默认配置 |
| 模型调度 | 提供方无关模型路由请求、用量/成本/缓存事实 | 产品形态默认模型、设置入口和降级文案 |
| 钩子 / 事件 | 事件 schema、钩子顺序、超时、错误策略 | 哪些特性注册钩子、UI 如何展示钩子结果 |

判断标准：

- 在 Desktop、CLI、Web、ACP 和 SDK 中都可复用，且不依赖 UI 或平台具体实现的能力，优先归智能体内核。
- 会改变用户入口、命令、设置、入口视图、默认策略或产品文案的能力，归产品特性。
- 会接触 OS、网络、终端、文件系统、远端主机、MCP server 或 AI 提供方具体实现的能力，归跨平台适配器或协议适配器。
- 来自外部插件、OpenCode、ACP 外部智能体/工具桥接、外部 skill 或第三方包的能力，先进入
  扩展层，再由产品组装注册到特性 / 内核 / 执行层的稳定接口；ACP 协议生命周期
  仍由 interfaces/acp 和对应入口适配器拥有。

### 1.3 运行时、能力服务、扩展与主机接口面

接口切面以 [`product-architecture.md`](product-architecture.md#2-接口切面) 为准。本文件不维护第二套能力服务状态词、插件接口字段或生态兼容矩阵，只补充运行时和 crate 归属：

| 切面 | 本文件补充的内容 | 不在本文件重复定义 |
|---|---|---|
| 前后端能力服务切面 | 智能体内核如何产出会话、事件、权限和诊断事实 | 入口线缆 DTO、插件状态视图字段、产品形态状态词 |
| BitFun 与插件切面 | 插件贡献如何进入内核、执行层和安全控制面 | 具体生态接口、未预算界面贡献字段、OpenCode 原始 payload |
| 插件通用运行时切面 | `PluginRuntimeBinding` 如何注入 Agent Runtime 内部 builder | `PluginRuntimeClient`、dispatch/read schema、隔离字段；这些由插件主机文档和 `runtime-ports` 代码定义 |
| OpenCode 适配切面 | 不进入 Agent Runtime SDK；只作为插件主机内部反腐层 | OpenCode client/server facade、配置导入细节、跨生态稳定接口 |

OpenCode 适配器、ACP 桥接和未来插件运行时必须先映射到主架构定义的切面，再由产品组装注册。它们不能直接写智能体内核权威状态，也不能绕过权限、沙箱、审计或界面宿主边界。

Agentic 前端事件视图属于事件归属子接口：智能体内核产生提供方无关 `AgenticEvent`，事件 schema 层给出事件名、事件类型、版本和载荷。Tauri、WebSocket、OpenCode 适配器或界面宿主只选择交付形态，不重新定义字段映射。

扩展注册接口不是产品组装的具体实现。插件运行时主机和兼容适配器只产出候选贡献；产品组装消费这些贡献并在具体产品形态内注册，避免扩展层反向依赖 assembly crate。

### 1.4 接口与 crate 边界

本设计按接口归属划分 crate，而不是按调用方或产品形态划分。一个 crate 只能拥有一类稳定边界；如果同一文件同时
处理 UI 入口、产品策略、内核状态和 OS I/O，应拆到对应归属模块。

| 接口 / 归属 | 主要 crate | 允许依赖 | 不允许依赖 | 对外承诺 |
|---|---|---|---|---|
| 产品组装接口 | `src/crates/assembly/*` | 特性包、内核接口、执行层接口、运行时服务、平台提供方 | 智能体内部状态机、具体 UI 组件实现作为下层依赖 | 按产品形态组装能力，输出类型化运行时部件 |
| 产品特性接口 | `product-capabilities`、`product-domains`、对应入口归属模块 | 内核接口、能力服务读模型、能力/副作用接口、领域接口 | OS 具体实现、Tauri 句柄、执行层具体实现、最终权限策略 | 把内核能力映射为用户功能、入口视图和默认策略 |
| Rust 内核接口 | `agent-runtime`、`agent-stream`、`runtime-services`、`runtime-ports`、`events`、`core-types` | 稳定接口、工具/工作流注册表、类型化服务 | `bitfun-core`、Tauri、Web UI、ACP 协议、提供方具体实现 | 会话 / 轮次 / 事件 / 权限 / 调度 / 上下文等 SDK 候选接口 |
| 执行层接口 | `tool-contracts`、`tool-provider-groups`、`tool-execution`、`harness` | 稳定接口、运行时端口、注入的服务端口 | 产品注册表、UI、具体文件系统/Git/终端/MCP 客户端 | 工具、skills、MCP 工具桥接、沙箱、工作流执行语义 |
| 扩展接口 | 插件运行时主机 / OpenCode 兼容 / ACP 适配器归属模块 | Rust 内核接口、工具/事件/权限子接口、能力/副作用接口 | Web UI React 实现、Tauri 状态、内核权威状态写入 | 把外部生态能力转换为声明、诊断、提供方和候选效果 |
| 平台/提供方适配器接口 | `services/*`、`adapters/*`、app-local provider | 运行时端口、稳定 DTO、允许的第三方库 | 产品特性、智能体内核状态机、UI 命令 | 实现文件系统、终端、网络、远端、Git、MCP 传输、AI 提供方等边界外 I/O |
| 稳定数据接口 | `contracts/*` | 低层无行为依赖或标准序列化依赖 | 上层 crate、具体管理器、UI 渲染 | DTO、事件、端口、能力/副作用、权限、沙箱、审计、类型化错误 |

禁止依赖：

- `contracts/*` 或 `runtime-ports` 依赖 `bitfun-core`、assembly、apps、UI 或具体服务。
- `agent-runtime` 依赖 `bitfun-core`、Tauri、Web UI、ACP 协议、AI 提供方具体实现、MCP 客户端具体实现或 OS 服务管理器。
- `tool-contracts` 依赖具体 service crate；`tool-execution` 依赖产品注册表、产品权限策略或具体 UI。
- `harness` 依赖具体文件系统/Git/终端管理器；它只通过端口和提供方接口获取能力。
- 插件运行时主机不能依赖 Web UI React 组件实现、Tauri app 状态或具体 core 管理器。
- 产品特性直接依赖平台适配器具体实现、执行层具体实现、全局可变运行时状态或边界外资源客户端。

接口暴露原则：

- 对外接口按层拆分：运行时内部接口、能力服务接口、扩展接口、主机内部 ABI、产品组装接口分别定义。
- 下层不暴露上层对象。内核不返回 UI 命令；执行层不返回 UI 实现或未预算的界面视图；平台适配器不返回产品命令。
- 注册接口接收类型化提供方、候选效果和策略，不接收 `Any`、无类型服务名或全局可变注册表。
- 兼容门面可以保留旧路径导出，但旧路径不得成为新接口的真实归属模块。

### 1.5 平台适配与边界外资源

平台 / 提供方适配器是仓库内实现层，负责把稳定端口转换为 OS、网络、终端、远端、MCP
传输、AI 提供方、浏览器运行时或第三方库调用。边界外资源不是 crate、不是逻辑层，也不是所有模块可依赖的
基础设施。

实现规则：

- 产品组装是唯一可以选择具体平台提供方的位置；选择结果以类型化运行时部件注入。
- 内核、执行层、扩展层和产品特性只消费稳定接口、端口句柄或已预算的类型化声明，不导入具体
  提供方 crate。
- 平台适配器不读取交付形态、特性包或 UI 命令；形态差异由产品组装注入。
- 外部资源错误必须在适配器边界转换为类型化错误、unsupported / temporarily-unavailable 或能力/副作用事实，不能泄漏为
  产品层专用分支。

## 2. 稳定接口与运行时服务

### 2.1 稳定数据与接口（Stable Contracts）

所属 crate：

- `bitfun-core-types`
- `bitfun-events`
- `bitfun-runtime-ports`

建议模块：

```text
bitfun-core-types
  error/
  identity/
  artifact/
  usage/
  surface/

bitfun-events
  runtime/
  tool/
  permission/
  product/

bitfun-runtime-ports
  agent/
  service/
  permission/
  subagent/
  tool/
  workspace/
```

接口原则：

- DTO 必须可序列化，避免携带 runtime handle。
- port trait 只描述能力，不描述产品 UI。
- permission / approval 必须包含 surface、thread、turn、agent、subagent identity。
- artifact ref 使用稳定 URI / logical path，不暴露本地绝对路径。

示例接口：

```rust
pub trait RuntimeEventSink: Send + Sync {
    fn emit(&self, event: RuntimeEvent);
}

#[async_trait::async_trait]
pub trait PermissionPort: Send + Sync {
    async fn request(&self, request: PermissionRequest) -> PermissionDecision;
}

#[async_trait::async_trait]
pub trait WorkspacePort: Send + Sync {
    async fn resolve(&self, identity: WorkspaceIdentity) -> Result<WorkspaceFacts, PortError>;
}
```

### 2.2 运行时服务

目标归属 crate：`bitfun-runtime-services`。

职责：

- 承载运行时可消费的类型化服务集合。
- 提供提供方注册和能力解析。
- 把具体实现与运行时端口隔离。
- 提供统一的 temporarily-unavailable / unsupported 错误。
- 为测试提供测试替身提供方 builder。

建议内部模块：

```text
bitfun-runtime-services
  bundle.rs             # RuntimeServices / ToolServices / HarnessServices
  builder.rs            # 类型化 builder
  capability.rs         # capability ids 与 availability
  registry.rs           # provider 注册
  errors.rs             # unsupported / temporarily-unavailable 映射
  test_support.rs       # 测试替身提供方
```

核心结构：

```rust
pub struct RuntimeServices {
    pub filesystem: Arc<dyn FileSystemPort>,
    pub workspace: Arc<dyn WorkspacePort>,
    pub session_store: Arc<dyn SessionStorePort>,
    pub permission: Arc<dyn PermissionPort>,
    pub events: Arc<dyn RuntimeEventSink>,
    pub clock: Arc<dyn ClockPort>,
    pub terminal: Option<Arc<dyn TerminalPort>>,
    pub network: Option<Arc<dyn NetworkPort>>,
    pub git: Option<Arc<dyn GitPort>>,
    pub mcp_catalog: Option<Arc<dyn McpCatalogPort>>,
    pub remote_connection: Option<Arc<dyn RemoteConnectionPort>>,
    pub remote_workspace: Option<Arc<dyn RemoteWorkspacePort>>,
    pub remote_projection: Option<Arc<dyn RemoteProjectionPort>>,
    pub remote_capabilities: Option<Arc<dyn RemoteCapabilityPort>>,
}

pub struct RuntimeServicesBuilder {
    // 仅 typed 字段
}

impl RuntimeServicesBuilder {
    pub fn with_filesystem(self, port: Arc<dyn FileSystemPort>) -> Self;
    pub fn with_optional_network(self, port: Option<Arc<dyn NetworkPort>>) -> Self;
    pub fn with_optional_git(self, port: Option<Arc<dyn GitPort>>) -> Self;
    pub fn with_optional_remote_connection(self, port: Option<Arc<dyn RemoteConnectionPort>>) -> Self;
    pub fn with_optional_remote_workspace(self, port: Option<Arc<dyn RemoteWorkspacePort>>) -> Self;
    pub fn with_optional_remote_projection(self, port: Option<Arc<dyn RemoteProjectionPort>>) -> Self;
    pub fn with_optional_remote_capabilities(self, port: Option<Arc<dyn RemoteCapabilityPort>>) -> Self;
    pub fn build(self) -> Result<RuntimeServices, RuntimeServicesError>;
}
```

Remote ports 的边界：

- `RemoteConnectionPort` 只描述连接身份、状态、认证上下文和连接生命周期请求，不暴露 SSH / relay / tunnel 具体句柄。
- `RemoteWorkspacePort` 只描述远端工作区身份、根目录解析、启动保护和持久化/会话事实。
- `RemoteProjectionPort` 只描述文件、终端、image/context 只读视图的请求 / 响应形态，不直接执行具体 OS 命令。
- `RemoteCapabilityPort` 只描述远端主机能力事实，例如文件系统、终端、review platform、model catalog 支持状态。
- SSH、relay、本地隧道、远端 OS、认证和传输实现必须留在具体 Remote 提供方，由产品组装注册。

设计约束：

- 不提供 `get<T>() -> Any` 作为主路径。
- 能力缺失必须返回类型化 unsupported 错误。
- 不在运行时服务中执行产品命令。
- 不在运行时服务中创建具体管理器；创建发生在产品组装。
- `RuntimeServices` 是运行时依赖集合，不是全局可变 app 状态。

### 2.3 安全控制面接口

安全控制面把 tool、MCP、skills、plugin、hook、shell、network、file、browser/desktop 和 remote 动作归一为
能力/副作用/安全决策。它跨越内核、执行层、扩展层、跨平台适配器和界面视图，
但最终决策必须由产品组装注入的确定性策略实现和内核事实共同约束。该接口定义的是跨层接口约束，
不是 contracts crate 内部的具体策略实现。

建议接口：

```rust
pub struct CapabilityEffectDeclaration {
    pub capability: CapabilityId,
    pub source: CapabilitySource,
    pub targets: Vec<EffectTarget>,
    pub data_classes: Vec<DataClass>,
    pub side_effects: Vec<SideEffectKind>,
    pub execution_domain: ExecutionDomain,
}

pub struct SecurityDecisionRequest {
    pub session: SessionIdentity,
    pub turn: Option<TurnIdentity>,
    pub agent: AgentIdentity,
    pub source: CapabilitySource,
    pub effect: CapabilityEffectDeclaration,
    pub proposed_action: ProposedAction,
}

pub trait SecurityDecisionPort: Send + Sync {
    fn decide(&self, request: SecurityDecisionRequest) -> SecurityDecisionFuture;
}
```

约束：

- UI 只展示 decision 和 user options，不成为最终授权来源。
- 插件运行时主机必须声明能力/副作用，未知或超声明能力默认受限。
- `allow_in_sandbox` 只能在实际 sandbox 或隔离路径存在时返回。
- 远程、ACP、MCP、插件、browser/desktop 和 cloud task 必须携带执行域。
- 模型输出只能辅助解释和候选判断，不能直接写权限、审计或策略状态。

## 3. 内核、工具与工作流

### 3.1 Agent Kernel / Runtime SDK

目标归属 crate：`bitfun-agent-runtime`。

目标职责：

- 会话生命周期。
- 对话轮次 / 模型轮生命周期。
- long-running task 生命周期、resume/checkpoint fact 和 result delivery。
- 调度器 / 队列 / 取消。
- 权限协调和安全事实投递。
- 模型路由 / 用量 / 成本 / 缓存事实。
- 提示循环和上下文组装。
- prompt cache 协调。
- memory / workspace facts。
- DFX / 遥测 / 审计事实。
- 智能体定义注册表、子智能体注册表查询和委派策略。
- fork context seeding。
- 工具调用调度。
- 运行时事件。
- 轮次后处理器。

公共门面：

```rust
pub struct AgentRuntimeBuilder {
    // typed runtime parts only
}

pub struct AgentRunRequest {
    pub session: SessionSelector,
    pub message: String,
    pub turn_id: Option<String>,
    pub source: Option<AgentSubmissionSource>,
    pub attachments: Vec<AgentInputAttachment>,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

pub struct AgentRunHandle {
    pub session_id: String,
    pub turn_id: String,
    pub agent_type: Option<String>,
    pub accepted: bool,
    pub events: Option<AgentEventStream>,
}

impl AgentRuntimeBuilder {
    pub fn with_submission_port(self, port: Arc<dyn AgentSubmissionPort>) -> Self;
    pub fn with_session_management_port(self, port: Arc<dyn AgentSessionManagementPort>) -> Self;
    pub fn with_dialog_turn_port(self, port: Arc<dyn AgentDialogTurnPort>) -> Self;
    pub fn with_lifecycle_delivery_port(self, port: Arc<dyn AgentLifecycleDeliveryPort>) -> Self;
    pub fn with_cancellation_port(self, port: Arc<dyn AgentTurnCancellationPort>) -> Self;
    pub fn with_services(self, services: RuntimeServices) -> Self;
    pub fn with_event_stream(self, events: AgentEventStream) -> Self;
    pub fn with_tool_registry(self, registry: Arc<dyn RuntimeToolRegistry>) -> Self;
    pub fn with_harness_registry(self, registry: Arc<HarnessRegistry>) -> Self;
    pub fn with_hook_registry(self, hooks: RuntimeHookRegistry) -> Self;
    pub fn with_agent_registry(self, agents: Arc<dyn RuntimeAgentRegistry>) -> Self;
    pub fn build(self) -> Result<AgentRuntime, RuntimeBuildError>;
}

impl AgentRuntime {
    pub async fn run(&self, request: AgentRunRequest) -> Result<AgentRunHandle, RuntimeError>;
}
```

该门面是目标接口形态。它必须只接收已组装的类型化部件，不负责创建
文件系统、终端、MCP、AI 客户端、Remote 提供方或产品命令。
当前 v1 preview 接口以 message / attachment / metadata 作为最小输入形态；若把
model-round cancellation token、结构化 AgentInput 或更复杂的事件游标纳入公开 SDK，
必须提升 SDK 接口版本并保留旧路径兼容。

产品特性边界：

- `/goal`、slash command、输入框按钮、设置项、UI panel 和默认文案不进入智能体内核。
- 内核只提供 goal / long-running task 所需的任务身份、生命周期、队列、resume/cancel、事件和持久化事实。
- 产品特性负责把这些内核事实映射为 `/goal` 命令、可见状态、快捷操作和默认策略。
- 若某个特性需要修改 Rust 和 UI，必须以特性包同时声明 Rust 运行时请求、入口视图和
  能力/副作用，不得仅在单侧隐式扩展。

兼容边界：

- `bitfun-agent-runtime` 只能依赖稳定接口、工具运行时、运行时服务接口和注入的提供方。
- 具体调度器生命周期、会话元数据存储、token 订阅器、事件投递、产品 `Tool`
  handler、具体提示组装、workspace / remote / config IO、自定义子智能体文件 IO 和平台适配器
  在行为等价未证明前不得下沉到运行时内核。
- 产品特性命令、UI 状态、settings 持久化、插件 UI 渲染和交付形态默认策略不得下沉到运行时内核。
- prompt、event、thread goal、scheduler 或 subagent 的纯事实如果进入 Agent Runtime SDK，旧归属模块只能保留兼容入口；
  行为等价需要有接口等价测试和边界保护证明。

建议内部模块：

```text
bitfun-agent-runtime
  lib.rs
  runtime.rs            # AgentRuntime 公共接口
  config.rs             # RuntimeConfig
  session/
    manager.rs
    state.rs
    persistence.rs
  turn/
    dialog_turn.rs
    model_round.rs
    continuation.rs
  scheduler/
    queue.rs
    cancellation.rs
    priority.rs
  prompt/
    assembly.rs
    cache.rs
    compression.rs
  agents/
    definitions.rs
    registry.rs
    prompts.rs
  subagent/
    delegation.rs
    fork_context.rs
    background.rs
  tools/
    dispatcher.rs
    permission.rs
    result_bridge.rs
  hooks/
    registry.rs
    prompt.rs
    post_turn.rs
  events/
    mapper.rs
```

公共接口：

```rust
pub struct AgentRuntime {
    services: RuntimeServices,
    tools: Arc<ToolRuntime>,
    agents: Arc<dyn AgentDefinitionRegistry>,
    hooks: Arc<RuntimeHookRegistry>,
    config: RuntimeConfig,
}

impl AgentRuntime {
    pub fn new(parts: AgentRuntimeParts) -> Result<Self, RuntimeBuildError>;

    pub async fn start_session(
        &self,
        request: StartSessionRequest,
    ) -> Result<SessionHandle, RuntimeError>;

    pub async fn submit_turn(
        &self,
        request: SubmitTurnRequest,
    ) -> Result<TurnHandle, RuntimeError>;

    pub async fn cancel_turn(
        &self,
        request: CancelTurnRequest,
    ) -> Result<CancelOutcome, RuntimeError>;
}
```

输入：

- `RuntimeServices`
- `ToolRuntime`
- `AgentDefinitionRegistry`
- `RuntimeHookRegistry`
- model / stream adapter
- 产品注入的 `RuntimeConfig`

输出：

- `RuntimeEvent`
- transcript delta
- artifact refs
- permission requests
- session state
- turn outcome

不得拥有：

- 具体 filesystem / Git / terminal / MCP client。
- Tauri、CLI TUI、Web rendering。
- ACP protocol。
- 产品 feature matrix。
- 具体 tool 实现。

关键保护：

- `SessionManager -> Session -> DialogTurn -> ModelRound` 语义不变。
- `/goal` custom metadata、post-turn verification、continuation event 不漂移。
- `get_goal` / `create_goal` / `update_goal` 的 tool response wire shape、blocked/complete 语义和 token budget report 不漂移。
- `Task.run_in_background` delivery 不漂移。
- `Task.fork_context` 禁止字段、prompt cache clone、context seeding 不漂移。
- DeepResearch citation renumber post-turn hook 保持 deterministic。

### 3.2 Tool Primitives

所属 crate：

- `tool-contracts`（Cargo package: `bitfun-agent-tools`）
- `tool-provider-groups`（Cargo package: `bitfun-tool-packs`）
- `tool-execution`（Cargo package: `tool-runtime`）

目标职责：

- `tool-contracts`：tool DTO、manifest、exposure、schema、path policy、result policy、admission gate 和 provider-neutral registry assembly。
- `tool-provider-groups`：tool provider group feature metadata 和 provider plan。
- `tool-execution`：低层 file/search/tool IO helper，不拥有产品 registry、permission policy 或 agent-facing tool surface。

建议模块：

```text
tool-contracts
  framework.rs
  restrictions.rs
  file_guidance.rs
  tool_result_storage.rs
  tool_execution_presentation.rs

tool-provider-groups
  provider_groups.rs

tool-execution
  filesystem.rs
  search.rs
  remote.rs
  result_window.rs
```

核心接口：

```rust
#[async_trait::async_trait]
pub trait ToolProvider: Send + Sync {
    fn id(&self) -> ToolProviderId;
    fn manifest(&self, ctx: ToolManifestContext) -> ToolManifest;
    async fn get(&self, name: &str) -> Option<Arc<dyn RuntimeTool>>;
}

#[async_trait::async_trait]
pub trait RuntimeTool: Send + Sync {
    fn spec(&self, ctx: ToolSpecContext) -> ToolSpec;

    async fn execute(
        &self,
        ctx: ToolExecutionContext,
        input: ToolInput,
    ) -> Result<ToolExecutionOutput, ToolExecutionError>;
}

pub struct ToolExecutionContext {
    pub facts: ToolContextFacts,
    pub services: ToolExecutionServices,
    pub cancellation: CancellationToken,
}
```

目标职责：

- 提供方无关清单、目录、权限门禁、执行准入、工具钩子、执行结果呈现和结果产物策略。
- `GetToolSpec` catalog、detail、assistant result 和 collapsed-tool unlock observation。
- 工作区服务、路径策略、运行时产物引用、远端路径限制和工具上下文事实的稳定接口。

兼容边界：

- core 允许保留旧路径门面、具体工具适配器、状态更新、注册表查询、确认、实际执行和文件系统持久化；目标状态要求只有在等价测试保护下才能移动这些行为。
- 工作区文件/shell 接口保留既有错误与取消语义；不得把错误分类、取消语义或产品工具暴露
  变更混入归属边界移动。

设计约束：

- `ToolExecutionContext` 不暴露具体 manager。
- `ToolContextFacts` 只包含 portable facts。
- 工具原语只消费 `ToolExecutionServices` 这样的窄服务视图，不依赖完整
  `RuntimeServices` bundle。
- path policy、runtime artifact ref、remote POSIX containment 由 `tool-contracts` 承载。
- MCP 工具作为外部工具提供方注入，不内置在 Agent Runtime SDK。
- `GetToolSpec` 是工具目录能力，不是产品 UI。

必须保护：

- prompt-visible manifest。
- expanded / collapsed exposure。
- `GetToolSpec` schema / assistant detail / detail JSON。
- collapsed unlock state 与 persistence 生命周期。
- readonly / enabled snapshot filter。
- MCP / ACP / desktop tool catalog 等价。
- oversized tool result persistence、flush、preview、artifact ref。
- Write/Edit/Read file-read-state guardrail。

### 3.3 工作流层

目标归属 crate：`bitfun-harness`。

职责：

- 把 SDD、DeepReview、DeepResearch、MiniApp、function-agent 等工作流从运行时内核中分离。
- 定义工作流描述符、路由计划、提供方注册表、工作流计划、步骤、策略、产物、
  review gate 和 post-processor。
- 通过 Agent Runtime SDK、工具运行时和服务端口编排。

建议内部模块：

```text
bitfun-harness
  provider.rs
  registry.rs
  plan.rs
  context.rs
  artifact.rs
  hooks.rs
  review_gate.rs
  sdd/
  deep_review/
  deep_research/
  miniapp/
```

核心接口：

```rust
#[async_trait::async_trait]
pub trait HarnessProvider: Send + Sync {
    fn id(&self) -> HarnessId;
    fn capabilities(&self) -> HarnessCapabilities;

    async fn plan(
        &self,
        ctx: HarnessPlanningContext,
        input: HarnessInput,
    ) -> Result<HarnessPlan, HarnessError>;

    async fn execute(
        &self,
        ctx: HarnessExecutionContext,
        plan: HarnessPlan,
    ) -> Result<HarnessOutcome, HarnessError>;
}

pub struct HarnessExecutionContext {
    pub runtime: Arc<AgentRuntime>,
    pub tools: Arc<ToolRuntime>,
    pub services: HarnessServices,
    pub events: Arc<dyn RuntimeEventSink>,
}
```

设计约束：

- 工作流允许编排运行时/工具，但不拥有会话管理器内部结构。
- 工作流不直接访问具体文件系统 / Git / 终端。
- 产品命令只映射到工作流能力，不把命令展示逻辑下沉。
- 新工作流通过提供方注册，不改 Agent Runtime SDK 内核。
- 描述符专用 / 旧门面提供方只能表达路由计划；不得被描述为已经拥有具体工作流执行。
  执行语义移动必须单独证明行为等价。

## 4. 产品组装与扩展

### 4.1 产品组装

产品组装是组装根。初始状态可由 `bitfun-core` 兼容门面承载；目标状态可拆成独立
产品组装 crate。它按特性包触发组装，同时连接界面宿主、Rust 内核、执行层、插件运行时主机
和跨平台适配器。

职责：

- 创建或接收具体服务实现、插件运行时主机绑定和适配器清单集合。
- 构建 `RuntimeServices`。
- 注册工具提供方组。
- 注册工作流提供方。
- 注册智能体定义、子智能体、skills、提示模块。
- 注册插件运行时绑定、适配器清单集合、ACP 桥接和已预算的入口视图提供方。
- 建立产品特性矩阵。
- 把接口命令映射到能力 / 工作流 / 运行时请求。
- 把特性包映射为 Rust 运行时请求、入口命令、插件能力和安全策略。
- 接收产品入口选择的唯一 `DeliveryProfile`，结合 `ProductProfile` 和 `SurfaceContract` 校验兼容性并派生
  静态能力计划、适配器清单集合和服务提供方集合；不得在组装内部再次选择交付形态。
- 对不支持能力返回类型化 unsupported / temporarily-unavailable 错误，而不是让下层运行时判断产品形态。
- 通过类型化 `PluginRuntimeBinding` 向内核 / Agent Runtime 内部 builder 注入插件运行时客户端或 disabled stub，不使用全局注册表；该绑定不进入 Agent Runtime SDK 门面。

建议模块：

```text
product-assembly
  full.rs
  product_profile.rs
  delivery_profile.rs
  capability_plan.rs
  capability_availability.rs
  feature_bundle.rs
  desktop.rs
  cli.rs
  server.rs
  remote.rs
  acp.rs
  feature_matrix.rs
  entry_projection.rs
  extensions.rs
  commands.rs
```

以下结构是产品组装目标态草图，不是当前稳定接口。任何字段进入代码前必须先满足
[`product-architecture.md`](product-architecture.md) 的接口切面、当前消费方和公开接口预算规则。

```rust
pub struct ProductProfileRef {
    pub id: String,
    pub schema_version: u32,
    pub content_digest: String,
}

pub enum DeliveryProfile {
    ProductFull,
    Desktop,
    Cli,
    Server,
    Remote,
    Acp,
    Web,
    MobileWeb,
    Sdk,
}

pub struct ProductProfileProjection {
    pub profile_ref: ProductProfileRef,
    pub capability_ids: Vec<CapabilityId>,
    pub default_policy_refs: Vec<PolicyRef>,
    pub bundled_extension_ids: Vec<String>,
}

// 完整 Product Profile 表示产品身份、资源、能力包、默认策略引用和发行事实；
// 产品组装根校验 authoring Profile、生成带内容摘要的 Resolved Product Manifest，
// 并只把运行时需要的投影交给 Product Assembly。
// DeliveryProfile 作为独立输入表示交付形态，不进入 ProductProfileProjection。
pub struct CapabilityPlan {
    pub agent_modes: Vec<AgentModeId>,
    pub tool_packs: Vec<ToolPackId>,
    pub harness_packs: Vec<HarnessId>,
    pub service_capabilities: Vec<ServiceCapabilityId>,
    pub command_providers: Vec<CommandProviderId>,
    pub entry_projections: Vec<EntryProjectionId>,
    pub extensions: ExtensionCapabilitySet,
}

pub struct CapabilityAvailabilitySet {
    pub version: u64,
    pub entries: Vec<CapabilityAvailability>,
}

pub struct CapabilityAvailability {
    pub capability: CapabilityId,
    pub state: CapabilityAvailabilityState,
    pub reason: Option<CapabilityAvailabilityReason>,
    pub provider: Option<ServiceCapabilityId>,
}

pub enum CapabilityAvailabilityState {
    Full,
    StatusOnly,
    TemporarilyUnavailable,
    Unsupported,
    PolicyDenied,
}

pub struct CapabilityAvailabilityReason {
    pub code: CapabilityAvailabilityReasonCode,
    pub message_key: Option<String>,
    pub policy_owner: Option<PolicyOwnerRef>,
    pub security_decision: Option<SecurityDecisionId>,
    pub provider_health: Option<ProviderHealthRef>,
}

pub enum CapabilityAvailabilityReasonCode {
    UnsupportedSurface,
    MissingProvider,
    TemporarilyUnavailable,
    PolicyDenied,
    ProviderUnhealthy,
}

pub struct ProductAssemblyPlan {
    pub product_profile: ProductProfileRef,
    pub delivery_profile: DeliveryProfile,
    pub capability_plan: CapabilityPlan,
    pub feature_groups: Vec<FeatureGroupId>,
    pub feature_bundles: Vec<FeatureBundleId>,
}

pub struct ExtensionCapabilitySet {
    pub plugin_runtime: PluginRuntimeAvailability,
    pub adapters: Vec<PluginAdapterCapability>,
}

pub enum PluginRuntimeAvailability {
    Disabled { reason: UnsupportedReason },
    ProjectionOnly { reason: UnsupportedReason },
    Enabled {
        compat_level: PluginCompatibilityLevel,
        execution: PluginExecutionLocation,
        trust_policy: PluginTrustPolicyId,
        fallback: PluginFallbackMode,
    },
    Unavailable { reason: UnsupportedReason },
}

pub enum PluginRuntimeBinding {
    Disabled(DisabledPluginRuntimeClient),
    ProjectionOnly(ProjectionOnlyPluginRuntimeClient),
    Client(Arc<dyn PluginRuntimeClient>),
}

pub struct ProductAssemblyPlanInput {
    pub product_profile: ProductProfileProjection,
    pub delivery_profile: DeliveryProfile,
    pub surface: SurfaceContractRef,
}

pub trait ProductAssembler {
    fn plan(&self, input: ProductAssemblyPlanInput) -> Result<ProductAssemblyPlan, AssemblyError>;
    fn build(&self, plan: ProductAssemblyPlan) -> Result<ProductRuntime, AssemblyError>;
}
```

`ProductAssemblyPlan` 只保存静态 eligibility、依赖和服务要求，不保存动态 availability。产品组装创建并注入
唯一的版本化 Capability Availability 读模型，由它把静态计划、provider health、运行时策略和已归一化的
quarantine 事实合并为 `CapabilityAvailabilitySet`。TUI、Exec、管理命令和其他入口只消费这个读模型。
`CapabilityAvailabilityState` 只表达产品形态可见的可用性，不暴露插件主机内部状态；插件状态、隔离和
诊断必须先经过能力服务读模型后再进入入口。

Product Profile 不承载用户 Runtime Configuration、凭据或任意构建脚本。`ProductProfileRef` 的内容摘要
标识已解析输入，不用 schema 版本代替内容版本。CLI 白标字段、Resolved Product Manifest、配置层级和
能力裁剪规则以 [`cli-product-line-design.md`](cli-product-line-design.md#5-product-profile-与白标定制)
为准；这些字段在出现真实组装消费方和验证路径前仍不构成稳定 Rust API。

`ExtensionCapabilitySet` 是产品扩展能力聚合；它不表示旧的扩展主机，也不暴露具体 adapter object。

实现注册方式：

```rust
pub struct ProductAssemblyInput {
    pub plan: ProductAssemblyPlan,
    pub services: ConcreteServiceProviders,
    pub tool_providers: Vec<Arc<dyn ToolProvider>>,
    pub harness_providers: Vec<Arc<dyn HarnessProvider>>,
    pub agents: Arc<dyn AgentDefinitionRegistry>,
    pub commands: Vec<CommandProviderRef>,
    pub entry_projections: Vec<EntryProjectionProviderRef>,
    pub plugin_runtime: PluginRuntimeBinding,
    pub hooks: RuntimeHookRegistry,
}

pub struct ProductRuntimeParts {
    pub services: RuntimeServices,
    pub tools: Arc<ToolRuntime>,
    pub harnesses: Arc<HarnessRegistry>,
    pub agents: Arc<dyn AgentDefinitionRegistry>,
    pub commands: ProductCommandRegistry,
    pub entry_projections: EntryProjectionRegistry,
    pub plugin_runtime: PluginRuntimeBinding,
    pub hooks: RuntimeHookRegistry,
}
```

注册路径：

- 具体服务提供方只注册到 `RuntimeServicesBuilder`。
- 工具提供方只注册到 `ToolRuntimeBuilder::install_provider`。
- 工作流提供方只注册到 `HarnessRegistryBuilder`。
- 智能体、子智能体、提示、skill 只注册到 `AgentDefinitionRegistry` 或对应注册表。
- 输入框命令、审核入口、MiniApp 入口只注册到 `ProductCommandRegistry`，再映射到能力或工作流。
- UI 面板、命令面板项、设置入口和状态视图当前不进入稳定描述符；只有出现真实入口消费方和公开接口预算后，才由界面归属模块定义视图。
- 适配器清单/能力集合由产品组装选择；具体适配器对象只由插件运行时主机内部加载。
  OpenCode / 插件适配器只能产出候选效果；ACP 桥接仍按协议适配器进入对应入口。
- unsupported / temporarily-unavailable 能力在 `CapabilityAvailability` 中表达，不让运行时内核读取产品形态。

示例构建流程：

```rust
pub fn build_desktop_runtime(input: DesktopAssemblyInput) -> Result<ProductRuntime, AssemblyError> {
    let services = RuntimeServicesBuilder::new()
        .with_filesystem(input.desktop_fs)
        .with_workspace(input.workspace)
        .with_permission(input.permission)
        .with_optional_git(input.git)
        .build()?;

    let tools = ToolRuntimeBuilder::new()
        .install_provider(input.core_tools)
        .install_provider(input.mcp_tools)
        .build()?;

    let plugin_runtime = input.plugin_runtime.into_runtime_boundary();

    let runtime = AgentRuntime::new(AgentRuntimeParts {
        services,
        tools,
        agents: input.agents,
        plugin_runtime,
        hooks: input.runtime_hooks,
        config: input.config,
    })?;

    Ok(ProductRuntime { runtime })
}
```

约束：

- 产品组装允许依赖具体实现；运行时内核不允许依赖具体实现。
- 不同产品允许注册不同入口命令和入口视图，但必须映射到稳定能力。
- 输入框命令、审核、MiniApp、ACP 客户端、自定义工具/子智能体/skill/插件均通过组装层注册。
- 组装层不得改变底层运行时语义来适配某个入口。
- `DeliveryProfile` 只能影响能力/提供方选择，不得让下层出现 `if desktop`
  或 `if cli` 这样的产品分支。
- Tauri 句柄、窗口、命令宏和桌面 app 状态只能存在于 Desktop 提供方或
  传输/接口适配器；运行时部件只接收类型化服务端口、DTO、事件事实和能力可用性。
- 插件运行时客户端只能作为内核可调用的类型化边界注入；智能体内核、工具运行时和工作流不直接加载
  OpenCode 插件代码。
- feature group 是构建时能力边界；能力计划和能力可用性是产品运行时能力边界；两者必须在
  组装层中显式对应，不得互相替代。
- 任何交付形态减少能力前，必须先更新 product matrix 并补产品入口验证。
- 产品组装不能把所有接口收敛到单个大对象；Rust 内核接口、能力服务读模型、能力/副作用接口
  必须按层分开。

### 4.2 产品形态与组装差异

| 产品形态 | 关键差异 | 组装时必须稳定的下层接口 / schema |
|---|---|---|
| Desktop | Tauri 窗口、桌面接口、本地权限界面 | 运行时事件、权限事实、产物引用、桌面服务提供方、能力服务读模型 |
| CLI | TUI、命令输入、终端展示、包工作流 | 命令提供方、智能体/会话/工具接口、CLI 安全服务提供方、能力服务读模型 |
| Server / SDK | HTTP/WebSocket 路由、server 工作区策略、外部 SDK 嵌入 | 传输 DTO、运行时请求/响应、工作区身份、稳定 Rust 内核接口 |
| Remote / mobile | 远端工作区、relay/bot、文件/终端视图 | 远端状态、逻辑路径、权限/事件事实、远端能力事实 |
| ACP | ACP 协议、客户端生命周期、远端探测 | 外部智能体/工具能力、环境事实、权限桥接 |
| Web UI / mobile web | UI 状态、hydration、配对、会话展示、插件状态视图 | 接口/传输 DTO、运行时事件事实、能力服务读模型 |

### 4.3 Product Capability 设计

Product Capability 是产品特性的声明单元，由产品组装消费，并引用内核 / 执行层 / 扩展层
的稳定贡献。它负责把较大粒度的产品能力拆成可组装的能力包；不拥有 UI，也不直接执行具体 IO。

`ProductProfileRef`、`ProductProfileProjection`、`CapabilityPack`、`CapabilityPlan`、`CapabilityAvailabilitySet` 和 `OverridePoint` 在本文件中只作为目标态草图出现，不构成稳定接口定义。稳定边界以
[`product-architecture.md`](product-architecture.md) 的接口切面、当前消费方和公开接口预算规则为准。运行时插件不得作为裁剪内置产品功能的主机制，Cargo feature 也不得直接当作用户可见能力事实。

建议模块：

```text
product-capabilities
  code_agent.rs
  deep_review.rs
  deep_research.rs
  miniapp.rs
  function_agent.rs
  remote_control.rs
  mcp_app.rs
  computer_use.rs
  long_running_task.rs
  plugin_extension.rs
  command_mapping.rs
```

目标态接口草图：

```rust
pub trait CapabilityPack: Send + Sync {
    fn id(&self) -> CapabilityId;
    fn shape(&self) -> CapabilityShapeDescriptor;
    fn required_services(&self) -> Vec<ServiceCapabilityId>;
    fn tool_packs(&self) -> Vec<ToolPackId>;
    fn harness_packs(&self) -> Vec<HarnessId>;
    fn agent_definitions(&self) -> Vec<AgentDefinitionRef>;
    fn command_providers(&self) -> Vec<CommandProviderRef>;
    fn entry_projections(&self) -> Vec<EntryProjectionRef>;
    fn extension_capabilities(&self) -> Vec<ExtensionCapabilityRef>;
}

pub struct CapabilityShapeDescriptor {
    pub id: CapabilityId,
    pub supported_surfaces: Vec<SurfaceKind>,
    pub dependencies: Vec<CapabilityId>,
    pub conflicts: Vec<CapabilityId>,
    pub required_providers: Vec<ServiceCapabilityId>,
    pub effects: Vec<CapabilityEffectDeclaration>,
    pub artifacts: Vec<ArtifactKind>,
    pub degradation: Vec<CapabilityDegradationRule>,
    pub verification_owner: VerificationOwnerRef,
}
```

分层规则：

- Code Agent 包允许声明智能体模式、工具包、提示模块，但不拥有工具执行。
- Deep Review 包允许声明工作流提供方、报告产物 schema、队列/重试策略，
  但目标解析和界面构造留在入口。
- MiniApp 包允许声明 MiniApp 工作流、领域端口、产物策略，但 worker 进程和
  文件系统 IO 通过运行时服务提供方。
- MCP App 包允许声明 MCP 工具/资源/提示能力；MCP 传输 / 目录属于平台/提供方适配器，
  物化后的工具/资源/提示视图属于执行层 / 稳定接口。
- 输入命令包只声明命令到能力/工作流/运行时请求的映射，不共享具体 UI。
- 长程任务包只声明任务入口、默认策略和命令映射；任务生命周期属于智能体内核。
- 插件扩展包只声明插件能力和外部接口映射；安全决策和最终状态写入属于内核 / 安全边界。

### 4.4 插件运行时主机与兼容适配器

插件运行时主机的权威设计见 [`plugin-runtime-host-design.md`](plugin-runtime-host-design.md)。本文件只约束 Agent Runtime 与插件主机的关系：

- Agent Runtime 只接收 `PluginRuntimeBinding`，不创建插件主机、不发现插件来源、不加载 OpenCode 适配器。
- `PluginRuntimeClient` 是 Agent Runtime 内部可调用边界，不进入 SDK 门面、能力服务接口或产品入口 DTO。
- OpenCode 适配层只存在于插件主机内部；Agent Runtime 不依赖 `bitfun-opencode-adapter`，也不按具体生态类型分支。
- 插件贡献进入 Agent Runtime 前必须已经是 BitFun 规范候选效果、诊断、隔离或类型化 unsupported。
- 工具贡献必须复用工具 ABI；事件订阅必须复用事件清单；权限候选必须复用安全控制面。

本文件不定义 `UiContributionDescriptor`、OpenCode client/server facade、泛 hook registry、来源发现接口或多生态能力矩阵。这些能力只有在存在真实产品消费方、公开接口预算和安全评审后，才允许进入对应归属文档和代码。

风险与保护：

| 风险 | 保护方式 |
|---|---|
| 外部生态接口反向成为内部归属模块 | OpenCode 只作为插件主机内部反腐层，输出 BitFun 接口对象或诊断 |
| Agent Runtime 直接感知具体适配器 | Agent Runtime 只依赖 `PluginRuntimeBinding` / `PluginRuntimeClient` |
| 插件越权修改权限或状态 | 插件只产出候选效果；最终决策、审计和状态写入由归属模块完成 |
| 工具 ABI 与内置/MCP/插件分裂 | custom tool 统一进入工具快照、提供方身份和权限/副作用过滤路径 |
| 远程/SDK 形态能力漂移 | 非完整入口只消费只读视图、disabled stub 或类型化 unsupported |

### 4.5 ACP 扩展方式

`bitfun-acp` 保持集成归属。

继续拥有：

- ACP protocol。
- ACP client lifecycle。
- config persistence。
- remote probing。
- startup timeout。
- workspace surface selection。

向上暴露：

```rust
pub trait ExternalAgentProvider: Send + Sync {
    fn list_agents(&self) -> Vec<ExternalAgentDescriptor>;
    async fn start(&self, request: ExternalAgentStartRequest) -> Result<ExternalAgentSession, AcpError>;
}

pub trait ExternalToolProvider: Send + Sync {
    fn tool_manifest(&self, ctx: ToolManifestContext) -> ToolManifest;
}
```

Agent Runtime SDK 只能看到 external agent/tool capability，不感知 ACP protocol、进程管理、
remote probing 或 startup timeout。

### 4.6 Skills / Prompt / Subagent

建议归属：

- prompt module：Agent Runtime SDK 的 prompt assembly contract。
- skill：prompt / resource / instruction 扩展，作为 agent definition 或 harness input 的一部分。
- subagent definition：Agent Definition Registry。
- subagent execution：Agent Runtime SDK。
- Task tool：Tool Runtime entrypoint，调用 Agent Runtime SDK。

约束：

- skills 不直接授予 service handle。
- subagent permission 来源必须包含 parent session、parent agent、target agent、surface。
- prompt module 只声明可组合内容，不执行 IO。
- skill resource 访问通过 filesystem/workspace port。

### 4.7 Hook 与 Event 设计

事件：

```rust
pub enum RuntimeEvent {
    SessionStarted(SessionStarted),
    TurnStarted(TurnStarted),
    PromptAssembled(PromptAssembled),
    ToolCallStarted(ToolCallStarted),
    PermissionRequested(PermissionRequested),
    SubagentSpawned(SubagentSpawned),
    ArtifactWritten(ArtifactWritten),
    TurnCompleted(TurnCompleted),
}
```

Runtime hook：

```rust
#[async_trait::async_trait]
pub trait PromptDecorator: Send + Sync {
    async fn decorate(&self, ctx: PromptHookContext, prompt: PromptBundle)
        -> Result<PromptBundle, HookError>;
}

#[async_trait::async_trait]
pub trait PostTurnProcessor: Send + Sync {
    async fn process(&self, ctx: PostTurnContext, outcome: TurnOutcome)
        -> Result<TurnOutcome, HookError>;
}
```

Tool hook：

```rust
#[async_trait::async_trait]
pub trait BeforeToolExecution: Send + Sync {
    async fn before(&self, ctx: ToolExecutionContext, input: ToolInput)
        -> Result<ToolInput, HookError>;
}
```

规则：

- hook registry 必须有稳定顺序。
- hook 必须有 timeout。
- hook error 必须可分类：fail turn、skip hook、deny tool、record warning。
- hook 不得获取未声明的具体 service。
- 修改 prompt / manifest / output 的 hook 必须有 snapshot 测试。

## 5. 质量保护与目标态判定

### 5.1 鲁棒性设计

错误：

- contracts crate 使用可移植错误事实。
- Agent Runtime SDK / Runtime Services 负责错误分类和事件上报边界。
- Product Surface 只负责展示逻辑。
- unsupported capability 必须明确，不允许泛化为 unknown failure。

取消：

- turn、tool、subagent、harness step 都必须接收 cancellation。
- cancellation outcome 必须可观测。
- background task 必须有 result delivery 或 explicit detached state。

持久化：

- session persistence 通过 port。
- artifact write 通过 port。
- oversized tool result 必须 flush 后再返回 ref。
- remote/local workspace path 通过 logical identity 表达。

并发：

- scheduler queue、subagent background、fork context 必须定义并发限制。
- fork context 继续保留禁止字段和递归 subagent 保护。
- 提供方注册表构建后应尽量不可变，避免运行时期间物化漂移。

### 5.2 设计边界

本文件描述目标接口、crate 内部结构和行为保护要求。若验证发现目标接口、crate 归属、行为边界或风险判断不成立，
必须先修正设计约束，再调整实现边界。

### 5.3 测试策略

Contract 测试：

- DTO serialization round-trip。
- permission facts source identity。
- artifact ref logical path。
- unsupported capability error。

Tool 测试：

- manifest ordering。
- expanded / collapsed exposure。
- `GetToolSpec` detail。
- readonly / enabled filter。
- oversized result persistence。

Runtime 测试：

- session start / turn submit / cancel。
- prompt assembly snapshot。
- post-turn processor deterministic output。
- subagent delegation policy。
- fork context seeding。
- background result delivery。

Harness 测试：

- provider 注册。
- plan 结构。
- artifact 输出。
- review gate。
- hook order。

Product 测试：

- Desktop / CLI / ACP product check。
- Remote workspace 行为。
- MCP dynamic tool catalog。
- MiniApp 与 review workflow。
- 插件状态视图和 host fallback。
- 内部 SDK 最小特性 / no-default-features 嵌入验证。
- OpenCode / plugin adapter 的 capability/effect 声明与安全决策测试。

### 5.4 目标态判定标准

- `bitfun-agent-runtime` 能在不依赖 `bitfun-core` 的情况下构建运行时内核。
- Agent Runtime SDK 门面能通过测试替身模型提供方、测试替身运行时服务、测试替身工具提供方和测试替身
  工作流提供方完成最小会话 / 轮次 / 事件流流程。
- 长程任务、调度、权限、上下文、会话/工作区、记忆、DFX、钩子/事件能作为内核能力被产品特性复用。
- `/goal`、DeepReview、MiniApp、输入框命令、settings 和 UI panel 通过特性包 / 产品组装来组装，不进入内核。
- 运行时内部接口、能力服务接口、扩展接口和主机内部 ABI 分层表达，不再合并为单一宽接口。
- 插件运行时主机 / 兼容适配器能把外部插件接口映射为 BitFun 的工具、受控钩子、工作流候选和
  能力/副作用声明，并受安全控制面约束。
- `bitfun-runtime-services` 提供类型化服务注入，并由边界检查保护。
- `tool-contracts`、`tool-provider-groups` 和 `tool-execution` 分别承担工具接口、提供方组计划和低层执行辅助；具体工具通过产品组装注册。
- `bitfun-harness` 支持工作流提供方扩展。
- `bitfun-core` 只作为兼容门面 / product-full 组装。
- 所有产品形态通过产品组装显式启用能力。
- 所有高风险行为有 snapshot、focused regression 或 product check 保护。
