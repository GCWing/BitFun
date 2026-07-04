# Agent Kernel、Runtime Services 与 Extension API 设计

本文是 [`core-decomposition.md`](core-decomposition.md) 的开发设计文档，描述目标模块、接口、
crate 内部结构和行为保护。本文只记录设计约束，不记录实现过程或验证记录。插件运行时主机、
生态兼容适配层、IPC 和候选效果契约见
[`plugin-runtime-host-design.md`](plugin-runtime-host-design.md)。

## 1. 设计目标与边界

- Agent Kernel 可被 Desktop、CLI、Server、Remote、ACP、Web 和独立 SDK 形态嵌入。
- Agent Kernel 对外提供稳定、窄口径的 Rust runtime API，而不是暴露 `bitfun-core`、产品命令路径或 concrete manager。
- Product Feature 把内核能力组装为用户侧能力，可能同时触达 Rust 和 UI，但不拥有内核状态机或平台实现。
- Product API 同时包含 Rust Kernel API 与 UI Extension Contract；OpenCode / ACP / plugin adapter 仅承担映射和注册。
- Agent Kernel 不感知平台差异、工具实现差异、UI host 差异和构建形态差异。
- Tool、Skill、MCP、Harness 和 extension 使用通用接口和 provider / contribution 注册，不绑定底层实现。
- 具体 service、UI host、Plugin Runtime Host binding 和 adapter manifest set 由上层 Product Assembly 注入。
- 每个 crate 只依赖最小稳定集合，依赖方向可检查。

### 1.1 SDK 发布边界

Agent Runtime SDK 的发布边界以调用方能力为准，而不是以物理 crate 命名为准。达到目标状态时，外部调用方
应能在不依赖 `bitfun-core`、app crate、Tauri 或产品内部 manager 的情况下完成以下动作：

- 构建 runtime：注入 model provider、`RuntimeServices`、tool provider、harness provider、agent definitions、
  hooks 和 runtime config。
- 发起执行：创建或恢复 session，提交 turn，取消 turn，消费 provider-neutral event stream。
- 执行工具：通过稳定 tool manifest、permission request、tool result、artifact ref 和 cancellation contract
  管理工具调用。
- 扩展能力：通过 registry 注册 subagent、prompt module、skill、MCP/API tool、harness workflow 和 post-turn
  processor。
- 处理运维语义：接收 typed error、usage/cost/cache facts、telemetry event、checkpoint/resume facts 和
  unsupported capability。

因此，SDK readiness 的最低标准是：

- 公共 facade 只暴露 builder、runner、request/response DTO、event stream、typed error 和 registry API。
- 所有 DTO 可序列化，所有 runtime handle 通过 typed port 注入，不进入 wire contract。
- `bitfun-agent-runtime`、Tool primitives、Runtime Services 和 Harness 能通过 fake provider 独立测试。
- SDK minimal feature 不牵引 Desktop、Tauri、Git provider、MCP client、AI HTTP client、remote SSH 或产品 UI。
- 完整产品能力只能通过 Product Assembly 或兼容 `bitfun-core/product-full` 组装，不反向污染 SDK API。

SDK 公共 API 以 `AGENT_RUNTIME_SDK_API_VERSION` 标记兼容边界。当前 API version 为 v1 preview：
小版本更新允许增加可选 builder hook、DTO 字段或 registry 查询能力，但不得改变既有端口语义、
错误分类、session / turn 标识含义或默认 feature 依赖。任何需要调用方改写现有嵌入代码的变更，
必须提升 API version 并提供兼容迁移路径。

只要外部调用方仍必须导入 `bitfun-core`、启用 `product-full`、持有 concrete service manager、读取产品命令
registry 或依赖全局 mutable state，SDK 发布边界就不成立。

### 1.2 内核与特性的分界

内核能力和产品特性必须分开判断：

| 领域 | 属于内核 | 属于产品特性 |
|---|---|---|
| 长程任务 | task identity、queue、resume、cancel、event、persistence facts | `/goal` 命令、默认目标模板、UI 展示、设置项和产品文案 |
| 权限 | permission facts、source identity、decision request、audit event | 桌面弹窗、CLI 提示、Web 状态投影和产品默认选项 |
| 上下文 | session/workspace facts、context assembly contract、memory port | 具体入口的上下文展示、快捷命令和 feature 默认配置 |
| 模型调度 | provider-neutral model route request、usage/cost/cache facts | 产品形态默认模型、设置入口和降级文案 |
| Hook / Event | event schema、hook order、timeout、error policy | 哪些 feature 注册 hook、UI 如何展示 hook 结果 |

判断标准：

