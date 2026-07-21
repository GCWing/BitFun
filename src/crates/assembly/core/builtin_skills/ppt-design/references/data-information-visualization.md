# PPT 数据可视化与信息可视化

本参考是数据与信息可视化的**设计词汇库**，用于在合适时拓宽 Agent 的表达选择，不是图表配方、逐页流程或合规清单。Agent 可以根据主题、受众、叙事、风格、数据质量与技术条件采用、组合、改造或放弃这里的建议。

不要把「可视化」狭义理解为标准商业图表。带标注的照片、按比例排布的文字、空间构图、逐步揭示的概念图、地图式叙事、编辑化数字排版和主题化隐喻都可能有效。图表名称只是可复用词汇，不是设计边界；只要不虚构或扭曲信息，可以创造更贴合主题的新形式。

## 目录

1. 灵活判断框架
2. 数据可视化选择
3. 信息可视化选择
4. 页面构图与标注
5. 数据诚信
6. 可编辑 PPTX 实现
7. 常见失败模式
8. 写时参考

## 1. 灵活判断框架

当页面包含数据或结构关系、或当前构图显得只是文字陈列时，可以从这些角度思考；不要求按顺序或逐页执行：

1. **提取证据**：列出真实数字、类别、时间点、目标值、步骤、角色、层级、因果或依赖关系。
2. **定义任务**：观众最需要完成的是比较、识别趋势、理解构成、定位异常、理解关系，还是查精确值？
3. **形成一句结论**：先写出该页希望观众复述的判断，再设计图。
4. **选择视觉编码**：位置和长度通常更容易准确比较；角度、面积、颜色、形状和图像也可以在适合主题时使用。
5. **选择页面形式**：一个主图、图 + 注释、表格、流程/架构图，或结构化文字。
6. **验证数据与导出**：检查单位、口径、来源、可编辑 PPTX 兼容性和画布溢出。

### 值得探索可视化的信号

- 有两个以上可比较数值。
- 有三个以上时间点，或明确的前后变化。
- 有整体与部分、目标与实际、基准与结果。
- 有步骤顺序、责任交接、层级、依赖或因果关系。
- 文字中反复出现「高于、低于、增长、下降、占比、集中、分散、先后、驱动、阻碍」。

### 文字、图片或表格可能更好的情况

- 观众需要查找精确值，而不是识别模式。
- 数据点很少且结论一句话即可讲清。
- 类别名称很长、数值只是辅助。
- 证据主要是定性判断、案例细节或引用。
- 数据不完整，无法支持诚实的尺度比较。

## 2. 数据可视化候选

下面描述常见适配关系，而非唯一答案。同一数据可以因叙事目的不同使用完全不同的表现形式。

### 比较与排名

- **水平条形图**：适合类别名较长、排名、正负值比较。
- **点图**：类别很多但希望更轻、更节省墨水时使用。
- **分组条形图**：同一类别只比较 2–3 个系列；系列更多时改小倍图或热力矩阵。
- 排名叙事通常适合排序；有固定顺序、分组逻辑或需要制造揭示节奏时可保留其他排列。

### 时间趋势

- **折线图**：时间连续、关注走势和拐点；系列尽量不超过 3 条。
- **柱状图**：时间离散、强调每期规模，或可编辑路径不适合复杂折线时使用。
- **斜率图**：只比较起点与终点，突出谁上升、谁下降。
- 定量趋势通常保持时间间隔真实；若采用非等距叙事时间线，应明确它表达的是事件而非连续尺度。不要选择性删除会改变结论的时点。

### 实际、目标与变化

- **子弹图**：实际值 + 目标线 + 可选参考区间，通常比表盘更紧凑；强调状态或品牌隐喻时也可使用其他表达。
- **进度条**：单一完成率；标清分母和截止日期。
- **瀑布图**：解释起点如何经正负因素形成终点；每个增减项直接标值。
- **指数化折线**：不同量级系列比较增长速度时，把起点统一为 100，并明确注明。

### 构成占比

- **100% 堆叠条**：适合比较一个或多个整体的构成。
- **环形/饼图**：少量类别、整体关系明确时可以很直观；类别多或需要精细比较时通常换条形图更清楚。
- **普通堆叠条**：既要看总量又要看构成，但中间分段较难比较；必要时拆成总量图 + 100% 构成图。
- 类别较多时可合并长尾、用条形图，或采用与主题相符的分组/分面表达。

### 分布、波动与离群值

