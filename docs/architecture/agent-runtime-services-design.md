# Agent Runtime SDK 与 Runtime Services 设计

本文是 [`core-decomposition.md`](core-decomposition.md) 的开发设计文档，描述目标模块、
接口、crate 内部结构和迁移保护。`bitfun-runtime-services` 已建立 typed service bundle、
builder、provider registry、capability availability 和 fake provider 基础；`bitfun-agent-runtime`
已创建并只承接可独立构建的 scheduler/background delivery 纯决策；`bitfun-harness`
已创建并承接 workflow descriptor、legacy route plan 和 provider registry contract。
未迁移的 session manager、prompt loop、subagent registry、concrete scheduler lifecycle 和
concrete workflow execution 不得被描述为已完成。

## 1. 设计目标与边界

- Agent Runtime SDK 可被 Desktop、CLI、Server、Remote、ACP 等产品形态嵌入。
- Runtime 不感知平台差异、工具实现差异和构建形态差异。
- Tool 使用通用接口和 provider 注册，不绑定底层实现。
- 具体 service 实现由上层 Product Assembly 注入。
- Harness 可扩展，新增 SDD 等工作流不侵入 runtime kernel。
- 每个 crate 只依赖最小稳定集合，依赖方向可检查。

### 1.1 crate 划分

```text
bitfun-core-types
bitfun-events
bitfun-runtime-ports
bitfun-runtime-services      # PR1 基础壳层
bitfun-agent-tools
tool-runtime
bitfun-agent-runtime         # 已创建，当前仅承接 scheduler/background delivery 决策
bitfun-harness               # 已创建，当前承接 workflow descriptor / registry contract
bitfun-services-core
bitfun-services-integrations
bitfun-product-domains
bitfun-acp
bitfun-core
apps/*
```

目标依赖：

```text
apps/*
  -> bitfun-core 或 Product Assembly crate
  -> 按需依赖 bitfun-acp / transport / api-layer

Product Assembly
  -> product capability packs
  -> bitfun-agent-runtime
  -> bitfun-harness
  -> tool-runtime
  -> bitfun-runtime-services
  -> 具体 service crates

Product Capability packs
  -> bitfun-harness
  -> bitfun-agent-runtime
  -> tool-runtime
  -> bitfun-product-domains

bitfun-agent-runtime
  -> bitfun-runtime-ports
  -> bitfun-events
  -> bitfun-agent-stream
  -> tool-runtime
  -> bitfun-runtime-services

tool-runtime
  -> bitfun-agent-tools
  -> bitfun-runtime-ports
  -> bitfun-events

bitfun-runtime-services
  -> bitfun-runtime-ports
  -> bitfun-core-types / bitfun-events（仅当 service DTO 或 event contract 需要时引入）

具体 service crates
  -> bitfun-runtime-ports
  -> bitfun-core-types
  -> 允许的 third-party 依赖
  -> External Systems
```

禁止依赖：

- `bitfun-runtime-ports` -> `bitfun-core`
- `bitfun-agent-tools` -> 具体 service crate
- `tool-runtime` -> 具体 tool 实现 crate
- `bitfun-agent-runtime` -> `bitfun-core`
- `bitfun-agent-runtime` -> Tauri / CLI / ACP protocol / Web UI
- `bitfun-harness` -> 具体 filesystem / Git / terminal manager

目标 crate 创建或继续扩展准入：

- 只有当 owner 边界、旧路径兼容、focused tests、依赖收益和 boundary check 都能同时落地时，才创建新的目标 crate。
- `bitfun-runtime-services` 已按该准入建立基础壳层；继续扩展时仍必须保持 typed builder、本地 service、remote service 和 fake provider 三类注入路径可测试。
- `bitfun-agent-runtime` 已通过 scheduler/background delivery 纯决策满足创建准入；继续扩展时仍必须保持旧路径 facade、focused tests 和 boundary check。
- `bitfun-harness` 已按 Deep Review、DeepResearch、MiniApp 三个 legacy-facade provider
  满足创建准入；继续扩展时仍必须保持 descriptor/registry、旧路径兼容、focused tests 和 boundary check。
- 若目标 crate 只能承接单个 helper 或只能通过 `bitfun-core` 才能测试，继续留在迁移期 facade，不提前拆 crate。

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

当前 crate：`bitfun-runtime-services`。

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

## 3. Runtime / Tool / Harness 内核

### 3.1 Agent Runtime SDK

当前 crate：`bitfun-agent-runtime`。

当前已承接范围：

- background delivery 状态决策：Processing 注入当前运行 turn；Missing / Idle / Error 提交 agent-session follow-up turn。
- persisted thread goal 的 portable DTO、status、continuation plan 和 tool response contract 已在
  `bitfun-runtime-ports`；concrete goal runtime 仍在 core。

仍留在 `bitfun-core` 的范围：

- concrete scheduler 生命周期、session manager、turn id 生成、injection buffer、submit 执行、prompt loop、
  subagent registry、thread goal runtime/store/tool handler 和 post-turn hook。

职责：

