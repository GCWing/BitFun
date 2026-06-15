# 主题与颜色 Token 优化方案

> 基线：`gcwing/main` 的 `b5f4f131`，扫描日期为 2026-06-15。

本文档用于梳理 BitFun 前端主题、硬编码颜色、重复 token、近似色冗余、
命名漂移和后续治理方案。目标不是把所有看起来相近的颜色都合并，而是让
每一个颜色都能追溯到明确的语义角色，并保留那些会帮助用户区分区域、状态、
层级或数据含义的视觉差异。

## 范围

本方案覆盖：

- `src/web-ui` 中的主题预设、运行时 CSS 变量注入和共享样式 token。
- `src/web-ui/src/component-library/styles` 下的 token 定义。
- 组件 SCSS/CSS/TSX 中的硬编码颜色、fallback 色值和局部 token。
- 旧 token 名称到新规范名称的兼容别名。
- 后续防止新增硬编码颜色的审计和约束规则。

## 治理原则

本次优化的方向需要同时满足两点：

- 色值数量要尽可能收敛。一个应用的基础色值不应该无限增长，后续需要用
  明确的预算上限来约束 palette、semantic token 和 component token。
- 合并必须有依据。除非两个颜色已经极其相似、肉眼基本不可区分，否则不能只
  因为“看起来接近”就合并；需要说明它们为什么是同一角色、为什么不会破坏
  区域区分、状态区分或数据含义。

建议把颜色分成三类预算：

| 类型 | 建议上限 | 说明 |
| --- | ---: | --- |
| Primitive palette | 80-120 | 包含核心 hue、neutral、alpha ramp；主题预设可以映射，但不应无限扩张。 |
| App semantic token | 40-70 | 覆盖背景、文本、边框、状态、交互和 app intent。 |
| Component token | 每个复杂 surface 8-20 | 只在 semantic token 不足以表达组件契约时添加。 |

预算不是为了追求某个机械数字，而是为了阻止“每个组件随手新增一个色值”。
新颜色必须进入以下流程之一：

1. 能映射到现有 token：直接复用，不新增色值。
2. 肉眼不可区分：合并到已有色值，并记录为直接合并。
3. 有独立语义：新增 semantic 或 component token，并说明为什么不能复用。
4. 属于 editor、terminal、syntax、diff 等专用域：进入 exception namespace。

第一阶段不覆盖：

- 重新设计品牌视觉方向或重做主题风格。
- 强行替换 Monaco editor、terminal ANSI、Mermaid、语法高亮或第三方内容
  的专用色板。
- 对每个页面做像素级重设计。
- 在所有调用方迁移完成前移除兼容别名。

## 当前现状

基于最新 `gcwing/main` 的扫描结果，当前颜色系统已经具备一定抽象，但分散
程度较高，重复和命名漂移明显。

| 指标 | 当前基线 |
| --- | ---: |
| 扫描的前端文件数 | 1711 |
| 包含颜色字面量的文件数 | 297 |
| 颜色字面量出现次数 | 5572 |
| 唯一颜色字面量数量 | 1532 |
| 组件或非 token 文件中的颜色出现次数 | 3735 |
| 包含组件或非 token 颜色的文件数 | 272 |
| `var(--token, fallback-color)` 出现次数 | 2847 |

债务集中在少数高频 UI 区域：

| 区域 | 文件 | 非 token 颜色出现次数 |
| --- | --- | ---: |
| Flow Chat 输入区 | `src/web-ui/src/flow_chat/components/ChatInput.scss` | 158 |
| Toolbar mode | `src/web-ui/src/flow_chat/components/toolbar-mode/ToolbarMode.scss` | 94 |
| Code editor | `src/web-ui/src/component-library/components/CodeEditor/CodeEditor.scss` | 86 |
| Profile nursery view | `src/web-ui/src/app/scenes/profile/views/NurseryView.scss` | 78 |
| Generative widget frame | `src/web-ui/src/tools/generative-widget/GenerativeWidgetFrame.tsx` | 78 |
| Select 组件 | `src/web-ui/src/component-library/components/Select/Select.scss` | 70 |
| Code review tool card | `src/web-ui/src/flow_chat/tool-cards/CodeReviewToolCard.scss` | 70 |
| Stream text | `src/web-ui/src/component-library/components/StreamText/StreamText.scss` | 66 |
| Snapshot diff viewer | `src/web-ui/src/flow_chat/tool-cards/SnapshotFullscreenDiffViewer.css` | 64 |
| Workspace manager | `src/web-ui/src/tools/workspace/components/WorkspaceManager.css` | 64 |

