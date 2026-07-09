# BitFun 子模块设计：Plugin Runtime Host 与 OpenCode 兼容适配

> 上游文档：[design.md](../design.md)、
> [product-architecture.md](../../architecture/product-architecture.md)、
> [agent-runtime-services-design.md](../../architecture/agent-runtime-services-design.md)、
> [plugin-runtime-host-design.md](../../architecture/plugin-runtime-host-design.md)

## 1. 模块定位

本模块描述 SDLC Harness 场景下的插件生态接入方式。OpenCode 兼容能力是 BitFun
Plugin Runtime Host 内部的 compatibility adapter，不是 BitFun 内部插件模型、运行时内核或公共接口归属方。

SDLC Harness 只关心插件能力进入开发治理链路后的信任、权限、证据、风险提示、插件诊断和候选效果。具体执行必须复用
BitFun 的 Plugin Runtime Host、Tool ABI、Event Manifest 和 Permission/Effect Control Plane；未预算界面贡献必须等真实入口消费方和安全评审后再进入后续阶段。

核心原则：

- OpenCode 接口只作为兼容输入和映射目标；BitFun 内部模块只依赖自身稳定接口。
- BitFun 插件安装包、随版本携带插件包、项目/组织插件源、受控外部包源、签名包和 registry 包是生产插件来源；OpenCode 配置只作为兼容导入源。
- 插件、hook、自定义工具和工具复写默认是主动配置，必须先发现、记录来源、hash、信任和权限，再进入信任审查。
- 插件只能产出候选效果；任务事实和审计事实落盘、工具结果、权限与安全决策 payload、就绪度/门禁视图分别由 Agent Kernel、Execution、Security Boundary 和变更就绪度模块写入。
- 未预算界面贡献不进入当前 P0 稳定接口；后续阶段也不得暴露 React、Tauri、DOM 或具体渲染句柄。
- 可写 JS/TS 插件运行时不是默认能力，必须在沙箱、secret、网络、包源安全和崩溃隔离具备独立评审后再开放。

外部参考口径：