- session 生命周期。
- dialog turn / model round 生命周期。
- scheduler / queue / cancellation。
- prompt loop 和 context assembly。
- prompt cache 协调。
- subagent registry 查询和 delegation policy。
- fork context seeding。
- tool call 调度。
- permission 协调。
- runtime events。
- post-turn processor。

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

### 3.2 Tool Runtime

所属 crate：

- `bitfun-agent-tools`
- `tool-runtime`
- `bitfun-tool-packs`

目标职责：

- `bitfun-agent-tools`：tool DTO、manifest、exposure、schema、path policy、result policy。
- `tool-runtime`：provider registry、permission gate、execution pipeline、hook。
- `bitfun-tool-packs`：tool pack feature metadata 和 provider plan。

建议 `tool-runtime` 模块：

```text
tool-runtime
  runtime.rs
  registry.rs
  provider.rs
  context.rs
  permission.rs
  execution.rs
  hooks.rs
  result.rs
  catalog.rs
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

当前已收敛范围：

- deterministic execution admission gate、manifest / catalog / snapshot facade 和 `GetToolSpecTool` product adapter 已有独立 owner。
- workspace file/shell service contract 已归入 `bitfun-runtime-ports`；core 保留旧路径 re-export 和 local / remote concrete adapter。
  迁移期该 contract 保留既有 `anyhow::Result` 和 `CancellationToken`，不把错误分类或取消语义变更混入 owner 迁移。
- collapsed unlock 的 `GetToolSpec` observation adapter 已归入 product runtime owner，execution engine 只消费已归一化的 unlock list。

设计约束：

- `ToolExecutionContext` 不暴露具体 manager。
- `ToolContextFacts` 只包含 portable facts。
- Tool Runtime 只消费 `ToolExecutionServices` 这样的窄 service 视图，不依赖完整
  `RuntimeServices` bundle。
- path policy、runtime artifact ref、remote POSIX containment 由 `bitfun-agent-tools` 承载。
- MCP tool 作为 external tool provider 注入，不内置在 Agent Runtime SDK。
- `GetToolSpec` 是 tool runtime/catalog 能力，不是产品 UI。

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

当前 crate：`bitfun-harness`。

职责：

- 把 SDD、DeepReview、DeepResearch、MiniApp、function-agent 等工作流从 runtime kernel
  中分离。当前只承接 workflow descriptor、route plan 和 provider registry。
- 定义 workflow plan、step、policy、artifact、review gate、post-processor。具体执行在迁移前继续留在旧路径。
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

- harness 可以编排 runtime/tool，但不拥有 session manager internals。
- harness 不直接访问 concrete filesystem / Git / terminal。
- 产品命令只映射到 harness capability，不把命令展示逻辑下沉。
- 新 harness 通过 provider 注册，不改 Agent Runtime SDK 内核。
- PR4 阶段的 legacy-facade provider 只生成 route plan，不执行 workflow；执行迁移必须在后续 PR 单独证明行为等价。

## 4. 产品组装与扩展

### 4.1 Product Assembly

Product Assembly 是 composition root。它可以位于 `bitfun-core` 迁移期 facade 内，也可以在
后续拆成独立 Product Assembly crate。

职责：

- 创建具体 service 实现。
- 构建 `RuntimeServices`。
- 注册 tool providers。
- 注册 harness providers。
- 注册 agent definitions、subagents、skills、prompt modules。
- 建立产品 feature matrix。
- 把 surface 命令映射到 capability / harness / runtime request。
- 根据交付形态选择 `DeliveryProfile`、`CapabilitySet` 和 service provider 集合。
- 对不支持能力返回 typed unsupported / unavailable 错误，而不是让下层 runtime 判断产品形态。

建议模块：

```text
product-assembly
  full.rs
  delivery_profile.rs
  capability_set.rs
  desktop.rs
  cli.rs
  server.rs
  remote.rs
  acp.rs
  feature_matrix.rs
  commands.rs
```

核心结构：

```rust
pub enum DeliveryProfile {
    Desktop,
    Cli,
    Server,
    Remote,
    Acp,
    Web,
}

pub struct CapabilitySet {
    pub agent_modes: Vec<AgentModeId>,
    pub tool_packs: Vec<ToolPackId>,
    pub harness_packs: Vec<HarnessId>,
    pub service_capabilities: Vec<ServiceCapabilityId>,
    pub command_providers: Vec<CommandProviderId>,
}

pub struct ProductAssemblyPlan {
    pub profile: DeliveryProfile,
    pub capabilities: CapabilitySet,
    pub feature_groups: Vec<FeatureGroupId>,
}

pub trait ProductAssembler {
    fn plan(&self, profile: DeliveryProfile) -> Result<ProductAssemblyPlan, AssemblyError>;
    fn build(&self, plan: ProductAssemblyPlan) -> Result<ProductRuntime, AssemblyError>;
}
```

实现注册方式：

```rust
pub struct ProductAssemblyInput {
    pub profile: DeliveryProfile,
    pub services: ConcreteServiceProviders,
    pub tool_providers: Vec<Arc<dyn ToolProvider>>,
    pub harness_providers: Vec<Arc<dyn HarnessProvider>>,
    pub agents: Arc<dyn AgentDefinitionRegistry>,
    pub commands: Vec<CommandProviderRef>,
    pub hooks: RuntimeHookRegistry,
}

