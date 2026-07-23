---
id: feature_shortcuts_v0_2_2
trigger: version_first_open
once_per_version: true
delay_ms: 5000
toast_title: "v0.2.2"
toast_desc: 快捷键系统上线，所有快捷键均可自定义
modal_size: lg
completion_action: never_show_again
auto_dismiss_ms: 15000
priority: 5
---

# 快捷键系统

BitFun 内置完整的快捷键系统，覆盖面板切换、场景跳转、编辑器操作等全部常用功能。

**所有快捷键均支持自定义**——你可以按照自己的习惯修改任意绑定，也可以随时恢复默认值。

快捷键分为三个作用域：
- **全局（App）** — 在任何地方都可触发
- **画布（Canvas）** — 仅在编辑器区域生效
- **聊天（Chat）** — 仅在对话输入框生效

<!-- page -->

# 如何设置快捷键

**打开快捷键设置**

1. 按 `Ctrl+,`（Mac 用 `Cmd+,`）打开设置
2. 在左侧导航选择 **键盘** → **快捷键**
3. 在列表中找到你想修改的动作
4. 点击右侧的按键区域，按下新的组合键
5. 按 `Enter` 确认，或按 `Escape` 取消

**提示：** 若新快捷键与现有绑定冲突，系统会提示并询问是否覆盖。

<!-- page -->

# 常用快捷键速查

**全局导航**

| 操作 | Windows / Linux | macOS |
|------|-----------------|-------|
| 打开设置 | `Ctrl+,` | `Cmd+,` |
| 切换左侧面板 | `Ctrl+B` | `Cmd+B` |
| 切换场景 | `Alt+1 / 2 / 3` | `Alt+1 / 2 / 3` |
| 打开 Git | `Ctrl+Shift+G` | `Cmd+Shift+G` |
| 打开终端 | `Ctrl+Shift+\`` | `Cmd+Shift+\`` |

**编辑器**

| 操作 | 快捷键 |
|------|--------|
| 分栏（横向）| `Ctrl+\` |
| 分栏（纵向）| `Ctrl+Shift+\` |
| 关闭标签页 | `Ctrl+W` |
| 任务控制 | `Ctrl+Tab` |
