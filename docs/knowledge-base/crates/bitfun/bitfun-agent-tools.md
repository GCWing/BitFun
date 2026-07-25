# bitfun-agent-tools

**路径**: src/crates/execution/tool-contracts
**名称**: bitfun-agent-tools（Cargo.toml 中 package name 为 bitfun-agent-tools，lib name 为 bitfun_agent_tools）

**描述**: Agent tool contracts。纯工具 DTO、helper 和框架，在具体工具框架和工具包移出 core facade 前驻留于此。

## 模块

- `acp_tool_bridge` — ACP 工具桥接（ACP 外部 agent 工具定义/渲染/校验）
- `computer_use` — 计算机使用工具
- `deferred_tool` — 延迟工具（CallDeferredTool 协议）
- `element_token` — 元素标记
- `execution_gate` — 执行门（工具执行准入校验）
- `file_guidance` — 文件引导消息
- `file_read_freshness` — 文件读取新鲜度检查
- `framework` — 工具框架核心（注册表、清单、路径解析、权限校验）
- `input_validator` — 输入校验器
- `mcp_tool_bridge` — MCP 工具桥接
- `permission_intent` — 权限意图
- `tool_execution_presentation` — 工具执行展示（错误消息、审批展示、结果渲染）
- `tool_result_storage` — 工具结果存储（持久化、预览、截断）
- `tool_snapshot` — 工具快照（memento 模式）

## 核心类型

- `ToolRegistry`, `ToolRegistryItem` — 工具注册表
- `ToolManifestDefinition`, `ToolManifestPolicyResolution`, `ToolManifestPolicyTool` — 工具清单
- `StaticToolProvider`, `StaticToolProviderGroup`, `StaticToolProviderFactory` — 静态工具提供者
- `ToolContextFacts`, `ToolRuntimeAssembly`, `ToolRuntimeRestrictions` — 工具运行时上下文
- `ContextualToolManifest`, `ContextualVisibleTools` — 上下文可见工具
- `PortableToolContextProvider` trait — 可移植工具上下文
- `GetToolSpecRuntime`, `GetToolSpecDetail`, `GetToolSpecExecutionPlan` — 工具规范查询
- `ToolCatalogRuntime`, `ToolCatalogSnapshotProvider` — 工具目录
- `DeferredToolUsageError`, `ResolvedToolInvocation`, `ToolInvocationKind` — 延迟工具类型
- `ToolExecutionAdmissionRejection`, `ToolExecutionAdmissionRequest` — 执行准入
- `FileReadFreshnessFacts` — 文件新鲜度事实
- `McpToolBridgeDefinition`, `McpToolBridgeToolInfo` — MCP 桥接
- `AcpExternalAgentToolDefinition`, `AcpExternalAgentToolDefinitionInput` — ACP 桥接
- `PermissionIntent` — 权限意图
- `PersistedToolOutput`, `ToolResultStoragePolicy` — 工具结果持久化
- `MaterializedToolSnapshot`, `ToolCallSnapshotGuard`, `ToolCancellationContract` — 工具快照
- `ToolExecutionErrorPresentation`, `ToolEffectFacts` — 执行展示

## 功能

工具契约 crate。定义完整的工具系统类型体系：工具注册表、清单定义、路径解析、权限校验、延迟工具、MCP/ACP 桥接、执行准入、结果持久化、执行展示。是 bitfun-core 中工具系统的基础。
