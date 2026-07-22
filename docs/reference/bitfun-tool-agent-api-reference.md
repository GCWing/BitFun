# BitFun 工具与 Agent 基础设施 API 参考

> 为迁移 taiji 的 `ComputeNode` / `NodeFactory` 模式而编制。
> 覆盖 `tool-contracts`、`tool-provider-groups`、`tool-execution`、`plugin-runtime-host`、`harness` 五个 crate。

---

## 目录

1. [如何定义工具](#1-如何定义工具)
   - [1.1 `ToolRegistryItem` — 工具的基础 trait](#11-toolregistryitem--工具的基础-trait)
   - [1.2 `ToolManifestDefinition` — 打包工具清单](#12-toolmanifestdefinition--打包工具清单)
   - [1.3 `ContextualToolManifestItem<Context>` — 上下文感知变体](#13-contextualtoolmanifestitemcontext--上下文感知变体)
   - [1.4 `ToolExposure` — 工具可见性策略](#14-toolexposure--工具可见性策略)
   - [1.5 `ToolResult` — 工具执行结果](#15-toolresult--工具执行结果)
   - [1.6 `ValidationResult` + `InputValidator` — 输入校验](#16-validationresult--inputvalidator--输入校验)
   - [1.7 `PermissionIntent` — 权限意图](#17-permissionintent--权限意图)
2. [如何注册工具](#2-如何注册工具)
   - [2.1 `ToolRegistry<Tool>` — 运行时注册表](#21-toolregistrytool--运行时注册表)
   - [2.2 `StaticToolProvider` / `StaticToolProviderGroup` — 静态提供者](#22-statictoolprovider--statictoolprovidergroup--静态提供者)
   - [2.3 `StaticToolProviderFactory` — 延迟实例化](#23-statictoolproviderfactory--延迟实例化)
   - [2.4 `ToolRuntimeAssembly` — 注册表构建器](#24-toolruntimeassembly--注册表构建器)
   - [2.5 `ToolProviderGroupPlan` + `ToolPackFeatureGroup` — 声明式分组](#25-toolprovidergroupplan--toolpackfeaturegroup--声明式分组)
3. [如何执行工具](#3-如何执行工具)
   - [3.1 执行准入门 — `ToolExecutionAdmissionRequest`](#31-执行准入门--toolexecutionadmissionrequest)
   - [3.2 `ToolRuntimeRestrictions` — 运行时限制](#32-toolruntimerestrictions--运行时限制)
   - [3.3 `ToolContextFacts` — 运行时上下文](#33-toolcontextfacts--运行时上下文)
   - [3.4 `ResolvedToolInvocation` — 延迟工具解析](#34-resolvedtoolinvocation--延迟工具解析)
   - [3.5 延迟工具协议 — `GetToolSpec` → `CallDeferredTool`](#35-延迟工具协议--gettoolspec--calldeferredtool)
   - [3.6 管线规划 — `pipeline.rs`](#36-管线规划--pipeliners)
   - [3.7 错误展示 — `tool_execution_presentation`](#37-错误展示--tool_execution_presentation)
   - [3.8 工具快照 — `ToolSnapshotItem` / `MaterializedToolSnapshot`](#38-工具快照--toolsnapshotitem--materializedtoolsnapshot)
4. [Plugin Runtime Host](#4-plugin-runtime-host)
   - [4.1 `PluginRuntimeHost`](#41-pluginruntimehost)
   - [4.2 `PluginHostAdapter` trait](#42-pluginthostadapter-trait)
5. [Execution Harness](#5-execution-harness)
   - [5.1 `HarnessWorkflow` / `HarnessCapability`](#51-harnessworkflow--harnesscapability)
   - [5.2 `HarnessProvider` trait](#52-harnessprovider-trait)
   - [5.3 `HarnessRegistry`](#53-harnessregistry)
6. [附录：完整类型索引](#6-附录完整类型索引)

---

## 1. 如何定义工具

### 1.1 `ToolRegistryItem` — 工具的基础 trait

位置：`src/crates/execution/tool-contracts/src/framework.rs` (lines 671-719)

每个工具至少实现此 trait 的 9 个必需方法：

```rust
pub trait ToolRegistryItem: Send + Sync + 'static {
    /// 唯一工具名（蛇形命名），如 "read"、"exec_command"、"task"
    fn name(&self) -> String;

    /// 面向模型的功能描述文本
    fn description(&self) -> String;

    /// JSON Schema 输入定义
    fn input_schema(&self) -> serde_json::Value;

    /// 简短的面向 UI 的描述
    fn short_description(&self) -> String;

    /// 默认可见性策略
    fn default_exposure(&self) -> ToolExposure;

    /// 是否为只读操作（无副作用的工具可以并发执行）
    fn is_readonly(&self) -> bool;

    /// 是否可以与其他工具调用并发执行
    fn is_concurrency_safe(&self) -> bool;

    /// 工具是否自行管理超时（如果返回 true，则不应施加外部超时）
    fn manages_own_execution_timeout(&self) -> bool;

    /// 运行时是否启用。如果返回 false，则工具在注册表中可被禁用
    fn is_enabled(&self) -> bool;
}
```

可选方法（带默认实现）：

```rust
    /// 模型感知版本的 input_schema，可以接收模型信息进行调整
    fn input_schema_for_model(&self, _primary_model_facts: &PrimaryModelFacts) -> serde_json::Value { ... }

    /// 若工具属于动态提供者，返回提供者 ID
    fn dynamic_provider_id(&self) -> Option<String> { None }

    /// 动态工具附加信息
    fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> { None }
}
```

**taiji 迁移对照**：taiji 的 `ComputeNode` 相当于 `ToolRegistryItem`。每个 taiji 节点应实现这组方法，用 `name()` 返回节点标识（如 `"screening"`、`"backtest"`），`input_schema()` 返回 JSON Schema 描述参数。

---

### 1.2 `ToolManifestDefinition` — 打包工具清单

提供者无关的工具包装结构：

```rust
pub struct ToolManifestDefinition {
    pub registry_item: Box<dyn ToolRegistryItem>,
}
```

构造方式：

```rust
impl ToolManifestDefinition {
    pub fn new(item: impl ToolRegistryItem) -> Self { ... }
}
```

一个具体的工具实现 `ToolRegistryItem` 后，通过 `ToolManifestDefinition::new(my_tool)` 即可得到标准包装。

---

### 1.3 `ContextualToolManifestItem<Context>` — 上下文感知变体

位置：`framework.rs` (lines 721-761)

当工具需要根据运行时上下文（会话、工作区类型等）动态调整行为时使用。它在 `ToolRegistryItem` 之上增加 3 个方法：

```rust
pub trait ContextualToolManifestItem<Context>: ToolRegistryItem {
    /// 在当前上下文中是否可用
    fn is_available_in_context(&self, context: &Context) -> bool;

    /// 带上下文信息的描述（可注入工作区路径等）
    fn description_with_context(&self, context: &Context) -> String;

    /// 带上下文的输入 schema
    fn input_schema_for_model_with_context(
        &self, context: &Context, model_facts: &PrimaryModelFacts
    ) -> serde_json::Value;
}
```

**taiji 迁移对照**：taiji 的市场上下文（合约参数、数据源）通过 `Context` 泛型传递，`is_available_in_context` 决定该节点在当前品种/周期下是否可用。

---

### 1.4 `ToolExposure` — 工具可见性策略

```rust
pub enum ToolExposure {
    /// 直接暴露给模型，在所有工具列表中可见
    Direct,
    /// 仅通过 GetToolSpec → CallDeferredTool 间接暴露（延迟加载）
    Deferred,
}
```

- **Direct**：模型始终知道此工具存在，适合高频调用工具（Read、Write、ExecCommand）
- **Deferred**：模型需先调用 `GetToolSpec` 获取工具的 schema 后才能调用。适合低频/高 cost 工具（Canvas、MiniApp、AgentControl 类）

---

### 1.5 `ToolResult` — 工具执行结果

```rust
pub enum ToolResult {
    Result(serde_json::Value),
    Progress {
        progress_token: String,
        progress: f64,
        total: Option<f64>,
    },
    StreamChunk {
        id: String,
        chunk: Vec<u8>,
    },
}
```

构造器：
- `ToolResult::ok(value)` — 成功结果
- `ToolResult::ok_with_images(value, data_urls)` — 带图片的成功结果

---

### 1.6 `ValidationResult` + `InputValidator` — 输入校验

```rust
pub struct ValidationResult {
    pub result: bool,
    pub message: Option<String>,
    pub error_code: Option<i32>,
    pub meta: Option<serde_json::Value>,
}
```

Builder 模式校验器：

```rust
let validation = InputValidator::new(&input)
    .validate_required("symbol")           // 必填字符串字段
    .validate_required_enum("direction", &["long", "short"])  // 枚举字段
    .finish();

if !validation.result {
    return Err(validation.message.unwrap_or_default());
}
```

链式调用，短路失败：一旦 `result == false`，后续 `validate_*` 调用自动跳过。

---

### 1.7 `PermissionIntent` — 权限意图

位置：`src/crates/execution/tool-contracts/src/permission_intent.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionIntent {
    pub action: String,                          // "write_to_path" | "read_from_path" | "shell_execute"
    pub resources: Vec<String>,                  // 受影响的资源列表
    pub save_resources: Vec<String>,             // 写操作的目标路径
    pub display_metadata: Map<String, Value>,    // 展示用的元数据
}
```

工具在执行前生成 `PermissionIntent`，用户/策略系统据此决定允许还是拒绝。

---

## 2. 如何注册工具

### 2.1 `ToolRegistry<Tool>` — 运行时注册表

位置：`framework.rs` (lines 1420-1604)

核心索引结构，管理所有已注册工具：

```rust
pub struct ToolRegistry<Tool: ToolRegistryItem> { ... }
```

关键方法：

```rust
impl<Tool: ToolRegistryItem> ToolRegistry<Tool> {
    pub fn new() -> Self;

    /// 注册单个工具
    pub fn register_tool(&mut self, tool: Tool) -> Result<(), String>;

    /// 安装一个静态工具提供者（批量注册）
    pub fn install_static_provider(
        &mut self,
        provider: impl StaticToolProvider<Tool>,
    ) -> Result<(), String>;

    /// 卸载工具
    pub fn unregister_tool(&mut self, tool_name: &str) -> bool;

    /// 查询工具
    pub fn get_tool(&self, name: &str) -> Option<&Tool>;

    /// 获取所有工具名（按注册顺序）
    pub fn get_tool_names(&self) -> Vec<String>;

    /// 工具是否为延迟暴露
    pub fn is_tool_deferred(&self, name: &str) -> bool;

    /// 生成当前快照（包含所有 Direct 工具）
    pub fn materialized_tool_snapshot(&self) -> MaterializedToolSnapshot;

    /// 快照版本号（每次注册/卸载后递增）
    pub fn current_snapshot_generation(&self) -> u64;
}
```

---

### 2.2 `StaticToolProvider` / `StaticToolProviderGroup` — 静态提供者

位置：`framework.rs` (lines 1250-1318)

```rust
/// 工具提供者接口：标识自身 + 提供工具列表
pub trait StaticToolProvider<Tool: ToolRegistryItem>: Send + Sync + 'static {
    fn provider_id(&self) -> ToolProviderIdentity;
    fn tools(&self) -> Vec<Tool>;
}
```

```rust
/// 具体实现：Arc 包装的静态工具组
pub struct StaticToolProviderGroup<Tool: ToolRegistryItem> { ... }

impl<Tool: ToolRegistryItem> StaticToolProviderGroup<Tool> {
    pub fn new(
        identity: ToolProviderIdentity,
        tools: Vec<Tool>,
    ) -> Self { ... }
}
```

**典型用法**：

```rust
let group = StaticToolProviderGroup::new(
    ToolProviderIdentity::builtin("core.basic"),
    vec![
        MyReadTool.into_manifest(),
        MyWriteTool.into_manifest(),
    ],
);
```

---

### 2.3 `StaticToolProviderFactory` — 延迟实例化

位置：`framework.rs` (lines 1262-1264)

当工具列表已知（通过 `StaticToolProviderPlan` 声明了 `tool_names()`）但工具对象需要懒加载时：

```rust
pub trait StaticToolProviderFactory<Tool: ToolRegistryItem>: Send + Sync + 'static {
    fn materialize_tool(&self, tool_name: &str) -> Option<Tool>;
}
```

与 `StaticToolProviderPlan` 配合使用：

```rust
pub trait StaticToolProviderPlan: Send + Sync + 'static {
    fn provider_id(&self) -> ToolProviderIdentity;
    fn tool_names(&self) -> Vec<String>;
}
```

---

### 2.4 `ToolRuntimeAssembly` — 注册表构建器

位置：`framework.rs` (lines 1347-1418)

将所有提供者组装成 `ToolRegistry`：

```rust
pub struct ToolRuntimeAssembly<Tool: ToolRegistryItem> { ... }

impl<Tool: ToolRegistryItem> ToolRuntimeAssembly<Tool> {
    /// 从已实例化的静态提供者创建注册表
    pub fn create_registry_from_static_providers(
        providers: Vec<impl StaticToolProvider<Tool>>,
    ) -> Result<ToolRegistry<Tool>, String>;

    /// 从计划+工厂创建注册表（延迟实例化路径）
    pub fn create_registry_from_static_provider_entries(
        entries: Vec<(impl StaticToolProviderPlan, impl StaticToolProviderFactory<Tool>)>,
    ) -> Result<ToolRegistry<Tool>, String>;
}
```

**taiji 迁移对照**：taiji 的 `NodeFactory` 对应 `StaticToolProviderFactory`。taiji 定义一组节点的 `tool_names()`（如 `["screening", "backtest", "optimize"]`），然后通过工厂方法为每个节点 `materialize_tool()`。

---

### 2.5 `ToolProviderGroupPlan` + `ToolPackFeatureGroup` — 声明式分组

位置：`src/crates/execution/tool-provider-groups/src/lib.rs`

**`ToolPackFeatureGroup`** — 编译时特性门控分类（9 种）：

```rust
pub enum ToolPackFeatureGroup {
    Basic,          // 14 工具：Read, Write, Edit, Delete, Glob, Grep, LS, ExecCommand, ...
    Git,            // Git 相关
    Mcp,            // MCP 协议工具
    BrowserWeb,     // 浏览器 & 网页工具
    ComputerUse,    // 桌面自动化
    ImageAnalysis,  // 图片分析
    MiniApp,        // 小应用运行时
    Canvas,         // Canvas 画布
    AgentControl,   // Agent 控制（Task、Skill、AskUserQuestion 等）
}
```

**`ToolProviderGroupPlan`** — 声明式提供者→工具→特性组的映射：

```rust
pub struct ToolProviderGroupPlan {
    pub provider_id: String,
    pub tool_names: Vec<String>,           // 该提供者拥有的工具名列表
    pub feature_groups: Vec<ToolPackFeatureGroup>,
}
```

**`PRODUCT_TOOL_PROVIDER_GROUP_PLAN`** — 静态常量，定义了 BitFun 全部 51 个内置工具的分组：

| 提供者 | 工具数 | 示例 |
|--------|--------|------|
| `core.basic` | 14 | Read, Write, Edit, Delete, Glob, Grep, LS, ExecCommand, ... |
| `core.agent` | 14 | Task, Skill, TodoWrite, AskUserQuestion, AgentWait, ... |
| `core.canvas` | 4 | CreateCanvas, ReadCanvas, UpdateCanvas, PatchCanvas |
| `core.session` | 4 | SessionControl, SessionMessage, SessionHistory, Cron |
| `core.integration` | 15 | MCP 工具、浏览器、计算机使用、Git、图片分析等 |

选择子集：

```rust
let subset = try_product_tool_provider_group_plan_for_ids(
    &["core.basic", "core.agent"]
)?;
```

---

## 3. 如何执行工具

### 3.1 执行准入门 — `ToolExecutionAdmissionRequest`

位置：`src/crates/execution/tool-contracts/src/execution_gate.rs`

在执行工具前，必须通过的三层校验：

```rust
pub struct ToolExecutionAdmissionRequest<'a> {
    pub tool_name: &'a str,
    pub allowed_tools: &'a [String],
    pub runtime_tool_restrictions: &'a ToolRuntimeRestrictions,
    pub invocation_is_deferred: bool,
    pub deferred_tools: &'a [String],
    pub loaded_deferred_tool_specs: &'a [LoadedDeferredToolSpec],
    pub current_catalog_generation: u64,
    pub get_tool_spec_tool_name: &'a str,
}
```

```rust
pub fn validate_tool_execution_admission(
    request: ToolExecutionAdmissionRequest<'_>,
) -> Result<(), ToolExecutionAdmissionRejection>;
```

三层依次检查：
1. **AllowedList**：工具名是否在模型允许的工具列表中
2. **RuntimeRestriction**：运行时限制策略是否放行
3. **Deferred**：延迟工具使用是否合法（schema 已加载、代际匹配）

---

### 3.2 `ToolRuntimeRestrictions` — 运行时限制

位置：`framework.rs` (lines 2218-2288)

```rust
pub struct ToolRuntimeRestrictions {
    pub allowed_tool_names: BTreeSet<String>,
    pub denied_tool_names: BTreeSet<String>,
    pub denied_tool_messages: HashMap<String, String>,  // 按工具名定制拒绝消息
    pub path_policy: ToolPathPolicy,
}
```

关键方法：

```rust
impl ToolRuntimeRestrictions {
    /// 检查工具是否被允许
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool;

    /// 检查并返回详细拒绝原因
    pub fn ensure_tool_allowed(&self, tool_name: &str) -> Result<(), ToolRestrictionError>;
}
```

**路径策略** (`ToolPathPolicy`)：

```rust
pub struct ToolPathPolicy {
    pub workspace_only: bool,  // 限制在 workspace 内
    pub deny_absolute: bool,   // 拒绝绝对路径
}
```

路径解析函数：

```rust
pub fn resolve_tool_path_with_context(
    raw: &str,
    workspace_root: &str,
    policy: &ToolPathPolicy,
) -> Result<ToolPathResolution, String>;
```

返回 `ToolPathResolution`，其中包含规范化后的绝对路径和对应的 `bitfun://` URI。

---

### 3.3 `ToolContextFacts` — 运行时上下文

位置：`framework.rs` + `tool-execution/src/context.rs`

传递给工具的标准化上下文快照：

```rust
pub struct ToolContextFacts {
    pub tool_call_id: Option<String>,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub dialog_turn_id: Option<String>,
    pub workspace_kind: Option<ToolWorkspaceKind>,  // Local | Remote
    pub workspace_root: Option<String>,
    pub runtime_tool_restrictions: ToolRuntimeRestrictions,
}
```

构建函数：

```rust
// 在 tool-execution/src/context.rs
pub fn project_tool_context_facts(input: ToolRuntimeContextFactsInput) -> ToolContextFacts;
pub fn build_tool_runtime_custom_data(input: ToolRuntimeCustomDataInput<'_>) -> HashMap<String, Value>;
pub fn delegation_policy_from_custom_data(custom_data: &HashMap<String, Value>) -> DelegationPolicy;
```

**`PrimaryModelFacts`** — 主模型信息（也在这里定义）：

```rust
pub struct PrimaryModelFacts {
    pub model_id: String,
    pub model_name: String,
    pub api_format: String,                    // "anthropic" | "openai" | "response"
    pub supports_image_inputs: bool,
}
```

---

### 3.4 `ResolvedToolInvocation` — 延迟工具解析

位置：`src/crates/execution/tool-contracts/src/deferred_tool.rs`

核心问题：模型发来的工具调用可能是 `CallDeferredTool`（网关工具），需要解析为目标工具。

```rust
pub struct ResolvedToolInvocation {
    pub effective_tool_name: String,   // 解析后的实际工具名
    pub effective_tool_args: String,   // 解析后的实际参数
}
```

```rust
pub enum ToolInvocationKind {
    Direct,     // 直接调用（Read、Write 等）
    Deferred,   // 通过 CallDeferredTool 的间接调用
}
```

```rust
impl ResolvedToolInvocation {
    /// 从模型发来的工具调用中解析。如果是 CallDeferredTool，则提取内部参数
    pub fn from_wire_call(
        wire_tool_name: &str,
        wire_tool_args: &str,
    ) -> Self { ... }

    /// 底层工具名
    pub fn effective_tool_invocation(&self) -> (&str, &str) { ... }
}
```

---

### 3.5 延迟工具协议 — `GetToolSpec` → `CallDeferredTool`

标准两阶段协议：

**第一阶段**：模型调用 `GetToolSpec` 获取工具 schema

```
输入: { "tool_name": "CreateCanvas" }
输出: 工具名、描述、JSON Schema 等完整清单
```

**第二阶段**：模型使用获得的信息调用 `CallDeferredTool`

```
输入: { "tool_name": "CreateCanvas", "args": { "title": "...", "source": "..." } }
```

`CallDeferredTool` 的常量定义：

```rust
pub const CALL_DEFERRED_TOOL_NAME: &str = "CallDeferredTool";
```

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CallDeferredToolInput {
    pub tool_name: String,
    pub args: serde_json::Value,
}

pub fn parse_call_deferred_tool_input(args: &str) -> Result<CallDeferredToolInput, String>;
```

**taiji 迁移对照**：taiji 的高 cost 节点（如完整回测流程、参数优化）可以设为 Deferred，减少 prompt 污染。

---

### 3.6 管线规划 — `pipeline.rs`

位置：`src/crates/execution/tool-execution/src/pipeline.rs`

**`ToolBatch`** — 执行批次：

```rust
pub struct ToolBatch {
    pub task_ids: Vec<String>,
    pub is_concurrent: bool,  // 批次内任务是否可并发
}
```

**`ToolTaskStateKind`** — 任务状态机（8 状态）：

```
Queued → Waiting → Running → Streaming → Completed
                                  ↓ Failed
                                  ↓ Rejected
                                  ↓ Cancelled
```

```rust
pub enum ToolTaskStateKind {
    Queued,     // 已排队
    Waiting,    // 等待依赖
    Running,    // 执行中
    Streaming,  // 流式输出中
    Completed,  // 成功
    Failed,     // 失败
    Rejected,   // 被拒绝
    Cancelled,  // 被取消
}
```

关键函数：

```rust
/// 将任务列表按并发安全性分区
pub fn partition_tool_batches(task_ids: &[String], flags: &[bool]) -> Vec<ToolBatch>;

/// SubAgent 并发策略特殊处理
pub fn tool_call_concurrency_safe_for_batch(
    tool_name: &str,
    tool_is_concurrency_safe: bool,
    same_batch_subagent_call_count: usize,
    subagent_batch_execution_policy: SubagentBatchExecutionPolicy,
) -> bool;

/// 重试判断
pub fn should_retry_tool_attempt(facts: ToolRetryAttemptFacts) -> bool;
pub fn retry_delay_ms(attempts: usize) -> u64;
```

**`ToolCancellationTokenStore`** — 取消令牌存储：

```rust
pub struct ToolCancellationTokenStore { ... }

impl ToolCancellationTokenStore {
    pub fn insert(&self, tool_id: String, token: CancellationToken);
    pub fn cancel(&self, tool_id: &str) -> bool;  // 取消并移除
    pub fn has_pending(&self, tool_id: &str) -> bool;
}
```

**`SubagentBatchExecutionPolicy`**：

```rust
pub enum SubagentBatchExecutionPolicy {
    SafeOnly,       // 仅并发的 SubAgent 可并行
    ForceParallel,  // 多个 SubAgent 调用强制并行（默认）
    Serial,         // 强制串行
}
```

**`ToolStateEventKind`** — 每个状态转换生成对应事件，包含详细计时信息（`duration_ms`、`queue_wait_ms`、`preflight_ms`、`confirmation_wait_ms`、`execution_ms`）。

---

### 3.7 错误展示 — `tool_execution_presentation`

位置：`src/crates/execution/tool-contracts/src/tool_execution_presentation.rs`

标准化的工具执行结果展示函数（全部返回 `ToolExecutionErrorPresentation`）：

```rust
pub struct ToolExecutionErrorPresentation {
    pub result_json: Value,          // 给前端/系统的结构化结果
    pub result_for_assistant: String,  // 给模型的中文错误消息
}
```

| 场景 | 函数 |
|------|------|
| 通用执行错误 | `build_tool_execution_error_presentation(tool_name, category, error_message, args)` |
| 用户中断（新消息） | `build_user_steering_interrupted_presentation(tool_name)` |
| 执行超时 | `build_tool_execution_timeout_presentation(tool_name, timeout_secs)` |
| 用户拒绝 | `build_user_rejected_tool_presentation(tool_name)` |
| 用户拒绝（带指令） | `build_user_rejected_tool_presentation_with_instruction(tool_name, instruction)` |
| 权限拒绝 | `build_permission_denied_tool_presentation(tool_name, reason)` |
| 非法工具调用 | `build_invalid_tool_call_error_message(tool_name, is_error, recovered_from_truncation, args)` |
| 截断恢复通知 | `build_tool_call_truncation_recovery_notice(tool_name)` |

---

### 3.8 工具快照 — `ToolSnapshotItem` / `MaterializedToolSnapshot`

位置：`src/crates/execution/tool-contracts/src/tool_snapshot.rs`

用于将当前注册表序列化为模型可见的工具列表：

```rust
pub struct ToolSnapshotItem {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub exposure: ToolExposure,
}

pub struct MaterializedToolSnapshot {
    pub generation: u64,
    pub tools: Vec<ToolSnapshotItem>,
}
```

```rust
/// 异步生成快照（可带模型信息以调整 schema）
pub async fn materialize_tool_snapshot(
    tools: Vec<ToolSnapshotItem>,
    model_facts: Option<&PrimaryModelFacts>,
) -> MaterializedToolSnapshot;
```

**`ToolProviderIdentity`** — 工具来源标识：

```rust
pub enum ToolProviderIdentity {
    Builtin(String),                    // "Builtin.core.basic"
    Static(String, String),            // "Static.{provider_id}.{tool_name}"
    Dynamic(String, String),           // "Dynamic.{provider_id}.{tool_name}"
}
```

---

## 4. Plugin Runtime Host

位置：`src/crates/execution/plugin-runtime-host/src/lib.rs`

### 4.1 `PluginRuntimeHost`

插件运行时生命周期管理器。核心设计原则：
- **幂等分发**：相同请求产生相同结果，通过 `DispatchCacheKey`（17 字段含各 epoch 版本号）实现
- **隔离失败**：适配器回调失败 → 隔离该插件（quarantine），不影响其他插件
- **LRU 缓存**：最多缓存 256 条分发结果

```rust
pub struct PluginRuntimeHost { ... }

impl PluginRuntimeHost {
    pub fn new(adapter: Box<dyn PluginHostAdapter>) -> Self;

    /// 按项目粒度清理（项目关闭时调用）
    pub fn dispose_project(&self);

    /// 重启宿主（清空缓存和隔离列表）
    pub fn restart(&self);
}
```

作为 `PluginRuntimeClient` 暴露两个核心操作：

```rust
/// 读取所有插件的元数据
fn read_plugins(&self) -> Vec<PluginManifest>;

/// 向指定插件发起分发请求（幂等）
fn dispatch(&self, plugin_id: &str, request: PluginDispatchRequest) -> PluginDispatchResult;
```

**隔离机制**：
- 适配器返回 Err → 隔离
- 分发超时 → 隔离
- 响应信封无效 → 隔离
- 隔离标识通过 `fnv1a64` 哈希生成

---

### 4.2 `PluginHostAdapter` trait

位置：`src/crates/execution/plugin-runtime-host/src/adapter.rs`

```rust
pub trait PluginHostAdapter: Send + Sync + 'static {
    fn adapter_id(&self) -> &'static str;

    /// 从后端读取所有可用插件
    fn read_plugins(&self) -> Vec<PluginManifest>;

    /// 向指定插件发送请求并等待响应
    fn dispatch(
        &self,
        plugin_id: &str,
        request: PluginDispatchRequest,
    ) -> Result<PluginDispatchResult, PluginHostError>;
}
```

**taiji 迁移对照**：taiji 的数据源适配器可以实现 `PluginHostAdapter`。`read_plugins()` 返回可用的数据节点列表，`dispatch()` 执行具体的数据拉取/计算请求。

---

## 5. Execution Harness

位置：`src/crates/execution/harness/src/lib.rs`

Harness 是工作流编排层，负责将高层声明式意图（SDD、Code Review、Deep Research 等）映射到具体的工具执行计划。

### 5.1 `HarnessWorkflow` / `HarnessCapability`

```rust
pub enum HarnessWorkflow {
    Sdd,           // 规格驱动开发
    DeepReview,    // 深度代码审查
    DeepResearch,  // 深度研究
    MiniApp,       // 小应用生成
    FunctionAgent, // 函数式 Agent
}
```

```rust
pub enum HarnessCapability {
    Plan,           // 生成执行计划
    Execute,        // 执行计划
    ReviewGate,     // 审查门控
    Artifact,       // 产出工件
    PostProcessor,  // 后处理
}
```

---

### 5.2 `HarnessProvider` trait

每个 Harness 工作流由一个 `HarnessProvider` 实现：

```rust
pub trait HarnessProvider: Send + Sync + 'static {
    fn provider_id(&self) -> &str;
    fn workflow(&self) -> HarnessWorkflow;
    fn capabilities(&self) -> Vec<HarnessCapability>;

    /// 生成执行计划
    fn plan(&self, context: &HarnessContext) -> Result<HarnessPlan, HarnessError>;

    /// 执行计划步骤
    fn execute(&self, plan: &HarnessPlan, context: &HarnessContext) -> Result<HarnessResult, HarnessError>;
}
```

```rust
pub struct HarnessPlan {
    pub steps: Vec<HarnessStep>,
}

pub struct HarnessStep {
    pub id: String,
    pub description: String,
    pub tool_calls: Vec<ToolCallPlan>,   // 这一步需要的工具调用
}
```

---

### 5.3 `HarnessRegistry`

```rust
pub struct HarnessRegistry { ... }

impl HarnessRegistry {
    pub fn new() -> Self;

    pub fn register(&mut self, provider: Box<dyn HarnessProvider>);

    /// 按工作流类型查找提供者
    pub fn provider_for_workflow(&self, workflow: HarnessWorkflow) -> Option<&dyn HarnessProvider>;

    /// 按 ID 查找提供者
    pub fn provider_by_id(&self, id: &str) -> Option<&dyn HarnessProvider>;
}
```

**`DescriptorHarnessProvider`** — 向后兼容的桥接：

```rust
pub struct DescriptorHarnessProvider { ... }
```

当老代码通过描述符（而非新 Harness 协议）请求工作流时，`DescriptorHarnessProvider` 路由到老路径。新模式直接在 `HarnessProvider` 上实现 `plan()` + `execute()`。

**taiji 迁移对照**：taiji 的策略流水线（数据→筛选→回测→优化→执行）可实现为一个 `HarnessProvider`。`plan()` 根据用户输入的交易策略生成每一步的工具调用列表，`execute()` 依次执行。

---

## 6. 附录：完整类型索引

### 定义层（`tool-contracts::framework`）

| 类型 | 说明 |
|------|------|
| `ToolRegistryItem` | 工具基础 trait（9 必需 + 3 可选方法） |
| `ToolManifestDefinition` | 工具包装结构 |
| `ContextualToolManifestItem<C>` | 上下文感知 trait（ToolRegistryItem + 3 方法） |
| `ToolExposure` | Direct / Deferred 枚举 |
| `ToolResult` | Result / Progress / StreamChunk 枚举 |
| `ValidationResult` | 输入校验结果 |
| `InputValidator` | Builder 模式校验器 |
| `ToolRegistry<T>` | 运行时注册表 |
| `ToolRuntimeAssembly<T>` | 注册表构建器 |
| `StaticToolProvider<T>` | 静态提供者 trait |
| `StaticToolProviderGroup<T>` | Arc 包装的静态提供者 |
| `StaticToolProviderFactory<T>` | 延迟实例化 trait |
| `StaticToolProviderPlan` | 声明工具名列表的 trait |
| `ToolRuntimeRestrictions` | 运行时工具限制（allow/deny） |
| `ToolPathPolicy` | 路径策略 |
| `ToolContextFacts` | 工具运行时上下文 |

### 执行层（`tool-contracts::deferred_tool`）

| 类型 | 说明 |
|------|------|
| `CALL_DEFERRED_TOOL_NAME` | 常量：`"CallDeferredTool"` |
| `CallDeferredToolInput` | 延迟调用输入 `{ tool_name, args }` |
| `ResolvedToolInvocation` | 解析后的有效工具名+参数 |
| `ToolInvocationKind` | Direct / Deferred 枚举 |
| `parse_call_deferred_tool_input()` | 解析延迟工具参数 |

### 执行层（`tool-contracts::execution_gate`）

| 类型 | 说明 |
|------|------|
| `ToolExecutionAdmissionRequest` | 准入请求（3 层校验） |
| `ToolExecutionAdmissionRejection` | 准入拒绝（AllowedList/RuntimeRestriction/Deferred） |
| `validate_tool_execution_admission()` | 准入校验函数 |

### 执行层（`tool-contracts::tool_execution_presentation`）

| 类型 | 说明 |
|------|------|
| `ToolExecutionErrorPresentation` | 统一的错误展示 `{ result_json, result_for_assistant }` |
| `build_tool_execution_error_presentation()` | 通用错误 |
| `build_user_steering_interrupted_presentation()` | 用户中断 |
| `build_tool_execution_timeout_presentation()` | 超时 |
| `build_user_rejected_tool_presentation()` | 用户拒绝 |
| `build_permission_denied_tool_presentation()` | 权限拒绝 |
| `build_invalid_tool_call_error_message()` | 非法调用 |
| `render_tool_result_for_assistant()` | 结果→文本 |

### 执行层（`tool-execution::pipeline`）

| 类型 | 说明 |
|------|------|
| `ToolBatch` | 执行批次 `{ task_ids, is_concurrent }` |
| `ToolTaskStateKind` | 8 状态任务状态机 |
| `ToolRetryAttemptFacts` | 重试参数 |
| `ToolCancellationTokenStore` | 取消令牌存储 |
| `SubagentBatchExecutionPolicy` | SafeOnly / ForceParallel / Serial |
| `ToolStateEventKind` | 状态转换事件（带计时） |
| `partition_tool_batches()` | 任务分批 |
| `should_retry_tool_attempt()` | 重试决策 |

### 执行层（`tool-execution::context`）

| 类型 | 说明 |
|------|------|
| `PrimaryModelFacts` | 主模型信息 |
| `ToolRuntimeContextFactsInput` | 上下文输入 |
| `ToolRuntimeCustomDataInput` | 自定义数据输入 |
| `build_tool_runtime_custom_data()` | 构建自定义数据 |
| `project_tool_context_facts()` | 构建上下文事实 |

### 快照层（`tool-contracts::tool_snapshot`）

| 类型 | 说明 |
|------|------|
| `ToolSnapshotItem` | 单个工具快照条目 |
| `MaterializedToolSnapshot` | 带代际的工具快照集合 |
| `ToolProviderIdentity` | Builtin / Static / Dynamic |
| `materialize_tool_snapshot()` | 异步快照生成 |

### 权限层（`tool-contracts::permission_intent`）

| 类型 | 说明 |
|------|------|
| `PermissionIntent` | 无副作用的权限意图声明 `{ action, resources, save_resources }` |

### Provider 组层（`tool-provider-groups`）

| 类型 | 说明 |
|------|------|
| `ToolPackFeatureGroup` | 编译时特性分类（9 种） |
| `ToolProviderGroupPlan` | 声明式提供者→工具映射 |
| `PRODUCT_TOOL_PROVIDER_GROUP_PLAN` | 全部 51 工具静态清单 |
| `try_product_tool_provider_group_plan_for_ids()` | 按 ID 选择子集 |

### Plugin Host 层（`plugin-runtime-host`）

| 类型 | 说明 |
|------|------|
| `PluginRuntimeHost` | 插件运行时宿主 |
| `PluginHostAdapter` | 插件适配器 trait |
| `PluginRuntimeClient` | 客户端接口（read_plugins + dispatch） |

### Harness 层（`harness`）

| 类型 | 说明 |
|------|------|
| `HarnessWorkflow` | 工作流类型（5 种） |
| `HarnessCapability` | 能力类型（5 种） |
| `HarnessProvider` | 工作流提供者 trait |
| `HarnessPlan` / `HarnessStep` | 执行计划 |
| `HarnessRegistry` | 提供者注册表 |
| `DescriptorHarnessProvider` | 老路径桥接 |

---

## taiji 迁移快速映射

| taiji 概念 | BitFun 对应 | 关键差异 |
|-----------|------------|---------|
| `ComputeNode` | `ToolRegistryItem` | BitFun 用 trait 而非类继承；需实现 9 个方法 |
| `NodeFactory` | `StaticToolProviderFactory` | BitFun 有 `StaticToolProviderPlan` 辅助声明阶段 |
| 节点注册 | `ToolRegistry::register_tool()` 或 `install_static_provider()` | BitFun 通过 `ToolRuntimeAssembly` 批量组装 |
| 节点执行 | `validate_tool_execution_admission()` → 管线调度 | BitFun 有 3 层准入 + 8 状态机 + 错误展示标准化 |
| 上下文传递 | `ToolContextFacts` | 标准化字段（session_id、workspace_root、restrictions） |
| 节点发现（高成本） | `ToolExposure::Deferred` | BitFun 支持两阶段延迟加载 |
| 策略流水线编排 | `HarnessProvider` | plan() + execute() 两阶段 |
| 数据源插件 | `PluginRuntimeHost` + `PluginHostAdapter` | 幂等分发 + 隔离失败 |