重复最多的原始色值主要是应用强调色、状态色、白色半透明叠层和暗色表面叠层：

| 色值 | 次数 | 推测角色 |
| --- | ---: | --- |
| `#60a5fa` | 162 | blue accent / focus / info |
| `rgba(255, 255, 255, 0.08)` | 115 | 暗色主题 subtle overlay |
| `rgba(255, 255, 255, 0.1)` | 112 | 暗色主题 hover/elevated overlay |
| `#f59e0b` | 112 | warning |
| `#ffffff` | 107 | white / inverse text |
| `#ef4444` | 110 | error / danger |
| `#22c55e` | 77 | success |
| `#3b82f6` | 70 | primary / info |
| `rgba(255, 255, 255, 0.05)` | 70 | 暗色主题低强度 overlay |
| `rgba(255, 255, 255, 0.06)` | 68 | 暗色主题低强度 overlay |

fallback 也已经形成了第二套分散色板。高频 fallback token 如下：

| fallback token | 次数 |
| --- | ---: |
| `--color-accent-500` | 147 |
| `--color-warning` | 127 |
| `--color-error` | 117 |
| `--color-success` | 109 |
| `--color-text-muted` | 77 |
| `--color-text-primary` | 75 |
| `--color-text-secondary` | 71 |
| `--border-subtle` | 55 |
| `--element-bg-subtle` | 39 |
| `--color-primary` | 41 |

## 现有架构地图

当前主题相关定义分布在多个层次：

- `src/web-ui/src/component-library/styles/tokens.scss` 定义 SCSS 变量和
  `:root` CSS 变量。
- `src/web-ui/src/infrastructure/theme/core/ThemeService.ts` 根据当前主题在运行时注入 CSS 变量，
  同时补充了一批 app 级别别名和覆盖值。
- `src/web-ui/src/theme/presets/*.ts` 定义主题预设色板。
- `src/web-ui/src/tools/generative-widget/themePayload.ts` 向 generative widget payload
  暴露部分主题变量。
- 组件 SCSS/CSS/TSX 中存在大量局部颜色字面量和局部 fallback。

主要架构问题：

- 静态 token 和运行时 token 没有共享单一注册表。
- 有些 token 只由 `ThemeService.ts` 动态注入，但组件 fallback 假设它们
  在所有渲染边界都存在。
- 同一个语义角色存在多种历史命名方式。
- 组件 fallback 中的字面量过多，导致 fallback 变成实际上的第二套色板。
- 当前主题验证链路不能作为充分的可访问性证据，contrast 计算需要真实实现
  后才能支撑大规模颜色合并判断。

## 问题分类

### 1. 组件内硬编码颜色

多个组件直接写入产品语义色，例如 `#60a5fa`、`#ef4444`、`#22c55e`
和大量白色半透明叠层。这会让主题调整变成跨文件替换，也会让同一语义角色
在不同组件中逐渐漂移。

改进方向：

- app 级语义颜色改为 CSS 变量。
- 组件独有角色使用组件 token。
- Monaco、terminal、语法高亮等特殊色板不直接映射到普通 app token，
  先建立专用命名空间。

### 2. 重复 fallback 色值

`var(--token, literal)` 对兼容有价值，但当它在大量组件中重复时，就会让
组件层携带 palette 副本。

改进方向：

- fallback 只保留在明确的兼容边界。
- 根主题层先补足兼容别名。
- 组件确认 canonical token 一定存在后，移除局部 fallback 字面量。

### 3. 未定义或历史命名 token

高频可疑名称包括：

- `--color-text-tertiary`
- `--accent-primary`
- `--color-bg-hover`
- `--text-secondary`
- `--color-danger`
- `--color-border-subtle`
- `--element-bg-hover`
- `--border-primary`

其中部分可能来自动态注入，但也有明显历史别名或命名分叉。它们需要显式
进入兼容映射，而不是依赖组件 fallback 暗中兜底。

改进方向：

