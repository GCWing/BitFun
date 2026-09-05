# OpenBitFun web-ui 交互流畅度(UI 响应性)审阅报告

- 审阅范围:`src/web-ui`(React 18 + Vite + zustand v5 + Monaco + xterm + react-virtuoso)
- 审阅方式:架构走读(入口/状态管理/场景) → 反模式全量 Grep 扫描(渲染、事件、CSS 三条线)→ 关键交互组件精读(聊天流式管线、输入框、终端、文件树、Monaco、Markdown 渲染)。**所有条目均已打开源码核对上下文**,文中行号以当前 main 分支(48a003b73)为准。
- 日期:2026-07-26

> 退役说明（2026-08）：本文是提交 `48a003b73` 的固定历史审阅证据。鼠标跟随光效此后已完整删除，
> 因此下文 F9 与 T10 已通过退役关闭；相关段落仅解释当时结论，不得据此恢复或重新实现该功能。

## 总体评价(先说结论)

这套前端在流畅度上的**基础架构是同类项目里少见的高水位**,大量常见反模式已被系统性规避。实施优化前请先了解以下"已做对、勿回退"清单:

| 已有优化 | 位置 |
|---|---|
| 聊天消息列表、文件树均已虚拟化(react-virtuoso) | `flow_chat/components/modern/VirtualMessageList.tsx:15`、`tools/file-system/components/VirtualFileTree.tsx:131` |
| 流式事件 rAF + 32ms 合批,非逐 chunk setState | `flow_chat/services/EventBatcher.ts:26` |
| 自适应打字机:rAF 驱动、≥16ms 最小 paint 间隔、每帧字符上限 | `flow_chat/hooks/useTypewriter.ts:83-113` |
| 稳定回合的 VirtualItem 用 WeakMap 缓存、引用相等短路同步 | `flow_chat/store/modernFlowChatStore.ts:361-404`、`flow_chat/services/storeSync.ts:71-93` |
| 终端:PTY 输出直写 xterm(绕过 React)、WebGL 渲染器、键入合并成批量 IPC | `tools/terminal/components/ConnectedTerminal.tsx:180-199,291-300`、`tools/terminal/components/Terminal.tsx:551-560` |
| Monaco worker 化(getWorker 按 label 分发) | `tools/editor/services/MonacoInitManager.ts:106-116` |
| 流式代码预览:useDeferredValue + 尾部切片(6000 字符/24 行上限) | `flow_chat/components/CodePreview.tsx:50-54,136-151` |
| diff 计算有界且仅在 completed 后执行一次 | `flow_chat/tool-cards/FileOperationToolCard.tsx:414-433`、`flow_chat/components/InlineDiffPreview.tsx:31` |
| mermaid 动态 import;Tauri listen 封装为同步 unlisten,规避经典 Promise 泄漏 | `tools/mermaid-editor/services/MermaidService.ts:15`、`infrastructure/api/adapters/tauri-adapter.ts:120` |
| 全局无 `* { transition }`;26 处 Observer 均正确 disconnect | (全量扫描确认) |

问题集中在三处:**流式期间 Context 抖动使上述 memo 体系整体失效**(F1/F2)、**聊天输入框每次按键的强制布局链**(F3)、以及若干 CSS/监听器层面的持续性开销。

---

## 一、发现总览表(按预期收益排序)