- **直方图**：连续数值的分布形状；分箱必须一致且有解释。
- **点图/蜂群图**：样本不多，希望保留每个观测值。
- **箱线图**：比较多组中位数、四分位和离群值；受众不熟悉时加简短读图说明。
- 只有平均值没有样本或范围时，不要假装展示分布。

### 关系与相关

- **散点图**：两个连续变量的关系；需要时加趋势线，但不要把相关写成因果。
- **气泡图**：只有第三变量确实重要时才用；面积编码难比较，气泡数量要少。
- **2×2 矩阵**：变量是定性高/低定位，而非精确连续数据。
- 标注关键点、异常点和目标对象，其余点弱化。

### 多维比较

- **热力矩阵**：多个对象 × 多个维度，颜色表达等级或区间；格内仍可保留关键数值。
- **对比表**：精确查阅、文字证据较多时使用；突出关键行列，不把整表都染色。
- **小倍图**：相同尺度下重复同一种小图，适合比较多个地区、产品或人群的趋势。
- **雷达图**：适合表达少量对象的整体轮廓或品牌化能力画像，但不适合精确比较；需要精度时考虑点图、热力矩阵或分组条形图。

### 空间数据

- 地理位置本身影响结论时，地图通常有价值；若地图只是氛围元素，则应确认它与整体视觉叙事一致。
- 地区值比较优先分级设色地图，地点事件优先点位图。
- 没有可靠边界、地理数据或可编辑实现时，改用地区排序条形图。
- 避免让地图只承担填满页面的作用。

## 3. 信息可视化选择

没有可量化数字，不等于只能写 bullet。信息存在结构时，可以考虑下列图示，也可以继续使用更有节奏的文字、图像或混合构图。

| 信息结构 | 推荐形式 | 关键要求 |
|----------|----------|----------|
| 线性步骤 | 流程图 | 动词开头，3–7 步，标明输入/输出 |
| 多角色交接 | 泳道图 | 泳道按责任方，节点按时间推进 |
| 时间演进 | 时间线/阶段带 | 日期与事件绑定，突出转折点 |
| 因果关系 | 因果链/问题树 | 区分原因、机制、结果，箭头有方向 |
| 层级与拆解 | 树状图/金字塔 | 上下位关系明确，同层粒度一致 |
| 系统组成 | 分层架构图 | 层名、模块、连接关系均有标签 |
| 多方关系 | 利益相关者图/关系网络 | 中心对象明确，线型或方向有含义 |
| 前后变化 | Before / After | 使用同一组维度对齐比较 |
| 方案选择 | 2×2 / 决策矩阵 | 轴和评分依据可解释 |
| 用户体验 | Journey map | 阶段、行为、痛点、机会按列对齐 |
| 论点结构 | 论点树/证据链 | 结论、分论点、证据层级清楚 |

信息图中的框、线、箭头和图标最好具有可解释的语义；纯装饰元素可以存在，但不应冒充信息关系或干扰阅读。

## 4. 页面构图与标注

### 推荐构图

- **结论 + 主图**：标题给结论，主图承担主要证据，旁边放少量解释；具体占比由风格和内容决定。
- **主图 + 证据注释**：在关键峰值、拐点、异常点旁直接注释原因。
- **图 + 明细表**：图显示模式，小表提供精确值；两者应使用同一口径。
- **小倍图**：每个小图使用相同尺度、相同时间范围和相同视觉编码。
- **过程图 + 结果栏**：左侧解释机制，右侧放结果指标或结论。

### 常见标注层级

1. 断言式标题或有意保留悬念的叙事标题。
2. 图内直接标签和关键数值。
3. 解释差异、原因或下一步的注释。
4. 必要的单位、时间、来源、样本与估算说明。

图例只在直接标注会造成拥挤时使用。系列名称放在线末端或条形旁通常比单独图例更快。

### 颜色与尺度

- 1 个强调色 + 中性色是稳妥方案；品牌化、文化或教育主题可以使用更丰富但有秩序的色彩系统。
- 同一 deck 中同一对象保持同色；不要在不同页面随意换色。
- 条形图数值轴通常从 0 开始；折线图等场景可使用截断轴，但应明确标出并避免夸大。
- 使用面积表达数量时按面积映射；若图形只是定性象征，应避免让观众误读为精确比例。
- 3D、透视、阴影体积和纹理容易扭曲数量判断；只有当它们主要承担风格表达且不会误导时再使用。

## 5. 数据诚信（硬约束）