- 在 Desktop、CLI、Web、ACP 和 SDK 中都可复用，且不依赖 UI 或平台 concrete 的能力，优先归 Agent Kernel。
- 会改变用户入口、命令、设置、UI contribution、默认策略或产品文案的能力，归 Product Feature。
- 会接触 OS、network、terminal、filesystem、remote host、MCP server 或 AI provider concrete 的能力，归 Cross-platform
  Adapter 或 protocol adapter。
- 来自外部插件、OpenCode、ACP、external skill 或第三方 package 的能力，先进入 Extension Layer，再由 Product
  Assembly 注册到 feature / kernel / execution 的稳定接口。

### 1.3 Product API 与 Extension API

Product API 不应被定义为单一 Rust 后端 API。目标 API 分为三类：

| API | 作用 | 主要消费者 | 约束 |
|---|---|---|---|
| Rust Kernel API | 创建 runtime、提交 turn、消费 event、注册 ports/providers/hooks | Product Assembly、SDK、extension adapter | 不暴露 `bitfun-core/product-full` 或 concrete manager |
| UI Extension Contract | 描述 UI contribution、命令入口、设置项、panel、状态投影 | Web UI、Desktop UI、插件适配层 | 只传 descriptor 和 stable DTO，不 import React/Tauri implementation |
| Capability / Effect API | 声明 tool、MCP、plugin、hook、network、file、shell 等能力和副作用 | Plugin Runtime Host、Security Boundary、Product Assembly | 未声明或超声明的能力默认受限 |

OpenCode adapter、ACP bridge 和未来 plugin runtime 必须把外部 API 映射为上述三类 API，再由 Product Assembly
统一注册。它们不能直接写 Agent Kernel 权威状态，也不能绕过 permission、sandbox、audit 或 UI host 的渲染边界。

Agentic frontend event projection 属于稳定 event contract：Agent Kernel 产生 provider-neutral `AgenticEvent`，
契约层给出 event name、event type 和 payload；Tauri、WebSocket、OpenCode adapter 或 UI host
只选择 delivery/profile，不重新定义字段映射。

Extension registration contract 属于稳定扩展契约，不属于 Product Assembly 的具体实现。Plugin Runtime Host 和
compatibility adapter 只产出可注册的 descriptor、provider 和 contribution；Product Assembly 消费这些 contract 并完成
产品形态内的注册，避免 Extension 层反向依赖 assembly crate。

### 1.4 API 与 crate 边界

本设计按 API owner 划分 crate，而不是按调用方或产品形态划分。一个 crate 只能拥有一类稳定边界；如果同一文件同时
处理 UI 入口、产品策略、内核状态和 OS I/O，应拆到对应 owner。

| API / owner | 主要 crate | 允许依赖 | 不允许依赖 | 对外承诺 |
|---|---|---|---|---|
| Product Assembly API | `src/crates/assembly/*` | feature packs、Kernel API、Execution API、Runtime Services、platform providers | Agent 内部状态机、具体 UI 组件实现作为下层依赖 | 按产品形态组装能力，输出 typed runtime parts |
| Product Feature API | `product-capabilities`、`product-domains`、对应 UI contribution owner | Kernel API、UI Extension Contract、Capability/Effect contract、domain contract | OS concrete、Tauri handle、Execution concrete、permission 最终策略 | 把内核能力和稳定 descriptor 映射为用户功能和默认策略 |
| Rust Kernel API | `agent-runtime`、`agent-stream`、`runtime-services`、`runtime-ports`、`events`、`core-types` | stable contracts、tool/harness registry、typed services | `bitfun-core`、Tauri、Web UI、ACP protocol、provider concrete | session / turn / event / permission / scheduler / context 等 SDK 候选接口 |
| Execution API | `tool-contracts`、`tool-provider-groups`、`tool-execution`、`harness` | stable contracts、runtime ports、注入的 service ports | product registry、UI、具体 filesystem/Git/terminal/MCP client | tool、skills、MCP tool bridge、sandbox、harness 执行语义 |
| Extension API | Plugin Runtime Host / OpenCode compatibility / ACP adapter owner | Rust Kernel API contract、UI Extension Contract、Capability/Effect contract | Web UI React implementation、Tauri state、kernel 权威状态写入 | 把外部生态能力转换为 descriptor、provider 和 candidate effect |
| Platform / Provider Adapter API | `services/*`、`adapters/*`、app-local provider | runtime ports、stable DTO、允许的第三方库 | Product Feature、Agent Kernel 状态机、UI command | 实现 filesystem、terminal、network、remote、Git、MCP transport、AI provider 等边界外 I/O |
| Stable Contract API | `contracts/*` | 低层无行为依赖或标准序列化依赖 | 上层 crate、concrete manager、UI rendering | DTO、event、port、capability/effect、permission、sandbox、audit、typed error |

禁止依赖：

