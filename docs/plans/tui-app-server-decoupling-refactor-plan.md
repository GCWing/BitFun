# TUI 与 App Server 解耦重构计划

> 状态：Phase 0-4 已完成当前定义的边界、协议基础、核心聊天、配置管理和外部集成接口迁移；Phase 5 Embedded direct-runtime 迁移待实现，Shared App Server 目标待评审。
>
> 当前状态基线：2026-08-13。一次性的运行证据保留在对应 PR/Actions 记录中；本文不绑定会因 rebase 失效的提交 SHA。
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
2. TUI 当前通过 app-local `TuiBackend` 使用产品后端；Phase 5 再把它拆成 Runtime port 与按 domain 注入的管理接口。`TuiAgentClient` 不直接依赖 Core、Runtime 实现、具体 Service、全局 singleton 或私有 IPC operation；view/reducer 也不执行 backend I/O。
3. Embedded TUI 当前使用 `AppServerTuiBackend`；Shared TUI 当前使用 `SharedTuiBackend` 映射 private Runtime IPC v17。Phase 5 才引入 `TuiRuntimePort`、`DirectRuntimeTuiRuntime` 和 Shared IPC Runtime adapter，并把管理用例移到 backend composition 的 owner service/provider 接口。
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
Embedded TUI（当前）
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

当前两条路径共用的是包含 Runtime 与管理方法的单体 `TuiBackend`。`AppServerTuiBackend`
把 Embedded 请求委托给 App Server client；`SharedTuiBackend` 把 Session/chat 请求映射到
private Runtime IPC v17，并直接持有具体 `AppManagementService` 承接 Host-local 管理能力。
Phase 3/4 已让 controller 使用 typed backend API，不代表 Phase 5 的 port 拆分或 adapter 接线已经完成。

### 2.2 Approved Embedded target

Phase 5 将当前单体 backend 拆成下面的 app-local composition：

```text
TuiAgentClient
  -> TuiBackend composition
     -> TuiRuntimePort
        -> DirectRuntimeTuiRuntime
        -> SharedIpcTuiRuntime
     -> owner-owned service/provider interfaces (management only)
```

这是已批准但尚未交付的 Phase 5 目标。`TuiRuntimePort` 将只覆盖 Embedded 和 Shared
都需要、且当前 private Runtime IPC v17 已经承载的 Runtime 行为：initialize/health、
Session、Turn、Permission/UserInput、shell、compact/undo/redo/reload、usage/settlement、
workspace reference/diff、lineage、fork、当前 Session 的 model/mode 更新、agent mode
catalog 和事件订阅。Phase 5 完成后，`DirectRuntimeTuiRuntime` 将 direct Runtime 映射到
该 port，Shared IPC adapter 将 v17 的结果和事件映射到同一组 TUI semantic types；后者
不运行 `BitfunAppServer`，也不是 Shared App Server transport。

Model catalog/CRUD、Skill、Subagent、MCP、Account、Settings Sync、Worktree、External
Source 和 Hook 不因为被 TUI 使用就进入 `TuiRuntimePort`，也不需要一个总括性的
`TuiManagementPort`。backend composition 按 domain 注入各 owner 已有的稳定 service/provider
trait；Embedded 直接使用同进程 owner service，Shared 使用 Host-local service/provider
adapter。只有原始 service 接口暴露内部类型、无法表达 TUI 所需的权限/上下文/unsupported，
或需要跨部署稳定 DTO 时，才在 owning crate 或 adapter 内增加最薄的 facade。不得把
`AppManagementService` 整体搬入 direct adapter，也不得让 controller/view 直接依赖具体
service 实现。缺少 provider 的 Shared/Remote 场景返回 typed unsupported，禁止静默回落控制端本机。

