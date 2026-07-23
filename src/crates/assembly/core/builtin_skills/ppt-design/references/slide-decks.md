# Slide Decks：HTML幻灯片制作规范

做幻灯片是设计工作的高频场景。这份文档说明怎么做好 HTML 幻灯片——架构选型、单页设计、交付目标对 HTML 的约束。

**能力覆盖**：
- **HTML 演示版（默认必做）** → 每页独立 HTML + `deck_index.html` 聚合，浏览器全屏翻页
- **可编辑 PPTX（PPT Live 默认）** → 每页严格 1280px × 720px，并从第一行遵守 `editable-pptx.md` 的 editable-only authoring contract
- **高保真演讲版** → 1920×1080px，视觉自由；与可编辑 pptx 不可混为同一套 HTML

> **HTML 是源。** 先做好 `slides/*.html`（+ 可选聚合 `index.html`），再谈文件格式。每页可单独打开验证；改内容只改 HTML。

---

## 🛑 开工前先确认交付格式（最硬的 checkpoint）

**这个决策比「单文件还是多文件」更先。** 2026-04-20 期权私董会项目实测：**不在动手前确认交付格式 = 2-3 小时返工。**

### 决策树（HTML-first 架构）

所有交付都从同一套 HTML（`slides/*.html`，可选 `index.html` 聚合）开始。先确认交付目标，再决定 **HTML 写法**：

```
【永远默认 · 必做】 HTML 聚合演示版（index.html + slides/*.html）
   │
   ├── 只要浏览器演讲 / 本地 HTML 存档   → 到这里已经完成，HTML 视觉自由度最大
   │
   ├── 还要 PDF（打印 / 发群 / 存档）     → HTML 写法自由（1920 演讲版即可）
   │
   └── 还要可编辑 PPTX（同事要改文字）    → 从第一行就按 1280×720 editable-only 契约
                                              只使用可映射的 text / shape / line / table / intentional image
```

### 需求确认（动手前一句）

> 我会先做可在浏览器里翻页的 HTML 幻灯片。请确认：**同事是否要在 PowerPoint 里改文字？**
> - **否**（演讲/存档为主）→ 可用 1920 版，视觉自由
> - **是** → 全程 1280px × 720px + editable-only authoring contract

### 为什么「要 PPTX 就得从头走 4 条硬约束」

PPTX 可编辑的前提是 PPT Live 把 DOM 规范化为 `EditableSlideScene`，再序列化为 OOXML。唯一链路是 **editable HTML → EditableSlideScene → OOXML**，没有视觉保底或降级成功路径：

1. body 固定 1280px × 720px（匹配 `LAYOUT_WIDE`，13.333″ × 7.5″）
2. 所有文字包在 `<p>`/`<h1>`-`<h6>` 里（禁止 div 直接放文字，禁止用 `<span>` 承载主文字）
3. `<p>`/`<h*>` 自身不能有 background/border/shadow（放外层 div）
4. `<div>` 不能用 `background-image`（用 `<img>` 标签）
5. authoring 只使用 solid color 与受支持的 SVG/CSS 原语；无法表示时停止生成并报告具体元素

**本 skill 默认的 HTML 视觉自由度高**——大量 span、嵌套 flex、复杂 SVG、web component（如 `<deck-stage>`）、CSS 渐变——**几乎没有一条能天然过 html2pptx 的约束**（实测视觉驱动的 HTML 直接上 html2pptx，pass 率 < 30%）。

### 两条真实路径的代价对比（2026-04-20 真实踩坑）

| 路径 | 做法 | 结果 | 代价 |
|------|------|------|------|
| ❌ **先自由写 HTML，事后补救 PPTX** | 单文件 deck-stage + 大量 SVG/span 装饰 | 要可编辑 PPTX 只剩两条路：<br>A. 手写 pptxgenjs 几百行 hardcode 坐标<br>B. 重写 17 页 HTML 成 Path A 格式 | 2-3 小时返工，且手写版**维护成本永续**（HTML 改一个字，PPTX 要再人肉同步） |
| ✅ **从第一步按 editable-only 约束写** | 每页独立 1280×720 HTML + 完整 authoring contract | 可转可编辑 PPTX，也能浏览器演讲 | 多花几分钟使用可映射原语，零返工 |

