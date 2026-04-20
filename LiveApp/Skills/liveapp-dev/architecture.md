# Live App（灵动应用）系统架构详解

## 数据流全景

```
AI 对话 → InitLiveApp 工具 → LiveAppManager::create()
  → storage.rs 持久化 source + meta.json
  → compiler.rs 生成 compiled_html（注入 window.app Bridge）
  → emit liveapp-created 事件
  → 前端 useLiveAppCatalogSync 监听 → 刷新应用列表
  → 用户点击「打开」→ LiveAppScene → LiveAppRunner
  → <iframe srcDoc={compiled_html}>
  → Bridge：window.app → postMessage
  → useLiveAppBridge 路由 → Tauri live_app_* 命令 → Rust LiveApp 服务
```

## 全局 LiveAppManager

`manager.rs` 使用 `OnceLock<Arc<LiveAppManager>>` 实现全局单例:

```rust
static GLOBAL_LIVE_APP_MANAGER: OnceLock<Arc<LiveAppManager>> = OnceLock::new();

// 在 app_state 启动时初始化
initialize_global_live_app_manager(live_app_manager.clone());

// Agent 工具中通过此函数获取
try_get_global_live_app_manager() -> Option<Arc<LiveAppManager>>
```

Workspace path 由工作区打开/关闭流程同步到 manager（`set_workspace_path`）。

## 存储结构

用户数据根目录为 `liveapps`（若仅有历史目录 `miniapps` 且无 `liveapps`，启动时会一次性重命名迁移）:

```
{user_data_dir}/liveapps/
└── {app_id}/
    ├── meta.json         # LiveAppMeta（不含完整 source 快照说明见 storage）
    ├── source/
    │   ├── index.html, style.css, ui.js, worker.js, package.json, …
    ├── compiled.html     # 可运行 HTML（注入 Bridge）
    └── versions/
        └── v{N}.json     # 历史版本快照
```

## 编译流程 (compiler.rs)

`compile(...)` 负责 Import Map、Runtime Adapter（`window.app`）、CSP、ESM 注入等；详见源码与 `bridge_builder.rs`。

## Bridge Builder（V2）

`bridge_builder.rs` 生成面向 iframe 的 **window.app** 适配层；用户 UI 为 ESM（`ui.js`），逻辑在独立 JS Worker（Bun/Node）中，经 JSON-RPC 与宿主通信。旧版 `require`/`__BITFUN__` shim 已弃用。

## 权限策略

`permission_policy.rs`：`resolve_policy(...)` 生成传给 Worker 启动参数的策略 JSON；Worker 内按策略拦截越权。路径占位符如 `{appdata}`、`{workspace}` 等由策略解析。

## Tauri 命令

前端通过 `invoke` 调用 `src/apps/desktop/src/api/live_app_api.rs` 中注册的命令，例如：`list_live_apps`、`get_live_app`、`create_live_app`、`update_live_app`、`delete_live_app`、`get_live_app_storage` / `set_live_app_storage`、`grant_live_app_workspace` / `grant_live_app_path`、`get_live_app_versions`、`rollback_live_app`、`live_app_runtime_status`、`live_app_worker_call`、`live_app_worker_stop`、`live_app_install_deps`、`live_app_recompile` 等。

## 前端状态管理

### liveAppStore（Zustand）

`src/web-ui/src/app/scenes/apps/live-app/liveAppStore.ts`：应用列表、打开的应用、运行中的 worker 等与灵动应用画廊/场景共享。

### 事件驱动刷新

`useLiveAppCatalogSync`：

- 挂载时加载列表与运行中的 worker  
- 监听 `liveapp-created`、`liveapp-updated`、`liveapp-deleted`、`liveapp-worker-restarted`、`liveapp-worker-stopped`  
- 统一刷新 store  

## 工具卡片集成

`InitLiveAppToolDisplay.tsx` 在 `flow_chat/tool-cards/index.ts` 注册，工具名为 **`InitLiveApp`**：

```typescript
TOOL_CARD_CONFIGS['InitLiveApp'] = { displayName: 'Init Live App', ... };
```

卡片支持流式状态、完成后的应用信息，以及「在灵动应用中打开」等操作（如 `openOverlay(\`live-app:${appId}\`)`）。