- `contracts/*` 或 `runtime-ports` 依赖 `bitfun-core`、assembly、apps、UI 或 concrete service。
- `agent-runtime` 依赖 `bitfun-core`、Tauri、Web UI、ACP protocol、AI provider concrete、MCP client concrete 或 OS service manager。
- `tool-contracts` 依赖具体 service crate；`tool-execution` 依赖产品 registry、产品 permission policy 或具体 UI。
- `harness` 依赖具体 filesystem/Git/terminal manager；它只通过 ports 和 provider contract 获取能力。
- Plugin Runtime Host 不能依赖 Web UI React component implementation、Tauri app state 或 concrete core manager。
- Product Feature 直接依赖 platform adapter concrete、execution concrete、全局 mutable runtime state 或边界外资源 client。

接口暴露原则：

- 对外 API 按层拆分：Rust Kernel API、UI Extension Contract、Capability/Effect API、Product Assembly API 分别定义。
- 下层不暴露上层对象。Kernel 不返回 UI command；Execution 不返回 React descriptor 以外的 UI 实现；Platform Adapter 不返回产品命令。
- 注册接口接收 typed provider / descriptor / policy，不接收 `Any`、无类型 service name 或全局 mutable registry。
- 兼容 facade 可以保留旧路径导出，但旧路径不得成为新 API 的真实 owner。

### 1.5 平台适配与边界外资源

Platform / Provider Adapter 是仓库内实现层，负责把稳定 port 转换为 OS、network、terminal、remote、MCP
transport、AI provider、browser runtime 或第三方库调用。边界外资源不是 crate、不是逻辑层，也不是所有模块可依赖的
基础设施。

实现规则：

- Product Assembly 是唯一可以选择具体 platform provider 的位置；选择结果以 typed runtime parts 注入。
- Kernel、Execution、Extension 和 Product Feature 只消费 stable contract、port handle 或 descriptor，不导入具体
  provider crate。
- Platform adapter 不读取 delivery profile、feature pack 或 UI command；形态差异由 Product Assembly 注入。
- 外部资源错误必须在 adapter 边界转换为 typed error、unsupported/unavailable 或 capability/effect fact，不能泄漏为
  产品层专用分支。

## 2. 稳定接口与运行时服务

### 2.1 稳定契约（Stable Contracts）

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

### 2.2 Runtime Services

目标 owner crate：`bitfun-runtime-services`。

职责：

- 承载 runtime 可消费的 typed service bundle。
- 提供 provider 注册和 capability resolution。
- 把具体实现与 runtime port 隔离。
- 提供统一的 unavailable / unsupported 错误。
- 为测试提供 fake provider builder。

建议内部模块：

