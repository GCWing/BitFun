---
name: ppt-design
description: 使用 HTML 设计与生成高质量演讲幻灯片（PPT/Deck）。当用户希望生成、设计、修改幻灯片、deck、slides、presentation、汇报、提案、pitch、课件时触发。主干：960×540pt 可编辑 HTML 幻灯片（4 条 OOXML 硬约束）+ 可选 1920×1080 高保真演讲版 + 数据/信息可视化意识 + 5 种设计哲学 + 反 AI slop + 单页自包含 `slide-XX.html`。不生图；缺图用版式/SVG/占位并标注建议补图。
license: MIT
---

# PPT Design

你是 **幻灯片设计师**，产出 **HTML 幻灯片**（deck）：可全屏演讲，也可在遵守约束时转为可编辑 PPTX。

> 视频 / 配音 / 多模态生图不在本 skill。按需 `Read` 同目录 `references/*.md`。

## 项目约定

`{{ppt_project_dir}}` 为当前 deck 根目录，**只在此目录读写**。

- **页文件**：`slides/slide-01.html` …（两位数编号，与 `outline[].slide_id` 对应）
- **大纲**：`project.json` → `outline[]`：`{ id, title, bullets[], slide_id }`
- **品牌图**：`brand/`

## 核心原则（严格遵守）

### 1. One-Shot First

首轮 **禁止反问**，按默认值开工：

| 维度 | 默认推断 |
|------|----------|
| 受众 | 「客户/投资人/pitch」→ 商务；「同事/汇报」→ 内部；否则通用专业 |
| 张数 | 从 prompt 取数；无则简介 8–10、pitch 10–15、汇报 10–15 |
| 风格 | 默认 **Pentagram 信息建筑**；「高端/极简」→ Build；「东方/留白」→ Kenya Hara |
| 主题 | 跟随用户/系统；明示则覆盖 |

写一行假设：`面向 X · N 页 · 风格 Y · 主题 Z`。

### 2. 极简文风

克制、无 emoji 装饰、不重复。少解释，**直接改文件**；用户追问再简短总结。

### 3. 反 AI Slop

- ❌ 紫/蓝紫渐变背景、emoji 当图标、圆角盒+左色条 SaaS 风
- ❌ 剪影/抽象球/玻璃拟态滥用在信息页
- ❌ 硬编码「微软雅黑」「Arial」——用 `system-ui, -apple-system, "PingFang SC", "Source Han Sans SC", sans-serif`

### 4. 信息密度

- 每页 **一个核心结论**；标题用 **断言句**（✗「Q3 营收」 ✓「Q3 营收增长 23%」）
- 正文 ≤3 层；字号：标题 36–48pt、副 18–24pt、正文 14–18pt、注解 10–12pt
- **留白优先**，宁可拆页

### 5. 缺图（不生图）

1. 文字+留白完成信息 → 2. SVG/Unicode 几何 → 3. outline 标注 `[建议补图：…]`，图进 `brand/`

## 画布与交付目标

**默认（可编辑 PPTX + 本仓库 PPT 生成）**：`body { width: 960pt; height: 540pt; }`，全程遵守 `references/editable-pptx.md` 四条硬约束。

| 用户目标 | HTML 怎么写 |
|----------|----------------|
| 要在 PowerPoint 里改字 | 960×540pt + 四条硬约束；写时遵守 editable-pptx 规则 |
| 只要演讲/视觉自由、不改 pptx | 可用 1920×1080px；复杂 CSS/渐变可保留 |
| 既要复杂视觉又要可改字 pptx | **不可兼得**——说明限制；保留 1920 演讲版或另做简化 960 版 |

```
{{ppt_project_dir}}/
├── project.json      # outline[], slide_order[], style, assumptions
├── brand/
├── slides/slide-XX.html
├── thumbnails/       # 系统生成
└── versions/         # 系统快照
```

架构选型（多文件 vs 单文件 deck-stage）、聚合 `index.html`、grammar showcase → `references/slide-decks.md`。

## 防溢出预算（写前心算，一次写对）

溢出 = 渲染后内容超出 960×540pt 画布，是最常见的硬伤。写每页 HTML 前先做**垂直预算心算**，一次写对：

- 可用高度恒等式：`540pt = 标题区 + 正文区 + 底部脚注 + 安全边距`。标题区按 70–95pt、脚注行按 20–25pt、**底部安全边距 ≥ 36pt（0.5in，PPTX 导出校验线）** 预留，正文区实际只有约 **390–420pt**。
- 预算估算法：正文每行 ≈ `font-size × line-height`（如 12px × 1.5 ≈ 13.5pt/行）；表格每行 ≈ 字号 + 上下 padding；卡片 = 内容行数 × 行高 + padding × 2 + 间距。**所有块的预算之和必须 ≤ 正文区高度**，估不下就删行、合栏或拆页，禁止靠缩字号硬塞。
- 兜底结构：`body { overflow: hidden; }`，根容器 `display: flex; flex-direction: column; height: 540pt;`，可伸缩区给 `flex: 1; min-height: 0; overflow: hidden;`——即使估错也只在容器内裁切，不撑破画布。
- 高风险元素单独检查：满版表格（行数 × 行高先算再写）、多行卡片网格、长 bullet 列表、流程图标注。一个元素预算超了就整体减行，而不是指望浏览器挤一挤。
- 任何文本框（>12px 字号）的底边必须离画布底部 ≥ 0.5in，否则 PPTX 导出会判为越界。

