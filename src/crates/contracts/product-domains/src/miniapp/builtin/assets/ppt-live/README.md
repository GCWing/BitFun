# PPT Live MiniApp — Developer Guide

PPT Live 是 BitFun 的内置 MiniApp，用于 AI 驱动的 PPT 生成、编辑和导出。

## 目录结构

```
ppt-live/
├── index.html              # MiniApp 入口 HTML（由 builtin.rs 加载）
├── style.css               # 全局样式
├── ui.js                   # UI 入口 JS（build-bitfun.mjs 的打包入口）
├── worker.js               # 空文件（PPT Live 不使用 worker）
├── build-bitfun.mjs        # 唯一的构建脚本 → 产出 dist/ui.bundle.js
├── meta.json               # MiniApp 元数据（含 version）
├── bundle.json             # bundle 标识（含 version）
├── source_manifest.json    # 构建产物清单
├── esm_dependencies.json   # ESM 依赖声明（当前为空数组）
├── dist/
│   └── ui.bundle.js        # 唯一的运行时 JS（由 builtin.rs 加载）
└── src/
    ├── export-deck-host.js       # 导出函数的 re-export 壳（ui.js 通过它引入导出能力）
    ├── export-deck-browser.js    # EditableSlideScene/PDF/PNG 导出实现
    ├── export-slide-browser.js   # 幻灯片预处理编排（挂载 DOM → sanitize → scene）
    ├── export-degrade.js         # 导出降级层（剥样式 / 移除元素 / 简化页面，代替阻断）
    ├── editable-slide-normalize.js # HTML/SVG/table → EditableSlideScene
    ├── editable-slide-scene.js   # 可编辑场景契约、校验与结构化错误
    ├── html2pptx-dom-core.js     # DOM 几何与原生元素提取辅助
    ├── pptx-html-build.js        # 唯一 EditableSlideScene→PPTX serializer
    ├── sanitize-slide-html.js    # 导出前 HTML 净化/修复
    ├── render.js                 # 幻灯片渲染（编辑器、缩略图、预览）
    ├── deck-ai.js                # AI 生成对接
    ├── state.js                  # 应用状态管理
    ├── style-presets.js          # 样式预设定义
    ├── i18n.js                   # 国际化
    ├── export-html.js            # HTML 导出
    ├── export-format-icons.js    # 导出格式图标
    ├── flat-select.js            # 自定义下拉组件
    └── bitfun-backend-adapter.js # BitFun 后端适配器
```

## 构建

### ⚠ 重要：`pnpm run desktop:dev` 不会构建 PPT Live 的 JS

PPT Live 的 JS 是**预构建的静态资源**，通过 Rust 的 `include_str!` 在编译时
直接嵌入到二进制中。`desktop:dev` 只提供 web-ui 前端的 Vite HMR 和 Rust 代码的
自动重新编译，**不会运行 `build-bitfun.mjs`**。

修改 PPT Live 的 JS 源码后，必须**手动**运行构建脚本。

### 改了什么文件 → 要做什么

| 修改的文件 | 需要重新构建 JS？ | 需要 bump 版本号？ | 说明 |
|---|---|---|---|
| `ui.js` | ✅ 是 | ✅ 是 | UI 入口，改动直接影响运行时 |
| `src/*.js`（所有子文件） | ✅ 是 | ✅ 是 | 打包源码，改动直接影响运行时 |
| `build-bitfun.mjs` | ❌ 否（本身是构建工具） | ❌ 否 | 下次构建时自动生效 |
| `index.html` | ❌ 否 | ✅ 是 | 由 `include_str!` 直接嵌入，bump 版本触发 Rust 重编译即可 |
| `style.css` | ❌ 否 | ✅ 是 | 同上 |
| `worker.js` | ❌ 否 | ✅ 是 | 同上 |
| `meta.json` / `bundle.json` | ❌ 否 | — | 本身就是版本号文件 |
| `README.md` / `source_manifest.json` / `esm_dependencies.json` | ❌ 否 | ❌ 否 | 文档/清单，不影响运行时 |

### 构建命令

```bash
# 从 repo 根目录
node src/crates/contracts/product-domains/src/miniapp/builtin/assets/ppt-live/build-bitfun.mjs

# 或进入 ppt-live 目录后运行
cd src/crates/contracts/product-domains/src/miniapp/builtin/assets/ppt-live
node build-bitfun.mjs
```

产出：`dist/ui.bundle.js`（未压缩，可读，开源项目不需要压缩 JS）。

### 完整操作流程（修改 JS 源码后）

```
1. 编辑 ui.js 或 src/ 下的 .js 文件
2. 运行构建：
   node src/crates/contracts/product-domains/src/miniapp/builtin/assets/ppt-live/build-bitfun.mjs
3. bump 版本号（三处必须一致，当前 +1）：
   - meta.json:   "version": N
   - bundle.json: "version": N
   - builtin.rs:  version: N,  （路径: src/crates/contracts/product-domains/src/miniapp/builtin.rs）
4. cargo check -p bitfun-product-domains
5. 重启 pnpm run desktop:dev（或 touch builtin.rs 触发 Rust 重编译）让新 bundle 生效
```

