# bitfun-services-core

**描述**: BitFun 核心服务 owner crate，提供平台无关的服务构建块。

**包名**: `bitfun-services-core` | lib: `bitfun_services_core`

## 核心模块

| 模块 | 说明 |
|------|------|
| `diagnostics` | 诊断日志脱敏（`redact_diagnostic_log_text`） |
| `diff` | 统一 diff 计算、合并、状态管理（`DiffService`） |
| `filesystem` | 平台中立的文件操作、目录遍历、文件树构建、搜索 |
| `json_store` | 通用 JSON 文件存储（带重试的原子写入） |
| `managed_runtime` | 托管运行时命令解析（`ManagedRuntimeResolver`） |
| `session` | 会话生命周期、元数据存储、内存工作区 diff |
| `session_usage` | 会话用量分类、脱敏、报告渲染 |
| `storage_cleanup` | 存储清理策略（临时/日志/缓存） |
| `process_manager` | 统一进程管理（防 Windows 子进程泄漏） |
| `process_tree` | 进程树监督（跨平台子进程清理） |
| `system` | 系统信息、命令检测与执行 |
| `token_usage` | Token 用量追踪与统计 |
| `persistence` | 数据持久化服务（带备份和锁） |
| `workspace_instructions` | 工作区指令文件（AGENTS.md / CLAUDE.md）读取 |
| `lsp` (feature) | LSP 插件注册、协议、项目检测、配置监视 |
| `markdown` (feature) | YAML front matter Markdown 解析/写入 |
| `permission_store` (feature) | SQLite 权限存储和审计 |
| `workspace` (feature) | 工作区运行时端口 |
| `local_runtime_ports` (feature) | 本地运行时端口实现 |

## 关键类型/功能

- `FileSystemService` — 文件系统操作服务
- `DiffService` — 文件差异计算
- `ProcessManager` / `ProcessTreeChild` — 进程管理
- `SessionMetadataPage` / `SessionMetadataStore` — 会话管理
- `ManagedRuntimeResolver` — 托管运行时可执行文件查找
- `TokenUsageService` — Token 用量统计
- `CleanupService` — 存储清理
- `ProjectPermissionSqliteStore` — 权限持久化
- `FrontMatterMarkdown` — YAML front matter 处理
- `LspPluginRegistry` — LSP 插件注册表

## 一句话总结

平台无关的核心服务基础设施层，提供文件系统、进程、会话、权限、LSP 等基础能力。
