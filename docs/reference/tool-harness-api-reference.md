
# BitFun Tool/Harness API Complete Reference

> 自动生成于 2026-07-25。覆盖 5 个 crate 的全部 `pub` 项。

---

## 1. tool-contracts (`src/crates/execution/tool-contracts/src/`)

### 1.1 `lib.rs` — Crate root

| 行 | 项 |
|----|-----|
| 21-29 | `pub mod` 声明 14 个子模块 |
| 30 | `pub use bitfun_core_types::ToolImageAttachment` — 工具消息中的图片附件 |
| 31-33 | `pub use bitfun_runtime_ports::{DynamicToolProvider, PortError, PortErrorKind, PortResult, ToolDecorator}` — 跨边界端口类型 |
| 34-126 | `pub use` 从各子模块重导出（见下方各模块） |

### 1.2 `framework.rs` — 核心工具框架（2684 行，最大的子模块）

**结构体和枚举：**

- `DynamicMcpToolInfo` :23 — 动态 MCP 工具元信息
- `DynamicToolInfo` :32 — 动态工具信息（MCP 或 ACP）
- `ToolWorkspaceKind` :42 — 工具的工作空间类型（host/remote）
- `ToolContextFacts` :49 — 工具调用上下文事实
- `DeferredToolUsageError` :73 — 延迟工具使用错误（未调用 GetToolSpec 等）
- `ToolExecutionAccessError` :125 — 工具执行访问错误（权限/速率限制/并发等）
- `ToolExposure` :210 — 工具可见性级别（full/hidden/deprecated）
- `ToolManifestDefinition` :216 — 工具清单定义（name/description/schema/readonly 等）
- `PromptVisibleToolManifestItem` :233 — 提示可见的工具清单项（tool 或 separator）
- `ToolManifestPolicyTool` :238 — 策略级工具（如 GetToolSpec）
- `ToolManifestPolicyResolution` :245 — 工具清单策略解析结果
- `ContextualVisibleTools` :252 — 上下文相关的可见工具集
- `ContextualToolManifest` :260 — 上下文工具清单（可见工具 + 策略工具 + 分类）
- `GetToolSpecDeferredToolSummary` :350 — GetToolSpec 返回的延迟工具摘要
- `GetToolSpecDetail` :355 — GetToolSpec 返回的工具详细规格
- `GetToolSpecExecutionError` :513 — GetToolSpec 执行错误
- `GetToolSpecExecutionPlan` :530 — GetToolSpec 执行计划（catalog/provider/multi 等）
- `GetToolSpecLoadObservation` :557 — GetToolSpec 加载观察结果
- `LoadedDeferredToolSpec` :565 — 已加载的延迟工具规格
- `ToolCatalogRuntime` :835 — 工具目录运行时（预注册工具 + GetToolSpec 目录）
- `GetToolSpecRuntime` :1006 — GetToolSpec 工具运行时
- `ToolRef` :1225 — `Arc<Tool>` 类型别名
- `ToolDecoratorRef` :1226 — `Arc<dyn ToolDecorator>` 类型别名
- `SnapshotToolDecorator` :1236 — 快照工具装饰器
- `StaticToolMaterializationError` :1269 — 静态工具实例化错误
- `StaticToolProviderGroup` :1292 — 静态工具提供者组
- `ToolRuntimeAssembly` :1349 — 工具运行时装配（ToolRef + ToolDecoratorRef 映射）
- `ToolRegistry` :1420 — 工具注册表（核心类型：注册/查找/列表工具）
- `ToolRenderOptions` :1634 — 工具渲染选项
- `ToolPathBackend` :1639 — 工具路径后端（filesystem/bitfun-uri）
- `ToolPathResolution` :1645 — 工具路径解析结果
- `ParsedBitFunRuntimeUri` :1679 — 解析后的 bitfun runtime URI
- `ParsedBitFunCurrentSessionUri` :1685 — 解析后的 session URI
- `ToolPathContractError` :1690 — 工具路径契约错误
- `ToolPathOperation` :2147 — 工具路径操作（read/write/execute）
- `ToolPathPolicy` :2164 — 工具路径策略
- `ToolRuntimeRestrictions` :2219 — 工具运行时限制
- `ToolRestrictionError` :2355 — 工具限制错误
- `ValidationResult` :2392 — 验证结果
- `ToolResult` :2413 — 工具结果（success/error/aborted）

**Trait：**

- `PortableToolContextProvider` :68 — 可移植工具上下文提供者
- `ToolRegistryItem` :674 — 工具注册表项
- `ContextualToolManifestItem` :724 — 上下文工具清单项
- `ToolCatalogSnapshotProvider` :766 — 工具目录快照提供者
- `GetToolSpecCatalogProvider` :771 — GetToolSpec 目录提供者
- `SnapshotToolWrapper` :1230 — 快照工具包装器 trait
- `StaticToolProvider` :1252 — 静态工具提供者 trait
- `StaticToolProviderPlan` :1258 — 静态工具提供者计划 trait
- `StaticToolProviderFactory` :1264 — 静态工具提供者工厂 trait

**函数（选要）：**

- `validate_tool_allowed_by_list` :148 — 验证工具是否在允许列表中
- `validate_deferred_tool_usage` :162 — 验证延迟工具使用
- `resolve_tool_manifest_policy` :268 — 解析工具清单策略
- `build_tool_manifest_policy_tools` :317 — 构建策略工具（GetToolSpec + Skill + Agent 清单）
- `get_tool_spec_input_schema` :374 — GetToolSpec 输入 JSON schema
- `build_get_tool_spec_description` :392 — 构建 GetToolSpec 工具描述
- `get_tool_spec_is_readonly` :421 — GetToolSpec 是否只读
- `validate_get_tool_spec_input` :437 — 验证 GetToolSpec 输入
- `resolve_get_tool_spec_execution_plan` :535 — 解析 GetToolSpec 执行计划
- `collect_loaded_deferred_tool_specs` :570 — 收集已加载的延迟工具规格
- `sort_tool_manifest_definitions` :656 — 排序工具清单定义
- `resolve_contextual_visible_tools` :1130 — 解析上下文可见工具
- `resolve_contextual_tool_manifest` :1166 — 解析上下文工具清单
- `materialize_static_tool_provider_groups` :1322 — 实例化静态工具提供者组
- `is_bitfun_runtime_uri` :1760 — 是否 bitfun runtime URI
- `resolve_host_path_with_workspace` :1793 — 解析 host 路径
- `resolve_tool_path_with_context` :1827 — 基于上下文解析工具路径
- `parse_bitfun_runtime_uri` :1957 — 解析 bitfun runtime URI
- `build_bitfun_runtime_uri` :2011 — 构建 bitfun runtime URI
- `tool_restrictions_for_delegation_policy` :2316 — 委派策略的工具限制

**常量：**

- `GET_TOOL_SPEC_TOOL_NAME` :70 — GetToolSpec 工具名
- `BITFUN_RUNTIME_URI_PREFIX` :1675 — `bitfun://` 前缀
- `BITFUN_CURRENT_SESSION_URI_PREFIX` :1676 — `bitfun-current-session://` 前缀

