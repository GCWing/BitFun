# Desktop Command 接口收敛重构计划

> 状态：提案，待按垂直切片实施。
>
> 当前状态基线：2026-08-18。本文中的数量来自本次静态盘点，只用于说明当前问题和衡量迁移收益，不作为可调高的永久基线；每个实施批次开始前应重新统计实际调用面。

相关文档：

- [产品运行时架构](../architecture/product-architecture.md)
- [App Server 架构设计](../architecture/app-server-architecture.md)
- [Agent Runtime 部署设计](../architecture/agent-runtime-deployment-design.md)
- [Remote Workspace Transport](../architecture/remote-workspace-transport.md)
- [Peer Device Mode](../architecture/peer-device-mode.md)
- [Detached Task Dispatch](../architecture/detached-task-dispatch.md)
- [I18n Architecture](../architecture/i18n.md)

## 1. 背景与问题

Desktop 当前通过 Tauri command 暴露绝大多数产品能力。静态盘点结果如下：

| 维护面 | 当前规模 | 主要问题 |
| --- | ---: | --- |
| `tauri::generate_handler!` 注册项 | 约 684 | Runtime、文件、Git、LSP、MiniApp、账号、窗口和控制面混在一个入口表中 |
| `#[tauri::command]` 实现 | `src/apps/desktop/src/api` 下约 679 个，另有少量位于 app 入口 | Command 同时承担业务用例、DTO、Host effect 和兼容桥职责 |
| Web UI `invoke` 调用 | 约 622 处，分布于约 77 个文件 | 前端稳定接口仍以 Tauri command 字符串为中心 |
| Remote workspace policy | 684 条，其中 332 条为 `LegacyUnaudited` | 每增加 command 都要扩展独立策略表，且大量历史行为未完成审计 |
| Peer local-only 表 | Desktop 约 105、CLI 约 101、Web UI 约 104 | 同一执行权威在三处手工同步，已存在漂移风险 |
| WebSocket command 映射 | 当前已映射约 35 个主要命令 | Web UI 先说 Tauri 名，再由 WebSocket adapter 转成 App Server method，DTO 还需二次归一化 |

问题不只是注册列表过长，而是一个操作身份分散在多处：

1. Desktop Rust command 注册与实现。
2. Web UI service API 的字符串调用。
3. App Server method 和 WebSocket 映射。
4. Remote workspace 路由策略。
5. Desktop/CLI/Web UI 三份 Peer authority 表。
6. Peer priority、timeout、retry 和幂等判断。
7. 请求、响应和错误的 Desktop/Web 形状转换。

新增或修改一个能力时，维护者必须人工判断并同步这些位置。任何一处遗漏都可能造成 Remote workspace 静默本机执行、Peer 控制端与执行端混淆、Web 与 Desktop 行为不一致，或 mutation 在弱连接上被错误重试。

## 2. 目标与非目标

### 2.1 目标

1. 前端业务代码依赖按领域组织的类型化产品客户端，不再依赖 Tauri command 名称。
2. 产品用例的稳定身份、请求、响应、错误和行为语义由 Runtime API 或能力 owner 持有。
3. Embedded Desktop 通过 Host-owned direct adapter 调用同进程 Runtime typed API 或对应 owner/service。
4. Tauri 只保留少量按宿主边界组织的 gateway、Desktop-native effect 和必要内部回调。
5. WebSocket/App Server、Desktop 和 Peer Host 复用同一产品操作语义，但保留各自的 transport、鉴权、重试、流量控制和生命周期。
6. Remote workspace、Peer authority、capability、幂等和重试事实有明确 owner，不再靠多份 command-name 表同步。
7. 每个可删除业务切片完成后删除对应旧 command、旧 DTO 转换和旧策略项，不形成永久双轨。
8. 保持旧版本数据、Peer 和 Remote Control 的升级兼容，不通过删除用户数据或静默本机回退恢复失败。

### 2.2 非目标

- 不把全部 command 包装成一个 `invoke(command: String, args: Value)` 万能入口。
- 不建立新的通用 `api-layer`、Service Locator 或能够调用任意 Rust 函数的动态注册框架。
- 不强制 Embedded Desktop 在同进程内创建 App Server、JSON-RPC client/server 或 in-memory transport。
- 不把 App Server wire DTO 当成 Runtime 或 Product Domain 的业务 owner。
- 不借接口收敛迁移 Session、Permission、Config、Workspace 等权威状态 owner。
- 不统一 GUI、TUI、Headless CLI、ACP、SDK Host 的 renderer、协议或生命周期。
- 不仅为减少文件数量而合并 Terminal、LSP、MiniApp 等具有独立流式和生命周期合同的能力。
- 不在新路径失败后静默调用旧 command；迁移和回滚必须由明确的 Host route 或版本协商决定。

## 3. 核心决策

### 3.1 区分逻辑接口与物理入口

接口收敛分为两个层次：

- **逻辑接口按领域保持类型化。** Agent、Session、Permission、Workspace、Git、Config、MCP 等仍有字段明确的独立方法、请求和响应。
- **物理 Tauri 入口按宿主边界收敛。** 同一领域的 Desktop delivery 可以经过一个封闭的 tagged request gateway，但 gateway 内只能穷举已声明操作，不能接受自由方法名并动态调用任意实现。

目标示意：