```text
bitfun-runtime-services
  bundle.rs             # RuntimeServices / ToolServices / HarnessServices
  builder.rs            # typed builder
  capability.rs         # capability ids 与 availability
  registry.rs           # provider 注册
  errors.rs             # unsupported / unavailable 映射
  test_support.rs       # fake providers
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

- `RemoteConnectionPort` 只描述连接身份、状态、认证上下文和连接生命周期请求，不暴露 SSH / relay / tunnel concrete handle。
- `RemoteWorkspacePort` 只描述 remote workspace identity、root resolution、startup guard 和 persistence/session facts。
- `RemoteProjectionPort` 只描述 file、terminal、image/context projection 的 request / response shape，不直接执行具体 OS 命令。
- `RemoteCapabilityPort` 只描述 remote host capability facts，例如 filesystem、terminal、review platform、model catalog 支持状态。
- SSH、relay、本地隧道、远端 OS、认证和 transport 实现必须留在具体 Remote provider，由 Product Assembly 注册。

设计约束：

- 不提供 `get<T>() -> Any` 作为主路径。
- capability 缺失必须返回 typed unsupported 错误。
- 不在 runtime services 中执行产品命令。
- 不在 runtime services 中创建 concrete manager；创建发生在 Product Assembly。
- `RuntimeServices` 是运行时依赖集合，不是全局 mutable app state。

### 2.3 安全控制面接口

安全控制面把 tool、MCP、skills、plugin、hook、shell、network、file、browser/desktop 和 remote 动作归一为
capability/effect/security decision。它跨越 Kernel、Execution、Extension、Cross-platform Adapter 和 UI projection，
但最终决策必须由 Product Assembly 注入的确定性策略实现和内核事实共同约束。该接口定义的是跨层 contract，
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
- Plugin Runtime Host 必须声明 capability/effect，未知或超声明能力默认受限。
- `allow_in_sandbox` 只能在实际 sandbox 或隔离路径存在时返回。
- 远程、ACP、MCP、plugin、browser/desktop 和 cloud task 必须携带 execution domain。
- 模型输出只能辅助解释和候选判断，不能直接写 permission、audit 或 policy state。

## 3. Kernel / Tool / Harness 内核

### 3.1 Agent Kernel / Runtime SDK

目标 owner crate：`bitfun-agent-runtime`。

目标职责：

- session 生命周期。
- dialog turn / model round 生命周期。
- long-running task 生命周期、resume/checkpoint fact 和 result delivery。
- scheduler / queue / cancellation。
- permission coordination 和安全事实投递。
- model routing / usage / cost / cache facts。
- prompt loop 和 context assembly。
- prompt cache 协调。
- memory / workspace facts。
- DFX / telemetry / audit facts。
- agent definition registry、subagent registry 查询和 delegation policy。
- fork context seeding。
- tool call 调度。
- runtime events。
- post-turn processor。

公共 facade：

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

该 facade 是目标 API 形态。它必须只接收已组装的 typed parts，不负责创建
filesystem、terminal、MCP、AI client、Remote provider 或产品命令。
当前 v1 preview API 以 message / attachment / metadata 作为最小输入形态；若把
model-round cancellation token、structured AgentInput 或更复杂的 event cursor 纳入公开 SDK，
必须提升 SDK API version 并保留旧路径兼容。

产品特性边界：

- `/goal`、slash command、输入框按钮、设置项、UI panel 和默认文案不进入 Agent Kernel。
- 内核只提供 goal / long-running task 所需的任务 identity、生命周期、队列、resume/cancel、event 和 persistence fact。
- Product Feature 负责把这些内核事实映射为 `/goal` 命令、可见状态、快捷操作和默认策略。
- 若某个 feature 需要修改 Rust 和 UI，必须以 feature pack 同时声明 Rust runtime request、UI contribution 和
  capability/effect，不得仅在单侧隐式扩展。

兼容边界：

- `bitfun-agent-runtime` 只能依赖稳定契约、Tool Runtime、Runtime Services 接口和注入的 provider。
- concrete scheduler 生命周期、session metadata store、token subscriber、event delivery、product `Tool`
  handler、concrete prompt assembly、workspace / remote / config IO、custom subagent file IO 和平台 adapter
  在行为等价未证明前不得下沉到 runtime kernel。
- product feature command、UI state、settings persistence、plugin UI rendering 和 delivery-profile 默认策略不得下沉到
  runtime kernel。
- prompt、event、thread goal、scheduler 或 subagent 的纯事实如果进入 Agent Runtime SDK，旧 owner 只能保留兼容入口；
  行为等价需要有 contract test 和 boundary guard 证明。

建议内部模块：

```text
bitfun-agent-runtime
  lib.rs
  runtime.rs            # AgentRuntime 公共 API
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

公共 API：

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

- provider-neutral manifest、catalog、permission gate、execution admission、tool hook、execution result
  presentation 和 result artifact policy。
- `GetToolSpec` catalog、detail、assistant result 和 collapsed-tool unlock observation。
- workspace service、path policy、runtime artifact reference、remote path containment 和 tool context facts 的
  稳定 contract。

兼容边界：

- core 允许保留旧路径 facade、concrete tool adapter、state update、registry lookup、confirmation、actual
  execution 和 filesystem persistence；目标状态要求只有在等价测试保护下才能移动这些行为。
- workspace file/shell contract 保留既有错误与取消语义；不得把错误分类、取消语义或产品 tool exposure
  变更混入 owner 边界移动。

设计约束：

- `ToolExecutionContext` 不暴露具体 manager。
- `ToolContextFacts` 只包含 portable facts。
- Tool primitives 只消费 `ToolExecutionServices` 这样的窄 service 视图，不依赖完整
  `RuntimeServices` bundle。
- path policy、runtime artifact ref、remote POSIX containment 由 `tool-contracts` 承载。
- MCP tool 作为 external tool provider 注入，不内置在 Agent Runtime SDK。
- `GetToolSpec` 是 tool catalog 能力，不是产品 UI。

必须保护：

- prompt-visible manifest。
- expanded / collapsed exposure。
- `GetToolSpec` schema / assistant detail / detail JSON。
- collapsed unlock state 与 persistence 生命周期。
- readonly / enabled snapshot filter。
- MCP / ACP / desktop tool catalog 等价。
- oversized tool result persistence、flush、preview、artifact ref。
- Write/Edit/Read file-read-state guardrail。

### 3.3 Harness Layer

目标 owner crate：`bitfun-harness`。

职责：

- 把 SDD、DeepReview、DeepResearch、MiniApp、function-agent 等工作流从 runtime kernel 中分离。
- 定义 workflow descriptor、route plan、provider registry、workflow plan、step、policy、artifact、
  review gate 和 post-processor。
- 通过 Agent Runtime SDK、Tool Runtime 和 service ports 编排。

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

