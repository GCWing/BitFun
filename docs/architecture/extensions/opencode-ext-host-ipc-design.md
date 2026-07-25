# OpenCode Ext-Host IPC 集成设计

本文定义 BitFun 如何沿现有插件运行时主链路集成 OpenCode Extension Host（Bun），并以一个真实 Tool 或稳定 Hook
consumer 完成 package plugin 的最小可验证闭环。本文是
[`opencode-plugin-runtime-adapter-design.md`](opencode-plugin-runtime-adapter-design.md) 的协议落地补充，并受
[`plugin-runtime-design.md`](plugin-runtime-design.md) 和
[`../product-architecture.md`](../product-architecture.md) 约束；发生冲突时以上位设计为准。

本文同时区分目标设计和当前实现。当前 BitFun 尚未集成 Bun Host；冻结的 ext-host 协议也还不能安全表达本文要求的
不可变 npm target，因此不能把设计目标写成已交付能力。

协议事实以公开 ext-host 仓库的 `PROTOCOL.md`、`src/protocol.ts` 和生成的 `protocol.schema.json` 为准（见 §6）。
Rust fixture 必须与冻结提交保持 wire-level 一致，不能仅按本文示例实现近似协议。

## 1. 架构定位

### 1.1 唯一主链

```mermaid
flowchart LR
  Source["来源与激活 owner"]
  Prepare["依赖准备服务"]
  Capability["Tool / Hook 等能力 owner"]
  Client["PluginRuntimeClient"]
  Adapter["OpenCode executable adapter"]
  Process["services 进程与 IPC 实现"]
  Host["Bun Plugin Host"]

  Source -->|"已批准来源"| Prepare
  Prepare -->|"不可变 prepared target + attestation"| Source
  Source -->|"当前 activation authority"| Capability
  Capability -->|"真实 consumer 调用"| Client
  Client --> Adapter
  Adapter -->|"provider-neutral 执行控制"| Process
  Process <-->|"JSON-RPC / loopback"| Host
  Host -->|"候选 contribution / 调用结果"| Adapter
  Adapter --> Client
  Client --> Capability
```

各层只有以下职责：

| Owner | 负责 | 不负责 |
|---|---|---|
| 外部来源与激活 owner | 来源身份、候选版本、用户选择、最终物化摘要、activation authority、停用和撤销事实 | 持有 Bun 进程、解释 Hook、注册 Tool |
| 依赖准备服务 | 在不 import 插件代码且禁用 install scripts 的条件下解析、安装、冻结并证明完整依赖闭包 | 授予执行权限、调用插件、保存产品激活状态 |
| `PluginRuntimeClient` | 请求身份、期限、队列、同实例顺序、取消、迟到结果拒绝、诊断和 fault 状态 | 来源发现、物理进程、业务状态提交 |
| OpenCode executable adapter | OpenCode 加载顺序、RPC 参数/结果、Hook/Tool 语义转换 | 产品策略、进程监督、贡献最终提交 |
| services 实现 | prepared target 校验、Host spawn、IPC、连接代次、物理健康、重启预算和进程树回收 | Session 生命周期、权限决定、Tool/Hook 最终状态 |
| Tool/Hook/Permission 等能力 owner | 以自身不变量校验并原子提交贡献、权限和调用结果 | import JS/TS、管理 Bun 进程 |
| Product Assembly | 选择 adapter/provider/service 实现并注入唯一主链 | 发现来源、准备依赖、执行插件、拥有 Session/Host 生命周期 |

`runtime-ports` 只在确有第二个 owner 和真实 consumer 时承载窄、provider-neutral 的执行控制 DTO/port。它不得出现
`ExtHostRuntime`、Session holder、workspace Host manager、物理清理状态或 ext-host wire 类型。Rust 侧所谓 Host 始终是
services 管理的 Bun 子进程，不新增第二个 Runtime/Controller owner。

### 1.2 当前静态预览不是执行入口

`src/crates/adapters/opencode-adapter` 中现有 `load_opencode_package_adapter` 保持 static-preview only。它可以投影候选和
诊断，但不得 spawn Host、安装依赖、import module、注册 Tool/Hook，或被扩展为新的受管 OpenCode 执行路径。