```text
React feature / store
  -> ProductBackendClient
     -> agent / session / permission / workspace / config / ...
  -> DesktopHostClient
     -> window / tray / picker / clipboard / notification / updater
  -> ControlPlaneClient
     -> account / peer / remote-connect / dispatch / relay-deploy

Desktop deployment
  -> Desktop transport adapter
  -> typed domain Tauri gateway
  -> Desktop direct Runtime adapter / owner service

Web deployment
  -> App Server typed client
  -> WebSocket transport
  -> App Server owner adapter

Peer deployment
  -> negotiated ProductOperation envelope or legacy alias adapter
  -> target Host typed dispatcher
```

### 3.2 三类前端客户端

#### ProductBackendClient

面向跨 Desktop/Web 或多 Host 复用的产品用例，按领域暴露类型化方法：

- Agent / Session / Turn / Permission
- Workspace / File / Search / Git / Snapshot / Worktree
- Config / Model / Skill / Subagent / MCP / Hook / External Source
- 适合跨 Host 的 MiniApp、Page 或其他产品领域能力

客户端隐藏 transport 和部署差异。UI component 不直接调用 `invoke`，也不判断当前是 Tauri、WebSocket 或 Peer。

#### DesktopHostClient

只承载 controller-local Desktop effect：

- 窗口、菜单、托盘和应用进程生命周期
- 原生文件/目录选择器、剪贴板和系统通知
- 更新器、系统设置、Desktop Computer Use 权限
- 本机语言偏好应用、原生菜单/托盘刷新和其他 surface-local 呈现 effect
- 本机嵌入式浏览器窗口和其他明确的本机呈现能力

这些 DTO 可以继续留在 `src/apps/desktop` 或 Web UI infrastructure；只有出现真实第二宿主且语义相同时才抽取共享契约。

#### ControlPlaneClient

承载 authority 不随当前 workspace surface 变化的控制面：

- Account identity 与 settings sync
- Peer attach/detach、device RPC 和 capability handshake
- Remote Connect 配置与 bot lifecycle
- Detached Dispatch controller observer/credential 操作
- Relay Deploy

每个操作必须明确由 controller、peer、target Host 还是 split endpoint 执行。

#### I18n 的拆分边界

I18n 不作为随当前 workspace 或 Peer rendered surface 切换的普通
`ProductBackendClient` 领域。它拆为三个已有 owner 的协作：

- locale id、alias、fallback 和支持列表继续由共享 i18n contract 持有，Web UI
  直接消费本 surface 的生成资源，不通过远端 Runtime 读取资源目录。
- 当前窗口的语言选择属于 controller/surface-local preference。Desktop 通过
  `DesktopHostClient` 持久化并应用到本机 backend locale、菜单和托盘；Web Host
  通过自己的 surface adapter 持久化，不把操作转发到当前 Peer target。
- Account settings sync 可以同步该偏好值，但同步 transport 不接管当前窗口的
  呈现 authority；收到同步值后仍由每个 surface 的本地 adapter 决定何时应用。

因此，旧 `i18n_*` command 可以收敛为窄的 Host preference/effect 入口，但不能被
包装成 target-host product operation。Peer 模式下改变 A 窗口语言不得重建 B 的
菜单或托盘。

### 3.3 两级操作描述

不能把所有策略塞进一个全局注册表。操作事实按 owner 分为两级：

#### Product operation contract

由 Runtime API、能力 contract 或已有稳定 owner 持有：

- 稳定 operation id；已有 App Server method 与语义完全一致时优先复用其 id。
- request / response / typed error。
- required capability。
- query、mutation 或 streaming 分类。
- mutation 的幂等键、可重试条件和 `outcome_unknown` 语义。
- execution/workspace scope 和阻塞交互合同。
- 事件、取消和恢复身份。

这些事实不要求塞进一个新的全仓 struct。每个 operation 的稳定 id 和可跨 Host
复用的行为事实必须与其 typed owner facade 相邻；只有两个以上 Host 确实需要消费
同一机器可读事实时，才在该 owner 已有的 stable contract crate 中增加窄 descriptor。
Runtime operation 优先留在 `runtime-ports`/Agent Runtime SDK，Product Domain 和
Service operation 留在各自现有 contract/owner，不新建 `desktop-api-contracts` 总包。

#### Host route descriptor

由 Desktop、Web、CLI Peer 等真实 Host 根据装配事实持有：

- 当前 Host 是否提供该 capability。
- Remote workspace 下 `RemoteRouted`、`RemoteUnsupported`、`LocalOnly` 或 `WorkspaceAgnostic`。
- Peer 中由 controller 还是 target Host 执行。
- transport timeout、priority、concurrency 和 retry budget。
- 日志脱敏和 payload size policy。
- 旧 operation/command alias 及其删除版本。

Host route 可以从 product contract 投影共同事实，再增加 Host-specific override。禁止前端、Desktop 和 CLI 各自重新猜测 read/mutation、authority 或幂等性；也禁止仅根据 `get_`、`list_`、`read_` 前缀决定自动重试。

迁移期间仍遵守仓库现有规则：每个物理 Tauri command 都必须在
`remote_workspace_policy.rs` 中显式声明 policy。新的领域 gateway 应使用一个明确的
variant-routed policy，表示物理入口只完成第一层准入，真正的 Remote workspace
决策必须逐 operation 执行；Desktop-native command 则继续声明 `LocalOnly`、
`WorkspaceAgnostic` 等物理策略。最终删除的是旧的一用例一 command policy entry，
不是取消物理 Tauri 入口的 fail-closed policy 合同。

#### Host catalog 与 TypeScript 投影

`Operation catalog` 是 Host 在组装时将“已选 product descriptor”和“本 Host route”
连接后的只读结果，不是第三个事实 owner，也不是运行时 Service Locator：

