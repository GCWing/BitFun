# 内置终端面板（Built-in Terminal Panel）需求文档

> 状态：能力规格 / 需求
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`src/web-ui/src/tools/terminal/`](../../src/web-ui/src/tools/terminal/)
> - [`src/web-ui/AGENTS.md`](../../src/web-ui/AGENTS.md)
> - [`terminal-capability.md`](./terminal-capability.md)

## 背景与需求描述

BitFun 桌面端在工具区提供一个**内置终端面板**，让用户在应用内直接与 shell 交互（区别于 Agent 经 `BashTool` / `ExecCommand` 调用的终端能力——见 `terminal-capability.md`）。面板基于 xterm.js 渲染，消费后端 `terminal-core` 的 PTY / 会话服务。

本需求定义内置终端面板的目标范围与行为契约：多会话 / 多标签、shell 配置、输入输出、resize、回放、外观、快捷键作用域、复制粘贴。

当前缺口与诉求：

- 面板须支持多会话并行与切换，会话可命名 / 配置 profile；
- 输出渲染须高效（懒加载、虚拟化），大量输出不卡顿；
- resize 须去抖与重绘守卫，避免逐帧重排；
- 输入须排队与 IME 安全，粘贴须安全处理（多行 / 控制字符）；
- 外观经 appearance 适配（xterm 颜色 / 字体），与全局主题一致；
- 快捷键在终端作用域内独立，不与编辑器 / 全局冲突；
- 远程工作区下面板连接受控端会话。

## 期望行为

### 1. 会话与标签

- 创建 / 切换 / 关闭终端会话，多会话并行；
- 手动 profile（`manualTerminalProfileService`）：命名 shell / 启动目录 / 环境变量；
- 面板偏好（`terminalPanelPreferenceService`）：默认 profile、布局位置、字号偏好等持久化。

### 2. 渲染与输出

- xterm 渲染（`xtermRendering`）；输出渲染器懒加载（`LazyTerminalOutputRenderer`）减少首屏负担；
- 会话回放（`terminalReplay`）：切换会话 / 重连时回放历史输出，保序不重不漏；
- 大量输出有缓冲与截断策略，不阻塞 UI。

### 3. 输入与粘贴

- 输入队列（`TerminalInputQueue`）：IME 组合期间缓冲，组合结束再提交；
- 粘贴（`terminalPaste`）：多行粘贴确认、控制字符过滤 / 转义，避免粘贴危险控制序列；
- 终端作用域快捷键（`data-shortcut-scope="terminal"`）拦截复制 / 粘贴，不冒泡到全局。

### 4. Resize 与重绘

- resize 去抖（`TerminalResizeDebouncer`）+ 重绘守卫（`resizeRepaintGuard`），避免逐 mousemove / 逐 resize 重排全工作区（吸取 F4 教训）；
- 容器尺寸变化时按 PTY 列 / 行 resize。

### 5. 外观

- xterm 外观经 `XtermAppearanceAdapter` 投影 appearance token（前景 / 背景 / ANSI 调色板 / 字体 / 字号）；
- 外观随主题切换即时刷新；
- 终端字号可独立于全局（如 `DEPLOY_TERMINAL_FONT_SIZE` 先例）。

### 6. 快捷键作用域

- 终端作用域内复制 / 粘贴 / 切换会话等快捷键独立，不继承 canvas / 编辑器作用域；
- IME 占用时 Escape 类快捷键不触发（对齐 ShortcutManager 行为）。

### 7. 远程工作区

- 远程控制另一台桌面时，面板连接受控端会话，输入 / 输出在受控端；
- 在 `remote_workspace_policy` 中声明终端面板相关命令远程策略，不支持时清晰提示。

## 非目标 / 范围外

- 不定义终端服务 / PTY / 会话后端（见 `terminal-capability.md`）；
- 不定义远程 SSH 终端 UI（属 `remote-ssh`）；
- 不覆盖 CLI/TUI 形态终端（属 `src/apps/cli`）；
- 不在本需求内做终端主题包编辑器（属 appearance）；
- 不替换 xterm.js 渲染内核。

## 建议的落地路径（基于现有分层）

1. **Web UI 组件 (`src/web-ui/src/tools/terminal/`)** — 面板主要落点：
   - `components/`（`Terminal.tsx` / `ConnectedTerminal.tsx` / `TerminalOutputRenderer` / `LazyTerminalOutputRenderer`）；
   - `services/`（`TerminalService` / `TerminalActionManager` / profile / preference 服务）；
   - `hooks/useTerminal`、`utils/`（渲染 / resize / 粘贴 / 输入队列 / 回放）。
2. **Web UI appearance 适配 (`src/web-ui/src/infrastructure/appearance/adapters/XtermAppearanceAdapter.ts`)** — xterm 外观投影。
3. **Web UI 快捷键 (`src/web-ui/src/infrastructure/services/ShortcutManager.ts`)** — 终端作用域与冲突检测。
4. **后端 (`src/crates/services/terminal`)** — PTY / 会话服务（见 `terminal-capability.md`）。
5. **App (`src/apps/desktop`)** — Tauri command 装配，远程策略声明。

### 分层与依赖边界要点

- UI 组件不直调 Tauri，走 `infrastructure/api` adapter；
- 复用既有 component-library 与 Zustand store，不新造前端原语；
- 外观 token 走 appearance，颜色审计失败时复用 token / 合并冗余 / 加最小 owner contract，不抬 baseline；
- 远程策略显式声明，不引入对远程工作区路径的隐式依赖。

## 设计草案 / 参考示例

- **渲染参考**：`xtermRendering` + `LazyTerminalOutputRenderer` 懒加载输出。
- **回放参考**：`terminalReplay` 切换 / 重连保序回放。
- **输入参考**：`TerminalInputQueue` IME 缓冲；`terminalPaste` 多行 / 控制字符安全处理。
- **resize 参考**：`TerminalResizeDebouncer` + `resizeRepaintGuard` 避免逐帧重排。
- **外观参考**：`XtermAppearanceAdapter` 投影 appearance token；`DEPLOY_TERMINAL_FONT_SIZE` 独立字号先例。
- **快捷键参考**：`data-shortcut-scope="terminal"` + ShortcutManager 作用域冲突检测。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 与 `terminal-capability.md` 的关系：本需求定义**应用内面板 UI**，后者定义**服务 / 能力**；面板消费后者。
- 与外观的关系：终端外观经 appearance 适配，主题变更走颜色审计。
- 与远程工作区的关系：面板连接受控端会话，远程策略显式声明，不支持时清晰提示。
- 相关分层入口：`src/web-ui/AGENTS.md`、`src/apps/desktop/AGENTS.md`、`src/crates/services/terminal/AGENTS.md`。
