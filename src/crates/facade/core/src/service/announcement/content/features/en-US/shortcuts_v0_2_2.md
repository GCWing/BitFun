---
id: feature_shortcuts_v0_2_2
trigger: version_first_open
once_per_version: true
delay_ms: 5000
toast_title: "v0.2.2"
toast_desc: Keyboard shortcuts are here — fully customisable
modal_size: lg
completion_action: never_show_again
auto_dismiss_ms: 15000
priority: 5
---

# Keyboard Shortcut System

BitFun ships with a complete shortcut system covering panel toggles, scene switching, editor operations and more.

**Every shortcut is customisable** — remap anything to match your muscle memory, or restore defaults at any time.

Shortcuts are organised into three scopes:
- **Global (App)** — fire from anywhere, even inside inputs
- **Canvas** — active only when focus is in the editor area
- **Chat** — active only in the conversation input

<!-- page -->

# How to Customise Shortcuts

**Open keyboard settings**

1. Press `Ctrl+,` (or `Cmd+,` on Mac) to open Settings
2. In the left nav, go to **Keyboard** → **Shortcuts**
3. Find the action you want to remap
4. Click the key badge next to it and press your new combination
5. Press `Enter` to confirm or `Escape` to cancel

**Tip:** If the new shortcut conflicts with an existing binding, BitFun will warn you and ask whether to overwrite.

<!-- page -->

# Shortcut Cheat Sheet

**Global Navigation**

| Action | Windows / Linux | macOS |
|--------|-----------------|-------|
| Open Settings | `Ctrl+,` | `Cmd+,` |
| Toggle left panel | `Ctrl+B` | `Cmd+B` |
| Switch scene | `Alt+1 / 2 / 3` | `Alt+1 / 2 / 3` |
| Open Git | `Ctrl+Shift+G` | `Cmd+Shift+G` |
| Open Terminal | `Ctrl+Shift+\`` | `Cmd+Shift+\`` |

**Editor Canvas**

| Action | Shortcut |
|--------|----------|
| Split horizontal | `Ctrl+\` |
| Split vertical | `Ctrl+Shift+\` |
| Close tab | `Ctrl+W` |
| Mission Control | `Ctrl+Tab` |