1. Desktop、Web 和 CLI 各自在 app/Host 边界声明自己提供的 route，并在构建或启动
   组装时与 owner descriptor 做完整性校验。
2. Catalog 只能查询 capability、authority、scope、transport policy 和兼容 alias，
   不能保存 handler closure、任意函数指针或业务状态。
3. Gateway/Peer ingress 必须先以 operation id 查得 Host route 并完成 fail-closed
   校验，再反序列化到封闭的领域 request 并调用 owner；“catalog 中有 id”本身不
   表示 capability available。
4. App Server method 是 wire id。只有 method 语义、request/response、错误和版本
   合同与 product operation 完全等价时才复用为稳定 operation id；否则保留显式
   映射，不能让 wire DTO 反向成为 Runtime owner。
5. 跨 Desktop/Web 的前端请求和响应类型继续通过现有
   `app-server-protocol` TypeScript schema exporter 产生 wire-facing 类型；Desktop
   gateway adapter 在边界将该形状映射到 owner DTO。不得为了导出 TypeScript 给
   Runtime/Service owner 引入 `ts-rs`，也不得再维护第二套手写 Desktop DTO。
6. 仅 Desktop-native 的 Host effect 类型留在 Web UI infrastructure 与 Desktop app
   边界，不进入 App Server protocol。阶段 0 的试点必须证明 Rust owner、App Server
   wire、Desktop gateway 和生成的 TypeScript client 之间只有明确 adapter，没有
   循环依赖或重复行为 owner。

### 3.4 Tauri gateway 形态

示意类型如下，最终名称以 owner 现有命名为准：

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", content = "request", rename_all = "camelCase")]
enum DesktopSessionRequest {
    Create(AgentSessionCreateRequest),
    Restore(SessionRestoreRequest),
    Rename(AgentSessionRenameRequest),
    Archive(AgentSessionArchiveStateRequest),
}

#[tauri::command]
async fn session_request(
    state: State<'_, DesktopRuntimeContext>,
    request: DesktopSessionRequest,
) -> Result<DesktopSessionResponse, DesktopHostError> {
    // Resolve and enforce the per-operation Host route before typed dispatch.
    // Exhaustive typed dispatch only.
}
```

约束：

1. Gateway command 继续遵循 `snake_case` 和结构化 `request` 参数。
2. Request enum 必须封闭、可穷举，并直接委托 Runtime/owner typed API。
3. Gateway 不拥有业务状态、权限策略或持久化规则。
4. Gateway 不接受自由字符串、任意 JSON method 或反射式 handler lookup。
5. 大 payload、PTY、事件流或独立生命周期能力可以保留专用 gateway，不为追求入口数量强行合并。
6. Tauri event 只投递 typed Runtime/App Server notification 或 Desktop 专属事件，不创建第二套 Runtime 事件语义。
7. Gateway 的 Tauri 名不能代替内部 operation 的 Remote/Peer 审计；每个 enum variant
   必须在进入 owner 前通过对应 Host route。
8. 同一 gateway 中只有 route authority、调用来源、payload/stream 限制和生命周期
   兼容的 operation 才能合并；否则即使属于同一领域也保留独立 gateway。

## 4. Command 分类与目标归宿

迁移前先把当前 command 分类，不能按文件名批量搬运：

| 分类 | 示例 | 目标归宿 |
| --- | --- | --- |
| Agent Runtime 用例 | create/start/cancel/steer turn、Session mode/model、Permission | Agent Runtime API + Desktop direct adapter |
| Product Domain / management | Config、Model、Skill、Subagent、MCP、Hook、External Source | 对应 owner/service + domain gateway |
| Workspace Host capability | File、Search、Git、Snapshot、Worktree、LSP、Terminal | 对应 port/service；显式 workspace/execution scope |
| Desktop-native effect | window、tray、picker、clipboard、notification、updater | `DesktopHostClient` + app-local Tauri handler |
| Surface preference / I18n effect | 当前窗口语言、backend locale、原生菜单和托盘刷新 | surface-local preference adapter + `DesktopHostClient`；不随 Peer surface 路由 |
| Controller control plane | Account、Peer、Remote Connect、Dispatch、Relay Deploy | `ControlPlaneClient` + controller-owned handler |
| Split-endpoint operation | Peer 文件下载到 controller 选择的路径 | 分解为 target read + controller local write，不传递另一台机器的路径 |
| Internal callback | WebDriver bridge result、Peer Host invoke completion | 窄内部 callback；不进入产品 operation catalog |
| Dead or compatibility-only | 无生产调用者、已被 owner API 替代的 command | 证明无调用后删除，或只留在版本边界 alias adapter |

## 5. 实施阶段

迁移按垂直行为闭环推进，不按 command 数量或源文件批量推进。每个可删除的迁移
阶段都要同时包含新路径、调用方切换、行为验证和旧路径删除；基础设施阶段只在
明确兼容边界和删除条件后才能完成。

### 阶段 0：冻结增长与建立可重复基线

目标：阻止维护面继续无约束增长，并形成可审阅的 command inventory、operation
descriptor 归属和 Host route 执法基础。阶段 0 只冻结边界、建立合同和刻画既有
行为，不切换生产调用方；生产 typed route 在阶段 1 交付。

工作项：

1. 从实际 `generate_handler!`、`#[tauri::command]`、前端 invoke、Remote policy 和 Peer 表生成一次性审计报告；报告保留在 PR，不新增可不断调高的 JSON 基线。
2. 为每个注册 command 标记 owner、上述分类、真实前端消费方、Remote workspace policy、Peer authority 和目标迁移批次。
3. 扩展 Desktop 本地 contract test：新增 product command 必须声明 owner 和 route；
   禁止进入 `LegacyUnaudited`。为新的领域 gateway 增加 variant-routed 物理 policy，
   并要求每个 variant 都有 operation-level route。
