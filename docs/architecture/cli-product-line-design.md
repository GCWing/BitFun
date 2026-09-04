# OpenBitFun CLI 产品线架构

本文记录 CLI 产品入口的稳定边界、当前运行路径和剩余架构缺口。它不是用户手册，也不记录单个 PR 的完成日志。

相关专题由各自文档负责：

- 仓库分层与产品边界：[`product-architecture.md`](product-architecture.md)
- Agent Runtime 服务归属：[`agent-runtime-services-design.md`](agent-runtime-services-design.md)
- Embedded / Shared 部署：[`agent-runtime-deployment-design.md`](agent-runtime-deployment-design.md)
- 公开 Agent SDK：[`agent-sdk-product-architecture.md`](agent-sdk-product-architecture.md)
- 产品定制：[`product-customization-blueprint.md`](product-customization-blueprint.md)
- 外部 AI 工作来源：[`extensions/external-ai-work-sources-design.md`](extensions/external-ai-work-sources-design.md)
- OpenCode 兼容矩阵：[`extensions/opencode-extension-compatibility.md`](extensions/opencode-extension-compatibility.md)
- 插件 Runtime：[`extensions/plugin-runtime-design.md`](extensions/plugin-runtime-design.md)
- Detached Dispatch：[`detached-task-dispatch.md`](detached-task-dispatch.md)
- 平台可移植性：[`platform-portability-design.md`](platform-portability-design.md)

设计文档中的目标能力不等于已交付能力。判断现状必须同时检查生产调用点、测试和当前 CLI 帮助。

## 1. 产品范围

OpenBitFun CLI 是独立的 Agent 产品入口，覆盖：

- 交互式 TUI
- 非交互 `exec` 与结构化输出
- Session、模型、Agent、MCP、Skill、Subagent 和诊断入口
- ACP 服务端、Peer Device Host、Detached Dispatch 和 Shared TUI 适配
- 产品组装结果在终端形态下的消费

CLI 不拥有 Session、Turn、Tool、Permission、Context、Workspace、MCP 或 Subagent 的产品逻辑。它也不通过 Fork 竞品 Runtime 来实现兼容。

竞品对齐只用于降低用户学习成本和补齐常用工程流程：

1. 等价入口优先采用 OpenCode 的命令名与交互。
2. Codex、Claude Code 和 OpenCode 的配置或扩展格式是外部来源，不是 OpenBitFun 内部模型。
3. 只有存在真实消费方、安全边界和兼容测试时才增加生态专属能力。
4. 不为“接口完整”发布没有运行路径的占位 API。

## 2. 分层与所有权

```text
CLI surface
  Clap / terminal lifecycle / TUI state / rendering / local effects
        |
        v
CLI adapters
  CliAgentRuntimeClient / Shared IPC / output projection / Peer host adapter
        |
        v
Agent Runtime SDK and typed owner ports
        |
        v
Core owners
  Session / execution / ToolPipeline / permissions / persistence / workspace
        |
        v
Platform services
  terminal / filesystem / git / network / remote execution
```

边界规则：

- CLI 可以决定“如何进入、如何展示、当前按键做什么”，不能重新决定产品事实。
- Adapter 负责协议与形态转换，不重新计算 owner 已经给出的状态。
- DTO 或 trait 提取只是依赖边界，不代表 Runtime owner 已迁移。
- 本地 UI effect 可以留在 CLI；会改变 Session、工作区或外部系统的操作必须进入共享 owner。
- Remote 不支持的本地能力必须返回明确 unsupported，不能静默在控制端本机执行。

## 3. 部署形态

### 3.1 Embedded

默认 TUI、`exec` 和一次性管理入口在当前进程组装 Runtime。一次 invocation 只构造一份产品 Runtime context，TUI、事件订阅和同一进程内的 Peer Host 复用它。

### 3.2 Shared TUI

`openbitfun chat --shared` 通过本机版本化 IPC 连接工作区 Runtime：

- 多个 TUI 可以复用一个工作区 Runtime。
- 一个 TUI 同时控制至多一个 Session；一个 Session 同时只有一个 controller。
- 有副作用的请求携带稳定 identity，并声明 controller、idle、序列化和 side-effect 规则。
- 超时或断线后无法证明是否已提交的请求返回 `OutcomeUnknown`，关闭连接并按已知 turn id 取消；客户端不能盲目重试。
- IPC 只暴露经过评审的闭集操作，不演变成通用 Tool 或 Core RPC。
- 不支持的附件或本地 effect 在 IPC 前失败。

