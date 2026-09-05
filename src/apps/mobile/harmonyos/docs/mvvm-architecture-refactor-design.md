# HarmonyOS 端 MVVM 架构重构设计

Date: 2026-08-06

Status: Implementation in progress; S0-S5 and S7 are complete, while S6 component decomposition and the wide-screen visual matrix remain pending

Scope: `src/apps/mobile/harmonyos/entry/src/main/ets`

Baseline: commit `6c35485bb`（窄屏 Local/Remote 统一完成后）

Reference: 华为官方文档
[MVVM模式（状态管理V2）](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides-V13/arkts-mvvm-v2-V13)、
[MVVM模式（V1）](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-mvvm)、
[状态管理（V1）](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-state-management-v1)

Related designs:

- [`adaptive-conversation-ui-redesign.md`](adaptive-conversation-ui-redesign.md)
- [`wide-conversation-navigation-design.md`](wide-conversation-navigation-design.md)
- [`responsive-file-preview-design.md`](responsive-file-preview-design.md)
- [`native-code-preview-implementation-design.md`](native-code-preview-implementation-design.md)

本文只负责**代码结构**，不改变任何用户可见行为。上述四篇设计继续负责路由合同、折痕几何、文件预览 placement 和会话 UI/UX；本文的每一个阶段都以"这些文档描述的行为在真机上完全不变"为验收前提。发生冲突时，以现有行为文档为准，重构方案让路。

---

## 0. 结论摘要

- **架构基准**：MVVM 是鸿蒙官方文档明确定义的模式，官方把它定位为**单模块内的文件组织方式**；整个应用的模块化官方推荐三层架构（products / features / commons）。本项目 `build-profile.json5` 只有一个 `entry` 模块，正落在 MVVM 覆盖的范围内——**MVVM 是本次重构正确且足够的框架，三层架构不在本次范围**。
- **好消息**：ViewModel 层已经是干净的。7 个 `*ViewModel` 共 1379 行，**没有任何一个 import `components/`**。MVVM 里最难守住的一条，这里已经守住了。
- **重构结果**：`services/` → `pages/`、`components` → `viewmodel`、`viewmodel` → `components` 当前均为零；运行时组合根已拆为 `AppRootRuntime` 与 `AppRootRuntimeComposition`，特性行为由四个 Controller 持有。
- **一条被更正的判断**：初版诊断把"10 个 `components/*` import `../state/`"列为分层违规，**这是错的**，详见 §3.4。
- **状态管理范式统一到 V2**：基线有 15 个 V1 struct（5114 行）与 19 个 V2 struct 混用；S5 已将这 15 个组件全部迁移到 V2。V2 是官方对新项目的推荐范式，也是官方 MVVM 示例的形式，详见 §2.8 与 §5 的 S5 阶段。
- **实施方式**：S0–S7 八个阶段，每个阶段独立可发布、可回滚，前三个阶段零行为变更。

---

## 1. 架构基准

### 1.1 官方 MVVM 的三条职责界定

引自华为官方文档：

- **model** —— 负责数据的获取和存储以及业务逻辑，**不与 view 关联**；
- **view** —— 负责界面展现和用户输入，**不与 model 关联**；
- **viewmodel** —— 作为连接二者的桥梁，负责将 model 数据转为 view 数据并管理界面状态。

官方 V2 示例的绑定形式是 `@ComponentV2` + `@Local` 持有 ViewModel 实例。

本文后续所有"违规"判定，都直接引用上面三句，不引入本文自创的架构偏好。

### 1.2 范围界定：MVVM vs 三层架构

官方对二者的分工是明确的：

> MVVM 的目录组织方式一般适用于**单个模块内**的文件组织；为了更好地适配复杂应用开发，建议采用**三层架构**对**整个应用**功能进行模块化。

| 层级 | 编译产物 | 依赖约束 |
| --- | --- | --- |
| products（产品定制层） | Entry HAP | 可依赖 features / commons，禁止横向调用 |
| features（基础特性层） | HAR / HSP | 可依赖 commons，避免反向依赖 products |
| commons（公共能力层） | HAR / HSP | 不可依赖上层 |

**本项目现状**：`build-profile.json5` 的 `modules` 只有 `entry` 一项，`compatibleSdkVersion 6.0.1(21)` / `targetSdkVersion 6.1.1(24)`。单模块 = MVVM 的适用范围。

**三层架构的引入时机**（记录，本次不做）：当需要为不同设备形态提供差异化入口（折叠屏 / 平板 / 车机各自的 Entry HAP），或 `services/` 需要被鸿蒙端之外复用时，才是把 `services/` 抽成 commons HAR、把会话/Remote 抽成 features HSP 的时机。在只有一个 entry 的现在做这件事，只增加构建复杂度，不带来收益。

### 1.3 ArkTS/ArkUI 层面必须遵守的既有教训

这些是本模块已经付出过代价的约束，重构中任何一步都不得违反：