### 1.3 `acp_tool_bridge.rs` — ACP 工具桥接

- `ACP_TOOL_PREFIX` :5 — ACP 工具名前缀常量
- `ACP_TOOL_SUFFIX` :6 — ACP 工具名后缀常量
- `AcpExternalAgentToolDefinitionInput` :9 — ACP 外部代理工具定义输入
- `AcpExternalAgentToolDefinition` :16 — ACP 外部代理工具定义
- `normalize_name_for_acp_tool_part` :26 — 规范化 ACP 工具名部分
- `build_acp_external_agent_tool_name` :40 — 构建 ACP 外部代理工具名
- `build_acp_external_agent_tool_definition` :47 — 构建 ACP 外部代理工具定义
- `acp_external_agent_tool_input_schema` :68 — ACP 外部代理工具输入 schema
- `validate_acp_external_agent_tool_input` :91 — 验证 ACP 外部代理工具输入
- `render_acp_external_agent_use_message` :109 — 渲染 ACP 外部代理工具使用消息
- `render_acp_external_agent_rejected_message` :118 — 渲染拒绝消息
- `render_acp_external_agent_result_message` :122 — 渲染结果消息
- `render_acp_external_agent_result_for_assistant` :130 — 渲染给 assistant 的结果
- `build_acp_external_agent_tool_result` :138 — 构建 ACP 外部代理工具结果

### 1.4 `computer_use.rs` — 计算机使用（1766 行）

**结构体/枚举（选要）：**

- `ComputerUseContractError` :12 — 计算机使用契约错误
- `ScreenshotCropCenter` :40 — 截图裁剪中心点
- `ComputerUseNavigationRect` :47 — 导航矩形区域
- `ComputerUseNavigateQuadrant` :57 — 导航象限枚举
- `ComputerUseImplicitScreenshotCenter` :67 — 隐式截图中心
- `ComputerUseScreenshotParams` :76 — 截图参数
- `ComputerUsePermissionSnapshot` :137 — 权限快照
- `ComputerUseForegroundApplication` :146 — 前台应用信息
- `ComputerUsePointerGlobal` :157 — 全局指针坐标
- `ComputerUseSessionSnapshot` :164 — 会话快照
- `ComputerUseImageContentRect` :173 — 图片内容矩形
- `ComputerUseImageGlobalBounds` :183 — 图片全局边界
- `ComputerScreenshot` :194 — 截图数据
- `OcrRegionNative` :253 — OCR 区域
- `OcrTextMatch` :263 — OCR 文本匹配
- `UiElementLocateQuery` :276 — UI 元素定位查询
- `UiElementLocateResult` :311 — UI 元素定位结果
- `OcrAccessibilityHit` :359 — OCR 无障碍命中
- `AppSelector` :380 — 应用选择器（by_name/by_pid/by_bundle_id）
- `AppInfo` :425 — 应用信息
- `AxNode` :451 — 无障碍树节点
- `AppMenuShortcut` :510 — 应用菜单快捷键
- `AppShortcutsSnapshot` :541 — 应用快捷键快照
- `AppStateSnapshot` :615 — 应用状态快照
- `InteractiveViewOpts` :660 — 交互视图选项
- `InteractiveElement` :708 — 交互元素
- `InteractiveView` :753 — 交互视图
- `ClickIndexTarget` :789 — 点击索引目标
- `InteractiveClickParams` :799 — 交互点击参数
- `InteractiveTypeTextParams` :833 — 交互输入文本参数
- `InteractiveScrollParams` :856 — 交互滚动参数
- `InteractiveActionResult` :879 — 交互操作结果
- `VisualMarkViewOpts` :894 — 视觉标记视图选项
- `VisualImageRegion` :918 — 视觉图片区域
- `VisualMark` :926 — 视觉标记
- `VisualMarkView` :937 — 视觉标记视图
- `VisualClickParams` :949 — 视觉点击参数
- `VisualActionResult` :969 — 视觉操作结果
- `ClickTarget` :980 — 点击目标
- `AppClickParams` :1040 — 应用点击参数
- `AppWaitPredicate` :1074 — 应用等待谓词
- `ComputerUseDisplayInfo` :1091 — 显示器信息
- `OpenAppResult` :1120 — 打开应用结果
- `ComputerUseScreenshotRefinement` :1131 — 截图细化
- `ComputerUseInteractionScreenshotKind` :1149 — 交互截图类型
- `ComputerUseLastMutationKind` :1158 — 最后变更类型
- `ComputerUseInteractionState` :1170 — 交互状态

**函数（选要）：**

- `parse_screenshot_params` :1362 — 解析截图参数
- `build_screenshot_tool_body_and_hint` :1444 — 构建截图工具体和提示
- `screenshot_covers_full_display` :1617 — 截图是否覆盖整个显示器

### 1.5 `deferred_tool.rs` — 延迟工具

- `CALL_DEFERRED_TOOL_NAME` :4 — "CallDeferredTool" 常量
- `CallDeferredToolInput` :7 — 调用延迟工具输入
- `CallDeferredToolInputError` :13 — 输入错误枚举
- `call_deferred_tool_input_schema` :41 — 输入 JSON schema
- `call_deferred_tool_short_description` :60 — 短描述
- `call_deferred_tool_description` :64 — 完整描述
- `parse_call_deferred_tool_input` :72 — 解析输入
- `effective_tool_invocation` :118 — 解析有效的工具调用名
- `ToolInvocationKind` :130 — 调用类型（direct/get_tool_spec/failure）
- `ResolvedToolInvocation` :136 — 解析后的工具调用

### 1.6 `element_token.rs` — 元素令牌

- `LRU_CAP_PER_PID` :66 — 每个进程的 LRU 缓存容量常量
- `STALE_TOKEN_ERROR` :72 — 令牌过期错误消息常量
- `TokenError` :95 — 令牌错误枚举
- `TokenRegistry` :142 — 令牌注册表（mint/resolve 快照元素令牌）
- `mint_snapshot_id` :263 — 铸造快照 ID
- `format_token` :280 — 格式化令牌字符串
- `global` :303 — 获取全局 TokenRegistry
- `token_for` :316 — 为元素创建令牌
- `ResolvedElement` :323 — 解析后的元素（UI/Accessibility/Visual 等）
- `resolve_element_args` :364 — 解析元素参数

### 1.7 `execution_gate.rs` — 执行门控

- `ToolExecutionAdmissionRequest` :9 — 工具执行准入请求
- `ToolExecutionAdmissionRejection` :21 — 准入拒绝原因枚举
- `validate_tool_execution_admission` :39 — 验证工具执行准入

### 1.8 `file_guidance.rs` — 文件指导

- `FILE_TOOL_GUIDANCE_PREFIX` :3 — 文件工具指导前缀常量
- `file_tool_guidance_message` :5 — 构建文件工具指导消息
- `is_file_tool_guidance_message` :9 — 判断是否为文件工具指导消息

### 1.9 `file_read_freshness.rs` — 文件读取新鲜度

- `FileReadFreshnessFacts` :4 — 文件读取新鲜度事实
- `normalize_tool_file_content` :21 — 规范化工具文件内容
- `file_read_facts_content_matches` :30 — 文件内容是否匹配
- `file_read_facts_are_fresh` :39 — 文件读取事实是否新鲜

