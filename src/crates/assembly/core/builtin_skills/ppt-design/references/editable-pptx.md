# 可编辑 PPTX Authoring Contract

PPT Live 的唯一导出链路是 **editable HTML → EditableSlideScene → OOXML**。HTML 是 authoring source，`EditableSlideScene` 是唯一规范化场景，OOXML 是唯一导出结果；不存在并行的图片导出、视觉保底或降级成功路径。

本契约高于其他 style preset 和视觉参考。遇到冲突时，必须改变视觉表达以适配本契约；不能改变导出路径。

## Authoring subset（生成规则）

- 每页严格为 **1280px × 720px**。只使用 solid color；不得生成 CSS gradient 或 `background-image`，即使 converter 对部分旧输入有兼容重写能力。
- HTML 文字只可放在 `<p>`、`<h1>`–`<h6>`、`<li>` 中；`span` 只作这些标签内的文本 run。不得生成 `div` 裸文字。
- 背景、border、圆角只放在 `div` 等几何容器；文字标签不得承载 background、border 或 shadow。
- `box-shadow` 只支持单层 outer、非 inset、zero spread 的原生映射；多层 shadow 只取首个可用层，负 spread 按 0 近似，inset 等其余不支持形态导出时自动移除，不得依赖。`text-shadow` 任何非 `none` 形态在导出时一律自动移除，不得依赖其呈现层次。
- 禁止 CSS `filter`、`mask`、generated content、animation、外部资源和复杂/filled SVG path；禁止任意顶点/非严格对称 polygon，仅允许严格对称 triangle/diamond。
- 线与曲线优先直接生成 `line` 或 `polyline`；只有确有必要时才写兼容边界所列的 path 子集，且最终仍须成为独立 editable line。
- Authoring 流程箭头只由 editable line + CSS border triangle，或 SVG line + strict symmetric triangle polygon 构成。
- 表格必须是真实 `<table>` 并导出为 native `a:tbl`；图表、流程箭头、虚线和曲线必须由支持的可编辑原语构成。
- intentional 图片只允许内联 base64 PNG、JPEG、WebP，且不得承载文字、图表或几何；禁止 GIF，因为无法证明其为静态内容。
- 禁止任何正向 rasterize、screenshot 或 fallback 建议；无法表示时停止生成并报告具体元素。

## Converter legacy rewrite boundary（兼容边界，不是生成建议）

- 本边界只用于兼容既有输入，不是生成许可；authoring agent 仍必须遵守上面的更严格 subset。
- SVG `text` 是 converter 支持的 SVG 原语；它不改变 authoring 时 HTML 文字只能使用规定标签的要求。
- `div` 裸文字仅属 repair 兼容，authoring 不应生成。
- path 仅支持 `M/L/H/V/C/S/Q/T/Z`（含相对命令），必须 `fill:none`；`Z` 可以闭合 path，但拒绝 `A` 和任何 path/ancestor `transform`。
- `C/S/Q/T` 曲线由 converter 自适应采样为多段 editable line，不是 PowerPoint curve；authoring 仍优先 `line`/`polyline`，确需 path 时才使用上述子集。
- SVG polygon 只识别几何上严格对称的 triangle 和 diamond；任意顶点或非严格对称 polygon 都会被拒绝。
- legacy CSS 只兼容受限 `linear-gradient`：角度接受 `deg`、`turn`、`rad`、`grad` 或 `to top/right/bottom/left` 及对角方向；颜色 stop 只接受 converter 支持的纯色，位置只接受 percentage stop，缺省 stop 在相邻已知位置间均匀分配。
- converter 拒绝 `radial-gradient`、px/em stop、double-position stop、color hint、不支持的颜色与非法 alpha；合法 `linear-gradient` 被采样为 editable solid strips，这不是生成建议。
- legacy 单层 hard ring `box-shadow`（`0 0 0 Npx`、非 inset、blur=0）会被重写为同心可编辑 shape；authoring 仍应优先 zero-spread outer shadow，不得依赖 ring rewrite。