- 在主题层增加兼容 alias map。
- 文档中标记 deprecated 名称。
- 调用点逐步迁移到 canonical 名称。

### 4. 精确重复 token 值

精确重复不一定是错误。很多重复其实是不同语义角色当前恰好使用同一色值。
问题在于当前定义没有清晰表达 alias 方向。

例子：

- `#0e0e10` 同时用于 `$color-bg-primary`、`$color-bg-tertiary`、
  `$color-bg-workbench`、`$color-bg-flowchat`。
- `#1c1c1f` 同时用于 `$color-bg-secondary`、`$color-bg-elevated`、
  `$color-bg-scene`。
- git 相关颜色与 app intent 色如 warning、error、info 有重复。
- `$panel-border`、`$card-border`、`$input-border`、`$nav-border`
  都指向 `$border-base`。

改进方向：

- 不因为值相同就删除语义 token。
- 以“primitive value -> semantic token -> component token”的方向表达别名。
- 标记哪些 alias 是稳定语义 alias，哪些只是迁移期 alias。

### 5. 近似色冗余

近似色是风险最高的一类。相似颜色可能是历史漂移，也可能是在保护区域边界、
状态差异或主题个性。

典型族群：

- 蓝色强调族：`#60a5fa`、`#58a6ff`、`#3b82f6`。
- 暗色表面族：`#0e0e10`、`#111114`、`#121214`、`#141414`、
  `#16161a`、`#18181a`、`#1a1a1a`、`#1c1c1f`、`#1e1e22`。
- 灰色文本和边框族：`#a0a0a0`、`#9ca3af`、`#6b7280`、
  `#64748b`、`#e8e8e8`、`#e5e5e5`。
- 白色 overlay alpha：从 `0.03` 到 `0.18` 都有出现。

改进方向：

- 不能只按色差或 RGB 距离合并。
- 先判断语义角色、相邻关系、交互状态、主题预设和可访问性，再决定是否替换。
- 对于白/黑透明叠层，先建立精确等价的 overlay ramp，例如
  `--color-overlay-white-08` 和 `--color-overlay-white-10`。这一步只消除散落
  硬编码，不合并不同 alpha，因为 alpha 差异经常用于表达层级和状态。

## 目标 Token 模型

建议采用分层 token 模型，每一层只承担一个职责。

### Primitive palette

primitive token 是原始色阶，不建议在普通组件样式中直接使用，只用于定义
语义 token。

示例：

- `--palette-blue-500`
- `--palette-red-500`
- `--palette-green-500`
- `--palette-amber-500`
- `--palette-neutral-900`
- `--palette-white`

### App semantic token

semantic token 描述产品级语义，应作为共享 UI 的默认使用层。

建议族群：

- 背景：`--color-bg-primary`、`--color-bg-secondary`、
  `--color-bg-tertiary`、`--color-bg-elevated`、`--color-bg-workbench`、
  `--color-bg-scene`、`--color-bg-flowchat`。
- 文本：`--color-text-primary`、`--color-text-secondary`、
  `--color-text-muted`、`--color-text-disabled`；如果设计系统确实需要第三层
  文本强度，再将 `--color-text-tertiary` 转正。
- 边框：`--border-base`、`--border-subtle`、`--border-emphasis`、
  `--border-focus`。
- 元素状态：`--element-bg-default`、`--element-bg-subtle`、
  `--element-bg-hover`、`--element-bg-active`、`--element-bg-selected`。
- 意图色：`--color-success`、`--color-warning`、`--color-error`、
  `--color-info`。
- 意图色背景：`--color-success-bg`、`--color-warning-bg`、
  `--color-error-bg`、`--color-info-bg`。

### Component token

当共享 semantic token 过于泛化，或者会隐藏组件自身契约时，使用组件 token。

示例：

- `--flowchat-input-bg`
- `--flowchat-input-border`
- `--flowchat-drop-zone-bg`
- `--toolbar-mode-bg`
- `--toolbar-mode-active-bg`
- `--tool-card-bg`
- `--tool-card-hover-bg`
- `--diff-added-bg`
- `--diff-deleted-bg`
- `--editor-token-keyword`
- `--terminal-ansi-green`

组件 token 默认可以映射到 semantic token，但当用户含义依赖差异时，需要保留
专用色值或专用映射。

### 兼容别名