### 1.10 `input_validator.rs` — 输入验证器

- `InputValidator` :4 — 输入验证器（schema 验证等）

### 1.11 `mcp_tool_bridge.rs` — MCP 工具桥接

- `MCP_TOOL_PREFIX` :5 — MCP 工具名前缀常量
- `MCP_TOOL_DELIMITER` :6 — MCP 工具名分隔符常量
- `normalize_name_for_mcp` :9 — 规范化 MCP 名称
- `build_mcp_tool_bridge_name` :21 — 构建 MCP 桥接工具名
- `McpToolBridgeToolInfo` :32 — MCP 工具信息
- `McpToolBridgeDefinition` :39 — MCP 桥接工具定义
- `McpToolBridgeBehaviorHints` :51 — 行为提示
- `McpToolBridgeDefinitionInput` :58 — 定义输入
- `build_mcp_tool_bridge_definition` :67 — 构建 MCP 桥接定义
- `mcp_tool_bridge_short_description` :98 — 短描述
- `mcp_tool_bridge_dynamic_tool_info` :108 — 动态工具信息
- `validate_mcp_tool_bridge_input` :120 — 验证输入
- `render_mcp_tool_bridge_use_message` :149 — 渲染使用消息
- `render_mcp_tool_bridge_rejected_message` :156 — 渲染拒绝消息
- `render_mcp_tool_bridge_result_message` :163 — 渲染结果消息
- `build_mcp_tool_bridge_result` :170 — 构建结果

### 1.12 `permission_intent.rs` — 权限意图

- `PermissionIntent` :9 — 权限意图（工具名 → 资源路径映射）

### 1.13 `tool_execution_presentation.rs` — 工具执行展示

- `TOOL_ERROR_ARGUMENTS_PREVIEW_BYTES` :3 — 错误参数预览字节数
- `USER_STEERING_INTERRUPTED_MESSAGE` :4 — 用户中断消息常量
- `USER_REJECTED_TOOL_MESSAGE` :5 — 用户拒绝消息常量
- `ToolExecutionErrorPresentation` :9 — 工具执行错误展示结构
- `render_tool_result_for_assistant` :14 — 渲染工具结果给 assistant
- `is_write_like_tool_name` :22 — 判断是否为写类工具
- `build_tool_call_truncation_recovery_notice` :26 — 构建截断恢复提示
- `truncate_tool_arguments_preview` :38 — 截断工具参数预览
- `build_tool_execution_error_presentation` :59 — 构建错误展示
- `build_user_steering_interrupted_presentation` :89 — 构建用户中断展示
- `build_tool_execution_timeout_presentation` :103 — 构建超时展示
- `build_user_rejected_tool_presentation` :130 — 构建用户拒绝展示
- `build_permission_denied_tool_presentation` :163 — 构建权限拒绝展示
- `build_invalid_tool_call_error_message` :184 — 构建无效工具调用错误消息

### 1.14 `tool_result_storage.rs` — 工具结果存储

- `DEFAULT_MAX_TOOL_RESULT_CHARS` :6 — 默认最大工具结果字符数
- `MAX_TOOL_RESULTS_PER_ROUND_CHARS` :7 — 每回合最大工具结果字符数
- `TOOL_RESULT_PREVIEW_CHARS` :8 — 结果预览字符数
- `PERSISTED_OUTPUT_TAG` :9 — 持久化输出标签常量
- `ToolResultStoragePolicy` :13 — 工具结果存储策略
- `PersistedToolOutput` :30 — 持久化工具输出
- `ToolResultPersistenceCandidate` :40 — 持久化候选
- `select_tool_result_indices_for_persistence` :45 — 选择需要持久化的结果
- `sanitize_tool_result_file_component` :65 — 清理文件名组件
- `generate_tool_result_preview` :84 — 生成预览
- `tool_result_is_persisted_output` :108 — 判断是否持久化输出
- `build_persisted_tool_output_message` :112 — 构建持久化输出消息

### 1.15 `tool_snapshot.rs` — 工具快照

- `ToolProviderIdentity` :8 — 工具提供者身份标识
- `ToolEffectFactsSource` :59 — 工具副作用来源枚举
- `ToolEffectFacts` :65 — 工具副作用事实
- `ToolCancellationContract` :73 — 工具取消契约
- `ToolEffectFilter` :79 — 工具副作用过滤器
- `ToolSnapshotItem` :100 — 工具快照项
- `MaterializedToolSnapshot` :115 — 实例化后的工具快照
- `ToolCallSnapshotGuard` :165 — 工具调用快照守卫（RAII）
- `ToolSnapshotCallError` :180 — 工具快照调用错误
- `materialize_tool_snapshot` :216 — 异步实例化工具快照

---

## 2. harness (`src/crates/execution/harness/src/lib.rs`)

- `HarnessWorkflow` :24 — 工作流枚举：Sdd（SDLC）/ DeepReview / DeepResearch / MiniApp / FunctionAgent
- `HarnessCapability` :42 — 能力枚举：Plan / Execute / ReviewGate / Artifact / PostProcessor
- `HarnessPlan` :60 — 工具编排计划（当前仅 Sdd 变体，含 ResourceSnapshot）
- `HarnessStep` :68 — 编排步骤
- `HarnessStepStatus` :78 — 步骤状态
- `HarnessStepOutcome` :88 — 步骤执行结果
- `HarnessCandidateView` :108 — 候选视图
- `HarnessPlanProgress` :120 — 计划进度
- `HarnessPlanFinal` :135 — 计划最终输出
- `HarnessResult` :151 — 编排结果
- `HarnessError` :170 — 编排错误
- `build_harness_result_not_available_error` :191 — 编排结果不可用错误
- `HarnessProvider` trait :253 — 编排提供者 trait，含 `plan()` 返回 `HarnessPlan`，`execute()` 返回 `HarnessResult`
- `HarnessRegistryBuilder` :320 — 编排注册表构建器
- `HarnessRegistryBuilderError` :335 — 构建错误
- `HarnessProviderDescriptor` :348 — 编排提供者描述符
- `HarnessRegistry` :365 — 编排注册表
- `build_descriptor_harness_registry` :438 — 从描述符构建编排注册表（便捷函数）

---

## 3. agent-runtime (`src/crates/execution/agent-runtime/src/`)

### 3.1 `lib.rs` — Crate root

| 行 | 项 |
|----|-----|
| 3-38 | `pub mod` 声明 35 个子模块（其中 33 个有源文件，2 个缺失：`deep_review.rs` 和 `skills.rs`） |

### 3.2 `agents.rs` — 内置代理定义