<!-- End editable contract -->

## 1. 固定画布

每一页都必须是完整 HTML 文档，并严格使用 **1280px × 720px**：

```css
html,
body {
  width: 1280px;
  height: 720px;
  margin: 0;
  overflow: hidden;
}
```

- 不得改用 960pt、1920×1080、百分比画布、`vw`/`vh` 或运行时缩放。
- 页面内容不得超出画布；正文文本框底边与画布底部至少留 48px。
- 画布背景必须是纯色。

## 2. 文本标签结构

- 所有可见文字必须位于 `<p>`、`<h1>`–`<h6>` 或 `<li>` 中。
- `<span>`、`<strong>`、`<em>`、`<b>`、`<i>`、`<u>` 只可作为上述文本标签内部的 run，用于局部字重、颜色或样式。
- `<div>`、`<td>`、`<th>` 和 shape 容器不得直接放 HTML 裸文字；表格单元格文字也要用 `<p>` 等文本标签包裹。converter 对 SVG `text` 的兼容支持见上方边界。
- 不得使用 `::before`、`::after` 或 `content` 生成任何文字、图标、编号或装饰。
- 文本标签只负责文本属性；background、边框、圆角和阴影不得放在文本标签上。

正确：

```html
<div class="card">
  <h2>结论标题</h2>
  <p>支持结论的说明。</p>
</div>
```

错误：

```html
<div class="card">裸文字</div>
<p style="background:#ffd700; border:1px solid #111;">错误结构</p>
```

## 3. Solid、Background 与 Border

- 所有 fill、background 和 border 都必须是纯色；禁止 CSS gradient、纹理、图案填充和半透明叠图模拟。
- 背景、边框和圆角只放在 `<div>` 等几何容器上。`box-shadow` 的精确可编辑子集为单层 outer shadow、非 inset、zero spread 的原生映射；多层 shadow 只取首个可用层，负 spread 按 0 近似，inset 等其余不支持形态导出时自动移除。converter 可将 legacy hard ring（`0 0 0 Npx`）重写为同心可编辑 shape，但 authoring 不得依赖该 rewrite。
- 文字标签不得有 background、border、border-radius 或 box-shadow；`text-shadow` 任何非 `none` 形态在导出时一律自动移除，不得依赖其呈现层次。
- `background-image` 在任何元素上都禁止。
- 数据条、色带和热力档位用多个离散纯色几何元素表达。

## 4. 支持的可编辑原语

生成时优先直接使用可映射 primitive：

- 文本：段落、标题、列表和文本 run。
- Shape：矩形、圆角矩形、圆/椭圆、预设三角形、预设箭头和导出器明确支持的 preset shape。
- Line：水平线、垂直线、旋转直线、虚线和由多个可编辑线段组成的曲线。
- Table：真实 HTML table。
- Image：仅限符合第 7 节的 intentional 图片。

CSS 三角、基础 SVG 和 open path 可由 rewriter 映射，但生成时优先直接使用可映射 primitive，减少重写歧义：

- CSS border 三角可映射为 preset triangle 加旋转。
- 基础 SVG converter 可处理 `rect`、`circle`、`ellipse`、`line`、`polyline`、`text` 等已支持元素；authoring 的普通文字仍使用 HTML 文本标签。
- path 仅使用兼容边界列出的命令，保持 `fill:none`；rewriter 把曲线采样为多个 editable line。
- SVG polygon 只能是几何上严格对称的 triangle 或 diamond；任意顶点轮廓不属于 authoring subset。

## 5. 明确禁止