### 混合交付怎么办

用户说「我要 HTML 演讲 **和** 可编辑 PPTX」——**这不是混合**，是 PPTX 需求覆盖 HTML 需求。按 Path A 写出来的 HTML 本身就能浏览器全屏演讲（加个 `deck_index.html` 拼接器就行）。**没有额外代价。**

用户说「我要 PPTX **和** 动画 / web component」——**这是真矛盾**。告诉用户：要可编辑 PPTX 就得牺牲这些视觉能力。让他做取舍，不要偷偷做手写 pptxgenjs 方案（会变成永续维护债）。

### 事后才知道要 PPTX 怎么办

若既有 HTML 不符合 editable-only contract，必须以原稿为内容与视觉参考，重写为 1280×720 的受支持 text、shape、line、table 与 intentional image 组合。任何元素无法无损映射时，停止导出并报告页码与源元素；禁止截图、栅格化、静默丢失或标记为降级成功。

---

## 🛑 批量制作前：先做 2 页 showcase 定 grammar

**只要 deck ≥ 5 页，绝对不能从第 1 页直接写到最后一页。** 2026-04-22 moxt brochure 实战验证的正确顺序：

1. 选 **2 个视觉差异最大的页面类型**先做 showcase（如「封面」+「情绪/引用页」，或「封面」+「产品展示页」）
2. 截图让用户确认 grammar（masthead / 字体 / 色 / 间距 / 结构 / 中英双语比例）
3. 方向通过了再批量推剩下 N-2 页，每页复用已建立的 grammar
4. 全部完成后合成 `index.html` 聚合（可选）

**为什么**：直接写 13 页到底 → 用户说「方向不对」= 返工 13 次。先做 2 页 showcase → 方向错 = 返工 2 次。视觉 grammar 一旦确立，后续 N 页的决策空间大幅收窄，只剩「内容怎么放进去」。

**showcase 页选择原则**：选视觉结构最不一样的两页。这两页过了 = 其他中间态都能过。

| Deck 类型 | 推荐 showcase 页组合 |
|-----------|---------------------|
| B2B brochure / 产品宣发 | 封面 + 内容页（理念/情感页） |
| 品牌发布 | 封面 + 产品特色页 |
| 数据报告 | 数据大图页 + 分析结论页 |
| 教程课件 | 章节封页 + 具体知识点页 |

---

## 📐 出版物 grammar 模板（moxt 实测可复用）

适合 B2B brochure / 产品宣发 / 长报告类 deck。每页复用这套结构 = 13 页视觉完全一致、0 返工。

### 每页骨架

```
┌─ masthead（顶部 strip + 横线）────────────┐
│  [logo 22-28px] · A Product Brochure                Issue · Date · URL │
├──────────────────────────────────────────┤
│                                          │
│  ── kicker（绿色短横 + uppercase 标签）   │
│  CHAPTER XX · SECTION NAME                 │
│                                          │
│  H1（中文 Noto Serif SC 900）             │
│  重点词单独上品牌主色                      │
│                                          │
│  English subtitle (Lora italic，副标题)   │
│  ─────────── 分隔线 ──────────            │
│                                          │
│  [具体内容：双栏 60/40 / 2x2 grid / 列表] │
│                                          │
├──────────────────────────────────────────┤
│ section name                     XX / total │
└──────────────────────────────────────────┘
```

### 样式约定（直接抄走）

- **H1**：中文 Noto Serif SC 900，字号 80-140px 看信息量，重点词单独上品牌主色（不要全文堆色）
- **英文副**：Lora italic 26-46px，品牌签名词（如 "AI team"）粗体 + 主色斜体
- **正文**：Noto Serif SC 17-21px，line-height 1.75-1.85
- **accent 高亮**：正文里用主色加粗标注关键词，每页不超过 3 处（过多就失去锚点作用）
- **背景**：暖米底 #FAFAFA + 极淡 radial-gradient noise（`rgba(33,33,33,0.015)`）增加纸感

