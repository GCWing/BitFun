# TUI 与 App Server 解耦重构计划

> 状态：Phase 5 实现切换已完成；统一跨部署 fixture、性能与升级兼容证据仍是后续门槛，Shared App Server 目标待评审。
>
> 当前状态基线：2026-08-14。一次性的运行证据保留在对应 PR/Actions 记录中；本文不绑定会因 rebase 失效的提交 SHA。
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
2. TUI 当前通过 app-local `CliAgentRuntimeClient` 使用 Runtime；其内部私有 `TuiRuntimePort` 组合 `Embedded(AgentRuntime)` 与 `Shared(RuntimeIpcClient)`。view/reducer 不直接依赖 Core、Runtime 实现、具体 Service、全局 singleton 或私有 IPC operation，也不执行 backend I/O。
3. 管理面由 `TuiManagementOwners` 按 domain 组合具体 Model、Registry、MCP、Account、Settings Sync、Worktree、Native/External Hook、External Source/Command provider；不建立总括性的 `TuiManagementPort`，也不把总括性的 App Server management service 搬入 CLI。
4. App Server 只适配稳定合同，不接管 Runtime、Service 或 Product Domain 的业务所有权。
5. Headless `exec`、ACP、Peer Host 和公开 SDK 保留各自经评审的 adapter。

不在本计划范围内：

- 重写 Ratatui 状态机或界面布局。
- 把 App Server 变成通用 Tool/Core RPC。
- 迁移 Runtime owner 或重新设计产品领域模型。
- 为旧 Web Server 私有协议建立长期兼容层。
- 把 clipboard、editor、terminal raw mode 等 controller-local effect 下沉到工作区 Host。

## 2. 当前路径与后续提案

### 2.1 Current

当前 head 有两条交互式 TUI Runtime 部署路径：

```text
Embedded TUI（当前）
  -> CliAgentRuntimeClient
  -> TuiRuntimePort::Embedded(AgentRuntime)
  -> Runtime API / owners

Shared TUI (--shared)
  -> CliAgentRuntimeClient
  -> TuiRuntimePort::Shared(RuntimeIpcClient)
  -> private Runtime IPC v18
  -> Shared Runtime Host process
  -> Runtime API / owners
```

两条路径共用 `CliAgentRuntimeClient` 的 typed Runtime API，但部署差异只存在于 client 私有的
`TuiRuntimePort`。管理调用不经过该 port：controller 使用 `TuiManagementOwners` 的具体
provider，provider 再调用真实 domain owner，并按 deployment/Remote scope 返回 capability 或
typed unsupported。CLI 不再构造 App Server client/server、in-memory transport 或旧 backend family。

### 2.2 Delivered Phase 5 composition

Phase 5 已交付下面的 app-local composition：

```text
TUI controller
  -> CliAgentRuntimeClient
     -> TuiRuntimePort::Embedded(AgentRuntime)
     -> TuiRuntimePort::Shared(RuntimeIpcClient) -> Runtime IPC v18
  -> TuiManagementOwners
     -> concrete domain owner providers (management only)
```

`TuiRuntimePort` 只覆盖 Embedded 和 Shared
都需要、且当前 private Runtime IPC v18 已经承载的 Runtime 行为：Session、Turn、
Permission/UserInput、shell、compact/undo/redo/reload、usage/settlement、
workspace reference/diff、lineage、fork、当前 Session 的 model/mode 更新、agent mode
catalog 和事件订阅。Embedded 分支将 direct Runtime 映射到
该 port，Shared 分支将 v18 的结果和事件映射到同一组 TUI semantic types；后者
不运行 `BitfunAppServer`，也不是 Shared App Server transport。initialize/health 属于
transport lifecycle：Embedded 不再伪造该握手；Shared v18 的协议与实例校验只发生在
`RuntimeIpcClient` 建连过程中，不进入 TUI Runtime port。

