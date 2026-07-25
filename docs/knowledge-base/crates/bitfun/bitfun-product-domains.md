# bitfun-product-domains

**路径**: src/crates/contracts/product-domains
**描述**: Product domain owner crate。产品子领域在不依赖完整 BitFun core runtime assembly 时可独立编译。

## 模块

- `canvas` — Canvas 领域
- `tool_permissions` — 工具权限策略（始终编译）
- `external_integration_policy` — 外部集成策略（feature: external-sources）
- `external_hook_contributions` — 外部 hook 贡献（feature: external-sources）
- `external_hook_catalog` — 外部 hook 目录（feature: external-sources）
- `external_source_control` — 外部源控制（feature: external-sources）
- `external_sources` — 外部源核心定义（feature: external-sources）
- `external_subagents` — 外部子 agent（feature: external-sources）
- `plugin_source` — 插件源（feature: plugin-source）
- `miniapp` — AI 生成的即时应用（feature: miniapp）
- `function_agents` — 函数式 agent（feature: function-agents）

## 核心类型

- `PermissionRuleset`, `PermissionRule`, `PermissionPolicyConfig`, `PermissionPolicyLayers` — 权限规则体系
- `PermissionEvaluator`, `PermissionGrant`, `PermissionGrantKey` — 权限评估器
- `PermissionRequest`, `PermissionRequestEvent`, `PermissionRequestSource` — 权限请求
- `PermissionEffect`, `PermissionReply`, `PermissionReplySource` — 权限效果与回复
- `PermissionAuditEvent`, `PermissionAuditRecord` — 权限审计
- `PermissionDelegationContext`, `PermissionInteractionConfig` — 权限委托
- `PermissionRuntimeCeiling`, `ToolPermissionConfig` — 运行时权限上限
- `EcosystemId`, `ProviderId`, `SourceKey`, `ExternalSourceCatalogEntry/Snapshot/Record` — 外部源类型
- `ExternalSourceHealth`, `ExternalSourceLifecycleState`, `ExternalSourceDiagnostic` — 外部源健康状态
- `PromptCommandDefinition`, `PromptCommandCatalogEntry`, `PromptCommandConflict` — 提示命令
- `PluginSourceRecord` — 插件源记录（plugin-source feature）
- MiniApp 相关类型（miniapp feature）

## 功能

产品领域 owner crate。核心是权限系统（tool_permissions）和外部源契约（external-sources 系列模块）。不依赖 bitfun-core，可通过 feature 选择编译子领域。权限类型通过 runtime-ports 重新导出供其他 crate 使用。