pub struct ProductRuntimeParts {
    pub services: RuntimeServices,
    pub tools: Arc<ToolRuntime>,
    pub harnesses: Arc<HarnessRegistry>,
    pub agents: Arc<dyn AgentDefinitionRegistry>,
    pub commands: ProductCommandRegistry,
    pub hooks: RuntimeHookRegistry,
}
```

注册路径：

- concrete service provider 只注册到 `RuntimeServicesBuilder`。
- tool provider 只注册到 `ToolRuntimeBuilder::install_provider`。
- harness provider 只注册到 `HarnessRegistryBuilder`。
- agent、subagent、prompt、skill 只注册到 `AgentDefinitionRegistry` 或对应 registry。
- 输入框命令、审核入口、MiniApp 入口只注册到 `ProductCommandRegistry`，再映射到 capability 或 harness。
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

    let runtime = AgentRuntime::new(AgentRuntimeParts {
        services,
        tools,
        agents: input.agents,
        hooks: input.runtime_hooks,
        config: input.config,
    })?;

    Ok(ProductRuntime { runtime })
}
```

约束：

- Product Assembly 可以依赖具体实现；runtime kernel 不可以。
- 不同产品可以注册不同 surface command，但必须映射到稳定 capability。
- 输入框命令、审核、MiniApp、ACP client、自定义 tool/subagent/skill 均通过 assembly 注册。
- assembly 不得改变底层 runtime 语义来适配某个 surface。
- `DeliveryProfile` 只能影响 capability/provider 选择，不得让下层出现 `if desktop`
  或 `if cli` 这样的 product 分支。
- feature group 是构建时能力边界，`CapabilitySet` 是产品运行时能力边界；两者必须在
  assembly 中显式对应。
- 任何交付形态减少能力前，必须先更新 product matrix 并补产品入口验证。

### 4.2 产品形态与组装差异

| 产品形态 | 关键差异 | 组装时必须稳定的下层契约 |
|---|---|---|
| Desktop | Tauri window、desktop API、本地 permission UI | runtime events、permission facts、artifact refs、desktop service providers |
| CLI | TUI、命令输入、终端展示、package workflow | command provider、agent/session/tool contract、CLI-safe service providers |
| Server | HTTP/WebSocket route、server workspace policy | transport DTO、runtime request/response、workspace identity |
| Remote / mobile | remote workspace、relay/bot、file/terminal projection | remote state、logical path、permission/event facts |
| ACP | ACP protocol、client lifecycle、remote probing | external agent/tool capability、environment facts |
| Web UI / mobile web | UI state、hydration、pairing、session 展示 | API/transport DTO、runtime event facts |

### 4.3 Product Capability 设计

Product Capability 位于 Product Assembly 与 Harness / Runtime / Tool 之间，负责把大块产品能力
拆成可组装的 capability pack。它不拥有 UI，也不直接执行具体 IO。

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
}
```

分层规则：

- Code Agent pack 可以声明 agent modes、tool packs、prompt modules，但不拥有 tool execution。
- Deep Review pack 可以声明 harness provider、report artifact contract、queue/retry policy，
  但 target resolution 和 UI construction 留在 surface。
- MiniApp pack 可以声明 MiniApp harness、domain ports、artifact policy，但 worker process 和
  filesystem IO 通过 Runtime Services provider。
- MCP App pack 可以声明 MCP tool/resource/prompt capability，但 MCP transport 属于
  `bitfun-services-integrations`。
- Input command pack 只声明 command 到 capability/harness/runtime request 的映射，不共享具体 UI。

### 4.4 ACP 扩展方式

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

### 4.5 Skills / Prompt / Subagent

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

### 4.6 Hook 与 Event 设计

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

## 5. 质量保护与完成定义

### 5.1 鲁棒性设计

错误：

- contract 层使用 portable error facts。
- runtime 层做错误分类和事件上报。
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

### 5.2 计划边界

本文只描述目标接口、crate 内部结构和行为保护要求，不定义 PR 顺序。Runtime Services、
Tool Runtime、Agent Runtime SDK、Harness Layer 和 Product Assembly 的阶段边界、PR 范围和执行安排
由 [`core-decomposition-plan.md`](../plans/core-decomposition-plan.md) 维护。

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

### 5.4 完成定义

- `bitfun-agent-runtime` 能在不依赖 `bitfun-core` 的情况下构建 runtime kernel。
- `bitfun-runtime-services` 提供 typed service injection，并由 boundary check 保护。
- `tool-runtime` 承担 provider registry 和 execution pipeline，具体 tool 通过 provider 注入。
- `bitfun-harness` 支持工作流 provider 扩展。
- `bitfun-core` 只作为迁移期 facade / product-full assembly。
- 所有产品形态通过 Product Assembly 显式启用能力。
- 所有高风险行为有 snapshot、focused regression 或 product check 保护。