第一阶段应先保留兼容别名，避免为了清理 token 引入大面积视觉变化。

| 历史或漂移 token | 建议 canonical 目标 | 说明 |
| --- | --- | --- |
| `--accent-primary` | `--color-accent-500` 或 `--color-primary` | 先明确 accent 与 primary 是否是两个角色。 |
| `--text-primary` | `--color-text-primary` | 仅兼容别名。 |
| `--text-secondary` | `--color-text-secondary` | 仅兼容别名。 |
| `--text-muted` | `--color-text-muted` | 仅兼容别名。 |
| `--bg-primary` | `--color-bg-primary` | 仅兼容别名。 |
| `--bg-secondary` | `--color-bg-secondary` | 仅兼容别名。 |
| `--bg-tertiary` | `--color-bg-tertiary` | 仅兼容别名。 |
| `--border-primary` | `--border-base` | 需要确认调用点是否期望更强边界。 |
| `--color-border-subtle` | `--border-subtle` | 建议统一 border 命名族。 |
| `--color-danger` | `--color-error` | 仅当 destructive 与 validation error 不需要区分时合并。 |
| `--color-bg-hover` | `--element-bg-hover` | 需要确认调用点是 element、card 还是 panel hover。 |
| `--radius-*` | `--size-radius-*` | 对齐静态 CSS export 与运行时/widget 命名。 |
| `--spacing-*` | `--size-gap-*` | 对齐静态 CSS export 与运行时/widget 命名。 |

## 近似色合并规则

近似色清理必须先做安全分类。不能批量把相近颜色替换成同一个值。

默认目标是收敛，而不是保守保留。判断顺序应为：

1. 先证明能不能复用已有 token。
2. 如果色差极小且肉眼基本不可区分，可以直接合并。
3. 如果色差可见，必须给出合并依据：相同语义、非相邻显示、非状态区分、
   非数据含义、contrast 安全。
4. 如果依据不足，先标记为 `defer`，并补截图或调用点证据。
5. 只有存在明确用户理解风险时，才标记为 `do not merge`。

### 可以安全合并

同时满足以下条件时，可以合并：

- 色值代表同一个语义角色。
- 正常工作流中不会相邻显示。
- 不用于区分状态、严重程度、来源、所有权或数据含义。
- 替换后 contrast 不低于验收阈值。
- 截图对比没有造成层级或交互 affordance 丢失。

常见安全场景：

- 精确重复的语义 alias。
- 组件 fallback 复制了已经保证存在的根 token。
- 历史 alias 在运行时已经稳定指向 canonical token。
- 同一个暗色主题状态下重复出现的白色 overlay 值。

极高相似度直接合并建议门槛：

- 同一色彩空间和同一 alpha 下，RGB 通道差异肉眼不可辨。
- 不涉及 status、diff、syntax、terminal、theme personality。
- 不在相邻区域中承担边界分隔。
- audit report 中标记为 `indistinguishable`，review 时只需抽样确认。

### 必须视觉复核后才能合并

出现以下任一情况，合并前必须做视觉复核：

- 两个颜色会出现在同一视口或相邻区域。
- 颜色用于区分嵌套 panel、card、canvas、tool surface。
- 一个颜色表示 hover、active、selected、disabled、drag-over 或 focus。
- 颜色出现在 Flow Chat、tool card、review panel、git/diff UI、generated widget
  frame 等高密度区域。
- 颜色属于某个主题预设的个性表达。

合并前检查：

- 桌面和窄屏布局都要有 before/after 截图。
- 检查 normal、hover、active、selected、disabled、loading、error 等状态。
- 检查同一视口中的相邻区域。
- 检查文字和图标在替换后背景上的 contrast。
- 明确回答用户是否会失去以下判断能力：
  - 我现在在哪个区域。
  - 哪些元素可交互。
  - 当前状态是什么。
  - 哪些内容发生了变化。

这类合并不应被视为禁止合并。它们是主要的色值压缩空间，但必须带证据：

- 调用点列表。
- 旧值和目标 token 的语义说明。
- 相邻区域判断。
- before/after 截图或等价视觉证据。
- 如果合并会产生可见变化，需要在 PR 描述中说明预期影响。

### 默认不合并

以下场景默认不按近似色合并：