- `SHARED_CODING_MODE_PROMPT_TEMPLATE` :9 — 共享编码模式提示模板常量
- `SHARED_CODING_MODE_CONFIG_PROFILE_ID` :10 — 配置档案 ID 常量
- `SHARED_CODING_MODE_CONFIG_PROFILE_LABEL` :11 — 配置档案标签常量
- `SHARED_CODING_MODE_IDS` :12 — 共享编码模式 ID 列表常量
- `resolve_mode_config_profile_id` :14 — 解析模式的配置档案 ID
- `mode_config_profile_member_mode_ids` :23 — 配置档案的成员模式 ID 列表
- `mode_config_profile_label` :30 — 配置档案标签
- `mode_presentation_rank` :37 — 模式展示排序
- `shared_coding_mode_user_context_policy` :50 — 共享编码模式的用户上下文策略
- `BuiltinAgentCategory` :59 — 内置代理分类枚举
- `BuiltinAgentDefinitionSpec` :66 — 内置代理定义规格
- `builtin_agent_definition_specs` :73 — 返回所有内置代理定义
- `default_model_id_for_builtin_agent` :207 — 内置代理的默认模型 ID
- `SubagentListScope` :242 — 子代理列表范围枚举
- `SubagentQueryContext<'a>` :248 — 子代理查询上下文
- `BuiltinSubagentExposure` :261 — 内置子代理可见性枚举
- `SubagentVisibilitySummary` :269 — 可见性摘要
- `SubagentVisibilityPolicy` :277 — 可见性策略（控制子代理在 UI 提示中的可见性）
- `SubagentSourceKind` :381 — 子代理来源类型
- `SubAgentSource` :392 — 子代理来源枚举
- `subagent_source_kind` :399 — 获取子代理来源类型
- `subagent_source_presentation_rank` :409 — 子代理来源展示权重
- `SubagentOverrideState` :421 — 子代理覆盖状态
- `SubagentStateReason` :428 — 子代理状态原因
- `SubagentOverrideLayers` :439 — 子代理覆盖层
- `ResolvedSubagentAvailability` :445 — 解析后的子代理可用性
- `resolve_subagent_default_enabled` :452 — 解析默认启用状态
- `resolve_subagent_availability` :466 — 解析子代理可用性（合并所有覆盖层）

### 3.3 `checkpoint.rs` — 轻量检查点

- `LightCheckpoint` :4 — 轻量检查点结构
- `LightCheckpointWorkspaceFacts` :12 — 工作区事实枚举
- `GitStatusCheckpointFacts` :22 — Git 状态检查点事实
- `build_light_checkpoint` :29 — 构建轻量检查点

### 3.4 `context_profile.rs` — 上下文档案

- `ContextProfile` :10 — 上下文档案枚举（agent/subagent）
- `ModelCapabilityProfile` :30 — 模型能力档案
- `ContextProfilePolicy` :77 — 上下文档案策略（循环检测/并发限制/超时等）

### 3.5 `custom_agent.rs` — 自定义代理

- `DEFAULT_CUSTOM_MODE_TOOLS` :10 — 默认 Mode 工具集常量
- `DEFAULT_CUSTOM_SUBAGENT_TOOLS` :26 — 默认 Subagent 工具集常量
- `DEFAULT_CUSTOM_MODE_READONLY` :27 — 默认只读标记常量
- `DEFAULT_CUSTOM_SUBAGENT_READONLY` :28 — 默认子代理只读常量
- `DEFAULT_CUSTOM_SUBAGENT_REVIEW` :29 — 默认审查标记常量
- `DEFAULT_CUSTOM_MODE_MODEL` :30 — 默认模型常量
- `DEFAULT_CUSTOM_SUBAGENT_MODEL` :31 — 默认子代理模型常量
- `CUSTOM_AGENT_PROJECT_AGENT_SUBDIRS` :34 — 项目代理子目录常量
- `CUSTOM_AGENT_SCHEMA_VERSION` :35 — Schema 版本常量
- `CustomAgentKind` :39 — 代理类型枚举（mode/subagent）
- `CustomAgentLevel` :46 — 代理级别枚举
- `CustomAgentDefinition` :52 — 自定义代理定义
- `CustomAgentFrontMatterMetadata` :72 — Frontmatter 元数据
- `ParsedCustomAgentDefinition` :79 — 解析后的定义
- `CustomAgentDefinitionError` :85 — 定义错误
- `CustomAgentDiscoveryRoots` :238 — 发现根目录
- `CustomAgentDirEntry` :245 — 目录条目
- `LoadedCustomAgentDefinition` :251 — 已加载的定义
- `CustomAgentLoadError` :258 — 加载错误
- `CustomAgentLoadReport` :264 — 加载报告
- `CustomAgentValidationContext<'a>` :270 — 验证上下文
- `CustomAgentValidationReport` :277 — 验证报告
- `CustomAgentModelFallback` :285 — 模型回退
- `default_custom_agent_tools` :297 — 默认工具集
- `load_custom_agent_definitions` :360 — 从磁盘加载所有自定义代理定义
- `validate_custom_agent_definition` :424 — 验证自定义代理定义

### 3.6 `custom_subagent.rs` — 自定义子代理

- `CustomSubagentKind` :15 — 类型别名 = `CustomAgentLevel`
- `CustomSubagentDefinition` :16 — 类型别名 = `CustomAgentDefinition`
- `load_custom_subagent_definitions` :77 — 加载自定义子代理定义
- (其他函数与 custom_agent 镜像，专门处理子代理路径)

### 3.7 `deep_research.rs` — 深度研究

- `ResearchCitationRenumberStats` :23 — 引用重编号统计
- `ResearchCitationDisplayMapEntry` :31 — 引用显示映射条目
- `ResearchCitationRenumberOutput` :38 — 重编号输出
- `should_post_process_research_report` :46 — 是否需要对研究报告后处理
- `renumber_research_report` :57 — 重新编号研究报告引用

### 3.8 `deep_review.rs` — 深度审查（源文件不存在，仅测试文件）

### 3.9 `dialog_turn.rs` — 对话回合

- `new_turn_id` :14 — 生成新回合 ID
- `TurnStats` :19 — 回合统计

### 3.10 `event_bus.rs` — 事件总线

- `EventBusError` :6 — 事件总线错误枚举
- `EventBusResult<T>` :17 — 类型别名
- `EventSubscriberResult` :18 — 类型别名

### 3.11 `event_queue.rs` — 事件队列

- `SessionEventReceiver` :34 — 会话事件接收器
- `EventQueueConfig` :88 — 事件队列配置
- `QueueStats` :104 — 队列统计
- `EventQueue` :116 — 事件队列（enqueue/dequeue/subscribe）

### 3.12 `event_router.rs` — 事件路由器

- `EventSubscriber` trait :13 — 事件订阅者 trait
- `EventRouter` :22 — 事件路由器（subscribe/route/route_batch）

### 3.13 `event_source.rs` — 事件源

- `AgentEventSource` :17 — 代理事件源
- `AgentEventReceiver` :43 — 类型别名
- `AgentSessionEventReceiver` :47 — 类型别名

### 3.14 `events.rs` — 事件定义

- `FinishReason` :9 — 完成原因枚举（stop/tool_calls/length/cancelled 等）
- `session_state_label` :37 — 会话状态标签
- `turn_outcome_status_kind` :46 — 回合结果状态类型
- `turn_outcome_kind` :54 — 回合结果类型

### 3.15 `evidence_ledger.rs` — 证据账本