| # | 问题 | 主要位置(文件:行) | 用户可感知影响 | 收益 |
|---|---|---|---|---|
| F1 | FlowChatContext value 每次流式 flush 重建,全部可见消息组件绕过 memo 重渲染 | `ModernFlowChatContainer.tsx:406-458` | 流式回复时整屏卡顿、掉帧,长会话尤甚 | 高 |
| F2 | FlowTextBlock 内联回调破坏 Markdown memo;流式期间每次 paint 全文 remark 重解析 | `FlowTextBlock.tsx:188-190`、`Markdown.tsx:785,1411` | 长回复流式后半段明显变卡 | 高 |
| F3 | ChatInput 每次按键:整棵输入容器 cloneNode 测量 + 写→读→写强制布局;5234 行单体组件全量重渲染 | `ChatInput.tsx:553-575,605-693,696-725` | 打字延迟、中文输入法卡顿 | 高 |
| F4 | 导航栏拖拽:每个 mousemove setState → 全工作区(含所有常驻场景)重渲染;无 unmount 清理 | `WorkspaceBody.tsx:53-91` | 拖侧栏分隔条明显跟手性差 | 中 |
| F5 | 每条用户消息各自注册 window resize 监听并做 4 次强制布局读 | `UserMessageItem.tsx:203-223`、`UserMessage.tsx:145-164` | 窗口缩放时长会话卡顿 | 中 |
| F6 | 桌面宠物:28ms 自失效 interval(每 tick 重建)+ 120ms Tauri IPC 轮询 + rect 读取 | `AgentCompanionDesktopPet.tsx:308-333,490-501` | 常驻 CPU 底噪、耗电 | 中 |
| F7 | 聊天输入框同一元素叠加 backdrop-filter: blur(16px) 与 min/max-height、padding 的 transition | `ChatInput.scss:632-640` | 胶囊↔多行切换 320ms 内逐帧重排+重采样模糊 | 中 |
| F8 | 工具卡完成退出动画:1000ms 的 max-height/margin 关键帧 + will-change 误用 | `ModelRoundItem.scss:394-445` | 流式时消息列表连续 1 秒逐帧重排 | 中 |
| F9（已关闭） | 历史鼠标跟随光效的常驻重绘问题；功能已退役 | 已删除 | 不再适用 | 已关闭 |
| F10 | zustand 全量订阅:useCanvasStore() 无 selector(且同时订阅 5 个作用域 store);useMessageEditStore() 每条消息全量订阅 | `canvasStore.ts:1299-1320` 及 4 处调用、`UserMessageItem.tsx:100-108` | 面板/编辑区、消息列表的额外级联重渲染 | 中 |
| F11 | VirtualMessageList 滚动补偿路径内刻意强制回流(写 footer 高度后立即读 offsetHeight/scrollHeight) | `VirtualMessageList.tsx:795-808,2255-2350` | 工具卡折叠 + 滚动同时发生时掉帧 | 中(高风险,慎改) |
| F12 | 滚动回调未节流 + 循环内 getBoundingClientRect(文件树 O(2N)/洞察页);ExploreGroupRenderer onScroll 直接 setState | `FileExplorer.tsx:63-107`、`InsightsScene.tsx:472-490`、`ExploreGroupRenderer.tsx:95-107,272` | 大目录/长页滚动变钝 | 低-中 |
| F13 | `transition: all` + hover 动画 width/height/box-shadow(面板分隔条,高频触发) | `resizer.css:15-38,56-70`、`SessionScene.scss:183,206,236-237,324,357` | 鼠标扫过面板边缘触发相邻面板重排 | 低-中 |
| F14 | 常驻 6 层 box-shadow 无限呼吸动画;宠物 SVG url() 滤镜叠加无限 transform 动画 | `FlexiblePanel.scss:215-240,494,558`、`AgentCompanionPet.scss:114,133,148-151` | 空闲时持续重绘、耗电 | 低 |
| F15 | 监听器清理缺口 4 处(GitStateManager dispose 不摘除全局监听、PeerHostInvokeBridge 与 useWindowControls 的 await-before-assign 竞态、WorkspaceAPI abort 监听残留) | `GitStateManager.ts:252-266,744-753`、`PeerHostInvokeBridge.tsx:75-112`、`useWindowControls.ts:164-210`、`WorkspaceAPI.ts:471-478` | 潜在泄漏/幽灵回调,当前影响有限 | 低 |
| F16 | SceneViewport 所有已打开场景常驻挂载且未 memo,视口任一状态变化重渲染全部场景 | `SceneViewport.tsx:84-213` | 放大 F4 等上游重渲染的代价 | 低 |

---

