# TUI 与 App Server 解耦重构计划

> 状态：Phase 0-2 已完成当前定义的边界、协议基础和核心聊天迁移；Phase 3-5 尚未开始。
>
> 当前状态基线：2026-08-05。一次性的运行证据保留在对应 PR/Actions 记录中；本文不绑定会因 rebase 失效的提交 SHA。
>
> 本文只记录当前差距、阶段和完成证据。稳定架构约束见相邻架构文档；Phase 0 的历史盘点已失效，不再作为当前能力清单。

相关文档：

- [CLI 产品线设计](../architecture/cli-product-line-design.md)
- [App Server 架构设计](../architecture/app-server-architecture.md)
- [Agent Runtime 部署设计](../architecture/agent-runtime-deployment-design.md)
- [产品架构](../architecture/product-architecture.md)

## 1. 范围与目标

本计划只迁移交互式 TUI 的产品后端调用：

1. TUI 保留终端输入、状态、渲染和 controller-local effect。
2. TUI 通过 app-local `TuiBackend` 使用产品后端，不直接依赖 Core、Runtime 实现、Service、全局 singleton 或私有 IPC operation。
3. Embedded TUI 使用 `AppServerTuiBackend`；Shared TUI 在 Shared App Server 交付前使用 `SharedTuiBackend` compatibility adapter。
4. App Server 只适配稳定合同，不接管 Runtime、Service 或 Product Domain 的业务所有权。
5. Headless `exec`、ACP、Peer Host 和公开 SDK 保留各自经评审的 adapter。

不在本计划范围内：

- 重写 Ratatui 状态机或界面布局。
- 把 App Server 变成通用 Tool/Core RPC。
- 迁移 Runtime owner 或重新设计产品领域模型。
- 为旧 Web Server 私有协议建立长期兼容层。
- 把 clipboard、editor、terminal raw mode 等 controller-local effect 下沉到工作区 Host。

## 2. 当前路径与目标路径

### 2.1 Current

当前 head 有两条交互式 TUI 后端路径：

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
  -> SharedTuiBackend compatibility adapter
  -> private Runtime IPC v17
  -> Shared Runtime Host process
  -> Runtime API / owners
```

两条路径统一的是 TUI 可见的行为端口。Shared compatibility adapter 会把 Runtime IPC 的结果和事件映射为 `TuiBackend` 使用的类型，但它没有运行 `BitfunAppServer`，也不是 Shared App Server transport。

Phase 3/4 尚未迁移的配置、MCP、Skill、Subagent、Hook、外部来源、Account 和 Worktree 管理面仍可能通过 CLI Host 中的现有 Core/Service compatibility 路径完成。它们是当前剩余差距，不能据 Phase 2 的核心聊天完成状态宣称整个 TUI 已解耦。

### 2.2 Proposed target

若 [App Server 目标架构](../architecture/app-server-architecture.md) 通过评审，交互式 Rich Client 的目标路径为：

```text
TUI renderer / input / state / local effects
                    |
                    v
              TuiBackend trait
                    |
                    v
          AppServerTuiBackend adapter
                    |
                    v
              AppServerClient
                    |
         Host-selected transport
          /                     \
 in-memory Embedded       controlled Shared local
          \                     /
                    v
               App Server
                    |
                    v
 Runtime API / Services / Product Domain owners
