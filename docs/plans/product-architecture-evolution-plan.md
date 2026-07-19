# BitFun 产品架构演进计划

本文把现有架构债务整理为可独立验收的工作流。稳定边界见
[产品运行时架构](../architecture/product-architecture.md)，专项细节见
[Core 迁移](core-decomposition-plan.md)、[CLI/TUI](../architecture/cli-product-line-design.md)、
[HarmonyOS PC 平台规约](../architecture/platform-portability-design.md)和
[OpenCode 兼容](opencode-extension-compatibility-plan.md)；能力 Provider、SDK 和外部宿主双向集成边界见
[能力装配与宿主集成](../architecture/extensions/capability-runtime-integration-design.md)。专项文档不能用自己的阶段编号扩大本计划范围。

本文所在提交记录本轮实现事实。后续事实变化必须随代码显式更新，只有代码、入口消费和对应验证同时成立的项目才标记为完成；
不在长期计划中固定易失效的上游提交号。

## 1. 裁决原则

1. 每项工作必须有当前问题、归属模块、真实调用方、最小结果和验证方式。
2. 优先修复已经存在的依赖反转、重复入口和不可用配置，再增加新能力。
3. 新公开接口必须由当前调用方需要；不能为了平台矩阵、竞品数量或未来兼容提前建立。
4. 现有行为迁移必须先证明等价，再删除旧路径；DTO 或 trait 移动不等于 owner 已迁移。
5. GUI、TUI、ACP、Server 和 SDK 共享运行时事实，不共享渲染、键位、协议生命周期或平台资源。
6. 竞品只用于验证用户语义和边界，不用于复制内部结构、命令数量或未公开实现。
7. 一个 PR 只完成一条可观察纵向路径；不同时重写 Runtime、TUI、插件协议、平台层和权限系统。

安全、凭据、组织策略和强隔离不在本轮新增。第三方脚本的进程隔离只能提供故障隔离；没有 OS/container 资源限制
时，不能宣称已限制其文件、网络、子进程、CPU 或内存副作用。

## 2. 已核实基线

| 范围 | 当前事实 | 近期结论 |
|---|---|---|
| 编译依赖 | `assembly/core -> apps/relay-server` 已移除；通用检查覆盖 normal/build/dev 依赖及 optional/target 变体 | 后续反向依赖和未知 crate 层级直接失败 |
| 公开面 | `bitfun-core` 仍有迁移期 re-export；CLI 主会话客户端已仅消费 Runtime SDK，其他产品入口仍保留兼容路径 | 按入口逐项迁移，不做全仓逐 symbol 台账或批量删除 |
| CLI/TUI | 宿主 `ACTION_SPECS` 已统一 Slash、Palette、Help、Keymap 与 dispatch；启动页及活动 turn 的 Linux PTY / Windows ConPTY 行为由进程级契约保护 | 保持现有 renderer 与交互规格，只按真实故障样例补可靠性契约；macOS 活动 PTY 另行验收 |
| OpenCode | Prompt Command、受支持的单文件 JavaScript Tool 和 Subagent 安全子集已分别通过能力专属 provider 接入；受管 package plugin 仍只有静态预览 | 先收敛三条已交付路径的诊断、运行时提示和配置失败语义，再按真实阻塞样例评估下一能力切片 |
| HarmonyOS PC | 未来平台目标，当前未实现 | 目标、问题、风险和旧设计闭环见平台规约；具体工作后续分别立项 |
| 入口迁移 | CLI 已消费 Runtime Parts；Desktop 主交互消费由现有 owner 构造的窄口径 Runtime SDK 门面，完整 Desktop Runtime Parts 尚未组装；CLI/ACP/Desktop 仍按需保留 `bitfun-core/product-full` 兼容 owner | 保持单一 owner，按真实端口逐项迁移，不批量删除兼容门面或用桩服务提前声明能力 |

## 3. 工作流一：边界与依赖可信

交付：

- Cargo metadata 实际解析图检查已覆盖 workspace、独立 manifest，以及 normal、build、dev 依赖及 optional/target 变体；
  未知层级或新增反向依赖直接失败。
