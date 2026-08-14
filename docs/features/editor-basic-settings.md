# 编辑器基本设置（Editor Basic Settings）需求文档

> 状态：能力规格 / 需求
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`src/web-ui/src/tools/editor/config/types.ts`](../../src/web-ui/src/tools/editor/config/types.ts)
> - [`src/web-ui/src/tools/editor/config/defaults.ts`](../../src/web-ui/src/tools/editor/config/defaults.ts)
> - [`src/web-ui/src/tools/editor/services/EditorOptionsBuilder.ts`](../../src/web-ui/src/tools/editor/services/EditorOptionsBuilder.ts)
> - [`src/web-ui/src/tools/editor/core/MonacoEditorCore.tsx`](../../src/web-ui/src/tools/editor/core/MonacoEditorCore.tsx)
> - [`font-size-setting.md`](./font-size-setting.md)

## 背景与需求描述

BitFun 桌面端内置 Monaco 代码编辑器，提供文件查看 / 编辑、跨文件跳转（`EditorJumpService`）、diff（`MonacoDiffCore`）。编辑器选项经 `EditorOptionsBuilder` 由 `config/types` + `config/defaults` 构造，已支持 tabSize、wordWrap、lineNumbers、minimap 等基本项。

本需求定义编辑器**基本设置**的目标范围与行为契约，让用户按习惯配置编辑器，并保证 Agent 编辑工具（`Edit` / `Write`）与用户编辑器选项不互相干扰。

当前缺口与诉求：

- 已有 tabSize / wordWrap / lineNumbers / minimap，但渲染空白、自动保存、括号配对着色、行高、字号、格式化等基本项须明确纳入设置；
- 设置须持久化、跨重启生效、随工作区可覆盖；
- diff 编辑器共享基础选项，须与普通编辑器一致；
- 编辑器选项变化须高效更新（`updateOptions`），不重建实例；
- Agent 的 `Edit` / `Write` 工具不受用户编辑器显示选项影响（显示与执行分离）。

## 期望行为

### 1. 基本设置项

- **缩进**：tabSize（默认 2）、是否空格转 tab；
- **换行**：wordWrap（off / on / wordWrapColumn / bounded）；
- **行号**：lineNumbers（on / off / relative / interval）；
- **缩略图**：minimap（enabled / side / size）；
- **渲染空白**：renderWhitespace（none / boundary / selection / all）；
- **字号 / 行高**：经 appearance（见 `font-size-setting.md`），编辑器独立可调；
- **括号配对着色 / 引导**：bracketPairColorization / guides；
- **自动保存 / 格式化（可选）**：首版可不启用，预留接口。

### 2. 持久化与覆盖

- 设置持久化到 config（如 `editor.*`），跨重启生效；
- 全局默认 + 工作区覆盖（工作区级优先）；
- 切换工作区时按覆盖刷新选项（`updateOptions`），不重建编辑器。

### 3. diff 与跳转

- diff 编辑器共享基础选项（`MonacoDiffCore`，`diffWordWrap` 等基继承）；
- 跨文件跳转（`EditorJumpService` / `EditorReadyManager`）行为不受显示选项影响；
- 行号 / 缩略图变化不破坏跳转定位。

### 4. 显示与执行分离

- 用户编辑器显示选项（行号 / 缩略图 / 换行）不影响 Agent `Edit` / `Write` 工具的执行语义；
- Agent 编辑工具走文件系统与 edit constraint guard，与 Monaco 显示选项解耦。

### 5. 更新与性能

- 选项变化经 `buildEditorOptions` 构造后 `editor.updateOptions(...)` 增量更新；
- 不因选项变化触发全工作区重渲染（吸取 F4 教训）；
- `MonacoEditorCore` 的 `lineNumbers` / `minimap` ref 跟随设置刷新。

## 非目标 / 范围外

- 不定义 LSP 功能（跳转定义 / 重命名 / 诊断，属 LSP owner）；
- 不做完整 VS Code 选项对齐，只覆盖基本阅读 / 编辑项；
- 不定义外观 / 主题（属 appearance）；
- 不覆盖 CLI/TUI 编辑形态；
- 不定义 Agent `Edit` / `Write` 工具执行契约（属 builtin-tools）。

## 建议的落地路径（基于现有分层）

1. **配置类型与默认 (`src/web-ui/src/tools/editor/config/types.ts`、`defaults.ts`)** — 设置项类型与默认值（tabSize / wordWrap / lineNumbers / minimap 等）。
2. **选项构造 (`src/web-ui/src/tools/editor/services/EditorOptionsBuilder.ts`)** — 由 config 构造 Monaco options，含 diff 继承。
3. **编辑器核心 (`src/web-ui/src/tools/editor/core/MonacoEditorCore.tsx`、`MonacoDiffCore.tsx`)** — `updateOptions` 增量刷新，ref 跟随。
4. **外观适配 (`MonacoAppearanceAdapter`)** — 字号 / 字体 / 行高投影（见 `font-size-setting.md`）。
5. **设置 UI (`src/web-ui/src/app/scenes/settings/`)** — 编辑器基本设置控件。
6. **Config** — 持久化 `editor.*`，工作区覆盖。

### 分层与依赖边界要点

- UI 组件不直调 Tauri；编辑器选项纯前端；
- 复用 Monaco + EditorOptionsBuilder，不新造第二套选项体系；
- 外观 token 经 appearance 适配，颜色审计独立；
- 显示选项与 Agent 执行工具解耦；
- 选项变化不触发全工作区逐帧重渲染。

## 设计草案 / 参考示例

- **选项参考**：`EditorOptionsBuilder` 已构造 tabSize / wordWrap / lineNumbers / minimap / lineNumbersMinChars。
- **默认值参考**：`config/defaults.ts`（tabSize: 2、wordWrap: 'off'、lineNumbers: 'on'、minimap 默认）。
- **diff 继承参考**：`MonacoDiffCore` 共享 `showMinimap` / `showLineNumbers` ref。
- **更新参考**：`MonacoEditorCore` 的 `editor.updateOptions(editorOptions)` 增量刷新。
- **跳转参考**：`EditorJumpService` / `EditorReadyManager` 不受显示选项影响。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 与 `font-size-setting.md` 的关系：编辑器字号经 appearance 适配，由字号设置驱动。
- 与 Agent 编辑工具的关系：显示选项与 `Edit` / `Write` 执行解耦，互不干扰。
- 与外观的关系：字体 / 字号 / 行高经 `MonacoAppearanceAdapter`，颜色经主题 token。
- 相关分层入口：`src/web-ui/AGENTS.md`、`docs/architecture/theme-token-optimization.md`。