每张数据图至少核对：

- 数值与来源一致，没有把估算写成事实。
- 单位、币种、时间范围、地域、样本量和分母明确。
- 百分比能解释分母；构成图总和应为 100% 或说明为何不是。
- 同一图中的系列口径一致；名义值与实际值、累计值与单期值不混用。
- 缺失值显示为空缺或 `N/A`，不自动当作 0。
- 预测、目标、实际用不同线型/填充并清楚标注。
- 没有数据时使用定性矩阵、流程或结构化文字，不生成伪图表。

## 6. 可编辑 PPTX 实现（技术约束）

默认 **1280×720px** 可编辑路径严格遵守 `editable-pptx.md`，唯一链路是 **editable HTML → EditableSlideScene → OOXML**。

### Authoring subset（生成规则）

- 每页严格为 **1280px × 720px**。只使用 solid color；不得生成 CSS gradient 或 `background-image`。
- HTML 文字只可放在 `<p>`、`<h1>`–`<h6>`、`<li>` 中；`span` 只作文本 run，不得生成 `div` 裸文字。
- `box-shadow` 只支持单层 outer、非 inset、zero spread 的原生映射；多层 shadow 只取首个可用层，负 spread 按 0 近似，inset 等其余不支持形态导出时自动移除，不得依赖。`text-shadow` 任何非 `none` 形态在导出时一律自动移除，不得依赖其呈现层次。
- 图表、流程箭头、虚线和曲线必须使用支持的可编辑原语；线与曲线优先直接生成 `line` 或 `polyline`。
- 条形、柱形、进度、堆叠条使用纯色 shape；点图使用 ellipse；目标线、坐标轴和连接线使用 line。
- Authoring 流程箭头只由 editable line + CSS border triangle，或 SVG line + strict symmetric triangle polygon 构成。
- 热力矩阵和精确数据表必须写真实的 `<table>`，导出为 native `a:tbl`。
- intentional 图片只允许内联 base64 PNG、JPEG、WebP，且不得承载文字、图表或几何；禁止 GIF，因为无法证明其为静态内容。
- 禁止 CSS `filter`、`mask`、generated content、animation、复杂/filled SVG path 与外部资源；禁止任意顶点/非严格对称 polygon，仅允许严格对称 triangle/diamond。
- 禁止任何正向 rasterize、screenshot 或 fallback 建议；复杂视觉必须重构为 shape、line、table 与独立文本，无法表示时报告具体限制。

### Converter legacy rewrite boundary（兼容边界，不是生成建议）

- 本边界只用于兼容既有输入，不是生成许可；authoring agent 仍必须遵守上面的更严格 subset。
- SVG `text` 是 converter 支持的 SVG 原语；`div` 裸文字仅属 repair 兼容，authoring 不应生成。
- path 仅支持 `M/L/H/V/C/S/Q/T/Z`，必须 `fill:none`；`Z` 可以闭合 path，但拒绝 `A` 和任何 path/ancestor `transform`。
- `C/S/Q/T` 曲线被采样为多段 editable line，不是 PowerPoint curve；authoring 优先 `line`/`polyline`，确需 path 时才使用上述子集。
- SVG polygon 只识别严格对称的 triangle 和 diamond；任意顶点或非严格对称 polygon 都会被拒绝。
- legacy CSS 仅兼容受限 `linear-gradient`：角度接受 `deg`、`turn`、`rad`、`grad` 与方向关键字；位置只接受 percentage stop，缺省 stop 均匀分配。
- converter 拒绝 `radial-gradient`、px/em stop、double-position stop、color hint、不支持颜色和非法 alpha；合法 gradient 被采样为 editable solid strips，这不是生成建议。
- legacy 单层 hard ring `box-shadow`（`0 0 0 Npx`、非 inset、blur=0）会被重写为同心可编辑 shape；authoring 仍应优先 zero-spread outer shadow，不得依赖 ring rewrite。

<!-- End editable contract -->