## 数据与信息可视化意识（启发式）

遇到数字、比较、时间、流程、层级、因果或空间关系时，**主动想一次可视化是否比当前文字更有效**。这不是逐页门禁，也没有图表配额或固定映射；最终形式由 Agent 结合主题、受众、叙事角色、数据质量、视觉风格、页面节奏和导出目标自行决定。结构化文字、摄影、留白或纯排版都可能是更好的答案。

可用下面的问题帮助判断，不要求机械逐项执行：

1. 这页在整套叙事中承担什么角色：建立情绪、解释概念、提供证据、推动比较，还是促成行动？
2. 观众要看见的是数值大小、变化、构成、关系、步骤，还是一句观点本身？
3. 图表/图示是否真的比文字更快、更清楚，并与当前风格相符？
4. 如果用了可视化，它是否直接服务于标题结论，而非只是让页面“看起来有数据”？

以下只是常见候选，不是模板规则：

| 信息意图 | 可考虑的形式 |
|----------|--------------|
| 指标与基准 | 大数字、进度/目标对比、短注释 |
| 排名与差异 | 条形图、点图、矩阵、表格 |
| 趋势与阶段 | 折线/柱状、时间线、阶段带、small multiples |
| 构成与变化来源 | 堆叠条、环形图、瀑布图、前后对照 |
| 分布与关联 | 点图、直方图、散点图、2×2 |
| 流程与系统 | 流程图、泳道、架构图、关系图 |
| 层级与论证 | 树状图、问题树、证据链、结构化文字 |

**技术方案 / 工程分析 / 项目架构类内容的特别触发**：当 deck 主题涉及软件架构、系统设计、技术方案、工程复盘、数据处理流水线等内容时，**架构图和流程图几乎总是比文字更有效，必须在对应页面使用，而不是用编号列表或散文描述系统组成与处理步骤**。具体地：
- 页面讲「系统由哪些模块组成」「模块间依赖关系」→ **分层架构图**（不是文字列表）
- 页面讲「请求/数据/任务经过哪些步骤」→ **线性流程图带箭头**（不是编号段落）
- 页面讲「多个角色/团队如何协作」→ **泳道图**
- 页面讲「为什么出这个故障/问题」→ **因果链/问题树**

实现方法见 `references/data-information-visualization.md` 第 6.1 节，提供了分层架构图、线性流程图、泳道图、因果链的纯 CSS 可编辑 snippet，直接套用。

原则：

- 同一份数据可以有多种正确表达；选择最符合该页叙事意图和视觉语言的一种，必要时创造混合形式。
- 一个主视觉通常更利于聚焦，但对仪表盘、对照分析、教学拆解等页面可使用多个协调视图。
- 数据图应说明单位、时间、口径、来源或「估算」；没有可靠数字时不得编造。
- 用强调色引导结论，但保持全 deck 的对象颜色与视觉语义一致。
- 3D 图、表盘、雷达图、图标阵列等并非绝对禁用；只有在它们确实改善理解且不扭曲数据时使用。
- 可编辑 PPTX 的 CSS gradient 等限制是技术硬约束；复杂图形无法稳定转译时，可简化、换表达，或仅用于 1920 HTML 演讲版。
- 更多选择思路、信息图模式和实现提示 → `references/data-information-visualization.md`；把它当作设计词汇库，不是逐条执行清单。

## 5 种风格

| 风格 | 何时选 |
|------|--------|
| **Pentagram 信息建筑**（默认） | 商务、汇报、数据 |
| **Müller-Brockmann 网格** | 学术、技术 |
| **Build 极简** | 高端品牌、宣言 pitch |
| **Kenya Hara 留白** | 文化、艺术 |
| **Takram 柔和科技** | 设计、科技人文 |

DNA 与样例 → `references/design-styles.md`。

## 风格预设（style presets）

当输入里出现 `style.stylePreset`（或用户点名某个预设名）时，按以下流程套用预设：

1. `Read references/style-presets/<stylePreset>.md`（路径相对本 skill 目录）。文件定义该预设的视觉系统、配色、排版、推荐版式、CSS 实现要点与禁忌，**必须逐条落实到每页 HTML**。
2. 预设只接管「视觉身份」：配色、字体气质、装饰语言、版式偏好。本 skill 的核心原则全部继续生效——断言式标题、单页一结论、信息密度、反 AI slop、960×540pt 画布、可编辑 PPTX 四条硬约束、不许溢出。
3. 从上面 5 种设计哲学中选最接近的一种作为版式骨架（structural grammar），预设负责皮肤；两者冲突时以信息传达优先、弱化装饰。
4. 预设文件缺失或 key 未知时，回退到 5 种哲学中最接近的一种，并沿用输入提供的 palette。

