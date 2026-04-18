---
name: miniapp-dev
description: Develops and maintains the BitFun Live App system (Zero-Dialect Runtime). Use when working on live_app modules, Live Apps gallery, bridge scripts, agent tool (InitLiveApp), permission policy, or code under src/crates/core/src/live_app/ or src/web-ui/src/app/scenes/apps/live-app/. Also use when the user mentions Live App, liveapps, bridge, or zero-dialect.
---

# BitFun Live App V2 开发指南

## 核心哲学：Zero-Dialect Runtime

Live App（灵动应用）使用 **标准 Web API + window.app**：UI 侧为 ESM 模块（`ui.js`），后端逻辑在独立 JS Worker 进程（Bun 优先 / Node 回退）中执行。Rust 负责进程管理、权限策略和 Tauri 独占 API；Bridge 从旧的 `require()` shim + `__BITFUN__` 替换为统一的 **window.app** Runtime Adapter。

## 代码架构

### Rust 后端

```
src/crates/core/src/live_app/
├── types.rs               # LiveAppSource (ui_js/worker_js/esm_dependencies/npm_dependencies), NodePermissions
├── manager.rs             # CRUD + recompile() + resolve_policy_for_app()
├── storage.rs             # ui.js, worker.js, package.json, esm_dependencies.json
├── compiler.rs            # Import Map + Runtime Adapter 注入 + ESM
├── bridge_builder.rs      # window.app 生成 + build_import_map()
├── permission_policy.rs   # resolve_policy() → JSON 策略供 Worker 启动
├── runtime_detect.rs      # detect_runtime() Bun/Node
├── js_worker.rs           # 单进程 stdin/stderr JSON-RPC
├── js_worker_pool.rs      # 池管理 + install_deps
├── exporter.rs            # 导出骨架
└── mod.rs
```

### Tauri Commands

```
src/apps/desktop/src/api/live_app_api.rs
```

- 应用管理: `list_live_apps`, `get_live_app`, `create_live_app`, `update_live_app`, `delete_live_app`
- 存储/授权: `get/set_live_app_storage`, `grant_live_app_workspace`, `grant_live_app_path`
- 版本: `get_live_app_versions`, `rollback_live_app`
- Worker/Runtime: `live_app_runtime_status`, `live_app_worker_call`, `live_app_worker_stop`, `live_app_install_deps`, `live_app_recompile`
- 对话框由前端 Bridge 用 Tauri dialog 插件处理，部分封装为 `live_app_dialog_*` 等

### Agent 工具

```
src/crates/core/src/agentic/tools/implementations/
└── live_app_init_tool.rs   # InitLiveApp — 创建骨架目录供 AI 用通用文件工具编辑
```

注册在 `registry.rs` 的 `register_all_tools()` 中。AI 后续用 Read/Edit/Write 等通用文件工具编辑 Live App 源文件。

### 前端

```
src/web-ui/src/app/scenes/apps/live-app/
├── liveAppStore.ts
├── components/ LiveAppCard, LiveAppRunner (iframe 带 data-app-id)
├── hooks/
│   ├── useLiveAppBridge.ts        # worker.call → workerCall() + dialog.open/save/message
│   └── useLiveAppCatalogSync.ts   # 列表与运行态同步
└── liveAppIcons.tsx 等

src/web-ui/src/app/scenes/apps/LiveAppScene.tsx / LiveAppScene.scss
src/web-ui/src/infrastructure/api/service-api/LiveAppAPI.ts
src/web-ui/src/flow_chat/tool-cards/InitLiveAppToolDisplay.tsx   # InitLiveAppDisplay，工具名 InitLiveApp
```

### Worker 宿主

```
src/apps/desktop/resources/worker_host.js
```

Node/Bun 标准脚本：从 argv 读策略 JSON，stdin 收 RPC、stderr 回响应，内置 fs/shell/net/os/storage dispatch + 加载用户 `source/worker.js` 自定义方法。

## Live App 数据模型 (V2)

```rust
// types.rs
LiveAppSource {
  html, css,
  ui_js,           // 浏览器侧 ESM
  esm_dependencies,
  worker_js,       // Worker 侧逻辑
  npm_dependencies,
}
LiveAppPermissions { fs?, shell?, net?, node? }  // node 替代 env/compute
```

## 权限模型

- **permission_policy.rs**：`resolve_policy(perms, app_id, app_data_dir, workspace_dir, granted_paths)` 生成 JSON 策略，传给 Worker 启动参数；Worker 内部按策略拦截越权。
- 路径变量同前：`{appdata}`, `{workspace}`, `{user-selected}`, `{home}` 等。

## Bridge 通信流程 (V2)

```
iframe 内 window.app.call(method, params)
  → postMessage({ method: 'worker.call', params: { method, params } })
  → useLiveAppBridge 监听
  → liveAppAPI.workerCall(appId, method, params)
  → Tauri invoke('live_app_worker_call')
  → JsWorkerPool → Worker 进程 stdin → stderr 响应
  → 结果回 iframe

dialog.open / dialog.save / dialog.message
  → postMessage → useLiveAppBridge 直接调 @tauri-apps/plugin-dialog
```

