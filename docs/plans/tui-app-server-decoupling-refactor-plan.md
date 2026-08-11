# TUI 与 App Server 解耦重构计划

> 状态：Phase 0-5 已完成当前定义；Shared TUI 已切换到 Shared App Server。
>
> 当前状态基线：2026-08-11。一次性运行证据保留在对应 PR/Actions 记录中；本文只保留可重复执行的当前事实与后续可靠性工作。

相关文档：

- [CLI 产品线设计](../architecture/cli-product-line-design.md)
- [App Server 架构设计](../architecture/app-server-architecture.md)
- [Agent Runtime 部署设计](../architecture/agent-runtime-deployment-design.md)
- [产品架构](../architecture/product-architecture.md)

## 1. 范围与不变量

本计划只约束交互式 TUI 的产品后端调用：

1. TUI 保留终端输入、状态、渲染和 controller-local effect。
2. TUI 通过 app-local `TuiBackend` 使用产品后端，不直接依赖 Core、Runtime 实现、Service 或全局 singleton。
3. Embedded 与 Shared TUI 都使用 `AppServerTuiBackend` 和正式 `AppServerClient`。
4. App Server 只适配稳定合同，不接管 Runtime、Service 或 Product Domain 的业务所有权。
5. Headless `exec`、ACP、Peer Host 和公开 SDK 保留各自经评审的 adapter。

本计划不重写 Ratatui 状态机，不把 App Server 变成通用 Tool/Core RPC，不迁移 Runtime owner，也不把 clipboard、editor、terminal raw mode 等本地 effect 下沉到工作区 Host。

## 2. 当前路径

```text
Embedded TUI
  -> TuiAgentClient
  -> TuiBackend
  -> AppServerTuiBackend
  -> AppServerClient
  -> private in-memory transport
  -> BitfunAppServer
  -> Runtime API / owners

Shared TUI (--shared)
  -> TuiAgentClient
  -> TuiBackend
  -> AppServerTuiBackend
  -> AppServerClient
  -> authenticated loopback transport
  -> Shared BitfunAppServer Host
  -> Runtime API / owners
```

两条路径复用同一 method、DTO、类型化错误、capability 和事件合同，只在 Host、transport、实例发现和生命周期上不同。Shared Host 按 canonical workspace 持有一个 Runtime owner，通过随机 bearer token 和实例 identity 认证连接；每条连接显式订阅 Session。多个已认证客户端可以订阅、观察和操作同一 Session，独立 Turn 仍由 Runtime owner 串行准入，steer/cancel 使用精确 Turn ID。

Mode/Model、Skill、Subagent、MCP、External Source、native/external Hook、Account、Settings Sync 和 Worktree 管理面均经过 `TuiBackend` 的 owner-specific typed API。Shared Host 使用 `AppManagementService::load_for_local_host` 装配真实本机 capability；TUI controller 不访问 compatibility owner，也不在 Remote workspace 静默回落控制端本机。

## 3. 当前能力与限制

| 范围 | 当前状态 |
| --- | --- |
| 初始化、健康和能力 | Embedded 与 Shared 都使用 `app/initialize`、`app/health` 和 Host 发布的真实 capability/limit |
| Session/Turn | list、create、sync、rename、delete、fork、model/mode、submit、cancel、steer、Shell、UserInput、compact、undo/redo、usage、settlement 和 lineage 均走 typed App Server method |
| Permission 与事件 | 连接按 Session subscription 接收 Agent、Permission 和 Config notification；pending Permission 可由 `session/sync` 恢复 |
| Workspace | binding、reference search、message reference、diff 和 Worktree 管理均在 App Server 边界内 |
| Management | Model、Mode、Skill、Subagent、MCP、External Source、Hook、Account、Settings Sync 和 Worktree 由 Host 装配的 owner service 提供 |
| Attachment | Shared TUI 图片附件仍明确 unsupported，不静默回落 Embedded 或控制端本机 |
| Remote | `--shared` 只选择本机 Shared TUI deployment，不隐式启用 Remote workspace、Remote control、Peer、Detached Dispatch、ACP 或 SDK Host |

