# TUI 与 App Server 解耦重构计划

> 状态：Phase 0-4 已完成当前定义的边界、协议基础、核心聊天、配置管理和外部集成接口迁移；Phase 5 Shared App Server 目标待评审。
>
> 当前状态基线：2026-08-09。一次性的运行证据保留在对应 PR/Actions 记录中；本文不绑定会因 rebase 失效的提交 SHA。
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

Phase 3 已将 Mode/Model、Skill、Subagent 和 MCP 管理面迁移到 `TuiBackend` 的 owner-specific typed API。Phase 4 进一步迁移了 External Source、native/external Hook、Account、Settings Sync 和 Worktree 管理面。具体 DTO/owner 适配由 App Server 的 `AppManagementService` 持有，并由 Host 显式装配；Embedded TUI 经 App Server 访问既有 owner，TUI controller 不再直接访问这些 compatibility owner。

Shared 的 Session/chat/mode authority 继续映射 v17；Host 实际提供的本机管理 capability 由 `SharedTuiBackend` 委托同一个具体 management service。当前 Shared Host 提供 Phase 4 的 External Source V1 和 Hook 管理，但不注入 Account/Settings Sync 或 Worktree owner；这些能力返回 typed unsupported。Remote workspace 对所有 controller-local management capability fail closed，不回落到控制端本机。Phase 4 完成表示接口边界已迁移，不表示所有 deployment 的 capability 完全相同。Phase 4 之后新增的 External Application V2 控制面目前只在 Embedded App Server 接线，Shared Runtime 明确 unsupported，不重新打开 Phase 4 的旧 owner 直连预算。

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
| Config 事件 | `config/event` | 当前 Shared bridge 不投影 Config 事件 | Embedded 已接线；Shared 的 TUI-facing 管理 capability 来自 Host 装配的 App Server management service，不代表 v17 已有 Config 事件 |
| 流失效与重同步 | `app/eventStreamState`、`app/syncEvents`、`session/sync` | adapter 投影 connection-local cursor、invalidation/resync 和 closed | 已有连接内 cursor/sync；没有跨连接持久 replay/resume |
| Session list/create/sync | `agent/listSessions`、`agent/createSession`、`session/sync` | list/create/atomic restore operation | 已交付；sync 包含 Runtime 状态、transcript、workspace binding 和 pending Permission |
| Session delete/rename/fork | typed App Server methods | v17 controller-scoped operations | 已交付或兼容映射；Shared 继续执行 controller/idle 规则 |
| Model/mode update | `session/updateModel`、`session/updateMode` | v17 current-controller operations | Session update wire 已覆盖；Embedded 与 Shared 都经 typed 管理 API 取得目录/defaults，Shared 的目录来自 Host 装配的 App Server management service |
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

### 3.3 管理面状态