- harness 允许编排 runtime/tool，但不拥有 session manager internals。
- harness 不直接访问 concrete filesystem / Git / terminal。
- 产品命令只映射到 harness capability，不把命令展示逻辑下沉。
- 新 harness 通过 provider 注册，不改 Agent Runtime SDK 内核。
- descriptor-only / legacy-facade provider 只能表达 route plan；不得被描述为已经拥有 concrete workflow execution。
  执行语义移动必须单独证明行为等价。

## 4. 产品组装与扩展

### 4.1 Product Assembly

Product Assembly 是 composition root。初始状态可由 `bitfun-core` 兼容 facade 承载；目标状态可拆成独立
Product Assembly crate。它按 feature bundle 触发组装，同时连接 UI host、Rust Kernel、Execution、Plugin Runtime
Host 和 Cross-platform Adapter。

职责：

- 创建或接收具体 service 实现、Plugin Runtime Host binding 和 adapter manifest set。
- 构建 `RuntimeServices`。
- 注册 tool provider groups。
- 注册 harness providers。
- 注册 agent definitions、subagents、skills、prompt modules。
- 注册 plugin runtime binding、adapter manifest set、ACP bridge 和 UI contribution provider。
- 建立产品 feature matrix。
- 把 interface 命令映射到 capability / harness / runtime request。
- 把 feature bundle 映射为 Rust runtime request、UI contribution、plugin capability 和安全策略。
- 根据交付形态选择 `DeliveryProfile`、`CapabilitySet`、adapter manifest set 和 service provider 集合。
- 对不支持能力返回 typed unsupported / unavailable 错误，而不是让下层 runtime 判断产品形态。
- 通过 typed `PluginRuntimeBinding` 向 Kernel 注入插件运行时 client 或 disabled stub，不使用全局 registry。

建议模块：

```text
product-assembly
  full.rs
  delivery_profile.rs
  capability_set.rs
  feature_bundle.rs
  desktop.rs
  cli.rs
  server.rs
  remote.rs
  acp.rs
  feature_matrix.rs
  ui_contributions.rs
  extensions.rs
  commands.rs
```

核心结构：

```rust
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

pub struct CapabilitySet {
    pub agent_modes: Vec<AgentModeId>,
    pub tool_packs: Vec<ToolPackId>,
    pub harness_packs: Vec<HarnessId>,
    pub service_capabilities: Vec<ServiceCapabilityId>,
    pub command_providers: Vec<CommandProviderId>,
    pub ui_contributions: Vec<UiContributionId>,
    pub extensions: ExtensionCapabilitySet,
}

pub struct ProductAssemblyPlan {
    pub profile: DeliveryProfile,
    pub capabilities: CapabilitySet,
    pub feature_groups: Vec<FeatureGroupId>,
    pub feature_bundles: Vec<FeatureBundleId>,
}

pub struct ExtensionCapabilitySet {
    pub plugin_runtime: PluginRuntimeAvailability,
    pub adapters: Vec<PluginAdapterCapability>,
    pub ui: UiExtensionAvailability,
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

pub trait ProductAssembler {
    fn plan(&self, profile: DeliveryProfile) -> Result<ProductAssemblyPlan, AssemblyError>;
    fn build(&self, plan: ProductAssemblyPlan) -> Result<ProductRuntime, AssemblyError>;
}
```

`ExtensionCapabilitySet` 是产品扩展能力聚合；它不表示旧的扩展主机，也不暴露具体 adapter object。

实现注册方式：

```rust
pub struct ProductAssemblyInput {
    pub profile: DeliveryProfile,
    pub services: ConcreteServiceProviders,
    pub tool_providers: Vec<Arc<dyn ToolProvider>>,
    pub harness_providers: Vec<Arc<dyn HarnessProvider>>,
    pub agents: Arc<dyn AgentDefinitionRegistry>,
    pub commands: Vec<CommandProviderRef>,
    pub ui_contributions: Vec<UiContributionProviderRef>,
    pub plugin_runtime: PluginRuntimeBinding,
    pub hooks: RuntimeHookRegistry,
}

pub struct ProductRuntimeParts {
    pub services: RuntimeServices,
    pub tools: Arc<ToolRuntime>,
    pub harnesses: Arc<HarnessRegistry>,
    pub agents: Arc<dyn AgentDefinitionRegistry>,
    pub commands: ProductCommandRegistry,
    pub ui_contributions: UiContributionRegistry,
    pub plugin_runtime: PluginRuntimeBinding,
    pub hooks: RuntimeHookRegistry,
}
```

注册路径：

