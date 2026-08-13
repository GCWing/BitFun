# 快捷键（Keyboard Shortcuts）需求文档

> 状态：能力规格 / 需求
> 仓库：BitFun-OHOS
> 相关架构入口：
> - [`src/web-ui/src/infrastructure/services/ShortcutManager.ts`](../../src/web-ui/src/infrastructure/services/ShortcutManager.ts)
> - [`src/web-ui/src/shared/types/shortcut.ts`](../../src/web-ui/src/shared/types/shortcut.ts)
> - [`src/web-ui/src/shared/constants/shortcuts.ts`](../../src/web-ui/src/shared/constants/shortcuts.ts)
> - [`src/web-ui/AGENTS.md`](../../src/web-ui/AGENTS.md)

## 背景与需求描述

桌面端需要一个统一的快捷键体系：用户通过键盘高效触发面板切换、搜索、文件树操作、编辑器命令、终端复制粘贴等。BitFun 已有 `ShortcutManager` 单例 + `useShortcut` hook + 快捷键目录（`EDITOR_SHORTCUTS` / `FILETREE_SHORTCUTS` 等）+ 用户自定义覆盖（`app.keybindings`）+ 作用域（app / chat / editor / canvas / terminal）+ 冲突检测 + 设置 UI（`KeyboardShortcutsTab`）。

本需求定义快捷键能力的目标范围与行为契约，确保体系一致、可定制、不冲突、跨平台、IME 安全。

当前缺口与诉求：

- 快捷键须有目录默认值，用户可覆盖，部分关键键不可自定义（`NON_USER_CUSTOMIZABLE_SHORTCUT_IDS`）；
- 作用域须正确隔离：app 全局、editor / canvas / terminal 局部，焦点在终端时不继承 canvas；
- 跨平台：macOS Command ↔ 其他平台 Ctrl 自动映射，目录默认平台无关；
- IME 占用时不触发 Escape 类快捷键；
- Monaco 编辑器管理自己的 keybindings，不纳入产品快捷键体系；
- 调试快捷键（DevTools）旁路产品体系，开发工具不被用户配置影响。

## 期望行为

### 1. 目录与默认值

- 快捷键目录（`shortcuts.ts`）登记 id / 默认组合 / 作用域 / 是否允许输入态触发 / 是否可自定义；
- `useShortcut(id, config, callback)` 注册，自动清理；仅 id 或 config 变化才重新注册；
- 启动期 keybindings 走 bootstrap 路径（`global_config.app.keybindings`），不经首窗口 IPC。

### 2. 用户自定义

- 用户覆盖存于 config `app.keybindings`，版本化形态读写；
- `getEffectiveConfig` 合并目录默认与用户覆盖；
- 不可自定义键（`NON_USER_CUSTOMIZABLE_SHORTCUT_IDS`）拒绝覆盖。

### 3. 作用域与冲突

- 作用域经 `data-shortcut-scope` DOM 属性探测（`closest('[data-shortcut-scope]')`），回退 `app`；
- canvas 作用域包含 canvas-scoped 快捷键；终端作用域不继承 canvas；
- app 作用域全局活跃，可与局部作用域互相遮蔽——冲突检测（`checkConflicts`）在注册与设置 UI 中提示；
- 同作用域内同组合冲突时拒绝注册或提示。

### 4. 跨平台与按键解析

- macOS 逻辑 Ctrl 映射为 Command；目录默认平台无关（如 `editor.findInFile`）；
- 数字行用 `event.code`（Digit1–Digit9）保证 Ctrl+digit 在 IME 下稳定；
- 括号键按物理 `code` 匹配，保证 Ctrl+Shift+[ 等稳定。

### 5. IME 与 Monaco

- IME 占用时不运行 Escape 类快捷键；
- Monaco 管理自己的 keybindings（`MonacoLspAdapter` 内 `keybindings: [monacoApi.KeyCode.F12]` 等），产品 ShortcutManager 不接管 Monaco 内部。

### 6. 调试快捷键

- DevTools 等调试快捷键旁路产品 ShortcutManager，只在 Tauri 桌面环境注册，开发工具不受用户配置影响（`useDebugInspector`）。

### 7. 设置 UI

- `KeyboardShortcutsTab` 展示所有快捷键（含未在运行时注册的）的有效配置，订阅注册 / 覆盖变化刷新；
- 录入新组合、冲突提示、重置默认。

## 非目标 / 范围外

- 不接管 Monaco 编辑器内部 keybindings；
- 不覆盖 CLI/TUI 形态的按键（ratatui/crossterm，属 `src/apps/cli`）；
- 不定义全局 OS 级快捷键（系统级 hotkey）；
- 不在本需求内做快捷键的跨设备云同步（属 remote connect）。

## 建议的落地路径（基于现有分层）

1. **共享类型与常量 (`src/web-ui/src/shared/types/shortcut.ts`、`shared/constants/shortcuts.ts`)** — 快捷键目录、作用域、不可自定义清单。
2. **基础设施 (`src/web-ui/src/infrastructure/services/ShortcutManager.ts` + `hooks/useShortcut.ts`)** — 注册 / 解析 / 冲突检测 / 用户覆盖 / 作用域探测。
3. **设置 UI (`src/web-ui/src/app/scenes/settings/components/KeyboardShortcutsTab.tsx`)** — 配置录入与冲突提示。
4. **Config (`app.keybindings`)** — 版本化用户覆盖存储。
5. **调试 (`src/web-ui/src/infrastructure/debug/useDebugInspector.ts`)** — 旁路产品体系的调试快捷键。

### 分层与依赖边界要点

- UI 组件不直调 Tauri；快捷键纯前端体系，经 `useShortcut` hook 注册；
- 复用 ShortcutManager 单例，不新造第二套键管理；
- keybindings 走 bootstrap 配置路径，不经首窗口 IPC；
- Monaco 内部 keybindings 不纳入产品体系。

## 设计草案 / 参考示例

- **目录参考**：`EDITOR_SHORTCUTS` / `FILETREE_SHORTCUTS`，默认平台无关。
- **作用域参考**：`data-shortcut-scope`（app / chat / editor / canvas / terminal）；终端不继承 canvas。
- **冲突检测参考**：`checkConflicts` 在注册与设置 UI 提示 app-scope 与 scoped 冲突。
- **跨平台参考**：Ctrl→Command 映射、数字行用 `event.code`、括号按物理 `code`。
- **IME 参考**：IME 占用时不触发 Escape 类。
- **调试参考**：`useDebugInspector` 旁路产品体系，仅桌面环境注册。

## 是否愿意贡献

- [x] 我愿意参与开发
- [ ] 我愿意参与讨论和测试
- [ ] 仅提出建议

## 补充说明

- 与终端面板的关系：终端作用域快捷键（复制 / 粘贴 / 切换会话）独立，不继承 canvas。
- 与编辑器的关系：Monaco 内部 keybindings 自管，产品快捷键不接管。
- 与 i18n 的关系：快捷键描述文案（`keyboard.shortcuts.*`）走 i18n 命名空间懒加载。
- 相关分层入口：`src/web-ui/AGENTS.md`。