### 构建原理

`build-bitfun.mjs` 使用 esbuild 从 `ui.js` 入口打包所有 `src/*.js` 和 npm 依赖
（`pptxgenjs`、`pdf-lib`、`jszip`），单次产出最终的 `dist/ui.bundle.js`。

**不存在中间 bundle**。历史上曾有一个 `vendor/ppt-export.bundle.mjs` 中间产物
和单独的 `build-vendor-bundle.mjs` 脚本，已于 2025 年移除。现在所有依赖在
`build-bitfun.mjs` 单次构建中统一解析和内联。

> **为什么需要 bump 版本号？**
> `builtin.rs` 用 `include_str!` 将 `dist/ui.bundle.js` 嵌入 Rust 二进制。
> 版本号变化会触发 Rust 重新编译，从而重新读取更新后的 JS 文件。
> 如果只改了 JS 但不 bump 版本号，Rust 可能不会重新编译，运行时仍用旧 JS。

## 导出管线

PPT Live 只有一条 PPTX 导出管线：HTML 幻灯片和旧 element-model 幻灯片都先归一化为
`EditableSlideScene`，再由 `buildSlideFromScene(scene, pres)` 这个唯一 serializer
映射为 PowerPoint 原生文本、形状、线、表格和有明确用户图片意图的 picture。

`EditableSlideScene` 不接受栅格兜底、整页截图、SVG 图片层或降级成功状态。可确定重写的
CSS/SVG 构造会转换为原生节点；其余无法表示的输入由导出降级层（`export-degrade.js`）处理：
不支持的样式（box-shadow、text-shadow、filter、mask、background-image、动画等）被剥离，
无法表示的元素被移除，仍失败的页面被替换为简化可编辑场景——所有降级都会记录在导出摘要中，
单个元素或单页问题不再阻断整个导出。表格通过 `addTable` 写为原生 `a:tbl`，生成的几何不得产生
`p:pic` 或媒体关系。

DOM 几何来自 `getBoundingClientRect()`（border-box），以 `px / 96` 转换为英寸。
CSS padding 会成为文本框或表格单元格 inset；垂直对齐由可表示的布局属性推导。

关键设计：
- `WIDTH_SAFETY_IN = 0.15"` — 文本框加宽 0.15 英寸以吸收浏览器与 PowerPoint
  之间的字体度量差异，防止 CJK 文字错误换行
- `safeTextBoxGeometry()` — 根据文字对齐方式调整 x 坐标：
  - `left`: x 不变（多出的宽度在右侧）
  - `right`: x 左移 safety（保持右边缘不变）
  - `center`: x 左移 safety/2（保持中心不变）

### 编排流程（`export-slide-browser.js`）

```
prepareEditableSlides(slides, options)
  → loadHtmlInExportRoot(html)     // 挂载到离屏 shadow-DOM div (1280×720)
  → sanitizeSlideDocumentRoot(doc) // 净化 HTML
  → waitForExportPaint()           // 等待两帧渲染
  → normalizeWithDegradation(...)  // 严格 normalize + 有界降级修复循环
  → buildSimplifiedEditableScene() // 单页最终兜底（保留页数与文字）
  → 返回 EditableSlideScene[]
```

`options.onDegrade(record)` 逐条收集降级记录（剥样式 / 移除元素 / 简化页面），
由 `summarizePptxExportDiagnostics(scenes, degradations)` 合并进导出摘要展示给用户。

旧 element-model 页面通过 `normalizeElementSlideToEditableScene` 进入同一 scene 契约。
`exportEditablePptx(deck, scenes)` 逐页调用唯一 serializer 生成最终 PPTX；流程为顺序执行，
不插入人工延时。

## 版本号协议

修改任何源码或资源后必须 bump 版本号。三个文件必须同步更新：

| 文件 | 字段 |
|------|------|
| `meta.json` | `"version": N` |
| `bundle.json` | `"version": N` |
| `builtin.rs` (Rust) | `version: N,` |

## npm 依赖

| 包 | 用途 |
|----|------|
| `pptxgenjs` | PPTX 生成 |
| `pdf-lib` | PDF 合并 |
| `jszip` | PNG 打包 |

这些包在 `build-bitfun.mjs` 打包时从 `node_modules` 解析并内联到最终 bundle 中。
运行时不需要 `node_modules`——所有依赖已经编译进 `dist/ui.bundle.js`。

## 单位换算速查

| 换算 | 公式 |
|------|------|
| px → inch | `px / 96` |
| px → pt | `px * 0.75` |
| inch → EMU | `inch * 914400` |
| 幻灯片尺寸 | 1280×720 px = 13.333"×7.5" (LAYOUT_WIDE) |
