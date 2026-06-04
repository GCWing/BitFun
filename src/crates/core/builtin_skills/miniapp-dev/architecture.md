# MiniApp 系统架构详解

## 数据流全景

```
AI 对话 → GenerateMiniApp Tool → MiniAppManager::create()
  → storage.rs 持久化 source + meta.json
  → compiler.rs 生成 compiled_html（注入 Bridge）
  → emit miniapp-created 事件
  → 前端 useMiniAppCatalogSync 监听 → 刷新 MiniAppGalleryView
  → 用户点击「打开」→ MiniAppScene → MiniAppRunner
  → <iframe srcDoc={compiled_html}>
  → Bridge Script 拦截 require/fetch → postMessage
  → useMiniAppBridge 路由 → Tauri Commands → Rust 服务
```

## 全局 MiniAppManager

`manager.rs` 使用 `OnceCell<Arc<MiniAppManager>>` 实现全局单例:

```rust
static GLOBAL_MINIAPP_MANAGER: OnceCell<Arc<MiniAppManager>> = OnceCell::new();

// 在 app_state.rs 启动时初始化
initialize_global_miniapp_manager(miniapp_manager.clone());

// Agent 工具中通过此函数获取
try_get_global_miniapp_manager() -> Option<Arc<MiniAppManager>>
```

Workspace path 由 `commands.rs` 中 `open_workspace`/`close_workspace` 同步更新:
```rust
state.miniapp_manager.set_workspace_path(Some(workspace_info.root_path.clone())).await;
```

## 存储结构

```
{user_data_dir}/miniapps/
└── {app_id}/
    ├── meta.json         # MiniAppMeta（不含 source/compiled）
    ├── source/
    │   ├── index.html
    │   ├── style.css
    │   └── script.js
    ├── compiled.html     # 完整可运行 HTML
    └── versions/
        └── v{N}.json     # 历史版本快照
```

## 编译流程 (compiler.rs)

`compile(app_id, source, permissions, theme)` 步骤:

1. `build_csp_content(permissions)` — 基于 net.allow 生成 CSP
2. `build_bridge_script(app_id, config)` — 生成 require/fetch shim
3. `build_scroll_boundary_script()` — 阻止外层滚动穿透
4. 组装 HTML:
   - `<meta http-equiv="Content-Security-Policy">`
   - CSS 变量（`--bg`, `--fg` 等主题变量）
   - 用户 CSS（`<style>` 内联）
   - CDN 依赖（`<script>/<link>` 标签）
   - Bridge Script（在用户 JS 之前）
   - 用户 JS（`<script>` 内联）

## Bridge Builder 详解

`bridge_builder.rs` 生成的 Bridge Script 包含:

### require() shim
- `fs/promises` — 所有方法映射为 `_rpc('fs.{method}', params)`
- `path` — 纯 JS 实现（join/resolve/dirname/basename/extname/parse）
- `child_process` — `exec` 映射为 `_rpc('shell.exec', params)`
- `os` — 纯 JS 实现（platform/homedir/tmpdir/cpus/hostname）
- `crypto` — 映射 `window.crypto`

### fetch() 代理
- `/api/*` 路径 → `_rpc('internal.fetch', ...)`
- `http(s)://` → `_rpc('net.fetch', ...)` 绕过 CORS
- 其他（data: URL 等）→ 原始 fetch

### __BITFUN__ 全局对象
- `appId`, `appDataDir`, `workspaceDir`, `theme`
- `showOpenDialog(opts)` → `_rpc('dialog.open', opts)`
- `showSaveDialog(opts)` → `_rpc('dialog.save', opts)`
- `showMessageBox(opts)` → `_rpc('dialog.message', opts)`

### _rpc() 通道
```javascript
function _rpc(method, params) {
  return new Promise((resolve, reject) => {
    const id = ++_rpcId;
    _pending.set(id, { resolve, reject });
    parent.postMessage({ jsonrpc: '2.0', id, method, params }, '*');
  });
}
```

## Permission Guard 实现

`permission_guard.rs` 核心逻辑:

### 路径权限
1. 规范化路径（canonicalize + 防路径穿越）
2. 将路径变量 `{appdata}`, `{workspace}` 解析为绝对路径
3. 检查请求路径是否在某个已声明 scope 下
4. `{appdata}` 自动允许，`{workspace}` 首次弹窗确认

### Shell 权限
检查命令是否在 `permissions.shell.allow` 白名单中（按命令名前缀匹配）。

### 网络权限
检查请求 URL 域名是否在 `permissions.net.allow` 中（`*` 表示全部允许）。

## Tauri Command 参数约定

所有 Bridge 调用使用统一请求结构:

```rust
#[derive(Deserialize)]
struct MiniAppFsRequest {
    app_id: String,
    op: String,        // "readFile" | "writeFile" | "readdir" | ...
    params: Value,     // JSON 参数
}
```

返回统一 `Result<Value, String>`，前端通过 `invoke()` 调用。

## 前端状态管理

### miniAppStore (Zustand)
```typescript
{
  apps: MiniAppMeta[],               // 画廊列表
  loading: boolean,
  openedAppIds: string[],            // 当前已打开的 MiniApp scene
  runningWorkerIds: string[],        // 当前运行中的 worker
  openApp(id),
  closeApp(id),
  setRunningWorkerIds(ids),
  markWorkerRunning(id),
  markWorkerStopped(id),
}
```

### 事件驱动刷新
`useMiniAppCatalogSync` hook:
- 组件挂载时加载列表和运行中的 worker
- 监听 `miniapp-created`, `miniapp-updated`, `miniapp-deleted`, `miniapp-worker-restarted`, `miniapp-worker-stopped`
- 事件触发时统一刷新 store，导航入口与画廊共享同一份状态

## 工具卡片集成

`MiniAppToolDisplay.tsx` 注册在 `flow_chat/tool-cards/index.ts`:

```typescript
TOOL_CARD_CONFIGS.set('GenerateMiniApp', { icon: Wrench, displayName: '生成 MiniApp', ... });
TOOL_CARD_CONFIGS.set('EditMiniApp', { icon: Edit3, displayName: '编辑 MiniApp', ... });
TOOL_CARD_CONFIGS.set('ListMiniApps', { icon: List, displayName: '列出 MiniApp', ... });
```

卡片支持:
- 流式显示工具调用状态
- 完成后显示应用名/ID/数量
- "Open in Mini App" 按钮直达 `sceneManager.openScene(\`miniapp:${appId}\`)`
