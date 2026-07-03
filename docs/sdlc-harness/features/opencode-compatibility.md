# BitFun 子模块设计：Plugin Runtime Host 与 OpenCode 兼容适配

> 上游文档：[design.md](../design.md)、
> [core-decomposition.md](../../architecture/core-decomposition.md)、
> [agent-runtime-services-design.md](../../architecture/agent-runtime-services-design.md)、
> [plugin-runtime-host-design.md](../../architecture/plugin-runtime-host-design.md)

## 1. 模块定位

本模块描述 SDLC Harness 场景下的插件生态接入方式。OpenCode 兼容能力是 BitFun
Plugin Runtime Host 内部的 compatibility adapter，不是 BitFun 内部插件模型、运行时内核或公共 API owner。

SDLC Harness 只关心插件能力进入开发治理链路后的信任、权限、证据、风险提示和 UI 投影。具体执行必须复用
BitFun 的 Plugin Runtime Host、Tool ABI、Event Manifest、Permission/Effect Control Plane 和
UI Extension Contract。

核心原则：

- OpenCode API 只作为兼容输入和映射目标；BitFun 内部模块只依赖自身稳定 contract。
- 插件、hook、自定义工具和工具复写默认是主动配置，必须先发现、记录来源和权限，再进入信任审查。
- 插件只能产出候选效果；授权、阻断、审计、通过/失败状态和权威任务事实由 BitFun 内核、安全边界和执行层写入。
- UI 扩展只通过 descriptor 暴露 slot、route、command、prompt、dialog、state view 等贡献，不暴露 React、Tauri、DOM 或具体 renderer handle。
- 可写 JS/TS 插件运行时不是默认能力，必须在沙箱、secret、网络、包源安全和崩溃隔离具备独立评审后再开放。

## 2. 架构对齐

| 能力 | 所属架构合同 | 本模块使用方式 |
|---|---|---|
| 插件生命周期 | Plugin Runtime Host | install、activate、deactivate、reload、dispose 必须可撤销、可审计 |
| 工具接入 | Tool ABI | built-in tool、MCP tool、plugin tool 使用同一 snapshot、provider identity、permission/effect 和 stale call guard |
| 事件订阅 | Event Manifest / Quality Data Plane | 插件消费 public event manifest；事件写入、回放和投影由质量数据面承接 |
| 权限与副作用 | Permission/Effect Control Plane | 插件 hook 只能返回 candidate decision；最终安全决策由安全边界完成 |
| UI 扩展 | UI Extension Contract | OpenCode TUI 能力映射为 descriptor，由各产品入口选择是否渲染 |
| 产品组装 | Product Assembly | 选择 adapter manifest/capability set、host 隔离等级、支持矩阵和 product capability |

禁止依赖：

- 插件或 OpenCode adapter 直接依赖 `bitfun-core/product-full`。
- 插件拿到 full `RuntimeServices`、concrete provider handle、session manager 或 UI implementation。
- OpenCode payload 进入 Agent Kernel、Execution、Security Boundary 的内部状态对象。

## 3. 范围与边界

范围：

- 发现 OpenCode 风格配置、hook、自定义工具和插件入口，并写入项目画像。
- 维护 OpenCode server plugin 和 TUI plugin 的支持矩阵。
- 将 OpenCode 事件、工具、permission hook、配置 transform 和 UI contribution 映射为 BitFun contract。
- 将插件输出整理为建议、证据候选、工具输入补丁、工具后置观察、UI contribution 或 typed unsupported。
- 为信任审查、安全决策、证据包、PR 就绪度和回放评测提供事件与测试夹具。

边界：

- 不复制 OpenCode 运行时，也不承诺任意社区插件无修改运行。
- 不允许插件直接写任务状态、审计事实、权限状态、PR 门禁、证据通过状态或安全策略。
- 不允许工具复写静默覆盖全局工具；复写必须按项目执行域、来源、hash、能力范围和期限生效。
- 不把插件失败解释为任务失败；失败必须进入降级、警告、候选丢弃或安全决策。
- 不把 JS worker、subprocess 或 WebView 当成自动可信沙箱。

## 4. OpenCode 映射模型

### 4.1 Server Plugin

| OpenCode 能力 | BitFun 映射 | 约束 |
|---|---|---|
| `tool.execute.before/after` | Tool ABI hook / evidence candidate | 只能建议、补丁或记录证据；不能直接写通过/失败 |
| `permission.ask` | Permission candidate hook | 最终 decision、audit、policy write 仍由安全边界完成 |
| custom tool | Tool provider contribution | 必须声明 schema、provider identity、capability/effect、cancellation 和 stale call guard |
| event subscription | Event Manifest subscription | 只能消费 public event；不能读取 session 内部结构 |
| config/provider/model transform | Scoped transform descriptor | 必须进入支持矩阵；未知能力返回 typed unsupported |
| client log / notification | Quality Data Plane / UI projection | 作为日志或提示候选，不作为权威事实 |

