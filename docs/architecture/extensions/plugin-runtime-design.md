# 插件运行时与 Plugin Host 设计

本文定义 BitFun 主应用与第三方插件代码之间的运行边界。OpenCode 的兼容范围见
[`opencode-extension-compatibility.md`](opencode-extension-compatibility.md)，生态语义与脚本接口见
[`opencode-plugin-runtime-adapter-design.md`](opencode-plugin-runtime-adapter-design.md)，外部来源的发现、确认和状态见
[`external-ai-work-sources-design.md`](external-ai-work-sources-design.md)。详细设计与
[`../product-architecture.md`](../product-architecture.md) 冲突时，以产品运行时架构为准。多个 GUI/TUI/Remote/CLI/SDK
实例并存时，Rust Runtime 部署与 Plugin Host 复用关系见
[`../agent-runtime-deployment-design.md`](../agent-runtime-deployment-design.md)。

本文同时区分目标设计和当前实现；目标不能被写成已经交付的能力。

## 1. 名词和边界

本专题只使用以下既有对象，不再为同一职责增加 Host、Controller、Manager 或 Coordinator 别名：

| 名称 | 唯一含义 |
|---|---|
| Plugin Host | 运行 Bun 与第三方 JS/TS 插件的受监督子进程；Host 不在 Rust 主应用进程内 |
| `PluginRuntimeClient` | Rust 主应用内部现有调用端口；校验请求和响应，管理超时、同一插件的串行调用、重复请求结果缓存、诊断与故障隔离 |
| `ScriptToolRuntime` / `NodeScriptToolRuntime` | 现有脚本执行端口及 services 实现；只负责 standalone Tool worker，不拥有共享 Plugin Host |
| 插件实例 | 由来源、插件身份和当前内容版本确定的已启用插件；启停事实仍由现有来源与能力模块管理 |
| contribution | Tool、Hook、Command、Route 或界面项等对外行为；由对应能力归属模块注册和提交 |

workspace、project、session、turn、run 和 working directory 是不同事实。它们不能统称为 runtime scope，也不默认
决定 Plugin Host 进程数量。只有某项并发或权威状态确实要求单一实例时，负责该状态的归属模块才能把 workspace
或其他身份加入自己的状态键，并说明清理与迁移语义。

当前本地生产路径可执行配置中显式声明的第三方 OpenCode package plugin。Desktop、CLI、Server/App Server 的 Runtime
入口按真实 execution root 确保 workspace 逻辑实例，复用所在 Rust Runtime 进程监督的共享 Bun Host；Remote
execution domain 不回退到控制机执行。Tool、Config、Permission、Agent/Skill 和有序 Hook 只通过既有归属模块提交，
其余兼容项仍按能力矩阵标记为未实现，不能由这条运行切片外推成完整 OpenCode Runtime。

## 2. 职责

```mermaid
flowchart LR
  Owners["能力归属模块"]
  Lifecycle["Core package lifecycle"]
  Runtime["HookFunctionRuntime"]
  Adapter["生态适配器"]
  Service["Process service"]
  Host["Plugin Host\nBun"]
  Legacy["legacy managed package"]
  Client["PluginRuntimeClient"]

  Owners <--> Lifecycle
  Lifecycle <--> Runtime
  Runtime <--> Adapter
  Adapter <--> Service
  Service <--> Host
  Legacy <--> Client
```

| 部分 | 负责 | 不负责 |
|---|---|---|
| `PluginRuntimeClient` | legacy managed-package 路径的请求校验、超时、串行调用、重复结果与诊断 | 当前 package-plugin Host 生命周期、运行 JS/TS、决定来源顺序或提交业务状态 |
| Core package lifecycle / `HookFunctionRuntime` | 当前 package-plugin workspace 逻辑实例、代际提交、owner 对接和类型化调用 | Plugin Host wire、进程句柄或 OpenCode 原始语义 |
| OpenCode 生态适配器 | 保留加载顺序、Config/Hook/Tool 参数、结果和错误语义；持有共享 Plugin Host 的连接和物理生命周期 | 创建跨生态最低公分母或成为第二个业务归属模块 |
| `ScriptToolRuntime` 与 services 实现 | 启停 standalone worker；通用进程树原语也供 Plugin Host adapter 使用 | 解释 Hook、决定权限、保存插件业务状态或拥有共享 Host 生命周期 |
| Plugin Host | 监督 Bun 子进程、加载真实模块、保存进程内模块实例，按适配协议执行 Plugin/Hook/Tool/Client 调用，并通过 services 进程树原语回收受管后代 | 成为第二个 Agent Runtime、写入 Rust 归属模块的权威状态或决定产品策略 |
| 能力归属模块 | 校验并提交 Tool、Hook 变换、配置、权限、会话、事件和界面贡献 | 直接加载第三方模块或管理 Plugin Host 进程 |