- success、warning、error、info、destructive action。
- git added、modified、deleted、renamed、branch、conflict。
- diff added/deleted 背景和行内高亮。
- Monaco syntax、terminal ANSI、code review token colors。
- cyber、tokyo、midnight、China 等主题个性颜色。
- 导航、scene、panel、canvas、input、floating overlay 等相邻布局区域的边界色。
- 任意可能改变 foreground/background contrast 的可访问性组合。

## 相邻关系审查模型

每个近似色合并候选都需要先回答这些问题：

| 问题 | 原因 |
| --- | --- |
| 它是否会和替换目标出现在同一视口？ | 相邻颜色可能承担区域分隔作用。 |
| 它是否区分父子表面？ | 合并可能让 card、panel、input 混在一起。 |
| 它是否区分交互状态？ | 合并可能削弱 hover、focus、active、selected。 |
| 它是否区分严重程度或数据含义？ | 状态色必须保持可读。 |
| 它是否同时影响亮色和暗色主题？ | 暗色下安全的合并，亮色下可能失败。 |
| 它是否出现在 generated widget 或 embedded frame？ | 嵌入表面不一定继承全部 root token。 |
| 它是否是主题个性的一部分？ | 主题预设可能需要保留接近但不同的 accent。 |

第一轮实施应优先建立以下高风险表面的 review inventory：

- 主框架：导航、scene viewport、content canvas、side panel。
- Flow Chat：transcript、input、collapsed input、tool card、toolbar mode、
  review team surface。
- Git 和 diff：状态 badge、文件状态、行高亮、branch indicator。
- Component library：select、code editor、stream text、button、input。
- Generated widget：widget frame、widget content、payload-exposed variables。

## 分阶段实施方案

### Phase 0：基线与工具

先建立可重复审计工具，再做批量修改。

交付物：

- 按文件和按色值聚合的颜色字面量清单。
- CSS 变量使用清单。
- 未定义或历史 token 报告。
- 精确重复 token 组。
- 按 hue/value/alpha 聚类的近似色报告。
- 高风险表面清单。

验收标准：

- 脚本可以在 `src/web-ui` 上无副作用运行。
- 报告可以对比 baseline 与当前分支。
- 报告能区分普通 app color 与已知 exception namespace。

### Phase 1：canonical token 契约

明确 canonical token 家族和兼容别名。

交付物：

- canonical token map。
- 历史名称兼容 alias。
- `tokens.scss`、`ThemeService.ts`、`themePayload.ts` 的静态与运行时变量
  对齐。
- deprecated token 名单。

验收标准：

- 现有 UI 不应出现可见变化。
- 组件可以直接使用 canonical 名称，不需要本地 fallback literal。
- generated widget 在合理范围内获得与 app surface 一致的尺寸和颜色变量。

### Phase 2：精确重复合并

只合并 alias 安全的精确重复。

交付物：

- token 定义通过 alias 表达方向，而不是重复字面量。
- intent 与 git/diff alias 分开记录。
- border alias 指向 canonical border token。
- 高频白/黑 overlay 字面量迁移到精确 alpha token；相近 alpha 只记录为候选，
  不在没有视觉证据时合并。

验收标准：

- 预期无截图可见变化。
- `git diff --check`、web lint、type check 和相关测试通过。

### Phase 3：legacy fallback 迁移

迁移高频 fallback 调用点。

建议顺序：

1. component-library 中的 select、input、button、stream text。
2. Flow Chat 的 toolbar 和 input。
3. tool card 与 review panel。
4. workspace、git 和 diff surface。
5. generated widget frame 和 payload consumer。

验收标准：

- 组件文件不再携带根 token 的 fallback palette。
- 兼容 alias 仍保留给旧调用方或外部边界。
- 剩余 fallback 必须能解释其边界，例如 embedded widget 或第三方内容。

### Phase 4：组件 token 抽取

为不适合泛化的角色建立组件 token。

交付物：

- Flow Chat token set。
- Tool card token set。
- Diff/git token set。
- Editor/terminal exception token set。
- Widget frame token set。

验收标准：

- 组件 token 默认映射到 semantic token。
- 有意保留的例外被记录，并完成视觉复核。
- 组件不再直接用 raw color 表达产品语义。

### Phase 5：近似色合并

只有在 Phase 0-4 完成后，才进入近似色合并。

交付物：