- relay 的 room/device 状态、account/sync 存储、asset store 与 HTTP/WebSocket router 已归属
  `services/relay-service`，standalone relay app 和嵌入式入口共同消费。
  standalone 的 TCP bind、静态 fallback 和进程生命周期留在 app；embedded 的对应宿主逻辑由窄
  `EmbeddedRelayHost` 端口迁至 Desktop，不构成 HarmonyOS 支持。
- 把 contracts/Product Domain 中的环境、路径或进程探测迁到 service。现有 Agent Runtime helper 只处理多生态
  Skill 根，不是 custom tool resolver，应留在 Skill owner；`.opencode/tools/` 发现由 OpenCode adapter 新增并由
  当前静态预览与后续执行共同消费。旧路径删除前保持生产行为等价。
- 审核新增或变更的公开 DTO/trait/re-export：记录 owner、当前调用方、兼容影响、验证和退场条件即可。不要建立
  与实现脱节的全仓术语分类或逐符号流程系统。

退出条件：

- `assembly -> apps` 反向边消失，standalone/embedded relay 共用同一已测试 router，embedded 宿主逻辑由 Desktop 持有；（已满足）
- 边界检查能命中 normal/build/dev 依赖及 optional/target 反向 fixture；（已满足）
- contracts 和 Agent Runtime 不再新增环境或生态来源探测；
- 本工作流没有新增无调用方端口、空 registry 或第二个 Runtime owner。

## 4. 工作流二：CLI action 与快捷键一致（已完成）

用户结果：持久化快捷键真正生效，Slash、命令面板、帮助、快捷键展示和执行不会互相漂移。

当前状态：CLI 宿主已建立统一 action registry，Slash、Palette、Help、Keymap 与 dispatch 共用同一组稳定条目；
显式旧快捷键、冲突配置、宿主安全 fallback 和帮助尺寸均有回归保护。后续终端可靠性工作不再重复设计 action 层。

交付：

- 在 CLI 宿主内建立一个 action registry。条目只包含稳定 action id、名称/别名、适用上下文、可用性、处理器、
  默认键位和来源；会话或工具状态仍由原 owner 管理。
- Slash、Palette、Help、Keymap 和 dispatch 从同一条目读取。Clap 子命令、flags、stdout 和 exit code 保持独立协议，
  但可以调用同一 controller。
- 默认键位以当前真实 dispatch 为兼容基线，不把 serde 补出的默认值解释为用户选择。只迁移配置文件中显式保存
  的旧值。
- 冲突结果必须稳定并可解释；退出、终端恢复和活动 turn 中断始终保留宿主 fallback。
- 只围绕 action 分发拆分 `chat.rs`，不同时改视觉布局、renderer 或所有输入能力。

退出条件：无配置、显式旧配置、冲突配置和真实按键输入均有测试；不存在绕过 registry 的第二套 Slash/Palette/
Help/dispatch 元数据；终端异常路径仍能恢复。

## 5. 工作流三：HarmonyOS PC 专题占位

本计划不设计或排期 HarmonyOS PC 实现。目标、问题、风险、旧设计闭环和禁止替代项统一见
[HarmonyOS PC 平台规约](../architecture/platform-portability-design.md)；具体工作后续分别立项，现有手机 Remote
App 保持不变。

## 6. 工作流四：OpenCode 纵向切片与稳定性收敛

执行顺序由[OpenCode 兼容计划](opencode-extension-compatibility-plan.md)定义。当前已经完成三个互相隔离的纵向切片：

1. Prompt Command 直接发现标准用户/项目来源，通过统一冲突决策后在交互式 TUI 中提交展开后的 prompt；
2. standalone Tool 只加载受支持的单文件 JavaScript 子集，经来源确认后接入现有 Tool Runtime；
3. Subagent 安全子集经模型、工具和同名冲突确认后接入现有 Subagent owner，仅支持 fresh single-run。