Phase 3 已将 Mode/Model、Skill、Subagent 和 MCP 管理面迁移到 TUI-facing typed API。Phase 4 进一步迁移了 External Source、native/external Hook、Account、Settings Sync 和 Worktree 管理面。当前这些方法仍位于单体 `TuiBackend`；Embedded 经 App Server 的 `AppManagementService` wiring 调用 owner，Shared 则由 `SharedTuiBackend` 委托其持有的具体 `AppManagementService`。Phase 5 将按 owner 归属把管理用例拆到 service/provider 接口，不建立管理总接口；TUI controller 仍不直接访问 compatibility owner。

Phase 5 中，`DirectRuntimeTuiRuntime` 将实现 `TuiRuntimePort`，可以依赖 Rust Runtime SDK 暴露的稳定 typed facade，但不得把 Runtime 内部类型、Core singleton 或事件队列 owner 暴露给 TUI。管理面 backend 可以直接调用 owner-owned 的稳定 service/provider trait；若该接口暴露内部类型，或需要 TUI-specific DTO、权限、上下文和 capability 裁剪，才在 owning crate 抽取薄 facade。不得把 `AppManagementService` 原样搬到 CLI，也不得由 direct adapter 复制其业务状态或策略。

当前 Shared 的 Session/chat/mode authority 由 `SharedTuiBackend` 映射 v17；Phase 5 完成后才由 Shared IPC Runtime adapter 实现 `TuiRuntimePort`。Host 实际提供的本机管理 capability 当前由 `SharedTuiBackend` 委托具体 `AppManagementService`，Phase 5 再改为 backend composition 按 domain 调用 Host-local service/provider adapter。当前 Shared Host 提供 Phase 4 的 External Source V1 和 Hook 管理，但不注入 Account/Settings Sync 或 Worktree owner；这些能力返回 typed unsupported。Remote workspace 对所有 controller-local management capability fail closed，不回落到控制端本机。Phase 4 完成表示 typed API 和 wiring 已迁移，不表示所有 deployment 的 capability 完全相同，也不表示 Phase 5 已完成。Phase 4 之后新增的 External Application V2 控制面目前只在 Embedded App Server 接线，Shared Runtime 明确 unsupported，不重新打开 Phase 4 的旧 owner 直连预算。

### 2.3 Optional Shared App Server proposal

Web 当前继续通过自己的 loopback WebSocket App Server 入口，不经过 TUI composition：

```text
Web UI -> Web Host -> loopback WebSocket App Server
       -> Runtime API / owner ports

Shared Rich Client（Phase 6 candidate）
  -> AppServerClient
  -> candidate private Pipe / UDS
  -> Shared App Server Host
  -> Runtime API / owner ports
```

Shared App Server 仍是 Phase 6 待评审提案，不是 Phase 5 的既定结果。Private Runtime IPC
v17 在候选 transport 的鉴权、实例身份、controller/lease、事件恢复、断连取消、
`outcome_unknown`、frame 限制和空闲退出达到行为等价前继续保留；评审也可以决定长期保留
v17。Embedded direct-runtime 不替换 v17，也不要求 direct facade 与 wire DTO 相同；两者只需
满足同一行为合同。

## 3. 当前能力矩阵

状态定义：

- **已交付**：生产 handler/client 已接线，并被当前 Embedded TUI 路径使用。
- **兼容映射**：Shared TUI 通过 Runtime IPC v17 和 `SharedTuiBackend` 提供与 Embedded 对照的 TUI Runtime 用例，但没有经过 App Server wire。
- **部分交付**：已有合同或 handler，但 Host 能力、恢复、安全或 TUI 调用路径仍不完整。
- **未迁移**：当前 TUI 仍使用既有 compatibility owner 路径，或尚无生产接口。
- **本地保留**：属于 TUI 或 controller-local effect，不迁移。

本矩阵的 Embedded 列记录 Phase 5 之前的 App Server 基线。Phase 5 完成后，Runtime 用例由
`DirectRuntimeTuiRuntime` 调用 Runtime typed facade；管理用例由 backend composition 调用
对应 owner service/provider。表中的行为合同和 Shared 对照场景保持不变。

### 3.1 核心聊天与 Session

