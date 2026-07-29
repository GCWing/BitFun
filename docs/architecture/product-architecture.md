# BitFun 产品运行时架构

本文件定义 BitFun 产品运行时的稳定架构边界。详细执行计划见
[`../plans/core-decomposition-plan.md`](../plans/core-decomposition-plan.md)；智能体内核、运行时服务和 crate
约束见 [`agent-runtime-services-design.md`](agent-runtime-services-design.md)；插件运行时、Plugin Host 进程和生态适配细节见
[`plugin-runtime-design.md`](extensions/plugin-runtime-design.md)；跨 GUI/TUI 的产品定制、布局选择和
内置扩展边界见 [`product-customization-blueprint.md`](product-customization-blueprint.md)；CLI 产品入口和配置
兼容见 [`cli-product-line-design.md`](cli-product-line-design.md)；HarmonyOS PC 原生 CLI/TUI 平台规约见
[`platform-portability-design.md`](platform-portability-design.md)。跨专题实施顺序见
[`../plans/product-architecture-evolution-plan.md`](../plans/product-architecture-evolution-plan.md)。外部 AI 工作内容体验、OpenCode 扩展总矩阵、配置资产、插件执行、
终端插件和外部集成适配分别见 [`external-ai-work-sources-design.md`](extensions/external-ai-work-sources-design.md)、
[`opencode-extension-compatibility.md`](extensions/opencode-extension-compatibility.md)、
[`opencode-config-assets-adapter-design.md`](extensions/opencode-config-assets-adapter-design.md)、
[`opencode-plugin-runtime-adapter-design.md`](extensions/opencode-plugin-runtime-adapter-design.md)、
[`opencode-tui-plugin-adapter-design.md`](extensions/opencode-tui-plugin-adapter-design.md) 和
[`opencode-external-integration-adapter-design.md`](extensions/opencode-external-integration-adapter-design.md)；BitFun 能力如何
可装配并双向接入 Claude Code、Codex、OpenCode、Trae 等宿主见
[`capability-runtime-integration-design.md`](extensions/capability-runtime-integration-design.md)；公开 BitFun Agent SDK、SDK Host、
Headless CLI 与各产品入口的统一心智见
[`agent-sdk-product-architecture.md`](agent-sdk-product-architecture.md)；多个 GUI/TUI/Remote/CLI/SDK 实例共存时的 Agent Runtime 部署、
状态共享、隔离、容量与 Plugin Host 关系见
[`agent-runtime-deployment-design.md`](agent-runtime-deployment-design.md)。详细设计与本文件冲突时，以本文件为准。

本文件只约束稳定边界，不记录单次 PR 进度，也不把未来可能支持的生态能力提前声明为公开接口。

## 1. 架构目标

BitFun 同时面向桌面 GUI、TUI/CLI、Web、ACP、Server、Remote、SDK 和插件生态。架构目标是降低后端实现高频变更对稳定接口的影响，同时保持插件生态和 OpenCode-compatible 能力可以按受控路径扩展。

设计原则：

1. **接口少而稳定**：每个接口边界只有一个主入口；不能因为新增生态适配或实现重构而新增平行接口。
2. **实现不外溢**：运行时、平台服务、生态适配器、插件执行单元和传输实现只能通过稳定接口、只读视图或内部 ABI 被消费。
3. **外部语义可变换，最终提交有归属**：OpenCode Hook 可以按其稳定语义修改输入、输出和权限决定；BitFun
   归属模块负责顺序、结构、一致性和用户/组织策略校验并提交最终状态，不能把可写 Hook 一律降级成只读候选。
4. **OpenCode 是兼容目标，不是内部模型**：适配层尽量保持 OpenCode plugin、hook、custom tool、TUI plugin、
   Client、配置和加载顺序的外部可观察行为，但这些类型不能反向成为 BitFun 智能体、配置或界面的内部数据模型。
5. **公开接口有预算**：新增公开 DTO、trait、模块或入口必须同时具备归属模块、真实消费方、版本策略、验证方式和删除条件。
6. **入口形态受宿主约束**：TUI、GUI、Web、Headless CLI 和公开 SDK adapter 共享 Agent Runtime API
   用例、能力服务接口和只读视图，不共享公开语言包、传输、渲染句柄、主题键、键位模型或界面状态；
   插件界面贡献必须先声明目标入口形态，再由对应宿主适配。
7. **产品定制先解析，运行时扩展后加载**：产品身份、能力上限和 GUI/TUI 布局选择在构建/组装期解析；用户配置和插件只能在该上限内扩展，不能反向改写产品事实。
8. **平台差异留在入口和具体能力实现**：target 只选择 ABI，feature 只控制确实可选的依赖；共享内核不按平台
   分叉业务语义，也不新增包含所有 OS 方法的总接口。新端口必须有当前调用方。
9. **发现无感，生效按风险分级**：外部用户/项目来源后台发现，不阻塞产品入口；无冲突的低风险声明式内容可自动应用并提供撤销；
   Command、Tool、Subagent 等可执行来源与产品本地能力或独立外部 provider 同名时必须由用户选择，且选择只在候选身份与内容版本不变时复用。现有 Skill 根继续按已发布顺序解析，并展示来源和默认覆盖项；带模式的管理界面展示应用模式开关后的实际采用项；
   可执行来源首次启用或能力扩大时形成非阻塞确认。激活后的本地 OpenCode 扩展默认按当前用户能力
   运行；经 BitFun 能力接口的调用可细分限制，脚本直接文件/网络/进程能力只在真实操作系统或容器边界存在时可
   粗粒度收紧，否则停用相应插件。策略降级必须与待确认、解析错误和插件故障分开显示。
10. **开放权限不降低可靠性**：第三方 JS/TS 始终位于受监督子进程；standalone Tool 使用现有 worker，完整 package plugin
    使用 Plugin Host。目标边界具备期限、取消、流量控制、崩溃回收、错误去重和结构校验；业务等待不得被单个插件无限阻塞。
    缺少平台硬资源限制时，内存、CPU 或进程风暴仍是明确残余风险，不能用“独立进程”宣称完全隔离。
11. **来源发现与执行许可分离**：生态来源和加载顺序只决定候选输入，不自动授予执行权限。任何可执行来源在
   首次激活、启动或 import 前，以及来源身份、内容版本、执行域/用户、策略上限或凭据/环境可见范围
    变化时，由既有归属模块重新检查是否允许执行。经 BitFun 接口发起的调用仍执行调用时权限判断；脚本运行时的
   直接文件、网络和进程副作用只能依靠真实 OS/容器边界限制。来源的首次选择由产品来源体验保存，
    不因此新增对内部准备阶段的重复激活、通用 trusted-folder 模型或独立信任服务。
12. **一个能力核心，多种宿主适配**：Memory、Context、Workflow、Subagent、Tool 等能力只在已有 owner 中按真实
     第二实现增量开放 Provider/策略装配；对外通过少量能力接口和宿主 adapter 暴露。MCP、Plugin、Hook、SDK 或
     Server 入口不能反向替换状态 owner、权限上限、取消树、资源硬额度、事件身份或审计，也不能被描述为一个
     跨产品通用插件包。
13. **一个 Agent Runtime，多种交付形态**：GUI、TUI、Headless CLI、公开 SDK、ACP 与 Server/Remote 都是
     同一 Agent Runtime 的 adapter。Query、Session、Tool/MCP、Permission、Hook、Event/Usage 只有一个行为
     归属模块；公开 SDK 不成为内部入口的依赖，ACP 和 Headless CLI 也不成为完整 SDK 的别名。目标部署中，第一方
     GUI/TUI/本机 Remote 可以共享 Agent Runtime，一次性 Headless CLI 保留 Embedded，公开 SDK 默认使用私有 SDK Host；
     这些 Rust 部署都只通过 `PluginRuntimeClient` 和 services 归属模块管理自己的 Node/Bun Plugin Host 子进程。