- [OpenCode 配置](https://opencode.ai/docs/config/)将 TUI 设置放在独立 `tui.json`，主题、键位和 TUI 行为不与 server 配置共用同一配置域；[OpenCode 插件](https://opencode.ai/docs/plugins/)主要通过事件和 hook 扩展行为。
- [OpenCode 主题](https://opencode.ai/docs/themes/)面向终端能力，支持内置主题、用户/项目主题目录、ANSI/truecolor、`none` 终端默认色和 JSON 颜色引用。
- [Codex CLI 配置](https://developers.openai.com/codex/config-reference)把 `tui.keymap.*`、`tui.status_line`、`tui.notifications`、`tui.terminal_title` 和 `tui.theme` 归入 `tui.*` 配置域；这些键不定义 GUI 入口扩展接口。

## 2. 架构对齐

| 能力 | 所属架构接口 | 本模块使用方式 |
|---|---|---|
| 插件来源与生命周期动作 | 能力服务 / 产品特性命令 + Plugin Runtime Host | 安装、启用、禁用、卸载和审计由能力服务 / 产品特性命令负责；Host 只消费已启用来源视图并提供 availability、read/dispatch、restart 清理和诊断 |
| 工具接入 | Tool ABI | built-in tool、MCP tool、plugin tool 使用同一 snapshot、provider identity、permission/effect 和 stale call guard |
| 事件订阅 | Event Manifest / Quality Data Plane | 插件消费 public event manifest；事件归一化、回放和只读视图由质量数据面承接，权威事件仍由对应 owner 产生 |
| 权限与副作用 | Permission/Effect Control Plane | 插件 hook 只能返回 candidate decision；最终安全决策由安全边界完成 |
| 未预算界面贡献 | 后续入口形态接口 | 当前 P0 不进入稳定面；只有真实入口消费方和安全评审后，才允许按 `tui`、`gui`、`web` 等目标入口形态映射 |
| 产品组装 | Product Assembly | 选择 adapter manifest/capability set、host 隔离等级、支持矩阵和 product capability |

禁止依赖：

- 插件或 OpenCode adapter 直接依赖 `bitfun-core/product-full`。
- 插件拿到 full `RuntimeServices`、concrete provider handle、session manager 或 UI implementation。
- OpenCode payload 进入 Agent Kernel、Execution、Security Boundary 的内部状态对象。

## 3. 范围与边界

范围：

- 发现 BitFun 插件来源中的 OpenCode-compatible 插件、hook、自定义工具和插件入口，并写入项目画像。
- 可选导入 OpenCode 风格配置、工作区插件目录或全局插件目录，并将其转换为 BitFun 插件来源、manifest、hash、诊断和信任状态。
- 维护 OpenCode server plugin 和 TUI plugin 的 unsupported / 后续阶段判定矩阵。
- 将 OpenCode 事件、工具和 permission hook 映射为 BitFun 接口；配置 transform、provider/model transform 和未预算界面贡献返回类型化 `unsupported` 或进入后续安全评审。
- 将插件输出整理为诊断、建议、只读证据候选、工具后置观察或类型化 `unsupported`；工具输入补丁不进入当前 P0。
- 为信任审查、安全决策、证据包、PR 就绪度和回放评测提供事件与测试夹具。

边界：

- 不复制 OpenCode 运行时，也不承诺任意社区插件无修改运行。
- 不要求用户本机安装 `opencode` CLI 才能加载或诊断 OpenCode-compatible 插件；外部 OpenCode CLI/ACP 互操作属于独立能力。
- 不把 `opencode.json`、`.opencode/plugins/*.js|ts` 或 OpenCode 全局插件目录作为 BitFun 插件生态的主配置系统。
- 不允许插件直接写任务状态、审计事实、权限状态、PR 门禁、证据通过状态或安全策略。
- 不允许工具复写静默覆盖全局工具；复写必须按项目执行域、来源、hash、能力范围和期限生效。
- 不把插件失败解释为任务失败；失败必须进入降级、警告、候选丢弃或安全决策。
- 不把 JS worker、subprocess 或 WebView 当成自动可信沙箱。

## 4. OpenCode 映射模型

### 4.1 Server Plugin

| OpenCode 能力 | BitFun 映射 | 约束 |
|---|---|---|
| `tool.execute.before/after` | 诊断 / 只读证据候选 | 当前 P0 不改写输入或结果；可写补丁必须进入 P0+ 安全评审 |
| `permission.ask` | Permission candidate hook | 最终 permission decision、`security.decided` payload 和安全审计 payload 由 Security Boundary 生成；审计事实落盘由 Agent Kernel 维护 |
| custom tool | Tool provider contribution | 必须声明 schema、provider identity、capability/effect、cancellation 和 stale call guard |
| event subscription | Event Manifest subscription | 只能消费 public event；不能读取 session 内部结构 |
| config/provider/model transform | 类型化 `unsupported` / P0+ 安全评审 | 任意 provider/model/config 转换不进入当前 P0 |
| client log / notification | Quality Data Plane / UI 提示视图 | 作为日志或提示候选，不作为权威事实 |

### 4.2 TUI Plugin（后续阶段）

本节描述后续阶段可能映射的 OpenCode TUI 能力，不属于当前产品运行时 P0 稳定接口。TUI 能力只能进入目标入口形态为 `tui` 的声明式接口；不能因为某个插件存在 GUI 路由、面板、对话框或 CSS 主题键，就推导出 TUI 支持。

| OpenCode 能力 | BitFun 映射 | 约束 |
|---|---|---|
| 斜杠命令 / 自定义命令 | TUI 入口命令候选 | 当前 P0 返回 unsupported/status-only；后续必须走产品命令注册、权限门禁和冲突检测 |
| 键位 | TUI 键位候选 | 只能声明命令意图和建议键位；宿主负责前导键、上下文优先级、冲突和禁用语义 |
| 状态行 / 终端标题 / 通知 | TUI 状态与通知候选 | 只能生成只读提示视图；不得写会话、任务或安全状态 |
| 提示词增强 | 提示词贡献候选 | 必须标注来源和信任级别 |
| 状态读取 | 只读状态视图 | 只能读取只读视图，不暴露内部 store |
| 主题 | TUI 主题语义 token 候选 | 只能声明语义角色和 TUI 回退；不能写全局 CSS、DOM、GUI 设计 token 或宿主实现对象 |
| 路由/面板/槽位/对话框/提示 | 非 TUI 能力 | OpenCode TUI 映射不承接 GUI 布局能力；如需 GUI 支持，必须进入目标入口形态为 `gui` 的后续接口 |

### 4.3 入口形态与主题键

TUI 和 GUI 的插件接入必须先声明目标入口形态。BitFun 不提供“全入口 UI 插件接口”，也不把主题键设计为跨入口共用的原始键集合。

| 声明项 | TUI 处理 | GUI 处理 | 不支持时 |
|---|---|---|---|
| 入口形态 | 目标入口形态为 `tui`，只允许命令、键位、状态/通知、主题语义 token 和只读状态 | 目标入口形态为 `gui`，只允许路由、面板、槽位、对话框、提示、主题语义 token 和只读状态 | 返回类型化 `unsupported`，包含目标入口形态和原因 |
| 主题 token | 语义角色映射到终端颜色、ANSI/truecolor 或 `none` 终端默认色 | 语义角色映射到设计 token、CSS 变量或组件主题槽 | 使用语义回退；无回退时返回类型化 `unsupported` |
| 入口专用键 | `tui.keymap.*`、状态行项、终端通知、终端主题名等只在 TUI 宿主内解释 | GUI 路由、槽位、组件主题槽、焦点策略等只在 GUI 宿主内解释 | 不跨入口透传原始键 |
| 可执行界面代码 | 不支持 | 不支持 | 返回类型化 `unsupported` 或安全诊断 |

主题映射采用两层结构：插件侧声明稳定的语义角色；宿主侧维护入口形态映射表。后续 PR 若要实现主题贡献，必须同时提供：目标入口形态、语义角色清单、宿主映射表、冲突处理、无障碍/对比度验证、类型化 `unsupported` 行为和退场条件。

## 5. 数据接口与 schema

兼容适配器需要使用窄接口，避免暴露实现细节。以下结构是只读视图草图，用于说明 schema 边界，不是当前稳定接口：

```ts
interface ExtensionCompatibilityView {
  adapter_id: string;
  ecosystem: "opencode";
  source: {
    kind: "package" | "bundled" | "project_source" | "organization_source" | "controlled_external_package" | "signed_bundle" | "registry_package";
    scope: "project" | "workspace" | "user" | "organization";
    location: string;
    hash?: string;
    imported_from?: "opencode_json" | "opencode_workspace_plugin" | "opencode_global_plugin";
  };
  trust: {
    state: "discovered" | "trusted" | "changed" | "disabled" | "revoked";
    scope: "project" | "worktree" | "session" | "organization";
  };
  capabilities: CapabilityDeclaration[];
  effects: EffectDeclaration[];
}
```

应用规则：

- IPC 信封、候选效果、插件诊断和隔离事实的权威 schema 见
  [`plugin-runtime-host-design.md`](../../architecture/plugin-runtime-host-design.md)。
- `project_epoch`、`trust_epoch`、`policy_epoch` 或 `tool_registry_epoch` 变化后，旧响应不得应用。
- 所有候选效果必须通过 `PluginResponseEnvelope` 进入应用流程；不得传递裸候选对象。
- 候选效果进入策略或安全边界前，必须重新校验 capability/effect 声明。
- 未预算界面贡献不进入当前 P0 稳定面；只有真实入口消费方和安全评审出现后，才声明目标入口形态、回退和 required capabilities。
- 真实工具结果只能由 Tool ABI 下的 Execution 路径写入；当前 P0 插件只能返回工具后置证据候选。工具前置补丁必须等真实消费方和安全评审后再进入后续阶段。

## 6. 安全与质量保护

| 风险 | 保护方式 |
|---|---|
| OpenCode 接口反向污染内部模型 | 内部只依赖 BitFun Plugin Runtime Host、Tool ABI、Event Manifest、Permission/Effect、插件诊断和插件效果候选 |
| OpenCode 配置反向成为主模型 | 配置导入只生成 BitFun 插件来源、manifest、hash、诊断和信任快照；运行时不直接消费原始配置 |
| 外部 OpenCode CLI 可用性污染 P0 | P0 插件加载不检查用户是否安装 `opencode`；外部 CLI/ACP 互操作独立降级 |
| 插件越权授权 | hook 只返回 candidate；最终安全决策和安全审计 payload 由 Security Boundary 生成，审计事实落盘由 Agent Kernel 维护 |
| 工具复写绕过内置工具策略 | 复写按项目执行域生效，重新经过 Tool ABI、权限声明和安全边界 |
| UI 插件耦合前端实现 | 当前 P0 不开放未预算界面贡献；TUI 与 GUI 分别声明目标入口形态和主题语义 token；不支持时返回 unsupported 或安全文本视图 |
| 跨项目状态串扰 | trust、event subscription、tool override、workspace path 和授权绑定 project domain |
| 远程/本地语义漂移 | envelope 必须携带 execution domain、logical path、workspace identity 和 capability facts |
| JS/TS 运行时供应链风险 | 默认不开放可写运行时；需要包源、secret、网络、权限和崩溃隔离安全评审 |
| 兼容承诺过宽 | 支持矩阵逐项列出能力、级别、测试和降级；禁止泛称“全量兼容” |

## 7. 兼容等级

本表描述支持等级，不是排期承诺。

| 能力等级 | 必须满足的接口条件 |
|---|---|
| Discovery | 能发现 BitFun 插件来源、记录来源/hash/权限并默认禁用；签名包和 registry 包必须保留签名、撤销和回滚事实；可选 OpenCode 配置导入只能生成来源事实和 `imported_from` provenance |
| Read-only adapter | 能消费 public event manifest，输出建议或证据候选，失败可降级 |
| Tool provider adapter | Tool ABI、permission/effect、stale call guard、cancellation 和项目执行域隔离通过测试 |
| 界面贡献适配器 | 仅在真实入口消费方和安全评审后进入后续阶段；必须验证目标入口形态、声明式界面接口、主题映射、入口回退、只读 state view 和 unsupported 行为 |
| Writable / executable plugin runtime | 沙箱、secret、网络、包源安全、资源预算、崩溃隔离和审计具备独立评审结论 |

## 8. 成功标准

- OpenCode adapter 不成为 BitFun 内部 owner。
- 当前产品运行时 P0 覆盖产品架构定义的 OpenCode-compatible 插件来源、诊断和最小候选效果消费路径；full plugin runtime、ACP permission bridge、未预算界面贡献和可写 JS/TS 插件运行时属于 P0+。这里的 P0+ 指产品架构 P0 后的独立产品决策阶段，不等同于 SDLC Harness P1。
- BitFun 插件来源能被发现、解释、禁用、重新信任和撤销；OpenCode 配置导入只能服务迁移和兼容诊断。
- 插件只能通过候选效果影响治理链路，不能直接写权威状态。
- 工具、MCP、自定义工具和插件工具使用同一 Tool ABI 语义。
- 界面扩展能力只有在真实入口消费方和公开接口预算后才进入支持矩阵；当前阶段必须能显式返回不可用或 unsupported。
- 未支持 OpenCode 能力返回类型化 `unsupported`，不静默忽略。
- adapter 失败、超时、旧响应和权限拒绝能被安全边界、质量数据面和证据包追踪。
