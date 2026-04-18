---
name: miniapp-dev
description: Develops and maintains the BitFun MiniApp system (Zero-Dialect Runtime). Use when working on miniapp modules, Mini Apps gallery, bridge scripts, agent tool (InitMiniApp), permission policy, or any code under src/crates/core/src/miniapp/ or src/web-ui/src/app/scenes/miniapps/. Also use when the user mentions MiniApp, miniapps, bridge, or zero-dialect.
---

# BitFun MiniApp V2 开发指南

## 核心哲学：Zero-Dialect Runtime

MiniApp 使用 **标准 Web API + window.app**：UI 侧为 ESM 模块（`ui.js`），后端逻辑在独立 JS Worker 进程（Bun 优先 / Node 回退）中执行。Rust 负责进程管理、权限策略和 Tauri 独占 API；Bridge 从旧的 `require()` shim + `__BITFUN__` 替换为统一的 **window.app** Runtime Adapter。

## 代码架构

### Rust 后端

```
src/crates/core/src/miniapp/
├── types.rs               # MiniAppSource (ui_js/worker_js/esm_dependencies/npm_dependencies), NodePermissions
├── manager.rs             # CRUD + recompile() + resolve_policy_for_app()
├── storage.rs             # ui.js, worker.js, package.json, esm_dependencies.json
├── compiler.rs            # Import Map + Runtime Adapter 注入 + ESM
├── bridge_builder.rs      # window.app 生成 + build_import_map()
├── permission_policy.rs   # resolve_policy() → JSON 策略供 Worker 启动 / host_dispatch 复用
├── host_dispatch.rs       # 宿主直连分发 fs/shell/os/net（无需 Bun/Node Worker）
├── runtime_detect.rs      # detect_runtime() Bun/Node
├── js_worker.rs           # 单进程 stdin/stderr JSON-RPC
├── js_worker_pool.rs      # 池管理 + install_deps
├── exporter.rs            # 导出骨架
└── mod.rs
```

### Tauri Commands

```
src/apps/desktop/src/api/miniapp_api.rs
```

- 应用管理: `list_miniapps`, `get_miniapp`, `create_miniapp`, `update_miniapp`, `delete_miniapp`
- 存储/授权: `get/set_miniapp_storage`, `grant_miniapp_workspace`, `grant_miniapp_path`
- 版本: `get_miniapp_versions`, `rollback_miniapp`
- Worker/Runtime: `miniapp_runtime_status`, `miniapp_worker_call`, `miniapp_host_call`, `miniapp_worker_stop`, `miniapp_install_deps`, `miniapp_recompile`
- 对话框由前端 Bridge 用 Tauri dialog 插件处理，无单独后端命令

### Agent 工具

```
src/crates/core/src/agentic/tools/implementations/
└── miniapp_init_tool.rs   # InitMiniApp — 唯一工具，创建骨架目录供 AI 用通用文件工具编辑
```

注册在 `registry.rs` 的 `register_all_tools()` 中。AI 后续用 Read/Edit/Write 等通用文件工具编辑 MiniApp 文件。

### 前端

```
src/web-ui/src/app/scenes/miniapps/
├── MiniAppGalleryScene.tsx / .scss
├── MiniAppScene.tsx / .scss
├── miniAppStore.ts
├── views/ MiniAppGalleryView
├── components/ MiniAppCard, MiniAppRunner (iframe 带 data-app-id)
├── hooks/
│   ├── useMiniAppBridge.ts        # worker.call → workerCall() + dialog.open/save/message
│   └── useMiniAppCatalogSync.ts   # 列表与运行态同步
└── utils/ miniAppIcons.tsx, buildMiniAppThemeVars.ts

src/web-ui/src/infrastructure/api/service-api/MiniAppAPI.ts  # runtimeStatus, workerCall, workerStop, installDeps, recompile
src/web-ui/src/flow_chat/tool-cards/MiniAppToolDisplay.tsx   # InitMiniAppDisplay
```

### Worker 宿主

```
src/apps/desktop/resources/worker_host.js
```

Node/Bun 标准脚本：从 argv 读策略 JSON，stdin 收 RPC、stderr 回响应，内置 fs/shell/net/os/storage dispatch + 加载用户 `source/worker.js` 自定义方法。

## MiniApp 数据模型 (V2)

```rust
// types.rs
MiniAppSource {
  html, css,
  ui_js,           // 浏览器侧 ESM
  esm_dependencies,
  worker_js,       // Worker 侧逻辑
  npm_dependencies,
}
MiniAppPermissions { fs?, shell?, net?, node? }  // node 替代 env/compute
```

## 权限模型

- **permission_policy.rs**：`resolve_policy(perms, app_id, app_data_dir, workspace_dir, granted_paths)` 生成 JSON 策略，传给 Worker 启动参数；Worker 内部按策略拦截越权。
- 路径变量同前：`{appdata}`, `{workspace}`, `{user-selected}`, `{home}` 等。