- 候选合并表：包含角色、调用点、相邻风险和决策。
- 每个 conditional merge 都有 before/after 截图。
- rejected merge list，记录有意保留的近似色。

验收标准：

- 被合并的颜色拥有相同语义角色。
- 相邻 UI 层级仍清晰。
- 状态和数据含义仍可区分。
- 主题个性没有被抹平。

### Phase 6：防回退约束

增加轻量约束，避免新增同类债务。

交付物：

- 对组件中新 app raw color 的 lint 或 audit 检查。
- 已知 exception file 与 namespace allowlist。
- CI 在迁移期只阻止新增问题，不因历史 baseline 直接失败。

验收标准：

- 新增组件级 raw color 必须有明确原因。
- 历史迁移可以按目录增量推进。
- exception 可见、可审查。

## 风险清单

| 风险 | 影响 | 缓解措施 |
| --- | --- | --- |
| 相邻表面的近似色被合并 | 用户可能无法区分 panel、card、输入区或工作区边界。 | 近似色合并前必须做相邻关系审查和截图对比。 |
| hover/active/selected 被合并到静态背景 | 交互 affordance 变弱。 | 状态 token 与 base surface token 分开建模。 |
| intent 色被过度归一 | warning、error、success、info 或 destructive 语义混淆。 | intent token 即使色值接近，也保留独立语义。 |
| git/diff 色被当作普通 success/error | added/deleted/changed/conflict 扫描效率下降。 | 使用专用 git/diff token，只有复核后才 alias 到 app intent。 |
| 主题个性被抹平 | 用户选择主题的价值下降。 | theme preset 保留自己的 primitive/accent 映射。 |
| fallback 先删、alias 后补 | embedded 或 early render surface 样式丢失。 | 先加 alias，再删除 fallback。 |
| 静态 token 与运行时 token 不一致 | widget、SCSS、runtime theme 注入结果不一致。 | `tokens.scss`、`ThemeService.ts`、`themePayload.ts` 同阶段对齐。 |
| contrast 验证不可信 | 可访问性回归可能漏掉。 | 先实现真实 contrast 检查，再声称可访问性改善。 |
| 迁移 PR 过大 | review 疲劳导致视觉回归漏审。 | 按 surface 拆 PR，每个 PR 附指标和截图。 |
| editor/terminal 颜色被强行泛化 | 代码语法和 terminal 语义下降。 | 建立 exception namespace，而不是直接套普通 app token。 |

## 候选决策

### 精确重复

建议先合并定义方式，不删除语义角色。

- `--color-bg-workbench`、`--color-bg-flowchat`、`--color-bg-primary`
  即使当前解析到同一个值，也应保留为不同语义契约。
- panel/card/input/nav border 可以 alias 到 `--border-base` 或
  `--border-subtle`，但需要根据真实 contrast 和相邻关系确认。
- git/diff token 即使映射到 app intent 色，也应在组件使用层保持独立名称。

### 暗色表面近似色

不建议一次性合并所有暗色背景。

原因：

- BitFun 的主界面是高密度相邻 surface。极小的暗色差异可能用于区分 scene、
  panel、card、editor、input、floating overlay。
- 应先建立层级表：
  base -> workbench -> scene -> panel -> card -> elevated -> overlay ->
  hover/selected。

### 白色 overlay alpha

只按状态角色合并，不按“都是 white alpha”合并。

建议 token：

- `--overlay-white-subtle`
- `--overlay-white-hover`
- `--overlay-white-active`
- `--overlay-white-selected`
- `--overlay-white-focus`

alpha 差异经常承担 elevation 和交互状态，不应全部压成一个值。

### 蓝色强调色

保留 theme-specific 和 state-specific 蓝色，直到调用点完成分类。

可能角色：

- `--color-primary`
- `--color-accent-500`
- `--color-info`
- `--border-focus`
- `--link-color`
- `--selection-bg`

在确认调用点究竟表示 accent、info、link、focus、selected 或主题个性之前，
不要合并 `#60a5fa`、`#3b82f6`、`#58a6ff`。

### Editor 和 terminal 色

使用专用命名空间，不直接使用普通 app token。

建议方向：

- `--editor-syntax-keyword`
- `--editor-syntax-string`
- `--editor-selection-bg`
- `--terminal-ansi-red`
- `--terminal-ansi-green`
- `--terminal-selection-bg`