真实 package plugin 必须使用单独的 executable adapter 实现现有 `PluginRuntimeClient` 主链，并由 Product Assembly 注入。
Assembly 只选择实现；来源协调器在后台准备 target，各能力 owner 才是 registrations 和调用的真实 consumer。

### 1.3 依赖方向

```text
能力 owner -> PluginRuntimeClient -> OpenCode executable adapter
                                     -> provider-neutral execution port
                                     -> services process/IPC implementation
                                     -> Bun Plugin Host
```

services 和 adapter 不依赖 Assembly；`PluginRuntimeClient` 不依赖具体 OpenCode adapter 或 services；Session owner 不直接
引用 ext-host、TCP、进程句柄或 contribution registration。

## 2. 安全边界：不可变 package target

### 2.1 两阶段批准与准备

来源批准只允许 BitFun 在不执行第三方代码的前提下读取元数据、下载包并准备候选；它不授予 import 权限。真正的激活批准
必须绑定最终将交给 Host 的完整物化结果：

```mermaid
sequenceDiagram
  participant User
  participant Source as 来源/激活 owner
  participant Prepare as 依赖准备服务
  participant Caps as 能力 owner
  participant Client as PluginRuntimeClient
  participant Host as Bun Host

  Source->>User: 展示来源、npm spec 和准备风险
  User->>Source: 批准来源与无脚本准备
  Source->>Prepare: prepare(exact source identity)
  Prepare->>Prepare: resolve + fetch + install with scripts disabled
  Prepare->>Prepare: freeze full closure and compute attestation
  Prepare-->>Source: prepared target + materialization digest
  Source->>User: 展示入口、锁文件、完整依赖闭包和最终摘要
  User->>Source: 批准该 materialization digest
  Source-->>Caps: activation authority for immutable target
  Caps->>Client: activate/调用已批准 target
  Client->>Host: instance.open(prepared local target)
  Host-->>Client: candidate contributions
  Client-->>Caps: 校验后的候选
  Caps->>Caps: 原子提交自己的状态
```

准备阶段必须满足：

1. npm spec 先解析为精确 package name、version、registry integrity 和入口；禁止 `latest`、tag 或 semver range 进入激活摘要。
2. 使用受支持的 npm/Arborist 语义和锁文件冻结完整传递依赖闭包；没有可验证 lock/integrity 的候选不得激活。
3. install scripts、生命周期脚本和准备期 module import 全部禁用。若某包必须运行脚本才能工作，P0 明确不支持。
4. attestation 至少包含规范来源身份、精确入口、lockfile 摘要、每个物化文件的相对路径/类型/摘要、完整依赖图、
   registry integrity、运行时及 adapter 版本、解析/安装选项和目标平台。
5. 用户确认的是最终 `materialization_digest`，不是 manifest 摘要、裸 spec 或可变缓存目录。
6. 确认后把结果提升到只读、内容寻址的 target。Host 只能读取该 target；不得获得准备服务的可写 cache、npm 配置或安装锁。
7. 物化结果、authority、当前执行域/用户、凭据和策略范围在 open 前复核；任一不匹配都 fail closed。

如果目标平台不能向 Host 提供不可变或 OS 强制只读的 prepared target，P0 在该平台返回明确不支持，不能退回可写 cache。
独立进程仍不是完整安全沙箱：获批代码按当前执行域的真实 OS 权限运行，文件、网络和子进程残余风险必须在确认界面展示。

### 2.2 `instance.open` 不得解析 npm

冻结 ext-host 实现会把裸包名解析为 `@latest`，并在可写 `cacheDirectory` 中执行
`bun add --ignore-scripts --exact <spec>`。该行为不能用于 BitFun P0，因为它使用户批准的对象与最终 import 的代码脱钩。

BitFun 选择 Host 外准备路线。协议升级后，`host.instance.open` 的插件项必须是 tagged prepared target，例如：

```json
{
  "kind": "prepared",
  "targetID": "sha256:...",
  "entrypoint": "file:///bitfun/plugin-targets/sha256-.../entry.js",
  "materializationDigest": "sha256:..."
}
```

以上只是目标 shape；字段名必须先进入 ext-host Zod 事实源、生成 schema 和跨语言 fixture，不能仅由 Rust 私自发送。
Host 在 BitFun profile 下必须拒绝裸字符串、npm spec、registry URL、相对源路径和 legacy install 选项；不得在
`instance.open` 内联网、运行 package manager 或写入 target。Host 返回 target id/digest，Rust 核对后才能发布贡献。

