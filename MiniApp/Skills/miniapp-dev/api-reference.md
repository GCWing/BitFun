# MiniApp API 参考

此文档定义 AI 生成的 MiniApp 代码中可用的全部 API，供 Agent 工具 system prompt 或调试时参考。

## 标准 Node.js API（通过 require() shim）

### fs/promises

```javascript
const fs = require('fs/promises');
```

| 方法 | 签名 | 说明 |
|------|------|------|
| `readFile` | `(path, opts?) → Promise<string>` | opts: `{ encoding: 'utf-8' \| 'base64' }` |
| `writeFile` | `(path, data, opts?) → Promise<void>` | opts: `{ encoding: 'utf-8' \| 'base64' }` |
| `appendFile` | `(path, data) → Promise<void>` | |
| `readdir` | `(path, opts?) → Promise<string[]>` | opts: `{ withFileTypes: boolean }` |
| `mkdir` | `(path, opts?) → Promise<void>` | opts: `{ recursive: boolean }` |
| `rmdir` | `(path, opts?) → Promise<void>` | opts: `{ recursive: boolean }` |
| `rm` | `(path, opts?) → Promise<void>` | opts: `{ recursive: boolean, force: boolean }` |
| `stat` | `(path) → Promise<Stats>` | Returns: `{ size, isFile, isDirectory, mtime, ctime }` |
| `lstat` | `(path) → Promise<Stats>` | |
| `access` | `(path) → Promise<void>` | throws if not accessible |
| `copyFile` | `(src, dst) → Promise<void>` | |
| `rename` | `(oldPath, newPath) → Promise<void>` | |
| `unlink` | `(path) → Promise<void>` | |

### path（纯 JS，零延迟）

```javascript
const path = require('path');
```

`join`, `resolve`, `dirname`, `basename`, `extname`, `parse`, `sep`

### child_process

```javascript
const { exec } = require('child_process');
```

| 方法 | 签名 | 说明 |
|------|------|------|
| `exec` | `(cmd, opts?, callback?) → Promise \| void` | opts: `{ cwd, timeout }` |

支持两种调用风格：
- **Promise 风格**：`const result = await exec(cmd, opts)` → 返回 `{ stdout, stderr, exit_code }`
- **Callback 风格**：`exec(cmd, opts, (err, stdout, stderr) => { ... })` → 无返回值

受 `permissions.shell.allow` 命令白名单限制。

### os（纯 JS）

```javascript
const os = require('os');
```

`platform()`, `homedir()`, `tmpdir()`, `cpus()`, `hostname()`

### crypto

```javascript
const crypto = require('crypto');
```

映射 `window.crypto.subtle`，支持 `randomUUID()`。

## 标准浏览器 API

MiniApp 运行在 iframe 中，完整支持:
- DOM、CSS（含 CSS 变量 `--bg`, `--fg`, `--accent`）
- Canvas 2D / WebGL
- Web Audio
- `navigator.clipboard`
- LocalStorage / SessionStorage（iframe 级隔离）

## fetch()（代理增强）

```javascript
// 外部请求 — 通过 Rust reqwest 代理，无 CORS 限制
const res = await fetch('https://api.example.com/data');

// 内部 API — 访问 BitFun 能力
const res = await fetch('/api/ai/complete', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ prompt: '...' })
});
```

### 内部 API 路由

| 路由 | 方法 | 功能 |
|------|------|------|
| `/api/ai/complete` | POST | AI 文本补全 |
| `/api/ai/chat` | POST | 向对话发消息 |
| `/api/storage` | GET/PUT/DELETE | KV 存储 |
| `/api/apps` | GET | 列出其他 MiniApp |
| `/api/apps/:id/send` | POST | 跨 App 通信 |

## __BITFUN__ 全局对象

唯一非标准 API，仅含对话框和环境信息:

```javascript
window.__BITFUN__ = {
  appId: 'uuid',
  appDataDir: '/path/to/app/data',
  workspaceDir: '/path/to/workspace',
  theme: 'dark' | 'light',
  _platform: 'win32' | 'darwin' | 'linux',

  showOpenDialog(opts): Promise<string | string[] | null>,
  showSaveDialog(opts): Promise<string | null>,
  showMessageBox(opts): Promise<string>,
};
```

### showOpenDialog

```javascript
const filePath = await __BITFUN__.showOpenDialog({
  title: '选择文件',
  directory: false,           // true 选目录
  multiple: false,            // true 多选
  filters: [
    { name: 'Images', extensions: ['png', 'jpg', 'webp'] }
  ]
});
```

### showSaveDialog

```javascript
const savePath = await __BITFUN__.showSaveDialog({
  title: '保存文件',
  defaultPath: 'output.png',
  filters: [
    { name: 'PNG', extensions: ['png'] }
  ]
});
```

## 权限声明格式

```json
{
  "permissions": {
    "fs": {
      "read": ["{workspace}", "{appdata}", "{user-selected}"],
      "write": ["{appdata}", "{user-selected}"]
    },
    "shell": {
      "allow": ["git", "ffmpeg"]
    },
    "net": {
      "allow": ["api.example.com", "cdn.jsdelivr.net"]
    },
    "env": false,
    "compute": true
  }
}
```

路径变量:
- `{appdata}` — `{user_data_dir}/miniapps/{app_id}/data/`，始终可读写
- `{workspace}` — 当前打开的工作区路径
- `{user-selected}` — 用户通过 showOpenDialog/showSaveDialog 选择的路径
- `{home}` — 用户主目录（高风险）

## CDN 依赖

通过 `source.dependencies` 声明，编译器自动注入 `<script>/<link>` 标签:

```json
{
  "dependencies": [
    { "url": "https://cdn.jsdelivr.net/npm/fabric@5/dist/fabric.min.js", "type": "script" },
    { "url": "https://cdn.jsdelivr.net/npm/monaco-editor@0.40/min/vs/loader.js", "type": "script" }
  ]
}
```

依赖 URL 的域名必须在 `permissions.net.allow` 中声明。