## 二、逐条详情

### F1(高)流式期间 FlowChatContext 整体抖动,memo 体系失效

**问题**:`ModernFlowChatContainer` 的 `contextValue` 用 `useMemo` 构建,但依赖数组里有**整个 `activeSession` 对象**(`ModernFlowChatContainer.tsx:419` 将其放入 value,`:447` 作为依赖)。而 FlowChatStore 采用不可变更新——流式期间每次 EventBatcher flush(约 30 次/秒)都会产生新的 session 引用,经 `storeSync.ts:71-93` 同步进 `modernFlowChatStore`,于是 `contextValue` 每次 flush 都是新对象。

**放大效应**:`useFlowChatContext()` 的消费者恰好是消息渲染的全部核心组件(共 9 个文件):`VirtualItemRenderer.tsx:25`、`FlowTextBlock.tsx:71-77`、`FlowToolCard.tsx`、`ModelRoundItem.tsx`、`UserMessageItem.tsx`、`ExploreGroupRenderer.tsx`、`FileOperationToolCard.tsx`。Context 变化会**穿透 React.memo** 直达消费者,因此:

- `VirtualItemRenderer.tsx:116-119` 的 `prev.item === next.item` 比较器被架空;
- `modernFlowChatStore.ts:361-404` 用 WeakMap 为稳定回合维持引用相等的努力被架空;
- 结果是流式期间**视口内所有历史消息、工具卡、markdown 块每 32ms 全部重渲染一遍**,并连带触发 F2 的 markdown 重解析。

**用户感知**:回复越长、屏幕上消息越多,流式越卡;打字机动画掉帧。

**优化方案**(收益:高):
1. 将 FlowChatContext 拆成两个:`FlowChatActionsContext`(onFileViewRequest/onToolConfirm 等稳定回调 + sessionId/workspacePath 等原始值)与 `FlowChatVolatileContext`(searchQuery、exploreGroupStates、pendingPermissionToolCallIds)。绝大多数消费者只需前者。
2. `contextValue` 依赖里去掉 `activeSession` 对象,改为派生原始值:`activeSession?.sessionId`、`workspacePath`、`remoteConnectionId`(FlowTextBlock 实际只用到这三个标量,见 `FlowTextBlock.tsx:78-81`)。
3. `pendingPermissionToolCallIdsForSession(...)` 的结果先用 useMemo 稳定化(内容相等时复用旧 Set)再放入 context。

### F2(高)Markdown memo 被内联回调破坏 + 流式全文重解析

**问题 A(memo 破坏)**:`FlowTextBlock.tsx:188-190` 向 `MarkdownRenderer` 传了内联箭头函数 `onOpenVisualization={(visualization) => {...}}`。`Markdown`(`Markdown.tsx:785`)是**默认浅比较**的 `React.memo`,该 prop 每次渲染都是新引用 → 只要 FlowTextBlock 重渲染(F1 使其每 flush 一次),**已完成消息的 ReactMarkdown 也会整棵重渲染**,即对每条可见消息做一次完整 remark/rehype 解析(`Markdown.tsx:1411`)。

**问题 B(流式全文重解析,架构固有)**:`useTypewriter` 每 ≥16ms `setDisplayText(target.slice(0, next))`(`useTypewriter.ts:110,327`),`Markdown` 收到的 `content` 变化后由 react-markdown 在主线程同步重解析**全文**。消息长到数万字符时,单次解析可达数毫秒~十几毫秒,叠加 60fps 打字机节奏后主线程吃紧。注释(`useTypewriter.ts:108-110`)表明作者已知并用 16ms 间隔缓解,但复杂度仍是 O(全文) × 每秒最多 60 次。