### 视觉主角必须差异化

13 页如果全是「文字 + 一张截图」就太单调。**每页的视觉主角类型轮换**：

| 视觉类型 | 适合的 section |
|---------|---------------|
| 封面排版（大字 + masthead + pillar） | 首页 / 篇章封 |
| 单角色 portrait（超大单只 momo 等） | 介绍单个概念/角色 |
| 多角色合影 / 头像卡并排 | 团队 / 用户案例 |
| 时间轴卡片递进 | 展示「长期关系」「演进」 |
| 知识图谱 / 连接节点图 | 展示「协作」「流动」 |
| Before/After 对比卡 + 中间箭头 | 展示「改变」「差异」 |
| 产品 UI 截图 + 描边设备框 | 具体功能展示 |
| 大引号 big-quote（半页大字） | 情绪页 / 问题页 / 引文页 |
| 真人头像 + 引言卡（2×2 或 1×4） | 用户见证 / 使用场景 |
| 大字封底 + URL 椭圆按钮 | CTA / 结尾 |

---

## ⚠️ 常见踩坑（moxt 实战总结）

### 1. Emoji 在 PDF/PNG 导出时不渲染

导出为 PDF/PNG 时，彩色 emoji 常显示为空方框。

**对策**：用 Unicode 文字符号（`✦` `✓` `✕` `→` `·` `—`）替代，或直接改纯文字（「Email · 23」而不是「📧 23 emails」）。

### 2. Google Fonts 未加载完 → 中文落回系统黑体

建议 `@font-face` 本地路径或 `shared/fonts/` self-host，减少网络依赖。

### 4. 信息密度失衡：内容页塞太多

moxt philosophy 页第一版用 2×2 = 4 段 + 底部 3 信条 = 7 块内容，挤压且重复。改成 1×3 = 3 段后呼吸感立刻回来。

**对策**：每页控制在「1 个核心信息 + 3-4 个辅助点 + 1 个视觉主角」，超过就拆到新页。**少即是多**——观众一页看 10 秒，给他 1 个记忆点比 4 个记忆点更容易记住。

---

## 🛑 先定架构：单文件 还是 多文件？

**这个选择是做幻灯片的第一步，错了会反复踩坑。先读完这一节再动手。**

### 两种架构对比

| 维度 | 单文件 + `deck_stage.js` | **多文件 + `deck_index.html` 拼接器** |
|------|--------------------------|--------------------------------------|
| 代码结构 | 一个 HTML，所有 slide 是 `<section>` | 每页独立 HTML，`index.html` 用 iframe 拼接 |
| CSS 作用域 | ❌ 全局，一页的样式可能影响所有页 | ✅ 天然隔离，iframe 各自一片天 |
| 验证粒度 | ❌ 要 JS goTo 才能切到某页 | ✅ 单页文件双击就能在浏览器看 |
| 并行开发 | ❌ 一个文件，多 agent 改会冲突 | ✅ 多 agent 可并行做不同页，零冲突 merge |
| 调试难度 | ❌ 一处 CSS 出错，全 deck 翻车 | ✅ 一页出错只影响自己 |
| 内嵌交互 | ✅ 跨页共享状态很简单 | 🟡 iframe 间需 postMessage |
| 打印 PDF | ✅ 内置 | ✅ 拼接器 beforeprint 遍历 iframe |
| 键盘导航 | ✅ 内置 | ✅ 拼接器内置 |

### 选哪个？（决策树）

```
│ 问：deck 预计有多少页？
├── ≤10 页、需要 in-deck 动画或跨页交互、pitch deck → 单文件
└── ≥10 页、学术讲座、课件、长 deck、多 agent 并行 → 多文件（推荐）
```

**默认走多文件路径**。它不是「备选」，是**长 deck 和团队协作的主路径**。原因：单文件架构的每一个优势（键盘导航、打印、scale）多文件都有，而多文件的作用域隔离和可验证性是单文件补不回来的。

### 为什么这条规则这么硬？（真实事故记录）

单文件架构曾经在 AI心理学讲座 deck 制作中连踩四坑：