来源发现、用户选择和当前启用版本继续由各自已有归属模块管理；`PluginRuntimeClient` 只使用已经允许执行的插件实例，
不建立第二套来源、信任、激活状态或插件生命周期对象。

## 3. Plugin Host 进程布局

### 3.1 默认复用规则

同一实际承载 Agent Runtime/`RuntimeServices` 的 Rust 进程中，Plugin Host 按下面的兼容性决策复用：

```mermaid
flowchart TD
  Plugin["插件实例"] --> Host{"同一机器和用户?"}
  Host -->|"否"| New["新进程"]
  Host -->|"是"| Backend{"运行环境相同?"}
  Backend -->|"否"| New
  Backend -->|"是"| Security{"安全范围相同?"}
  Security -->|"否"| New
  Security -->|"是"| Singleton{"要求独占?"}
  Singleton -->|"是"| New
  Singleton -->|"否"| Reuse["复用进程"]
```

workspace、session、插件和贡献数量都不是默认进程键。一个 Plugin Host 可以承载多个来源、多个
workspace 的逻辑实例和多个 session 的调用；这些身份必须随请求显式传递，进程内状态仍按生态的真实语义分区。
因此 Shared Agent Runtime 中多个 GUI/TUI/Remote Client 不会各自创建 Plugin Host；一次性 Embedded CLI、私有 SDK Host 和
目标机器 Runtime 则各自只管理自己的子进程树，不跨 Rust 进程或 execution domain 共享模块实例。

容量压力本身不直接创建进程。只有测量证明单进程队列不足，并且待拆调用同时满足“无共享模块状态、无顺序 Hook
语义、状态可序列化且可独立恢复”，才允许增加 Plugin Host。它不是通用 worker pool；闭包、模块实例、`globalThis`
或隐式单例没有显式合同与兼容测试时，仍留在原 Host。

### 3.2 插件之间的隔离承诺

Plugin Host 的首要目的，是把第三方 JS/TS 异常与 Rust 主应用进程隔开，不是把插件彼此隔开。共享同一 Host 的
插件会共同承担同步死循环、OOM、`process.exit`、进程级环境修改和未文档化全局状态带来的风险。BitFun 可以按
插件身份归因普通异常和撤下贡献，但不承诺同一 Host 内的插件故障互不影响，也不承诺兼容插件之间依赖
`globalThis` 或模块缓存的未文档化协作。

## 4. 生命周期

### 4.1 首次启动与装载

静态准备：

```mermaid
flowchart LR
  Discover["Discover"] --> Parse["Parse"] --> Approve["Approve"] --> Prepare["Prepare files"]
```

运行激活：

```mermaid
flowchart LR
  Activate["Activate"] --> Acquire["Acquire Host"] --> Init["Initialize"] --> Validate["Validate"] --> Publish["Publish"]
```

不能等到“首次工具调用”才启动通用 Plugin Host，因为 Hook、事件订阅、认证或 Provider 插件可能没有工具调用，
但必须先完成 import 才能知道真实贡献。依赖准备和 Host 启动在后台进行，不阻塞 GUI/TUI 主线程。

同一 Host 内的插件初始化按固定生态顺序执行。单个初始化抛错时，回滚该插件本次收集的贡献并继续处理后续
插件；如果初始化破坏进程、阻塞事件循环或使协议不可用，则按整个 Host 故障处理。

### 4.2 正常运行与并发

调用调度：

```mermaid
flowchart LR
  Call["Call"] --> Ordered{"Ordered Hook?"}
  Ordered -->|"yes"| Serial["Serial"]
  Ordered -->|"no"| Budget["Budget"] --> Concurrent["Concurrent"]
  Serial --> Result["Result"]
  Concurrent --> Result
```

取消与超时：

```mermaid
flowchart LR
  Cancel["Cancel / timeout"] --> Reject["Reject late result"] --> Healthy{"Host healthy?"}
  Healthy -->|"yes"| Continue["Continue"]
  Healthy -->|"no"| Reclaim["Reclaim tree"]
```

写调用和可能产生副作用的调用不自动重试；只读调用仅在归属模块明确声明可重试时使用有限退避。心跳与进程存活由
Rust 监督路径检查，不能依赖可能已被同步插件代码阻塞的业务消息队列。并发额度来自在途请求、队列字节、调用类别和
真实资源测量，不由 workspace 数量推导。

通用设计不承诺自动识别 CPU 密集调用或把它们移入 worker pool。若未来为某类明确的无状态、可序列化调用增加
独立执行进程，必须单独定义状态复制、取消、如何拒绝迟到结果、资源上限和兼容差异。

### 4.3 更新与安全重启