已实现路径已经收敛运行时依赖与配置失败：Node.js 在当前进程首次检查时不可用时，只提示修复、刷新和可继续使用其他功能；
当前宿主没有可靠的刷新前后证据，因此不主动建议或触发重启。读取 BitFun 模型配置失败时阻止外部 Subagent 激活并显示独立原因，不能伪装成
“请求模型不存在”。同名冲突的待选择和当前选择保持可见，TUI 通过通用 `/tools` 与 `/agents` 入口按能力和来源分组；
`/agents` 同时容纳主 Agent、Subagent 和外部来源管理，不再创建 `/subagents` 或 `external-*` 平行命令。随后只有官方 import 型 tool、
package plugin、Hook 或 TUI contribution 的真实样例证明当前 owner/契约不足时，才增加对应的最小切片。原始
OpenTUI/Solid renderer、完整 package runtime 和 Remote 执行仍保持不支持。

工具实际加载并取得有效定义和 `execute` 后才能显示为可用。静态名称、可解析模块或进程启动成功都不等于工具
可调用。Remote 和 HarmonyOS PC 原生 CLI/TUI 未通过同一冻结样例前必须明确不支持，不能借 Desktop 代执行；
HarmonyOS 手机 Remote App 不在该平台执行范围内。

## 7. 工作流五：入口逐项迁移

- CLI：主会话客户端的会话创建（包括 `exec --session-id` 和缺失后端会话通过独立固定 ID 方法按原 ID 重建）/列举/删除/恢复、
  类型化转录、本地分支、用量生成、轮次提交/取消与精确结算、活动会话模型更新和 TUI 工具确认/拒绝/用户问题回答
  已由真实入口消费 Runtime SDK；交互模式下的 Peer Host 也通过同一 SDK 提交/精确取消 turn、更新活动会话模型并处理工具
  确认/拒绝。远程分支保持不支持；TUI 用量卡片持久化、快照和 Peer Host/ACP 维护等产品操作继续由现有单一兼容路径转发。
- ACP：CLI 托管服务端的会话创建/列举、轮次提交/取消、活动会话模型/模式更新、工具确认/拒绝、用户问题回答和事件订阅
  已复用同一 SDK 语义；ACP stdio、连接、权限 RPC 与通知生命周期仍留在接口入口。
- Desktop：主界面轮次提交/取消、活动会话模型更新、工具确认/拒绝和用户问题回答已通过现有协调器与调度器端口构造的窄口径 SDK 门面；
  完整产品组装需等待真实必需服务与事件消费路径。会话 CRUD/恢复视图、模型目录/提供方配置、MCP、MiniApp、Cron、远程连接、
  Tauri 窗口和 app-local 资源仍留在原入口。
- SDK/Server/Remote：只有真实独立调用方出现后才增加；枚举、空计划或测试替身不构成发布能力。

每个入口都必须证明生产行为、错误、取消和恢复等价后再删除旧路径。迁移期间不能在新旧路径同时写同一状态，
也不能让 Runtime SDK 吸收 CLI keymap、GUI layout、ACP 协议生命周期或 OpenCode 原始类型。

## 8. 工作流六：能力对外复用从一个真实消费者开始

本工作流不与前五项绑定成一次性交付。启动条件是某项现有 BitFun 能力已经有清楚 owner、稳定调用路径，以及
一个具名仓库外试点消费者、具体用例、验收 owner 和冻结宿主版本；不能为了“未来 SDK”先创建全量 Memory、
Context、Workflow、Subagent 或 Scheduler 接口。

顺序：

1. 选择一个只读或副作用边界清楚的高价值用例，只暴露该用例需要的请求、结果、状态和错误。
2. 优先通过 MCP/Skill/sidecar 接入一个宿主；只有该用例确实需要生命周期拦截时，才增加一个版本锁定的 Host Hook/
   Plugin adapter。
3. 为该宿主冻结分发单元、注册作用域，以及 install/register、enable、disable、uninstall、升级和失败恢复语义；
   物理安装状态以宿主为准，BitFun 不伪造成功。