- 禁止 CSS `filter`、`mask`、生成内容和 `background-image`。
- 禁止 `clip-path`、任意顶点/非严格对称 polygon、混合模式和依赖浏览器合成的视觉效果；仅严格对称 triangle/diamond 可进入 polygon rewrite。
- 禁止复杂或填充的 SVG `path`；禁止把 path、文字或图表嵌入 SVG 再作为整体视觉。
- 禁止外部图片资源、animation，以及任何 rasterize、screenshot、fallback 建议。
- 禁止外链 CSS、字体、脚本、SVG、PNG、JPEG、WebP 或其他网络/文件资源。
- 禁止 Web Component、canvas、video、animated GIF、CSS transition、keyframes 和运行时动画。
- 禁止把不支持的视觉隐藏、忽略或标记为“降级成功”；无法表示时必须停止生成并指出具体元素。

## 6. 表格必须成为 Native `a:tbl`

表格必须写真实的 `<table>`，使用 `thead`、`tbody`、`tr`、`th` 和 `td` 表达结构：

```html
<table>
  <thead>
    <tr>
      <th><p>指标</p></th>
      <th><p>结果</p></th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td><p>转化率</p></td>
      <td><p>42%</p></td>
    </tr>
  </tbody>
</table>
```

- 导出结果必须是 native `a:tbl`，单元格文字、fill、border、宽度和对齐都保持可编辑。
- 不得用 div grid、独立矩形集合、SVG、截图或图片伪装表格。
- 只使用导出器支持的规则；不确定的 `rowspan`/`colspan` 应改写为无合并单元格的真实表格。

## 7. Intentional 图片

intentional 图片只可使用 `<img>` 和内联 base64 raster，例如：

```html
<img
  src="data:image/png;base64,..."
  alt="用户提供的产品实拍"
  style="position:absolute; left:760px; top:120px; width:420px; height:320px;"
>
```

- 可用 PNG、JPEG 或 WebP data URL；不得使用外部 URL、相对路径、绝对路径、blob URL 或 SVG data URL。
- intentional 图片只承载本来就是照片、实拍、扫描件或用户明确提供的位图内容，且不得承载文字、图表或几何。
- 不得把生成内容先转成位图再以内联图片绕过 authoring contract。

## 8. 图表、流程与关系图

- 图表、流程箭头、虚线和曲线必须使用支持的可编辑原语。
- 条形、柱形、堆叠条和热力矩阵用纯色 shape；点图用圆/椭圆；坐标轴、目标线和连接线用 line。
- 流程节点用支持的基础 shape；流程箭头用 editable line + CSS border triangle，或 SVG line + strict symmetric triangle polygon，方向必须明确。
- 虚线使用 line 的 dash 属性对应写法，不得用点状图片或重复背景。
- 折线和曲线优先直接拆成 editable line/polyline；必要 path 由 converter 采样成多个 editable line，不得使用 filled path。
- 所有标签和数值都作为独立文本对象，不得烘焙进 SVG 或图片。

## 9. 合并文本框

需要在 PowerPoint 中连续编辑多段文字时，可给容器添加 `data-pptx-merge="true"`：

```html
<div data-pptx-merge="true">
  <h2>标题</h2>
  <p>第一段。</p>
  <p>第二段。</p>
</div>
```

- merge 容器不能嵌套。
- 容器的纯色 background、border 和圆角仍作为独立 shape。
- 合并段落必须使用统一对齐和行距；需要不同对齐时拆成多个文本框。

## 10. 写入前检查

每页写入时一次满足：

1. body 严格为 1280px × 720px。
2. 所有文字使用规定标签，且无 generated content。
3. 所有视觉都能映射为 text、shape、line、native `a:tbl` 或 intentional image。
4. 无 filter、mask、background-image、复杂/filled SVG path、非严格对称 triangle/diamond 的 SVG polygon、外部资源和 animation。
5. 无 rasterize、screenshot 或 fallback 路径。
6. 表格为真实 `<table>`，图片为允许的内联 base64 raster。
7. 内容无溢出，所有对象可被唯一场景链路序列化。