1. **`@Builder` 的值参数不具备响应式**。只有按引用传入的单个对象参数才会驱动重渲染；builder 内部读 `this.<state>` 才是可靠的。拆分 builder 时，凡是原先从父 builder 传入的宽度、来源等标量，一律改为在子 builder 内部读状态。
2. **`NavPathStack` 不可观测**。任何存活于 `Navigation` 之外的界面（抽屉是典型）都不能靠它驱动刷新，必须消费 `AppShellState.activeRoute` 这个 `@Trace` 镜像。该镜像由 `AppShellViewModel.syncActiveRoute()` 统一维护，**新增导航路径必须经由 `AppShellViewModel`**。
3. **V1 / V2 混用现状**：`@Component/@State/@Prop` 与 `@ComponentV2/@Local/@Param/@Event` 并存。本次**全量迁移到 V2**，范式统一后 §1.3.1 和 §1.3.2 两条约束的心智负担也随之下降（V2 的观测边界比 V1 明确）。分布数据见 §2.8，实施见 §5 的 S5 阶段。

---

## 2. 现状测量

以下全部为实测值，非估算。

### 2.1 规模基线

| 目录 | 文件数 | 行数 |
| --- | --- | --- |
| `pages/components` | 39 | 14518 |
| `services`（含 `general-chat` 21 / 3296） | 51 | 7492 |
| `pages/state` | 21 | 5645 |
| `i18n` | — | 581 |
| `model` | — | 465 |
| `pages/navigation` | — | 110 |
| 测试 `entry/src/test` | 8 | 6612 |

### 2.2 `pages/state/` 的真实构成（一个目录装了三层）

| 类别 | 文件 | 行数 |
| --- | --- | --- |
| ViewModel | `AppShellViewModel` 98、`ConversationViewModel` 22、`GeneralChatConversationViewModel` 336、`RemoteActivityViewModel` 163、`RemoteConnectionViewModel` 353、`RemoteSessionViewModel` 236、`RemoteWorkspaceViewModel` 171 | 1379 |
| State（`@ObservedV2` 绑定对象） | `AppShellState` 58、`ConversationViewState` 120、`FilePreviewState` 107、`GeneralChatPageState` 200、`RemoteCreateSessionState` 113、`RemotePageState` 373 | 971 |
| Policy（纯逻辑，零 `@Trace`） | `ConversationLayoutPolicy` 156、`FilePreviewPlacementPolicy` 185、`ConversationModelPresentationPolicy` 82、`ConversationSessionFilterPolicy` 51、`SessionActionPolicy` 31 | 505 |
| God Facade | `AppRootRuntime` | 2608 |
| 其他 | `ConversationIntentDispatcher`、`FilePreviewTarget` 等 | 约 182 |

### 2.3 两个引力井的内部构成

**`AppRootPresentation.ets`（1449 行）**——可分离，各段落关注点互不相干：

| 段落 | 行数 | 性质 |
| --- | --- | --- |
| 7 个 action DTO 定义（L62–270） | 209 | 属于 model 定义，不该在 view 文件里 |
| Remote UI builders | 305 | 一个独立特性面 |
| Remote 辅助方法 | 143 | 同上 |
| 宽屏几何计算 | 179 | 纯计算，可脱离 UI，当前零单测覆盖 |
| 宽屏 builders | 275 | 一个独立布局面 |

共 25 个 `@Builder`、约 50 个私有方法、21 个 `@Local`（其中 13 个属于宽屏几何、8 个属于 Remote 过滤/元数据）。两组 `@Local` 混在同一 struct 内，意味着改宽屏分栏宽度会连带触发 Remote 过滤区重算。

**`AppRootRuntime.ets`（2608 行）**——性质不同，是"所有特性的门面开在同一个类上"：

- 183 个方法级条目，约 101 个 public，其中 **45 个是一行转发**；
- 75 个 import；
- 字段初始化块从 L140 延伸到 L761（621 行）；
- 单个方法最长 `selectCloudAccountDevice` 91 行。

### 2.4 接线代码

12 个 `*Hooks` / `*Actions` 类：定义 412 行，在 `AppRootRuntime` 中的构造点 235 行，合计约 **650 行纯接线**。

其中 7 个定义在 `AppRootPresentation.ets` 内（L62–270，209 行）。构造点规模：`AppRootPresentationActions` 96 行、`RemoteSessionViewModelHooks` 46 行、`ConversationIntentDispatcherHooks` 39 行、两个 Hooks 各 23 行、一个 8 行。

全部为**位置参数构造**：

```ts
new AppRootPresentationActions(a, b, c, d, /* …共 96 行实参 */)
```

代价不只是行数——新增一个回调要同步改三处（DTO 定义、构造点、消费点），且位置参数在 ArkTS 里没有编译期的名字保护：两个相邻的同签名回调若被调换顺序，编译通过、运行时行为错乱。这是本模块唯一一类"改对了也无法在编译期确认"的修改。

### 2.5 会话状态的重复

`GeneralChatPageState`（200）与 `RemotePageState`（373）有**约 15 个字段同名同义**。为了让上层统一消费，又长出两层扇入扇出：

- `services/AppRootRouteState.ets`（88 行）——存在的唯一理由是在两者之间搬数据；
- `ConversationViewState.project(route, remote, general, …)`——再做一遍同样的归约；
- 分散各处的 `compact` 布尔与 `if (source === General)` 分支。

后果：每新增一项会话能力（附件、引用、重发……），要在两个 State 各写一次，再在两个投影层各接一次。