**优化方案**:
1. 【低风险,先做】把 `onOpenVisualization` 包装进 `useCallback` 提升到 FlowTextBlock 顶部(或直接从 context 透传稳定引用),让已完成消息真正命中 memo。配合 F1 后,流式期间只有正在打字的那一个块重渲染。
2. 【中风险,后做】流式 markdown 分段渲染:以"最后一个未闭合块"为界,把前文(已稳定部分)与活跃尾部拆成两个 Markdown 实例,前文 content 不变即命中 memo,重解析范围从全文降到活跃尾块。`Markdown.tsx:825-838` 已有"未闭合围栏补全"逻辑可作为切分依据。

### F3(高)ChatInput 每次按键的强制布局链与单体重渲染

**证据链**(均已核对):
- 每次输入:tiptap/contenteditable → `RichTextInput.tsx:761-810` `handleInput`:TreeWalker 遍历全部文本节点做不可见字符清洗 + `extractTextContent()` 全文提取 + `querySelectorAll('[data-context-id]')`,然后 `onChange`。
- `ChatInput.tsx:2458-2475` `handleInputChange` → `dispatchInput({type:'SET_VALUE'})`:**5234 行、约 160 个 hooks 的单体组件整体重跑**(35 个 useState,渲染树含工具条、picker、上下文条等全部子树)。
- 值变化 effect(`ChatInput.tsx:696-703`)每键触发 `measureIsMultiLine`(`:605-693`):`getComputedStyle` + 4 次 `getBoundingClientRect`/`offsetWidth`(`:624-635`),随后经典写→读→写:`el.style.flex/minHeight/width` 三连写 → 读 `el.scrollHeight`(强制同步布局)→ 三连写回(再次失效布局)(`:654-660`)。
- 宽度基准 `measureCapsuleInputWidth`(`:553-575`):**cloneNode(true) 克隆整个输入容器**、appendChild 到 body、读 rect、再移除——一次测量付出整棵重复子树的样式计算 + 布局。
- 另有 MutationObserver 通路(`:708-725`)在流式/IME 场景重复驱动同一测量;`:711-716` 的 `rafId` 被多次 mutation 覆盖,同帧可排入多个测量回调,cleanup 只能取消最后一个。

**用户感知**:打字延迟(尤其长文本、IME 组合输入),流式回复进行中打字更卡(与 F1/F2 叠加)。

**优化方案**:
1. 【低风险】测量结果缓存:capsule 宽度只在容器 resize/控件变化时重测(挂 ResizeObserver),而非每键;`measureIsMultiLine` 的“是否换行”优先用轻量判定(文本无 `\n` 且长度×平均字宽 < 阈值时跳过 DOM 测量),仅在临界区间做 DOM 测量。
2. 【低风险】用常驻的隐藏测量节点(mirror div,只同步文本)替代整棵 cloneNode。
3. 【中风险】拆分 ChatInput:输入值/多行状态收敛到小组件,工具条、picker、workspace strip 等以 memo 子组件隔离,slash 命令解析改用 `useDeferredValue`。
4. 修复 MutationObserver 的 rAF 去重(有 pending 就不再排队)。

### F4(中)导航分隔条拖拽:全工作区逐 mousemove 重渲染

**问题**:`WorkspaceBody.tsx:64-91`,`handleMouseMove` 每个原始 mousemove 调 `setNavWidth` → WorkspaceBody 重渲染 → 未 memo 的 `NavPanel`、`SceneBar`、`SceneViewport` 全部重跑;而 `SceneViewport` 会重渲染**所有已打开且常驻挂载的场景**(F16),包括正在流式的会话场景。宽度经 CSS 变量落到 `:104` 的 style,本身还必然引发全局布局。另外(与事件扫描交叉确认)mousedown 注册的 window mousemove/mouseup 监听**没有 unmount 兜底清理**:拖拽中若组件卸载,监听器永久残留并持续 setState。

**优化方案**:拖拽期间不走 React——用 ref 直接写 `--nav-width` 到容器元素 style,mouseup 时才 `setNavWidth` 提交一次;mousemove 内加 rAF 合并;在 useEffect cleanup 中兜底移除 window 监听。风险低、改动小。

### F5(中)每条用户消息一个 window resize 监听 + 强制布局