### 4.2 TUI Plugin

| OpenCode 能力 | BitFun 映射 | 约束 |
|---|---|---|
| route / panel / slot | UI contribution descriptor | 各入口可选择渲染或返回 unsupported |
| keymap / command | UiCommandDescriptor | 不能绕过权限和产品命令注册 |
| prompt augmentation | Prompt contribution candidate | 必须标注来源和信任级别 |
| dialog / toast | UI notification descriptor | 只能投影提示，不写权威状态 |
| state access | Read-only state view | 只能读取投影，不暴露内部 store |
| theme | Theme contribution descriptor | 不能写全局 CSS、DOM 或宿主实现对象 |

## 5. 数据合同

兼容适配器需要使用窄合同，避免暴露实现细节：

```ts
interface ExtensionCompatibilityDescriptor {
  adapter_id: string;
  ecosystem: "opencode" | "kiro" | "claude_code" | "bitfun_native";
  source: {
    kind: "workspace" | "user" | "organization" | "remote";
    location: string;
    hash?: string;
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

- IPC 信封、候选效果和 UI descriptor 的权威 schema 见
  [`plugin-runtime-host-design.md`](../../architecture/plugin-runtime-host-design.md)。
- `project_epoch`、`trust_epoch`、`policy_epoch` 或 `tool_registry_epoch` 变化后，旧响应不得应用。
- 所有候选效果必须通过 `PluginResponseEnvelope` 进入应用流程；不得传递裸候选对象。
- 候选效果进入策略或安全边界前，必须重新校验 capability/effect 声明。
- UI contribution 必须声明目标 surface、fallback 和 required capabilities。
- 真实工具结果只能由 Tool ABI 下的 Execution 路径写入；插件只能返回工具前置补丁或工具后置证据候选。

## 6. 安全与质量保护

| 风险 | 保护方式 |
|---|---|
| OpenCode API 反向污染内部模型 | 内部只依赖 BitFun Plugin Runtime Host、Tool ABI、Event Manifest、Permission/Effect 和 UI descriptor |
| 插件越权授权 | hook 只返回 candidate；最终安全决策和审计由安全边界写入 |
| 工具复写绕过内置工具策略 | 复写按项目执行域生效，重新经过 Tool ABI、权限声明和安全边界 |
| UI 插件耦合前端实现 | descriptor-only；宿主适配器渲染；不支持时返回 unsupported 或安全文本投影 |
| 跨项目状态串扰 | trust、event subscription、tool override、workspace path 和授权绑定 project domain |
| 远程/本地语义漂移 | envelope 必须携带 execution domain、logical path、workspace identity 和 capability facts |
| JS/TS 运行时供应链风险 | 默认不开放可写运行时；需要包源、secret、网络、权限和崩溃隔离安全评审 |
| 兼容承诺过宽 | 支持矩阵逐项列出能力、级别、测试和降级；禁止泛称“全量兼容” |

## 7. 兼容等级

本表描述支持等级，不是排期承诺。

| 能力等级 | 必须满足的合同 |
|---|---|
| Discovery | 能发现配置、记录来源/hash/权限并默认禁用 |
| Read-only adapter | 能消费 public event manifest，输出建议或证据候选，失败可降级 |
| Tool provider adapter | Tool ABI、permission/effect、stale call guard、cancellation 和项目执行域隔离通过测试 |
| UI contribution adapter | descriptor round-trip、入口 fallback、只读 state view 和 unsupported 行为通过测试 |
| Writable / executable plugin runtime | 沙箱、secret、网络、包源安全、资源预算、崩溃隔离和审计具备独立评审结论 |

## 8. 成功标准

- OpenCode adapter 不成为 BitFun 内部 owner。
- 插件配置能被发现、解释、禁用、重新信任和撤销。
- 插件只能通过候选效果影响治理链路，不能直接写权威状态。
- 工具、MCP、自定义工具和插件工具使用同一 Tool ABI 语义。
- UI 扩展能力可在 Desktop、CLI、Server、Remote、ACP、Web、Mobile Web、SDK 等形态下明确支持或显式不可用。
- 未支持 OpenCode 能力返回 typed unsupported，不静默忽略。
- adapter 失败、超时、旧响应和权限拒绝能被安全边界、质量数据面和证据包追踪。