### 2.6 组件层

内联 glyph / icon builder 共 **538 行**，分布在 10 个文件：`AppSidebar` 179、`ConnectView` 99、`ToolStatusList` 91、`ConversationView` 61、`ChatMessageBubble` 35、`SessionActionSurface` 19、`CreateSessionSheet` 18，`ComposerBar` / `RemoteCreateSessionView` / `ChatTimeline` 各 12。

第二梯队大结构体：`ToolStatusList` 1442 行 / 16 builders、`ConnectView` 1344 / 24、`ChatMessageBubble` 1246 / 18、`AppSidebar` 908 / 29。

### 2.7 现有安全网

`entry/src/test/` 共 6612 行 hypium 用例：

| 文件 | 行数 |
| --- | --- |
| `RemoteControllersUnit` | 2177 |
| `TransportAndGeneralChatUnit` | 1255 |
| `LocalTestFixtures` | 1078 |
| `ConversationStateUnit` | 1057 |
| `LifecycleUnit` | 748 |
| `AppRootLifecycleUnit` | 129 |
| `ArchitectureUnit` | 95 |
| `AppRootRuntimeStartupUnit` | 51 |

本地运行方式（已实测通过，BUILD SUCCESSFUL 11s，报告落在 `entry/.test/default/outputs/test/reports/`）：

```bash
source scripts/ohos-env.sh
"$HVIGORW" --mode module -p module=entry@default -p ohos.test.type=LocalTest test --no-daemon
```

注：现有 `ArchitectureUnit`（95 行）测的是**行为**（生成号失效、时间线归约、路由栈不变量），不是分层。分层目前无任何自动化约束。

### 2.8 V1 / V2 范式分布（重构前基线）

**结构体**：V1（`@Component`）15 个，共 **5114 行**；V2（`@ComponentV2`）19 个。

**装饰器用量**：

| V1 | 次数 | V2 | 次数 |
| --- | --- | --- | --- |
| `@Prop` | 79 | `@Param` | 155 |
| `@State` | 53 | `@Local` | 62 |
| `@BuilderParam` | 7 | `@Event` | 104 |
| `@Watch` | 4 | `@Trace` | 99 |
| `@Link` | 2 | `@ObservedV2` | 5 |
| `@Observed` / `@ObjectLink` / `@Provide` / `@Consume` / `@StorageLink` / `@StorageProp` | 0 | `@Monitor` | 4 |

（`@BuilderParam` 在 V1 与 V2 中均受支持，不属于迁移面。）

**V1 文件清单与迁移面**：

| 文件 | 行数 | `@State` | `@Prop` | `@Link` | `@Watch` |
| --- | --- | --- | --- | --- | --- |
| `ConnectView.ets` | 1344 | 12 | 16 | — | — |
| `AppSidebar.ets` | 908 | 7 | 11 | — | — |
| `RemoteControlSettingsSheet.ets` | 872 | 13 | 12 | — | 1 |
| `ModelServiceSettingsPanel.ets` | 662 | 10 | 5 | — | — |
| `SettingsSheet.ets` | 297 | 4 | 8 | — | — |
| `CreateSessionSheet.ets` | 226 | — | 4 | 2 | — |
| `MarkdownContent.ets` | 199 | — | 1 | — | — |
| `OpenBitFunAccountLoginPage.ets` | 146 | 5 | — | — | — |
| `StreamingMarkdownContent.ets` | 142 | 1 | 3 | — | 3 |
| `FileReferenceCard.ets` | 85 | — | 8 | — | — |
| `ThinkingBlock.ets` | 67 | 1 | 5 | — | — |
| `ChatStatusBar.ets` | 60 | — | 4 | — | — |
| `AppRoot.ets` | 48 | — | — | — | — |
| `ConversationSourceSwitcher.ets` | 40 | — | 1 | — | — |
| `DefaultAccountAvatar.ets` | 18 | — | 1 | — | — |

**集中度**：前 4 个文件占 3786 行（V1 总量的 74%）、86 个 V1 状态装饰器（占 65%）。其中 `ConnectView` 与 `AppSidebar` 同时也是 S6 拆分的目标，可就近编排。

**当前是否已有跨范式错误用法**：已逐文件核查，**没有**。5 个 `@ObservedV2` 类（`AppShellState`、`RemotePageState`、`GeneralChatPageState`、`RemoteCreateSessionState`、`FilePreviewState`）**没有任何一处被 V1 的 `@State` / `@Prop` / `@Link` 持有**——官方不支持 `@ObservedV2` 对象走 V1 观测机制，这条目前没有被踩到。

所以全量迁移 V2 **不是在修复既有 bug，而是在消除一类风险**：只要 V1 struct 还在，任何一次后续改动都可能把某个 `@ObservedV2` 对象传进 V1 的 `@Prop`，届时得到的是"编译通过、界面不刷新"——与本模块此前踩过的抽屉不刷新（§1.3.2）完全同型、且同样难以定位的故障。

---

## 3. 诊断

### 3.1 符合官方定义的部分