4. 增加前端约束测试或 lint：新的 UI component 不得直接导入 Tauri `invoke`；裸字符串调用仅允许存在于 infrastructure adapter 和迁移 allowlist。
5. 记录无调用者、重复命令、同义命令和参数未结构化命令，优先形成删除候选。
6. 选择一个低风险、已有 App Server 合同的 Config query 作为 catalog 试点，明确
   product descriptor、Desktop/Web/CLI route、实际 capability、Remote policy、Peer
   authority、retry/idempotency 和 alias 的物理 owner。
7. 为该试点建立最小的 Rust owner -> App Server wire -> Desktop route -> Web UI
   类型投影 characterization fixture，先刻画现有结果、错误和 capability；
   TypeScript 类型只能由现有 protocol schema exporter 或明确的 surface-local
   contract 生成，不复制第二套 DTO owner。
8. 定义 operation-level route 的 fail-closed 合同测试，覆盖未声明 route、未装配
   capability、错误 execution scope 和 local-only 访问；阶段 1 的生产 gateway
   必须通过该测试后才能接入调用方。
9. 定义 Peer 稳定 envelope、capability handshake、legacy alias 元数据和双向
   mixed-version fixture；生产 ingress/egress 在阶段 1 接入，旧 alias 不得继续
   依赖 WebView 任意执行 `invoke(command)`。

完成条件：

- 现有注册项都有分类和目标 owner。
- 新增 command 无法绕过 owner/route 声明。
- 数量统计可通过同一命令重复获得，但不把当前数量变成可上调门槛。
- 至少一条真实 operation 的 descriptor、Host route、现有 Desktop/Web 行为和
  TypeScript 投影被同一 characterization fixture 覆盖；catalog 不持有业务状态或
  handler closure。
- 新领域 gateway 的物理 policy 与逐 operation route 合同已经由测试固定，未声明
  任一层都无法暴露 operation。
- Peer envelope、legacy alias 和 capability handshake 的合同 fixture 已能在旧/新
  controller 与 Host 组合中区分 unsupported、transport failure 和 product failure；
  此时尚不宣称生产 Peer ingress 已切换。

### 阶段 1：先交付路由基础设施，不删除旧 command

目标：完成阶段 0 试点所需的 typed gateway、operation route 和 Peer ingress，但暂时
保留旧 command 作为未迁移 operation 和 mixed-version alias 的物理兼容边界。此阶段
不把多个业务 owner 绑成一个大切片，也不迁移 I18n。

工作项：

1. 在 Web UI infrastructure 定义按领域的 `ProductBackendClient`，让 feature/store
   只依赖类型化方法；迁移 allowlist 只能位于 infrastructure adapter，不能扩散到
   component/store。
2. 复用 owner DTO；已有 App Server protocol 类型只有在语义完全等价且已有多个
   生产 Host 消费时才复用，不把 wire 类型反向变成 owner。
3. 为试点 operation 接入 Desktop typed gateway，并在 gateway 内先查询 Host route
   再调用现有 Config owner；不创建第二份 Config、Session 或 capability 状态。
4. 为同一 operation 接入 WebSocket typed client。旧 Tauri-name 映射暂时保留在
   迁移层，直到新 client 与旧路径 fixture 等价；不得在新 client 返回错误后
   动态 fallback。
5. 为 Desktop/CLI Peer Host 接入稳定 operation envelope、typed dispatcher 和
   legacy alias adapter。Desktop legacy alias 必须绕过 WebView `invoke(command)`，
   CLI alias 也必须进入同一 operation-level authority/route 校验。
6. 将 capability handshake 扩展为真实 operation/capability 支持集及兼容 alias
   范围；不以 package version 相等推断 typed envelope 能力。
7. 记录请求 identity、幂等键、retry budget、`outcome_unknown`、execution scope、
   permission/mailbox 和事件恢复事实；没有这些事实的 operation 默认不可自动 retry。

阶段 1 的基础设施试点只覆盖一个 Config query 和一个等价的 unsupported/local-only
fixture，不宣称 Session 或 I18n 已迁移。必须验证：

- Desktop direct path 与 Web/App Server path 的 request/response/error 等价。
- 新/旧 Peer controller 与新/旧 Host 的双向兼容、alias 删除条件和 unsupported 语义。
- operation-level Remote route 校验不被单一 Tauri gateway 名绕过。
- Host local-only 拒绝、Remote unsupported 和 transport failure 保持可区分。

完成条件：

- 试点业务调用方可以只依赖 typed client；旧 Tauri command 仍由迁移 allowlist
  保护，并已有阶段 2 的删除条件，但本阶段不删除。
- Desktop、Web、CLI Peer 对试点 operation 使用同一行为事实，并有 direct/wire/
  alias fixture。
- 新 typed ingress 已成为旧 Peer command 的边界适配器；产品请求不再经 WebView
  任意执行 `invoke(command)`。
- 试点 operation 的 route、capability、retry、authority 和 alias 都有唯一 owner
  与删除条件。

### 阶段 2：首批可删除切片：Config 查询