1. **CSS 特异性覆盖**：`.emotion-slide { display: grid }` (特异性 10) 干翻 `deck-stage > section { display: none }` (特异性 2)，导致所有页同时渲染叠加。
2. **Shadow DOM slot 规则被外层 CSS 压制**：`::slotted(section) { display: none }` 挡不住 outer rule 的覆盖，sections 不肯隐藏。
3. **localStorage + hash 导航竞态**：刷新后不是跳到 hash 位置，而是停在 localStorage 记录的旧位置。
4. **验证成本高**：必须 `page.evaluate(d => d.goTo(n))` 才能截某页，比直接 `goto(file://.../slides/05-X.html)` 慢一倍，还常报错。

全部根因是**单一全局命名空间**——多文件架构从物理层面把这些问题消除了。

---

## 路径 A（默认）：多文件架构

### 目录结构

```
我的Deck/
├── index.html              # 从 assets/deck_index.html 复制来，改 MANIFEST
├── shared/
│   ├── tokens.css          # 共享设计 token（色板/字号/常用 chrome）
│   └── fonts.html          # <link> 引入 Google Fonts（每页 include）
└── slides/
    ├── 01-cover.html       # 每个文件都是完整 1920×1080 HTML
    ├── 02-agenda.html
    ├── 03-problem.html
    └── ...
```

### 每张 slide 的模板骨架

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>P05 · Chapter Title</title>
<link href="https://fonts.googleapis.com/css2?family=..." rel="stylesheet">
<link rel="stylesheet" href="../shared/tokens.css">
<style>
  /* 这一页独有的样式。用任何 class 名都不会污染别的页。*/
  body { padding: 120px; }
  .my-thing { ... }
</style>
</head>
<body>
  <!-- 1920×1080 的内容（由 body 的 width/height 在 tokens.css 里锁定）-->
  <div class="page-header">...</div>
  <div>...</div>
  <div class="page-footer">...</div>
</body>
</html>
```

**关键约束**：
- `<body>` 就是画布，直接在上面布局。不要包 `<section>` 或其他 wrapper。
- `width: 1920px; height: 1080px` 由 `shared/tokens.css` 里的 `body` 规则锁定。
- 引 `shared/tokens.css` 共享设计 token（色板、字号、page-header/footer 等）。
- 字体 `<link>` 每页自己写（fonts 单独 import 不贵，且保证每页独立可打开）。

### 拼接器：`deck_index.html`

**直接从 `assets/deck_index.html` 复制**。你只需要改一处——`window.DECK_MANIFEST` 数组，按顺序列出所有 slide 文件名和人类可读标签：

```js
window.DECK_MANIFEST = [
  { file: "slides/01-cover.html",    label: "封面" },
  { file: "slides/02-agenda.html",   label: "目录" },
  { file: "slides/03-problem.html",  label: "问题陈述" },
  // ...
];
```

拼接器已内置：键盘导航（←/→/Home/End/数字键/P 打印）、scale + letterbox、右下计数器、localStorage 记忆、hash 跳页、打印模式（遍历 iframe 按页输出 PDF）。

### 单页验证（这是多文件架构的杀手级优势）

每张 slide 都是独立 HTML。**做完一张就在浏览器双击打开看**：

```bash
open slides/05-personas.html
```

单页路径独立，不会被其他页的 CSS 污染——「改一点、打开这一页看」成本接近零。

### 并行开发

把每张 slide 的任务拆给不同 agent，同时跑——HTML 文件彼此独立，merge 时没有冲突。长 deck 用这种并行方式能把制作时间压到 1/N。

### `shared/tokens.css` 该放什么

只放**真正跨页共用**的东西：

- CSS 变量（色板、字号阶、间距阶）
- `body { width: 1920px; height: 1080px; }` 这样的 canvas 锁定
- `.page-header` / `.page-footer` 这种每页都用一模一样的 chrome

**不要**把单页的布局 class 塞进来——那会退化回单文件架构的全局污染问题。

---

## 路径 B（小 deck）：单文件 + `deck_stage.js`

适用于 ≤10 页、需要跨页共享状态（比如一个 React tweaks 面板要操控所有页）、或者做 pitch deck demo 这种要求极度紧凑的场景。

### 基本用法

1. 从 `assets/deck_stage.js` 读取内容，嵌入 HTML 的 `<script>`（或 `<script src="deck_stage.js">`）
2. 在 body 里用 `<deck-stage>` 包 slide
3. 🛑 **script 标签必须放在 `</deck-stage>` 之后**（见下方硬约束）

```html
<body>

  <deck-stage>
    <section>
      <h1>Slide 1</h1>
    </section>
    <section>
      <h1>Slide 2</h1>
    </section>
  </deck-stage>

  <!-- ✅ 正确：script 在 deck-stage 之后 -->
  <script src="deck_stage.js"></script>