4. 证明身份映射、权限上限、取消、Generation、事件损失、成本归属和降级后，再评估第二个宿主或第二项能力。
5. 只有非 `bitfun-core` 嵌入方能在最小依赖下稳定运行，才冻结和发布 Agent Runtime SDK 子接口。

退出条件：外部消费者从注册/安装、启用、调用、停用/卸载到恢复端到端可用；宿主矩阵只把该切片标为
native/translated/degraded；未实现能力保持 unsupported/experimental；没有引入第二状态 owner、通用服务定位器、
跨宿主任意 payload 或空 registry。

## 9. 依赖与并行关系

| 工作 | 必须等待 | 可以并行 |
|---|---|---|
| Relay 共享 owner / 反向边修复 | 已完成；embedded 宿主已归位 Desktop | OpenCode fixture |
| CLI action/快捷键 | 当前 CLI 行为和配置 fixture | OpenCode 已实现切片收敛、入口 API 迁移 |
| OpenCode 已实现切片收敛 | Command/Tool/Subagent 三条生产路径和聚焦 fixture | CLI action、入口迁移 |
| OpenCode package/Hook/TUI | 前一切片稳定且有真实阻塞样例；TUI action 另等 action registry | 入口迁移 |
| Desktop 主交互迁移 | 已完成窄口径 SDK 门面接入；完整 Desktop profile 与剩余入口需分别证明服务可用和行为等价 | ACP 与其他非扩展架构工作 |
| 一个能力对外复用 | 现有能力 owner、具名试点/用例/验收 owner、冻结宿主版本和最小权限/取消语义 | OpenCode standalone tool、单入口迁移 |

这些依赖表示开始条件，不要求放在同一个 PR，也不形成统一大版本。

## 10. 验证

| 范围 | 最小证据 |
|---|---|
| 文档与仓库边界 | `pnpm run check:repo-hygiene`、边界脚本测试、`git diff --check`、本地链接/锚点检查 |
| Cargo 方向 | metadata fixture 覆盖各 dependency kind；已知债务只能减少 |
| Relay | standalone/embedded 启动、路由、关闭和错误等价 |
| CLI action | 无配置/旧配置/冲突配置、真实输入 dispatch、Help/Palette/Slash 一致、终端恢复 |
| OpenCode 扩展 | Command 展开与冲突、Tool 的 load/execute/cancel/timeout、Subagent 的配置/模型/工具/Generation 失败语义；静态预览不会进入可调用集合 |
| HarmonyOS PC | 本计划只检查平台规约没有被实现文档提前展开；各专题启动后独立定义验证。HAP、`hdc shell`、移动 Remote App 与远端代执行不替代 |
| 入口迁移 | 单入口生产消费、行为等价、旧转发删除和 focused test |
| 能力对外复用 | 一个外部消费者的注册/安装、启停、请求/结果、权限、取消、Generation、事件损失、宿主降级、卸载和恢复端到端验证 |

## 11. 暂停条件和延期

出现以下情况时停止扩大当前切片：

- 新增无当前调用方的 trait/DTO/registry，或同一事实出现第二个写 owner；
- 为平台或生态建立巨型总接口、服务定位器或新的 Agent/Tool Runtime；
- 只有静态解析或编译成功，却把能力标记为产品可用；
- HarmonyOS PC 用户发行或真实终端证据失败后，改用 HAP、`hdc shell`、移动端或 Desktop/Remote 代执行仍声称本地支持；
- 为追平竞品数量同时加入全量配置、Hook、renderer、Server 或权限系统；
- 一次迁移要求重写完整 CLI、Desktop 或 Core，无法独立验收。

明确延期：新权限语言和应用沙箱、全量 OpenCode config/Hook/TUI renderer/Server/Remote plugin、完整 Codex/Claude
插件 ABI、Trae 深层 Hook/SDK 适配、跨宿主会话迁移、HarmonyOS PC 具体实现、PC GUI、移动端本地适配，以及
Vim、语音、分享和协作等非核心 TUI 深度功能。单个真实外部消费切片不解除这些延期项。