插件 import 可能立即启动后台任务或产生文件、网络和进程副作用。因此新旧 Plugin Host 不能同时加载同一组插件。
旧 Host 服务期间只能做不执行插件代码的来源、完整性、依赖和策略检查；真正加载新代码需要一个短暂停机窗口。

本节描述完整生命周期目标。当前 OC-R2 可用切片已实现内容摘要、逻辑 generation 撤下、崩溃后的进程树回收与
下一次使用恢复；正常源码更新、配置停用时的共享 Host 物理 generation 替换仍按兼容矩阵第 6 节作为后续生命周期
工作跟踪。在该项完成前，更新后的贡献不会与旧逻辑 generation 并行发布，但 import 期创建且未被插件 `dispose`
清理的进程内副作用可能持续到 Host 崩溃或应用退出，因此当前状态不能表述为已完成安全热更新。

```mermaid
flowchart LR
  Change["Source changed"] --> Check["Static checks"] --> Ready["Ready to restart"]
```

```mermaid
sequenceDiagram
  participant Life as Source owner
  participant Client as Runtime client
  participant Service as Process service
  participant Caps as Capability owners

  Life->>Client: Stop new calls
  Client-->>Life: Calls settled
  Life->>Service: Stop old Host
  Service-->>Life: Process tree stopped
  Life->>Caps: Withdraw old contributions
  Life->>Service: Start new Host
  Service-->>Life: Ready + contributions
  Life->>Caps: Register contributions
  Life->>Client: Accept calls
```

进程服务必须通过 OS 进程句柄确认主进程退出，并确认 Job Object 或 process group 管理的后代已经回收；IPC 连接关闭本身
不构成停止证据。若旧进程树无法确认停止，不启动新 Host。若新 Host 加载或贡献注册失败，插件保持不可用；只有保存了
完整、校验通过的旧版本文件时，才可以按同一停机顺序重新启动旧版本，不能让旧进程在后台继续运行。

更新期间显示“更新中”。每个响应只按产生它的进程连接和 request id 结算；旧连接的迟到消息只记诊断。进程服务只管理
进程和连接，不发布 Tool、Hook 或其他业务贡献。

### 4.4 停用、空闲与应用退出

共享 Host 中的模块可能在加载时启动后台任务，因此停用一个插件不能只撤下它的贡献。普通停用也先停止并确认旧 Host，
再启动只包含剩余插件的新 Host；权限撤销或安全策略失效时立即阻止新调用并停止旧 Host，再检查和恢复仍然合规的插件。
取消后的迟到响应按 request id 丢弃，不需要增加插件级编号。

```mermaid
flowchart LR
  Disable["Disable"] --> Stop["Stop old"] --> Remaining["Start remaining"]
  Revoke["Security revoke"] --> StopNow["Stop now"] --> Review["Review remaining"]
```

Host 生命周期由真实使用情况驱动：

```mermaid
flowchart LR
  Use["Plugins in use"] --> Host["Host running"]
  Host --> Stop["Stop new calls"] --> Dispose["Dispose"] --> Kill["Reclaim tree"]
  Host --> Lost["Process lost"] --> Recover["Reload or unavailable"]
```

首个完整 package-plugin 实现不对通用 Plugin Host 做空闲回收：保持一个共享进程通常比反复冷启动、import 全部
模块和重建状态更省时，也更容易维护。以后只有仅含可确定重建的按需工具、没有订阅和内存状态依赖，并且实测
内存收益高于冷启动成本时，才可以增加有期限的休眠；恢复时重新读取当前插件内容，并在新连接完成初始化后接受调用。

承载 RuntimeServices 的 Rust 进程退出时按以下顺序处理：停止新调用、取消可取消请求、有限等待正在执行的调用、逆序 dispose、
终止完整进程树。Shared Agent Runtime 的单个 Client 断线不是该生命周期事件；只要仍有活动插件实例、在途调用、事件订阅、
后台任务或其他 Client，兼容 Plugin Host 继续复用。清理超时不能阻止监督进程退出。

## 5. 状态归属与恢复

| 状态 | 权威位置 | Host 重启后的处理 |
|---|---|---|
| 来源、用户选择、执行许可和内容摘要 | 外部来源与安全归属模块 | 重新读取，不由 Host 猜测 |
| 当前内容版本与贡献注册 | 对应来源/能力归属模块 | 重新读取内容版本；完整重载并校验后再发布 |
| 重复请求结果和故障诊断 | 当前 package 路径由 Core lifecycle 与 OpenCode adapter 共同生成并投影到只读诊断；legacy managed 路径由 `PluginRuntimeClient` 持有 | 按明确恢复条件清理，不由新进程静默抹除 |
| 子进程句柄、IPC 连接、物理健康和重启预算 | package Host 由 OpenCode adapter 持有并复用 services 进程树原语；standalone worker 由 `ScriptToolRuntime` 持有 | 同一进程故障只消费一次进程级重启预算 |
| 模块实例、`globalThis`、闭包和内存缓存 | Plugin Host | 易失；不复制、不持久化，也不承诺恢复 |
| Tool/Hook/Config/Permission/Session 最终状态 | 各能力归属模块 | 不从 Host 内存反向恢复 |