## Bridge 通信流程 (V2)

```
iframe 内 window.app.call(method, params)
  → postMessage({ method: 'worker.call', params: { method, params } })
  → useMiniAppBridge 监听
  ├─ 框架原语 (fs.* / shell.* / os.* / net.*)：
  │   ├─ node.enabled = false  → miniAppAPI.hostCall → Tauri invoke('miniapp_host_call')
  │   │                          → bitfun_core::miniapp::host_dispatch（纯 Rust，无需 Bun/Node）
  │   └─ node.enabled = true   → miniAppAPI.workerCall → Tauri invoke('miniapp_worker_call')
  │                              → JsWorkerPool（保留旧路径，允许 worker.js 覆写 fs/shell 等）
  ├─ 自定义方法：始终走 worker.call → JsWorkerPool（要求 node.enabled = true 且 worker.js 导出）
  └─ storage.* (node.enabled = false 时)：直接走 get/set_miniapp_storage 命令

dialog.open / dialog.save / dialog.message
  → postMessage → useMiniAppBridge 直接调 @tauri-apps/plugin-dialog
```

### 何时使用「无 Node 模式」（推荐）

只要小应用的后端能力可以用 `fs.*` / `shell.*` / `os.*` / `net.*` 完成（例如调用 `git` 拉数据、读写工作区文件、抓取 HTTP API），就把 `permissions.node.enabled` 设为 `false`：

- 不依赖 Bun/Node 安装环境，bundle 后即点即用，避免 "JS Worker pool not initialized" 类问题；
- 安全与性能与 Worker 路径完全等价（同一份 `permission_policy`，Rust 直接执行）；
- 仍然可以使用 `app.shell.exec / fs.* / net.fetch / os.info / storage.get|set` 全部框架原语。

什么时候需要 `node.enabled = true`：

- 需要写 `worker.js` 自定义方法（CPU 密集 / 长流程 / 复杂解析等）；
- 需要 `npm_dependencies` 安装第三方 npm 包；
- 需要在 worker 内长期持有连接、缓存、状态。

> 走「无 Node 模式」时，**禁止** 调用 `app.call('myCustomMethod', …)`，宿主会显式报错；只能调用框架原语和 `app.storage.*`。

## 能力边界（重要）

MiniApp 框架**只暴露下列能力**，没有任何"通用 BitFun 后端通道"。设计 / 生成新小应用前请先比对，能力不在表内的需求请走相应替代方案，**不要假设有 `app.bitfun.*` / `app.workspace.*` / `app.git.*` / `app.session.*` 之类的接口存在。**

| 能力 | 入口 | 说明 |
|---|---|---|
| 文件系统 | `app.fs.*` | 受 `permissions.fs.read/write` 路径白名单限制 |
| 子进程 / 命令行 | `app.shell.exec` | 受 `permissions.shell.allow` 命令名白名单限制 |
| HTTP | `app.net.fetch` | 受 `permissions.net.allow` 域名白名单限制 |
| 系统信息 | `app.os.info` | 仅 platform / cpus / homedir / tmpdir 等只读字段 |
| KV 存储 | `app.storage.get/set` | 每个小应用独立的 `storage.json`，跨会话保留 |
| AI | `app.ai.complete / chat / cancel / getModels` | 复用宿主 AIClient，受 `permissions.ai`（含 `allowed_models` / 速率限制） |
| 对话框 | `app.dialog.open/save/message` | Tauri dialog 插件 |
| 剪贴板 | `app.clipboard.readText/writeText` | 宿主 navigator.clipboard |
| 自定义后端 | `app.call('xxx', …)` + `worker.js` | 仅 `node.enabled = true` 时可用，自己实现业务逻辑 |
| 主题 / i18n | `app.theme` / `app.locale` / `app.onThemeChange` / `app.onLocaleChange` / `app.t(...)` | 见对应章节 |

### 框架**不**直接暴露的 BitFun 后端能力（截至本文档）

下面这些 BitFun 内部服务，目前**没有**给小应用开放调用通道：

- WorkspaceService（结构化工作区索引、统一搜索）
- GitService（结构化 status / diff / blame，区别于裸 `git` 命令）
- TerminalService（创建/读写交互式终端）
- Session / AgenticSystem（启动 Agent 会话、消费工具调用与流式事件）
- LSP / Snapshot / Mermaid / Skills / Browser API / Computer Use / Config 等

需要这类能力时的合规姿势：

