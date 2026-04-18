# Live App API 参考

此文档定义 AI 生成的 Live App 代码中可用的全部 API，供 Agent 工具 system prompt 或调试时参考。

> **实际全局对象为 `window.app`**（非 `window.__BITFUN__`），以下各节均基于 `window.app`。

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

Live App 运行在 iframe 中，完整支持:
- DOM、CSS（含 CSS 变量 `--bitfun-bg`, `--bitfun-text`, `--bitfun-accent` 等）
- Canvas 2D / WebGL
- Web Audio
- LocalStorage / SessionStorage（iframe 级隔离）
- `navigator.clipboard`（通过 `app.clipboard.*` 代理，绕过 sandbox 限制）

## `window.app` — 全局 Runtime Adapter

Live App 中所有与宿主通信的 API 均通过 `window.app` 暴露。

### 基本属性

```javascript
app.appId        // string — 当前 Live App 的 ID
app.appDataDir   // string — 应用数据目录绝对路径
app.workspaceDir // string — 当前工作区路径
app.theme        // 'dark' | 'light' — 当前主题
app.platform     // 'win32' | 'darwin' | 'linux'
app.mode         // 'hosted'
```

### `app.fs.*` — 文件系统

需在 `permissions.fs` 中声明读写范围。

| 方法 | 签名 | 说明 |
|------|------|------|
| `readFile` | `(path, opts?) → Promise<string>` | opts: `{ encoding: 'utf-8' \| 'base64' }` |
| `writeFile` | `(path, data, opts?) → Promise<void>` | opts: `{ encoding: 'utf-8' \| 'base64' }` |
| `appendFile` | `(path, data) → Promise<void>` | |
| `readdir` | `(path, opts?) → Promise<string[]>` | opts: `{ withFileTypes: boolean }` |
| `mkdir` | `(path, opts?) → Promise<void>` | opts: `{ recursive: boolean }` |
| `rm` | `(path, opts?) → Promise<void>` | opts: `{ recursive: boolean, force: boolean }` |
| `stat` | `(path) → Promise<Stats>` | `{ size, isFile, isDirectory, mtime, ctime }` |
| `copyFile` | `(src, dst) → Promise<void>` | |
| `rename` | `(oldPath, newPath) → Promise<void>` | |

### `app.storage.*` — KV 持久化存储

无权限要求，数据存储在 `{appdata}/storage.json`。

```javascript
await app.storage.set('myKey', { foo: 'bar' });
const value = await app.storage.get('myKey'); // { foo: 'bar' }
```

### `app.dialog.*` — 系统对话框

```javascript
const path = await app.dialog.open({
  title: '选择文件',
  multiple: false,
  filters: [{ name: 'SVG', extensions: ['svg'] }]
});
const savePath = await app.dialog.save({ title: '保存', defaultPath: 'output.svg' });
await app.dialog.message({ title: '提示', message: '操作成功' });
```

### `app.shell.*` — Shell 命令执行

需在 `permissions.shell.allow` 中声明命令白名单。

```javascript
const result = await app.shell.exec('git log --oneline -10', { cwd: app.workspaceDir });
```

### `app.net.*` — 网络请求（Worker 侧）

需在 `permissions.net.allow` 中声明域名白名单。

```javascript
const data = await app.net.fetch('https://api.example.com/data', { method: 'GET' });
```

### `app.os.*` — 系统信息

```javascript
const info = await app.os.info(); // { platform, homedir, tmpdir, ... }
```

### `app.call(method, params)` — 调用 Worker 方法

调用 `source/worker.js` 中导出的函数。

```javascript
const result = await app.call('myWorkerMethod', { key: 'value' });
```

---

## `app.ai.*` — AI 接口（v2）

直接复用宿主应用的 AI Client，无需配置 API Key。需在 `permissions.ai` 中声明。

### `app.ai.complete(prompt, opts?)` — 单次补全

返回完整文本，适合一次性生成场景。