| Domain | 当前状态 | 当前结论 / 后续 |
| --- | --- | --- |
| Mode/Model 管理 | Embedded 经 typed mode catalog 和 model list/get/add/update/delete/default API；read DTO 只含 secret configured metadata，mutation 使用 preserve/replace/clear | Phase 3 已完成；Shared mode catalog 来自 Runtime Host，model 管理由 Host 装配的 App Server management service 转发，Session model mutation 仍由 v17 owner 提交 |
| Skill/Subagent | TUI 经 typed list/toggle API 消费 visible/manageable read model；App Server management service 委托既有 registry owner | Phase 3 已完成；Embedded 与 Shared 共用具体 service，Shared capability 明确属于本机 CLI compatibility scope |
| MCP | 当前 TUI 用例经 typed catalog/status/toggle/add/delete/external decision/conflict API；read projection 与 Debug 输出不暴露凭据 | Phase 3 已完成当前定义；Shared 通过当前 CLI 进程的本地 MCP compatibility service 保留迁移前管理行为。该 service 的 MCP 进程状态和 tool registry 不会即时重配已经运行的 Shared Runtime Host；要取得 Host 侧新状态仍需显式的同步/restart contract，不能把本地 toggle 描述成 v17 远端控制 |
| External Source/Tool/Command/Agent | TUI 经 typed snapshot/control/review、conflict choice、command expansion 和事件接口消费既有 owner；后续 External Application V2 snapshot/review/action 已在 Embedded 接线 | Phase 4 当前定义已完成；Shared 保留 V1 本机 compatibility，V2 明确 unsupported，Remote 不回落本机 |
| Hooks | TUI 经 typed native overview 与 external snapshot/plan/apply/mutate API 消费既有 owner | Phase 4 已完成；native user hooks、compiled-in `post_call_hooks` 和 external hook catalog 继续分离，Remote 明确 unsupported |
| Account/Settings Sync | typed snapshot/login/finalize/logout 与 sync start/snapshot/cancel/local-changed 已接线；凭据不进入 read model 或 Debug 输出 | Phase 4 接口迁移已完成；Embedded Host 注入共享 `AccountRuntime`，App Server 直接做 domain-to-wire 适配；当前 Shared Host 未注入并返回 typed unsupported |
| Worktree | typed repository status、bind/release 和 operation identity 已接线 | Phase 4 接口迁移已完成；Embedded Host 注入 Worktree owner，当前 Shared Host 与 Remote workspace 明确 unsupported |
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
| Phase 3：配置管理 | TUI controller 不再访问 config/registry/MCP compatibility owner；secret-safe typed APIs 完成，CLI Host adapter 可保留显式 compatibility forwarding | owner tests、App Server contract tests、CLI behavior tests | 已完成当前定义 | 本变更的 protocol/client/server/CLI focused tests 与 Core boundary checks |
| Phase 4：外部集成 | External Source、Hook、Account、Settings Sync、Worktree 管理面经 typed backend；remote 不回落本机 | owner/remote/security contract tests | 已完成当前定义 | [PR #2146 checks](https://github.com/GCWing/BitFun/pull/2146/checks)、zero-budget contract 与 Core boundary checks |
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

状态：已完成当前定义。

完成条件：

- 模型、Mode、Skill、Subagent 和 MCP 使用 owner-specific typed APIs。
- secret 不出现在 read model、日志或 generic config payload 中。
- capability 由 Host 注入的 management service、授权和健康状态决定。
- management service unavailable 时返回明确 unsupported；Shared 的本机 compatibility forwarding 必须显式装配并发布真实 capability，不能在 Remote workspace 静默回落控制端本机。

交付摘要：

- `app-server-protocol` 提供 Mode、Model、Skill、Subagent 和 MCP 的 owner-specific DTO 与 method；model read model 不返回 secret 值，model mutation 使用 preserve/replace/clear 语义。
- App Server 由 Host 显式注入具体 `AppManagementService`，按 `tui.modes`、`tui.models`、`tui.skills`、`tui.subagents` 和 `tui.mcp` 发布真实 availability；service 缺失或 unavailable 时返回带 capability id 的 structured unsupported。
- `AppManagementService` 位于 App Server server wiring，复用现有 config、registry、MCP 和 external-source owner，不成为第二个业务 owner；Startup 与 Chat controller 只经 `TuiAgentClient -> TuiBackend` 调用这些管理用例。
- `SharedTuiBackend` 继续映射 v17 mode catalog，并将 Model、Skill、Subagent 和 MCP 管理委托 Host 装配的具体 App Server management service。v17 不承载这些目录、CRUD 或 defaults；Shared 发布的是 adapter-scoped 本地 capability，current-Session model update 仍按 v17 controller/idle/outcome-unknown 合同提交给 Runtime Host。Shared MCP service 的运行态只属于当前 CLI 进程，不宣称可以即时控制已经运行的 Shared Runtime Host。
- Core boundary budgets 已移除 Phase 3 owner 直连债务，并要求 Startup 的 Subagent 管理继续使用 typed backend。

### 5.3 Phase 4

目标：迁移外部来源、Hook、Account、Settings Sync 和 Worktree 管理面。

状态：已完成当前定义。

完成条件：

- mutation 有 identity/revision、stale、取消和 audit 语义。
- external source 的发现、审批、冲突和运行时可用性保持由既有 owner 管理。
- native user hooks、compiled-in `post_call_hooks` 和 external hook catalog 保持分离。
- remote workspace 不支持的能力返回 typed unsupported，不在 controller 本机执行。

交付摘要：

- `app-server-protocol`、client 和 production handlers 已提供 External Source、native/external Hook、Account、Settings Sync 与 Worktree 的 owner-specific typed API；side-effecting 请求使用 operation identity，External Source 与 Hook mutation 保留 owner revision/stale 合同，Settings Sync 提供显式取消与 snapshot。
- `TuiAgentClient`、Startup 和 Chat controller 只经 `TuiBackend` 调用这些用例。Phase 4 涉及的 `bitfun_core`、account/account-sync compatibility marker 已从 controller 文件移除，对应 Core boundary budget 固定为零。
- Embedded Host 显式注入共享 `AccountRuntime` 并启用 App Server 内建的本机 Worktree 映射；App Server management service 直接适配 owner，不定义 `AccountManagementHost` 或持有第二份账户、同步、外部来源、Hook、Worktree 权威状态。CLI 的窄 `AccountRuntimeHost` 只实现 daemon、Relay/Peer 路由宿主效果，Session 备份通过独立端口读取 Agent Runtime compatibility owner。
- Shared adapter 只发布 Host 实际可用的 capability。External Source V1 与 Hook 管理可使用当前本机 compatibility service；Account/Settings Sync、Worktree、Remote workspace 和后续未接线的 External Application V2 返回 typed unsupported，不静默回落本机。
- Phase 4 未扩展 private Runtime IPC v17，也未改变 Phase 5 的评审门槛。

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
cargo test -p bitfun-app-server-protocol --offline
cargo test -p bitfun-app-server-client --offline
cargo test -p bitfun-agent-runtime-ipc --offline
cargo check -p bitfun-cli --bin bitfun --offline
cargo test -p bitfun-cli --bin bitfun --offline
pnpm run check:core-boundaries
```

Phase 0-2 的具体命令结果和 CI 状态保留在 [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) 中。Phase 3 和 Phase 4 分别运行了对应的 protocol、client、server、CLI binary、owner contract 与 Core boundary focused checks；Phase 4 另有 zero-budget contract 防止 TUI controller 恢复旧 owner 直连。一次性结果保留在对应 PR/Actions 记录中，本文只保留可重复执行的验证命令和阶段状态，后续阶段必须重新记录自己的验证结果。

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

1. Phase 3/4 当前定义的管理面已迁移；后续新增 capability 也不得绕过 `TuiBackend` 或恢复旧 owner 直连。
2. TUI 产品请求和订阅只经过 `TuiBackend`，TUI view/reducer 不执行 backend I/O。
3. protocol/client 和 TUI-facing 依赖闭包不包含 Core、Runtime/Service 实现、`product-full` 或 private IPC operation。
4. capability、limits、身份和作用域来自真实 Host/transport，而不是通用 protocol 默认值。
5. 事件、断线、恢复、权限、取消和 unknown outcome 有明确合同与故障测试。
6. remote workspace 不存在 controller-local fallback。
7. 重复 DTO、无效 handler 和无生产消费方的旁路已删除。
8. 若采用 Shared App Server，迁移满足 Phase 5 的双栈、回滚、性能、安全和删除门槛；否则文档明确 v17 是保留的私有 compatibility transport。