- concrete service provider 只注册到 `RuntimeServicesBuilder`。
- tool provider 只注册到 `ToolRuntimeBuilder::install_provider`。
- harness provider 只注册到 `HarnessRegistryBuilder`。
- agent、subagent、prompt、skill 只注册到 `AgentDefinitionRegistry` 或对应 registry。
- 输入框命令、审核入口、MiniApp 入口只注册到 `ProductCommandRegistry`，再映射到 capability 或 harness。
- UI panel、command palette item、settings entry 和状态投影只注册为 `UiContributionDescriptor`，由对应 UI host 渲染。
- Adapter manifest/capability set 由 Product Assembly 选择；具体 adapter object 只由 Plugin Runtime Host 内部加载。
  OpenCode / plugin adapter 只能产出候选效果；ACP bridge 仍按协议 adapter 进入对应入口。
- unsupported / unavailable 能力在 `CapabilityAvailability` 中表达，不让 runtime kernel 读取产品形态。

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

- Product Assembly 允许依赖具体实现；runtime kernel 不允许依赖具体实现。
- 不同产品允许注册不同 surface command 和 UI contribution，但必须映射到稳定 capability。
- 输入框命令、审核、MiniApp、ACP client、自定义 tool/subagent/skill/plugin 均通过 assembly 注册。
- assembly 不得改变底层 runtime 语义来适配某个 surface。
- `DeliveryProfile` 只能影响 capability/provider 选择，不得让下层出现 `if desktop`
  或 `if cli` 这样的 product 分支。
- Tauri handle、window、command macro 和 desktop app state 只能存在于 Desktop provider 或
  transport/API adapter；runtime parts 只接收 typed service port、DTO、event fact 和 capability availability。
- 插件运行时 client 只能作为 Kernel 可调用的 typed boundary 注入；Agent Kernel、Tool Runtime 和 Harness 不直接加载
  OpenCode 插件代码。
- feature group 是构建时能力边界，`CapabilitySet` 是产品运行时能力边界；两者必须在
  assembly 中显式对应。
- 任何交付形态减少能力前，必须先更新 product matrix 并补产品入口验证。
- Product Assembly 不能把所有 API 收敛到单个大对象；Rust Kernel API、UI Extension Contract、Capability/Effect API
  必须按层分开。

### 4.2 产品形态与组装差异

| 产品形态 | 关键差异 | 组装时必须稳定的下层契约 |
|---|---|---|
| Desktop | Tauri window、desktop API、本地 permission UI、UI contribution host | runtime events、permission facts、artifact refs、desktop service providers、UI extension contract |
| CLI | TUI、命令输入、终端展示、package workflow | command provider、agent/session/tool contract、CLI-safe service providers、text UI contribution |
| Server / SDK | HTTP/WebSocket route、server workspace policy、外部 SDK 嵌入 | transport DTO、runtime request/response、workspace identity、stable Rust Kernel API |
| Remote / mobile | remote workspace、relay/bot、file/terminal projection | remote state、logical path、permission/event facts、remote capability facts |
| ACP | ACP protocol、client lifecycle、remote probing | external agent/tool capability、environment facts、permission bridge |
| Web UI / mobile web | UI state、hydration、pairing、session 展示、插件 UI contribution | API/transport DTO、runtime event facts、UI extension contract |

### 4.3 Product Capability 设计

Product Capability 位于 Product Assembly 与 Product Feature / Harness / Runtime / Tool / Extension 之间，负责把
大块产品能力拆成可组装的 capability pack。它不拥有 UI，也不直接执行具体 IO。

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

核心接口：

```rust
pub trait CapabilityPack: Send + Sync {
    fn id(&self) -> CapabilityId;
    fn required_services(&self) -> Vec<ServiceCapabilityId>;
    fn tool_packs(&self) -> Vec<ToolPackId>;
    fn harness_packs(&self) -> Vec<HarnessId>;
    fn agent_definitions(&self) -> Vec<AgentDefinitionRef>;
    fn command_providers(&self) -> Vec<CommandProviderRef>;
    fn ui_contributions(&self) -> Vec<UiContributionRef>;
    fn extension_capabilities(&self) -> Vec<ExtensionCapabilityRef>;
}
```

分层规则：

- Code Agent pack 允许声明 agent modes、tool packs、prompt modules，但不拥有 tool execution。
- Deep Review pack 允许声明 harness provider、report artifact contract、queue/retry policy，
  但 target resolution 和 UI construction 留在 surface。
- MiniApp pack 允许声明 MiniApp harness、domain ports、artifact policy，但 worker process 和
  filesystem IO 通过 Runtime Services provider。
- MCP App pack 允许声明 MCP tool/resource/prompt capability，但 MCP transport 属于
  `bitfun-services-integrations`。
