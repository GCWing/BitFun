# 终端能力（Terminal Capability）需求文档

> 状态：能力规格 / 需求
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`src/crates/services/terminal/AGENTS.md`](../../src/crates/services/terminal/AGENTS.md)
> - [`docs/architecture/agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
> - [`src/crates/assembly/core/AGENTS.md`](../../src/crates/assembly/core/AGENTS.md)

## 背景与需求描述

BitFun Agent 要"进入真实环境"完成任务，必须能在用户工作区里驱动 shell：执行命令、读输出、发信号、取消进程。终端能力是 Agent 桌面执行层的基础设施之一，与文件系统、Git、浏览器操作并列。

本需求定义**终端能力**（区别于"内置终端面板 UI"——见 `builtin-terminal-panel.md`）：即可被 Agent 工具与产品宿主复用的 PTY / 会话 / shell 集成服务，以及 Agent 直接调用的执行类工具。

当前缺口与诉求：

- 终端能力应以**独立、平台无关的服务 crate** 形式存在，不耦合 `bitfun-core` / Tauri / 产品域 / AI / Git / MCP / transport；
- Agent 执行命令需稳定契约：创建会话、写 stdin、读输出、resize、信号、取消、关闭，跨 Windows / macOS / Linux 行为一致；
- 远程工作区场景下，终端会话应在受控端执行，控制端只发起与展示，不静默失败；
- 进程生命周期须受控回收（受管子进程清理），不残留孤儿进程；
- 转写（transcript）须可持久化与回放，并做路径 / 命令脱敏。

## 期望行为

### 1. PTY 与会话生命周期

- 创建 PTY 会话（`CreateSessionRequest`），分配 shell，产出会话 id；
- 写入（`WriteRequest` / `SendCommandRequest`）、读输出（事件流）、resize（`ResizeRequest`）、信号（`SignalRequest`）、关闭（`CloseSessionRequest`）；
- 会话生命周期事件经 `TerminalEvent` / `TerminalEventEmitter` 投影到前端；
- 会话可持久化与回放（`session/replay`、`session/persistent`），冷启动后可恢复。

### 2. Shell 检测与集成

- 跨平台 shell 检测（Windows / macOS / Linux，`shell/detection`）：探测、选择、缓存；
- shell 集成脚本注入（`shell/integration`、`shell/scripts_manager`、`shell/profiles`），支持命令补全提示、当前目录上报、退出码；
- 用户可配置 shell 偏好（`parse_configured_shell_preference`、`resolve_local_exec_shell`）。

### 3. Agent 执行工具

- `BashTool` / `ExecCommand` / `WriteStdin` / `ExecControl`（属 `core.basic` 工具组）供 Agent 调用；
- 执行支持后台化、stdin 写入、取消（`ExecControlAction`）、完成态（`ExecSessionCompletion`）；
- 进程树受管清理（`services-core::process_tree`）：Unix 进程组、Windows Job Object 兜底，明确这是生命周期收容而非 OS 沙箱。

### 4. 转写与脱敏

- 转写根目录可配置，`None` 禁用录制；
- 转写支持回放、截断保序、跨重启恢复；
- 转写与日志遵循脱敏：路径、命令、凭据按既有 redaction 规则。

### 5. 远程工作区

- 远程控制另一台桌面时，终端会话在受控端执行；
- 在 `remote_workspace_policy` 中声明终端相关命令的远程策略（远程路由 / 本地 / 不支持），不支持时清晰提示；
- 远程 SSH 终端属 `remote-ssh` owner，不在本需求。

### 6. 跨平台与鸿蒙

- 基础终端能力平台无关，Windows / macOS / Linux shell 兼容；
- 鸿蒙 PC CLI/TUI 的 PTY / 信号 / `/dev` 语义属未来专题（见 `platform-portability-design.md`），不在本需求承诺范围；鸿蒙不可用时显式 unsupported。

## 非目标 / 范围外

- 不在本需求内定义内置终端面板 UI（见 `builtin-terminal-panel.md`）；
- 不覆盖 CLI/TUI 形态的终端（ratatui/crossterm，属 `src/apps/cli`）；
- 不定义远程 SSH 终端协议（属 `remote-ssh` owner）；
- 不在本需求内做终端外观 / 主题（属 appearance）；
- 不替换 `BashTool` / `ExecCommand` 既有契约。

## 建议的落地路径（基于现有分层）

1. **Contracts (`src/crates/contracts`)** — 终端 DTO / port：会话 id、请求 / 响应、事件形态、转写契约，行为轻量。
2. **Services (`src/crates/services/terminal`，包名 `terminal-core`)** — PTY（`pty`）、会话（`session`）、shell（`shell`）、配置（`config`）、事件（`events`）、转写（`transcript`）、exec（`exec` / `exec_shell`）。边界：不依赖 core / Tauri / 产品域 / AI / Git / MCP / transport；平台差异在 terminal 抽象后。
3. **Assembly / Core (`src/crates/assembly/core`)** — `BashTool` / `ExecCommand` / `WriteStdin` / `ExecControl` 工具实现与 product runtime 装配；远程路由。
4. **App / UI** — 宿主注册终端服务、暴露 Tauri command；内置面板见 `builtin-terminal-panel.md`。

### 分层与依赖边界要点

- 终端能力平台无关：PTY / 会话 / shell / 转写决策在 `terminal-core`，平台无关；具体宿主命令与 UI 在上层；
- `terminal-core` 不依赖 `bitfun-core` / app / Tauri；
- 进程清理是生命周期收容，不是 OS 沙箱或资源限额，须在表面保持显式残余风险；
- 远程策略显式声明，不引入对远程工作区路径的隐式依赖。

## 设计草案 / 参考示例

- **模块布局参考**：`terminal-core` 的 `pty` / `session` / `shell` / `config` / `events` / `transcript` / `exec` 分层。
- **shell 检测参考**：`shell/detection`（probe / selection / platform / path / cache）跨平台兼容。
- **转写参考**：`transcript.rs` 的录制 / 回放 / 截断 / 跨重启恢复与脱敏。
- **进程清理参考**：`services-core::process_tree` 的 Unix 进程组与 Windows Job Object 兜底。
- **远程参考**：`remote_workspace_policy` 显式声明终端命令远程策略。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 与 `builtin-terminal-panel.md` 的关系：本需求定义终端**服务 / 能力**，后者定义**应用内面板 UI**；面板消费本服务。
- 与远程工作区的关系：终端会话远程路由在 `remote_workspace_policy` 显式声明，不支持时清晰提示。
- 相关分层入口：`src/crates/contracts/AGENTS.md`、`src/crates/services/terminal/AGENTS.md`、`src/crates/assembly/core/AGENTS.md`。
