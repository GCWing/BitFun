# BitFun 产品架构演进计划

本文把现有架构债务整理为可独立验收的工作流。稳定边界见
[产品运行时架构](../architecture/product-architecture.md)，专项细节见
[Core 迁移](core-decomposition-plan.md)、[CLI/TUI](../architecture/cli-product-line-design.md)、
[HarmonyOS PC 平台规约](../architecture/platform-portability-design.md)和
[OpenCode 兼容](opencode-extension-compatibility-plan.md)。专项文档不能用自己的阶段编号扩大本计划范围。

本轮对照的上游基线为 `ecad4f843`（2026-07-16）；本文所在提交记录
本轮实现事实。后续事实变化必须随代码显式更新，只有代码、入口消费和对应验证同时成立的项目才标记为完成。

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
| 公开面 | `bitfun-core` 仍有迁移期 re-export；CLI 只完成部分 Runtime SDK 接入 | 按入口逐项迁移，不做全仓逐 symbol 台账或批量删除 |
| CLI/TUI | `ShortcutsConfig` 已加载但真实按键分发仍硬编码；Slash、Palette、帮助和执行不是同一来源 | 先统一宿主 action 声明和键位解析，不重写 renderer |
| OpenCode | 只有来源确认和静态工具名预览，没有 JS/TS `execute` 或真实工具注册 | 先做一个无外部依赖、遵循官方公开契约的 standalone custom tool 端到端样例 |
| HarmonyOS PC | 未来平台目标，当前未实现 | 目标、问题、风险和旧设计闭环见平台规约；具体工作后续分别立项 |
| 入口迁移 | CLI 已消费 Runtime Parts，但 CLI/ACP/Desktop 仍直接依赖 `bitfun-core/product-full` | 保持单一 owner，按 CLI → ACP → Desktop 的独立行为等价切片推进 |

## 3. 工作流一：边界与依赖可信

交付：

- Cargo metadata 实际解析图检查已覆盖 workspace、独立 manifest，以及 normal、build、dev 依赖及 optional/target 变体；
  未知层级或新增反向依赖直接失败。
- relay 的 room/device 状态、account/sync 存储、asset store 与 HTTP/WebSocket router 已归属
  `services/relay-service`，standalone relay app 和嵌入式入口共同消费。
  standalone 的 TCP bind、静态 fallback 和进程生命周期留在 app；embedded 的对应宿主逻辑暂留 assembly 兼容路径，
  其迁移是独立后续工作，不构成 HarmonyOS 支持。
- 把 contracts/Product Domain 中的环境、路径或进程探测迁到 service。现有 Agent Runtime helper 只处理多生态
  Skill 根，不是 custom tool resolver，应留在 Skill owner；`.opencode/tools/` 发现由 OpenCode adapter 新增并由
  当前静态预览与后续执行共同消费。旧路径删除前保持生产行为等价。
- 审核新增或变更的公开 DTO/trait/re-export：记录 owner、当前调用方、兼容影响、验证和退场条件即可。不要建立
  与实现脱节的全仓术语分类或逐符号流程系统。

退出条件：

- `assembly -> apps` 反向边消失，standalone/embedded relay 共用同一已测试 router；（共享 owner 与 Cargo 方向已满足，
  embedded 宿主逻辑归位待后续）
- 边界检查能命中 normal/build/dev 依赖及 optional/target 反向 fixture；（已满足）
- contracts 和 Agent Runtime 不再新增环境或生态来源探测；
- 本工作流没有新增无调用方端口、空 registry 或第二个 Runtime owner。

## 4. 工作流二：CLI action 与快捷键一致

用户结果：持久化快捷键真正生效，Slash、命令面板、帮助、快捷键展示和执行不会互相漂移。

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

## 6. 工作流四：OpenCode 从一个真实工具开始

执行顺序由[OpenCode 兼容计划](opencode-extension-compatibility-plan.md)定义：

1. 固定稳定版本、官方 custom tool 文档/源码和当前静态预览事实，并增加一个无外部依赖的契约样例；
2. 直接发现官方 `.opencode/tools/` 来源，真实加载该样例的 `execute`，接入现有 Tool Runtime，并验证参数、结果、
   ToolContext 身份/路径字段、`abort` 取消、期限、异常和诊断；`metadata`/`ask` 在定义现有 owner 映射前明确不支持；