| TUI 用例 | Embedded App Server | Shared v17 compatibility | 当前结论 |
| --- | --- | --- | --- |
| 初始化、版本、健康 | `app/initialize`、`app/health` | adapter 根据 v17 握手结果合成 TUI-facing initialize/health | Embedded 已交付；Shared 尚不是 App Server connection |
| Agent、Permission 事件 | `agent/event`、`agent/permissionEvent` | IPC 事件桥映射为 `AppServerEvent` | 两边均可驱动当前核心 TUI；底层恢复合同不同 |
| Config 事件 | `config/event` | 当前 Shared bridge 不投影 Config 事件 | Embedded 已接线；Shared 的 TUI-facing 管理 capability 当前由 `SharedTuiBackend` 委托其具体 `AppManagementService`，不代表 v17 已有 Config 事件；Phase 5 再拆成 owner service/provider adapter |
| 流失效与重同步 | `app/eventStreamState`、`app/syncEvents`、`session/sync` | adapter 投影 connection-local cursor、invalidation/resync 和 closed | 已有连接内 cursor/sync；没有跨连接持久 replay/resume |
| Session list/create/sync | `agent/listSessions`、`agent/createSession`、`session/sync` | list/create/atomic restore operation | 已交付；sync 包含 Runtime 状态、transcript、workspace binding 和 pending Permission |
| Session delete/rename/fork | typed App Server methods | v17 controller-scoped operations | 已交付或兼容映射；Shared 继续执行 controller/idle 规则 |
| Model/mode update | `session/updateModel`、`session/updateMode` | v17 current-controller operations | Session update wire 已覆盖；当前 Embedded 经 `AppServerTuiBackend` 提交，Shared 经 `SharedTuiBackend -> Runtime IPC v17` 提交；Phase 5 完成后再分别由 Direct/Shared Runtime adapter 实现 `TuiRuntimePort` |
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
| Mode/Model 管理 | Embedded App Server 提供 typed mode catalog 和 model list/get/add/update/delete/default API；read DTO 只含 secret configured metadata，mutation 使用 preserve/replace/clear | Phase 3 已完成 typed API 与 wiring；Shared mode catalog 来自 Runtime Host，model 目录/管理由 `SharedTuiBackend` 持有的具体 `AppManagementService` 提供，Session model mutation 由 `SharedTuiBackend` 提交给 v17 owner；Phase 5 再拆分 owner service/provider 与 Runtime port |
| Skill/Subagent | 当前 `TuiBackend` 提供 typed list/toggle API 和 visible/manageable read model；Embedded 经 App Server wiring，Shared 由 `SharedTuiBackend` 委托其具体 `AppManagementService` | Phase 3 已完成 typed API 与 wiring；Shared capability 明确属于本机 CLI compatibility scope，Phase 5 再拆成 owner service/provider adapter |
| MCP | 当前 `TuiBackend` 提供 typed catalog/status/toggle/add/delete/external decision/conflict API；read projection 与 Debug 输出不暴露凭据 | Phase 3 已完成当前定义；Shared 由 `SharedTuiBackend` 委托当前 CLI 进程的具体 `AppManagementService`，以本地 MCP compatibility service 保留迁移前管理行为。该 service 的 MCP 进程状态和 tool registry 不会即时重配已经运行的 Shared Runtime Host；要取得 Host 侧新状态仍需显式的同步/restart contract，不能把本地 toggle 描述成 v17 远端控制 |
| External Source/Tool/Command/Agent | 当前 `TuiBackend` 提供 typed snapshot/control/review、conflict choice、command expansion 和事件接口；Embedded 经 App Server wiring，Shared V1 由 `SharedTuiBackend` 委托其具体 `AppManagementService` | Phase 4 当前定义已完成；Shared 保留 V1 本机 compatibility，V2 明确 unsupported，Remote 不回落本机；Phase 5 再拆成 owner service/provider adapter |
| Hooks | 当前 `TuiBackend` 提供 typed native overview 与 external snapshot/plan/apply/mutate API；Embedded 经 App Server wiring，Shared 由 `SharedTuiBackend` 委托其具体 `AppManagementService` | Phase 4 已完成 typed API 与 wiring；native user hooks、compiled-in `post_call_hooks` 和 external hook catalog 继续分离，Remote 明确 unsupported；Phase 5 再拆成 owner service/provider adapter |
| Account/Settings Sync | typed snapshot/login/finalize/logout 与 sync start/snapshot/cancel/local-changed 已接线；凭据不进入 read model 或 Debug 输出 | Phase 4 接口迁移已完成；Embedded Host 注入共享 `AccountRuntime`，App Server 直接做 domain-to-wire 适配；当前 Shared Host 未注入并返回 typed unsupported |
| Worktree | typed repository status、bind/release 和 operation identity 已接线 | Phase 4 接口迁移已完成；Embedded Host 注入 Worktree owner，当前 Shared Host 与 Remote workspace 明确 unsupported |
| Desktop/Web Host 安全 | WebSocket Host 仅为 loopback 单用户；Desktop 当前仍使用 Tauri adapter，独立 direct Runtime 迁移尚未实施 | Host allowlist、身份/作用域、真实 limits 与平台 capability provider |

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
| `src/apps/cli` | 当前拥有 `TuiAgentClient`、单体 `TuiBackend`、`AppServerTuiBackend`、`SharedTuiBackend`、transport 和进程生命周期、TUI-local effect；Phase 5 再引入 Runtime port 与按 domain 注入的 owner service/provider composition |
| Runtime/Service/Product Domain owners | Session、Turn、Permission、Workspace、配置和其他业务权威事实 |