1. **能用裸命令行解决的**（如 git）→ 在 `permissions.shell.allow` 里加命令名，用 `app.shell.exec` 包一层（参考 `builtin-coding-selfie/ui.js` 的 `scanGitWorkspace`）；
2. **只是要读 BitFun 工作区内的文件**（如某些项目元数据） → 把 `{workspace}` 加到 `permissions.fs.read`，自己用 `app.fs.*` 读 + 在前端解析；
3. **必须真调用某个内部服务** → 暂不支持，先记录到需求池。**不要**自己起一个 worker 去模拟服务行为，会和真正的 service 行为漂移。

> 维护者：以后若新增 `app.bitfun.*` / `app.workspace.*` 这类宿主直通通道，请同步更新本节，避免"文档说没有、代码偷偷加了"的不一致。

## window.app 运行时 API

MiniApp UI 内通过 **window.app** 访问：

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

MiniApp 在 iframe 中运行时自动与主应用主题同步，避免界面风格与主应用差距过大。

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

宿主会将主应用主题映射为以下 CSS 变量并注入 iframe 的 `:root`。在 MiniApp 的 CSS 中建议用 `var(--bitfun-*, <fallback>)` 引用，以便在 BitFun 内与主应用一致，导出为独立应用时 fallback 生效。

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

## 国际化（i18n）

MiniApp 框架在 V2 之后内置 i18n 支持，开发者**必须**为多语言用户考虑两类文案：

1. **Gallery 元数据**（`name` / `description` / `tags`）—— 在 `meta.json` 顶层加 `i18n.locales` 块，宿主 Gallery / Card / Scene 标题自动按当前语言挑选。
2. **应用内文案**（HTML / JS 中的所有可见字符串）—— 通过 `window.app.locale`、`window.app.onLocaleChange(fn)` 与 `window.app.t(table, fallback)` 实现。

### `meta.json` 多语言示例

```json
{
  "id": "your-app",
  "name": "默认名（兜底）",
  "description": "默认描述",
  "tags": ["默认标签"],
  "i18n": {
    "locales": {
      "zh-CN": { "name": "中文名", "description": "中文描述", "tags": ["中文"] },
      "en-US": { "name": "English Name", "description": "English desc", "tags": ["en"] }
    }
  }
}
```

回退顺序：`current` → `en-US` → `zh-CN` → 顶层默认值。

### `window.app` i18n 运行时 API

| 成员 | 说明 |
|------|------|
| `app.locale` | 当前语言 ID（如 `'zh-CN'` / `'en-US'`），随宿主切换更新 |
| `app.onLocaleChange(fn)` | 注册语言切换回调，参数为新 locale 字符串 |
| `app.t(table, fallback)` | 从 `{ 'zh-CN': '...', 'en-US': '...' }` 表挑选字符串；解析顺序：current → en-US → zh-CN → 表的第一项 → fallback |

### HTML 静态文案：`data-i18n` 约定

宿主不强制要求该写法，但推荐 MiniApp 内部统一约定：

- `<span data-i18n="key">默认</span>` —— 切换语言时 `applyStaticI18n()` 读取 `data-i18n` 并替换 `textContent`
- `<div data-i18n="ariaKey" data-i18n-attr="aria-label">...</div>` —— 设置某个属性而非文本

参考 `builtin/assets/gomoku/ui.js` 等内置应用的 `I18N` 表 + `applyStaticI18n()` + `app.onLocaleChange` 三件套即可复用。

### 编写自检清单

- [ ] `meta.json` 已加 `i18n.locales`（至少 `zh-CN` / `en-US`）
- [ ] HTML 中静态文案均带 `data-i18n` 属性
- [ ] JS 内动态拼接的字符串使用 `app.t()` 或自有 `I18N` 表
- [ ] 注册了 `app.onLocaleChange`，切换语言时重新渲染（包括动态列表、aria-label、title）
- [ ] 持久化数据（`app.storage`）保存语言无关的索引/键，而非已翻译的字符串

## 开发约定

### 新增 Agent 工具

当前仅 **InitMiniApp**。若扩展：
1. `implementations/miniapp_xxx_tool.rs` 实现 `Tool`
2. `mod.rs` + `registry.rs` 注册
3. `flow_chat/tool-cards/index.ts` 与 `MiniAppToolDisplay.tsx` 增加对应卡片

### 修改编译器

`compiler.rs`：注入 Import Map（`build_import_map`）、Runtime Adapter（`build_bridge_script`）、CSP；用户脚本以 `<script type="module">` 注入 `ui_js`。

### 前端事件

后端 `miniapp-created` / `miniapp-updated` / `miniapp-deleted` / `miniapp-worker-*`，前端 `useMiniAppCatalogSync` 统一监听并刷新 store。

## 场景注册检查清单

同前：`SceneBar/types.ts`、`scenes/registry.ts`、`SceneViewport.tsx`、`NavPanel/config.ts`、`app/types/index.ts`、locales。

## 参考

- 重构计划: `.cursor/plans/miniapp_v2_full_refactor_*.plan.md`
- 架构说明见 plan 内「MiniApp V2 一步到位重构计划」