</body>
```

### 🛑 Script 位置硬约束（2026-04-20 真实踩坑）

**不能把 `<script src="deck_stage.js">` 放在 `<head>` 里。** 即使它在 `<head>` 里能定义 `customElements`，parser 在解析到 `<deck-stage>` 开始标签时就会触发 `connectedCallback`——此时子 `<section>` 还没被 parse，`_collectSlides()` 拿到空数组，counter 显示 `1 / 0`，所有页同时叠加渲染。

**三条合规写法**（任选其一）：

```html
<!-- ✅ 最推荐：script 在 </deck-stage> 之后 -->
</deck-stage>
<script src="deck_stage.js"></script>

<!-- ✅ 也可：script 在 head 但加 defer -->
<head><script src="deck_stage.js" defer></script></head>

<!-- ✅ 也可：module 脚本天然 defer -->
<head><script src="deck_stage.js" type="module"></script></head>
```

`deck_stage.js` 本身已内置 `DOMContentLoaded` 延迟收集防御，即使 script 放 head 也不会彻底炸掉——但 `defer` 或放 body 底部仍然是更干净的做法，避免依赖防御分支。

### ⚠️ 单文件架构的 CSS 陷阱（务必阅读）

单文件架构最常见的坑——**`display` 属性被单页样式偷走**。

常见错误姿势 1（直接写 display: flex 到 section）：

```css
/* ❌ 外部 CSS 特异性 2，覆盖了 shadow DOM 的 ::slotted(section){display:none}（也是 2）*/
deck-stage > section {
  display: flex;            /* 所有页会同时叠加渲染！ */
  flex-direction: column;
  padding: 80px;
  ...
}
```

常见错误姿势 2（section 有特异性更高的 class）：

```css
.emotion-slide { display: grid; }   /* 特异性: 10，更糟 */
```

两种都会让 **所有 slide 同时叠加渲染**——counter 可能显示 `1 / 10` 假装正常，但视觉上第一页盖着第二页盖着第三页。

### ✅ Starter CSS（开工直接 copy，不踩坑）

**section 自身**只管「可见/不可见」；**layout（flex/grid 等）写到 `.active` 上**：

```css
/* section 只定义非 display 的通用样式 */
deck-stage > section {
  background: var(--paper);
  padding: 80px 120px;
  overflow: hidden;
  position: relative;
  /* ⚠️ 不要在这里写 display! */
}

/* 锁死「非激活即隐藏」——特异性+权重双保险 */
deck-stage > section:not(.active) {
  display: none !important;
}

/* 激活页才写需要的 display + layout */
deck-stage > section.active {
  display: flex;
  flex-direction: column;
  justify-content: center;
}

/* 打印模式：所有页都要显示，覆盖 :not(.active) */
@media print {
  deck-stage > section { display: flex !important; }
  deck-stage > section:not(.active) { display: flex !important; }
}
```

替代方案：**把单页的 flex/grid 写到内部 wrapper `<div>` 上**，section 本身永远只是 `display: block/none` 的切换器。这是最干净的做法：

```html
<deck-stage>
  <section>
    <div class="slide-content flex-layout">...</div>
  </section>
</deck-stage>
```

### 自定义尺寸

```html
<deck-stage width="1080" height="1920">
  <!-- 9:16 竖版 -->