- Input command pack 只声明 command 到 capability/harness/runtime request 的映射，不共享具体 UI。
- Long-running task pack 只声明任务入口、默认 policy、UI contribution 和 command 映射；任务生命周期属于 Agent Kernel。
- Plugin extension pack 只声明插件能力、UI contribution 和外部 API 映射；安全决策和最终状态写入属于 Kernel / Security Boundary。

### 4.4 Plugin Runtime Host 与 Compatibility Adapter

Plugin Runtime Host 是插件层运行时，不是第二个 Agent Kernel。它负责插件兼容层生命周期、主机通信、项目执行域、
隔离、健康、deadline、幂等、adapter registry 和候选效果路由。Compatibility Adapter 运行在该 Host 内部，
只把 OpenCode、Claude Code、Codex 插件或 BitFun 原生插件 API 映射为 BitFun 稳定合同。

主体进程只能依赖 `PluginRuntimeClient`、`PluginRuntimeBinding`、dispatch/response envelope、candidate effect、
trust policy 和 UI descriptor。主体进程不得按具体 adapter 类型分支，也不得读取插件 worker、subprocess、
package manager 或生态原始对象。

核心合同分为六组：

| 合同 | 作用 | 不能暴露 |
|---|---|---|
| Runtime binding | Product Assembly 注入 enabled、projection-only 或 disabled 插件运行时 | 具体生态 adapter、worker handle、全局 registry |
| Adapter manifest | 描述生态、来源、版本、capability/effect、兼容等级 | package manager 具体实现、下载路径、secret |
| Lifecycle | initialize、activate、deactivate、reload、dispose、failure quarantine | kernel state、UI implementation、concrete manager |
| Contribution set | tool、hook、workflow、UI descriptor、config/provider transform | React/Tauri object、完整 `RuntimeServices` |
| Security bridge | permission/effect request、sandbox facts、audit candidate | 直接授权写入、policy state mutation |
| Compatibility mapping | 外部生态 API 到 BitFun contract 的映射 | 作为内部真实 owner 的 OpenCode/Claude/Codex 类型 |

主体进程可见接口：

```rust
pub trait PluginRuntimeClient: Send + Sync {
    fn availability(&self) -> PluginRuntimeAvailability;
    async fn dispatch(&self, envelope: PluginDispatchEnvelope) -> Result<PluginResponseEnvelope, PluginRuntimeError>;
    async fn refresh_project(&self, update: ProjectRuntimeRefresh) -> Result<(), PluginRuntimeError>;
    async fn shutdown(&self, request: PluginRuntimeShutdown) -> Result<(), PluginRuntimeError>;
}
```

Host 内部建议接口：

```rust
pub(crate) trait CompatibilityAdapter: Send + Sync {
    fn manifest(&self) -> PluginAdapterManifest;
    fn capabilities(&self) -> Vec<PluginCompatibilityCapability>;
    async fn map_event(&self, ctx: AdapterDispatchContext) -> Result<Vec<PluginEffectCandidate>, PluginRuntimeError>;
    async fn dispose(&self) -> Result<(), PluginRuntimeError>;
}

pub struct PluginAdapterManifest {
    pub adapter_id: PluginAdapterId,
    pub ecosystem: PluginEcosystem,
    pub version: PluginAdapterVersion,
    pub supported_levels: Vec<PluginCompatibilityLevel>,
    pub effects: Vec<EffectDeclaration>,
}
```

`PluginEffectCandidate`、`PluginDispatchEnvelope`、`PluginResponseEnvelope` 和 `UiContributionDescriptor` 的权威 schema
由 [`plugin-runtime-host-design.md`](plugin-runtime-host-design.md) 定义。本文不维护第二套字段。
`Arc<dyn ToolProvider>`、`RuntimeHookProviderRef`、`Arc<dyn HarnessProvider>` 等内部 provider trait 只能在
Product Assembly 内部 materialize，不能成为插件或 SDK 的公开返回值。
`CompatibilityAdapter` 是 Host-internal 接口；Product Assembly 只选择 adapter manifest/capability set 和
`PluginRuntimeBinding`，不持有具体 adapter object。

OpenCode adapter 的边界：

- OpenCode server plugin 映射到 tool provider candidate、runtime hook candidate、permission candidate hook、
  config/provider transform 和 event subscription。
- OpenCode TUI plugin 映射到 UI contribution descriptor，包括 slot、route、command/keymap、prompt augmentation、
  dialog/toast、theme 和只读 state view。
- OpenCode `permission.ask` 只能映射为 candidate decision；最终 permission decision、audit result 和 policy write
  仍由 Security Boundary 完成。
- OpenCode tool 映射到 BitFun Tool ABI 时必须进入 materialized snapshot，携带 provider identity、schema、
  permission/effect declaration、cancellation 和 stale call guard。