Automation、Desktop、Server、Relay 和公开 SDK 不因 Shared TUI 自动改用同一协议。

## 4. 关键运行路径

### 4.1 普通对话

```text
ComposerDraft
  -> CliAgentRuntimeClient
  -> AgentRuntime::submit_dialog_turn
  -> Core Session / execution owner
  -> Agent events
  -> CLI ChatState and rendering
```

`ComposerDraft` 统一保存文本、结构化工作区引用和图片。附件只在提交时转换为 Runtime DTO；Shared TUI 当前不支持的图片在序列化前拒绝。

### 4.2 显式 Shell 输入

空 composer 键入首个 `!` 进入 SHELL，这是 OpenCode 的既有入口，不增加 `/shell`：

```text
SHELL composer
  -> CliAgentRuntimeClient::run_user_shell_command
  -> AgentUserShellCommandPort
  -> Core coordinator
  -> ToolPipeline(ExecCommand)
  -> TerminalPort or RemoteExecPort
  -> UserDialog + ModelRound + ToolResult
```

稳定语义：

- Shell mode 只是 UI 状态；命令执行不属于 CLI。
- 只允许 idle Session；命令使用 Session 已解析的本地或远程 workspace。
- 非交互、`tty=false`，不接受图片或结构化 `@` 引用。
- `/` 在 Shell mode 中是命令文本，不进入 slash registry。
- 显式用户命令自动处理交互式 `ask`，但 project/global/profile 的 `deny` 仍由 ToolPipeline 执行。
- 取消、工具事件、审计、上下文和持久化复用正常 Runtime 路径。
- 保存为普通 `UserDialog` 与 `ExecCommand` ModelRound，因此 CLI、Desktop 和恢复流程消费同一事实。

这不是通用 Tool SDK，不向 UI 暴露任意 Tool 调用，也不允许 CLI 直接 spawn 进程。

### 4.3 本地 UI effects

`/editor`、复制和导出只改变当前客户端状态或本地目标，不构造产品 turn。它们可以留在 CLI，但必须：

- 在活动 Runtime 操作之外执行，避免阻塞事件消费。
- 失败时保留原 draft 或明确报告部分结果。
- 不把本地 effect 伪装成 Shared/Remote 已支持的能力。
- 终端让渡后无论成功、失败或 panic 都恢复 TUI 状态。

### 4.4 非交互执行

`exec` 复用同一 Agent Runtime owner，不维护第二套 Agent loop。

- `text` stdout 只包含最终文本。
- `json` stdout 是一个最终对象。
- `stream-json` stdout 每行是一个完整 Agent event。
- 日志与诊断进入 stderr 或日志文件。
- 默认拒绝需要人工确认的操作；只有显式调用级策略可以自动批准。
- 非交互入口不等待人工确认，也不从全局外部来源状态推断特殊任务结果。能力不可用时返回普通失败；能够可靠归属到 Tool、Agent 或 MCP owner 时，错误只给出对应管理入口。
- 取消、事件失步、失败完成和 Patch 失败不能报告成功。

## 5. TUI 内部边界

TUI 增量保持四个可测试边界：

1. 输入归一化：终端事件、paste、resize、mouse。
2. 状态转换：composer、popup、history、selection、processing。
3. Effect：Runtime 请求、搜索、clipboard、editor、export。
4. 渲染：只读取状态，不访问文件系统、网络或 Agent owner。

Slash、Palette、Help、快捷键和 availability 从同一 Action Registry 派生。竞品已有等价入口时不自创命令；局部 UI 状态不进入 Agent Runtime contract。

终端恢复是强约束：正常退出、取消、初始化失败、错误返回和 panic 都要尽力恢复 raw mode、alternate screen、mouse/paste capture 与 cursor。

## 6. 配置、产品和外部来源

### 6.1 配置

CLI-local 配置只保存终端形态偏好与调用入口设置。共享权限、模型、Agent、MCP 和产品策略由各自 owner 解析。显式导入是快照操作，持续兼容来源是只读视图，两者不能共享“已启用”推断。