`UserMessageItem.tsx:203-223`(现代视图)与 `UserMessage.tsx:145-164`(旧视图)各自为**每条**消息注册 `window.addEventListener('resize', checkOverflow)`,回调里读 `scrollHeight/clientHeight/scrollWidth/clientWidth`(4 次布局读)后 setState。长会话 N 条消息 = N 个监听 × N 次强制布局 × 最多 N 次 setState,窗口拖拽缩放时集中爆发。清理是正确的,属纯性能问题。

**优化方案**:改为对消息内容元素挂**单个共享 ResizeObserver**(observer 天然在布局后批量回调,无强制回流),或提供一个基于 `useSyncExternalStore` 的全局 resize 源 + 各组件仅在可见时测量。

### F6(中)桌面宠物:自失效 interval 与 IPC 轮询

- `AgentCompanionDesktopPet.tsx:308-333`:打字机 interval(28ms)的 effect 依赖 `[typedOutputBySessionId]`,而 interval 每 tick 都会 set 该状态 → **每 tick 拆除并重建 interval**(约 36 次/秒的 effect 抖动),节奏相位也随之抖动。
- `:490-501` + `:471-481`:120ms 轮询做 Tauri IPC(光标/窗口几何)+ `querySelectorAll` + 逐气泡 `getBoundingClientRect` + 两次 setState,空闲时也满速运行。

**优化方案**:打字机改为 rAF + ref 目标值(仿 `useTypewriter.ts` 的成熟实现),依赖数组不含被写状态;hover 轮询在无气泡/宠物 idle 时暂停,或改由 pointerenter/leave 事件驱动。

### F7(中)输入框 backdrop-filter 与布局属性 transition 叠加

`ChatInput.scss:632-640`:`.openbitfun-chat-input__box` 同时有 `backdrop-filter: blur(16px) saturate(1.2)` 和对 `padding/min-height/max-height/box-shadow` 的 0.32s transition。胶囊↔多行切换(由 F3 的测量高频驱动)期间每帧:重排(布局属性)+ 16px 背景重采样 + box-shadow 重绘。嵌套层另有 blur(8/12/20px)(`:659,668,689,1137`)。

**优化方案**:transition 收窄为 `border-radius, border-color, box-shadow`;高度变化改用容器 `grid-template-rows`/transform 方案或接受瞬时切换;评估把模糊降级为半透明底色(至少在低端机/省电模式)。

### F8(中)工具卡退出动画:1 秒 max-height/margin 逐帧重排

`ModelRoundItem.scss:404,424-445`:`flow-tool-completed-exit` 关键帧动画 `max-height: 96px→0`、`margin-bottom→0`,时长 **1000ms**——消息列表内连续 1 秒每帧重排,流式时多卡并发退出;还会触发 VirtualMessageList 的折叠补偿链(F11)。`:396` 的 `will-change: opacity, transform, max-height` 中 max-height 不可合成,该声明只带来图层内存开销。

**优化方案**:退出动画拆两段——先 opacity/transform(合成器)200ms,再一次性(或用 `SmoothHeightCollapse` 的一次测量 + height 过渡,但缩短时长)收起高度;移除 will-change 中的 max-height;总时长压到 ≤300ms 以缩短逐帧重排窗口。

### F9（已关闭）鼠标跟随光效已退役

该功能及其全局指针监听、覆盖层和样式已删除，无后续优化任务。

### F10(中)zustand 全量订阅点

- `canvasStore.ts:1299-1320`:`useCanvasStore()` 无 selector 时退化为 `state => state`,且实现上**同时订阅 5 个作用域 store**(agent/project/git/panel-view/bottom-terminal),任一 store 任意字段变化都重渲染调用组件。无 selector 调用点:`ContentCanvas.tsx`、`EditorArea.tsx`、`MissionControl.tsx`、`AuxPane.tsx`——都是编辑区/面板级大组件。
- `UserMessageItem.tsx:100-108`:每条消息 `useMessageEditStore()` 全量解构,任何一条消息进入编辑态(draft 每键变化)会使**列表中所有** UserMessageItem 重渲染。

