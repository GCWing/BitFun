# bitfun-runtime-ports

**路径**: src/crates/contracts/runtime-ports
**描述**: Thin runtime ports for BitFun core decomposition。只包含 DTO 和 trait，不依赖具体 manager、platform adapter、bitfun-core 或 app crate。

## 模块

- `local_workspace_snapshot` — 本地 Workspace 快照 port
- `permission` — 权限存储 port（feature: permission）
- `plugin` — 插件运行时契约
- `script_tool` — 脚本工具运行时

## 核心类型

### Port 基础
- `PortError`, `PortErrorKind`, `PortResult<T>` — 标准 port 错误类型
- `RuntimeServiceCapability` — 运行时服务能力枚举（FileSystem, Workspace, SessionStore, Terminal, Git 等 14 种）
- `RuntimeServicePort` trait — 所有 port 的基础 trait
- `AgentType` — agent 类型枚举（Agentic/Plan/Cowork/Other）

### Workspace
- `WorkspaceFileSystem` trait — 统一文件系统操作（read/write/exists/read_dir）
- `WorkspaceShell` trait — 统一 Shell 执行（exec/exec_with_options）
- `WorkspaceCommandOptions`, `WorkspaceCommandResult` — 命令选项/结果
- `WorkspaceDirEntry`, `WorkspaceServices` — 目录项 / 服务包

### Terminal
- `TerminalPort` trait — 终端执行、stdin 写入、会话控制
- `TerminalExecCommandRequest/Response`, `TerminalWriteStdinRequest`, `TerminalExecControlRequest`

### Remote Exec
- `RemoteExecPort` trait — 远程 shell 执行
- `RemoteExecOneShotCommandRequest/Response`, `RemoteExecCommandRequest/Response`

### Session
- `SessionStorePort` trait — Session 存储路径解析
- `AgentSubmissionPort` trait — Session 创建与消息提交
- `AgentSessionManagementPort` trait — Session 列表/删除/重命名/归档
- `AgentSessionClosePort` trait — 临时 Session 清理
- `AgentSessionModelPort`, `AgentSessionModePort` traits — Session 模型/模式更新
- `AgentLocalCommandTurnPort` trait — 本地命令记录
- 大量 Session 相关 DTO（Create/List/Delete/Rename/Archive/Fork/Submission/DialogTurn 等）

### Plugin
- `PluginRuntimeClient` trait — 插件运行时客户端
- `PluginDispatchEnvelope`, `PluginResponseEnvelope` — 插件调度信封
- `PluginStatusSnapshot`, `PluginQuarantineState`, `PluginRiskLevel`, `PluginTrustLevel` — 插件状态
- `DisabledPluginRuntimeClient`, `ProjectionOnlyPluginRuntimeClient` — 桩实现

### Script Tool
- `ScriptToolRuntime` trait — 脚本工具加载与调用
- `ScriptToolDescriptor`, `ScriptToolLoadRequest/Response`, `ScriptToolInvokeRequest/Response`

### Thread Goal
- `ThreadGoal`, `ThreadGoalStatus` — 线程目标
- `DialogRoundInjectionSource` trait — 轮次注入观察
- `RoundInjection`, `RoundInjectionKind`, `RoundInjectionExecutionPolicy` — 轮次注入控制

### Remote
- `RemoteWorkspaceRuntimeHost` trait — 远程 workspace 命令
- `RemoteInitialSyncRuntimeHost` trait — 远程初始同步
- `RemoteWorkspaceFileRuntimeHost` trait — 远程文件投影
- `RemoteWorkspacePort`, `RemoteProjectionPort`, `RemoteCapabilityPort` — 注册边界 trait
- `RemoteWorkspaceFacts`, `RemoteRecentWorkspaceFacts`, `RemoteSessionMetadata` — 远程 DTO

### 其他 Port
- `ClockPort`, `FileSystemPort`, `WorkspacePort`, `NetworkPort`, `GitPort`, `McpCatalogPort`, `RemoteConnectionPort`
- `ToolRuntimeHandles` — 工具执行上下文句柄
- `EventEmitter` trait — 事件发送

## 功能

核心 Port/契约 crate。定义运行时服务边界的所有 trait 和 DTO，包括文件系统、Shell、终端、Session 管理、远程执行、插件运行时、权限存储、线程目标、脚本工具等。不依赖具体实现，作为解耦边界被几乎所有上层 crate 依赖。