凭据发现、凭据使用和配置导入保持分离。日志、结构化输出、导入报告和诊断不能包含原始 token、header、secret 或不必要的绝对路径。

### 6.2 产品组装

CLI 通过 `DeliveryProfile::Cli` 消费经过校验的产品 Runtime parts。产品定义、Delivery Profile、Runtime Configuration 和 Capability Availability 是不同概念：

- 编译期由 CLI 显式选择 `agent-runtime` 生命周期基线、实际 service owner、
  `external-sources` / `plugin-runtime` / `ssh-remote` 和九组 `tools-*`；这保持现有
  CLI capability plan，但不再从 Core 基线暗带具体能力，也不继承 Desktop 后续加入
  `product-full` 的能力。
- 隐藏入口不证明后端依赖被移除。
- CLI 不读取 authoring product definition 作为运行时业务配置。
- 品牌、资源、数据 namespace、更新渠道和内置扩展由产品定制 owner 生成，CLI 只消费结果。

### 6.3 外部来源与插件

CLI 只消费 typed summary 与 typed action：

- `/extensions` 只提供外部应用/来源的简短状态、启停和刷新，不拥有审批、冲突或批量决策。
- `/tools`、`/agent`、`/mcp` 和 `/hooks` 是对应能力的直接管理入口；需要用户允许时由真实 owner 在该入口处理，不再增加跨能力复审流程。
- 静态发现不等于代码执行或服务健康。
- 配置导入不授予插件执行权限。
- ACP、MCP import、Hook import、可执行插件和 TUI contribution 使用独立状态与生命周期。
- 插件不能直接持有 Ratatui Frame、终端句柄、Session writer 或权限存储。

详细阶段和 OpenCode 能力映射由 extensions 专题文档维护，避免在本文件复制一套会漂移的矩阵。

## 7. 剩余架构工作

下列是演进方向，不代表已经排期：

| 方向 | 当前问题 | 下一步约束 |
| --- | --- | --- |
| Core 兼容面收敛 | 部分 Peer/ACP/维护操作仍通过兼容 facade | 只按真实调用方提取窄 port；保持旧路径等价测试，不把 DTO 提取描述成 owner migration。 |
| TUI 模块化 | 少数编排文件仍聚集输入、effect 与生命周期 | 沿现有模块增量提取，不重写 TUI，不复制状态机。 |
| Shared Runtime 闭集扩展 | 各形态能力仍有差异 | 逐操作评审 identity、lease、timeout、cancel 和 unsupported；不做通用 RPC。 |
| 配置来源解释 | 非 MCP 资产的来源/冲突解释仍不统一 | 共享 owner 给出 typed provenance；CLI 只展示，不建立第二套合并器。 |
| 插件真实执行 | 静态来源与真实可调用能力容易混淆 | 只有 worker 健康、定义已加载并注册到 Tool Runtime 后才显示 available。 |
| 平台验证 | Windows/macOS/Linux 的终端和进程行为不同 | 在对应切片增加 PTY/ConPTY、取消、路径和进程清理测试；HarmonyOS PC 以平台专题的真机证据为准。 |

架构优化优先减少 owner 重复、依赖方向和失败歧义，而不是增加抽象数量。没有第二个真实消费者时，不提升为通用平台 API。

## 8. 验证

| 变更范围 | 最小验证 |
| --- | --- |
| TUI state/input/render | focused reducer/input/render test + `cargo test -p openbitfun-cli` |
| Agent Runtime SDK/port | `cargo test -p openbitfun-agent-runtime --no-default-features --features agent-runtime --lib` + owner focused test |
| Shared IPC | protocol round-trip、controller/idle、timeout/outcome-unknown、disconnect cancel |
| Core turn/tool | 权限 allow/ask/deny、取消、事件、上下文、持久化恢复 |
| `exec` output | stdout/stderr、单一最终状态、session conflict、Ctrl+C、Patch |
| terminal lifecycle | Linux PTY / Windows ConPTY 对应 smoke 或 deterministic fixture |
| product/packaging | product assembly + archive 双入口和签名/摘要 smoke |

跨共享 Rust owner 的变更最终遵循仓库 `AGENTS.md` 的 workspace 验证矩阵。不能运行的检查必须在 PR 中说明原因、替代证据和剩余风险。
