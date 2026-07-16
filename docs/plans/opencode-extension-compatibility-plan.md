# OpenCode 扩展兼容执行计划

本文只定义近期可执行顺序。完整能力差异保留在
[兼容矩阵](../architecture/extensions/opencode-extension-compatibility.md)，运行边界见
[插件运行时主机](../architecture/extensions/plugin-runtime-host-design.md)和
[OpenCode 插件适配](../architecture/extensions/opencode-plugin-runtime-adapter-design.md)。兼容矩阵是审计库存，不是默认
路线图。

当前基线只有来源确认和静态 custom tool 名称预览：不执行 JS/TS，不注册真实工具，也不运行 Hook、Client 或 TUI
插件。任何计划项都不能被表述成当前能力。

## 1. 执行原则

1. 先完成一个遵循官方公开契约、但不依赖外部软件包的 standalone custom tool，再考虑依赖物化、package plugin、Hook 或 TUI contribution。
2. 只实现固定版本文档、源码和真实样例共同需要的语义；未知接口稳定失败，不伪造成功。
3. 复用现有 Tool Runtime、权限和事件 owner；界面反馈只进入目标宿主已经存在的状态入口，不建立插件专用工具调用状态机。
4. 外部类型停留在 adapter/worker 内；BitFun 只接收经过校验的工具定义、调用结果、变换或诊断。
5. 脚本执行实现不进入通用平台抽象。若首个样例确实需要新端口，它只表达该调用方需要的 load/invoke/cancel/
   dispose 和诊断，不暴露 Bun、worker 数量、IPC 或进程句柄。
6. Desktop、Remote 和 HarmonyOS PC 原生 CLI/TUI 分别资格验证；一个平台可用不能推导其他平台可用。HarmonyOS
   手机 Remote App 不在本计划的平台执行范围内。

第三方脚本可以直接访问文件、网络、环境和子进程。独立进程、期限、取消和有界队列能限制故障传播，但没有
OS/container 资源限制时不能称为沙箱，也不能保证阻止 CPU、内存或进程耗尽。来源身份、执行域和现有策略必须在
module import 前确定；严格策略无法落实时停用该 target。

## 2. 阶段总览

| 阶段 | 可观察结果 | 明确不包含 |
|---|---|---|
| OC-E0 基线 | CLI 准确显示“静态预览，未执行”，固定稳定版本、官方契约和无外部依赖样例 | JS/TS 执行、完整配置导入 |
| OC-E1 standalone tool | `.opencode/tools/` 中一个无外部依赖的契约样例可被真实调用，支持身份/路径字段和 `abort` | `metadata`/`ask`、官方 import 型样例、package plugin、npm 依赖安装、Hook、TUI |
| OC-E2 package plugin | 一个代表性真实插件无需改包即可工作 | 全部历史 loader、完整 Client/Server、插件市场 |
| OC-E3 样例驱动扩展 | 一个真实 Hook 或最小 TUI contribution 闭环 | 全量 Hook、原始 renderer、完整 Server、Remote plugin |

OC-E0 和 OC-E1 是近期范围。OC-E2/OC-E3 只有在前一阶段稳定并选定真实阻塞样例后才启动。

## 3. OC-E0：让基线可信

交付：

- 固定 OpenCode 稳定版本、custom tool 官方文档和对应源码/测试；记录本次使用的 commit。
- 冻结一个遵循官方 export/ToolContext 契约、但不引用外部软件包的最小 `.opencode/tools/*.ts|js` 样例，并记录它与官方 import 型示例的差异。
- 记录 BitFun 当前 source resolver、静态预览、CLI 状态和 Tool Runtime 的真实代码路径。
- 静态名称只能显示为预览，不得进入模型可调用工具集合。
- 官方文档只承诺复数 `tools/` 目录。单数 `tool/` 若由冻结源码和测试证明仍兼容，可作为版本化兼容输入；不能
  写成长期公开保证，也不能复制第二套 resolver。

退出条件：版本和样例可复现；当前状态不包含“ready/available/兼容运行时”等误导文案；产品状态能明确区分来源
可见与代码已执行。

## 4. OC-E1：standalone tool 纵向闭环

启动条件：OpenCode 路径发现已经归到唯一 adapter/source resolver，且 OC-E0 基线通过。

最小路径：

```text
OpenCode source resolver
  -> script loader
  -> OpenCode adapter
  -> Plugin Runtime Host
  -> existing Tool Runtime
  -> model/CLI invocation
```

调用返回时沿同一路径回到 Tool Runtime；Host/adapter 不各自维护一套调用或生命周期状态。

交付：

- 从 workspace/user 官方目录发现 tool，不要求 `bitfun.plugin.json`、复制目录或安装 OpenCode CLI。
- 真实加载 module exports；只有取得有效 description/args/`execute` 的工具才注册。
- 保留冻结样例的参数校验和执行行为；不能只把静态扫描得到的名称或 schema 当作执行定义。官方示例使用的 `@opencode-ai/plugin` 解析和依赖等待不属于本阶段，未支持前必须明确报告兼容差异。
- 冻结版源码的 `ToolContext` 有 `agent`、`sessionID`、`messageID`、`directory`、`worktree`、`abort`、`metadata`
  和 `ask` 八项。OC-E1 必须提供前五项并把 `abort` 接到 Host 取消/期限；首个样例不调用 `metadata`/`ask`，两者在
  本阶段返回明确 `unsupported`，因此 OC-E1 只能声明该契约子集可用。需要这两项的真实样例出现后，必须先定义其
  到现有事件/权限 owner 的关联、取消和审计行为，再扩大兼容范围。上述字段只留在版本化 adapter，不提升为 BitFun
  跨生态稳定接口。