- CSS 三角、基础 SVG 与受限 path 可以由 converter 重写，但这不扩张 authoring subset。
- 100% 构成使用相邻纯色 shape；禁止 `conic-gradient`、任意顶点/非严格对称 polygon 和 filled SVG path，仅严格对称 triangle/diamond 可进入 polygon rewrite。
- 所有图表标签与数值保持独立 HTML 文本对象；背景、边框放在 `div`，文本放在 `<p>/<h*>/<li>`。
- **布局纪律（防止塌陷与重叠，硬约束）**：
  - **不要用垂直方向的 `flex:1` 去拉伸连接线/竖线**。当容器高度不确定（嵌套 `flex:1`、或父容器无明确高度）时，`flex:1` 的竖线会塌陷为 0 高度，导致时间线、连接线消失、内容堆叠。连接线一律用**固定 `height` 的 div**（如 `height:12px`）。
  - **不要用绝对定位画多象限/多区域布局**。绝对定位元素若只设 `top` 不设 `height`（或只设 `left` 不设 `width`）会无限撑开，与相邻绝对定位元素重叠。多区域等分一律用 **CSS grid**（`grid-template-columns`/`grid-template-rows`），高度由 grid 自动均分，绝不塌陷。
  - **水平等宽列用 `flex:1` 是安全的**（横向 `display:flex` + 子项 `flex:1` 等分宽度），本纪律只针对垂直拉伸。

### 6.1 架构图 / 流程图 / 系统图的可编辑实现

技术方案、工程分析、项目架构、系统设计这类内容，架构图和流程图几乎是不可或缺的表达。它们必须拆成独立的 shape、line 和文本对象，不用 filled/complex SVG path、不用 `background-image`。

**通用构建积木**：
- **节点框**：纯色 `<div style="border:1.5px solid #主色; padding:6px 10px;">` + 内部 `<p>` 写节点名。
- **连接线（水平/垂直/虚线/曲线）**：优先直接使用可映射 `line`/`polyline`；必要曲线才使用兼容 path 子集并由 converter 采样成多段 line。
- **箭头**：只用 editable line + CSS border triangle，或 SVG line + strict symmetric triangle polygon；不依赖未实现的 arrow producer。
- **分组容器（泳道/层）**：`<div style="border:1px solid #e2e8f0; background:#f8fafc; padding:10px;">`，内部用 flex 或 grid 排子节点。
- **所有文字放 `<p>`/`<h*>`/`<li>`**，`span` 仅作为文本 run；背景/边框放 `div`。

**① 分层架构图（系统组成：分层结构）**
```html
<div style="display:flex; flex-direction:column; gap:8px;">
  <!-- 一层 -->
  <div style="border:1px solid #e2e8f0; background:#f8fafc; padding:8px 10px;">
    <p style="font-size:11px; font-weight:700; color:#64748b; margin:0 0 4px 0;">第一层（如接入/表层）</p>
    <div style="display:flex; gap:6px;">
      <div style="flex:1; border:1.5px solid #1e3a8a; padding:5px 8px; text-align:center;">
        <p style="font-size:11px; font-weight:700; color:#1e3a8a; margin:0;">模块一</p>
      </div>
      <div style="flex:1; border:1.5px solid #1e3a8a; padding:5px 8px; text-align:center;">
        <p style="font-size:11px; font-weight:700; color:#1e3a8a; margin:0;">模块二</p>
      </div>
    </div>
  </div>
  <!-- 层间连接：用一个居中的竖线 div -->
  <div style="display:flex; justify-content:center;">
    <div style="width:1.5px; height:12px; background:#94a3b8;"></div>
  </div>
  <!-- 下一层同构，层名与模块名由具体主题决定 -->
</div>
```
> 适用：任何分层结构的系统组成——微服务架构、技术栈分层、数据流水线、组织层级、产品模块图等。每层一个分组容器，层内模块用并排节点框，层间用竖线连接。旁边的文字解读栏逐层解释职责。

**② 线性流程图（步骤序列：输入→处理→输出）**
```html
<div style="display:flex; align-items:center; gap:0; flex-wrap:wrap;">
  <div style="border:1.5px solid #1e3a8a; padding:6px 10px; text-align:center; min-width:70px;">
    <p style="font-size:11px; font-weight:700; color:#1e3a8a; margin:0;">① 步骤一</p>
  </div>
  <div style="width:16px; height:1.5px; background:#1e3a8a;"></div>
  <div style="width:0; height:0; border-left:5px solid #1e3a8a; border-top:4px solid transparent; border-bottom:4px solid transparent;"></div>
  <div style="border:1.5px solid #1e3a8a; padding:6px 10px; text-align:center; min-width:70px;">
    <p style="font-size:11px; font-weight:700; color:#1e3a8a; margin:0;">② 步骤二</p>
  </div>
  <!-- 后续节点同构，步骤多时可折行（flex-wrap） -->
</div>
```
> 适用：任何线性流程——数据处理流水线、CI/CD、请求处理链路、业务审批、项目阶段等。节点用动词开头，箭头明确方向。步骤超过 6 个时折行或拆成两行，旁边配编号解读。