只有在语法和 terminal 含义仍然清晰时，才考虑把它们映射到 app palette。

## 验证方案

文档变更：

- `git diff --check`

实现类 PR：

- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`
- 被修改 surface 的 focused screenshot review。
- changed foreground/background pair 的 contrast 检查。

大型 theme/runtime 变更还需要：

- 验证静态 CSS 变量和运行时注入变量都存在。
- 验证 generated widget payload 变量。
- 验证 dark 和 light theme。
- 至少覆盖以下 surface：
  - main shell
  - Flow Chat input 和 transcript
  - toolbar mode
  - tool card
  - review team panel
  - git/diff view
  - code editor
  - generated widget frame

建议每个实现 PR 都附 before/after 指标：

| 指标 | 目标 |
| --- | --- |
| 组件文件 raw color literal | 每个迁移 PR 递减。 |
| 组件级 fallback literal | 兼容 alias 落地后递减。 |
| 未定义或历史 token 使用 | 除文档化 alias 外逐步清零。 |
| token 文件中的精确重复 literal | 改为 alias 表达。 |
| 近似色合并候选 | 每个都有 `merge`、`defer` 或 `do not merge` 决策。 |
| 视觉回归 | 已复核 surface 无回归。 |

长期预算目标：

| 指标 | 目标 |
| --- | --- |
| app 级 raw color literal | 普通组件中趋近于 0。 |
| unique app color literal | 进入 token 层后受预算约束，不再随组件增长。 |
| undocumented component color | 0。 |
| exception namespace color | 有 allowlist 和 owner。 |

## Review Checklist

颜色合并 PR 合入前必须检查：

- 每个被替换的字面量是否有明确语义角色。
- 旧色和新色是否可能在同一视口相邻出现。
- 旧差异是否用于区分父子 surface。
- 旧差异是否用于区分 hover、active、selected、focus、disabled、
  loading、drag-over 或 error。
- 旧差异是否用于区分状态、严重程度、数据来源或文件变更类型。
- 变更是否同时影响 light 和 dark theme。
- 变更是否影响 generated widget、code editor、terminal、Mermaid 或第三方内容。
- 删除 fallback 前，兼容 alias 是否已经存在。
- 高风险 surface 是否有截图或 focused visual check。
- PR 描述是否说明了任何用户可见视觉变化。

## 建议 PR 拆分

建议按证据和 surface 拆分，避免一次性大迁移：

1. 审计工具和 baseline report。
2. canonical token map 与 compatibility alias。
3. 静态和运行时 token 对齐。
4. token 文件中的精确重复合并。
5. component-library fallback 迁移。
6. Flow Chat surface 迁移。
7. tool card 和 review panel 迁移。
8. git/diff surface 迁移。
9. widget/editor/terminal namespace 清理。
10. 带截图的近似色合并批次。
11. 新增 raw app color 的防回退约束。

每个 PR 应包含：

- 范围和影响 surface。
- before/after 指标。
- 用户可见 surface 的截图。
- 明确保留的近似色列表。
- 验证命令和结果。

## 待决问题

- `--color-text-tertiary` 应转正为一等 semantic token，还是迁移到
  `--color-text-muted`。
- `--color-primary` 和 `--color-accent-500` 是否是两个角色，还是应统一为
  一个 accent contract。
- `--color-danger` 是否需要和 `--color-error` 区分，以表达 destructive action。
- 尺寸 token 长期应统一为 `--size-radius-*` / `--size-gap-*`，还是继续暴露
  `--radius-*` / `--spacing-*` 兼容名。
- 迁移期 CI 应如何严格：只阻止新增 raw app color，还是按目录迁移完成后
  对该目录启用失败约束。

## 完成标准

整体优化完成时应满足：

- 共享 app color 由 canonical semantic token 表达。
- 组件专属角色由文档化 component token 表达。
- 历史 token 名称已迁移或明确 alias。
- 普通组件文件不再出现 app 级 raw color。
- 近似色都有 merge、defer 或 reject 决策记录。
- 相邻 surface、交互状态、状态语义和主题个性仍能被用户清楚识别。
- 静态 token、运行时 token、widget payload token 对齐。
- 新增 raw color 必须经过可见 review 决策。