如果 ext-host 选择保留 Host 内安装作为其他 consumer 的兼容路径，则该路径必须与 BitFun profile 显式区分；BitFun
仍只发送 `kind = prepared`。本文不采用“Host 安装后返回清单、Rust 再追认”的方案，因为 import 前校验更容易形成
单一、可审计的安全边界。

### 2.3 Authority 与竞态

`PluginActivationAuthority` 继续由来源/激活 owner 生成和撤销。内部 prepared target handle 不支持 serde，不接受 UI DTO，
至少绑定：

- 精确来源、package 和内容身份；
- `materialization_digest` 与只读内容地址；
- execution domain、实际 OS 用户、目标平台和 runtime/adapter 版本；
- activation epoch、策略上限、凭据和环境可见范围；
- 被允许的 capability envelope。

构造 wire request 的函数保持 services 实现私有，只接收经过当前 authority 复核的内部 handle。open 期间发生撤销、摘要变化
或策略收紧时，不发布 contribution；停止接受该 target 的新调用，撤下已发布贡献，并按 §3.4 的安全重启顺序回收包含它的
Host。仅在下一次 RPC 前检查 authority 不足以终止 timer、后台任务或 import 时副作用。

### 2.4 Remote fail closed

package plugin 必须在工作区真实 execution domain 中完成准备、保存 prepared target 并运行 Host：

- Remote RuntimeServices 可用时，依赖准备、Host、working directory、网络、凭据和进程树都位于远端；
- 远端能力缺失或断线时显示 `RemotePluginRuntimeUnavailable`，不在本机补偿执行；
- 本地界面只代理状态和调用，不把远端路径当成本地路径；
- execution domain 或远端身份变化会使旧 authority 和 prepared target handle 失效。

## 3. 生命周期与故障域

### 3.1 三层定义

本文保留三层生命周期，但不把 Session、workspace 或单个 plugin 作为物理进程层。在运行环境和安全范围兼容的默认
部署中，每个承载 `RuntimeServices` 的 BitFun 后端进程维护一个共享 Plugin Host，即后端与 Host 默认 1:1。这里的
1:1 表示一个后端默认只有一个共享 Host 进程；Host 可以按真实插件使用延迟启动，但首个完整 package-plugin 实现不做
通用空闲回收，并在后端存活期间持续复用该 Host。

```text
RuntimeServices backend
└── Shared Plugin Host
    ├── workspace A / plugin instances
    ├── workspace B / plugin instances
    └── calls from multiple sessions
```

| 层次 | 默认键 | Owner | 退出条件 |
|---|---|---|---|
| Host/connection generation | 当前 `RuntimeServices` backend 内兼容的执行域、OS 用户、runtime 和安全范围 | services | 安全重启、故障回收或 `RuntimeServices` backend 退出 |
| logical plugin instance/contribution generation | 来源、插件身份、内容版本及必要的 workspace-specific state | 来源与能力 owner；Host 保存易失 module instance | 停用、替换、authority 失效或 Host generation 被回收 |
| invocation | request id、调用类别、execution id、session/turn/cancel context | `PluginRuntimeClient` 与调用能力 owner | 成功、错误、取消、期限或 Host generation 丢失 |

workspace、plugin 和 session 只区分 Host 内的逻辑实例、调用身份、取消和权限上下文，不决定物理进程数量；请求必须
显式携带这些身份。只有 execution domain、OS 用户、runtime 或安全范围不兼容时，才允许在同一产品部署中拆分额外 Host。
仅为了单插件故障隔离、workspace 数量、Session 数量或容量压力而改变默认 1:1 模型，必须先修改并评审上位
`plugin-runtime-design.md`，并给出安全收益、启动/内存数据、加载顺序与模块缓存兼容差异以及行为等价测试。

Session 归档或删除只取消该 Session 的在途 invocation，并由 Session owner 提交自身状态。它不关闭 Host、不撤下共享
contribution，也不持有 Host lease。Host 是否仍被事件订阅、后台任务、其他 Client、其他 workspace 或其他插件使用，
由真实插件使用与 services 进程状态决定。

四类生命周期事件按以下规则处理：

