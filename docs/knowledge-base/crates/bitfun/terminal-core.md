# terminal-core

**描述**: 独立的终端模块，提供完整的 PTY 终端实现。

**包名**: `terminal-core` | lib: `terminal_core`

## 核心模块

| 模块 | 说明 |
|------|------|
| `pty` | PTY 进程管理、数据缓冲、流控制 |
| `session` | 终端会话生命周期、绑定、回放历史 |
| `shell` | Shell 检测、集成脚本、配置管理 |
| `config` | 终端配置类型和默认值 |
| `events` | 前端通信事件定义和事件发射器 |
| `api` | 外部消费者公共 API |
| `exec` | 一次性执行进程管理 |
| `exec_shell` | Shell 解析和执行配置 |
| `runtime_port` | 运行时端口适配 |
| `transcript` | 终端转录 |

## 关键类型/功能

- `TerminalApi` — 终端外部 API
- `PtyService` / `PtyController` / `PtyWriter` — PTY 控制
- `SessionManager` / `TerminalSession` — 会话管理
- `ShellDetector` / `ShellIntegration` / `ShellProfile` — Shell 集成
- `CommandStream` / `CommandStreamEvent` — 命令流
- `TerminalReplayHistory` / `TerminalReplayEvent` — 回放
- `ExecProcessManager` — 一次性执行管理
- `TerminalEventEmitter` / `TerminalEvent` — 事件系统
- `TerminalResult<T>` / `TerminalError` — 错误类型

## 关键依赖

- `portable-pty` — 跨平台 PTY
- `bitfun-runtime-ports` — 运行时端口契约
- `chardetng` / `encoding_rs` — 字符编码检测

## 一句话总结

完整的跨平台终端实现，提供 PTY 管理、会话生命周期、Shell 集成和命令执行能力。