- **ViewModel 层是干净的**：7 个 VM 共 1379 行，零 import `components/`。ViewModel 完全不知道 UI 存在。
- **Policy 层是纯的**：5 个 Policy 共 505 行，零 `@Trace` / 零 `@ObservedV2`，可直接单测。
- **已有一处标准 MVVM 三件套**：`ConversationViewState`（投影，120）→ `ConversationViewHost`（哑视图，91）→ `ConversationIntent` / `ConversationIntentDispatcher`（意图，120）。**这是本次重构要推广的形状，不需要发明新范式。**

### 3.2 硬违规（按 §1.1 官方定义判定）

| 官方职责 | 违规 | 证据 |
| --- | --- | --- |
| model **不与 view 关联** | `services/` → `pages/` 反向依赖 | `AppRootRouteState`、`FileTargetResolver`、`RemoteFilePreviewController`、`MessageFileReferenceProjector` 共 4 个文件 import `../pages/` |
| view **不与 model 关联** | view 文件持有 model 定义，导致真实模块环 | `AppRootPresentation.ets` L62–270 定义 209 行 action DTO → `AppRootRuntime` 反向 import `AppRootPresentation` |
| viewmodel 是**桥梁** | `AppRootRuntime` 不是桥梁，是 God Facade | 2608 行 / 101 public / 45 一行转发 / 621 行字段初始化块；所有 view 绑到同一个巨型对象，而非各自绑到所属特性的 VM |

### 3.3 结构性问题（不算违规，但是主要成本来源）

1. **接线子系统化**（§2.4，约 650 行）——位置参数构造带来无编译期保护的修改风险。
2. **会话状态双份实现**（§2.5）——每项能力写四遍。
3. **目录命名说谎**（§2.2）——`pages/state/` 一个目录装了 ViewModel / State / Policy / God Facade 四类东西，"这个文件属于哪一层"无法从路径判断，也导致分层断言写不出来。
4. **组件层关注点混合**（§2.6）——538 行内联图标 + 四个千行级结构体。

### 3.4 更正：一条被推翻的初版判断

初版诊断把 **"10 个 `components/*` import `../state/`" 列为分层被打穿。这个判断是错的**，此处保留记录以免后续重复犯错。

逐文件查证结果——这 10 个文件 import 的**全部是 State 类与 Policy 类，没有一个 import `*ViewModel`**：

```
AppShell.ets              → AppShellState
AppSidebar.ets            → SessionActionPolicy
ConversationViewHost.ets  → ConversationViewState
ComposerBar.ets           → ConversationModelPresentationPolicy
FilePreviewSurface.ets    → FilePreviewState
ConversationIntent.ets    → FilePreviewTarget
ConversationViewSettings  → ConversationSessionFilterPolicy
RemoteSessionList.ets     → SessionActionPolicy, ConversationSessionFilterPolicy
RemoteCreateSessionView   → RemoteCreateSessionState
AppRootPresentation.ets   → AppShellState 等 6 个 State/Policy
```

View 持有 `@ObservedV2` 状态对象**正是 ArkUI V2 官方推荐的绑定方式**，不是违规。真正的问题是 §3.3 第 3 条：目录名叫 `state`，内容却是四层，让合规的 import 看起来像违规。

**因此 S6 的目标已相应修正**：从"切断 `components → state` 的 import"改为"消除内联图标与多关注点混合"。

---

## 4. 目标结构

依赖单向向下，`pages/state/` 按真实层次拆开：

```
pages/
  ├─ AppRoot.ets            @Entry，组合根
  ├─ actions/               所有 Actions/Hooks 接口定义（从 view 文件搬出，环即断）
  ├─ viewmodel/             7 个 *ViewModel + 按特性拆出的 Controller
  ├─ state/                 纯 @ObservedV2 绑定对象
  ├─ policy/                纯逻辑，无装饰器，全部可单测
  ├─ layout/                WideLayoutGeometry 等纯几何计算
  ├─ navigation/            AppRouteContract（叶子）
  └─ components/            哑视图 + Glyphs 图标库
services/                   model 层：领域与传输，禁止 import ../pages
model/ i18n/                叶子
```

**五条硬约束**（S0 写入 `AGENTS.md` 并以"已知清单"模式开始由 `ArchitectureUnit` 拦截新增违规；第 5 条在 S5 完成后转为强制，第 1–3 条在 S7 完成后转为强制）：

1. `services/**` 不得 import `../pages/`；
2. `pages/components/**` 不得 import `pages/viewmodel/`（import `state/` `policy/` 合法）；
3. 不存在任何模块环，`viewmodel → components` 方向禁止；
4. Actions/Hooks 一律 `interface` + 对象字面量，禁止位置参数构造；
5. **组件一律 `@ComponentV2`**，禁止新增 `@Component` / `@State` / `@Prop` / `@Link` / `@Watch`（`@BuilderParam` 不在此列，V2 亦支持）。

**每个特性面的标准形状**（推广 §3.1 已有的三件套）：

```
XxxViewState   投影：把 model 数据转成 view 数据
XxxHost        哑视图：只接 @Param 和回调
XxxIntent      意图：view 向上表达"用户想做什么"
XxxViewModel   桥梁：持有 state、消费 services、处理 intent
```

---

## 5. 分阶段方案

按"风险调整后收益"排序。S0–S2 零行为变更。每阶段独立可发布、可回滚。

### S0 · 立规则与护栏（0.5 天，零行为变更）