- **Session 关闭**：只取消该 Session 的在途调用并清理调用上下文，不关闭 Host，也不移除其他 Session 可见的贡献；
- **Workspace 关闭**：停用并移除只属于该 workspace 的逻辑 plugin instances；不默认关闭共享 Host；
- **插件停用或更新**：更新逻辑实例和贡献；若无法在现有进程内可靠卸载旧模块，则按 §3.4 安全重启共享 Host；
- **后端退出**：关闭共享 Host，等待 EOF 与主进程退出，并回收完整受管进程树。

### 3.2 后台启动与 target 隔离

来源协调器在准备/激活后后台维护每个 logical target 的状态：

```text
Discovered -> Preparing -> AwaitingApproval -> Activating -> Ready
                                            \-> Degraded / Disabled
```

GUI/TUI 的 Session 创建、恢复和聊天入口不得等待所有插件 spawn、handshake 或 registration。只有完成校验并由能力 owner
原子发布的贡献才可见；单个 target 的 Bun 缺失、准备失败、import 错误或注册失败只使该 target 进入 Degraded，不使无关
Session 创建失败。初始化按固定 OpenCode 顺序执行；单插件普通异常回滚其候选贡献并继续，协议损坏或事件循环失活则升级为
Host generation 故障。

### 3.3 有界启动

services 的启动 future 必须在统一 startup deadline 内同时等待以下事件，而不是裸等 `listener.accept()`：

| 事件 | 处理 |
|---|---|
| loopback accept + handshake 成功 | 绑定新的 connection generation，继续初始化 |
| child 提前退出 | 返回带 stderr 摘要的 target/Host 诊断 |
| startup deadline | 关闭 listener/socket，终止并确认回收进程树 |
| target activation 被取消或 authority 失效 | 中止启动并回收，不发布贡献 |
| RuntimeServices/application shutdown | 中止启动并进入全局清理 |
| 握手 token、版本或 schema 不匹配 | fail closed，关闭连接并回收 child |

每次 spawn 都使用不可预测的一次性 token，只接受 loopback 对端；成功连接必须绑定 child handle 和 connection generation。
旧连接的 response、notification、EOF 和健康任务不得改变新 generation 状态。

### 3.4 停用、更新与安全重启

模块 import 可以创建 timer、后台任务和其他进程，不能假设 `instance.close` 能卸载任意 JS module。停用或替换共享 Host 中
的一个插件时，沿 `plugin-runtime-design.md` 执行整个 Host 的安全重启：

1. 来源 owner 阻止受影响 target 的新激活，能力 owner 暂停新调用；
2. `PluginRuntimeClient` 有界结算或取消该 Host generation 的在途调用；
3. services 请求 graceful shutdown，并等待响应、TCP EOF、主进程退出和受管后代回收；
4. 旧进程树未确认停止前不得加载新代码；
5. 能力 owner 按旧 contribution generation 撤下贡献；
6. services 使用仍然合规的 immutable targets 启动新 Host；
7. adapter 校验候选，能力 owner 原子发布新的 contribution generation；
8. Client 恢复接受调用。

安全撤销可跳过等待业务调用自然完成，但不能跳过进程树确认。新 Host 加载失败时插件保持不可用；只有存在完整、已批准且
校验通过的旧 prepared target 时，才可按同一停机顺序恢复旧版本。禁止让新旧 Host 同时执行同一组插件。

### 3.5 正常退出和进程树

首个完整 package-plugin 实现不做通用 idle reclaim。只要仍有 active plugin instance、事件订阅、后台任务、在途调用或
其他 Client，兼容 Host 保持运行。RuntimeServices 退出时停止新调用、取消可取消请求、有界等待、逆序 dispose、关闭连接，
并回收完整进程树。

Windows 使用 `ProcessTreeChild` 在 suspended child 恢复前附加 kill-on-close Job Object；附加失败必须 fail closed。
Unix 使用独立 process group。graceful shutdown 响应不等于物理退出，必须继续观察 EOF、child exit 和受管后代回收。
这些机制只提供生命周期 containment，不是 CPU、内存、文件或网络沙箱；Unix 主动脱离 process group 的后代仍是残余风险。

### 3.6 崩溃恢复

Host generation 丢失时：