事件 cursor 仍是 connection-local。`app/syncEvents` 返回当前连接 cursor 与 pending Permission snapshot，`session/sync` 恢复 Session state、transcript、workspace binding 和 pending Permission；当前没有跨连接持久 replay、历史事件重放或透明 resume。

Shared transport 已提供 loopback 限制、随机 token、canonical identity、实例锁、128 KiB request、8 MiB response/event、最多 64 连接和 30 秒空闲退出。仍需补齐通用 `outcome_unknown` 查询/恢复、断连取消的完整合同、慢客户端背压治理和对应故障测试。客户端不得盲目重试无法确认结果的副作用请求。

## 4. 分阶段状态

| 阶段 | 当前状态 | 完成事实 |
| --- | --- | --- |
| Phase 0：边界 | 已完成 | `TuiBackend`、behavior-light protocol/client crate 与 source/Cargo guard 已建立 |
| Phase 1：协议基础 | 已完成 | initialize/health、typed events、connection-local cursor、resync 和稳定错误已接线 |
| Phase 2：核心聊天 | 已完成 | Embedded 核心 TUI 用例全部经过 App Server；TUI controller 不引用 Runtime SDK |
| Phase 3：配置管理 | 已完成 | Mode/Model、Skill、Subagent、MCP 使用 secret-safe typed API |
| Phase 4：外部集成 | 已完成 | External Source、Hook、Account、Settings Sync、Worktree 经过 typed backend；Remote 不回落本机 |
| Phase 5：Shared App Server | 已完成当前定义 | `--shared` 使用 Shared App Server；旧 Shared transport、CLI compatibility adapter 和独立协议 crate 已删除 |

## 5. 后续可靠性工作

后续工作以现有 Shared App Server 为基础，不恢复双栈：

1. 定义跨连接 cursor/replay owner，或明确保持重新 initialize + authoritative sync 的非透明恢复合同。
2. 为副作用请求增加 operation identity、未知结果查询/恢复和禁止盲重试的端到端测试。
3. 补齐断连取消、迟到响应、Host 崩溃、慢 client/backpressure 和 frame 超限的故障矩阵。
4. 记录 startup、延迟、内存、连接/队列上限和长时间多客户端运行数据。
5. Remote、Peer、Detached Dispatch、ACP 和 SDK Host 如需 Shared 能力，必须各自增加协商 capability 和目标执行域设计，不能复用本机入口作隐式 fallback。

## 6. 验证

```bash
cargo check -p bitfun-app-server --offline
cargo test -p bitfun-app-server --offline
cargo test -p bitfun-app-server-protocol --offline
cargo test -p bitfun-app-server-client --offline
cargo check -p bitfun-cli --bin bitfun --offline
cargo test -p bitfun-cli --bin bitfun --offline
pnpm run check:core-boundaries
```

行为 fixture 至少覆盖：

| 场景组 | 必须覆盖 |
| --- | --- |
| Chat | create、sync、submit、stream、Permission、UserInput、cancel、steer、Shell |
| Session | rename、model/mode、fork、undo/redo、compact、usage、settlement |
| Workspace | binding、references、diff、worktree、remote unsupported |
| Lineage | tree、descendant transcript、settlement、targeted cancellation |
| Failure | unsupported、lag、invalidated、disconnect、deadline、unknown outcome |
| Deployment | Embedded 与 Shared App Server 的 TUI behavior parity |

## 7. 完成定义

当前定义满足以下条件：

1. TUI 产品请求和订阅只经过 `TuiBackend`，view/reducer 不执行 backend I/O。
2. Embedded 与 Shared TUI 都使用 App Server client/server 合同，不保留旧 transport 或 compatibility adapter。
3. protocol/client 和 TUI-facing 依赖闭包不包含 Core、Runtime/Service 实现或 `product-full`。
4. capability、limits、身份和作用域来自真实 Host/transport。
5. Remote workspace 不存在 controller-local fallback。
6. 重复 DTO、无效 handler 和无生产消费方的旁路已删除。

事件跨连接恢复、未知结果和慢客户端治理是明确记录的后续可靠性缺口，不改变上述架构完成事实。