```

Shared Runtime IPC v17 在 Shared App Server 的鉴权、实例身份、controller/lease、事件恢复、断连取消、`outcome_unknown`、frame 限制和空闲退出达到行为等价前继续保留。是否最终删除 v17 由等价测试、性能数据、真实 Rich Client 消费方和回滚证据决定，不能只依据 schema 相同或 adapter 已存在。

## 3. 当前能力矩阵

状态定义：

- **已交付**：生产 handler/client 已接线，并被当前 Embedded TUI 路径使用。
- **兼容映射**：Shared TUI 通过 Runtime IPC v17 和 `SharedTuiBackend` 提供等价 TUI 用例，但没有经过 App Server wire。
- **部分交付**：已有合同或 handler，但 Host 能力、恢复、安全或 TUI 调用路径仍不完整。
- **未迁移**：当前 TUI 仍使用既有 compatibility owner 路径，或尚无生产接口。
- **本地保留**：属于 TUI 或 controller-local effect，不迁移。

### 3.1 核心聊天与 Session

| TUI 用例 | Embedded App Server | Shared v17 compatibility | 当前结论 |
| --- | --- | --- | --- |
| 初始化、版本、健康 | `app/initialize`、`app/health` | adapter 根据 v17 握手结果合成 TUI-facing initialize/health | Embedded 已交付；Shared 尚不是 App Server connection |
| Agent、Permission 事件 | `agent/event`、`agent/permissionEvent` | IPC 事件桥映射为 `AppServerEvent` | 两边均可驱动当前核心 TUI；底层恢复合同不同 |
| Config 事件 | `config/event` | 当前 Shared bridge 不投影 Config 事件 | Embedded 已接线；Shared 配置管理面仍属 Phase 3 |
| 流失效与重同步 | `app/eventStreamState`、`app/syncEvents`、`session/sync` | adapter 投影 connection-local cursor、invalidation/resync 和 closed | 已有连接内 cursor/sync；没有跨连接持久 replay/resume |
| Session list/create/sync | `agent/listSessions`、`agent/createSession`、`session/sync` | list/create/atomic restore operation | 已交付；sync 包含 Runtime 状态、transcript、workspace binding 和 pending Permission |
| Session delete/rename/fork | typed App Server methods | v17 controller-scoped operations | 已交付或兼容映射；Shared 继续执行 controller/idle 规则 |
| Model/mode update | `session/updateModel`、`session/updateMode` | v17 current-controller operations | 当前 Session 更新已覆盖；完整目录和默认值仍属 Phase 3 |
| Submit/cancel/steer | typed Agent methods | v17 Turn operations | 已交付或兼容映射 |
| User Shell/UserInput | `agent/runUserShellCommand`、`agent/submitUserAnswers` | v17 typed operations | 已交付或兼容映射；执行和权限仍由 Runtime owner 持有 |
| Permission pending/respond | typed Permission methods/events | v17 pending/respond and event stream | 已交付或兼容映射 |
| Transcript/local command record | `session/readTranscript`、`session/recordLocalCommandTurn` | v17 transcript/record operation | 已交付或兼容映射 |
| Compact/undo/redo/reload | typed Session methods | v17 current-controller operations | 已交付或兼容映射 |
| Usage/settlement | `session/usage`、`session/waitForSettlement` | v17 usage/settlement operations | 已交付或兼容映射 |
| Workspace references/diff | typed Workspace methods | v17 reference/diff operations | 已交付或兼容映射 |
| Lineage query/inspect/cancel | typed Session methods | v17 root-controller operations | 已交付或兼容映射 |

### 3.2 事件恢复的准确边界

当前 App Server 已发送带 `connection_id + stream + sequence` 的 cursor，并在 server receiver lag/closed 时提供明确 resync directive。`session/sync` 可恢复 Session、Runtime 状态、transcript、workspace binding 和 pending Permission；`app/syncEvents` 返回所请求 stream 的当前 connection-local cursor 与 pending Permission snapshot，但当前不提供 Agent 或 Config snapshot。

当前未交付的是跨连接持久化 cursor、历史事件 replay 和断线后的透明 resume。Shared Runtime IPC v17 仍按自己的 lag/closed、断连取消和 controller 隔离规则工作；`SharedTuiBackend` 只为当前 TUI connection 投影单调 cursor，不能把该投影描述为底层 IPC 已有 replay。

### 3.3 尚未迁移的管理面

| Domain | 当前状态 | Phase 3/4 需要完成 |
| --- | --- | --- |
| Mode/Model 管理 | 当前 Session mode/model 更新已交付；目录、secret-safe CRUD 和 defaults 未形成完整 App Server 用例 | Runtime-resolved catalog、secret-safe mutation、默认值与 availability |
| Skill/Subagent | 仍使用既有 CLI/Core 管理路径 | visible/manageable read model、override/model binding、context reload 触发规则 |
| MCP | 仍使用既有 CLI/Core/Service 路径 | catalog/status、CRUD、restart、approval、conflict 和 events |
| External Source/Tool/Command/Agent | 当前 App Server production fallback 明确不支持旧 external route | owner snapshot、mutation、review、conflict、generation 和 typed events |
| Hooks | 仍使用既有 native/external hook 管理路径 | native overview 与 external import lifecycle；保持两类 Hook 分离 |
| Account/Settings Sync | 尚无 TUI App Server 闭环 | secret-safe auth flow、sync operation identity、冲突、取消和 snapshot recovery |
| Worktree | Session workspace binding 已进入 sync；bind/release/status 管理未迁移 | owner-scoped worktree lifecycle 和 remote unsupported |
| Desktop/Web Host 安全 | WebSocket Host 仅为 loopback 单用户；Desktop 尚未迁移为 App Server Host | Host allowlist、身份/作用域、真实 limits 与平台 capability provider |

### 3.4 本地保留

以下能力不新增 App Server method：

| 能力 | 所有者 |
| --- | --- |
| Terminal raw/alternate screen/cursor lifecycle | TUI Host |
| Ratatui render/input/mouse/resize/scroll | TUI |
| Composer draft/history/prompt stash | TUI |
| Theme、terminal color、palette、help、key bindings | TUI |
| Clipboard、图片捕获、外部编辑器 | controller-local capability |
| Controller-local copy/export、notification、bell | controller-local capability |

图片提交仍须转成受限附件 DTO 并进入后端合同。导出到 controller-local 路径是本地 effect；写入工作区或后端 artifact 必须由工作区 owner 提供数据，再由本地 effect 选择保存位置。

## 4. Crate 与 ownership

当前职责拆分如下：

| 路径 | 职责 |
| --- | --- |
| `src/crates/interfaces/app-server-protocol` | behavior-light method、DTO、wire error、event envelope 和角色定义 |
| `src/crates/interfaces/app-server-client` | 类型化请求、事件分发和 host-supplied transport 抽象 |
| `src/crates/interfaces/app-server` | server 生命周期、生产 handler 注册、Runtime/domain 与 wire 转换、错误映射 |
| `src/apps/cli` | `TuiBackend`、Embedded/Shared adapter 选择、transport 和进程生命周期、TUI-local effect |
| Runtime/Service/Product Domain owners | Session、Turn、Permission、Workspace、配置和其他业务权威事实 |

边界规则：

- protocol/client 的依赖闭包不得引入 `bitfun-core`、Runtime 实现、Service 实现、UI framework 或 `product-full`。
- `bitfun-app-server` 可依赖生产 handler 所需的明确 owner feature，但禁止选择 `bitfun-core/product-full`。
- Host 负责 transport、认证、作用域、真实 capability/limits、平台能力和进程生命周期。
- handler 只做合同校验、DTO 转换和错误映射，不持有第二份业务权威状态。
- DTO 提取不代表 Runtime owner 迁移。

## 5. 分阶段状态

计划状态以完成条件和验证证据为准，不以 method 数量或文件存在为准：

| 阶段 | 完成条件 | 验证方式 | 当前状态 | 验证记录 |
| --- | --- | --- | --- | --- |
| Phase 0：边界 | `TuiBackend`、behavior-light protocol/client crate、source/Cargo guard 已建立 | Core boundary tests 和 dependency checks | 已完成 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 1：协议基础 | initialize/health、typed events、connection-local cursor、resync、稳定错误和 Embedded connection 已接线 | App Server protocol/client/server focused tests | 已完成 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 2：核心聊天 | Embedded 核心用例经 App Server；Shared 经同一 `TuiBackend` 映射 v17；TUI 核心不引用 Runtime SDK/IPC operation | CLI、App Server、Runtime IPC 和 boundary focused tests | 已完成当前定义 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 3：配置管理 | TUI 不再访问 config/registry/MCP compatibility owner；secret-safe typed APIs 完成 | owner tests、App Server contract tests、CLI behavior tests | 未开始 | - |
| Phase 4：外部集成 | External Source、Hook、Account、Worktree 管理面经 typed backend；remote 不回落本机 | owner/remote/security contract tests | 未开始 | - |
| Phase 5：Shared App Server | Shared Host 达到 v17 治理等价，opt-in 双栈验证完成，并有回滚与删除证据 | 跨 transport parity、故障、性能和安全测试 | 未开始，目标待评审 | - |

### 5.1 Phase 0-2 已交付摘要

- `TuiAgentClient`、Startup 和 `ChatMode` 只消费 app-local `TuiBackend`。
- Embedded Host 在专用 OS 线程的 current-thread Tokio runtime + `LocalSet` 中运行 private `BitfunAppServer`，TUI 保持在原多线程 runtime。
- `AppServerTuiBackend` 通过正式 `AppServerClient` 和 in-memory transport 完成核心用例。
- `SharedTuiBackend` 将相同用例映射到 private Runtime IPC v17；TUI client/controller 不引用 IPC operation。
- App Server 核心 handler 覆盖 sync、turn、Permission、revert、context、usage、settlement、Workspace 和 lineage；Config 事件也已在 Embedded connection 接线。
- Runtime IPC v17 为当前 parity 增加 restore Runtime 状态、usage、settlement 和本地命令 transcript 记录；没有增加 replay、observer、通用 controller transfer 或公开 SDK 能力。
- capability 声明列出当前注册方法，但 Host-specific availability 和方向性 limits 仍是后续收紧项。

### 5.2 Phase 3

目标：移除 TUI 对全局 config、registry 和 MCP service 的直接访问。

完成条件：

- 模型、Mode、Skill、Subagent 和 MCP 使用 owner-specific typed APIs。
- secret 不出现在 read model、日志或 generic config payload 中。
- capability 由 Host 注入的 provider、授权和健康状态决定。
- 管理面 unsupported 不静默回退既有直连路径。

### 5.3 Phase 4

目标：迁移外部来源、Hook、Account、Settings Sync 和 Worktree 管理面。

完成条件：

- mutation 有 identity/revision、stale、取消和 audit 语义。
- external source 的发现、审批、冲突和运行时可用性保持由既有 owner 管理。
- native user hooks、compiled-in `post_call_hooks` 和 external hook catalog 保持分离。
- remote workspace 不支持的能力返回 typed unsupported，不在 controller 本机执行。

### 5.4 Phase 5

Phase 5 不以“删除 v17”为起点。建议顺序：

1. 在 Shared Host 中增加默认关闭的 App Server local transport。
2. 两条 transport 复用同一 Host-scoped connection authority、controller registry、Session 事件过滤、operation identity/deadline/cancel 和未知结果登记。
3. 使用一个第一方 Rich Client 进行 opt-in 双栈验证，覆盖跨 transport 竞争、断连、迟到结果、Host 崩溃和回滚。
4. 记录 startup、延迟、内存、frame/queue 上限和长期维护成本。
5. 只有行为、安全、恢复和性能达到完成门槛后，才评审是否切换 `--shared` 默认实现并删除 v17。

保留 private v17 作为稳定终态也是允许的：只要业务用例和 owner 仍统一，物理 wire 不必为了形式统一而提前收敛。

## 6. 验证

### 6.1 当前 focused commands

```bash
cargo check -p bitfun-app-server --offline
cargo test -p bitfun-app-server --offline
cargo test -p bitfun-app-server-protocol
cargo test -p bitfun-app-server-client
cargo check -p bitfun-cli
cargo test -p bitfun-cli
pnpm run check:core-boundaries
```

Phase 0-2 的具体命令结果和 CI 状态保留在 [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) 中，本文只保留可重复执行的验证命令和阶段状态。后续阶段必须在各自变更中重新记录验证结果，不能沿用一次性提交 SHA 作为证据。

### 6.2 行为等价场景

| 场景组 | 当前必须覆盖 |
| --- | --- |
| Chat | create、sync、submit、stream、Permission、UserInput、cancel、steer、shell |
| Session | rename、model/mode、fork、undo/redo、compact、usage、settlement |
| Workspace | binding、references、diff、remote facts |
| Lineage | tree、descendant transcript、settlement、targeted cancellation |
| Failure | unsupported、lag、invalidated、disconnect、deadline、`outcome_unknown` |
| Deployment | Embedded App Server 与 Shared v17 compatibility 的 TUI behavior parity |

Shared App Server 实现后，同一 fixture 必须增加 Embedded App Server、Shared App Server 和 v17 rollback 三方验证，直到 v17 被正式保留或删除。

## 7. 完成定义

只有同时满足以下条件，才能宣布 TUI/App Server 解耦完成：

1. Phase 3/4 管理面已迁移，或从产品范围明确移除。
2. TUI 产品请求和订阅只经过 `TuiBackend`，TUI view/reducer 不执行 backend I/O。
3. protocol/client 和 TUI-facing 依赖闭包不包含 Core、Runtime/Service 实现、`product-full` 或 private IPC operation。
4. capability、limits、身份和作用域来自真实 Host/transport，而不是通用 protocol 默认值。
5. 事件、断线、恢复、权限、取消和 unknown outcome 有明确合同与故障测试。
6. remote workspace 不存在 controller-local fallback。
7. 重复 DTO、无效 handler 和无生产消费方的旁路已删除。
8. 若采用 Shared App Server，迁移满足 Phase 5 的双栈、回滚、性能、安全和删除门槛；否则文档明确 v17 是保留的私有 compatibility transport。