- 覆盖正常结果、参数错误、throw、超时、取消、迟到响应、进程退出、结果过大和不可序列化结果。
- Tool Runtime 继续负责排队、权限、执行状态和结果；worker/Host 只提供真实执行和诊断。
- CLI 显示来源、可用/不可用原因、执行错误和恢复建议；module 可解析或进程启动成功不等于工具可用。

退出条件：无外部依赖且不调用 `metadata`/`ask` 的契约样例在 Desktop 完成发现、真实调用、取消和失败恢复；作者无需
改源码或重打包；产品状态明确列出 context 子集；失败不阻塞 TUI
输入、终端恢复和无关工具；静态预览与实际 exports 冲突时以实际加载结果为准。

Remote 和 HarmonyOS PC 原生 CLI/TUI 在各自通过同一冻结样例前保持明确不支持，不能调用 Desktop worker 代执行
工作区代码；手机 Remote App 不作为 HarmonyOS PC 的通过证据。

## 5. OC-E2：一个真实 package plugin

启动前必须选定一个 standalone tool 无法覆盖的真实插件，并在评审中列出：

- 它需要 package/server loader 的原因；
- 实际调用的 PluginInput、Client、`$` 或 `serverUrl` 方法；
- 依赖安装、版本身份、停用和恢复需求；
- 不支持它时的用户影响。

只实现该样例需要的来源、入口解析、依赖物化、最小 Client 和生命周期。未知方法必须稳定失败，写操作不得伪造
成功。停用、更新失败或进程崩溃后，旧 contribution 只有在来源版本和当前策略仍可验证时才能继续；否则撤下并
说明不可恢复。

不在本阶段预建所有 npm/Arborist 选项、历史入口 fallback、完整回环 Server、全局插件管理 UI 或完整 Client。

退出条件：代表性插件无需改包工作；standalone tool 路径没有回归；依赖或插件失败只影响对应 target；未选择的
插件形态仍保持未承诺。

## 6. OC-E3：按真实样例增加 Hook 或 TUI contribution

### Hook

每次只选择一个阻塞真实插件的稳定 Hook。先确定 BitFun 最终 owner、允许变换的字段、执行顺序、最终校验和失败
范围，再扩展 adapter/host。合法变换由 owner 提交；插件不能直接写会话、权限、工具结果或审计状态。

每个 Hook 独立验证正常、链式、非法结果、异常、超时、取消和 owner 终检。一个 Hook 完成不表示其他 Hook 或
“完整服务插件面”完成。

### TUI contribution

首批只考虑：

- command / slash alias；
- key binding 候选；
- toast 只在 CLI 已有类型化状态/通知 owner 后另行加入，不能复用 GUI 本地服务来假装跨宿主能力。

command/slash/key 进入 CLI action registry。键位冲突、退出/恢复 fallback、焦点和布局由
宿主决定。插件不能持有 Ratatui Frame 或终端句柄。Route/Dialog/Prompt/slot/theme/state/KV/client/event 继续留在
兼容矩阵中，只有真实样例阻塞时再单独立项；原始 `CliRenderer`、Solid/OpenTUI 组件树保持不支持。

退出条件：冻结样例可发现、启停并清理 contribution；冲突来源可见；异常不会造成输入锁死、空白页面或终端无法
恢复。

## 7. 验证与发布

| 证据 | E0 | E1 | E2 | E3 |
|---|---:|---:|---:|---:|
| 固定版本和样例 | 必需 | 必需 | 必需 | 必需 |
| resolver/adapter/Host focused test | 基线 | 必需 | 必需 | 必需 |
| Tool Runtime 端到端 | - | 必需 | 必需 | Hook 涉及时 |
| CLI 状态与诊断 | 必需 | 必需 | 必需 | 必需 |
| TUI 输入/恢复 | 文案 | 失败路径 | 失败路径 | TUI 项必需 |
| Remote/HarmonyOS PC 原生 CLI/TUI | 明确状态 | 分别资格验证 | 分别资格验证 | 分别资格验证 |

发布说明只列已通过的阶段、样例和平台。例如应写“Desktop 支持 OpenCode standalone custom tool 样例；package
plugin、Hook 与 TUI plugin 尚未支持”，不能笼统写“已兼容 OpenCode 插件”。

## 8. 暂停条件

出现以下情况时停止扩面：

- 为一个样例新建第二个 Tool Runtime、Agent Runtime、会话 owner 或通用生态 API；
- 新内部端口暴露 Bun/QuickJS、worker 数、IPC 或 OS 进程句柄；
- 只有静态解析，没有真实 `execute`，却把工具标记为可用；
- 为“以后可能需要”增加 Client、Hook 或 TUI 方法，无当前样例；
- 一个阶段同时要求全量配置、package manager、Hook、renderer 和权限系统；
- 平台只通过 cross-check，没有同一样例的运行证据。

延期项：完整配置兼容、所有 Hook、原始 OpenTUI renderer、完整 OpenCode Server/OpenAPI、Remote plugin、IDE/Web/
attach、GitHub/GitLab/Slack 连接器、实验接口，以及新的沙箱、凭据或组织策略设计。