**做什么**

1. 把 §1.1 官方三条职责、§1.2 范围界定、§4 五条硬约束写入 `src/apps/mobile/harmonyos/AGENTS.md`；
2. 把 §2.7 的本地测试命令补进 `AGENTS.md`（目前未文档化）；
3. 扩展 `ArchitectureUnit.test.ets`，新增两组源文件扫描断言，均采用**"已知清单"模式**——断言"当前违规集合 == 登记清单"，从此新增违规立即失败，存量按阶段递减：
   - 分层断言：登记当前 5 处（`services → pages` 4 处 + `runtime → presentation` 1 处），S7 清零；
   - **范式断言**：登记当前 15 个 V1 文件（§2.8 清单），S5 清零。这一条从 S0 当天起就阻止新增 V1 组件，避免迁移期间边迁边长。

**为什么先做**：规则来自官方文档，不需要团队内部论证；两份清单让后续每阶段的进度可测，且"只减不增"是机器保证的。

**风险**：无。不触碰产物代码。

---

### S1 · 从 view 中取出 model 定义，断环 + 拆分引力井（1–2 天，零行为变更）

**做什么**

1. **7 个 action DTO（L62–270，209 行）→ `pages/actions/`**。单独这一步就消掉硬违规 ② 与循环依赖，建议独立成第一个 commit。
2. Remote builders + helpers（448 行）→ `pages/components/remote/RemoteSurfaceHost.ets`，带走 8 个 Remote `@Local`。
3. 宽屏几何（179 行）→ `pages/layout/WideLayoutGeometry.ets`，纯函数，**顺带补单测**（当前零覆盖）。宽屏 builders 带走 13 个几何 `@Local`。
4. `pages/state/` 按 §4 拆成 `viewmodel/` `state/` `policy/`——纯改目录与 import 路径，零逻辑改动，但让 S0 的断言写得出来。

目标：`AppRootPresentation.ets` 从 1449 行收敛到约 300 行的装配壳。

**实际结果（2026-08-07）**：7 组 action DTO 已迁入 `pages/actions/`；Remote、
窄屏路由、宽屏会话与根级 overlay 分别由 `RemoteSurfaceHost`、
`ConversationRouteSurface`、`WideConversationHost`、`AppRootOverlaySurfaces`
持有。宽屏几何已迁入 `pages/layout/WideLayoutGeometry.ets`，并由
`ArchitectureUnit` 覆盖关键几何约束。`AppRootPresentation.ets` 从基线 1449 行
收敛到 406 行，保留响应式测量、`Navigation`、compact preview overlay、Remote
settings sheet 与顶层装配。架构门禁要求该文件不超过 500 行，并要求上述拆分文件
持续存在。

HAP、LocalTest 与窄屏真机 Local → Remote → Local 往返均通过。真机 smoke 曾发现
`@BuilderParam` slot 内直接构造 V2 组件会触发 `class constructor cannot called without
'new'`；现已改为由 `@Builder` 方法承接 slot，并复验进程在完整往返中持续存活。
当前两个 target 分别为 1080 × 2444 真机和 466 × 466 模拟器，均不能提供宽屏三栏
验收条件，因此 S1 的宽屏视觉复验仍记为待办。

**风险点（本阶段唯一）**：`@Builder` 值参数不响应式（§1.3.1）。拆分后凡是原先由父 builder 传入的标量，必须改为子 builder 内读状态——`wideMasterPaneCurrentWidth()` 就是这个坑的既有修复案例。

**验证**：完整验证回路 + **必须真机复验宽屏三栏与窄屏抽屉来源切换**。

---

### S2 · 消灭位置参数接线（2–3 天，零行为变更）

**做什么**：12 个 `*Hooks` / `*Actions` 由 `class` + 位置构造改为 `interface` + 对象字面量。

```ts
// before —— 96 行实参，顺序错了编译期无感
new AppRootPresentationActions(onA, onB, onC, /* … */)

// after —— 字段名保护，新增回调只改两处
const actions: AppRootPresentationActions = {
  onA: () => { /* … */ },
  onB: () => { /* … */ },
  onC: () => { /* … */ }
};
```

约 650 行接线降至约 250 行。可按 12 个类逐个 commit，每个独立可回滚。

**风险**：低。ArkTS 对象字面量要求有明确声明类型，`interface` 满足；改造过程中若某个 Hooks 含方法实现而非纯回调字段，保留为 class 但改为具名参数对象构造。

---

### S3 · 统一会话状态（3–5 天，**有行为风险**）

**做什么**

1. 抽出承载 §2.5 那 15 个共享字段的公共载体；`GeneralChatPageState` / `RemotePageState` 只保留各自特有字段；
2. 删除 `services/AppRootRouteState.ets`（88 行）——同时消掉硬违规 ① 的四分之一；
3. 收敛 `ConversationViewState.project` 的双源分支。

**前置 spike（0.5 天，必做）**：验证 ArkUI V2 的 `@Trace` 能否穿透 `@ObservedV2` 基类继承——本模块目前没有先例，不能假设。

- 若可以 → 用继承（`ConversationSessionState` 基类）。
- 若不行 → **退化为组合**：两个 State 各持有一个 `ConversationCore` 字段，投影层只读 core。效果等价，只是访问路径多一层。