3. 官方 import 型 tool 或真实 package plugin 首次受阻时，才增加样例需要的依赖解析、loader 和最小 client；
4. Hook 和 TUI contribution 只按真实阻塞样例逐项加入。command/slash/key 复用工作流二的 action registry；toast
   必须等待 CLI 拥有类型化状态/通知 owner，不能借用 GUI 本地服务；原始 OpenTUI/Solid renderer 保持不支持。

工具实际加载并取得有效定义和 `execute` 后才能显示为可用。静态名称、可解析模块或进程启动成功都不等于工具
可调用。Remote 和 HarmonyOS PC 原生 CLI/TUI 未通过同一冻结样例前必须明确不支持，不能借 Desktop 代执行；
HarmonyOS 手机 Remote App 不在该平台执行范围内。

## 7. 工作流五：入口逐项迁移

- CLI：只迁移 Runtime SDK 已有稳定调用方的 session/turn/cancel 等路径；恢复视图、消息、分支、用量和工具确认
  在补齐端口与行为测试前继续由现有单一兼容路径转发。
- ACP：CLI 行为稳定后单独迁移会话、权限和事件投影；ACP stdio 生命周期留在接口入口。
- Desktop：按服务簇迁移，Tauri、窗口和 app-local 资源留在 Desktop。
- SDK/Server/Remote：只有真实独立调用方出现后才增加；枚举、空计划或测试替身不构成发布能力。

每个入口都必须证明生产行为、错误、取消和恢复等价后再删除旧路径。迁移期间不能在新旧路径同时写同一状态，
也不能让 Runtime SDK 吸收 CLI keymap、GUI layout、ACP 协议生命周期或 OpenCode 原始类型。

## 8. 依赖与并行关系

| 工作 | 必须等待 | 可以并行 |
|---|---|---|
| Relay 共享 owner / 反向边修复 | 已完成；embedded 宿主归位待后续 | OpenCode fixture |
| CLI action/快捷键 | 当前 CLI 行为和配置 fixture | OpenCode standalone tool、入口 API 迁移 |
| OpenCode standalone tool | OpenCode adapter 内的单一 source resolver、冻结版本/样例 | CLI action |
| OpenCode package/Hook/TUI | 前一切片稳定且有真实阻塞样例；TUI action 另等 action registry | 入口迁移 |
| ACP/Desktop 迁移 | 前一入口行为等价 | OpenCode 深兼容 |

这些依赖表示开始条件，不要求放在同一个 PR，也不形成统一大版本。

## 9. 验证

| 范围 | 最小证据 |
|---|---|
| 文档与仓库边界 | `pnpm run check:repo-hygiene`、边界脚本测试、`git diff --check`、本地链接/锚点检查 |
| Cargo 方向 | metadata fixture 覆盖各 dependency kind；已知债务只能减少 |
| Relay | standalone/embedded 启动、路由、关闭和错误等价 |
| CLI action | 无配置/旧配置/冲突配置、真实输入 dispatch、Help/Palette/Slash 一致、终端恢复 |
| OpenCode tool | 冻结无外部依赖契约样例的 load/execute/context/cancel/timeout/error 端到端；静态预览不会进入工具集合 |
| HarmonyOS PC | 本计划只检查平台规约没有被实现文档提前展开；各专题启动后独立定义验证。HAP、`hdc shell`、移动 Remote App 与远端代执行不替代 |
| 入口迁移 | 单入口生产消费、行为等价、旧转发删除和 focused test |

## 10. 暂停条件和延期

出现以下情况时停止扩大当前切片：

- 新增无当前调用方的 trait/DTO/registry，或同一事实出现第二个写 owner；
- 为平台或生态建立巨型总接口、服务定位器或新的 Agent/Tool Runtime；
- 只有静态解析或编译成功，却把能力标记为产品可用；
- HarmonyOS PC 用户发行或真实终端证据失败后，改用 HAP、`hdc shell`、移动端或 Desktop/Remote 代执行仍声称本地支持；
- 为追平竞品数量同时加入全量配置、Hook、renderer、Server 或权限系统；
- 一次迁移要求重写完整 CLI、Desktop 或 Core，无法独立验收。

明确延期：新权限语言和应用沙箱、全量 OpenCode config/Hook/TUI renderer/Server/Remote plugin、Codex/Claude 插件
ABI、HarmonyOS PC 具体实现、PC GUI、移动端本地适配，以及 Vim、语音、分享和协作等非核心 TUI 深度功能。