## window.app 运行时 API

Live App UI 内通过 **window.app** 访问：

| API | 说明 |
|-----|------|
| `app.call(method, params)` | 调用 Worker 方法（含 fs/shell/net/os/storage 及用户 worker.js 导出） |
| `app.fs.*` | 封装为 worker.call('fs.*', …) |
| `app.shell.*` | 同上 |
| `app.net.*` | 同上 |
| `app.os.*` | 同上 |
| `app.storage.*` | 同上 |
| `app.dialog.open/save/message` | 由 Bridge 转 Tauri dialog 插件 |
| 生命周期 / 事件 | 见 bridge_builder 生成的适配器 |

## 主题集成

Live App 在 iframe 中运行时自动与主应用主题同步，避免界面风格与主应用差距过大。

### 只读属性与事件

| 成员 | 说明 |
|------|------|
| `app.theme` | 当前主题类型字符串：`'dark'` 或 `'light'`（随主应用切换更新） |
| `app.onThemeChange(fn)` | 注册主题变更回调，参数为 payload：`{ type, id, vars }` |

### data-theme-type 属性

编译后的 HTML 根元素 `<html>` 带有 `data-theme-type="dark"` 或 `"light"`，便于用 CSS 按主题写样式，例如：

```css
[data-theme-type="light"] .panel { background: #f5f5f5; }
[data-theme-type="dark"] .panel { background: #1a1a1a; }
```

### --bitfun-* CSS 变量

宿主会将主应用主题映射为以下 CSS 变量并注入 iframe 的 `:root`。在 Live App 的 CSS 中建议用 `var(--bitfun-*, <fallback>)` 引用，以便在 BitFun 内与主应用一致，导出为独立应用时 fallback 生效。

**背景**

- `--bitfun-bg` — 主背景
- `--bitfun-bg-secondary` — 次级背景（如工具栏、面板）
- `--bitfun-bg-tertiary` — 第三级背景
- `--bitfun-bg-elevated` — 浮层/卡片背景

**文字**

- `--bitfun-text` — 主文字
- `--bitfun-text-secondary` — 次要文字
- `--bitfun-text-muted` — 弱化文字

**强调与语义**

- `--bitfun-accent`、`--bitfun-accent-hover` — 强调色及悬停
- `--bitfun-success`、`--bitfun-warning`、`--bitfun-error`、`--bitfun-info` — 语义色

**边框与元素**

- `--bitfun-border`、`--bitfun-border-subtle` — 边框
- `--bitfun-element-bg`、`--bitfun-element-hover` — 控件背景与悬停

**圆角与字体**

- `--bitfun-radius`、`--bitfun-radius-lg` — 圆角
- `--bitfun-font-sans`、`--bitfun-font-mono` — 无衬线与等宽字体

**滚动条**

- `--bitfun-scrollbar-thumb`、`--bitfun-scrollbar-thumb-hover` — 滚动条滑块

示例（在 `style.css` 中）：

```css
:root {
  --bg: var(--bitfun-bg, #121214);
  --text: var(--bitfun-text, #e8e8e8);
  --accent: var(--bitfun-accent, #60a5fa);
}
body {
  font-family: var(--bitfun-font-sans, system-ui, sans-serif);
  color: var(--text);
  background: var(--bg);
}
```

### 同步时机

- iframe 加载后 bridge 会向宿主发送 `bitfun/request-theme`，宿主回推当前主题变量，iframe 内 `_applyThemeVars` 写入 `:root`。
- 主应用切换主题时，宿主会向 iframe 发送 `themeChange` 事件，bridge 更新变量并触发 `onThemeChange` 回调。

## 开发约定

### 新增 Agent 工具

当前仅 **InitLiveApp**。若扩展：

1. `implementations/live_app_xxx_tool.rs`（或他名）实现 `Tool`
2. `mod.rs` + `registry.rs` 注册
3. `flow_chat/tool-cards/index.ts` 增加对应卡片组件

### 修改编译器

`compiler.rs`：注入 Import Map（`build_import_map`）、Runtime Adapter（`build_bridge_script`）、CSP；用户脚本以 `<script type="module">` 注入 `ui_js`。

### 前端事件

后端 `liveapp-created` / `liveapp-updated` / `liveapp-deleted` / `liveapp-worker-*`，前端 `useLiveAppCatalogSync` 统一监听并刷新 store。

## 场景注册检查清单

同前：`SceneBar/types.ts`、`scenes/registry.ts`、`SceneViewport.tsx`、`NavPanel/config.ts`、`app/types/index.ts`、locales。

## 参考

- 历史重构计划（文件名可能仍含 miniapp）: `.cursor/plans/miniapp_v2_full_refactor_*.plan.md`
- 架构摘要: 同目录 `architecture.md`