**Spike 结论（2026-08-06）**：采用组合方案。当前工程没有可证明 `@Trace`
跨 `@ObservedV2` 基类继承订阅关系的运行时先例，HAP 编译和 LocalTest 只能证明语法与
状态行为，不能证明 UI 订阅穿透。`GeneralChatPageState` 与 `RemotePageState` 因此各自组合
独立的 `ConversationCoreState`，组件和 `ConversationViewState` 直接读取 core。已通过窄屏
真机 Local → Remote → Local 往返验证；宽屏真机仍需在折叠设备展开后复验。

**风险**：本方案中最高。但安全网充足——`ConversationStateUnit`（1057）+ `RemoteControllersUnit`（2177）直接覆盖这块。

**验证**：完整回路 + 真机走通四条路径：本地新建/继续会话、Remote 新建/继续会话、窄屏抽屉来源切换、宽屏来源切换。

---

### S4 · 拆解 God Facade（4–6 天，分批）

**做什么**：按 S3 建立的特性边界，把 `AppRootRuntime` 切成 `ConversationController` / `RemoteConnectionController` / `SettingsController` / `FilePreviewController`，`AppRootRuntime` 退化为持有它们的组合根。

**实施结果（2026-08-07，已完成）**：已落地 `FilePreviewController`、
`SettingsController`，并建立 `ConversationController` 的首批跨表面 composer/voice 状态边界；
对应旧方法已从 `AppRootRuntime` 删除，静态门禁禁止回流。现有连接实现也已从
`RemoteConnectionViewModel` 更名为 `RemoteConnectionController`，根运行时的 21 个状态 getter
和 11 个连接状态转发已删除；路由、workspace/session 列表、polling/heartbeat 的 28 个
owner 转发也已改为直接绑定。云账号凭据、持久化、云模型目录、权限设置与账号设备切换
闭环也已迁入 `SettingsController`，包括原 91 行的 `selectCloudAccountDevice`。
远程会话的发送、停止/重试、工具动作、时间线投影与 polling cursor 运行态已迁入
`ConversationController`；Remote 新建会话的设备/workspace/模型选择、提交与路由流程也由其
统一持有。本地会话的打开/新建/发送、草稿、归档与时间线投影同样已收口到该 owner。
根运行时由 2608 行降至 372 行；纯依赖实例化和回调接线迁入
`AppRootRuntimeComposition`，其抽象端口仍由根运行时实现，避免装配层反向拥有页面生命周期行为。
HAP、完整 LocalTest 与窄屏真机 Local → Remote → Local 往返均通过。尚未完成
宽屏复验，仍等待可用的展开设备。

顺序（每步独立 commit）：

1. 清理 45 个一行转发——调用点直接指向真正的 owner；
2. 拆 621 行字段初始化块（L140–761）为各 Controller 的构造；
3. 处理 `selectCloudAccountDevice`（91 行）等长方法；
4. 按官方 V2 形状收口：view 用 `@ComponentV2` + `@Local` 持有**所属特性的** ViewModel，而非同一个巨型对象。

**与 S5 的次序说明**：本阶段涉及的装配层（`AppRootPresentation` 及其拆出的 host）已经是 V2，`AppRoot.ets` 虽是 V1 但无任何状态装饰器，因此第 4 步不需要等 S5。S5 排在其后，是因为它的主体（`ConnectView`、`AppSidebar` 等叶子组件）与 Controller 拆分互不相干，放在结构稳定之后迁移，可以避免同一文件被两种性质的改动连续翻动。

目标：`AppRootRuntime` < 500 行。消除硬违规 ③。

**风险**：中。生命周期是重点——`aboutToAppear` / `onPageShow` / `onPageHide` / `aboutToDisappear` / `handleRootBack` 的调用顺序与轮询启停必须逐一保持。`LifecycleUnit`（748）+ `AppRootLifecycleUnit`（129）+ `AppRootRuntimeStartupUnit`（51）覆盖此处。

---

### S5 · V1 全量迁移到 V2（4–5 天，**逐文件有行为风险，已完成 2026-08-07**）

**做什么**：把 §2.8 清单里的 15 个 V1 struct 全部迁到 `@ComponentV2`，之后 `pages/` 下不再存在 V1 装饰器。

**为什么值得单列一个阶段**（而不是像初版那样"顺手统一"）：

1. **官方推荐**。V2 是官方对新项目的推荐范式，官方 MVVM 示例也是 `@ComponentV2` + `@Local` 持有 ViewModel 实例的形式。范式统一后 §4 的目标结构与官方文档一一对应，不需要读代码的人在两套心智模型间切换。
2. **消除一类难定位故障**。§2.8 已核查：目前**没有**任何 `@ObservedV2` 对象被 V1 装饰器持有。但只要 V1 struct 还在，后续任何一次改动都可能把状态对象传进 `@Prop`，得到"编译通过、界面不刷新"——与抽屉不刷新（§1.3.2）同型的故障，本模块已经为这类问题付出过一次排查成本。
3. **观测边界更明确**。V2 的 `@Trace` 深度观测与 `@Monitor` 的新旧值回调，比 V1 的 `@Observed` / `@ObjectLink` 嵌套观测更容易推理，也更容易在 review 中判断对错。

