# OpenCode 插件兼容接口暴露面审计

本文件用于审计 OpenCode-compatible 能力进入 BitFun 前的接口暴露面。主架构口径见
[`product-architecture.md`](product-architecture.md)，插件主机内部 ABI 见
[`plugin-runtime-host-design.md`](plugin-runtime-host-design.md)。本文件不维护第二套路线图，也不为非 OpenCode 生态声明稳定接口。

## 1. 审计结论

当前风险不是缺少更多接口，而是已有文档容易把能力服务接口、扩展贡献接口、插件主机 ABI 和生态适配层混成一个宽接口。OpenCode 兼容必须先通过准入表收口，再进入实现。

结论：

- OpenCode 适配器只能是主机内部反腐层。
- BitFun 插件来源、manifest、hash、信任、诊断和权限/副作用门禁是权威路径。
- OpenCode 配置和目录只作为可选导入输入，不是 BitFun 插件主配置。
- 用户本机是否安装 `opencode` CLI 不影响 BitFun 加载或诊断 OpenCode-compatible 插件。
- TUI 与 GUI 插件能力必须分别声明目标入口形态；主题贡献只能通过语义 token 映射，不能共享或透传原始主题键。
- 没有 OpenCode 对应能力、没有当前消费方、不能复用 BitFun 工具/事件/权限子接口的公开接口，不进入稳定面。

## 2. 接口准入表

| OpenCode 能力 | P0 处理 | BitFun 承接位置 | 处理方式 |
|---|---|---|---|
| `opencode.json` plugin 配置 | 可作为兼容输入进入 P0 | OpenCode 适配层 -> BitFun 插件来源读模型 | 只读扫描，生成 provenance、manifest、hash、诊断和候选来源 |
| `.opencode/plugins/*.js|ts` | 可作为兼容输入进入 P0 | OpenCode 适配层 | 只识别文件形态和能力声明，不直接执行 |
| 全局插件目录 | 可作为兼容输入进入 P0 | OpenCode 适配层 | 只读导入；不继承 OpenCode 启用顺序 |
| npm 插件列表 | P0 可诊断，执行属于后续 | OpenCode 适配层 | 只产出来源和 unsupported / projection-only 诊断 |
| custom tool | 是，最小候选能力 | 扩展贡献接口 -> 工具 ABI | 映射为 provider candidate；物化前必须走权限和工具快照 |
| permission hook | 是，候选能力 | 权限/副作用子接口 | 只能产生候选确认或诊断；不能直接批准 |
| `tool.execute.before` | 否，P0 只诊断 | 诊断 / status-only | 不改写输入、权限或工具结果 |
| `tool.execute.after` | 否，P0 只诊断 | 诊断 / status-only | 不伪造工具结果或审计成功 |
| event subscription / SSE | 受限进入 | 事件清单 | 只能订阅公开事件清单的子集 |
| TUI/GUI 界面贡献 | 否，除非已有真实入口消费方和目标入口形态 | 声明式入口形态接口 | P0-B 返回 unsupported/status-only；不得暴露界面实现、渲染句柄或跨入口主题键 |
| shell/env helper | 否 | 诊断 / 未来受控工具请求候选 | 默认 unsupported，不开放无约束 shell/env |
| client/server facade | 否 | 不进入稳定面 | 不暴露 OpenCode client/server facade 给插件或产品入口 |

## 3. 必须拒绝的接口扩张

以下能力不得作为当前 PR 或 P0 稳定接口：

- 多生态通用运行时接口，包括 Claude Code-compatible、Codex-compatible 的稳定 runtime ABI。
- 完整 UI 插槽、路由、键位、对话框、提示、主题矩阵，或不区分 TUI/GUI 的全入口界面接口。
- 跨入口复用原始主题键、键位标识、CSS 变量或渲染状态。
- 任意 provider/model/config 转换。
- 可写 before/after tool hook。
- 无约束 JS/TS runtime、localhost server、shell/env helper。
- 插件直接覆写内置能力。
- 插件直接写权限、审计、工具结果、会话状态或界面状态。
- 以 OpenCode 原始配置或 OpenCode CLI 可用性作为 BitFun 权威状态。

如确需进入后续阶段，必须重新满足：真实产品场景、当前消费方、安全评审、测试目标、降级语义和退场条件。

## 4. 当前代码暴露面复核

| 区域 | 当前判断 | 后续处理 |
|---|---|---|
| `runtime-ports` plugin contract | 已有主机 ABI、读模型、候选效果、权限提示、诊断和隔离类型；公开符号较多但已受脚本预算约束 | 不继续新增泛描述符；公开符号必须声明接口切面、消费方和验证目标 |
| `plugin-runtime-host` | 已有受控 host 边界、deadline、幂等、隔离和 restart 清理路径 | 继续保持窄方法集；来源发现和激活属于 P0-C |
| `opencode-adapter` | 当前提供 projection-only 来源发现适配器，只做配置导入、来源发现和诊断读模型 | 不得实现 `PluginRuntimeClient`，不得声明可执行可用性 |
| `events` | 已有产品事件清单 | 需要在真实插件事件消费前定义可订阅子集，不新增插件专用事件模型 |
| `tool-contracts` | 已有动态工具提供方和工具快照 | custom tool 映射必须复用它，不新增插件专用工具 ABI |

## 5. PR 审查问题

每个 OpenCode 相关 PR 必须回答：

1. 该变更属于哪个接口切面？
2. 是否可以复用工具 ABI、事件清单或权限控制面？
3. 是否有当前消费方，还是仅为未来兼容预留？
4. 是否新增了 OpenCode 专用产品入口或稳定 DTO？
5. 不支持能力是否返回类型化 unsupported / 诊断，而不是新增空接口？
6. 是否要求用户安装 OpenCode CLI？
7. 是否让插件写入最终权限、审计、工具结果或内核状态？
8. 涉及界面或主题时，是否声明目标入口形态、语义 token、宿主映射、冲突处理和 unsupported 行为？

无法明确回答的问题不应进入实现 PR。