目标：在阶段 0/1 的基础设施完成后，选择可独立回滚的低副作用查询，
先迁移 Config 查询并删除对应本机旧 command、WebSocket Tauri-name 映射和旧
policy 项。不把 Session、mutation、Permission mailbox 和 I18n 混入。

工作项：

1. 先迁移 Config query：`get_config`、`get_configs`、`get_agent_profile_config`
   等明确 read contract；每个 operation 通过 catalog/route 校验后调用现有
   Config owner。
2. WebSocket adapter 直接使用稳定 client 方法；对应旧映射仅在 alias boundary
   保留，且不能被 UI feature 直接调用。
3. 新 controller -> 旧 Host 继续使用 legacy command alias；旧 controller -> 新
   Host 由 Host ingress 转成 typed operation；新/新双方走稳定 envelope。
4. 对每个迁移 operation 先完成 Remote workspace policy，再切换调用方；gateway
   名称不作为 policy 依据。
5. 通过同一行为 fixture 验证 Desktop direct、Web/App Server、Peer typed 和
   legacy alias 的结果、错误、scope、事件顺序、恢复和 `unsupported` 等价。
6. 只有在 mixed-version fixture、旧 payload 读取和新路径观测数据通过后，才删除
   该 operation 的旧 Tauri handler、旧 WebSocket normalizer 和旧 Remote policy
   entry。删除必须是按 operation/variant 的明确变更，而不是删除整个领域的兼容层。

完成条件：

- Config query 的旧本机 product command、重复 DTO、WebSocket Tauri-name 映射和旧
  policy entry 已按各自删除条件移除。
- 已迁移操作的 Remote、Peer、retry 和 idempotency 只有 owner fact + Host route
  两级来源；不存在 product request 在 Peer/Remote 失败后回落 controller 本机。
- Peer target Host 对 controller-owned operation 继续强制拒绝，且不依赖前端 deny
  表才能成立。
- 旧 alias 仍只服务于支持范围内的 mixed-version peer；没有删除条件的兼容代码
  不得合入。

### 阶段 3：Session 读路径与运行中投影

该批次迁移 Session list/restore/read projection，必须在 Config 查询稳定后执行。
它不包含 Session mutation、Permission mailbox 和 I18n。

工作项：

1. 迁移 Session list、restore、turn window、lineage 和当前已有的 Runtime
   event/interaction snapshot；保持 Peer surface identity、事件 cursor、mailbox
   和 persisted snapshot 的既有规则。
2. 保持 `DesktopRuntimeContext`/Agent Runtime SDK 的现有 owner，不把 Session 或
   scheduler 状态复制到 gateway、App Server 或 Peer adapter。
3. 对窗口化 history 和运行中 projection 分别定义 cursor、snapshot、streamId、
   stale/invalidated 和重挂载行为；不得把 persisted checkpoint 当作当前 Turn
   owner。
4. WebSocket adapter、Desktop direct adapter、Peer typed dispatcher 和 legacy alias
   使用同一 Session read fixture，覆盖旧 Host 缺少 additive snapshot 字段的兼容
   fallback。
5. Remote Control 的 mobile/bot surface 对被移动的 Session read operation 保持
   现有 `RemoteCommand` 行为；尚未支持的入口必须返回显式 unsupported。
6. 只有在 mixed-version fixture、旧 payload 读取和新路径观测数据通过后，才删除
   对应 read operation 的旧 Tauri handler、WebSocket normalizer 和 Remote policy
   entry。

完成条件：

- Session read/projection 的旧本机 product command、重复 DTO、WebSocket Tauri-name
  映射和旧 policy entry 已按各自删除条件移除。
- Desktop direct、Web/App Server、Peer typed 和 legacy alias 对 Session read 的
  结果、错误、scope、事件顺序、cursor、恢复和 `unsupported` fixture 通过。
- Remote workspace、Remote Control、Peer Device 和 Detached Dispatch 的受影响
  场景已实际验证；不支持能力 fail closed。

### 阶段 4：Session/Turn 写路径

该批次进入有副作用的 Session mutation 和 Dialog Turn，必须在阶段 3 的读路径稳定
后执行。Permission mailbox 单独处理，不把阻塞交互隐藏在普通 mutation gateway
中。

工作项：

1. 分别迁移 Session mutation（create/delete/rename/archive/fork、mode/model、
   context reload）和 Dialog Turn（submit/cancel/interrupt/steer/compact）；每个
   operation 明确 request identity、owner、lease/controller、幂等性和
   `outcome_unknown` 查询方式。
2. 保持 `DesktopRuntimeContext`/Agent Runtime SDK 的现有 owner，不把 Session 或
   scheduler 状态复制到 gateway、App Server 或 Peer adapter。
3. mutation 默认禁止自动 retry。只有 Host handshake 明确声明 operation 的幂等
   contract，且请求带稳定 identity 时，才可重放；超时/断连先查询 authority，
   不盲目重提副作用。
4. 新/旧 Desktop、CLI Peer Host 和 App Server adapter 使用同一 operation identity
   和错误 fixture；旧 alias 仅在声明的 mixed-version 范围内可用。
5. Session lifecycle、mode/model、approval、attachment 和 Turn mutation 若可由
   mobile/bot 驱动，必须同步保持 `RemoteCommand`/command router 行为；不能到达的
   能力返回显式 unsupported。

完成条件：

- Session/Turn direct、App Server、Peer typed 和 legacy alias 行为 fixture 通过，
  覆盖事件顺序、取消、scope、幂等、权限前置检查和 unknown outcome。