**③ 泳道图（多角色交接）**
```html
<div style="display:flex; gap:0; border:1px solid #e2e8f0;">
  <!-- 泳道：每条一列，左侧角色标签 -->
  <div style="display:flex; width:100%;">
    <div style="width:80px; background:#f1f5f9; border-right:1px solid #e2e8f0; display:flex; align-items:center; justify-content:center;">
      <p style="font-size:11px; font-weight:700; color:#1e3a8a; writing-mode:vertical-rl; margin:0;">角色一</p>
    </div>
    <div style="flex:1; padding:8px; display:flex; align-items:center; gap:6px;">
      <div style="border:1.5px solid #1e3a8a; padding:5px 8px;"><p style="font-size:10px; color:#1e3a8a; margin:0;">动作A</p></div>
    </div>
  </div>
  <!-- 其它泳道（角色二/角色三）同构，纵向堆叠，跨泳道箭头用绝对定位或简化为文字标注 -->
</div>
```
> 适用：多角色协作流程、多团队交接、跨部门审批等多泳道场景。

**④ 因果链 / 问题树（根因分析）**
```html
<div style="display:flex; flex-direction:column; align-items:center; gap:6px;">
  <!-- 结果在顶部，原因在下方逐层展开 -->
  <div style="border:1.5px solid #dc2626; padding:5px 12px; background:#fef2f2;">
    <p style="font-size:11px; font-weight:700; color:#dc2626; margin:0;">现象：待分析的表层结果</p>
  </div>
  <div style="width:1.5px; height:10px; background:#94a3b8;"></div>
  <!-- 分叉：用 flex 横排多个原因节点 -->
  <div style="display:flex; gap:12px;">
    <div style="border:1px solid #e2e8f0; padding:5px 8px;"><p style="font-size:10px; color:#1f2937; margin:0;">原因一</p></div>
    <div style="border:1px solid #e2e8f0; padding:5px 8px;"><p style="font-size:10px; color:#1f2937; margin:0;">原因二</p></div>
    <div style="border:1px solid #e2e8f0; padding:5px 8px;"><p style="font-size:10px; color:#1f2937; margin:0;">原因三</p></div>
  </div>
</div>
```
> 适用：故障复盘、性能问题根因分析、业务归因、技术方案的风险推演等任何「由果溯因」场景。

**架构图/流程图的质量要点**：
- 节点是否用**动词或明确名词**（不是模糊的「处理模块」）？
- 箭头方向是否清晰？跨层/跨步的依赖是否标全？
- 旁边是否有**逐节点/逐层的文字解读**（不能只有图没有解释）？
- 节点数量是否在可读范围（线性流程 ≤ 7 步，分层架构 ≤ 4 层，超出就拆页或分层展开）？

## 7. 常见风险信号

以下现象值得重新思考，但不自动代表设计错误；若它们服务于明确的叙事或风格意图，可以保留。

- 有 5 个季度数据，却做成 5 张指标卡。
- 有 8 个类别排名，却用饼图。
- 用一个巨大数字，却没有单位、基准、时间或目标。
- 图标题只写主题，不告诉观众该看什么。
- 所有系列同等鲜艳，关键结论没有视觉焦点。
- 图例与图形距离太远，观众需要反复匹配颜色。
- 表格整页都是数字，没有高亮行列和结论。
- 流程图只有名词框，没有动作、方向、输入输出。
- 复杂图塞在半页，字号和标签小到无法演示。
- 为了「更像数据页」编造数字、补齐缺失值或伪造精度。

## 8. 写时参考（不要求生成后回头逐页复核）

以下要点供写每页时参考，不要求生成完 deck 后再整体回看或返工：

1. 有数据或结构的页面里，是否错过了能显著提升理解的可视化机会？
2. 是否也存在为了“看起来丰富”而硬加的图表或图示？
3. deck 的视觉表达是否随主题和叙事变化，而不是重复同一种卡片或同一种图表？
4. 图形、文字、图片和留白之间是否形成了合适的节奏？
5. 数据页的数值、单位、时间、口径和来源是否齐全？
6. 所有页面是否满足画布、底部安全边距和目标导出路径的技术约束？

目标不是增加图表数量，而是扩大 Agent 的表达选择：该图则图，该文则文，也允许创造无法被上述分类表完整命名的新形式。