Model catalog/CRUD、Skill、Subagent、MCP、Account、Settings Sync、Worktree、External
Source 和 Hook 不因为被 TUI 使用就进入 `TuiRuntimePort`，也不需要一个总括性的
`TuiManagementPort`。`TuiManagementOwners` 组合各 domain 的稳定 provider；只有原始 owner
接口暴露内部类型、无法表达 TUI 所需的权限/上下文/unsupported，或需要稳定 projection 时，
才在 owning crate 或 CLI adapter 内增加最薄 facade。不得把总括性的 App Server management service 搬入 CLI，
也不得让 controller/view 直接依赖具体 service 实现。缺少 provider 的 Shared/Remote 场景返回
typed unsupported，禁止静默回落控制端本机。

Shared 的 Session/chat/mode authority 由 `CliAgentRuntimeClient` 的 Shared 分支映射 v18；
Host-local 管理 capability 由 `TuiManagementOwners` 的具体 provider 提供。Phase 4 之后新增的
External Application V2 控制面在 Shared Runtime 明确 unsupported，不重新打开旧 owner 直连预算。

### 2.3 Optional Shared App Server proposal

Web 当前继续通过自己的 loopback WebSocket App Server 入口，不经过 CLI Runtime/management composition：

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
v18 在候选 transport 的鉴权、实例身份、controller/lease、事件恢复、断连取消、
`outcome_unknown`、frame 限制和空闲退出达到行为等价前继续保留；评审也可以决定长期保留
v18。Embedded direct-runtime 不替换 v18，也不要求 direct facade 与 wire DTO 相同；两者只需
满足同一行为合同。

## 3. 当前能力矩阵

状态定义：

- **已交付**：生产 Runtime client/provider 已接线，并被当前 Embedded TUI 路径使用。
- **兼容映射**：Shared TUI 通过 `CliAgentRuntimeClient` 和 Runtime IPC v18 提供与 Embedded 对照的 TUI Runtime 用例，但没有经过 App Server wire。
- **部分交付**：已有合同或 handler，但 Host 能力、恢复、安全或 TUI 调用路径仍不完整。
- **未迁移**：当前 TUI 仍使用既有 compatibility owner 路径，或尚无生产接口。
- **本地保留**：属于 TUI 或 controller-local effect，不迁移。

本矩阵的 Embedded 列记录 Phase 5 当前 direct-runtime。Runtime 用例由
`CliAgentRuntimeClient -> Embedded(AgentRuntime)` 调用 Runtime typed facade；管理用例由
`TuiManagementOwners` 调用对应 owner provider。表中的 Shared 对照场景继续使用 v18。

### 3.1 核心聊天与 Session

| TUI 用例 | Embedded direct Runtime | Shared v18 compatibility | 当前结论 |
| --- | --- | --- | --- |
| 初始化、版本、健康 | 不建立 TUI-facing transport 握手；从 direct Runtime context 构造 client | `RuntimeIpcClient` 建连时完成 v18 协议/实例校验，不向 TUI 暴露 initialize/health | transport lifecycle 不进入 `TuiRuntimePort`；Shared 尚不是 App Server connection |
| Agent、Permission 事件 | `AgentRuntime` typed subscriptions | IPC 事件桥映射为同一 TUI semantic event | 两边均可驱动当前核心 TUI；底层恢复合同不同 |
| Config / management 状态 | `TuiManagementOwners` 具体 provider | 同一 Host-local provider 组合，按 capability/scope fail closed | 不进入 v18 event wire；provider 状态由真实 owner 投影 |
| 流失效与重同步 | direct event/Permission subscriptions 与 typed session sync | adapter 投影 connection-local cursor、invalidation/resync 和 closed | 已有连接内 cursor/sync；没有跨连接持久 replay/resume |
| Session list/create/sync | direct Runtime list/create/restore methods | list/create/coherent restore operation | 已交付；restore 在 Session mutation 边界内返回 Runtime 状态与 transcript，并携带 workspace binding、pending Permission 和 Runtime 注册顺序的 pending UserInput |
| Session delete/rename/fork | direct Runtime methods | v18 controller-scoped operations | 已交付或兼容映射；Shared 继续执行 controller/idle 规则 |
| Model/mode update | `CliAgentRuntimeClient` direct Runtime methods | v18 current-controller operations | Direct/Shared 分支实现同一 `TuiRuntimePort` 行为；model catalog/CRUD 仍属于 `TuiManagementOwners::model` |
| Submit/cancel/steer | typed Agent methods | v18 Turn operations | 已交付或兼容映射 |
| User Shell/UserInput | direct Runtime shell/answers methods | v18 typed operations | 已交付或兼容映射；执行和权限仍由 Runtime owner 持有 |
| Permission pending/respond | typed Permission methods/events | v18 pending/respond and event stream | 已交付或兼容映射 |
| Transcript/local command record | direct Runtime transcript/record methods | v18 transcript/record operation | 已交付或兼容映射 |
| Compact/undo/redo/reload | typed Session methods | v18 current-controller operations | 已交付或兼容映射 |
| Usage/settlement | direct Runtime usage/settlement methods | v18 usage/settlement operations | 已交付或兼容映射 |
| Workspace references/diff | typed Workspace methods | v18 reference/diff operations | 已交付或兼容映射 |
| Lineage query/inspect/cancel | typed Session methods | v18 root-controller operations | 已交付或兼容映射 |