调用路径长度只作为工程成本处理，不作为独立架构目标。允许保留承担兼容隔离、只读视图或能力选择职责的中间层；不允许为了兼容而长期暴露没有消费方的抽象接口。

## 2. 4+1 Architecture Views

4+1 视图分别描述系统职责、代码组织、运行协作、部署边界和关键场景，避免把逻辑模块、crate、进程和调用链混在同一张图中。分类沿用 [Kruchten 4+1](https://www3.software.ibm.com/ibmdl/pub/software/rational/web/whitepapers/2003/Pbk4p1.pdf)，图的层级、动态协作和部署节点表达参考 [C4](https://c4model.com/diagrams) 以及 arc42 的 [Building Block](https://docs.arc42.org/section-5/)、[Runtime](https://docs.arc42.org/section-6/) 和 [Deployment](https://docs.arc42.org/section-7/) 视图；这些方法只提供视角和表达规则，不替代 BitFun 的真实 owner 与代码边界。

Level 0 展示系统级主要边界和依赖方向；Level 1 再按 Level 0 的模块或范围展开。每张图必须能独立说明范围和图例，关系使用明确方向或协议，逻辑模块、crate、运行任务和部署实例不要求一一对应。

### 2.1 Logical View · Level 0

Logical View 只表达当前系统的职责模块与依赖方向，不表达 crate 归属或进程位置。

```mermaid
%%{init: {"theme":"base","flowchart":{"curve":"basis","nodeSpacing":28,"rankSpacing":34},"themeVariables":{"fontFamily":"Inter, ui-sans-serif, system-ui","primaryColor":"#ffffff","primaryTextColor":"#171717","primaryBorderColor":"#737373","lineColor":"#525252","secondaryColor":"#fafafa","tertiaryColor":"#ffffff","clusterBkg":"#ffffff","clusterBorder":"#a3a3a3"}}}%%
flowchart TB
  subgraph Consumers["Consumers"]
    direction LR
    Users["Users"] ~~~ APIClients["API Clients"]
  end

  subgraph BitFun["BitFun"]
    direction TB

    subgraph Product["Product"]
      direction LR
      ProductHosts["Product Hosts"]

      subgraph RuntimeAssembly["Runtime Assembly"]
        direction LR
        CapabilityPlan["Capability Plan"] ~~~ RuntimeWiring["Runtime Wiring"]
      end

      ProductFeatures["Product Features"]
    end

    RuntimeAPI["Runtime API"]

    subgraph AgentCore["Agent Core"]
      direction LR

      subgraph AgentKernel["Agent Kernel"]
        direction LR
        SessionState["Session State"] ~~~ TaskControl["Task Control"]
      end

      subgraph Execution["Execution"]
        direction LR
        AgentLoop["Agent Loop"] ~~~ ToolRuntime["Tool Runtime"]
      end
    end

    subgraph Extensions["Extensions"]
      direction LR
      Contributions["Contributions"] ~~~ PluginRuntime["Plugin Runtime"]
    end

    ServicePorts["Service Ports"]

    subgraph SharedBoundary["Shared Boundary"]
      direction LR
      StableContracts["Stable Contracts"]
      SecurityControl["Security Control"]
      PlatformServices["Platform Services"]
      StableContracts ~~~ SecurityControl ~~~ PlatformServices
    end
  end

  PluginEcosystems["Plugin Ecosystems"]

  subgraph PlatformSystems["Platform Resources"]
    direction LR
    AIProviders["AI Providers"] ~~~ OS["OS"] ~~~ RemoteSystems["Remote Systems"]
  end

  Consumers --> ProductHosts
  ProductHosts --> ProductFeatures
  ProductHosts --> RuntimeAPI
  ProductFeatures --> RuntimeAPI
  RuntimeAssembly -.-> ProductHosts
  RuntimeAssembly -.-> ProductFeatures
  RuntimeAssembly -.-> AgentKernel
  RuntimeAssembly -.-> Extensions
  RuntimeAPI --> AgentKernel
  AgentKernel --> Execution
  Execution --> ServicePorts
  Extensions ==> Execution
  Extensions ==> ServicePorts
  ServicePorts --> SharedBoundary
  Extensions ==> PluginEcosystems
  SharedBoundary --> PlatformSystems

  classDef module fill:#ffffff,stroke:#737373,stroke-width:1.3px,color:#171717;
  classDef interface fill:#fafafa,stroke:#404040,stroke-width:1.6px,color:#171717;
  class Users,APIClients,ProductHosts,CapabilityPlan,RuntimeWiring,ProductFeatures,SessionState,TaskControl,AgentLoop,ToolRuntime,Contributions,PluginRuntime,StableContracts,SecurityControl,PlatformServices,PluginEcosystems,AIProviders,OS,RemoteSystems module;
  class RuntimeAPI,ServicePorts interface;

  style BitFun fill:#ffffff,stroke:#171717,stroke-width:2.2px;
  style Product fill:#fafafa,stroke:#737373,stroke-width:1.3px;
  style AgentCore fill:#fafafa,stroke:#737373,stroke-width:1.3px;
  style SharedBoundary fill:#fafafa,stroke:#737373,stroke-width:1.3px;
  style RuntimeAssembly fill:#ffffff,stroke:#a3a3a3;
  style AgentKernel fill:#ffffff,stroke:#a3a3a3;
  style Execution fill:#ffffff,stroke:#a3a3a3;
  style Extensions fill:#ffffff,stroke:#737373,stroke-width:1.3px;
  style Consumers fill:#ffffff,stroke:#a3a3a3;
  style PlatformSystems fill:#ffffff,stroke:#a3a3a3;
```

实线表示依赖，虚线表示装配，粗线表示扩展路径。

| Boundary | Responsibility |
|---|---|
| Product | 承载产品入口、能力选择和用户功能 |
| Runtime API | 向产品入口提供稳定用例接口 |
| Agent Core | 管理会话与任务，并推进 Agent 和工具执行 |
| Extensions | 接收生态贡献并隔离插件执行 |
| Service Ports | 隔离执行逻辑与具体平台能力 |
| Shared Boundary | 统一稳定契约、安全控制和平台服务 |

稳定边界：Product Hosts 只经过 Runtime API 和只读投影消费能力，不能直接调用插件执行单元或具体平台实现；Extensions 只能提交受控贡献，最终状态、权限结果、工具结果和审计事实仍由对应 owner 提交。`PluginRuntimeClient` 持有类型化调用、期限、串行化和响应校验；物理进程健康与进程树回收属于 Services。

### 2.2 Development View · Level 0

Development View 展示仓库的静态代码组织。层间依赖只允许向下，可跨过中间层，但不能反向依赖上层；图中子项表示主要 crate 家族或产品入口，不等同于 Logical View 的职责模块。

```mermaid
flowchart TB
  subgraph AppsLayer[" "]
    direction LR
    AppsTitle["1 · Apps & Interfaces"] ~~~ Desktop["Desktop"] ~~~ CLI["CLI"] ~~~ Server["Server"] ~~~ Relay["Relay"] ~~~ WebUI["Web UI"] ~~~ MobileUI["Mobile UI"] ~~~ ACP["ACP"] ~~~ SDKHost["SDK Host"]
  end

  subgraph AssemblyLayer[" "]
    direction LR
    AssemblyTitle["2 · Assembly"] ~~~ CoreAssembly["Core Assembly"] ~~~ ExternalSources["External Sources"] ~~~ ProductCaps["Product Capabilities"]
  end

  subgraph AdaptersLayer[" "]
    direction LR
    AdaptersTitle["3 · Adapters"] ~~~ RuntimeIPC["Runtime IPC"] ~~~ ModelAdapters["Model Adapters"] ~~~ SourceAdapters["Source Adapters"] ~~~ Transport["Transport"] ~~~ WebDriver["WebDriver"]
  end

  subgraph ServicesLayer[" "]
    direction LR
    ServicesTitle["4 · Services"] ~~~ CoreServices["Core Services"] ~~~ Integrations["Integrations"] ~~~ MiniAppMarket["MiniApp Market"] ~~~ RelayService["Relay Service"] ~~~ Terminal["Terminal"] ~~~ PageRuntime["Page Runtime"]
  end

  subgraph ExecutionLayer[" "]
    direction LR
    ExecutionTitle["5 · Execution"] ~~~ AgentRuntime["Agent Runtime"] ~~~ AgentStream["Agent Stream"] ~~~ ToolRuntime["Tool Runtime"] ~~~ PluginClient["Plugin Client"] ~~~ Harness["Harness"] ~~~ RuntimeServices["Runtime Services"]
  end

  subgraph ContractsLayer[" "]
    direction LR
    ContractsTitle["6 · Contracts"] ~~~ CoreTypes["Core Types"] ~~~ Events["Events"] ~~~ RuntimePorts["Runtime Ports"] ~~~ ProductDomains["Product Domains"]
  end

  AppsTitle --> AssemblyTitle --> AdaptersTitle --> ServicesTitle --> ExecutionTitle --> ContractsTitle

  classDef header fill:#fafafa,stroke:#404040,stroke-width:1.6px,color:#171717;
  classDef module fill:#ffffff,stroke:#737373,stroke-width:1.3px,color:#171717;
  class AppsTitle,AssemblyTitle,AdaptersTitle,ServicesTitle,ExecutionTitle,ContractsTitle header;
  class Desktop,CLI,Server,Relay,WebUI,MobileUI,ACP,SDKHost,CoreAssembly,ExternalSources,ProductCaps,RuntimeIPC,ModelAdapters,SourceAdapters,Transport,WebDriver,CoreServices,Integrations,MiniAppMarket,RelayService,Terminal,PageRuntime,AgentRuntime,AgentStream,ToolRuntime,PluginClient,Harness,RuntimeServices,CoreTypes,Events,RuntimePorts,ProductDomains module;
  style AppsLayer fill:#ffffff,stroke:#a3a3a3;
  style AssemblyLayer fill:#ffffff,stroke:#a3a3a3;
  style AdaptersLayer fill:#ffffff,stroke:#a3a3a3;
  style ServicesLayer fill:#ffffff,stroke:#a3a3a3;
  style ExecutionLayer fill:#ffffff,stroke:#a3a3a3;
  style ContractsLayer fill:#ffffff,stroke:#a3a3a3;
```

箭头表示允许的依赖方向；实际 crate 可以直接依赖任意更低层。Logical 与 Development 的主要映射如下，映射是多对多关系：

| Development layer | Repository scope | Logical elements |
|---|---|---|
| Apps & Interfaces | `src/apps/*`、Web/Mobile UI、`interfaces/*` | Product Hosts、Product Features、Runtime API |
| Assembly | `assembly/*` | Runtime Assembly、Product Features、Agent Kernel |
| Adapters | `adapters/*` | Extensions、Platform Services |
| Services | `services/*` | Agent Kernel、Security Control、Platform Services |
| Execution | `execution/*` | Agent Core、Extensions、Service Ports |
| Contracts | `contracts/*` | Stable Contracts、Security Control、Service Ports |

Assembly 是唯一组装根，只选择下层能力和实现，不能反向依赖 app。每个生态 adapter 独立保留外部格式和顺序语义，再映射到 BitFun owner；生态 adapter 之间不能形成兄弟依赖。

### 2.3 Process View · Level 0

Process View 展示当前 Agent Runtime 内的异步任务、流和取消传播；Embedded 与 Shared 复用同一任务结构。本视图不描述具体部署环境，也不把一次用户场景误作进程结构。

```mermaid
flowchart LR
  HostRequest["Host Request"]
  RuntimeAPI["Runtime API"]
  SessionOwner["Session Owner"]
  TurnTask["Turn Task"]
  ModelAdapter["Model Adapter"]
  AIProvider["AI Provider"]
  StreamTask["Stream Task"]
  ToolTasks["Tool Tasks"]
  ServicePorts["Service Ports"]
  OSProcess["OS Process"]
  TurnState["Turn State"]
  EventRouter["Event Router"]
  HostEvents["Host Events"]

  HostRequest --> RuntimeAPI --> SessionOwner
  SessionOwner -->|spawn| TurnTask
  SessionOwner -.->|cancel| TurnTask
  SessionOwner -.->|cancel| StreamTask
  SessionOwner -.->|cancel| ToolTasks
  TurnTask -->|request| ModelAdapter --> AIProvider
  AIProvider -->|stream| StreamTask -->|chunks| TurnState
  TurnTask -->|spawn| ToolTasks --> ServicePorts -->|spawn / I/O| OSProcess
  TurnTask --> TurnState
  ToolTasks -->|results| TurnState
  TurnState --> EventRouter
  StreamTask --> EventRouter
  ToolTasks --> EventRouter
  EventRouter --> HostEvents

  classDef host fill:#fafafa,stroke:#404040,stroke-width:1.5px,color:#171717;
  classDef task fill:#ffffff,stroke:#737373,stroke-width:1.3px,color:#171717;
  classDef boundary fill:#ffffff,stroke:#737373,stroke-width:1.3px,stroke-dasharray:4 3,color:#171717;
  class HostRequest,RuntimeAPI,HostEvents host;
  class SessionOwner,TurnTask,StreamTask,ToolTasks,TurnState,EventRouter task;
  class ModelAdapter,AIProvider,ServicePorts,OSProcess boundary;
```

实线表示调用、数据或事件流，虚线表示取消传播。Session Owner 持有会话与活动 turn 的生命周期；Turn、Stream 和 Tool 任务可异步重叠，但只能通过事件和类型化结果提交状态。产品入口只经过 Runtime API，不能直接调用 Tool Tasks 或具体平台进程。

### 2.4 Physical View · Level 0

Physical View 展示当前可执行单元到设备、主机和存储的映射。Desktop、CLI、ACP 和 SDK Host 使用 Embedded Runtime；交互式 TUI 可以显式连接 Shared Runtime。当前 Web Server 和 Relay Server 都不承载 Agent Runtime。

```mermaid
flowchart LR
  subgraph LocalHost["Local Host"]
    direction TB
    subgraph EmbeddedNodes["Embedded"]
      direction LR
      DesktopApp["Desktop App"] ~~~ CLIApp["CLI App"] ~~~ ACPApp["ACP"] ~~~ SDKHost["SDK Host"]
    end
    SharedRuntime["Shared Runtime"]
    WorkspaceData["Workspace Data"]
    ToolProcesses["Tool Processes"]
  end

  subgraph UserDevice["Client Device"]
    direction TB
    WebClient["Web Client"]
    MobileClient["Mobile Client"]
  end

  WebServer["Web Server"]

  subgraph RelayHost["Relay Node"]
    direction TB
    RelayServer["Relay Server"]
    RelayDB["Relay DB"]
    AssetStore["Asset Store"]
  end

  AIProviders["AI Providers"]
  RemoteHosts["Remote Hosts"]

  WebClient -->|WebSocket| WebServer
  MobileClient -->|HTTPS| RelayServer
  DesktopApp <-->|WebSocket| RelayServer
  CLIApp <-->|WebSocket| RelayServer
  CLIApp -.->|Local IPC| SharedRuntime
  RelayServer --> RelayDB
  RelayServer --> AssetStore
  EmbeddedNodes --> WorkspaceData
  SharedRuntime --> WorkspaceData
  EmbeddedNodes -->|spawn| ToolProcesses
  SharedRuntime -->|spawn| ToolProcesses
  EmbeddedNodes -->|HTTPS| AIProviders
  SharedRuntime -->|HTTPS| AIProviders
  DesktopApp -->|SSH| RemoteHosts

  classDef unit fill:#ffffff,stroke:#737373,stroke-width:1.3px,color:#171717;
  class DesktopApp,CLIApp,ACPApp,SDKHost,SharedRuntime,WorkspaceData,ToolProcesses,WebClient,MobileClient,WebServer,RelayServer,RelayDB,AssetStore,AIProviders,RemoteHosts unit;
  style LocalHost fill:#ffffff,stroke:#737373;
  style EmbeddedNodes fill:#ffffff,stroke:#a3a3a3;
  style UserDevice fill:#ffffff,stroke:#a3a3a3;
  style RelayHost fill:#ffffff,stroke:#737373;
```

实线表示主要协议、存储访问或进程创建，虚线表示显式启用的 Shared TUI 本机连接。Relay DB 只在账户模式启用，Asset Store 的具体实现由部署配置选择。完整 package plugin 尚未形成生产闭环，因此不把规划中的 Plugin Host 画成当前部署实例。

| Deployment unit | Main contents |
|---|---|
| Desktop App | Web UI、Tauri Host、embedded Agent Runtime |
| CLI App | TUI、Headless、Peer；默认 Embedded，可显式使用 Shared TUI |
| Shared Runtime | 私有本机 IPC；当前只有交互式 TUI consumer |
| ACP | Embedded Agent Runtime、ACP 协议生命周期 |
| SDK Host | 私有跨进程 adapter；公开 SDK 产品尚未交付 |
| Web Server | Health、Info、WebSocket 外壳；不包含 Agent Runtime |
| Relay Server | WebSocket/HTTP bridge、账户与同步；不包含 Agent Runtime |

### 2.5 Scenarios (+1) · Level 0

Scenarios 选择少量具有架构意义的当前路径来校验前四个视图，不穷举产品功能，也不重复 Process View 的任务调度细节。

```mermaid
flowchart TB
  subgraph InteractiveTurn["Chat Turn"]
    direction LR
    TurnUser["User"] --> TurnHost["Product Host"] --> TurnCore["Agent Core"] --> TurnProvider["AI Provider"] --> TurnResponse["Response"]
  end

  subgraph ToolExecution["Tool Run"]
    direction LR
    ToolCore["Agent Core"] --> ToolRuntime["Tool Runtime"] --> ToolPorts["Service Ports"] --> PlatformResource["Platform Resource"] --> ToolResult["Tool Result"]
  end

  subgraph SourceDiscovery["Source Scan"]
    direction LR
    SourceRoots["Source Roots"] --> SourceAdapters["Source Adapters"] --> ControlPlane["Control Plane"] --> SourceHost["Product Host"]
  end

  subgraph RemoteControl["Remote Turn"]
    direction LR
    RemoteClient["Mobile Client"] --> RemoteRelay["Relay Server"] --> RemoteDesktop["Desktop Host"] --> RemoteAPI["Runtime API"] --> RemoteCore["Agent Core"]
  end

  InteractiveTurn ~~~ ToolExecution ~~~ SourceDiscovery ~~~ RemoteControl

  classDef step fill:#ffffff,stroke:#737373,stroke-width:1.3px,color:#171717;
  class TurnUser,TurnHost,TurnCore,TurnProvider,TurnResponse,ToolCore,ToolRuntime,ToolPorts,PlatformResource,ToolResult,SourceRoots,SourceAdapters,ControlPlane,SourceHost,RemoteClient,RemoteRelay,RemoteDesktop,RemoteAPI,RemoteCore step;
  style InteractiveTurn fill:#ffffff,stroke:#a3a3a3;
  style ToolExecution fill:#ffffff,stroke:#a3a3a3;
  style SourceDiscovery fill:#ffffff,stroke:#a3a3a3;
  style RemoteControl fill:#ffffff,stroke:#a3a3a3;
```

四条路径分别覆盖核心对话、内置工具、运行时无关的外部来源发现，以及经 Relay 回到 Desktop owner 的远程控制。Source Scan 的发现与控制事实由 `ExternalSourceControlPlane` 统一持有，adapter 不成为第二个业务 owner。完整 package plugin、公开 SDK 产品和 HarmonyOS 不在当前生产闭环中，因此不作为 Level 0 场景。

## 3. 接口边界

BitFun 只保留四个稳定接口边界；工具、事件和权限作为归属子接口被复用，不在插件层重复定义。本文使用“接口”描述可被调用或依赖的能力面；只有描述跨进程消息封装、结构化 schema、序列化对象或强兼容约束时才使用“契约”；只读状态视图表示从权威状态派生出的查询结果。

| 接口边界 | 谁使用 | 提供 | 不包含 |
|---|---|---|---|
| Agent Runtime API | GUI、TUI/CLI、Web、ACP、Server、Remote、SDK adapter | Query、Session、Tool/MCP、Permission、Hook、Event、Usage | UI、协议和具体服务实现 |
| BitFun 与插件接口 | `PluginRuntimeClient`、安全模块、产品组装、生态适配器 | 来源、能力、Hook 变换、界面贡献、诊断 | 最终权限、工具结果、审计和内核状态 |
| 插件运行时接口 | Runtime、执行层、产品组装、`PluginRuntimeClient` | 请求身份、期限、响应校验和诊断 | SDK/UI 对象、生态原始对象和进程句柄 |
| 外部生态兼容接口 | 来源管理、能力模块、`PluginRuntimeClient`、Plugin Host | 发现、顺序、参数、诊断和明确映射 | 跨生态任意数据、兄弟适配器依赖和外部 CLI 前置依赖 |

这四项是能力必须归入的接口分类，不表示表中每项已有稳定 API。当前接口仍须满足 3.1 节的真实消费方、版本与验证条件。

归属子接口：

| 子接口 | 归属 | 用法 |
|---|---|---|
| 工具 ABI | `tool-contracts` / 执行层 | 具备真实执行实现的插件 custom tool、MCP 工具和内置工具进入同一可调用工具集合、权限和陈旧调用保护路径；只有声明或候选项的插件工具不能进入该集合。 |
| 事件清单 | `events` / 智能体内核事件 schema | 对固定生态版本维护各自事件清单；插件观察兼容事件，BitFun 内部私有字段在对应适配层转换或脱敏。 |
| 权限与副作用 | 安全模块 / runtime ports | 插件启用后，默认兼容策略允许 OpenCode `permission.ask` 和直接脚本能力按当前用户权限运行；经 BitFun 接口的调用可细分收紧，直接脚本能力只能由真实 OS/容器环境粗粒度限制，否则停用插件。 |

### 3.1 公开接口进入条件

新增或保留公开接口必须满足以下条件：

1. 属于上表一个明确接口边界，不能同时承担前后端协议、插件扩展、host ABI 和生态适配职责。
2. 有当前消费方；仅为了未来兼容、完整矩阵或概念完整性保留的代码接口不进入稳定面。该规则不阻止需求、
   风险、完整能力矩阵和阶段计划记录未来工作，也不能用来把官方稳定能力从兼容审计中删除。
3. 能映射到 OpenCode-compatible P0 关键场景，或属于 BitFun 已有关键路径的稳定子接口。
4. 不能由既有工具 ABI、事件清单、权限模块或能力服务接口承接时，才允许新增。
5. PR 必须说明版本影响、验证命令和删除条件。

`scripts/core-boundaries/rules/source/public-api-rules.mjs` 当前是插件与运行时公开接口的增量 allowlist，不是全仓
`pub` 符号扫描器。已登记接口必须声明 `contractSlice` 供机器校验归属；未登记接口仍须满足上述进入条件，并由
PR 审查和最近的边界测试验证。边界脚本通过不能解释为全仓公开接口已经自动完成预算审计。

没有 OpenCode 对应能力、没有当前消费方、不能归入关键 BitFun 场景的接口，处理方式只有三种：删除、降级为主机内部实现，或返回类型化 `unsupported` / 诊断。

已批准后续工作所需的短期前置接口不等于占位实现。确需预留时，必须在相邻设计中写明首个消费方、稳定语义、
接入验证和未接入时的删除条件；在端到端调用链落地前保持内部可见或显式标为未接入，不能用空实现、测试替身或
公开 re-export 宣称产品支持。无法给出这些信息时，仍按无消费方接口处理。

“前后端能力接口”是概念边界，不对应一个必须存在的统一 API crate。单一宿主使用的命令转换、宿主协议 DTO 和
协议转换留在该宿主入口；只有多个当前生产宿主或独立版本化的外部消费者确实复用同一语义，并且版本与删除条件
明确时，才抽取共享 API 模块。仅返回合成 ID、空历史、固定健康状态，或绕过既有服务直接执行文件 I/O 的占位
handler，不构成生产消费完整流程。

传输 adapter 是已接入宿主的交付实现，不是未来协议路线图。保留一个 transport adapter 必须同时存在生产构造点、
事件或请求消费方、宿主生命周期，以及错误、取消或流量控制语义的验证。独立存在的 Server 路由、前端 WebSocket
client 或未来 CLI/HarmonyOS 计划，不能证明同名 Rust transport adapter 已接入；未接入实现应删除，待端到端
调用链确定后再按宿主边界实现。

### 3.2 宿主通信契约与 Tauri 薄适配

前后端契约按能力语义归属，不按 Tauri command 名称归属。稳定的请求、响应、状态事实和类型化错误放在对应
`contracts/*`、Agent Runtime API 或能力归属模块；Tauri、HTTP/WebSocket、CLI/TUI、ACP 与公开 SDK
Host adapter 只负责把各自协议映射到
这些类型。该规则降低框架耦合，但不要求把每个 Desktop DTO 都搬进共享 crate。

| 层 | 允许 | 禁止 |
|---|---|---|
| 能力归属模块 / Agent Runtime API | 字段明确的请求和响应、状态事实、权限/取消规则、与框架无关的用例方法 | `tauri::State`、`AppHandle`、窗口/菜单对象、command 宏、HTTP/WebSocket/ACP/SDK Host 消息结构 |
| Desktop Tauri adapter | 读取宿主状态、构造稳定请求、调用对应 Agent Runtime API 或归属模块接口、把明确错误转换为 Desktop 协议、投递桌面事件 | 复制业务校验、持有第二份权威状态、把 Tauri 类型传入下层 |
| Server / Remote adapter | 路由鉴权、协议消息、连接生命周期、流量控制与取消转换 | 为同一能力另建业务含义不同的 DTO 或 handler |
| GUI / TUI 消费方 | 依赖入口侧 API interface、稳定读模型或 Agent Runtime API；各自保留渲染状态 | 依赖公开 Python/TypeScript SDK、直接持有平台句柄，或让 React/TUI 状态成为后端契约 |

本文其他章节和历史设计中出现的“Runtime SDK”，如果指 `agent-runtime::sdk`，统一称为
**Rust Runtime SDK（当前 preview）**；它是共享 **Agent Runtime API** 的当前 Rust 入口。只有
[`agent-sdk-product-architecture.md`](agent-sdk-product-architecture.md) 定义的 Python/TypeScript package 才称为公开
**BitFun Agent SDK**，其跨进程适配器称为 **SDK Host**。该术语区分不要求机械重命名现有 crate/module，但禁止用
Rust preview 的存在证明公开 SDK 已交付。

第一方多实例目标称为 **Shared Agent Runtime deployment**。承载它的 Rust 进程与 SDK Host、Plugin Host、Server/Relay 和 Remote
execution Host 都是不同职责；其部署与进程生命周期以
[`agent-runtime-deployment-design.md`](agent-runtime-deployment-design.md) 为准。

Rust 与 TypeScript 的字段一致性以能力所有者的 DTO 为事实源，不以 Tauri command 参数为事实源。单宿主阶段由
前端基础设施层维护对应接口，并用序列化契约测试锁定字段命名、可选字段和错误形状；达到独立版本化门槛后，才使用
不依赖 Tauri 的 JSON Schema 或类型生成任务输出只读 TypeScript 类型。生成结果只同步数据形状，不承载权限、重试或
业务分支。本阶段不为此新增生成器或框架依赖。

抽取共享契约需要满足以下任一条件：至少两个当前生产宿主复用同一语义，或存在独立版本化的外部消费者。只有一个
Desktop command 使用的序列化对象继续留在 `src/apps/desktop`；即使它不含 Tauri 类型，也不因“未来可能复用”而
提升为公共 DTO。共享的框架中立用例 handler 也遵循同一门槛：它必须拥有真实的编排、权限、取消或错误语义，不能
只是通用转发层。

单条能力按垂直切片迁移：

1. 先确认权威 owner、当前生产消费方、远程/多产品形态语义和现有行为基线。
2. 把稳定事实与请求/响应放到能力所有者的契约模块，并以序列化、错误、取消和行为等价测试锁定。
3. 让非 Desktop 消费方或第二宿主先通过 Agent Runtime API / owner 接口形成真实调用链。
4. 将 Tauri command 简化为薄 adapter；前端基础设施层负责 `invoke` 映射，UI 组件不直接依赖 Tauri API。
5. 删除重复 DTO、旧 handler 或兼容方法；无法证明等价时保留已标注的兼容边界，不做批量迁移。

因此仓库不恢复一个通用 `api-layer` 作为默认中转层。只有达到上述复用门槛且现有 owner 无法合理承载时，才评审
窄范围共享 API 模块。HarmonyOS GUI/TUI 可复用稳定能力契约，但仍需各自的平台宿主、生命周期和交付验证；契约
抽取只是前置条件，不代表 HarmonyOS 已受支持。

### 3.3 入口形态接口规则

入口形态接口只描述宿主可消费的声明，不描述具体渲染实现。TUI 与 GUI 的能力边界不同，不能因为存在一个界面插件就自动扩展为全入口稳定接口。

| 目标入口形态 | 可进入稳定接口的内容 | 必须由宿主决定 | 禁止进入插件接口 |
|---|---|---|---|
| TUI / CLI | 斜杠命令、键位候选、状态行/通知候选、终端主题语义 token、只读状态视图 | 键位冲突处理、终端能力降级、ANSI/truecolor 映射、文本回退 | React/DOM/Tauri 句柄、CSS token、GUI 布局、可执行界面代码 |
| Desktop GUI / Web | 路由、面板、槽位、对话框、提示、GUI 主题语义 token、只读状态视图 | 组件装载位置、布局约束、焦点与可访问性、设计 token 映射 | 终端键位、ANSI 颜色、TUI 状态行键、宿主组件实例 |
| SDK / Server / Remote / ACP | 状态、诊断、能力清单、类型化 `unsupported` | 是否暴露只读状态或降级原因 | 任意界面贡献、主题键、渲染句柄 |

主题贡献只能声明语义角色和目标入口形态，例如 `accent`、`danger`、`surface`、`text`、`border`。TUI 宿主把语义角色映射为终端颜色、ANSI 或 truecolor；GUI 宿主把语义角色映射为设计 token 或 CSS 变量。若插件只提供 GUI 主题键而当前入口是 TUI，系统只能使用语义回退或返回类型化 `unsupported`，不得把 GUI 主题键直接传给 TUI。

## 4. 运行协作细节

本节在 Process View Level 0 之下展开产品入口、插件调用和平台能力的当前调用链；这些图描述组件协作，不构成新的 4+1 视图。

### 4.1 产品入口

```mermaid
flowchart LR
  Products["GUI · TUI · CLI · Web"] --> Adapter["入口适配器"]
  Protocol["ACP · Server · Remote"] --> Adapter
  SDK["Agent SDK"] --> SDKHost["SDK Host"]
  Adapter --> API["Runtime API"]
  SDKHost --> API
  API --> Runtime["共享 Runtime"]
  Assembly["产品组装"] -. "选择" .-> Runtime
```

### 4.2 插件调用

```mermaid
flowchart LR
  Owner["能力归属模块"] <--> Client["PluginRuntimeClient"]
  Client <--> Adapter["生态 adapter"]
  Adapter <--> Service["Process service"]
  Service <--> Host["Plugin Host"]
```

插件贡献走独立的提交链，不绕过能力归属模块：

```mermaid
flowchart LR
  Adapter["生态 adapter"] --> Provider["能力 Provider"] --> Owner["能力归属模块"]
```

### 4.3 平台能力

```mermaid
flowchart LR
  Runtime["Runtime / Plugin client"] --> Port["平台端口"]
  Port --> Adapter["平台 adapter"]
  Adapter --> System["OS / 外部系统"]
```

关键规则：

- 产品入口先经过自己的 adapter，再消费 Agent Runtime API 和只读视图；公开 SDK 只多一层 SDK Host 跨进程适配。
  Agent Runtime API 是一组小而明确的用例接口，不是必须实例化的总入口；adapter 可以调用对应归属模块的少量接口，
  但不能访问内部状态、绕过既有编排或复制业务规则。任何入口都不直接调用 Plugin Host。
- 插件只进入扩展贡献接口，不直接写内核状态、工具结果、权限结果或审计事实。
- Rust 主应用内只有 `PluginRuntimeClient` 及 services 层现有脚本执行实现：前者当前负责类型化调用、期限、同一插件实例
  串行化、重复请求结果、响应校验和故障诊断；取消结果失效、有界队列和旧连接结果拒绝只有在端口具备相应身份后
  才能作为目标能力加入。后者沿 `ScriptToolRuntime` 边界负责 Plugin Host 的物理健康、资源预算与进程树回收。Host 仅指运行
  Node/Bun 和第三方 JS/TS 的子进程；插件启停与贡献生命周期仍由既有来源和能力归属模块管理。
- 外部来源的 Command、Tool、Subagent、MCP 仍保留能力专属 DTO 和 owner，但它们的发现调度统一由
  `ExternalSourceControlPlane` 持有；跨 Desktop/TUI/Peer/Server 的控制事实只通过版本化的 product-domain 只读视图共享，
  不复制生态 payload、界面状态机或远端专用 DTO。
- 每个生态适配层独立保留该生态的外部格式、来源顺序和调用语义，并映射到 BitFun 归属模块；它本身不成为新的
  业务归属模块，也不能依赖或修改兄弟生态 adapter。通用目录、`ExternalSourceControlPlane` 和能力归属模块只依赖开放生态 ID、
  来源限定身份与能力专属 provider 契约，不按 OpenCode、Codex 或 Claude Code 分支行为。
- 产品组装是组装根，只在组装期选择能力、服务实现、插件运行时绑定和降级策略。
- 对外能力接口只提供现有归属模块的窄用例、只读状态、事件和明确错误；它不是第二个 Agent Runtime、通用服务
  定位器或插件 Host。外部产品扩展、外部 SDK 控制端和“使用外部 Runtime 组装新产品”是三种不同交付路径，
  覆盖上限和兼容结论分别维护。
- 依赖方向保持为产品入口 / interfaces → assembly → adapters / services / execution → contracts。assembly
  可以选择下层提供方，但不能依赖 app crate；需要同时被独立应用和嵌入式模式复用的实现必须下沉到可复用 owner，
  再由各 app 和 assembly 组合。

### 4.4 名词与定义归属

全仓人工维护文档、AGENTS、README 和代码注释遵守以下规则：

- `Host` 首次出现或跨文档引用时必须带限定词，并表示实际承载执行或协议的进程/产品，例如 Plugin Host、SDK Host、Peer Host；
  同一小节已明确指代后可简称 Host，Rust 插件调用可靠性实现不得称为 Host。
- 插件侧只保留插件实例、能力贡献、`PluginRuntimeClient` 和 Plugin Host 四个跨文档名词；进程监督、脚本执行、
  来源发现和能力提交直接使用已有归属模块的职责描述，不再增加平行的 Manager/Controller/Coordinator 名称。
- 不建立额外的插件运行对象、注册表或状态机。插件实例由现有来源模块标识，贡献由对应能力模块管理；Plugin Host
  只是可以承载多个插件实例的物理进程组。
- workspace 只在具体归属模块确有独立配置、状态、版本或并发单例时作为该状态的限定键；它不是通用
  runtime、Plugin Host 或 session 的别名。
- `Product Assembly` 是唯一组装名称。当前 Rust 内部入口称为
  Rust Runtime SDK（preview），只有公开 Python/TypeScript 产品称为 BitFun Agent SDK。
- 生态 adapter 必须按方向说明是来源导入还是外部宿主输出。插件兼容接口、组合规则、脚本执行后端和当前能力版本都是
  已有归属模块的具体职责，不建立同义的第二层架构名词。

专项术语只在唯一归属文档定义；其他位置链接或使用，不复制状态机、生命周期和同义职责表。历史计划保留完成
事实时也使用当前规范名词，第三方、生成内容和固定兼容 fixture 不机械改写。

## 5. OpenCode-compatible 当前基线与目标

Plugin Runtime P0 只验证了 BitFun 专用插件目录中的来源校验、工作区审核、启停记录、CLI 诊断和 custom tool 名称预览。
它不执行 JS/TS，不注册真实工具，也不运行 OpenCode 钩子、Client 或终端插件。现有能力只能称为“静态预览”，
不能称为“OpenCode 插件运行时”。详细代码事实集中在
[`plugin-runtime-design.md#7-当前实现`](extensions/plugin-runtime-design.md#7-当前实现)。

与 Plugin Runtime 分离的四条纵向基线已经通过各自的能力专属 provider 契约接入：Prompt Command 可发现本地
用户/项目 OpenCode Command、处理跨来源冲突，并在 CLI/TUI 中执行受支持的 prompt-only 模板；standalone Tool
可把受支持的单文件 `.js` 经确认后接入现有 Tool Runtime；Subagent 可把全局/项目声明的安全子集经确认和同名冲突
选择后接入现有 Task/Subagent 归属模块；fresh single-run 调用持续使用启动时选定的版本。MCP 可把受支持的用户/项目
配置经确认和同名冲突选择后交给现有 MCP owner 运行。四类贡献对象互不复用，主体逻辑不按生态分支。当前仍不表示
package plugin、OpenCode/通用动态 Hook Runtime、primary agent、外部 agent 续接、SSH Remote 工作区来源发现或完整
配置兼容已经可用。独立目录可以发现并脱敏展示 OpenCode、Claude Code 与 Codex 的本地 Hook 声明；其中只有明确审阅的
Claude Code/Codex 命令子集可复制为既有 `AgentHookEngine` 的原生层，OpenCode 和其余声明仍不加载 handler 或授予权限。

目标路线不要求 OpenCode 插件作者维护 `bitfun.plugin.json` 或复制到 `.bitfun/plugins`。BitFun 直接发现用户和
项目的 OpenCode 配置、插件目录、工具目录和软件包来源；低风险内容按用户偏好自动应用或先询问，可执行来源在
首次启用或能力扩大时非阻塞确认。用户允许执行的候选自动记录当前版本，在自有脚本进程中真实加载插件，再通过兼容
适配层把工具、稳定钩子、Client 和 TUI 插件入口接入现有归属模块。

```mermaid
flowchart LR
  Source["OpenCode 用户 / 项目来源"] --> Discover["发现配置、入口与依赖"]
  Discover --> Catalog["来源清单、使用范围与能力摘要"]
  Catalog --> Policy["自动应用 / 待确认 / 策略限制"]
  Policy --> Prepare["记录已批准版本"]
  Prepare --> Client["PluginRuntimeClient"]
  Client <--> Adapter["OpenCode adapter"]
  Adapter <--> Service["Process service"]
  Service <--> Host["Plugin Host"]
  Client <--> Owners["工具 / 配置 / 权限 / 会话 / TUI 归属模块"]
  Owners --> Surface["桌面 / CLI / Web / Remote"]
```

稳定决策如下：

- 不启动完整 OpenCode Runtime，也不依赖用户安装 OpenCode CLI；BitFun 实现自己的监督、适配和 Rust 转发层。
  当前 standalone Tool 子集通过受监督的 Node.js worker 执行且不安装依赖；未来只有固定的 package plugin 样例证明
  确有需要时，才单独裁决 Bun、依赖准备和版本兼容方案。OpenCode v2 当前同时维护 Bun 编译产物与 Node SEA 并行
  产物，因此 BitFun 不把外部项目尚未稳定的运行时选择提升为插件内部 ABI 或核心架构约束。
- 用户全局和项目来源自动发现；低风险内容默认无感应用并显示可撤销摘要，可执行来源首次启用或能力扩大时等待
  非阻塞确认。确认前不得 import module、启动 worker、读取凭据或产生直接脚本副作用。
- 激活后的本地插件默认按 OpenCode 语义运行，允许当前用户通常拥有的文件、网络、进程和环境能力；用户、
  产品或组织可以按需收紧，差异必须明确显示为策略限制。
- 同一实际承载 Agent Runtime/`RuntimeServices` 的 Rust 进程内，位于相同实际执行机器、使用相同 OS 用户和兼容脚本后端，且沙箱、网络、环境变量和凭据条件可合并的插件默认
  共享 Plugin Host。workspace、session 和 plugin 都不是默认进程键；只有执行主机、后端/原生依赖或
  进程级环境、凭据、沙箱事实不兼容时才拆分进程。
- Plugin Host 用于隔离 Rust 主应用与第三方 JS/TS，不承诺隔离同一 Host 内的插件。普通异常按调用/插件实例
  归因；进程崩溃时同组插件实例、在途调用和贡献同时失效，由 services 层执行实现按一次进程级预算恢复。
- 初始化和有序 Hook 保持生态顺序；独立调用可以在有界队列内并发，但不承诺自动识别 CPU 调用或透明迁移到
  worker pool。并发容量来自实际队列和资源测量，不按 workspace 数量推导。
- 首次可执行插件激活/import 时启动或复用 Host，通用 Host 首期不做空闲回收；更新先完成不执行插件代码的
  静态检查，再停止并确认旧进程树退出，最后加载新 Host。退出时停止新调用、有限等待、dispose 并回收完整进程树。
- 执行进程实际加载的工具、钩子和导出是权威结果；静态扫描只可用于快速预览，不能作为拒绝动态插件的依据。
- 插件工具只有具备真实定义和执行函数、接入现有 Tool Runtime 并经过调用时权限判断后，才能显示为可用工具。
- OpenCode 可写钩子按固定版本和原始顺序执行合法变换，最后由对应归属模块做结构和策略校验。
- 服务入口和终端入口独立启停、注册贡献并归因普通错误；若共享的 Plugin Host 失效，两者会共同失效。
- 来源变化先检查新版本；import 前可见的运行条件扩大时先确认。新代码只在旧进程树停止后加载；若 import 后发现
  新增动态贡献需要确认，则停止新 Host 并显示差异。静态准备失败时仍合规的旧进程可以继续；旧进程停止后失败只能从
  内容完整、校验通过的旧版本文件重新启动。明确删除、撤销、停用或策略失效必须停止旧 Host 并撤下贡献。
- GUI、TUI、Web 和 Remote 只消费能力服务、稳定状态和操作接口，不直接依赖 `PluginRuntimeClient`、Plugin Host
  进程或 OpenCode 原始类型。

最明显的首期降级是 OpenCode TUI 的原始 `CliRenderer`、Solid/OpenTUI 组件树。BitFun CLI 使用 Ratatui，无法直接
执行这些组件；宿主操作和结构化贡献可以适配，原始组件必须返回明确降级且不能打开空白或无法退出的页面。
其他暂不承诺项、原因和风险统一在
[`opencode-extension-compatibility.md#6-明确限制与延期决策`](extensions/opencode-extension-compatibility.md#6-明确限制与延期决策)
维护，不能因为某一项降级就把整体状态写成“完整覆盖”。

产品内置扩展与用户插件可以复用主机可靠性和最终能力归属，但来源、升级、卸载和产品必要性不同。只有产品
身份、安全恢复或法律要求等少量明确保护项不可被覆盖；普通内置命令、工具和主题可经用户明确选择被外部扩展
替换或关闭，不能按注册或适配器顺序静默切换。具体规则见
[`product-customization-blueprint.md#8-产品内置扩展与用户插件`](product-customization-blueprint.md#8-产品内置扩展与用户插件)。

完整能力状态、设计细节和阶段顺序分别见
[`opencode-extension-compatibility.md`](extensions/opencode-extension-compatibility.md)、
[`opencode-plugin-runtime-adapter-design.md`](extensions/opencode-plugin-runtime-adapter-design.md) 和
[`../plans/opencode-extension-compatibility-plan.md`](../plans/opencode-extension-compatibility-plan.md)。

## 6. 产品形态与降级

产品定义、Delivery Profile、Runtime Configuration 和 Capability Availability 必须分离：

- 产品定义只在构建/组装期选择产品身份、品牌资源、产品能力上限、默认策略引用、内置扩展版本和发行事实；
  不承载用户配置、凭据或任意脚本。
- Delivery Profile 只表示 CLI、Desktop、ACP、SDK 等交付形态，不表示品牌或 SKU。
- 声明一个 Delivery Profile、生成测试计划或通过 crate 单测，不等于该产品形态已经接入生产。只有入口实际提交
  唯一 profile、消费组装结果和统一能力可用性，并通过入口级行为验证后，才能把该 profile 标为已接入。
- 产品入口向组装根提交唯一 Delivery Profile；组装根只校验并派生静态计划，不在内部再次选择交付形态。
- Runtime Configuration 承载用户、项目、工作区和本次运行的可变配置；不能启用产品定义
  未组装的能力，也不能放宽产品或组织策略。
- Capability Availability 是根据产品计划、服务健康和当前策略计算出的能力状态；所有入口读取同一状态，
  入口隐藏不等于能力已禁用。
- 构建期校验器读取产品定义、品牌资源和 GUI/TUI 布局选择，输出本次交付的产品组装结果；它不是常驻服务，
  也不执行产品定义中携带的任意脚本。
- Product Assembly 只消费产品组装结果和调用方唯一传入的 Delivery Profile；不读取原始品牌资源，
  不运行构建脚本，也不从产品定义再次选择 Delivery。
- GUI 与 TUI 布局由对应宿主独立校验，只共享产品身份、Capability ID、品牌资源索引和策略引用，不共享布局、
  组件、主题键、键位或渲染状态。
- 布局选择只能引用宿主已注册的稳定 ID；品牌生成和校验继续使用仓库现有构建流程，不新增通用脚本运行时。
- 产品内置扩展、BitFun 原生包和 OpenCode 标准来源不共享来源根、信任/启用记录、安装状态、更新通道或卸载
  生命周期；三者只复用适用的包校验、插件内部 ABI、Plugin Host 进程边界和经 BitFun 能力接口的权限/审计路径。

产品定制和品牌资源的详细边界见
[`product-customization-blueprint.md`](product-customization-blueprint.md)；CLI/TUI 的消费方式和配置导入见
[`cli-product-line-design.md`](cli-product-line-design.md)。

产品形态由产品组装决定，不由插件配置、单个 Cargo feature 或生态适配器临时决定。

当前本机入口组装：

```mermaid
flowchart TB
  Desktop["Desktop"] --> Full["product-full"]
  CLI["CLI / TUI"] --> Full
  ACP["ACP"] --> Parts["Runtime Parts"]
  SDKHost["SDK Host"] --> Parts
  ServerBootstrap["Server agent bootstrap · dormant"] --> Full

  Full --> Coordinator["ConversationCoordinator"]
  Parts --> Coordinator
  Ownership["CoreRuntimeOwnership"] -. "first-party composition injects once" .-> Coordinator
```

当前公开 HTTP Server 不调用 agent bootstrap，因此不创建 Runtime 或 workspace ownership；图中的 Server 节点只记录已有 agent-enabled composition 边界，不能据此宣称 Server Agent API 已交付。

当前 Peer 运行连接：

```mermaid
flowchart LR
  Peer["Peer UI"] --> Host["Peer Host"]
```

尚未交付的公开 SDK 路径：

```mermaid
flowchart LR
  SDK["Public SDK"] -.-> Host["SDK Host"]
```

| 当前入口 | 已有能力 | 明确边界 |
|---|---|---|
| Desktop | 使用 `product-full`；显示外部来源、审批、冲突、诊断和 Host 能力 | 可执行能力在事实所在 Host 运行；Safe Mode 只阻止新调用，不改来源、不取消正在运行的调用 |
| CLI / TUI | 使用 `product-full`；提供 `/extensions`、统一 `/hooks`（旧 `/hooks_external` 为别名）、`/tools` 和 `/agents`；Claude Code/Codex 命令 Hook 可经显式审阅复制为原生层 | 生态解析仍在适配器，不启动第二套 Agent Runtime；OpenCode Hook 仍只静态发现；远程能力未接入时不回退本机 |
| ACP | 使用 `DeliveryProfile::Acp` 和 Runtime Parts | load 成功后才发布活动状态；close 排空后再卸载；完整历史和配置仍由 Core/ACP 管理 |
| Peer / Server | Server 提供 control/catalog；Peer Host 执行真实工作区操作；当前 HTTP Server 不装配 Agent Runtime | 控制端不替远端发现或执行；旧 Host 明确降级，SSH Remote 未接入时返回不支持；只读 Server 不声明 Runtime ownership |
| Web / Mobile Web | 依赖现有后端入口 | 不持有插件执行单元，也不能据空 profile 宣称独立能力 |
| HarmonyOS 手机 Remote | phone-only ArkTS 远程入口 | 不等于 HarmonyOS PC 本地 Runtime、CLI/TUI 或 GUI |

| 目标形态 | 当前状态 | 设计边界 |
|---|---|---|
| HarmonyOS PC CLI/TUI | 未实现 | HAP、手机 Remote App 和远端代执行均不能替代 |
| HarmonyOS PC GUI | 未实现 | 与 CLI/TUI 共享 Runtime 语义，但独立验证宿主、界面和发布 |
| Public Agent SDK | Python/TypeScript 尚未交付；Rust Runtime SDK 是内部 preview | 一个 `AgentClient`、多个语言绑定；SDK Host 不依赖或冒充 CLI |

Shared Agent Runtime 是第一方多实例的目标部署，不是上表新增的当前产品形态。当前文档中表示“事实实际所在位置”的泛称 Host
可能仍指 Desktop 进程、Peer、Server 或 Remote execution host，不能据此推断 Shared deployment、多 Client Session 单写或
跨进程重连已经交付；完成条件以
[`agent-runtime-deployment-design.md`](agent-runtime-deployment-design.md) 为准。

对外一级状态统一使用[外部 AI 工作内容设计](extensions/external-ai-work-sources-design.md#7-状态与提示规则)定义的
已发现、已应用、可用、需确认、更新中、沿用上一版本、部分受限、暂时过期、已移除/已停用和不可用，并附带
原因与恢复建议。Host 的准备完成、重启、暂停、不支持或失败只能作为详情映射，不能形成第二套并列产品状态。
现有代码中的过渡状态只能展示为“静态预览、未执行”，不能因为进入来源清单就误报为已应用或可用。

## 7. 完成判定

架构或实现 PR 必须满足：

- 未新增无消费方的公开接口、空注册表、泛描述符或多生态稳定接口。
- 没有把 OpenCode 类型或 CLI 可用性提升为 BitFun 内部数据模型；适配器仍应保持 OpenCode 配置、加载顺序和
  冲突的外部可观察语义。
- 插件可按 OpenCode Hook 语义提出并链式应用变换，最终结构、策略、审计和状态提交仍由对应模块完成。
- 只有名称或静态声明、没有真实执行实现的插件工具不能进入最终可调用工具集合。
- 前后端入口不能消费 `PluginRuntimeClient`、host 内部状态、生态原始载荷或插件执行单元句柄。
- 工具、事件、权限能力优先复用既有归属子接口，不在插件层重复建模。
- 可替换 Provider 只替换实现或策略，不替换 session/turn/run 身份、权威状态提交、最终权限、取消/资源硬上限、
  事件因果和审计；单选、顺序执行、名称并存、失败回退或结果汇总规则必须由能力归属模块明确。
- TUI 与 GUI 不共享内部主题键、键位模型或界面状态；OpenCode TUI 原始键和组件只存在于适配层，转换后由
  TUI 宿主消费，不能用构建期布局选择冒充运行时插件兼容。
- 只有产品身份、安全恢复和法律要求等明确保护项不能被用户扩展覆盖；普通内置工具、命令和主题作为 BitFun
  来源候选保留，跨生态同名时由用户选择，不能按注册顺序静默决胜。冲突界面固定先展示 BitFun 候选，但展示顺序
  不等于自动选择。产品内置扩展不能复用用户来源批准或启用记录，产品签名也不能绕过运行时
  权限、审计和故障隔离。
- GUI/TUI 布局选择不复制主题 schema，不固化动态能力状态，也不携带可执行 UI 或任意构建脚本。
- 新 profile 只有在真实入口消费组装结果、能力可用性和类型化降级后才算接入；仅有枚举、空计划、re-export
  或单测不构成产品支持。
- assembly 不得依赖 app crate。relay 的 room/device 状态、account/sync 存储、asset store 与 HTTP/WebSocket router
  归属 `services/relay-service`，Cargo metadata 实际解析图检查阻止同类依赖回流。Desktop embedded relay 的 TCP bind、
  静态 fallback 和任务生命周期由 `src/apps/desktop` 通过窄 `EmbeddedRelayHost` 端口持有；assembly 只保留连接方式选择、
  启停顺序和失败回滚。这项宿主接入不构成 CLI、Server、ACP 或 HarmonyOS 本地产品支持。
- HarmonyOS PC 的完整目标同时包含本地 CLI/TUI 与 GUI，当前均不能标记可用；两种宿主分别验收，具体支持证据和禁止替代项以平台规约及各自专题为准。
- 文档、边界脚本和 focused 测试能说明本次变更保护了哪个稳定接口边界，或删除/降级了哪个过宽接口。