- `EvidenceLedgerTargetKind` :11 — 目标类型
- `EvidenceLedgerEventStatus` :27 — 事件状态
- `EvidenceLedgerCheckpoint` :43 — 检查点
- `EvidenceLedgerEvent` :54 — 事件（含时间戳/类型/状态/文件/输出等）
- `EvidenceLedgerSummaryItem` :73 — 摘要项
- `EvidenceLedgerSummary` :88 — 摘要
- `SessionEvidenceLedger` :97 — 会话级证据账本（append/query/summary）

### 3.16 `file_read_state.rs` — 文件读取状态

- `FILE_UNEXPECTEDLY_MODIFIED_ERROR` :9 — 文件意外修改错误常量
- `FileMutationKind` :13 — 文件变更类型
- `FileReadState` :28 — 文件读取状态（记录最近读取的文件内容和时间）
- `validate_prior_read_state` :90 — 验证之前读取状态
- `content_unchanged_since_full_read` :114 — 内容自全量读取后是否未变
- `assert_file_not_unexpectedly_modified` :121 — 断言文件未被意外修改
- `validate_edit_content_freshness_against_read_state` :141 — 基于读取状态验证编辑内容新鲜度
- `validate_write_mtime_freshness_against_read_state` :168 — 基于读取状态验证写入 mtime 新鲜度
- `validate_write_content_freshness_against_read_state` :183 — 基于读取状态验证写入内容新鲜度
- `FileReadStateStore` :207 — 文件读取状态存储（会话级）

### 3.17 `output_surface.rs` — 输出表面

- `TOOL_CONTEXT_INLINE_MARKDOWN_IMAGE_DISPLAY_KEY` :3 — 内联图片显示键常量
- `supports_inline_markdown_images_for_source` :5 — 判断来源是否支持内联图片

### 3.18 `permission.rs` — 权限管理

- `AUTO_APPROVE_ASK_CONTEXT_KEY` :24 — 自动批准上下文键常量
- `PermissionRequestEventReceiver` :26 — 类型别名
- `PermissionWaitOutcome` :29 — 等待结果枚举
- `PendingPermissionReceiver` :35 — 待处理权限接收器
- `PermissionReplyResolution` :55 — 回复解析
- `PermissionRequestManagerError` :63 — 管理器错误
- `PermissionRequestManager` :85 — 权限请求管理器（subscribe/register/reply/cancel）

### 3.19 `post_call_hooks.rs` — 调用后钩子

- `RuntimeHookKind` :11 — 钩子类型枚举
- `successful_tool_post_call_hooks` :16 — 成功工具调用后的钩子列表
- `RuntimeHookErrorPolicy` :22 — 错误策略
- `RuntimeHookPlan` :30 — 钩子执行计划
- `RuntimeHookRegistryBuildError` :86 — 构建错误
- `RuntimeHookRegistryBuilder` :96 — 注册表构建器
- `RuntimeHookRegistry` :133 — 钩子注册表
- `SuccessfulToolPostCallHookExecutor<C>` trait :147 — 成功调用后钩子执行器
- `run_successful_tool_post_call_hooks` :156 — 运行钩子
- `DeepReviewSharedContextToolUseFacts<'a>` :175 — 深度审查共享上下文工具使用事实
- `DeepReviewSharedContextToolUseRecord` :185 — 工具使用记录
- `resolve_deep_review_shared_context_tool_use` :192 — 解析共享上下文工具使用

### 3.20 `prompt.rs` — 提示构建

- `PromptEnvironmentFacts<'a>` :23 — 环境事实
- `render_prompt_environment_info` :30 — 渲染环境信息
- `RemoteExecutionHints` :63 — 远程执行提示
- `RuntimeContextNeeds` :70 — 运行时上下文需求
- `RuntimeShellFacts` :125 — Shell 事实
- `RuntimeContextFacts` :132 — 运行时上下文事实
- `render_runtime_context_reminder` :143 — 渲染运行时上下文提醒
- `PromptRelatedPath` :254 — 提示相关路径
- `WorkspaceContextFacts` :260 — 工作区上下文事实
- `render_workspace_context` :266 — 渲染工作区上下文
- `ProjectLayoutFacts` :329 — 项目布局事实
- `render_project_layout` :336 — 渲染项目布局（树状）
- `render_user_context_reminder` :356 — 渲染用户上下文提醒
- `UserContextSection` :471 — 用户上下文区段枚举
- `UserContextPolicy` :479 — 用户上下文策略（控制 workspace/memory/layout 等上下文注入）
- `ToolListingSections` :553 — 工具列表区段（skill/agent/deferred 工具）
- `PrependedPromptReminders` :607 — 前置提示提醒

### 3.21 `prompt_cache.rs` — 提示缓存

- `PROMPT_CACHE_SCHEMA_VERSION` :6 — Schema 版本
- `DEFAULT_PROMPT_CACHE_PERSISTENCE_TTL` :7 — 默认持久化 TTL
- `PromptCachePolicy` :10 — 缓存策略
- `SystemPromptCacheIdentity` :25 — 系统提示缓存身份
- `UserContextCacheIdentity` :38 — 用户上下文缓存身份
- `prompt_cache_scope_key` :50 — 缓存作用域键
- `CachedPromptText` :58 — 缓存的提示文本
- `CachedSystemPrompt` :80 — 缓存的系统提示
- `CachedUserContext` :105 — 缓存的用户上下文
- `SessionPromptCache` :130 — 会话提示缓存
- `PromptCachePersistenceWriteAction` :169 — 持久化写入动作
- `PromptCacheRestoreDecision` :175 — 恢复决策
- `reconcile_prompt_cache_restore` :190 — 协调缓存恢复
- `prompt_cache_persist_action` :205 — 持久化动作
- `PromptCacheScope` :216 — 缓存作用域枚举
- `SessionPromptCacheStore` :232 — 会话提示缓存存储
- `PromptCacheLookup` :236 — 缓存查找结果

### 3.22 `prompt_markup.rs` — 提示标记

- `USER_QUERY_TAG` :3 — 用户查询标签常量
- `SYSTEM_REMINDER_TAG` :4 — 系统提醒标签常量
- `PromptBlockKind` :9 — 提示块类型
- `PromptBlock` :15 — 提示块
- `PromptEnvelope` :44 — 提示信封（多块组合）
- `render_user_query` :79 — 渲染用户查询
- `render_system_reminder` :83 — 渲染系统提醒
- `has_prompt_markup` :87 — 是否有提示标记
- `is_system_reminder_only` :94 — 是否仅系统提醒
- `strip_prompt_markup` :100 — 移除标记

### 3.23 `remote_file_delivery.rs` — 远程文件交付

- `TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY` :4 — 远程文件交付键常量
- `needs_computer_links_for_source` :6 — 来源是否需要计算机链接
- `remote_file_delivery_reminder` :13 — 远程文件交付提醒
- `workspace_relative_link` :19 — 工作区相对链接
- `computer_link` :25 — 计算机链接
- `user_file_link` :31 — 用户文件链接

### 3.24 `runtime.rs` — 代理运行时（核心模块，2704 行）