- 每个 mutation 的旧 command 只在声明的 mixed-version alias 边界存在，并有最后
  发送方范围和删除版本/条件。
- Remote workspace、Remote Control、Peer Device 和 Detached Dispatch 的受影响
  场景已实际验证；不支持能力 fail closed。

### 阶段 5：Permission mailbox 与阻塞交互

Permission list/subscribe/respond/batch respond 作为独立 mailbox 切片迁移。该
阶段的成功标准是断连和 surface switch 后仍能恢复交互，而不是只完成一次事件转发。

工作项：

1. Runtime owner 保留待处理 request、revision、Session/Turn identity 和取消/drop
   语义；`restore_session_view` 或等价 attach path 返回可重放 mailbox。
2. Desktop/CLI Peer Host、App Server 和 Web UI 均使用同一 mailbox 行为合同；
   frontend projection 按 Surface 保存，并以 epoch/revision fence 保护迟到 response。
3. Remote Control mobile/bot 对新增阻塞交互提供 RemoteCommand/显式 unsupported
   回复；不能只发送一次 UI event 后让执行 future 永久等待。
4. Permission response 是 mutation，默认不自动 retry；只有 operation identity
   和 Host capability 明确允许时才可重放，重复 response 必须返回稳定结果。

完成条件：

- mailbox reattach、permission audit、取消、重复 response、断连和 surface switch
  fixture 通过。
- Desktop direct、Web/App Server、Peer typed 和 legacy alias 的阻塞交互语义等价，
  且旧 alias 有明确最后发送方和删除条件。
- 受影响的 Remote Control、Peer Device 和 Detached Dispatch 场景实际验证；不支持
  入口返回 typed unsupported。

### 阶段 6：Workspace、File、Search、Git、Snapshot、Worktree

该批次涉及 workspace scope、远程文件系统和大量路径参数，必须先完成执行域建模再切换入口。

工作项：

1. 请求显式携带 workspace identity、execution target 和必要 remote binding，不从 UI 当前 tab 或全局路径猜测。
2. Remote workspace path 始终按 POSIX 语义处理；不得用 controller OS 的 `std::path` 规则拆分或拼接。
3. File、Search 和 Git 调用进入现有 service/port；Desktop gateway 不直接复制本地/SSH 分支。
4. Snapshot/rollback 保持 Session transaction、live ownership 和 Remote unsupported 边界。
5. Split-endpoint 下载保持 target read + controller write；禁止把 controller save path 发给 peer。
6. Web/Server 未组装的能力返回 typed `Unavailable`/`unsupported`，不能因 DTO 或 handler 存在而宣传 available。

完成条件：

- Workspace 类 UI 不再依赖 Desktop command 名。
- 本机、SSH Remote workspace 和 Peer workspace 都通过明确 execution scope。
- 对应 `LegacyUnaudited` 项完成审计并归零或删除。

### 阶段 7：Model、Skill、Subagent、MCP、Hook、External Source

该批次收敛管理面和扩展控制面，但不建立新的统一 management owner。

工作项：

1. 各领域 gateway 直接调用现有 owner/service；revision、凭据、冲突选择、执行许可和持久化仍由原 owner 持有。
2. MCP process lifecycle、OAuth、interaction response 和资源读取保持独立能力合同。
3. External Source/Hook 保留来源身份、内容版本、审批和 Remote unsupported 语义。
4. Secret、命令正文、Hook 内容和大 payload 按已有日志脱敏与大小限制处理。

完成条件：

- 管理面 UI 只依赖领域客户端。
- Desktop 与 App Server owner adapter 不复制业务状态。
- 各管理面 operation 的 capability、authority 和 unsupported 行为由 owner contract
  与 Host route 投影，不由 UI command-name 表推断。

### 阶段 8：Terminal、LSP、MiniApp 与其他长生命周期能力

这些领域具有独立事件、背压、worker/process 生命周期或安全模型，不要求合并为同一个 gateway。

工作项：

1. Terminal 保持 create/write/resize/signal/ack/close 的 typed session protocol 和事件流；不能把高频输入退化为通用 JSON mutation 队列。
2. LSP 保持 workspace/document/server identity，避免按每个 LSP method 建立独立 Tauri 注册，同时保留 cancellation 和 server-state 事件。
3. MiniApp 区分 domain CRUD、worker protocol、Host call、draft、market 和 AI/Agent lifecycle；只合并共享 owner 和生命周期相同的操作。
4. Browser、Speech、SSH、Page、Appearance、Announcement 等按真实 Host/owner 边界迁移，不追求一次性总入口。
5. 大 payload 和流式事件测量序列化、内存、队列和响应延迟，必要时保留专用 transport command。

完成条件：

- 不再因新增一个领域动作修改全局 `generate_handler!` 大表和多份 Peer policy 表。
- 每个长生命周期能力有独立取消、断连和资源回收测试。

### 阶段 9：控制面、I18n 与内部回调收尾

工作项：

1. Account、Peer attach/detach、Remote Connect、Dispatch、Relay Deploy 根据
   controller authority 收敛成少量 control-plane gateway。
2. Detached Dispatch 保持 target-owned durable job；controller 只观察和回答 mailbox，不成为 Runtime/filesystem proxy。
3. Window/tray/updater/picker/clipboard/notification 等保留在 `DesktopHostClient`。
4. I18n 按三段式边界收尾：locale contract/生成资源留在各 surface，语言偏好由
   controller/surface-local adapter 持有，Desktop backend locale、macOS 菜单和托盘
   刷新由 `DesktopHostClient` 执行。Account settings sync 只同步偏好值，不把
   `i18n_set_language` 路由到 Peer target。