- services 一次性报告该物理故障并结算绑定该连接的所有 pending request；
- 能力 owner 撤下受影响 contribution，不能让每个插件分别消耗一份进程级重启预算；
- 未知结果的有副作用调用返回 `OutcomeUnknown`，不自动重放；
- 只在来源、authority、prepared target 和 RuntimeServices 仍有效时，按有界预算重载整组插件；
- 超出预算后保持 Degraded，Session 和其他非插件能力继续可用。

## 4. IPC 调用可靠性

### 4.1 传输与握手

冻结协议使用 loopback TCP、4-byte big-endian 长度前缀和 JSON-RPC 2.0。实现必须限制单帧大小、解码深度、队列条目、
总在途字节和 stream 分片；解析错误关闭连接。握手至少校验一次性 token、协议版本、Host build identity 和所需 capability。

连接成功后生成不可复用的 `connection_generation`。每个 pending request、stream、execution 和 notification 都绑定该 generation；
重连后收到的旧消息只记有界诊断，不提交状态。

### 4.2 Instance 生命周期

`host.instance.open` 只接收 §2.2 的 prepared target、逻辑 project/workspace context 和经过裁剪的配置。成功响应表示 Host
完成该 instance 的加载，但返回内容仍只是候选；adapter 校验顺序、标识和 schema 后，各能力 owner 才能发布。

`host.instance.close` 是 best-effort 的逻辑清理提示，不是任意模块已经卸载或子进程已退出的证明。停用、更新和 authority
撤销遵循 §3.4 的 Host 安全重启。

`host.shutdown` 成功后 Rust 继续等待 TCP EOF 和 child exit；deadline 后关闭 socket 并终止受管进程树。重复 shutdown、
EOF 先到和 child 已退出按同一 generation 幂等结算。

### 4.3 请求模型、期限与背压

`PluginRuntimeClient` 的内部请求至少携带：

| 字段 | 用途 |
|---|---|
| request id + connection generation | 拒绝重复或旧连接结果 |
| logical plugin instance id + contribution generation | 路由并阻止旧贡献提交 |
| call kind | 区分 Tool、Hook、Auth、Provider 等取消/重试策略 |
| execution id | 对应 `host.tool.cancel` 和审计身份 |
| session/turn/caller context | 取消树、权限和工作目录，不作为 Host 进程键 |
| deadline + cancellation token | 有界等待并传播取消 |
| idempotency key / effect class | 仅在 owner 声明安全时允许有限重试 |

每个 Host/instance 和全局队列都有条目数与字节数上限。队列满立即返回 typed `Overloaded`；调用方可以降级，但有副作用
调用不得因过载自动重试。并发额度来自真实资源测量和调用类别，不由 workspace 或 Session 数量推导。

### 4.4 Tool 取消

Tool timeout 或 cancellation token 触发时不能只删除 Rust pending entry：

1. Client 把原调用标记为 cancelling，拒绝其普通完成结果提交；
2. 在仍匹配的 connection generation 上发送 `host.tool.cancel { instanceID, executionID }`；
3. 有界等待取消确认或原调用终止；
4. 成功确认后以 Cancelled/TimedOut 结算，并丢弃以后迟到消息；
5. Host 不确认、连接失活或取消期限到期时，把该 Host generation 标记为 poisoned，停止新调用并执行 §3.4 回收；
6. 受影响调用返回 `OutcomeUnknown` 或更精确的 typed fault，不自动重放。

共享 Host 被 poison 时，其承载的其他 logical instances 也会短暂不可用。这是共享进程的明确故障域，不能伪装成只回收某个
workspace target。

### 4.5 不可取消调用和迟到结果

冻结协议没有 `host.hook.cancel`。Hook timeout/cancel 时先用 request、connection 和 contribution generation fence 拒绝迟到
结果；如果 Hook 可能仍在运行、继续产生副作用或阻塞 Host，则 poison 并回收整个 Host generation。只丢弃结果不能证明
副作用停止。

Auth/Provider/stream 等类别必须按冻结 schema 明确是否有 cancel RPC。没有协议级取消且不能证明调用已经结束时，使用相同
poison/recycle 规则。旧连接迟到 response、notification 和 stream chunk 不更新 Tool、Hook、Permission、Session 或审计状态。

### 4.6 Backend facade

Host -> BitFun 的 `backend.*` 方法只能调用现有 owner 的窄接口：

