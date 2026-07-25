# bitfun-runtime-services

**路径**: src/crates/execution/runtime-services
**描述**: Typed runtime service assembly for BitFun runtimes。类型化的运行时服务组装。

## 模块

- `backend_events` — 后端事件
- `test_support` — 测试支持

## 核心类型

- `RuntimeServices` — 运行时服务聚合体
  - 必要服务：filesystem, workspace, session_store, events, clock
  - 可选服务：terminal, remote_exec, network, git, mcp_catalog, remote_connection, remote_workspace, remote_projection, remote_capabilities
  - `has_capability(capability)` — 查询能力是否可用
  - `require_capability(capability)` — 要求能力可用
- `RuntimeServicesBuilder` — 构建器模式
  - `with_filesystem`, `with_workspace`, `with_session_store`, `with_events`, `with_clock` — 必要服务
  - `with_optional_*` — 可选服务
  - `build()` — 构建并校验能力匹配
- `RuntimeServicesRegistry` — 注册表（组织多个 provider 组合）
  - `with_provider(provider)` — 注册 `RuntimeServicesProvider`
  - `build(builder)` — 依次应用 provider 并构建
- `RuntimeServicesProvider` trait — 服务提供者（register 方法）
- `RuntimeServiceMarkerPort` — 标记 port（用于无具体实现的 Network/Git/McpCatalog port）
- `CapabilityAvailability` — 能力可用性
- `RuntimeServicesError` — 错误枚举（MissingRequired, Unsupported, CapabilityMismatch）

## 功能

运行时服务组装 crate。定义 RuntimeServices 作为所有运行时服务的统一聚合体，提供构建器、注册表和 provider 模式。5 个必要服务（filesystem, workspace, session_store, events, clock）和 9 个可选服务（terminal, remote_exec, network, git, mcp_catalog, remote_connection 等）。被 product-capabilities 用于服务可用性校验。