**迁移映射表**（逐条替换，不是全局改名）：

| V1 | V2 | 语义差异——**必须逐字段确认，这是本阶段的主要风险**|
| --- | --- | --- |
| `@Component` | `@ComponentV2` | — |
| `@State`（53） | `@Local` | 基本等价，子组件自有状态 |
| `@Prop`（79） | `@Param` | **不等价**。V1 `@Prop` 是**深拷贝**，子组件可以本地改写；V2 `@Param` 是**按引用只读**，子组件不可赋值。凡是子组件确实在本地改写该字段的，需迁为 `@Param @Once`（仅初始同步、之后子组件自持）或 `@Local` + 显式初始化 |
| `@Link`（2） | `@Param` + `@Event` | **不等价**。V2 取消了双向绑定，须拆成"向下传值 + 向上回调"。仅 `CreateSessionSheet.ets` 的 `sessionTitle` / `instruction` 两处 |
| `@Watch`（4） | `@Monitor` | 回调签名不同，`@Monitor` 提供新旧值；`RemoteControlSettingsSheet` 1 处、`StreamingMarkdownContent` 3 处 |
| `@BuilderParam`（7） | 不变 | V2 同样支持，不属于迁移面 |

**顺序**（每个文件独立 commit，从小到大以便先摸清坑）：

1. 先迁 5 个小文件（`DefaultAccountAvatar` 18、`ConversationSourceSwitcher` 40、`AppRoot` 48、`ChatStatusBar` 60、`ThinkingBlock` 67）——`AppRoot` 无任何状态装饰器，是纯粹的 `@Component` → `@ComponentV2` 改名，可作为第一个 commit 验证工具链；
2. 迁 `@Link` / `@Watch` 三个特殊文件（`CreateSessionSheet` 226、`StreamingMarkdownContent` 142、`RemoteControlSettingsSheet` 872）——语义变化集中在这里，单独处理便于 review；
3. 迁剩余中等文件（`FileReferenceCard` 85、`OpenBitFunAccountLoginPage` 146、`MarkdownContent` 199、`SettingsSheet` 297、`ModelServiceSettingsPanel` 662）；
4. 最后迁 `AppSidebar`（908）与 `ConnectView`（1344）——这两个占 V1 总量 44%，且是 S6 的拆分目标，**先迁后拆**：若先拆再迁，会在拆分过程中制造 V1/V2 交界，把两类风险叠在同一个 commit 里。

**风险**：中。集中在 `@Prop` → `@Param` 的 79 处——**不能批量替换**，每一处都要确认子组件是否本地改写。`StreamingMarkdownContent` 尤其要小心：它的 3 个 `@Prop` 全部带 `@Watch`，流式 Markdown 的增量渲染依赖这套回调时序。

**验证**：完整回路，且**每个 commit 都要真机验证该组件所在界面**。重点回归：连接流程（`ConnectView`）、侧栏与会话列表（`AppSidebar`）、Remote 控制设置（`RemoteControlSettingsSheet`）、流式回复渲染（`StreamingMarkdownContent`）、新建会话（`CreateSessionSheet`）。

**完成标志**：`ArchitectureUnit` 的 V1 已知清单清空，范式断言由"等于清单"翻为"必须为空"；此后新增 V1 组件在 CI 直接失败。

**实际结果**：15 个 V1 页面组件全部迁移。逐字段审计结论是：只读父输入迁为
`@Param`；需要用户编辑的值由子组件 `@Local` draft 持有，并通过显式事件上送；
`CreateSessionSheet` 的两个 `@Link` 拆为 `@Param` + `@Event`；
`StreamingMarkdownContent` 与 `RemoteControlSettingsSheet` 的监听迁为 `@Monitor`。
本轮没有字段符合“只接收一次父级初值、之后完全由子组件持有”的语义，因此没有使用
`@Param @Once`。HAP 编译同时验证 `@Param` 未被子组件赋值，架构门禁中的 V1 清单
已经为空。HAP、LocalTest、窄屏启动与 Local → Remote → Local 往返均通过。

---

### S6 · 纯化组件层（3–4 天，纯视觉风险）

**做什么**

1. 侧栏和工具列表的重复 glyph 已分别收口到 `SidebarGlyphs.ets`、`ToolGlyphs.ets`；
2. 按视觉关注点拆出 `ConnectAccountDevicePage`（账号设备选择）、
   `ChatMessageContent`（图片/Markdown/文件卡片）两个 V2 子组件，
   `AppSidebar` 从 908 行降至 700 行，`ConnectView` 从 1344 行降至 1055 行。
   `ToolStatusList` 的业务分组和交互状态仍保留在原 owner，避免纯视觉迁移改变工具动作时序。
3. S1 同时完成根展示面的纯视觉拆分：Remote、窄屏路由、宽屏会话和 overlay 已由
   四组 V2 host/surface 组件持有，`AppRootPresentation` 当前为 406 行。
4. 第二批拆分已落地：`ConnectManualPairingOverlay` 持有手工配对表单，
   `ToolInteractionPanels` 持有工具 JSON 编辑/批准和问答草稿，`ChatMessageChrome`
   持有用户气泡、重试提示和流式三点动画。对应主文件当前分别为
   `ConnectView` 695 行、`ToolStatusList` 1106 行、`ChatMessageBubble` 972 行；预算已写入
   `pnpm run harmony:architecture`，禁止展示职责回流。