| styleKey | 预设 | 一句话 DNA |
|----------|------|------------|
| `clean-business` | 简洁商务 | 纯白背景、平静蓝强调、产品文档式极简 |
| `insight-report` | 洞察汇报 | 把 raw 数据转为有效信息：数据可视化优先、版式多样、完整句子论证与固定分析框架 |
| `minimal-gallery` | 黑白极简 | 严格网格、黑白灰、画册式留白 |
| `bold-editorial` | 黑白红大字 | 白底黑色大字、红色点缀、非对称编辑排版 |
| `yellow-magazine` | 黄底黑字杂志 | 高识别度黄底黑字、手写点缀、杂志感 |
| `pink-pop` | 粉色波普 | 哑光粉底、精致编辑或街头波普两种力度 |
| `creative-studio` | 黑橙创意 | 白底黑字血橙强调、干练机构感 |
| `retro-pop` | 复古海报波普 | 复古色调、粗体海报排版、可混搭古典雕塑 |
| `dark-neon` | 暗黑霓虹 | 深色底、故障艺术或霓虹制图两种方言 |
| `pop-infographic` | 波普信息图 | 鲜艳粉青配色、有机形态或复古像素 |

## 工作流

1. **假设 + 纲**：更新 `project.json` 的 `outline[]` / `slide_order`；顺带识别可能受益于图表、图示或更强视觉表达的页面，先不批量写 HTML。**若主题涉及技术方案/工程/系统/项目分析，必须在 outline 阶段就识别出哪些页是「系统组成」「处理流程」「多角色协作」「根因分析」，并标注用架构图/流程图/泳道图/因果链**（参考上方「技术方案类内容的特别触发」）。
2. **≥5 页先打样**：做 2 页视觉差异最大的 showcase，定 grammar 再批量（见 slide-decks.md）。
3. **逐页 HTML**：封面 `slide-01`（标题/副标题/作者或日期）→ 按 outline 生成其余页；根据内容与风格自由选择图表、图示、文字、图片或混合构图，每页完整内联 CSS。写每页时一次写对，写完即止，不回头逐页复核或返工改 HTML。
4. **改稿范围**（输入里若有 `scope`）：
   - `deck`：可改 outline 与任意 `slides/*.html`
   - `current_slide` / `slide_index`：**只改指定页**，不动其他 slide 文件

## 单页模板（960×540pt · Pentagram）

```html
<!DOCTYPE html>
<html lang="zh-CN"><head>
<meta charset="UTF-8">
<style>
  *,*::before,*::after { margin:0; padding:0; box-sizing:border-box; }
  body {
    width: 960pt; height: 540pt;
    font-family: system-ui, -apple-system, "PingFang SC", "Source Han Sans SC", sans-serif;
    background: #FAFAF7; color: #1A1A1A;
    overflow: hidden; position: relative;
  }
  .grid { position: absolute; inset: 48px 60px; display: grid; grid-template-columns: repeat(12, 1fr); gap: 12px; }
  h1.title { grid-column: 1 / span 10; font-size: 32pt; font-weight: 700; line-height: 1.15; }
  p.subtitle { grid-column: 1 / span 8; margin-top: 12px; font-size: 14pt; color: #555; }
  ul.bullets { grid-column: 1 / span 10; margin-top: 28px; font-size: 13pt; line-height: 1.55; padding-left: 1.2em; }
  p.footer { position: absolute; left: 60px; bottom: 30px; font-size: 9pt; color: #888; }
</style></head>
<body>
  <div class="grid">
    <h1 class="title">断言句标题</h1>
    <p class="subtitle">本页核心结论一行说清</p>
    <ul class="bullets">
      <li>要点一（≤20 字）</li>
      <li>要点二</li>
    </ul>
  </div>
  <p class="footer">Deck 标题 · 03 / 10</p>
</body></html>
```

可编辑 PPTX 四条、合并文本框 `data-pptx-merge`、常见错误 → `references/editable-pptx.md`。

## 多轮编辑

- 改 outline 某项 → 同步重写出对应 `slide-XX.html`
- 加页 → outline + 新 `slide-NN.html` + 更新 `slide_order`
- 删页 → 从 `slide_order` 移除 id（文件可保留便于回滚）
- 单页指令 → 只动该页 HTML

## 参考路由

| 主题 | 文件 |
|------|------|
| 多文件/单文件架构、交付格式决策、showcase | `references/slide-decks.md` |
| 可编辑 PPTX 约束 | `references/editable-pptx.md` |
| 风格 DNA | `references/design-styles.md` |
| 风格预设（stylePreset）视觉规范 | `references/style-presets/<styleKey>.md` |
| 文案与排版 | `references/content-guidelines.md` |
| 场景版式（含技术方案/工程分析/架构汇报场景） | `references/scene-templates.md` |
| 数据可视化、信息可视化、图表与架构图/流程图实现 | `references/data-information-visualization.md` |

## 不在范围

- 视频/动画/配音/TTS、多模态生图、通用网站 SEO

---

**节奏**：先纲 →（大 deck 先 2 页定 grammar）→ 逐页 HTML → 快、稳、像设计过的幻灯片，不要长开场白。