- `RuntimeBuildError` :40 — 运行时构建错误
- `RuntimeError` :50 — 运行时错误
- `AgentSessionRestoreRequest` :93 — 会话恢复请求
- `AgentSessionRestoreResult` :106 — 会话恢复结果
- `AgentSessionRestorePort` trait :112 — 会话恢复端口
- `AgentUserAnswersRequest` :122 — 用户回答请求
- `AgentInteractionResponsePort` trait :131 — 交互响应端口
- `AgentEventStream` :136 — 代理事件流
- `RuntimeAgentRegistryQuery<'a>` :175 — 代理注册表查询
- `RuntimeAgentRegistry` trait :179 — 代理注册表
- `RuntimeToolRegistry` trait :346 — 工具注册表 trait
- `AgentRuntimeBuilder` :360 — 运行时构建器（含 20+ `with_*port()` 方法）
- `AgentRunRequest` :639 — 代理运行请求
- `AgentRunHandle` :682 — 代理运行句柄
- `AgentRuntime` — 代理运行时（含 50+ pub 方法：`create_session/delete_session/submit_turn/run/cancel_turn/set_session_archived/...`）
- `SessionSelector` :595 — 会话选择器

### 3.25 `scheduler.rs` — 调度器

- `DEFAULT_MAX_DIALOG_QUEUE_DEPTH` :17 — 默认最大对话队列深度
- `ActiveDialogTurn` :20 — 活跃对话回合
- `ActiveDialogTurnStore` :125 — 活跃对话回合存储
- `DialogTurnQueue<T>` :264 — 对话回合队列（FIFO + max_depth）
- `AgentSessionReplyPlan` :362 — 代理会话回复计划
- `AgentSessionReplyAction` :373 — 回复动作枚举
- `DialogSteeringAction` :380 — 对话引导动作
- `BackgroundDeliveryFacts` :391 — 后台交付事实
- `BackgroundDeliveryAction` :396 — 后台交付动作
- `BackgroundInjectionKind` :402 — 后台注入类型
- `ThreadGoalDeliveryReminderKind` :408 — 线程目标交付提醒类型
- `ThreadGoalDeliveryReminder` :414 — 提醒内容
- `ThreadGoalDeliveryPlan` :420 — 交付计划
- `DialogRoundInjectionInterrupt` :517 — 对话回合注入中断
- `SessionRoundInjectionBuffer` :566 — 会话回合注入缓冲区
- `TurnOutcome` :737 — 回合结果
- `TurnOutcomeStatus` :756 — 回合结果状态
- `TurnOutcomeLifecyclePlan` :834 — 回合结果生命周期计划
- `DialogStartRouteFacts` :852 — 对话启动路由事实
- `DialogStartRoute` :858 — 对话启动路由枚举
- `resolve_turn_outcome_lifecycle_plan` :874 — 解析生命周期计划
- `resolve_agent_session_reply_action` :901 — 解析回复动作
- `resolve_dialog_steering_action` :939 — 解析引导动作

### 3.26 `scheduled_job.rs` — 定时任务

- `DEFAULT_SCHEDULED_JOB_RETRY_DELAY_MS` :9 — 默认重试延迟常量
- `ScheduledJobRunStatus` :13 — 运行状态
- `ScheduledJobRuntimeState` :23 — 运行时状态
- `ScheduledJobTriggerAction` :53 — 触发动作
- `ScheduledJobEnqueueFailureAction` :59 — 入队失败动作

### 3.27 `session.rs` — 会话

- `Session` :12 — 会话结构（含 id/workspace/thread_goal/state/config 等）
- `CompressionState` :73 — 压缩状态
- `SessionConfig` :138 — 会话配置
- `SessionSummary` :207 — 会话摘要
- `PersistedSessionStateFile` :238 — 持久化会话状态文件
- `sanitize_persisted_session_state` :255 — 清理持久化会话状态

### 3.28 `session_control.rs` — 会话控制

- `SessionControlAction` :9 — 会话控制动作枚举
- `SessionControlAgentType` :28 — 会话控制代理类型
- `SessionControlInput` :48 — 输入结构
- `SessionControlValidationContext<'a>` :57 — 验证上下文
- `SessionControlValidationResult` :63 — 验证结果
- `SessionControlCancelRoute` :82 — 取消路由
- `validate_session_control_input` :176 — 验证输入
- `render_session_control_tool_use_message` :229 — 渲染工具使用消息

### 3.29 `session_state.rs` — 会话状态

- `SessionState` :8 — 会话状态枚举
- `ProcessingPhase` :32 — 处理阶段枚举
- `session_state_label_for_state` :41 — 状态标签

### 3.30 `session_state_manager.rs` — 会话状态管理器

- `SessionStateManager` :13 — 会话状态管理器（initialize/get_state/update_state/set_idle 等）

### 3.31 `side_question.rs` — 侧问题

- `SideQuestionRuntime` :9 — 侧问题运行时
- `ActiveBtwTurn` :15 — 活跃的回合间问答

### 3.32 `skill_agent_snapshot.rs` — Skill/Agent 快照

- `SkillSnapshotEntry` :8 — Skill 快照条目
- `AgentSnapshotEntry` :24 — Agent 快照条目
- `TurnSkillAgentSnapshot` :42 — 回合 skill/agent 快照
- `SkillAgentDiff` :56 — 快照差异（用于增量提示更新）
- `diff_skill_agent_snapshot` :144 — 计算快照差异
- `TurnSkillAgentSnapshotStore` :297 — 快照存储

### 3.33 `skills.rs` — Skill（源文件不存在）

### 3.34 `sdk.rs` — SDK 外观层

- `AGENT_RUNTIME_SDK_API_VERSION` :11 — SDK API 版本常量
- `AgentRuntimeSdkStability` :15 — 稳定性标记
- `AgentRuntimeSdkCompatibility` :21 — 兼容性信息
- `AgentRuntime` :95 — SDK 外观（包装 `crate::runtime::AgentRuntime`，暴露全部 50+ 方法）
- `AgentRuntimeBuilder` :106 — SDK 外观构建器

### 3.35 `thread_goal.rs` — 线程目标

- `GOAL_CONTINUATION_SUBMIT_RETRY_BASE_DELAY_MS` :18 — 重试基础延迟
- `GOAL_CONTINUATION_SUBMIT_RETRY_MAX_DELAY_MS` :19 — 最大重试延迟
- `effective_subagent_timeout_seconds` :32 — 有效子代理超时秒数
- `should_skip_goal_for_turn` :44 — 是否跳过目标执行
- `should_skip_goal_continuation_after_turn` :62 — 跳过后继执行
- `continuation_prompt` :106 — 继续提示
- `completion_budget_report` :231 — 完成预算报告
- `goal_tool_response` :231 — 目标工具响应
- `thread_goal_patch` :250 — 目标补丁
- `clear_thread_goal_patch` :256 — 清除目标补丁
- `ThreadGoalRuntimeError` :354 — 运行时错误
- `SetThreadGoalRequest` :370 — 设置目标请求
- `build_set_thread_goal_result` :381 — 构建设置结果
- `ThreadGoalRuntime` :473 — 目标运行时
- `ThreadGoalContinuationFacts<'a>` :668 — 继续事实
- `ThreadGoalContinuationOutcome` :676 — 继续结果