</deck-stage>
```

---

## Slide Labels

Deck_stage 和 deck_index 都会给每页打标签（计数器显示）。给它们**更有意义**的 label：

**多文件**：在 `MANIFEST` 里写 `{ file, label: "04 问题陈述" }`
**单文件**：在 section 上加 `<section data-screen-label="04 Problem Statement">`

**关键：Slide 编号从 1 开始，不要从 0**。

用户说"slide 5"时，他指的是第 5 张，永远不是数组位置 `[4]`。人类不说 0-indexed。

---

## Speaker Notes

**默认不加**，只在用户明确要求时才加。

加了 speaker notes 你就可以把 slide 上的文字减少到最小，focus on impactful visuals——notes 承载完整 script。

### 格式

**多文件**：在 `index.html` 的 `<head>` 里写：

```html
<script type="application/json" id="speaker-notes">
[
  "第1张的 script...",
  "第2张的 script...",
  "..."
]
</script>
```

**单文件**：同上位置。

### Notes 写作要点

- **完整**：不是提纲，是真要讲的话
- **对话式**：像平时说话，不是书面语
- **对应**：数组第 N 个对应第 N 张 slide
- **长度**：200-400 字最佳
- **情绪线**：标注重音、停顿、强调点

---

## Slide 设计模式

### 1. 建立一个系统（必做）

探索完 design context 后，**先口头说你要用的系统**：

```markdown
Deck系统：
- 背景色：最多2种（90% 白 + 10% 深色 section divider）
- 字型：display 用 Instrument Serif，body 用 Geist Sans
- 节奏：section divider 用 full-bleed 彩色 + 白字，普通 slide 白底
- 图像：hero slide 用 full-bleed 照片，data slide 用 chart

我按这个系统做，有问题告诉我。
```

用户确认后再往下做。

### 2. 常用 slide layouts

- **Title slide**：纯色背景 + 巨大标题 + 副标题 + 作者/日期
- **Section divider**：彩色背景 + 章节号 + 章节标题
- **Content slide**：白底 + 标题 + 1-3 bullet points
- **Data slide**：标题 + 大图表/数字 + 简短说明
- **Image slide**：full-bleed 照片 + 底部小 caption
- **Quote slide**：留白 + 巨大 quote + attribution
- **Two-column**：左右对比（vs / before-after / problem-solution）

一个 deck 里最多用 4-5 种 layout。

### 3. Scale（再次强调）

- 正文最小 **24px**，理想 28-36px
- 标题 **60-120px**
- Hero 字 **180-240px**
- 幻灯片是给 10 米外看的，字要够大

### 4. 视觉节奏

Deck 需要 **intentional variety**：

- 颜色节奏：大部分白底 + 偶尔彩色 section divider + 偶尔 dark 片段
- 密度节奏：几张 text-heavy 的 + 几张 image-heavy 的 + 几张 quote 留白
- 字号节奏：正常标题 + 偶尔巨型 hero 文字

**不要每张 slide 长一样**——那是 PPT 模板，不是设计。

### 5. 空间呼吸（数据密集页必读）

**新手最容易踩的坑**：把所有能放的信息都塞进一页。

信息密度 ≠ 有效信息传达。学术/演讲类 deck 尤其要克制：

- 列表/矩阵页：不要把 N 个元素都画成同一大小。用 **主次分层**——今天要聊的 5 个放大做主角，剩下 16 个缩小做背景 hint。
- 大数字页：数字本身是视觉主角。周围的 caption 不要超过 3 行，否则观众眼球来回跳。
- 引用页：引语和 attribution 之间要有留白隔开，不要贴在一起。

对照「数据是不是主角」「文字有没有挤在一起」两条自我审查，改到留白让你有点不安为止。

---

## 打印为 PDF

**多文件**：`deck_index.html` 已处理 `beforeprint` 事件，按页输出 PDF。

**单文件**：`deck_stage.js` 同样处理。

打印样式已写好，不需要额外写 `@media print` CSS。

---

## 可编辑 PPTX：HTML 硬性约束

要在 PowerPoint 里改字时，HTML 须满足下列约束（详见 `references/editable-pptx.md`）：
- 每页严格为 **1280px × 720px**，唯一导出链路为 **editable HTML → EditableSlideScene → OOXML**
- 所有文字必须在 `<p>`/`<h1>`-`<h6>`/`<ul>`/`<ol>` 里（禁止裸文本 div）
- `<p>`/`<h*>` 标签自身不能有 background/border/shadow（放外层 div）
- 不用 `::before`/`::after` 插入装饰文字（伪元素提不出来）
- inline 元素（span/em/strong）不能有 margin
- authoring 不生成 CSS gradient；兼容重写能力不构成生成许可
- div 不用 `background-image`（用 `<img>`）
- 无法表示时停止生成并报告具体元素；禁止截图、栅格化或成功丢失

脚本已内置**自动预处理器**——把 "叶子 div 里的裸文本" 自动包成 `<p>`（保留 class）。这解决了最常见的违规（裸文本）。但其他违规（p 上有 border、span 上有 margin 等）仍需 HTML 源头合规。

**字体回落 caveat**：
- 测量环境与 PowerPoint/Keynote 本机字体可能不一致 → **溢出或错位**，导出后要肉眼过
- 建议目标机器装好 HTML 里用的字体，或明确回退到 `system-ui`

**视觉优先 + 要可改字 pptx** → 若视觉超出 editable subset，必须重构为受支持原语或阻止导出，不能接受成功产物丢失视觉效果。

### 从一开始就让 HTML 对导出友好

长期可维护 deck：**从第一行就按 editable 四条硬约束写**。额外成本不大：

```html
<!-- ❌ 不好 -->
<div class="title">关键发现</div>