**优化方案**:上述调用点改为细粒度 selector(编辑态可先用 `useMessageEditStore(s => s.editingTurnId === turnId)` 门控,再在编辑中的那一条内部订阅 draft);为 `useCanvasStore` 增加 ESLint 约束或移除无 selector 重载。

### F11(中,高风险区)VirtualMessageList 滚动补偿的强制回流

`VirtualMessageList.tsx:2255-2350` 的 `handleScroll`(passive,未 rAF 节流)在补偿激活时同步执行:读 scrollTop → `applyFooterCompensationNow`(`:795-808`:写 footer height/minHeight 后**刻意** `void footer.offsetHeight; void scroller.scrollHeight` 强制回流)→ `snapshotMeasuredContentHeight` 再读 → 可能再写 scrollTop。一次滚动事件可触发 ≥2 次全量布局,对象是全应用最大子树;流式时还与 60fps follow 循环的 scrollTop 写交错。

**必须强调**:该文件是精心调校的滚动稳定性机器(见同目录 `FLOWCHAT_SCROLL_STABILITY.md`),补偿路径仅在工具卡折叠/pin 保留期激活,强制回流是有意为之的正确性手段。**不建议重构逻辑**,只建议微手术:① 把 `:806-807` 的双读合并为单次(读一个即可使布局生效);② handleScroll 内多次 `scrollerElement.scrollTop/scrollHeight` 读取合并为函数开头一次快照;③ 配合 F8 缩短折叠动画时长,直接减少补偿路径的激活时间窗。改动前后需人工回归滚动跟随/折叠补偿场景。

### F12(低-中)未节流滚动回调与循环内 rect 读取

- `FileExplorer.tsx:63-107` `detectCurrentDirectory`:每个 scroll 事件 `querySelectorAll` 展开目录 + 每节点 2 次 `getBoundingClientRect`(O(2N)),后 setState。
- `InsightsScene.tsx:472-490`:每个 scroll 事件对全部 `[data-section]` 读 rect 后 `setActiveSection`。
- `ExploreGroupRenderer.tsx:95-107,272`:容器 `onScroll={checkScrollState}` 直接 setState(每次新对象,恒触发重渲染),回调内读 scrollHeight/clientHeight。

对照:`useAutoScroll.ts:67-73`、`useVisibleTaskInfo.ts:148`、`useScrollToTurnHeader.ts:113` 都已 rAF 节流——这三处是遗漏。**方案**:统一加 rAF 合并;ExploreGroupRenderer 的 setState 加浅比较短路;FileExplorer 可改 IntersectionObserver。

### F13(低-中)`transition: all` + 布局属性 hover 动画

- `app/styles/utilities/resizer.css:15,24,38,56,62,70`:分隔条 hover/dragging 过渡 width/height,`transition: all` 连带 box-shadow(`:30-33`)一起动画;分隔条是 flex 子元素,width 过渡使相邻面板整个 motion 时长内反复重排。
- `SessionScene.scss:183,206,236-237,324,357`:同模式(该文件 `:76/102/107/311` 已用 `--dragging/--no-animation` 关闭拖拽中的子树动画,是正确做法,但 hover 路径未覆盖)。

**方案**:`transition: all` 改为显式列出 `opacity, background-color`;宽度反馈改用 `transform: scaleX` 或伪元素覆盖层。

### F14(低)常驻无限动画

- `FlexiblePanel.scss:215-240,494,558-559`:`needs-fix` 态 6 层 box-shadow(含 color-mix)3s infinite 呼吸,且 `:559` 的 `transition: box-shadow` 与 animation 争抢同一属性——一旦出现即永久逐帧重绘。改为动画化伪元素的 opacity。
- `AgentCompanionPet.scss:114,133,148-151` 等 15+ 处 infinite:SVG `filter: url(#...)` 宿主上跑永不停的 transform 动画,每帧重跑滤镜图;`--working` 态 0.36s 无限抖动更甚。建议 idle 降频/暂停(页面 hidden 或宠物不可见时移除动画类),暗色主题描边改用预渲染素材。

