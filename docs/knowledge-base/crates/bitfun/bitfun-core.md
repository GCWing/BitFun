# bitfun-core

**路径**: src/crates/assembly/core
**描述**: BitFun Core Library - Platform-agnostic business logic。兼容性外观与完整产品运行时组装。

## 模块

- `agentic` — Agent 系统、工具系统、产品运行时编排（feature: product-full）
- `external_hooks` — 外部 hooks（feature: product-full）
- `external_mcp` — 外部 MCP（feature: product-full）
- `external_sources` — 外部源（feature: product-full）
- `external_subagents` — 外部子 agent
- `external_tools` — 外部工具（feature: product-full）
- `function_agents` — 函数式 agent（feature: product-domains）
- `infrastructure` — AI 客户端、存储、日志、事件
- `miniapp` — AI 生成的即时应用（feature: product-domains）
- `plugin_runtime` — 插件运行时（feature: product-full）
- `plugin_source` — 插件源（feature: plugin-source / product-domains）
- `product_assembly` — 产品组装（feature: product-full）
- `product_domain_runtime` — 产品域运行时（feature: product-domains）
- `product_runtime` — 产品运行时（feature: product-full）
- `service` — Workspace/Config/FileSystem/Terminal/Git 实现
- `service_agent_runtime` — 服务层 agent 运行时（feature: service-integrations）
- `util` — 通用类型、错误、辅助函数

## 核心类型（重新导出）

- `Message`, `Session` — 核心消息/会话
- `AgenticEvent`, `EventQueue`, `EventRouter` — 事件系统
- `ExecutionEngine`, `StreamProcessor` — 执行引擎
- `Tool`, `ToolPipeline`, `ToolRegistry` — 工具系统
- `ConfigManager`, `ConfigService` — 配置管理
- `WorkspaceManager`, `WorkspaceProvider`, `WorkspaceService` — Workspace 服务
- `AIClient` — AI 客户端（feature: ai-adapter-runtime）
- `BackendEventManager` — 后端事件管理器
- `VERSION`, `CORE_NAME` — 版本信息

## 功能

最大的组装 crate。依赖几乎全部内部 crate（contracts/execution/services/adapters/assembly），通过 feature 选择组装完整产品运行时。是产品的核心业务逻辑入口，同时也是遗留兼容层（新代码应写入 owner crate）。重新导出 runtime-ports、util 和服务层类型。