```javascript
const result = await app.ai.complete('生成一个设置图标的 SVG，viewBox 24x24，线性风格', {
  systemPrompt: '你是一个图标设计专家，只输出 SVG 代码，不含任何说明文字。',
  model: 'fast',        // 'primary' | 'fast' | 具体 model_id，默认 'primary'
  maxTokens: 4096,
  temperature: 0.7,
});
console.log(result.text);   // SVG 字符串
console.log(result.usage);  // { promptTokens, completionTokens, totalTokens }
```

### `app.ai.chat(messages, opts?)` — 流式对话

支持多轮对话和流式输出，适合交互式生成场景。

```javascript
const handle = await app.ai.chat(
  [
    { role: 'user', content: '设计一个首页图标，圆角风格，24px 网格' }
  ],
  {
    systemPrompt: '你是图标设计专家，生成符合设计规范的 SVG 代码。',
    model: 'primary',
    onChunk: ({ text, reasoningContent }) => {
      // 实时更新预览
      if (text) appendToPreview(text);
    },
    onDone: ({ fullText, usage }) => {
      // 完成后处理完整结果
      const svg = extractSvg(fullText);
      renderIcon(svg);
    },
    onError: ({ message }) => {
      console.error('AI error:', message);
    },
  }
);

// 取消流式请求
cancelButton.onclick = () => handle.cancel();

// handle.streamId — 当前流的唯一 ID
```

### `app.ai.getModels()` — 查询可用模型

返回当前 Live App 权限范围内可用的模型列表（不含 API Key 等敏感信息）。

```javascript
const models = await app.ai.getModels();
// [{ id: 'gpt4o', name: 'GPT-4o', provider: 'openai', isDefault: true }, ...]
```

### `app.ai.cancel(streamId)` — 取消流式请求

```javascript
await app.ai.cancel(handle.streamId);
```

### AI 权限声明

```json
{
  "permissions": {
    "ai": {
      "enabled": true,
      "allowed_models": ["primary", "fast"],
      "max_tokens_per_request": 8192,
      "rate_limit_per_minute": 30
    }
  }
}
```

- `allowed_models`：可用模型引用列表，支持 `"primary"`、`"fast"` 及具体 model_id；为空则允许所有模型
- `max_tokens_per_request`：单次请求最大输出 token 数
- `rate_limit_per_minute`：每分钟最大请求次数（按 app 计数）

---

## `app.clipboard.*` — 剪贴板

通过宿主代理，绕过 iframe sandbox 的 clipboard 限制。

```javascript
await app.clipboard.writeText('Hello World');
const text = await app.clipboard.readText();
```

---

## 生命周期钩子

```javascript
app.onActivate(() => { /* Tab 变为活跃状态 */ });
app.onDeactivate(() => { /* Tab 切走 */ });
app.onThemeChange((payload) => {
  // payload: { type: 'dark'|'light', vars: { '--bitfun-bg': '...', ... } }
});
```

## 自定义事件

```javascript
app.on('myEvent', (payload) => { /* 处理事件 */ });
app.off('myEvent', handler);
```

---

## `app.dialog.*` — 系统对话框（详细）

### `app.dialog.open`

```javascript
const filePath = await app.dialog.open({
  title: '选择文件',
  directory: false,           // true 选目录
  multiple: false,            // true 多选
  filters: [
    { name: 'Images', extensions: ['png', 'jpg', 'webp'] }
  ]
});
```

### `app.dialog.save`

```javascript
const savePath = await app.dialog.save({
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
    "ai": {
      "enabled": true,
      "allowed_models": ["primary", "fast"],
      "max_tokens_per_request": 8192,
      "rate_limit_per_minute": 30
    },
    "node": {
      "enabled": true,
      "timeout_ms": 30000
    }
  }
}
```

路径变量:
- `{appdata}` — `{user_data_dir}/liveapps/{app_id}/data/`，始终可读写
- `{workspace}` — 当前打开的工作区路径
- `{user-selected}` — 用户通过 app.dialog.open/save 选择的路径
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