边界规则：

- protocol/client 的依赖闭包不得引入 `bitfun-core`、Runtime 实现、Service 实现、UI framework 或 `product-full`。
- `bitfun-app-server` 可依赖生产 handler 所需的明确 owner feature，但禁止选择 `bitfun-core/product-full`。
- Host 负责 transport、认证、作用域、真实 capability/limits、平台能力和进程生命周期。
- handler 只做合同校验、DTO 转换和错误映射，不持有第二份业务权威状态。
- Phase 5 引入的 `TuiRuntimePort` 只抽取 Embedded/Shared 共同需要的 Runtime 行为；不定义总括性的 `TuiManagementPort`。
- Phase 5 将管理面按 domain 拆到 owner-owned 的稳定 service/provider trait；只有需要 DTO、权限/上下文
  适配或 capability 裁剪时才增加薄 facade，不能把具体 service 实现或 `AppManagementService`
  整体暴露给 TUI。
- DTO 提取不代表 Runtime owner 迁移。

## 5. 分阶段状态

计划状态以完成条件和验证证据为准，不以 method 数量或文件存在为准：

| 阶段 | 完成条件 | 验证方式 | 当前状态 | 验证记录 |
| --- | --- | --- | --- | --- |
| Phase 0：边界 | `TuiBackend`、behavior-light protocol/client crate、source/Cargo guard 已建立 | Core boundary tests 和 dependency checks | 已完成 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 1：协议基础 | initialize/health、typed events、connection-local cursor、resync、稳定错误和 Embedded connection 已接线 | App Server protocol/client/server focused tests | 已完成 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 2：核心聊天（旧路径） | Embedded 核心用例经 App Server；Shared 经同一 `TuiBackend` 映射 v17；TUI 核心不引用 Runtime SDK/IPC operation | CLI、App Server、Runtime IPC 和 boundary focused tests | 已完成当前定义，作为迁移基线 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 3：配置管理 | TUI controller 不再访问 config/registry/MCP compatibility owner；secret-safe typed APIs 完成，CLI Host adapter 可保留显式 compatibility forwarding | owner tests、App Server contract tests、CLI behavior tests | 已完成当前定义 | 本变更的 protocol/client/server/CLI focused tests 与 Core boundary checks |
| Phase 4：外部集成 | External Source、Hook、Account、Settings Sync、Worktree 管理面经 typed backend；remote 不回落本机 | owner/remote/security contract tests | 已完成当前定义 | [PR #2146 checks](https://github.com/GCWing/BitFun/pull/2146/checks)、zero-budget contract 与 Core boundary checks |
| Phase 5：Embedded direct-runtime | `TuiRuntimePort` 与 owner service/provider 接入边界拆分完成；Embedded TUI 删除 App Server client/server 与 in-memory transport，改用 direct Runtime adapter；管理面不进入 Shared IPC，旧路径随切换删除 | Runtime port 对 Shared v17 operation 的覆盖测试；direct-runtime 与 Shared v17 的 Runtime 行为测试；各管理 service/provider 的 capability/unsupported 测试 | 待实现 | - |
| Phase 6：Shared App Server | Shared Host 达到 v17 治理等价，opt-in 双栈验证完成，并有回滚与删除证据 | 跨 transport parity、故障、性能和安全测试 | 未开始，目标待评审 | - |

### 5.1 Phase 0-2 已交付摘要

- `TuiAgentClient`、Startup 和 `ChatMode` 只消费 app-local `TuiBackend`；Runtime 调用和管理调用均不下沉到 view/reducer。
- Embedded Host 当前在专用 OS 线程的 current-thread Tokio runtime + `LocalSet` 中运行 private `BitfunAppServer`，TUI 保持在原多线程 runtime；这是 direct-runtime 迁移前的基线。
- `AppServerTuiBackend` 通过正式 `AppServerClient` 和 in-memory transport 完成核心用例，后续由 direct Runtime composition 替换。
- `SharedTuiBackend` 将相同 Runtime 行为映射到 private Runtime IPC v17；TUI client/controller 不引用 IPC operation。将这些方法抽取为 Runtime port 属于 Phase 5。
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
- `AppManagementService` 位于 App Server server wiring，复用现有 config、registry、MCP 和 external-source owner，不成为第二个业务 owner；Startup 与 Chat controller 只调用 `TuiBackend` 的 typed 方法。Phase 5 才把这些方法按 domain 拆到 owner service/provider，且不会建立 direct TUI 的总管理接口。
- `SharedTuiBackend` 继续映射 v17 mode catalog，并将 Model、Skill、Subagent 和 MCP 管理委托其持有的具体 `AppManagementService`。v17 不承载这些目录、CRUD 或 defaults；Shared 发布的是 adapter-scoped 本地 capability，current-Session model update 仍按 v17 的 controller/idle/outcome-unknown 合同提交给 Runtime Host。Phase 5 再以 owner service/provider adapter 和 Shared IPC Runtime adapter 替换这段单体 wiring。Shared MCP service 的运行态只属于当前 CLI 进程，不宣称可以即时控制已经运行的 Shared Runtime Host。
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
- `TuiAgentClient`、Startup 和 Chat controller 只经单体 `TuiBackend` 的 typed API 调用这些用例；Embedded 由 `AppServerTuiBackend` 委托 App Server wiring，Shared 由 `SharedTuiBackend` 委托 v17 或其具体 `AppManagementService`。Phase 4 涉及的 `bitfun_core`、account/account-sync compatibility marker 已从 controller 文件移除，对应 Core boundary budget 固定为零；Runtime port 与按 domain 的 management composition 仍留给 Phase 5。
- Embedded Host 显式注入共享 `AccountRuntime` 并启用 App Server 内建的本机 Worktree 映射；App Server management service 直接适配 owner，不定义 `AccountManagementHost` 或持有第二份账户、同步、外部来源、Hook、Worktree 权威状态。CLI 的窄 `AccountRuntimeHost` 只实现 daemon、Relay/Peer 路由宿主效果，Session 备份通过独立端口读取 Agent Runtime compatibility owner。
- Shared adapter 只发布 Host 实际可用的 capability。External Source V1 与 Hook 管理可使用当前本机 compatibility service；Account/Settings Sync、Worktree、Remote workspace 和后续未接线的 External Application V2 返回 typed unsupported，不静默回落本机。
- Phase 4 未扩展 private Runtime IPC v17，也未改变 Phase 6 的 Shared transport 评审门槛。

### 5.4 Phase 5

Embedded direct-runtime Phase 5 建议顺序：

1. 按当前 Shared IPC v17 operation 集合冻结窄 `TuiRuntimePort`，定义统一的 TUI semantic request/result/event/error；direct facade 与 v17 wire 不要求共享 DTO。
2. 将当前包含 Runtime 和管理面方法的单体 `TuiBackend` 拆为 backend composition：Runtime 调用进入 `TuiRuntimePort`，Model/Skill/Subagent/MCP/Account/Settings/Worktree/External Source/Hook 按 domain 进入各自 owner service/provider 接口；不创建 `TuiManagementPort` 总接口。
3. 逐项检查管理 service 的暴露面：能直接复用稳定 owner-owned trait 的直接注入；暴露内部类型或需要 TUI DTO、权限/上下文、capability 裁剪的，才抽取最薄 facade。`AppManagementService` 仅保留为 App Server wiring，不原样迁入 CLI。
4. 为 `DirectRuntimeTuiRuntime` 和 Shared IPC adapter 实现 Runtime port；再将 Embedded Host 从 `EmbeddedAppServerHost` 切换为 direct Runtime Host，删除 in-memory transport、App Server thread 和 initialize/health wire handshake。
5. 按 Chat、Session、Permission/UserInput、Workspace、Config/Management 垂直切片迁移并验证事件订阅、pending Permission、取消、unknown outcome、workspace/execution binding、错误映射和 Host shutdown 回收。
6. 完成 direct Runtime 的性能、升级兼容和跨入口行为验证后删除旧 App Server；不保留 rollback adapter，direct adapter 不支持的能力必须返回 typed unsupported。

Shared App Server Phase 6 不以“删除 v17”为起点。建议顺序：

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
| Deployment | 当前覆盖 Embedded App Server 与 Shared v17 compatibility；Phase 5 切换到 direct-runtime 并删除旧 Embedded App Server |

Embedded direct-runtime 实现后，同一 fixture 必须覆盖 direct Runtime 和 Shared v17；迁移前可用旧 Embedded App Server 建立基线，但不维护第三条回滚路径。Shared App Server 实现后再增加候选 transport 路径。

## 7. 完成定义

只有同时满足以下条件，才能宣布 TUI/App Server 解耦完成：

1. Phase 3/4 当前定义的管理面已迁移；Phase 5 direct-runtime 已完成；后续新增 capability 也不得绕过 backend composition 或恢复旧 owner 直连。
2. TUI 产品请求和订阅经过 `TuiAgentClient` 的 backend composition；Runtime 行为经过 `TuiRuntimePort`，管理能力经过对应 owner service/provider，TUI view/reducer 不执行 backend I/O。
3. protocol/client 和 TUI-facing 依赖闭包不包含 Core、Runtime/Service 实现、`product-full` 或 private IPC operation；只有 CLI Host/backend composition 可以按 owner 注入已审核的 service/provider。
4. capability、limits、身份和作用域来自真实 Host/transport，而不是通用 protocol 默认值；管理 service/provider 缺失时返回 typed unsupported。
5. 事件、断线、恢复、权限、取消和 unknown outcome 有明确合同与故障测试；Runtime port 的每个 Shared v17 operation 都有对应覆盖证据。
6. remote workspace 不存在 controller-local fallback。
7. 重复 DTO、无效 handler、70 方法单体管理 trait 和无生产消费方的旁路已删除或不再属于稳定边界。
8. 旧 Embedded App Server 已删除且不再作为 rollback adapter；若采用 Shared App Server，迁移满足 Phase 6 的双栈、回滚、性能、安全和删除门槛；否则文档明确 v17 是保留的私有 compatibility transport。
