---
id: feature_shortcuts_v0_2_2
trigger: version_first_open
once_per_version: true
delay_ms: 5000
toast_title: "v0.2.2"
toast_desc: 快捷鍵系統上線，所有快捷鍵均可自定義
modal_size: lg
completion_action: never_show_again
auto_dismiss_ms: 15000
priority: 5
---

# 快捷鍵系統

BitFun 內置完整的快捷鍵系統，覆蓋面板切換、場景跳轉、編輯器操作等全部常用功能。

**所有快捷鍵均支持自定義**——你可以按照自己的習慣修改任意綁定，也可以隨時恢復默認值。

快捷鍵分為三個作用域：
- **全局（App）** — 在任何地方都可觸發
- **畫布（Canvas）** — 僅在編輯器區域生效
- **聊天（Chat）** — 僅在對話輸入框生效

<!-- page -->

# 如何設置快捷鍵

**打開快捷鍵設置**

1. 按 `Ctrl+,`（Mac 用 `Cmd+,`）打開設置
2. 在左側導航選擇 **鍵盤** → **快捷鍵**
3. 在列表中找到你想修改的動作
4. 點擊右側的按鍵區域，按下新的組合鍵
5. 按 `Enter` 確認，或按 `Escape` 取消

**提示：** 若新快捷鍵與現有綁定衝突，系統會提示並詢問是否覆蓋。

<!-- page -->

# 常用快捷鍵速查

**全局導航**

| 操作 | Windows / Linux | macOS |
|------|-----------------|-------|
| 打開設置 | `Ctrl+,` | `Cmd+,` |
| 切換左側面板 | `Ctrl+B` | `Cmd+B` |
| 切換場景 | `Alt+1 / 2 / 3` | `Alt+1 / 2 / 3` |
| 打開 Git | `Ctrl+Shift+G` | `Cmd+Shift+G` |
| 打開終端 | `Ctrl+Shift+\`` | `Cmd+Shift+\`` |

**編輯器**

| 操作 | 快捷鍵 |
|------|--------|
| 分欄（橫向）| `Ctrl+\` |
| 分欄（縱向）| `Ctrl+Shift+\` |
| 關閉標籤頁 | `Ctrl+W` |
| 任務控制 | `Ctrl+Tab` |