### 3.2 事件恢复的准确边界

当前 Embedded direct path 在 restore 前先挂载 Runtime typed subscriptions，再从同一个 owner restore snapshot 恢复 Session、Runtime 状态、transcript、workspace binding、pending Permission 和 pending UserInput，不使用 App Server cursor。Shared IPC Server 同样在执行 restore 前预订阅对应 Session，执行期间事件先进入连接 buffer；restore snapshot 在 Session mutation 边界内一次捕获 state 与 transcript，避免 response/subscription 窗口丢事件。Web App Server 仍使用自己的 `connection_id + stream + sequence` 与 connection-local resync 合同；它不是 CLI 的恢复路径。

当前未交付的是跨连接持久化 cursor、历史事件 replay 和断线后的透明 resume。Shared Runtime IPC v18 仍按自己的 lag/closed、断连取消和 controller 隔离规则工作；`CliAgentRuntimeClient` 的 Shared 分支只为当前 TUI connection 投影单调 cursor，不能把该投影描述为底层 IPC 已有 replay。

### 3.3 管理面状态

| Domain | 当前状态 | 当前结论 / 后续 |
| --- | --- | --- |
| Mode/Model 管理 | `TuiManagementOwners::model` 提供 secret-safe model catalog/CRUD/default projection；mode catalog 和 current-Session mutation 属于 Runtime client | Phase 5 已拆开管理 provider 与 Runtime port；Shared Session mutation仍由 v18 owner提交 |
| Skill/Subagent | `TuiManagementOwners::registry` 提供 typed list/toggle 与 visible/manageable projection | capability 属于当前 CLI Host scope，不进入 v18 wire |
| MCP | `TuiManagementOwners::mcp` 提供 typed catalog/status/toggle/add/delete/external decision/conflict projection | provider 的 MCP 进程状态和 tool registry 属于当前 CLI 进程；不能把本地 toggle 描述成 v18 远端控制 |
| External Source/Tool/Command/Agent | `external_source` 与 `external_command` provider 提供 typed snapshot/control/review、conflict choice、command expansion 和事件接口 | Shared V1 只发布实际可用 capability；V2 与 Remote unsupported 不回落本机 |
| Hooks | `native_hook` 与 `external_hook` provider 提供 typed overview/snapshot/plan/apply/mutate API | native user hooks、compiled-in `post_call_hooks` 和 external hook catalog 继续分离，Remote 明确 unsupported |
| Account/Settings Sync | `account` / `settings_sync` provider 提供 typed snapshot/login/finalize/logout 与 sync start/snapshot/cancel/local-changed；凭据不进入 read model 或 Debug 输出 | provider 按当前 deployment/scope 调用真实 owner或返回 typed unsupported |
| Worktree | `worktree` provider 提供 typed repository status、bind/release 和 operation identity | Embedded 调用 Worktree owner；Shared/Remote 不可用时明确 unsupported |
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
| `src/apps/cli` | 拥有 `CliAgentRuntimeClient`、私有 `TuiRuntimePort`、Shared transport/进程生命周期、`TuiManagementOwners` 具体 provider 组合和 TUI-local effect；不拥有 App Server 或业务 owner |
| Runtime/Service/Product Domain owners | Session、Turn、Permission、Workspace、配置和其他业务权威事实 |

边界规则：

