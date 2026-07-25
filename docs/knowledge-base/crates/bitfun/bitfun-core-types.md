# bitfun-core-types

**路径**: src/crates/contracts/core-types
**描述**: BitFun shared low-level product DTOs — 轻量 DTO crate，不依赖 runtime、network、platform 或 product assembly。

## 模块

- `ai` ��� AI 配置、模型信息、ToolCall/定义
- `errors` — AI 错误详情、错误分类
- `lsp` — LSP 相关类型
- `session` — Session DTO（配置、种类、策略）
- `session_tree` — Session 树结构
- `session_usage` — Session 用量统计
- `speech` — 语音识别类型
- `surface` — Surface/平台类型（审批源、权限、运行时工件）
- `tool_image_attachment` — Tool 图片附件

## 核心类型

- `AIConfig`, `ProxyConfig`, `ReasoningMode` — AI 配置
- `Message`, `ToolCall`, `ToolDefinition` — 消息与工具定义
- `ToolCallConfirmationDetails`, `ToolCallRequestInfo`, `ToolCallResponseInfo` — 工具调用详情
- `ConnectionTestResult`, `ConnectionTestMessageCode`, `RemoteModelInfo` — 连接测试
- `AiErrorDetail`, `ErrorCategory` — 错误类型
- `SessionContinuationPolicy`, `SessionKind`, `SessionModelBindingPolicy` — Session 策略
- `SurfaceKind`, `ThreadEnvironment`, `ThreadEnvironmentKind` — 平台环境
- `ApprovalSource`, `CapabilityRequest`, `CapabilityRequestKind` — ���批与能力请求
- `PermissionDecision`, `PermissionScope`, `PermissionRule` — 权限决策
- `RuntimeArtifactKind`, `RuntimeArtifactRef` — 运行时工件引用
- `ToolImageAttachment` — 工具图片附件

## 功能

最底层的 DTO crate，定义 AI 会话、工具调用、权限、语音、Surface 等基础数据类型。被几乎所有其他 crate 引用。