5. WebDriver result、Peer completion 等内部 callback 保持最小、不可被普通产品客户端调用。
6. 删除旧 re-export、无调用 `api` 模块、旧 command DTO、旧 WebSocket normalizer 和过渡 allowlist。
7. 将 `generate_handler!` 拆为按 Host capability 组合的少量注册函数或模块，入口文件不再手写数百项列表。
8. 只有在 operation authority 已覆盖所有发送方、Host ingress 能独立 fail closed、
   mixed-version fixture 通过后，才删除 Desktop/CLI/Web UI 三份 Peer command-name
   deny 表；同一变更同步更新 Peer 架构文档、仓库 guardrail 和合同测试。

完成条件：

- 产品用例不再以“一用例一个 Tauri command”注册。
- 注册面只剩领域 gateway、Desktop-native capability 和必要 callback。
- I18n 不再作为 Peer target product operation；本机语言、backend locale、原生菜单
  和托盘刷新均遵循 controller/surface-local authority。
- 不存在无删除条件的 legacy adapter。

## 6. 兼容与迁移策略

### 6.1 本机 Desktop

Desktop 前端与 Rust Host 随同一安装包发布，因此本机旧 command 名不需要永久保留。一个垂直切片切换完成、行为测试通过后，应删除旧注册项，避免同一能力存在两条本机路径。

迁移期间允许新旧领域并存，但同一个 operation 的 route 必须确定：

- 已迁移 operation 只走新 gateway。
- 未迁移 operation 继续走旧 command。
- 新 gateway 返回 unsupported/error 后不得动态 fallback 到旧 command。

### 6.2 Peer Device 跨版本

Peer 是真实跨版本边界，必须双向兼容：

- **新 controller -> 旧 Host**：handshake 未声明稳定 operation envelope 时，controller 使用 legacy command adapter；mutation 仍遵循旧 Host 已声明的幂等能力。
- **旧 controller -> 新 Host**：新 Host 的 HostInvoke ingress 接受旧 command alias，在边界将其转换为稳定 typed operation；不要求重新注册全部旧本机 Tauri command。
- **新 controller -> 新 Host**：直接使用稳定 operation id、capability 和 typed envelope。

Alias 必须记录引入版本、最后发送方范围和删除条件。不能通过 package version 相等推断 capability。

迁移期间，`peer_host_invoke.rs`、CLI Peer Host deny 和 Web UI Peer adapter 的现有
local-only 表仍按仓库规则保持同步。只有 controller-owned 操作全部改走明确的
control-plane client、target ingress 在没有前端表时仍能拒绝越权操作、并且支持
范围内的旧 controller fixture 已通过，才能在同一批次删除三份名称表并更新对应
架构 guardrail；不能提前删除其中任意一份。

### 6.3 App Server 与 Web

- 已发布 method id、可选字段和错误 kind 保持兼容。
- 新字段提供默认值，反序列化容忍未知字段。
- Desktop 迁移不能改变 WebSocket Host 的认证、allowlist、scope 或 transport limit。
- WebSocket adapter 中的旧 Tauri-name 映射只作为迁移层；切片完成后前端直接调用稳定客户端方法并删除映射。

### 6.4 Persisted data

本计划原则上不修改持久化形状。若某个垂直切片确实需要新增字段：

- 字段必须可缺省并有旧数据测试。
- 不复用或收窄已有字段语义。
- 解析失败时保留原数据并显式降级，禁止删除或重置 Session、Config、Workspace 或连接记录。
- 同时覆盖 legacy deserialize 和旧 payload round trip。

## 7. 远程场景要求

每个垂直切片都必须在 PR 中声明实际验证过的场景；Local Desktop 通过不能被描述为远程行为证据。

| 场景 | 必须证明的行为 |
| --- | --- |
| Local Desktop | 新旧切片行为等价；事件、取消、错误和持久化无回归 |
| Remote workspace | 文件、终端、搜索、Git 和 Agent subprocess 在远端执行；不支持时明确拒绝；无本机 fallback |
| Remote control | Session 级能力经 `RemoteCommand` 和 bot/mobile surface 到达；未实现入口返回显式 unsupported |
| Peer Device Mode | controller/target authority 正确；surface switch 不取消离开的工作；事件和 mailbox 可重挂载；新旧 Host 双向兼容 |
| Detached Dispatch | target 独立持有 job、Session、worktree、event log 和 Permission mailbox；controller 断开后任务继续 |

新增阻塞交互时，Runtime owner 必须保留可重放 mailbox，并通过 attach/snapshot 暴露。只发送一次 UI event 而让执行 future 永久等待不算完成。

## 8. 验证策略

执行时以最近的 `AGENTS.md` 为准，优先运行 owner 的最小命令。典型验证如下：

### Desktop Host

```bash
pnpm run fmt:rs
cargo check -p bitfun-desktop
cargo test -p bitfun-desktop
```

按修改范围增加 Remote policy、Peer Host、Session application 或具体领域的测试过滤；启动、WebDriver、Browser/Computer Use 或打包行为变化时再运行 `cargo build -p bitfun-desktop`。

### Web UI

```bash
pnpm run type-check:web
pnpm --dir src/web-ui run test:run src/infrastructure/api/adapters/tauri-adapter.test.ts
pnpm --dir src/web-ui run test:run src/infrastructure/api/adapters/websocket-adapter.test.ts
pnpm --dir src/web-ui run test:run src/infrastructure/api/adapters/peer-device-adapter.test.ts
```