| 方法 | 边界 |
|---|---|
| `backend.handshake` | token、版本、capability 和 build identity 校验 |
| `backend.http.request` | per-instance gateway 转发；按当前 authority 和网络策略复核 |
| `backend.auth.get` | 每次读取凭据 owner 的当前值，不在 Host 持久缓存 |
| `backend.tool.ask` | 转发给 Permission/Tool owner；Host 不决定权限 |
| `backend.tool.metadata` | 接收并校验候选 metadata，不直接提交产品状态 |
| `backend.diagnostic.publish` | 有界、脱敏诊断 |
| `backend.stream.read/cancel` | 只操作 Rust-owned stream handle |

反向调用同样绑定 instance、connection generation、authority 和有界队列。Host 不能通过 backend facade 获得任意 service
locator，也不能把 instance id 当作充分授权。

### 4.7 错误分类

Rust 必须按 JSON-RPC code、`data.kind`、call kind 和 connection generation 联合分类。至少区分：

- protocol/handshake failure：关闭连接并回收 Host；
- overloaded：调用级 typed failure；
- plugin initialization failure：回滚该插件候选，其他插件可继续；
- call failure：只结算该调用；
- cancellation unconfirmed / protocol corruption / event-loop stall：poison Host generation；
- process exit：结算整条连接并按进程级预算恢复；
- authority/materialization mismatch：安全拒绝，不计作普通插件 crash。

## 5. 仓库集成点

### 5.1 `runtime-ports` 与 `PluginRuntimeClient`

不新增 `ExtHostRuntime`。若现有 `ScriptToolRuntime` 不能承载 package Host，新增的公共面只能是由真实 Tool/Hook consumer
驱动的 provider-neutral execution port，表达以下操作和事实：

- 激活一个已批准的 prepared target handle；
- 按 typed call kind 调用；
- 按 execution id 取消；
- 停用 logical target；
- 查询只读 availability/fault；
- RuntimeServices shutdown。

该 port 不接受 npm spec、路径字符串、UI DTO 或 Session holder，不返回 OS handle、TCP socket、workspace lease、Bun wire
JSON 或物理清理内部状态。`PluginRuntimeClient` 继续负责 deadline、队列、调用次序、取消结算和旧连接结果拒绝；services
通过 port 返回 connection lost/poisoned/cleanup outcome 等窄事实。

先以一个真实 Tool 或稳定 Hook consumer 和固定 fixture 证明 API，再决定是否提升公共 trait。只为设计完整性新增、但没有
当前 consumer 的公开符号不得合入。

### 5.2 `opencode-adapter`

新增 executable adapter，而不是修改 `load_opencode_package_adapter` 的 static-preview 语义。它负责：

- 把来源 owner 已确定的 OpenCode 加载顺序转换为 prepared target activation；
- 把 Tool/Hook 请求和结果映射到现有 `PluginRuntimeClient` DTO；
- 对 ext-host candidate registrations 做版本、标识、顺序和 schema 校验；
- 对冻结协议不支持的能力返回 typed unsupported diagnostics。

它不安装 npm 包、不保存 approval/activation、不直接调用能力 owner 的 register 方法，也不拥有 Host 进程。

### 5.3 `services-integrations` 与 `services-core`

`services-integrations` 在现有 script runtime family 后实现依赖准备和 package Host execution port；`services-core` 的
`process_tree`/`process_manager` 继续提供受管进程树。实现必须包含：

- content-addressed prepared target store 与 attestation 校验；
- loopback listener、一次性 token 和 versioned handshake；
- §3.3 的 startup deadline/select；
- connection generation、pending request 和 bounded stream 管理；
- Tool cancel 与 poison/recycle；
- graceful shutdown、EOF/exit 观察和 `ProcessTreeChild` 兜底回收。

services 不解释 OpenCode Hook，不注册产品 Tool，不读取 Session manager，也不依赖 Assembly。

### 5.4 Product Assembly、来源协调器和能力 owner

`src/crates/assembly/core/src/plugin_runtime.rs` 仍只是经评审的 product-full composition：选择 executable adapter、services
provider 和 `PluginRuntimeClient`，然后注入现有 RuntimeServices。它不得新增 `Session::create`、等待所有 target ready、
消费 registrations 后直接注册 Tool/Hook，或接管来源/Host 生命周期。