### 3.36 `thread_goal_tools.rs` — 线程目标工具

- `GET_GOAL_TOOL_NAME` :9 — "get_goal" 工具名常量
- `CREATE_GOAL_TOOL_NAME` :10 — "create_goal" 工具名常量
- `UPDATE_GOAL_TOOL_NAME` :11 — "update_goal" 工具名常量
- `CreateGoalArgs` :15 — 创建目标参数
- `UpdateGoalArgs` :22 — 更新目标参数
- `GoalToolResult` :27 — 目标工具结果
- `ThreadGoalToolError` :33 — 工具错误
- `parse_create_goal_args` :49 — 解析创建参数
- `parse_update_goal_args` :55 — 解析更新参数
- `parse_update_goal_status` :61 — 解析状态更新
- `build_goal_tool_result` :71 — 构建工具结果

### 3.37 `turn_cancellation.rs` — 回合取消

- `DialogTurnCancellationTokenStore` :8 — 对话回合取消令牌存储

### 3.38 `user_questions.rs` — 用户问答

- `QuestionOption` :11 — 问题选项
- `Question` :17 — 问题
- `AskUserQuestionInput` :26 — 问用户输入
- `UserQuestionToolResult` :31 — 工具结果
- `UserInputResponse` :37 — 用户输入响应
- `UserInputSendError` :42 — 发送错误
- `UserInputManager` :49 — 用户输入管理器
- `USER_INPUT_MANAGER` :113 — 全局静态实例
- `ask_user_question_available_for_acp_transport` :124 — ACP 传输中是否可用
- `validate_ask_user_question_input` :136 — 验证输入

---

## 4. events (`src/crates/contracts/events/src/`)

### 4.1 `lib.rs` — Crate root

- `pub mod agentic` — 代理事件定义
- `pub mod backend` — 后端事件
- `pub mod emitter` — 事件发射器
- `pub mod frontend_projection` — 前端投影
- `pub mod types` — 类型基础
- 重导出 `AgenticEvent` / `AgenticEventEnvelope` / `EventEmitter` / `AgenticFrontendEvent` 等

### 4.2 `agentic.rs` — 核心代理事件

- `AgenticEventPriority` :7 — 事件优先级枚举
- `SubagentParentInfo` :15 — 子代理父信息
- `DeepReviewQueueStatus` :27 — 深度审查队列状态枚举
- `DeepReviewQueueReason` :35 — 队列原因枚举
- `DeepReviewQueueState` :45 — 队列状态
- `AgenticEvent` :70 — 核心事件枚举（ToolExecution/UserInteraction/SessionLifecycle/DeepReview/Background/ThreadGoal 等 ~30 变体）
- `ToolEventIdentity` :348 — 工具事件身份
- `ToolEventData` :389 — 工具事件数据枚举
- `AgenticEventEnvelope` :498 — 事件信封（含时间戳/优先级/发射源）

### 4.3 `backend.rs` — 后端事件

- `ToolExecutionStartedInfo` :4 — 工具执行开始信息
- `ToolExecutionProgressInfo` :16 — 工具执行进度信息
- `ToolTerminalReadyInfo` :25 — 终端就绪信息
- `BackgroundCommandLifecycleInfo` :32 — 后台命令生命周期
- `ToolExecutionCompletedInfo` :47 — 工具执行完成信息
- `ToolExecutionErrorInfo` :57 — 工具执行错误信息

### 4.4 `emitter.rs` — 事件发射器

- `EventEmitter` trait :11 — 事件发射器 trait（emit/emit_lsp/emit_profile/emit_file_watch/emit_terminal/emit_snapshot）
- `NullEmitter` :97 — 空实现（测试/禁用用）
- `LoggingEmitter` :109 — 日志实现

### 4.5 `frontend_projection.rs` — 前端投影

- `AgenticFrontendEvent` :11 — 前端事件投影结构
- `project_agentic_frontend_event` :25 — 投影函数（AgenticEvent → AgenticFrontendEvent）

### 4.6 `types.rs` — 类型基础

- `EventPriority` :8 — 事件优先级枚举

---

## 5. runtime-ports (`src/crates/contracts/runtime-ports/src/`)

### 5.1 `lib.rs` — 核心端口定义（3358 行）

**错误类型：**

- `PortErrorKind` :37 — 端口错误种类枚举（NotFound/PermissionDenied/Timeout/InternalError 等 20+ 变体）
- `PortError` :49 — 端口错误（含 kind + retryable）
- `PortResult<T>` — 类型别名 `Result<T, PortError>`

**能力枚举：**

- `RuntimeServiceCapability` :65 — 运行时服务能力枚举（14 变体：TerminalPort/RemoteExecPort/FileSystemPort/WorkspacePort/SessionStorePort/ClockPort/NetworkPort/GitPort/McpCatalogPort/RemoteConnectionPort/RemoteWorkspacePort/RemoteProjectionPort/RemoteCapabilityPort/AgentSessionManagementPort）

**端口 trait：**

- `RuntimeServicePort` trait :100 — 顶层端口聚合（ability_label / capability_availability）
- `FileSystemPort` trait :115 — 文件系统端口（read/write/delete/mkdir/list/stat/move/exists/unzip/grep/git-worktree 等 ~25 方法）
- `WorkspacePort` trait :165 — 工作区端口（workspace_path / shell_type / host_name / os / arch / file_system / shell）
- `SessionStorePort` trait :184 — 会话存储端口
- `ClockPort` trait :194 — 时钟端口
- `TerminalPort` trait :198 — 终端端口（exec / kill）
- `RemoteExecPort` trait :206 — 远程执行端口
- `NetworkPort` trait :215 — 网络端口
- `GitPort` trait :225 — Git 端口
- `McpCatalogPort` trait :233 — MCP 目录端口
- `RemoteConnectionPort` trait :241 — 远程连接端口
- `RemoteWorkspacePort` trait :247 — 远程工作区端口
- `RemoteProjectionPort` trait :253 — 远程投影端口
- `RemoteCapabilityPort` trait :259 — 远程能力端口

**工作区类型：**

- `WorkspaceFileSystem` :280 — 工作区文件系统（path + FileSystemPort）
- `WorkspaceShell` :290 — 工作区 Shell（shell_type + host_name）
- `WorkspaceServices` :302 — 工作区服务聚合

**终端执行类型：**

- `TerminalExecRequest` ~310 — 终端执行请求
- `TerminalExecResponse` — 终端执行响应
- `RemoteExecRequest` — 远程执行请求
- `RemoteExecResponse` — 远程执行响应

**工具运行时类型：**

- `ToolRuntimeHandles` :350 — 工具运行时句柄集合
- `DynamicToolDescriptor` :360 — 动态工具描述符
- `DynamicToolProvider` trait :370 — 动态工具提供者 trait
- `ToolDecorator` trait :378 — 工具装饰器 trait

**代理会话管理类型：**