垂直切片还需运行对应 service API 的 focused test。

### App Server / contracts

仅在稳定 operation、wire DTO、handler 或 typed client 变化时运行：

```bash
cargo check --locked -p bitfun-app-server --offline
cargo test --locked -p bitfun-app-server-protocol --offline --test legacy_wire_contracts
cargo test --locked -p bitfun-app-server --offline --lib server::wire::tests
pnpm run check:core-boundaries
```

### CLI Peer Host

Peer operation/authority 变化时，运行 CLI 最近指南指定的 focused tests，并至少覆盖：

- controller-owned operation 在 CLI Peer Host fail closed。
- Desktop 和 CLI Peer Host 对稳定 operation capability 的共同语义。
- 新 controller/旧 Host 与旧 controller/新 Host fixture。

### 必需合同测试

1. Operation catalog、Desktop route、Host capability 和实际注册 handler 一致。
2. 未声明 Remote policy 的 operation 无法暴露。
3. Mutation 未声明幂等合同就不能自动 retry。
4. Host local-only 拒绝不能被前端 route 绕过。
5. Desktop direct adapter 与 App Server adapter 对同一 use case 的结果、错误和事件 fixture 等价。
6. Typed `unsupported`、cancelled、lagged、invalidated 和 `outcome_unknown` 保持可区分。
7. 旧 payload 缺省字段和新 payload 未知字段可兼容读取。

## 9. 风险与控制

| 风险 | 控制措施 |
| --- | --- |
| 万能 gateway 隐藏类型错误 | 只允许封闭 tagged enum 和穷举 match；禁止自由 method string + `Value` dispatcher |
| 新抽象增加一层但旧 command 不删 | 每个切片将旧注册和映射删除列为完成条件；无删除条件不得合并 |
| App Server wire 反向成为内部模型 | owner DTO 和行为合同优先；App Server 继续只做 wire adapter |
| Peer mixed-version 中断 | capability negotiation + 双向 legacy alias fixture；不按版本号猜测 |
| Mutation timeout 后重复副作用 | 显式 operation identity、幂等键和 `outcome_unknown`；默认禁止自动 retry |
| Remote workspace 本机数据泄漏 | operation-level execution scope 校验；unsupported fail closed；无 fallback |
| 高频 Terminal/LSP/MiniApp 退化 | 保留专用 gateway/stream，测量 payload、背压、延迟和内存 |
| 策略“单一真源”变成无 owner 总表 | Product fact 与 Host route 两级归属，Host-specific 策略不下沉 contracts |
| 大规模一次性切换难回滚 | 按领域垂直切片；route 在构建/握手时确定，不在错误后动态切换 |

## 10. 进度指标

以下指标用于观察真实收敛，不以单独降低数字代替行为完成：

1. `generate_handler!` 中一用例一 command 的 product 注册项持续减少，最终只剩领域 gateway、Desktop-native capability 和内部 callback。
2. Web UI feature/component 中裸 `invoke` 调用最终为零；只允许 transport/infrastructure 和明确的 legacy adapter 使用。
3. `LegacyUnaudited` 从当前 332 条逐批降为零；旧的一用例一 product-command policy
   entry 随切片删除，物理 gateway 和 Desktop-native command 继续保留显式 policy。
4. Desktop/CLI/Web UI 三份 Peer local-only 名称表在最终兼容门槛通过后，被 operation
   authority + Host route 替代，并同步更新架构 guardrail。
5. WebSocket Tauri-name 映射和请求/响应 normalizer 随切片删除。
6. 每个新 product operation 只有一个行为 owner，并有真实 Host consumer、capability、版本与删除策略。
7. 阶段 0/1 的新增基础设施受固定范围和迁移 allowlist 约束；从阶段 2 开始，每个
   可删除业务切片都有旧代码净删除。若仅新增 facade、DTO 或路由表而没有减少旧
   维护面，该业务切片不算完成。

## 11. 总体完成定义

同时满足以下条件时，Desktop command 接口收敛完成：

1. React 业务层只依赖 `ProductBackendClient`、`DesktopHostClient` 或 `ControlPlaneClient`，不依赖 Tauri command 名称。
2. Embedded Desktop 产品请求通过 Host-owned direct adapter 调用 Runtime typed API 或真实 owner/service，不创建同进程 App Server wire。
3. 产品用例不再按“一用例一个 Tauri command”注册；Tauri 注册面只包含领域 gateway、Desktop-native capability 和必要 callback。
4. Gateway 使用封闭 typed request/response，不存在任意字符串到 Rust handler 的通用调用器。
5. Remote workspace、Peer authority、capability、idempotency 和 retry 不再由多份 command-name 表手工同步。
6. `LegacyUnaudited` 为零，旧的一用例一 product-command policy entry、Peer deny 表
   和 WebSocket command normalizer 已按删除条件移除；剩余物理 Tauri command 仍有
   显式 Remote policy，Peer guardrail 已与新的 authority 合同同步更新。
7. Desktop direct、Web/App Server 和 Peer adapter 对共享 use case 通过行为等价和 mixed-version 测试。
8. Remote workspace、Remote control、Peer Device Mode 和 Detached Dispatch 均按受影响范围提供实际验证证据；未支持能力明确 fail closed。
9. 旧持久化数据和旧 payload 可兼容读取，不通过删除或重置用户数据恢复。
10. 旧本机 product command、重复 DTO、兼容 facade 和无消费方模块已删除，不保留静默 fallback。