<!-- ✅ 好（p 包裹，class 继承） -->
<p class="title">关键发现</p>

<!-- ❌ 不好（border 在 p 上） -->
<p class="stat" style="border-left: 3px solid red;">41%</p>

<!-- ✅ 好（border 在外层 div） -->
<div class="stat-wrap" style="border-left: 3px solid red;">
  <p class="stat">41%</p>
</div>
```

### 何时选哪个

| 场景 | 推荐 |
|------|------|
| 给主办方/档案存档 | **1920 演讲 HTML** 或浏览器打印稿 |
| 发给协作者改字 | **1280×720 editable-only contract**（可编辑 pptx 管线） |
| 现场演讲、不改内容 | **1920 演讲 HTML** + 聚合翻页 |
| HTML 是首选呈现媒介 | 直接浏览器播放 |

长期协作、反复改字 → 一开始就按 `editable-pptx.md` 四条写 HTML。

---

## 常见问题

**多文件：iframe 里的页打不开 / 白屏**
→ 检查 `MANIFEST` 的 `file` 路径是否相对 `index.html` 正确。用浏览器 DevTools 看 iframe 的 src 能否直接访问。

**多文件：某页样式和别页冲突**
→ 不可能（iframe 隔离）。如果感觉冲突，那是缓存——Cmd+Shift+R 强刷。

**单文件：多 slide 同时渲染叠加**
→ CSS 特异性问题。看上面「单文件架构的 CSS 陷阱」一节。

**单文件：缩放看起来不对**
→ 检查是否所有 slide 直接挂在 `<deck-stage>` 下作为 `<section>`。中间不能包 `<div>`。

**单文件：想跳到特定 slide**
→ URL 加 hash：`index.html#slide-5` 跳到第 5 张。

**两种架构都适用：字在不同屏幕下位置不一致**
→ 用固定尺寸（1920×1080）和 `px` 单位，不要用 `vw`/`vh` 或 `%`。缩放统一处理。

---

## 验证检查清单（做完 deck 必过）

1. [ ] 浏览器直接打开 `index.html`（或主 HTML），检查首页无破图、字体已加载
2. [ ] 按 → 键翻到每一页，没有空白页、没有布局错位
3. [ ] 按 P 键打印预览，每页恰好一张 A4（或 1920×1080）且无裁切
4. [ ] 随机选 3 页 Cmd+Shift+R 强刷，localStorage 记忆正常工作
5. [ ] 抽查 3 张单页 HTML（`open slides/xx.html`），布局与字体正常
6. [ ] 搜 `TODO` / `placeholder` 残留并清理