**目标已按 §3.4 修正**：不包含"切断 `components → state`"——该 import 合法。**也不再包含装饰器统一**——S5 已完成，本阶段拆出的新组件天然是 V2。

**风险**：纯视觉回归。**每一步必须真机截图，窄屏 + 宽屏 × 浅色 + 深色四组**；所有颜色走 `Theme.ets` 语义 token，`pnpm run theme:color-audit:all` 必须干净。

**实际进度（2026-08-07，进行中）**：已完成窄屏浅色启动、侧栏展开、
Local → Remote → Local 往返截图；新接入 HUAWEI MatePad Pro `WEB-W00`
（2880 × 1920），已安装本轮 HAP，并完成 Pad 浅色/深色下 Local、Remote Home 和连接
设备面板截图，应用进程持续存活。宽屏合同不等于 Pad 合同：现有
`ConversationLayoutPolicy` 同时读取零/一/两道纵向折痕，两道折痕的三折叠继续使用
“左屏 master + 中/右两屏同一个 detail”，正文与关键热区选择不跨第二道折痕的最宽
内容带；零/一/两道折痕、非对称三屏和非法折痕均有 LocalTest 覆盖。

三折叠完整展开及双屏/三屏动态切换仍需要真实两折痕设备验证，Pad 不能替代该项；
文件预览打开/关闭矩阵也尚未闭合，因此 S6 仍不能标记为完成。

---

### S7 · 关闭护栏（0.5 天）

原 3 处 `services/` → `pages/` 反向依赖已在 S1/S3 的文件归属迁移中清零；当前
`pnpm run harmony:architecture` 的 `serviceToPages`、`componentToViewmodel`、
`viewmodelToComponents` 均为空，V1 清单也为空。门禁已从基线清单切换为永久空集，
并补齐了 `AGENTS.md` 与 `ArchitectureUnit` 的归属说明。

---

## 6. 每阶段固定验证回路

```bash
source scripts/ohos-env.sh

# 1. 构建
"$HVIGORW" --mode module -p product=default -p module=entry@default assembleHap --no-daemon

# 2. 本地单元测试（6612 行 hypium 用例）
"$HVIGORW" --mode module -p module=entry@default -p ohos.test.type=LocalTest test --no-daemon

# 3. 颜色审计
pnpm run theme:color-audit:all

# 4. 真机验证（折叠设备 5ZU0226202001116）
hdc -t 5ZU0226202001116 shell snapshot_display -f /data/local/tmp/s.jpeg
hdc -t 5ZU0226202001116 file recv /data/local/tmp/s.jpeg ./s.jpeg
```

设备侧注意事项（已踩过的坑）：

- bundle 名以 `AppScope/app.json5` 为准，和手表端共用且作为升级兼容标识保持稳定；OpenBitFun 是产品展示名，不迁移 package identity、Preferences/HUKS/RDB/KV 的持久化标识。`distributedKVStore` 按 bundleName + storeId 隔离，任意改名都会让手机与手表落入互不相干的库，并让已安装用户丢失配对与登录恢复能力；
- `hdc` 必须带 `-t <serial>`，否则报 `[Fail]ExecuteCommand need connect-key`（列出了两个 target）；
- 外屏分辨率 1080×2444；点击用 `hdc -t <id> shell uinput -T -c X Y`。

**真机验证的最低集合**（每阶段都要过）：窄屏抽屉 Local ↔ Remote 来源切换、宽屏三栏、文件预览打开/关闭、深浅色各一轮。

---

## 7. 明确不做的事

- **不引入三层架构（products / features / commons）**。理由见 §1.2：单 entry 模块，收益为零、构建复杂度为正。
- **不引入新的状态管理库或跨端抽象层**。问题是组织方式，不是工具。
- **不重构 `services/general-chat/`（21 文件 / 3296 行）内部结构**。它自身分层是干净的，只需在 S7 切断对 `pages/` 的反向依赖。
- **不追求行数目标本身**。S1 + S2 净减约 800 行是副产品；真正的收益是"改一处不用改三处"和"违规能被 CI 挡住"。

> 初版方案曾把"V1 → V2 全量迁移"列在本节。该判断已推翻——理由见 §5 的 S5 阶段，迁移已提升为独立阶段。

---

## 8. 遗留事项

- **窄屏"刷新"与"助手选择"入口缺失**（baseline `6c35485bb` 引入）。删除 `RemoteHomeView.ets` 统一窄屏 Remote 界面时，这两个入口一并移除，宽屏本来就没有。待定：是否补进共享侧栏的 `...` 菜单。此项与本重构无依赖关系，可独立处理。
- **S6 组件纯化尚未完成**。优先继续拆分 `ToolStatusList`、`ChatMessageBubble` 与
  `ConnectView`，每次拆分保持动作 owner 和时序不变。
- **视觉验证矩阵尚未闭合**。仍需补窄屏深色、文件预览打开/关闭，以及宽屏三栏的
  深浅色截图；后者等待可用的展开折叠屏或平板 target。