### F15(低)监听器清理缺口(修复清单)

1. `GitStateManager.ts:252-266` 的 `dispose()` 未摘除 `:744-753` 注册的 `window focus` 与 `gitEventService` 监听;`resetInstance()` 每次调用净增一套活监听。
2. `PeerHostInvokeBridge.tsx:75-112`:`unlisten = await listen(...)` 在 fire-and-forget IIFE 中,cleanup 早于 resolve(StrictMode 首挂载必现)时注册永不移除。修法参照 `AgentCompanionDesktopPet.tsx:190-196`(resolve 后若已 disposed 立即调用)。
3. `useWindowControls.ts:164-210`:同类 await-before-assign 竞态,泄漏的 onResized 回调还持有 300ms debounce 定时器继续对已卸载树 setState。
4. `WorkspaceAPI.ts:471-478`:`signal.addEventListener('abort', ...)` 在 race 胜出后未移除(同文件 `:555-559` 已有正确模式可对齐);当前调用方每次新建 controller,属潜在而非活跃泄漏。

### F16(低)SceneViewport 常驻场景的连带重渲染

`SceneViewport.tsx:170-209`:所有已打开场景保持挂载(为保状态,合理),但 `renderScene` 内联调用且场景组件未 memo——SceneViewport 自身任何 state 变化(transition/readyVersion)及父级重渲染(F4 拖拽时逐 mousemove)都会重跑**全部**场景子树。**方案**:`renderScene` 结果按 tabId useMemo 化,或把每个场景包一层 `React.memo` 的 SceneHost(props 仅 tabId/isActive/workspacePath 等标量)。

---

## 三、实施建议清单(可直接分派)

> 约定:每个任务附验收标准;风险=改动引入回归的可能性。改动前后建议用 React DevTools Profiler + Performance 面板录制"流式长回复 + 打字 + 滚动"同一场景对比。