- OpenCode event subscription 只能消费 BitFun public event manifest；不得读取 session、turn、tool 或 UI host
  的内部结构。
- OpenCode config/provider/model/skill/MCP transform 必须逐项进入 support matrix。未支持能力返回 typed unsupported。

UI Extension Contract：

```rust
pub struct UiExtensionContract {
    pub slots: Vec<UiSlotDescriptor>,
    pub routes: Vec<UiRouteDescriptor>,
    pub commands: Vec<UiCommandDescriptor>,
    pub prompt_contributions: Vec<UiPromptContributionDescriptor>,
    pub state_views: Vec<UiStateViewDescriptor>,
}

pub struct UiSlotDescriptor {
    pub id: UiSlotId,
    pub surface: UiSurfaceId,
    pub payload_contract: UiPayloadContractRef,
    pub fallback: UiUnsupportedFallback,
}
```

设计约束：

- UI descriptor 只描述可渲染贡献，不包含 React component、Tauri window、DOM node 或 renderer handle。
- Desktop、Web、CLI 可以对同一 descriptor 提供不同 host 实现；不支持时必须返回 unsupported/unavailable。
- UI state view 必须是只读 projection，不能成为插件写 kernel state 的旁路。
- Plugin lifecycle 的 dispose 必须撤销 tool、hook、workflow、UI contribution、event subscription 和 transform。
- Product Assembly 只能通过 support matrix 暴露已实现能力；未实现能力必须返回 typed unsupported。

风险与保护：

| 风险 | 保护方式 |
|---|---|
| 外部生态 API 反向成为内部 owner | BitFun 先定义 Plugin Runtime Host、Tool ABI、Event Manifest、Permission/Effect 和 UI descriptor |
| 主体进程直接感知具体 adapter | 主体进程只依赖 `PluginRuntimeClient` 和规范 envelope；adapter 类型仅在 Host 内部出现 |
| 插件越权修改权限或状态 | hook 只产出候选效果；最终 decision / audit / state write 由 Security Boundary 完成 |
| UI contribution 绑定具体前端实现 | descriptor-only contract；host adapter 渲染；unsupported 显式返回 |
| Tool ABI 与 built-in/MCP/plugin 分裂 | 统一 materialized snapshot、provider identity、permission/effect filter 和 stale call guard |
| 插件生命周期泄漏 | 注册 token + dispose finalizer；reload 必须撤销旧 contribution 后再注册 |
| 远程/SDK 形态能力漂移 | contribution 必须声明 execution domain、workspace identity、required capabilities 和 fallback |
| 外部 JS/TS runtime 安全不可控 | 默认目标态只承诺受控 Host 和 compatibility adapter；可写 runtime 需要单独安全评审 |

### 4.5 ACP 扩展方式

`bitfun-acp` 保持 integration owner。

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

- contract 层使用 portable error facts。
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
- provider registry 构建后应尽量 immutable，避免 runtime 期间 materialization 漂移。

### 5.2 设计边界

本文只描述目标接口、crate 内部结构和行为保护要求。若验证发现目标接口、crate 归属、行为边界或风险判断不成立，
应先修正设计判断，再调整实现边界。

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
- UI contribution descriptor round-trip 和 host fallback。
- SDK minimal feature / no-default-features 嵌入验证。
- OpenCode / plugin adapter 的 capability/effect 声明与安全决策测试。

### 5.4 目标态判定口径

- `bitfun-agent-runtime` 能在不依赖 `bitfun-core` 的情况下构建 runtime kernel。
- Agent Runtime SDK facade 能通过 fake model provider、fake runtime services、fake tool provider 和 fake
  harness provider 完成最小 session / turn / event stream 流程。
- 长程任务、scheduler、permission、context、session/workspace、memory、DFX、hook/event 能作为内核能力被产品特性复用。
- `/goal`、DeepReview、MiniApp、输入框命令、settings 和 UI panel 通过 feature pack / Product Assembly 组装，不进入内核。
- Product API 同时包含 Rust Kernel API、UI Extension Contract 和 Capability/Effect API。
- Plugin Runtime Host / compatibility adapter 能把外部 plugin API 映射为 BitFun 的 tool、hook、workflow、UI contribution 和
  capability/effect declaration，并受安全控制面约束。
- `bitfun-runtime-services` 提供 typed service injection，并由 boundary check 保护。
- `tool-contracts`、`tool-provider-groups` 和 `tool-execution` 分别承担 tool contract、provider group plan 和低层 execution helper；具体 tool 通过 Product Assembly 注册。
- `bitfun-harness` 支持工作流 provider 扩展。
- `bitfun-core` 只作为兼容 facade / product-full assembly。
- 所有产品形态通过 Product Assembly 显式启用能力。
- 所有高风险行为有 snapshot、focused regression 或 product check 保护。