- protocol/client 的依赖闭包不得引入 `bitfun-core`、Runtime 实现、Service 实现、UI framework 或 `product-full`。
- `bitfun-app-server` 可依赖生产 handler 所需的明确 owner feature，但禁止选择 `bitfun-core/product-full`。
- Host 负责 transport、认证、作用域、真实 capability/limits、平台能力和进程生命周期。
- handler 只做合同校验、DTO 转换和错误映射，不持有第二份业务权威状态。
- Phase 5 已交付的 `TuiRuntimePort` 只抽取 Embedded/Shared 共同需要的 Runtime 行为；不定义总括性的 `TuiManagementPort`。
- `TuiManagementOwners` 已将管理面按 domain 组合到具体 owner provider；只有需要 DTO、权限/上下文
  适配或 capability 裁剪时才增加薄 facade，不能把具体 service 实现或总括性的 App Server management service
  整体暴露给 TUI。
- DTO 提取不代表 Runtime owner 迁移。

## 5. 分阶段状态

计划状态以完成条件和验证证据为准，不以 method 数量或文件存在为准：

| 阶段 | 完成条件 | 验证方式 | 当前状态 | 验证记录 |
| --- | --- | --- | --- | --- |
| Phase 0：边界 | `TuiBackend`、behavior-light protocol/client crate、source/Cargo guard 已建立 | Core boundary tests 和 dependency checks | 已完成 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 1：协议基础 | initialize/health、typed events、connection-local cursor、resync、稳定错误和 Embedded connection 已接线 | App Server protocol/client/server focused tests | 已完成 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 2：核心聊天（旧路径） | Embedded 核心用例经 App Server；Shared 经同一 `TuiBackend` 映射当时的 Runtime IPC；TUI 核心不引用 Runtime SDK/IPC operation | CLI、App Server、Runtime IPC 和 boundary focused tests | 已完成当前定义，作为迁移基线 | [PR #2034 checks](https://github.com/GCWing/BitFun/pull/2034/checks) |
| Phase 3：配置管理 | TUI controller 不再访问 config/registry/MCP compatibility owner；secret-safe typed APIs 完成，CLI Host adapter 可保留显式 compatibility forwarding | owner tests、App Server contract tests、CLI behavior tests | 已完成当前定义 | 本变更的 protocol/client/server/CLI focused tests 与 Core boundary checks |
| Phase 4：外部集成 | External Source、Hook、Account、Settings Sync、Worktree 管理面经 typed backend；remote 不回落本机 | owner/remote/security contract tests | 已完成当前定义 | [PR #2146 checks](https://github.com/GCWing/BitFun/pull/2146/checks)、zero-budget contract 与 Core boundary checks |
| Phase 5：Embedded direct-runtime | `TuiRuntimePort` 与 owner provider 接入边界拆分完成；Embedded TUI 删除 App Server client/server 与 in-memory transport，改用 direct Runtime；管理面不进入 Shared IPC，旧路径随切换删除 | Runtime client/port focused tests；direct-runtime 与 Shared v18 的 deployment-specific 行为测试；各管理 provider 的 capability/unsupported 测试；Core boundary checks；统一跨部署 fixture 作为后续证据门槛 | 实现切换完成；统一 fixture 与性能/升级证据待补 | 本变更的 CLI focused tests 与 Core boundary checks；不把源码形状断言计作跨部署行为证据 |
| Phase 6：Shared App Server | Shared Host 达到 v18 治理等价，opt-in 双栈验证完成，并有回滚与删除证据 | 跨 transport parity、故障、性能和安全测试 | 未开始，目标待评审 | - |

### 5.1 Phase 0-2 历史迁移基线

- Phase 0-2 曾由 app-local `TuiBackend` 隔离 controller 与后端，Runtime 调用和管理调用不下沉到 view/reducer；该接口 family 已在 Phase 5 删除。
- Embedded Host 曾在专用 OS 线程运行 private `BitfunAppServer`；这是已退役的行为基线，不是当前路径。
- `AppServerTuiBackend` / `SharedTuiBackend` 曾分别映射 App Server 与当时的 private Runtime IPC；Phase 5 已由 `CliAgentRuntimeClient` 的 Direct/Shared 分支替换。
- App Server 核心 handler 覆盖 sync、turn、Permission、revert、context、usage、settlement、Workspace 和 lineage；Config 事件也已在 Embedded connection 接线。
- Runtime IPC v17 为当前 parity 增加 restore Runtime 状态、usage、settlement 和本地命令 transcript 记录；v18 再增加权威 pending UserInput restore 与 post-registration ready 事件。两者都没有增加 replay、observer、通用 controller transfer 或公开 SDK 能力。
- capability 声明列出当前注册方法，但 Host-specific availability 和方向性 limits 仍是后续收紧项。

### 5.2 Phase 3

目标：移除 TUI 对全局 config、registry 和 MCP service 的直接访问。

状态：已完成当前定义。

完成条件：

- 模型、Mode、Skill、Subagent 和 MCP 使用 owner-specific typed APIs。
- secret 不出现在 read model、日志或 generic config payload 中。
- capability 由真实 Host-bound owner provider、授权和健康状态决定；当前通用 App Server Host 没有管理 owner。
- owner 未注入时返回明确 structured unsupported；Shared 的本机 compatibility forwarding 必须显式装配并发布真实 capability，不能在 Remote workspace 静默回落控制端本机。

交付摘要：

- `app-server-protocol` 提供 Mode、Model、Skill、Subagent 和 MCP 的 owner-specific DTO 与 method；model read model 不返回 secret 值，model mutation 使用 preserve/replace/clear 语义。
- App Server 不保留无生产消费者的管理 service；当前通用 Server Host 未注入 management owner，相关方法返回带 capability id 的 structured unsupported。未来只有真实 Host 消费者具备 Host-bound scope/auth contract 后，才可增加按 owner 注入的窄 adapter，不能把 test-only seam 写成已交付 Web 管理能力。
- CLI Phase 5 已改由 `TuiManagementOwners` 的具体 provider 调用真实 owner，不建立 direct TUI 总管理接口，也不复制一套 App Server 业务编排。
- `CliAgentRuntimeClient` 的 Shared 分支继续映射 v18 mode catalog 和 current-Session model mutation；Model、Skill、Subagent 管理由 `TuiManagementOwners` 的具体 provider 提供。v18 不承载这些目录、CRUD 或 defaults。MCP 进程属于 Shared Runtime Host，controller 不启动重复 service；当前 Shared TUI 的 MCP 管理返回 typed unsupported，并要求退出 Shared clients、在 Embedded 模式管理后重启 Shared Runtime。
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

- `app-server-protocol` 与 client 保留 External Source、native/external Hook、Account、Settings Sync 和 Worktree 的 owner-specific typed method；通用 Host 注册的 management handlers 当前只返回 structured unsupported。未来 scoped Host adapter 的 side-effecting 请求仍必须使用 operation identity，并保留 owner revision/stale、显式取消与 snapshot 合同。
- Startup 和 Chat controller 只经 `CliAgentRuntimeClient` 或 `TuiManagementOwners` 的 typed API 调用这些用例。Phase 4 涉及的 `bitfun_core`、account/account-sync compatibility marker 已从 controller 文件移除，对应 Core boundary budget 固定为零；Phase 5 已完成 Runtime port 与按 domain 的 management composition。
- 已退役的 Embedded App Server 注入路径不再承担账户或 Worktree 管理；Direct CLI 通过具体 provider 复用共享 `AccountRuntime` 与 Worktree owner。未来真实 Host 若接线 App Server management，必须先接收 Host-bound scope/auth，并且只能直接适配这些 owner；不得定义 `AccountManagementHost` 或持有第二份账户、同步、外部来源、Hook、Worktree 权威状态。CLI 的窄 `AccountRuntimeHost` 只实现 daemon、Relay/Peer 路由宿主效果，Session 备份通过独立端口读取 Agent Runtime compatibility owner。
- Shared adapter 只发布 Host 实际可用的 capability。External Source V1 与 Hook 管理可使用当前本机 compatibility service；Account/Settings Sync、Worktree、Remote workspace 和后续未接线的 External Application V2 返回 typed unsupported，不静默回落本机。
- Phase 4 未扩展当时的 private Runtime IPC v17，也未改变 Phase 6 的 Shared transport 评审门槛。

### 5.4 Phase 5

Embedded direct-runtime Phase 5 已完成当前定义：

1. `CliAgentRuntimeClient` 内部的窄 `TuiRuntimePort` 以 `Embedded(AgentRuntime)` 与
   `Shared(RuntimeIpcClient)` 统一 TUI semantic request/result/event/error；direct facade 与 v18
   wire 不共享 DTO。
2. Runtime 与管理边界已拆开：Runtime 调用进入 client 私有 port；Model/Skill/Subagent/MCP/
   Account/Settings/Worktree/External Source/Hook 按 domain 进入 `TuiManagementOwners` 的具体 provider，
   不创建 `TuiManagementPort` 总接口。
3. App Server 不保留无生产消费者的 management service；相关方法保持 structured unsupported，
   CLI provider 复用真实 owner use case 或最薄 projection，不复制业务状态和策略。
4. Embedded Host 已切换为 direct Runtime，CLI 的 in-memory transport、App Server thread、
   initialize/health wire handshake 与旧 backend family 已删除。
5. Chat、Session、Permission/UserInput、Workspace 和 Management 使用 focused tests 与 boundary rules
   锁定事件、取消、unknown outcome、workspace/execution binding、错误映射和 shutdown 行为。
6. 旧 App Server 不作为 rollback adapter；缺少的 deployment/provider capability 返回 typed unsupported。

Shared App Server Phase 6 不以“删除 v18”为起点。建议顺序：

1. 在 Shared Host 中增加默认关闭的 App Server local transport。
2. 两条 transport 复用同一 Host-scoped connection authority、controller registry、Session 事件过滤、operation identity/deadline/cancel 和未知结果登记。
3. 使用一个第一方 Rich Client 进行 opt-in 双栈验证，覆盖跨 transport 竞争、断连、迟到结果、Host 崩溃和回滚。
4. 记录 startup、延迟、内存、frame/queue 上限和长期维护成本。
5. 只有行为、安全、恢复和性能达到完成门槛后，才评审是否切换 `--shared` 默认实现并删除 v18。

保留 private v18 作为稳定终态也是允许的：只要业务用例和 owner 仍统一，物理 wire 不必为了形式统一而提前收敛。

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
| Deployment | 当前分别覆盖 Embedded direct Runtime 与 Shared v18 compatibility 的关键路径；旧 Embedded App Server 不再是测试或回滚路径 |

当前 focused tests 分别覆盖 direct Runtime 和 Shared v18 的关键行为；统一跨部署 fixture 尚未完成，仍是后续验证门槛。旧 Embedded App Server 只保留在历史证据中，不维护第三条回滚路径；Shared App Server 实现后再增加候选 transport 路径。

## 7. 完成定义

只有同时满足以下条件，才能宣布 TUI/App Server 解耦完成：

1. Phase 3/4 当前定义的管理面已迁移；Phase 5 direct-runtime 已完成；后续新增 capability 也不得绕过 `CliAgentRuntimeClient` / `TuiManagementOwners` composition 或恢复旧 owner 直连。
2. TUI 产品请求和订阅经过 `CliAgentRuntimeClient`；Runtime 行为经过其私有 `TuiRuntimePort`，管理能力经过 `TuiManagementOwners` 的具体 owner provider，TUI view/reducer 不执行 backend I/O。
3. protocol/client 和 TUI-facing 依赖闭包不包含 Core、Runtime/Service 实现、`product-full` 或 private IPC operation；只有 CLI Host/backend composition 可以按 owner 注入已审核的 service/provider。
4. capability、limits、身份和作用域来自真实 Host/transport，而不是通用 protocol 默认值；管理 service/provider 缺失时返回 typed unsupported。
5. 事件、断线、恢复、权限、取消和 unknown outcome 有明确合同与故障测试；Runtime port 的每个 Shared v18 operation 都有对应覆盖证据。
6. remote workspace 不存在 controller-local fallback。
7. 重复 DTO、无效 handler、旧单体 `TuiBackend` family 和无生产消费方的旁路已删除或不再属于稳定边界。
8. 旧 Embedded App Server 已删除且不再作为 rollback adapter；若采用 Shared App Server，迁移满足 Phase 6 的双栈、回滚、性能、安全和删除门槛；否则文档明确 v18 是保留的私有 compatibility transport。