后台来源协调器负责把当前 approved source 推进到 prepared/awaiting-approval/activated 状态，并把失效事实发送给现有 owner。
Tool/Hook owner 以 contribution generation 原子发布/撤下贡献。Session owner 只把 session/turn/cancel/permission context 传给
调用链；归档 Session 不触发 Host shutdown。

### 5.5 最小实施闭环

P0 只交付以下纵向闭环：

```text
approved immutable package target
  -> external dependency preparation + attestation
  -> final materialization approval
  -> unique Runtime/Services chain
  -> one real Tool or stable Hook consumer
  -> cancellable, recyclable, fixture-verified Host call
```

Client、Auth、Provider、Workspace 和其他 Hook 只有在该闭环通过跨语言 fixture、故障注入和 owner 测试后再扩展。

## 6. 协议、许可证与交付

### 6.1 冻结事实源

- 实现仓库：[`ztpublic/opencode-ext-host`](https://github.com/ztpublic/opencode-ext-host)
- 协议文档：`PROTOCOL.md`
- Zod 事实源：`src/protocol.ts`
- 生成 schema：`protocol.schema.json`
- 当前审计提交：[`e084c921b68c1b3588a1c18409a5b85aa906b3a7`](https://github.com/ztpublic/opencode-ext-host/commit/e084c921b68c1b3588a1c18409a5b85aa906b3a7)
- 当前 package：`@opencode-ai/extension-host@0.1.0`
- 当前兼容目标：`@opencode-ai/plugin@1.17.18`

该提交仍允许 Host 解析/安装 npm spec，因此只可作为当前审计基线，不能原样作为 BitFun P0 的安全协议。实现前必须先把
§2.2 prepared target tagged union、拒绝 legacy spec 的 BitFun profile、取消确认语义和对应 fixture 固定到新的公开提交。

### 6.2 仓库内快照

升级后的冻结协议需在 `docs/protocols/opencode-ext-host-v1/`（或与新 major 对应的目录）保存：

- `PROTOCOL.md` 快照；
- 生成的 `protocol.schema.json`；
- Rust/Bun 共用的 request、response、cancel、late-result 和 malformed fixtures；
- 上游 commit、package version、生成命令和校验摘要。

快照目录当前不存在，因此这是实现前置条件，不是已完成事实。schema、Host、Rust codec 和本文必须在同一变更中升级。

### 6.3 许可证与供应链

复制协议/schema、构建或随产品分发 Host 前必须完成并记录：

1. ext-host、OpenCode plugin SDK、Bun/runtime 和所有分发依赖的许可证及 notice 义务；
2. 从冻结 commit 到可分发 artifact 的可复现构建步骤、锁文件和 provenance；
3. artifact 摘要、签名验证和构建/发布身份；
4. Windows、macOS、Linux 的 Bun/runtime 获取、打包、启动和卸载策略；
5. Host 与 Rust 的兼容矩阵、升级/回滚、旧版本拒绝和安全公告流程。

以上任一项不明确时，不得复制或分发外部实现，也不得在运行时从未固定来源下载 Host。

## 7. 验证要求

### 7.1 安全与物化

- 裸 package name、`@latest`、tag、range、registry URL、相对路径和 UI DTO 无法到达 Host open；
- package version 或任一传递依赖在最终确认前变化会产生不同 digest；确认后变化 fail closed；
- lockfile、入口、完整依赖图、每个文件摘要、runtime/adapter/install options 都参与 attestation；
- install scripts 和准备期 import 不执行；无锁或 integrity 不完整的候选拒绝；
- Host 只能读取 prepared target，不能访问可写安装 cache 或在 open 中联网解析；
- authority 在 open 前后撤销时不发布 contribution，并回收执行过该 target 的 Host；
- symlink/reparse point、目录逃逸、非普通文件和确认到 import 的替换竞态被拒绝；
- Remote 缺少远端 RuntimeServices 时明确拒绝，不本机回退。

### 7.2 Owner 与生命周期

- 同一 RuntimeServices 中兼容插件和多个 workspace 默认共享 Host；Session 数量不改变 PID；
- Session 创建/恢复不等待插件启动，单插件失败不击穿无关 Session；
- Session 归档只取消自己的调用，不关闭仍被插件实例、订阅、后台任务或其他 Client 使用的 Host；
- 停用一个共享 Host 内的插件按整 Host 安全重启，旧进程树退出后才加载新组；
- capability owner 原子发布/撤下 contribution，Assembly、adapter、services 不建立第二份注册状态；
- process crash 只消费一次 Host generation 重启预算，所有旧连接结果被拒绝。

### 7.3 启动、取消与故障注入

- Bun 缺失、spawn 失败、启动后不连接、握手卡住/错误、child 提前退出和 application shutdown 均在期限内结算；
- startup timeout 后 listener、socket、主进程和受管后代全部回收；
- Tool timeout/cancel 发送准确 `instanceID + executionID`，有界等待确认；
- cancel 不确认时 poison/recycle Host，调用返回结果未知且副作用不重放；
- Hook 无 cancel 时使用 generation fence；仍可能执行时回收 Host，而非只丢弃 Rust pending entry；
- queue/byte limits、overload、malformed frame、重复 response、旧连接 notification 和 stream late chunk 均有 fixture；
- Windows Job Object 和 Unix process group 回收由真实子进程/孙进程测试证明。

### 7.4 真实纵向 fixture

固定一个最小 OpenCode package fixture，至少包含一个真实 Tool 或稳定 Hook，并验证：

1. prepared target 的 digest 在 Rust 和 Bun 两端一致；
2. Host open 不产生网络/package-manager 写入；
3. contribution 只由能力 owner 发布一次；
4. 正常调用、权限请求、取消确认、迟到结果和进程 crash；
5. shutdown response -> TCP EOF -> child/descendant exit 的完整顺序；
6. Remote fixture 在远端执行域完成同一流程，断线不触发本机 Host。

文档变更至少运行：

```bash
git diff --check
pnpm run check:repo-hygiene
node scripts/check-core-boundaries.mjs
```

实现阶段再按触及 crate 运行 `runtime-ports`、`plugin-runtime-client`、`opencode-adapter`、`services-integrations` 和
`bitfun-core` 的 focused tests；测试名称以实现时真实 target 为准，本文不预造不存在的测试入口。

## 8. 与现有文档的关系

| 文档 | 关系 |
|---|---|
| [`plugin-runtime-design.md`](plugin-runtime-design.md) | 上位约束：共享 Host 的进程键、生命周期、状态 owner、故障传播和恢复 |
| [`opencode-plugin-runtime-adapter-design.md`](opencode-plugin-runtime-adapter-design.md) | 上位约束：OpenCode 加载、npm/Arborist、Hook/Tool 语义和兼容范围 |
| [`external-ai-work-sources-design.md`](external-ai-work-sources-design.md) | 来源、确认、持续更新和 capability-specific coordinator |
| [`opencode-extension-compatibility.md`](opencode-extension-compatibility.md) | OpenCode 能力矩阵、当前基线和阶段退出条件 |
| 本文 | ext-host IPC 的不可变 target、安全调用和最小纵向闭环 |

本文不改变上位 owner；若实现需要按插件或 workspace 隔离 Host，必须先修改并评审 `plugin-runtime-design.md`，不能在本
P0 文档中隐式改写默认模型。

## 9. 当前实现状态与退出条件

当前仓库已经具备受管 package 来源发现/静态预览、activation authority 主链，以及 standalone `.js` Tool 经
`ScriptToolRuntime` 的独立 Node worker 执行。这些能力不等于 package plugin、Hook、完整 Client 或 Bun Host 已实现；
`load_opencode_package_adapter` 仍是 static-preview only。

当前 ext-host 审计提交已有 Bun Host、JSON-RPC schema、shutdown 和 EOF 清理基础，但仍会从裸 npm spec 解析/安装代码，
也没有完成 BitFun 所需的 prepared target、完整取消确认、Rust supervisor 和跨语言 fixture。因此本设计的实现退出条件是：

1. 冻结并审计支持 prepared target 的新协议提交；
2. 完成 Host 外依赖准备、最终 attestation 和 materialization approval；
3. 沿唯一 Runtime/Services 主链接入一个真实 Tool 或稳定 Hook consumer；
4. 证明 startup deadline、Tool cancel、不可取消调用 recycle、旧连接 fencing 和进程树回收；
5. 保持 static-preview 入口、Session owner、来源 owner 和各能力 owner 的既有边界；
6. 完成许可证、可复现构建、签名、升级和三平台交付决策。

在这些条件全部通过前，产品和文档只能把 package-plugin ext-host 标记为 target/planned capability。