| 任务 | 内容 | 涉及文件 | 风险 |
|---|---|---|---|
| T1 | **拆分/稳定化 FlowChatContext**:contextValue 依赖去掉 `activeSession` 对象,改传 `sessionId/workspacePath/remoteConnectionId` 标量;将 volatile 字段(searchQuery、exploreGroupStates、pendingPermissionToolCallIds)拆到独立 context;`pendingPermissionToolCallIds` 内容相等时复用旧 Set。验收:流式期间 Profiler 中非活跃回合的 VirtualItemRenderer 不再重渲染。 | `ModernFlowChatContainer.tsx:406-458`、`FlowChatContext.tsx`、`BtwSessionPanel.tsx:277`、9 个消费组件 | 中(需核对每个消费者取用字段) |
| T2 | **稳定化 FlowTextBlock→Markdown 回调**:`onOpenVisualization` 提为 useCallback;核对其余 props 引用稳定性。验收:流式时已完成消息的 `Markdown` 组件零重渲染(Profiler)。 | `FlowTextBlock.tsx:188-190` | 低 |
| T3 | **ChatInput 测量降频**:capsule 宽度改 ResizeObserver 缓存;cloneNode 测量改常驻 mirror 节点;`measureIsMultiLine` 增加无 DOM 快速路径;修 MutationObserver rAF 去重。验收:单次按键 Performance 录制中 Layout 次数 ≤1。 | `ChatInput.tsx:553-575,605-693,696-725` | 中(多行/胶囊切换需回归,含 IME) |
| T4 | **ChatInput 组件拆分**(T3 之后评估):输入核心与工具条/picker/strip 分离 memo,slash 解析 useDeferredValue。 | `ChatInput.tsx` 全文 | 中-高(建议单独立项) |
| T5 | **导航拖拽直写 CSS 变量**:mousemove 内 ref 直写 `--nav-width` + rAF 合并,mouseup 提交 setState;useEffect cleanup 兜底移除 window 监听。验收:拖拽期间 React 无 commit。 | `WorkspaceBody.tsx:53-91` | 低 |
| T6 | **用户消息溢出检测共享化**:两处 per-message window resize 改共享 ResizeObserver。 | `UserMessageItem.tsx:203-223`、`UserMessage.tsx:145-164` | 低 |
| T7 | **宠物打字机与轮询修复**:interval 改 rAF+ref(依赖不含被写状态);120ms hover 轮询按需启停。 | `AgentCompanionDesktopPet.tsx:308-333,471-501` | 低 |
| T8 | **输入框 transition 收窄**:`ChatInput.scss:634-640` 只保留 border-radius/border-color/box-shadow;评估嵌套 blur 层合并。 | `ChatInput.scss:632-640,659,668,689,1137` | 低(视觉验收) |
| T9 | **工具卡退出动画重做**:改 opacity/transform 主导 + 缩短高度收起窗口至 ≤300ms;移除 `will-change` 中的 max-height;顺带清理 `SceneBar.scss:80`、`SmoothHeightCollapse.scss:9` 的布局属性 will-change。 | `ModelRoundItem.scss:394-445`、`SceneBar.scss:66-80`、`SmoothHeightCollapse.scss` | 中(与 F11 补偿逻辑联动,需回归滚动) |
| T10（已关闭） | 历史鼠标跟随光效优化任务；功能已退役，不得实施或恢复。 | 已删除 | 不适用 |
| T11 | **zustand selector 化**:4 处 `useCanvasStore()` 与 `UserMessageItem` 的 `useMessageEditStore()` 改细粒度 selector(编辑态用 turnId 门控)。 | `ContentCanvas.tsx`、`EditorArea.tsx`、`MissionControl.tsx`、`AuxPane.tsx`、`UserMessageItem.tsx:100-108` | 低 |
| T12 | **滚动回调节流**:三处加 rAF 合并 + setState 浅比较短路;FileExplorer 评估 IntersectionObserver。 | `FileExplorer.tsx:63-107`、`InsightsScene.tsx:472-490`、`ExploreGroupRenderer.tsx:95-107,272` | 低 |
| T13 | **transition:all 清理**:分隔条与 SessionScene 显式属性列表,宽度反馈改 transform/伪元素。 | `resizer.css`、`SessionScene.scss` | 低 |
| T14 | **常驻动画整治**:FlexiblePanel 呼吸灯改伪元素 opacity;宠物 idle 降频、hidden 时暂停;删除 `animations.css` 死代码(`.anim-glass-shine` 等,TSX 零引用)。 | `FlexiblePanel.scss:215-240,558-559`、`AgentCompanionPet.scss`、`animations.css:193,293` | 低 |
| T15 | **监听器清理修复**:GitStateManager dispose 补摘除;两处 await 竞态改 resolve-后判 disposed;WorkspaceAPI abort 监听对齐 `:555-559` 模式。 | `GitStateManager.ts`、`PeerHostInvokeBridge.tsx`、`useWindowControls.ts`、`WorkspaceAPI.ts` | 低 |
| T16 | **SceneViewport 场景 memo 化**:renderScene 按 tabId 缓存或 SceneHost memo 包装。 | `SceneViewport.tsx:170-209` | 低-中(确认场景 props 均为标量) |
| T17 | 【谨慎,最后做】**VirtualMessageList 读写微整**:`applyFooterCompensationNow` 双读并一、handleScroll 几何读取快照化。**不得改动补偿/跟随逻辑本身**,改后人工回归 `FLOWCHAT_SCROLL_STABILITY.md` 所列场景。 | `VirtualMessageList.tsx:795-808,2255-2350` | 高 |

**建议实施顺序**:T2 → T1 → T5/T6/T7/T11/T12/T15(低风险批)→ T3 → T8/T9/T13/T14 → T16 → T4 → T17。T1+T2 是收益最大的组合拳,预计流式期间的 React commit 范围可从"整屏消息"缩小到"单个活跃文本块"。
