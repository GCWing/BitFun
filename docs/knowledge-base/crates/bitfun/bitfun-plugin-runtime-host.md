# bitfun-plugin-runtime-host

**路径**: src/crates/execution/plugin-runtime-host
**描述**: Plugin Runtime Host boundary for BitFun extension execution。插件运行时宿主边界。

## 模块

- `adapter` — PluginHostAdapter trait

## 核心类型

- `PluginRuntimeHost` — 插件运行时宿主主体
  - 实现 `PluginRuntimeClient` trait（read_plugins / dispatch）
  - 状态管理：`PluginRuntimeHostState`（缓存调度、诊断、隔离、已 dispose 域）
  - 缓存：最近 256 个调度的响应缓存（DispatchCacheKey）
  - 隔离（quarantine）：按 domain 的插件隔离机制
  - 锁：按 (domain, plugin) 粒度的调度锁
  - Dispose：显式清理已 dispose 的 project workspace
  - Restart：重启时清理缓存和隔离状态
- `PluginHostAdapter` trait — 宿主适配器（dispatch / read_plugins / adapter_id）

## 内部类型

- `ExecutionDomainKey` — 执行域标识（project_domain_id + workspace_id）
- `DispatchCacheKey` — 调度缓存键（基于 envelope 所有字段哈希）
- `PluginDispatchLockKey` — 调度锁键（按 domain + plugin 粒度）
- `QuarantineCacheKey` — 隔离缓存键
- `PluginRuntimeHostState` — 内部状态（缓存、隔离、诊断、已 dispose 域）
- `StoredQuarantine` — 存储的隔离状态

## 功能

插件运行时宿主 crate。管理插件的调度生命周期，包括：基于 envelope 内容的响应缓存、按 domain/plugin 粒度的并发锁、失败/超时/无效请求的自动隔离（quarantine）、domain dispose/restart 生命周期管理。适配器模式——通过 PluginHostAdapter trait 接入具体插件后端。是扩展性基础设施的重要组成部分。