workspace 可以参与来源查找、working directory、配置和某个插件实例的逻辑键，但不能因此升级为进程生命周期
归属模块。session、turn 和 call 只用于调用身份、取消和权限上下文，也不拥有 Plugin Host。

## 6. 故障传播与恢复

故障范围：

```mermaid
flowchart LR
  Failure["Failure"] --> Scope{"Scope"}
  Scope -->|"call"| Call["End call"]
  Scope -->|"plugin init"| Plugin["Rollback plugin"]
  Scope -->|"process"| Host["Stop Host"]
```

进程恢复：

```mermaid
flowchart LR
  Lost["Host lost"] --> Withdraw["Withdraw"] --> Budget{"Retry budget?"}
  Budget -->|"yes"| Reload["Reload"]
  Budget -->|"no"| Unavailable["Unavailable"]
  Lost --> Effect{"Outcome known?"}
  Effect -->|"no"| Unknown["Result uncertain"]
```

Windows 使用 Job Object，Unix 至少使用独立 process group 管理完整进程树。无法阻止后代脱离或限制资源时必须显示
残余风险，不能把“独立进程”描述为完整沙箱。Plugin Host 故障不会终止 Rust 主应用，也不能让每个插件实例分别拉起
整组进程形成重启风暴。

## 7. 当前实现

```mermaid
flowchart LR
  Config["显式配置的 package plugin"] --> Assembly["Core lifecycle assembly"]
  Assembly --> Host["Shared Bun Plugin Host"]
  Host --> Runtime["typed HookFunctionRuntime"]
  Runtime --> Owners["Tool / Hook / Config / Permission / Agent / Skill owners"]
  Script["Standalone .js tool"] --> Worker["Dedicated Node worker"]
```

当前 package-plugin 切片会准备并加载配置中显式声明的插件，发布 generation-fenced 注册快照，并把完整合并配置、
Config Hook、Tool、`tool.execute.before/after`、反向 metadata/ask 和最小 Client gateway 接入现有 owner。一个 Rust
Runtime 进程监督一个物理 Host，Host 可承载多个 workspace 逻辑实例；Session/Tool 调用只携带上下文，不成为进程键。
已知 workspace 的创建/激活失败会发布对应范围可查询的 `plugin.activation_failed` 并继续原生 Session；
existing-session ensure 暂缺 execution root 时的诊断可能进入全局范围，按兼容文档第 6 节跟踪。初次激活失败时没有
插件贡献；刷新失败可保留已确认的上一代贡献，因此诊断表达“本次激活/刷新失败”，不能把原生路径成功写成插件激活成功。

standalone `.js` Tool 继续由 `ScriptToolRuntime` 为每个脚本启动 Node worker，两条执行路径不共享生命周期对象。当前
分发仍要求系统 Bun 或 `BITFUN_BUN_COMMAND`，自动目录发现、完整 OpenCode Client/Hook/TUI 表面、安全启用门禁、
不可变旧版本恢复和资源沙箱仍未完成；共享 Host 进程隔离也不承诺插件间隔离。

## 8. 验证要求

当前 Rust 边界调整至少运行：

- `cargo test --locked -p bitfun-runtime-ports --no-default-features --features plugin-runtime --test plugin_runtime_contracts plugin_runtime_contracts`
- `cargo test --locked -p bitfun-runtime-ports --no-default-features --features plugin-runtime --test plugin_runtime_contracts plugin_runtime_diagnostics_contracts`
- `cargo test -p bitfun-plugin-runtime-client`
- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-core --no-default-features --features plugin-runtime --lib plugin_runtime::tests`
- `node scripts/check-core-boundaries.mjs`

目标 Plugin Host 还必须使用固定版本真实 fixture 验证：

1. 多插件确定加载顺序、多个导出、Hook 顺序和并发 Tool 调用；
2. 普通异常、初始化失败、同步死循环、OOM、`process.exit`、协议损坏和进程树回收；
3. 共享进程故障时全部插件实例的同时失效、单次重启预算和禁止副作用重放；
4. 更新时先完成不执行插件代码的静态检查；停止旧组新调用并有界完成或取消在途调用，确认旧进程树已终止后，
   才加载新 Host 并发布贡献；验证旧连接迟到消息、结果未知、新 Host 加载失败和贡献注册失败；
5. 本地与 Remote、不同 OS 用户、不同后端，或沙箱、网络、环境变量和凭据条件不兼容时的必要多进程拆分；
6. 启动时间、常驻内存、并发吞吐、队列等待、安全重启停机时间和恢复时间。