- `AgentSessionCreateRequest` :390 — 创建会话请求
- `AgentSessionCreateResult` :405 — 创建结果
- `AgentSessionListRequest` :415 — 列表请求
- `AgentSessionSummary` :430 — 会话摘要
- `AgentSubmissionRequest` :445 — 代理提交请求
- `DialogTurnRequest` :460 — 对话回合请求
- `AgentSessionManagementPort` trait — 代理会话管理端口
- `AgentSubmissionPort` trait — 代理提交端口
- `AgentDialogTurnPort` trait — 对话回合端口

**线程目标类型（也定义在此）：**

- `ThreadGoal` — 线程目标（objective/token_budget/status）
- `ThreadGoalStatus` — 目标状态枚举
- `ThreadGoalToolResponse` — 工具响应

**会话转录：**

- `SessionTranscriptReaderPort` trait — 会话转录读取端口

### 5.2 `local_workspace_snapshot.rs` — 本地工作区快照

- `LocalWorkspaceSnapshotSessionRequest` :12 — 会话级请求
- `LocalWorkspaceSnapshotTurnRequest` :17 — 回合金请求
- `LocalWorkspaceSnapshotStats` :24 — 快照统计
- `LocalWorkspaceSnapshotPort` trait :33 — 快照端口

### 5.3 `permission.rs` — 权限存储端口 (cfg gated)

- `PermissionGrantStorePort` trait :9 — 权限授权存储端口
- `PermissionAuditStorePort` trait :21 — 权限审计存储端口
- `PermissionReplyStorePort` trait :32 — 权限回复存储端口

### 5.4 `plugin.rs` — 插件运行时端口（~1100 行）

**核心枚举/结构体（选要）：**

- `PluginRuntimeUnavailableReason` :8 — 运行时不可用原因
- `ExtensionCapabilityAvailability` :35 — 扩展能力可用性（disabled/projection_only/executable）
- `PluginSourceKind` :67 — 来源类型
- `PluginTrustLevel` :74 — 信任级别
- `PluginSourceRef` :84 — 来源引用
- `PluginManifestRef` :98 — 清单引用
- `PluginConfigValidationStatus` :107 — 配置验证状态
- `PluginConfigValidationIssue` :117 — 验证问题
- `PluginConfigValidationState` :125 — 验证状态
- `PluginStatusKind` :133 — 状态类型
- `PluginStatusSnapshot` :146 — 状态快照
- `PluginOwnerKind` :161 — 所有者类型
- `PluginOwnerRef` :170 — 所有者引用
- `PluginCapabilityRef` :177 — 能力引用
- `PluginTargetRef` :184 — 目标引用
- `PluginAuditRef` :194 — 审计引用
- `PluginArtifactRef` :202 — 制品引用
- `PluginDataClassification` :212 — 数据分类
- `PluginPayloadRedaction` :222 — 载荷脱敏级别
- `PluginPayloadRef` :231 — 载荷引用
- `PluginRiskLevel` :242 — 风险级别
- `PermissionPromptEffectKind` :251 — 提示效果类型
- `PluginRollbackMode` :258 — 回滚模式
- `PluginRollbackPolicy` :267 — 回滚策略
- `PermissionPromptDenyState` :275 — 拒绝状态
- `PermissionPromptDescriptor` :286 — 提示描述符
- `PluginPermissionGate` :302 — 权限门控
- `PluginEffectCandidatePayload` :322 — 效果候选载荷
- `PluginEffectCandidate` :336 — 效果候选
- `PluginDiagnosticSeverity` :350 — 诊断严重级别
- `PluginDiagnosticDetail` :359 — 诊断详情
- `PluginDiagnostic` :393 — 诊断
- `PluginQuarantineScope` :406 — 隔离范围
- `PluginQuarantineReason` :447 — 隔离原因
- `PluginQuarantineClearCondition` :458 — 隔离解除条件
- `PluginQuarantineState` :465 — 隔离状态
- `PluginHostLifecyclePhase` :481 — 主机生命周期阶段
- `PluginRuntimeEpochs` :494 — 运行时纪元
- `PluginRuntimeReadRequest` :504 — 读取请求
- `PluginRuntimeReadResponse` :516 — 读取响应
- `PluginDispatchEnvelope` :531 — 分发信封
- `PluginResponseEnvelope` :553 — 响应信封
- `PluginRuntimeClient` trait :573 — 插件运行时客户端
- `validate_plugin_runtime_read_response` :593 — 验证读取响应
- `validate_plugin_dispatch_response` :635 — 验证分发响应
- `DisabledPluginRuntimeClient` :969 — 禁用时的空实现
- `ProjectionOnlyPluginRuntimeClient` :1016 — 仅投影实现
- `PluginRuntimeBinding` :1095 — 运行时绑定枚举

### 5.5 `script_tool.rs` — 脚本工具运行时

- `ScriptToolRuntimeAvailability` :10 — 可用性枚举
- `ScriptToolLoadRequest` :17 — 加载请求
- `ScriptToolExpectedExport` :28 — 预期导出
- `ScriptToolDescriptor` :35 — 描述符
- `ScriptToolLoadResponse` :44 — 加载响应
- `ScriptToolInvokeRequest` :52 — 调用请求
- `ScriptToolInvokeResponse` :68 — 调用响应
- `ScriptToolRuntime` trait :73 — 脚本工具运行时端口

### 5.6 `tool_permissions.rs`（bitfun_product_domains，cfg gated）

- `PermissionEffect` :14 — 权限效果枚举（allow/deny/ask）
- `PermissionRule` :22 — 权限规则
- `PermissionRuleset` :43 — 类型别名 `Vec<PermissionRule>`
- `PermissionRuntimeCeiling` :50 — 运行时天花板
- `PermissionRuntimeCeilingValidationError` :90 — 验证错误
- `PermissionPolicyPreset` :111 — 策略预设枚举
- `PermissionPolicyConfig` :150 — 策略配置
- `PermissionInteractionConfig` :158 — 交互配置
- `ToolPermissionConfig` :165 — 工具权限配置
- `PermissionPolicyLayers` :178 — 策略层
- `ChildPermissionPolicyLayers` :192 — 子策略层
- `resolve_permission_policy` :203 — 解析权限策略
- `resolve_child_permission_policy` :217 — 解析子策略
- `PermissionRequestSourceKind` :233 — 请求来源类型
- `PermissionRequestSource` :241 — 请求来源
- `PermissionDelegationContext` :255 — 委派上下文
- `PermissionRequest` :272 — 权限请求
- `PermissionReply` :317 — 权限回复
- `PermissionReplySource` :327 — 回复来源
- `PermissionRequestEvent` :336 — 请求事件
- `PermissionGrant` :358 — 授权
- `PermissionGrantKey` :377 — 授权键
- `PermissionAuditEvent` :386 — 审计事件
- `PermissionAuditRecord` :399 — 审计记录
- `PermissionResourceCaseSensitivity` :409 — 资源大小写敏感
- `PermissionEvaluator` :416 — 权限评估器
- `wildcard_matches` :513 — 通配符匹配

---

## 附录：模块缺失说明

- `agent-runtime/src/deep_review.rs` — 在 `lib.rs` 中声明但源文件不存在（仅有测试文件 `tests/deep_review_policy_contracts.rs`）
- `agent-runtime/src/skills.rs` — 在 `lib.rs` 中声明但源文件不存在
